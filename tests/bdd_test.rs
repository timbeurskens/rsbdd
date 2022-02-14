use rsbdd::bdd;
use rsbdd::bdd::*;
use std::rc::Rc;

type BDD = bdd::BDD<usize>;

use rsbdd::bdd_io::*;
use std::fs::File;

#[test]
fn test_equivalence() {
    let e = BDDEnv::new();

    assert_ne!(e.var(0), e.var(1));
    assert_eq!(e.var(0), e.var(0));

    let _true = e.mk_const(true);
    let _false = e.mk_const(false);

    assert_eq!(e.mk_const(true), e.mk_const(true));
    assert_eq!(e.mk_const(false), e.mk_const(false));
    assert_ne!(e.mk_const(true), e.mk_const(false));
    assert_ne!(e.mk_const(false), e.mk_const(true));

    assert_eq!(e.and(_true.clone(), _true.clone()), _true.clone());
    assert_ne!(e.and(e.var(0), e.var(1)), e.and(e.var(1), e.var(2)));
    assert_eq!(e.and(e.var(0), e.var(1)), e.and(e.var(1), e.var(0)));

    assert_eq!(
        e.not(e.var(0)),
        e.mk_choice(_false.clone(), 0, _true.clone())
    )
}

#[test]
fn test_simplifications() {
    let e = BDDEnv::new();

    let _v0 = e.var(0);

    // true, false, v0
    assert_eq!(e.size(), 3);

    let _v1 = e.var(1);

    // true, false, v0, v1
    assert_eq!(e.size(), 4);

    let _v0 = e.var(0);

    // true, false, v0
    assert_eq!(e.size(), 4);
}

#[test]
fn test_simple_duplicates() {
    let e = BDDEnv::new();

    assert_eq!(e.duplicates(e.var(0)), 0);

    assert_eq!(e.duplicates(e.mk_const(true)), 0);

    assert_eq!(e.duplicates(e.and(e.mk_const(true), e.mk_const(false))), 0);

    assert_eq!(e.duplicates(e.and(e.mk_const(false), e.var(0))), 0);

    assert_eq!(e.duplicates(e.and(e.mk_const(true), e.var(0))), 0);

    assert_eq!(e.duplicates(e.amn(&vec![1, 2], 1)), 0);
}

#[test]
fn trivial_bdd() {
    let e = BDDEnv::new();

    assert_eq!(e.and(e.mk_const(true), e.mk_const(true)), e.mk_const(true));
    assert_eq!(
        e.and(e.mk_const(false), e.mk_const(true)),
        e.mk_const(false)
    );
    assert_eq!(e.and(e.var(0), e.mk_const(false)), e.mk_const(false));
    assert_eq!(e.and(e.var(0), e.mk_const(true)), e.var(0));

    assert_eq!(e.or(e.mk_const(true), e.mk_const(false)), e.mk_const(true));
    assert_eq!(e.or(e.mk_const(true), e.var(0)), e.mk_const(true));
    assert_eq!(e.or(e.mk_const(false), e.var(0)), e.var(0));
}

#[test]
fn test_combined() {
    let e = BDDEnv::new();

    assert_eq!(
        e.and(
            e.or(e.var(0), e.not(e.var(0))),
            e.or(e.var(1), e.not(e.var(1)))
        ),
        e.mk_const(true)
    );
    assert_eq!(e.xor(e.mk_const(true), e.mk_const(true)), e.mk_const(false));
    assert_eq!(e.xor(e.mk_const(false), e.mk_const(true)), e.mk_const(true));
    assert_eq!(
        e.xor(e.mk_const(false), e.mk_const(false)),
        e.mk_const(false)
    );
    assert_eq!(e.eq(e.var(0), e.var(0)), e.mk_const(true));
}

#[test]
fn test_quantifiers() {
    let e = BDDEnv::new();

    assert_eq!(e.exists(0, e.or(e.var(0), e.var(1))), e.mk_const(true));
    assert_eq!(e.all(0, e.var(0)), e.mk_const(false));
    assert_eq!(e.all(0, e.mk_const(true)), e.mk_const(true));
    assert_eq!(e.exists(0, e.mk_const(false)), e.mk_const(false));
}

#[test]
fn test_fixedpoint() {
    let e = BDDEnv::new();

    assert_eq!(
        e.fp(e.mk_const(false), |x: Rc<BDD>| e.or(x, e.mk_const(true))),
        e.mk_const(true)
    );
}

#[test]
fn test_ite() {
    let e = BDDEnv::new();

    assert_eq!(e.ite(e.mk_const(true), e.var(0), e.var(1)), e.var(0));
    assert_eq!(e.ite(e.mk_const(false), e.var(0), e.var(1)), e.var(1));
    assert_eq!(
        e.ite(e.var(0), e.mk_const(false), e.mk_const(true)),
        e.not(e.var(0))
    );
}

#[test]
fn test_exn() {
    let e = BDDEnv::new();

    assert_eq!(e.exn(&vec![], 0), e.mk_const(true));
    assert_eq!(e.exn(&vec![], 1), e.mk_const(false));
    assert_eq!(e.exn(&vec![0], 1), e.var(0));
    assert_eq!(
        e.exn(&vec![0, 1], 1),
        e.or(
            e.and(e.not(e.var(0)), e.var(1)),
            e.and(e.not(e.var(1)), e.var(0))
        )
    );
}

#[test]
fn test_aln() {
    let e = BDDEnv::new();

    assert_eq!(e.aln(&vec![], 0), e.mk_const(true));
    assert_eq!(e.aln(&vec![0], 0), e.mk_const(true));
    assert_eq!(e.aln(&vec![0], 1), e.var(0));
    assert_eq!(e.aln(&vec![0, 1], 1), e.or(e.var(0), e.var(1)));
    assert_eq!(
        e.aln(&vec![0, 1, 2], 1),
        e.or(e.or(e.var(0), e.var(1)), e.var(2))
    );
}

#[test]
fn test_amn() {
    let e = BDDEnv::new();

    assert_eq!(e.amn(&vec![], 1), e.mk_const(true));
    assert_eq!(e.amn(&vec![], 0), e.mk_const(true));
    assert_eq!(e.amn(&vec![0], 0), e.not(e.var(0)));
    assert_eq!(e.amn(&vec![0], 1), e.mk_const(true));
    assert_eq!(
        e.amn(&vec![0, 1], 1),
        e.or(
            e.and(e.not(e.var(0)), e.not(e.var(1))),
            e.or(
                e.and(e.var(0), e.not(e.var(1))),
                e.and(e.not(e.var(0)), e.var(1))
            )
        )
    );
    assert_ne!(e.amn(&vec![0, 1, 2], 1), e.mk_const(false));
}

#[test]
fn test_model() {
    let e = BDDEnv::new();

    let bdd = e.and(e.var(0), e.var(1));
    let model = e.model(bdd);

    dbg!(&model);

    assert_eq!(e.implies(model.clone(), e.var(0)), e.mk_const(true));
    assert_eq!(e.implies(model.clone(), e.var(1)), e.mk_const(true));
    assert_ne!(e.implies(model.clone(), e.var(2)), e.mk_const(true));
}

#[test]
fn test_exn_model() {
    let e = BDDEnv::new();

    // semi-exhaustive test for exactly n
    for n in 0..15 {
        for c in 0..=n {
            let vars: Vec<usize> = (0..n).collect();
            let expr = e.exn(&vars, c);
            let model = e.model(expr);

            let mut count = 0;
            for i in vars {
                if e.implies(model.clone(), e.var(i)) == e.mk_const(true) {
                    count += 1;
                }
            }

            assert_eq!(count, c);
        }
    }
}

#[test]
fn test_exn_interference_model() {
    let e = BDDEnv::new();

    // semi-exhaustive test for exactly n
    for n in 1..8 {
        for o in 0..n {
            for c in 0..=n {
                println!("n: {}, o: {}, c: {}", n, o, c);

                let vars: Vec<usize> = (0..n).collect();
                let vars_interference: Vec<usize> = (n - o..(2 * n)).collect();

                let expr = e.exn(&vars, c);
                let expr_interference = e.exn(&vars_interference, c);

                let expr_comb = e.and(expr, expr_interference);

                let model = e.model(expr_comb);

                let mut count = 0;
                for i in vars {
                    if e.implies(model.clone(), e.var(i)) == e.mk_const(true) {
                        count += 1;
                    }
                }

                assert_eq!(count, c);

                count = 0;
                for i in vars_interference {
                    if e.implies(model.clone(), e.var(i)) == e.mk_const(true) {
                        count += 1;
                    }
                }

                assert_eq!(count, c);
            }
        }
    }
}

#[test]
fn test_amn_model() {
    let e = BDDEnv::new();

    // non-exhaustive test for at most n
    for n in 0..15 {
        for c in 0..=n {
            let vars: Vec<usize> = (0..n).collect();
            let expr = e.amn(&vars, c);
            let model = e.model(expr);

            let mut count = 0;
            for i in vars {
                if e.implies(model.clone(), e.var(i)) == e.mk_const(true) {
                    count += 1;
                }
            }
            assert!(count <= c);
        }
    }
}

#[test]
fn test_aln_model() {
    let e = BDDEnv::new();

    // non-exhaustive test for at least n
    for n in 0..15 {
        for c in 0..=n {
            let vars: Vec<usize> = (0..n).collect();
            let expr = e.aln(&vars, c);
            let model = e.model(expr);

            let mut count = 0;
            for i in vars {
                if e.implies(model.clone(), e.var(i)) == e.mk_const(true) {
                    count += 1;
                }
            }
            assert!(count >= c);
        }
    }
}

#[test]
fn test_queens() {
    let e = BDDEnv::new();

    let n = 5;

    // every row must contain exactly one queen
    let row_expr = (0..n)
        .map(|i| (0..n).map(|j| j + i * n).collect::<Vec<_>>())
        .map(|ref c| e.exn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    // every column must contain exactly one queen
    let col_expr = (0..n)
        .map(|i| (0..n).map(|j| j * n + i).collect::<Vec<_>>())
        .map(|ref c| e.exn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    let diag_expr_hl = (0..n)
        .map(|i| (0..=(n - i)).map(|j| i + (j * (n + 1))).collect::<Vec<_>>())
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vl = (1..n)
        .map(|i| {
            (0..=(n - i))
                .map(|j| (i * n) + (j * (n + 1)))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    let diag_expr_hr = (0..n)
        .map(|i| (0..=i).map(|j| i + (j * (n - 1))).collect::<Vec<_>>())
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vr = (1..n)
        .map(|i| (0..=i).map(|j| (i * n) + (j * (n - 1))).collect::<Vec<_>>())
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    let expr_list: Vec<Rc<BDD>> = vec![
        row_expr,
        col_expr,
        diag_expr_hl,
        diag_expr_vl,
        diag_expr_hr,
        diag_expr_vr,
    ];

    let expr_comb = expr_list.iter().fold(e.mk_const(true), |ref acc, ref k| {
        e.and(Rc::clone(acc), Rc::clone(k))
    });

    // duplicates tested in hash.rs
    // assert_eq!(e.duplicates(expr_comb.clone()), 0);

    let model = e.model(expr_comb.clone());

    // only retain the queens
    let queens: Vec<usize> = (0..(n * n))
        .filter(|&i| e.infer(model.clone(), i).1)
        .collect();

    dbg!(&queens);

    assert_eq!(queens.len(), n);

    println!("size of environment: {} nodes", e.size());

    let mut f = File::create("n_queens_full.dot").unwrap();

    let graph = BDDGraph::new(&Rc::new(e), &expr_comb);

    graph.render_dot(&mut f);
}
