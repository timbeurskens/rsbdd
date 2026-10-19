# RsBDD

[![Rust](https://github.com/timbeurskens/rsbdd/actions/workflows/rust.yml/badge.svg)](https://github.com/timbeurskens/rsbdd/actions/workflows/rust.yml)

_Solving satisfiability problems in Rust_

RsBDD is a SAT solver based on binary decision diagrams (BDDs). Problems are written in a small logic language with
quantifiers, counting constraints, fixed points, definitions and substitution. The solver reports every satisfying
assignment, a single model, or the BDD itself. Rewrite rules over terms such as `suc(_0)` or `next(H1, H2)` describe
problems over domains other than booleans, and turn the solver into a theorem prover by term rewriting, completion and induction.
The package also contains generators for n-queens, sudoku, graph coloring and max-clique problems.

```bash
cargo install --bins --path .
rsbdd -e "a & -b" -t
```

## Table of contents

1. [Installation](docs/installation.md)
2. [Syntax](docs/syntax.md)
   - [Operators and precedence](docs/syntax.md#binary-operators)
   - [Quantifiers](docs/syntax.md#quantifiers)
   - [Counting](docs/syntax.md#counting)
   - [Fixed points](docs/syntax.md#fixed-points)
   - [Definitions and constraint lists](docs/syntax.md#definitions-and-constraint-lists)
   - [Substitution](docs/syntax.md#substitution)
3. [Rewrite rules](docs/rewriting.md)
   - [Usage](docs/rewriting.md#usage)
   - [Combining rewrite rules and fixed points](docs/rewriting.md#combining-rewrite-rules-and-fixed-points)
4. [Theorem proving](docs/theorem_proving.md)
   - [Theorem proving by term rewriting](docs/theorem_proving.md#theorem-proving-by-term-rewriting)
   - [Completion](docs/theorem_proving.md#completion)
   - [Proofs by induction](docs/theorem_proving.md#proofs-by-induction)
   - [First-order proofs](docs/theorem_proving.md#first-order-proofs)
5. [Finite mode and first-order proofs](docs/finite_and_first_order.md)
   - [Supported syntax](docs/finite_and_first_order.md#supported-syntax)
   - [How `--prove` reads the rules](docs/finite_and_first_order.md#how---prove-reads-the-rules)
   - [Unique names](docs/finite_and_first_order.md#unique-names)
   - [Constructs evaluated as in finite mode](docs/finite_and_first_order.md#constructs-evaluated-as-in-finite-mode)
   - [Remaining differences](docs/finite_and_first_order.md#remaining-differences)
6. [Examples](docs/examples.md): transitivity, the 4 queens problem, model checking Peterson's protocol, a simplified Einstein riddle
7. [CLI usage](docs/cli.md): `rsbdd`, `max_clique_gen`, `random_graph_gen`, `n_queens_gen`, `sudoku_gen`
