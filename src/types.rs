//! Types: declarations `let f: A;`, typed binders `forall x: A #` and their meaning.
//!
//! A type is a term: `type`, a declared type name, a type variable, or a declared type constructor applied to types.
//! A binder `x: A` is the guard `x: A`, an atom of a predicate that cannot be written as a name, so
//! `forall x: A # f` is `forall x # x: A => f`. The declarations give the guard its meaning:
//!
//! - In finite mode, the declarations are rules for the guard, so `x: A` holds for the ground terms of type `A`.
//! - With `--prove`, the declarations are axioms: every application of a declared symbol to arguments of the declared
//!   types has the declared result type, and every type has an element. This is the guard encoding of typed
//!   first-order logic, which is sound and complete (Blanchette et al., Encoding monomorphic and polymorphic types,
//!   2016). The rules of finite mode cannot be used, as their completion would make every element of a type a term.

use std::io;

use rustc_hash::FxHashMap;

use crate::NamedSymbol;
use crate::parser::{
    BinaryOperator, DomainConstant, ParsedFormula, QuantifierType, ReferenceContents, Signature,
    SymbolicBDD, argument_symbol, type_symbol,
};
use crate::rewriter::Rule;

fn error(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("type error: {message}"))
}

fn conjunction(guards: Vec<DomainConstant>) -> SymbolicBDD {
    guards
        .into_iter()
        .map(SymbolicBDD::RuleApplication)
        .reduce(|l, r| SymbolicBDD::BinaryOp(BinaryOperator::And, Box::new(l), Box::new(r)))
        .unwrap_or(SymbolicBDD::True)
}

fn forall(vars: Vec<NamedSymbol>, body: SymbolicBDD) -> SymbolicBDD {
    if vars.is_empty() {
        body
    } else {
        SymbolicBDD::Quantifier(QuantifierType::Forall, vars, Box::new(body))
    }
}

/// The rules of finite mode for the guard: `x0.., x1.. # f(x0.., x1..): B -> x0: type & .. & x1: A1 & ..`
pub fn typing_rules(signatures: &[(NamedSymbol, Signature)]) -> Vec<Rule> {
    signatures
        .iter()
        .filter_map(|(f, signature)| {
            let result = signature.result.clone()?;
            let (application, guards) = signature.application(f);
            Some(Rule {
                vars: signature.vars(),
                lhs: DomainConstant::has_type(application, result),
                rhs: conjunction(guards),
            })
        })
        .collect()
}

/// The axioms of `--prove`: the result type of every declared function, and an element of every declared type
pub fn typing_axioms(signatures: &[(NamedSymbol, Signature)]) -> Vec<SymbolicBDD> {
    let mut axioms = Vec::new();
    for (f, signature) in signatures {
        let Some(result) = &signature.result else {
            continue;
        };
        let (application, guards) = signature.application(f);
        let vars = signature.vars();
        let typed = SymbolicBDD::RuleApplication(DomainConstant::has_type(
            application.clone(),
            result.clone(),
        ));
        axioms.push(forall(
            vars.clone(),
            SymbolicBDD::binop(BinaryOperator::Implies, conjunction(guards.clone()), typed),
        ));
        if *result == DomainConstant::Terminal(type_symbol()) {
            let element = argument_symbol(vars.len());
            let inhabited = SymbolicBDD::Quantifier(
                QuantifierType::Exists,
                vec![element.clone()],
                Box::new(SymbolicBDD::RuleApplication(DomainConstant::has_type(
                    DomainConstant::Terminal(element),
                    application,
                ))),
            );
            axioms.push(forall(
                vars,
                SymbolicBDD::binop(BinaryOperator::Implies, conjunction(guards), inhabited),
            ));
        }
    }
    axioms
}

/// Whether the guards can be erased: no typed variable is an operand of `=`.
///
/// A type is monotonic if a model with more elements of the type exists whenever a model exists, and the guards of
/// monotonic types can be erased without changing satisfiability (Claessen, Lillieström & Smallbone, Sort it out with
/// monotonicity, 2011). A type is monotonic if no variable of the type is an operand of an equation, which is checked
/// in either polarity. Erasing the guards avoids the infinitely many well-typed terms that the typing axioms of a
/// recursive signature derive.
pub fn erasable(formula: &SymbolicBDD) -> bool {
    fn naked(f: &SymbolicBDD, typed: &mut Vec<NamedSymbol>) -> bool {
        match f {
            SymbolicBDD::Quantifier(q, vars, body) => {
                let depth = typed.len();
                typed.extend(
                    SymbolicBDD::binder_types(*q, body)
                        .into_iter()
                        .filter(|(v, _)| vars.contains(v))
                        .map(|(v, _)| v.clone()),
                );
                let result = naked(body, typed);
                typed.truncate(depth);
                result
            }
            SymbolicBDD::BinaryOp(BinaryOperator::Equals, l, r) => [l, r]
                .iter()
                .any(|side| matches!(side.as_ref(), SymbolicBDD::Var(v) if typed.contains(v))),
            other => other.children().into_iter().any(|c| naked(c, typed)),
        }
    }
    !naked(formula, &mut Vec::new())
}

/// The formula without guards: `forall x: A # f` becomes `forall x # f`
pub fn erase(formula: &SymbolicBDD) -> SymbolicBDD {
    let erase_all = |fs: &[SymbolicBDD]| fs.iter().map(erase).collect();
    match formula {
        SymbolicBDD::RuleApplication(t) if t.as_guard().is_some() => SymbolicBDD::True,
        SymbolicBDD::Quantifier(q, vars, body) => {
            SymbolicBDD::Quantifier(*q, vars.clone(), Box::new(erase(body)))
        }
        SymbolicBDD::AtomQuantifier(q, atoms, body) => {
            SymbolicBDD::AtomQuantifier(*q, atoms.clone(), Box::new(erase(body)))
        }
        SymbolicBDD::Summation(vars, body) => {
            SymbolicBDD::Summation(vars.clone(), Box::new(erase(body)))
        }
        SymbolicBDD::FixedPoint(v, initial, body) => {
            SymbolicBDD::FixedPoint(v.clone(), *initial, Box::new(erase(body)))
        }
        SymbolicBDD::Not(f) => SymbolicBDD::Not(Box::new(erase(f))),
        SymbolicBDD::BinaryOp(op, l, r) => SymbolicBDD::binop(*op, erase(l), erase(r)),
        SymbolicBDD::Ite(c, t, e) => {
            SymbolicBDD::Ite(Box::new(erase(c)), Box::new(erase(t)), Box::new(erase(e)))
        }
        SymbolicBDD::CountableConst(op, fs, n) => {
            SymbolicBDD::CountableConst(*op, erase_all(fs), *n)
        }
        SymbolicBDD::CountableVariable(op, l, r) => {
            SymbolicBDD::CountableVariable(*op, erase_all(l), erase_all(r))
        }
        other => other.clone(),
    }
}

/// Checks that declared symbols are applied to arguments of their declared types
struct Checker<'a> {
    signatures: FxHashMap<&'a NamedSymbol, &'a Signature>,
    // the bound variables with their types, innermost last
    scope: Vec<(NamedSymbol, Option<DomainConstant>)>,
}

fn substitute(
    t: &DomainConstant,
    theta: &FxHashMap<&NamedSymbol, &DomainConstant>,
) -> DomainConstant {
    match t {
        DomainConstant::Terminal(v) => theta
            .get(v)
            .map_or_else(|| t.clone(), |value| (*value).clone()),
        DomainConstant::Application(f, args) => DomainConstant::Application(
            f.clone(),
            args.iter().map(|a| substitute(a, theta)).collect(),
        ),
    }
}

impl Checker<'_> {
    /// `Some` for a bound variable, with its type if it has one
    fn bound(&self, v: &NamedSymbol) -> Option<Option<&DomainConstant>> {
        self.scope
            .iter()
            .rev()
            .find_map(|(w, ty)| (w == v).then_some(ty.as_ref()))
    }

    /// `A` is `type`, a type variable, or a declared type name or constructor applied to types
    fn check_type(&self, ty: &DomainConstant) -> io::Result<()> {
        let is_type = |s: &DomainConstant| *s == DomainConstant::Terminal(type_symbol());
        let (f, args) = match ty {
            DomainConstant::Terminal(v) if *v == type_symbol() => return Ok(()),
            DomainConstant::Terminal(v) if self.bound(v).flatten().is_some_and(is_type) => {
                return Ok(());
            }
            DomainConstant::Terminal(v) => (v, &[][..]),
            DomainConstant::Application(f, args) => (f, args.as_slice()),
        };
        match self.signatures.get(f) {
            Some(signature)
                if signature.params.is_empty()
                    && signature.args.len() == args.len()
                    && signature.args.iter().all(is_type)
                    && signature.result.as_ref().is_some_and(is_type) =>
            {
                args.iter().try_for_each(|a| self.check_type(a))
            }
            _ => Err(error(format!("{ty} is not a declared type"))),
        }
    }

    /// The result of a declared application, `Some(None)` for a predicate, after checking its arguments
    fn application(&self, t: &DomainConstant) -> io::Result<Option<Option<DomainConstant>>> {
        let (f, args) = match t {
            DomainConstant::Terminal(v) if self.bound(v).is_some() => return Ok(None),
            DomainConstant::Terminal(v) => (v, &[][..]),
            DomainConstant::Application(f, args) => (f, args.as_slice()),
        };
        let Some(signature) = self.signatures.get(f) else {
            for arg in args {
                self.term(arg)?;
            }
            return Ok(None);
        };
        let n = signature.params.len() + signature.args.len();
        if args.len() != n {
            let plural = if n == 1 { "" } else { "s" };
            return Err(error(format!("{t}: {f} takes {n} argument{plural}")));
        }
        let (type_args, value_args) = args.split_at(signature.params.len());
        for arg in type_args {
            self.check_type(arg)?;
        }
        let theta: FxHashMap<_, _> = signature.params.iter().zip(type_args).collect();
        for (arg, declared) in value_args.iter().zip(&signature.args) {
            let declared = substitute(declared, &theta);
            if let Some(actual) = self.term(arg)?
                && actual != declared
            {
                return Err(error(format!(
                    "{t}: the argument {arg} has type {actual}, but {f} expects {declared}"
                )));
            }
        }
        Ok(Some(
            signature.result.as_ref().map(|r| substitute(r, &theta)),
        ))
    }

    /// The type of a term, if it is known
    fn term(&self, t: &DomainConstant) -> io::Result<Option<DomainConstant>> {
        if let DomainConstant::Terminal(v) = t
            && let Some(ty) = self.bound(v)
        {
            return Ok(ty.cloned());
        }
        match self.application(t)? {
            Some(None) => Err(error(format!("{t} is a formula, not a term"))),
            Some(result) => Ok(result),
            None => Ok(None),
        }
    }

    fn atom(&self, t: &DomainConstant) -> io::Result<()> {
        match self.application(t)? {
            Some(Some(ty)) => Err(error(format!("{t} is a term of type {ty}, not a formula"))),
            _ => Ok(()),
        }
    }

    fn as_term(f: &SymbolicBDD) -> Option<DomainConstant> {
        match f {
            SymbolicBDD::Var(v) => Some(DomainConstant::Terminal(v.clone())),
            SymbolicBDD::RuleApplication(t) => Some(t.clone()),
            _ => None,
        }
    }

    fn formula(&mut self, f: &SymbolicBDD) -> io::Result<()> {
        match f {
            SymbolicBDD::Quantifier(q, vars, body) => {
                let depth = self.scope.len();
                let types = SymbolicBDD::binder_types(*q, body);
                for v in vars {
                    let ty = types.iter().find_map(|(w, ty)| (*w == v).then_some(*ty));
                    self.scope.push((v.clone(), ty.cloned()));
                }
                for (_, ty) in &types {
                    self.check_type(ty)?;
                }
                let result = self.formula(body);
                self.scope.truncate(depth);
                result
            }
            SymbolicBDD::RuleApplication(t) if t.as_guard().is_some() => Ok(()),
            SymbolicBDD::RuleApplication(t) => self.atom(t),
            SymbolicBDD::Var(v) => self.atom(&DomainConstant::Terminal(v.clone())),
            SymbolicBDD::BinaryOp(
                op @ (BinaryOperator::Equals | BinaryOperator::Rewrite),
                l,
                r,
            ) => {
                let (Some(lt), Some(rt)) = (Self::as_term(l), Self::as_term(r)) else {
                    // a rule for a predicate
                    if let Some(lt) = Self::as_term(l) {
                        self.atom(&lt)?;
                    }
                    return self.formula(r);
                };
                let predicate = |t| Ok::<_, io::Error>(matches!(self.application(t)?, Some(None)));
                if *op == BinaryOperator::Rewrite && (predicate(&lt)? || predicate(&rt)?) {
                    self.atom(&lt)?;
                    return self.formula(r);
                }
                if let (Some(a), Some(b)) = (self.term(&lt)?, self.term(&rt)?)
                    && a != b
                {
                    return Err(error(format!(
                        "{l} {op} {r}: {a} and {b} are different types"
                    )));
                }
                Ok(())
            }
            other => other
                .children()
                .into_iter()
                .try_for_each(|c| self.formula(c)),
        }
    }
}

/// Check the declarations, and that declared symbols are applied to arguments of their declared types
pub fn check(parsed: &ParsedFormula) -> io::Result<()> {
    let mut checker = Checker {
        signatures: parsed.signatures.iter().map(|(f, s)| (f, s)).collect(),
        scope: Vec::new(),
    };
    for (_, signature) in &parsed.signatures {
        let types = signature.args.iter().chain(&signature.result);
        for param in &signature.params {
            checker
                .scope
                .push((param.clone(), Some(DomainConstant::Terminal(type_symbol()))));
        }
        for ty in types {
            checker.check_type(ty)?;
        }
        checker.scope.clear();
    }
    for definition in parsed.definitions.borrow().values() {
        if let ReferenceContents::Syntax(f) = definition {
            checker.formula(f)?;
        }
    }
    checker.formula(&parsed.bdd)
}
