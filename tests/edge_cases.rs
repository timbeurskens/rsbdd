use pretty_assertions::assert_eq;

use rsbdd::bdd;
use rsbdd::bdd::{BDDEnv, Node};

type BDD = bdd::BDD<usize>;

const VARS: usize = 4;

fn eval(b: &BDD, assignment: &[bool]) -> bool {
    match b.node() {
        Node::True => true,
        Node::False => false,
        Node::Choice(t, v, f) => eval(if assignment[v] { &t } else { &f }, assignment),
    }
}

fn same(a: &BDD, b: &BDD) -> bool {
    a == b
}

fn assignments() -> impl Iterator<Item = Vec<bool>> {
    (0..1 << VARS).map(|m| (0..VARS).map(|i| (m >> i) & 1 == 1).collect())
}

fn formulas(e: &BDDEnv<usize>) -> Vec<BDD> {
    let v = |i| e.var(i);
    vec![
        e.mk_const(false),
        e.mk_const(true),
        v(0),
        v(3),
        e.not(v(1)),
        e.and(v(0), v(2)),
        e.or(v(1), e.not(v(3))),
        e.xor(v(2), v(1)),
        e.ite(v(3), v(0), e.not(v(2))),
    ]
}

fn count(branches: &[BDD], a: &[bool]) -> i64 {
    branches.iter().filter(|b| eval(b, a)).count() as i64
}

#[test]
fn test_operators_match_boolean_semantics() {
    let e = BDDEnv::new();
    let fs = formulas(&e);

    for x in &fs {
        let not = e.not(x.clone());
        for a in assignments() {
            assert_eq!(eval(&not, &a), !eval(x, &a));
        }

        for y in &fs {
            let ops: [(BDD, fn(bool, bool) -> bool); 7] = [
                (e.and(x.clone(), y.clone()), |p, q| p && q),
                (e.or(x.clone(), y.clone()), |p, q| p || q),
                (e.xor(x.clone(), y.clone()), |p, q| p ^ q),
                (e.nand(x.clone(), y.clone()), |p, q| !(p && q)),
                (e.nor(x.clone(), y.clone()), |p, q| !(p || q)),
                (e.implies(x.clone(), y.clone()), |p, q| !p || q),
                (e.eq(x.clone(), y.clone()), |p, q| p == q),
            ];

            for (result, op) in &ops {
                for a in assignments() {
                    assert_eq!(eval(result, &a), op(eval(x, &a), eval(y, &a)));
                }
            }

            for z in &fs {
                let ite = e.ite(x.clone(), y.clone(), z.clone());
                for a in assignments() {
                    let expected = if eval(x, &a) {
                        eval(y, &a)
                    } else {
                        eval(z, &a)
                    };
                    assert_eq!(eval(&ite, &a), expected);
                }
            }
        }
    }
}

#[test]
fn test_equivalent_formulas_share_nodes() {
    let e = BDDEnv::new();
    let (a, b, c) = (e.var(0), e.var(1), e.var(2));

    assert!(same(
        &e.and(a.clone(), b.clone()),
        &e.and(b.clone(), a.clone())
    ));
    assert!(same(&e.not(e.not(c.clone())), &c));
    assert!(same(
        &e.xor(a.clone(), b.clone()),
        &e.or(
            e.and(a.clone(), e.not(b.clone())),
            e.and(e.not(a.clone()), b.clone())
        )
    ));
    assert!(same(
        &e.and(a.clone(), e.or(b.clone(), c.clone())),
        &e.or(e.and(a.clone(), b.clone()), e.and(a.clone(), c.clone()))
    ));
    assert!(same(&e.or(a.clone(), e.not(a.clone())), &e.mk_const(true)));

    let size = e.size();
    e.exn(&[a.clone(), b.clone(), c.clone()], 1);
    let size_after_first = e.size();
    e.exn(&[a, b, c], 1);
    assert!(size_after_first >= size);
    assert_eq!(e.size(), size_after_first);
    assert_eq!(e.duplicates(e.var(0)), 0);
}

#[test]
fn test_operator_results_do_not_collide() {
    let e = BDDEnv::new();
    let (a, b) = (e.var(0), e.var(1));

    let and = e.and(a.clone(), b.clone());
    let or = e.or(a.clone(), b.clone());
    let not = e.not(a.clone());

    assert_ne!(and, or);
    assert_eq!(e.and(a.clone(), b.clone()), and);
    assert_eq!(e.or(b, a.clone()), or);
    assert_eq!(e.not(a.clone()), not);
    assert_eq!(e.and(a.clone(), a.clone()), a);
    assert_eq!(e.or(a.clone(), a.clone()), a);
    assert_eq!(e.and(a.clone(), not.clone()), e.mk_const(false));
    assert_eq!(e.or(a, not), e.mk_const(true));
}

#[test]
fn test_counting_out_of_range() {
    let e = BDDEnv::new();
    let vs: Vec<BDD> = (0..3).map(|i| e.var(i)).collect();
    let all = vs
        .iter()
        .fold(e.mk_const(true), |acc, v| e.and(acc, v.clone()));
    let none = vs
        .iter()
        .fold(e.mk_const(true), |acc, v| e.and(acc, e.not(v.clone())));

    assert_eq!(e.aln(&vs, -1), e.mk_const(true));
    assert_eq!(e.aln(&vs, 3), all);
    assert_eq!(e.aln(&vs, 4), e.mk_const(false));
    assert_eq!(e.amn(&vs, -1), e.mk_const(false));
    assert_eq!(e.amn(&vs, 0), none);
    assert_eq!(e.amn(&vs, 3), e.mk_const(true));
    assert_eq!(e.exn(&vs, -1), e.mk_const(false));
    assert_eq!(e.exn(&vs, 0), none);
    assert_eq!(e.exn(&vs, 3), all);
    assert_eq!(e.exn(&vs, 4), e.mk_const(false));
}

#[test]
fn test_counting_repeated_and_compound_branches() {
    let e = BDDEnv::new();
    let (a, b) = (e.var(0), e.var(1));

    assert_eq!(e.exn(&[a.clone(), a.clone()], 1), e.mk_const(false));
    assert_eq!(e.exn(&[a.clone(), a.clone()], 2), a);
    assert_eq!(e.exn(&[a.clone(), e.not(a.clone())], 1), e.mk_const(true));
    assert_eq!(
        e.aln(&[e.and(a.clone(), b.clone()), a.clone()], 2),
        e.and(a, b)
    );
}

#[test]
fn test_counting_matches_boolean_semantics() {
    let e = BDDEnv::new();
    let fs = formulas(&e);
    let lists: Vec<Vec<BDD>> = vec![
        vec![],
        fs[2..5].to_vec(),
        fs[4..].to_vec(),
        vec![fs[2].clone(), fs[2].clone(), fs[5].clone()],
        fs.clone(),
    ];

    for l in &lists {
        let len = l.len() as i64;
        for n in -1..=len + 1 {
            let (aln, amn, exn) = (e.aln(l, n), e.amn(l, n), e.exn(l, n));
            for a in assignments() {
                let c = count(l, &a);
                assert_eq!(eval(&aln, &a), c >= n);
                assert_eq!(eval(&amn, &a), c <= n);
                assert_eq!(eval(&exn, &a), c == n);
            }
        }

        for r in &lists {
            let results = [
                (e.count_lt(l, r), i64::lt as fn(&i64, &i64) -> bool),
                (e.count_leq(l, r), i64::le),
                (e.count_eq(l, r), i64::eq),
                (e.count_geq(l, r), i64::ge),
                (e.count_gt(l, r), i64::gt),
            ];
            for (result, cmp) in &results {
                for a in assignments() {
                    assert_eq!(eval(result, &a), cmp(&count(l, &a), &count(r, &a)));
                }
            }
        }
    }
}

#[test]
fn test_quantifier_edge_cases() {
    let e = BDDEnv::new();
    let f = e.and(e.var(0), e.var(2));

    assert!(same(&e.exists(vec![], f.clone()), &f));
    assert!(same(&e.exists(vec![1], f.clone()), &f));
    assert!(same(&e.exists(vec![5], f.clone()), &f));
    assert!(same(&e.all(vec![1, 5], f.clone()), &f));
    assert_eq!(
        e.exists(vec![0, 0], f.clone()),
        e.exists(vec![0], f.clone())
    );
    assert_eq!(e.exists(vec![0, 2], f.clone()), e.mk_const(true));
    assert_eq!(e.all(vec![0, 2], f), e.mk_const(false));
    assert_eq!(
        e.exists(vec![0], e.and(e.var(0), e.not(e.var(0)))),
        e.mk_const(false)
    );
    assert_eq!(
        e.all(vec![0, 1], e.or(e.var(0), e.not(e.var(0)))),
        e.mk_const(true)
    );
}

#[test]
fn test_quantifiers_match_boolean_semantics() {
    let e = BDDEnv::new();

    for f in formulas(&e) {
        for v in 0..VARS {
            let exists = e.exists(vec![v], f.clone());
            let all = e.all(vec![v], f.clone());
            for mut a in assignments() {
                a[v] = false;
                let lo = eval(&f, &a);
                let (ex_lo, all_lo) = (eval(&exists, &a), eval(&all, &a));
                a[v] = true;
                let hi = eval(&f, &a);
                assert_eq!(ex_lo, lo || hi);
                assert_eq!(all_lo, lo && hi);
                assert_eq!(eval(&exists, &a), lo || hi);
                assert_eq!(eval(&all, &a), lo && hi);
            }
        }
    }
}

#[test]
fn test_model_edge_cases() {
    let e = BDDEnv::new();

    assert_eq!(e.model(e.mk_const(false)), e.mk_const(false));
    assert_eq!(e.model(e.mk_const(true)), e.mk_const(true));
    assert_eq!(e.model(e.not(e.var(0))), e.not(e.var(0)));

    for f in formulas(&e) {
        let model = e.model(f.clone());
        assert_eq!(model.is_false(), f.is_false());
        assert_eq!(e.implies(model.clone(), f), e.mk_const(true));
        assert_eq!(e.model(model.clone()), model);
    }
}

#[test]
fn test_fixed_point_terminates_immediately() {
    let e = BDDEnv::new();
    let a = e.var(0);

    assert_eq!(e.fp(a.clone(), |x| x), a);
    assert_eq!(e.fp(e.mk_const(false), |x| e.or(x, a.clone())), a);
    assert_eq!(e.fp(e.mk_const(true), |x| e.and(x, a.clone())), a);
}
