# RsBDD

[![Rust](https://github.com/timbeurskens/rsbdd/actions/workflows/rust.yml/badge.svg)](https://github.com/timbeurskens/rsbdd/actions/workflows/rust.yml)

_Solving satisfiability problems in Rust_

## Installation

1) Make sure to install the [Rust toolchain](https://www.rust-lang.org/tools/install).

2) Clone the latest version of this repository:

```
$ git clone git@github.com:timbeurskens/rsbdd.git
```

3) Build and install the RsBDD tools:

```
$ cd rsbdd
$ cargo install --bins --path .
```

The following tools will be available after installing the RsBDD package:

- `max_clique_gen`
- `n_queens_gen`
- `random_graph_gen`
- `rsbdd`
- `sudoku_gen`

## Syntax

### Comments

Characters contained within "..." (excluding the " char itself) are regarded as comments and can be placed at any point
in the formula.

### Constants

The most basic building blocks of the syntax are 'variables' and 'constants'. A constant can be either 'true' or '
false'. A variable can accept either a 'true' or 'false' value after evaluation depending on its environment.

```
true
false
```

### Variables

A variable is a single word starting with a non-digit character. Examples of good variable names are:

```
a
a'
alpha
_x
a1
hello_world
```

### Negation

A variable, constant, or sub-formula can be negated using the negation operator. This operator can be expressed by
either `!`, `-`, or `not`.

```
not true
-false
!variable
```

### Binary operators

RsBDD supports the most common, and some uncommon binary operators, such as conjunction, disjunction, implication and
bi-implication.

Most operators have a symbolic and textual representation, e.g. `and` or `&`.

| Operator           | Option 1          | Option 2 |
|--------------------|-------------------|----------|
| Conjunction        | `and`             | `&`      |
| Disjunction        | `or`              | `\|`     |
| Implication        | `implies` or `in` | `=>`     |
| Bi-implication     | `iff` or `eq`     | `<=>`    |
| Exlusive or        | `xor`             | `^`      |
| Joint denial       | `nor`             | N.A.     |
| Alternative denial | `nand`            | N.A.     |

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

A simplification of a common expression `(a => b) & ((!a) => c)` can be made using the ternary if-then-else (ite)
operator.

```
if a then b else c
if exists a # a <=> b then b <=> c else false | c
```

### Quantifiers

The RsBDD supports universal and existential quantification using the `exists` and `forall`/`all`
keywords: `{forall|exists} var_1, var_2, .., var_n # {subformula}`

```
forall a # true
forall a # a | b
forall a, b # exists c # (c | a) & (c | b)
```

### Counting

For some problems it can be beneficial to express properties relating to the number of true or false variables, e.g. "at
least 2 of the 4 properties must hold".

The counting operator (`[]`) in combination with five new equality and inequality operators (`=`, `<=`, `>=`, `<`, `>`)
can be used to concisely express these properties.

_Note:_ like most operators, the counting operator can be expressed using logic primitives, but this operator simplifies
the expression significantly.

A counting comparison can either be made by comparing a set of expressions to a given constant, or an other set of
expressions.

```
"exactly one of a, b, and c holds"
[a, b, c] = 1

"there are strictly less true expressions in a, b, c than d, e, f"
[a, b, c] < [d, e, f]
```

Counting comparison also allows us to specify optimization problems.
Example: the max-clique problem can be described as a clique problem, such that
for all satisfiable cliques, the reported result is the largest.

```
-(a & f) &
-(a & g) &

-(b & d) &
-(b & e) &

-(c & e) &
-(c & g) &

forall _a,_b,_c,_d,_e,_f,_g # (
    -(_a & _f) &
    -(_a & _g) &
    -(_b & _d) &
    -(_b & _e) &
    -(_c & _e) &
    -(_c & _g)
) => [a,b,c,d,e,f,g] >= [_a,_b,_c,_d,_e,_f,_g]
```

### Fixed points

The rsbdd language supports least-fixpoint (`lfp` / `mu`) and greatest-fixpoint (`gfp` / `nu`) operations to find a
respectively minimal or maximal solution by repeatedly applying a given transformer function until the solution is
stable.

Only monotonic transformer functions are guaranteed to terminate. Termination of fixed point operations are not checked
and will run indefinatedly if not handled correctly.

Its basic properties are defined as follows.

```
gfp X # X           <=> true
lfp X # X           <=> false

nu X # ...          <=> gfp X # ...
mu X # ...          <=> lfp X # ...

gfp/lfp X # a       <=> a
gfp/lfp X # true    <=> true
gfp/lfp X # false   <=> false
```

### Parse-tree display

Adding the `-p {path}` argument to `rsbdd` constructs a graphviz graph of the parse-tree. This can be used to for
introspection of the intended formula, or for reporting purposes. An example of the parse-tree output
for `exists b,c # a | (b ^ c)` is displayed below.

![parse tree](docs/images/parsetree.svg)

### Experimental and/or upcoming features

Currently the RsBDD language relies heavily on logical primitives. Integer arithmetic could be expressed by manually
introducing the primitive 'bits' of a number. Rewrite rules could significantly simplify this process by introducting
domains other than boolean variables. Embedding rewrite rules in the BDD could prove to be a challenge.

## Examples

### Example 1: transitivity of the `>=` operator

```
([a1,a2,a3,a4] >= [b1,b2,b3,b4] & [b1,b2,b3,b4] >= [c1,c2,c3,c4]) => [a1,a2,a3,a4] >= [c1,c2,c3,c4]
```

### Example 2: the 4 queens problem

The famous n-queens problem can be expressed efficiently in the RsBDD language.
The example below shows a 4-queens variant, which can be solved in roughly 15 milliseconds. The library contains a
generator for arbitrary n-queens problems.
At this point, the largest verified problem size is n=8, which reports all solutions in less than 20 minutes on modern
hardware.
The explosive nature of the problem makes n=9 an infeasable problem. Further optimizations (such as multi-processor
parallellism, or vertex ordering) could decrease the run-time in the future.

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

Running this example with the following arguments yields a truth-table showing the queen configuration(s) on a 4x4 chess
board.

```bash
rsbdd -i examples/4_queens.txt -t -ft
```

| _0x0  | _0x1  | _0x2  | _0x3  | _1x0  | _1x1  | _1x2  | _1x3  | _2x0  | _2x1  | _2x2  | _2x3  | _3x0  | _3x1  | _3x2  | _3x3  | *    |
|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|------|
| False | False | True  | False | True  | False | False | False | False | False | False | True  | False | True  | False | False | True |
| False | True  | False | False | False | False | False | True  | True  | False | False | False | False | False | True  | False | True |

## CLI Usage

### rsbdd

```
A BDD-based SAT solver

Usage: rsbdd [OPTIONS] [FILE]

Arguments:
  [FILE]  The input file containing a logic formula in rsbdd format

Options:
  -p, --parsetree <PARSETREE>            Write the parse tree in dot format to the specified file
  -t, --truthtable                       Print the truth table to stdout
  -d, --dot <DOT>                        Write the bdd to a dot graphviz file
  -m, --model                            Compute a single satisfying model as output
  -v, --vars                             Print all satisfying variables leading to a truth value
  -f, --filter <FILTER>                  Only show true or false entries in the output [default: Any]
  -c, --retain-choices <RETAIN_CHOICES>  Only retain choice variables when filtering [default: Any]
  -b, --benchmark <N>                    Repeat the solving process n times for more accurate performance reports
  -g, --plot                             Use GNUPlot to plot the runtime distribution
  -e, --evaluate <EVALUATE>              Parse the formula as string
  -o, --ordering <ORDERING>              Read a custom variable ordering from file
  -r, --export-ordering                  Export the automatically derived ordering to stdout
  -h, --help                             Print help
  -V, --version                          Print version

```

### max_clique_gen

```
Converts a graph into a max-clique specification

Usage: max_clique_gen [OPTIONS] [INPUT] [OUTPUT]

Arguments:
  [INPUT]   Input file graph in csv edge-list format
  [OUTPUT]  The output rsbdd file

Options:
  -u, --undirected  Use undirected edges (test for both directions in the set-complement operation)
  -a, --all         Construct a satisfiable formula for all cliques
  -h, --help        Print help
  -V, --version     Print version

```

### random_graph_gen

```
Generates a random edge list formatted graph

Usage: random_graph_gen [OPTIONS] [VERTICES] [EDGES]

Arguments:
  [VERTICES]  The number of vertices in the output graph
  [EDGES]     The number of edges in the output graph

Options:
  -o, --output <FILE>   The output filename (or stdout if not provided)
  -u, --undirected      Use undirected edges (test for both directions in the set-complement operation)
      --complete        Construct a complete graph
  -d, --dot             Output in dot (GraphViz) format
      --convert <FILE>  If this argument is provided, the provided edge-list will be used to generate a graph
  -c, --colors <N>      Generate a graph-coloring problem with N colors
  -h, --help            Print help
  -V, --version         Print version

```

### n_queens_gen

```
Generates n-queen formulae for the SAT solver

Usage: n_queens_gen [OPTIONS] [OUTPUT]

Arguments:
  [OUTPUT]  The output rsbdd file

Options:
  -n, --queens <QUEENS>  The number of queens [default: 4]
  -h, --help             Print help
  -V, --version          Print version

```

### sudoku_gen

```
Generates a random edge list formatted graph

Usage: sudoku_gen [OPTIONS] [INPUT] [OUTPUT]

Arguments:
  [INPUT]   The input sudoku file
  [OUTPUT]  The output rsbdd file

Options:
  -r, --root <N>  The root value of the puzzle. Typically the square root of the largest possible number [default: 3]
  -h, --help      Print help
  -V, --version   Print version

```
