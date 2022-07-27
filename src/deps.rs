use crate::dag::DAG;
use crate::error::Error;
use crate::file::File;
use crate::op::{PreOperation, SharedOperation};
use crate::process::Process;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::sync::Arc;

// The dependecy graph node's can be Process or File
#[derive(Hash, PartialEq, Eq)]
pub enum GraphNode {
    Process(Process),
    File(Arc<File>),
}

impl GraphNode {
    pub fn ty(&self) -> &str {
        match self {
            &GraphNode::File(_) => "file",
            &GraphNode::Process(_) => "process",
        }
    }

    pub fn process(&self) -> Result<&Process, Error> {
        match self {
            GraphNode::Process(p) => Ok(p),
            _ => Err(Error::InvalidType("GraphNode".to_string())),
        }
    }
}

impl Display for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            &GraphNode::Process(ref p) => write!(f, "{}", p.pid()),
            &GraphNode::File(ref file) => write!(f, "{}", file.path().unwrap()),
        }
    }
}

///
/// The relationship between the Processes and the Files
/// The nodes are Processes and Files and the edges between them are the operation names
///
pub struct DependencyGraph {
    pub dag: DAG<GraphNode, SharedOperation>,
}

impl DependencyGraph {
    /// Generate the dependency graph form the vector of processes
    pub fn new(processes: Vec<Process>) -> Result<Self, Error> {
        let mut dag = DAG::new();

        for process in processes {
            let pnode = dag.add_node(GraphNode::Process(process.clone()));
            for op in process.ops() {
                if let Some(file) = op.op().lock()?.file() {
                    let fnode = dag.add_node(GraphNode::File(file));
                    // dag.add_edge(format!("{}:{}", op_id, op.name()), pnode.clone(), fnode);
                    dag.add_edge(op.clone(), pnode.clone(), fnode);
                }
            }
        }

        Ok(Self { dag })
    }

    /// Generate the vector of processes from the dependency graph
    pub fn processes(&self) -> Vec<Process> {
        // TODO: fix the unwrap
        Vec::from_iter(
            self.dag
                .nodes()
                .filter(|node| node.data().ty() == "process")
                .cloned()
                .map(|node| node.data().process().unwrap().clone()),
        )
    }

    pub fn mark_dependencies(&self) -> Result<(), Error> {
        for node in self.dag.nodes() {
            if node.in_degree() > 1 {
                // the file node, say f1, which has been accessed more than one time
                if let Some(edges) = self.dag.edges_to(node.clone()) {
                    for current_edge in edges.iter() {
                        // edges to f1
                        let current_edge_name = current_edge.label().op().lock()?.name();
                        let current_pid = current_edge.source().data().process()?.pid();
                        match current_edge_name.as_str() {
                            "Mkdir" => {
                                // mkdir should be executed before all other operations, except rename
                                let other_edges = edges
                                    .iter()
                                    .filter(|edge| *edge != current_edge)
                                    .collect::<Vec<_>>();
                                for edge in other_edges {
                                    let edge_name = edge.label().op().lock()?.name();
                                    if edge_name != "Mkdir" {
                                        // add the current operation as pre-operation to other operations by other processes
                                        let pid = edge.source().data().process()?.pid();
                                        if pid != current_pid {
                                            edge.label().op().lock()?.add_pre(PreOperation::new(
                                                current_edge.label().clone(),
                                                current_pid,
                                            ));
                                        }
                                    }
                                }
                            }
                            "Mknod" => {
                                // mknod should be executed before all other operations, except rename
                                let other_edges = edges
                                    .iter()
                                    .filter(|edge| *edge != current_edge)
                                    .collect::<Vec<_>>();
                                for edge in other_edges {
                                    let edge_name = edge.label().op().lock()?.name();
                                    if edge_name != "Mknod" {
                                        // add the current operation as pre-operation to other operations by other processes
                                        let pid = edge.source().data().process()?.pid();
                                        if pid != current_pid {
                                            edge.label().op().lock()?.add_pre(PreOperation::new(
                                                current_edge.label().clone(),
                                                current_pid,
                                            ));
                                        }
                                    }
                                }
                            }
                            "Write" => {
                                // write should be executed before read, remove and truncate operations
                                let other_edges = edges
                                    .iter()
                                    .filter(|edge| *edge != current_edge)
                                    .collect::<Vec<_>>();
                                for edge in other_edges {
                                    let edge_name = edge.label().op().lock()?.name();
                                    if edge_name == "Read"
                                        || edge_name == "Remove"
                                        || edge_name == "Rename"
                                        || edge_name == "Truncate"
                                    {
                                        // add the current operation as pre-operation to other operations by other processes
                                        let pid = edge.source().data().process()?.pid();
                                        if pid != current_pid {
                                            edge.label().op().lock()?.add_pre(PreOperation::new(
                                                current_edge.label().clone(),
                                                current_pid,
                                            ));
                                        }
                                    }
                                }
                            }
                            "Read" => {
                                // read should be executed before remove and truncate operations
                                let other_edges = edges
                                    .iter()
                                    .filter(|edge| *edge != current_edge)
                                    .collect::<Vec<_>>();
                                for edge in other_edges {
                                    let edge_name = edge.label().op().lock()?.name();
                                    if edge_name == "Remove"
                                        || edge_name == "Rename"
                                        || edge_name == "Truncate"
                                    {
                                        // add the current operation as pre-operation to other operations by other processes
                                        let pid = edge.source().data().process()?.pid();
                                        if pid != current_pid {
                                            edge.label().op().lock()?.add_pre(PreOperation::new(
                                                current_edge.label().clone(),
                                                current_pid,
                                            ));
                                        }
                                    }
                                }
                            }
                            _ => {
                                // other operations such as stat, fstat, ... can be executed in parallel
                            }
                        }
                    }
                }
            }
        }

        Ok(())
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
    use crate::error::Error;
    use crate::file::File;
    use crate::op::{Operation, SharedOperation};
    use crate::process::Process;
    use std::sync::Arc;

    #[test]
    fn dependencies() -> Result<(), Error> {
        // the files
        let f1 = Arc::new(File::new("f1".to_string()));
        let f2 = Arc::new(File::new("f2".to_string()));
        let d1 = Arc::new(File::new("d1".to_string()));
        let f3 = Arc::new(File::new("f3".to_string()));
        let f4 = Arc::new(File::new("f4".to_string()));

        // the operations
        let read_f1_op: SharedOperation = Operation::read(f1.clone(), 1, 1).into();
        let mknod_f2_op1: SharedOperation = Operation::mknod(f2.clone()).into();
        let mknod_f2_op2: SharedOperation = Operation::mknod(f2.clone()).into();
        let write_f2_op1: SharedOperation =
            Operation::write(f2.clone(), "something".to_string(), 1, 1).into();
        let write_f4_op: SharedOperation =
            Operation::write(f4.clone(), "something".to_string(), 1, 1).into();
        let remove_f2_op: SharedOperation = Operation::remove(f2.clone()).into();
        let mkdir_d1_op: SharedOperation =
            Operation::mkdir(d1.clone(), "a_mode".to_string()).into();
        let write_f2_op2: SharedOperation =
            Operation::write(f2.clone(), "something".to_string(), 1, 1).into();
        let stat_d1_op: SharedOperation = Operation::stat(d1.clone()).into();
        let read_f3_op: SharedOperation = Operation::read(f3.clone(), 1, 1).into();
        let read_f4_op: SharedOperation = Operation::read(f4.clone(), 1, 1).into();

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

        dep_graph.mark_dependencies()?;
        println!("main graph, marked:");
        println!("{}", dep_graph);

        // after marking the dependencies, the mknod_f2_op should be in the pre list of write_f2_op2 and remove_f2_op
        assert!(write_f2_op2
            .op()
            .lock()?
            .pre_list()
            .find(|pre_op| pre_op.pre_op() == mknod_f2_op1)
            .is_some());
        assert!(remove_f2_op
            .op()
            .lock()?
            .pre_list()
            .find(|pre_op| pre_op.pre_op() == mknod_f2_op1)
            .is_some());

        // note that mknod_f2_op1 should not be in the pre list of write_f2_op1 as both operations are done by process p1
        assert!(write_f2_op1
            .op()
            .lock()?
            .pre_list()
            .find(|pre_op| pre_op.pre_op() == mknod_f2_op1)
            .is_none());

        // also, write_f4_op should be in the pre list of read_f4_op
        assert!(read_f4_op
            .op()
            .lock()?
            .pre_list()
            .find(|pre_op| pre_op.pre_op() == write_f4_op)
            .is_some());

        Ok(())
    }
}
