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
fn test_rewritten() {
    insta::assert_snapshot!(run(&[
        "-w",
        "-",
        "tests/data/rewrite_open_atoms_is_true.txt"
    ]));
}
