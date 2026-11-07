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

The full BDD tests every square, most of them only to rule out a queen. `-c true` retains only the choices that can be
true: a variable whose `T` edge only leads to `false` is omitted (and reported on stderr as `omitted choice ...`), so
an omitted square has no queen. The BDD, obtained with `rsbdd examples/4_queens.txt -c true -d 4_queens.dot`, contains
both solutions as the two paths to `true`, and only tests the squares that hold a queen in one of them.

![4 queens bdd](images/4_queens_bdd.svg)

## Example 3: model checking Peterson's protocol

Peterson's algorithm lets two processes share a critical section using nothing but two flags and a `turn` variable.
A process that wants to enter raises its flag and politely gives the turn to the other process. It then waits until
the other process is not interested (its flag is down) or it is its own turn. On leaving, it lowers its flag.

[examples/peterson.txt](../examples/peterson.txt) encodes the protocol as a transition relation `step(u, v)` between
states `u` and `v`. The program counter `pc(w, p, l)` and the turn `turn(w)` of a state `w` are functions, and the
substitution `[pc(Cur, P0, _) := pc(Prev, P0, _), ...]` renames the current state to its predecessor. Four properties
are checked using fixed points:

- `mutex`: no reachable state has both processes in the critical section (least fixpoint for reachability)
- `deadlock_free`: every reachable state has a successor
- `recoverable`: from every reachable state, P0 can still reach its critical section (least fixpoint)
- `no_overtaking`: while P0 waits, P1 cannot keep moving forever on its own (greatest fixpoint)

```
let Reach := lfp R # init |
    exists pc(Prev, _, _), turn(Prev) #
        R[pc(Cur, P0, _) := pc(Prev, P0, _), pc(Cur, P1, _) := pc(Prev, P1, _), turn(Cur) := turn(Prev)] & step(Prev, Cur);

mutex <=> forall pc(Cur, _, _), turn(Cur) # Reach => -(is_at(Cur, P0, Crit) & is_at(Cur, P1, Crit));
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
-    is_at(u, P0, Wait) & (is_at(u, P1, Idle) | -turn(u)) & is_at(v, P0, Crit) & keept(u, v) |
+    is_at(u, P0, Wait) & is_at(v, P0, Crit) & keept(u, v) |
```

Running the checks again shows that mutual exclusion is violated:

| mutex | deadlock_free | recoverable | no_overtaking | *    |
|-------|---------------|-------------|---------------|------|
| False | True          | True        | True          | True |

To find a counterexample, replace the four properties at the end of the file by a query for the bad states:

```
Reach & is_at(Cur, P0, Crit) & is_at(Cur, P1, Crit)
```

| pc(Cur, P0, Idle) | pc(Cur, P0, Wait) | pc(Cur, P0, Crit) | pc(Cur, P1, Idle) | pc(Cur, P1, Wait) | pc(Cur, P1, Crit) | turn(Cur) | *    |
|-------------------|-------------------|-------------------|-------------------|-------------------|-------------------|-----------|------|
| False             | False             | True              | False             | False             | True              | True      | True |

Both processes are in the critical section while the turn belongs to P1. This state is reached when P1 starts
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

The BDD, obtained with `rsbdd examples/einstein_small.txt -c true -d einstein_small.dot`, has a single path to `true`,
so the solution is unique. Following its `T` edges: the Dane lives in the green first house with the cat, the Swede in the
blue middle house with the dog, and the Brit in the red last house with the fish.

![simplified einstein bdd](images/einstein_small_bdd.svg)

## Example 5: knights and knaves

On Raymond Smullyan's island, knights always tell the truth and knaves always lie. A visitor asks A whether he is a
knight or a knave, but cannot hear the answer. B says "A said that he is a knave", and C says "Don't believe B, he is
lying!".

[examples/knights_knaves.txt](../examples/knights_knaves.txt) gives every person a rule for their statement. A person
is a knight exactly when their statement is true. A's unheard answer is the unknown `claims_knave(A)`:

```
person(A) -> true;
person(B) -> true;
person(C) -> true;

statement(A) -> if claims_knave(A) then -knight(A) else knight(A);
statement(B) -> claims_knave(A);
statement(C) -> -knight(B);

forall p # person(p) => (knight(p) <=> statement(p))
```

```bash
rsbdd examples/knights_knaves.txt -t -ft
```

| knight(A) | claims_knave(A) | knight(B) | knight(C) | *    |
|-----------|-----------------|-----------|-----------|------|
| Any       | False           | False     | True      | True |

Nobody can say "I am a knave": a knight would be lying, and a knave would be telling the truth. So B is a knave and C
is a knight. A must have answered "I am a knight", which both knights and knaves can say, so A's type stays `Any`.

The BDD, obtained with `rsbdd examples/knights_knaves.txt -c true -d knights_knaves.dot`, omits the choices
`knight(B)` and `claims_knave(A)`, which must be false. Only `knight(C)` remains; `knight(A)` is not tested at all:

![knights and knaves bdd](images/knights_knaves_bdd.svg)

## Example 6: wolf, goat and cabbage

A farmer wants to cross a river with a wolf, a goat and a cabbage. The boat only has room for the farmer and one of
the three. Left alone, the wolf eats the goat, and the goat eats the cabbage.

[examples/wolf_goat_cabbage.txt](../examples/wolf_goat_cabbage.txt) gives every actor one bit for its bank (`false` is
the left bank). Like [examples/reachability.txt](../examples/reachability.txt), it computes the reachable states with a
least fixpoint over the current state `pf, pw, pg, pc` and the next state `f, w, g, c`:

```
let init := -f & -w & -g & -c;
let safe := -((w <=> g) & (f ^ g)) & -((g <=> c) & (f ^ g));

let T :=
    (f ^ pf) &
    ((w ^ pw) => (pw <=> pf)) &
    ((g ^ pg) => (pg <=> pf)) &
    ((c ^ pc) => (pc <=> pf)) &
    [w ^ pw, g ^ pg, c ^ pc] <= 1 &
    safe;

lfp R # init | exists pf, pw, pg, pc # R[f := pf, w := pw, g := pg, c := pc] & T
```

```bash
rsbdd examples/wolf_goat_cabbage.txt -t -ft
```

| f     | w     | g     | c     | *    |
|-------|-------|-------|-------|------|
| False | False | False | Any   | True |
| False | False | True  | False | True |
| False | True  | False | Any   | True |
| True  | False | True  | Any   | True |
| True  | True  | False | True  | True |
| True  | True  | True  | Any   | True |

The table covers 10 of the 16 states, including `f & w & g & c`: everything ends up on the right bank, so the puzzle is
solvable. The fixpoint does not record which crossings lead there. One of the two 7-move solutions takes the goat over,
returns, takes the wolf over, brings the goat back, takes the cabbage over, returns and takes the goat over.

The BDD, obtained with `rsbdd examples/wolf_goat_cabbage.txt -d wolf_goat_cabbage.dot`:

![wolf goat cabbage bdd](images/wolf_goat_cabbage_bdd.svg)

## Example 7: Cheryl's birthday

Cheryl gives Albert and Bernard a list of 10 possible dates for her birthday: May 15, 16, 19, June 17, 18, July 14, 16
and August 14, 15, 17. She tells Albert only the month and Bernard only the day.

1) Albert: I don't know when Cheryl's birthday is, but I know that Bernard does not know too.
2) Bernard: At first I didn't know when Cheryl's birthday is, but I know now.
3) Albert: Then I also know when Cheryl's birthday is.

In [examples/cheryl.txt](../examples/cheryl.txt), the month and day of a date `w` are functions, interpreted by the
solver: like `val(v, n)` in [examples/peano_solve.txt](../examples/peano_solve.txt), the unknowns `month(w, m)` and
`day(w, d)` state that the month of `w` is `m` and the day of `w` is `d`. Cheryl's birthday is the date `Cheryl`.

Someone knows the birthday `w` if every candidate date they cannot tell apart from `w` is `w` itself. The other
candidate is the date `other(w)`, and "every candidate" is a quantifier over the interpretation of its month and day:
`forall month(other(w), _), day(other(w), _)`. Every statement keeps the candidate dates for which it is true.

```
date(May, D15) -> true;
...
date(August, D17) -> true;

sum w # candidate(List, w) ->
    exists m, d # date(m, d) & month(w, m) & day(w, d) &
        forall m2, d2 # date(m2, d2) => (month(w, m2) => m2 = m) & (day(w, d2) => d2 = d);

sum w, v # same_month(w, v) -> exists m, d # date(m, d) & month(w, m) & month(v, m);
sum w, v # same_day(w, v) -> exists m, d # date(m, d) & day(w, d) & day(v, d);
sum w, v # indistinguishable(Albert, w, v) -> same_month(w, v);
sum w, v # indistinguishable(Bernard, w, v) -> same_day(w, v);

sum p, s, w # knows(p, s, w) ->
    forall month(other(w), _), day(other(w), _) #
        candidate(s, other(w)) & indistinguishable(p, w, other(w)) => same_month(w, other(w)) & same_day(w, other(w));

sum w # candidate(S1, w) ->
    candidate(List, w) & -knows(Albert, List, w) &
    forall month(other(w), _), day(other(w), _) #
        candidate(List, other(w)) & indistinguishable(Albert, w, other(w)) => -knows(Bernard, List, other(w));
sum w # candidate(S2, w) -> candidate(S1, w) & -knows(Bernard, List, w) & knows(Bernard, S1, w);
sum w # candidate(S3, w) -> candidate(S2, w) & knows(Albert, S2, w);

candidate(S3, Cheryl)
```

The rewriter only grounds the rules; the knowledge of Albert and Bernard is computed by quantification over the BDD.

```bash
rsbdd examples/cheryl.txt -t -ft
```

The truth table has a single row, in which only `month(Cheryl, July)` and `day(Cheryl, D16)` are true: Cheryl's
birthday is July 16.

The BDD, obtained with `rsbdd examples/cheryl.txt -c true -d cheryl.dot`, omits the months and days that cannot be
the birthday, leaving a single choice:

![cheryl bdd](images/cheryl_bdd.svg)
