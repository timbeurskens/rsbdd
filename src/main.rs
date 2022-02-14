use rsbdd::bdd::*;
use rsbdd::bdd_io::*;
use rsbdd::set::BDDSet;
use std::fs::File;
use std::rc::Rc;

fn main() {
    println!("Hello, world!");

    let mut f1 = File::create("output1.dot").unwrap();
    // let mut f2 = File::create("output2.dot").unwrap();

    // let mut set = BDDSet::new(8);

    // for i in 0..0x1000 {
    //     set = set.insert(i);
    // }

    // set.bdd.render_dot(&mut f);

    let vars: Vec<usize> = (0..4).collect();

    let env = BDDEnv::new();

    let b = env.amn(&vars, 2);

    println!("dups: {:?}", env.duplicates(b.clone()));

    // let c = env.clean(b.clone());

    // println!("dups: {:?}", env.duplicates(c.clone()));

    // dbg!(&env.nodes);

    let env_ptr = Rc::new(env);

    let graph1 = BDDGraph::new(&env_ptr, &b);
    graph1.render_dot(&mut f1);

    // let graph2 = BDDGraph::new(&env_ptr, &c);
    // graph2.render_dot(&mut f2);

    // dbg!(b);
}
