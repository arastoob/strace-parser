# Strace-parser: A parsing strace logs library written in Rust 

Strace-parser is a library for parsing strace logs captured by tools such as [Strace](https://man7.org/linux/man-pages/man1/strace.1.html)
and find the inter-process relationships based on their access patterns to files and directories.

This parser is useful when someone wants to replay trace logs for evaluations purposes, such as file system benchmarks
like [Fs-bench](https://gitlab.com/arastoob/fs-bench), which supports trace replays as a benchmarking mode.

Strace-parser generates a list of [process objects](https://gitlab.com/arastoob/strace-parser/-/blob/main/src/process.rs),
each of which includes a list of [Operations](https://gitlab.com/arastoob/strace-parser/-/blob/main/src/op.rs),
which are executed on a file or directory. Also, it generates a DAG of process objects ordered based on their
access patterns. The accessed files and directories during the trace log are recorded in a list, which can be accessed
by the [`existing_files`](https://gitlab.com/arastoob/strace-parser/-/blob/main/src/parser.rs#L1054) method.

Strace-parser accepts trace logs that include the process/thread ids at the beginning of each line. Some trace files 
can be found from the [trace-examples](https://gitlab.com/arastoob/strace-parser/-/tree/main/trace-examples) directory. 
For including the process/thread ids in the Strace outputs, the `-f` argument should be passed to `strace` command.

## Example
The below example shows how to use strace-parser to parse a strace log file and generate the process DAG:
<br />
<pre>
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = Parser::new("trace-examples/multithread_strace1.log");
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
}
</pre>

A list of available processes is returned in each iteration of the while loop in the above code. The available processes
are the ones that can be replayed in parallel without interfering with each other. The list of accessed/visited files 
and directories during the trace log can also be accessed as:
<br />
<pre>
let files = parser.existing_files()?;
</pre>

## Run
To run the strace-parser, execute the following command:
<br />
<pre>
git clone https://gitlab.com/arastoob/strace-parser.git
cd strace-parser/
cargo run --release -- -p {path-to-strace-log-file} -o {path-to-output-file}
</pre>


