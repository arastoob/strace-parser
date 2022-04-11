use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, PartialEq)]
pub enum Operation {
    Read(String, i32, usize),          // args: path, offset, len
    Write(String, i32, usize, String), // args: path, offset, len, content
    Mkdir(String, String),             // args: path, mode
    Mknod(String),                     // args: path
    Remove(String),                    // args: path
    Rename(String, String),            // args: from_path, to_path
    OpenAt(String, i32),               // args: path, offset
    Truncate(String),                  // args: path
    GetRandom(usize),                  // args: len
    Stat(String),                      // args: path
    Fstat(String),                     // args: path
    Statx(String),                     // args: path
    StatFS(String),                    // args: path
    Fstatat(String),                   // args: path
    NoOp,
}

impl Operation {
    pub fn read(len: usize, offset: i32, path: String) -> Self {
        Operation::Read(path, offset, len)
    }

    pub fn no_op() -> Self {
        Operation::NoOp
    }

    pub fn mkdir(path: String, mode: String) -> Self {
        Operation::Mkdir(path, mode)
    }

    pub fn mknod(path: String) -> Self {
        Operation::Mknod(path)
    }

    pub fn remove(path: String) -> Self {
        Operation::Remove(path)
    }

    pub fn open_at(offset: i32, path: String) -> Self {
        Operation::OpenAt(path, offset)
    }

    pub fn truncate(path: String) -> Self {
        Operation::Truncate(path)
    }

    pub fn write(content: String, len: usize, offset: i32, path: String) -> Self {
        Operation::Write(path, offset, len, content)
    }

    pub fn get_random(len: usize) -> Self {
        Operation::GetRandom(len)
    }

    pub fn stat(path: String) -> Self {
        Operation::Stat(path)
    }

    pub fn fstat(path: String) -> Self {
        Operation::Fstat(path)
    }

    pub fn statx(path: String) -> Self {
        Operation::Statx(path)
    }

    pub fn statfs(path: String) -> Self {
        Operation::StatFS(path)
    }

    pub fn fstatat(path: String) -> Self {
        Operation::Fstatat(path)
    }

    pub fn rename(from: String, to: String) -> Self {
        Operation::Rename(from, to)
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            &Operation::Mkdir(ref path, ref mode) => write!(f, "mkdir({}, {})", path, mode),
            &Operation::Mknod(ref path) => {
                write!(f, "mknod({})", path)
            }
            &Operation::Remove(ref path) => write!(f, "remove({})", path),
            &Operation::Read(ref path, ref offset, ref len) => {
                write!(f, "read({}, {}, {})", path, offset, len)
            }
            &Operation::Write(ref path, ref offset, ref len, ref content) => {
                write!(f, "write({}, {}, {}, {})", path, offset, len, content)
            }
            &Operation::OpenAt(ref path, ref offset) => write!(f, "open({}, {})", path, offset),
            &Operation::Truncate(ref path) => write!(f, "truncate({})", path),
            &Operation::GetRandom(ref len) => write!(f, "get_random({})", len),
            &Operation::Stat(ref path) => write!(f, "stat({})", path),
            &Operation::Fstat(ref path) => write!(f, "fstat({})", path),
            &Operation::Statx(ref path) => write!(f, "statx({})", path),
            &Operation::StatFS(ref path) => write!(f, "statfs({})", path),
            &Operation::Fstatat(ref path) => write!(f, "fstatat({})", path),
            &Operation::Rename(ref old_path, ref new_path) => {
                write!(f, "rename({} {})", old_path, new_path)
            }
            &Operation::NoOp => write!(f, "no-op"),
        }
    }
}
