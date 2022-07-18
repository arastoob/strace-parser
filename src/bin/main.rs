use clap::Parser as ClapParser;
use std::path::PathBuf;
use strace_parser::Parser;

/// A library for parsing the strace output log
#[derive(ClapParser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The path to the logged traces
    #[clap(short, long)]
    path: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut parser = Parser::new(args.path);
    let processes = parser.parse()?;

    for process in processes {
        println!("{}", process);
    }

    Ok(())
}
