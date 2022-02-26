use rsbdd::parser::*;
use rsbdd::rewriter::*;
use std::io;
use std::io::BufReader;

#[test]
fn test_simple_rewrite_summation() -> io::Result<()> {
    let rules_str = r#"
    (sum p # (is_person(p) -> p = Alice))
    
    "#;

    let formula_str = r#"
    is_person(Alice)
    "#;
    
    let rules_tree = ParsedFormula::new(&mut BufReader::new(rules_str.as_bytes()))?;
    let formula_tree = ParsedFormula::new(&mut BufReader::new(formula_str.as_bytes()))?;

    dbg!(&rules_tree);
    dbg!(&formula_tree);

    let rewriter = Rewriter::new(rules_tree.bdd, formula_tree.bdd);

    dbg!(&rewriter);

    Ok(())
}