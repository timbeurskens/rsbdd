use crate::bdd::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BDDSet {
    bdd: BDD,
    bits: usize,
}

impl BDDSet {
    pub fn new(bits: usize) -> BDDSet {
        BDDSet {
            bdd: BDD::False,
            bits: bits,
        }
    }

    pub fn from_bdd(bdd: &BDD, bits: usize) -> BDDSet {
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

    pub fn from_element(e: usize, bits: usize) -> Self {
        BDDSet {
            bdd: (0..bits).map(|i| {
                if (e >> i) & 1 == 0 {
                    not(&var(i))
                } else {                
                    var(i)
                }
            }).reduce(|a, e| {
                and(&a, &e)
            }).unwrap(),
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

    pub fn contains(&self, e: usize) -> bool {
        let singleton = BDDSet::from_element(e, self.bits);
        self.intersect(&singleton) == singleton
    }
}