use std::fmt::{Display, Debug};
// pub type Symbol = usize;

pub trait BDDSymbol : PartialOrd + Ord + Display + Debug + Clone + Copy {

}

impl<T> BDDSymbol for T where T: PartialOrd + Ord + Display + Debug + Clone + Copy {

}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BDD<Symbol: BDDSymbol> {
    True,
    False,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Box<BDD<Symbol>>, Symbol, Box<BDD<Symbol>>),
}

impl<S: BDDSymbol> BDD<S> {
    fn simplify(&self) -> Self {
        match self {
            // if lhs equals rhs, then remove the choice from the subtree
            &Self::Choice(ref t, _, ref f) if t == f => t.as_ref().clone(),
            _ => self.clone(),
        }
    }
}

pub fn and<'a, S: BDDSymbol>(a: &'a BDD<S>, b: &'a BDD<S>) -> BDD<S> {
    match (a, b) {
        (&BDD::False, _) | (_, &BDD::False) => BDD::False,
        (&BDD::True, ref f) | (ref f, &BDD::True) => (*f).clone(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => BDD::Choice(Box::new(and(at, b)), va, Box::new(and(af, b))).simplify(),
        (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => BDD::Choice(Box::new(and(bt, a)), vb, Box::new(and(bf, a))).simplify(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => BDD::Choice(Box::new(and(at, bt)), va, Box::new(and(af, bf))).simplify(),
        _ => panic!("unsupported match: {:?} {:?}", a, b),
    }
}

pub fn implies<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>) -> BDD<S> {
    match (a, b) {
        (&BDD::False, _) | (_, &BDD::True) => BDD::True,
        (&BDD::True, f) => f.clone(),
        (&BDD::Choice(ref t, v, ref f), &BDD::False) => BDD::Choice(Box::new(implies(t, &BDD::False)), v, Box::new(implies(f, &BDD::False))).simplify(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => BDD::Choice(Box::new(implies(at, b)), va, Box::new(implies(af, b))).simplify(),
        (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => BDD::Choice(Box::new(implies(bt, a)), vb, Box::new(implies(bf, a))).simplify(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => BDD::Choice(Box::new(implies(at, bt)), va, Box::new(implies(af, bf))).simplify(),
        _ => panic!("unsupported match: {:?} {:?}", a, b),
    }
}

pub fn ite<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>, c: &BDD<S>) -> BDD<S> {
    and(&implies(a, b), &implies(&not(a), c))
}

pub fn eq<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>) -> BDD<S> {
    and(&implies(a, b), &implies(b, a))
}

pub fn not<S: BDDSymbol>(a: &BDD<S>) -> BDD<S> {
    implies(a, &BDD::False)
}

pub fn or<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>) -> BDD<S> {
    not(&and(&not(a), &not(b)))
}

pub fn xor<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>) -> BDD<S> {
    or(&and(&not(a), b), &and(a, &not(b)))
}

/// var constructs a new BDD for a given variable.
pub fn var<S: BDDSymbol>(s: S) -> BDD<S> {
    BDD::Choice(Box::new(BDD::True), s, Box::new(BDD::False))
}

/// amn(vars, n) constructs a new bdd such that at most n variables in vars are true
/// perhaps this can be computed using fixed-point operations?
pub fn amn<S: BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    if vars.len() == 0 {
        BDD::True
    } else {
        let first = vars[0];
        let remainder = vars[1..].to_vec();
    
        if remainder.len() == 0 {
            if n == 0 {
                not(&var(first))
            } else {
                BDD::True
            }
        } else {
            let next_n = if n == 0 { 0 } else { n - 1 };
            // if first then amn(remainder, n-1)
            // if not first then amn(remainder, n)
            ite(&var(first), &amn(&remainder, next_n), &amn(&remainder, n))
        }
    }    
}

/// aln constructs a bdd such that at least n variables in vars are true
pub fn aln<S: BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    if vars.len() == 0 {
        if n == 0 {
            BDD::True
        } else {
            BDD::False
        }
    } else {
        let first = vars[0];
        let remainder = vars[1..].to_vec();

        if remainder.len() == 0 {
            if n == 0 {
                BDD::True
            } else if n == 1 {
                var(first)
            } else {
                BDD::False
            }
        } else {
            let next_n = if n == 0 { 0 } else { n - 1 };
            // if first the aln(remainder, n-1)
            // if not first then aln(remainder, n)
            ite(&var(first), &aln(&remainder, next_n), &aln(&remainder, n))
        }
    }    
}

/// exn constructs a bdd such that exactly n variables in vars are true
pub fn exn<S:BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    and(&amn(vars, n), &aln(vars, n))
}

/// existential quantification
pub fn exists<S: BDDSymbol>(s: S, b: &BDD<S>) -> BDD<S> {
    match b {
        &BDD::False | &BDD::True => b.clone(),
        &BDD::Choice(ref t, v, ref f) if v == s => or(t, f),
        &BDD::Choice(ref t, v, ref f) => BDD::Choice(Box::new(exists(s, t)), v, Box::new(exists(s, f))).simplify(),
    }
}

pub fn all<S: BDDSymbol>(s: S, b: &BDD<S>) -> BDD<S> {
    not(&exists(s, &not(b)))
}

/// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
pub fn fp<S: BDDSymbol, F>(a: &BDD<S>, t: F) -> BDD<S> where F: Fn(&BDD<S>) -> BDD<S> {
    let mut s = a.clone();
    loop {
        let snew = t(&s);
        if snew == s {
            break;
        }
        s = snew;
    }
    s.clone()
}