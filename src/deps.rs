use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::dag::DAG;
use crate::file::File;
use crate::process::Process;

// The dependecy graph node's can be Process or File
#[derive(Hash, PartialEq, Eq, Clone)]
pub enum GraphNode {
    Process(Rc<Process>),
    File(Rc<File>)
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
    pub dag: DAG<GraphNode>
}

impl DependencyGraph {
    pub fn new(processes: Vec<Process>) -> Self {
        let mut dag = DAG::new();

        for process in processes {
            let process = Rc::new(process);
            let pnode = dag.add_node(GraphNode::Process(process.clone()));
            for op in process.ops().iter() {
                if let Some(file) = op.file() {
                    let fnode = dag.add_node(GraphNode::File(file));
                    dag.add_edge(&op.name(),pnode.clone(), fnode);
                }
            }
        }

        Self {
            dag
        }
    }
}

impl Display for DependencyGraph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for node in self.dag.nodes() {
            for neighbor in node.neighbors() {
                if let Some(edges) = self.dag.edges_between(node.clone(), neighbor.clone()) {
                    for edge in edges {
                        writeln!(f, "   {} --> {} [{}]", node.data(), neighbor.data(), edge.label());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;
    use crate::deps::DependencyGraph;
    use crate::error::Error;
    use crate::file::File;
    use crate::op::Operation1;
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

        p1.add_op( Operation1::read(f1.clone(), 1, 1));
        p1.add_op( Operation1::mknod(f2.clone()));
        p1.add_op( Operation1::write(f2.clone(), "something".to_string(), 1, 1));
        p1.add_op( Operation1::write(f4.clone(), "something".to_string(), 1, 1));
        p2.add_op( Operation1::remove(f2.clone()));
        p2.add_op( Operation1::mkdir(d1.clone(), "a_mode".to_string()));
        p2.add_op( Operation1::write(f2.clone(), "something".to_string(), 1, 1));
        p3.add_op( Operation1::stat(d1.clone()));
        p3.add_op( Operation1::read(f3.clone(), 1, 1));
        p3.add_op( Operation1::read(f4.clone(), 1, 1));


        processes.push(p1);
        processes.push(p2);
        processes.push(p3);

        processes
    }

    #[test]
    fn dep_graph() -> Result<(), Error> {
        let deg_graph = DependencyGraph::new(processes());

        println!("{}", deg_graph);

        Ok(())
    }
}