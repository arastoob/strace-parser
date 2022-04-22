use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use std::slice::Iter;

///
/// A Directed Acyclic Graph
/// The DAG can have multiple edges between two nodes, of course with different labels
///
pub struct DAG<N>
    where N: std::hash::Hash + std::cmp::PartialEq + Clone + Display
{
    nodes: Vec<Rc<Node<N>>>,
    edges: Vec<Rc<Edge<N>>>
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>DAG<N> {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![]
        }
    }

    pub fn from_edges(edges: Vec<Rc<Edge<N>>>) -> Self {
        let mut nodes = vec![];
        for edge in edges.iter() {
            let source = edge.source();
            let target = edge.target();
            if !nodes.contains(&source) {
                nodes.push(source);
            }

            if !nodes.contains(&target) {
                nodes.push(target);
            }
        }

        Self {
            nodes,
            edges
        }
    }

    pub fn add_node(&mut self, data: N) -> Rc<Node<N>> {
        let node = Rc::new(Node::new(data));
        if !self.node_exist(&node) {
            self.nodes.push(node.clone());
            node
        } else {
            self.nodes.iter().find(|n| n.data == node.data).unwrap().clone()
        }


    }

    // node exist in the dag?
    pub fn node_exist(&self, node: &Rc<Node<N>>) -> bool {
        self.nodes.contains(node)
    }

    // edge exist in the dag?
    pub fn edge_exist(&self, edge: &Rc<Edge<N>>) -> bool {
        self.edges.contains(edge)
    }

    pub fn add_edge(&mut self, label: &str, source: Rc<Node<N>>, target: Rc<Node<N>>) -> Rc<Edge<N>> {
        // the source and target node exist
        assert!(self.node_exist(&source));
        assert!(self.node_exist(&target));

        // fix the neighbors and parent
        source.add_neighbor(target.clone());
        target.add_parent(&source);

        let edge = Rc::new(Edge::new(label.to_string(), source, target));
        if !self.edge_exist(&edge) {
            self.edges.push(edge.clone());
        }

        edge
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn nodes(&self) -> Iter<'_, Rc<Node<N>>> {
        self.nodes.iter()
    }

    pub fn edges(&self) -> Iter<'_, Rc<Edge<N>>> {
        self.edges.iter()
    }

    // get the edges between source and target
    pub fn edges_between(&self, source: Rc<Node<N>>, target: Rc<Node<N>>) -> Option<Vec<Rc<Edge<N>>>> {
        let edges  = self.edges.iter()
            .filter(|edge| edge.source == source && edge.target == target)
            .map(|edge| edge.clone())
            .collect::<Vec<_>>();

        if edges.len() != 0 {
            Some(edges)
        } else {
            None
        }
    }

    // get the edges starting from a node
    pub fn edges_from(&self, source: Rc<Node<N>>) -> Option<Vec<Rc<Edge<N>>>> {
        let edges  = self.edges.iter()
            .filter(|edge| edge.source == source)
            .map(|edge| edge.clone())
            .collect::<Vec<_>>();

        if edges.len() != 0 {
            Some(edges)
        } else {
            None
        }
    }

    // get the edges going into a node
    pub fn edges_to(&self, target: Rc<Node<N>>) -> Option<Vec<Rc<Edge<N>>>> {
        let edges  = self.edges.iter()
            .filter(|edge| edge.target == target)
            .map(|edge| edge.clone())
            .collect::<Vec<_>>();

        if edges.len() != 0 {
            Some(edges)
        } else {
            None
        }
    }

    pub fn in_degree_of(&self, node: Rc<Node<N>>) -> usize {
        node.in_degree()
    }

    pub fn out_degree_of(&self, node: Rc<Node<N>>) -> usize {
        node.neighbors().len()
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Display for DAG<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for node in self.nodes() {
            for neighbor in node.neighbors() {
                if let Some(edges) = self.edges_between(node.clone(), neighbor.clone()) {
                    for edge in edges {
                        writeln!(f, "   {} --> {} [{}]", node.data(), neighbor.data(), edge.label());
                    }
                }
            }
        }
        Ok(())
    }
}


// The DAG nodes
#[derive(Debug)]
pub struct Node<N>
    where N: std::hash::Hash + std::cmp::PartialEq + Clone + Display
{
    data: N, // node data
    neighbors: RefCell<Vec<Rc<Node<N>>>>, // node's neighbors
    parent: RefCell<Weak<Node<N>>>, // node's parent
    in_degree: RefCell<usize>,
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>PartialEq for Node<N> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Hash for Node<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Eq for Node<N> {}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Node<N> {
    pub fn new(data: N) -> Self {
        Self {
            data,
            neighbors: RefCell::new(vec![]),
            parent: RefCell::new(Weak::new()),
            in_degree: RefCell::new(0)
        }
    }

    pub fn neighbors(&self) -> Vec<Rc<Node<N>>> {
       let n = self.neighbors.borrow().iter()
           .map(|n| n.clone()).collect::<Vec<_>>();
        n
    }

    pub fn parent(&self) -> Option<Rc<Node<N>>> {
        self.parent.borrow().upgrade()
    }

    pub fn data(&self) -> N {
        self.data.clone()
    }

    pub fn add_neighbor(&self, neighbor: Rc<Node<N>>) {
        if self.neighbors.borrow().iter().find(|n| **n == neighbor).is_none() {
            self.neighbors.borrow_mut().push(neighbor);
        }
    }

    pub fn add_parent(&self, parent: &Rc<Node<N>>) {
        *self.parent.borrow_mut() = Rc::downgrade(parent);
        *self.in_degree.borrow_mut() += 1;
    }

    pub fn in_degree(&self) -> usize {
        self.in_degree.borrow().clone()
    }
}

// The DAG edges
pub struct Edge<N>
    where N: std::hash::Hash + std::cmp::PartialEq + Clone + Display
{
    label: String, // edge label
    source: Rc<Node<N>>, // edge's source node
    target: Rc<Node<N>>, // edge's target node
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>PartialEq for Edge<N> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.target == other.target && self.label == other.label
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Hash for Edge<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.target.hash(state);
        self.label.hash(state);
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq + Clone + Display>Eq for Edge<N> {}

impl<N: std::hash::Hash + std::cmp::PartialEq + Clone + Display> Edge<N> {
    pub fn new(label: String, source: Rc<Node<N>>, target: Rc<Node<N>>) -> Self {
        Self {
            label,
            source,
            target
        }
    }

    pub fn source(&self) -> Rc<Node<N>> {
        self.source.clone()
    }

    pub fn target(&self) -> Rc<Node<N>> {
        self.target.clone()
    }

    pub fn label(&self) -> String {
        self.label.clone()
    }
}


#[cfg(test)]
mod test {
    use crate::dag::DAG;

    #[test]
    fn add_node() {
        let mut dag1 = DAG::new();
        dag1.add_node(1);
        dag1.add_node(2);
        dag1.add_node(1);
        assert_eq!(dag1.nodes.len(), 2);

        let mut dag2 = DAG::new();
        dag2.add_node("1");
        dag2.add_node("2");
        dag2.add_node("1");
        assert_eq!(dag2.nodes.len(), 2);
    }

    #[test]
    fn add_edge() {
        let mut dag = DAG::new();
        let n1 = dag.add_node(1);
        let n2 = dag.add_node(2);

        let e1 = dag.add_edge("a_label", n1.clone(), n2.clone());
        // we cannot add repeated edge
        let _e2 = dag.add_edge("a_label", n1.clone(), n2.clone());
        // but we can have multiple edges with different labels between the same nodes
        let _e3 = dag.add_edge("another_label", n1.clone(), n2.clone());
        assert_eq!(dag.edges.len(), 2);

        assert_eq!(e1.source, n1);
        assert_eq!(e1.target, n2);
    }

    #[test]
    fn dag() {

        //
        //         n1
        //        /  \
        //       /    \
        //    n1_n2  n1_n3
        //     /        \
        //    /          \
        //   V            V
        //   n2            n3
        //

        let mut dag = DAG::new();
        let n1 = dag.add_node("n1");
        let n2 = dag.add_node("n2");
        let n3 = dag.add_node("n3");

        dag.add_edge("n1_n2", n1.clone(), n2.clone());
        dag.add_edge("n1_n3", n1.clone(), n3.clone());

        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.edges.len(), 2);

        assert_eq!(n1.neighbors().len(), 2);

        assert!(n2.parent().is_some());
        assert_eq!(n2.parent().unwrap(), n1);

        assert!(n3.parent().is_some());
        assert_eq!(n3.parent().unwrap(), n1);
    }
}
