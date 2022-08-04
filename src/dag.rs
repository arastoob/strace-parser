use std::cell::{Ref, RefCell, RefMut};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use std::slice::Iter;
use crate::error::Error;

///
/// A Directed Acyclic Graph
/// The DAG can have multiple edges between two nodes, of course with different labels
///
pub struct DAG<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    nodes: Vec<Rc<Node<N>>>,
    edges: Vec<Rc<Edge<N, L>>>,
}

#[allow(dead_code)]
impl<N, L> DAG<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }

    pub fn from_edges(edges: Vec<Rc<Edge<N, L>>>) -> Self {
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

        Self { nodes, edges }
    }

    pub fn add_node(&mut self, data: N) -> Result<Rc<Node<N>>, Error> {
        let node = Rc::new(Node::new(data));
        if !self.node_exist(&node) {
            self.nodes.push(node.clone());
            Ok(node)
        } else {
            Ok(self.nodes
                .iter()
                .find(|n| n.data == node.data)
                .ok_or(Error::NoneValue(format!("the find result of {}", node.data())))?
                .clone())
        }
    }

    // node exist in the dag?
    pub fn node_exist(&self, node: &Rc<Node<N>>) -> bool {
        self.nodes.contains(node)
    }

    // edge exist in the dag?
    pub fn edge_exist(&self, edge: &Rc<Edge<N, L>>) -> bool {
        self.edges.contains(edge)
    }

    pub fn add_edge(
        &mut self,
        label: L,
        source: Rc<Node<N>>,
        target: Rc<Node<N>>,
    ) -> Rc<Edge<N, L>> {
        // the source and target node exist
        assert!(self.node_exist(&source));
        assert!(self.node_exist(&target));

        // fix the neighbors and parent
        source.add_outgoing_neighbor(target.clone());
        target.add_incoming_neighbor(&source);

        let edge = Rc::new(Edge::new(label, source, target));
        if !self.edge_exist(&edge) {
            self.edges.push(edge.clone());
        }

        edge
    }

    pub fn remove_edge(&mut self, edge: Rc<Edge<N, L>>) {
        let source = edge.source();
        let target = edge.target();
        if self.edge_exist(&edge) {
            self.edges.retain(|e| *e != edge);
        }

        target.remove_incoming_neighbor(&source);
        source.remove_outgoing_neighbor(target);
    }

    pub fn remove_node(&mut self, node: &Rc<Node<N>>) {
        if self.node_exist(&node) {
            // remove incoming and outgoing edges
            if let Some(edges) = self.edges_to(node.clone()) {
                for edge in edges {
                    self.remove_edge(edge);
                }
            }

            if let Some(edges) = self.edges_from(node.clone()) {
                for edge in edges {
                    self.remove_edge(edge);
                }
            }

            // now, remove the node itself
            self.nodes.retain(|n| *n != *node);
        }
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

    pub fn edges(&self) -> Iter<'_, Rc<Edge<N, L>>> {
        self.edges.iter()
    }

    // get the edges between source and target
    pub fn edges_between(
        &self,
        source: Rc<Node<N>>,
        target: Rc<Node<N>>,
    ) -> Option<Vec<Rc<Edge<N, L>>>> {
        let edges = self
            .edges
            .iter()
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
    pub fn edges_from(&self, source: Rc<Node<N>>) -> Option<Vec<Rc<Edge<N, L>>>> {
        let edges = self
            .edges
            .iter()
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
    pub fn edges_to(&self, target: Rc<Node<N>>) -> Option<Vec<Rc<Edge<N, L>>>> {
        let edges = self
            .edges
            .iter()
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
        node.out_degree()
    }

    pub fn topological_sort(&self) -> Vec<Rc<Node<N>>> {
        let mut stack: Vec<Rc<Node<N>>> = vec![];
        for node in self.nodes() {
            self.get_order(node.clone(), &mut stack);
        }
        stack.reverse();

        stack
    }

    fn get_order(&self, node: Rc<Node<N>>, stack: &mut Vec<Rc<Node<N>>>) {
        for outgoing_neighbor in node.outgoing_neighbors().iter() {
            self.get_order(outgoing_neighbor.clone(), stack);
        }

        if !stack.contains(&node) {
            stack.push(node);
        }
    }

}

impl<N, L> Display for DAG<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let nodes = self.topological_sort();
        for node in nodes.iter() {
            for neighbor in node.outgoing_neighbors().iter() {
                if let Some(edges) = self.edges_between(node.clone(), neighbor.clone()) {
                    for edge in edges {
                        writeln!(
                            f,
                            "\t{} --> {} [{}]",
                            node.data(),
                            neighbor.data(),
                            edge.label()
                        )?;
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
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
{
    data: RefCell<N>,                          // node data
    out_neighbors: RefCell<Vec<Rc<Node<N>>>>,  // outgoing node's neighbors
    in_neighbors: RefCell<Vec<Weak<Node<N>>>>, // incoming node's parent
    in_degree: RefCell<usize>,
}

impl<N> PartialEq for Node<N>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
{
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<N> Hash for Node<N>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.borrow().hash(state);
    }
}

impl<N> Eq for Node<N> where N: std::hash::Hash + std::cmp::PartialEq + Display {}

#[allow(dead_code)]
impl<N> Node<N>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
{
    pub fn new(data: N) -> Self {
        Self {
            data: RefCell::new(data),
            out_neighbors: RefCell::new(vec![]),
            in_neighbors: RefCell::new(vec![]),
            in_degree: RefCell::new(0),
        }
    }

    pub fn outgoing_neighbors(&self) -> Ref<'_, Vec<Rc<Node<N>>>> {
        self.out_neighbors.borrow()
    }

    pub fn outgoing_neighbors_mut(&self) -> RefMut<'_, Vec<Rc<Node<N>>>> {
        self.out_neighbors.borrow_mut()
    }

    pub fn incoming_neighbors(&self) -> Ref<'_, Vec<Weak<Node<N>>>> {
        self.in_neighbors.borrow()
    }

    pub fn incoming_neighbors_mut(&self) -> RefMut<'_, Vec<Weak<Node<N>>>> {
        self.in_neighbors.borrow_mut()
    }

    pub fn data(&self) -> Ref<'_, N> {
        self.data.borrow()
    }

    pub fn data_mut(&self) -> RefMut<'_, N> {
        self.data.borrow_mut()
    }

    fn add_outgoing_neighbor(&self, neighbor: Rc<Node<N>>) {
        if self
            .out_neighbors
            .borrow()
            .iter()
            .find(|n| **n == neighbor)
            .is_none()
        {
            self.out_neighbors.borrow_mut().push(neighbor);
        }
    }

    fn remove_outgoing_neighbor(&self, neighbor: Rc<Node<N>>) {
        self
            .out_neighbors
            .borrow_mut()
            .retain(|n| **n != *neighbor);
    }

    fn add_incoming_neighbor(&self, neighbor: &Rc<Node<N>>) {
        if self
            .in_neighbors
            .borrow()
            .iter()
            .find(|n| Weak::ptr_eq(n, &Rc::downgrade(neighbor)))
            .is_none()
        {
            self.in_neighbors.borrow_mut().push(Rc::downgrade(neighbor));
            *self.in_degree.borrow_mut() += 1;
        }
    }

    fn remove_incoming_neighbor(&self, neighbor: &Rc<Node<N>>) {
        self
            .in_neighbors
            .borrow_mut()
            .retain(|n| !Weak::ptr_eq(n, &Rc::downgrade(neighbor)));
        *self.in_degree.borrow_mut() -= 1;
    }

    pub fn in_degree(&self) -> usize {
        self.in_degree.borrow().clone()
    }

    pub fn out_degree(&self) -> usize {
        self.out_neighbors.borrow().len()
    }
}

// The DAG edges
pub struct Edge<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    label: L,            // edge label
    source: Rc<Node<N>>, // edge's source node
    target: Rc<Node<N>>, // edge's target node
}

impl<N, L> PartialEq for Edge<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.target == other.target && self.label == other.label
    }
}

impl<N, L> Hash for Edge<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.target.hash(state);
        self.label.hash(state);
    }
}

impl<N, L> Eq for Edge<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
}

impl<N, L> Edge<N, L>
where
    N: std::hash::Hash + std::cmp::PartialEq + Display,
    L: std::hash::Hash + std::cmp::PartialEq + Clone + Display,
{
    pub fn new(label: L, source: Rc<Node<N>>, target: Rc<Node<N>>) -> Self {
        Self {
            label,
            source,
            target,
        }
    }

    pub fn source(&self) -> Rc<Node<N>> {
        self.source.clone()
    }

    pub fn target(&self) -> Rc<Node<N>> {
        self.target.clone()
    }

    pub fn label(&self) -> L {
        self.label.clone()
    }
}

#[cfg(test)]
mod test {
    use crate::dag::DAG;
    use std::rc::{Rc, Weak};
    use crate::error::Error;

    #[test]
    fn add_node() -> Result<(), Error> {
        let mut dag1: DAG<i32, String> = DAG::new();
        dag1.add_node(1)?;
        dag1.add_node(2)?;
        dag1.add_node(1)?;
        assert_eq!(dag1.nodes.len(), 2);

        let mut dag2: DAG<&str, String> = DAG::new();
        dag2.add_node("1")?;
        dag2.add_node("2")?;
        dag2.add_node("1")?;
        assert_eq!(dag2.nodes.len(), 2);

        Ok(())
    }

    #[test]
    fn add_edge() -> Result<(), Error> {
        let mut dag = DAG::new();
        let n1 = dag.add_node(1)?;
        let n2 = dag.add_node(2)?;

        let e1 = dag.add_edge("a_label", n1.clone(), n2.clone());
        // we cannot add repeated edge
        let _e2 = dag.add_edge("a_label", n1.clone(), n2.clone());
        // but we can have multiple edges with different labels between the same nodes
        let _e3 = dag.add_edge("another_label", n1.clone(), n2.clone());
        assert_eq!(dag.edges.len(), 2);

        assert_eq!(e1.source, n1);
        assert_eq!(e1.target, n2);

        Ok(())
    }

    #[test]
    fn remove_edge() -> Result<(), Error> {
        //
        //         n1            n4
        //        /  \           /
        //       /    \         /
        //    n1_n2  n1_n3     /
        //     /        \     /
        //    /          \   /
        //   V            V V
        //   n2            n3
        //

        let mut dag = DAG::new();
        let n1 = dag.add_node("n1")?;
        let n2 = dag.add_node("n2")?;
        let n3 = dag.add_node("n3")?;
        let n4 = dag.add_node("n4")?;

        dag.add_edge("n1_n2", n1.clone(), n2.clone());
        dag.add_edge("n1_n3", n1.clone(), n3.clone());
        dag.add_edge("n4_n3", n4.clone(), n3.clone());

        assert_eq!(dag.nodes.len(), 4);
        assert_eq!(dag.edges.len(), 3);

        let edges = dag.edges_to(n3.clone());
        assert!(edges.is_some());
        assert_eq!(edges.ok_or(Error::NoneValue("dag edges".to_string()))?.len(), 2);

        let edges = dag.edges_to(n2.clone());
        assert!(edges.is_some());
        let edge = edges.ok_or(Error::NoneValue("dag edges".to_string()))?[0].clone();
        assert_eq!(edge.label, "n1_n2");
        assert_eq!(edge.source, n1);
        assert_eq!(edge.target, n2);
        assert_eq!(dag.out_degree_of(n1.clone()), 2);

        dag.remove_edge(edge);

        // after remove:
        //         n1            n4
        //           \           /
        //            \         /
        //           n1_n3     /
        //              \     /
        //               \   /
        //                V V
        //   n2            n3
        //

        assert_eq!(dag.nodes.len(), 4);
        assert_eq!(dag.edges.len(), 2);
        assert_eq!(dag.in_degree_of(n2), 0);
        assert_eq!(dag.out_degree_of(n1), 1);

        Ok(())
    }

    #[test]
    fn remove_node() -> Result<(), Error> {
        //
        //         n1            n4
        //        /  \           /
        //       /    \         /
        //    n1_n2  n1_n3     /
        //     /        \     /
        //    /          \   /
        //   V            V V
        //   n2            n3
        //

        let mut dag = DAG::new();
        let n1 = dag.add_node("n1")?;
        let n2 = dag.add_node("n2")?;
        let n3 = dag.add_node("n3")?;
        let n4 = dag.add_node("n4")?;

        dag.add_edge("n1_n2", n1.clone(), n2.clone());
        dag.add_edge("n1_n3", n1.clone(), n3.clone());
        dag.add_edge("n4_n3", n4.clone(), n3.clone());

        assert_eq!(dag.nodes.len(), 4);
        assert_eq!(dag.edges.len(), 3);

        let edges = dag.edges_to(n3.clone());
        assert!(edges.is_some());
        assert_eq!(edges.ok_or(Error::NoneValue("dag edges".to_string()))?.len(), 2);

        let edges = dag.edges_to(n2.clone());
        assert!(edges.is_some());
        let edge = edges.ok_or(Error::NoneValue("dag edges".to_string()))?[0].clone();
        assert_eq!(edge.label, "n1_n2");
        assert_eq!(edge.source, n1);
        assert_eq!(edge.target, n2);
        assert_eq!(dag.out_degree_of(n1.clone()), 2);

        dag.remove_node(&n3);

        // after remove n3:
        //
        //         n1            n4
        //        /
        //       /
        //    n1_n2
        //     /
        //    /
        //   V
        //   n2
        //

        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.edges.len(), 1);
        assert_eq!(dag.out_degree_of(n1), 1);
        assert_eq!(dag.out_degree_of(n4), 0);

        Ok(())
    }

    #[test]
    fn dag() -> Result<(), Error> {
        //
        //         n1            n4
        //        /  \           /
        //       /    \         /
        //    n1_n2  n1_n3     /
        //     /        \     /
        //    /          \   /
        //   V            V V
        //   n2            n3
        //

        let mut dag = DAG::new();
        let n1 = dag.add_node("n1")?;
        let n2 = dag.add_node("n2")?;
        let n3 = dag.add_node("n3")?;
        let n4 = dag.add_node("n4")?;

        dag.add_edge("n1_n2", n1.clone(), n2.clone());
        dag.add_edge("n1_n3", n1.clone(), n3.clone());
        dag.add_edge("n4_n3", n4.clone(), n3.clone());

        assert_eq!(dag.nodes.len(), 4);
        assert_eq!(dag.edges.len(), 3);

        assert_eq!(n1.outgoing_neighbors().len(), 2);

        assert_eq!(n2.incoming_neighbors().len(), 1);
        assert!(Weak::ptr_eq(
            &n2.incoming_neighbors()[0],
            &Rc::downgrade(&n1)
        ));

        assert_eq!(n3.incoming_neighbors().len(), 2);
        assert!(Weak::ptr_eq(
            &n3.incoming_neighbors()[0],
            &Rc::downgrade(&n1)
        ));
        assert!(Weak::ptr_eq(
            &n3.incoming_neighbors()[1],
            &Rc::downgrade(&n4)
        ));

        Ok(())
    }

    #[test]
    fn topological_sort() -> Result<(), Error> {

        let mut dag = DAG::new();
        let node0 = dag.add_node(0)?;
        let node1 = dag.add_node(1)?;
        let node2 = dag.add_node(2)?;
        let node3 = dag.add_node(3)?;
        let node4 = dag.add_node(4)?;

        dag.add_edge("", node0, node2.clone());
        dag.add_edge("", node1, node2.clone());
        dag.add_edge("", node2.clone(), node3.clone());
        dag.add_edge("", node3.clone(), node4.clone());

        let mut ordered = dag.topological_sort();

        // the topological sort should be [0, 1, 2, 3, 4] or [1, 0, 2, 3, 4]

        assert_eq!(ordered.pop(), Some(node4));
        assert_eq!(ordered.pop(), Some(node3));
        assert_eq!(ordered.pop(), Some(node2));

        Ok(())
    }
}
