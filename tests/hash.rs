// use rsbdd::bdd::*;
use itertools::Itertools;
use rsbdd::bdd;
use rsbdd::bdd::BDDEnv;
use rsbdd::bdd::BDDNode;
use rustc_hash::{FxHasher, FxHashMap};
use std::hash::Hash;
use std::rc::Rc;
use std::vec::Vec;

type BDD = bdd::BDD<usize>;

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
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    // every column must contain exactly one queen
    let col_expr = (0..n)
        .map(|i| (0..n).map(|j| e.var(j * n + i)).collect::<Vec<_>>())
        .map(|ref c| e.exn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    let diag_expr_hl = (0..n)
        .map(|i| {
            (0..=(n - i))
                .map(|j| e.var(i + (j * (n + 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
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
            e.and(Rc::clone(acc), Rc::clone(k))
        });

    let diag_expr_hr = (0..n)
        .map(|i| {
            (0..=i)
                .map(|j| e.var(i + (j * (n - 1))))
                .collect::<Vec<_>>()
        })
        .map(|ref c| e.amn(c, 1))
        .fold(e.mk_const(true), |ref acc, ref k| {
            e.and(Rc::clone(acc), Rc::clone(k))
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

    let expr_comb = expr_list.iter().fold(e.mk_const(true), |ref acc, k| {
        e.and(Rc::clone(acc), Rc::clone(k))
    });

    let expr_comb_clean = e.clean(Rc::clone(&expr_comb));

    // b contains a small example with duplicate nodes

    let mut hm: FxHashMap<u64, Vec<Rc<BDD>>> = FxHashMap::default();

    let mut max_size: usize = 0;

    for ref node in e.node_list(Rc::clone(&expr_comb_clean)) {
        let h = node.get_hash();

        if let Some(l) = hm.get_mut(&h) {
            l.push(Rc::clone(node));
            if l.len() > max_size {
                max_size = l.len();
            }
        } else {
            hm.insert(h, vec![Rc::clone(node)]);
        }
    }

    // dbg!(&hm);

    dbg!(max_size);

    dbg!(e.duplicates(expr_comb_clean));

    for (_, nvec) in &hm {
        for i in nvec {
            let l = hm
                .get(&i.get_hash())
                .unwrap()
                .iter()
                .map(|x| Rc::into_raw(Rc::clone(x)) as u64)
                .unique()
                .count();

            // every node in the bdd must be contained in the node map
            for j in nvec {
                if e.nodes.borrow().get(i.as_ref()).is_some() {
                    assert!(e.nodes.borrow().get(j.as_ref()).is_some());
                }
            }

            // every node must uniquely exist in the node map
            assert_eq!(l, 1);
        }
    }
}

#[test]
fn test_hash_populated_equals_unpopulated() {
    let populated = BDD::new(&BDDNode::True);
    let unpopulated = BDD::new_unoptimized(&BDDNode::True);

    assert_eq!(populated.get_hash(), unpopulated.get_hash());
}

#[test]
fn test_hash_objects_compute_same_hash() {
    let object_to_hash: usize = 42;

    let mut hasher1 = FxHasher::default();
    let mut hasher2 = FxHasher::default();

    assert_eq!(object_to_hash.hash(&mut hasher1), object_to_hash.hash(&mut hasher2));
}