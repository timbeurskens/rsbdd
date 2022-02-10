use rsbdd::bdd::*;
// use rsbdd::set::BDDSet;
use std::fs::File;

fn main() {
    println!("Hello, world!");

    let mut f = File::create("output.dot").unwrap();

    // let mut set = BDDSet::new(8);

    // for i in 0..0x1000 {
    //     set = set.insert(i);
    // }

    // set.bdd.render_dot(&mut f);

    let vars: Vec<usize> = (0..4).collect();

    let env = BDDEnv::new();

    let b = env.amn(&vars, 2);

    // b.render_dot(&mut f);

    dbg!(b);
}
