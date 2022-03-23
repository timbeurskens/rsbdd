use rsbdd::bdd::*;
use rsbdd::parser::*;
use std::fs::File;
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

    // counting operations
    assert_eq!(parse_and_evaluate("[a] = 1")?, env().var(0));

    Ok(())
}

#[test]
fn test_4_queens_file() -> io::Result<()> {
    let n = 4;

    let input_file = File::open("examples/4_queens.txt").expect("Could not open input file");

    let input_parsed =
        ParsedFormula::new(&mut BufReader::new(input_file)).expect("Could not parse input file");

    let input_evaluated = input_parsed.eval();

    let model = input_parsed.env.borrow().model(input_evaluated);

    // only retain the queens
    let queens: Vec<usize> = (0..(n * n))
        .filter(|&i| {
            input_parsed
                .env
                .borrow()
                .infer(
                    model.clone(),
                    input_parsed.name2var(input_parsed.vars[i].as_str()),
                )
                .1
        })
        .collect();

    assert_eq!(queens.len(), n);

    Ok(())
}

#[test]
fn test_cliques_file() -> io::Result<()> {
    let input_file = File::open("examples/cliques.txt").expect("Could not open input file");

    let input_parsed =
        ParsedFormula::new(&mut BufReader::new(input_file)).expect("Could not parse input file");

    let input_evaluated = input_parsed.eval();

    let model = input_parsed.env.borrow().model(input_evaluated);

    let var_map: Vec<(String, (bool, bool))> = input_parsed
        .free_vars
        .iter()
        .map(|v| {
            (
                v.clone(),
                input_parsed
                    .env
                    .borrow()
                    .infer(model.clone(), input_parsed.name2var(v)),
            )
        })
        .collect();

    dbg!(&var_map);

    let reference: Vec<(String, (bool, bool))> = vec![
        ("a".to_string(), (false, false)),
        ("f".to_string(), (true, true)),
        ("g".to_string(), (true, true)),
        ("b".to_string(), (false, false)),
        ("d".to_string(), (true, true)),
        ("e".to_string(), (true, true)),
        ("c".to_string(), (false, false)),
    ];

    assert_eq!(var_map, reference);

    Ok(())
}
