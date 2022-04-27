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
    let (main_processes, postponed_processes) = parser.parse()?;

    for main_process in main_processes {
        println!("{}", main_process);
    }
    println!("postponed processes: ");
    for postponed_process in postponed_processes {
        println!("{}", postponed_process);
    }

    Ok(())
}
