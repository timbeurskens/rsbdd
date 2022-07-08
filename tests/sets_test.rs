use rsbdd::bdd::BDDEnv;
use rsbdd::set::BDDSet;
use std::rc::Rc;

#[ignore]
#[test]
fn test_set_ops() {
    let bits = 8;

    let env = Rc::new(BDDEnv::new());

    assert_eq!(
        BDDSet::from_element(2, bits, &env),
        BDDSet::from_element(2, bits, &env)
    );

    dbg!(BDDSet::from_element(2, bits, &env));
    dbg!(BDDSet::from_element(3, bits, &env));
    dbg!(BDDSet::from_element(4, bits, &env));

    assert!(BDDSet::from_element(2, bits, &env).contains(2));

    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(3));
    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(1));
    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(4));
    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(6));
    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(7));
    assert!(!BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(8));
    assert!(BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(2));
    assert!(BDDSet::from_element(2, bits, &env)
        .union(&BDDSet::from_element(5, bits, &env))
        .contains(5));

    let set_template = BDDSet::new(bits);

    assert_eq!(
        set_template.empty().complement(set_template.universe()),
        set_template.empty()
    );
    assert_eq!(
        set_template.universe().complement(set_template.empty()),
        set_template.universe()
    );
    assert_eq!(
        set_template.empty().complement(set_template.empty()),
        set_template.empty()
    );
}
