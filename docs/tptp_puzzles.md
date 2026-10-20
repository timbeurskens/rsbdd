# The TPTP puzzles

This chapter builds on [Theorem proving](theorem_proving.md) and
[Finite mode and first-order proofs](finite_and_first_order.md).

The [TPTP library](https://tptp.org) (v9.3.1) has 164 problems in its puzzle domain, PUZ. This chapter describes how
`rsbdd --prove` does on the 150 problems in first-order form (`fof`) or clause normal form (`cnf`), and which
limitations of rsbdd keep it from solving the others. The remaining problems are higher-order (`thf`) or typed
(`tff`), which rsbdd does not read.

Several of the puzzles are also examples, written by hand:

| TPTP   | Puzzle                         | Example                                                                |
|--------|--------------------------------|------------------------------------------------------------------------|
| PUZ001 | Dreadbury Mansion              | [fol/dreadbury.txt](../examples/fol/dreadbury.txt), [dreadbury.txt](../examples/dreadbury.txt) |
| PUZ002 | The animals puzzle             | [fol/animals.txt](../examples/fol/animals.txt)                         |
| PUZ003 | The barber puzzle              | [fol/barber.txt](../examples/fol/barber.txt)                           |
| PUZ005 | The lion and the unicorn       | [lion_unicorn.txt](../examples/lion_unicorn.txt)                       |
| PUZ010 | Who owns the zebra?            | [einstein.txt](../examples/einstein.txt)                               |
| PUZ019 | The jobs puzzle                | [jobs.txt](../examples/jobs.txt)                                       |
| PUZ031 | Schubert's steamroller         | [fol/steamroller.txt](../examples/fol/steamroller.txt)                 |
| PUZ034 | N queens                       | [4_queens.txt](../examples/4_queens.txt)                               |
| PUZ047 | Wolf, goat and cabbage         | [wolf_goat_cabbage.txt](../examples/wolf_goat_cabbage.txt)             |
| PUZ056 | Tower of Hanoi                 | [tower_of_hanoi.txt](../examples/tower_of_hanoi.txt)                   |
| PUZ164 | A loop from 0 to 0             | [term_reachability.txt](../examples/term_reachability.txt)             |

## Method

Every problem was translated mechanically to an rsbdd formula and run with `--prove` and a time limit of 30 seconds:

- A clause becomes `forall` over its variables; `fof` formulas map one-to-one onto the rsbdd operators, with
  `a != b` as `-(a = b)` and `<~>` as `-(a <=> b)`.
- The formula is `(axioms) => (conjectures)`, or `(clauses) => false` for a problem without a conjecture. There are
  no rewrite rules: every axiom is part of the formula.
- TPTP variables become rsbdd variables, and names that are rsbdd keywords, such as `or` and `eq`, are renamed.

As the axioms are part of the formula, the constants of a problem are not covered by the axioms of unique names
unless they are only used in terms (see [Unique names](finite_and_first_order.md#unique-names)).

## Results

| TPTP status                                    | Problems | `theorem` | `not a theorem` | No answer in 30 s |
|------------------------------------------------|---------:|----------:|----------------:|------------------:|
| Theorem or Unsatisfiable                       |      101 |        68 |               0 |                33 |
| Satisfiable, CounterSatisfiable or Unknown     |       49 |         0 |              13 |                36 |

There were no wrong answers. Of the 68 proofs, 63 took under 5 seconds (with eight problems running in parallel).

## Provable problems without a proof

The 33 provable problems without a proof fall into four groups.

**Finite puzzles in clause form.** Who owns the zebra? (PUZ010), The houses (PUZ017), The jobs puzzle (PUZ019) and
4 queens (PUZ034-1.004) are finite assignment problems. The clause form encodes "every value belongs to exactly one
person" with explicit inequality axioms and large disjunctions, and superposition explores the case splits blindly.
These puzzles are solved immediately in finite mode, where the BDD solver handles the case splits and `[..] = 1`
states the counting constraints directly; see [einstein.txt](../examples/einstein.txt),
[jobs.txt](../examples/jobs.txt) and [4_queens.txt](../examples/4_queens.txt).

**Search through states.** Quo vadis (PUZ039, PUZ042, PUZ050), the Rubik's cube (PUZ037-3) and the Towers of Hanoi
with k discs (PUZ056-2.k, PUZ056-10.k) encode a state as a term and a move as a clause, and the proof is a long
chain of moves. Each step needs an inference between the current state and a move clause. As the prover has no term
indexing, every inference scans all clauses, and the chains are too long. PUZ056-1, the Tower of Hanoi with three
discs, is small but describes the pins by an associative and commutative operator `nextto`, which needs
AC-unification.

**Large ground problems.** The Wumpus world (PUZ073, PUZ074, PUZ076) has about 5,000 unit formulas, and the mutilated
checkerboard (PUZ062-1, PUZ064-1) about 2,700 clauses. These are easy for a prover with indexing, but rsbdd compares
every clause with every other clause.

**Small problems.** Knights and knaves #36 (PUZ035-7, 14 clauses), the mutilated checkerboard (PUZ062-2, PUZ064-2),
the Wumpus world (PUZ077), Leo the liar (PUZ078) and the symmetry of N queens (PUZ133+1, PUZ133+2) have fewer than
30 clauses or formulas. Why the search does not finish has not been analysed.

## Satisfiable problems

`--prove` proves theorems, but it cannot show that a set of axioms has a model. On a satisfiable problem the best it
can do is `not a theorem`, after it has saturated the clauses. This happened for 13 of the 49 problems, all of them
small. The others are:

- Sudokus (PUZ065 to PUZ072, PUZ079, PUZ080, PUZ138): finding the solution is a model search, which finite mode
  does well; see [sudoku.txt](../examples/sudoku.txt).
- Unreachable states (Quo vadis PUZ041, PUZ048, PUZ049, PUZ051, PUZ059, the Rubik's cube PUZ052, PUZ053, the
  fifteen-puzzle PUZ055 and the Tower of Hanoi PUZ057, PUZ058): showing that no sequence of moves reaches a state
  needs an invariant or a finite model of the axioms. Saturation with superposition does not terminate on these.
- Coverings of the mutilated checkerboard (PUZ015), The interns (PUZ018-2), 3 queens (PUZ034-1.003) and the
  symmetry of N queens (PUZ133+3).

## PUZ164: a loop from 0 to 0

PUZ164 is a term rewriting system of Arts and Giesl that searches a graph for a path, and asks whether the loop
0 -> 1 -> 2 -> 3 -> 0 exists. With the equations as term rules, as in
[term_reachability.txt](../examples/term_reachability.txt), finite mode rewrites the goal to `T` immediately.

Writing it in rsbdd revealed a bug, which is now fixed. A symbol that only occurred as the head of the right-hand
side of a term rule, such as `if_reach_2` in

```
sum x, y, u, v, i, h # if_reach_1(T, x, y, edge(u, v, i), h) -> if_reach_2(equal(y, v), x, y, edge(u, v, i), h);
```

was not recognised as a function symbol. Its rules were read as rules of a predicate and never applied to the term,
so the goal rewrote to `false`. The smallest case was `sum x # f(x) -> h(x); h(A) -> B; f(A) = B`, which was false
in finite mode and `not a theorem` with `--prove`.

`--prove` still finds no proof of PUZ164 within 60 seconds. The reason has not been analysed.

## Limitations of rsbdd

The analysis shows these limitations, roughly from the easiest to the hardest to remove:

- **Keywords as symbols.** `eq`, `or`, `and`, `not`, `in`, `all`, `any`, `if`, `sum` and the other keywords cannot
  be used as function or predicate names, and `true` and `false` cannot be used as terms. TPTP problems use them,
  so they have to be renamed.
- **Rules for constants.** A rule needs an application on its left-hand side, so `g -> edge(..)` is rejected, and a
  constant that names a term has to be written out.
- **TPTP input.** rsbdd does not read TPTP files, so the problems above were translated by an external script.
- **Answers.** Some puzzles ask for values: PUZ035-7 asks what the asked person and the other person are, given the
  answer. `--prove` reports `theorem` with the proof, but does not extract the values of the existential variables.
- **Model finding.** `--prove` searches for a countermodel as Mace4 does, up to 16 elements or a million BDD nodes.
  The BDD of a binary function grows quickly with the domain, so without the flattening and symmetry breaking of
  Paradox, models of more than three or four elements with such functions are out of reach.
- **Indexing.** Without term indexing, large ground problems and long chains of moves take too long. This is the
  main gap with provers such as E or Vampire.
- **AC-unification.** Problems with associative and commutative operators, such as PUZ056-1, need unification modulo
  AC.
