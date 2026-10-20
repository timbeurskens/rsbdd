# Finite mode and first-order proofs

This chapter builds on [Rewrite rules](rewriting.md) and [Theorem proving](theorem_proving.md).

rsbdd reads one language in two modes:

- **Finite mode** (the default) grounds the formula with the rewrite rules and solves it with the BDD solver. It
  decides whether a formula is true in one *canonical model* of the rules: the ground terms modulo the term rules,
  in which a predicate is false unless a rule makes it true.
- **`--prove`** reads the rules as first-order axioms and the rest of the formula as the goal, and decides by
  superposition whether the goal holds in *every model* of the axioms.

`--prove` reads the rules as finite mode does, and evaluates the constructs of finite mode that are not part of
first-order logic as finite mode does. A theorem of `--prove` is therefore valid in finite mode. The converse does
not hold: the canonical model satisfies formulas that do not hold in every model.

## Supported syntax

| Construct                                                         | Finite mode | `--prove`                                     |
|-------------------------------------------------------------------|-------------|-----------------------------------------------|
| `true`, `false`, variables, all binary operators, `if-then-else`  | yes         | yes                                           |
| `forall` / `exists` over term variables                           | yes         | yes                                           |
| `forall` / `exists` over boolean variables                        | yes         | yes, expanded over `true` and `false`         |
| `t1 = t2`                                                         | yes         | yes                                           |
| `let` definitions, constraint lists `;`, comments                 | yes         | yes                                           |
| Substitution of variables `f[a := c]`                             | yes         | yes                                           |
| Rewrite rules `->`, `sum`, term rules                             | yes         | yes                                           |
| Atom quantifiers `exists p(_) # ...`                              | yes         | yes, evaluated as in finite mode              |
| Substitution of atoms `f[p(_) := q(_)]`                           | yes         | yes, evaluated as in finite mode              |
| Counting `[a, b] = 1`, `[a] <= [a, b]`                            | yes         | yes, evaluated as in finite mode              |
| Fixed points `lfp` / `gfp`                                        | yes         | yes, evaluated as in finite mode              |
| Induction `induct`                                                | yes         | yes, the proven equation becomes an axiom     |

A construct that is evaluated as in finite mode may not apply a predicate defined by rules to a variable of the
proof, such as `forall x # [p(x)] = 1` with a rule for `p`. Finite mode matches the rules syntactically, while in a
proof `x` may be any element. `--prove` rejects these with the error `... applies a rule to a variable`.

## How `--prove` reads the rules

- **Term rules.** A term rule `l -> r` is the equation `l = r` for all values of its variables.
- **Definitions.** The rules of a predicate are its *Clark completion* (Clark, 1978). An application holds if and
  only if some rule matches, and the right-hand side of every matching rule holds. For the rules
  `sum x # p(x) -> q(x)` and `p(A) -> r`, this is

  ```
  forall x1 # p(x1) <=> ((exists x # x1 = x) | x1 = A) & (forall x # x1 = x => q(x)) & (x1 = A => r)
  ```

  As in finite mode, `p(A)` is `q(A) & r`, and `q(A) <=> r` is not a theorem.

## Unique names

In finite mode, distinct names are distinct elements: `p(A) -> true; -p(B)` is valid, as no rule makes `p(B)`
true. `--prove` adds the axioms of unique names for the *constructors*: terms with distinct constructors are
unequal, and constructors are injective.

- The constructors are the symbols of the terms in the rules and the formula, except the symbols that are the head
  of a term rule after [completion](theorem_proving.md#completion), and the symbols in an equation `=` of the
  formula.
- Equations of the formula are read in first-order logic, so the formula may equate any terms.
  `(forall x # mul(x, x) = e) => ...` assumes that `mul(x, x)` and `e` are equal, and does not contradict unique
  names.
- If the term rules cannot be completed, no axioms of unique names are added.

## Constructs evaluated as in finite mode

Counting, fixed points, atom quantifiers and atom substitution are evaluated by the rewriter and the BDD solver, as
in finite mode. The rewriter grounds the subformula with the rules, and the BDD solver decides it. Every node of the
resulting BDD becomes a new atom with its definition `node <=> if v then hi else lo`, where `v` is the atom of the
BDD variable. The clauses of these definitions are added to the proof.

An `induct` is proven by the rewriter as in finite mode. A proven equation is also an axiom for the proof, so

```
sum y # plus(_0, y) -> y;
sum x, y # plus(suc(x), y) -> suc(plus(x, y));

induct x # plus(x, _0) = x;
forall x # plus(x, _0) = x
```

is a theorem. A proof that uses an inductive lemma holds in the models in which the lemma holds, such as the
natural numbers, rather than in every model of the rules.

## Remaining differences

The canonical model of the rules cannot be described by first-order axioms when there are function symbols, so
some formulas that are valid in finite mode are not theorems of `--prove`:

| Formula                                                                  | Finite mode | `--prove`     |
|--------------------------------------------------------------------------|-------------|---------------|
| `-(A = B)`, `-(_0 = suc(_0))`                                            | valid       | not a theorem |
| `exists x, y # -(x = y)`                                                 | valid       | not a theorem |
| `e(A, B) -> true; sum x, y # r(x, y) -> e(x, y) \| exists z # e(x, z) & r(z, y); -r(B, A)` | valid | no answer |
| `forall x # plus(x, _0) = x`, with the rules of `plus`                   | not valid   | no answer     |

- **Equations between names.** The names in an equation of the formula are not constructors, so `-(A = B)` does
  not follow from unique names.
- **Domain.** In finite mode, quantifiers range over the ground terms in the formula and an arbitrary constant for
  every variable, all distinct. In `--prove`, they range over any domain, which may have a single element.
- **Recursive rules.** In finite mode, a recursive rule is the least fixpoint of its rules. Its completion also has
  larger models, so `-r(B, A)` cannot be proven, and the search does not terminate. Consequences of the least
  fixpoint, such as `r(A, B)`, are theorems.
- **Induction.** `forall x # plus(x, _0) = x` only holds in the standard model of the natural numbers. It is a
  theorem once `induct x # plus(x, _0) = x` is proven first.
