use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};

#[derive(Debug)]
struct Node<N>
    where N: std::hash::Hash + std::cmp::PartialEq
{
    data: N,
    pub neighbors: RefCell<Vec<Rc<Node<N>>>>,
    pub parent: RefCell<Weak<Node<N>>>
}

impl <N: std::hash::Hash + std::cmp::PartialEq>PartialEq for Node<N> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq>Hash for Node<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq>Eq for Node<N> {}

impl <N: std::hash::Hash + std::cmp::PartialEq>Node<N> {
    pub fn new(data: N) -> Self {
        Self {
            data,
            neighbors: RefCell::new(vec![]),
            parent: RefCell::new(Weak::new())
        }
    }

    pub fn neighbors(&self) -> Vec<Rc<Node<N>>> {
       self.neighbors.borrow().to_vec()
    }

    pub fn parent(&self) -> Option<Rc<Node<N>>> {
        self.parent.borrow().upgrade()
    }
}




struct Edge<N>
    where N: std::hash::Hash + std::cmp::PartialEq
{
    label: String,
    source: Rc<Node<N>>,
    target: Rc<Node<N>>,
}

impl <N: std::hash::Hash + std::cmp::PartialEq>PartialEq for Edge<N> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.target == other.target
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq>Hash for Edge<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.target.hash(state);
        self.label.hash(state);
    }
}

impl <N: std::hash::Hash + std::cmp::PartialEq>Eq for Edge<N> {}

impl<N: std::hash::Hash + std::cmp::PartialEq> Edge<N> {
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
}



pub struct DAG<N>
    where N: std::hash::Hash + std::cmp::PartialEq
{
    nodes: HashSet<Rc<Node<N>>>,
    edges: HashSet<Rc<Edge<N>>>
}

impl <N: std::hash::Hash + std::cmp::PartialEq>DAG<N> {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            edges: HashSet::new()
        }
    }

    pub fn add_node(&mut self, data: N) -> Rc<Node<N>> {
        let node = Rc::new(Node::new(data));
        if !self.nodes.contains(&node) {
            self.nodes.insert(node.clone());
        }

        node
    }

    pub fn node_exist(&self, node: &Rc<Node<N>>) -> bool {
        self.nodes.contains(node)
    }

    pub fn add_edge(&mut self, label: &str, source: Rc<Node<N>>, target: Rc<Node<N>>) -> Rc<Edge<N>> {
        assert!(self.node_exist(&source));
        assert!(self.node_exist(&target));

        // fix the neighbors and parent
        source.neighbors.borrow_mut().push(target.clone());
        *target.parent.borrow_mut() = Rc::downgrade(&source);

        let edge = Rc::new(Edge::new(label.to_string(), source, target));
        if !self.edges.contains(&edge) {
            self.edges.insert(edge.clone());
        }

        edge
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
