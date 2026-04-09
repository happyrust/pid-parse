use thiserror::Error;

#[derive(Debug, Error)]
pub enum PidError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cfb error: {0}")]
    Cfb(#[from] cfb::Error),

    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("missing stream: {0}")]
    MissingStream(String),

    #[error("invalid utf16 data in stream: {0}")]
    InvalidUtf16(String),

    #[error("unsupported structure: {0}")]
    Unsupported(String),

    #[error("parse failure in {context}: {message}")]
    ParseFailure { context: String, message: String },
}
