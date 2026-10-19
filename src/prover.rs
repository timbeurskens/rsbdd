use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io;
use std::rc::Rc;

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::NamedSymbol;
use crate::parser::{BinaryOperator, DomainConstant, ParsedFormula, QuantifierType, SymbolicBDD};
use crate::rewriter::{
    Rule, Substitution, invalid, lpo, match_pattern, normalize, occurs_as_term, positions, rank,
    replace, resolve, size, split_rules, subst, term, unify,
};

const PROVER_STEPS: usize = 100000;
const DISTRIBUTION_LIMIT: usize = 4;

/// The result of a proof attempt
pub enum Outcome {
    /// The formula is valid, with the derivation of the empty clause
    Proven(Vec<String>),
    /// The clauses are saturated without the empty clause: the formula is not valid
    Disproven,
    /// No proof was found within the step limit
    Unknown,
}

#[derive(Clone, PartialEq)]
struct Literal {
    positive: bool,
    l: DomainConstant,
    r: DomainConstant,
}

struct Clause {
    literals: Vec<Literal>,
    vars: Vec<NamedSymbol>,
    origin: &'static str,
    parents: Vec<usize>,
}

#[derive(Clone, Default)]
struct Scope {
    terms: Substitution,
    universals: Vec<DomainConstant>,
    booleans: FxHashMap<NamedSymbol, bool>,
}

/// A first-order theorem prover by superposition (Bachmair & Ganzinger, 1994) with a given-clause loop.
///
/// The rules of the formula are axioms: a term rule `l -> r` is the equation `l = r`, a rule `p(..) -> f` is the
/// equivalence `p(..) <=> f`, both for all values of its variables. The remaining formula is the goal: its negation is
/// Skolemized and converted to clauses together with the axioms, and the formula is proven if the clauses derive the
/// empty clause. A predicate `p(..)` is the equation `p(..) = true`, so all inferences are on equations. The
/// inferences are restricted by the lexicographic path order of the completion, and clauses are simplified by
/// demodulation with oriented unit equations, and by tautology deletion and subsumption.
pub struct Prover {
    clauses: Vec<Clause>,
    variables: FxHashSet<NamedSymbol>,
    precedence: FxHashMap<NamedSymbol, (usize, usize)>,
    top: DomainConstant,
    fresh: usize,
    definitions: Vec<Vec<Literal>>,
}

impl Prover {
    pub fn prove(parsed: &ParsedFormula) -> io::Result<Outcome> {
        let (rules, goal, universe) = split_rules(parsed)?;
        let mut prover = Self {
            clauses: Vec::new(),
            variables: FxHashSet::default(),
            precedence: FxHashMap::default(),
            top: DomainConstant::Terminal(NamedSymbol {
                name: Rc::new("true".to_string()),
                id: usize::MAX / 2,
            }),
            fresh: 0,
            definitions: Vec::new(),
        };

        for rule in &rules {
            let lhs = SymbolicBDD::from_term(&rule.lhs);
            let axiom = match &rule.lhs {
                DomainConstant::Application(f, _) if universe.functions.contains(f) => {
                    SymbolicBDD::binop(BinaryOperator::Equals, lhs, rule.rhs.clone())
                }
                _ => SymbolicBDD::binop(BinaryOperator::Iff, lhs, rule.rhs.clone()),
            };
            let axiom =
                SymbolicBDD::Quantifier(QuantifierType::Forall, rule.vars.clone(), Box::new(axiom));
            prover.input(&axiom, true, "axiom")?;
        }
        prover.input(&goal, false, "negated goal")?;

        let mut ranks = FxHashMap::default();
        rank(&prover.top, &[], &mut ranks);
        for clause in &prover.clauses {
            for literal in &clause.literals {
                rank(&literal.l, &clause.vars, &mut ranks);
                rank(&literal.r, &clause.vars, &mut ranks);
            }
        }
        prover.precedence = ranks;

        Ok(prover.saturate())
    }

    /// Add the clauses of a formula, or of its negation
    fn input(
        &mut self,
        formula: &SymbolicBDD,
        positive: bool,
        origin: &'static str,
    ) -> io::Result<()> {
        let nnf = self.nnf(formula, positive, &Scope::default())?;
        for literals in nnf.into_iter().chain(std::mem::take(&mut self.definitions)) {
            self.add(literals, origin, vec![]);
        }
        Ok(())
    }

    /// The clauses of the negation normal form of a formula (or its negation), with Skolemized existentials
    fn nnf(
        &mut self,
        f: &SymbolicBDD,
        positive: bool,
        scope: &Scope,
    ) -> io::Result<Vec<Vec<Literal>>> {
        use BinaryOperator::{And, Equals, Iff, Implies, ImpliesInv, Nand, Nor, Or, Xor};
        let literal = |l: DomainConstant, r: DomainConstant| vec![vec![Literal { positive, l, r }]];

        Ok(match f {
            SymbolicBDD::True | SymbolicBDD::False => {
                if matches!(f, SymbolicBDD::True) == positive {
                    vec![]
                } else {
                    vec![vec![]]
                }
            }
            SymbolicBDD::Var(v) if scope.booleans.contains_key(v) => {
                let value = if scope.booleans[v] {
                    SymbolicBDD::True
                } else {
                    SymbolicBDD::False
                };
                self.nnf(&value, positive, scope)?
            }
            SymbolicBDD::Var(_) | SymbolicBDD::RuleApplication(_) => {
                literal(term(f, &scope.terms)?, self.top.clone())
            }
            SymbolicBDD::BinaryOp(Equals, l, r) => {
                literal(term(l, &scope.terms)?, term(r, &scope.terms)?)
            }
            SymbolicBDD::Not(g) => self.nnf(g, !positive, scope)?,
            SymbolicBDD::BinaryOp(op @ (And | Or | Nand | Nor), a, b) => {
                let positive_child = matches!(op, And | Or) == positive;
                let a = self.nnf(a, positive_child, scope)?;
                let b = self.nnf(b, positive_child, scope)?;
                self.combine(matches!(op, And | Nor) == positive, a, b)
            }
            SymbolicBDD::BinaryOp(op @ (Implies | ImpliesInv), a, b) => {
                let (a, b) = if *op == Implies { (a, b) } else { (b, a) };
                let a = self.nnf(a, !positive, scope)?;
                let b = self.nnf(b, positive, scope)?;
                self.combine(!positive, a, b)
            }
            SymbolicBDD::BinaryOp(op @ (Iff | Xor), a, b) => {
                let positive = (*op == Iff) == positive;
                let forward = SymbolicBDD::binop(Implies, a.as_ref().clone(), b.as_ref().clone());
                let backward = SymbolicBDD::binop(Implies, b.as_ref().clone(), a.as_ref().clone());
                self.nnf(&SymbolicBDD::binop(And, forward, backward), positive, scope)?
            }
            SymbolicBDD::Ite(c, t, e) => {
                let then = SymbolicBDD::binop(Implies, c.as_ref().clone(), t.as_ref().clone());
                let otherwise = SymbolicBDD::binop(Or, c.as_ref().clone(), e.as_ref().clone());
                self.nnf(&SymbolicBDD::binop(And, then, otherwise), positive, scope)?
            }
            SymbolicBDD::Quantifier(
                q @ (QuantifierType::Forall | QuantifierType::Exists),
                vars,
                body,
            ) => {
                let Some((v, rest)) = vars.split_first() else {
                    return self.nnf(body, positive, scope);
                };
                let rest = SymbolicBDD::Quantifier(*q, rest.to_vec(), body.clone());
                let universal = (*q == QuantifierType::Forall) == positive;
                let mut scope = scope.clone();
                if !occurs_as_term(body, v) {
                    scope.booleans.insert(v.clone(), true);
                    let a = self.nnf(&rest, positive, &scope)?;
                    scope.booleans.insert(v.clone(), false);
                    let b = self.nnf(&rest, positive, &scope)?;
                    return Ok(self.combine(universal, a, b));
                }
                let symbol = self.symbol(if universal {
                    v.name.to_string()
                } else {
                    format!("sk{}", self.fresh + 1)
                });
                let value = if universal {
                    self.variables.insert(symbol.clone());
                    DomainConstant::Terminal(symbol)
                } else if scope.universals.is_empty() {
                    DomainConstant::Terminal(symbol)
                } else {
                    DomainConstant::Application(symbol, scope.universals.clone())
                };
                if universal {
                    scope.universals.push(value.clone());
                }
                scope.terms.insert(v.clone(), value);
                self.nnf(&rest, positive, &scope)?
            }
            other => {
                return Err(invalid(format!(
                    "{other} is not supported in a first-order proof"
                )));
            }
        })
    }

    /// The clauses of a conjunction or disjunction of clauses. Before distributing a disjunction over large
    /// conjunctions, the larger side is named by a new atom `def(..)` over its variables, which implies it
    /// (Plaisted & Greenbaum, 1986).
    fn combine(
        &mut self,
        conjunction: bool,
        a: Vec<Vec<Literal>>,
        b: Vec<Vec<Literal>>,
    ) -> Vec<Vec<Literal>> {
        if conjunction {
            return [a, b].concat();
        }
        let (mut a, mut b) = (a, b);
        if a.len() > 1 && b.len() > 1 && a.len() * b.len() > DISTRIBUTION_LIMIT {
            if a.len() < b.len() {
                std::mem::swap(&mut a, &mut b);
            }
            let vars: Vec<_> = a
                .iter()
                .flatten()
                .flat_map(|l| [&l.l, &l.r])
                .flat_map(|t| {
                    let mut subterms = Vec::new();
                    positions(t, &mut vec![], &mut subterms);
                    subterms
                        .into_iter()
                        .map(|(_, t)| t.clone())
                        .collect::<Vec<_>>()
                })
                .filter(|t| matches!(t, DomainConstant::Terminal(v) if self.variables.contains(v)))
                .unique()
                .collect();
            let symbol = self.symbol(format!("def{}", self.fresh + 1));
            let atom = if vars.is_empty() {
                DomainConstant::Terminal(symbol)
            } else {
                DomainConstant::Application(symbol, vars)
            };
            let negative = Literal {
                positive: false,
                l: atom.clone(),
                r: self.top.clone(),
            };
            for clause in a {
                self.definitions
                    .push([vec![negative.clone()], clause].concat());
            }
            a = vec![vec![Literal {
                positive: true,
                l: atom,
                r: self.top.clone(),
            }]];
        }
        a.iter()
            .cartesian_product(&b)
            .map(|(x, y)| [x.clone(), y.clone()].concat())
            .collect()
    }

    fn symbol(&mut self, name: String) -> NamedSymbol {
        self.fresh += 1;
        NamedSymbol {
            name: Rc::new(name),
            id: usize::MAX / 2 + self.fresh,
        }
    }

    /// Add a clause with fresh variables, returning its index
    fn add(&mut self, literals: Vec<Literal>, origin: &'static str, parents: Vec<usize>) -> usize {
        let mut theta = Substitution::default();
        let mut vars = Vec::new();
        let mut subterms = Vec::new();
        for literal in &literals {
            positions(&literal.l, &mut vec![], &mut subterms);
            positions(&literal.r, &mut vec![], &mut subterms);
        }
        for (_, t) in subterms {
            if let DomainConstant::Terminal(v) = t
                && self.variables.contains(v)
                && !theta.contains_key(v)
            {
                let fresh = self.symbol(v.name.to_string());
                self.variables.insert(fresh.clone());
                vars.push(fresh.clone());
                theta.insert(v.clone(), DomainConstant::Terminal(fresh));
            }
        }
        let literals = literals
            .iter()
            .filter(|l| l.positive || l.l != l.r)
            .map(|l| Literal {
                positive: l.positive,
                l: subst(&l.l, &theta),
                r: subst(&l.r, &theta),
            })
            .collect();
        self.clauses.push(Clause {
            literals,
            vars,
            origin,
            parents,
        });
        self.clauses.len() - 1
    }

    fn saturate(&mut self) -> Outcome {
        let mut by_weight = BinaryHeap::new();
        let mut by_age = BinaryHeap::new();
        let mut taken = FxHashSet::default();
        let mut passive: Vec<usize> = (0..self.clauses.len()).collect();
        let mut active: Vec<usize> = Vec::new();
        let mut demodulators: Vec<(Rule, usize)> = Vec::new();

        for step in 0..PROVER_STEPS {
            for c in std::mem::take(&mut passive) {
                by_weight.push(Reverse((self.weight(c), c)));
                by_age.push(Reverse(c));
            }
            let next = |taken: &FxHashSet<usize>,
                        by_weight: &mut BinaryHeap<_>,
                        by_age: &mut BinaryHeap<_>| loop {
                let c = if step % 5 == 0 {
                    by_age.pop().map(|Reverse(c)| c)
                } else {
                    by_weight.pop().map(|Reverse((_, c))| c)
                }?;
                if !taken.contains(&c) {
                    return Some(c);
                }
            };
            let Some(given) = next(&taken, &mut by_weight, &mut by_age) else {
                return Outcome::Disproven;
            };
            taken.insert(given);
            let Some(given) = self.simplify(given, &active, &demodulators) else {
                continue;
            };
            if self.clauses[given].literals.is_empty() {
                return Outcome::Proven(self.proof(given));
            }

            active.retain(|&a| !self.subsumes(given, a));
            if let Some(rule) = self.demodulator(given) {
                demodulators.push((rule, given));
            }
            active.push(given);

            let copy = self.rename(given);
            for &other in &active {
                let other = if other == given { copy } else { other };
                for (a, b) in [(given, other), (other, given)] {
                    for conclusion in self.superpose(a, b) {
                        passive.push(conclusion);
                    }
                }
            }
            for conclusion in self.resolve_and_factor(given) {
                passive.push(conclusion);
            }
            if let Some(&empty) = passive
                .iter()
                .find(|&&c| self.clauses[c].literals.is_empty())
            {
                return Outcome::Proven(self.proof(empty));
            }
        }
        Outcome::Unknown
    }

    fn weight(&self, c: usize) -> usize {
        self.clauses[c]
            .literals
            .iter()
            .map(|l| size(&l.l) + size(&l.r))
            .sum()
    }

    fn rename(&mut self, c: usize) -> usize {
        let literals = self.clauses[c].literals.clone();
        let origin = self.clauses[c].origin;
        let parents = self.clauses[c].parents.clone();
        self.add(literals, origin, parents)
    }

    /// Simplify a clause by demodulation, removing `s != s` and duplicate literals, and delete it if it is a tautology
    /// or subsumed by an active clause
    fn simplify(
        &mut self,
        c: usize,
        active: &[usize],
        demodulators: &[(Rule, usize)],
    ) -> Option<usize> {
        let mut literals: Vec<Literal> = Vec::new();
        let mut used = Vec::new();
        let mut changed = false;
        for literal in &self.clauses[c].literals {
            let l = demodulate(&literal.l, demodulators, &mut used)?;
            let r = demodulate(&literal.r, demodulators, &mut used)?;
            changed |= l != literal.l || r != literal.r;
            let simplified = Literal {
                positive: literal.positive,
                l,
                r,
            };
            if !simplified.positive && simplified.l == simplified.r {
                changed = true;
                continue;
            }
            if simplified.positive && simplified.l == simplified.r {
                return None;
            }
            if literals
                .iter()
                .any(|o| same_atom(o, &simplified) && o.positive != simplified.positive)
            {
                return None;
            }
            if literals
                .iter()
                .any(|o| same_atom(o, &simplified) && o.positive == simplified.positive)
            {
                changed = true;
                continue;
            }
            literals.push(simplified);
        }
        let c = if changed {
            self.add(literals, "simplification", [vec![c], used].concat())
        } else {
            c
        };
        (!active.iter().any(|&a| self.subsumes(a, c))).then_some(c)
    }

    /// An oriented unit equation of a clause as a rewrite rule
    fn demodulator(&self, c: usize) -> Option<Rule> {
        let clause = &self.clauses[c];
        let [literal] = clause.literals.as_slice() else {
            return None;
        };
        if !literal.positive {
            return None;
        }
        [(&literal.l, &literal.r), (&literal.r, &literal.l)]
            .into_iter()
            .find(|(l, r)| lpo(l, r, &clause.vars, &self.precedence))
            .map(|(l, r)| Rule {
                vars: clause.vars.clone(),
                lhs: l.clone(),
                rhs: SymbolicBDD::from_term(r),
            })
    }

    /// Whether an instance of the literals of clause `a` is contained in the literals of clause `b`, as multisets
    fn subsumes(&self, a: usize, b: usize) -> bool {
        fn extend(
            pattern: &[Literal],
            target: &[Literal],
            used: &mut Vec<usize>,
            vars: &[NamedSymbol],
            sigma: &Substitution,
        ) -> bool {
            let Some((first, rest)) = pattern.split_first() else {
                return true;
            };
            (0..target.len()).any(|k| {
                let t = &target[k];
                !used.contains(&k)
                    && t.positive == first.positive
                    && [(&first.l, &first.r), (&first.r, &first.l)]
                        .into_iter()
                        .any(|(l, r)| {
                            let mut sigma = sigma.clone();
                            used.push(k);
                            let found = match_pattern(l, &t.l, vars, &mut sigma)
                                && match_pattern(r, &t.r, vars, &mut sigma)
                                && extend(rest, target, used, vars, &sigma);
                            used.pop();
                            found
                        })
            })
        }
        let (a, b) = (&self.clauses[a], &self.clauses[b]);
        a.literals.len() <= b.literals.len()
            && extend(
                &a.literals,
                &b.literals,
                &mut Vec::new(),
                &a.vars,
                &Substitution::default(),
            )
    }

    /// Whether inferences may use a literal of a clause: the heaviest negative literal is selected if there is one,
    /// otherwise the literals that may be maximal (no other literal is greater)
    fn eligible(&self, c: usize, i: usize) -> bool {
        let clause = &self.clauses[c];
        let selected = clause
            .literals
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.positive)
            .max_by_key(|(k, l)| (size(&l.l) + size(&l.r), Reverse(*k)))
            .map(|(k, _)| k);
        if let Some(selected) = selected {
            return i == selected;
        }
        let greater = |a: &Literal, b: &Literal| {
            [&a.l, &a.r].into_iter().any(|s| {
                [&b.l, &b.r].into_iter().all(|t| {
                    lpo(s, t, &clause.vars, &self.precedence)
                        || (s == t && !a.positive && b.positive)
                })
            })
        };
        let literal = &clause.literals[i];
        !clause
            .literals
            .iter()
            .enumerate()
            .any(|(j, other)| j != i && greater(other, literal))
    }

    /// Superposition of a positive equation of clause `from` into a literal of clause `into`
    fn superpose(&mut self, from: usize, into: usize) -> Vec<usize> {
        let vars = [
            self.clauses[from].vars.clone(),
            self.clauses[into].vars.clone(),
        ]
        .concat();
        let greater = |s: &DomainConstant, t: &DomainConstant| lpo(s, t, &vars, &self.precedence);
        let mut conclusions = Vec::new();
        for (i, equation) in self.clauses[from].literals.iter().enumerate() {
            if !equation.positive || !self.eligible(from, i) {
                continue;
            }
            for (s, t) in [(&equation.l, &equation.r), (&equation.r, &equation.l)] {
                if greater(t, s) || *s == self.top {
                    continue;
                }
                for (j, target) in self.clauses[into].literals.iter().enumerate() {
                    if !self.eligible(into, j) {
                        continue;
                    }
                    for (w, other, left) in
                        [(&target.l, &target.r, true), (&target.r, &target.l, false)]
                    {
                        if greater(other, w) {
                            continue;
                        }
                        let mut subterms = Vec::new();
                        positions(w, &mut vec![], &mut subterms);
                        for (path, u) in subterms {
                            let predicate = path.is_empty() && *other == self.top;
                            if matches!(u, DomainConstant::Terminal(v) if vars.contains(v))
                                || *u == self.top
                                || (predicate
                                    && matches!(s, DomainConstant::Terminal(v) if vars.contains(v)))
                            {
                                continue;
                            }
                            let mut sigma = Substitution::default();
                            if !unify(s, u, &vars, &mut sigma)
                                || greater(&resolve(t, &sigma), &resolve(s, &sigma))
                                || greater(&resolve(other, &sigma), &resolve(w, &sigma))
                            {
                                continue;
                            }
                            let rewritten = replace(w, &path, t);
                            let (l, r) = if left {
                                (rewritten, other.clone())
                            } else {
                                (other.clone(), rewritten)
                            };
                            let mut literals: Vec<Literal> = self.clauses[from]
                                .literals
                                .iter()
                                .enumerate()
                                .filter(|(k, _)| *k != i)
                                .chain(
                                    self.clauses[into]
                                        .literals
                                        .iter()
                                        .enumerate()
                                        .filter(|(k, _)| *k != j),
                                )
                                .map(|(_, l)| l.clone())
                                .collect();
                            literals.push(Literal {
                                positive: target.positive,
                                l,
                                r,
                            });
                            conclusions.push((instantiate(&literals, &sigma), [from, into]));
                        }
                    }
                }
            }
        }
        conclusions
            .into_iter()
            .map(|(literals, parents)| self.add(literals, "superposition", parents.to_vec()))
            .collect()
    }

    /// Equality resolution and equality factoring of a clause
    fn resolve_and_factor(&mut self, c: usize) -> Vec<usize> {
        let clause = &self.clauses[c];
        let vars = &clause.vars;
        let without = |skip: usize| -> Vec<Literal> {
            clause
                .literals
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != skip)
                .map(|(_, l)| l.clone())
                .collect()
        };
        let mut conclusions = Vec::new();
        for (i, a) in clause.literals.iter().enumerate() {
            if !self.eligible(c, i) {
                continue;
            }
            let mut sigma = Substitution::default();
            if !a.positive && unify(&a.l, &a.r, vars, &mut sigma) {
                conclusions.push((instantiate(&without(i), &sigma), "equality resolution"));
            }
            if !a.positive {
                continue;
            }
            for (j, b) in clause.literals.iter().enumerate() {
                if j == i || !b.positive {
                    continue;
                }
                for (s, t) in [(&a.l, &a.r), (&a.r, &a.l)] {
                    for (s2, t2) in [(&b.l, &b.r), (&b.r, &b.l)] {
                        let mut sigma = Substitution::default();
                        if unify(s, s2, vars, &mut sigma)
                            && !lpo(
                                &resolve(t, &sigma),
                                &resolve(s, &sigma),
                                vars,
                                &self.precedence,
                            )
                        {
                            let mut literals = without(i);
                            literals.push(Literal {
                                positive: false,
                                l: t.clone(),
                                r: t2.clone(),
                            });
                            conclusions
                                .push((instantiate(&literals, &sigma), "equality factoring"));
                        }
                    }
                }
            }
        }
        conclusions
            .into_iter()
            .map(|(literals, origin)| self.add(literals, origin, vec![c]))
            .collect()
    }

    /// The derivation of a clause, one line per clause
    fn proof(&self, c: usize) -> Vec<String> {
        let mut used = FxHashSet::default();
        let mut stack = vec![c];
        while let Some(c) = stack.pop() {
            if used.insert(c) {
                stack.extend(&self.clauses[c].parents);
            }
        }
        used.into_iter()
            .sorted()
            .map(|c| {
                let clause = &self.clauses[c];
                let parents = if clause.parents.is_empty() {
                    String::new()
                } else {
                    format!(" {}", clause.parents.iter().join(", "))
                };
                format!("{c}. {} [{}{parents}]", self.display(clause), clause.origin)
            })
            .collect()
    }

    /// A clause in rsbdd syntax, with distinct names for its variables
    fn display(&self, clause: &Clause) -> String {
        if clause.literals.is_empty() {
            return "false".to_string();
        }
        let mut names = FxHashSet::default();
        let theta: Substitution = clause
            .vars
            .iter()
            .map(|v| {
                let name = (0..)
                    .map(|i| {
                        if i == 0 {
                            v.name.to_string()
                        } else {
                            format!("{}{i}", v.name)
                        }
                    })
                    .find(|n| names.insert(n.clone()))
                    .expect("unbounded names");
                let renamed = NamedSymbol {
                    name: Rc::new(name),
                    id: v.id,
                };
                (v.clone(), DomainConstant::Terminal(renamed))
            })
            .collect();
        clause
            .literals
            .iter()
            .map(|literal| {
                let sign = if literal.positive { "" } else { "-" };
                let l = subst(&literal.l, &theta);
                if literal.r == self.top {
                    format!("{sign}{l}")
                } else {
                    format!("{sign}({l} = {})", subst(&literal.r, &theta))
                }
            })
            .join(" | ")
    }
}

/// Rewrite a term to normal form with the demodulators, collecting the clauses of the demodulators that were used
fn demodulate(
    t: &DomainConstant,
    demodulators: &[(Rule, usize)],
    used: &mut Vec<usize>,
) -> Option<DomainConstant> {
    let mut t = t.clone();
    'rewrite: loop {
        for (rule, clause) in demodulators {
            let s = normalize(&t, std::slice::from_ref(rule)).ok()?;
            if s != t {
                t = s;
                if !used.contains(clause) {
                    used.push(*clause);
                }
                continue 'rewrite;
            }
        }
        return Some(t);
    }
}

fn same_atom(a: &Literal, b: &Literal) -> bool {
    (a.l == b.l && a.r == b.r) || (a.l == b.r && a.r == b.l)
}

fn instantiate(literals: &[Literal], sigma: &Substitution) -> Vec<Literal> {
    literals
        .iter()
        .map(|l| Literal {
            positive: l.positive,
            l: resolve(&l.l, sigma),
            r: resolve(&l.r, sigma),
        })
        .collect()
}
