use rustc_hash::FxHashSet;

use rsbdd::bdd;
use rsbdd::bdd::{BDDEnv, Node};

type BDD = bdd::BDD<usize>;

fn queens(e: &BDDEnv<usize>, n: usize) -> BDD {
    let cell = |r: usize, c: usize| e.var(r * n + c);
    let lines = (0..n)
        .map(|r| e.exn(&(0..n).map(|c| cell(r, c)).collect::<Vec<_>>(), 1))
        .chain((0..n).map(|c| e.exn(&(0..n).map(|r| cell(r, c)).collect::<Vec<_>>(), 1)));
    let diagonals = (0..2 * n - 1).flat_map(|d| {
        let down = (0..n)
            .filter(|&r| d + r >= n - 1 && d + r - (n - 1) < n)
            .map(|r| cell(r, d + r - (n - 1)))
            .collect::<Vec<_>>();
        let up = (0..n)
            .filter(|&r| d >= r && d - r < n)
            .map(|r| cell(r, d - r))
            .collect::<Vec<_>>();
        [e.amn(&down, 1), e.amn(&up, 1)]
    });

    lines
        .chain(diagonals)
        .fold(e.mk_const(true), |acc, c| e.and(acc, c))
}

fn same_structure(a: &BDD, b: &BDD, seen: &mut FxHashSet<(u64, u64)>) -> bool {
    if !seen.insert((a.get_hash(), b.get_hash())) {
        return true;
    }
    match (a.node(), b.node()) {
        (Node::True, Node::True) | (Node::False, Node::False) => true,
        (Node::Choice(at, va, af), Node::Choice(bt, vb, bf)) => {
            va == vb && same_structure(&at, &bt, seen) && same_structure(&af, &bf, seen)
        }
        _ => false,
    }
}

fn swap_files() -> usize {
    let prefix = format!("rsbdd-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir())
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter(|f| f.file_name().to_string_lossy().starts_with(&prefix))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn test_gc_frees_unreferenced_nodes() {
    let e = BDDEnv::new();
    let kept = e.and(e.var(0), e.var(1));
    {
        let vars: Vec<BDD> = (0..12).map(|i| e.var(i)).collect();
        let _garbage = e.exn(&vars, 3);
    }

    let before = e.size();
    e.gc();

    assert!(e.size() < before);
    assert_eq!(e.size(), 4);
    assert!(e.mk_const(true).is_true());
    assert!(e.mk_const(false).is_false());
    assert_eq!(kept, e.and(e.var(1), e.var(0)));
}

#[test]
fn test_gc_keeps_results_canonical() {
    let e = BDDEnv::new();
    let a = queens(&e, 5);
    e.gc();
    let size = e.size();

    let b = queens(&e, 5);
    assert_eq!(a, b);

    e.gc();
    assert_eq!(e.size(), size);
    assert_eq!(e.duplicates(a), 0);
}

#[test]
fn test_memory_limit_pages_to_disk() {
    let limit = 256 * 1024;
    let unlimited = BDDEnv::new();
    let limited = BDDEnv::with_memory_limit(limit);

    let expected = queens(&unlimited, 7);
    let result = queens(&limited, 7);

    assert!(unlimited.memory() > limit);
    assert!(limited.memory() <= limit);
    assert!(same_structure(
        &expected,
        &result,
        &mut FxHashSet::default()
    ));
    assert_eq!(limited.model(result).is_false(), expected.is_false());
    assert_eq!(swap_files(), 0);

    drop(limited);
    assert_eq!(swap_files(), 0);
}
