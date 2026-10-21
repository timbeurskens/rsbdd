use std::process::Command;

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rsbdd"))
        .args(args)
        .output()
        .expect("failed to run rsbdd");

    format!(
        "$ rsbdd {}\nstatus: {}\n--- stdout\n{}--- stderr\n{}",
        args.join(" "),
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn snapshot_file(name: &str, path: &std::path::Path) {
    let path = path
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .expect("file outside of crate")
        .to_str()
        .expect("non UTF-8 path");
    let output = [
        run(&["-t", "-f", "true", path]),
        run(&["-v", path]),
        run(&["-m", "-t", "-f", "true", path]),
    ]
    .join("\n");

    insta::assert_snapshot!(name, output);
}

#[test]
fn test_examples() {
    insta::glob!("../examples", "*.txt", |p| snapshot_file("examples", p));
}

#[test]
fn test_first_order_examples() {
    insta::glob!("../examples/fol", "*.txt", |p| {
        let path = p
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .expect("file outside of crate")
            .to_str()
            .expect("non UTF-8 path");
        insta::assert_snapshot!(run(&["--prove", path]));
    });
}

#[test]
fn test_data() {
    insta::glob!("data", "*.txt", |p| snapshot_file("data", p));
}

fn run_formulas(formulas: &[&str]) -> String {
    formulas
        .iter()
        .map(|f| run(&["-t", &format!("--evaluate={f}")]))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_parse_errors() {
    insta::assert_snapshot!(run_formulas(&[
        "",
        "a &",
        "&",
        "(a",
        "a)",
        "a b",
        "[a, b",
        "[a] = ",
        "exists # a",
        "if a then b",
    ]));
}

#[test]
fn test_formula_edge_cases() {
    insta::assert_snapshot!(run_formulas(&[
        "true & false",
        "-(-a)",
        "a & -a",
        "a ^ a",
        "a nand a",
        "a nor a",
        "a <= b",
        "a <=> b",
        "if a then b else c",
        "[a, b] = 3",
        "[a, b] >= 0",
        "[a, b] < 0",
        "[a, b] > 2",
        "[a, a] = 1",
        "[a] = [b]",
        "[a, b] < [c]",
        "[a] > []",
        "exists z # a",
        "exists a, a # a & b",
        "forall a # a",
        "forall a # a | -a",
        "nu X # X & a",
        "mu X # X | a",
        "mu X # X",
        "nu X # X",
        "{undefined}",
        "\"only a comment\" a",
        "a and b or not c",
    ]));
}

#[test]
fn test_default_output() {
    insta::assert_snapshot!(run(&["examples/4_queens.txt"]));
}

#[test]
fn test_witness() {
    insta::assert_snapshot!(run(&["--prove", "--witness", "examples/fol/sudoku.txt"]));
}

#[test]
fn test_rewritten() {
    insta::assert_snapshot!(run(&[
        "-w",
        "-",
        "tests/data/rewrite_open_atoms_is_true.txt"
    ]));
}

#[test]
fn test_type_errors() {
    insta::assert_snapshot!(run_formulas(&[
        "let a: type; let c: a; let f: (a) -> a; f(c, c) = c",
        "let a, b: type; let c: a; let p: (b) -> bool; p(c)",
        "let a: type; let c: a; c",
        "let a: type; let p: (a) -> bool; exists x: a # p(p(x))",
        "let a, b: type; let c: a; let d: b; c = d",
        "forall x: a # true",
        "let f: pi T # (T) -> T; let a: type; let c: a; f(c, c) = c",
        "let a: type; let a: type; true",
        "let type: type; true",
        "exists p(_): bool # p(a)",
    ]));
}

#[test]
fn test_declarations() {
    insta::assert_snapshot!(run_formulas(&[
        // a declaration does not make a definition, and `p` and `q` are still propositions
        "let p, q: bool; let d := p & q; d",
        "let a: type; let c, d: a; let f: (a) -> a; forall x: a # f(x) = f(x)",
        "let a: type; let c, d: a; exists x: a # x = c",
    ]));
}
