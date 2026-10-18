use std::io;
use std::rc::Rc;

use itertools::Itertools;
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
    functions: FxHashSet<NamedSymbol>,
}

/// Grounds rewrite rules into a plain formula.
///
/// An application of a predicate with rules is replaced by the conjunction of the right-hand sides of all
/// matching rules, or `false` if no rule matches. An application of a predicate without rules becomes a
/// boolean variable. Quantifiers over variables used as terms range over all ground terms in the formula.
///
/// A rule for a function symbol (a symbol used inside a term or as an operand of `=`) is a term rule: its right-hand
/// side is a term. Terms are rewritten to normal form before they are compared, matched or turned into variables,
/// so `=` holds modulo the term rules. This decides equality if the term rules form a convergent rewrite system.
///
/// `induct x, .. # l = r` proves an equation by induction on `x`, using the patterns of the rules defining a function
/// applied to `x` as cases and the induction hypotheses as rewrite rules. It is `true` if every case is proven and
/// `false` otherwise. A proven equation becomes a rewrite rule for the remainder of the formula.
pub struct Rewriter<'a> {
    rules: &'a [Rule],
    term_rules: Vec<Rule>,
    definitions: usize,
    skolems: usize,
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

        let (term_rules, rules): (Vec<_>, Vec<_>) = rules.into_iter().partition(|rule| {
            matches!(&rule.lhs, DomainConstant::Application(f, _) if universe.functions.contains(f))
        });
        for rule in &term_rules {
            add_element(
                &term(&rule.rhs, &Substitution::default())?,
                &rule.vars,
                &mut universe,
            );
        }
        let terms: Vec<_> = universe
            .terms
            .iter()
            .map(|t| normalize(t, &term_rules))
            .collect::<io::Result<_>>()?;
        let terms: Vec<_> = terms.into_iter().unique().collect();

        let mut rewriter = Rewriter {
            rules: &rules,
            definitions: term_rules.len(),
            term_rules,
            skolems: 0,
            universe: &terms,
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
                if normalize(&term(l, theta)?, &self.term_rules)?
                    == normalize(&term(r, theta)?, &self.term_rules)?
                {
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
            SymbolicBDD::Quantifier(QuantifierType::Induction, vars, body) => {
                let SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r) = body.as_ref() else {
                    return Err(invalid("the body of an induction must be an equation"));
                };
                let (l, r) = (term(l, theta)?, term(r, theta)?);
                if !self.induct(vars, &l, &r)? {
                    return Ok(SymbolicBDD::False);
                }
                self.term_rules.extend(orient(&l, &r, vars));
                SymbolicBDD::True
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
            QuantifierType::Exists => join(BinaryOperator::Or, instances),
            _ => join(BinaryOperator::And, instances),
        })
    }

    fn apply(&mut self, ground: DomainConstant) -> io::Result<SymbolicBDD> {
        let ground = normalize(&ground, &self.term_rules)?;
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

    /// Prove `l = r` for all values of the first variable, with a case for every pattern of the rules defining a function
    /// applied to it, and an induction hypothesis for every variable of the pattern
    fn induct(
        &mut self,
        vars: &[NamedSymbol],
        l: &DomainConstant,
        r: &DomainConstant,
    ) -> io::Result<bool> {
        let (x, rest) = vars
            .split_first()
            .ok_or_else(|| invalid("an induction requires a variable"))?;
        let definitions = &self.term_rules[..self.definitions];
        let Some((f, i)) = position(l, x, definitions).or_else(|| position(r, x, definitions))
        else {
            return Ok(false);
        };
        let cases: Vec<_> = definitions
            .iter()
            .filter_map(|rule| match &rule.lhs {
                DomainConstant::Application(_, args) if same_functor(&rule.lhs, &f) => {
                    Some((args[i].clone(), rule.vars.clone()))
                }
                _ => None,
            })
            .collect();

        let at = |t: &DomainConstant, value: &DomainConstant| {
            subst(t, &Substitution::from_iter([(x.clone(), value.clone())]))
        };
        for (pattern, pattern_vars) in cases {
            let skolems: Substitution = pattern_vars
                .iter()
                .filter(|v| contains(&pattern, v))
                .map(|v| (v.clone(), DomainConstant::Terminal(self.skolem(v))))
                .collect();
            let case = subst(&pattern, &skolems);
            let hypotheses: Vec<_> = skolems
                .values()
                .filter(|s| **s != case)
                .filter_map(|s| orient(&at(l, s), &at(r, s), rest))
                .collect();

            let n = self.term_rules.len();
            self.term_rules.extend(hypotheses);
            let proven = normalize(&at(l, &case), &self.term_rules)
                .and_then(|l| Ok(l == normalize(&at(r, &case), &self.term_rules)?));
            self.term_rules.truncate(n);
            if !proven? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn skolem(&mut self, v: &NamedSymbol) -> NamedSymbol {
        self.skolems += 1;
        NamedSymbol {
            name: Rc::new(format!("{v}'")),
            id: usize::MAX - self.skolems,
        }
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

/// Rewrite a term to normal form, innermost first
fn normalize(t: &DomainConstant, rules: &[Rule]) -> io::Result<DomainConstant> {
    let t = match t {
        DomainConstant::Application(f, args) => DomainConstant::Application(
            f.clone(),
            args.iter()
                .map(|a| normalize(a, rules))
                .collect::<io::Result<_>>()?,
        ),
        terminal => terminal.clone(),
    };
    for rule in rules {
        let mut sigma = Substitution::default();
        if match_pattern(&rule.lhs, &t, &rule.vars, &mut sigma) {
            return normalize(&term(&rule.rhs, &sigma)?, rules);
        }
    }
    Ok(t)
}

/// Find an application of a defined function with `x` as an argument, returning the application and the position of `x`
fn position(
    t: &DomainConstant,
    x: &NamedSymbol,
    rules: &[Rule],
) -> Option<(DomainConstant, usize)> {
    let DomainConstant::Application(_, args) = t else {
        return None;
    };
    args.iter()
        .position(|a| matches!(a, DomainConstant::Terminal(v) if v == x))
        .filter(|_| rules.iter().any(|rule| same_functor(&rule.lhs, t)))
        .map(|i| (t.clone(), i))
        .or_else(|| args.iter().find_map(|a| position(a, x, rules)))
}

/// Orient an equation into a rewrite rule whose right-hand side is not an instance of its left-hand side
fn orient(l: &DomainConstant, r: &DomainConstant, vars: &[NamedSymbol]) -> Option<Rule> {
    [(l, r), (r, l)]
        .into_iter()
        .find(|(lhs, rhs)| {
            matches!(lhs, DomainConstant::Application(..))
                && vars.iter().all(|v| !contains(rhs, v) || contains(lhs, v))
                && !match_pattern(lhs, rhs, vars, &mut Substitution::default())
        })
        .map(|(lhs, rhs)| Rule {
            vars: vars.to_vec(),
            lhs: lhs.clone(),
            rhs: match rhs {
                DomainConstant::Terminal(v) => SymbolicBDD::Var(v.clone()),
                t => SymbolicBDD::RuleApplication(t.clone()),
            },
        })
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
    if let DomainConstant::Application(f, _) = t {
        universe.functions.insert(f.clone());
    }
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
