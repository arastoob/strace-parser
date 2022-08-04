// use crate::op::SharedOperation;
use crate::Operation;
use std::fmt::Formatter;
use std::hash::Hash;

#[derive(PartialEq, Eq, Clone, Hash)]
pub struct Process {
    pid: usize,
    ops: Vec<Operation>, // the process operations
}

impl Process {
    pub fn new(pid: usize) -> Self {
        Self { pid, ops: vec![] }
    }

    pub fn add_op(&mut self, op: Operation) {
        self.ops.push(op);
    }

    pub fn ops(&self) -> &Vec<Operation> {
        &self.ops
    }

    pub fn ops_mut(&mut self) -> &mut Vec<Operation> {
        &mut self.ops
    }

    pub fn pid(&self) -> usize {
        self.pid
    }
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for op in self.ops.iter() {
            writeln!(f, "{} {}", self.pid, op)?;
        }

        Ok(())
    }
}
