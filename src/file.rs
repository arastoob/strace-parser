use crate::error::Error;
use std::fmt::Formatter;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Hash, Eq)]
pub struct File {
    path: PathBuf,
}

impl File {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> Result<&str, Box<dyn std::error::Error>> {
        self.path
            .as_os_str()
            .to_str()
            .ok_or(Box::new(Error::ParseError(
                "failed to convert PathBuf to String".to_string(),
            )))
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "file({})", self.path().unwrap_or(""))
    }
}
