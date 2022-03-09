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
        (@arg dot: -d --dot !takes_value "Output in dot format")
        (@arg convert: -c --convert +takes_value "Do not generate a new graph, but convert an existing edge list")
    )
    .get_matches();

    let undirected = args.is_present("undirected");

    let selection = if args.is_present("convert") {
        let file = File::open(args.value_of("convert").unwrap()).expect("Could not open file");
        let mut bufreader = BufReader::new(file);
        read_graph(&mut bufreader, undirected).expect("Could not parse edge list")
    } else {
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

        generate_graph(num_vertices, num_edges, undirected)
    };

    let output = args.value_of("output");

    let mut writer = if output.is_some() {
        let file = File::create(output.unwrap())?;
        Box::new(BufWriter::new(file)) as Box<dyn Write>
    } else {
        Box::new(BufWriter::new(io::stdout())) as Box<dyn Write>
    };

    if args.is_present("dot") {
        if undirected {
            writeln!(writer, "graph G {{")?;
            for edge in selection {
                writeln!(writer, "    {} -- {}", edge.0, edge.1)?;
            }
            writeln!(writer, "}}")?;
        } else {
            writeln!(writer, "digraph G {{")?;
            for edge in selection {
                writeln!(writer, "    {} -> {}", edge.0, edge.1)?;
            }
            writeln!(writer, "}}")?;
        }
    } else {
        for edge in selection {
            writeln!(writer, "{},{}", edge.0, edge.1)?;
        }
    }

    // flush the writer before dropping it
    writer.flush().expect("Could not flush write buffer");

    Ok(())
}

fn read_graph<R: Read>(reader: R, undirected: bool) -> io::Result<Vec<(String, String)>> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(reader);

    let mut edges: Vec<(String, String)> = Vec::new();

    for edge_record in csv_reader.records() {
        let edge = edge_record?;

        assert!(edge.len() == 2);

        if !(undirected && edges.contains(&(edge[1].to_string(), edge[0].to_string()))) {
            edges.push((edge[0].to_string(), edge[1].to_string()));
        }
    }

    Ok(edges)
}

fn generate_graph(
    num_vertices: usize,
    num_edges: usize,
    undirected: bool,
) -> Vec<(String, String)> {
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

    edges[0..num_edges].to_vec()
}
