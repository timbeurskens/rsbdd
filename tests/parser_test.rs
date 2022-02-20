use rsbdd::parser::*;
use std::io;
use std::io::BufReader;

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
        "false"
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
