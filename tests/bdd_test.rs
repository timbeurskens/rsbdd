use rsbdd::bdd;
use rsbdd::bdd::*;

type BDD = bdd::BDD<usize>;

use std::fs::File;
use rsbdd::bdd_io;

#[test]
fn test_equivalence() {
    assert_ne!(var(0), var(1));
    assert_eq!(var(0), var(0));

    assert_eq!(&BDD::True, &BDD::True);
    assert_eq!(&BDD::False, &BDD::False);
    assert_ne!(&BDD::True, &BDD::False);
    assert_ne!(&BDD::False, &BDD::True);

    assert_eq!(&and(&BDD::True, &BDD::True), &BDD::True);
    assert_ne!(and(&var(0), &var(1)), and(&var(1), &var(2)));
    assert_eq!(and(&var(0), &var(1)), and(&var(1), &var(0)));

    assert_eq!(not(&var(0)), BDD::Choice(Box::new(BDD::False), 0, Box::new(BDD::True)))
}

#[test]
fn trivial_bdd() {
    assert_eq!(and(&BDD::True, &BDD::True), BDD::True);
    assert_eq!(and(&BDD::False, &BDD::True), BDD::False);
    assert_eq!(and(&var(0), &BDD::False), BDD::False);
    assert_eq!(and(&var(0), &BDD::True), var(0));

    assert_eq!(or(&BDD::True, &BDD::False), BDD::True);
    assert_eq!(or(&BDD::True, &var(0)), BDD::True);
    assert_eq!(or(&BDD::False, &var(0)), var(0));
}

#[test]
fn test_combined() {
    assert_eq!(and(&or(&var(0), &not(&var(0))), &or(&var(1), &not(&var(1)))), BDD::True);
    assert_eq!(xor(&BDD::True, &BDD::True), BDD::False);
    assert_eq!(xor(&BDD::False, &BDD::True), BDD::True);
    assert_eq!(xor(&BDD::False, &BDD::False), BDD::False);
    assert_eq!(eq(&var(0), &var(0)), BDD::True);
}

#[test]
fn test_quantifiers() {
    assert_eq!(exists(0, &or(&var(0), &var(1))), BDD::True);
    assert_eq!(all(0, &var(0)), BDD::False);
    assert_eq!(all(0, &BDD::True), BDD::True);
    assert_eq!(exists(0, &BDD::False), BDD::False);
}

#[test]
fn test_fixedpoint() {
    assert_eq!(fp(&BDD::False, |x: &BDD| or(x, &BDD::True)), BDD::True);
}

#[test]
fn test_ite() {
    assert_eq!(ite(&BDD::True, &var(0), &var(1)), var(0));
    assert_eq!(ite(&BDD::False, &var(0), &var(1)), var(1));
    assert_eq!(ite(&var(0), &BDD::False, &BDD::True), not(&var(0)));
}

#[test]
fn test_exn() {
    assert_eq!(exn(&vec![], 0), BDD::True);
    assert_eq!(exn(&vec![], 1), BDD::False);
    assert_eq!(exn(&vec![0], 1), var(0));
    assert_eq!(exn(&vec![0, 1], 1), or(&and(&not(&var(0)), &var(1)), &and(&not(&var(1)), &var(0))));
}

#[test]
fn test_aln() {
    assert_eq!(aln(&vec![], 0), BDD::True);
    assert_eq!(aln(&vec![0], 0), BDD::True);
    assert_eq!(aln(&vec![0], 1), var(0));
    assert_eq!(aln(&vec![0, 1], 1), or(&var(0), &var(1)));
    assert_eq!(aln(&vec![0, 1, 2], 1), or(&or(&var(0), &var(1)), &var(2)));
}

#[test]
fn test_amn() {
    assert_eq!(amn(&vec![], 1), BDD::True);
    assert_eq!(amn(&vec![], 0), BDD::True);
    assert_eq!(amn(&vec![0], 0), not(&var(0)));
    assert_eq!(amn(&vec![0], 1), BDD::True);
    assert_eq!(amn(&vec![0, 1], 1), or(&and(&not(&var(0)), &not(&var(1))), &or(&and(&var(0), &not(&var(1))), &and(&not(&var(0)), &var(1)))));
    assert_ne!(amn(&vec![0, 1, 2], 1), BDD::False);
}

#[test]
fn test_model() {
    let bdd = and(&var(0), &var(1));
    let model = model(&bdd);
    
    dbg!(&model);

    assert_eq!(implies(&model, &var(0)), BDD::True);
    assert_eq!(implies(&model, &var(1)), BDD::True);
    assert_ne!(implies(&model, &var(2)), BDD::True);
}

#[test]
fn test_exn_model() {
    // semi-exhaustive test for exactly n
    for n in 0..15 {
        for c in 0..=n {
            let vars : Vec<usize> = (0..n).collect();
            let expr = exn(&vars, c);
            let model = model(&expr);

            let mut count = 0;
            for i in vars {
                if implies(&model, &var(i)) == BDD::True {
                    count += 1;
                }
            }

            assert_eq!(count, c);
        }    
    }
    
}

#[test]
fn test_exn_interference_model() {
    // semi-exhaustive test for exactly n
    for n in 1..8 {
        for o in 0..n {
            for c in 0..=n {
                println!("n: {}, o: {}, c: {}", n, o, c);

                let vars : Vec<usize> = (0..n).collect();
                let vars_interference : Vec<usize> = (n-o..(2*n)).collect();
    
                let expr = exn(&vars, c);
                let expr_interference = exn(&vars_interference, c);
    
                let expr_comb = and(&expr, &expr_interference);
    
                let model = model(&expr_comb);
    
                let mut count = 0;
                for i in vars {
                    if implies(&model, &var(i)) == BDD::True {
                        count += 1;
                    }
                }
    
                assert_eq!(count, c);
    
                count = 0;
                for i in vars_interference {
                    if implies(&model, &var(i)) == BDD::True {
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
    // non-exhaustive test for at most n
    for n in 0..15 {
        for c in 0..=n {
            let vars : Vec<usize> = (0..n).collect();
            let expr = amn(&vars, c);
            let model = model(&expr);

            let mut count = 0;
            for i in vars {
                if implies(&model, &var(i)) == BDD::True {
                    count += 1;
                }
            }
            assert!(count <= c);
        }    
    }
}

#[test]
fn test_aln_model() {
    // non-exhaustive test for at least n
    for n in 0..15 {
        for c in 0..=n {
            let vars : Vec<usize> = (0..n).collect();
            let expr = aln(&vars, c);
            let model = model(&expr);

            let mut count = 0;
            for i in vars {
                if implies(&model, &var(i)) == BDD::True {
                    count += 1;
                }
            }
            assert!(count >= c);
        }    
    }
}

#[test]
fn test_queens() {
    let n = 5;

    // every row must contain exactly one queen
    let row_expr = (0..n)
        .map(|i| (0..n).map(|j| j + i * n).collect::<Vec<_>>())
        .map(|ref c| exn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    // every column must contain exactly one queen
    let col_expr = (0..n)
        .map(|i| (0..n).map(|j| j * n + i).collect::<Vec<_>>())
        .map(|ref c| exn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    let diag_expr_hl = (0..n)
        .map(|i| (0..=(n-i)).map(|j| i + (j * (n+1))).collect::<Vec<_>>())
        .map(|ref c| amn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vl = (1..n)
        .map(|i| (0..=(n-i)).map(|j| (i * n) + (j * (n+1))).collect::<Vec<_>>())
        .map(|ref c| amn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    let diag_expr_hr = (0..n)
        .map(|i| (0..=i).map(|j| i + (j * (n-1))).collect::<Vec<_>>())
        .map(|ref c| amn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vr = (1..n)
        .map(|i| (0..=i).map(|j| (i * n) + (j * (n-1))).collect::<Vec<_>>())
        .map(|ref c| amn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    let expr_list : Vec<BDD> = vec![
        row_expr,
        col_expr,
        diag_expr_hl,
        diag_expr_vl,
        diag_expr_hr,
        diag_expr_vr,
    ];

    let expr_comb = expr_list.iter().fold(BDD::True, |ref acc, ref k| and(acc, k));

    let model = model(&expr_comb);

    // only retain the queens
    let queens : Vec<usize> = (0..(n*n))
        .filter(|&i| infer(&model, i).1)
        .collect();

    dbg!(&queens);

    let mut f = File::create("n_queens.dot").unwrap();

    model.render_dot(&mut f);

    let mut f = File::create("n_queens_full.dot").unwrap();

    expr_comb.render_dot(&mut f);

    /*
x  1  2  3  4
5  6  x  8  9
10 11 12 13 x
15 x  17 18 19
20 21 22 x  24
    */

}