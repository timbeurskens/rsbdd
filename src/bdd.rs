use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHasher};
use std::cell::RefCell;

use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::{BDDSymbol, NamedSymbol, TruthTableEntry};

#[macro_export]
macro_rules! bdd {
    ($($expr:tt)+) => {{
        let input = stringify!($($expr)+);
        let mut input_reader = std::io::BufReader::new(input.as_bytes());
        let parsed_formula = rsbdd::parser::ParsedFormula::new(&mut input_reader, None).expect("could not parse expression");

        parsed_formula.eval()
    }};
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub enum BDD<Symbol: BDDSymbol> {
    #[default]
    False,
    True,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Rc<BDD<Symbol>>, Symbol, Rc<BDD<Symbol>>),
}

impl<Symbol> BDD<Symbol>
where
    Symbol: BDDSymbol,
{
    pub fn is_choice(&self) -> bool {
        matches!(self, BDD::Choice(_, _, _))
    }

    pub fn is_const(&self) -> bool {
        !self.is_choice()
    }

    pub fn is_true(&self) -> bool {
        self == &BDD::True
    }

    pub fn is_false(&self) -> bool {
        self == &BDD::False
    }

    pub fn node_list(self: &Rc<Self>) -> Vec<Rc<Self>> {
        match self.as_ref() {
            BDD::Choice(l, _, r) => {
                let l_nodes = l.node_list();
                let r_nodes = r.node_list();

                l_nodes
                    .iter()
                    .chain(&vec![Rc::clone(self)])
                    .chain(r_nodes.iter())
                    .cloned()
                    .collect()
            }
            BDD::True | BDD::False => vec![Rc::clone(self)],
        }
    }
}

impl From<BDD<NamedSymbol>> for BDD<usize> {
    fn from(bdd: BDD<NamedSymbol>) -> BDD<usize> {
        match bdd {
            BDD::False => BDD::False,
            BDD::True => BDD::True,
            BDD::Choice(true_subtree, symbol, false_subtree) => BDD::Choice(
                Rc::new(BDD::from(true_subtree.as_ref().clone())),
                symbol.into(),
                Rc::new(BDD::from(false_subtree.as_ref().clone())),
            ),
        }
    }
}

impl<S: BDDSymbol> BDD<S> {
    pub fn get_hash(&self) -> u64 {
        let mut s = FxHasher::default();
        self.hash(&mut s);
        s.finish()
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
    /// Compute the size of the BDD (number of nodes)
    pub fn size(&self) -> usize {
        self.nodes.borrow().len()
    }

    // clean tries to reduce all duplicate subtrees to single nodes in the lookup table
    // this function currently has no effect, might be removed later
    pub fn clean(&self, root: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match root.as_ref() {
            BDD::Choice(l, s, r) => {
                let _l = self.find(l);
                let _r = self.find(r);

                self.mk_choice(_l, s.clone(), _r)
            }
            _ => self.find(&root),
        }
    }

    pub fn duplicates(&self, root: Rc<BDD<S>>) -> usize {
        let all_nodes: Vec<Rc<BDD<S>>> = root.node_list();

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
            .count();

        unique_pointers - unique_hashes
    }

    /// Make a new choice based on the given symbol and the left and right subtree.
    /// The new choice is then simplified and a reference is added to the lookup table.
    ///
    /// This function is probably the main bottleneck for bdd computations (in its current implementation).
    pub fn mk_choice(
        &self,
        true_subtree: Rc<BDD<S>>,
        symbol: S,
        false_subtree: Rc<BDD<S>>,
    ) -> Rc<BDD<S>> {
        // early simplification step
        let ins = self.simplify(&Rc::new(BDD::Choice(true_subtree, symbol, false_subtree)));

        // pre-borrow the nodes as mutable
        let mut nodes_borrow = self.nodes.borrow_mut();

        // if the node already exists, return a reference to it
        if let Some(subtree) = nodes_borrow.get(&ins) {
            Rc::clone(subtree)
        } else {
            // only insert if it is not already in the lookup table
            nodes_borrow.insert(ins.as_ref().clone(), Rc::clone(&ins));
            Rc::clone(&ins)
        }
    }

    /// Find the true or false node in the lookup table and return a reference to it.
    pub fn mk_const(&self, v: bool) -> Rc<BDD<S>> {
        if v {
            Rc::clone(self.nodes.borrow().get(&BDD::True).unwrap())
        } else {
            Rc::clone(self.nodes.borrow().get(&BDD::False).unwrap())
        }
    }

    /// Find an equivalent subtree in the lookup table and return a reference to it
    pub fn find(&self, r: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        Rc::clone(self.nodes.borrow().get(r.as_ref()).unwrap())
    }

    /// Create a new BDD graph
    pub fn new() -> Self {
        let mut nodes = FxHashMap::default();

        nodes.insert(BDD::True, Rc::new(BDD::True));
        nodes.insert(BDD::False, Rc::new(BDD::False));

        BDDEnv {
            nodes: RefCell::new(nodes),
        }
    }

    /// Logic conjunction
    pub fn and(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (BDD::False, _) | (_, &BDD::False) => self.mk_const(false),
            (BDD::True, _) => Rc::clone(&b),
            (_, BDD::True) => Rc::clone(&a),
            (BDD::Choice(at, va, af), BDD::Choice(_, vb, _)) if va < vb => self.mk_choice(
                self.and(Rc::clone(at), Rc::clone(&b)),
                va.clone(),
                self.and(Rc::clone(af), Rc::clone(&b)),
            ),
            (BDD::Choice(_, va, _), BDD::Choice(bt, vb, bf)) if vb < va => self.mk_choice(
                self.and(Rc::clone(bt), Rc::clone(&a)),
                vb.clone(),
                self.and(Rc::clone(bf), Rc::clone(&a)),
            ),
            (BDD::Choice(at, va, af), BDD::Choice(bt, vb, bf)) if va == vb => self.mk_choice(
                self.and(Rc::clone(at), Rc::clone(bt)),
                va.clone(),
                self.and(Rc::clone(af), Rc::clone(bf)),
            ),
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    /// Disjunction
    pub fn or(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        // self.not(self.nor(a, b))

        match (a.as_ref(), b.as_ref()) {
            (BDD::True, _) | (_, BDD::True) => self.mk_const(true),
            (BDD::False, _) => Rc::clone(&b),
            (_, &BDD::False) => Rc::clone(&a),
            // todo:
            (BDD::Choice(at, va, af), BDD::Choice(_, vb, _)) if va < vb => self.mk_choice(
                self.or(Rc::clone(at), Rc::clone(&b)),
                va.clone(),
                self.or(Rc::clone(af), Rc::clone(&b)),
            ),
            (BDD::Choice(_, va, _), BDD::Choice(bt, vb, bf)) if vb < va => self.mk_choice(
                self.or(Rc::clone(bt), Rc::clone(&a)),
                vb.clone(),
                self.or(Rc::clone(bf), Rc::clone(&a)),
            ),
            (BDD::Choice(at, va, af), BDD::Choice(bt, vb, bf)) if va == vb => self.mk_choice(
                self.or(Rc::clone(at), Rc::clone(bt)),
                va.clone(),
                self.or(Rc::clone(af), Rc::clone(bf)),
            ),
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    /// Logic negation
    pub fn not(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            BDD::False => self.mk_const(true),
            BDD::True => self.mk_const(false),
            BDD::Choice(at, va, af) => {
                self.mk_choice(self.not(Rc::clone(at)), va.clone(), self.not(Rc::clone(af)))
            }
        }
    }

    /// Implication
    pub fn implies(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(self.not(a), b)
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
            self.implies(b, a),
        )
    }

    // exclusive disjunction
    pub fn xor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(
            self.and(self.not(Rc::clone(&a)), Rc::clone(&b)),
            self.and(a, self.not(b)),
        )
    }

    // joint denial (nor)
    pub fn nor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(self.not(a), self.not(b))
    }

    // alternative denial (nand)
    pub fn nand(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.and(a, b))
    }

    /// var constructs a new BDD for a given variable.
    pub fn var(&self, s: S) -> Rc<BDD<S>> {
        self.mk_choice(self.mk_const(true), s, self.mk_const(false))
    }

    fn cmp_count<CmpFn: Fn(i64) -> bool + Copy>(
        &self,
        branches: &[Rc<BDD<S>>],
        n: i64,
        cmp: CmpFn,
    ) -> Rc<BDD<S>> {
        if branches.is_empty() {
            self.mk_const(cmp(n))
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.cmp_count(&remainder, n - 1, cmp),
                self.cmp_count(&remainder, n, cmp),
            )
        }
    }

    /// at least n: [i64] of the branches: [Rc<BDD<S>>] are true
    pub fn aln(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n <= 0)
    }

    /// at most n: [i64] of the branches: [Rc<BDD<S>>] are true
    pub fn amn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n >= 0)
    }

    /// exactly n: [i64] of the branches [Rc<BDD<S>>] are true
    pub fn exn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n == 0)
    }

    pub fn count_leq(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_leq_recursive(a, b, 0)
    }

    pub fn count_lt(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_leq_recursive(a, b, 1)
    }

    fn count_leq_recursive(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count_compare(a, b, n, Self::aln)
    }

    fn count_geq_recursive(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count_compare(a, b, n, Self::amn)
    }

    #[inline]
    fn cmp_count_compare<CmpFn: Fn(&Self, &[Rc<BDD<S>>], i64) -> Rc<BDD<S>> + Copy>(
        &self,
        a: &[Rc<BDD<S>>],
        b: &[Rc<BDD<S>>],
        n: i64,
        cmp: CmpFn,
    ) -> Rc<BDD<S>> {
        if a.is_empty() {
            cmp(self, b, n)
        } else {
            let first = &a[0];
            let remainder = a[1..].to_vec();

            self.ite(
                Rc::clone(first),
                self.cmp_count_compare(&remainder, b, n + 1, cmp),
                self.cmp_count_compare(&remainder, b, n, cmp),
            )
        }
    }

    pub fn count_gt(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_geq_recursive(a, b, -1)
    }

    pub fn count_geq(&self, a: &[Rc<BDD<S>>], b: &[Rc<BDD<S>>]) -> Rc<BDD<S>> {
        self.count_geq_recursive(a, b, 0)
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

            self.exists_impl(first, self.exists(remainder, b))
        }
    }

    // existential quantification
    pub fn exists_impl(&self, s: &S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match b.as_ref() {
            BDD::False | &BDD::True => b,
            BDD::Choice(t, v, f) if v == s => self.or(Rc::clone(t), Rc::clone(f)),
            BDD::Choice(t, v, f) => self.mk_choice(
                self.exists_impl(s, Rc::clone(t)),
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
        match a.as_ref() {
            BDD::Choice(t, v, f) => {
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
            BDD::True | BDD::False => a,
        }
    }

    // determine whether variable b is always true or false for a given bdd a
    // returns a tuple (bool, bool) where the first item determines whether b is bound
    // the second item determines the truth value for b
    pub fn infer(&self, a: Rc<BDD<S>>, b: S) -> (bool, bool) {
        let ff = self.implies(a, self.var(b));
        match ff.as_ref() {
            BDD::Choice(_, _, _) => (false, false),
            BDD::True => (true, true),
            BDD::False => (true, false),
        }
    }

    // simplify removes a choice node if both subtrees are equivalent
    pub fn simplify(&self, a: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            BDD::Choice(t, _, f) if t.as_ref() == f.as_ref() => Rc::clone(t),
            _ => Rc::clone(a),
        }
    }

    pub fn retain_choice_bottom_up(&self, src: Rc<BDD<S>>, filter: TruthTableEntry) -> Rc<BDD<S>> {
        match filter {
            // if we don't filter, we can just return the source
            TruthTableEntry::Any => src,
            // otherwise, remove nodes depending on truth value of the filter
            _ => {
                match src.as_ref() {
                    BDD::Choice(left, symbol, right) => {
                        // recursively run the retain function
                        let left = self.retain_choice_bottom_up(Rc::clone(left), filter);
                        let right = self.retain_choice_bottom_up(Rc::clone(right), filter);

                        if left.is_const() && right.is_choice() {
                            if left.is_true() != filter.is_true() {
                                // omit choice
                                eprintln!("omitted choice {symbol}");
                                right
                            } else {
                                self.mk_choice(left, symbol.clone(), right)
                            }
                        } else if right.is_const() && left.is_choice() {
                            if right.is_true() != filter.is_true() {
                                // omit choice
                                eprintln!("omitted choice {symbol}");
                                left
                            } else {
                                self.mk_choice(left, symbol.clone(), right)
                            }
                        } else {
                            self.mk_choice(left, symbol.clone(), right)
                        }
                    }
                    // if the node is a constant, we can just return it
                    _ => src,
                }
            }
        }
    }
}
