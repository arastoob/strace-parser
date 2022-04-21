use std::fmt;
use std::fmt::Formatter;
use std::rc::Rc;
use crate::file::File;

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum Operation1 {
    Read(Rc<File>, i32, usize),          // args: FileDir, offset, len
    Write(Rc<File>, i32, usize, String), // args: FileDir, offset, len, content
    Mkdir(Rc<File>, String),             // args: path, mode
    Mknod(Rc<File>),                     // args: path
    Remove(Rc<File>),                    // args: FileDir
    Rename(Rc<File>, String),            // args: FileDir, new_name
    OpenAt(Rc<File>, i32),               // args: FileDir, offset
    Truncate(Rc<File>),                  // args: FileDir
    GetRandom(usize),                  // args: len
    Stat(Rc<File>),                      // args: path
    Fstat(Rc<File>),                     // args: path
    Statx(Rc<File>),                     // args: path
    StatFS(Rc<File>),                    // args: path
    Fstatat(Rc<File>),                   // args: path
    Clone(usize),                      // args: process id of the cloned process
    NoOp,
}

impl Operation1 {
    pub fn read(file: Rc<File>, len: usize, offset: i32) -> Self {
        Operation1::Read(file, offset, len)
    }

    pub fn no_op() -> Self {
        Operation1::NoOp
    }

    pub fn mkdir(file: Rc<File>, mode: String) -> Self {
        Operation1::Mkdir(file, mode)
    }

    pub fn mknod(file: Rc<File>) -> Self {
        Operation1::Mknod(file)
    }

    pub fn remove(file: Rc<File>) -> Self {
        Operation1::Remove(file)
    }

    pub fn open_at(file: Rc<File>, offset: i32) -> Self {
        Operation1::OpenAt(file, offset)
    }

    pub fn truncate(file: Rc<File>) -> Self {
        Operation1::Truncate(file)
    }

    pub fn write(file: Rc<File>, content: String, len: usize, offset: i32) -> Self {
        Operation1::Write(file, offset, len, content)
    }

    pub fn get_random(len: usize) -> Self {
        Operation1::GetRandom(len)
    }

    pub fn stat(file: Rc<File>) -> Self {
        Operation1::Stat(file)
    }

    pub fn fstat(file: Rc<File>) -> Self {
        Operation1::Fstat(file)
    }

    pub fn statx(file: Rc<File>) -> Self {
        Operation1::Statx(file)
    }

    pub fn statfs(file: Rc<File>) -> Self {
        Operation1::StatFS(file)
    }

    pub fn fstatat(file: Rc<File>) -> Self {
        Operation1::Fstatat(file)
    }

    pub fn rename(file: Rc<File>, to: String) -> Self {
        Operation1::Rename(file, to)
    }

    pub fn clone(pid: usize) -> Self {
        Operation1::Clone(pid)
    }

    pub fn file(&self) -> Option<Rc<File>> {
        match &self {
            &Operation1::Mkdir(file, _) => Some(file.clone()),
            &Operation1::Mknod(file) => Some(file.clone()),
            &Operation1::Remove(file) => Some(file.clone()),
            &Operation1::Read(file, _, _) => Some(file.clone()),
            &Operation1::Write(file, _, _, _) => Some(file.clone()),
            &Operation1::OpenAt(file, _) => Some(file.clone()),
            &Operation1::Truncate(file) => Some(file.clone()),
            &Operation1::GetRandom(_) => None,
            &Operation1::Stat(file) => Some(file.clone()),
            &Operation1::Fstat(file) => Some(file.clone()),
            &Operation1::Statx(file) => Some(file.clone()),
            &Operation1::StatFS(file) => Some(file.clone()),
            &Operation1::Fstatat(file) => Some(file.clone()),
            &Operation1::Rename(file, _) => Some(file.clone()),
            &Operation1::Clone(_) => None,
            &Operation1::NoOp => None,
        }
    }

    pub fn name(&self) -> String {
        match &self {
            &Operation1::Mkdir(_, _) => "Mkdir".to_string(),
            &Operation1::Mknod(_) => "Mknod".to_string(),
            &Operation1::Remove(_) => "Remove".to_string(),
            &Operation1::Read(_, _, _) => "Read".to_string(),
            &Operation1::Write(_, _, _, _) => "Write".to_string(),
            &Operation1::OpenAt(_, _) => "OpenAt".to_string(),
            &Operation1::Truncate(_) => "Truncate".to_string(),
            &Operation1::GetRandom(_) => "GetRandom".to_string(),
            &Operation1::Stat(_) => "Stat".to_string(),
            &Operation1::Fstat(_) => "Fstat".to_string(),
            &Operation1::Statx(_) => "Statx".to_string(),
            &Operation1::StatFS(_) => "StatFS".to_string(),
            &Operation1::Fstatat(_) => "Fstatat".to_string(),
            &Operation1::Rename(_, _) => "Rename".to_string(),
            &Operation1::Clone(_) => "Clone".to_string(),
            &Operation1::NoOp => "NoOp".to_string(),
        }
    }
}

impl fmt::Display for Operation1 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            &Operation1::Mkdir(ref file, ref mode) => write!(f, "mkdir({}, {})", file, mode),
            &Operation1::Mknod(ref file) => {
                write!(f, "mknod({})", file)
            }
            &Operation1::Remove(ref file) => write!(f, "remove({})", file),
            &Operation1::Read(ref file, ref offset, ref len) => {
                write!(f, "read({}, {}, {})", file, offset, len)
            }
            &Operation1::Write(ref file, ref offset, ref len, ref content) => {
                write!(f, "write({}, {}, {}, {})", file, offset, len, content)
            }
            &Operation1::OpenAt(ref file, ref offset) => write!(f, "open({}, {})", file, offset),
            &Operation1::Truncate(ref file) => write!(f, "truncate({})", file),
            &Operation1::GetRandom(ref len) => write!(f, "get_random({})", len),
            &Operation1::Stat(ref path) => write!(f, "stat({})", path),
            &Operation1::Fstat(ref path) => write!(f, "fstat({})", path),
            &Operation1::Statx(ref path) => write!(f, "statx({})", path),
            &Operation1::StatFS(ref path) => write!(f, "statfs({})", path),
            &Operation1::Fstatat(ref path) => write!(f, "fstatat({})", path),
            &Operation1::Rename(ref file, ref to) => {
                write!(f, "rename({} {})", file, to)
            }
            &Operation1::Clone(ref pid) => write!(f, "clone({})", pid),
            &Operation1::NoOp => write!(f, "no-op"),
        }
    }
}
