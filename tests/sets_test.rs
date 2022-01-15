use rsbdd::set::BDDSet;

#[test]
fn test_set_ops() {
    let bits = 8;

    assert_eq!(BDDSet::from_element(2, bits), BDDSet::from_element(2, bits));

    dbg!(BDDSet::from_element(2, bits));
    dbg!(BDDSet::from_element(3, bits));
    dbg!(BDDSet::from_element(4, bits));

    assert_eq!(BDDSet::from_element(2, bits).contains(2), true);

    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(3), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(1), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(4), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(6), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(7), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(8), false);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(2), true);
    assert_eq!(BDDSet::from_element(2, bits).union(&BDDSet::from_element(5, bits)).contains(5), true);

    let set_template = BDDSet::new(bits);

    assert_eq!(set_template.empty().complement(&set_template.universe()), set_template.empty());
    assert_eq!(set_template.universe().complement(&set_template.empty()), set_template.universe());
    assert_eq!(set_template.empty().complement(&set_template.empty()), set_template.empty());
}