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

/// Ground terms in order of first occurrence
#[derive(Default)]
struct Universe {
    terms: Vec<DomainConstant>,
    seen: FxHashSet<DomainConstant>,
}

/// Grounds rewrite rules into a plain formula.
///
/// An application of a predicate with rules is replaced by the conjunction of the right-hand sides of all
/// matching rules, or `false` if no rule matches. An application of a predicate without rules becomes a
/// boolean variable. Quantifiers over variables used as terms range over all ground terms in the formula.
pub struct Rewriter<'a> {
    rules: &'a [Rule],
    universe: &'a [DomainConstant],
    atoms: FxHashMap<DomainConstant, NamedSymbol>,
    base_id: usize,
    expanding: FxHashSet<DomainConstant>,
}

impl Rewriter<'_> {
    /// Rewrite the parsed formula, returning the ground formula and the variables introduced for open atoms
    pub fn rewrite(parsed: &ParsedFormula) -> io::Result<(SymbolicBDD, Vec<NamedSymbol>)> {
        let mut rules = Vec::new();
        let mut conjuncts = Vec::new();
        split(
            &inline(parsed, &parsed.bdd),
            &[],
            &mut rules,
            &mut conjuncts,
        )?;
        let formula = join(BinaryOperator::And, conjuncts);

        let mut universe = Universe::default();
        for rule in &rules {
            add_args(&rule.lhs, &rule.vars, &mut universe);
            collect_terms(&rule.rhs, &rule.vars, &mut universe);
        }
        collect_terms(&formula, &[], &mut universe);

        let mut rewriter = Rewriter {
            rules: &rules,
            universe: &universe.terms,
            atoms: FxHashMap::default(),
            base_id: parsed.vars.iter().map(|v| v.id + 1).max().unwrap_or(0),
            expanding: FxHashSet::default(),
        };
        let result = rewriter.expand(&formula, &Substitution::default())?;

        let mut new_vars: Vec<_> = rewriter.atoms.into_values().collect();
        new_vars.sort_by_key(|v| v.id);
        Ok((result, new_vars))
    }

    fn expand(&mut self, formula: &SymbolicBDD, theta: &Substitution) -> io::Result<SymbolicBDD> {
        let mut expand = |f: &SymbolicBDD| self.expand(f, theta);

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
                let (domain, boolean): (Vec<_>, Vec<_>) =
                    vars.iter().cloned().partition(|v| occurs_as_term(body, v));
                let expanded = self.expand_domain(*q, &domain, body, &mut theta.clone())?;

                if boolean.is_empty() {
                    expanded
                } else {
                    SymbolicBDD::Quantifier(*q, boolean, Box::new(expanded))
                }
            }
            SymbolicBDD::Not(f) => match expand(f)? {
                SymbolicBDD::True => SymbolicBDD::False,
                SymbolicBDD::False => SymbolicBDD::True,
                f => SymbolicBDD::Not(Box::new(f)),
            },
            SymbolicBDD::BinaryOp(op, l, r) => {
                use BinaryOperator::{And, Implies, Or};
                use SymbolicBDD::{False, True};

                let l = expand(l)?;
                let absorbing = matches!((op, &l), (And, False) | (Or, True) | (Implies, False));
                let r = if absorbing { True } else { expand(r)? };
                SymbolicBDD::binop(*op, l, r)
            }
            SymbolicBDD::Ite(c, t, e) => SymbolicBDD::Ite(
                Box::new(expand(c)?),
                Box::new(expand(t)?),
                Box::new(expand(e)?),
            ),
            SymbolicBDD::FixedPoint(v, i, f) => {
                SymbolicBDD::FixedPoint(v.clone(), *i, Box::new(expand(f)?))
            }
            SymbolicBDD::CountableConst(op, fs, n) => SymbolicBDD::CountableConst(
                *op,
                fs.iter().map(&mut expand).collect::<io::Result<_>>()?,
                *n,
            ),
            SymbolicBDD::CountableVariable(op, l, r) => SymbolicBDD::CountableVariable(
                *op,
                l.iter().map(&mut expand).collect::<io::Result<_>>()?,
                r.iter().map(&mut expand).collect::<io::Result<_>>()?,
            ),
            SymbolicBDD::True
            | SymbolicBDD::False
            | SymbolicBDD::Var(_)
            | SymbolicBDD::Subtree(_)
            | SymbolicBDD::Reference(_) => formula.clone(),
        })
    }

    fn expand_domain(
        &mut self,
        q: QuantifierType,
        vars: &[NamedSymbol],
        body: &SymbolicBDD,
        theta: &mut Substitution,
    ) -> io::Result<SymbolicBDD> {
        let Some((v, rest)) = vars.split_first() else {
            return self.expand(body, theta);
        };

        let mut instances = Vec::with_capacity(self.universe.len());
        for value in self.universe {
            let previous = theta.insert(v.clone(), value.clone());
            let instance = self.expand_domain(q, rest, body, theta);
            match previous {
                Some(previous) => theta.insert(v.clone(), previous),
                None => theta.remove(v),
            };
            instances.push(instance?);
        }

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
        for rule in self.rules {
            let mut sigma = Substitution::default();
            if match_pattern(&rule.lhs, &ground, &rule.vars, &mut sigma) {
                bodies.push(self.expand(&rule.rhs, &sigma)?);
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
        let id = self.base_id + self.atoms.len();
        self.atoms
            .entry(ground)
            .or_insert_with_key(|ground| NamedSymbol {
                name: Rc::new(ground.to_string()),
                id,
            })
            .clone()
    }
}

/// Replace references to syntactic definitions by their contents
fn inline(parsed: &ParsedFormula, formula: &SymbolicBDD) -> SymbolicBDD {
    let inline = |f: &SymbolicBDD| inline(parsed, f);
    let boxed = |f: &SymbolicBDD| Box::new(inline(f));

    match formula {
        SymbolicBDD::Reference(name) => match parsed.get_definition(name) {
            Some(ReferenceContents::Syntax(definition)) => inline(&definition),
            _ => formula.clone(),
        },
        SymbolicBDD::Not(f) => SymbolicBDD::Not(boxed(f)),
        SymbolicBDD::Quantifier(q, vars, f) => SymbolicBDD::Quantifier(*q, vars.clone(), boxed(f)),
        SymbolicBDD::Summation(vars, f) => SymbolicBDD::Summation(vars.clone(), boxed(f)),
        SymbolicBDD::FixedPoint(v, i, f) => SymbolicBDD::FixedPoint(v.clone(), *i, boxed(f)),
        SymbolicBDD::BinaryOp(op, l, r) => SymbolicBDD::BinaryOp(*op, boxed(l), boxed(r)),
        SymbolicBDD::Ite(c, t, e) => SymbolicBDD::Ite(boxed(c), boxed(t), boxed(e)),
        SymbolicBDD::CountableConst(op, fs, n) => {
            SymbolicBDD::CountableConst(*op, fs.iter().map(inline).collect(), *n)
        }
        SymbolicBDD::CountableVariable(op, l, r) => SymbolicBDD::CountableVariable(
            *op,
            l.iter().map(inline).collect(),
            r.iter().map(inline).collect(),
        ),
        SymbolicBDD::True
        | SymbolicBDD::False
        | SymbolicBDD::Var(_)
        | SymbolicBDD::RuleApplication(_)
        | SymbolicBDD::Subtree(_) => formula.clone(),
    }
}

/// Separate the top-level conjuncts of a formula into rewrite rules and remaining conjuncts
fn split(
    formula: &SymbolicBDD,
    vars: &[NamedSymbol],
    rules: &mut Vec<Rule>,
    conjuncts: &mut Vec<SymbolicBDD>,
) -> io::Result<()> {
    match formula {
        SymbolicBDD::BinaryOp(BinaryOperator::And, l, r) => {
            split(l, vars, rules, conjuncts)?;
            split(r, vars, rules, conjuncts)
        }
        SymbolicBDD::BinaryOp(BinaryOperator::Rewrite, l, r) => {
            let SymbolicBDD::RuleApplication(lhs) = l.as_ref() else {
                return Err(invalid(
                    "the left-hand side of a rewrite rule must be an application",
                ));
            };
            if let Some(v) = vars.iter().find(|v| !contains(lhs, v)) {
                return Err(invalid(format!(
                    "summation variable {v} does not occur in the left-hand side of {lhs}"
                )));
            }
            rules.push(Rule {
                vars: vars.to_vec(),
                lhs: lhs.clone(),
                rhs: r.as_ref().clone(),
            });
            Ok(())
        }
        SymbolicBDD::Summation(sum_vars, body) => {
            split(body, &[vars, sum_vars].concat(), rules, conjuncts)
        }
        SymbolicBDD::Reference(_) => {
            conjuncts.push(formula.clone());
            Ok(())
        }
        _ if !vars.is_empty() => Err(invalid("a summation may only contain rewrite rules")),
        _ => {
            conjuncts.push(formula.clone());
            Ok(())
        }
    }
}

fn occurs_as_term(formula: &SymbolicBDD, v: &NamedSymbol) -> bool {
    match formula {
        SymbolicBDD::RuleApplication(t) => contains(t, v),
        SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r)
            if [l, r]
                .iter()
                .any(|o| matches!(o.as_ref(), SymbolicBDD::Var(x) if x == v)) =>
        {
            true
        }
        other => other.children().into_iter().any(|c| occurs_as_term(c, v)),
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn join(op: BinaryOperator, items: Vec<SymbolicBDD>) -> SymbolicBDD {
    let unit = match op {
        BinaryOperator::Or => SymbolicBDD::False,
        _ => SymbolicBDD::True,
    };
    items
        .into_iter()
        .fold(unit, |acc, item| SymbolicBDD::binop(op, acc, item))
}

fn term(operand: &SymbolicBDD, theta: &Substitution) -> io::Result<DomainConstant> {
    match operand {
        SymbolicBDD::Var(v) => Ok(subst(&DomainConstant::Terminal(v.clone()), theta)),
        SymbolicBDD::RuleApplication(t) => Ok(subst(t, theta)),
        other => Err(invalid(format!("expected a term, got {other:?}"))),
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

fn match_pattern(
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
                && ps
                    .iter()
                    .zip(gs)
                    .all(|(p, g)| match_pattern(p, g, vars, sigma))
        }
        _ => pattern == ground,
    }
}

/// Add the arguments of an application (but not the application itself) to the universe
fn add_args(t: &DomainConstant, bound: &[NamedSymbol], universe: &mut Universe) {
    if let DomainConstant::Application(_, args) = t {
        for a in args {
            add_element(a, bound, universe);
        }
    }
}

/// Add a term and its subterms to the universe, skipping terms containing bound variables
fn add_element(t: &DomainConstant, bound: &[NamedSymbol], universe: &mut Universe) {
    if !bound.iter().any(|v| contains(t, v)) && universe.seen.insert(t.clone()) {
        universe.terms.push(t.clone());
    }
    add_args(t, bound, universe);
}

fn collect_terms(formula: &SymbolicBDD, bound: &[NamedSymbol], universe: &mut Universe) {
    match formula {
        SymbolicBDD::RuleApplication(t) => add_args(t, bound, universe),
        SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r) => {
            for operand in [l, r] {
                match operand.as_ref() {
                    SymbolicBDD::Var(v) => {
                        add_element(&DomainConstant::Terminal(v.clone()), bound, universe)
                    }
                    SymbolicBDD::RuleApplication(t) => add_element(t, bound, universe),
                    other => collect_terms(other, bound, universe),
                }
            }
        }
        SymbolicBDD::Quantifier(_, vars, f) | SymbolicBDD::Summation(vars, f) => {
            collect_terms(f, &[bound, vars].concat(), universe)
        }
        other => {
            for c in other.children() {
                collect_terms(c, bound, universe);
            }
        }
    }
}
