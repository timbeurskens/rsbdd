use std::cell::RefCell;
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
use rustc_hash::FxHashMap;

use crate::bdd::{BDDEnv, BDD};
use crate::NamedSymbol;

lazy_static! {
    static ref TOKENIZER: Regex = Regex::new(r#"(?P<symbol>!|&|=>|-|<=>|<=|\||\^|#|\*|\+|>=|=|>|<|\[|\]|,|\(|\))|(?P<countable>\d+)|\{(?P<reference>[\w']+)\}|(?P<identifier>[\w']+)|(?P<eof>$)|(?P<comment>"[^"]*")"#).expect("Error setting-up tokenizer regex");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicBDDToken {
    Var(NamedSymbol),
    Countable(usize),
    Reference(String),
    And,
    Or,
    Not,
    Xor,
    Nor,
    Nand,
    Implies,
    ImpliesInv,
    Iff,
    If,
    Then,
    Else,
    Exists,
    Forall,
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
    Subtree(Rc<BDD<NamedSymbol>>),
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
    BDD(Rc<BDD<NamedSymbol>>),
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
        }
    }

    pub fn to_free_index(&self, ns: &NamedSymbol) -> usize {
        self.raw2free[ns.id].unwrap_or_else(|| panic!("{} is not a free variable", ns))
    }

    pub fn extract_vars(tokens: &[SymbolicBDDToken]) -> Vec<NamedSymbol> {
        tokens
            .iter()
            .filter_map(|t| match t {
                SymbolicBDDToken::Var(v) => Some(v.clone()),
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
        vars.sort_by(|a, b| a.id.cmp(&b.id));

        let formula = SymbolicBDD::parse_formula(&mut tokens.iter().peekable())?;

        let n = vars.len();
        let mut result = Self {
            vars,
            free_vars: Vec::new(),
            raw2free: Vec::with_capacity(n),
            bdd: formula,
            env,
            definitions: Default::default(),
        };

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

    pub fn eval(&self) -> Rc<BDD<NamedSymbol>> {
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

    fn eval_recursive(&self, root: &SymbolicBDD) -> Rc<BDD<NamedSymbol>> {
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
                let branches: Vec<Rc<BDD<NamedSymbol>>> =
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
                let l_branches: Vec<Rc<BDD<NamedSymbol>>> =
                    l.iter().map(|b| self.eval_recursive(b)).collect();
                let r_branches: Vec<Rc<BDD<NamedSymbol>>> =
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
            SymbolicBDD::Subtree(t) => Rc::clone(t),
            SymbolicBDD::Reference(name) => self.get_definition(name).map_or_else(
                || self.env.mk_const(false),
                |t| match t {
                    ReferenceContents::Syntax(syntax) => self.eval_recursive(&syntax),
                    ReferenceContents::BDD(bdd) => bdd,
                },
            ),
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
    fn parse_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        let result = Self::parse_sub_formula(tokens)?;

        expect(SymbolicBDDToken::Eof, tokens)?;

        Ok(result)
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
            Some(SymbolicBDDToken::Var(_)) => Ok(Self::Var(Self::parse_variable_name(tokens)?)),
            Some(SymbolicBDDToken::Not) => Self::parse_negation(tokens),
            Some(SymbolicBDDToken::Exists) => Self::parse_existence_quantifier(tokens),
            Some(SymbolicBDDToken::Forall) => Self::parse_universal_quantifier(tokens),
            Some(SymbolicBDDToken::GFP) => Self::parse_fixed_point(tokens, true),
            Some(SymbolicBDDToken::LFP) => Self::parse_fixed_point(tokens, false),
            Some(SymbolicBDDToken::If) => Self::parse_ite(tokens),
            None | Some(SymbolicBDDToken::Eof) => {
                Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected EOF"))
            }
            Some(other) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected token {:?}", other),
            )),
        }
    }

    fn parse_sub_formula(tokens: &mut TokenReader) -> io::Result<Self> {
        let left = Self::parse_simple_sub_formula(tokens)?;

        // either a binary operator or end of sub-formula
        match tokens.peek() {
            Some(SymbolicBDDToken::And)
            | Some(SymbolicBDDToken::Or)
            | Some(SymbolicBDDToken::Xor)
            | Some(SymbolicBDDToken::Nor)
            | Some(SymbolicBDDToken::Nand)
            | Some(SymbolicBDDToken::Implies)
            | Some(SymbolicBDDToken::ImpliesInv)
            | Some(SymbolicBDDToken::Iff) => {
                let op = Self::parse_binary_operator(tokens)?;
                let right = Self::parse_sub_formula(tokens)?;
                Ok(Self::BinaryOp(op, Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
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

    fn parse_variable_name(tokens: &mut TokenReader) -> io::Result<NamedSymbol> {
        match tokens.next() {
            Some(SymbolicBDDToken::Var(var)) => Ok(var.clone()),
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
                vars.push(Self::parse_variable_name(tokens)?);

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

        let transformer_var = Self::parse_variable_name(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;

        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::FixedPoint(
            transformer_var,
            initial,
            Box::new(formula),
        ))
    }

    fn parse_existence_quantifier(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::Exists, tokens)?;
        let vars = Self::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::Quantifier(
            QuantifierType::Exists,
            vars,
            Box::new(formula),
        ))
    }

    fn parse_universal_quantifier(tokens: &mut TokenReader) -> io::Result<Self> {
        expect(SymbolicBDDToken::Forall, tokens)?;
        let vars = Self::parse_variable_list(tokens)?;

        expect(SymbolicBDDToken::Hash, tokens)?;
        let formula = Self::parse_sub_formula(tokens)?;

        Ok(Self::Quantifier(
            QuantifierType::Forall,
            vars,
            Box::new(formula),
        ))
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
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected binary operator, got {:?}", other),
            )),
        }
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
                    "if" => result.push(SymbolicBDDToken::If),
                    "then" => result.push(SymbolicBDDToken::Then),
                    "else" => result.push(SymbolicBDDToken::Else),
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
        t => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected {:?}, got {:?}", token, t),
        )),
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
