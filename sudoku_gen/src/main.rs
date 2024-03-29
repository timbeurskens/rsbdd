use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::io::*;
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser, value_name = "INPUT")]
    /// The input sudoku file
    input: Option<PathBuf>,

    #[clap(value_parser, value_name = "OUTPUT")]
    /// The output rsbdd file
    output: Option<PathBuf>,

    #[clap(short, long, value_parser, value_name = "N", default_value_t = 3)]
    /// The root value of the puzzle. Typically the square root of the largest possible number
    root: usize,
}

fn main() -> io::Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    let args = Args::parse();

    let root = args.root;
    let square = root * root;
    let numcells = square * square;

    let mut writer = if let Some(output) = args.output {
        let file = File::create(output)?;
        Box::new(BufWriter::new(file)) as Box<dyn Write>
    } else {
        Box::new(BufWriter::new(io::stdout())) as Box<dyn Write>
    };

    let mut puzzle_input = String::new();

    if let Some(input) = args.input {
        let mut file = File::open(input)?;
        file.read_to_string(&mut puzzle_input)?;
    } else {
        io::stdin().read_to_string(&mut puzzle_input)?;
    }

    let puzzle_input: String = puzzle_input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    writeln!(
        writer,
        "\"Generated by sudoku-gen version {} puzzle=[{}]\"",
        version, puzzle_input
    )?;

    writeln!(writer)?;

    writeln!(writer, "\"sudoku hints\"")?;

    writeln!(writer)?;

    for i in 0..numcells {
        if let Some(ch) = puzzle_input.chars().nth(i) {
            if char::is_digit(ch, 10) {
                writeln!(writer, "_{}_is_{} &", i, ch)?;
            }
        }
    }

    writeln!(writer)?;

    writeln!(writer, "\"each cell can either be 1, 2, .., {}\"", square)?;

    writeln!(writer)?;

    for i in 0..numcells {
        let vars = (1..=square)
            .map(|j| format!("_{}_is_{}", i, j))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(writer, "[{}] = 1 &", vars)?;
    }

    writeln!(writer)?;

    writeln!(
        writer,
        "\"each row and every column contains every number exactly once\""
    )?;

    writeln!(writer)?;

    for i in 0..square {
        for k in 1..=square {
            let vars = (0..square)
                .map(|j| format!("_{}_is_{}", i * square + j, k))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(writer, "[{}] = 1 &", vars)?;

            let vars = (0..square)
                .map(|j| format!("_{}_is_{}", j * square + i, k))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(writer, "[{}] = 1 &", vars)?;
        }
    }

    writeln!(writer)?;

    writeln!(writer, "\"each nonet contains every number exactly once\"")?;

    writeln!(writer)?;

    for i in 0..root {
        for j in 0..root {
            let lt = (i * root) * square + (j * root);
            for k in 1..=square {
                let vars = (0..square)
                    .map(|l| format!("_{}_is_{}", lt + ((l / root) * square + (l % root)), k))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(writer, "[{}] = 1 &", vars)?;
            }
        }
    }

    writeln!(writer, "true")?;

    writer.flush()?;

    Ok(())
}
