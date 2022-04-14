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
    let operations = parser.parse()?;

    println!("operations:");
    for operation in operations {
        println!("{}", operation);
    }

    println!("\naccessed files:");
    let accessed_files = parser.accessed_files()?;
    for file_dir in accessed_files {
        println!("{}", file_dir);
    }

    println!("\noperations done by each process:");
    let pid_op = parser.processes_operations()?;
    for (pid, ops) in pid_op {
        println!("pid: {}, ops: {:?}", pid, ops);
    }

    Ok(())
}
