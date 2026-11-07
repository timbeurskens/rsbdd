"""
Turns a model of examples/hardest_logic_puzzle.txt into readable questions.

Every question in the model is a truth table: true_in(q, s) is the truth of question q in situation s.
For every question, the shortest formula over the facts 'X is True/False/Random' and 'da means yes' that matches
the table is printed. Situations marked * (don't care) are ignored. If no short formula matches, the question lists
the situations in which it is true.

Usage:
    rsbdd examples/hardest_logic_puzzle.txt -m -v | python3 examples/hardest_logic_puzzle.py
"""
import sys, itertools
txt = (open(sys.argv[1]) if len(sys.argv) > 1 else sys.stdin).read().strip()
def split0(s):
    out, d, cur = [], 0, ''
    for ch in s:
        d += ch == '('; d -= ch == ')'
        if ch == ',' and d == 0: out.append(cur.strip()); cur = ''
        else: cur += ch
    return [x for x in out + [cur.strip()] if x]
items = split0(txt)
true_vars = {i for i in items if not i.endswith('*')}
dc = {i[:-1] for i in items if i.endswith('*')}
roles = ['True', 'False', 'Random']
sits = [(p, m) for p in itertools.permutations(roles) for m in ['Yes', 'No']]
def sname(p, m): return f'w({p[0]}, {p[1]}, {p[2]}, {m})'
# atomic facts about a situation
atoms = [(f'{g} is {r}', (lambda i, r: lambda p, m: p[i] == r)('ABC'.index(g), r)) for g in 'ABC' for r in roles]
atoms.append(('da means yes', lambda p, m: m == 'Yes'))
def node(path):
    n = 'Q1'
    for a in path: n = f'{a}({n})'
    return n
def table(q):
    t = {}
    for p, m in sits:
        v = f'true_in({q}, {sname(p, m)})'
        if v in dc: continue
        t[(p, m)] = v in true_vars
    return t
def find(t):
    # 1) single literal, 2) chain of 'iff' over 2-3 atoms, 3) conjunction / disjunction of 2 literals
    for k in (1, 2, 3):
        for combo in itertools.combinations(atoms, k):
            for neg in (False, True):
                f = lambda p, m: (sum(not a[1](p, m) for a in combo) % 2 == 0) != neg
                if all(f(p, m) == v for (p, m), v in t.items()):
                    s = ' if and only if '.join(a[0] for a in combo) if k > 1 else combo[0][0]
                    return ('it is not the case that ' + s) if neg else s
    for (a, b) in itertools.combinations(atoms, 2):
        for na, nb in itertools.product((False, True), repeat=2):
            for op, name in ((all, 'and'), (any, 'or')):
                f = lambda p, m: op([a[1](p, m) != na, b[1](p, m) != nb])
                if all(f(p, m) == v for (p, m), v in t.items()):
                    lit = lambda x, n: ('not ' if n else '') + x[0]
                    return f'({lit(a, na)}) {name} ({lit(b, nb)})'
    trues = [f'(A is {p[0]}, B is {p[1]}, C is {p[2]}, da means {m.lower()})' for (p, m), v in t.items() if v]
    return ' or '.join(trues) or 'false'
def walk(path):
    q = node(path)
    if len(path) == 3:
        st = {g: [r for r in roles if f'states({q}, {g}, {r})' in true_vars] for g in 'ABC'}
        print('    ' * 3 + f'-> ' + ', '.join(f'{g} is {"/".join(v) or "?"}' for g, v in st.items()))
        return
    god = [g for g in 'ABC' if f'asks({q}, {g})' in true_vars][0]
    print('    ' * len(path) + f'Ask {god}: is it true that {find(table(q))}?')
    for a in ('da', 'ja'):
        print('    ' * len(path) + f'  {a}:')
        walk(path + [a])
walk([])
