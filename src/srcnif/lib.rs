// Declare the modules
pub mod error;
pub mod parser;
pub mod read_utils;
pub mod structs;

// Re-export key types for easier use by the binary or other crates
pub use error::{ParseError, Result};
pub use parser::parse_nif_start;
pub use structs::*;

