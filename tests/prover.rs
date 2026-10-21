use std::fs;
use std::io;
use std::rc::Rc;

use rsbdd::bdd::BDDEnv;
use rsbdd::parser::ParsedFormula;
use rsbdd::prover::{Outcome, Prover};

fn prove(formula: &str) -> io::Result<Outcome> {
    let parsed = ParsedFormula::parse(Rc::new(BDDEnv::new()), &mut formula.as_bytes(), None)?;
    Prover::prove(&parsed, false)
}

fn is_theorem(formula: &str) -> io::Result<bool> {
    Ok(matches!(prove(formula)?, Outcome::Proven(..)))
}

fn is_not_theorem(formula: &str) -> io::Result<bool> {
    Ok(matches!(prove(formula)?, Outcome::Disproven(_)))
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
fn test_countermodels() -> io::Result<()> {
    let countermodel = |formula: &str| -> io::Result<Vec<String>> {
        match prove(formula)? {
            Outcome::Disproven(Some(model)) => {
                Ok(model.to_string().lines().map(String::from).collect())
            }
            _ => panic!("no countermodel of {formula}"),
        }
    };
    assert_eq!(
        countermodel("(exists y # p(y)) => (forall y # p(y))")?,
        ["domain: e1, e2", "p(e1)", "-p(e2)", "sk1 = e1", "sk2 = e2"]
    );
    assert_eq!(countermodel("exists x, y # -(x = y)")?, ["domain: e1"]);
    let five = "(exists a, b, c, d, e # -(a = b) & -(a = c) & -(a = d) & -(a = e) & -(b = c) & -(b = d) & \
        -(b = e) & -(c = d) & -(c = e) & -(d = e)) => exists x, y # p(x) <=> -p(y)";
    assert_eq!(countermodel(five)?[0], "domain: e1, e2, e3, e4, e5");
    let group = "((forall x, y, z # m(m(x, y), z) = m(x, m(y, z))) & (forall x # m(e, x) = x & m(x, e) = x) & \
        (forall x # m(i(x), x) = e & m(x, i(x)) = e)) => forall x, y # m(x, y) = m(y, x)";
    assert_eq!(countermodel(group)?[0], "domain: e1, e2, e3, e4, e5, e6");
    assert_eq!(
        countermodel("(forall x # f(f(x)) = x) => forall x # f(x) = x")?,
        ["domain: e1, e2", "f(e1) = e2", "f(e2) = e1", "sk2 = e1"]
    );
    assert_eq!(
        countermodel("p(A) -> true; p(B)")?,
        ["domain: e1, e2", "p(e1)", "-p(e2)", "A = e1", "B = e2"]
    );
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

/// Typed formulas: declared types, typed binders and polymorphic signatures
#[test]
fn test_types() -> io::Result<()> {
    let theorems = [
        // PUZ139_1: the mixture of syrups is a syrup
        "let beverage, syrup: type; let coffee: beverage; let vanilla, caramel: syrup; \
         let mixture: pi T # (T, syrup) -> T; let awake: (beverage) -> bool; \
         (forall s: syrup # awake(mixture(beverage, coffee, s))) => \
         awake(mixture(beverage, coffee, mixture(syrup, caramel, vanilla)))",
        "let a: type; let c: a; let id: pi T # (T) -> T; (forall x: a # id(a, x) = x) => id(a, c) = c",
        "let list: (type) -> type; let nil: pi T # list(T); forall T: type # exists l: list(T) # l = nil(T)",
        // every type has an element
        "let a: type; let p: (a) -> bool; (forall x: a # p(x)) => exists x: a # p(x)",
    ];
    for formula in theorems {
        assert!(is_theorem(formula)?, "{formula}");
    }
    let not_theorems = [
        // the untyped formula is a theorem: a type with one element does not make every type a singleton
        "let a, b: type; let c: b; (forall x: a, y: a # x = y) => -(exists x: b, y: b # -(x = y))",
        // the axiom is about mixtures of coffee, not coffee itself
        "let beverage, syrup: type; let coffee: beverage; let awake: (beverage) -> bool; \
         let mixture: pi T # (T, syrup) -> T; \
         (forall s: syrup # awake(mixture(beverage, coffee, s))) => awake(coffee)",
    ];
    for formula in not_theorems {
        assert!(is_not_theorem(formula)?, "{formula}");
    }
    Ok(())
}
