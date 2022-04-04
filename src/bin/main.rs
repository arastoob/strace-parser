use std::path::PathBuf;
use clap::Parser as ClapParser;
use strace_parser::parser::Parser;

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
    let operations = parser.parse()?;

    for operation in operations {
        println!("{}", operation);
    }

    Ok(())
}
