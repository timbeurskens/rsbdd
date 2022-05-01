use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHasher};
use std::cell::RefCell;
use std::fmt;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

pub trait BDDSymbol: Ord + Display + Debug + Clone + Hash {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Hash {}

#[derive(Debug, Clone)]
pub struct NamedSymbol {
    pub name: Rc<String>,
    pub id: usize,
}

impl fmt::Display for NamedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.name, f)
    }
}

impl Hash for NamedSymbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for NamedSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for NamedSymbol {}

impl Ord for NamedSymbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for NamedSymbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<NamedSymbol> for usize {
    fn from(ns: NamedSymbol) -> Self {
        ns.id
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BDD<S: BDDSymbol> {
    cached_hash: Option<u64>,
    pub node: BDDNode<S>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum BDDNode<Symbol: BDDSymbol> {
    False,
    True,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Rc<BDD<Symbol>>, Symbol, Rc<BDD<Symbol>>),
}

impl<S: BDDSymbol> Hash for BDD<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(cached) = self.cached_hash {
            cached.hash(state);
        } else {
            self.node.hash(state);
        }
    }
}

impl From<BDD<NamedSymbol>> for BDD<usize> {
    fn from(bdd: BDD<NamedSymbol>) -> BDD<usize> {
        match bdd.node {
            BDDNode::False => BDD::new(&BDDNode::False),
            BDDNode::True => BDD::new(&BDDNode::True),
            BDDNode::Choice(true_subtree, symbol, false_subtree) => BDD::new(&BDDNode::Choice(
                Rc::new(BDD::from(true_subtree.as_ref().clone())),
                symbol.into(),
                Rc::new(BDD::from(false_subtree.as_ref().clone())),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TruthTableEntry {
    True,
    False,
    Any,
}

impl Display for TruthTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            TruthTableEntry::True => "True",
            TruthTableEntry::False => "False",
            TruthTableEntry::Any => "Any",
        })
    }
}

impl<S: BDDSymbol> Default for BDD<S> {
    fn default() -> Self {
        BDD::new(&BDDNode::False)
    }
}

impl<S: BDDSymbol> BDD<S> {
    pub fn get_hash(&self) -> u64 {
        let mut s = FxHasher::default();
        self.hash(&mut s);
        s.finish()
    }
}

impl<S: BDDSymbol> BDD<S> {
    pub fn new(node: &BDDNode<S>) -> BDD<S> {
        let mut result = BDD {
            cached_hash: None,
            node: node.clone(),
        };

        result.populate_hash();

        result
    }

    pub fn new_unoptimized(node: &BDDNode<S>) -> BDD<S> {
        BDD {
            cached_hash: None,
            node: node.clone(),
        }
    }

    pub fn populate_hash(&mut self) {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        self.cached_hash = Some(hasher.finish());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDEnv<Symbol: BDDSymbol> {
    pub nodes: RefCell<FxHashMap<BDD<Symbol>, Rc<BDD<Symbol>>>>,
}

impl<S: BDDSymbol> Default for BDDEnv<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: BDDSymbol> BDDEnv<S> {
    pub fn size(&self) -> usize {
        self.nodes.borrow().len()
    }

    // clean tries to reduce all duplicate subtrees to single nodes in the lookup table
    // this function currently has no effect, might be removed later
    pub fn clean(&self, root: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match root.as_ref().node {
            BDDNode::Choice(ref l, ref s, ref r) => {
                let _l = self.find(l);
                let _r = self.find(r);

                self.mk_choice(_l, s.clone(), _r)
            }
            _ => self.find(&root),
        }
    }

    pub fn duplicates(&self, root: Rc<BDD<S>>) -> usize {
        let all_nodes: Vec<Rc<BDD<S>>> = self.node_list(root);

        // todo: conclusion: hashes are stricter than pointers
        // try to rephrase the equivalence check, such hash a == hash b <=> a == b

        let unique_hashes = all_nodes
            .iter()
            .map(|n| self.find(n))
            .unique_by(|n| n.get_hash())
            .count();

        let unique_pointers = all_nodes
            .iter()
            .unique_by(|&n| Rc::into_raw(Rc::clone(n)) as u32)
            .cloned()
            .count();

        unique_pointers - unique_hashes
    }

    pub fn node_list(&self, root: Rc<BDD<S>>) -> Vec<Rc<BDD<S>>> {
        match root.as_ref().node {
            BDDNode::Choice(ref l, _, ref r) => {
                let l_nodes = self.node_list(Rc::clone(l));
                let r_nodes = self.node_list(Rc::clone(r));

                l_nodes
                    .iter()
                    .chain(&vec![Rc::clone(&root)])
                    .chain(r_nodes.iter())
                    .cloned()
                    .collect()
            }
            BDDNode::True | BDDNode::False => vec![Rc::clone(&root)],
        }
    }

    // make a new choice based on the given symbol and the left and right subtree.
    // the new choice is then simplified and a reference is added to the lookup table
    pub fn mk_choice(
        &self,
        true_subtree: Rc<BDD<S>>,
        symbol: S,
        false_subtree: Rc<BDD<S>>,
    ) -> Rc<BDD<S>> {
        // early simplification step
        let ins = self.simplify(&Rc::new(BDD::new_unoptimized(&BDDNode::Choice(true_subtree, symbol, false_subtree))));

        // pre-borrow the nodes as mutable
        let mut nodes_borrow = self.nodes.borrow_mut();

        // if the node already exists, return a reference to it
        if let Some(subtree) = nodes_borrow.get(&ins) {
            Rc::clone(subtree)
        } else {
            // only insert if it is not already in the lookup table
            // first pre-compute the hash
            let mut ins_populated = ins.as_ref().clone();
            ins_populated.populate_hash();
            let ins_toinsert = Rc::new(ins_populated);

            nodes_borrow.insert(ins_toinsert.as_ref().clone(), Rc::clone(&ins_toinsert));
            Rc::clone(&ins_toinsert)
        }
    }

    // find the true or false node in the lookup table and return a reference to it
    pub fn mk_const(&self, v: bool) -> Rc<BDD<S>> {
        if v {
            // todo: return unoptimized node
            Rc::clone(self.nodes.borrow().get(&BDD::new(&BDDNode::True)).unwrap())
        } else {
            Rc::clone(self.nodes.borrow().get(&BDD::new(&BDDNode::False)).unwrap())
        }
    }

    // find an equivalent subtree in the lookup table and return a reference to it
    pub fn find(&self, r: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        Rc::clone(self.nodes.borrow().get(r.as_ref()).unwrap())
    }

    pub fn new() -> Self {
        let mut nodes = FxHashMap::default();

        let true_node = BDD::new(&BDDNode::True);
        let false_node = BDD::new(&BDDNode::False);

        nodes.insert(true_node.clone(), Rc::new(true_node));
        nodes.insert(false_node.clone(), Rc::new(false_node));

        BDDEnv {
            nodes: RefCell::new(nodes),
        }
    }

    // conjunction
    pub fn and(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (&a.as_ref().node, &b.as_ref().node) {
            (BDDNode::False, _) | (_, BDDNode::False) => self.mk_const(false),
            (BDDNode::True, _) => Rc::clone(&b),
            (_, BDDNode::True) => Rc::clone(&a),
            (BDDNode::Choice(ref at, ref va, ref af), BDDNode::Choice(_, ref vb, _)) if va < vb => {
                self.mk_choice(
                    self.and(Rc::clone(at), Rc::clone(&b)),
                    va.clone(),
                    self.and(Rc::clone(af), Rc::clone(&b)),
                )
            }
            (BDDNode::Choice(_, ref va, _), BDDNode::Choice(ref bt, ref vb, ref bf)) if vb < va => {
                self.mk_choice(
                    self.and(Rc::clone(bt), Rc::clone(&a)),
                    vb.clone(),
                    self.and(Rc::clone(bf), Rc::clone(&a)),
                )
            }
            (BDDNode::Choice(ref at, ref va, ref af), BDDNode::Choice(ref bt, ref vb, ref bf))
                if va == vb =>
            {
                self.mk_choice(
                    self.and(Rc::clone(at), Rc::clone(bt)),
                    va.clone(),
                    self.and(Rc::clone(af), Rc::clone(bf)),
                )
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    pub fn not(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref().node {
            BDDNode::False => self.mk_const(true),
            BDDNode::True => self.mk_const(false),
            BDDNode::Choice(ref at, ref va, ref af) => {
                self.mk_choice(self.not(Rc::clone(at)), va.clone(), self.not(Rc::clone(af)))
            }
        }
    }

    pub fn implies(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(self.not(Rc::clone(&a)), Rc::clone(&b))
    }

    /// ite computes if a then b else c
    pub fn ite(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>, c: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(
            self.implies(Rc::clone(&a), Rc::clone(&b)),
            self.implies(self.not(Rc::clone(&a)), Rc::clone(&c)),
        )
    }

    /// eq computes a iff b
    pub fn eq(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(
            self.implies(Rc::clone(&a), Rc::clone(&b)),
            self.implies(Rc::clone(&b), Rc::clone(&a)),
        )
    }

    // disjunction
    pub fn or(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.and(self.not(a), self.not(b)))
    }

    // exclusive disjunction
    pub fn xor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(
            self.and(self.not(Rc::clone(&a)), Rc::clone(&b)),
            self.and(Rc::clone(&a), self.not(Rc::clone(&b))),
        )
    }

    // joint denial (nor)
    pub fn nor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.or(Rc::clone(&a), Rc::clone(&b)))
    }

    // alternative denial (nand)
    pub fn nand(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.and(Rc::clone(&a), Rc::clone(&b)))
    }

    /// var constructs a new BDD for a given variable.
    pub fn var(&self, s: S) -> Rc<BDD<S>> {
        self.mk_choice(self.mk_const(true), s, self.mk_const(false))
    }

    pub fn aln(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        if branches.is_empty() {
            if n > 0 {
                self.mk_const(false)
            } else {
                self.mk_const(true)
            }
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.aln(&remainder, n - 1),
                self.aln(&remainder, n),
            )
        }
    }

    pub fn amn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        if branches.is_empty() {
            if n >= 0 {
                self.mk_const(true)
            } else {
                self.mk_const(false)
            }
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.amn(&remainder, n - 1),
                self.amn(&remainder, n),
            )
        }
    }

    pub fn exn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.and(self.amn(branches, n), self.aln(branches, n))
    }

    pub fn count_leq(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_leq_recursive(a, b, 0)
    }

    pub fn count_lt(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_leq_recursive(a, b, 1)
    }

    fn count_leq_recursive(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        if a.is_empty() {
            self.aln(b, n)
        } else {
            let first = &a[0];
            let remainder = a[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.count_leq_recursive(&remainder, b, n + 1),
                self.count_leq_recursive(&remainder, b, n),
            )
        }
    }

    pub fn count_gt(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_geq_recursive(a, b, -1)
    }

    pub fn count_geq(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_geq_recursive(a, b, 0)
    }

    fn count_geq_recursive(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        if a.is_empty() {
            self.amn(b, n)
        } else {
            let first = &a[0];
            let remainder = a[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.count_geq_recursive(&remainder, b, n + 1),
                self.count_geq_recursive(&remainder, b, n),
            )
        }
    }

    pub fn count_eq(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.and(self.count_leq(a, b), self.count_geq(a, b))
    }

    pub fn exists(&self, s: Vec<S>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        if s.is_empty() {
            b
        } else {
            let first = &s[0];
            let remainder = s[1..].to_vec();

            self.exists_impl(first.clone(), self.exists(remainder, b))
        }
    }

    // existential quantification
    pub fn exists_impl(&self, s: S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match b.as_ref().node {
            BDDNode::False | BDDNode::True => b,
            BDDNode::Choice(ref t, ref v, ref f) if *v == s => self.or(Rc::clone(t), Rc::clone(f)),
            BDDNode::Choice(ref t, ref v, ref f) => self.mk_choice(
                self.exists_impl(s.clone(), Rc::clone(t)),
                v.clone(),
                self.exists_impl(s, Rc::clone(f)),
            ),
        }
    }

    // forall quantification
    pub fn all(&self, s: Vec<S>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.exists(s, self.not(b)))
    }

    /// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
    pub fn fp<F>(&self, a: Rc<BDD<S>>, t: F) -> Rc<BDD<S>>
    where
        F: Fn(Rc<BDD<S>>) -> Rc<BDD<S>>,
    {
        let mut s = Rc::clone(&a);
        loop {
            let snew = t(Rc::clone(&s));
            if snew == s {
                break;
            }
            s = snew;
        }
        s
    }

    pub fn model(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref().node {
            BDDNode::Choice(ref t, ref v, ref f) => {
                let lhs = self.model(Rc::clone(t));
                let rhs = self.model(Rc::clone(f));
                if lhs != self.mk_const(false) {
                    self.and(lhs, self.var(v.clone()))
                } else if rhs != self.mk_const(false) {
                    self.and(self.not(self.var(v.clone())), rhs)
                } else {
                    self.mk_const(false)
                }
            }
            BDDNode::True | BDDNode::False => a,
        }
    }

    // determine whether variable b is always true or false for a given bdd a
    // returns a tuple (bool, bool) where the first item determines whether b is bound
    // the second item determines the truth value for b
    pub fn infer(&self, a: Rc<BDD<S>>, b: S) -> (bool, bool) {
        let ff = self.implies(a, self.var(b));
        match ff.as_ref().node {
            BDDNode::Choice(_, _, _) => (false, false),
            BDDNode::True => (true, true),
            BDDNode::False => (true, false),
        }
    }

    // simplify removes a choice node if both subtrees are equivalent
    pub fn simplify(&self, a: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        let result = match a.as_ref().node {
            BDDNode::Choice(ref t, _, ref f) if t.as_ref() == f.as_ref() => t,
            _ => a,
        };

        Rc::clone(result)
    }
}
