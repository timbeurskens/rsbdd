use macro_syntax::bdd;

#[test]
fn test_basic_syntax_1() {
    let form = bdd! {
        a & b
    };

    print!("{:?}", form);
}
