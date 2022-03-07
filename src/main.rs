use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::parser::*;
use rsbdd::parser_io::*;
use rsbdd::plot::*;
use std::fs::File;
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[macro_use]
extern crate clap;

fn main() {
    let args = clap_app!(Solver =>
        (version: "0.2.1")
        (author: "Tim Beurskens")
        (about: "A BDD-based SAT solver")
        (@arg input: -i --input +takes_value "logic input file")
        (@arg show_parsetree: -p --parsetree +takes_value "write the parse tree in dot format to this file")
        (@arg show_truth_table: -t --truthtable !takes_value "print the truth-table to stdout")
        (@arg show_dot: -d --dot +takes_value "write the bdd to a dot graphviz file")
        (@arg model: -m --model !takes_value "use a model of the bdd as output (instead of the satisfying assignment)")
        (@arg expect: -e --expect +takes_value "only show true or false entries in the truth-table")
        (@arg benchmark: -b --benchmark +takes_value "Repeat the solving process n times for more accurate performance reports")
        (@arg show_plot: --plot !takes_value "show a distribution plot of the runtime")
    )
    .get_matches();

    let repeat = args
        .value_of("benchmark")
        .unwrap_or("1")
        .parse::<usize>()
        .expect("Could not parse benchmark value as usize");

    if let Some(input_filename) = args.value_of("input") {
        let input_file = File::open(input_filename).expect("Could not open input file");

        let input_parsed = ParsedFormula::new(&mut BufReader::new(input_file))
            .expect("Could not parse input file");

        if let Some(parsetree_filename) = args.value_of("show_parsetree") {
            let mut f =
                File::create(parsetree_filename).expect("Could not create parsetree dot file");

            let graph = SymbolicParseTree::new(&input_parsed.bdd);

            graph
                .render_dot(&mut f)
                .expect("Could not write parsetree to dot file");
        }

        let mut result: Rc<BDD<NamedSymbol>> = Rc::default();
        let mut exec_times = Vec::new();

        for _ in 0..repeat {
            let tick = Instant::now();
            result = input_parsed.eval();
            exec_times.push(tick.elapsed());
        }

        // only print performance results when the benchmark flag is available, and more than 1 run has completed
        if args.is_present("benchmark") && repeat > 0 {
            print_performance_results(&exec_times);

            if args.is_present("show_plot") {
                plot_performance_results(&exec_times);
            }
        }

        if args.is_present("model") {
            result = input_parsed.env.borrow().model(result);
        }

        if args.is_present("show_truth_table") {
            let filter = match args.value_of("expect") {
                Some("true") => TruthTableEntry::True,
                Some("false") => TruthTableEntry::False,
                _ => TruthTableEntry::Any,
            };

            println!("{:?}", input_parsed.vars);
            print_truth_table_recursive(
                &result,
                input_parsed
                    .vars
                    .iter()
                    .map(|_| TruthTableEntry::Any)
                    .collect(),
                &input_parsed,
                filter,
            );
        }

        if let Some(dot_filename) = args.value_of("show_dot") {
            let mut f = File::create(dot_filename).expect("Could not create dot file");

            let graph = BDDGraph::new(&Rc::new(input_parsed.env.borrow().clone()), &result);

            graph
                .render_dot(&mut f)
                .expect("Could not write BDD to dot file");
        }
    } else {
        println!("No input file specified");
    }
}

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
fn print_performance_results(results: &Vec<Duration>) {
    let (min, max, median, mean, stddev) = stats(results);

    eprintln!("Runtime report for {} iterations:", results.len());
    eprintln!("Min runtime: {:.4}s", min);
    eprintln!("Max runtime: {:.4}s", max);
    eprintln!("Median runtime: {:.4}s", median);
    eprintln!("Mean runtime: {:.4}s", mean);
    eprintln!("Standard deviation: {:.4}s", stddev);
}

fn plot_performance_results(results: &Vec<Duration>) {
    let (_, _, _, mean, stddev) = stats(results);

    let mut gnuplot_cmd = Command::new("gnuplot")
        .arg("-p") // persistent mode
        .arg("-") // piped mode
        .stdin(Stdio::piped())
        .spawn()
        .expect("Could not spawn gnuplot");

    // let mut writer = BufWriter::new(gnuplot_cmd.stdin.as_mut().unwrap());

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

#[derive(Debug, Clone, PartialEq)]
enum TruthTableEntry {
    True,
    False,
    Any,
}

fn print_truth_table_recursive(
    root: &Rc<BDD<NamedSymbol>>,
    vars: Vec<TruthTableEntry>,
    e: &ParsedFormula,
    filter: TruthTableEntry,
) {
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vars = vars.clone();
            r_vars[s.id] = TruthTableEntry::False;
            print_truth_table_recursive(r, r_vars, e, filter.clone());

            // then visit the true subtree
            let mut l_vars = vars.clone();
            l_vars[s.id] = TruthTableEntry::True;
            print_truth_table_recursive(l, l_vars, e, filter.clone());
        }
        c if filter == TruthTableEntry::Any => println!("{:?} {:?}", vars, c),
        c if filter == TruthTableEntry::True && *c == BDD::True => println!("{:?} {:?}", vars, c),
        c if filter == TruthTableEntry::False && *c == BDD::False => println!("{:?} {:?}", vars, c),
        _ => {}
    }
}
