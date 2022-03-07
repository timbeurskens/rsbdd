use rsbdd::bdd;
use rsbdd::bdd_io::*;
use std::fs::File;
use std::rc::Rc;

type Env = bdd::BDDEnv<usize>;

fn main() {
    println!("Hello, world!");

    let e = Env::new();

    let x = e.var(0);
    let y = e.var(1);
    let z = e.var(2);
    let w = e.var(3);

    let f1 = e.or(x.clone(), y.clone());
    let f2 = e.implies(x.clone(), y.clone());
    let f3 = e.xor(x.clone(), y.clone());

    let ft = e.and(f1, e.and(f2, f3));

    let g1 = e.exists(
        vec![0],
        e.exists(
            vec![1],
            e.and(
                e.eq(x.clone(), z.clone()),
                e.and(e.eq(y.clone(), w.clone()), e.or(z.clone(), w.clone())),
            ),
        ),
    );

    let gt = e.and(ft, g1);

    dbg!(&gt);

    let mut f = File::create("numeric.dot").unwrap();

    let graph = BDDGraph::new(&Rc::new(e), &gt, bdd::TruthTableEntry::Any);

    graph
        .render_dot(&mut f)
        .expect("could not render to dot graph");
}
