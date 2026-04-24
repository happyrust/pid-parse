use std::fmt::{Display, Formatter};
use std::io::Error as IoError;

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    ParseError(&'static str),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IoError(err) => write!(f, "IO Error: {err}"),
            Error::ParseError(msg) => write!(f, "Parse Error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(err) => Some(err),
            Error::ParseError(_) => None,
        }
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Self::IoError(err)
    }
}

impl From<&'static str> for Error {
    fn from(msg: &'static str) -> Self {
        Self::ParseError(msg)
    }
}
