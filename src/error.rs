use std::fmt;

///
/// Problems that can arise in strace-parser.
///
#[derive(Debug)]
pub enum Error {
    /// Something not found
    NotFound(String),

    IO(std::io::Error),

    Unknown(String),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            &Error::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::NotFound(ref detail) => write!(f, "{} not found", detail),
            &Error::IO(ref err) => write!(f, "IO error: {}", err),
            &Error::Unknown(ref detail) => write!(f, "Unknown error: {}", detail),
        }
    }
}
