use itertools::Itertools;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::fmt;

pub trait BDDSymbol: Ord + Display + Debug + Clone + Hash {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Hash {}

#[derive(Debug, Clone)]
pub struct NamedSymbol {
    pub name: Rc<String>,
    pub id: usize,
}

// impl BDDSymbol for NamedSymbol {}

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

impl<S: BDDSymbol> BDD<S> {
    // fn short_hash<H: Hasher>(&self, state: &mut H) {
    //     match self {
    //         &BDD::Choice(_, s, _) => {
    //             s.hash(state);
    //         },
    //         &BDD::True => {
    //             true.hash(state);
    //         },
    //         &BDD::False => {
    //             false.hash(state);
    //         }
    //     }
    // }

    pub fn get_hash(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);
        s.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDEnv<Symbol: BDDSymbol> {
    pub nodes: RefCell<HashMap<BDD<Symbol>, Rc<BDD<Symbol>>>>,
}

impl<S: BDDSymbol> BDDEnv<S> {
    pub fn size(&self) -> usize {
        self.nodes.borrow().len()
    }

    // clean tries to reduce all duplicate subtrees to single nodes in the lookup table
    // this function currently has no effect, might be removed later
    pub fn clean(&self, root: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match root.as_ref() {
            &BDD::Choice(ref l, ref s, ref r) => {
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

        let unique_hashes: Vec<Rc<BDD<S>>> = all_nodes
            .clone()
            .iter()
            .map(|n| self.find(n))
            .unique_by(|n| n.get_hash())
            .collect();
        let unique_pointers: Vec<Rc<BDD<S>>> = all_nodes
            .clone()
            .iter()
            .unique_by(|&n| Rc::into_raw(Rc::clone(n)) as u32)
            .cloned()
            .collect();

        unique_pointers.len() - unique_hashes.len()
    }

    pub fn node_list(&self, root: Rc<BDD<S>>) -> Vec<Rc<BDD<S>>> {
        match root.as_ref() {
            &BDD::Choice(ref l, _, ref r) => {
                let l_nodes = self.node_list(Rc::clone(l));
                let r_nodes = self.node_list(Rc::clone(r));

                l_nodes
                    .iter()
                    .chain(&vec![Rc::clone(&root)])
                    .chain(r_nodes.iter())
                    .cloned()
                    .collect()
            }
            &BDD::True | &BDD::False => vec![Rc::clone(&root)].into(),
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
        let ins = self.simplify(&Rc::new(BDD::Choice(true_subtree, symbol, false_subtree)));

        if self.nodes.borrow().contains_key(&ins) {
            self.find(&ins)
        } else {
            // only insert if it is not already in the lookup table
            self.nodes
                .borrow_mut()
                .insert(ins.as_ref().clone(), Rc::clone(&ins));
            Rc::clone(&ins)
        }
    }

    // find the true or false node in the lookup table and return a reference to it
    pub fn mk_const(&self, v: bool) -> Rc<BDD<S>> {
        if v {
            Rc::clone(self.nodes.borrow().get(&BDD::True).unwrap())
        } else {
            Rc::clone(self.nodes.borrow().get(&BDD::False).unwrap())
        }
    }

    // find an equivalent subtree in the lookup table and return a reference to it
    pub fn find(&self, r: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        Rc::clone(self.nodes.borrow().get(r.as_ref()).unwrap())
    }

    pub fn new() -> Self {
        let mut nodes = HashMap::new();

        nodes.insert(BDD::True, Rc::new(BDD::True));
        nodes.insert(BDD::False, Rc::new(BDD::False));

        BDDEnv {
            nodes: RefCell::new(nodes),
        }
    }

    // conjunction
    pub fn and(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (&BDD::False, _) | (_, &BDD::False) => self.mk_const(false),
            (&BDD::True, _) => Rc::clone(&b),
            (_, &BDD::True) => Rc::clone(&a),
            (&BDD::Choice(ref at, ref va, ref af), &BDD::Choice(_, ref vb, _)) if va < vb => self
                .mk_choice(
                    self.and(Rc::clone(at), Rc::clone(&b)),
                    va.clone(),
                    self.and(Rc::clone(af), Rc::clone(&b)),
                ),
            (&BDD::Choice(_, ref va, _), &BDD::Choice(ref bt, ref vb, ref bf)) if vb < va => self
                .mk_choice(
                    self.and(Rc::clone(bt), Rc::clone(&a)),
                    vb.clone(),
                    self.and(Rc::clone(bf), Rc::clone(&a)),
                ),
            (&BDD::Choice(ref at, ref va, ref af), &BDD::Choice(ref bt, ref vb, ref bf)) if va == vb => {
                self.mk_choice(
                    self.and(Rc::clone(at), Rc::clone(bt)),
                    va.clone(),
                    self.and(Rc::clone(af), Rc::clone(bf)),
                )
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
    }

    // if a then b
    pub fn implies(&self, a: Rc<BDD<S>>, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match (a.as_ref(), b.as_ref()) {
            (&BDD::False, _) | (_, &BDD::True) => self.mk_const(true),
            (&BDD::True, _) => Rc::clone(&b),
            (&BDD::Choice(ref t, ref v, ref f), &BDD::False) => self.mk_choice(
                self.implies(Rc::clone(t), self.mk_const(false)),
                v.clone(),
                self.implies(Rc::clone(f), self.mk_const(false)),
            ),
            (&BDD::Choice(ref at, ref va, ref af), &BDD::Choice(_, ref vb, _)) if va < vb => self
                .mk_choice(
                    self.implies(Rc::clone(at), Rc::clone(&b)),
                    va.clone(),
                    self.implies(Rc::clone(af), Rc::clone(&b)),
                ),
            (&BDD::Choice(_, ref va, _), &BDD::Choice(ref bt, ref vb, ref bf)) if vb < va => self
                .mk_choice(
                    self.implies(Rc::clone(bt), Rc::clone(&a)),
                    vb.clone(),
                    self.implies(Rc::clone(bf), Rc::clone(&a)),
                ),
            (&BDD::Choice(ref at, ref va, ref af), &BDD::Choice(ref bt, ref vb, ref bf)) if va == vb => {
                self.mk_choice(
                    self.implies(Rc::clone(at), Rc::clone(bt)),
                    va.clone(),
                    self.implies(Rc::clone(af), Rc::clone(bf)),
                )
            }
            _ => panic!("unsupported match: {:?} {:?}", a, b),
        }
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

    // negation
    pub fn not(&self, a: Rc<BDD<S>>) -> Rc<BDD<S>> {
        self.implies(a, self.mk_const(false))
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

    pub fn aln(&self, branches: &Vec<Rc<BDD<S>>>, n: i64) -> Rc<BDD<S>> {
        if branches.len() == 0 {
            if n > 0 {
                self.mk_const(false)
            } else {
                self.mk_const(true)
            }
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.and(
                self.implies(Rc::clone(&first), self.aln(&remainder, n - 1)),
                self.implies(self.not(Rc::clone(&first)), self.aln(&remainder, n)),
            )
        }
    }

    pub fn amn(&self, branches: &Vec<Rc<BDD<S>>>, n: i64) -> Rc<BDD<S>> {
        if branches.len() == 0 {
            if n >= 0 {
                self.mk_const(true)
            } else {
                self.mk_const(false)
            }
        } else {
            let first = &branches[0];
            let remainder = branches[1..].to_vec();

            self.and(
                self.implies(Rc::clone(&first), self.amn(&remainder, n - 1)),
                self.implies(self.not(Rc::clone(&first)), self.amn(&remainder, n)),
            )
        }
    }

    pub fn exn(&self, branches: &Vec<Rc<BDD<S>>>, n: i64) -> Rc<BDD<S>> {
        self.and(self.amn(branches, n), self.aln(branches, n))
    }

    /// existential quantification
    pub fn exists(&self, s: S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
        match b.as_ref() {
            &BDD::False | &BDD::True => b,
            &BDD::Choice(ref t, ref v, ref f) if *v == s => self.or(Rc::clone(t), Rc::clone(f)),
            &BDD::Choice(ref t, ref v, ref f) => self.mk_choice(
                self.exists(s.clone(), Rc::clone(t)),
                v.clone(),
                self.exists(s.clone(), Rc::clone(f)),
            ),
        }
    }

    // forall quantification
    pub fn all(&self, s: S, b: Rc<BDD<S>>) -> Rc<BDD<S>> {
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
            &BDD::Choice(ref t, ref v, ref f) => {
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

    // simplify removes a choice node if both subtrees are equivalent
    pub fn simplify(&self, a: &Rc<BDD<S>>) -> Rc<BDD<S>> {
        let result = match a.as_ref() {
            &BDD::Choice(ref t, _, ref f) if t.as_ref() == f.as_ref() => t,
            _ => a,
        };

        // let dups = self.duplicates(result.clone());

        // assert_eq!(dups.len(), 0);

        Rc::clone(result)
    }
}
