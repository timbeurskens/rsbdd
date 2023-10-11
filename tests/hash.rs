// use rsbdd::bdd::*;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use rsbdd::bdd::BDDContainer;
use rsbdd::bdd::BDDEnv;
use rustc_hash::FxHashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

// try and check whether we can find nodes with the same hash, but are not equal
#[test]
fn test_duplicates() {
    let e = BDDEnv::new();

    let n = 5;

    // every row must contain exactly one queen
    let row_expr = (0..n)
        .map(|i| (0..n).map(|j| e.var(j + i * n)).collect::<Vec<_>>())
        .map(|ref c| e.exn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    // every column must contain exactly one queen
    let col_expr = (0..n)
        .map(|i| (0..n).map(|j| e.var(j * n + i)).collect::<Vec<_>>())
        .map(|ref c| e.exn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    let diag_expr_hl = (0..n)
        .map(|i| {
            (0..=(n - i))
                .map(|j| e.var(i + (j * (n + 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vl = (1..n)
        .map(|i| {
            (0..=(n - i))
                .map(|j| e.var((i * n) + (j * (n + 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    let diag_expr_hr = (0..n)
        .map(|i| {
            (0..=i)
                .map(|j| e.var(i + (j * (n - 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    // skip the first, as this is already covered by the previous expression
    let diag_expr_vr = (1..n)
        .map(|i| {
            (0..=i)
                .map(|j| e.var((i * n) + (j * (n - 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(acc.clone(), k.clone())
        });

    let expr_list: Vec<BDDContainer<usize>> = vec![
        row_expr,
        col_expr,
        diag_expr_hl,
        diag_expr_vl,
        diag_expr_hr,
        diag_expr_vr,
    ];

    let expr_comb = expr_list
        .iter()
        .fold(e.mk_const(true), |ref acc, k| e.and(acc.clone(), k.clone()));

    let expr_comb_clean = e.clean(expr_comb.clone());

    // b contains a small example with duplicate nodes

    let mut hm: FxHashMap<u64, Vec<BDDContainer<usize>>> = FxHashMap::default();

    let mut max_size: usize = 0;

    for ref node in e.node_list(expr_comb_clean.clone()) {
        let h = node.get_hash();

        if let Some(l) = hm.get_mut(&h) {
            l.push(node.clone());
            if l.len() > max_size {
                max_size = l.len();
            }
        } else {
            hm.insert(h, vec![node.clone()]);
        }
    }

    // dbg!(&hm);

    dbg!(max_size);

    dbg!(e.duplicates(expr_comb_clean));

    for nvec in hm.values() {
        for i in nvec {
            let l = hm
                .get(&i.get_hash())
                .unwrap()
                .iter()
                .map(|x| Arc::into_raw(x.clone()) as u64)
                .unique()
                .count();

            // every node in the bdd must be contained in the node map
            for j in nvec {
                if e.nodes.read().unwrap().get(i.as_ref()).is_some() {
                    assert!(e.nodes.read().unwrap().get(j.as_ref()).is_some());
                }
            }

            // every node must uniquely exist in the node map
            assert_eq!(l, 1);
        }
    }
}
