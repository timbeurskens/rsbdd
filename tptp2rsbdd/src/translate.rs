//! Translation of a parsed TPTP problem to an rsbdd formula for `rsbdd --prove`.
//!
//! The formula is `(axioms) => (conjectures)`, or `(formulas) => false` for a problem without a conjecture. A `cnf`
//! clause is universally closed over its variables, and the `fof` connectives map onto the rsbdd operators.

use std::fmt::Write;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::tptp::{Annotated, Formula, Quantifier, Symbol, Term};

/// The identifiers that the rsbdd tokenizer reads as keywords
pub const KEYWORDS: [&str; 26] = [
    "true", "false", "not", "and", "or", "xor", "nor", "nand", "implies", "in", "iff", "eq",
    "exists", "any", "forall", "all", "induct", "if", "then", "else", "sum", "gfp", "nu", "lfp",
    "mu", "let",
];

/// A word that can be used as an rsbdd function or predicate symbol as it is
fn is_plain(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_lowercase())
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !KEYWORDS.contains(&word)
}

/// Call `visit` on every term of a formula that is an atom or an operand of `=`, with the variables bound around it
fn visit_terms<'a>(
    formula: &'a Formula,
    bound: &mut Vec<&'a str>,
    visit: &mut impl FnMut(&'a Term, &[&'a str]),
) {
    match formula {
        Formula::True | Formula::False => {}
        Formula::Atom(term) => visit(term, bound),
        Formula::Eq(left, right) => {
            visit(left, bound);
            visit(right, bound);
        }
        Formula::Not(f) => visit_terms(f, bound, visit),
        Formula::And(fs) | Formula::Or(fs) => {
            for f in fs {
                visit_terms(f, bound, visit);
            }
        }
        Formula::Implies(left, right) | Formula::Iff(left, right) => {
            visit_terms(left, bound, visit);
            visit_terms(right, bound, visit);
        }
        Formula::Quant(_, variables, f) => {
            let depth = bound.len();
            bound.extend(variables.iter().map(String::as_str));
            visit_terms(f, bound, visit);
            bound.truncate(depth);
        }
    }
}

/// The symbols of the formulas, in order of first occurrence
fn symbols(formulas: &[Annotated]) -> Vec<&Symbol> {
    fn collect<'a>(term: &'a Term, seen: &mut FxHashSet<&'a Symbol>, out: &mut Vec<&'a Symbol>) {
        if let Term::App(symbol, arguments) = term {
            if seen.insert(symbol) {
                out.push(symbol);
            }
            for argument in arguments {
                collect(argument, seen, out);
            }
        }
    }

    let mut seen = FxHashSet::default();
    let mut out = Vec::new();
    for annotated in formulas {
        visit_terms(&annotated.formula, &mut Vec::new(), &mut |term, _| {
            collect(term, &mut seen, &mut out);
        });
    }
    out
}

/// The free variables of a formula, in order of first occurrence
fn free_variables(formula: &Formula) -> Vec<&str> {
    fn collect<'a>(term: &'a Term, bound: &[&str], free: &mut Vec<&'a str>) {
        match term {
            Term::Var(v) => {
                if !bound.contains(&v.as_str()) && !free.contains(&v.as_str()) {
                    free.push(v);
                }
            }
            Term::App(_, arguments) => {
                for argument in arguments {
                    collect(argument, bound, free);
                }
            }
        }
    }

    let mut free = Vec::new();
    visit_terms(formula, &mut Vec::new(), &mut |term, bound| {
        collect(term, bound, &mut free);
    });
    free
}

/// The rsbdd names of the symbols of a problem. Symbols are renamed if they are keywords, contain characters that
/// are not part of an rsbdd identifier, or are numbers, which the tokenizer reads as counts. A renamed symbol starts
/// with a lowercase letter, so it is distinct from the variables, and does not clash with any other symbol.
struct Names<'a>(FxHashMap<&'a Symbol, String>);

impl<'a> Names<'a> {
    fn new(symbols: &[&'a Symbol]) -> Self {
        let mut used: FxHashSet<String> = symbols
            .iter()
            .filter_map(|symbol| match symbol {
                Symbol::Word(word) if is_plain(word) => Some(word.clone()),
                _ => None,
            })
            .collect();
        let mut rename = |symbol: &Symbol| {
            let (prefix, text) = match symbol {
                Symbol::Word(word) if is_plain(word) => return word.clone(),
                Symbol::Word(word) if word.starts_with(|c: char| c.is_ascii_lowercase()) => {
                    ("", word)
                }
                Symbol::Word(word) => ("q_", word),
                Symbol::Number(number) => ("n", number),
                Symbol::Distinct(object) => ("d_", object),
            };
            let sanitized: String = text
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let mut name = format!("{prefix}{sanitized}");
            while KEYWORDS.contains(&name.as_str()) || used.contains(&name) {
                name.push('_');
            }
            used.insert(name.clone());
            name
        };
        Self(
            symbols
                .iter()
                .map(|&symbol| (symbol, rename(symbol)))
                .collect(),
        )
    }

    fn get(&self, symbol: &Symbol) -> &str {
        self.0.get(symbol).map_or("", String::as_str)
    }
}

fn write_term(out: &mut String, term: &Term, names: &Names) {
    match term {
        Term::Var(v) => out.push_str(v),
        Term::App(symbol, arguments) => {
            out.push_str(names.get(symbol));
            if !arguments.is_empty() {
                out.push('(');
                for (i, argument) in arguments.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write_term(out, argument, names);
                }
                out.push(')');
            }
        }
    }
}

fn write_operands<'a>(
    out: &mut String,
    operator: &str,
    operands: impl IntoIterator<Item = &'a Formula>,
    names: &Names,
) {
    for (i, operand) in operands.into_iter().enumerate() {
        if i > 0 {
            write!(out, " {operator} ").ok();
        }
        write_formula(out, operand, names, false);
    }
}

/// Write a formula with every equation, binary operator and quantifier in parentheses, unless `bare` is set, as
/// the precedence of the rsbdd operators differs from TPTP and the body of a quantifier extends as far as possible
fn write_formula(out: &mut String, formula: &Formula, names: &Names, bare: bool) {
    let wrap = !bare
        && !matches!(
            formula,
            Formula::True | Formula::False | Formula::Atom(_) | Formula::Not(_)
        );
    if wrap {
        out.push('(');
    }
    match formula {
        Formula::True => out.push_str("true"),
        Formula::False => out.push_str("false"),
        Formula::Atom(term) => write_term(out, term, names),
        Formula::Eq(left, right) => {
            write_term(out, left, names);
            out.push_str(" = ");
            write_term(out, right, names);
        }
        Formula::Not(f) => {
            out.push('-');
            write_formula(out, f, names, false);
        }
        Formula::And(fs) => write_operands(out, "&", fs, names),
        Formula::Or(fs) => write_operands(out, "|", fs, names),
        Formula::Implies(left, right) => write_operands(out, "=>", [&**left, &**right], names),
        Formula::Iff(left, right) => write_operands(out, "<=>", [&**left, &**right], names),
        Formula::Quant(quantifier, variables, f) => {
            let keyword = match quantifier {
                Quantifier::Forall => "forall",
                Quantifier::Exists => "exists",
            };
            write!(out, "{keyword} {} # ", variables.join(", ")).ok();
            write_formula(out, f, names, true);
        }
    }
    if wrap {
        out.push(')');
    }
}

/// A formula as an rsbdd formula, universally closed over its free variables
fn closed(formula: &Formula, names: &Names) -> String {
    let free = free_variables(formula);
    let mut out = String::new();
    if free.is_empty() {
        write_formula(&mut out, formula, names, false);
    } else {
        write!(out, "(forall {} # ", free.join(", ")).ok();
        write_formula(&mut out, formula, names, true);
        out.push(')');
    }
    out
}

fn comment(text: &str) -> String {
    text.replace('"', "")
}

/// Write the formulas as a conjunction, one per line with its name and role in a comment
fn write_conjunction(out: &mut String, items: &[(String, String)]) {
    out.push_str("(\n");
    for (i, (label, formula)) in items.iter().enumerate() {
        let separator = if i + 1 < items.len() { " &" } else { "" };
        writeln!(out, "    \"{}\" {formula}{separator}", comment(label)).ok();
    }
    out.push(')');
}

/// Translate a TPTP problem to an rsbdd formula, preceded by `header` as a comment
pub fn translate(formulas: &[Annotated], header: &str) -> String {
    let symbols = symbols(formulas);
    let names = Names::new(&symbols);
    let item = |a: &Annotated| {
        let label = format!("{} ({}, {})", a.name, a.language.as_str(), a.role);
        (label, closed(&a.formula, &names))
    };

    let mut axioms: Vec<(String, String)> = formulas
        .iter()
        .filter(|a| !a.is_conjecture())
        .map(item)
        .collect();
    // distinct objects are unequal to each other
    let distinct: Vec<Term> = symbols
        .iter()
        .filter(|symbol| matches!(symbol, Symbol::Distinct(_)))
        .map(|&symbol| Term::App(symbol.clone(), Vec::new()))
        .collect();
    for (i, left) in distinct.iter().enumerate() {
        for right in &distinct[i + 1..] {
            let unequal = Formula::Not(Box::new(Formula::Eq(left.clone(), right.clone())));
            axioms.push(("distinct objects".to_string(), closed(&unequal, &names)));
        }
    }
    let conjectures: Vec<(String, String)> = formulas
        .iter()
        .filter(|a| a.is_conjecture())
        .map(item)
        .collect();

    let mut out = String::new();
    if !header.is_empty() {
        writeln!(out, "\"\n{}\n\"\n", comment(header)).ok();
    }
    match (axioms.is_empty(), conjectures.is_empty()) {
        (true, true) => out.push_str("true"),
        (false, true) => {
            write_conjunction(&mut out, &axioms);
            out.push_str(" => false");
        }
        (true, false) => write_conjunction(&mut out, &conjectures),
        (false, false) => {
            write_conjunction(&mut out, &axioms);
            out.push_str(" => ");
            write_conjunction(&mut out, &conjectures);
        }
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tptp::parse;
    use pretty_assertions::assert_eq;

    fn run(text: &str) -> String {
        let formulas = parse(text, &mut |path| anyhow::bail!("unexpected include {path}"))
            .expect("parse error");
        translate(&formulas, "")
    }

    #[test]
    fn cnf_problem() {
        assert_eq!(
            run("cnf(a, axiom, p(X) | ~ q(X, Y)). cnf(b, negated_conjecture, ~ p(c))."),
            "(\n    \"a (cnf, axiom)\" (forall X, Y # p(X) | -q(X, Y)) &\n    \"b (cnf, negated_conjecture)\" -p(c)\n) => false\n"
        );
    }

    #[test]
    fn fof_problem() {
        assert_eq!(
            run(
                "fof(a, axiom, ![X]: (p(X) <~> X != c)). fof(g, conjecture, ?[X]: (p(X) <= (q ~& $false)))."
            ),
            "(\n    \"a (fof, axiom)\" (forall X # -(p(X) <=> -(X = c)))\n) => (\n    \"g (fof, conjecture)\" (exists X # -(q & false) => p(X))\n)\n"
        );
    }

    #[test]
    fn renamed_symbols() {
        assert_eq!(
            run("fof(a, axiom, or(in, 'Big one', or_, 12, \"x\", \"y\", 'plain'))."),
            "(\n    \"a (fof, axiom)\" or__(in_, q_Big_one, or_, n12, d_x, d_y, plain) &\n    \"distinct objects\" -(d_x = d_y)\n) => false\n"
        );
    }

    #[test]
    fn header_comment() {
        let formulas = parse("fof(g, conjecture, $true).", &mut |_| {
            anyhow::bail!("no includes")
        })
        .expect("parse error");
        assert_eq!(
            translate(&formulas, "Problem : \"quoted\""),
            "\"\nProblem : quoted\n\"\n\n(\n    \"g (fof, conjecture)\" true\n)\n"
        );
    }
}
