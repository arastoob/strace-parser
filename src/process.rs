use crate::op::SharedOperation;
use std::fmt::Formatter;
use std::hash::Hash;

#[derive(PartialEq, Eq, Clone, Hash)]
pub struct Process {
    pid: usize,
    ops: Vec<SharedOperation>, // the process operations
}

impl Process {
    pub fn new(pid: usize) -> Self {
        Self { pid, ops: vec![] }
    }

    pub fn add_op(&mut self, op: SharedOperation) {
        self.ops.push(op);
    }

    pub fn ops(&self) -> &Vec<SharedOperation> {
        &self.ops
    }

    pub fn ops_mut(&mut self) -> &mut Vec<SharedOperation> {
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
