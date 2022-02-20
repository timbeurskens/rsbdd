use crate::bdd::{BDDEnv, BDD};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::iter::Peekable;
use std::result::Result;
use std::slice::Iter;
use std::string::String;
use std::vec::Vec;
use std::cell::RefCell;
use std::rc::Rc;

lazy_static! {
    static ref TOKENIZER: Regex = Regex::new(r#"(?P<symbol>!|&|=>|-|<=>|<=|\||\^)|(?P<identifier>[\w\d]+)|(?P<open>\()|(?P<close>\))|(?P<eof>$)"#).unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicBDDToken {
    Var(String),
    And,
    Or,
    Not,
    Xor,
    Implies,
    ImpliesInv,
    Iff,
    OpenParen,
    CloseParen,
    False,
    True,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    And,
    Or,
    Xor,
    Implies,
    ImpliesInv,
    Iff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicBDD {
    False,
    True,
    Var(String),
    Not(Box<SymbolicBDD>),
    BinaryOp(BinaryOperator, Box<SymbolicBDD>, Box<SymbolicBDD>),
}

#[derive(Debug, Clone)]
pub struct ParsedFormula {
    pub vars: Vec<String>,
    pub bdd: SymbolicBDD,
    env: RefCell<BDDEnv<usize>>,
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

        Ok(ParsedFormula { vars, bdd: formula, env: RefCell::new(BDDEnv::new()) })
    }

    pub fn eval(&self) -> Rc<BDD<usize>> {
        self.eval_recursive(&self.bdd)
    }

    fn eval_recursive(&self, root: &SymbolicBDD) -> Rc<BDD<usize>> {
        match root {
            SymbolicBDD::False => self.env.borrow().mk_const(false),
            SymbolicBDD::True => self.env.borrow().mk_const(true),
            SymbolicBDD::Var(v) => self.env.borrow().var(self.var2usize(v)),
            SymbolicBDD::Not(b) => self.env.borrow().not(self.eval_recursive(b)),
            SymbolicBDD::BinaryOp(op, l, r) => {
                let l = self.eval_recursive(l);
                let r = self.eval_recursive(r);

                match op {
                    BinaryOperator::And => self.env.borrow().and(l, r),
                    BinaryOperator::Or => self.env.borrow().or(l, r),
                    BinaryOperator::Xor => self.env.borrow().xor(l, r),
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
            Some(SymbolicBDDToken::False) => {
                expect(SymbolicBDDToken::False, tokens)?;
                SymbolicBDD::False
            }
            Some(SymbolicBDDToken::True) => {
                expect(SymbolicBDDToken::True, tokens)?;
                SymbolicBDD::False
            }
            Some(SymbolicBDDToken::Var(var)) => {
                expect(SymbolicBDDToken::Var(var.clone()), tokens)?;
                SymbolicBDD::Var(var.clone())
            }
            Some(SymbolicBDDToken::Not) => SymbolicBDD::parse_negation(tokens)?,
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

        match tokens.peek() {
            Some(SymbolicBDDToken::And)
            | Some(SymbolicBDDToken::Or)
            | Some(SymbolicBDDToken::Xor)
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
        let negated = SymbolicBDD::parse_sub_formula(tokens)?;
        Ok(SymbolicBDD::Not(Box::new(negated)))
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
                    "&" => result.push(SymbolicBDDToken::And),
                    "|" => result.push(SymbolicBDDToken::Or),
                    "^" => result.push(SymbolicBDDToken::Xor),
                    "-" => result.push(SymbolicBDDToken::Not),
                    "!" => result.push(SymbolicBDDToken::Not),
                    "=>" => result.push(SymbolicBDDToken::Implies),
                    "<=" => result.push(SymbolicBDDToken::ImpliesInv),
                    "<=>" => result.push(SymbolicBDDToken::Iff),
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
                    "implies" => result.push(SymbolicBDDToken::Implies),
                    "iff" => result.push(SymbolicBDDToken::Iff),
                    "eq"=> result.push(SymbolicBDDToken::Iff),
                    var => result.push(SymbolicBDDToken::Var(var.to_string())),
                }
            } else if let Some(_) = c.name("open") {
                result.push(SymbolicBDDToken::OpenParen);
            } else if let Some(_) = c.name("close") {
                result.push(SymbolicBDDToken::CloseParen);
            } else if let Some(_) = c.name("eof") {
                result.push(SymbolicBDDToken::Eof);
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown token"));
            }
        }

        Ok(result)
    }
}

fn expect(token: SymbolicBDDToken, tokens: &mut TokenReader) -> io::Result<()> {
    match &tokens.next() {
        &Some(t) if *t == token => return Ok(()),
        &Some(t) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected {:?}, got {:?}", token, t),
            ))
        }
        &None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected {:?}, but got None", token),
            ))
        }
    }
}

// syntax:
// a
// a & b
// a | b
// a and b
// a or b
// a => b
// a <=> b
// !a
// not a
// a & !b
// !a & b
// !a & !b
// (a & b) | c
// (a & b) | (c & d)
// (a & b) | (c & d) | e
// a | b | c
// a & b & c
// a => b => c == (a => (b => c))
// ((a)) == a
//
