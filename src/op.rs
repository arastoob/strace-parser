use crate::file::File;
use std::fmt;
use std::fmt::Formatter;
use std::sync::Arc;

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum Operation {
    Read(Arc<File>, i32, usize),          // args: FileDir, offset, len
    Write(Arc<File>, i32, usize, String), // args: FileDir, offset, len, content
    Mkdir(Arc<File>, String),             // args: path, mode
    Mknod(Arc<File>),                     // args: path
    Remove(Arc<File>),                    // args: FileDir
    Rename(Arc<File>, String),            // args: FileDir, new_name
    OpenAt(Arc<File>, i32),               // args: FileDir, offset
    Truncate(Arc<File>),                  // args: FileDir
    GetRandom(usize),                    // args: len
    Stat(Arc<File>),                      // args: path
    Fstat(Arc<File>),                     // args: path
    Statx(Arc<File>),                     // args: path
    StatFS(Arc<File>),                    // args: path
    Fstatat(Arc<File>),                   // args: path
    Clone(usize),                        // args: process id of the cloned process
    NoOp,
}

impl Operation {
    pub fn read(file: Arc<File>, len: usize, offset: i32) -> Self {
        Operation::Read(file, offset, len)
    }

    pub fn no_op() -> Self {
        Operation::NoOp
    }

    pub fn mkdir(file: Arc<File>, mode: String) -> Self {
        Operation::Mkdir(file, mode)
    }

    pub fn mknod(file: Arc<File>) -> Self {
        Operation::Mknod(file)
    }

    pub fn remove(file: Arc<File>) -> Self {
        Operation::Remove(file)
    }

    pub fn open_at(file: Arc<File>, offset: i32) -> Self {
        Operation::OpenAt(file, offset)
    }

    pub fn truncate(file: Arc<File>) -> Self {
        Operation::Truncate(file)
    }

    pub fn write(file: Arc<File>, content: String, len: usize, offset: i32) -> Self {
        Operation::Write(file, offset, len, content)
    }

    pub fn get_random(len: usize) -> Self {
        Operation::GetRandom(len)
    }

    pub fn stat(file: Arc<File>) -> Self {
        Operation::Stat(file)
    }

    pub fn fstat(file: Arc<File>) -> Self {
        Operation::Fstat(file)
    }

    pub fn statx(file: Arc<File>) -> Self {
        Operation::Statx(file)
    }

    pub fn statfs(file: Arc<File>) -> Self {
        Operation::StatFS(file)
    }

    pub fn fstatat(file: Arc<File>) -> Self {
        Operation::Fstatat(file)
    }

    pub fn rename(file: Arc<File>, to: String) -> Self {
        Operation::Rename(file, to)
    }

    pub fn clone_op(pid: usize) -> Self {
        Operation::Clone(pid)
    }

    pub fn file(&self) -> Option<Arc<File>> {
        match &self {
            &Operation::Mkdir(file, _) => Some(file.clone()),
            &Operation::Mknod(file) => Some(file.clone()),
            &Operation::Remove(file) => Some(file.clone()),
            &Operation::Read(file, _, _) => Some(file.clone()),
            &Operation::Write(file, _, _, _) => Some(file.clone()),
            &Operation::OpenAt(file, _) => Some(file.clone()),
            &Operation::Truncate(file) => Some(file.clone()),
            &Operation::GetRandom(_) => None,
            &Operation::Stat(file) => Some(file.clone()),
            &Operation::Fstat(file) => Some(file.clone()),
            &Operation::Statx(file) => Some(file.clone()),
            &Operation::StatFS(file) => Some(file.clone()),
            &Operation::Fstatat(file) => Some(file.clone()),
            &Operation::Rename(file, _) => Some(file.clone()),
            &Operation::Clone(_) => None,
            &Operation::NoOp => None,
        }
    }

    pub fn name(&self) -> String {
        match &self {
            &Operation::Mkdir(_, _) => "Mkdir".to_string(),
            &Operation::Mknod(_) => "Mknod".to_string(),
            &Operation::Remove(_) => "Remove".to_string(),
            &Operation::Read(_, _, _) => "Read".to_string(),
            &Operation::Write(_, _, _, _) => "Write".to_string(),
            &Operation::OpenAt(_, _) => "OpenAt".to_string(),
            &Operation::Truncate(_) => "Truncate".to_string(),
            &Operation::GetRandom(_) => "GetRandom".to_string(),
            &Operation::Stat(_) => "Stat".to_string(),
            &Operation::Fstat(_) => "Fstat".to_string(),
            &Operation::Statx(_) => "Statx".to_string(),
            &Operation::StatFS(_) => "StatFS".to_string(),
            &Operation::Fstatat(_) => "Fstatat".to_string(),
            &Operation::Rename(_, _) => "Rename".to_string(),
            &Operation::Clone(_) => "Clone".to_string(),
            &Operation::NoOp => "NoOp".to_string(),
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            &Operation::Mkdir(ref file, ref mode) => write!(f, "mkdir({}, {})", file, mode),
            &Operation::Mknod(ref file) => {
                write!(f, "mknod({})", file)
            }
            &Operation::Remove(ref file) => write!(f, "remove({})", file),
            &Operation::Read(ref file, ref offset, ref len) => {
                write!(f, "read({}, {}, {})", file, offset, len)
            }
            &Operation::Write(ref file, ref offset, ref len, ref content) => {
                write!(f, "write({}, {}, {}, {})", file, offset, len, content)
            }
            &Operation::OpenAt(ref file, ref offset) => write!(f, "open({}, {})", file, offset),
            &Operation::Truncate(ref file) => write!(f, "truncate({})", file),
            &Operation::GetRandom(ref len) => write!(f, "get_random({})", len),
            &Operation::Stat(ref path) => write!(f, "stat({})", path),
            &Operation::Fstat(ref path) => write!(f, "fstat({})", path),
            &Operation::Statx(ref path) => write!(f, "statx({})", path),
            &Operation::StatFS(ref path) => write!(f, "statfs({})", path),
            &Operation::Fstatat(ref path) => write!(f, "fstatat({})", path),
            &Operation::Rename(ref file, ref to) => {
                write!(f, "rename({} {})", file, to)
            }
            &Operation::Clone(ref pid) => write!(f, "clone({})", pid),
            &Operation::NoOp => write!(f, "no-op"),
        }
    }
}
