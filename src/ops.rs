use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, PartialEq)]
pub enum OperationType {
    Read,
    Write(String), // the input string is the content of the write op
    Mkdir(String), // the input string is the mode, e.g, 0777
    Mknod,
    OpenAt,
    GetRandom,
    NoOp
}
pub struct Operation {
    pub kind: OperationType,
    pub len: Option<usize>,
    pub offset: Option<i32>,
    pub path: Option<String>,
}

impl Operation {
    fn new(kind: OperationType, len: Option<usize>, offset: Option<i32>, path: Option<String>) -> Self {
        Operation {
            kind,
            len,
            offset,
            path,
        }
    }

    pub fn read(size: usize, offset: i32, path: String) -> Self {
        Operation::new(OperationType::Read, Some(size), Some(offset), Some(path))
    }

    pub fn no_op() -> Self {
        Operation::new(OperationType::NoOp, None, None, None)
    }

    pub fn mkdir(mode: String, path: String) -> Self {
        Operation::new(OperationType::Mkdir(mode), None, None, Some(path))
    }

    pub fn mknod(size: usize, offset: i32, path: String) -> Self {
        Operation::new(OperationType::Mknod, Some(size), Some(offset), Some(path))
    }

    pub fn open_at(offset: i32, path: String) -> Self {
        Operation::new(OperationType::OpenAt, None, Some(offset), Some(path))
    }

    pub fn write(content: String, size: usize, offset: i32, path: String) -> Self {
        Operation::new(OperationType::Write(content), Some(size), Some(offset), Some(path))
    }

    pub fn get_random(size: usize) -> Self {
        Operation::new(OperationType::GetRandom, Some(size), None, None)
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let path = self.path.clone().unwrap_or("".to_string());
        let offset = self.offset.unwrap_or(0);
        let len = self.len.unwrap_or(0);

        match &self.kind {
            &OperationType::Mkdir(ref mode) => write!(f, "mkdir({}, {})", path, mode),
            &OperationType::Mknod => write!(f, "mknod({}, {}, {})", path, offset, len),
            &OperationType::Read => write!(f, "read({}, {}, {})", path, offset, len),
            &OperationType::Write(ref content) => write!(f, "write({}, {}, {}, {})", path, content, offset, len),
            &OperationType::OpenAt => write!(f, "open({}, {})", path, offset),
            &OperationType::GetRandom => write!(f, "get_random({})", len),
            &OperationType::NoOp => write!(f, "no-op"),
        }
    }
}