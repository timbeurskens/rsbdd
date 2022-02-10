use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

pub trait BDDSymbol: Ord + Display + Debug + Clone + Copy + Hash {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Copy + Hash {}

// todo: place bdd items in a collection (hashmap?)
// when constructing a new bdd, check if it already exists in the collection.
// if it does, return a reference to the existing bdd.
// otherwise, create a new bdd and return a reference to it.

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum BDD<Symbol: BDDSymbol> {
    True,
    False,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Rc<BDD<Symbol>>, Symbol, Rc<BDD<Symbol>>),
}

// impl<S: BDDSymbol> Hash for BDD<S> {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         match self {
//             &BDD::Choice(ref l, s, ref r) => {
//                 l.short_hash(state);
//                 s.hash(state);
//                 r.short_hash(state);
//             },
//             &BDD::True | &BDD::False => self.short_hash(state)
//         }
//     }
// }

// impl<S: BDDSymbol> BDD<S> {
//     fn short_hash<H: Hasher>(&self, state: &mut H) {
//         match self {
//             &BDD::Choice(_, s, _) => {
//                 s.hash(state);
//             },
//             &BDD::True => {
//                 true.hash(state);
//             },
//             &BDD::False => {
//                 false.hash(state);
//             }
//         }
//     }

//     pub fn get_hash(&self) -> u64 {
//         let mut s = DefaultHasher::new();
//         self.hash(&mut s);
//         s.finish()
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDEnv<Symbol: BDDSymbol> {
    pub nodes: RefCell<HashMap<BDD<Symbol>, Rc<BDD<Symbol>>>>,
}

impl<S: BDDSymbol> BDDEnv<S> {
    pub fn size(&self) -> usize {
        self.nodes.borrow().len()
    }

    pub fn mk_choice(
        &self,
        true_subtree: Rc<BDD<S>>,
        symbol: S,
        false_subtree: Rc<BDD<S>>,
    ) -> Rc<BDD<S>> {
        let ins = BDD::Choice(true_subtree, symbol, false_subtree);

        self.nodes
            .borrow_mut()
            .insert(ins.clone(), Rc::new(ins.clone()));
        Self::simplify(&self.find(&Rc::new(ins)))
    }

    pub fn mk_const(&self, v: bool) -> Rc<BDD<S>> {
        if v {
            self.nodes.borrow().get(&BDD::True).unwrap().clone()
        } else {
            self.nodes.borrow().get(&BDD::False).unwrap().clone()
        }
    }

    pub fn find(&self, r: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.nodes.borrow().get(r.as_ref()).unwrap().clone()
    }

    pub fn new() -> Self {
        let mut nodes = HashMap::new();

        nodes.insert(BDD::True, Rc::new(BDD::True));
        nodes.insert(BDD::False, Rc::new(BDD::<S>::False));

        BDDEnv {
            nodes: RefCell::new(nodes),
        }
    }

    pub fn and(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (&BDD::False, _) | (_, &BDD::False) => self.mk_const(false),
            (&BDD::True, _) => b.clone(),
            (_, &BDD::True) => a.clone(),
            (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => self
                .mk_choice(
                    self.and(at.clone(), b.clone()),
                    va,
                    self.and(af.clone(), b.clone()),
                ),
            (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => self
                .mk_choice(
                    self.and(bt.clone(), a.clone()),
                    vb,
                    self.and(bf.clone(), a.clone()),
                ),
            (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => {
                self.mk_choice(
                    self.and(at.clone(), bt.clone()),
                    va,
                    self.and(af.clone(), bf.clone()),
                )
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    pub fn implies(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (&BDD::False, _) | (_, &BDD::True) => self.mk_const(true),
            (&BDD::True, _) => b.clone(),
            (&BDD::Choice(ref t, v, ref f), &BDD::False) => self.mk_choice(
                self.implies(t.clone(), self.mk_const(false)),
                v,
                self.implies(f.clone(), self.mk_const(false)),
            ),
            (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => self
                .mk_choice(
                    self.implies(at.clone(), b.clone()),
                    va,
                    self.implies(af.clone(), b.clone()),
                ),
            (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => self
                .mk_choice(
                    self.implies(bt.clone(), a.clone()),
                    vb,
                    self.implies(bf.clone(), a.clone()),
                ),
            (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => {
                self.mk_choice(
                    self.implies(at.clone(), bt.clone()),
                    va,
                    self.implies(af.clone(), bf.clone()),
                )
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    /// ite computes if a then b else c
    pub fn ite(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>, c: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(
            self.implies(a.clone(), b.clone()),
            self.implies(self.not(a.clone()), c.clone()),
        )
    }

    /// eq computes a iff b
    pub fn eq(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.and(
            self.implies(a.clone(), b.clone()),
            self.implies(b.clone(), a.clone()),
        )
    }

    pub fn not(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.implies(a, self.mk_const(false))
    }

    pub fn or(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.and(self.not(a), self.not(b)))
    }

    pub fn xor(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.or(
            self.and(self.not(a.clone()), b.clone()),
            self.and(a.clone(), self.not(b.clone())),
        )
    }

    /// var constructs a new BDD for a given variable.
    pub fn var(&self, s: S) -> Rc<BDD<S>> {
        self.mk_choice(self.mk_const(true), s, self.mk_const(false))
    }

    // for all variables in vars at least n must be true
    pub fn aln(&self, vars: &Vec<S>, n: usize) -> Rc<BDD<S>> {
        let mut vars = vars.clone();
        vars.sort();

        self.aln_recursive(&vars, n as i64)
    }

    fn aln_recursive(&self, vars: &Vec<S>, n: i64) -> Rc<BDD<S>> {
        if vars.len() == 0 {
            if n > 0 {
                self.mk_const(false)
            } else {
                self.mk_const(true)
            }
        } else {
            let first = vars[0];
            let remainder = vars[1..].to_vec();

            self.mk_choice(
                self.aln_recursive(&remainder, n - 1),
                first,
                self.aln_recursive(&remainder, n),
            )
        }
    }

    // for all variables in vars at most n must be true
    pub fn amn(&self, vars: &Vec<S>, n: usize) -> Rc<BDD<S>> {
        let mut vars = vars.clone();
        vars.sort();

        self.amn_recursive(&vars, n as i64)
    }

    fn amn_recursive(&self, vars: &Vec<S>, n: i64) -> Rc<BDD<S>> {
        if vars.len() == 0 {
            if n >= 0 {
                self.mk_const(true)
            } else {
                self.mk_const(false)
            }
        } else {
            let first = vars[0];
            let remainder = vars[1..].to_vec();

            self.mk_choice(
                self.amn_recursive(&remainder, n - 1),
                first,
                self.amn_recursive(&remainder, n),
            )
        }
    }

    /// exn constructs a bdd such that exactly n variables in vars are true
    pub fn exn(&self, vars: &Vec<S>, n: usize) -> Rc<BDD<S>> {
        self.and(self.amn(vars, n), self.aln(vars, n))
    }

    /// existential quantification
    pub fn exists(&self, s: S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match b.as_ref() {
            &BDD::False | &BDD::True => b,
            &BDD::Choice(ref t, v, ref f) if v == s => self.or(t.clone(), f.clone()),
            &BDD::Choice(ref t, v, ref f) => {
                self.mk_choice(self.exists(s, t.clone()), v, self.exists(s, f.clone()))
            }
        }
    }

    pub fn all(&self, s: S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.not(self.exists(s, self.not(b)))
    }

    /// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
    pub fn fp<F>(&self, a: Rc<BDD<S>>, t: F) -> Rc<BDD<S>>
    where
        F: Fn(Rc<BDD<S>>) -> Rc<BDD<S>>,
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

    pub fn model(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            &BDD::Choice(ref t, v, ref f) => {
                let lhs = self.model(t.clone());
                let rhs = self.model(f.clone());
                if lhs != self.mk_const(false) {
                    self.and(lhs, self.var(v))
                } else if rhs != self.mk_const(false) {
                    self.and(self.not(self.var(v)), rhs)
                } else {
                    self.mk_const(false)
                }
            }
            &BDD::True | &BDD::False => a,
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

    pub fn simplify(a: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        match a.as_ref() {
            &BDD::Choice(ref t, _, ref f) if t == f => t.clone(),
            _ => a.clone(),
        }
    }
}
