use std::cell::RefCell;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHasher};

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

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum BDD<Symbol: BDDSymbol> {
    #[default]
    False,
    True,
    /// Choice (true-subtree, symbol, false-subtree)
    Choice(Rc<Self>, Symbol, Rc<Self>),
}

impl<Symbol> BDD<Symbol>
where
    Symbol: BDDSymbol,
{
    pub const fn is_choice(&self) -> bool {
        matches!(self, Self::Choice(_, _, _))
    }

    pub const fn is_const(&self) -> bool {
        !self.is_choice()
    }

    pub fn is_true(&self) -> bool {
        self == &Self::True
    }

    pub fn is_false(&self) -> bool {
        self == &Self::False
    }

    pub fn node_list(self: &Rc<Self>) -> Vec<Rc<Self>> {
        match self.as_ref() {
            Self::Choice(l, _, r) => {
                let l_nodes = l.node_list();
                let r_nodes = r.node_list();

                l_nodes
                    .iter()
                    .chain(&vec![Rc::clone(self)])
                    .chain(r_nodes.iter())
                    .cloned()
                    .collect()
            }
            Self::True | Self::False => vec![Rc::clone(self)],
        }
    }
}

impl From<BDD<NamedSymbol>> for BDD<usize> {
    fn from(bdd: BDD<NamedSymbol>) -> Self {
        match bdd {
            BDD::False => Self::False,
            BDD::True => Self::True,
            BDD::Choice(true_subtree, symbol, false_subtree) => Self::Choice(
                Rc::new(Self::from(true_subtree.as_ref().clone())),
                symbol.into(),
                Rc::new(Self::from(false_subtree.as_ref().clone())),
            ),
        }
    }
}

impl<S: BDDSymbol> Hash for BDD<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        if let Self::Choice(t, s, f) = self {
            s.hash(state);
            Rc::as_ptr(t).hash(state);
            Rc::as_ptr(f).hash(state);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Op {
    And,
    Or,
    Not,
}

type OpCache<S> = FxHashMap<(Op, Rc<BDD<S>>, Rc<BDD<S>>), Rc<BDD<S>>>;

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
    cache: RefCell<OpCache<Symbol>>,
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

    /// clean tries to reduce all duplicate subtrees to single nodes in the lookup table
    /// this function currently has no effect, might be removed later
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

        Rc::clone(
            self.nodes
                .borrow_mut()
                .entry(ins.as_ref().clone())
                .or_insert(ins),
        )
    }

    /// Find the true or false node in the lookup table and return a reference to it.
    pub fn mk_const(&self, v: bool) -> Rc<BDD<S>> {
        if v {
            Rc::clone(self.nodes.borrow().get(&BDD::True).expect("'True' is not a const in this BDD, make sure to initialize the data structure correctly"))
        } else {
            Rc::clone(self.nodes.borrow().get(&BDD::False).expect("'False' is not a const in this BDD, make sure to initialize the data structure correctly"))
        }
    }

    /// Find an equivalent subtree in the lookup table and return a reference to it
    pub fn find(&self, r: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        Rc::clone(self.nodes.borrow().get(r.as_ref()).expect(
            "Subtree not found in this BDD, make sure to initialize the data structure correctly",
        ))
    }

    /// Create a new BDD graph
    pub fn new() -> Self {
        let mut nodes = FxHashMap::default();

        nodes.insert(BDD::True, Rc::new(BDD::True));
        nodes.insert(BDD::False, Rc::new(BDD::False));

        Self {
            nodes: RefCell::new(nodes),
            cache: RefCell::default(),
        }
    }

    /// Logic conjunction
    pub fn and(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (BDD::False, _) | (_, &BDD::False) => self.mk_const(false),
            (BDD::True, _) => b,
            (_, BDD::True) => a,
            _ => self.apply(Op::And, a, b),
        }
    }

    /// Disjunction
    pub fn or(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (BDD::True, _) | (_, BDD::True) => self.mk_const(true),
            (BDD::False, _) => b,
            (_, &BDD::False) => a,
            _ => self.apply(Op::Or, a, b),
        }
    }

    /// Logic negation
    pub fn not(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            BDD::False => self.mk_const(true),
            BDD::True => self.mk_const(false),
            _ => self.apply(Op::Not, Rc::clone(&a), a),
        }
    }

    fn apply(&self, op: Op, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        let key = (op, a, b);
        if let Some(result) = self.cache.borrow().get(&key) {
            return Rc::clone(result);
        }

        let rec = |x: &Rc<BDD<S>>, y: &Rc<BDD<S>>| match op {
            Op::And => self.and(Rc::clone(x), Rc::clone(y)),
            Op::Or => self.or(Rc::clone(x), Rc::clone(y)),
            Op::Not => self.not(Rc::clone(x)),
        };

        let result = match (key.1.as_ref(), key.2.as_ref()) {
            (BDD::Choice(at, va, af), BDD::Choice(_, vb, _)) if va < vb => {
                self.mk_choice(rec(at, &key.2), va.clone(), rec(af, &key.2))
            }
            (BDD::Choice(_, va, _), BDD::Choice(bt, vb, bf)) if vb < va => {
                self.mk_choice(rec(bt, &key.1), vb.clone(), rec(bf, &key.1))
            }
            (BDD::Choice(at, va, af), BDD::Choice(bt, _, bf)) => {
                self.mk_choice(rec(at, bt), va.clone(), rec(af, bf))
            }
            _ => panic!("unsupported match: {:?} {:?}", key.1, key.2),
        };

        self.cache.borrow_mut().insert(key, Rc::clone(&result));
        result
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

    /// exclusive disjunction
    pub fn xor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(
            self.and(self.not(Rc::clone(&a)), Rc::clone(&b)),
            self.and(a, self.not(b)),
        )
    }

    /// joint denial (nor)
    pub fn nor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(self.not(a), self.not(b))
    }

    /// alternative denial (nand)
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
        memo: &mut FxHashMap<(usize, i64), Rc<BDD<S>>>,
    ) -> Rc<BDD<S>> {
        let Some((first, remainder)) = branches.split_first() else {
            return self.mk_const(cmp(n));
        };
        if let Some(result) = memo.get(&(branches.len(), n)) {
            return Rc::clone(result);
        }

        let result = self.ite(
            Rc::clone(first),
            self.cmp_count(remainder, n - 1, cmp, memo),
            self.cmp_count(remainder, n, cmp, memo),
        );
        memo.insert((branches.len(), n), Rc::clone(&result));
        result
    }

    /// at least n: [i64] of the branches: [Rc<BDD<S>>] are true
    pub fn aln(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n <= 0, &mut FxHashMap::default())
    }

    /// at most n: [i64] of the branches: [Rc<BDD<S>>] are true
    pub fn amn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n >= 0, &mut FxHashMap::default())
    }

    /// exactly n: [i64] of the branches [Rc<BDD<S>>] are true
    pub fn exn(&self, branches: &[Rc<BDD<S>>], n: i64) -> Rc<BDD<S>> {
        self.cmp_count(branches, n, |n| n == 0, &mut FxHashMap::default())
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

    fn cmp_count_compare<CmpFn: Fn(&Self, &[Rc<BDD<S>>], i64) -> Rc<BDD<S>> + Copy>(
        &self,
        a: &[Rc<BDD<S>>],
        b: &[Rc<BDD<S>>],
        n: i64,
        cmp: CmpFn,
    ) -> Rc<BDD<S>> {
        self.cmp_count_compare_memo(a, b, n, cmp, &mut FxHashMap::default())
    }

    fn cmp_count_compare_memo<CmpFn: Fn(&Self, &[Rc<BDD<S>>], i64) -> Rc<BDD<S>> + Copy>(
        &self,
        a: &[Rc<BDD<S>>],
        b: &[Rc<BDD<S>>],
        n: i64,
        cmp: CmpFn,
        memo: &mut FxHashMap<(usize, i64), Rc<BDD<S>>>,
    ) -> Rc<BDD<S>> {
        if let Some(result) = memo.get(&(a.len(), n)) {
            return Rc::clone(result);
        }

        let result = match a.split_first() {
            None => cmp(self, b, n),
            Some((first, remainder)) => self.ite(
                Rc::clone(first),
                self.cmp_count_compare_memo(remainder, b, n + 1, cmp, memo),
                self.cmp_count_compare_memo(remainder, b, n, cmp, memo),
            ),
        };
        memo.insert((a.len(), n), Rc::clone(&result));
        result
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
        s.iter().rev().fold(b, |acc, v| self.exists_impl(v, acc))
    }

    /// existential quantification
    pub fn exists_impl(&self, s: &S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.exists_memo(s, b, &mut FxHashMap::default())
    }

    fn exists_memo(
        &self,
        s: &S,
        b: Rc<BDD<S>>,
        memo: &mut FxHashMap<Rc<BDD<S>>, Rc<BDD<S>>>,
    ) -> Rc<BDD<S>> {
        if let Some(result) = memo.get(&b) {
            return Rc::clone(result);
        }

        let result = match b.as_ref() {
            BDD::False | BDD::True => return b,
            BDD::Choice(_, v, _) if v > s => return b,
            BDD::Choice(t, v, f) if v == s => self.or(Rc::clone(t), Rc::clone(f)),
            BDD::Choice(t, v, f) => self.mk_choice(
                self.exists_memo(s, Rc::clone(t), memo),
                v.clone(),
                self.exists_memo(s, Rc::clone(f), memo),
            ),
        };
        memo.insert(b, Rc::clone(&result));
        result
    }

    /// forall quantification
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
            BDD::Choice(t, v, _) if !t.is_false() => {
                self.and(self.model(Rc::clone(t)), self.var(v.clone()))
            }
            BDD::Choice(_, v, f) => {
                self.and(self.not(self.var(v.clone())), self.model(Rc::clone(f)))
            }
            BDD::True | BDD::False => a,
        }
    }

    /// determine whether variable b is always true or false for a given bdd a
    /// returns a tuple (bool, bool) where the first item determines whether b is bound
    /// the second item determines the truth value for b
    pub fn infer(&self, a: Rc<BDD<S>>, b: S) -> (bool, bool) {
        let ff = self.implies(a, self.var(b));
        match ff.as_ref() {
            BDD::Choice(_, _, _) => (false, false),
            BDD::True => (true, true),
            BDD::False => (true, false),
        }
    }

    /// simplify removes a choice node if both subtrees are equivalent
    pub fn simplify(&self, a: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            BDD::Choice(t, _, f) if Rc::ptr_eq(t, f) => Rc::clone(t),
            _ => Rc::clone(a),
        }
    }

    /// retain choice variables only if their truth value changes the bdd
    pub fn retain_choice_bottom_up(&self, src: Rc<BDD<S>>, filter: TruthTableEntry) -> Rc<BDD<S>> {
        match filter {
            // if we don't filter, we can just return the source
            TruthTableEntry::Any => src,
            // otherwise, remove nodes depending on truth value of the filter
            _ => {
                match src.as_ref() {
                    BDD::Choice(lhs, symbol, rhs) => {
                        // recursively run the retain function
                        let true_subtree = self.retain_choice_bottom_up(Rc::clone(lhs), filter);
                        let false_subtree = self.retain_choice_bottom_up(Rc::clone(rhs), filter);

                        if filter.is_true() {
                            if true_subtree.is_true() || true_subtree.is_choice() {
                                // retain choice
                                self.mk_choice(true_subtree, symbol.clone(), false_subtree)
                            } else {
                                // return true subtree
                                eprintln!("omitted choice {symbol}");
                                false_subtree
                            }
                        } else if false_subtree.is_true() || true_subtree.is_choice() {
                            // retain choice
                            self.mk_choice(true_subtree, symbol.clone(), false_subtree)
                        } else {
                            // return false subtree
                            eprintln!("omitted choice {symbol}");
                            true_subtree
                        }
                    }
                    // if the node is a constant, we can just return it
                    _ => src,
                }
            }
        }
    }
}
