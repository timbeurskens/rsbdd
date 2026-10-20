use std::io;
use std::rc::Rc;

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::NamedSymbol;
use crate::parser::{
    BinaryOperator, DomainConstant, ParsedFormula, QuantifierType, ReferenceContents, SymbolicBDD,
};

pub(crate) type Substitution = FxHashMap<NamedSymbol, DomainConstant>;
type Equation = (Vec<NamedSymbol>, DomainConstant, DomainConstant);

const COMPLETION_STEPS: usize = 1000;
const LEMMA_DEPTH: usize = 2;

#[derive(Clone)]
pub(crate) struct Rule {
    pub(crate) vars: Vec<NamedSymbol>,
    pub(crate) lhs: DomainConstant,
    pub(crate) rhs: SymbolicBDD,
}

/// Ground terms in order of first occurrence
#[derive(Default)]
pub(crate) struct Universe {
    terms: Vec<DomainConstant>,
    seen: FxHashSet<DomainConstant>,
    pub(crate) functions: FxHashSet<NamedSymbol>,
    pub(crate) symbols: FxHashSet<(NamedSymbol, usize)>,
}

impl Universe {
    /// A rule for a function symbol is a term rule, whose right-hand side is a term
    pub(crate) fn is_term_rule(&self, rule: &Rule) -> bool {
        matches!(&rule.lhs, DomainConstant::Application(f, _) if self.functions.contains(f))
    }
}

/// The rewrite rules and remaining formula of a parsed formula, with the ground terms and function symbols in them
pub(crate) fn split_rules(
    parsed: &ParsedFormula,
) -> io::Result<(Vec<Rule>, SymbolicBDD, Universe)> {
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

    // The right-hand side of a term rule is a term, so its head is a function symbol as well
    loop {
        let known = universe.functions.len();
        for rule in &rules {
            if let SymbolicBDD::RuleApplication(t) = &rule.rhs
                && universe.is_term_rule(rule)
            {
                add_element(t, &rule.vars, &mut universe);
            }
        }
        if universe.functions.len() == known {
            break;
        }
    }
    Ok((rules, formula, universe))
}

/// Grounds rewrite rules into a plain formula.
///
/// An application of a predicate with rules is replaced by the conjunction of the right-hand sides of all
/// matching rules, or `false` if no rule matches. An application of a predicate without rules becomes a
/// boolean variable, and an application that depends on itself is the least fixpoint of its rules. Quantifiers over
/// variables used as terms range over all ground terms in the formula and an arbitrary constant for every variable.
/// A quantifier over an atom pattern such as `p(A, _)` ranges over the boolean variables of the atoms in its body
/// that match the pattern, where `_` matches any term.
///
/// A rule for a function symbol (a symbol used inside a term or as an operand of `=`) is a term rule: its right-hand
/// side is a term. Terms are rewritten to normal form before they are compared, matched or turned into variables,
/// so `=` holds modulo the term rules. The term rules are completed into a convergent rewrite system by Knuth-Bendix
/// completion if possible, in which case this decides equality in the equational theory of the rules.
///
/// `induct x, .. # l = r` proves an equation by induction on `x`, using the patterns of the rules defining a function
/// applied to `x` as cases and the induction hypotheses as rewrite rules. A case that does not rewrite to an identity
/// is generalised into a lemma and proven by induction first. It is `true` if every case is proven and `false`
/// otherwise. A proven equation becomes a rewrite rule for the remainder of the formula.
pub struct Rewriter {
    rules: Rc<[Rule]>,
    term_rules: Vec<Rule>,
    definitions: usize,
    skolems: usize,
    universe: Vec<DomainConstant>,
    arbitrary: FxHashMap<NamedSymbol, DomainConstant>,
    recursive: FxHashMap<DomainConstant, NamedSymbol>,
    atoms: FxHashMap<DomainConstant, NamedSymbol>,
    // the atom of every variable introduced by the rewriter, indexed by id - base_id
    grounds: Vec<DomainConstant>,
    base_id: usize,
    expanding: FxHashSet<DomainConstant>,
}

impl Rewriter {
    /// Rewrite the parsed formula, returning the ground formula and the variables introduced for open atoms
    pub fn rewrite(parsed: &ParsedFormula) -> io::Result<(SymbolicBDD, Vec<NamedSymbol>)> {
        let (mut rewriter, formula) = Self::new(parsed)?;
        let result = rewriter.expand(&formula, &Substitution::default())?;

        let mut new_vars: Vec<_> = rewriter.atoms.into_values().collect();
        new_vars.sort_by_key(|v| v.id);
        Ok((result, new_vars))
    }

    /// A rewriter for the rules of a parsed formula, with the remaining formula
    pub(crate) fn new(parsed: &ParsedFormula) -> io::Result<(Self, SymbolicBDD)> {
        let (rules, formula, mut universe) = split_rules(parsed)?;

        let (term_rules, rules): (Vec<_>, Vec<_>) = rules
            .into_iter()
            .partition(|rule| universe.is_term_rule(rule));
        let term_rules = complete(&term_rules).unwrap_or(term_rules);
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

        let rewriter = Self {
            rules: rules.into(),
            definitions: term_rules.len(),
            term_rules,
            skolems: 0,
            universe: terms,
            arbitrary: FxHashMap::default(),
            recursive: FxHashMap::default(),
            atoms: FxHashMap::default(),
            grounds: Vec::new(),
            base_id: parsed.vars.iter().map(|v| v.id + 1).max().unwrap_or(0),
            expanding: FxHashSet::default(),
        };
        Ok((rewriter, formula))
    }

    /// The atom of a variable introduced by the rewriter
    pub(crate) fn atom_of(&self, v: &NamedSymbol) -> Option<&DomainConstant> {
        v.id.checked_sub(self.base_id)
            .and_then(|i| self.grounds.get(i))
    }

    /// Whether a predicate rule is defined for the functor of a term
    pub(crate) fn defines(&self, t: &DomainConstant) -> bool {
        self.rules.iter().any(|rule| same_functor(&rule.lhs, t))
    }

    pub(crate) fn expand(
        &mut self,
        formula: &SymbolicBDD,
        theta: &Substitution,
    ) -> io::Result<SymbolicBDD> {
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
                if !self.induct(vars, &l, &r, LEMMA_DEPTH)? {
                    return Ok(SymbolicBDD::False);
                }
                self.term_rules.extend(orient(&l, &r, vars));
                SymbolicBDD::True
            }
            SymbolicBDD::Quantifier(q, vars, body) => {
                let (domain, boolean): (Vec<_>, Vec<_>) =
                    vars.iter().cloned().partition(|v| occurs_as_term(body, v));
                let expanded = self.expand_domain(*q, &domain, body, &mut theta.clone())?;
                quantify(*q, boolean, expanded)
            }
            SymbolicBDD::AtomQuantifier(q, patterns, body) => {
                let body = self.expand(body, theta)?;
                let patterns: Vec<_> = patterns
                    .iter()
                    .map(|p| normalize(&subst(p, theta), &self.term_rules))
                    .collect::<io::Result<_>>()?;
                let mut vars = Vec::new();
                self.matching_atoms(&body, &patterns, &mut vars);
                vars.sort_by_key(|v| v.id);
                vars.dedup();
                quantify(*q, vars, body)
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

        let open = v.name.as_str() != "_";
        if open {
            if !self.arbitrary.contains_key(v) {
                let constant = DomainConstant::Terminal(self.skolem(v));
                self.arbitrary.insert(v.clone(), constant);
            }
            self.universe.push(self.arbitrary[v].clone());
        }

        let mut instances = Vec::with_capacity(self.universe.len());
        for i in 0..self.universe.len() {
            let value = self.universe[i].clone();
            let previous = theta.insert(v.clone(), value);
            let instance = self.expand_domain(q, rest, body, theta);
            match previous {
                Some(previous) => theta.insert(v.clone(), previous),
                None => theta.remove(v),
            };
            instances.push(instance?);
        }
        if open {
            self.universe.pop();
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
            if !self.recursive.contains_key(&ground) {
                let x = self.skolem(&ground);
                self.recursive.insert(ground.clone(), x);
            }
            return Ok(SymbolicBDD::Var(self.recursive[&ground].clone()));
        }

        let mut bodies = Vec::new();
        for rule in Rc::clone(&self.rules).iter() {
            let mut sigma = Substitution::default();
            if match_pattern(&rule.lhs, &ground, &rule.vars, &mut sigma) {
                bodies.push(self.expand(&rule.rhs, &sigma)?);
            }
        }

        self.expanding.remove(&ground);

        let body = if bodies.is_empty() {
            SymbolicBDD::False
        } else {
            join(BinaryOperator::And, bodies)
        };
        Ok(match self.recursive.remove(&ground) {
            Some(x) => SymbolicBDD::FixedPoint(x, false, Box::new(body)),
            None => body,
        })
    }

    /// Prove `l = r` for all values of the first variable, with a case for every pattern of the rules defining a function
    /// applied to it, and an induction hypothesis for every variable of the pattern
    fn induct(
        &mut self,
        vars: &[NamedSymbol],
        l: &DomainConstant,
        r: &DomainConstant,
        depth: usize,
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

            let (cl, cr) = self.normalize_with(&hypotheses, &at(l, &case), &at(r, &case))?;
            if cl == cr {
                continue;
            }
            let generalised: Vec<_> = skolems
                .values()
                .filter_map(|s| match s {
                    DomainConstant::Terminal(v) => Some(v),
                    _ => None,
                })
                .chain(rest)
                .filter(|v| contains(&cl, v) || contains(&cr, v))
                .cloned()
                .collect();
            if depth == 0 || !self.speculate(&generalised, &cl, &cr, depth - 1)? {
                return Ok(false);
            }
            let (cl, cr) = self.normalize_with(&hypotheses, &cl, &cr)?;
            if cl != cr {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn normalize_with(
        &mut self,
        hypotheses: &[Rule],
        l: &DomainConstant,
        r: &DomainConstant,
    ) -> io::Result<(DomainConstant, DomainConstant)> {
        let n = self.term_rules.len();
        self.term_rules.extend_from_slice(hypotheses);
        let normal =
            normalize(l, &self.term_rules).and_then(|l| Ok((l, normalize(r, &self.term_rules)?)));
        self.term_rules.truncate(n);
        normal
    }

    /// Prove `l = r` as a lemma by induction on any of the variables, and add it as a rewrite rule
    fn speculate(
        &mut self,
        vars: &[NamedSymbol],
        l: &DomainConstant,
        r: &DomainConstant,
        depth: usize,
    ) -> io::Result<bool> {
        let Some(lemma) = orient(l, r, vars) else {
            return Ok(false);
        };
        for i in 0..vars.len() {
            let mut order = vars.to_vec();
            order.rotate_left(i);
            if self.induct(&order, l, r, depth)? {
                self.term_rules.push(lemma);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn skolem(&mut self, v: &impl std::fmt::Display) -> NamedSymbol {
        self.skolems += 1;
        NamedSymbol {
            name: Rc::new(format!("{v}'")),
            id: usize::MAX - self.skolems,
        }
    }

    fn atom(&mut self, ground: DomainConstant) -> NamedSymbol {
        if let Some(v) = self.atoms.get(&ground) {
            return v.clone();
        }
        let v = NamedSymbol {
            name: Rc::new(ground.to_string()),
            id: self.base_id + self.grounds.len(),
        };
        self.grounds.push(ground.clone());
        self.atoms.insert(ground, v.clone());
        v
    }

    /// Collect the variables in a formula of the atoms matching one of the patterns
    fn matching_atoms(
        &self,
        formula: &SymbolicBDD,
        patterns: &[DomainConstant],
        vars: &mut Vec<NamedSymbol>,
    ) {
        match formula {
            SymbolicBDD::Var(v) => {
                let atom =
                    v.id.checked_sub(self.base_id)
                        .and_then(|i| self.grounds.get(i));
                if atom.is_some_and(|atom| {
                    patterns
                        .iter()
                        .any(|p| match_pattern(p, atom, &[], &mut Substitution::default()))
                }) {
                    vars.push(v.clone());
                }
            }
            other => {
                for c in other.children() {
                    self.matching_atoms(c, patterns, vars);
                }
            }
        }
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
        SymbolicBDD::AtomQuantifier(q, atoms, f) => {
            SymbolicBDD::AtomQuantifier(*q, atoms.clone(), boxed(f))
        }
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

pub(crate) fn occurs_as_term(formula: &SymbolicBDD, v: &NamedSymbol) -> bool {
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

/// `q vars # body`, or `body` if there are no variables
fn quantify(q: QuantifierType, vars: Vec<NamedSymbol>, body: SymbolicBDD) -> SymbolicBDD {
    if vars.is_empty() {
        body
    } else {
        SymbolicBDD::Quantifier(q, vars, Box::new(body))
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(crate) fn join(op: BinaryOperator, items: Vec<SymbolicBDD>) -> SymbolicBDD {
    let unit = match op {
        BinaryOperator::Or => SymbolicBDD::False,
        _ => SymbolicBDD::True,
    };
    items
        .into_iter()
        .fold(unit, |acc, item| SymbolicBDD::binop(op, acc, item))
}

pub(crate) fn term(operand: &SymbolicBDD, theta: &Substitution) -> io::Result<DomainConstant> {
    match operand {
        SymbolicBDD::Var(v) => Ok(subst(&DomainConstant::Terminal(v.clone()), theta)),
        SymbolicBDD::RuleApplication(t) => Ok(subst(t, theta)),
        other => Err(invalid(format!("expected a term, got {other:?}"))),
    }
}

pub(crate) fn subst(t: &DomainConstant, theta: &Substitution) -> DomainConstant {
    match t {
        DomainConstant::Terminal(v) => theta.get(v).cloned().unwrap_or_else(|| t.clone()),
        DomainConstant::Application(f, args) => {
            DomainConstant::Application(f.clone(), args.iter().map(|a| subst(a, theta)).collect())
        }
    }
}

pub(crate) fn contains(t: &DomainConstant, v: &NamedSymbol) -> bool {
    match t {
        DomainConstant::Terminal(x) => x == v,
        DomainConstant::Application(_, args) => args.iter().any(|a| contains(a, v)),
    }
}

/// Rewrite a term to normal form, innermost first
pub(crate) fn normalize(t: &DomainConstant, rules: &[Rule]) -> io::Result<DomainConstant> {
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
        .map(|(lhs, rhs)| term_rule(vars.to_vec(), lhs.clone(), rhs))
}

fn term_rule(vars: Vec<NamedSymbol>, lhs: DomainConstant, rhs: &DomainConstant) -> Rule {
    Rule {
        vars,
        lhs,
        rhs: SymbolicBDD::from_term(rhs),
    }
}

pub(crate) fn same_functor(a: &DomainConstant, b: &DomainConstant) -> bool {
    matches!((a, b), (DomainConstant::Application(f, xs), DomainConstant::Application(g, ys)) if f == g && xs.len() == ys.len())
}

pub(crate) fn match_pattern(
    pattern: &DomainConstant,
    ground: &DomainConstant,
    vars: &[NamedSymbol],
    sigma: &mut Substitution,
) -> bool {
    match (pattern, ground) {
        (DomainConstant::Terminal(v), _) if v.name.as_str() == "_" => true,
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
    match t {
        DomainConstant::Application(f, args) => {
            universe.functions.insert(f.clone());
            universe.symbols.insert((f.clone(), args.len()));
        }
        DomainConstant::Terminal(c) if !bound.contains(c) => {
            universe.symbols.insert((c.clone(), 0));
        }
        DomainConstant::Terminal(_) => {}
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

/// Complete term rules into a convergent rewrite system by Knuth-Bendix completion (Knuth & Bendix, 1970). The given
/// rules keep their direction, derived equations are oriented by the lexicographic path order.
pub(crate) fn complete(rules: &[Rule]) -> Option<Vec<Rule>> {
    let precedence = precedence(rules);
    let mut fresh = 0;
    let mut pending = Vec::new();
    for (i, rule) in rules.iter().enumerate() {
        pending.extend(overlaps(rule, &rules[..i], &mut fresh));
    }
    let mut complete = rules.to_vec();

    for _ in 0..COMPLETION_STEPS {
        let Some(i) = pending
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, l, r))| size(l) + size(r))
            .map(|(i, _)| i)
        else {
            return Some(complete);
        };
        let (vars, s, t) = pending.swap_remove(i);
        let s = normalize(&s, &complete).ok()?;
        let t = normalize(&t, &complete).ok()?;
        if s == t {
            continue;
        }
        let (lhs, rhs) = [(&s, &t), (&t, &s)]
            .into_iter()
            .find(|(l, r)| lpo(l, r, &vars, &precedence))?;
        let vars = vars.into_iter().filter(|v| contains(lhs, v)).collect();
        let rule = term_rule(vars, lhs.clone(), rhs);

        let (collapsed, kept): (Vec<_>, Vec<_>) = complete.into_iter().partition(|old| {
            normalize(&old.lhs, std::slice::from_ref(&rule)).is_ok_and(|t| t != old.lhs)
        });
        for old in collapsed {
            let rhs = term(&old.rhs, &Substitution::default()).ok()?;
            pending.push((old.vars, old.lhs, rhs));
        }
        complete = kept;
        complete.push(rule);
        for k in 0..complete.len() {
            let rhs = term(&complete[k].rhs, &Substitution::default()).ok()?;
            complete[k].rhs = SymbolicBDD::from_term(&normalize(&rhs, &complete).ok()?);
        }
        let (new, others) = complete.split_last()?;
        pending.extend(overlaps(new, others, &mut fresh));
    }

    None
}

/// The critical pairs of a rule with itself and with each of the other rules, in both directions
fn overlaps(new: &Rule, others: &[Rule], fresh: &mut usize) -> Vec<Equation> {
    let mut pairs = critical_pairs(new, new, true, fresh);
    for old in others {
        pairs.extend(critical_pairs(new, old, false, fresh));
        pairs.extend(critical_pairs(old, new, false, fresh));
    }
    pairs
}

/// The equations obtained by unifying the left-hand side of `inner` with a non-variable subterm of the left-hand side
/// of `outer`
fn critical_pairs(outer: &Rule, inner: &Rule, same: bool, fresh: &mut usize) -> Vec<Equation> {
    let (outer_vars, l1, r1) = rename(outer, fresh);
    let (inner_vars, l2, r2) = rename(inner, fresh);
    let vars = [outer_vars, inner_vars].concat();
    let mut subterms = Vec::new();
    positions(&l1, &mut Vec::new(), &mut subterms);

    let mut pairs = Vec::new();
    for (path, subterm) in subterms {
        if matches!(subterm, DomainConstant::Terminal(v) if vars.contains(v))
            || (same && path.is_empty())
        {
            continue;
        }
        let mut sigma = Substitution::default();
        if unify(subterm, &l2, &vars, &mut sigma) {
            let rhs = resolve(&replace(&l1, &path, &r2), &sigma);
            pairs.push((vars.clone(), resolve(&r1, &sigma), rhs));
        }
    }
    pairs
}

fn rename(rule: &Rule, fresh: &mut usize) -> Equation {
    let vars: Vec<_> = rule
        .vars
        .iter()
        .map(|v| {
            *fresh += 1;
            NamedSymbol {
                name: v.name.clone(),
                id: usize::MAX / 2 + *fresh,
            }
        })
        .collect();
    let theta: Substitution = rule
        .vars
        .iter()
        .cloned()
        .zip(vars.iter().cloned().map(DomainConstant::Terminal))
        .collect();
    let rhs = term(&rule.rhs, &theta).expect("the right-hand side of a term rule is a term");
    (vars, subst(&rule.lhs, &theta), rhs)
}

pub(crate) fn positions<'t>(
    t: &'t DomainConstant,
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, &'t DomainConstant)>,
) {
    out.push((path.clone(), t));
    if let DomainConstant::Application(_, args) = t {
        for (i, a) in args.iter().enumerate() {
            path.push(i);
            positions(a, path, out);
            path.pop();
        }
    }
}

pub(crate) fn replace(t: &DomainConstant, path: &[usize], s: &DomainConstant) -> DomainConstant {
    match (t, path.split_first()) {
        (DomainConstant::Application(f, args), Some((&i, rest))) => {
            let mut args = args.clone();
            args[i] = replace(&args[i], rest, s);
            DomainConstant::Application(f.clone(), args)
        }
        _ => s.clone(),
    }
}

pub(crate) fn unify(
    a: &DomainConstant,
    b: &DomainConstant,
    vars: &[NamedSymbol],
    sigma: &mut Substitution,
) -> bool {
    let (a, b) = (resolve(a, sigma), resolve(b, sigma));
    match (&a, &b) {
        _ if a == b => true,
        (DomainConstant::Terminal(v), t) | (t, DomainConstant::Terminal(v))
            if vars.contains(v) && !contains(t, v) =>
        {
            sigma.insert(v.clone(), t.clone());
            true
        }
        (DomainConstant::Application(_, xs), DomainConstant::Application(_, ys)) => {
            same_functor(&a, &b) && xs.iter().zip(ys).all(|(x, y)| unify(x, y, vars, sigma))
        }
        _ => false,
    }
}

pub(crate) fn resolve(t: &DomainConstant, sigma: &Substitution) -> DomainConstant {
    match t {
        DomainConstant::Terminal(v) => sigma
            .get(v)
            .map_or_else(|| t.clone(), |u| resolve(u, sigma)),
        DomainConstant::Application(f, args) => {
            DomainConstant::Application(f.clone(), args.iter().map(|a| resolve(a, sigma)).collect())
        }
    }
}

pub(crate) fn size(t: &DomainConstant) -> usize {
    match t {
        DomainConstant::Terminal(_) => 1,
        DomainConstant::Application(_, args) => 1 + args.iter().map(size).sum::<usize>(),
    }
}

/// The precedence of the symbols of the rules, following the default of Prover9: constants are the smallest, followed
/// by symbols of increasing arity, with unary symbols the largest. Symbols of the same arity are ordered by their first
/// occurrence.
fn precedence(rules: &[Rule]) -> FxHashMap<NamedSymbol, (usize, usize)> {
    let mut ranks = FxHashMap::default();
    for rule in rules {
        rank(&rule.lhs, &rule.vars, &mut ranks);
        if let Ok(rhs) = term(&rule.rhs, &Substitution::default()) {
            rank(&rhs, &rule.vars, &mut ranks);
        }
    }
    ranks
}

/// Add the symbols of a term to a precedence, see [precedence]
pub(crate) fn rank(
    t: &DomainConstant,
    vars: &[NamedSymbol],
    ranks: &mut FxHashMap<NamedSymbol, (usize, usize)>,
) {
    let n = ranks.len();
    if let Some((f, args)) = head(t, vars) {
        let arity = if args.len() == 1 {
            usize::MAX
        } else {
            args.len()
        };
        ranks.entry(f.clone()).or_insert((arity, n));
        for a in args {
            rank(a, vars, ranks);
        }
    }
}

/// The lexicographic path order: `s > t`
pub(crate) fn lpo(
    s: &DomainConstant,
    t: &DomainConstant,
    vars: &[NamedSymbol],
    precedence: &FxHashMap<NamedSymbol, (usize, usize)>,
) -> bool {
    let Some((f, ss)) = head(s, vars) else {
        return false;
    };
    let Some((g, ts)) = head(t, vars) else {
        return matches!(t, DomainConstant::Terminal(v) if s != t && contains(s, v));
    };
    let rank = |f: &NamedSymbol| precedence.get(f).copied().unwrap_or_default();

    ss.iter().any(|si| si == t || lpo(si, t, vars, precedence))
        || ts.iter().all(|tj| lpo(s, tj, vars, precedence))
            && match rank(f).cmp(&rank(g)) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal if f == g => ss
                    .iter()
                    .zip(ts)
                    .find(|(si, ti)| si != ti)
                    .is_some_and(|(si, ti)| lpo(si, ti, vars, precedence)),
                _ => false,
            }
}

/// The function symbol and arguments of a term, with a constant as a function without arguments
fn head<'t>(
    t: &'t DomainConstant,
    vars: &[NamedSymbol],
) -> Option<(&'t NamedSymbol, &'t [DomainConstant])> {
    match t {
        DomainConstant::Terminal(v) if vars.contains(v) => None,
        DomainConstant::Terminal(c) => Some((c, &[])),
        DomainConstant::Application(f, args) => Some((f, args)),
    }
}
