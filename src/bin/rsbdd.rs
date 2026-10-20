use std::cmp::max;
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::ops::Index;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::{CommandFactory, Parser, ValueHint};

use rsbdd::BDDSymbol;
use rsbdd::NamedSymbol;
use rsbdd::TruthTableEntry;
use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::parser::*;
use rsbdd::parser_io::*;
use rsbdd::plot::*;
use rsbdd::prover::*;

#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about,
    long_about = None,
    after_help = "Without output options, the satisfying rows of the truth table are printed.\n\
Exit status: 10 if the formula is satisfiable, 20 if it is unsatisfiable, 1 or 2 on errors.\n\
With --prove: 10 if the formula is a theorem, 20 if it is not, 30 if no proof was found.\n\n\
Examples:\n  \
rsbdd examples/4_queens.txt\n  \
rsbdd -e \"a & -b\" -t\n  \
rsbdd examples/peano_puzzle.txt -w -\n  \
rsbdd examples/fol/dreadbury.txt --prove"
)]
struct Args {
    #[clap(value_parser, value_name = "FILE", value_hint = ValueHint::FilePath)]
    /// The input file containing a logic formula in rsbdd format, or '-' for stdin.
    input: Option<PathBuf>,

    #[clap(
        short,
        long,
        value_parser,
        conflicts_with = "input",
        help_heading = "Input"
    )]
    /// Parse the formula as string.
    evaluate: Option<String>,

    #[clap(short, long, value_parser, value_name = "FILE", value_hint = ValueHint::FilePath, help_heading = "Input")]
    /// Read a custom variable ordering from file.
    ordering: Option<PathBuf>,

    #[clap(short, long, help_heading = "Output")]
    /// Print the truth table to stdout.
    truthtable: bool,

    #[clap(short, long, help_heading = "Output")]
    /// Print all satisfying variables leading to a truth value.
    vars: bool,

    #[clap(short, long, value_parser, value_name = "FILE", value_hint = ValueHint::FilePath, help_heading = "Output")]
    /// Write the bdd to a dot graphviz file ('-' for stdout).
    dot: Option<PathBuf>,

    #[clap(short, long, value_parser, value_name = "FILE", value_hint = ValueHint::FilePath, help_heading = "Output")]
    /// Write the parse tree in dot format to the specified file ('-' for stdout).
    parsetree: Option<PathBuf>,

    #[clap(short = 'w', long, value_parser, value_name = "FILE", value_hint = ValueHint::FilePath, help_heading = "Output")]
    /// Write the formula after applying rewrite rules to the specified file ('-' for stdout).
    rewritten: Option<PathBuf>,

    #[clap(short = 'r', long, help_heading = "Output")]
    /// Export the automatically derived ordering to stdout.
    export_ordering: bool,

    #[clap(short, long, help_heading = "Filtering")]
    /// Compute a single satisfying model as output.
    model: bool,

    #[clap(
        short,
        long,
        value_parser,
        value_name = "true|false|any",
        help_heading = "Filtering"
    )]
    /// Only show true or false entries in the output [default: true without output options, any otherwise]
    filter: Option<TruthTableEntry>,

    #[clap(short = 'c', long, value_parser, value_name = "true|false|any", default_value_t = TruthTableEntry::Any, help_heading = "Filtering")]
    /// Only retain choice variables when filtering.
    retain_choices: TruthTableEntry,

    #[clap(
        short,
        long,
        value_parser,
        value_name = "N",
        help_heading = "Benchmarking"
    )]
    /// Repeat the solving process n times for more accurate performance reports.
    benchmark: Option<usize>,

    #[clap(
        short = 'g',
        long,
        requires = "benchmark",
        help_heading = "Benchmarking"
    )]
    /// Use GNUPlot to plot the runtime distribution.
    plot: bool,

    #[clap(
        short = 'l',
        long,
        value_parser,
        value_name = "MiB",
        help_heading = "Solver"
    )]
    /// Limit the memory used by the solver, nodes exceeding the limit are paged to disk [default: 75% of available memory]
    memory_limit: Option<usize>,

    #[clap(short = 'P', long, help_heading = "Solver")]
    /// Prove the formula in first-order logic by superposition, with the rewrite rules as axioms, and print the proof.
    prove: bool,
}

fn default_memory_limit() -> Option<usize> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("MemAvailable:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<usize>()
        .ok()
        .map(|kb| kb / 4 * 3 * 1024)
}

fn is_stdio(path: &Path) -> bool {
    path.as_os_str() == "-"
}

fn output(path: &Path) -> anyhow::Result<Box<dyn Write>> {
    if is_stdio(path) {
        Ok(Box::new(io::stdout()))
    } else {
        let file =
            File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
        Ok(Box::new(file))
    }
}

fn main() -> anyhow::Result<ExitCode> {
    let wild_args = wild::args_os();
    let args_in = argfile::expand_args_from(wild_args, argfile::parse_fromfile, argfile::PREFIX)?;
    let mut args = Args::parse_from(args_in);

    if args.evaluate.is_none() && args.input.is_none() && io::stdin().is_terminal() {
        Args::command().print_help()?;
        return Ok(ExitCode::from(2));
    }

    if !(args.truthtable
        || args.vars
        || args.export_ordering
        || args.dot.is_some()
        || args.parsetree.is_some()
        || args.rewritten.is_some()
        || args.benchmark.is_some())
    {
        args.truthtable = true;
        args.filter.get_or_insert(TruthTableEntry::True);
    }
    let filter = args.filter.unwrap_or(TruthTableEntry::Any);

    let repeat = args.benchmark.unwrap_or(1);

    let mut reader = if let Some(inline_str) = &args.evaluate {
        Box::new(BufReader::new(inline_str.as_bytes())) as Box<dyn BufRead>
    } else if let Some(path) = args.input.as_ref().filter(|p| !is_stdio(p)) {
        let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        Box::new(BufReader::new(file)) as Box<dyn BufRead>
    } else {
        Box::new(BufReader::new(io::stdin())) as Box<dyn BufRead>
    };

    let pre_variable_ordering = if let Some(ord_filename) = args.ordering {
        let file = File::open(&ord_filename)
            .with_context(|| format!("cannot open {}", ord_filename.display()))?;
        let mut contents = Box::new(BufReader::new(file)) as Box<dyn BufRead>;
        let tokens = SymbolicBDD::tokenize(&mut contents, None)?;
        let vars = ParsedFormula::extract_vars(&tokens);
        Some(vars)
    } else {
        None
    };

    if args.prove {
        let parsed = ParsedFormula::parse(Rc::new(BDDEnv::new()), &mut reader, None)?;
        let (message, status) = match Prover::prove(&parsed)? {
            Outcome::Proven(proof) => {
                for line in proof {
                    println!("{line}");
                }
                ("theorem", 10)
            }
            Outcome::Disproven(model) => {
                if let Some(model) = model {
                    println!("{model}");
                }
                ("not a theorem", 20)
            }
            Outcome::Unknown => ("unknown", 30),
        };
        println!("{message}");
        return Ok(ExitCode::from(status));
    }

    let input_parsed = ParsedFormula::new(&mut reader, pre_variable_ordering)?;

    if let Some(limit) = args
        .memory_limit
        .map(|mb| mb * 1024 * 1024)
        .or_else(default_memory_limit)
    {
        input_parsed.env.set_memory_limit(limit);
    }

    if let Some(parsetree_filename) = args.parsetree {
        let graph = SymbolicParseTree::new(&input_parsed.bdd);

        graph.render_dot(&mut output(&parsetree_filename)?)?;
    }

    if let Some(rewritten_filename) = args.rewritten {
        writeln!(output(&rewritten_filename)?, "{}", input_parsed.bdd)?;
    }

    let mut result = input_parsed.env.mk_const(false);
    let mut exec_times = Vec::new();

    // Benchmark: repeat n times and log runtime per iteration
    for i in 0..repeat {
        let tick = Instant::now();
        result = input_parsed.eval();
        exec_times.push(tick.elapsed());

        if args.benchmark.is_some() {
            eprintln!("finished {}/{} runs", i + 1, repeat);
        }
    }

    let satisfiable = !result.is_false();
    // Simplify the output when retain_choices is on
    if !args.retain_choices.is_any() {
        result = input_parsed
            .env
            .retain_choice_bottom_up(result, args.retain_choices);
    }

    // only print performance results when the benchmark flag is available, and more than 1 run has completed
    if args.benchmark.is_some() && repeat > 0 {
        print_performance_results(&exec_times);

        if args.plot {
            plot_performance_results(&exec_times)?;
        }
    }

    // reduce the bdd to a single path from root to a single 'true' node
    if args.model {
        result = input_parsed.env.model(result);
    }

    // show ordered variable list

    if args.export_ordering {
        let mut ordered_variables = input_parsed.vars.clone();
        ordered_variables.sort_by_key(|a| a.id);
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
            filter,
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
        let graph = BDDGraph::new(&result, filter);

        graph.render_dot(&mut output(&dot_filename)?)?
    }

    Ok(ExitCode::from(if satisfiable { 10 } else { 20 }))
}

fn print_sized_line<B, C, D>(labels: &[D], widths: &B, result: &BDD<C>)
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
        if result.is_true() { "True" } else { "False" },
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

    let min = sresults
        .iter()
        .min()
        .expect("sresults.min() is None")
        .as_secs_f64();
    let max = sresults
        .iter()
        .max()
        .expect("sresults.min() is None")
        .as_secs_f64();

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
fn plot_performance_results(results: &[Duration]) -> anyhow::Result<()> {
    let (_, _, _, mean, stddev) = stats(results);

    let mut gnuplot_cmd = Command::new("gnuplot")
        .arg("-p") // persistent mode
        .arg("-") // piped mode
        .stdin(Stdio::piped())
        .spawn()?;

    let stdin = gnuplot_cmd.stdin.as_mut().expect("cannot take stdin");
    write_gnuplot_normal_distribution(
        stdin,
        stddev.mul_add(-2.0, mean),
        stddev.mul_add(2.0, mean),
        mean,
        stddev,
    )?;

    gnuplot_cmd.wait()?;

    Ok(())
}

// print all variables which can take a 'true' value in the bdd
fn print_true_vars_recursive(
    root: &BDD<NamedSymbol>,
    values: Vec<TruthTableEntry>,
    vars: &[String],
    parsed: &ParsedFormula,
) {
    match root.node() {
        Node::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vals = values.clone();
            r_vals[parsed.to_free_index(&s)] = TruthTableEntry::False;
            print_true_vars_recursive(r, r_vals, vars, parsed);

            // then visit the true subtree
            let mut l_vals = values;
            l_vals[parsed.to_free_index(&s)] = TruthTableEntry::True;
            print_true_vars_recursive(l, l_vals, vars, parsed);
        }
        Node::True => {
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
    root: &BDD<NamedSymbol>,
    vars: Vec<TruthTableEntry>,
    filter: TruthTableEntry,
    parsed: &ParsedFormula,
    sizes: &A,
) where
    A: Index<usize, Output = usize>,
{
    match root.node() {
        Node::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vars = vars.clone();
            r_vars[parsed.to_free_index(&s)] = TruthTableEntry::False;
            print_truth_table_recursive(r, r_vars, filter, parsed, sizes);

            // then visit the true subtree
            let mut l_vars = vars;
            l_vars[parsed.to_free_index(&s)] = TruthTableEntry::True;
            print_truth_table_recursive(l, l_vars, filter, parsed, sizes);
        }
        _ if (filter == TruthTableEntry::Any)
            || (filter == TruthTableEntry::True && root.is_true())
            || (filter == TruthTableEntry::False && root.is_false()) =>
        {
            print_sized_line(&vars, sizes, root);
        }
        _ => {}
    }
}
