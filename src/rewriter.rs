use crate::parser::{SymbolicBDD, DomainConstant};

#[derive(Debug, Clone)]
pub struct Rewriter {
    pub rules: SymbolicBDD,
    pub formula: SymbolicBDD,
}

impl Rewriter {
    pub fn new(rules: SymbolicBDD, formula: SymbolicBDD) -> Self {
        Self { rules, formula }
    }

    pub fn merge(&mut self) {

    }

    fn merge_recursive(root: &SymbolicBDD, replacement: &SymbolicBDD) -> SymbolicBDD {
        match root {
            SymbolicBDD::RuleApplication(ref rule) => Rewriter::apply_rules(rule, replacement),
            SymbolicBDD::Not(ref f) => SymbolicBDD::Not(Box::new(Rewriter::merge_recursive(f, replacement))),
            SymbolicBDD::Quantifier(ref t, ref v, ref f) => SymbolicBDD::Quantifier(t.clone(), v.clone(), Box::new(Rewriter::merge_recursive(f, replacement))),
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

    fn apply_rules(dc: &DomainConstant, rules: &SymbolicBDD) -> SymbolicBDD {
        unimplemented!()
    }
}