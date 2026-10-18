# Examples

## Example 1: transitivity of the `>=` operator

```
([a1,a2,a3,a4] >= [b1,b2,b3,b4] & [b1,b2,b3,b4] >= [c1,c2,c3,c4]) => [a1,a2,a3,a4] >= [c1,c2,c3,c4]
```

## Example 2: the 4 queens problem

The famous n-queens problem can be expressed efficiently in the RsBDD language.
The example below shows a 4-queens variant, which can be solved in roughly 15 milliseconds. The library contains a
generator for arbitrary n-queens problems.
At this point, the largest verified problem size is n=10, which reports all solutions in less 1 minute on modern hardware.
Further optimizations (such as multi-processor parallellism, or vertex ordering) could decrease the run-time in the future.

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

The BDD, obtained with `rsbdd examples/4_queens.txt -d 4_queens.dot`, contains both solutions as the two paths to
`true`. A queen in the corner (`_0x0`) leads to `false` immediately.

![4 queens bdd](images/4_queens_bdd.svg)

## Example 3: model checking Peterson's protocol

Peterson's algorithm lets two processes share a critical section using nothing but two flags and a `turn` variable.
A process that wants to enter raises its flag and politely gives the turn to the other process. It then waits until
the other process is not interested (its flag is down) or it is its own turn. On leaving, it lowers its flag.

[examples/peterson.txt](../examples/peterson.txt) encodes the protocol as a transition system `T` over the program
counters of both processes and the turn bit, and checks four properties using fixed points:

- `mutex`: no reachable state has both processes in the critical section (least fixpoint for reachability)
- `deadlock_free`: every reachable state has a successor
- `recoverable`: from every reachable state, P0 can still reach its critical section (least fixpoint)
- `no_overtaking`: while P0 waits, P1 cannot keep moving forever on its own (greatest fixpoint)

```
let Reach := lfp R # init |
    (exists a1, a0, b1, b0, t # R & T)[a1' := a1, a0' := a0, b1' := b1, b0' := b0, t' := t];

mutex <=> forall a1, a0, b1, b0, t # Reach => -(crit0 & crit1);
...
```

```bash
rsbdd examples/peterson.txt -t -ft
```

| mutex | deadlock_free | recoverable | no_overtaking | *    |
|-------|---------------|-------------|---------------|------|
| True  | True          | True        | True          | True |

In the BDD, obtained with `rsbdd examples/peterson.txt -d peterson.dot`, the only path to `true` sets every property
to true: the protocol satisfies all four.

![peterson bdd](images/peterson_bdd.svg)

### Finding bugs

Suppose P0 is "optimized" to enter the critical section without checking the flag and turn of P1:

```diff
-    wait0 & (idle1 | -t) & crit0[a1 := a1', a0 := a0'] & keept |
+    wait0 & crit0[a1 := a1', a0 := a0'] & keept |
```

Running the checks again shows that mutual exclusion is violated:

| mutex | deadlock_free | recoverable | no_overtaking | *    |
|-------|---------------|-------------|---------------|------|
| False | True          | True        | True          | True |

To find a counterexample, replace the four properties at the end of the file by a query for the bad states:

```
Reach & crit0 & crit1
```

| a1   | a0    | b1   | b0    | t    | *    |
|------|-------|------|-------|------|------|
| True | False | True | False | True | True |

Both processes are in the critical section (`10`) while the turn belongs to P1. This state is reached when P1 starts
waiting, P0 starts waiting and hands the turn to P1, P1 enters, and P0 barges in after it.

Other mistakes are found the same way. If neither process looks at `turn`, so a process only enters when the other is
idle, `deadlock_free` and `recoverable` fail: both processes can wait for each other forever. If P1 keeps the turn to
itself instead of handing it to P0, both `mutex` and `no_overtaking` fail.

## Example 4: a simplified Einstein riddle

[examples/einstein.txt](../examples/einstein.txt) solves Einstein's riddle using rewrite rules: five houses, and five
kinds of values (nationality, color, beverage, cigar and pet). Its BDD is too large to read, so
[examples/einstein_small.txt](../examples/einstein_small.txt) asks the same question, "who owns the fish?", for three
houses with a nationality, a color and a pet:

1) the Brit lives in the red house
2) the Swede keeps the dog
3) the green house is on the left of the blue house
4) the Dane lives in the first house
5) the fish does not live in the green house

```
house(H1) -> true;
...
left(H1, H2) -> true;
left(H2, H3) -> true;
...
forall h # house(h) => [keeps(h, Dog), keeps(h, Cat), keeps(h, Fish)] = 1;
forall p # pet(p) => [keeps(H1, p), keeps(H2, p), keeps(H3, p)] = 1;
...
forall h # house(h) => (has_color(h, Blue) => exists g # left(g, h) & has_color(g, Green));
has_nationality(H1, Dane);
forall h # house(h) => (has_color(h, Green) => -keeps(h, Fish))
```

The BDD, obtained with `rsbdd examples/einstein_small.txt -d einstein_small.dot`, has a single path to `true`, so the
solution is unique. Following its `T` edges: the Dane lives in the green first house with the cat, the Swede in the
blue middle house with the dog, and the Brit in the red last house with the fish.

![simplified einstein bdd](images/einstein_small_bdd.svg)
