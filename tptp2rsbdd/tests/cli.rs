use std::io::Cursor;
use std::process::Command;
use std::rc::Rc;

use rsbdd::bdd::BDDEnv;
use rsbdd::parser::{ParsedFormula, SymbolicBDD, SymbolicBDDToken};
use rsbdd::prover::{Outcome, Prover};
use tptp2rsbdd::translate::KEYWORDS;

fn translate(path: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tptp2rsbdd"))
        .args(["--file", path])
        .env_remove("TPTP")
        .output()
        .expect("failed to run tptp2rsbdd");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("non UTF-8 output")
}

fn is_theorem(formula: &str) -> bool {
    let parsed = ParsedFormula::parse(Rc::new(BDDEnv::new()), &mut Cursor::new(formula), None)
        .expect("parse error");
    matches!(
        Prover::prove(&parsed, false).expect("prover error"),
        Outcome::Proven(..)
    )
}

#[test]
fn test_puzzles() {
    for problem in ["PUZ001-1", "PUZ001+1"] {
        let formula = translate(&format!("tests/data/{problem}.p"));
        // the version changes with every release
        let unversioned = formula.replace(env!("CARGO_PKG_VERSION"), "[version]");
        insta::assert_snapshot!(problem, unversioned);
        assert!(is_theorem(&formula), "{problem} is not proven");
    }
}

fn is_identifier(word: &str) -> bool {
    let tokens = SymbolicBDD::tokenize(&mut Cursor::new(word), None).expect("tokenize error");
    matches!(tokens.first(), Some(SymbolicBDDToken::Ident(_)))
}

/// The keywords that are renamed are the identifiers that the rsbdd tokenizer does not read as names
#[test]
fn test_keywords() {
    for keyword in KEYWORDS {
        assert!(!is_identifier(keyword), "{keyword} is not an rsbdd keyword");
    }
    assert!(is_identifier("agatha"));
}
