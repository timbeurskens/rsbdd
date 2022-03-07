use rsbdd::bdd::*;
use rsbdd::parser::*;
use std::io;
use std::io::BufReader;
use std::rc::Rc;

#[test]
fn test_basic_tokens() -> io::Result<()> {
    let test_strs: Vec<&str> = vec![
        "a\0",
        "a & b\0",
        "alpha | beta\0",
        "(alpha & beta )\0",
        "( alpha & beta)\0",
        "  a \0",
        "a  &b\0",
        "a|b\0",
        "a | b\0",
        "a|b|c\0",
        "(a&b)|c\0",
        "(a)and(b)\0",
        "mand\0",
        "m and\0",
        "true",
        "false",
    ];

    for test_str in test_strs {
        dbg!(test_str);
        dbg!(SymbolicBDD::tokenize(&mut BufReader::new(
            test_str.as_bytes()
        ))?);
    }

    Ok(())
}

#[test]
fn test_parser() -> io::Result<()> {
    let test_strs: Vec<&str> = vec![
        "a\0",
        "a & b\0",
        "alpha | beta\0",
        "(alpha & beta )\0",
        "( alpha & beta)\0",
        "  a \0",
        "a  &b\0",
        "a|b\0",
        "a | b\0",
        "a|b|c\0",
        "(a&b)|c\0",
        "(a)and(b)\0",
        "mand\0",
        "a|a|a\0",
    ];

    for test_str in test_strs {
        dbg!(test_str);
        let result = ParsedFormula::new(&mut BufReader::new(test_str.as_bytes()))?;
        dbg!(&result);

        dbg!(result.eval());
    }

    Ok(())
}

fn parse_and_evaluate(test_str: &str) -> io::Result<Rc<BDD<usize>>> {
    let result = ParsedFormula::new(&mut BufReader::new(test_str.as_bytes()))?;
    Ok(Rc::new(BDD::<usize>::from(result.eval().as_ref().clone())))
}

fn env() -> BDDEnv<usize> {
    BDDEnv::new()
}

#[test]
fn test_parsed_solutions() -> io::Result<()> {
    // constants are evaluated to true and
    assert_eq!(parse_and_evaluate("true")?, env().mk_const(true));
    assert_eq!(parse_and_evaluate("false")?, env().mk_const(false));

    // variables are evaluated to choice nodes
    assert_eq!(parse_and_evaluate("a")?, env().var(0));

    // simple tautologies and contradictions
    assert_eq!(parse_and_evaluate("-a & a")?, env().mk_const(false));
    assert_eq!(parse_and_evaluate("-a | a")?, env().mk_const(true));
    assert_eq!(parse_and_evaluate("-true & true")?, env().mk_const(false));
    assert_eq!(parse_and_evaluate("-true | true")?, env().mk_const(true));
    assert_eq!(parse_and_evaluate("-false & false")?, env().mk_const(false));
    assert_eq!(parse_and_evaluate("-false | false")?, env().mk_const(true));

    Ok(())
}
