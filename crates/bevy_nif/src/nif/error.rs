use std::fmt;
use std::io::Error as IoError;
use std::string::FromUtf8Error;

// --- Error Type ---
#[derive(Debug)]
pub enum ParseError {
    Io(IoError),
    Utf8(FromUtf8Error),
    InvalidData(String),
    UnsupportedBlockType(String),
}

impl From<IoError> for ParseError {
    fn from(err: IoError) -> Self {
        ParseError::Io(err)
    }
}

impl From<FromUtf8Error> for ParseError {
    fn from(err: FromUtf8Error) -> Self {
        ParseError::Utf8(err)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "IO Error: {}", e),
            ParseError::Utf8(e) => write!(f, "UTF8 Error: {}", e),
            ParseError::InvalidData(s) => write!(f, "Invalid Data: {}", s),
            ParseError::UnsupportedBlockType(s) => write!(f, "Unsupported Block Type: {}", s),
        }
    }
}

impl std::error::Error for ParseError {}

// Define a crate-wide Result type alias
pub type Result<T> = std::result::Result<T, ParseError>;
