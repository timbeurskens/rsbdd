use macro_syntax::bdd;

#[test]
fn test_basic_syntax_1() {
    let e1 = bdd!(a | -a);
    let e2 = bdd!(a | b | c);
    let e3 = bdd!(false);

    println!("{:#?}\n{:#?}\n{:#?}", e1, e2, e3);
}
