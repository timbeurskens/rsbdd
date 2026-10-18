# Theorem proving

This chapter builds on [Rewrite rules](rewriting.md).

## Theorem proving by term rewriting

A symbol used inside a term or as an operand of `=` is a function symbol. Its rules are term rules: the right-hand
side is a term instead of a formula.

```
sum y # plus(_0, y) -> y;
sum x, y # plus(suc(x), y) -> suc(plus(x, y));

plus(suc(_0), suc(_0)) = suc(suc(_0))
```

- Terms are rewritten to normal form (innermost first, using the first matching rule) before they are compared,
  matched against rules or turned into boolean variables. `t1 = t2` therefore holds modulo the term rules, and
  `p(plus(_0, A))` and `p(A)` are the same proposition.
- If the term rules form a convergent (terminating and confluent) rewrite system, two terms are equal in the
  equational theory of the rules if and only if their normal forms are identical (Knuth & Bendix, 1970; Baader &
  Nipkow, *Term Rewriting and All That*, 1998). A theorem is proven if the formula evaluates to `true`.
- The rules are [completed](#completion) if possible. Otherwise convergence is not checked: a non-terminating system
  does not terminate, and the result of a non-confluent system depends on the order of the rules.
- Quantifiers range over the normal forms of the ground terms in the formula, so a quantified theorem is only proven
  for those terms. Use `induct` for a proof for all values.

See [examples/peano_arith.txt](../examples/peano_arith.txt) and [examples/group.txt](../examples/group.txt), which proves
theorems of group theory from the group axioms by [completion](#completion).

Term rules combine with unknowns solved by the solver. [examples/peano_solve.txt](../examples/peano_solve.txt) finds
natural numbers `x` and `y` with `x + y = 4` and `x * y = 3`, and proves by refutation that `x` is odd in every
solution: adding the negation of the theorem makes the formula unsatisfiable.

```bash
rsbdd examples/peano_solve.txt -t -ft
```

The BDD, obtained with `rsbdd examples/peano_solve.txt -c true -d peano_solve.dot`, has two paths to `true`:
`x = 1, y = 3` and `x = 3, y = 1`. The `-c true` flag omits every value that occurs in no solution.

![peano solve bdd](images/peano_solve_bdd.svg)

## Completion

Term rules need not form a convergent rewrite system: they are completed by Knuth-Bendix completion (Knuth & Bendix,
1970), which adds the equations needed for confluence (critical pairs) as new rules. The three group axioms below do
not rewrite `mul(x, e)`, but completion derives the convergent rewrite system of Knuth and Bendix, with rules such as
`mul(x, e) -> x` and `inv(inv(x)) -> x`, which prove both goals.

```
sum x # mul(e, x) -> x;
sum x # mul(inv(x), x) -> e;
sum x, y, z # mul(mul(x, y), z) -> mul(x, mul(y, z));

mul(x, e) = x;
mul(x, inv(x)) = e
```

- The given rules keep their direction. Derived equations are oriented by the lexicographic path order, with the
  default precedence of Prover9: constants are the smallest symbols, followed by symbols of increasing arity, with
  unary symbols the largest. Symbols of the same arity are ordered by their first occurrence.
- A name in a goal that does not occur in the rules, such as `x` above, is an arbitrary constant: the goal is proven
  for every value of `x`, like the Skolem constant Prover9 introduces when denying the goal.
- A completed rule set has equational semantics: with `f(a) -> b; f(a) -> c`, both `f(a) = b` and `f(a) = c` hold.
- If completion fails, because a derived equation cannot be oriented or completion does not finish within 1000 steps,
  the rules are used as given. A given rule that does not terminate, such as `f(x, y) -> f(y, x)`, still does not
  terminate.

See [examples/right_identity.txt](../examples/right_identity.txt), the same problem as
[examples/in_progress/right-identity.prover9.txt](../examples/in_progress/right-identity.prover9.txt) for Prover9.

## Proofs by induction

Rewriting proves `plus(suc(_0), _0) = suc(_0)`, but not `plus(x, _0) = x` for every natural number `x`: `plus`
recurses on its first argument, so `plus(x, _0)` cannot be rewritten while `x` is unknown. `induct` proves such an
equation by structural induction.

```
sum y # plus(_0, y) -> y;
sum x, y # plus(suc(x), y) -> suc(plus(x, y));

induct x # plus(x, _0) = x;
induct x, y # plus(x, suc(y)) = suc(plus(x, y));
induct x, y # plus(x, y) = plus(y, x)
```

- `induct x, y # l = r` proves the equation `l = r` by induction on `x`, for every value of `y`.
- The cases are the patterns of the rules of the function applied to `x` (a cover set): `_0` and `suc(x')` for
  `plus`. A case is proven if both sides rewrite to the same normal form. The induction hypothesis for every variable
  of the pattern, such as `plus(x', _0) = x'`, is used as an extra rewrite rule in that case.
- A proven equation `l = r` becomes the rewrite rule `l -> r` (or `r -> l` if `l` is a variable) for the remainder of
  the formula, so it can be used as a lemma. An equation such as commutativity, which would rewrite forever, is not
  added. In the example above, commutativity is proven using the first two theorems.
- `induct` is `true` if the equation is proven, and `false` if it is not. As the method is incomplete, `false` means
  "not proven" rather than "disproven", so `induct` should not be negated.
- `forall` is not affected: it remains an exact check over the ground terms in the formula.

This follows the Boyer–Moore approach (Boyer & Moore, *A Computational Logic*, 1979): the induction scheme follows the
recursion of the definitions (cover set induction; Zhang, Kapur & Krishnamoorthy, 1988), the hypothesis is used by
rewriting (rewriting induction; Reddy, 1990), and proven theorems become rewrite rules.

Limitations:
- Needed lemmas are not discovered and goals are not generalised: they must be stated as earlier theorems.
- The body of `induct` must be a single equation.
- The patterns of the rules are assumed to cover all values (sufficient completeness): the theorem is proven for the
  terms covered by the rules, i.e. `_0` and `suc(..)`.
- Induction variables must not be used as constants in the rules.

See [examples/induction.txt](../examples/induction.txt), which proves identity, commutativity and associativity of
addition.
