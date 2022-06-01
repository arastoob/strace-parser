use std::fmt;

///
/// Problems that can arise in strace-parser.
///
#[derive(Debug)]
pub enum Error {
    /// Something not found
    NotFound(String),

    /// An error occurred on the disk or network
    IO(std::io::Error),

    ParseError(String),

    InvalidType(String),

    PoisonError(String),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Error {
        Error::PoisonError(err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::NotFound(ref detail) => write!(f, "{} not found", detail),
            &Error::ParseError(ref detail) => write!(f, "could not parse {}", detail),
            &Error::InvalidType(ref detail) => write!(f, "invalid type: {}", detail),
            &Error::PoisonError(ref detail) => {
                write!(f, "could not acquire a lock oh shared object: {}", detail)
            }
            Error::IO(ref err) => write!(f, "IO error: {}", err),
        }
    }
}
