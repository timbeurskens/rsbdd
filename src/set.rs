use crate::bdd::*;
use std::ops::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDSet {
    pub bdd: BDD<usize>,
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
    pub fn new(bits: usize) -> BDDSet {
        BDDSet {
            bdd: BDD::False,
            bits: bits,
        }
    }

    pub fn from_bdd(bdd: &BDD<usize>, bits: usize) -> BDDSet {
        BDDSet {
            bdd: bdd.clone(),
            bits: bits,
        }
    }

    pub fn empty(&self) -> Self {
        BDDSet {
            bdd: BDD::False,
            bits: self.bits,
        }
    }

    pub fn universe(&self) -> Self {
        BDDSet {
            bdd: BDD::True,
            bits: self.bits,
        }
    }

    pub fn from_element<T: BDDCategorizable>(e: T, bits: usize) -> Self {
        BDDSet {
            bdd: (0..bits).map(|i| {
                if e.categorize(i) {
                    var(i)
                } else {
                    not(&var(i))
                }
            }).fold(BDD::True, |a, e| {
                and(&a, &e)
            }),
            bits: bits,
        }
    }

    pub fn insert(&self, e: usize) -> Self {
        self.union(&BDDSet::from_element(e, self.bits))
    }

    pub fn union(&self, other: &BDDSet) -> Self {
        BDDSet {
            bdd: or(&self.bdd, &other.bdd),
            bits: self.bits,
        }
    }

    pub fn intersect(&self, other: &BDDSet) -> Self {
        BDDSet {
            bdd: and(&self.bdd, &other.bdd),
            bits: self.bits,
        }
    }

    pub fn complement(&self, other: &BDDSet) -> Self {
        BDDSet {
            bdd: and(&self.bdd, &not(&other.bdd)),
            bits: self.bits,
        }
    }

    pub fn contains<T: BDDCategorizable>(&self, e: T) -> bool {
        let singleton = BDDSet::from_element(e, self.bits);
        self.intersect(&singleton) == singleton
    }
}