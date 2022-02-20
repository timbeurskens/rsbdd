use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::parser::*;
use rsbdd::set::BDDSet;
use std::fs::File;
use std::rc::Rc;
use std::io::BufReader;

#[macro_use]
extern crate clap;



fn main() {
    let args = clap_app!(Solver =>
        (version: "0.1")
        (author: "Tim Beurskens")
        (about: "A BDD-based SAT solver")
        (@arg input: -i --input +takes_value "logic input file")
        (@arg show_parsetree: -p --parsetree +takes_value "write the parse tree in dot format to this file")
        (@arg show_truth_table: -t --truthtable !takes_value "print the truth-table to stdout")
    )
    .get_matches();

    if let Some(input_filename) = args.value_of("input") {
        let input_file = File::open(input_filename).expect("Could not open input file");

        let input_parsed = ParsedFormula::new(&mut BufReader::new(input_file)).expect("Could not parse input file");
    
        let result = input_parsed.eval();
    
        if args.is_present("show_truth_table") {
            println!("{:?}", input_parsed.vars);
            print_truth_table_recursive(&result, input_parsed.vars.iter().map(|_| TruthTableEntry::Any).collect(), &input_parsed);
        }
    } else {
        println!("No input file specified");
    }
}

#[derive(Debug, Clone)]
enum TruthTableEntry {
    True,
    False,
    Any,
}

fn print_truth_table_recursive(root: &Rc<BDD<usize>>, vars: Vec<TruthTableEntry>, e: &ParsedFormula) {
    match root.as_ref() {
        BDD::Choice(ref l, s, ref r) => {
            // first visit the false subtree
            let mut r_vars = vars.clone();
            r_vars[*s] = TruthTableEntry::False;
            print_truth_table_recursive(r, r_vars, e);

            // then visit the true subtree
            let mut l_vars = vars.clone();
            l_vars[*s] = TruthTableEntry::True;
            print_truth_table_recursive(l, l_vars, e);
        },
        c => println!("{:?} {:?}", vars, c)
    }
}
