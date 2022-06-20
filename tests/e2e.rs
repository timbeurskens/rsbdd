use rsbdd::bdd::*;
use rsbdd::parser::*;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::rc::Rc;

fn file_assert_eq<P: AsRef<Path>>(file1: P, file2: P, ordering: &[&str]) {
    let ord = ordering
        .iter()
        .enumerate()
        .map(|(i, s)| NamedSymbol {
            name: Rc::new(s.to_string()),
            id: i,
        })
        .collect::<Vec<NamedSymbol>>();

    let f1 = File::open(file1).unwrap();
    let f2 = File::open(file2).unwrap();

    let input_parsed_1 = ParsedFormula::new(&mut BufReader::new(f1), Some(ord.clone()))
        .expect("Could not parse input file 1");
    let input_parsed_2 = ParsedFormula::new(&mut BufReader::new(f2), Some(ord))
        .expect("Could not parse input file 2");

    let input_evaluated_1 = input_parsed_1.eval();
    let input_evaluated_2 = input_parsed_2.eval();

    assert_eq!(input_evaluated_1, input_evaluated_2);
}

#[test]
fn test_fixpoint_1() {
    file_assert_eq(
        "tests/data/test_fixpoint.txt",
        "tests/data/set_abc.txt",
        &["a", "b", "c"],
    );
}

#[test]
fn test_fixpoint_empty() {
    file_assert_eq("tests/data/nu_empty.txt", "tests/data/true.txt", &[]);

    file_assert_eq("tests/data/mu_empty.txt", "tests/data/false.txt", &[]);
}
