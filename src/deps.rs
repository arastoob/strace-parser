use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::dag::{DAG, Edge};
use crate::error::Error;
use crate::file::File;
use crate::process::Process;

// The dependecy graph node's can be Process or File
#[derive(Hash, PartialEq, Eq)]
pub enum GraphNode {
    Process(Process),
    File(Rc<File>)
}

impl GraphNode {
    pub fn ty(&self) -> &str {
        match self {
            &GraphNode::File(_) => "file",
            &GraphNode::Process(_) => "process"
        }
    }

    pub fn process_mut(&mut self) -> Result<&mut Process, Error> {
        match self {
            GraphNode::Process(p) => Ok(p),
            _ => Err(Error::InvalidType("GraphNode".to_string()))
        }
    }

    pub fn process(&self) -> Result<&Process, Error> {
        match self {
            GraphNode::Process(p) => Ok(p),
            _ => Err(Error::InvalidType("GraphNode".to_string()))
        }
    }
}

impl Display for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            &GraphNode::Process(ref p) => write!(f, "{}", p.pid()),
            &GraphNode::File(ref file) => write!(f, "{}", file.path().unwrap())
        }
    }
}

///
/// The relationship between the Processes and the Files
/// The nodes are Processes and Files and the edges between them are the operation names
///
pub struct DependencyGraph {
    pub dag: DAG<GraphNode, String>
}

impl DependencyGraph {
    // Generate the dependency graph form the vector of processes
    pub fn new(processes: Vec<Process>) -> Self {
        let mut dag = DAG::new();

        for process in processes {
            // let process = Rc::new(process);
            let pnode = dag.add_node(GraphNode::Process(process.clone()));
            for (op_id, op) in process.ops() {
                if let Some(file) = op.file() {
                    let fnode = dag.add_node(GraphNode::File(file));
                    dag.add_edge(format!("{}:{}", op_id, op.name()),pnode.clone(), fnode);
                }
            }
        }

        Self {
            dag
        }
    }

    // Generate the vector of processes from the dependency graph
    pub fn processes(&self) -> Vec<Process> {

        // TODO: fix the unwrap
        Vec::from_iter(
            self.dag.nodes()
                .filter(|node| node.data().ty() == "process")
                .cloned()
                .map(|node| node.data().process().unwrap().clone())
        )
    }

    // the sub-edges from the main graph that contains the parallel read and write accesses to the same file node
    fn parallel_rw_edges(&self) -> Vec<Rc<Edge<GraphNode, String>>> {
        // the sub graph containing the nodes and their edges that represent multiple accesses to a file node

        // read and write edges to the files accessed more than one time
        let multi_rw_edges =
            self.dag.edges()
                .filter(|edge| edge.target().in_degree() > 1)
                .filter(|edge| edge.label().contains("Write") || edge.label().contains("Read"))
                .cloned()
                .collect::<Vec<_>>();

        // the sub graph from multi_accessed_edges that contains just the read and write accesses
        let rw_dag = DAG::from_edges(
            multi_rw_edges
        );

        let mut parallel_rw_edges = vec![];
        for node in rw_dag.nodes() {
            if let Some(mut edges_to) = rw_dag.edges_to(node.clone()) {
                if edges_to.iter().find(|e| e.label().contains("Read")).is_some() &&
                    edges_to.iter().find(|e| e.label().contains("Write")).is_some() {
                    // this is the file node that has both read and write access
                    parallel_rw_edges.append(&mut edges_to);
                }
            }
        }

        parallel_rw_edges
    }

    // the sub-graph from the main graph that contains the read accesses to the files that need
    // to be postponed, i.e, the read accesses that are in parallel with the write accesses
    // to the same file node
    pub fn postponed_r_graph(&self) -> DependencyGraph {
        DependencyGraph {
            // the graph from the parallel read and write edges that contains the read accesses
            // that should be executed later
            dag: DAG::from_edges(
                self.parallel_rw_edges().iter()
                    .filter(|edge| edge.label().contains("Read"))
                    .cloned()
                    .collect::<Vec<_>>()
            )
        }
    }
}

impl Display for DependencyGraph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.dag)
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;
    use crate::dag::DAG;
    use crate::deps::DependencyGraph;
    use crate::error::Error;
    use crate::file::File;
    use crate::op::Operation;
    use crate::process::Process;

    fn processes() -> Vec<Process> {
        let f1 = Rc::new(File::new("f1".to_string()));
        let f2 = Rc::new(File::new("f2".to_string()));
        let d1 = Rc::new(File::new("d1".to_string()));
        let f3 = Rc::new(File::new("f3".to_string()));
        let f4 = Rc::new(File::new("f4".to_string()));

        let mut processes = vec![];
        let mut p1 = Process::new(1);
        let mut p2 = Process::new(2);
        let mut p3 = Process::new(3);

        p1.add_op(1, Operation::read(f1.clone(), 1, 1));
        p1.add_op(2, Operation::mknod(f2.clone()));
        p1.add_op(3, Operation::write(f2.clone(), "something".to_string(), 1, 1));
        p1.add_op(4, Operation::write(f4.clone(), "something".to_string(), 1, 1));
        p2.add_op(5, Operation::remove(f2.clone()));
        p2.add_op(6, Operation::mkdir(d1.clone(), "a_mode".to_string()));
        p2.add_op(7, Operation::write(f2.clone(), "something".to_string(), 1, 1));
        p3.add_op(8, Operation::stat(d1.clone()));
        p3.add_op(9, Operation::read(f3.clone(), 1, 1));
        p3.add_op(10, Operation::read(f4.clone(), 1, 1));


        processes.push(p1);
        processes.push(p2);
        processes.push(p3);

        processes
    }

    #[test]
    fn postponed_dag() -> Result<(), Error> {
        let dep_graph = DependencyGraph::new(processes());

        // the main graph is:
        //    1 --> f1 [1:Read]
        //    1 --> f2 [2:Mknod]
        //    1 --> f2 [3:Write]
        //    1 --> f4 [4:Write]
        //    2 --> f2 [5:Remove]
        //    2 --> f2 [7:Write]
        //    2 --> d1 [6:Mkdir]
        //    3 --> d1 [8:Stat]
        //    3 --> f3 [9:Read]
        //    3 --> f4 [10:Read]

        assert_eq!(dep_graph.dag.node_count(), 8);
        assert_eq!(dep_graph.dag.edge_count(), 10);
        println!("main graph:");
        println!("{}", dep_graph);


        // the sub-graph containing the parallel read and write accesses to the same file node should be:
        //    1 --> f4 [4:Write]
        //    3 --> f4 [10:Read]

        let parallel_rw_dag = DAG::from_edges(dep_graph.parallel_rw_edges());
        assert_eq!(parallel_rw_dag.node_count(), 3);
        assert_eq!(parallel_rw_dag.edge_count(), 2);
        println!("parallel read write sub-graph:");
        println!("{}", parallel_rw_dag);

        // the postponed sub-graph should be:
        //    3 --> f4 [10:Read]

        let postponed_r_graph = dep_graph.postponed_r_graph();
        assert_eq!(postponed_r_graph.dag.node_count(), 2);
        assert_eq!(postponed_r_graph.dag.edge_count(), 1);
        println!("postponed read sub-graph:");
        println!("{}", postponed_r_graph);

        Ok(())
    }
}