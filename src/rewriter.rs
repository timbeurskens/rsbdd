use std::io;
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::NamedSymbol;
use crate::parser::{
    BinaryOperator, DomainConstant, ParsedFormula, QuantifierType, ReferenceContents, SymbolicBDD,
};

type Substitution = FxHashMap<NamedSymbol, DomainConstant>;

struct Rule {
    vars: Vec<NamedSymbol>,
    lhs: DomainConstant,
    rhs: SymbolicBDD,
}

/// Grounds rewrite rules into a plain formula.
///
/// An application of a predicate with rules is replaced by the conjunction of the right-hand sides of all
/// matching rules, or `false` if no rule matches. An application of a predicate without rules becomes a
/// boolean variable. Quantifiers over variables used as terms range over all ground terms in the formula.
pub struct Rewriter<'a> {
    parsed: &'a ParsedFormula,
    rules: Vec<Rule>,
    universe: Vec<DomainConstant>,
    atoms: FxHashMap<DomainConstant, NamedSymbol>,
    new_vars: Vec<NamedSymbol>,
    next_id: usize,
    expanding: FxHashSet<DomainConstant>,
}

impl<'a> Rewriter<'a> {
    /// Rewrite the parsed formula, returning the ground formula and the variables introduced for open atoms
    pub fn rewrite(parsed: &'a ParsedFormula) -> io::Result<(SymbolicBDD, Vec<NamedSymbol>)> {
        let mut rewriter = Self {
            parsed,
            rules: Vec::new(),
            universe: Vec::new(),
            atoms: FxHashMap::default(),
            new_vars: Vec::new(),
            next_id: parsed.vars.iter().map(|v| v.id + 1).max().unwrap_or(0),
            expanding: FxHashSet::default(),
        };

        let mut conjuncts = Vec::new();
        rewriter.split(&parsed.bdd, &[], &mut conjuncts)?;
        let formula = join(BinaryOperator::And, conjuncts);

        let mut universe = Vec::new();
        for rule in &rewriter.rules {
            add_term(&rule.lhs, &rule.vars, &mut universe, false);
            collect_terms(parsed, &rule.rhs, &mut rule.vars.clone(), &mut universe);
        }
        collect_terms(parsed, &formula, &mut Vec::new(), &mut universe);
        rewriter.universe = universe;

        let result = rewriter.expand(&formula, &Substitution::default())?;
        Ok((result, rewriter.new_vars))
    }

    fn split(
        &mut self,
        formula: &SymbolicBDD,
        vars: &[NamedSymbol],
        conjuncts: &mut Vec<SymbolicBDD>,
    ) -> io::Result<()> {
        match formula {
            SymbolicBDD::BinaryOp(BinaryOperator::And, l, r) => {
                self.split(l, vars, conjuncts)?;
                self.split(r, vars, conjuncts)
            }
            SymbolicBDD::BinaryOp(BinaryOperator::Rewrite, l, r) => {
                let SymbolicBDD::RuleApplication(lhs) = l.as_ref() else {
                    return Err(invalid(
                        "the left-hand side of a rewrite rule must be an application",
                    ));
                };
                if let Some(v) = vars.iter().find(|v| !contains(lhs, v)) {
                    return Err(invalid(&format!(
                        "summation variable {} does not occur in the left-hand side of {}",
                        v, lhs
                    )));
                }
                self.rules.push(Rule {
                    vars: vars.to_vec(),
                    lhs: lhs.clone(),
                    rhs: r.as_ref().clone(),
                });
                Ok(())
            }
            SymbolicBDD::Summation(sum_vars, body) => {
                self.split(body, &[vars, sum_vars].concat(), conjuncts)
            }
            SymbolicBDD::Reference(name) => match self.parsed.get_definition(name) {
                Some(ReferenceContents::Syntax(definition)) => {
                    self.split(&definition, vars, conjuncts)
                }
                _ => {
                    conjuncts.push(formula.clone());
                    Ok(())
                }
            },
            _ if !vars.is_empty() => Err(invalid("a summation may only contain rewrite rules")),
            _ => {
                conjuncts.push(formula.clone());
                Ok(())
            }
        }
    }

    fn expand(&mut self, formula: &SymbolicBDD, theta: &Substitution) -> io::Result<SymbolicBDD> {
        let mut expand = |f: &SymbolicBDD| self.expand(f, theta).map(Box::new);

        Ok(match formula {
            SymbolicBDD::RuleApplication(t) => return self.apply(subst(t, theta)),
            SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r) => {
                if term(l, theta)? == term(r, theta)? {
                    SymbolicBDD::True
                } else {
                    SymbolicBDD::False
                }
            }
            SymbolicBDD::BinaryOp(BinaryOperator::Rewrite, _, _) | SymbolicBDD::Summation(_, _) => {
                return Err(invalid(
                    "rewrite rules and summations must be top-level conjuncts",
                ));
            }
            SymbolicBDD::Quantifier(q, vars, body) => {
                let (domain, boolean): (Vec<_>, Vec<_>) = vars
                    .iter()
                    .cloned()
                    .partition(|v| self.occurs_as_term(body, v));
                let expanded = self.expand_domain(*q, &domain, body, theta)?;

                if boolean.is_empty() {
                    expanded
                } else {
                    SymbolicBDD::Quantifier(*q, boolean, Box::new(expanded))
                }
            }
            SymbolicBDD::Reference(name) => match self.parsed.get_definition(name) {
                Some(ReferenceContents::Syntax(definition)) => {
                    return self.expand(&definition, theta);
                }
                _ => formula.clone(),
            },
            SymbolicBDD::Not(f) => match *expand(f)? {
                SymbolicBDD::True => SymbolicBDD::False,
                SymbolicBDD::False => SymbolicBDD::True,
                f => SymbolicBDD::Not(Box::new(f)),
            },
            SymbolicBDD::BinaryOp(op, l, r) => {
                use BinaryOperator::{And, Implies, Or};
                use SymbolicBDD::{False, True};

                let l = *expand(l)?;
                match (op, &l) {
                    (And, False) => False,
                    (Or, True) | (Implies, False) => True,
                    _ => {
                        let r = *expand(r)?;
                        match (op, &l, &r) {
                            (And, True, _) | (Or, False, _) | (Implies, True, _) => r,
                            (And, _, True) | (Or, _, False) => l,
                            (And, _, False) => False,
                            (Or, _, True) | (Implies, _, True) => True,
                            _ => SymbolicBDD::BinaryOp(*op, Box::new(l), Box::new(r)),
                        }
                    }
                }
            }
            SymbolicBDD::Ite(c, t, e) => SymbolicBDD::Ite(expand(c)?, expand(t)?, expand(e)?),
            SymbolicBDD::FixedPoint(v, i, f) => SymbolicBDD::FixedPoint(v.clone(), *i, expand(f)?),
            SymbolicBDD::CountableConst(op, fs, n) => SymbolicBDD::CountableConst(
                *op,
                fs.iter()
                    .map(|f| expand(f).map(|f| *f))
                    .collect::<io::Result<_>>()?,
                *n,
            ),
            SymbolicBDD::CountableVariable(op, l, r) => SymbolicBDD::CountableVariable(
                *op,
                l.iter()
                    .map(|f| expand(f).map(|f| *f))
                    .collect::<io::Result<_>>()?,
                r.iter()
                    .map(|f| expand(f).map(|f| *f))
                    .collect::<io::Result<_>>()?,
            ),
            SymbolicBDD::True
            | SymbolicBDD::False
            | SymbolicBDD::Var(_)
            | SymbolicBDD::Subtree(_) => formula.clone(),
        })
    }

    fn expand_domain(
        &mut self,
        q: QuantifierType,
        vars: &[NamedSymbol],
        body: &SymbolicBDD,
        theta: &Substitution,
    ) -> io::Result<SymbolicBDD> {
        let Some((v, rest)) = vars.split_first() else {
            return self.expand(body, theta);
        };

        let instances = self
            .universe
            .clone()
            .into_iter()
            .map(|value| {
                let mut theta = theta.clone();
                theta.insert(v.clone(), value);
                self.expand_domain(q, rest, body, &theta)
            })
            .collect::<io::Result<Vec<_>>>()?;

        Ok(match q {
            QuantifierType::Forall => join(BinaryOperator::And, instances),
            QuantifierType::Exists => join(BinaryOperator::Or, instances),
        })
    }

    fn apply(&mut self, ground: DomainConstant) -> io::Result<SymbolicBDD> {
        if !self
            .rules
            .iter()
            .any(|rule| same_functor(&rule.lhs, &ground))
        {
            return Ok(SymbolicBDD::Var(self.atom(ground)));
        }

        if !self.expanding.insert(ground.clone()) {
            return Ok(SymbolicBDD::False);
        }

        let mut bodies = Vec::new();
        for i in 0..self.rules.len() {
            let mut sigma = Substitution::default();
            let rule = &self.rules[i];
            if matches(&rule.lhs, &ground, &rule.vars, &mut sigma) {
                let rhs = rule.rhs.clone();
                bodies.push(self.expand(&rhs, &sigma)?);
            }
        }

        self.expanding.remove(&ground);

        Ok(if bodies.is_empty() {
            SymbolicBDD::False
        } else {
            join(BinaryOperator::And, bodies)
        })
    }

    fn atom(&mut self, ground: DomainConstant) -> NamedSymbol {
        if let Some(symbol) = self.atoms.get(&ground) {
            return symbol.clone();
        }

        let symbol = NamedSymbol {
            name: Rc::new(ground.to_string()),
            id: self.next_id,
        };
        self.next_id += 1;
        self.new_vars.push(symbol.clone());
        self.atoms.insert(ground, symbol.clone());
        symbol
    }

    fn occurs_as_term(&self, formula: &SymbolicBDD, v: &NamedSymbol) -> bool {
        match formula {
            SymbolicBDD::RuleApplication(t) => contains(t, v),
            SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r)
                if [l, r]
                    .iter()
                    .any(|o| matches!(o.as_ref(), SymbolicBDD::Var(x) if x == v)) =>
            {
                true
            }
            SymbolicBDD::Reference(name) => match self.parsed.get_definition(name) {
                Some(ReferenceContents::Syntax(definition)) => self.occurs_as_term(&definition, v),
                _ => false,
            },
            other => children(other)
                .into_iter()
                .any(|c| self.occurs_as_term(c, v)),
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn join(op: BinaryOperator, items: Vec<SymbolicBDD>) -> SymbolicBDD {
    let (unit, zero) = match op {
        BinaryOperator::Or => (SymbolicBDD::False, SymbolicBDD::True),
        _ => (SymbolicBDD::True, SymbolicBDD::False),
    };

    let mut result: Option<SymbolicBDD> = None;
    for item in items {
        if item == zero {
            return zero;
        }
        if item != unit {
            result = Some(match result {
                None => item,
                Some(acc) => SymbolicBDD::BinaryOp(op, Box::new(acc), Box::new(item)),
            });
        }
    }
    result.unwrap_or(unit)
}

fn children(formula: &SymbolicBDD) -> Vec<&SymbolicBDD> {
    match formula {
        SymbolicBDD::Not(f)
        | SymbolicBDD::Quantifier(_, _, f)
        | SymbolicBDD::FixedPoint(_, _, f)
        | SymbolicBDD::Summation(_, f) => vec![f],
        SymbolicBDD::BinaryOp(_, l, r) => vec![l, r],
        SymbolicBDD::Ite(c, t, e) => vec![c, t, e],
        SymbolicBDD::CountableConst(_, fs, _) => fs.iter().collect(),
        SymbolicBDD::CountableVariable(_, l, r) => l.iter().chain(r).collect(),
        _ => vec![],
    }
}

fn term(operand: &SymbolicBDD, theta: &Substitution) -> io::Result<DomainConstant> {
    match operand {
        SymbolicBDD::Var(v) => Ok(subst(&DomainConstant::Terminal(v.clone()), theta)),
        SymbolicBDD::RuleApplication(t) => Ok(subst(t, theta)),
        other => Err(invalid(&format!("expected a term, got {:?}", other))),
    }
}

fn subst(t: &DomainConstant, theta: &Substitution) -> DomainConstant {
    match t {
        DomainConstant::Terminal(v) => theta.get(v).cloned().unwrap_or_else(|| t.clone()),
        DomainConstant::Application(f, args) => {
            DomainConstant::Application(f.clone(), args.iter().map(|a| subst(a, theta)).collect())
        }
    }
}

fn contains(t: &DomainConstant, v: &NamedSymbol) -> bool {
    match t {
        DomainConstant::Terminal(x) => x == v,
        DomainConstant::Application(_, args) => args.iter().any(|a| contains(a, v)),
    }
}

fn same_functor(a: &DomainConstant, b: &DomainConstant) -> bool {
    matches!((a, b), (DomainConstant::Application(f, xs), DomainConstant::Application(g, ys)) if f == g && xs.len() == ys.len())
}

fn matches(
    pattern: &DomainConstant,
    ground: &DomainConstant,
    vars: &[NamedSymbol],
    sigma: &mut Substitution,
) -> bool {
    match (pattern, ground) {
        (DomainConstant::Terminal(v), _) if vars.contains(v) => {
            sigma.entry(v.clone()).or_insert_with(|| ground.clone()) == ground
        }
        (DomainConstant::Application(f, ps), DomainConstant::Application(g, gs)) => {
            f == g
                && ps.len() == gs.len()
                && ps.iter().zip(gs).all(|(p, g)| matches(p, g, vars, sigma))
        }
        _ => pattern == ground,
    }
}

fn add_term(
    t: &DomainConstant,
    bound: &[NamedSymbol],
    universe: &mut Vec<DomainConstant>,
    is_element: bool,
) {
    if is_element && !bound.iter().any(|v| contains(t, v)) && !universe.contains(t) {
        universe.push(t.clone());
    }
    if let DomainConstant::Application(_, args) = t {
        for a in args {
            add_term(a, bound, universe, true);
        }
    }
}

fn collect_terms(
    parsed: &ParsedFormula,
    formula: &SymbolicBDD,
    bound: &mut Vec<NamedSymbol>,
    universe: &mut Vec<DomainConstant>,
) {
    match formula {
        SymbolicBDD::RuleApplication(t) => add_term(t, bound, universe, false),
        SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r) => {
            for operand in [l, r] {
                match operand.as_ref() {
                    SymbolicBDD::Var(v) => {
                        add_term(&DomainConstant::Terminal(v.clone()), bound, universe, true)
                    }
                    SymbolicBDD::RuleApplication(t) => add_term(t, bound, universe, true),
                    other => collect_terms(parsed, other, bound, universe),
                }
            }
        }
        SymbolicBDD::Quantifier(_, vars, f) | SymbolicBDD::Summation(vars, f) => {
            let n = bound.len();
            bound.extend(vars.iter().cloned());
            collect_terms(parsed, f, bound, universe);
            bound.truncate(n);
        }
        SymbolicBDD::Reference(name) => {
            if let Some(ReferenceContents::Syntax(definition)) = parsed.get_definition(name) {
                collect_terms(parsed, &definition, bound, universe);
            }
        }
        other => {
            for c in children(other) {
                collect_terms(parsed, c, bound, universe);
            }
        }
    }
}
