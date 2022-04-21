use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use crate::Operation;

/// Order the serial operations done by the processes.
/// If a resource is accessed by more than one process, the accesses should be ordered based on
/// their operation type. For example:
///     (p1 read f1)    and    (p2 create f1)
/// the order of execution should be:
///     (p2 create f1)
///     (p1 read f1)
///
#[derive(Debug)]
pub struct OrderManager {
    ops: Vec<(usize, Operation)>,
    pub resources: HashMap<String, Vec<Access>> // a map from resource path to a list of its accesses
}

#[derive(Debug, Clone)]
pub struct Access {
    by: usize, // pid
    op: Rc<Operation>,
    order: u16 // the order of operation, the lower means the operation should be executed sooner
}

impl OrderManager {
    pub fn new(ops: &Vec<(usize, Operation)>) -> Self {
        let mut resources: HashMap<String, Vec<Access>>  = HashMap::new();
        for (pid, op) in ops.iter() {
            let op = Rc::new(op.to_owned());
            if let Some(path) = op.path() {
                match resources.get_mut(&path) {
                    Some(accesses) => {
                        let order = OrderManager::op_order(&op);
                        let access = Access { by: *pid, op: op.clone(), order };
                        accesses.push(access);
                    },
                    None => {
                        let order = OrderManager::op_order(&op);
                        let access = Access { by: *pid, op,  order};
                        resources.insert(path, vec![access]);
                    }
                }
            }
        }

        Self {
            ops: ops.to_vec(),
            resources
        }
    }

    pub fn order(&mut self) -> (Vec<(usize, Operation)>, Vec<(usize, Operation)>) {

        let serial_ops = self.serial_ops();

        for (spid, sop) in serial_ops.iter() {
            let idx = self.ops.iter().position(|(pid, op)| pid == spid && op == sop);
            if let Some(idx) = idx {
                self.ops.remove(idx);
            }
        }

        (serial_ops, self.ops.clone())
    }

    // find the resources that have been accessed more than one time
    fn multiple_accesses(&self) -> HashMap<String, Vec<Access>> {
        let mut multi: HashMap<String, Vec<Access>>  = HashMap::new();

        for (resource, accesses) in self.resources.iter() {
            if accesses.len() > 1 {
                // the resource is accessed multiple times, so check its accessors
                let mut accessed_by = HashMap::new();
                for access in accesses {
                    accessed_by.insert(access.by, &access.op);
                }

                if accessed_by.len() > 1 {
                    multi.insert(resource.clone(), accesses.clone());
                }
            }
        }
        multi
    }

    fn serial_ops(&self) -> Vec<(usize, Operation)> {
        let mut serial_ops = vec![];
        let mut multiple_accesses = self.multiple_accesses();

        for (_resource, accesses) in multiple_accesses.iter_mut() {
            accesses.sort_by(|a, b| a.order.cmp(&b.order));

            for access in accesses {
                serial_ops.push((access.by, access.op.deref().to_owned()))
            }
        }

        serial_ops
    }

    fn op_order(op: &Operation) -> u16 {
        match op {
            &Operation::Mkdir(_, _) => 0,
            &Operation::Mknod(_) => 0,
            &Operation::OpenAt(_, _) => 1,
            &Operation::Write(_, _, _, _) => 2,
            &Operation::Read(_, _, _) => 3,
            &Operation::Truncate(_) => 4,
            &Operation::Stat(_) => 5,
            &Operation::Fstat(_) => 5,
            &Operation::Statx(_) => 5,
            &Operation::StatFS(_) => 5,
            &Operation::Fstatat(_) => 5,
            &Operation::Rename(_, _) => 6,
            &Operation::Remove(_) => 7,
            _ => 10,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Operation;
    use crate::order_manager::OrderManager;

    fn ops() -> Vec<(usize, Operation)> {
        let mut ops = vec![];
        let f1 = "f1".to_string();
        let f2 = "f2".to_string();
        let d1 = "d1".to_string();
        let f3 = "f3".to_string();
        let f4 = "f4".to_string();

        ops.push((1, Operation::read(1, 1, f1)));
        ops.push((1, Operation::mknod(f2.clone())));
        ops.push((1, Operation::write("something".to_string(), 1, 1, f2.clone())));
        ops.push((1, Operation::write("something".to_string(), 1, 1,f4.clone())));
        ops.push((2, Operation::remove(f2.clone())));
        ops.push((2, Operation::mkdir(d1.clone(), "a_mode".to_string())));
        ops.push((2, Operation::write("something".to_string(), 1, 1, f2.clone())));
        ops.push((3, Operation::stat(d1)));
        ops.push((3, Operation::read(1, 1, f3)));
        ops.push((3, Operation::read(1, 1, f4)));

        ops
    }

    #[test]
    fn order_manager() -> Result<(), Box<dyn std::error::Error>> {
        let ops = ops();

        let mut om = OrderManager::new(&ops);

        let (serial, parallel) = om.order();

        let mut expected_parallel = vec![
            (1, Operation::read(1, 1, "f1".to_string())),
            (3, Operation::read(1, 1, "f3".to_string()))
        ];

        let mut expected_serial = ops.clone();
        for (pid, op) in expected_parallel.iter() {
            let idx = expected_serial.iter().position(|(spid, sop)| pid == spid && op == sop);
            if let Some(idx) = idx {
                expected_serial.remove(idx);
            }
        }

        assert_eq!(expected_parallel.len(), parallel.len());
        assert_eq!(expected_serial.len(), serial.len());

        Ok(())
    }
}