use std::fmt;
use std::rc::Rc;

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::NamedSymbol;
use crate::bdd::{BDD, BDDEnv, Node};
use crate::parser::DomainConstant;
use crate::prover::{Clause, Literal, made_by_prover};

/// The largest domain searched for a model
const MODEL_SIZE: usize = 16;
/// The number of BDD nodes allocated for a domain after which the search stops, as larger domains need more
const NODE_BUDGET: usize = 1_000_000;
/// The number of decisions in a domain after which the search stops
const SEARCH_BUDGET: usize = 100_000;

/// A symbol applied to the indices of elements of the domain
pub type Application = (NamedSymbol, Vec<usize>);

/// A finite model over the domain `e1, .., en`
///
/// The ground atoms that are true or false and the values of the functions of the formula, including Skolem functions.
/// Definitions introduced by the prover are left out.
pub struct Model {
    pub size: usize,
    pub atoms: Vec<(Application, bool)>,
    pub values: Vec<(Application, usize)>,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain: {}", (0..self.size).map(element).join(", "))?;
        for (atom, value) in &self.atoms {
            write!(f, "\n{}{}", if *value { "" } else { "-" }, show(atom))?;
        }
        for (application, value) in &self.values {
            write!(f, "\n{} = {}", show(application), element(*value))?;
        }
        Ok(())
    }
}

/// A model of clauses over the smallest domain that has one, searched up to `MODEL_SIZE` elements or until a domain
/// needs more than `NODE_BUDGET` BDD nodes or `SEARCH_BUDGET` decisions
pub(crate) fn find_model(clauses: &[Clause], top: &DomainConstant) -> Option<Model> {
    for n in 1..=MODEL_SIZE {
        let mut search = Search::new(n, top);
        if let Some(model) = search.model(clauses) {
            return Some(model);
        }
        search.env.gc();
        if search.env.size() > NODE_BUDGET || search.decisions > SEARCH_BUDGET {
            break;
        }
    }
    None
}

enum Variable {
    Atom(Application),
    Value(Application, usize),
}

impl Variable {
    fn args(&self) -> &[usize] {
        let (Self::Atom((_, args)) | Self::Value((_, args), _)) = self;
        args
    }
}

/// A search for a model over the domain `e1, .., en`, as in Mace4 (McCune, 2003). Every ground atom `p(e1, ..)` and
/// every value `f(e1, ..) = e2` of a function is a variable of the BDD, and a function has exactly one value for every
/// argument. A term is evaluated to the conditions under which it denotes each element. Every ground instance of a
/// clause is a constraint, and the values of the variables are searched depth-first with unit propagation.
struct Search<'a> {
    n: usize,
    top: &'a DomainConstant,
    env: BDDEnv<usize>,
    variables: Vec<Variable>,
    atoms: FxHashMap<Application, usize>,
    // the first of the `n` variables of the values of every application of a function to elements
    values: FxHashMap<Application, usize>,
    constraints: Vec<BDD<usize>>,
    // the constraints in which every variable occurs
    occurs: Vec<Vec<usize>>,
    decisions: usize,
}

impl<'a> Search<'a> {
    fn new(n: usize, top: &'a DomainConstant) -> Self {
        let env = BDDEnv::new();
        Self {
            n,
            top,
            env,
            variables: Vec::new(),
            atoms: FxHashMap::default(),
            values: FxHashMap::default(),
            constraints: Vec::new(),
            occurs: Vec::new(),
            decisions: 0,
        }
    }

    fn model(&mut self, clauses: &[Clause]) -> Option<Model> {
        for clause in clauses {
            for tuple in itertools::repeat_n(0..self.n, clause.vars.len()).multi_cartesian_product()
            {
                let mut vars = clause.vars.clone();
                let mut tuple = tuple;
                let assumed = self.env.mk_const(false);
                if !self.flatten(&clause.literals, &mut vars, &mut tuple, assumed) {
                    return None;
                }
            }
        }

        let constraints = std::mem::take(&mut self.constraints);
        let mut units = Vec::new();
        self.occurs = vec![Vec::new(); self.variables.len()];
        for (i, c) in constraints.iter().enumerate() {
            units.extend(unit(c));
            let mut vars = Vec::new();
            support(c, &mut FxHashSet::default(), &mut vars);
            for v in vars {
                self.occurs[v].push(i);
            }
        }
        let values = self.solve(constraints, vec![None; self.variables.len()], units, None)?;
        let mut model = Model {
            size: self.n,
            atoms: Vec::new(),
            values: Vec::new(),
        };
        for (variable, value) in self.variables.iter().zip(values) {
            match (variable, value) {
                (Variable::Atom(atom), Some(value)) if !hidden(&atom.0) => {
                    model.atoms.push((atom.clone(), value));
                }
                (Variable::Value(application, i), Some(true)) if !hidden(&application.0) => {
                    model.values.push((application.clone(), *i));
                }
                _ => {}
            }
        }
        Some(model)
    }

    /// Values of the variables that satisfy the constraints after propagating units, given the largest element `mdn`
    /// mentioned by a decision. A function takes at most one of the elements that no decision mentions, as these are
    /// interchangeable: the least number heuristic of Mace4. As in Mace4, the constants are chosen first, and then the
    /// applications to the smallest elements.
    fn solve(
        &mut self,
        mut constraints: Vec<BDD<usize>>,
        mut values: Vec<Option<bool>>,
        mut units: Vec<(usize, bool)>,
        mdn: Option<usize>,
    ) -> Option<Vec<Option<bool>>> {
        while let Some((v, value)) = units.pop() {
            match values[v] {
                Some(b) if b == value => continue,
                Some(_) => return None,
                None => values[v] = Some(value),
            }
            let var = self.env.var(v);
            let literal = if value { var } else { self.env.not(var) };
            for &c in &self.occurs[v] {
                let restricted = self.env.and(constraints[c].clone(), literal.clone());
                constraints[c] = self.env.exists(vec![v], restricted);
                if constraints[c].is_false() {
                    return None;
                }
                units.extend(unit(&constraints[c]));
            }
        }
        let open = (0..values.len()).filter(|&v| {
            values[v].is_none() && self.occurs[v].iter().any(|&c| constraints[c].is_choice())
        });
        let Some(v) = open.min_by_key(|&v| (self.variables[v].args().iter().max().copied(), v))
        else {
            return Some(values);
        };

        self.decisions += 1;
        if self.decisions > SEARCH_BUDGET {
            return None;
        }
        let mdn = self.variables[v].args().iter().copied().chain(mdn).max();
        let decisions: Vec<_> = match &self.variables[v] {
            Variable::Atom(_) => vec![(v, true, mdn), (v, false, mdn)],
            Variable::Value(application, _) => {
                let base = self.values[application];
                let least = mdn.map_or(0, |m| m + 1).min(self.n - 1);
                (0..=least)
                    .filter(|&i| values[base + i].is_none())
                    .map(|i| (base + i, true, mdn.max(Some(i))))
                    .collect()
            }
        };
        for (v, value, mdn) in decisions {
            let units = vec![(v, value)];
            if let Some(values) = self.solve(constraints.clone(), values.clone(), units, mdn) {
                return Some(values);
            }
        }
        None
    }

    /// Add the constraints of the literals of a clause, with its variables bound to a tuple of elements. A nested
    /// subterm is replaced by a variable for each of its values, which the constraint assumes, as the BDD of a nested
    /// term grows with the values of all the applications it may denote. Returns false if a constraint is false.
    fn flatten(
        &mut self,
        literals: &[Literal],
        vars: &mut Vec<NamedSymbol>,
        tuple: &mut Vec<usize>,
        assumed: BDD<usize>,
    ) -> bool {
        let nested = literals.iter().find_map(|literal| {
            [&literal.l, &literal.r]
                .into_iter()
                .find_map(|t| nested(head(t).1, vars))
        });
        let Some(subterm) = nested.cloned() else {
            if self.env.size() > NODE_BUDGET {
                self.env.gc();
            }
            let instance = self.instance(literals, vars, tuple);
            let constraint = self.env.or(assumed, instance);
            if constraint.is_false() || self.env.size() > NODE_BUDGET {
                return false;
            }
            self.constraints.push(constraint);
            return true;
        };
        let x = NamedSymbol {
            name: Rc::new(String::new()),
            id: usize::MAX - vars.len(),
        };
        let literals: Vec<_> = literals
            .iter()
            .map(|literal| Literal {
                positive: literal.positive,
                l: replace(&literal.l, &subterm, &x),
                r: replace(&literal.r, &subterm, &x),
            })
            .collect();
        let values = self.term(&subterm, vars, tuple);
        vars.push(x);
        for (i, value) in values.into_iter().enumerate() {
            if value.is_false() {
                continue;
            }
            tuple.push(i);
            let assumed = self.env.or(assumed.clone(), self.env.not(value));
            let consistent = self.flatten(&literals, vars, tuple, assumed);
            tuple.pop();
            if !consistent {
                return false;
            }
        }
        vars.pop();
        true
    }

    /// The disjunction of the literals of a clause, with its variables bound to a tuple of elements
    fn instance(
        &mut self,
        literals: &[Literal],
        vars: &[NamedSymbol],
        tuple: &[usize],
    ) -> BDD<usize> {
        let mut result = self.env.mk_const(false);
        for literal in literals {
            let holds = if literal.r == *self.top {
                self.atom(&literal.l, vars, tuple)
            } else {
                let l = self.term(&literal.l, vars, tuple);
                let r = self.term(&literal.r, vars, tuple);
                l.into_iter()
                    .zip(r)
                    .fold(self.env.mk_const(false), |eq, (l, r)| {
                        self.env.or(eq, self.env.and(l, r))
                    })
            };
            let holds = if literal.positive {
                holds
            } else {
                self.env.not(holds)
            };
            result = self.env.or(result, holds);
            if result.is_true() {
                break;
            }
        }
        result
    }

    fn atom(&mut self, t: &DomainConstant, vars: &[NamedSymbol], tuple: &[usize]) -> BDD<usize> {
        let p = head(t).0;
        let mut result = self.env.mk_const(false);
        for (guard, args) in self.cases(t, vars, tuple) {
            let key = (p.clone(), args);
            let next = self.variables.len();
            let v = *self.atoms.entry(key.clone()).or_insert(next);
            if v == next {
                self.variables.push(Variable::Atom(key));
            }
            result = self.env.or(result, self.env.and(guard, self.env.var(v)));
            if self.env.size() > NODE_BUDGET {
                break;
            }
        }
        result
    }

    /// The conditions under which a term denotes each element
    fn term(
        &mut self,
        t: &DomainConstant,
        vars: &[NamedSymbol],
        tuple: &[usize],
    ) -> Vec<BDD<usize>> {
        if let DomainConstant::Terminal(x) = t
            && let Some(i) = vars.iter().position(|v| v == x)
        {
            return (0..self.n)
                .map(|j| self.env.mk_const(tuple[i] == j))
                .collect();
        }
        let f = head(t).0;
        let mut result = vec![self.env.mk_const(false); self.n];
        for (guard, args) in self.cases(t, vars, tuple) {
            let key = (f.clone(), args);
            let next = self.variables.len();
            let base = *self.values.entry(key.clone()).or_insert(next);
            if base == next {
                self.variables
                    .extend((0..self.n).map(|i| Variable::Value(key.clone(), i)));
                let values: Vec<_> = (base..base + self.n).map(|v| self.env.var(v)).collect();
                self.constraints.push(self.env.exn(&values, 1));
            }
            if self.env.size() > NODE_BUDGET {
                break;
            }
            for (i, value) in result.iter_mut().enumerate() {
                let case = self.env.and(guard.clone(), self.env.var(base + i));
                *value = self.env.or(value.clone(), case);
            }
        }
        result
    }

    /// The tuples of elements the arguments of an application may denote, with the condition under which they do
    fn cases(
        &mut self,
        t: &DomainConstant,
        vars: &[NamedSymbol],
        tuple: &[usize],
    ) -> Vec<(BDD<usize>, Vec<usize>)> {
        let args: Vec<_> = head(t)
            .1
            .iter()
            .map(|a| self.term(a, vars, tuple))
            .collect();
        let supports = args.iter().map(|arg| {
            (0..self.n)
                .filter(|&i| !arg[i].is_false())
                .collect::<Vec<_>>()
        });
        supports
            .multi_cartesian_product()
            .map(|elements| {
                let guard = args
                    .iter()
                    .zip(&elements)
                    .fold(self.env.mk_const(true), |guard, (arg, &i)| {
                        self.env.and(guard, arg[i].clone())
                    });
                (guard, elements)
            })
            .collect()
    }
}

/// Whether a symbol is left out of a model: a variable or definition made by the prover, but not a Skolem function
fn hidden(symbol: &NamedSymbol) -> bool {
    made_by_prover(symbol) && !symbol.name.starts_with("sk")
}

/// The literal a constraint forces at its root
fn unit(c: &BDD<usize>) -> Option<(usize, bool)> {
    match c.node() {
        Node::Choice(hi, v, lo) if hi.is_false() || lo.is_false() => Some((v, lo.is_false())),
        _ => None,
    }
}

/// The variables of a BDD
#[allow(clippy::mutable_key_type)]
fn support(b: &BDD<usize>, seen: &mut FxHashSet<BDD<usize>>, vars: &mut Vec<usize>) {
    if let Node::Choice(hi, v, lo) = b.node()
        && seen.insert(b.clone())
    {
        if !vars.contains(&v) {
            vars.push(v);
        }
        support(&hi, seen, vars);
        support(&lo, seen, vars);
    }
}

/// An innermost application in the arguments of a term that are not variables
fn nested<'a>(args: &'a [DomainConstant], vars: &[NamedSymbol]) -> Option<&'a DomainConstant> {
    args.iter().find_map(|a| match a {
        DomainConstant::Terminal(x) if vars.contains(x) => None,
        _ => nested(head(a).1, vars).or(Some(a)),
    })
}

/// A term with every occurrence of a subterm replaced by a variable
fn replace(t: &DomainConstant, subterm: &DomainConstant, x: &NamedSymbol) -> DomainConstant {
    match t {
        _ if t == subterm => DomainConstant::Terminal(x.clone()),
        DomainConstant::Terminal(_) => t.clone(),
        DomainConstant::Application(f, args) => DomainConstant::Application(
            f.clone(),
            args.iter().map(|a| replace(a, subterm, x)).collect(),
        ),
    }
}

fn head(t: &DomainConstant) -> (&NamedSymbol, &[DomainConstant]) {
    match t {
        DomainConstant::Terminal(f) => (f, &[]),
        DomainConstant::Application(f, args) => (f, args),
    }
}

fn element(i: usize) -> String {
    format!("e{}", i + 1)
}

fn show((f, args): &Application) -> String {
    if args.is_empty() {
        f.to_string()
    } else {
        format!("{f}({})", args.iter().map(|&i| element(i)).join(", "))
    }
}
