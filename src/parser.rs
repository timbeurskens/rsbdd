use std::cell::RefCell;
use std::fmt::Display;
use std::io;
use std::io::BufRead;
use std::iter::Peekable;
use std::rc::Rc;
use std::slice::Iter;
use std::string::String;
use std::vec::Vec;

use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::NamedSymbol;
use crate::bdd::{BDD, BDDEnv};
use crate::rewriter::Rewriter;

lazy_static! {
    static ref TOKENIZER: Regex = Regex::new(r#"(?P<symbol>!|&|=>|->|-|<=>|<=|\||\^|#|\*|\+|>=|:=|=|>|<|\[|\]|,|;|\(|\))|(?P<countable>\d+)|\{(?P<reference>[\w']+)\}|(?P<identifier>[\w']+)|(?P<eof>$)|(?P<comment>"[^"]*")"#).expect("Error setting-up tokenizer regex");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicBDDToken {
    Ident(NamedSymbol),
    Countable(usize),
    Reference(String),
    And,
    Or,
    Not,
    Xor,
    Nor,
    Nand,
    Implies,
    Rewrite,
    ImpliesInv,
    Iff,
    If,
    Then,
    Else,
    Exists,
    Forall,
    Induct,
    Sum,
    Eq,
    Geq,
    Gt,
    Lt,
    OpenParen,
    CloseParen,
    OpenSquare,
    CloseSquare,
    Comma,
    False,
    True,
    LFP,
    GFP,
    Hash,
    Let,
    Assign,
    Semicolon,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    And,
    Or,
    Xor,
    Nor,
    Nand,
    Implies,
    ImpliesInv,
    Iff,
    Equals,
    Rewrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CountableOperator {
    AtMost,
    LessThan,
    AtLeast,
    MoreThan,
    Exactly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuantifierType {
    Exists,
    Forall,
    Induction,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DomainConstant {
    Terminal(NamedSymbol),
    Application(NamedSymbol, Vec<Self>),
}

impl Display for DomainConstant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(s) => write!(f, "{}", s),
            Self::Application(s, args) => write!(f, "{}({})", s, args.iter().join(", ")),
        }
    }
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::And => "&",
            Self::Or => "|",
            Self::Xor => "^",
            Self::Nor => "nor",
            Self::Nand => "nand",
            Self::Implies => "=>",
            Self::ImpliesInv => "<=",
            Self::Iff => "<=>",
            Self::Equals => "=",
            Self::Rewrite => "->",
        })
    }
}

impl Display for CountableOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::AtMost => "<=",
            Self::LessThan => "<",
            Self::AtLeast => ">=",
            Self::MoreThan => ">",
            Self::Exactly => "=",
        })
    }
}

impl Display for SymbolicBDD {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::False => write!(f, "false"),
            Self::True => write!(f, "true"),
            Self::Var(v) => write!(f, "{}", v),
            Self::Not(e) => write!(f, "-({})", e),
            Self::Quantifier(QuantifierType::Exists, vars, e) => {
                write!(f, "(exists {} # {})", vars.iter().join(", "), e)
            }
            Self::Quantifier(QuantifierType::Forall, vars, e) => {
                write!(f, "(forall {} # {})", vars.iter().join(", "), e)
            }
            Self::Quantifier(QuantifierType::Induction, vars, e) => {
                write!(f, "(induct {} # {})", vars.iter().join(", "), e)
            }
            Self::CountableConst(op, es, n) => {
                write!(f, "([{}] {} {})", es.iter().join(", "), op, n)
            }
            Self::CountableVariable(op, l, r) => write!(
                f,
                "([{}] {} [{}])",
                l.iter().join(", "),
                op,
                r.iter().join(", ")
            ),
            Self::FixedPoint(v, true, e) => write!(f, "(gfp {} # {})", v, e),
            Self::FixedPoint(v, false, e) => write!(f, "(lfp {} # {})", v, e),
            Self::Ite(c, t, e) => write!(f, "(if {} then {} else {})", c, t, e),
            Self::BinaryOp(op, l, r) => write!(f, "({} {} {})", l, op, r),
            Self::Summation(vars, e) => write!(f, "(sum {} # {})", vars.iter().join(", "), e),
            Self::RuleApplication(t) => write!(f, "{}", t),
            Self::Subtree(t) if t.is_true() => write!(f, "true"),
            Self::Subtree(t) if t.is_false() => write!(f, "false"),
            Self::Subtree(_) => write!(f, "{{bdd}}"),
            Self::Reference(name) => write!(f, "{{{}}}", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolicBDD {
    False,
    True,
    Var(NamedSymbol),
    Not(Box<Self>),
    Quantifier(QuantifierType, Vec<NamedSymbol>, Box<Self>),
    CountableConst(CountableOperator, Vec<Self>, usize),
    CountableVariable(CountableOperator, Vec<Self>, Vec<Self>),
    // the fixed-point operator with a single transformer variable, initial value (as bool), and the transformer function as a symbolic bdd description
    FixedPoint(NamedSymbol, bool, Box<Self>),
    Ite(Box<Self>, Box<Self>, Box<Self>),
    BinaryOp(BinaryOperator, Box<Self>, Box<Self>),
    Summation(Vec<NamedSymbol>, Box<Self>),
    RuleApplication(DomainConstant),
    Subtree(BDD<NamedSymbol>),
    Reference(String),
}

#[derive(Debug, Clone)]
pub struct ParsedFormula {
    // all variables in the parse tree, sorted according to the variable ordering
    pub vars: Vec<NamedSymbol>,
    // all variables not bound by a quantifier in the parse tree, sorted according to the variable ordering
    pub free_vars: Vec<NamedSymbol>,
    // lookup table for converting a raw variable to a variable in the 'free' set
    pub raw2free: Vec<Option<usize>>,
    // the parse tree
    pub bdd: SymbolicBDD,
    // the environment
    pub env: Rc<BDDEnv<NamedSymbol>>,

    pub definitions: RefCell<FxHashMap<String, ReferenceContents>>,
}

#[derive(Debug, Clone)]
pub enum ReferenceContents {
    Syntax(SymbolicBDD),
    BDD(BDD<NamedSymbol>),
}

type TokenReader<'a> = Peekable<Iter<'a, SymbolicBDDToken>>;

impl ParsedFormula {
    /// Define a new BDD by name
    pub fn define(&self, name: &str, contents: ReferenceContents) {
        self.definitions
            .borrow_mut()
            .insert(name.to_string(), contents);
    }

    /// Get a reference to a previously defined BDD by name
    pub fn get_definition(&self, name: &str) -> Option<ReferenceContents> {
        self.definitions.borrow().get(name).cloned()
    }

    /// replace a variable in the formula with a new sub-formula
    pub fn replace_var(
        &self,
        formula: &SymbolicBDD,
        var: &NamedSymbol,
        replacement: &SymbolicBDD,
    ) -> SymbolicBDD {
        match formula {
            SymbolicBDD::Var(v) if v == var => replacement.clone(),
            SymbolicBDD::Quantifier(q, v, f) => {
                if v.contains(var) {
                    formula.clone()
                } else {
                    SymbolicBDD::Quantifier(
                        *q,
                        v.clone(),
                        Box::new(self.replace_var(f, var, replacement)),
                    )
                }
            }
            SymbolicBDD::FixedPoint(v, i, f) => {
                if v == var {
                    formula.clone()
                } else {
                    SymbolicBDD::FixedPoint(
                        v.clone(),
                        *i,
                        Box::new(self.replace_var(f, var, replacement)),
                    )
                }
            }
            SymbolicBDD::Ite(a, b, c) => SymbolicBDD::Ite(
                Box::new(self.replace_var(a, var, replacement)),
                Box::new(self.replace_var(b, var, replacement)),
                Box::new(self.replace_var(c, var, replacement)),
            ),
            SymbolicBDD::Not(f) => {
                SymbolicBDD::Not(Box::new(self.replace_var(f, var, replacement)))
            }
            SymbolicBDD::BinaryOp(op, l, r) => SymbolicBDD::BinaryOp(
                *op,
                Box::new(self.replace_var(l, var, replacement)),
                Box::new(self.replace_var(r, var, replacement)),
            ),
            SymbolicBDD::CountableConst(op, n, sz) => SymbolicBDD::CountableConst(
                *op,
                n.iter()
                    .map(|v| self.replace_var(v, var, replacement))
                    .collect(),
                *sz,
            ),
            SymbolicBDD::CountableVariable(op, l, r) => SymbolicBDD::CountableVariable(
                *op,
                l.iter()
                    .map(|v| self.replace_var(v, var, replacement))
                    .collect(),
                r.iter()
                    .map(|v| self.replace_var(v, var, replacement))
                    .collect(),
            ),
            SymbolicBDD::Reference(name) => self.get_definition(name).map_or_else(
                || formula.clone(),
                |t| match t {
                    ReferenceContents::Syntax(syntax) => {
                        self.replace_var(&syntax, var, replacement)
                    }
                    ReferenceContents::BDD(_) => unimplemented!(
                        "variable replacement in referenced BDDs is not supported (yet)"
                    ),
                },
            ),
            SymbolicBDD::True
            | SymbolicBDD::False
            | SymbolicBDD::Subtree(_)
            | SymbolicBDD::Var(_) => formula.clone(),
            SymbolicBDD::Summation(_, _) | SymbolicBDD::RuleApplication(_) => {
                unreachable!("removed by the rewriter")
            }
        }
    }

    pub fn to_free_index(&self, ns: &NamedSymbol) -> usize {
        self.raw2free[ns.id].unwrap_or_else(|| panic!("{} is not a free variable", ns))
    }

    pub fn extract_vars(tokens: &[SymbolicBDDToken]) -> Vec<NamedSymbol> {
        tokens
            .iter()
            .filter_map(|t| match t {
                SymbolicBDDToken::Ident(v) => Some(v.clone()),
                _ => None,
            })
            .unique()
            .collect()
    }

    pub fn new(
        contents: &mut dyn BufRead,
        variable_ordering: Option<Vec<NamedSymbol>>,
    ) -> io::Result<Self> {
        Self::new_with_env(Rc::new(BDDEnv::new()), contents, variable_ordering)
    }

    pub fn new_with_env(
        env: Rc<BDDEnv<NamedSymbol>>,
        contents: &mut dyn BufRead,
        variable_ordering: Option<Vec<NamedSymbol>>,
    ) -> io::Result<Self> {
        let tokens = SymbolicBDD::tokenize(contents, variable_ordering)?;

        let mut vars: Vec<NamedSymbol> = Self::extract_vars(&tokens);
        vars.sort_by_key(|a| a.id);

        let (definitions, formula) = SymbolicBDD::parse_formula(&mut tokens.iter().peekable())?;

        let n = vars.len();
        let mut result = Self {
            vars,
            free_vars: Vec::new(),
            raw2free: Vec::with_capacity(n),
            bdd: formula,
            env,
            definitions: Default::default(),
        };

        for (name, definition) in definitions {
            result.define(&name, ReferenceContents::Syntax(definition));
        }

        if result.uses_rewriting(&result.bdd) {
            let (bdd, atoms) = Rewriter::rewrite(&result)?;
            result.bdd = bdd;
            result.vars.extend(atoms);
        }

        let mut vi = 0;
        for v in &result.vars {
            result.raw2free.push(if result.var_is_free(&result.bdd, v) {
                result.free_vars.push(v.clone());
                let v_result = vi;
                vi += 1;

                Some(v_result)
            } else {
                None
            });
        }

        Ok(result)
    }

    fn uses_rewriting(&self, formula: &SymbolicBDD) -> bool {
        match formula {
            SymbolicBDD::Summation(_, _)
            | SymbolicBDD::RuleApplication(_)
            | SymbolicBDD::BinaryOp(BinaryOperator::Rewrite | BinaryOperator::Equals, _, _) => true,
            SymbolicBDD::Reference(name) => matches!(
                self.get_definition(name),
                Some(ReferenceContents::Syntax(definition)) if self.uses_rewriting(&definition)
            ),
            other => other.children().into_iter().any(|c| self.uses_rewriting(c)),
        }
    }

    pub fn eval(&self) -> BDD<NamedSymbol> {
        self.eval_recursive(&self.bdd)
    }

    // check whether a given variable is bound by a quantifier in the formula
    pub fn var_is_free(&self, formula: &SymbolicBDD, var: &NamedSymbol) -> bool {
        match formula {
            SymbolicBDD::Var(v) => v == var,
            SymbolicBDD::Quantifier(_, vars, f) => {
                if !vars.contains(var) {
                    self.var_is_free(f, var)
                } else {
                    false
                }
            }
            SymbolicBDD::Ite(a, b, c) => {
                self.var_is_free(a, var) || self.var_is_free(b, var) || self.var_is_free(c, var)
            }
            SymbolicBDD::Not(f) => self.var_is_free(f, var),
            SymbolicBDD::BinaryOp(_, a, b) => self.var_is_free(a, var) || self.var_is_free(b, var),
            SymbolicBDD::CountableConst(_, sub, _) => sub.iter().any(|f| self.var_is_free(f, var)),
            SymbolicBDD::CountableVariable(_, l, r) => {
                l.iter().any(|f| self.var_is_free(f, var))
                    || r.iter().any(|f| self.var_is_free(f, var))
            }
            SymbolicBDD::FixedPoint(v, _, f) => v != var && self.var_is_free(f, var),
            SymbolicBDD::Subtree(_t) => unimplemented!(),
            SymbolicBDD::True | SymbolicBDD::False => false,
            SymbolicBDD::Summation(_, _) | SymbolicBDD::RuleApplication(_) => {
                unreachable!("removed by the rewriter")
            }
            SymbolicBDD::Reference(name) => {
                self.get_definition(name).map_or_else(
                    || true,
                    |f| match f {
                        ReferenceContents::Syntax(syntax) => self.var_is_free(&syntax, var),
                        // a bdd is quantifier free by definition
                        ReferenceContents::BDD(_) => true,
                    },
                )
            }
        }
    }

    fn eval_recursive(&self, root: &SymbolicBDD) -> BDD<NamedSymbol> {
        match root {
            SymbolicBDD::False => self.env.mk_const(false),
            SymbolicBDD::True => self.env.mk_const(true),
            SymbolicBDD::Var(v) => self.env.var(v.clone()),
            SymbolicBDD::Not(b) => self.env.not(self.eval_recursive(b)),
            SymbolicBDD::Quantifier(QuantifierType::Exists, v, b) => {
                self.env.exists(v.clone(), self.eval_recursive(b))
            }
            SymbolicBDD::Quantifier(QuantifierType::Forall, v, b) => {
                self.env.all(v.clone(), self.eval_recursive(b))
            }
            SymbolicBDD::CountableConst(op, bs, n) => {
                let branches: Vec<BDD<NamedSymbol>> =
                    bs.iter().map(|b| self.eval_recursive(b)).collect();

                match op {
                    CountableOperator::AtMost => self.env.amn(&branches, *n as i64),
                    CountableOperator::AtLeast => self.env.aln(&branches, *n as i64),
                    CountableOperator::Exactly => self.env.exn(&branches, *n as i64),
                    CountableOperator::LessThan => self.env.amn(&branches, *n as i64 - 1),
                    CountableOperator::MoreThan => self.env.aln(&branches, *n as i64 + 1),
                }
            }
            SymbolicBDD::CountableVariable(op, l, r) => {
                let l_branches: Vec<BDD<NamedSymbol>> =
                    l.iter().map(|b| self.eval_recursive(b)).collect();
                let r_branches: Vec<BDD<NamedSymbol>> =
                    r.iter().map(|b| self.eval_recursive(b)).collect();

                match op {
                    CountableOperator::AtMost => self.env.count_leq(&l_branches, &r_branches),
                    CountableOperator::AtLeast => self.env.count_geq(&l_branches, &r_branches),
                    CountableOperator::Exactly => self.env.count_eq(&l_branches, &r_branches),
                    CountableOperator::LessThan => self.env.count_lt(&l_branches, &r_branches),
                    CountableOperator::MoreThan => self.env.count_gt(&l_branches, &r_branches),
                }
            }
            SymbolicBDD::Ite(c, t, e) => self.env.ite(
                self.eval_recursive(c),
                self.eval_recursive(t),
                self.eval_recursive(e),
            ),
            SymbolicBDD::BinaryOp(op, l, r) => {
                let l = self.eval_recursive(l);
                let r = self.eval_recursive(r);

                match op {
                    BinaryOperator::And => self.env.and(l, r),
                    BinaryOperator::Or => self.env.or(l, r),
                    BinaryOperator::Xor => self.env.xor(l, r),
                    BinaryOperator::Nor => self.env.nor(l, r),
                    BinaryOperator::Nand => self.env.nand(l, r),
                    BinaryOperator::Implies => self.env.implies(l, r),
                    BinaryOperator::ImpliesInv => self.env.implies(r, l),
                    BinaryOperator::Iff => self.env.as_ref().eq(l, r),
                    other => panic!("unexpected operator: {:?}", other),
                }
            }
            SymbolicBDD::FixedPoint(var, initial, transformer) => {
                let env = &self.env;

                env.fp(env.mk_const(*initial), |x| {
                    self.eval_recursive(&self.replace_var(
                        transformer,
                        var,
                        &SymbolicBDD::Subtree(x),
                    ))
                })
            }
            SymbolicBDD::Subtree(t) => t.clone(),
            SymbolicBDD::Reference(name) => self.get_definition(name).map_or_else(
                || self.env.mk_const(false),
                |t| match t {
                    ReferenceContents::Syntax(syntax) => self.eval_recursive(&syntax),
                    ReferenceContents::BDD(bdd) => bdd,
                },
            ),
            SymbolicBDD::Summation(_, _)
            | SymbolicBDD::RuleApplication(_)
            | SymbolicBDD::Quantifier(QuantifierType::Induction, _, _) => {
                unreachable!("removed by the rewriter")
            }
        }
    }

    pub fn usize2var(&self, usize: usize) -> &NamedSymbol {
        &self.vars[usize]
    }

    pub fn name2var(&self, name: &str) -> Option<NamedSymbol> {
        for v in &self.vars {
            if v.name.as_ref() == name {
                return Some(v.clone());
            }
        }
        None
    }
}

impl SymbolicBDD {
    /// The direct sub-formulas of this formula
    pub fn children(&self) -> Vec<&Self> {
        match self {
            Self::Not(f)
            | Self::Quantifier(_, _, f)
            | Self::FixedPoint(_, _, f)
            | Self::Summation(_, f) => vec![f],
            Self::BinaryOp(_, l, r) => vec![l, r],
            Self::Ite(c, t, e) => vec![c, t, e],
            Self::CountableConst(_, fs, _) => fs.iter().collect(),
            Self::CountableVariable(_, l, r) => l.iter().chain(r).collect(),
            _ => vec![],
        }
    }

    /// Build `l op r`, folding the constants `true` and `false` for `&`, `|` and `=>`
    pub fn binop(op: BinaryOperator, l: Self, r: Self) -> Self {
        use BinaryOperator::{And, Implies, Or};
        use SymbolicBDD::{False, True};

        match (op, l, r) {
            (And, False, _) | (And, _, False) => False,
            (Or, True, _) | (Or, _, True) | (Implies, False, _) | (Implies, _, True) => True,
            (And | Implies, True, r) | (Or, False, r) => r,
            (And, l, True) | (Or, l, False) => l,
            (op, l, r) => Self::BinaryOp(op, Box::new(l), Box::new(r)),
        }
    }

    fn parse_formula(tokens: &mut TokenReader) -> io::Result<(Vec<(String, Self)>, Self)> {
        let mut definitions = Vec::new();

        while eat(SymbolicBDDToken::Let, tokens) {
            let name = Self::parse_reference_name(tokens)?;
            expect(SymbolicBDDToken::Assign, tokens)?;
            definitions.push((name, Self::parse_sub_formula(tokens)?));
            expect(SymbolicBDDToken::Semicolon, tokens)?;
        }

        let mut result = Self::parse_sub_formula(tokens)?;

        while eat(SymbolicBDDToken::Semicolon, tokens)
            && check(SymbolicBDDToken::Eof, tokens).is_err()
        {
            let next = Self::parse_sub_formula(tokens)?;
            result = Self::BinaryOp(BinaryOperator::And, Box::new(result), Box::new(next));
        }

        expect(SymbolicBDDToken::Eof, tokens)?;

        Ok((definitions, result))
    }

    fn parse_simple_sub_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        match tokens.peek() {
            Some(SymbolicBDDToken::OpenParen) => Self::parse_parentized_formula(tokens),
            Some(SymbolicBDDToken::OpenSquare) => Self::parse_countable_formula(tokens),
            Some(SymbolicBDDToken::False) => {
                expect(SymbolicBDDToken::False, tokens)?;
                Ok(Self::False)
            }
            Some(SymbolicBDDToken::True) => {
                expect(SymbolicBDDToken::True, tokens)?;
                Ok(Self::True)
            }
            Some(SymbolicBDDToken::Reference(_)) => {
                Ok(Self::Reference(Self::parse_reference_name(tokens)?))
            }
            Some(SymbolicBDDToken::Ident(_)) => match Self::parse_domain_constant(tokens)? {
                DomainConstant::Terminal(name) => Ok(Self::Var(name)),
                application => Ok(Self::RuleApplication(application)),
            },
            Some(SymbolicBDDToken::Not) => Self::parse_negation(tokens),
            Some(SymbolicBDDToken::Exists) => {
                Self::parse_quantifier(tokens, SymbolicBDDToken::Exists, QuantifierType::Exists)
            }
            Some(SymbolicBDDToken::Forall) => {
                Self::parse_quantifier(tokens, SymbolicBDDToken::Forall, QuantifierType::Forall)
            }
            Some(SymbolicBDDToken::Induct) => {
                Self::parse_quantifier(tokens, SymbolicBDDToken::Induct, QuantifierType::Induction)
            }
            Some(SymbolicBDDToken::GFP) => Self::parse_fixed_point(tokens, true),
            Some(SymbolicBDDToken::LFP) => Self::parse_fixed_point(tokens, false),
            Some(SymbolicBDDToken::If) => Self::parse_ite(tokens),
            Some(SymbolicBDDToken::Sum) => Self::parse_summation(tokens),
            None | Some(SymbolicBDDToken::Eof) => {
                Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected EOF"))
            }
            Some(other) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected token {:?}", other),
            )),
        }
    }

    // parse terms in the shape a, a(b), a(b, c(d), ...)
    fn parse_domain_constant(tokens: &mut TokenReader) -> io::Result<DomainConstant> {
        let name = Self::parse_ident(tokens)?;

        if !eat(SymbolicBDDToken::OpenParen, tokens) {
            return Ok(DomainConstant::Terminal(name));
        }

        let mut args = vec![Self::parse_domain_constant(tokens)?];
        while eat(SymbolicBDDToken::Comma, tokens) {
            args.push(Self::parse_domain_constant(tokens)?);
        }
        expect(SymbolicBDDToken::CloseParen, tokens)?;

        Ok(DomainConstant::Application(name, args))
    }

    fn parse_sub_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        Self::parse_binary(tokens, 0)
    }

    // precedence climbing: binary operators with a precedence lower than min_prec end the sub-formula
    fn parse_binary(tokens: &mut TokenReader, min_prec: u8) -> io::Result<Self> {
        let mut left = Self::parse_operand(tokens)?;

        while let Some((op, prec, right_assoc)) = tokens.peek().and_then(|t| binary_operator(t)) {
            if prec < min_prec {
                break;
            }
            tokens.next();

            let right = Self::parse_binary(tokens, if right_assoc { prec } else { prec + 1 })?;
            left = Self::BinaryOp(op, Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_operand(tokens: &mut TokenReader) -> io::Result<Self> {
        let mut operand = Self::parse_simple_sub_formula(tokens)?;

        while eat(SymbolicBDDToken::OpenSquare, tokens) {
            operand = Self::parse_substitution(tokens, operand)?;
        }

        Ok(operand)
    }

    // f[x := y, ...] is rewritten to: exists x, ... # f & (x <=> y) & ...
    fn parse_substitution(tokens: &mut TokenReader, formula: Self) -> io::Result<Self> {
        let mut targets = Vec::new();
        let mut replacements = Vec::new();

        loop {
            targets.push(Self::parse_ident(tokens)?);
            expect(SymbolicBDDToken::Assign, tokens)?;
            replacements.push(match tokens.next() {
                Some(SymbolicBDDToken::Ident(v)) => Self::Var(v.clone()),
                Some(SymbolicBDDToken::True) => Self::True,
                Some(SymbolicBDDToken::False) => Self::False,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Expected variable or constant in substitution, got {:?}",
                            other
                        ),
                    ));
                }
            });

            if !eat(SymbolicBDDToken::Comma, tokens) {
                break;
            }
        }

        expect(SymbolicBDDToken::CloseSquare, tokens)?;

        if let Some(v) = replacements.iter().find_map(|r| match r {
            Self::Var(v) if targets.contains(v) => Some(v),
            _ => None,
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Substitution target {} cannot also be used as a replacement",
                    v
                ),
            ));
        }

        let body = targets
            .iter()
            .zip(replacements)
            .fold(formula, |acc, (target, replacement)| {
                let binding = Self::BinaryOp(
                    BinaryOperator::Iff,
                    Box::new(Self::Var(target.clone())),
                    Box::new(replacement),
                );
                Self::BinaryOp(BinaryOperator::And, Box::new(acc), Box::new(binding))
            });

        Ok(Self::Quantifier(
            QuantifierType::Exists,
            targets,
            Box::new(body),
        ))
    }

    fn parse_ite(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::If, tokens)?;
        let cond = Self::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::Then, tokens)?;
        let then = Self::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::Else, tokens)?;
        let else_ = Self::parse_sub_formula(tokens)?;

        Ok(Self::Ite(Box::new(cond), Box::new(then), Box::new(else_)))
    }

    fn parse_formula_list(tokens: &mut TokenReader) -> io::Result<Vec<Self>> {
        expect(SymbolicBDDToken::OpenSquare, tokens)?;
        let mut subforms = Vec::new();

        loop {
            if check(SymbolicBDDToken::CloseSquare, tokens).is_err() {
                subforms.push(Self::parse_sub_formula(tokens)?);

                // if no comma is found after the sub-formula, the formula should end with a closing square bracket
                if check(SymbolicBDDToken::Comma, tokens).is_err() {
                    break;
                } else {
                    // otherwise expect a comma
                    expect(SymbolicBDDToken::Comma, tokens)?;
                }
            } else {
                break;
            }
        }

        expect(SymbolicBDDToken::CloseSquare, tokens)?;

        Ok(subforms)
    }

    fn parse_countable(tokens: &mut TokenReader) -> io::Result<usize> {
        match tokens.next() {
            Some(SymbolicBDDToken::Countable(n)) => Ok(*n),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected number, got {:?}", other),
            )),
        }
    }

    fn parse_countable_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        let leftlist = Self::parse_formula_list(tokens)?;

        let operator = match tokens.next() {
            Some(SymbolicBDDToken::Eq) => CountableOperator::Exactly,
            Some(SymbolicBDDToken::ImpliesInv) => CountableOperator::AtMost,
            Some(SymbolicBDDToken::Geq) => CountableOperator::AtLeast,
            Some(SymbolicBDDToken::Lt) => CountableOperator::LessThan,
            Some(SymbolicBDDToken::Gt) => CountableOperator::MoreThan,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected countable operator, got {:?}", other),
                ));
            }
        };

        if check(SymbolicBDDToken::OpenSquare, tokens).is_ok() {
            let rightlist = Self::parse_formula_list(tokens)?;

            Ok(Self::CountableVariable(operator, leftlist, rightlist))
        } else {
            let count = Self::parse_countable(tokens)?;

            Ok(Self::CountableConst(operator, leftlist, count))
        }
    }

    fn parse_ident(tokens: &mut TokenReader) -> io::Result<NamedSymbol> {
        match tokens.next() {
            Some(SymbolicBDDToken::Ident(var)) => Ok(var.clone()),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected variable, got {:?}", other),
            )),
        }
    }

    fn parse_reference_name(tokens: &mut TokenReader) -> io::Result<String> {
        match tokens.next() {
            Some(SymbolicBDDToken::Reference(name)) => Ok(name.clone()),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected reference, got {:?}", other),
            )),
        }
    }

    fn parse_variable_list(tokens: &mut TokenReader) -> io::Result<Vec<NamedSymbol>> {
        let mut vars = Vec::new();

        loop {
            if check(SymbolicBDDToken::Hash, tokens).is_err() {
                vars.push(Self::parse_ident(tokens)?);

                // if no comma is found after the variable, the list should end with a closing hash
                if check(SymbolicBDDToken::Comma, tokens).is_err() {
                    break;
                } else {
                    // otherwise expect a comma
                    expect(SymbolicBDDToken::Comma, tokens)?;
                }
            } else {
                break;
            }
        }

        Ok(vars)
    }

    fn parse_fixed_point(tokens: &mut TokenReader, initial: bool) -> io::Result<Self> {
        expect(
            if initial {
                SymbolicBDDToken::GFP
            } else {
                SymbolicBDDToken::LFP
            },
            tokens,
        )?;

        let transformer_var = Self::parse_ident(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;

        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::FixedPoint(
            transformer_var,
            initial,
            Box::new(formula),
        ))
    }

    fn parse_quantifier(
        tokens: &mut TokenReader,
        token: SymbolicBDDToken,
        q: QuantifierType,
    ) -> io::Result<Self> {
        expect(token, tokens)?;
        let vars = Self::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::Quantifier(q, vars, Box::new(formula)))
    }

    fn parse_summation(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::Sum, tokens)?;
        let vars = Self::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::Summation(vars, Box::new(formula)))
    }

    fn parse_negation(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::Not, tokens)?;

        let sf = Self::parse_simple_sub_formula(tokens);

        if let Ok(sf_ok) = sf {
            Ok(Self::Not(Box::new(sf_ok)))
        } else {
            // failover if the next part is not a simple formula
            Ok(Self::Not(Box::new(Self::parse_sub_formula(tokens)?)))
        }
    }

    fn parse_parentized_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::OpenParen, tokens)?;
        let subform = Self::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::CloseParen, tokens)?;

        Ok(subform)
    }

    pub fn tokenize(
        contents: &mut dyn BufRead,
        variable_ordering: Option<Vec<NamedSymbol>>,
    ) -> io::Result<Vec<SymbolicBDDToken>> {
        let mut src: String = String::new();
        let mut result = Vec::new();

        let mut variable_indexes: FxHashMap<String, usize> = FxHashMap::default();
        let mut definitions: FxHashSet<String> = FxHashSet::default();
        let mut var_id_counter: usize = 0;

        if let Some(variables) = variable_ordering {
            for var in variables {
                variable_indexes.insert(var.name.as_ref().clone(), var.id);
                if var.id >= var_id_counter {
                    var_id_counter = var.id + 1;
                }
            }
        }

        contents.read_to_string(&mut src)?;

        for c in TOKENIZER.captures_iter(src.as_str()) {
            if let Some(symbol) = c.name("symbol") {
                match symbol.as_str() {
                    "&" | "*" => result.push(SymbolicBDDToken::And),
                    "|" | "+" => result.push(SymbolicBDDToken::Or),
                    "^" => result.push(SymbolicBDDToken::Xor),
                    "-" | "!" => result.push(SymbolicBDDToken::Not),
                    "=>" => result.push(SymbolicBDDToken::Implies),
                    "->" => result.push(SymbolicBDDToken::Rewrite),
                    "<=" => result.push(SymbolicBDDToken::ImpliesInv),
                    "<=>" => result.push(SymbolicBDDToken::Iff),
                    "#" => result.push(SymbolicBDDToken::Hash),
                    "=" => result.push(SymbolicBDDToken::Eq),
                    "<" => result.push(SymbolicBDDToken::Lt),
                    ">" => result.push(SymbolicBDDToken::Gt),
                    ">=" => result.push(SymbolicBDDToken::Geq),
                    "(" => result.push(SymbolicBDDToken::OpenParen),
                    ")" => result.push(SymbolicBDDToken::CloseParen),
                    "[" => result.push(SymbolicBDDToken::OpenSquare),
                    "]" => result.push(SymbolicBDDToken::CloseSquare),
                    "," => result.push(SymbolicBDDToken::Comma),
                    ":=" => result.push(SymbolicBDDToken::Assign),
                    ";" => result.push(SymbolicBDDToken::Semicolon),
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Unknown symbol: {}", symbol.as_str()),
                        ));
                    }
                }
            } else if let Some(reference) = c.name("reference") {
                result.push(SymbolicBDDToken::Reference(reference.as_str().to_string()));
            } else if let Some(identifier) = c.name("identifier") {
                match identifier.as_str() {
                    "false" => result.push(SymbolicBDDToken::False),
                    "true" => result.push(SymbolicBDDToken::True),
                    "not" => result.push(SymbolicBDDToken::Not),
                    "and" => result.push(SymbolicBDDToken::And),
                    "or" => result.push(SymbolicBDDToken::Or),
                    "xor" => result.push(SymbolicBDDToken::Xor),
                    "nor" => result.push(SymbolicBDDToken::Nor),
                    "nand" => result.push(SymbolicBDDToken::Nand),
                    "implies" | "in" => result.push(SymbolicBDDToken::Implies),
                    "iff" | "eq" => result.push(SymbolicBDDToken::Iff),
                    "exists" | "any" => result.push(SymbolicBDDToken::Exists),
                    "forall" | "all" => result.push(SymbolicBDDToken::Forall),
                    "induct" => result.push(SymbolicBDDToken::Induct),
                    "if" => result.push(SymbolicBDDToken::If),
                    "then" => result.push(SymbolicBDDToken::Then),
                    "else" => result.push(SymbolicBDDToken::Else),
                    "sum" => result.push(SymbolicBDDToken::Sum),
                    "gfp" | "nu" => result.push(SymbolicBDDToken::GFP),
                    "lfp" | "mu" => result.push(SymbolicBDDToken::LFP),
                    "let" => result.push(SymbolicBDDToken::Let),
                    name if result.last() == Some(&SymbolicBDDToken::Let)
                        || definitions.contains(name) =>
                    {
                        if result.last() == Some(&SymbolicBDDToken::Let) {
                            definitions.insert(name.to_string());
                        }
                        result.push(SymbolicBDDToken::Reference(name.to_string()));
                    }
                    var => {
                        let var_str = var.to_string();
                        let var_id: usize;

                        if let Some(id) = variable_indexes.get(&var_str) {
                            var_id = *id;
                        } else {
                            var_id = var_id_counter;
                            var_id_counter += 1;

                            variable_indexes.insert(var_str.clone(), var_id);
                        }

                        result.push(SymbolicBDDToken::Ident(NamedSymbol {
                            name: Rc::new(var_str),
                            id: var_id,
                        }))
                    }
                }
            } else if let Some(number) = c.name("countable") {
                let parsed_number = number.as_str().parse().expect("Failed to parse number");
                result.push(SymbolicBDDToken::Countable(parsed_number));
            } else if c.name("eof").is_some() {
                result.push(SymbolicBDDToken::Eof);
            } else if c.name("comment").is_some() {
                // ignore comments
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown token"));
            }
        }

        // force-insert EOF if not exists
        if result.last() != Some(&SymbolicBDDToken::Eof) {
            result.push(SymbolicBDDToken::Eof);
        }

        Ok(result)
    }
}

// precedence and right-associativity of a binary operator token
const fn binary_operator(token: &SymbolicBDDToken) -> Option<(BinaryOperator, u8, bool)> {
    match token {
        SymbolicBDDToken::Iff => Some((BinaryOperator::Iff, 1, false)),
        SymbolicBDDToken::Implies => Some((BinaryOperator::Implies, 2, true)),
        SymbolicBDDToken::Rewrite => Some((BinaryOperator::Rewrite, 2, true)),
        SymbolicBDDToken::ImpliesInv => Some((BinaryOperator::ImpliesInv, 2, false)),
        SymbolicBDDToken::Or => Some((BinaryOperator::Or, 3, false)),
        SymbolicBDDToken::Nor => Some((BinaryOperator::Nor, 3, false)),
        SymbolicBDDToken::Xor => Some((BinaryOperator::Xor, 4, false)),
        SymbolicBDDToken::And => Some((BinaryOperator::And, 5, false)),
        SymbolicBDDToken::Nand => Some((BinaryOperator::Nand, 5, false)),
        SymbolicBDDToken::Eq => Some((BinaryOperator::Equals, 6, false)),
        _ => None,
    }
}

fn expect(token: SymbolicBDDToken, tokens: &mut TokenReader) -> io::Result<()> {
    match &tokens.next() {
        &Some(t) if *t == token => Ok(()),
        t => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected {:?}, got {:?}", token, t),
        )),
    }
}

fn eat(token: SymbolicBDDToken, tokens: &mut TokenReader) -> bool {
    tokens.next_if(|t| **t == token).is_some()
}

fn check(token: SymbolicBDDToken, tokens: &mut TokenReader) -> io::Result<()> {
    match tokens.peek() {
        Some(&t) if *t == token => Ok(()),
        t => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Checked for {:?}, got {:?}; No capture condition available",
                token, t
            ),
        )),
    }
}
