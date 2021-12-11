use rsbdd::bdd::{BDD, var, and, not, or, implies, exists, all};

#[test]
fn test_x() {
    assert_eq!(1, 1);
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

}

#[test]
fn test_quantifiers() {
    assert_eq!(exists(0, &or(&var(0), &var(1))), BDD::True);

    assert_eq!(all(0, &var(0)), BDD::False);

    assert_eq!(all(0, &BDD::True), BDD::True);

    assert_eq!(exists(0, &BDD::False), BDD::False);
}