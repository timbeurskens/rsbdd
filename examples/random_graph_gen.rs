#[macro_use]
extern crate clap;

use rand::prelude::*;
use std::fs::File;
use std::io;
use std::io::Write;
use std::io::*;

fn main() -> io::Result<()> {
    let args = clap_app!(RandomGraphGenerator =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: "Tim Beurskens")
        (about: "Generates a random edge list formatted graph")
        (@arg vertices: -v --vertices +takes_value "Number of vertices")
        (@arg edges: -e --edges +takes_value "Number of edges")
        (@arg output: -o --output +takes_value "The output file")
        (@arg undirected: -u --undirected !takes_value "Use undirected edges (test for both directions in the set complement operation)")
    )
    .get_matches();

    let num_vertices = args
        .value_of("vertices")
        .expect("Specify the number of vertices")
        .parse::<usize>()
        .unwrap();
    let num_edges = args
        .value_of("edges")
        .expect("Specify the number of edges")
        .parse::<usize>()
        .unwrap();

    let undirected = args.is_present("undirected");

    let mut rng = rand::thread_rng();

    let vertices = (0..num_vertices)
        .map(|vi| format!("v{}", vi))
        .collect::<Vec<String>>();
    let mut edges: Vec<(String, String)> = Vec::new();

    let p = 1.0 / (num_vertices * num_vertices) as f64;

    while edges.len() < num_edges {
        for (i, v1) in vertices.iter().enumerate() {
            for v2 in vertices[(i + 1)..].iter() {
                let clear = if undirected {
                    !(edges.contains(&(v1.clone(), v2.clone()))
                        || edges.contains(&(v2.clone(), v1.clone())))
                } else {
                    !edges.contains(&(v1.clone(), v2.clone()))
                };

                if clear && edges.len() < num_edges && rng.gen_bool(p) {
                    edges.push((v1.clone(), v2.clone()));
                }
            }
        }
    }

    let output = args.value_of("output");

    let mut writer = if output.is_some() {
        let file = File::create(output.unwrap())?;
        Box::new(BufWriter::new(file)) as Box<dyn Write>
    } else {
        Box::new(BufWriter::new(io::stdout())) as Box<dyn Write>
    };

    for edge in edges {
        writeln!(writer, "{},{}", edge.0, edge.1)?;
    }

    // flush the writer before dropping it
    writer.flush().expect("Could not flush write buffer");

    Ok(())
}
