
pub type Symbol = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BDD {
    True,
    False,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Box<BDD>, Symbol, Box<BDD>),
}

impl BDD {
    fn simplify(&self) -> BDD {
        match self {
            // if lhs equals rhs, then remove the choice from the subtree
            &BDD::Choice(ref t, _, ref f) if t == f => t.as_ref().clone(),
            _ => self.clone(),
        }
    }
}

pub fn and<'a>(a: &'a BDD, b: &'a BDD) -> BDD {
    match (a, b) {
        (&BDD::False, _) | (_, &BDD::False) => BDD::False,
        (&BDD::True, ref f) | (ref f, &BDD::True) => (*f).clone(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => BDD::Choice(Box::new(and(at, b)), va, Box::new(and(af, b))).simplify(),
        (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => BDD::Choice(Box::new(and(bt, a)), vb, Box::new(and(bf, a))).simplify(),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => BDD::Choice(Box::new(and(at, bt)), va, Box::new(and(af, bf))).simplify(),
        _ => panic!("unsupported match: {:?} {:?}", a, b),
    }
}

pub fn implies(a: &BDD, b: &BDD) -> BDD {
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

pub fn eq(a: &BDD, b: &BDD) -> BDD {
    and(&implies(a, b), &implies(b, a))
}

pub fn not(a: &BDD) -> BDD {
    implies(a, &BDD::False)
}

pub fn or(a: &BDD, b: &BDD) -> BDD {
    not(&and(&not(a), &not(b)))
}

pub fn xor(a: &BDD, b: &BDD) -> BDD {
    or(&and(&not(a), b), &and(a, &not(b)))
}

/// var constructs a new BDD for a given variable.
pub fn var(s: Symbol) -> BDD {
    BDD::Choice(Box::new(BDD::True), s, Box::new(BDD::False))
}

/// existential quantification
pub fn exists(s: Symbol, b: &BDD) -> BDD {
    match b {
        &BDD::False | &BDD::True => b.clone(),
        &BDD::Choice(ref t, v, ref f) if v == s => or(t, f),
        &BDD::Choice(ref t, v, ref f) => BDD::Choice(Box::new(exists(s, t)), v, Box::new(exists(s, f))).simplify(),
    }
}

pub fn all(s: Symbol, b: &BDD) -> BDD {
    not(&exists(s, &not(b)))
}

/// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
pub fn fp<F>(a: &BDD, t: F) -> BDD where F: Fn(&BDD) -> BDD {
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