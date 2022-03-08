#[macro_use]
extern crate clap;

use rand::seq::SliceRandom;
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

    for (i, v1) in vertices.iter().enumerate() {
        if undirected {
            for v2 in vertices[(i + 1)..].iter() {
                edges.push((v1.clone(), v2.clone()));
            }
        } else {
            for (j, v2) in vertices.iter().enumerate() {
                if i != j {
                    edges.push((v1.clone(), v2.clone()));
                }
            }
        }
    }

    edges.shuffle(&mut rng);

    let output = args.value_of("output");

    let mut writer = if output.is_some() {
        let file = File::create(output.unwrap())?;
        Box::new(BufWriter::new(file)) as Box<dyn Write>
    } else {
        Box::new(BufWriter::new(io::stdout())) as Box<dyn Write>
    };

    for edge in edges[0..num_edges].iter() {
        writeln!(writer, "{},{}", edge.0, edge.1)?;
    }

    // flush the writer before dropping it
    writer.flush().expect("Could not flush write buffer");

    Ok(())
}
