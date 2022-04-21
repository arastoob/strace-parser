use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use crate::op::Operation1;

#[derive(Debug, PartialEq, Hash, Eq)]
pub struct Process {
    pid: usize,
    ops: Vec<Operation1>
}

impl Process {
    pub fn new(pid: usize) -> Self {
        Self {
            pid,
            ops: vec![]
        }
    }

    pub fn add_op(&mut self, op: Operation1) {
        self.ops.push(op);
    }

    pub fn ops(&self) -> &Vec<Operation1> {
        self.ops.as_ref()
    }

    pub fn pid(&self) -> usize {
        self.pid
    }
}