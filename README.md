# RsBDD

_Solving satisfiability problems in Rust_

## Syntax

### Comments

Characters contained within "..." (excluding the " char itself) are regarded as comments and can be placed at any point in the formula.

### Constants

The most basic building blocks of the syntax are 'variables' and 'constants'. A constant can be either 'true' or 'false'. A variable can accept either a 'true' or 'false' value after evaluation depending on its environment.

```
true
false
```

### Variables

A variable is a single word starting with a non-digit character. Examples of good variable names are:

```
a
alpha
_x
a1
hello_world
```

### Negation

A variable, constant, or sub-formula can be negated using the negation operator. This operator can be expressed by either `!`, `-`, or `not`.

```
not true
-false
!variable
```

### Binary operators

RsBDD supports the most common, and some uncommon binary operators, such as conjunction, disjunction, implication and bi-implication.

Most operators have a symbolic and textual representation, e.g. `and` or `&`.

| Operator           | Option 1      | Option 2 |
|--------------------|---------------|----------|
| Conjunction        | `and`         | `&`      |
| Disjunction        | `or`          | `\|`     |
| Implication        | `implies`     | `=>`     |
| Bi-implication     | `iff` or `eq` | `<=>`    |
| Exlusive or        | `xor`         | `^`      |
| Joint denial       | `nor`         | N.A.     |
| Alternative denial | `nand`        | N.A.     |

```
true or false
true | false
a | b
a & b
a and b
a => b
hello <=> world
on ^ off
```

### Composition

Larger formulae can be composed using left and right parentheses: `(`, `)`:

```
a | (a & b)
(a)
((a))
!(a & b)
(a & b) | (b & c)
```

### If-then-else

A simplification of a common expression `(a => b) & ((!a) => c)` can be made using the ternary if-then-else (ite) operator.

```
if a then b else c
if exists a # a <=> b then b <=> c else false | c
```

### Quantifiers

The RsBDD supports universal and existential quantification using the `exists` and `forall`/`all` keywords: `{forall|exists} var_1, var_2, .., var_n # {subformula}`

```
forall a # true
forall a # a | b
forall a, b # exists c # (c | a) & (c | b)
```

### Counting

For some problems it can be beneficial to express properties relating to the number of true or false variables, e.g. "at least 2 of the 4 properties must hold".

The counting operator (`[]`) in combination with five new equality and inequality operators (`=`, `<=`, `>=`, `<`, `>`) can be used to concisely express these properties.

_Note:_ like most operators, the counting operator can be expressed using logic primitives, but this operator simplifies the expression significantly.

A counting comparison can either be made by comparing a set of expressions to a given constant, or an other set of expressions.

```
"exactly one of a, b, and c holds"
[a, b, c] = 1

"there are strictly less true expressions in a, b, c than d, e, f"
[a, b, c] < [d, e, f]
```

### Experimental and/or upcoming features

The RsBDD library supports the use of fixed-point equations. At this moment the expressiveness of the language is too limited to benefit from fixed-points, so it is not yet available in the language. The recent introduction of counting operators provides a framework for fixed-point problem definitions which will be explored in the future.

Currently the RsBDD language relies heavily on logical primitives. Integer arithmetic could be expressed by manually introducing the primitive 'bits' of a number. Rewrite rules could significantly simplify this process by introducting domains other than boolean variables. Embedding rewrite rules in the BDD could prove to be a challenge.

## Rewrite rules (experimental)

Rewrite rules introduce rewriting of domain-based variables and constants into logical primitives.
The rewrite system introduces two basic types, variables and constants, and some new operators.

- Summation (`sum`): introduce a new variable which can take any value.
- Rewrite (`->`): binary operator specifying a uni-directional rewrite rule.
- Bi-directional rewrite (`<->`): the symmetric version of the rewrite operator.
- Equivalence (`=`): binary relation specifying the equivalence of the left and right variables / constants.

A rewrite rule, yields a truth value similar to an implication. 
It is therefore possible that the rewrite operator and the implication operator will share the same symbol.
For extra clarity, the rewrite operator `->` differs from the implication operator `=>` in this design document.
For every formula requiring a rewrite, every rewrite rule must be satisfied (conjunction of all rules).

**Rewriting:**

For every rewrite application in the formula, the rule application is replaced by a conjunction of all rewrite rules.
The environment is updated (recursively), where the rule application is now an assumption / goal.
Until the formula is closed (no remaining variables), the algorithm tries to match any open rewrite rule to an assumption in the environment.
If a match has been found for some rewrite rule `x -> y`, `x` is replaced by `true`. If no match can be found for the rule, `x` is replaced by `false`.
A rewrite rule `false -> y` can be rewritten as `true` by `rewrite elimination`. A rewrite rule `true -> y` can be rewritten as `y` by `rewrite elimination`.

**Matching:**

**Quantification:**

Quantifiers (`forall`, `exists`) convert a rewrite rule to a logically equivalent rule application with additional conditions.
Suppose a formula has variables `Alice`, `Bob`, and `Neighbour(p)`. The formula `forall q # has_house(q)` can be converted to `has_house(Alice) & has_house(Bob) & (forall q # has_house(Neighbour(q)))`.

A rewrite rule is embedded into the BDD. New rewrite rules can be introduced later in the formula, and rewrites can be influenced by early assumptions.
This property is illustrated in example 'assumption rewriting'.

**A basic example is provided below:**

```
Given rule:
sum p # is_person(p) -> p = Alice

Formula:
is_person(Alice)

Rewrites to:
sum p # is_person(Alice) -> Alice = Alice   [p -> Alice; summation]
is_person(Alice) -> Alice = Alice           [sum elimination]
is_person(Alice) -> true                    [(Alice = Alice) -> true; equivalence]
true -> true                                [is_person(Alice) -> true; goal/assumption]
true                                        [logical equivalence]

Alternatively: (conditional rewriting)
sum p # (p = Alice) -> (is_person(p) -> true)

Formula:
is_person(Alice)

Rewrites to:
sum p # (Alice = Alice) -> (is_person(Alice) -> true)   [p -> Alice; summation]
(Alice = Alice) -> (is_person(Alice) -> true)           [sum elimination]
(true) -> (is_person(Alice) -> true)                    [(Alice = Alice) -> true; equivalence]
(true) -> (true -> true)                                [is_person(Alice) -> true; goal/assumption]
(true) -> (true)                                        [logical equivalence]
(true)                                                  [logical equivalence]
```

**Multiple rules:**

```
Given rules:
is_person(Alice) -> true &
is_person(Bob) -> true &
is_person(Chair) -> false

Formula:
is_person(Bob)

Rewrites to:
(is_person(Alice) -> true) & (true -> true) & (is_person(Chair) -> false)   [is_person(Bob) -> true; goal/assumption]
(false -> true) & (true -> true) & (is_person(Chair) -> false)              [is_person(Alice) -> false; no match for closed rewrite rule]
(false -> true) & (true -> true) & (false -> false)                         [is_person(Chair) -> false; no match for closed rewrite rule]
(true) & (true) & (true)                                                    [logical equivalence]
(true)                                                                      [logical equivalence]
```

**Assumption rewriting:**

```
Formula:
(sum p # is_person(p) -> (p = Alice)) &
(forall p # is_person(p) => (p = Alice))

Rewrites to:
(sum p # is_person(p) -> (p = Alice)) & (forall p # is_person(p) => (p = Alice))


Formula:
(sum p # is_person(p) -> (p = Alice) | (p = Bob)) &
(forall p # is_person(p) => (p = Alice))

Rewrites to:

Formula:
(sum p # is_person(p) -> (p = Alice) | (p = Bob)) &
(exists p # is_person(p) => (p = Alice))

Rewrites to:

```

**Quantifiers:**

```
X
```

**Peano natural numbers:**

Recursive rewrite rules can be illustrated by an implementation of peanos natural numbers.
This example contains two constants: `_0` and `suc(n)`, where `suc` accepts a variable `n`.

```
Formula:
(is_nat(_0) -> true) &
(sum n # is_nat(suc(n)) -> is_nat(n)) &
is_nat(suc(suc(_0)))

Rewrites to:
assume: is_nat(suc(suc(_0)))                                                        [goal]
(is_nat(_0) -> true) & (sum n # is_nat(suc(n)) -> is_nat(n))                        [insert rules]
(false -> true) & (is_nat(suc(suc(_0))) -> is_nat(suc(_0)))                         [match n = suc(_0)]
(false -> true) & (true -> is_nat(suc(_0)))                                         [match goal]
(true) & (is_nat(suc(_0)))                                                          [rewrite elimination]
--
assume: is_nat(suc(_0))                                                             [new goal (recursive)]
(true) & ((is_nat(_0) -> true) & (sum n # is_nat(suc(n)) -> is_nat(n)))             [insert rules]
(true) & ((false -> true) & (is_nat(suc(_0)) -> is_nat(_0)))                        [match n = _0]
(true) & ((false -> true) & (true -> is_nat(_0)))                                   [match goal]
(true) & ((true) & (is_nat(_0)))                                                    [rewrite elimination]
---
assume: is_nat(_0)                                                                  [new goal (recursive)]
(true) & ((true) & ((is_nat(_0) -> true) & (sum n # is_nat(suc(n)) -> is_nat(n))))  [insert rules]
(true) & ((true) & ((true -> true) & (sum n # is_nat(suc(n)) -> is_nat(n))))        [match goal]
(true) & ((true) & ((true -> true) & (false -> is_nat(n))))                         [no match (difficult operation!)]
(true) & ((true) & ((true) & (true)))                                               [rewrite elimination]
---
(true)                                                                              [logical equivalence]
```

**Deferred evaluation:**

In some cases, rewriting only is not sufficient to reach a closed formula.
The example below shows equivalence testing on unknown variables. 
By replacing these equivalence tests with boolean variables the obtained formula can be evaluated by the SAT solver.

```
const(Alice) -> true &
const(Bob) -> true &
forall d # ((d = _0) & (d = _1)) => (_0 = _1)
(((Alice = _0) & (Alice = _1)) => (_0 = _1)) & (((Bob = _0) & (Bob = _1)) => (_0 = _1)) [forall elimination]
(((Alice = _0) & (Alice = _1)) => (_0 = _1)) & (((Bob = _0) & (Bob = _1)) => (_0 = _1)) [Alice = _0 -> a; Alice = _1 -> b; boolean variable introduction]

this needs work!

```

## Examples

### Example 1: transitivity of the `>=` operator

```
([a1,a2,a3,a4] >= [b1,b2,b3,b4] & [b1,b2,b3,b4] >= [c1,c2,c3,c4]) => [a1,a2,a3,a4] >= [c1,c2,c3,c4]
```

### Example 2: the 4 queens problem

The famous n-queens problem can be expressed efficiently in the RsBDD language.
The example below shows a 4-queens variant, which can be solved in roughly 15 milliseconds. The library contains a generator for arbitrary n-queens problems.
At this point, the largest verified problem size is n=8, which reports all solutions in less than 20 minutes on modern hardware.
The explosive nature of the problem makes n=9 an infeasable problem. Further optimizations (such as multi-processor parallellism, or vertex ordering) could decrease the run-time in the future.

```
"every row must contain exactly one queen"
[_0x0, _0x1, _0x2, _0x3] = 1 &
[_1x0, _1x1, _1x2, _1x3] = 1 &
[_2x0, _2x1, _2x2, _2x3] = 1 &
[_3x0, _3x1, _3x2, _3x3] = 1 &

"every column must contain exactly one queen"
[_0x0, _1x0, _2x0, _3x0] = 1 &
[_0x1, _1x1, _2x1, _3x1] = 1 &
[_0x2, _1x2, _2x2, _3x2] = 1 &
[_0x3, _1x3, _2x3, _3x3] = 1 & 

"every diagonal must contain at most one queen"
[_0x0] <= 1 &
[_0x1, _1x0] <= 1 &
[_0x2, _1x1, _2x0] <= 1 &
[_0x3, _1x2, _2x1, _3x0] <= 1 &
[_1x3, _2x2, _3x1] <= 1 &
[_2x3, _3x2] <= 1 &
[_3x3] <= 1 &

"the other diagonal"
[_0x3] <= 1 &
[_0x2, _1x3] <= 1 &
[_0x1, _1x2, _2x3] <= 1 &
[_0x0, _1x1, _2x2, _3x3] <= 1 &
[_1x0, _2x1, _3x2] <= 1 &
[_2x0, _3x1] <= 1 &
[_3x0] <= 1
```

## CLI Usage

```
Solver 0.2.0
Tim Beurskens
A BDD-based SAT solver

USAGE:
    rsbdd [FLAGS] [OPTIONS]

FLAGS:
    -h, --help          Prints help information
    -m, --model         use a model of the bdd as output (instead of the satisfying assignment)
    -t, --truthtable    print the truth-table to stdout
    -V, --version       Prints version information

OPTIONS:
    -e, --expect <expect>               only show true or false entries in the truth-table
    -i, --input <input>                 logic input file
    -d, --dot <show_dot>                write the bdd to a dot graphviz file
    -p, --parsetree <show_parsetree>    write the parse tree in dot format to this file
```
