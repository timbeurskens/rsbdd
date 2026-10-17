use std::fmt::Write;
use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};

use rsbdd::parser::ParsedFormula;

/// Problems solved by the performance test, with a time budget for parsing and solving in release builds.
const CASES: &[(&str, u64)] = &[
    ("tests/perf/8_queens.txt", 500),
    ("tests/perf/sudoku.txt", 1000),
    ("examples/einstein.txt", 100),
    ("examples/eurovision_2021.txt", 200),
    ("examples/state_machine.txt", 50),
    ("examples/transitivity.txt", 50),
];

/// Solves every case, snapshotting the number of nodes and the memory in use afterwards.
/// No memory limit is set, so nodes are never collected and both values are the peak.
#[test]
fn test_performance() {
    let mut report = String::new();
    let mut slow = Vec::new();

    for &(path, budget_ms) in CASES {
        let file = File::open(path).expect("Could not open input file");
        let start = Instant::now();
        let parsed = ParsedFormula::new(&mut BufReader::new(file), None)
            .expect("Could not parse input file");
        parsed.eval();
        let elapsed = start.elapsed();

        println!("{path}: {elapsed:?}");
        if elapsed > Duration::from_millis(budget_ms) {
            slow.push(format!("{path}: {elapsed:?} exceeds {budget_ms}ms"));
        }

        writeln!(
            report,
            "{path}: {} nodes, {} bytes",
            parsed.env.size(),
            parsed.env.memory()
        )
        .expect("Could not write report");
    }

    insta::assert_snapshot!(report);
    assert!(cfg!(debug_assertions) || slow.is_empty(), "{slow:#?}");
}
