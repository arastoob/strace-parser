use crate::op::Operation;
use std::fmt::Formatter;
use std::hash::Hash;
use std::slice::Iter;

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct Process {
    pid: usize,
    ops: Vec<(usize, Operation)>, // the process operations and their unique ids
}

impl Process {
    pub fn new(pid: usize) -> Self {
        Self { pid, ops: vec![] }
    }

    pub fn add_op(&mut self, id: usize, op: Operation) {
        self.ops.push((id, op));
    }

    pub fn remove_op(&mut self, id: &usize) {
        self.ops.retain(|(op_id, _)| op_id != id);
    }

    pub fn ops(&self) -> Iter<'_, (usize, Operation)> {
        self.ops.iter()
    }

    pub fn pid(&self) -> usize {
        self.pid
    }
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (op_id, op) in self.ops.iter() {
            writeln!(f, "{} {}:{}", self.pid, op_id, op)?;
        }

        Ok(())
    }
}
