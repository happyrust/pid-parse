use async_std::io::Error as IoError;
use std::convert::From;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IoError(err) => write!(f, "IO Error: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(err) => Some(err),
        }
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Self::IoError(err)
    }
}
