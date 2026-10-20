use std::fmt;

use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::NamedSymbol;
use crate::bdd::{BDD, BDDEnv, Node};
use crate::parser::DomainConstant;
use crate::prover::{Clause, made_by_prover};

/// The largest domain searched for a model
const MODEL_SIZE: usize = 16;
/// The number of BDD nodes allocated for a domain after which the search stops, as larger domains need more
const NODE_BUDGET: usize = 1_000_000;

/// A symbol applied to the indices of elements of the domain
pub type Application = (NamedSymbol, Vec<usize>);

/// A finite model over the domain `e1, .., en`
///
/// The ground atoms that are true or false and the values of the functions of the formula. Symbols introduced by the
/// prover (Skolem functions and definitions) are left out.
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
/// needs more than `NODE_BUDGET` BDD nodes
pub(crate) fn find_model(clauses: &[Clause], top: &DomainConstant) -> Option<Model> {
    for n in 1..=MODEL_SIZE {
        let mut search = Search::new(n, top);
        if let Some(model) = search.model(clauses) {
            return Some(model);
        }
        if search.env.size() > NODE_BUDGET {
            break;
        }
    }
    None
}

enum Variable {
    Atom(Application),
    Value(Application, usize),
}

/// A search for a model over the domain `e1, .., en`, as in Mace4 (McCune, 2003). Every ground atom `p(e1, ..)` and
/// every value `f(e1, ..) = e2` of a function is a variable of the BDD, and a function has exactly one value for every
/// argument. A term is evaluated to the conditions under which it denotes each element.
struct Search<'a> {
    n: usize,
    top: &'a DomainConstant,
    env: BDDEnv<usize>,
    variables: Vec<Variable>,
    atoms: FxHashMap<Application, usize>,
    // the first of the `n` variables of the values of every application of a function to elements
    values: FxHashMap<Application, usize>,
    // the constraints of the functions that have not been added to the formula yet
    functional: BDD<usize>,
}

impl<'a> Search<'a> {
    fn new(n: usize, top: &'a DomainConstant) -> Self {
        let env = BDDEnv::new();
        Self {
            n,
            top,
            functional: env.mk_const(true),
            env,
            variables: Vec::new(),
            atoms: FxHashMap::default(),
            values: FxHashMap::default(),
        }
    }

    fn model(&mut self, clauses: &[Clause]) -> Option<Model> {
        let mut result = self.env.mk_const(true);
        for clause in clauses {
            for tuple in itertools::repeat_n(0..self.n, clause.vars.len()).multi_cartesian_product()
            {
                let instance = self.instance(clause, &tuple);
                let functional = std::mem::replace(&mut self.functional, self.env.mk_const(true));
                result = self.env.and(result, self.env.and(instance, functional));
                if result.is_false() || self.env.size() > NODE_BUDGET {
                    return None;
                }
            }
        }

        let mut model = Model {
            size: self.n,
            atoms: Vec::new(),
            values: Vec::new(),
        };
        let mut path = self.env.model(result);
        while let Node::Choice(hi, v, lo) = path.node() {
            let value = !hi.is_false();
            match &self.variables[v] {
                Variable::Atom(atom) if !made_by_prover(&atom.0) => {
                    model.atoms.push((atom.clone(), value));
                }
                Variable::Value(application, i) if value && !made_by_prover(&application.0) => {
                    model.values.push((application.clone(), *i));
                }
                _ => {}
            }
            path = if value { hi } else { lo };
        }
        Some(model)
    }

    /// The disjunction of the literals of a clause, with its variables bound to a tuple of elements
    fn instance(&mut self, clause: &Clause, tuple: &[usize]) -> BDD<usize> {
        let mut result = self.env.mk_const(false);
        for literal in &clause.literals {
            let holds = if literal.r == *self.top {
                self.atom(&literal.l, clause, tuple)
            } else {
                let l = self.term(&literal.l, clause, tuple);
                let r = self.term(&literal.r, clause, tuple);
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

    fn atom(&mut self, t: &DomainConstant, clause: &Clause, tuple: &[usize]) -> BDD<usize> {
        let p = head(t).0;
        let mut result = self.env.mk_const(false);
        for (guard, args) in self.cases(t, clause, tuple) {
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
    fn term(&mut self, t: &DomainConstant, clause: &Clause, tuple: &[usize]) -> Vec<BDD<usize>> {
        if let DomainConstant::Terminal(x) = t
            && let Some(i) = clause.vars.iter().position(|v| v == x)
        {
            return (0..self.n)
                .map(|j| self.env.mk_const(tuple[i] == j))
                .collect();
        }
        let f = head(t).0;
        let mut result = vec![self.env.mk_const(false); self.n];
        for (guard, args) in self.cases(t, clause, tuple) {
            let key = (f.clone(), args);
            let next = self.variables.len();
            let base = *self.values.entry(key.clone()).or_insert(next);
            if base == next {
                self.variables
                    .extend((0..self.n).map(|i| Variable::Value(key.clone(), i)));
                let values: Vec<_> = (base..base + self.n).map(|v| self.env.var(v)).collect();
                let exactly_one = self.env.exn(&values, 1);
                self.functional = self.env.and(self.functional.clone(), exactly_one);
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
        clause: &Clause,
        tuple: &[usize],
    ) -> Vec<(BDD<usize>, Vec<usize>)> {
        let args: Vec<_> = head(t)
            .1
            .iter()
            .map(|a| self.term(a, clause, tuple))
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
