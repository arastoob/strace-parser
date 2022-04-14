use clap::Parser as ClapParser;
use std::path::PathBuf;
use strace_parser::Parser;

/// A library for parsing the strace output log
#[derive(ClapParser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The path to store benchmark results
    #[clap(short, long)]
    log_path: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut parser = Parser::new(args.log_path);
    let (serial, parallel) = parser.parse()?;

    println!("serial operations count: {:#?}", serial.len());
    println!("parallel operations count: {}", parallel.len());

    Ok(())
}
