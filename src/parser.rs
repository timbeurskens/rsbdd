use crate::bdd::{BDDEnv, NamedSymbol, BDD};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use std::cell::RefCell;
use std::io;
use std::io::BufRead;
use std::iter::Peekable;
use std::rc::Rc;
use std::slice::Iter;
use std::string::String;
use std::vec::Vec;

lazy_static! {
    static ref TOKENIZER: Regex = Regex::new(r#"(?P<symbol>!|&|=>|-|<=>|<=|\||\^|#|\*|\+|>=|=|>|<|\[|\]|,|\(|\))|(?P<countable>\d+)|(?P<identifier>\w+)|(?P<comment>"[^"]*")|(?P<eof>$)"#).unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicBDDToken {
    Var(String),
    Countable(usize),
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
    Hash,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CountableOperator {
    AtMost,
    LessThan,
    AtLeast,
    MoreThan,
    Exactly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QuantifierType {
    Exists,
    Forall,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolicBDD {
    False,
    True,
    Var(String),
    Not(Box<SymbolicBDD>),
    Quantifier(QuantifierType, Vec<String>, Box<SymbolicBDD>),
    CountableConst(CountableOperator, Vec<SymbolicBDD>, usize),
    CountableVariable(CountableOperator, Vec<SymbolicBDD>, Vec<SymbolicBDD>),
    Ite(Box<SymbolicBDD>, Box<SymbolicBDD>, Box<SymbolicBDD>),
    BinaryOp(BinaryOperator, Box<SymbolicBDD>, Box<SymbolicBDD>),
}

#[derive(Debug, Clone)]
pub struct ParsedFormula {
    pub vars: Vec<String>,
    pub bdd: SymbolicBDD,
    pub env: RefCell<BDDEnv<NamedSymbol>>,
}

type TokenReader<'a> = Peekable<Iter<'a, SymbolicBDDToken>>;

impl ParsedFormula {
    pub fn new(contents: &mut dyn BufRead) -> io::Result<Self> {
        let tokens = SymbolicBDD::tokenize(contents)?;

        let vars = tokens
            .iter()
            .filter_map(|t| match t {
                SymbolicBDDToken::Var(v) => Some(v.clone()),
                _ => None,
            })
            .unique()
            .collect();

        let formula = SymbolicBDD::parse_formula(&mut tokens.iter().peekable())?;

        Ok(ParsedFormula {
            vars,
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
            SymbolicBDD::Var(v) => self.env.borrow().var(self.name2var(v)),
            SymbolicBDD::Not(b) => self.env.borrow().not(self.eval_recursive(b)),
            SymbolicBDD::Quantifier(QuantifierType::Exists, v, b) => self.env.borrow().exists(
                v.into_iter().map(|i| self.name2var(i)).collect(),
                self.eval_recursive(b),
            ),
            SymbolicBDD::Quantifier(QuantifierType::Forall, v, b) => self.env.borrow().all(
                v.into_iter().map(|i| self.name2var(i)).collect(),
                self.eval_recursive(b),
            ),
            SymbolicBDD::CountableConst(op, bs, n) => {
                let branches = bs.iter().map(|b| self.eval_recursive(b)).collect();

                match op {
                    CountableOperator::AtMost => self.env.borrow().amn(&branches, *n as i64),
                    CountableOperator::AtLeast => self.env.borrow().aln(&branches, *n as i64),
                    CountableOperator::Exactly => self.env.borrow().exn(&branches, *n as i64),
                    CountableOperator::LessThan => self.env.borrow().amn(&branches, *n as i64 - 1),
                    CountableOperator::MoreThan => self.env.borrow().aln(&branches, *n as i64 + 1),
                }
            }
            SymbolicBDD::CountableVariable(op, l, r) => {
                let l_branches = l.iter().map(|b| self.eval_recursive(b)).collect();
                let r_branches = r.iter().map(|b| self.eval_recursive(b)).collect();

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
                }
            }
        }
    }

    pub fn var2usize(&self, var: &str) -> usize {
        self.vars.iter().position(|v| v == var).unwrap()
    }

    pub fn usize2var(&self, usize: usize) -> &str {
        &self.vars[usize]
    }

    pub fn name2var(&self, name: &str) -> NamedSymbol {
        NamedSymbol {
            name: Rc::new(name.to_string()),
            id: self.var2usize(name),
        }
    }
}

impl SymbolicBDD {
    fn parse_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let result = SymbolicBDD::parse_sub_formula(tokens)?;

        expect(SymbolicBDDToken::Eof, tokens)?;

        Ok(result)
    }

    fn parse_sub_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        let left = match tokens.peek() {
            Some(SymbolicBDDToken::OpenParen) => SymbolicBDD::parse_parentized_formula(tokens)?,
            Some(SymbolicBDDToken::OpenSquare) => SymbolicBDD::parse_countable_formula(tokens)?,
            Some(SymbolicBDDToken::False) => {
                expect(SymbolicBDDToken::False, tokens)?;
                SymbolicBDD::False
            }
            Some(SymbolicBDDToken::True) => {
                expect(SymbolicBDDToken::True, tokens)?;
                SymbolicBDD::True
            }
            Some(SymbolicBDDToken::Var(_)) => {
                SymbolicBDD::Var(SymbolicBDD::parse_variable_name(tokens)?)
            }
            Some(SymbolicBDDToken::Not) => SymbolicBDD::parse_negation(tokens)?,
            Some(SymbolicBDDToken::Exists) => SymbolicBDD::parse_existence_quantifier(tokens)?,
            Some(SymbolicBDDToken::Forall) => SymbolicBDD::parse_universal_quantifier(tokens)?,
            Some(SymbolicBDDToken::If) => SymbolicBDD::parse_ite(tokens)?,
            None | Some(SymbolicBDDToken::Eof) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected EOF"))
            }
            Some(other) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unexpected token {:?}", other),
                ))
            }
        };

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
                let op = SymbolicBDD::parse_binary_operator(tokens)?;
                let right = SymbolicBDD::parse_sub_formula(tokens)?;
                Ok(SymbolicBDD::BinaryOp(op, Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
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

    fn parse_variable_name(tokens: &mut TokenReader) -> io::Result<String> {
        match tokens.next() {
            Some(SymbolicBDDToken::Var(var)) => Ok(var.clone()),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected variable, got {:?}", other),
                ))
            }
        }
    }

    fn parse_variable_list(tokens: &mut TokenReader) -> io::Result<Vec<String>> {
        let mut vars = Vec::new();

        loop {
            if check(SymbolicBDDToken::Hash, tokens).is_err() {
                vars.push(SymbolicBDD::parse_variable_name(tokens)?);

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

    fn parse_negation(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::Not, tokens)?;

        if check_ident(tokens).is_ok() {
            Ok(SymbolicBDD::Not(Box::new(SymbolicBDD::Var(
                SymbolicBDD::parse_variable_name(tokens)?,
            ))))
        } else {
            let negated = SymbolicBDD::parse_sub_formula(tokens)?;
            Ok(SymbolicBDD::Not(Box::new(negated)))
        }
    }

    fn parse_parentized_formula(tokens: &mut TokenReader) -> io::Result<SymbolicBDD> {
        expect(SymbolicBDDToken::OpenParen, tokens)?;
        let subform = SymbolicBDD::parse_sub_formula(tokens)?;
        expect(SymbolicBDDToken::CloseParen, tokens)?;

        Ok(subform)
    }

    pub fn tokenize(contents: &mut dyn BufRead) -> io::Result<Vec<SymbolicBDDToken>> {
        let mut src: String = String::new();
        let mut result = Vec::new();

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
                    "implies" => result.push(SymbolicBDDToken::Implies),
                    "iff" => result.push(SymbolicBDDToken::Iff),
                    "eq" => result.push(SymbolicBDDToken::Iff),
                    "exists" => result.push(SymbolicBDDToken::Exists),
                    "forall" => result.push(SymbolicBDDToken::Forall),
                    "all" => result.push(SymbolicBDDToken::Forall),
                    "if" => result.push(SymbolicBDDToken::If),
                    "then" => result.push(SymbolicBDDToken::Then),
                    "else" => result.push(SymbolicBDDToken::Else),
                    var => result.push(SymbolicBDDToken::Var(var.to_string())),
                }
            } else if let Some(number) = c.name("countable") {
                let parsed_number = number.as_str().parse().expect("Failed to parse number");
                result.push(SymbolicBDDToken::Countable(parsed_number));
            } else if let Some(_) = c.name("eof") {
                result.push(SymbolicBDDToken::Eof);
            } else if let Some(_) = c.name("comment") {
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
        &Some(t) if *t == token => return Ok(()),
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

fn check_ident(tokens: &mut TokenReader) -> io::Result<()> {
    match tokens.peek() {
        Some(SymbolicBDDToken::Var(_)) => Ok(()),
        t => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Checked for ident/var, got {:?}; No capture condition available",
                t
            ),
        )),
    }
}
