use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHasher};
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

#[macro_export]
macro_rules! bdd {
    ($($expr:tt)+) => {{
        let input = stringify!($($expr)+);
        let mut input_reader = std::io::BufReader::new(input.as_bytes());
        let parsed_formula = rsbdd::parser::ParsedFormula::new(&mut input_reader, None).expect("could not parse expression");

        parsed_formula.eval()
    }};
}

pub trait BDDSymbol: Ord + Display + Debug + Clone + Hash + Send + Sync {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Hash + Send + Sync {}

#[derive(Debug, Clone)]
pub struct NamedSymbol {
    pub name: Arc<String>,
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

pub type BDDContainer<S> = Arc<BDD<S>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum BDD<Symbol: BDDSymbol> {
    False,
    True,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(BDDContainer<Symbol>, Symbol, BDDContainer<Symbol>),
}

impl From<BDD<NamedSymbol>> for BDD<usize> {
    fn from(bdd: BDD<NamedSymbol>) -> BDD<usize> {
        match bdd {
            BDD::False => BDD::False,
            BDD::True => BDD::True,
            BDD::Choice(true_subtree, symbol, false_subtree) => BDD::Choice(
                Arc::new(BDD::from(true_subtree.as_ref().clone())),
                symbol.into(),
                Arc::new(BDD::from(false_subtree.as_ref().clone())),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruthTableEntry {
    True,
    False,
    Any,
}

#[derive(Debug)]
pub struct TruthTableEntryParseError {
    pub input: String,
}

impl Error for TruthTableEntryParseError {}

impl Display for TruthTableEntryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Could not parse truth table entry: {}", self.input)
    }
}

impl FromStr for TruthTableEntry {
    type Err = TruthTableEntryParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "true" | "True" | "t" | "T" | "1" => Ok(TruthTableEntry::True),
            "false" | "False" | "f" | "F" | "0" => Ok(TruthTableEntry::False),
            "any" | "Any" | "a" | "A" => Ok(TruthTableEntry::Any),
            _ => Err(TruthTableEntryParseError {
                input: s.to_string(),
            }),
        }
    }
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
        BDD::False
    }
}

impl<S: BDDSymbol> BDD<S> {
    pub fn get_hash(&self) -> u64 {
        let mut s = FxHasher::default();
        self.hash(&mut s);
        s.finish()
    }
}

#[derive(Debug, Clone)]
pub struct BDDEnv<Symbol: BDDSymbol> {
    pub nodes: Arc<RwLock<FxHashMap<BDD<Symbol>, BDDContainer<Symbol>>>>,
}

impl<S: BDDSymbol> Default for BDDEnv<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: BDDSymbol> BDDEnv<S> {
    pub fn size(&self) -> usize {
        self.nodes.read().unwrap().len()
    }

    // clean tries to reduce all duplicate subtrees to single nodes in the lookup table
    // this function currently has no effect, might be removed later
    pub fn clean(&self, root: BDDContainer<S>) -> BDDContainer<S> {
        match root.as_ref() {
            BDD::Choice(l, s, r) => {
                let _l = self.find(l);
                let _r = self.find(r);

                self.mk_choice(_l, s.clone(), _r)
            }
            _ => self.find(&root),
        }
    }

    pub fn duplicates(&self, root: BDDContainer<S>) -> usize {
        let all_nodes: Vec<BDDContainer<S>> = self.node_list(root);

        // todo: conclusion: hashes are stricter than pointers
        // try to rephrase the equivalence check, such hash a == hash b <=> a == b

        let unique_hashes = all_nodes
            .iter()
            .map(|n| self.find(n))
            .unique_by(|n| n.get_hash())
            .count();

        let unique_pointers = all_nodes
            .iter()
            .unique_by(|&n| Arc::into_raw(n.clone()) as u32)
            .count();

        unique_pointers - unique_hashes
    }

    pub fn node_list(&self, root: BDDContainer<S>) -> Vec<BDDContainer<S>> {
        match root.as_ref() {
            BDD::Choice(l, _, r) => {
                let l_nodes = self.node_list(l.clone());
                let r_nodes = self.node_list(r.clone());

                l_nodes
                    .iter()
                    .chain(&vec![root.clone()])
                    .chain(r_nodes.iter())
                    .cloned()
                    .collect()
            }
            BDD::True | BDD::False => vec![root.clone()],
        }
    }

    // make a new choice based on the given symbol and the left and right subtree.
    // the new choice is then simplified and a reference is added to the lookup table
    pub fn mk_choice(
        &self,
        true_subtree: BDDContainer<S>,
        symbol: S,
        false_subtree: BDDContainer<S>,
    ) -> BDDContainer<S> {
        // early simplification step
        let ins = self.simplify(&Arc::new(BDD::Choice(true_subtree, symbol, false_subtree)));

        // pre-borrow the nodes as mutable
        let mut nodes_borrow = self.nodes.write().unwrap();

        // if the node already exists, return a reference to it
        if let Some(subtree) = nodes_borrow.get(&ins) {
            subtree.clone()
        } else {
            // only insert if it is not already in the lookup table
            nodes_borrow.insert(ins.as_ref().clone(), ins.clone());
            ins.clone()
        }
    }

    // find the true or false node in the lookup table and return a reference to it
    pub fn mk_const(&self, v: bool) -> BDDContainer<S> {
        if v {
            self.nodes.write().unwrap().get(&BDD::True).unwrap().clone()
        } else {
            self.nodes
                .write()
                .unwrap()
                .get(&BDD::False)
                .unwrap()
                .clone()
        }
    }

    // find an equivalent subtree in the lookup table and return a reference to it
    pub fn find(&self, r: &BDDContainer<S>) -> BDDContainer<S> {
        self.nodes.read().unwrap().get(r.as_ref()).unwrap().clone()
    }

    pub fn new() -> Self {
        let mut nodes = FxHashMap::default();

        nodes.insert(BDD::True, Arc::new(BDD::True));
        nodes.insert(BDD::False, Arc::new(BDD::False));

        BDDEnv {
            nodes: Arc::new(RwLock::new(nodes)),
        }
    }

    // conjunction
    pub fn and(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        match (a.as_ref(), b.as_ref()) {
            (BDD::False, _) | (_, &BDD::False) => self.mk_const(false),
            (BDD::True, _) => b.clone(),
            (_, BDD::True) => a.clone(),
            (BDD::Choice(at, va, af), BDD::Choice(_, vb, _)) if va < vb => {
                let (left, right) = rayon::join(
                    || self.and(at.clone(), b.clone()),
                    || self.and(af.clone(), b.clone()),
                );

                self.mk_choice(left, va.clone(), right)
            }
            (BDD::Choice(_, va, _), BDD::Choice(bt, vb, bf)) if vb < va => {
                let (left, right) = rayon::join(
                    || self.and(bt.clone(), a.clone()),
                    || self.and(bf.clone(), a.clone()),
                );

                self.mk_choice(left, vb.clone(), right)
            }
            (BDD::Choice(at, va, af), BDD::Choice(bt, vb, bf)) if va == vb => {
                let (left, right) = rayon::join(
                    || self.and(at.clone(), bt.clone()),
                    || self.and(af.clone(), bf.clone()),
                );

                self.mk_choice(left, va.clone(), right)
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    // disjunction
    pub fn or(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        match (a.as_ref(), b.as_ref()) {
            (BDD::True, _) | (_, BDD::True) => self.mk_const(true),
            (BDD::False, _) => b.clone(),
            (_, &BDD::False) => a.clone(),
            // todo:
            (BDD::Choice(at, va, af), BDD::Choice(_, vb, _)) if va < vb => {
                let (left, right) = rayon::join(
                    || self.or(at.clone(), b.clone()),
                    || self.or(af.clone(), b.clone()),
                );

                self.mk_choice(left, va.clone(), right)
            }
            (BDD::Choice(_, va, _), BDD::Choice(bt, vb, bf)) if vb < va => {
                let (left, right) = rayon::join(
                    || self.or(bt.clone(), a.clone()),
                    || self.or(bf.clone(), a.clone()),
                );

                self.mk_choice(left, vb.clone(), right)
            }
            (BDD::Choice(at, va, af), BDD::Choice(bt, vb, bf)) if va == vb => {
                let (left, right) = rayon::join(
                    || self.or(at.clone(), bt.clone()),
                    || self.or(af.clone(), bf.clone()),
                );

                self.mk_choice(left, va.clone(), right)
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    pub fn not(&self, a: BDDContainer<S>) -> BDDContainer<S> {
        match a.as_ref() {
            BDD::False => self.mk_const(true),
            BDD::True => self.mk_const(false),
            BDD::Choice(at, va, af) => {
                let (left, right) = rayon::join(|| self.not(at.clone()), || self.not(af.clone()));

                self.mk_choice(left, va.clone(), right)
            }
        }
    }

    pub fn implies(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.or(self.not(a), b)
    }

    /// ite computes if a then b else c
    pub fn ite(
        &self,
        a: BDDContainer<S>,
        b: BDDContainer<S>,
        c: BDDContainer<S>,
    ) -> BDDContainer<S> {
        let (left, right) = rayon::join(
            || self.implies(a.clone(), b.clone()),
            || self.implies(self.not(a.clone()), c.clone()),
        );

        self.and(left, right)
    }

    /// eq computes a iff b
    pub fn eq(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.and(self.implies(a.clone(), b.clone()), self.implies(b, a))
    }

    // exclusive disjunction
    pub fn xor(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.or(
            self.and(self.not(a.clone()), b.clone()),
            self.and(a, self.not(b)),
        )
    }

    // joint denial (nor)
    pub fn nor(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.and(self.not(a), self.not(b))
    }

    // alternative denial (nand)
    pub fn nand(&self, a: BDDContainer<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.not(self.and(a, b))
    }

    /// var constructs a new BDD for a given variable.
    pub fn var(&self, s: S) -> BDDContainer<S> {
        self.mk_choice(self.mk_const(true), s, self.mk_const(false))
    }

    #[inline]
    fn cmp_count<CmpFn: Fn(i64) -> bool + Copy>(
        &self,
        branches: &[BDDContainer<S>],
        n: i64,
        cmp: CmpFn,
    ) -> BDDContainer<S> {
        if branches.is_empty() {
            self.mk_const(cmp(n))
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.ite(
                first.clone(),
                self.cmp_count(&remainder, n - 1, cmp),
                self.cmp_count(&remainder, n, cmp),
            )
        }
    }

    /// at least n: [i64] of the branches: [BDDContainer<S>] are true
    pub fn aln(&self, branches: &[BDDContainer<S>], n: i64) -> BDDContainer<S> {
        self.cmp_count(branches, n, |n| n <= 0)
    }

    /// at most n: [i64] of the branches: [BDDContainer<S>] are true
    pub fn amn(&self, branches: &[BDDContainer<S>], n: i64) -> BDDContainer<S> {
        self.cmp_count(branches, n, |n| n >= 0)
    }

    /// exactly n: [i64] of the branches [BDDContainer<S>] are true
    pub fn exn(&self, branches: &[BDDContainer<S>], n: i64) -> BDDContainer<S> {
        self.cmp_count(branches, n, |n| n == 0)
    }

    pub fn count_leq(&self, a: &[BDDContainer<S>], b: &[BDDContainer<S>]) -> BDDContainer<S> {
        self.count_leq_recursive(a, b, 0)
    }

    pub fn count_lt(&self, a: &[BDDContainer<S>], b: &[BDDContainer<S>]) -> BDDContainer<S> {
        self.count_leq_recursive(a, b, 1)
    }

    fn count_leq_recursive(
        &self,
        a: &[BDDContainer<S>],
        b: &[BDDContainer<S>],
        n: i64,
    ) -> BDDContainer<S> {
        if a.is_empty() {
            self.aln(b, n)
        } else {
            let first = &a[0];
            let remainder = a[1..].to_vec();

            self.ite(
                first.clone(),
                self.count_leq_recursive(&remainder, b, n + 1),
                self.count_leq_recursive(&remainder, b, n),
            )
        }
    }

    pub fn count_gt(&self, a: &[BDDContainer<S>], b: &[BDDContainer<S>]) -> BDDContainer<S> {
        self.count_geq_recursive(a, b, -1)
    }

    pub fn count_geq(&self, a: &[BDDContainer<S>], b: &[BDDContainer<S>]) -> BDDContainer<S> {
        self.count_geq_recursive(a, b, 0)
    }

    fn count_geq_recursive(
        &self,
        a: &[BDDContainer<S>],
        b: &[BDDContainer<S>],
        n: i64,
    ) -> BDDContainer<S> {
        if a.is_empty() {
            self.amn(b, n)
        } else {
            let first = &a[0];
            let remainder = a[1..].to_vec();

            self.ite(
                first.clone(),
                self.count_geq_recursive(&remainder, b, n + 1),
                self.count_geq_recursive(&remainder, b, n),
            )
        }
    }

    pub fn count_eq(&self, a: &[BDDContainer<S>], b: &[BDDContainer<S>]) -> BDDContainer<S> {
        self.and(self.count_leq(a, b), self.count_geq(a, b))
    }

    pub fn exists(&self, s: Vec<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        if s.is_empty() {
            b
        } else {
            let first = &s[0];
            let remainder = s[1..].to_vec();

            self.exists_impl(first, self.exists(remainder, b))
        }
    }

    // existential quantification
    pub fn exists_impl(&self, s: &S, b: BDDContainer<S>) -> BDDContainer<S> {
        match b.as_ref() {
            BDD::False | &BDD::True => b,
            BDD::Choice(t, v, f) if v == s => self.or(t.clone(), f.clone()),
            BDD::Choice(t, v, f) => self.mk_choice(
                self.exists_impl(s, t.clone()),
                v.clone(),
                self.exists_impl(s, f.clone()),
            ),
        }
    }

    // forall quantification
    pub fn all(&self, s: Vec<S>, b: BDDContainer<S>) -> BDDContainer<S> {
        self.not(self.exists(s, self.not(b)))
    }

    /// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
    pub fn fp<F>(&self, a: BDDContainer<S>, t: F) -> BDDContainer<S>
    where
        F: Fn(BDDContainer<S>) -> BDDContainer<S>,
    {
        let mut s = a.clone();
        loop {
            let snew = t(s.clone());
            if snew == s {
                break;
            }
            s = snew;
        }
        s
    }

    pub fn model(&self, a: BDDContainer<S>) -> BDDContainer<S> {
        match a.as_ref() {
            BDD::Choice(t, v, f) => {
                let lhs = self.model(t.clone());
                let rhs = self.model(f.clone());
                if lhs != self.mk_const(false) {
                    self.and(lhs, self.var(v.clone()))
                } else if rhs != self.mk_const(false) {
                    self.and(self.not(self.var(v.clone())), rhs)
                } else {
                    self.mk_const(false)
                }
            }
            BDD::True | BDD::False => a,
        }
    }

    // determine whether variable b is always true or false for a given bdd a
    // returns a tuple (bool, bool) where the first item determines whether b is bound
    // the second item determines the truth value for b
    pub fn infer(&self, a: BDDContainer<S>, b: S) -> (bool, bool) {
        let ff = self.implies(a, self.var(b));
        match ff.as_ref() {
            BDD::Choice(_, _, _) => (false, false),
            BDD::True => (true, true),
            BDD::False => (true, false),
        }
    }

    // simplify removes a choice node if both subtrees are equivalent
    pub fn simplify(&self, a: &BDDContainer<S>) -> BDDContainer<S> {
        match a.as_ref() {
            BDD::Choice(t, _, f) if t.as_ref() == f.as_ref() => t.clone(),
            _ => a.clone(),
        }
    }
}
