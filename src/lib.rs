mod error;
mod parser;
mod ops;

// re-export the required modules
pub use parser::Parser;
pub use ops::{Operation, OperationType};