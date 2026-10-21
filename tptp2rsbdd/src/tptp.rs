//! A reader for the `fof`, `cnf` and `tff` languages of TPTP.
//!
//! The files are parsed by a parser generated from the ANTLR grammar of the TPTP language (Sutcliffe, The TPTP
//! Problem Library and Associated Infrastructure, 2017), and the parse tree is converted to formulas. Of `tff`, the
//! monomorphic (TF0) and polymorphic (TF1) forms are read, without arithmetic and the extensions of TFX. The other
//! languages (`thf`, `tcf`, `tpi`) are rejected.

use std::iter::successors;

use anyhow::{Context, anyhow, bail};
use rustc_hash::FxHashSet;

use crate::generated::tptp_lexer::TptpLexer;
use crate::generated::tptp_parser::{
    self, AtomicWordContext, CnfFormulaContext, CnfLiteralContext, DefinedTermContext,
    FofArgumentsContext, FofAtomicFormulaContext, FofInfixUnaryContext, FofLogicFormulaContext,
    FofPlainTermContext, FofTermContext, FofUnaryFormulaContext, FofUnitFormulaContext,
    FofUnitaryFormulaContext, FofVariableListContext, IncludeContext, NameContext, NameListContext,
    TffArgumentsContext, TffAtomTypingContext, TffAtomicFormulaContext, TffAtomicTypeContext,
    TffLogicFormulaContext, TffMonotypeContext, TffPlainAtomicContext, TffPreunitFormulaContext,
    TffTermContext, TffTopLevelTypeContext, TffUnaryFormulaContext, TffUnitFormulaContext,
    TffUnitaryFormulaContext, TffUnitaryTermContext, TffUnitaryTypeContext, TffVariableListContext,
    TffXprodTypeContext, TptpFileContext, TptpParser, ValidatedTreeContext,
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
    /// The type of individuals `$i` of `tff`
    Individual,
}

/// A type of `tff`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// The type of types `$tType`
    TType,
    /// The type of formulas `$o`
    Bool,
    Var(String),
    /// A type name, `$i`, or a type constructor applied to types
    App(Symbol, Vec<Self>),
}

/// The type of a declared symbol: `!>[T1: $tType, ..]: ((A1 * .. * An) > B)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub params: Vec<String>,
    pub args: Vec<Type>,
    pub result: Type,
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
    /// The bound variables, with their types in `tff`
    Quant(Quantifier, Vec<(String, Option<Type>)>, Box<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Fof,
    Cnf,
    Tff,
}

impl Language {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fof => "fof",
            Self::Cnf => "cnf",
            Self::Tff => "tff",
        }
    }
}

/// An annotated formula: a formula, or the declaration of the type of a symbol in `tff`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Formula(Formula),
    Declaration(Symbol, Signature),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotated {
    pub language: Language,
    pub name: String,
    pub role: String,
    pub statement: Statement,
}

impl Annotated {
    /// A conjecture is the goal of a problem; all other roles are axioms
    pub fn is_conjecture(&self) -> bool {
        self.role == "conjecture" || self.role == "question"
    }

    pub const fn formula(&self) -> Option<&Formula> {
        match &self.statement {
            Statement::Formula(formula) => Some(formula),
            Statement::Declaration(..) => None,
        }
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

fn variables(list: FofVariableListContext<'_, V>) -> Vec<(String, Option<Type>)> {
    successors(Some(list), |l| l.fof_variable_list())
        .map(|l| (l.variable().text(), None))
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

// tff: the formulas of TF0 and TF1, which are the formulas of fof with typed variables and type declarations

fn unsupported(what: &str) -> anyhow::Error {
    anyhow!("{what} is not supported in tff, only TF0 and TF1 without arithmetic")
}

fn atomic_type(ctx: &TffAtomicTypeContext<'_, V>) -> anyhow::Result<Type> {
    if let Some(parenthesized) = ctx.tff_atomic_type() {
        return atomic_type(&parenthesized);
    }
    if let Some(variable) = ctx.variable() {
        return Ok(Type::Var(variable.text()));
    }
    if let Some(constant) = ctx.type_constant() {
        let name = atomic_word(&constant.type_functor().atomic_word())?;
        return Ok(Type::App(Symbol::Word(name), Vec::new()));
    }
    if let Some(functor) = ctx.type_functor() {
        let name = atomic_word(&functor.atomic_word())?;
        let args = successors(ctx.tff_type_arguments(), |a| a.tff_type_arguments())
            .map(|a| atomic_type(&a.tff_atomic_type()))
            .collect::<anyhow::Result<_>>()?;
        return Ok(Type::App(Symbol::Word(name), args));
    }
    if let Some(defined) = ctx.defined_type() {
        return match defined.text().as_str() {
            "$tType" => Ok(Type::TType),
            "$o" => Ok(Type::Bool),
            "$i" => Ok(Type::App(Symbol::Individual, Vec::new())),
            other => Err(unsupported(&format!("the type {other}"))),
        };
    }
    Err(unsupported(&format!("the type {}", ctx.text())))
}

/// The types of a product `A1 * .. * An`, which is left-recursive
fn product(ctx: &TffXprodTypeContext<'_, V>) -> anyhow::Result<Vec<Type>> {
    let mut types = match (ctx.tff_xprod_type(), ctx.tff_unitary_type()) {
        (Some(left), _) => product(&left)?,
        (None, Some(left)) => unitary_type(&left)?,
        (None, None) => bail!("expected a product type"),
    };
    types.push(atomic_type(&ctx.tff_atomic_type())?);
    Ok(types)
}

fn unitary_type(ctx: &TffUnitaryTypeContext<'_, V>) -> anyhow::Result<Vec<Type>> {
    match (ctx.tff_atomic_type(), ctx.tff_xprod_type()) {
        (Some(atomic), _) => Ok(vec![atomic_type(&atomic)?]),
        (None, Some(xprod)) => product(&xprod),
        (None, None) => bail!("expected a type"),
    }
}

fn type_variables(list: TffVariableListContext<'_, V>) -> anyhow::Result<Vec<String>> {
    successors(Some(list), |l| l.tff_variable_list())
        .map(|l| {
            let variable = l.tff_variable();
            let typed = variable.tff_typed_variable();
            match typed {
                Some(typed) if atomic_type(&typed.tff_atomic_type())? == Type::TType => {
                    Ok(typed.variable().text())
                }
                _ => bail!(
                    "the type variable {} must have type $tType",
                    variable.text()
                ),
            }
        })
        .collect()
}

fn monotype(ctx: &TffMonotypeContext<'_, V>) -> anyhow::Result<Signature> {
    if let Some(atomic) = ctx.tff_atomic_type() {
        return Ok(Signature {
            params: Vec::new(),
            args: Vec::new(),
            result: atomic_type(&atomic)?,
        });
    }
    if let Some(mapping) = ctx.tff_mapping_type() {
        return Ok(Signature {
            params: Vec::new(),
            args: unitary_type(&mapping.tff_unitary_type())?,
            result: atomic_type(&mapping.tff_atomic_type())?,
        });
    }
    let quantified = ctx.tf1_quantified_type().context("expected a type")?;
    let mut signature = monotype(&quantified.tff_monotype())?;
    let mut params = type_variables(quantified.tff_variable_list())?;
    params.append(&mut signature.params);
    signature.params = params;
    Ok(signature)
}

fn top_level_type(ctx: &TffTopLevelTypeContext<'_, V>) -> anyhow::Result<Signature> {
    if let Some(atomic) = ctx.tff_atomic_type() {
        return Ok(Signature {
            params: Vec::new(),
            args: Vec::new(),
            result: atomic_type(&atomic)?,
        });
    }
    let mut non_atomic = ctx.tff_non_atomic_type().context("expected a type")?;
    while let Some(parenthesized) = non_atomic.tff_non_atomic_type() {
        non_atomic = parenthesized;
    }
    if let Some(mapping) = non_atomic.tff_mapping_type() {
        return Ok(Signature {
            params: Vec::new(),
            args: unitary_type(&mapping.tff_unitary_type())?,
            result: atomic_type(&mapping.tff_atomic_type())?,
        });
    }
    let quantified = non_atomic
        .tf1_quantified_type()
        .context("expected a type")?;
    let mut signature = monotype(&quantified.tff_monotype())?;
    let mut params = type_variables(quantified.tff_variable_list())?;
    params.append(&mut signature.params);
    signature.params = params;
    Ok(signature)
}

fn declaration(ctx: &TffAtomTypingContext<'_, V>) -> anyhow::Result<Statement> {
    if let Some(parenthesized) = ctx.tff_atom_typing() {
        return declaration(&parenthesized);
    }
    let atom = ctx.typeable_atom().context("expected a symbol")?;
    let symbol = match (atom.constant(), atom.distinct_object_token()) {
        (Some(constant), _) => Symbol::Word(atomic_word(&constant.functor().atomic_word())?),
        (None, Some(object)) => Symbol::Distinct(unquote(&object.to_string())),
        (None, None) => bail!("expected a symbol"),
    };
    let signature = top_level_type(&ctx.tff_top_level_type().context("expected a type")?)?;
    Ok(Statement::Declaration(symbol, signature))
}

fn tff_arguments(ctx: &TffArgumentsContext<'_, V>) -> anyhow::Result<Vec<Term>> {
    std::iter::once(ctx.tff_term())
        .chain(ctx.comma_tff_term_children().map(|c| c.tff_term()))
        .map(|t| tff_term(&t))
        .collect()
}

fn tff_plain_atomic(ctx: &TffPlainAtomicContext<'_, V>) -> anyhow::Result<Term> {
    let functor = ctx
        .constant()
        .map(|c| c.functor())
        .or_else(|| ctx.functor())
        .context("expected a functor")?;
    let arguments = ctx
        .tff_arguments()
        .map_or_else(|| Ok(Vec::new()), |a| tff_arguments(&a))?;
    Ok(Term::App(
        Symbol::Word(atomic_word(&functor.atomic_word())?),
        arguments,
    ))
}

fn tff_defined_term(ctx: &DefinedTermContext<'_, V>) -> anyhow::Result<Term> {
    if ctx.number().is_some() {
        return Err(unsupported(&format!("the number {}", ctx.text())));
    }
    Ok(defined_term(ctx))
}

/// A term that is an atomic formula or a variable
fn tff_atomic_term(ctx: &TffAtomicFormulaContext<'_, V>) -> anyhow::Result<Term> {
    ctx.tff_plain_atomic().map_or_else(
        || Err(unsupported(&format!("the term {}", ctx.text()))),
        |plain| tff_plain_atomic(&plain),
    )
}

fn tff_term(ctx: &TffTermContext<'_, V>) -> anyhow::Result<Term> {
    if let Some(defined) = ctx.defined_term() {
        return tff_defined_term(&defined);
    }
    let formula = ctx.tff_logic_formula().context("expected a term")?;
    // a term is parsed as a formula, which is a term if it is an atom or a variable
    if let Some(unitary) = formula.tff_unitary_formula() {
        if let Some(variable) = unitary.txf_unitary_formula() {
            return Ok(Term::Var(variable.variable().text()));
        }
        if let Some(atomic) = unitary.tff_atomic_formula() {
            return tff_atomic_term(&atomic);
        }
    }
    Err(unsupported(&format!(
        "the formula {} as a term",
        ctx.text()
    )))
}

fn tff_unitary_term(ctx: &TffUnitaryTermContext<'_, V>) -> anyhow::Result<Term> {
    if let Some(variable) = ctx.variable() {
        return Ok(Term::Var(variable.text()));
    }
    if let Some(atomic) = ctx.tff_atomic_formula() {
        return tff_atomic_term(&atomic);
    }
    if let Some(defined) = ctx.defined_term() {
        return tff_defined_term(&defined);
    }
    Err(unsupported(&format!("the term {}", ctx.text())))
}

fn tff_equation<'a>(
    terms: impl Iterator<Item = TffUnitaryTermContext<'a, V>>,
) -> anyhow::Result<Formula> {
    let terms = terms
        .map(|t| tff_unitary_term(&t))
        .collect::<anyhow::Result<_>>()?;
    let [left, right] = two(terms, "terms in an equation")?;
    Ok(Formula::Eq(left, right))
}

fn tff_atomic_formula(ctx: &TffAtomicFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(plain) = ctx.tff_plain_atomic() {
        return Ok(Formula::Atom(tff_plain_atomic(&plain)?));
    }
    if let Some(defined) = ctx.tff_defined_atomic() {
        let plain = defined.tff_defined_plain();
        return match plain.text().as_str() {
            "$true" => Ok(Formula::True),
            "$false" => Ok(Formula::False),
            other => Err(unsupported(&format!("the formula {other}"))),
        };
    }
    Err(unsupported(&format!("the formula {}", ctx.text())))
}

fn tff_logic_formula(ctx: &TffLogicFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(binary) = ctx.tff_binary_formula() {
        if let Some(pair) = binary.tff_binary_pair() {
            let operands = pair
                .tff_unit_formula_children()
                .map(|u| tff_unit_formula(&u))
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
            .tff_binary_list()
            .context("expected a binary formula")?;
        if let Some(or) = list.tff_or_formula() {
            let lists: Vec<_> = successors(Some(or), |c| c.tff_or_formula()).collect();
            return lists
                .iter()
                .rev()
                .flat_map(|c| c.tff_unit_formula_children().collect::<Vec<_>>())
                .map(|u| tff_unit_formula(&u))
                .collect::<anyhow::Result<_>>()
                .map(Formula::Or);
        }
        if let Some(and) = list.tff_and_formula() {
            let lists: Vec<_> = successors(Some(and), |c| c.tff_and_formula()).collect();
            return lists
                .iter()
                .rev()
                .flat_map(|c| c.tff_unit_formula_children().collect::<Vec<_>>())
                .map(|u| tff_unit_formula(&u))
                .collect::<anyhow::Result<_>>()
                .map(Formula::And);
        }
    }
    if let Some(unary) = ctx.tff_unary_formula() {
        return tff_unary_formula(&unary);
    }
    if let Some(unitary) = ctx.tff_unitary_formula() {
        return tff_unitary_formula(&unitary);
    }
    if let Some(infix) = ctx.tff_defined_infix() {
        return tff_equation(infix.tff_unitary_term_children());
    }
    Err(unsupported(&format!("the formula {}", ctx.text())))
}

fn tff_unary_formula(ctx: &TffUnaryFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(infix) = ctx.tff_infix_unary() {
        return tff_equation(infix.tff_unitary_term_children()).map(not);
    }
    let prefix = ctx.tff_prefix_unary().context("expected a negation")?;
    if prefix.tff_unary_connective().unary_connective().is_none() {
        return Err(unsupported(&format!(
            "the connective {}",
            prefix.tff_unary_connective().text()
        )));
    }
    tff_preunit_formula(&prefix.tff_preunit_formula()).map(not)
}

fn tff_preunit_formula(ctx: &TffPreunitFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(unitary) = ctx.tff_unitary_formula() {
        return tff_unitary_formula(&unitary);
    }
    let prefix = ctx.tff_prefix_unary().context("expected a formula")?;
    if prefix.tff_unary_connective().unary_connective().is_none() {
        return Err(unsupported(&format!(
            "the connective {}",
            prefix.tff_unary_connective().text()
        )));
    }
    tff_preunit_formula(&prefix.tff_preunit_formula()).map(not)
}

fn tff_unit_formula(ctx: &TffUnitFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(unary) = ctx.tff_unary_formula() {
        return tff_unary_formula(&unary);
    }
    if let Some(infix) = ctx.tff_defined_infix() {
        return tff_equation(infix.tff_unitary_term_children());
    }
    tff_unitary_formula(&ctx.tff_unitary_formula().context("expected a formula")?)
}

fn tff_unitary_formula(ctx: &TffUnitaryFormulaContext<'_, V>) -> anyhow::Result<Formula> {
    if let Some(parenthesized) = ctx.tff_logic_formula() {
        return tff_logic_formula(&parenthesized);
    }
    if let Some(quantified) = ctx.tff_quantified_formula() {
        let quantifier = match quantified.tff_quantifier().text().as_str() {
            "!" => Quantifier::Forall,
            "?" => Quantifier::Exists,
            other => return Err(unsupported(&format!("the quantifier {other}"))),
        };
        // a variable without a type is an individual
        let variables = successors(Some(quantified.tff_variable_list()), |l| {
            l.tff_variable_list()
        })
        .map(|l| {
            let variable = l.tff_variable();
            Ok(match variable.tff_typed_variable() {
                Some(typed) => (
                    typed.variable().text(),
                    Some(atomic_type(&typed.tff_atomic_type())?),
                ),
                None => (
                    variable.text(),
                    Some(Type::App(Symbol::Individual, Vec::new())),
                ),
            })
        })
        .collect::<anyhow::Result<_>>()?;
        return Ok(Formula::Quant(
            quantifier,
            variables,
            Box::new(tff_unit_formula(&quantified.tff_unit_formula())?),
        ));
    }
    if let Some(atomic) = ctx.tff_atomic_formula() {
        return tff_atomic_formula(&atomic);
    }
    Err(unsupported(&format!("the formula {}", ctx.text())))
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
        let (language, name_ctx, role, statement) = if let Some(fof) = annotated.fof_annotated() {
            let formula = fof
                .fof_formula()
                .fof_logic_formula()
                .context("sequents are not supported")
                .and_then(|f| logic_formula(&f));
            (
                Language::Fof,
                fof.name(),
                fof.formula_role(),
                formula.map(Statement::Formula),
            )
        } else if let Some(cnf) = annotated.cnf_annotated() {
            let formula = cnf_formula(&cnf.cnf_formula());
            (
                Language::Cnf,
                cnf.name(),
                cnf.formula_role(),
                formula.map(Statement::Formula),
            )
        } else if let Some(tff) = annotated.tff_annotated() {
            let formula = tff.tff_formula();
            let statement = formula.tff_atom_typing().map_or_else(
                || {
                    formula.tff_logic_formula().map_or_else(
                        || Err(unsupported("a subtype")),
                        |logic| tff_logic_formula(&logic).map(Statement::Formula),
                    )
                },
                |typing| declaration(&typing),
            );
            (Language::Tff, tff.name(), tff.formula_role(), statement)
        } else {
            let (keyword, name_ctx) = if let Some(f) = annotated.thf_annotated() {
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
                "in formula {}: {keyword} formulas are not supported, only fof, cnf and tff",
                name(&name_ctx)?
            );
        };
        let name = name(&name_ctx)?;
        formulas.push(Annotated {
            language,
            role: role.lower_word_token().to_string(),
            statement: statement.with_context(|| format!("in formula {name}"))?,
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
        match formulas.remove(0).statement {
            Statement::Formula(formula) => formula,
            Statement::Declaration(..) => panic!("expected a formula"),
        }
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
                vec![("X".to_string(), None), ("Y".to_string(), None)],
                Box::new(Formula::Quant(
                    Quantifier::Exists,
                    vec![("Z".to_string(), None)],
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
        assert_eq!(formulas[0].formula(), Some(&atom("p", vec![])));
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
            parse("thf(t, type, p: $o).", &mut no_includes).expect_err("thf is not supported");
        assert_eq!(
            format!("{error:#}"),
            "in formula t: thf formulas are not supported, only fof, cnf and tff"
        );
        let error = parse("tff(n, axiom, ![X: $int]: $less(X, 1)).", &mut no_includes)
            .expect_err("arithmetic is not supported");
        assert_eq!(
            format!("{error:#}"),
            "in formula n: the type $int is not supported in tff, only TF0 and TF1 without arithmetic"
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
    fn tff_declarations() {
        let text = "tff(b, type, beverage: $tType). tff(l, type, list: $tType > $tType). \
                    tff(m, type, mixture: !>[T: $tType]: ((T * syrup) > T)). \
                    tff(p, type, awake: beverage > $o). tff(c, type, c: $i).";
        let declarations: Vec<_> = parse(text, &mut no_includes)
            .expect("parse error")
            .into_iter()
            .map(|a| a.statement)
            .collect();
        let name = |n: &str| Type::App(Symbol::Word(n.to_string()), vec![]);
        let declare = |n: &str, params: &[&str], args, result| {
            Statement::Declaration(
                Symbol::Word(n.to_string()),
                Signature {
                    params: params.iter().map(ToString::to_string).collect(),
                    args,
                    result,
                },
            )
        };
        assert_eq!(
            declarations,
            [
                declare("beverage", &[], vec![], Type::TType),
                declare("list", &[], vec![Type::TType], Type::TType),
                declare(
                    "mixture",
                    &["T"],
                    vec![Type::Var("T".to_string()), name("syrup")],
                    Type::Var("T".to_string())
                ),
                declare("awake", &[], vec![name("beverage")], Type::Bool),
                declare("c", &[], vec![], Type::App(Symbol::Individual, vec![])),
            ]
        );
    }

    #[test]
    fn tff_formula() {
        let formula = parse_one("tff(a, axiom, ! [S: syrup, X] : p(m(beverage, c, S), X)).");
        let ty = |n: Symbol| Some(Type::App(n, vec![]));
        assert_eq!(
            formula,
            Formula::Quant(
                Quantifier::Forall,
                vec![
                    ("S".to_string(), ty(Symbol::Word("syrup".to_string()))),
                    ("X".to_string(), ty(Symbol::Individual)),
                ],
                Box::new(atom(
                    "p",
                    vec![
                        Term::App(
                            Symbol::Word("m".to_string()),
                            vec![constant("beverage"), constant("c"), var("S")]
                        ),
                        var("X")
                    ]
                ))
            )
        );
    }

    #[test]
    fn header_block() {
        let text = "%----\n% File : X\n%   indented\n\n%----\nfof(a, axiom, p).\n% later\n";
        assert_eq!(header(text), "File : X\n  indented");
    }
}
