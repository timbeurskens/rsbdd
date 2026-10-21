use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHasher};

use crate::store::{FALSE, RawNode, Store, TRUE};
use crate::{BDDSymbol, NamedSymbol, TruthTableEntry};

#[macro_export]
macro_rules! bdd {
    ($($expr:tt)+) => {
        (|| -> anyhow::Result<_> {
            let input = stringify!($($expr)+);
            let mut input_reader = std::io::BufReader::new(input.as_bytes());
            let parsed_formula = rsbdd::parser::ParsedFormula::new(&mut input_reader, None)?;

            Ok(parsed_formula.eval())
        })()
    };
}

struct Inner<S: BDDSymbol> {
    store: Store,
    vars: RefCell<Vec<S>>,
    var_ids: RefCell<FxHashMap<S, u32>>,
}

/// A reference to a node in a [`BDDEnv`]. Nodes stay alive as long as a reference to them exists.
pub struct BDD<S: BDDSymbol> {
    id: u32,
    env: Rc<Inner<S>>,
}

/// A single level of a [`BDD`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node<S: BDDSymbol> {
    False,
    True,
    /// Choice (true-subtree, symbol, false-subtree)
    Choice(BDD<S>, S, BDD<S>),
}

impl<S: BDDSymbol> BDD<S> {
    fn new(id: u32, env: &Rc<Inner<S>>) -> Self {
        env.store.root(id);
        Self {
            id,
            env: Rc::clone(env),
        }
    }

    pub fn node(&self) -> Node<S> {
        match self.id {
            FALSE => Node::False,
            TRUE => Node::True,
            id => {
                let n = self.env.store.node(id);
                Node::Choice(
                    Self::new(n.hi, &self.env),
                    self.env.vars.borrow()[n.var as usize].clone(),
                    Self::new(n.lo, &self.env),
                )
            }
        }
    }

    pub const fn is_choice(&self) -> bool {
        self.id > TRUE
    }

    pub const fn is_const(&self) -> bool {
        !self.is_choice()
    }

    pub const fn is_true(&self) -> bool {
        self.id == TRUE
    }

    pub const fn is_false(&self) -> bool {
        self.id == FALSE
    }

    pub fn node_list(&self) -> Vec<Self> {
        match self.node() {
            Node::Choice(l, _, r) => l
                .node_list()
                .into_iter()
                .chain([self.clone()])
                .chain(r.node_list())
                .collect(),
            Node::True | Node::False => vec![self.clone()],
        }
    }

    pub fn get_hash(&self) -> u64 {
        let mut s = FxHasher::default();
        self.hash(&mut s);
        s.finish()
    }
}

impl<S: BDDSymbol> Clone for BDD<S> {
    fn clone(&self) -> Self {
        Self::new(self.id, &self.env)
    }
}

impl<S: BDDSymbol> Drop for BDD<S> {
    fn drop(&mut self) {
        self.env.store.unroot(self.id);
    }
}

impl<S: BDDSymbol> PartialEq for BDD<S> {
    fn eq(&self, other: &Self) -> bool {
        if Rc::ptr_eq(&self.env, &other.env) {
            self.id == other.id
        } else {
            self.node() == other.node()
        }
    }
}

impl<S: BDDSymbol> Eq for BDD<S> {}

impl<S: BDDSymbol> Hash for BDD<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<S: BDDSymbol> Debug for BDD<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.node().fmt(f)
    }
}

impl From<BDD<NamedSymbol>> for BDD<usize> {
    fn from(bdd: BDD<NamedSymbol>) -> Self {
        fn convert(
            src: &BDD<NamedSymbol>,
            id: u32,
            env: &BDDEnv<usize>,
            memo: &mut FxHashMap<u32, u32>,
        ) -> u32 {
            if id <= TRUE {
                return id;
            }
            if let Some(&result) = memo.get(&id) {
                return result;
            }
            let n = src.env.store.node(id);
            let var = env.var_index(src.env.vars.borrow()[n.var as usize].id);
            let hi = convert(src, n.hi, env, memo);
            let lo = convert(src, n.lo, env, memo);
            let result = env.inner.store.mk(var, hi, lo);
            memo.insert(id, result);
            result
        }

        let env = BDDEnv::new();
        let id = convert(&bdd, bdd.id, &env, &mut FxHashMap::default());
        env.wrap(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    And,
    Or,
    Not,
}

/// The environment owning all nodes of a set of [`BDD`]s.
#[derive(Clone)]
pub struct BDDEnv<S: BDDSymbol> {
    inner: Rc<Inner<S>>,
}

impl<S: BDDSymbol> Default for BDDEnv<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: BDDSymbol> PartialEq for BDDEnv<S> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<S: BDDSymbol> Eq for BDDEnv<S> {}

impl<S: BDDSymbol> Debug for BDDEnv<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BDDEnv")
            .field("size", &self.size())
            .finish()
    }
}

impl<S: BDDSymbol> BDDEnv<S> {
    /// Create a new BDD graph
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                store: Store::new(),
                vars: RefCell::default(),
                var_ids: RefCell::default(),
            }),
        }
    }

    /// Create a new BDD graph which keeps its memory usage below the given number of bytes,
    /// by collecting unused nodes and paging nodes to disk.
    pub fn with_memory_limit(bytes: usize) -> Self {
        let env = Self::new();
        env.set_memory_limit(bytes);
        env
    }

    pub fn set_memory_limit(&self, bytes: usize) {
        self.inner.store.set_limit(bytes);
    }

    /// The memory limit in bytes, `usize::MAX` if there is none
    pub fn memory_limit(&self) -> usize {
        self.inner.store.limit()
    }

    /// Estimated number of bytes in memory
    pub fn memory(&self) -> usize {
        self.inner.store.memory()
    }

    /// Free all nodes that are no longer referenced
    pub fn gc(&self) {
        self.inner.store.gc();
    }

    /// Compute the size of the BDD (number of nodes)
    pub fn size(&self) -> usize {
        self.inner.store.size()
    }

    fn wrap(&self, id: u32) -> BDD<S> {
        let result = BDD::new(id, &self.inner);
        self.inner.store.gc_point();
        result
    }

    fn id(&self, b: &BDD<S>) -> u32 {
        debug_assert!(
            Rc::ptr_eq(&self.inner, &b.env),
            "BDD belongs to a different environment"
        );
        b.id
    }

    fn ids(&self, bs: &[BDD<S>]) -> Vec<u32> {
        bs.iter().map(|b| self.id(b)).collect()
    }

    fn var_index(&self, s: S) -> u32 {
        if let Some(&id) = self.inner.var_ids.borrow().get(&s) {
            return id;
        }
        let mut vars = self.inner.vars.borrow_mut();
        let id = vars.len() as u32;
        vars.push(s.clone());
        self.inner.var_ids.borrow_mut().insert(s, id);
        id
    }

    fn cmp_vars(&self, a: u32, b: u32) -> Ordering {
        if a == b {
            Ordering::Equal
        } else {
            let vars = self.inner.vars.borrow();
            vars[a as usize].cmp(&vars[b as usize])
        }
    }

    fn node(&self, id: u32) -> RawNode {
        self.inner.store.node(id)
    }

    fn mk(&self, var: u32, hi: u32, lo: u32) -> u32 {
        self.inner.store.mk(var, hi, lo)
    }

    pub fn duplicates(&self, root: BDD<S>) -> usize {
        let all_nodes = root.node_list();
        let unique_ids = all_nodes.iter().unique_by(|n| n.id).count();
        let unique_nodes = all_nodes
            .iter()
            .unique_by(|n| match n.id {
                FALSE | TRUE => (u32::MAX, n.id, n.id),
                id => {
                    let r = self.node(id);
                    (r.var, r.hi, r.lo)
                }
            })
            .count();

        unique_ids - unique_nodes
    }

    /// Make a new choice based on the given symbol and the left and right subtree.
    pub fn mk_choice(&self, true_subtree: BDD<S>, symbol: S, false_subtree: BDD<S>) -> BDD<S> {
        let var = self.var_index(symbol);
        self.wrap(self.mk(var, self.id(&true_subtree), self.id(&false_subtree)))
    }

    pub fn mk_const(&self, v: bool) -> BDD<S> {
        self.wrap(if v { TRUE } else { FALSE })
    }

    /// Logic conjunction
    pub fn and(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        self.wrap(self.and_id(self.id(&a), self.id(&b)))
    }

    /// Disjunction
    pub fn or(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        self.wrap(self.or_id(self.id(&a), self.id(&b)))
    }

    /// Logic negation
    pub fn not(&self, a: BDD<S>) -> BDD<S> {
        self.wrap(self.not_id(self.id(&a)))
    }

    /// Implication
    pub fn implies(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        self.wrap(self.implies_id(self.id(&a), self.id(&b)))
    }

    /// ite computes if a then b else c
    pub fn ite(&self, a: BDD<S>, b: BDD<S>, c: BDD<S>) -> BDD<S> {
        self.wrap(self.ite_id(self.id(&a), self.id(&b), self.id(&c)))
    }

    /// eq computes a iff b
    pub fn eq(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        let (a, b) = (self.id(&a), self.id(&b));
        self.wrap(self.and_id(self.implies_id(a, b), self.implies_id(b, a)))
    }

    /// exclusive disjunction
    pub fn xor(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        let (a, b) = (self.id(&a), self.id(&b));
        self.wrap(self.or_id(
            self.and_id(self.not_id(a), b),
            self.and_id(a, self.not_id(b)),
        ))
    }

    /// joint denial (nor)
    pub fn nor(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        let (a, b) = (self.id(&a), self.id(&b));
        self.wrap(self.and_id(self.not_id(a), self.not_id(b)))
    }

    /// alternative denial (nand)
    pub fn nand(&self, a: BDD<S>, b: BDD<S>) -> BDD<S> {
        let (a, b) = (self.id(&a), self.id(&b));
        self.wrap(self.not_id(self.and_id(a, b)))
    }

    /// var constructs a new BDD for a given variable.
    pub fn var(&self, s: S) -> BDD<S> {
        let var = self.var_index(s);
        self.wrap(self.mk(var, TRUE, FALSE))
    }

    fn and_id(&self, a: u32, b: u32) -> u32 {
        match (a, b) {
            (FALSE, _) | (_, FALSE) => FALSE,
            (TRUE, x) | (x, TRUE) => x,
            _ if a == b => a,
            _ => self.apply(Op::And, a.min(b), a.max(b)),
        }
    }

    fn or_id(&self, a: u32, b: u32) -> u32 {
        match (a, b) {
            (TRUE, _) | (_, TRUE) => TRUE,
            (FALSE, x) | (x, FALSE) => x,
            _ if a == b => a,
            _ => self.apply(Op::Or, a.min(b), a.max(b)),
        }
    }

    fn not_id(&self, a: u32) -> u32 {
        match a {
            FALSE => TRUE,
            TRUE => FALSE,
            _ => self.apply(Op::Not, a, a),
        }
    }

    fn apply(&self, op: Op, a: u32, b: u32) -> u32 {
        let store = &self.inner.store;
        if let Some(result) = store.cache_get(op as u32, a, b) {
            return result;
        }

        let na = self.node(a);
        let result = if op == Op::Not {
            let hi = self.not_id(na.hi);
            let lo = self.not_id(na.lo);
            self.mk(na.var, hi, lo)
        } else {
            let nb = self.node(b);
            let (var, (ah, al), (bh, bl)) = match self.cmp_vars(na.var, nb.var) {
                Ordering::Less => (na.var, (na.hi, na.lo), (b, b)),
                Ordering::Greater => (nb.var, (a, a), (nb.hi, nb.lo)),
                Ordering::Equal => (na.var, (na.hi, na.lo), (nb.hi, nb.lo)),
            };
            let binop = |x, y| match op {
                Op::And => self.and_id(x, y),
                _ => self.or_id(x, y),
            };
            let hi = binop(ah, bh);
            let lo = binop(al, bl);
            self.mk(var, hi, lo)
        };

        store.cache_put(op as u32, a, b, result);
        result
    }

    fn implies_id(&self, a: u32, b: u32) -> u32 {
        self.or_id(self.not_id(a), b)
    }

    fn ite_id(&self, a: u32, b: u32, c: u32) -> u32 {
        self.and_id(self.implies_id(a, b), self.implies_id(self.not_id(a), c))
    }

    fn cmp_count<CmpFn: Fn(i64) -> bool + Copy>(
        &self,
        branches: &[u32],
        n: i64,
        cmp: CmpFn,
        memo: &mut FxHashMap<(usize, i64), u32>,
    ) -> u32 {
        let Some((&first, remainder)) = branches.split_first() else {
            return if cmp(n) { TRUE } else { FALSE };
        };
        if let Some(&result) = memo.get(&(branches.len(), n)) {
            return result;
        }

        let result = self.ite_id(
            first,
            self.cmp_count(remainder, n - 1, cmp, memo),
            self.cmp_count(remainder, n, cmp, memo),
        );
        memo.insert((branches.len(), n), result);
        result
    }

    fn aln_id(&self, branches: &[u32], n: i64) -> u32 {
        self.cmp_count(branches, n, |n| n <= 0, &mut FxHashMap::default())
    }

    fn amn_id(&self, branches: &[u32], n: i64) -> u32 {
        self.cmp_count(branches, n, |n| n >= 0, &mut FxHashMap::default())
    }

    /// at least n: [i64] of the branches: [BDD<S>] are true
    pub fn aln(&self, branches: &[BDD<S>], n: i64) -> BDD<S> {
        self.wrap(self.aln_id(&self.ids(branches), n))
    }

    /// at most n: [i64] of the branches: [BDD<S>] are true
    pub fn amn(&self, branches: &[BDD<S>], n: i64) -> BDD<S> {
        self.wrap(self.amn_id(&self.ids(branches), n))
    }

    /// exactly n: [i64] of the branches [BDD<S>] are true
    pub fn exn(&self, branches: &[BDD<S>], n: i64) -> BDD<S> {
        let ids = self.ids(branches);
        self.wrap(self.cmp_count(&ids, n, |n| n == 0, &mut FxHashMap::default()))
    }

    pub fn count_leq(&self, a: &[BDD<S>], b: &[BDD<S>]) -> BDD<S> {
        self.wrap(self.count_leq_id(&self.ids(a), &self.ids(b), 0))
    }

    pub fn count_lt(&self, a: &[BDD<S>], b: &[BDD<S>]) -> BDD<S> {
        self.wrap(self.count_leq_id(&self.ids(a), &self.ids(b), 1))
    }

    pub fn count_gt(&self, a: &[BDD<S>], b: &[BDD<S>]) -> BDD<S> {
        self.wrap(self.count_geq_id(&self.ids(a), &self.ids(b), -1))
    }

    pub fn count_geq(&self, a: &[BDD<S>], b: &[BDD<S>]) -> BDD<S> {
        self.wrap(self.count_geq_id(&self.ids(a), &self.ids(b), 0))
    }

    pub fn count_eq(&self, a: &[BDD<S>], b: &[BDD<S>]) -> BDD<S> {
        let (a, b) = (self.ids(a), self.ids(b));
        self.wrap(self.and_id(self.count_leq_id(&a, &b, 0), self.count_geq_id(&a, &b, 0)))
    }

    fn count_leq_id(&self, a: &[u32], b: &[u32], n: i64) -> u32 {
        self.cmp_count_compare(a, b, n, Self::aln_id, &mut FxHashMap::default())
    }

    fn count_geq_id(&self, a: &[u32], b: &[u32], n: i64) -> u32 {
        self.cmp_count_compare(a, b, n, Self::amn_id, &mut FxHashMap::default())
    }

    fn cmp_count_compare<CmpFn: Fn(&Self, &[u32], i64) -> u32 + Copy>(
        &self,
        a: &[u32],
        b: &[u32],
        n: i64,
        cmp: CmpFn,
        memo: &mut FxHashMap<(usize, i64), u32>,
    ) -> u32 {
        if let Some(&result) = memo.get(&(a.len(), n)) {
            return result;
        }

        let result = match a.split_first() {
            None => cmp(self, b, n),
            Some((&first, remainder)) => self.ite_id(
                first,
                self.cmp_count_compare(remainder, b, n + 1, cmp, memo),
                self.cmp_count_compare(remainder, b, n, cmp, memo),
            ),
        };
        memo.insert((a.len(), n), result);
        result
    }

    pub fn exists(&self, s: Vec<S>, b: BDD<S>) -> BDD<S> {
        self.wrap(self.exists_id(&s, self.id(&b)))
    }

    fn exists_id(&self, s: &[S], b: u32) -> u32 {
        s.iter().rev().fold(b, |acc, v| {
            let var = self.inner.var_ids.borrow().get(v).copied();
            var.map_or(acc, |var| {
                self.exists_memo(var, acc, &mut FxHashMap::default())
            })
        })
    }

    /// existential quantification
    pub fn exists_impl(&self, s: &S, b: BDD<S>) -> BDD<S> {
        self.wrap(self.exists_id(std::slice::from_ref(s), self.id(&b)))
    }

    fn exists_memo(&self, var: u32, b: u32, memo: &mut FxHashMap<u32, u32>) -> u32 {
        if b <= TRUE {
            return b;
        }
        if let Some(&result) = memo.get(&b) {
            return result;
        }

        let n = self.node(b);
        let result = match self.cmp_vars(n.var, var) {
            Ordering::Greater => b,
            Ordering::Equal => self.or_id(n.hi, n.lo),
            Ordering::Less => {
                let hi = self.exists_memo(var, n.hi, memo);
                let lo = self.exists_memo(var, n.lo, memo);
                self.mk(n.var, hi, lo)
            }
        };
        memo.insert(b, result);
        result
    }

    /// forall quantification
    pub fn all(&self, s: Vec<S>, b: BDD<S>) -> BDD<S> {
        let b = self.id(&b);
        self.wrap(self.not_id(self.exists_id(&s, self.not_id(b))))
    }

    /// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
    pub fn fp<F>(&self, a: BDD<S>, t: F) -> BDD<S>
    where
        F: Fn(BDD<S>) -> BDD<S>,
    {
        let mut s = a;
        loop {
            let snew = t(s.clone());
            if snew == s {
                break;
            }
            s = snew;
        }
        s
    }

    pub fn model(&self, a: BDD<S>) -> BDD<S> {
        self.wrap(self.model_id(self.id(&a)))
    }

    fn model_id(&self, a: u32) -> u32 {
        if a <= TRUE {
            return a;
        }
        let n = self.node(a);
        let var = self.mk(n.var, TRUE, FALSE);
        if n.hi == FALSE {
            self.and_id(self.not_id(var), self.model_id(n.lo))
        } else {
            self.and_id(self.model_id(n.hi), var)
        }
    }

    /// determine whether variable b is always true or false for a given bdd a
    /// returns a tuple (bool, bool) where the first item determines whether b is bound
    /// the second item determines the truth value for b
    pub fn infer(&self, a: BDD<S>, b: S) -> (bool, bool) {
        let var = self.var_index(b);
        let ff = self.wrap(self.implies_id(self.id(&a), self.mk(var, TRUE, FALSE)));
        (ff.is_const(), ff.is_true())
    }

    /// retain choice variables only if their truth value changes the bdd
    pub fn retain_choice_bottom_up(&self, src: BDD<S>, filter: TruthTableEntry) -> BDD<S> {
        match filter {
            TruthTableEntry::Any => src,
            _ => self.wrap(self.retain_choice_id(self.id(&src), filter)),
        }
    }

    fn retain_choice_id(&self, src: u32, filter: TruthTableEntry) -> u32 {
        if src <= TRUE {
            return src;
        }

        let n = self.node(src);
        let true_subtree = self.retain_choice_id(n.hi, filter);
        let false_subtree = self.retain_choice_id(n.lo, filter);
        let keep = if filter.is_true() {
            true_subtree != FALSE
        } else {
            false_subtree == TRUE || true_subtree > TRUE
        };

        if keep {
            self.mk(n.var, true_subtree, false_subtree)
        } else {
            eprintln!(
                "omitted choice {}",
                self.inner.vars.borrow()[n.var as usize]
            );
            if filter.is_true() {
                false_subtree
            } else {
                true_subtree
            }
        }
    }
}
