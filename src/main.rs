use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::parser::*;
use rsbdd::parser_io::*;
use rsbdd::plot::*;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[macro_use]
extern crate clap;

fn main() {
    let args = clap_app!(Solver =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: "Tim Beurskens")
        (about: "A BDD-based SAT solver")
        (@arg input: -i --input +takes_value "logic input file")
        (@arg show_parsetree: -p --parsetree +takes_value "write the parse tree in dot format to this file")
        (@arg show_truth_table: -t --truthtable !takes_value "print the truth-table to stdout")
        (@arg show_dot: -d --dot +takes_value "write the bdd to a dot graphviz file")
        (@arg model: -m --model !takes_value "use a model of the bdd as output (instead of the satisfying assignment)")
        (@arg vars: -v --vars !takes_value "print all true variables leading to a truth evaluation")
        (@arg filter: -f --filter +takes_value "only show true or false entries in the truth-table")
        (@arg benchmark: -b --benchmark +takes_value "Repeat the solving process n times for more accurate performance reports")
        (@arg show_plot: --plot !takes_value "show a distribution plot of the runtime")
        (@arg evaluate: -e --eval +takes_value "Inline evaluate the given formula")
    )
    .get_matches();

    let repeat = args
        .value_of("benchmark")
        .unwrap_or("1")
        .parse::<usize>()
        .expect("Could not parse benchmark value as usize");

    let inline_eval = args.value_of("evaluate");
    let input_filename = args.value_of("input");

    let mut reader = if inline_eval.is_some() {
        let inline_str = inline_eval.unwrap().as_bytes();
        Box::new(BufReader::new(inline_str)) as Box<dyn BufRead>
    } else if input_filename.is_some() {
        let file = File::open(input_filename.unwrap()).expect("Could not open input file");
        Box::new(BufReader::new(file)) as Box<dyn BufRead>
    } else {
        Box::new(BufReader::new(io::stdin())) as Box<dyn BufRead>
    };

    let input_parsed = ParsedFormula::new(&mut reader).expect("Could not parse input file");

    if let Some(parsetree_filename) = args.value_of("show_parsetree") {
        let mut f = File::create(parsetree_filename).expect("Could not create parsetree dot file");

        let graph = SymbolicParseTree::new(&input_parsed.bdd);

        graph
            .render_dot(&mut f)
            .expect("Could not write parsetree to dot file");
    }

    let mut result: Rc<BDD<NamedSymbol>> = Rc::default();
    let mut exec_times = Vec::new();

    // Benchmark: repeat n times and log runtime per iteration
    for i in 0..repeat {
        let tick = Instant::now();
        result = input_parsed.eval();
        exec_times.push(tick.elapsed());

        eprintln!("finished {}/{} runs", i + 1, repeat);
    }

    // only print performance results when the benchmark flag is available, and more than 1 run has completed
    if args.is_present("benchmark") && repeat > 0 {
        print_performance_results(&exec_times);

        if args.is_present("show_plot") {
            plot_performance_results(&exec_times);
        }
    }

    // reduce the bdd to a single path from root to a single 'true' node
    if args.is_present("model") {
        result = input_parsed.env.borrow().model(result);
    }

    let filter = match args.value_of("filter") {
        Some("true" | "True" | "t" | "T" | "1") => TruthTableEntry::True,
        Some("false" | "False" | "f" | "F" | "0") => TruthTableEntry::False,
        _ => TruthTableEntry::Any,
    };

    if args.is_present("show_truth_table") {
        println!("{:?}", input_parsed.free_vars);
        print_truth_table_recursive(
            &result,
            input_parsed
                .free_vars
                .iter()
                .map(|_| TruthTableEntry::Any)
                .collect(),
            filter,
        );
    }

    if args.is_present("vars") {
        print_true_vars_recursive(
            &result,
            input_parsed
                .free_vars
                .iter()
                .map(|_| TruthTableEntry::Any)
                .collect(),
            &input_parsed.free_vars,
        );
    }

    if let Some(dot_filename) = args.value_of("show_dot") {
        let mut f = File::create(dot_filename).expect("Could not create dot file");

        let graph = BDDGraph::new(&Rc::new(input_parsed.env.borrow().clone()), &result, filter);

        graph
            .render_dot(&mut f)
            .expect("Could not write BDD to dot file");
    }
}

// compute run-time statistics: minimum, maximum, median, mean, standard-deviation
fn stats(results: &Vec<Duration>) -> (f64, f64, f64, f64, f64) {
    let mut sresults = results.clone();
    sresults.sort();

    let median = sresults[sresults.len() / 2].as_secs_f64();
    let sum: Duration = sresults.iter().sum();
    let mean = sum.as_secs_f64() / (sresults.len() as f64);

    let sum_variance: f64 = sresults
        .iter()
        .map(|d| (d.as_secs_f64() - mean) * (d.as_secs_f64() - mean))
        .sum();
    let variance = sum_variance / (sresults.len() as f64);
    let stddev = variance.sqrt();

    let min = sresults.iter().min().unwrap().as_secs_f64();
    let max = sresults.iter().max().unwrap().as_secs_f64();

    (min, max, median, mean, stddev)
}

// print performance results to stderr
fn print_performance_results(results: &Vec<Duration>) {
    let (min, max, median, mean, stddev) = stats(results);

    eprintln!("Runtime report for {} iterations:", results.len());
    eprintln!("Min runtime: {:.4}s", min);
    eprintln!("Max runtime: {:.4}s", max);
    eprintln!("Median runtime: {:.4}s", median);
    eprintln!("Mean runtime: {:.4}s", mean);
    eprintln!("Standard deviation: {:.4}s", stddev);
}

// invoke gnuplot to show the run-time distribution plot
fn plot_performance_results(results: &Vec<Duration>) {
    let (_, _, _, mean, stddev) = stats(results);

    let mut gnuplot_cmd = Command::new("gnuplot")
        .arg("-p") // persistent mode
        .arg("-") // piped mode
        .stdin(Stdio::piped())
        .spawn()
        .expect("Could not spawn gnuplot");

    let stdin = gnuplot_cmd.stdin.as_mut().unwrap();
    write_gnuplot_normal_distribution(
        stdin,
        mean - (stddev * 2.0),
        mean + (stddev * 2.0),
        mean,
        stddev,
    )
    .expect("Could not write to gnuplot command");
    drop(stdin);

    gnuplot_cmd
        .wait()
        .expect("Could not wait for gnuplot to finish");
}

fn print_true_vars_recursive(
    root: &Rc<BDD<NamedSymbol>>,
    values: Vec<TruthTableEntry>,
    vars: &Vec<String>,
) {
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vals = values.clone();
            r_vals[s.id] = TruthTableEntry::False;
            print_true_vars_recursive(r, r_vals, vars);

            // then visit the true subtree
            let mut l_vals = values.clone();
            l_vals[s.id] = TruthTableEntry::True;
            print_true_vars_recursive(l, l_vals, vars);
        }
        BDD::True => {
            let mut vars_str = Vec::new();
            for (i, v) in values.iter().enumerate() {
                if *v == TruthTableEntry::True {
                    vars_str.push(vars[i].clone());
                } else if *v == TruthTableEntry::Any {
                    vars_str.push(vars[i].clone() + "*");
                }
            }
            println!("{};", vars_str.join(", "));
        }
        _ => {}
    }
}

// recursively walk through the bdd and assign values to the variables until every permutation is assigned a true or false value
fn print_truth_table_recursive(
    root: &Rc<BDD<NamedSymbol>>,
    vars: Vec<TruthTableEntry>,
    filter: TruthTableEntry,
) {
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vars = vars.clone();
            r_vars[s.id] = TruthTableEntry::False;
            print_truth_table_recursive(r, r_vars, filter.clone());

            // then visit the true subtree
            let mut l_vars = vars.clone();
            l_vars[s.id] = TruthTableEntry::True;
            print_truth_table_recursive(l, l_vars, filter.clone());
        }
        c if (filter == TruthTableEntry::Any)
            || (filter == TruthTableEntry::True && *c == BDD::True)
            || (filter == TruthTableEntry::False && *c == BDD::False) =>
        {
            println!("{:?} {:?}", vars, c)
        }
        _ => {}
    }
}
