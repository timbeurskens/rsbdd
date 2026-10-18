use rsbdd::bdd::BDD;
use rsbdd::parser::*;
use std::fs;
use std::io;
use std::io::BufReader;

fn evaluate(formula: &str) -> io::Result<BDD<usize>> {
    let parsed = ParsedFormula::new(&mut BufReader::new(formula.as_bytes()), None)?;
    Ok(BDD::<usize>::from(parsed.eval()))
}

fn is_true(formula: &str) -> io::Result<bool> {
    Ok(evaluate(formula)? == evaluate("true")?)
}

fn is_false(formula: &str) -> io::Result<bool> {
    Ok(evaluate(formula)? == evaluate("false")?)
}

#[test]
fn test_peano() -> io::Result<()> {
    assert!(is_true(&fs::read_to_string("examples/peano.txt")?)?);
    Ok(())
}

#[test]
fn test_summation() -> io::Result<()> {
    assert!(is_true(
        "(sum p # is_person(p) -> p = Alice) & is_person(Alice)"
    )?);
    assert!(is_false(
        "(sum p # is_person(p) -> p = Alice) & is_person(Bob)"
    )?);
    Ok(())
}

#[test]
fn test_multiple_rules() -> io::Result<()> {
    let rules = "is_person(Alice) -> true; is_person(Bob) -> true; is_person(Chair) -> false;";
    assert!(is_true(&format!("{rules} is_person(Bob)"))?);
    assert!(is_false(&format!("{rules} is_person(Chair)"))?);
    assert!(is_false(&format!("{rules} is_person(Dave)"))?);
    Ok(())
}

#[test]
fn test_open_atoms() -> io::Result<()> {
    assert!(is_false("color(H1, Red) & -color(H1, Red)")?);
    assert!(!is_false("color(H1, Red) & -color(H2, Red)")?);
    Ok(())
}

#[test]
fn test_domain_quantification() -> io::Result<()> {
    let domain = "person(Alice) -> true; person(Bob) -> true;";
    assert!(is_false(&format!(
        "{domain} (forall p # person(p) => likes(p, Tea)); -likes(Bob, Tea)"
    ))?);
    assert!(is_true(&format!(
        "{domain} exists p # person(p) & p = Bob"
    ))?);
    Ok(())
}

#[test]
fn test_cyclic_rules() -> io::Result<()> {
    assert!(is_false("(p(A) -> p(A)) & p(A)")?);
    Ok(())
}

#[test]
fn test_misplaced_rules() {
    assert!(evaluate("a | (p(A) -> true)").is_err());
    assert!(evaluate("(sum x # a) & p(A)").is_err());
    assert!(evaluate("(sum x # p(A) -> true) & p(A)").is_err());
}

#[test]
fn test_einstein() -> io::Result<()> {
    let riddle = fs::read_to_string("examples/einstein.txt")?;
    assert!(!is_false(&riddle)?);
    assert!(is_false(&format!(
        "{riddle} -(exists h # has_nationality(h, German) & keeps(h, Fish))"
    ))?);
    Ok(())
}

#[test]
fn test_term_equality_without_rules() -> io::Result<()> {
    assert!(is_true("(A = A) & -(A = B)")?);
    assert!(is_false("A = B")?);
    Ok(())
}

#[test]
fn test_peano_arithmetic() -> io::Result<()> {
    assert!(is_true(&fs::read_to_string("examples/peano_arith.txt")?)?);
    Ok(())
}

#[test]
fn test_group_theory() -> io::Result<()> {
    let theory = fs::read_to_string("examples/group.txt")?;
    assert!(is_true(&theory)?);
    assert!(is_false(&format!("{theory}; mul(a, b) = mul(b, a)"))?);
    Ok(())
}

#[test]
fn test_peano_solve() -> io::Result<()> {
    let problem = fs::read_to_string("examples/peano_solve.txt")?;
    assert!(!is_false(&problem)?);
    assert!(is_false(&format!(
        "{problem}; exists x # is_nat(x) & val(X, x) & -(x = suc(_0) | x = suc(suc(suc(_0))))"
    ))?);
    Ok(())
}

#[test]
fn test_induction() -> io::Result<()> {
    let plus = "sum y # plus(_0, y) -> y; sum x, y # plus(suc(x), y) -> suc(plus(x, y));";
    assert!(is_true(&fs::read_to_string("examples/induction.txt")?)?);
    assert!(evaluate(&format!("{plus} induct x # plus(x, _0) => x")).is_err());
    Ok(())
}
