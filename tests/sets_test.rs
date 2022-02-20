use rsbdd::bdd::BDDEnv;
use rsbdd::set::BDDSet;
use std::rc::Rc;

#[ignore]
#[test]
fn test_set_ops() {
    let bits = 8;

    let env = Rc::new(BDDEnv::new());

    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone()),
        BDDSet::from_element(2, bits, &env.clone())
    );

    dbg!(BDDSet::from_element(2, bits, &env.clone()));
    dbg!(BDDSet::from_element(3, bits, &env.clone()));
    dbg!(BDDSet::from_element(4, bits, &env.clone()));

    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone()).contains(2),
        true
    );

    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(3),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(1),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(4),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(6),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(7),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(8),
        false
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(2),
        true
    );
    assert_eq!(
        BDDSet::from_element(2, bits, &env.clone())
            .union(&BDDSet::from_element(5, bits, &env.clone()))
            .contains(5),
        true
    );

    let set_template = BDDSet::new(bits);

    assert_eq!(
        set_template.empty().complement(&set_template.universe()),
        set_template.empty()
    );
    assert_eq!(
        set_template.universe().complement(&set_template.empty()),
        set_template.universe()
    );
    assert_eq!(
        set_template.empty().complement(&set_template.empty()),
        set_template.empty()
    );
}
