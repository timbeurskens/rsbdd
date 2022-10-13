use clap::Parser;
use rand::seq::SliceRandom;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::io::*;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about = "Generates a random edge list formatted graph", long_about = None)]
struct Args {
    #[clap(value_parser, value_name = "VERTICES")]
    /// The number of vertices in the output graph
    vertices: Option<usize>,

    #[clap(value_parser, value_name = "EDGES")]
    /// The number of edges in the output graph
    edges: Option<usize>,

    #[clap(value_parser, short, long)]
    /// The output filename (or stdout if not provided)
    output: Option<PathBuf>,

    #[clap(short, long)]
    /// Use undirected edges (test for both directions in the set-complement operation)
    undirected: bool,

    #[clap(short, long)]
    /// Output in dot (GraphViz) format
    dot: bool,

    #[clap(long, value_parser)]
    /// If this argument is provided, the provided edge-list will be used to generate a graph
    convert: Option<PathBuf>,

    #[clap(short, long, value_parser, value_name = "N")]
    /// Generate a graph-coloring problem with N colors
    colors: Option<usize>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut selection = if let Some(file_to_convert) = args.convert {
        let file = File::open(file_to_convert).expect("Could not open file");
        let mut bufreader = BufReader::new(file);
        read_graph(&mut bufreader, args.undirected).expect("Could not parse edge list")
    } else {
        if args.vertices.is_none() || args.edges.is_none() {
            panic!("Must provide vertices and edges if not converting a graph");
        }
        generate_graph(args.vertices.unwrap(), args.edges.unwrap(), args.undirected)
    };

    // convert to a graph-coloring problem
    if let Some(num_colors) = args.colors {
        selection = augment_colors(&selection, num_colors);
    }

    let mut writer = if let Some(output_file) = args.output {
        let file = File::create(output_file)?;
        Box::new(BufWriter::new(file)) as Box<dyn Write>
    } else {
        Box::new(BufWriter::new(io::stdout())) as Box<dyn Write>
    };

    if args.dot {
        if args.undirected {
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

fn augment_colors(edges: &Vec<(String, String)>, num_colors: usize) -> Vec<(String, String)> {
    let mut vertex_map: FxHashMap<String, String> = FxHashMap::default();
    let mut color_map: FxHashMap<String, usize> = FxHashMap::default();

    for edge in edges {
        for color in 0..num_colors {
            let v1 = format!("{}_c{}", edge.0, color);
            let v2 = format!("{}_c{}", edge.1, color);

            vertex_map.insert(v1.clone(), edge.0.clone());
            vertex_map.insert(v2.clone(), edge.1.clone());

            color_map.insert(v1, color);
            color_map.insert(v2, color);
        }
    }

    let mut new_edges: Vec<(String, String)> = Vec::new();

    let vertices = vertex_map.keys().cloned().collect::<Vec<String>>();

    for (i, v1) in vertices.iter().enumerate() {
        for v2 in vertices[(i + 1)..].iter() {
            let c1 = color_map[v1];
            let c2 = color_map[v2];

            let ov1 = &vertex_map[v1];
            let ov2 = &vertex_map[v2];

            // only add an edge to new_edges if the colors are different, or the vertices are not connected
            if ov1 != ov2
                && (c1 != c2
                    || (!edges.contains(&(ov1.clone(), ov2.clone()))
                        && !edges.contains(&(ov2.clone(), ov1.clone()))))
            {
                new_edges.push((v1.clone(), v2.clone()));
            }
        }
    }

    new_edges
}
