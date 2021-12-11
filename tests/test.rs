use rsbdd::bdd::{BDD, var, and, not, or, implies, exists, all, fp, xor, eq};

#[test]
fn test_equivalence() {
    assert_ne!(var(0), var(1));
    assert_eq!(var(0), var(0));

    assert_eq!(&BDD::True, &BDD::True);
    assert_eq!(&BDD::False, &BDD::False);
    assert_ne!(&BDD::True, &BDD::False);
    assert_ne!(&BDD::False, &BDD::True);

    assert_eq!(&and(&BDD::True, &BDD::True), &BDD::True);
    assert_ne!(and(&var(0), &var(1)), and(&var(1), &var(2)));
    assert_eq!(and(&var(0), &var(1)), and(&var(1), &var(0)));
}

#[test]
fn trivial_bdd() {
    assert_eq!(and(&BDD::True, &BDD::True), BDD::True);
    assert_eq!(and(&BDD::False, &BDD::True), BDD::False);
    assert_eq!(and(&var(0), &BDD::False), BDD::False);
    assert_eq!(and(&var(0), &BDD::True), var(0));

    assert_eq!(or(&BDD::True, &BDD::False), BDD::True);
    assert_eq!(or(&BDD::True, &var(0)), BDD::True);
    assert_eq!(or(&BDD::False, &var(0)), var(0));
}

#[test]
fn test_combined() {
    assert_eq!(and(&or(&var(0), &not(&var(0))), &or(&var(1), &not(&var(1)))), BDD::True);
    assert_eq!(xor(&BDD::True, &BDD::True), BDD::False);
    assert_eq!(xor(&BDD::False, &BDD::True), BDD::True);
    assert_eq!(xor(&BDD::False, &BDD::False), BDD::False);
    assert_eq!(eq(&var(0), &var(0)), BDD::True);
}

#[test]
fn test_quantifiers() {
    assert_eq!(exists(0, &or(&var(0), &var(1))), BDD::True);
    assert_eq!(all(0, &var(0)), BDD::False);
    assert_eq!(all(0, &BDD::True), BDD::True);
    assert_eq!(exists(0, &BDD::False), BDD::False);
}

#[test]
fn test_fixedpoint() {
    assert_eq!(fp(&BDD::False, |x: &BDD| or(x, &BDD::True)), BDD::True);
}