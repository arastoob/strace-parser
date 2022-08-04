use crate::file::File;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::sync::Arc;

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum OperationType {
    Read(Arc<File>, i32, usize),          // args: FileDir, offset, len
    Write(Arc<File>, i32, usize, String), // args: FileDir, offset, len, content
    Mkdir(Arc<File>, String),             // args: path, mode
    Mknod(Arc<File>),                     // args: path
    Remove(Arc<File>),                    // args: FileDir
    Rename(Arc<File>, String),            // args: FileDir, new_name
    OpenAt(Arc<File>, i32),               // args: FileDir, offset
    Truncate(Arc<File>),                  // args: FileDir
    GetRandom(usize),                     // args: len
    Stat(Arc<File>),                      // args: path
    Fstat(Arc<File>),                     // args: path
    Statx(Arc<File>),                     // args: path
    StatFS(Arc<File>),                    // args: path
    Fstatat(Arc<File>),                   // args: path
    Clone(usize),                         // args: process id of the cloned process
    Chdir(String),                        // args: the current directory path
    NoOp,
}

///
/// The operation that is done by a process
///
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Operation {
    op_type: OperationType, // type of operation
}

impl Operation {
    pub fn new(op_type: OperationType) -> Self {
        Operation {
            op_type,
        }
    }

    pub fn read(file: Arc<File>, len: usize, offset: i32) -> Self {
        Self::new(OperationType::Read(file, offset, len))
    }

    pub fn no_op() -> Self {
        Self::new(OperationType::NoOp)
    }

    pub fn mkdir(file: Arc<File>, mode: String) -> Self {
        Self::new(OperationType::Mkdir(file, mode))
    }

    pub fn mknod(file: Arc<File>) -> Self {
        Self::new(OperationType::Mknod(file))
    }

    pub fn remove(file: Arc<File>) -> Self {
        Self::new(OperationType::Remove(file))
    }

    pub fn open_at(file: Arc<File>, offset: i32) -> Self {
        Self::new(OperationType::OpenAt(file, offset))
    }

    pub fn truncate(file: Arc<File>) -> Self {
        Self::new(OperationType::Truncate(file))
    }

    pub fn write(file: Arc<File>, content: String, len: usize, offset: i32) -> Self {
        Self::new(OperationType::Write(file, offset, len, content))
    }

    pub fn get_random(len: usize) -> Self {
        Self::new(OperationType::GetRandom(len))
    }

    pub fn stat(file: Arc<File>) -> Self {
        Self::new(OperationType::Stat(file))
    }

    pub fn fstat(file: Arc<File>) -> Self {
        Self::new(OperationType::Fstat(file))
    }

    pub fn statx(file: Arc<File>) -> Self {
        Self::new(OperationType::Statx(file))
    }

    pub fn statfs(file: Arc<File>) -> Self {
        Self::new(OperationType::StatFS(file))
    }

    pub fn fstatat(file: Arc<File>) -> Self {
        Self::new(OperationType::Fstatat(file))
    }

    pub fn rename(file: Arc<File>, to: String) -> Self {
        Self::new(OperationType::Rename(file, to))
    }

    pub fn clone_op(pid: usize) -> Self {
        Self::new(OperationType::Clone(pid))
    }

    pub fn chdir(path: String) -> Self {
        Self::new(OperationType::Chdir(path))
    }

    ///
    /// Get the OperationType
    ///
    pub fn op_type(&self) -> &OperationType {
        &self.op_type
    }

    ///
    /// Get the file accessed by this operation
    ///
    pub fn file(&self) -> Option<Arc<File>> {
        match &self.op_type {
            OperationType::Mkdir(file, _) => Some(file.clone()),
            OperationType::Mknod(file) => Some(file.clone()),
            OperationType::Remove(file) => Some(file.clone()),
            OperationType::Read(file, _, _) => Some(file.clone()),
            OperationType::Write(file, _, _, _) => Some(file.clone()),
            OperationType::OpenAt(file, _) => Some(file.clone()),
            OperationType::Truncate(file) => Some(file.clone()),
            OperationType::GetRandom(_) => None,
            OperationType::Stat(file) => Some(file.clone()),
            OperationType::Fstat(file) => Some(file.clone()),
            OperationType::Statx(file) => Some(file.clone()),
            OperationType::StatFS(file) => Some(file.clone()),
            OperationType::Fstatat(file) => Some(file.clone()),
            OperationType::Rename(file, _) => Some(file.clone()),
            OperationType::Clone(_) => None,
            OperationType::Chdir(_) => None,
            OperationType::NoOp => None,
        }
    }

    ///
    /// Name of the operation
    ///
    pub fn name(&self) -> String {
        match &self.op_type {
            &OperationType::Mkdir(_, _) => "Mkdir".to_string(),
            &OperationType::Mknod(_) => "Mknod".to_string(),
            &OperationType::Remove(_) => "Remove".to_string(),
            &OperationType::Read(_, _, _) => "Read".to_string(),
            &OperationType::Write(_, _, _, _) => "Write".to_string(),
            &OperationType::OpenAt(_, _) => "OpenAt".to_string(),
            &OperationType::Truncate(_) => "Truncate".to_string(),
            &OperationType::GetRandom(_) => "GetRandom".to_string(),
            &OperationType::Stat(_) => "Stat".to_string(),
            &OperationType::Fstat(_) => "Fstat".to_string(),
            &OperationType::Statx(_) => "Statx".to_string(),
            &OperationType::StatFS(_) => "StatFS".to_string(),
            &OperationType::Fstatat(_) => "Fstatat".to_string(),
            &OperationType::Rename(_, _) => "Rename".to_string(),
            &OperationType::Clone(_) => "Clone".to_string(),
            &OperationType::Chdir(_) => "Chdir".to_string(),
            &OperationType::NoOp => "NoOp".to_string(),
        }
    }
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            &OperationType::Mkdir(ref file, ref mode) => write!(f, "mkdir({}, {})", file, mode),
            &OperationType::Mknod(ref file) => {
                write!(f, "mknod({})", file)
            }
            &OperationType::Remove(ref file) => write!(f, "remove({})", file),
            &OperationType::Read(ref file, ref offset, ref len) => {
                write!(f, "read({}, {}, {})", file, offset, len)
            }
            &OperationType::Write(ref file, ref offset, ref len, ref content) => {
                write!(f, "write({}, {}, {}, {})", file, offset, len, content)
            }
            &OperationType::OpenAt(ref file, ref offset) => write!(f, "open({}, {})", file, offset),
            &OperationType::Truncate(ref file) => write!(f, "truncate({})", file),
            &OperationType::GetRandom(ref len) => write!(f, "get_random({})", len),
            &OperationType::Stat(ref path) => write!(f, "stat({})", path),
            &OperationType::Fstat(ref path) => write!(f, "fstat({})", path),
            &OperationType::Statx(ref path) => write!(f, "statx({})", path),
            &OperationType::StatFS(ref path) => write!(f, "statfs({})", path),
            &OperationType::Fstatat(ref path) => write!(f, "fstatat({})", path),
            &OperationType::Rename(ref file, ref to) => {
                write!(f, "rename({} {})", file, to)
            }
            &OperationType::Clone(ref pid) => write!(f, "clone({})", pid),
            &OperationType::Chdir(ref path) => write!(f, "chdir({})", path),
            &OperationType::NoOp => write!(f, "no-op"),
        }
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.op_type,
        )?;

        Ok(())
    }
}
