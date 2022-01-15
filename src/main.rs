use std::fs::File;
use rsbdd::set::BDDSet;
use rsbdd::bdd::*;

fn main() {
    println!("Hello, world!");

    let mut f = File::create("output.dot").unwrap();

    // let mut set = BDDSet::new(8);
    
    // for i in 0..0x1000 {
    //     set = set.insert(i);
    // }

    // set.bdd.render_dot(&mut f);

    let vars: Vec<usize> = (0..4).collect();

    let b = amn(&vars, 2);

    b.render_dot(&mut f);
}
