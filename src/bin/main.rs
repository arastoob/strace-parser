use std::path::PathBuf;
use strace_parser::error::Error;
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

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let mut parser = Parser::new(args.log_path);
    parser.parse()?;

    Ok(())
}
