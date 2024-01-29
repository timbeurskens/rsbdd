use std::cell::RefCell;
use std::rc::Rc;

use crate::bdd::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDSet {
    env: Rc<BDDEnv<usize>>,
    pub bdd: RefCell<Rc<BDD<usize>>>,
    bits: usize,
}

pub trait BDDCategorizable {
    fn categorize(&self, c: usize) -> bool;
}

impl BDDCategorizable for usize {
    fn categorize(&self, c: usize) -> bool {
        (self >> c) & 1 == 0
    }
}

impl BDDSet {
    pub fn new(bits: usize) -> Self {
        let env = BDDEnv::new();
        Self::with_env(bits, &Rc::new(env))
    }

    pub fn with_env(bits: usize, env: &Rc<BDDEnv<usize>>) -> Self {
        Self {
            env: env.clone(),
            bdd: RefCell::new(env.mk_const(false)),
            bits,
        }
    }

    pub fn from_bdd(bdd: &Rc<BDD<usize>>, bits: usize, env: &Rc<BDDEnv<usize>>) -> Self {
        Self {
            env: env.clone(),
            bdd: RefCell::new(bdd.clone()),
            bits,
        }
    }

    pub fn empty(&self) -> &Self {
        self.bdd.replace(self.env.mk_const(false));
        self
    }

    pub fn universe(&self) -> &Self {
        self.bdd.replace(self.env.mk_const(true));
        self
    }

    pub fn from_element<T: BDDCategorizable>(e: T, bits: usize, env: &Rc<BDDEnv<usize>>) -> Self {
        let new_set = Self::with_env(bits, env);
        new_set.insert(e);

        new_set
    }

    pub fn insert<T: BDDCategorizable>(&self, e: T) -> &Self {
        let new_item = (0..self.bits)
            .map(|i| {
                if e.categorize(i) {
                    self.env.var(i)
                } else {
                    self.env.not(self.env.var(i))
                }
            })
            .fold(self.env.mk_const(true), |a, e| self.env.and(a, e));

        let _self = self.bdd.borrow().clone();

        self.bdd.replace(self.env.or(_self, new_item));
        self
    }

    pub fn union(&self, other: &Self) -> &Self {
        let _self = self.bdd.borrow().clone();
        self.bdd
            .replace(self.env.or(_self, other.bdd.borrow().clone()));
        self
    }

    pub fn intersect(&self, other: &Self) -> &Self {
        let _self = self.bdd.borrow().clone();

        self.bdd
            .replace(self.env.and(_self, other.bdd.borrow().clone()));
        self
    }

    pub fn complement(&self, other: &Self) -> &Self {
        let new: Rc<BDD<usize>> = self.bdd.borrow().clone();

        self.bdd
            .replace(self.env.and(new, other.bdd.borrow().clone()));

        self
    }

    pub fn contains<T: BDDCategorizable>(&self, e: T) -> bool {
        let singleton = Self::from_element(e, self.bits, &self.env);
        self.intersect(&singleton) == &singleton
    }
}
