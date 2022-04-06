mod error;
mod ops;
mod parser;

// re-export the required modules
pub use ops::{Operation, OperationType};
pub use parser::{FileDir, Parser};
