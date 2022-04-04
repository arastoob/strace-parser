use std::fmt;
use std::io::ErrorKind;

///
/// Problems that can arise in fs-bench.
///
#[derive(Debug)]
pub enum Error {
    /// Something not found
    NotFound(String),

    /// There was an error reading formatted data
    FormatError {
        format_of: String,
        detail: String,
    },

    /// Configuration information was incorrect
    InvalidConfig(String),

    /// There has been an attempt to access data at an invalid index
    InvalidIndex {
        kind: String,
        index: usize,
        max: usize,
    },

    /// An incorrect path was specified
    InvalidPath(String),

    /// An error occurred on the disk or network
    IO(std::io::Error),

    Unknown(String),
}

impl Error {
    pub fn format<S1, S2>(format_of: S1, detail: S2) -> Error
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Error::FormatError {
            format_of: format_of.into(),
            detail: detail.into(),
        }
    }

    pub fn index<S>(kind: S, index: usize, max: usize) -> Error
    where
        S: Into<String>,
    {
        Error::InvalidIndex {
            kind: kind.into(),
            index,
            max,
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            &Error::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<Error> for std::io::Error {
    fn from(err: Error) -> std::io::Error {
        std::io::Error::new(ErrorKind::Other, err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::NotFound(ref detail) => write!(f, "{} not found", detail),
            &Error::FormatError {
                ref format_of,
                ref detail,
            } => write!(f, "Format error in {}: {:?}", format_of, detail),
            &Error::IO(ref err) => write!(f, "IO error: {}", err),
            &Error::InvalidConfig(ref detail) => write!(f, "Configuration error: {}", detail),
            &Error::InvalidIndex {
                ref kind,
                index,
                max,
            } => write!(f, "Invalid {} index: {} (max: {})", kind, index, max),
            &Error::InvalidPath(ref path) => write!(f, "Invalid path: '{}'", path),
            &Error::Unknown(ref detail) => write!(f, "Unknown error: {}", detail),
        }
    }
}
