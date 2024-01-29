use std::fs::File;
use std::io;
use std::io::BufReader;
use std::rc::Rc;

use pretty_assertions::assert_eq;

use rsbdd::bdd::*;
use rsbdd::parser::*;
use rsbdd::NamedSymbol;

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
        dbg!(SymbolicBDD::tokenize(
            &mut BufReader::new(test_str.as_bytes()),
            None,
        )?);
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
        let result = ParsedFormula::new(&mut BufReader::new(test_str.as_bytes()), None)?;
        dbg!(&result);

        dbg!(result.eval());
    }

    Ok(())
}

fn parse_and_evaluate(test_str: &str) -> io::Result<Rc<BDD<usize>>> {
    let result = ParsedFormula::new(&mut BufReader::new(test_str.as_bytes()), None)?;
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

    // counting operations
    assert_eq!(parse_and_evaluate("[a] = 1")?, env().var(0));

    Ok(())
}

#[test]
fn test_4_queens_file() -> io::Result<()> {
    let n = 4;

    let input_file = File::open("examples/4_queens.txt").expect("Could not open input file");

    let input_parsed = ParsedFormula::new(&mut BufReader::new(input_file), None)
        .expect("Could not parse input file");

    let input_evaluated = input_parsed.eval();

    let model = input_parsed.env.model(input_evaluated);

    // only retain the queens
    let queens = (0..(n * n))
        .filter(|&i| {
            input_parsed
                .env
                .infer(model.clone(), input_parsed.vars[i].clone())
                .1
        })
        .count();

    assert_eq!(queens, n);

    Ok(())
}

#[test]
fn test_cliques_file() -> io::Result<()> {
    let input_file = File::open("examples/cliques.txt").expect("Could not open input file");

    let input_parsed = ParsedFormula::new(&mut BufReader::new(input_file), None)
        .expect("Could not parse input file");

    let input_evaluated = input_parsed.eval();

    let model = input_parsed.env.model(input_evaluated);

    let var_map: Vec<(NamedSymbol, (bool, bool))> = input_parsed
        .free_vars
        .iter()
        .map(|v| (v.clone(), input_parsed.env.infer(model.clone(), v.clone())))
        .collect();

    dbg!(&var_map);

    let reference: Vec<(NamedSymbol, (bool, bool))> = vec![
        (input_parsed.name2var("a").unwrap(), (false, false)),
        (input_parsed.name2var("f").unwrap(), (true, true)),
        (input_parsed.name2var("g").unwrap(), (true, true)),
        (input_parsed.name2var("b").unwrap(), (false, false)),
        (input_parsed.name2var("d").unwrap(), (true, true)),
        (input_parsed.name2var("e").unwrap(), (true, true)),
        (input_parsed.name2var("c").unwrap(), (false, false)),
    ];

    assert_eq!(var_map, reference);

    Ok(())
}
