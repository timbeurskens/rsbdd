# CLI Usage

## rsbdd

```
A BDD-based SAT solver

Usage: rsbdd [OPTIONS] [FILE]

Arguments:
  [FILE]  The input file containing a logic formula in rsbdd format, or '-' for stdin

Options:
  -h, --help     Print help
  -V, --version  Print version

Input:
  -e, --evaluate <EVALUATE>  Parse the formula as string
  -o, --ordering <FILE>      Read a custom variable ordering from file

Output:
  -t, --truthtable        Print the truth table to stdout
  -v, --vars              Print all satisfying variables leading to a truth value
  -d, --dot <FILE>        Write the bdd to a dot graphviz file ('-' for stdout)
  -p, --parsetree <FILE>  Write the parse tree in dot format to the specified file ('-' for stdout)
  -w, --rewritten <FILE>  Write the formula after applying rewrite rules to the specified file ('-' for stdout)
  -r, --export-ordering   Export the automatically derived ordering to stdout

Filtering:
  -m, --model                            Compute a single satisfying model as output
  -f, --filter <true|false|any>          Only show true or false entries in the output [default: true without output options, any
                                         otherwise]
  -c, --retain-choices <true|false|any>  Only retain choice variables when filtering [default: Any]

Benchmarking:
  -b, --benchmark <N>  Repeat the solving process n times for more accurate performance reports
  -g, --plot           Use GNUPlot to plot the runtime distribution

Solver:
  -l, --memory-limit <MiB>  Limit the memory used by the solver, nodes exceeding the limit are paged to disk [default: 75% of available
                            memory]
  -P, --prove               Prove the formula in first-order logic by superposition, with the rewrite rules as axioms, and print the
                            proof

Without output options, the satisfying rows of the truth table are printed.
Exit status: 10 if the formula is satisfiable, 20 if it is unsatisfiable, 1 or 2 on errors.
With --prove: 10 if the formula is a theorem, 20 if it is not, 30 if no proof was found.

Examples:
  rsbdd examples/4_queens.txt
  rsbdd -e "a & -b" -t
  rsbdd examples/peano_puzzle.txt -w -
  rsbdd examples/fol/dreadbury.txt --prove

```

## max_clique_gen

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

## random_graph_gen

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

## n_queens_gen

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

## sudoku_gen

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
