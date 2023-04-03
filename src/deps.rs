use crate::dag::{Edge, DAG};
use crate::error::Error;
use crate::file::File;
use crate::process::Process;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Arc;

// The dependecy graph node's can be Process or File
#[derive(Hash, PartialEq, Eq)]
pub enum GraphNode {
    Process(Process),
    File(Arc<File>),
    Start,
    End,
}

impl GraphNode {
    pub fn ty(&self) -> &str {
        match self {
            &GraphNode::File(_) => "file",
            &GraphNode::Process(_) => "process",
            &GraphNode::Start => "start",
            &GraphNode::End => "end",
        }
    }

    pub fn process(&self) -> Result<&Process, Error> {
        match self {
            GraphNode::Process(p) => Ok(p),
            _ => Err(Error::InvalidType("GraphNode".to_string())),
        }
    }

    pub fn file(&self) -> Result<Arc<File>, Error> {
        match self {
            GraphNode::File(f) => Ok(f.clone()),
            _ => Err(Error::InvalidType("GraphNode".to_string())),
        }
    }

    pub fn is_dummy(&self) -> bool {
        match self {
            GraphNode::End => true,
            GraphNode::Start => true,
            _ => false,
        }
    }
}

impl Display for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            &GraphNode::Process(ref p) => write!(f, "{}", p.pid()),
            &GraphNode::File(ref file) => write!(f, "{}", file.path().unwrap_or("")),
            &GraphNode::Start => write!(f, "start"),
            &GraphNode::End => write!(f, "end"),
        }
    }
}

///
/// The relationship between the Processes and the Files
/// The nodes are Processes and Files and the edges between them are the operation names
///
pub struct DependencyGraph {
    pub dag: DAG<GraphNode, String>,
}

impl DependencyGraph {
    /// Generate the dependency graph form the vector of processes
    pub fn new(processes: Vec<Process>) -> Result<Self, Error> {
        let mut dag = DAG::new();
        for process in processes {
            let pnode = dag.add_node(GraphNode::Process(process.clone()))?;
            for op in process.ops() {
                let op_name = op.name();
                if let Some(file) = op.file() {
                    let fnode = dag.add_node(GraphNode::File(file))?;
                    dag.add_edge(op_name, pnode.clone(), fnode);
                }
            }
        }

        Ok(Self { dag })
    }

    pub fn order(&self) -> Result<Self, Error> {
        // first, simplify the dag by summarizing the edges between two nodes to just one edge
        // with Read or Write label
        let simplified = self.simplify()?;

        // then, generate the read-write dag:
        //      - if the edge label between p1 and f1 is Write, we have an edge from p1 to f1 labeled Write
        //      - if the edge label between p1 and f1 is Read, we have an edge from f1 to p1 labeled Read
        let mut read_write_dag = DAG::new();
        for node in simplified.dag.nodes() {
            if node.in_degree() > 0 {
                // the file node, say f1

                if let Some(edges) = simplified.dag.edges_to(node.clone()) {
                    for edge in edges.iter() {
                        // edges to f1

                        // the process node, say p1
                        let source = edge.source();

                        let pnode = read_write_dag
                            .add_node(GraphNode::Process(source.data().process()?.clone()))?;
                        let fnode =
                            read_write_dag.add_node(GraphNode::File(node.data().file()?))?;

                        if edge.label().as_str() == "Write" {
                            // add an edge from p1 to f1 with Write lable
                            read_write_dag.add_edge("Write".to_string(), pnode, fnode);
                        } else if edge.label().as_str() == "Read" {
                            // add an edge from f1 to p1 with Read label
                            read_write_dag.add_edge("Read".to_string(), fnode, pnode);
                        }
                    }
                }
            }
        }

        // now, remove the file nodes between processes to make it even simpler
        let mut process_dag = DAG::new();
        let start_node = process_dag.add_node(GraphNode::Start)?;
        let end_node = process_dag.add_node(GraphNode::End)?;

        for node in read_write_dag.nodes() {
            if node.data().ty() == "file" {
                if !node.incoming_neighbors().is_empty() {
                    for incoming_process in node.incoming_neighbors().iter() {
                        let incoming_process = incoming_process.upgrade().ok_or(
                            Error::NoneValue("weak reference to process node".to_string()),
                        )?;
                        let p1 = process_dag.add_node(GraphNode::Process(
                            incoming_process.data().process()?.clone(),
                        ))?;

                        if !node.outgoing_neighbors().is_empty() {
                            for outgoing_process in node.outgoing_neighbors().iter() {
                                let p2 = process_dag.add_node(GraphNode::Process(
                                    outgoing_process.data().process()?.clone(),
                                ))?;
                                process_dag.add_edge("".to_string(), p1.clone(), p2);
                            }
                        } else {
                            process_dag.add_edge("".to_string(), p1, end_node.clone());
                        }
                    }
                } else {
                    // there is no incoming edges to the file node, it means that it has only been read
                    for outgoing_process in node.outgoing_neighbors().iter() {
                        let p2 = process_dag.add_node(GraphNode::Process(
                            outgoing_process.data().process()?.clone(),
                        ))?;
                        process_dag.add_edge("".to_string(), start_node.clone(), p2);
                    }
                }
            }
        }

        // now remove the start and end node
        process_dag.remove_node(&start_node);
        process_dag.remove_node(&end_node);

        Ok(Self { dag: process_dag })
    }

    fn simplify(&self) -> Result<Self, Error> {
        let mut simplified = DAG::new();
        for node in self.dag.nodes() {
            if node.in_degree() == 0 {
                // the process node, say p1

                for neighbour in node.outgoing_neighbors().iter() {
                    if let Some(edges) = self.dag.edges_between(node.clone(), neighbour.clone()) {
                        let summarized_label = self.summarize_edges(&edges)?;

                        let pnode = simplified
                            .add_node(GraphNode::Process(node.data().process()?.clone()))?;
                        let fnode =
                            simplified.add_node(GraphNode::File(neighbour.data().file()?))?;
                        simplified.add_edge(summarized_label, pnode, fnode);
                    }
                }
            }
        }

        Ok(Self { dag: simplified })
    }

    fn summarize_edges(&self, edges: &Vec<Rc<Edge<GraphNode, String>>>) -> Result<String, Error> {
        let write_filter = edges
            .iter()
            .filter(|edge| {
                edge.label() == "Mkdir"
                    || edge.label() == "Mknod"
                    || edge.label() == "Write"
                    || edge.label() == "Truncate"
            })
            .collect::<Vec<_>>();

        if !write_filter.is_empty() {
            Ok("Write".to_string())
        } else {
            Ok("Read".to_string())
        }
    }

    pub fn available_set(&mut self) -> Result<Vec<Process>, Box<dyn std::error::Error>> {
        if self.dag.nodes().len() == 0 {
            return Ok(vec![]);
        }

        // get the nodes with in-degree of 0
        let mut first_level_nodes = self
            .dag
            .nodes()
            .filter(|node| node.in_degree() == 0)
            .cloned()
            .collect::<Vec<_>>();

        // TODO: fix the unwraps
        if first_level_nodes.len() > 1 {
            // there is more than one process nodes with in-degree of 0, so pick the one
            // with lower pid
            first_level_nodes.sort_by(|n1, n2| {
                n1.data()
                    .process()
                    .unwrap()
                    .pid()
                    .partial_cmp(&n2.data().process().unwrap().pid())
                    .unwrap()
            });
        }

        let mut available_set = vec![];
        for node in first_level_nodes {
            // remove the node from the dag
            self.dag.remove_node(&node);
            available_set.push(node.data().process()?.clone());
        }
        Ok(available_set)
    }
}

impl Display for DependencyGraph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.dag)
    }
}

#[cfg(test)]
mod test {
    use crate::deps::DependencyGraph;
    use crate::file::File;
    use crate::op::Operation;
    use crate::process::Process;
    use std::sync::Arc;

    #[test]
    fn dependencies() -> Result<(), Box<dyn std::error::Error>> {
        // the files
        let f1 = Arc::new(File::new("f1".to_string()));
        let f2 = Arc::new(File::new("f2".to_string()));
        let d1 = Arc::new(File::new("d1".to_string()));
        let f3 = Arc::new(File::new("f3".to_string()));
        let f4 = Arc::new(File::new("f4".to_string()));

        // the operations
        let read_f1_op = Operation::read(f1.clone(), 1, 1);
        let mknod_f2_op1 = Operation::mknod(f2.clone());
        let mknod_f2_op2 = Operation::mknod(f2.clone());
        let write_f2_op1 = Operation::write(f2.clone(), "something".to_string(), 1, 1);
        let write_f4_op = Operation::write(f4.clone(), "something".to_string(), 1, 1);
        let remove_f2_op = Operation::remove(f2.clone());
        let mkdir_d1_op = Operation::mkdir(d1.clone(), "a_mode".to_string());
        let write_f2_op2 = Operation::write(f2.clone(), "something".to_string(), 1, 1);
        let stat_d1_op = Operation::stat(d1.clone());
        let read_f3_op = Operation::read(f3.clone(), 1, 1);
        let read_f4_op = Operation::read(f4.clone(), 1, 1);

        // the processes
        let mut p1 = Process::new(1);
        let mut p2 = Process::new(2);
        let mut p3 = Process::new(3);

        // the operations done by each process
        p1.add_op(read_f1_op.clone());
        p1.add_op(mknod_f2_op1.clone());
        p1.add_op(write_f2_op1.clone());
        p1.add_op(write_f4_op.clone());
        p2.add_op(remove_f2_op.clone());
        p2.add_op(mkdir_d1_op.clone());
        p2.add_op(write_f2_op2.clone());
        p3.add_op(stat_d1_op.clone());
        p3.add_op(read_f3_op.clone());
        p3.add_op(read_f4_op.clone());
        p3.add_op(mknod_f2_op2.clone());

        let mut processes = vec![];
        processes.push(p1);
        processes.push(p2);
        processes.push(p3);

        let dep_graph = DependencyGraph::new(processes)?;

        // the dep_graph should look like this:
        //    p1 --> f1 [Read]
        //    p1 --> f2 [Mknod]
        //    p1 --> f2 [Write]
        //    p1 --> f4 [Write]
        //    p2 --> f2 [Remove]
        //    p2 --> f2 [Write]
        //    p2 --> d1 [Mkdir]
        //    p3 --> d1 [Stat]
        //    p3 --> f3 [Read]
        //    p3 --> f4 [Read]
        //    p3 --> f2 [Mknod]

        assert_eq!(dep_graph.dag.node_count(), 8);
        assert_eq!(dep_graph.dag.edge_count(), 11);
        println!("main graph:");
        println!("{}", dep_graph);

        let mut dep_graph_ordered = dep_graph.order()?;
        println!("main graph, ordered:");
        println!("{}", dep_graph_ordered);

        // after order, we should have a dag including process nodes as follows:
        //  p1 --> p3
        //  p2 --> p3

        assert_eq!(dep_graph_ordered.available_set()?.len(), 2);
        assert_eq!(dep_graph_ordered.available_set()?.len(), 1);
        assert_eq!(dep_graph_ordered.available_set()?.len(), 0);

        Ok(())
    }
}
