use std::fmt;

///
/// Problems that can arise in strace-parser.
///
#[derive(Debug)]
pub enum Error {
    /// Something not found
    NotFound(String),

    ParseError(String),

    InvalidType(String)
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::NotFound(ref detail) => write!(f, "{} not found", detail),
            &Error::ParseError(ref detail) => write!(f, "could not parse {}", detail),
            &Error::InvalidType(ref detail) => write!(f, "invalid type: {}", detail),
        }
    }
}
