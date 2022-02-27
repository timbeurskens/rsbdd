use crate::parser::{SymbolicBDD, DomainConstant};
use std::vec::Vec;

#[derive(Debug, Clone)]
pub struct Rewriter {
    pub environment: Vec<DomainConstant>,
    pub rules: SymbolicBDD,
    pub formula: SymbolicBDD,
}

impl Rewriter {
    pub fn new(rules: SymbolicBDD, formula: SymbolicBDD) -> Self {
        Self {
            environment: vec![],
            rules,
            formula,
        }
    }

    pub fn merge(&mut self) {

    }

    fn merge_recursive(&self, root: &SymbolicBDD) -> SymbolicBDD {
        match root {
            SymbolicBDD::RuleApplication(ref rule) => self.apply_rules(rule),
            SymbolicBDD::Not(ref f) => SymbolicBDD::Not(Box::new(self.merge_recursive(f))),
            SymbolicBDD::Quantifier(ref t, ref v, ref f) => SymbolicBDD::Quantifier(t.clone(), v.clone(), Box::new(self.merge_recursive(f))),
            // CountableConst(CountableOperator, Vec<SymbolicBDD>, usize),
            // CountableVariable(CountableOperator, Vec<SymbolicBDD>, Vec<SymbolicBDD>),
            // Ite(Box<SymbolicBDD>, Box<SymbolicBDD>, Box<SymbolicBDD>),
            // BinaryOp(BinaryOperator, Box<SymbolicBDD>, Box<SymbolicBDD>),
            // Summation(Vec<String>, Box<SymbolicBDD>),
            // RuleApplication(DomainConstant),
            // RewriteRule(DomainConstant, Box<SymbolicBDD>),
            SymbolicBDD::RewriteRule(_, _) => panic!("RewriteRule should not be in the formula"),
            other => other.clone(),
        }
    }

    fn apply_rules(&self, dc: &DomainConstant) -> SymbolicBDD {
        unimplemented!()
    }
}