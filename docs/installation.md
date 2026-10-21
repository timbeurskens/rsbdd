# Installation

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

- `maxclique2rsbdd`
- `nqueens2rsbdd`
- `random_graph_gen`
- `rsbdd`
- `sudoku2rsbdd`

`tptp2rsbdd` is installed separately:

```
$ cargo install --path tptp2rsbdd
```
