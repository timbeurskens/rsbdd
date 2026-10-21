use std::cell::{Cell, RefCell};
use std::fs::{File, OpenOptions};
use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use hashbrown::HashTable;
use rustc_hash::{FxHashMap, FxHasher};

pub const FALSE: u32 = 0;
pub const TRUE: u32 = 1;

const PAGE_BITS: u32 = 12;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const NODE_BYTES: usize = 12;
const PAGE_BYTES: usize = PAGE_SIZE * NODE_BYTES;
const UNIQUE_SLOT_BYTES: usize = 10;
const CACHE_SLOT_BYTES: usize = 16;
const MIN_CACHE_SLOTS: usize = 1 << 12;
const MAX_CACHE_SLOTS: usize = 1 << 24;
const INVALID: u32 = u32::MAX;

static SWAP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RawNode {
    pub var: u32,
    pub hi: u32,
    pub lo: u32,
}

impl RawNode {
    fn hash(self) -> u32 {
        let mut h = FxHasher::default();
        h.write_u32(self.var);
        h.write_u32(self.hi);
        h.write_u32(self.lo);
        h.finish() as u32
    }
}

const fn spread(h: u32) -> u64 {
    (h as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

struct Page {
    nodes: Option<Box<[RawNode]>>,
    dirty: bool,
    referenced: Cell<bool>,
}

#[derive(Clone, Copy)]
struct CacheEntry {
    op: u32,
    a: u32,
    b: u32,
    result: u32,
}

const EMPTY_ENTRY: CacheEntry = CacheEntry {
    op: INVALID,
    a: 0,
    b: 0,
    result: 0,
};

struct Swap {
    file: File,
    path: PathBuf,
}

impl Drop for Swap {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct Store {
    pages: RefCell<Vec<Page>>,
    resident: Cell<usize>,
    clock: Cell<usize>,
    len: Cell<u32>,
    free: RefCell<Vec<u64>>,
    free_count: Cell<usize>,
    free_cursor: Cell<usize>,
    unique: RefCell<HashTable<(u32, u32)>>,
    unique_capacity: Cell<usize>,
    cache: RefCell<Vec<CacheEntry>>,
    roots: RefCell<FxHashMap<u32, u32>>,
    limit: Cell<usize>,
    next_gc: Cell<usize>,
    swap: RefCell<Option<Swap>>,
}

impl Store {
    pub fn new() -> Self {
        let store = Self {
            pages: RefCell::default(),
            resident: Cell::new(0),
            clock: Cell::new(0),
            len: Cell::new(0),
            free: RefCell::default(),
            free_count: Cell::new(0),
            free_cursor: Cell::new(0),
            unique: RefCell::default(),
            unique_capacity: Cell::new(0),
            cache: RefCell::new(vec![EMPTY_ENTRY; MIN_CACHE_SLOTS]),
            roots: RefCell::default(),
            limit: Cell::new(usize::MAX),
            next_gc: Cell::new(usize::MAX),
            swap: RefCell::default(),
        };
        store.alloc(RawNode::default());
        store.alloc(RawNode::default());
        store
    }

    pub fn set_limit(&self, bytes: usize) {
        self.limit.set(bytes);
        self.next_gc.set(0);
    }

    pub const fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Number of allocated nodes, including the two constants.
    pub const fn size(&self) -> usize {
        self.len.get() as usize - self.free_count.get()
    }

    /// Estimated number of bytes used by resident pages, the unique table and the operation cache.
    pub fn memory(&self) -> usize {
        self.resident.get() * PAGE_BYTES
            + self.unique_capacity.get() * UNIQUE_SLOT_BYTES
            + self.cache.borrow().len() * CACHE_SLOT_BYTES
            + self.free.borrow().len() * 8
    }

    #[inline]
    pub fn node(&self, id: u32) -> RawNode {
        let (p, i) = ((id >> PAGE_BITS) as usize, id as usize & (PAGE_SIZE - 1));
        {
            let pages = self.pages.borrow();
            let page = &pages[p];
            if let Some(nodes) = &page.nodes {
                page.referenced.set(true);
                return nodes[i];
            }
        }
        self.page_in(p);
        self.node(id)
    }

    pub fn mk(&self, var: u32, hi: u32, lo: u32) -> u32 {
        if hi == lo {
            return hi;
        }

        let node = RawNode { var, hi, lo };
        let h = node.hash();
        let found = self
            .unique
            .borrow()
            .find(spread(h), |&(id, nh)| nh == h && self.node(id) == node)
            .map(|&(id, _)| id);
        if let Some(id) = found {
            return id;
        }

        let id = self.alloc(node);
        let mut unique = self.unique.borrow_mut();
        if unique.len() == unique.capacity() {
            self.make_room(2 * unique.capacity().max(1) * UNIQUE_SLOT_BYTES);
        }
        unique.insert_unique(spread(h), (id, h), |&(_, nh)| spread(nh));
        self.unique_capacity.set(unique.capacity());
        id
    }

    fn alloc(&self, node: RawNode) -> u32 {
        let id = self.take_free().unwrap_or_else(|| {
            let id = self.len.get();
            assert!(id < INVALID, "the BDD exceeds the maximum number of nodes");
            if (id as usize).is_multiple_of(PAGE_SIZE) {
                self.new_page();
            }
            self.len.set(id + 1);
            id
        });

        let (p, i) = ((id >> PAGE_BITS) as usize, id as usize & (PAGE_SIZE - 1));
        if self.pages.borrow()[p].nodes.is_none() {
            self.page_in(p);
        }
        let mut pages = self.pages.borrow_mut();
        let page = &mut pages[p];
        page.dirty = true;
        page.referenced.set(true);
        page.nodes.as_mut().expect("page is resident")[i] = node;
        id
    }

    fn take_free(&self) -> Option<u32> {
        if self.free_count.get() == 0 {
            return None;
        }
        let mut free = self.free.borrow_mut();
        let mut w = self.free_cursor.get();
        while free[w] == 0 {
            w += 1;
        }
        let b = free[w].trailing_zeros() as usize;
        free[w] &= free[w] - 1;
        self.free_cursor.set(w);
        self.free_count.set(self.free_count.get() - 1);
        Some((w * 64 + b) as u32)
    }

    fn new_page(&self) {
        self.grow_cache();
        self.make_room(PAGE_BYTES);
        self.pages.borrow_mut().push(Page {
            nodes: Some(vec![RawNode::default(); PAGE_SIZE].into_boxed_slice()),
            dirty: true,
            referenced: Cell::new(true),
        });
        self.resident.set(self.resident.get() + 1);
    }

    fn grow_cache(&self) {
        let max = MAX_CACHE_SLOTS.min(self.limit.get() / 8 / CACHE_SLOT_BYTES);
        let mut cache = self.cache.borrow_mut();
        if cache.len() < self.size() && cache.len() * 2 <= max {
            *cache = vec![EMPTY_ENTRY; cache.len() * 2];
        }
    }

    fn make_room(&self, bytes: usize) {
        while self.memory() + bytes > self.limit.get() && self.evict_one() {}
    }

    /// Evict a single page using the clock (second chance) algorithm.
    fn evict_one(&self) -> bool {
        let mut pages = self.pages.borrow_mut();
        let current = (self.len.get().saturating_sub(1) >> PAGE_BITS) as usize;
        for _ in 0..2 * pages.len() {
            let p = self.clock.get();
            self.clock.set((p + 1) % pages.len());

            let page = &mut pages[p];
            if p == current || page.nodes.is_none() {
                continue;
            }
            if page.referenced.replace(false) {
                continue;
            }

            let nodes = page.nodes.take().expect("page is resident");
            if page.dirty {
                self.write_page(p, &nodes);
                page.dirty = false;
            }
            self.resident.set(self.resident.get() - 1);
            return true;
        }
        false
    }

    fn write_page(&self, p: usize, nodes: &[RawNode]) {
        let mut buf = Vec::with_capacity(PAGE_BYTES);
        for n in nodes {
            buf.extend_from_slice(&n.var.to_le_bytes());
            buf.extend_from_slice(&n.hi.to_le_bytes());
            buf.extend_from_slice(&n.lo.to_le_bytes());
        }

        let mut swap = self.swap.borrow_mut();
        let swap = swap.get_or_insert_with(|| {
            let path = std::env::temp_dir().join(format!(
                "rsbdd-{}-{}.swap",
                std::process::id(),
                SWAP_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap_or_else(|e| panic!("cannot create swap file {}: {e}", path.display()));
            let _ = std::fs::remove_file(&path);
            Swap { file, path }
        });
        swap.file
            .seek(SeekFrom::Start((p * PAGE_BYTES) as u64))
            .and_then(|_| swap.file.write_all(&buf))
            .unwrap_or_else(|e| panic!("cannot write to swap file {}: {e}", swap.path.display()));
    }

    fn page_in(&self, p: usize) {
        self.make_room(PAGE_BYTES);

        let mut buf = vec![0u8; PAGE_BYTES];
        {
            let mut swap = self.swap.borrow_mut();
            let swap = swap.as_mut().expect("evicted page without swap file");
            swap.file
                .seek(SeekFrom::Start((p * PAGE_BYTES) as u64))
                .and_then(|_| swap.file.read_exact(&mut buf))
                .unwrap_or_else(|e| {
                    panic!("cannot read from swap file {}: {e}", swap.path.display())
                });
        }

        let word = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let nodes = buf
            .as_chunks::<NODE_BYTES>()
            .0
            .iter()
            .map(|c| RawNode {
                var: word(&c[0..4]),
                hi: word(&c[4..8]),
                lo: word(&c[8..12]),
            })
            .collect();

        let mut pages = self.pages.borrow_mut();
        pages[p].nodes = Some(nodes);
        pages[p].referenced.set(true);
        self.resident.set(self.resident.get() + 1);
    }

    pub fn cache_get(&self, op: u32, a: u32, b: u32) -> Option<u32> {
        let cache = self.cache.borrow();
        let e = cache[Self::cache_slot(op, a, b, cache.len())];
        (e.op == op && e.a == a && e.b == b).then_some(e.result)
    }

    pub fn cache_put(&self, op: u32, a: u32, b: u32, result: u32) {
        let mut cache = self.cache.borrow_mut();
        let slot = Self::cache_slot(op, a, b, cache.len());
        cache[slot] = CacheEntry { op, a, b, result };
    }

    fn cache_slot(op: u32, a: u32, b: u32, len: usize) -> usize {
        let mut h = FxHasher::default();
        h.write_u32(op);
        h.write_u32(a);
        h.write_u32(b);
        (spread(h.finish() as u32) >> 32) as usize & (len - 1)
    }

    pub fn root(&self, id: u32) {
        if id > TRUE {
            *self.roots.borrow_mut().entry(id).or_insert(0) += 1;
        }
    }

    pub fn unroot(&self, id: u32) {
        if id > TRUE {
            let mut roots = self.roots.borrow_mut();
            let count = roots.get_mut(&id).expect("unrooting an unknown node");
            *count -= 1;
            if *count == 0 {
                roots.remove(&id);
            }
        }
    }

    /// Collect garbage if the number of allocated nodes exceeds the budget given by the memory limit.
    /// Must only be called when every node in use is rooted.
    pub fn gc_point(&self) {
        if self.size() >= self.next_gc.get() && self.memory() + PAGE_BYTES > self.limit.get() {
            self.gc();
        }
    }

    /// Free all nodes that are not reachable from a root.
    /// Must only be called when every node in use is rooted.
    pub fn gc(&self) {
        let mut marked = vec![0u64; (self.len.get() as usize).div_ceil(64)];
        let is_marked = |m: &[u64], id: u32| m[id as usize / 64] & (1u64 << (id % 64)) != 0;
        let mut stack: Vec<u32> = self.roots.borrow().keys().copied().collect();
        marked[0] |= 0b11;

        while let Some(id) = stack.pop() {
            if !is_marked(&marked, id) {
                marked[id as usize / 64] |= 1u64 << (id % 64);
                let n = self.node(id);
                stack.extend([n.hi, n.lo].into_iter().filter(|&c| !is_marked(&marked, c)));
            }
        }

        self.unique
            .borrow_mut()
            .retain(|&mut (id, _)| is_marked(&marked, id));

        let len = self.len.get() as usize;
        let live: usize = marked.iter().map(|w| w.count_ones() as usize).sum();
        for w in &mut marked {
            *w = !*w;
        }
        if !len.is_multiple_of(64) {
            marked[len / 64] &= (1u64 << (len % 64)) - 1;
        }
        *self.free.borrow_mut() = marked;
        self.free_count.set(len - live);
        self.free_cursor.set(0);
        self.cache.borrow_mut().fill(EMPTY_ENTRY);

        self.make_room(2 * live * UNIQUE_SLOT_BYTES);
        let mut unique = self.unique.borrow_mut();
        unique.shrink_to_fit(|&(_, h)| spread(h));
        self.unique_capacity.set(unique.capacity());
        drop(unique);

        self.next_gc.set(2 * self.size());
    }
}
