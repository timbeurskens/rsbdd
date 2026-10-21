//! A reader for the `fof` and `cnf` languages of TPTP.
//!
//! The files are parsed by a parser generated from the ANTLR grammar of the TPTP language (Sutcliffe, The TPTP
//! Problem Library and Associated Infrastructure, 2017), and the parse tree is converted to formulas. The other
//! languages (`tff`, `thf`, `tcf`, `tpi`) are rejected.

use std::iter::successors;

use anyhow::{Context, anyhow, bail};
use rustc_hash::FxHashSet;

use crate::generated::tptp_lexer::TptpLexer;
use crate::generated::tptp_parser::{
    self, AtomicWordContext, CnfFormulaContext, CnfLiteralContext, DefinedTermContext,
    FofArgumentsContext, FofAtomicFormulaContext, FofInfixUnaryContext, FofLogicFormulaContext,
    FofPlainTermContext, FofTermContext, FofUnaryFormulaContext, FofUnitFormulaContext,
    FofUnitaryFormulaContext, FofVariableListContext, IncludeContext, NameContext, NameListContext,
    TptpFileContext, TptpParser, ValidatedTreeContext,
};

/// A function symbol, predicate symbol or constant
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// A lower word or a single-quoted atom, without its quotes
    Word(String),
    /// A number, which is an uninterpreted constant in `fof` and `cnf`
    Number(String),
    /// A distinct object `"..."`, without its quotes: distinct objects are unequal to each other
    Distinct(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Var(String),
    App(Symbol, Vec<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    Forall,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    True,
    False,
    /// An application of a predicate
    Atom(Term),
    Eq(Term, Term),
    Not(Box<Self>),
    And(Vec<Self>),
    Or(Vec<Self>),
    Implies(Box<Self>, Box<Self>),
    Iff(Box<Self>, Box<Self>),
    Quant(Quantifier, Vec<String>, Box<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Fof,
    Cnf,
}

impl Language {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fof => "fof",
            Self::Cnf => "cnf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotated {
    pub language: Language,
    pub name: String,
    pub role: String,
    pub formula: Formula,
}

impl Annotated {
    /// A conjecture is the goal of a problem; all other roles are axioms
    pub fn is_conjecture(&self) -> bool {
        self.role == "conjecture" || self.role == "question"
    }
}

/// Reads the contents of an included file, given its path relative to the TPTP directory, such as
/// `Axioms/SET001-0.ax`
pub type Resolver<'a> = dyn FnMut(&str) -> anyhow::Result<String> + 'a;

type V = ValidatedTreeContext;

fn not(formula: Formula) -> Formula {
    Formula::Not(Box::new(formula))
}

fn two<T>(items: Vec<T>, what: &str) -> anyhow::Result<[T; 2]> {
    <[T; 2]>::try_from(items).map_err(|_| anyhow!("expected two {what}"))
}

/// The contents of a single- or double-quoted token, without the quotes and escapes
fn unquote(text: &str) -> String {
    let mut chars = text.chars();
    chars.next();
    chars.next_back();
    let mut content = String::new();
    while let Some(c) = chars.next() {
        content.extend(if c == '\\' { chars.next() } else { Some(c) });
    }
    content
}

fn atomic_word(ctx: &AtomicWordContext<'_, V>) -> anyhow::Result<String> {
    if let Some(word) = ctx.lower_word_token() {
        Ok(word.to_string())
    } else if let Some(quoted) = ctx.single_quoted_token() {
        Ok(unquote(&quoted.to_string()))
    } else {
        bail!("the symbol {} is not supported", ctx.text())
    }
}

fn name(ctx: &NameContext<'_, V>) -> anyhow::Result<String> {
    ctx.atomic_word()
        .map_or_else(|| Ok(ctx.text()), |word| atomic_word(&word))
}

// the lists of the grammar are right-recursive: `x` or `x, list`

fn names(list: NameListContext<'_, V>) -> anyhow::Result<FxHashSet<String>> {
    successors(Some(list), |l| l.name_list())
        .map(|l| name(&l.name()))
        .collect()
}

fn variables(list: FofVariableListContext<'_, V>) -> Vec<String> {
    successors(Some(list), |l| l.fof_variable_list())
        .map(|l| l.variable().text())
        .collect()
}

fn arguments(list: FofArgumentsContext<'_, V>) -> anyhow::Result<Vec<Term>> {
    successors(Some(list), |l| l.fof_arguments())
        .map(|l| term(&l.fof_term()))
        .collect()
}

fn plain_term(ctx: &FofPlainTermContext<'_, V>) -> anyhow::Result<Term> {
    let functor = ctx
        .constant()
        .map(|c| c.functor())
        .or_else(|| ctx.functor())
        .context("expected a functor")?;
    let arguments = ctx
        .fof_arguments()
        .map_or_else(|| Ok(Vec::new()), arguments)?;
    Ok(Term::App(
        Symbol::Word(atomic_word(&functor.atomic_word())?),
        arguments,
    ))
}

fn defined_term(ctx: &DefinedTermContext<'_, V>) -> Term {
    let symbol = ctx.distinct_object_token().map_or_else(
        || Symbol::Number(ctx.text()),
        |object| Symbol::Distinct(unquote(&object.to_string())),
    );
    Term::App(symbol, Vec::new())
}

fn term(ctx: &FofTermContext<'_, V>) -> anyhow::Result<Term> {
    if let Some(variable) = ctx.variable() {
        return Ok(Term::Var(variable.text()));
    }
    let function = ctx.fof_function_term().context("expected a term")?;
    if let Some(plain) = function.fof_plain_term() {
        plain_term(&plain)
    } else if let Some(defined) = function.fof_defined_term().and_then(|d| d.defined_term()) {
        Ok(defined_term(&defined))
    } else {
        bail!("the symbol {} is not supported", function.text())
    }
}

fn equation<'a>(terms: impl Iterator<Item = FofTermContext<'a, V>>) -> anyhow::Result<Formula> {
    let terms = terms.map(|t| term(&t)).collect::<anyhow::Result<_>>()?;
    let [left, right] = two(terms, "terms in an equation")?;
    Ok(Formula::Eq(left, right))
}

fn inequality(ctx: &FofInfixUnaryContext<'_, V>) -> anyhow::Result<Formula> {
    equation(ctx.fof_term_children()).map(not)
}

fn atomic_formula(ctx: &FofAtomicFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(plain) = ctx.fof_plain_atomic_formula() {
        return Ok(Formula::Atom(plain_term(&plain.fof_plain_term())?));
    }
    if let Some(defined) = ctx.fof_defined_atomic_formula() {
        if let Some(infix) = defined.fof_defined_infix_formula() {
            return equation(infix.fof_term_children());
        }
        if let Some(plain) = defined.fof_defined_plain_formula() {
            return match plain.fof_defined_plain_term().text().as_str() {
                "$true" => Ok(Formula::True),
                "$false" => Ok(Formula::False),
                _ => bail!("the symbol {} is not supported", plain.text()),
            };
        }
    }
    bail!("the symbol {} is not supported", ctx.text())
}

/// The operands of `a | b | c`, which is `(a | b) | c` in the left-recursive grammar
fn binary_list<'a, C>(
    list: C,
    previous: impl Fn(&C) -> Option<C>,
    units: impl Fn(&C) -> Vec<FofUnitFormulaContext<'a, V>>,
) -> anyhow::Result<Vec<Formula>> {
    let lists: Vec<C> = successors(Some(list), previous).collect();
    lists
        .iter()
        .rev()
        .flat_map(units)
        .map(|u| unit_formula(&u))
        .collect()
}

fn logic_formula(ctx: &FofLogicFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(binary) = ctx.fof_binary_formula() {
        if let Some(pair) = binary.fof_binary_pair() {
            let operands = pair
                .fof_unit_formula_children()
                .map(|u| unit_formula(&u))
                .collect::<anyhow::Result<_>>()?;
            let [left, right] = two(operands, "operands")?;
            return Ok(match pair.nonassoc_connective().text().as_str() {
                "<=>" => Formula::Iff(Box::new(left), Box::new(right)),
                "=>" => Formula::Implies(Box::new(left), Box::new(right)),
                "<=" => Formula::Implies(Box::new(right), Box::new(left)),
                "<~>" => not(Formula::Iff(Box::new(left), Box::new(right))),
                "~|" => not(Formula::Or(vec![left, right])),
                "~&" => not(Formula::And(vec![left, right])),
                connective => bail!("unknown connective {connective}"),
            });
        }
        let list = binary
            .fof_binary_list()
            .context("expected a binary formula")?;
        if let Some(or) = list.fof_or_formula() {
            return binary_list(
                or,
                |c| c.fof_or_formula(),
                |c| c.fof_unit_formula_children().collect(),
            )
            .map(Formula::Or);
        }
        if let Some(and) = list.fof_and_formula() {
            return binary_list(
                and,
                |c| c.fof_and_formula(),
                |c| c.fof_unit_formula_children().collect(),
            )
            .map(Formula::And);
        }
    }
    if let Some(unary) = ctx.fof_unary_formula() {
        return unary_formula(&unary);
    }
    if let Some(unitary) = ctx.fof_unitary_formula() {
        return unitary_formula(&unitary);
    }
    bail!("unsupported formula {}", ctx.text())
}

fn unary_formula(ctx: &FofUnaryFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(infix) = ctx.fof_infix_unary() {
        return inequality(&infix);
    }
    let operand = ctx.fof_unit_formula().context("expected an operand of ~")?;
    unit_formula(&operand).map(not)
}

fn unit_formula(ctx: &FofUnitFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(unary) = ctx.fof_unary_formula() {
        unary_formula(&unary)
    } else {
        unitary_formula(&ctx.fof_unitary_formula().context("expected a formula")?)
    }
}

fn unitary_formula(ctx: &FofUnitaryFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(parenthesized) = ctx.fof_logic_formula() {
        return logic_formula(&parenthesized);
    }
    if let Some(quantified) = ctx.fof_quantified_formula() {
        let quantifier = match quantified.fof_quantifier().text().as_str() {
            "!" => Quantifier::Forall,
            _ => Quantifier::Exists,
        };
        return Ok(Formula::Quant(
            quantifier,
            variables(quantified.fof_variable_list()),
            Box::new(unit_formula(&quantified.fof_unit_formula())?),
        ));
    }
    atomic_formula(&ctx.fof_atomic_formula().context("expected a formula")?)
}

fn literal(ctx: &CnfLiteralContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(infix) = ctx.fof_infix_unary() {
        return inequality(&infix);
    }
    let atom = atomic_formula(&ctx.fof_atomic_formula().context("expected a literal")?)?;
    let negated = ctx.direct_terminals().any(|t| t.to_string() == "~");
    Ok(if negated { not(atom) } else { atom })
}

fn cnf_formula(ctx: &CnfFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(parenthesized) = ctx.cnf_formula() {
        return cnf_formula(&parenthesized);
    }
    // the disjunction is left-recursive, so its literals come from the last to the first
    let disjunction = ctx.cnf_disjunction().context("expected a clause")?;
    let mut literals = successors(Some(disjunction), |d| d.cnf_disjunction())
        .map(|d| literal(&d.cnf_literal()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    literals.reverse();
    Ok(if literals.len() == 1 {
        literals.remove(0)
    } else {
        Formula::Or(literals)
    })
}

fn include(ctx: &IncludeContext<'_, V>, resolve: &mut Resolver) -> anyhow::Result<Vec<Annotated>> {
    let path = atomic_word(&ctx.file_name().atomic_word())?;
    let selection = ctx
        .include_optionals()
        .formula_selection()
        .and_then(|s| s.name_list())
        .map(names)
        .transpose()?;

    let text = resolve(&path).with_context(|| format!("reading include '{path}'"))?;
    let mut formulas = parse(&text, resolve).with_context(|| format!("in include '{path}'"))?;
    if let Some(selected) = selection {
        formulas.retain(|f| selected.contains(&f.name));
    }
    Ok(formulas)
}

/// Parse a TPTP problem, reading its includes with `resolve`
pub fn parse(text: &str, resolve: &mut Resolver) -> anyhow::Result<Vec<Annotated>> {
    let validated = tptp_parser::parse_validated(text, TptpLexer::new, TptpParser::tptp_file)?;
    let file = validated
        .tree()
        .downcast_ref::<TptpFileContext<'_, V>>()
        .context("expected a TPTP file")?;

    let mut formulas = Vec::new();
    for input in file.tptp_input_children() {
        if let Some(included) = input.include() {
            formulas.extend(include(&included, resolve)?);
            continue;
        }
        let Some(annotated) = input.annotated_formula() else {
            continue;
        };
        let (language, name_ctx, role, formula) = if let Some(fof) = annotated.fof_annotated() {
            let formula = fof
                .fof_formula()
                .fof_logic_formula()
                .context("sequents are not supported")
                .and_then(|f| logic_formula(&f));
            (Language::Fof, fof.name(), fof.formula_role(), formula)
        } else if let Some(cnf) = annotated.cnf_annotated() {
            let formula = cnf_formula(&cnf.cnf_formula());
            (Language::Cnf, cnf.name(), cnf.formula_role(), formula)
        } else {
            let (keyword, name_ctx) = if let Some(f) = annotated.tff_annotated() {
                ("tff", f.name())
            } else if let Some(f) = annotated.thf_annotated() {
                ("thf", f.name())
            } else if let Some(f) = annotated.tcf_annotated() {
                ("tcf", f.name())
            } else {
                let tpi = annotated
                    .tpi_annotated()
                    .context("expected an annotated formula")?;
                ("tpi", tpi.name())
            };
            bail!(
                "in formula {}: {keyword} formulas are not supported, only fof and cnf",
                name(&name_ctx)?
            );
        };
        let name = name(&name_ctx)?;
        formulas.push(Annotated {
            language,
            role: role.lower_word_token().to_string(),
            formula: formula.with_context(|| format!("in formula {name}"))?,
            name,
        });
    }
    Ok(formulas)
}

/// The leading `%` comment block of a problem, without the `%` characters and separator lines
pub fn header(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .take_while(|line| line.is_empty() || line.starts_with('%'))
        .map(|line| line.trim_start_matches('%'))
        .filter(|line| !line.chars().all(|c| c == '-'))
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn no_includes(path: &str) -> anyhow::Result<String> {
        bail!("unexpected include {path}")
    }

    fn parse_one(text: &str) -> Formula {
        let mut formulas = parse(text, &mut no_includes).expect("parse error");
        assert_eq!(formulas.len(), 1);
        formulas.remove(0).formula
    }

    fn atom(name: &str, args: Vec<Term>) -> Formula {
        Formula::Atom(Term::App(Symbol::Word(name.to_string()), args))
    }

    fn var(name: &str) -> Term {
        Term::Var(name.to_string())
    }

    fn constant(name: &str) -> Term {
        Term::App(Symbol::Word(name.to_string()), vec![])
    }

    #[test]
    fn cnf_clause() {
        let formula = parse_one("cnf(c, axiom, ( ~ p(X) | q(X,a) | X != b )).");
        assert_eq!(
            formula,
            Formula::Or(vec![
                Formula::Not(Box::new(atom("p", vec![var("X")]))),
                atom("q", vec![var("X"), constant("a")]),
                Formula::Not(Box::new(Formula::Eq(var("X"), constant("b")))),
            ])
        );
    }

    #[test]
    fn fof_connectives() {
        let formula =
            parse_one("fof(f, axiom, ! [X,Y] : ? [Z] : ((p <= q) <~> (r ~| (s ~& $true)))).");
        let not = |f| Formula::Not(Box::new(f));
        assert_eq!(
            formula,
            Formula::Quant(
                Quantifier::Forall,
                vec!["X".to_string(), "Y".to_string()],
                Box::new(Formula::Quant(
                    Quantifier::Exists,
                    vec!["Z".to_string()],
                    Box::new(not(Formula::Iff(
                        Box::new(Formula::Implies(
                            Box::new(atom("q", vec![])),
                            Box::new(atom("p", vec![]))
                        )),
                        Box::new(not(Formula::Or(vec![
                            atom("r", vec![]),
                            not(Formula::And(vec![atom("s", vec![]), Formula::True]))
                        ]))),
                    )))
                ))
            )
        );
    }

    #[test]
    fn associative_connectives() {
        let formula = parse_one("fof(f, axiom, (a & b & c) | d | ~ e).");
        assert_eq!(
            formula,
            Formula::Or(vec![
                Formula::And(vec![
                    atom("a", vec![]),
                    atom("b", vec![]),
                    atom("c", vec![])
                ]),
                atom("d", vec![]),
                Formula::Not(Box::new(atom("e", vec![]))),
            ])
        );
    }

    #[test]
    fn quoted_numbers_and_distinct_objects() {
        let formula = parse_one("fof(1, axiom, 'A b'(-1, 2.5, \"x\\\"y\")).");
        assert_eq!(
            formula,
            Formula::Atom(Term::App(
                Symbol::Word("A b".to_string()),
                vec![
                    Term::App(Symbol::Number("-1".to_string()), vec![]),
                    Term::App(Symbol::Number("2.5".to_string()), vec![]),
                    Term::App(Symbol::Distinct("x\"y".to_string()), vec![]),
                ]
            ))
        );
    }

    #[test]
    fn annotations_and_comments() {
        let text = "% header\n/* block\ncomment */ fof(a, lemma, p, inference(r, [status(thm)], [b, c]), [x]).";
        let formulas = parse(text, &mut no_includes).expect("parse error");
        assert_eq!(formulas[0].role, "lemma");
        assert_eq!(formulas[0].formula, atom("p", vec![]));
    }

    #[test]
    fn includes_with_selection() {
        let mut resolve = |path: &str| -> anyhow::Result<String> {
            assert_eq!(path, "Axioms/A.ax");
            Ok("fof(a1, axiom, p). fof(a2, axiom, q). fof(a3, axiom, r).".to_string())
        };
        let text = "include('Axioms/A.ax', [a1, a3]). fof(g, conjecture, s).";
        let names: Vec<_> = parse(text, &mut resolve)
            .expect("parse error")
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["a1", "a3", "g"]);
    }

    #[test]
    fn unsupported_formulas() {
        let error =
            parse("tff(t, type, p: $o).", &mut no_includes).expect_err("tff is not supported");
        assert_eq!(
            format!("{error:#}"),
            "in formula t: tff formulas are not supported, only fof and cnf"
        );
        let error = parse("fof(d, axiom, $distinct(a, b)).", &mut no_includes)
            .expect_err("unsupported symbol");
        assert_eq!(
            format!("{error:#}"),
            "in formula d: the symbol $distinct(a,b) is not supported"
        );
        parse("fof(a, axiom, p & & q).", &mut no_includes).expect_err("syntax error");
    }

    #[test]
    fn header_block() {
        let text = "%----\n% File : X\n%   indented\n\n%----\nfof(a, axiom, p).\n% later\n";
        assert_eq!(header(text), "File : X\n  indented");
    }
}
