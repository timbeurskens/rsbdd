use crate::bdd::{BDDEnv, NamedSymbol, BDD};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use std::cell::RefCell;
use std::fmt::Display;
use std::io;
use std::io::BufRead;
use std::iter::Peekable;
use std::rc::Rc;
use std::slice::Iter;
use std::string::String;
use std::vec::Vec;

use rustc_hash::FxHashMap;

lazy_static! {
    static ref TOKENIZER: Regex = Regex::new(r#"(?P<symbol>!|&|=>|-|<=>|<=|\||\^|#|\*|\+|>=|=|>|<|\[|\]|,|\(|\)|->|-)|(?P<countable>\d+)|(?P<identifier>\w+)|(?P<eof>$)|(?P<comment>"[^"]*")"#).unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicBDDToken {
    Ident(NamedSymbol),
    Countable(usize),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DomainConstant {
    Terminal(String),
    Application(String, Box<DomainConstant>),
}

impl Display for DomainConstant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainConstant::Terminal(s) => write!(f, "{}", s),
            DomainConstant::Application(s, c) => write!(f, "{}({})", s, c),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolicBDD {
    False,
    True,
    Var(NamedSymbol),
    Not(Box<SymbolicBDD>),
    Quantifier(QuantifierType, Vec<NamedSymbol>, Box<SymbolicBDD>),
    CountableConst(CountableOperator, Vec<SymbolicBDD>, usize),
    CountableVariable(CountableOperator, Vec<SymbolicBDD>, Vec<SymbolicBDD>),
    // the fixed-point operator with a single transformer variable, initial value (as bool), and the transformer function as a symbolic bdd description
    FixedPoint(NamedSymbol, bool, Box<SymbolicBDD>),
    Ite(Box<SymbolicBDD>, Box<SymbolicBDD>, Box<SymbolicBDD>),
    BinaryOp(BinaryOperator, Box<SymbolicBDD>, Box<SymbolicBDD>),
    Summation(Vec<String>, Box<SymbolicBDD>),
    RuleApplication(DomainConstant),
    RewriteRule(DomainConstant, Box<SymbolicBDD>),
    Subtree(Rc<BDD<NamedSymbol>>),
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
    pub env: RefCell<BDDEnv<NamedSymbol>>,
}

type TokenReader<'a> = Peekable<Iter<'a, SymbolicBDDToken>>;

impl ParsedFormula {
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
        let tokens = SymbolicBDD::tokenize(contents, variable_ordering)?;

        let mut vars: Vec<NamedSymbol> = Self::extract_vars(&tokens);
        vars.sort_by(|a, b| a.id.partial_cmp(&b.id).unwrap());

        let formula = SymbolicBDD::parse_formula(&mut tokens.iter().peekable())?;

        let mut free_vars = Vec::new();
        let mut raw2free = Vec::with_capacity(vars.len());

        let mut vi = 0;

        for v in &vars {
            raw2free.push(if formula.var_is_free(v) {
                free_vars.push(v.clone());
                let result = vi;
                vi += 1;

                Some(result)
            } else {
                None
            });
        }

        Ok(ParsedFormula {
            vars,
            free_vars,
            raw2free,
            bdd: formula,
            env: RefCell::new(BDDEnv::new()),
        })
    }

    pub fn eval(&self) -> Rc<BDD<NamedSymbol>> {
        self.eval_recursive(&self.bdd)
    }

    fn eval_recursive(&self, root: &SymbolicBDD) -> Rc<BDD<NamedSymbol>> {
        match root {
            SymbolicBDD::False => self.env.borrow().mk_const(false),
            SymbolicBDD::True => self.env.borrow().mk_const(true),
            SymbolicBDD::Var(v) => self.env.borrow().var(v.clone()),
            SymbolicBDD::Not(b) => self.env.borrow().not(self.eval_recursive(b)),
            SymbolicBDD::Quantifier(QuantifierType::Exists, v, b) => {
                self.env.borrow().exists(v.clone(), self.eval_recursive(b))
            }
            SymbolicBDD::Quantifier(QuantifierType::Forall, v, b) => {
                self.env.borrow().all(v.clone(), self.eval_recursive(b))
            }
            SymbolicBDD::CountableConst(op, bs, n) => {
                let branches: Vec<Rc<BDD<NamedSymbol>>> =
                    bs.iter().map(|b| self.eval_recursive(b)).collect();

                match op {
                    CountableOperator::AtMost => self.env.borrow().amn(&branches, *n as i64),
                    CountableOperator::AtLeast => self.env.borrow().aln(&branches, *n as i64),
                    CountableOperator::Exactly => self.env.borrow().exn(&branches, *n as i64),
                    CountableOperator::LessThan => self.env.borrow().amn(&branches, *n as i64 - 1),
                    CountableOperator::MoreThan => self.env.borrow().aln(&branches, *n as i64 + 1),
                }
            }
            SymbolicBDD::CountableVariable(op, l, r) => {
                let l_branches: Vec<Rc<BDD<NamedSymbol>>> =
                    l.iter().map(|b| self.eval_recursive(b)).collect();
                let r_branches: Vec<Rc<BDD<NamedSymbol>>> =
                    r.iter().map(|b| self.eval_recursive(b)).collect();

                match op {
                    CountableOperator::AtMost => {
                        self.env.borrow().count_leq(&l_branches, &r_branches)
                    }
                    CountableOperator::AtLeast => {
                        self.env.borrow().count_geq(&l_branches, &r_branches)
                    }
                    CountableOperator::Exactly => {
                        self.env.borrow().count_eq(&l_branches, &r_branches)
                    }
                    CountableOperator::LessThan => {
                        self.env.borrow().count_lt(&l_branches, &r_branches)
                    }
                    CountableOperator::MoreThan => {
                        self.env.borrow().count_gt(&l_branches, &r_branches)
                    }
                }
            }
            SymbolicBDD::Ite(c, t, e) => self.env.borrow().ite(
                self.eval_recursive(c),
                self.eval_recursive(t),
                self.eval_recursive(e),
            ),
            SymbolicBDD::BinaryOp(op, l, r) => {
                let l = self.eval_recursive(l);
                let r = self.eval_recursive(r);

                match op {
                    BinaryOperator::And => self.env.borrow().and(l, r),
                    BinaryOperator::Or => self.env.borrow().or(l, r),
                    BinaryOperator::Xor => self.env.borrow().xor(l, r),
                    BinaryOperator::Nor => self.env.borrow().nor(l, r),
                    BinaryOperator::Nand => self.env.borrow().nand(l, r),
                    BinaryOperator::Implies => self.env.borrow().implies(l, r),
                    BinaryOperator::ImpliesInv => self.env.borrow().implies(r, l),
                    BinaryOperator::Iff => self.env.borrow().eq(l, r),
                    other => panic!("unexpected operator: {:?}", other),
                }
            }
            SymbolicBDD::FixedPoint(var, initial, transformer) => {
                let env = self.env.borrow();

                env.fp(env.mk_const(*initial), |x| {
                    self.eval_recursive(&transformer.replace_var(var, &SymbolicBDD::Subtree(x)))
                })
            }
            SymbolicBDD::Subtree(t) => Rc::clone(t),
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
    fn parse_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let result = SymbolicBDD::parse_sub_formula(tokens)?;

        expect(SymbolicBDDToken::Eof, tokens)?;

        Ok(result)
    }

    // replace a variable in the formula with a new sub-formula
    pub fn replace_var(&self, var: &NamedSymbol, replacement: &Self) -> Self {
        match self {
            SymbolicBDD::Var(v) if v == var => replacement.clone(),
            SymbolicBDD::Quantifier(q, v, f) => {
                if v.contains(var) {
                    self.clone()
                } else {
                    SymbolicBDD::Quantifier(
                        *q,
                        v.clone(),
                        Box::new(f.replace_var(var, replacement)),
                    )
                }
            }
            SymbolicBDD::FixedPoint(v, i, f) => {
                if v == var {
                    self.clone()
                } else {
                    SymbolicBDD::FixedPoint(
                        v.clone(),
                        *i,
                        Box::new(f.replace_var(var, replacement)),
                    )
                }
            }
            SymbolicBDD::Ite(a, b, c) => SymbolicBDD::Ite(
                Box::new(a.replace_var(var, replacement)),
                Box::new(b.replace_var(var, replacement)),
                Box::new(c.replace_var(var, replacement)),
            ),
            SymbolicBDD::Not(f) => SymbolicBDD::Not(Box::new(f.replace_var(var, replacement))),
            SymbolicBDD::BinaryOp(op, l, r) => SymbolicBDD::BinaryOp(
                *op,
                Box::new(l.replace_var(var, replacement)),
                Box::new(r.replace_var(var, replacement)),
            ),
            SymbolicBDD::CountableConst(op, n, sz) => SymbolicBDD::CountableConst(
                *op,
                n.iter().map(|v| v.replace_var(var, replacement)).collect(),
                *sz,
            ),
            SymbolicBDD::CountableVariable(op, l, r) => SymbolicBDD::CountableVariable(
                *op,
                l.iter().map(|v| v.replace_var(var, replacement)).collect(),
                r.iter().map(|v| v.replace_var(var, replacement)).collect(),
            ),
            SymbolicBDD::True
            | SymbolicBDD::False
            | SymbolicBDD::Subtree(_)
            | SymbolicBDD::Var(_) => self.clone(),
        }
    }

    // check whether a given variable is bound by a quantifier in the formula
    pub fn var_is_free(&self, var: &NamedSymbol) -> bool {
        match self {
            SymbolicBDD::Var(v) => v == var,
            SymbolicBDD::Quantifier(_, vars, f) => {
                if !vars.contains(var) {
                    f.var_is_free(var)
                } else {
                    false
                }
            }
            SymbolicBDD::Ite(a, b, c) => {
                a.var_is_free(var) || b.var_is_free(var) || c.var_is_free(var)
            }
            SymbolicBDD::Not(f) => f.var_is_free(var),
            SymbolicBDD::BinaryOp(_, a, b) => a.var_is_free(var) || b.var_is_free(var),
            SymbolicBDD::CountableConst(_, sub, _) => sub.iter().any(|f| f.var_is_free(var)),
            SymbolicBDD::CountableVariable(_, l, r) => {
                l.iter().any(|f| f.var_is_free(var)) || r.iter().any(|f| f.var_is_free(var))
            }
            SymbolicBDD::FixedPoint(v, _, f) => v != var && f.var_is_free(var),
            SymbolicBDD::Subtree(_t) => unimplemented!(),
            SymbolicBDD::True | SymbolicBDD::False => false,
        }
    }

    fn parse_simple_sub_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        match tokens.peek() {
            Some(SymbolicBDDToken::OpenParen) => SymbolicBDD::parse_parentized_formula(tokens),
            Some(SymbolicBDDToken::OpenSquare) => SymbolicBDD::parse_countable_formula(tokens),
            Some(SymbolicBDDToken::False) => {
                expect(SymbolicBDDToken::False, tokens)?;
                Ok(SymbolicBDD::False)
            }
            Some(SymbolicBDDToken::True) => {
                expect(SymbolicBDDToken::True, tokens)?;
                Ok(SymbolicBDD::True)
            }
            Some(SymbolicBDDToken::Ident(_)) => {
                // either a variable, or a constant, or a rewrite rule
                let name = SymbolicBDD::parse_ident(tokens)?;

                if check(SymbolicBDDToken::OpenParen, tokens).is_ok() {
                    expect(SymbolicBDDToken::OpenParen, tokens)?;
                    let inside = SymbolicBDD::parse_domain_constant(tokens)?;
                    expect(SymbolicBDDToken::CloseParen, tokens)?;

                    let apply = DomainConstant::Application(name, Box::new(inside));

                    if check(SymbolicBDDToken::Rewrite, tokens).is_ok() {
                        expect(SymbolicBDDToken::Rewrite, tokens)?;
                        let inside = SymbolicBDD::parse_sub_formula(tokens)?;

                        Ok(SymbolicBDD::RewriteRule(apply, Box::new(inside)))
                    } else {
                        Ok(SymbolicBDD::RuleApplication(apply))
                    }
                } else {
                    Ok(SymbolicBDD::Var(name))
                }
            }
            Some(SymbolicBDDToken::Not) => SymbolicBDD::parse_negation(tokens),
            Some(SymbolicBDDToken::Exists) => SymbolicBDD::parse_existence_quantifier(tokens),
            Some(SymbolicBDDToken::Forall) => SymbolicBDD::parse_universal_quantifier(tokens),
            Some(SymbolicBDDToken::GFP) => SymbolicBDD::parse_fixed_point(tokens, true),
            Some(SymbolicBDDToken::LFP) => SymbolicBDD::parse_fixed_point(tokens, false),
            Some(SymbolicBDDToken::If) => SymbolicBDD::parse_ite(tokens),
            Some(SymbolicBDDToken::Sum) => SymbolicBDD::parse_summation(tokens),
            None | Some(SymbolicBDDToken::Eof) => {
                Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected EOF"))
            }
            Some(other) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected token {:?}", other),
            )),
        }
    }

    fn parse_sub_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let left = SymbolicBDD::parse_simple_sub_formula(tokens)?;

        // either a binary operator or end of sub-formula
        match tokens.peek() {
            Some(SymbolicBDDToken::And)
            | Some(SymbolicBDDToken::Or)
            | Some(SymbolicBDDToken::Xor)
            | Some(SymbolicBDDToken::Nor)
            | Some(SymbolicBDDToken::Nand)
            | Some(SymbolicBDDToken::Implies)
            | Some(SymbolicBDDToken::ImpliesInv)
            | Some(SymbolicBDDToken::Iff)
            | Some(SymbolicBDDToken::Eq) => {
                let op = SymbolicBDD::parse_binary_operator(tokens)?;
                let right = SymbolicBDD::parse_sub_formula(tokens)?;
                Ok(SymbolicBDD::BinaryOp(op, Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    // parse formulas in the shape a(b), a(b(c)), a(b(c(...)))
    fn parse_rule_application(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let rulename = SymbolicBDD::parse_ident(tokens)?;
        expect(SymbolicBDDToken::OpenParen, tokens)?;
        let inside = SymbolicBDD::parse_domain_constant(tokens)?;
        expect(SymbolicBDDToken::CloseParen, tokens)?;

        Ok(SymbolicBDD::RuleApplication(DomainConstant::Application(
            rulename,
            Box::new(inside),
        )))
    }

    fn parse_domain_constant(tokens: &mut TokenReader) -> io::Result<DomainConstant> {
        let constname = SymbolicBDD::parse_ident(tokens)?;
        if check(SymbolicBDDToken::OpenParen, tokens).is_ok() {
            expect(SymbolicBDDToken::OpenParen, tokens)?;
            let inside = SymbolicBDD::parse_domain_constant(tokens)?;
            expect(SymbolicBDDToken::CloseParen, tokens)?;

            Ok(DomainConstant::Application(constname, Box::new(inside)))
        } else {
            Ok(DomainConstant::Terminal(constname))
        }
    }

    fn parse_ite(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::If, tokens)?;
        let cond = SymbolicBDD::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::Then, tokens)?;
        let then = SymbolicBDD::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::Else, tokens)?;
        let else_ = SymbolicBDD::parse_sub_formula(tokens)?;

        Ok(SymbolicBDD::Ite(
            Box::new(cond),
            Box::new(then),
            Box::new(else_),
        ))
    }

    fn parse_formula_list(tokens: &mut TokenReader) -> io::Result<Vec<SymbolicBDD>> {
        expect(SymbolicBDDToken::OpenSquare, tokens)?;
        let mut subforms = Vec::new();

        loop {
            if check(SymbolicBDDToken::CloseSquare, tokens).is_err() {
                subforms.push(SymbolicBDD::parse_sub_formula(tokens)?);

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
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected number, got {:?}", other),
                ))
            }
        }
    }

    fn parse_countable_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let leftlist = SymbolicBDD::parse_formula_list(tokens)?;

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
                ))
            }
        };

        if check(SymbolicBDDToken::OpenSquare, tokens).is_ok() {
            let rightlist = SymbolicBDD::parse_formula_list(tokens)?;

            Ok(SymbolicBDD::CountableVariable(
                operator, leftlist, rightlist,
            ))
        } else {
            let count = SymbolicBDD::parse_countable(tokens)?;

            Ok(SymbolicBDD::CountableConst(operator, leftlist, count))
        }
    }

    fn parse_variable_name(tokens: &mut TokenReader) -> io::Result<NamedSymbol> {
        match tokens.next() {
            Some(SymbolicBDDToken::Ident(var)) => Ok(var.clone()),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected variable, got {:?}", other),
                ))
            }
        }
    }

    fn parse_variable_list(tokens: &mut TokenReader) -> io::Result<Vec<NamedSymbol>> {
        let mut vars = Vec::new();

        loop {
            if check(SymbolicBDDToken::Hash, tokens).is_err() {
                vars.push(SymbolicBDD::parse_ident(tokens)?);

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

    fn parse_fixed_point(tokens: &mut TokenReader, initial: bool) -> io::Result<SymbolicBDD> {
        expect(
            if initial {
                SymbolicBDDToken::GFP
            } else {
                SymbolicBDDToken::LFP
            },
            tokens,
        )?;

        let transformer_var = SymbolicBDD::parse_variable_name(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;

        let formula = SymbolicBDD::parse_sub_formula(tokens)?;

        Ok(SymbolicBDD::FixedPoint(
            transformer_var,
            initial,
            Box::new(formula),
        ))
    }

    fn parse_existence_quantifier(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::Exists, tokens)?;
        let vars = SymbolicBDD::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = SymbolicBDD::parse_sub_formula(tokens)?;

        Ok(SymbolicBDD::Quantifier(
            QuantifierType::Exists,
            vars,
            Box::new(formula),
        ))
    }

    fn parse_universal_quantifier(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::Forall, tokens)?;
        let vars = SymbolicBDD::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = SymbolicBDD::parse_sub_formula(tokens)?;

        Ok(SymbolicBDD::Quantifier(
            QuantifierType::Forall,
            vars,
            Box::new(formula),
        ))
    }

    fn parse_summation(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::Sum, tokens)?;
        let vars = SymbolicBDD::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = SymbolicBDD::parse_sub_formula(tokens)?;

        Ok(SymbolicBDD::Summation(vars, Box::new(formula)))
    }

    fn parse_binary_operator(tokens: &mut TokenReader) -> io::Result<BinaryOperator> {
        match tokens.peek() {
            Some(SymbolicBDDToken::And) => {
                expect(SymbolicBDDToken::And, tokens)?;
                Ok(BinaryOperator::And)
            }
            Some(SymbolicBDDToken::Or) => {
                expect(SymbolicBDDToken::Or, tokens)?;
                Ok(BinaryOperator::Or)
            }
            Some(SymbolicBDDToken::Xor) => {
                expect(SymbolicBDDToken::Xor, tokens)?;
                Ok(BinaryOperator::Xor)
            }
            Some(SymbolicBDDToken::Nor) => {
                expect(SymbolicBDDToken::Nor, tokens)?;
                Ok(BinaryOperator::Nor)
            }
            Some(SymbolicBDDToken::Nand) => {
                expect(SymbolicBDDToken::Nand, tokens)?;
                Ok(BinaryOperator::Nand)
            }
            Some(SymbolicBDDToken::Implies) => {
                expect(SymbolicBDDToken::Implies, tokens)?;
                Ok(BinaryOperator::Implies)
            }
            Some(SymbolicBDDToken::ImpliesInv) => {
                expect(SymbolicBDDToken::ImpliesInv, tokens)?;
                Ok(BinaryOperator::ImpliesInv)
            }
            Some(SymbolicBDDToken::Iff) => {
                expect(SymbolicBDDToken::Iff, tokens)?;
                Ok(BinaryOperator::Iff)
            }
            Some(SymbolicBDDToken::Eq) => {
                expect(SymbolicBDDToken::Eq, tokens)?;
                Ok(BinaryOperator::Equals)
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected binary operator, got {:?}", other),
            )),
        }
    }

    fn parse_negation(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::Not, tokens)?;

        let sf = SymbolicBDD::parse_simple_sub_formula(tokens);

        if let Ok(sf_ok) = sf {
            Ok(SymbolicBDD::Not(Box::new(sf_ok)))
        } else {
            // failover if the next part is not a simple formula
            Ok(SymbolicBDD::Not(Box::new(SymbolicBDD::parse_sub_formula(
                tokens,
            )?)))
        }
    }

    fn parse_parentized_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::OpenParen, tokens)?;
        let subform = SymbolicBDD::parse_sub_formula(tokens)?;
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
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Unknown symbol: {}", symbol.as_str()),
                        ))
                    }
                }
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
                    "exists" => result.push(SymbolicBDDToken::Exists),
                    "forall" | "all" => result.push(SymbolicBDDToken::Forall),
                    "if" => result.push(SymbolicBDDToken::If),
                    "then" => result.push(SymbolicBDDToken::Then),
                    "else" => result.push(SymbolicBDDToken::Else),
                    "sum" => result.push(SymbolicBDDToken::Sum),
                    "gfp" | "nu" => result.push(SymbolicBDDToken::GFP),
                    "lfp" | "mu" => result.push(SymbolicBDDToken::LFP),
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

                        result.push(SymbolicBDDToken::Var(NamedSymbol {
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

fn expect(token: SymbolicBDDToken, tokens: &mut TokenReader) -> io::Result<()> {
    match &tokens.next() {
        &Some(t) if *t == token => Ok(()),
        t => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected {:?}, got {:?}", token, t),
            ))
        }
    }
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
