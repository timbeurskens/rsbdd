use std::fmt::{Debug, Display};

pub trait BDDSymbol: Ord + Display + Debug + Clone + Copy {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Copy {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BDD<Symbol: BDDSymbol> {
    True,
    False,
    // Choice (true-subtree, symbol, false-subtree)
    Choice(Box<BDD<Symbol>>, Symbol, Box<BDD<Symbol>>),
}

impl<S: BDDSymbol> BDD<S> {
    pub fn mk_choice(true_subtree: Box<BDD<S>>, symbol: S, false_subtree: Box<BDD<S>>) -> BDD<S> {
        BDD::Choice(true_subtree, symbol, false_subtree).simplify()
    }

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
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => {
            BDD::mk_choice(Box::new(and(at, b)), va, Box::new(and(af, b)))
        }
        (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => {
            BDD::mk_choice(Box::new(and(bt, a)), vb, Box::new(and(bf, a)))
        }
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => {
            BDD::mk_choice(Box::new(and(at, bt)), va, Box::new(and(af, bf)))
        }
        _ => panic!("unsupported match: {:?} {:?}", a, b),
    }
}

pub fn implies<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>) -> BDD<S> {
    match (a, b) {
        (&BDD::False, _) | (_, &BDD::True) => BDD::True,
        (&BDD::True, f) => f.clone(),
        (&BDD::Choice(ref t, v, ref f), &BDD::False) => BDD::mk_choice(
            Box::new(implies(t, &BDD::False)),
            v,
            Box::new(implies(f, &BDD::False)),
        ),
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(_, vb, _)) if va < vb => {
            BDD::mk_choice(Box::new(implies(at, b)), va, Box::new(implies(af, b)))
        }
        (&BDD::Choice(_, va, _), &BDD::Choice(ref bt, vb, ref bf)) if vb < va => {
            BDD::mk_choice(Box::new(implies(bt, a)), vb, Box::new(implies(bf, a)))
        }
        (&BDD::Choice(ref at, va, ref af), &BDD::Choice(ref bt, vb, ref bf)) if va == vb => {
            BDD::mk_choice(Box::new(implies(at, bt)), va, Box::new(implies(af, bf)))
        }
        _ => panic!("unsupported match: {:?} {:?}", a, b),
    }
}

/// ite computes if a then b else c
pub fn ite<S: BDDSymbol>(a: &BDD<S>, b: &BDD<S>, c: &BDD<S>) -> BDD<S> {
    and(&implies(a, b), &implies(&not(a), c))
}

/// eq computes a iff b
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
    BDD::mk_choice(Box::new(BDD::True), s, Box::new(BDD::False))
}

// for all variables in vars at least n must be true
pub fn aln<S: BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    let mut vars = vars.clone();
    vars.sort();

    aln_recursive(&vars, n as i64)
}

fn aln_recursive<S: BDDSymbol>(vars: &Vec<S>, n: i64) -> BDD<S> {
    if vars.len() == 0 {
        if n > 0 {
            BDD::False
        } else {
            BDD::True
        }
    } else {
        let first = vars[0];
        let remainder = vars[1..].to_vec();

        BDD::mk_choice(
            Box::new(aln_recursive(&remainder, n - 1)),
            first,
            Box::new(aln_recursive(&remainder, n)),
        )
    }
}

// for all variables in vars at most n must be true
pub fn amn<S: BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    let mut vars = vars.clone();
    vars.sort();

    amn_recursive(&vars, n as i64)
}

fn amn_recursive<S: BDDSymbol>(vars: &Vec<S>, n: i64) -> BDD<S> {
    if vars.len() == 0 {
        if n >= 0 {
            BDD::True
        } else {
            BDD::False
        }
    } else {
        let first = vars[0];
        let remainder = vars[1..].to_vec();

        BDD::mk_choice(
            Box::new(amn_recursive(&remainder, n - 1)),
            first,
            Box::new(amn_recursive(&remainder, n)),
        )
    }
}

/// exn constructs a bdd such that exactly n variables in vars are true
pub fn exn<S: BDDSymbol>(vars: &Vec<S>, n: usize) -> BDD<S> {
    and(&amn(vars, n), &aln(vars, n))
}

/// existential quantification
pub fn exists<S: BDDSymbol>(s: S, b: &BDD<S>) -> BDD<S> {
    match b {
        &BDD::False | &BDD::True => b.clone(),
        &BDD::Choice(ref t, v, ref f) if v == s => or(t, f),
        &BDD::Choice(ref t, v, ref f) => {
            BDD::mk_choice(Box::new(exists(s, t)), v, Box::new(exists(s, f)))
        }
    }
}

pub fn all<S: BDDSymbol>(s: S, b: &BDD<S>) -> BDD<S> {
    not(&exists(s, &not(b)))
}

/// fp computes the fixed point starting from the initial state a, by iteratively applying the transformer t.
pub fn fp<S: BDDSymbol, F>(a: &BDD<S>, t: F) -> BDD<S>
where
    F: Fn(&BDD<S>) -> BDD<S>,
{
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

pub fn model<S: BDDSymbol>(a: &BDD<S>) -> BDD<S> {
    match a {
        &BDD::Choice(ref t, v, ref f) => {
            let lhs = model(t);
            let rhs = model(f);
            if lhs != BDD::False {
                and(&lhs, &var(v))
            } else if rhs != BDD::False {
                and(&not(&var(v)), &rhs)
            } else {
                BDD::False
            }
        }
        &BDD::True => BDD::True,
        &BDD::False => BDD::False,
    }
}

// determine whether variable b is always true or false for a given bdd a
// returns a tuple (bool, bool) where the first item determines whether b is bound
// the second item determines the truth value for b
pub fn infer<S: BDDSymbol>(a: &BDD<S>, b: S) -> (bool, bool) {
    let ff = implies(a, &var(b));
    match ff {
        BDD::Choice(_, _, _) => (false, false),
        BDD::True => (true, true),
        BDD::False => (true, false),
    }
}