use rsbdd::bdd;
use rsbdd::bdd::{BDDSymbol, var, and, or, not, implies, xor, exists, all, fp, eq, amn, aln, exn, ite, model};

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
fn test_queens() {
    let n = 6;

    // every row must contain at least one queen
    let row_expr = (0..n)
        .map(|i| (0..n).map(|j| j + i * n).collect::<Vec<_>>())
        .map(|ref c| exn(c, 1))
        .reduce(|ref acc, ref k| and(acc, k)).unwrap();

    // every column must contain at least one queen
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

    dbg!(&model);

    let mut f = File::create("n_queens.dot").unwrap();

    model.render_dot(&mut f)


}