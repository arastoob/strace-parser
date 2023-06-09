use clap::Parser as ClapParser;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use strace_parser::Parser;

/// A library for parsing the strace output log
#[derive(ClapParser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The path to the logged traces
    #[clap(short, long)]
    path: PathBuf,

    /// The output file path
    #[clap(short, long)]
    out: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut parser = Parser::new(args.path);
    let mut dep_graph = parser.parse()?;

    let out_file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(false)
        .open(args.out)?;
    let mut writer = BufWriter::new(out_file);

    let mut available_set = dep_graph.available_set()?;
    while !available_set.is_empty() {
        for process in available_set.iter() {
            writer.write(process.to_string().as_ref())?;
        }

        available_set = dep_graph.available_set()?;
    }
    writer.flush()?;

    Ok(())
}
