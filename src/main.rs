use clap::Parser;
use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::parser::*;
use rsbdd::parser_io::*;
use rsbdd::plot::*;
use std::cmp::max;
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::ops::Index;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser, value_name = "FILE")]
    /// The input file containing a logic formula in rsbdd format.
    input: Option<PathBuf>,

    #[clap(short, long, value_parser)]
    /// Write the parse tree in dot format to the specified file.
    parsetree: Option<PathBuf>,

    #[clap(short, long)]
    /// Print the truth table to stdout.
    truthtable: bool,

    #[clap(short, long, value_parser)]
    /// Write the bdd to a dot graphviz file.
    dot: Option<PathBuf>,

    /// Compute a single satisfying model as output.
    #[clap(short, long)]
    model: bool,

    /// Print all satisfying variables leading to a truth value.
    #[clap(short, long)]
    vars: bool,

    /// Only show true or false entries in the output.
    #[clap(short, long, value_parser, default_value_t = TruthTableEntry::Any)]
    filter: TruthTableEntry,

    /// Repeat the solving process n times for more accurate performance reports.
    #[clap(short, long, value_parser, value_name = "N")]
    benchmark: Option<usize>,

    /// Use GNUPlot to plot the runtime distribution.
    #[clap(short = 'g', long)]
    plot: bool,

    /// Parse the formula as string.
    #[clap(short, long, value_parser)]
    evaluate: Option<String>,

    /// Read a custom variable ordering from file.
    #[clap(short, long, value_parser)]
    ordering: Option<PathBuf>,

    /// Export the automatically derived ordering to stdout.
    #[clap(short = 'r', long)]
    export_ordering: bool,
}

fn main() {
    let args = Args::parse();

    let repeat = args.benchmark.unwrap_or(1);

    let inline_eval = args.evaluate;
    let input_filename = args.input;

    let mut reader = if let Some(inline_str) = &inline_eval {
        Box::new(BufReader::new(inline_str.as_bytes())) as Box<dyn BufRead>
    } else if let Some(some_input_filename) = input_filename {
        let file = File::open(some_input_filename).expect("Could not open input file");
        Box::new(BufReader::new(file)) as Box<dyn BufRead>
    } else {
        Box::new(BufReader::new(io::stdin())) as Box<dyn BufRead>
    };

    let pre_variable_ordering = if let Some(ord_filename) = args.ordering {
        let file = File::open(ord_filename).expect("Could not open variable ordering file");
        let mut contents = Box::new(BufReader::new(file)) as Box<dyn BufRead>;
        let tokens = SymbolicBDD::tokenize(&mut contents, None)
            .expect("Could not extract tokens from variable ordering");
        let vars = ParsedFormula::extract_vars(&tokens);
        Some(vars)
    } else {
        None
    };

    let input_parsed =
        ParsedFormula::new(&mut reader, pre_variable_ordering).expect("Could not parse input file");

    if let Some(parsetree_filename) = args.parsetree {
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
    if args.benchmark.is_some() && repeat > 0 {
        print_performance_results(&exec_times);

        if args.plot {
            plot_performance_results(&exec_times);
        }
    }

    // reduce the bdd to a single path from root to a single 'true' node
    if args.model {
        result = input_parsed.env.borrow().model(result);
    }

    // show ordered variable list

    if args.export_ordering {
        let mut ordered_variables = input_parsed.vars.clone();
        ordered_variables.sort_by(|a, b| a.id.partial_cmp(&b.id).unwrap());
        let ordered_variable_names = ordered_variables
            .iter()
            .map(|v| v.name.as_ref())
            .cloned()
            .collect::<Vec<String>>();

        for v in &ordered_variable_names {
            println!("{}", v);
        }
    }

    // show truth table

    let mut headers = input_parsed
        .free_vars
        .iter()
        .map(|v| v.name.as_ref())
        .cloned()
        .collect::<Vec<String>>();
    headers.push("*".to_string());

    let widths: Vec<usize> = headers.iter().map(|v| max(5, v.len())).collect();

    if args.truthtable {
        print_header(&headers, &widths);
        print_truth_table_recursive(
            &result,
            input_parsed
                .free_vars
                .iter()
                .map(|_| TruthTableEntry::Any)
                .collect(),
            args.filter,
            &input_parsed,
            &widths,
        );
    }

    if args.vars {
        print_true_vars_recursive(
            &result,
            input_parsed
                .free_vars
                .iter()
                .map(|_| TruthTableEntry::Any)
                .collect(),
            &headers,
            &input_parsed,
        );
    }

    if let Some(dot_filename) = args.dot {
        let mut f = File::create(dot_filename).expect("Could not create dot file");

        let graph = BDDGraph::new(&result, args.filter);

        graph
            .render_dot(&mut f)
            .expect("Could not write BDD to dot file");
    }
}

fn print_sized_line<B, C, D>(labels: &Vec<D>, widths: &B, result: &BDD<C>)
where
    B: Index<usize, Output = usize>,
    C: BDDSymbol,
    D: Display + Sized,
{
    print!("|");
    let len = labels.len();
    for (i, label) in labels.iter().enumerate() {
        print!(" {:indent$} |", label, indent = widths[i]);
    }
    println!(
        " {:indent$} |",
        match result {
            BDD::True => "True",
            BDD::False => "False",
            _ => unreachable!(),
        },
        indent = widths[len]
    );
}

// print header
fn print_header<'a, A, B>(labels: A, widths: B)
where
    A: IntoIterator<Item = &'a String>,
    B: IntoIterator<Item = &'a usize>,
{
    print!("|");
    for free_var in labels {
        let len = 1 + max(5, free_var.len());
        print!(" {:indent$}|", free_var, indent = len);
    }
    println!();
    for width in widths {
        print!("|{:->width$}", "", width = width + 2);
    }
    println!("|");
}

// compute run-time statistics: minimum, maximum, median, mean, standard-deviation
fn stats(results: &[Duration]) -> (f64, f64, f64, f64, f64) {
    let mut sresults = results.to_vec();
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
fn print_performance_results(results: &[Duration]) {
    let (min, max, median, mean, stddev) = stats(results);

    eprintln!("Runtime report for {} iterations:", results.len());
    eprintln!("Min runtime: {:.4}s", min);
    eprintln!("Max runtime: {:.4}s", max);
    eprintln!("Median runtime: {:.4}s", median);
    eprintln!("Mean runtime: {:.4}s", mean);
    eprintln!("Standard deviation: {:.4}s", stddev);
}

// invoke gnuplot to show the run-time distribution plot
fn plot_performance_results(results: &[Duration]) {
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

    gnuplot_cmd
        .wait()
        .expect("Could not wait for gnuplot to finish");
}

// print all variables which can take a 'true' value in the bdd
fn print_true_vars_recursive(
    root: &Rc<BDD<NamedSymbol>>,
    values: Vec<TruthTableEntry>,
    vars: &[String],
    parsed: &ParsedFormula,
) {
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vals = values.clone();
            r_vals[parsed.to_free_index(s)] = TruthTableEntry::False;
            print_true_vars_recursive(r, r_vals, vars, parsed);

            // then visit the true subtree
            let mut l_vals = values;
            l_vals[parsed.to_free_index(s)] = TruthTableEntry::True;
            print_true_vars_recursive(l, l_vals, vars, parsed);
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
fn print_truth_table_recursive<A>(
    root: &Rc<BDD<NamedSymbol>>,
    vars: Vec<TruthTableEntry>,
    filter: TruthTableEntry,
    parsed: &ParsedFormula,
    sizes: &A,
) where
    A: Index<usize, Output = usize>,
{
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vars = vars.clone();
            r_vars[parsed.to_free_index(s)] = TruthTableEntry::False;
            print_truth_table_recursive(r, r_vars, filter, parsed, sizes);

            // then visit the true subtree
            let mut l_vars = vars;
            l_vars[parsed.to_free_index(s)] = TruthTableEntry::True;
            print_truth_table_recursive(l, l_vars, filter, parsed, sizes);
        }
        c if (filter == TruthTableEntry::Any)
            || (filter == TruthTableEntry::True && *c == BDD::True)
            || (filter == TruthTableEntry::False && *c == BDD::False) =>
        {
            print_sized_line(&vars, sizes, c);
        }
        _ => {}
    }
}
