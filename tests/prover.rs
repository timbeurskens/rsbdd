use std::fs;
use std::io;
use std::rc::Rc;

use rsbdd::bdd::BDDEnv;
use rsbdd::parser::ParsedFormula;
use rsbdd::prover::{Outcome, Prover};

fn prove(formula: &str) -> io::Result<Outcome> {
    let parsed = ParsedFormula::parse(Rc::new(BDDEnv::new()), &mut formula.as_bytes(), None)?;
    Prover::prove(&parsed)
}

fn is_theorem(formula: &str) -> io::Result<bool> {
    Ok(matches!(prove(formula)?, Outcome::Proven(_)))
}

fn is_not_theorem(formula: &str) -> io::Result<bool> {
    Ok(matches!(prove(formula)?, Outcome::Disproven))
}

/// Problems from Pelletier, "Seventy-five problems for testing automatic theorem provers" (1986)
#[test]
fn test_pelletier() -> io::Result<()> {
    let problems = [
        "(p => q) <=> (-q => -p)",
        "((p <=> q) <=> r) <=> (p <=> (q <=> r))",
        "exists y # forall x # (f(y) => f(x))",
        "exists x # forall y, z # ((p(y) => q(z)) => (p(x) => q(x)))",
        "(forall x, y # exists z # forall w # ((p(x) & q(y)) => (r(z) & s(w)))) => \
            ((exists x, y # (p(x) & q(y))) => exists z # r(z))",
        "((exists x # (p => f(x))) & (exists x # (f(x) => p))) => exists x # (p <=> f(x))",
        "-(exists x # forall y # (f(y, x) <=> -f(y, y)))",
        "(forall x, y # (q(x, y) <=> forall z # (f(z, x) <=> f(z, y)))) => forall x, y # (q(x, y) <=> q(y, x))",
        "((a = b | c = d) & (a = c | b = d)) => (a = d | b = c)",
        "(exists z, w # forall x, y # (f(x, y) <=> (x = z & y = w))) => \
            exists z # forall x # ((exists w # forall y # (f(x, y) <=> y = w)) <=> x = z)",
        "(forall x, y, z # f(x, f(y, z)) = f(f(x, y), z)) => \
            forall x, y, z, w # f(x, f(y, f(z, w))) = f(f(f(x, y), z), w)",
    ];
    for problem in problems {
        assert!(is_theorem(problem)?, "{problem}");
    }
    Ok(())
}

/// Andrews' challenge, Pelletier problem 34, which needs many more steps than the other problems
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn test_andrews_challenge() -> io::Result<()> {
    assert!(is_theorem(
        "((exists x # forall y # (p(x) <=> p(y))) <=> ((exists x # q(x)) <=> (forall y # q(y)))) <=> \
            ((exists x # forall y # (q(x) <=> q(y))) <=> ((exists x # p(x)) <=> (forall y # p(y))))"
    )?);
    Ok(())
}

#[test]
fn test_not_theorems() -> io::Result<()> {
    for formula in [
        "(forall x # p(x)) <=> p(A)",
        "exists x, y # -(x = y)",
        "(exists x # p(x)) => forall x # p(x)",
        "a & -a",
    ] {
        assert!(is_not_theorem(formula)?, "{formula}");
    }
    Ok(())
}

#[test]
fn test_examples() -> io::Result<()> {
    for example in [
        "fol/barber.txt",
        "fol/boolean_group.txt",
        "fol/dreadbury.txt",
        "fol/drinker.txt",
        "fol/steamroller.txt",
        "right_identity.txt",
        "group.txt",
    ] {
        let formula = fs::read_to_string(format!("examples/{example}"))?;
        assert!(is_theorem(&formula)?, "{example}");
    }
    Ok(())
}

/// Formulas valid in finite mode that are theorems of `--prove`, and one that is not valid in finite mode
#[test]
fn test_compatibility() -> io::Result<()> {
    let plus = "sum y # plus(_0, y) -> y; sum x, y # plus(suc(x), y) -> suc(plus(x, y));";
    for formula in [
        "p(A) -> true; -p(B)",
        "is_nat(_0) -> true; sum n # is_nat(suc(n)) -> is_nat(n); is_nat(suc(_0)) & -is_nat(A)",
        "sum x # p(x) -> q(x); p(A) -> r; p(A) <=> (q(A) & r)",
        "[a, b] = 1 => (a ^ b)",
        "[a] <= [a, b]",
        "forall x # ([p(x), q(x)] = 1 => (p(x) ^ q(x)))",
        "(p(A) & -p(B))[p(_) := q(_)] <=> (q(A) & -q(B))",
        "exists p(_) # p(A)",
        "(lfp X # a | X) <=> a",
        "sum x # f(x) -> h(x); h(A) -> B; f(A) = B",
        "e(A, B) -> true; sum x, y # r(x, y) -> e(x, y) | exists z # e(x, z) & r(z, y); r(A, B)",
        &format!("{plus} induct x # plus(x, _0) = x; forall x # plus(x, _0) = x"),
    ] {
        assert!(is_theorem(formula)?, "{formula}");
    }
    assert!(is_not_theorem(
        "sum x # p(x) -> q(x); p(A) -> r; q(A) <=> r"
    )?);
    Ok(())
}

#[test]
fn test_unsupported() {
    assert!(prove("p(A) -> true; forall x # [p(x)] = 1").is_err());
}
