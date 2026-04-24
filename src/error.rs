//! Top-level error type shared by the CFB reader, the writer, and
//! most in-crate helpers.
//!
//! The `publish` pipeline (MDF → `_Data.xml` / `_Meta.xml`) has its
//! own narrower [`crate::publish::PublishError`] and does not funnel
//! through `PidError` — keep them separate so callers can tell a
//! "failed to read a `.pid`" path from a "failed to generate publish
//! XML" path without string matching.

use thiserror::Error;

/// Errors surfaced by the `.pid` reader/writer pipeline.
///
/// Variants group naturally:
///
/// - `Io` / `Xml` — wrapped lower-level errors (file I/O,
///   [`quick_xml`]).
/// - `MissingStream` / `InvalidUtf16` / `Unsupported` — structural
///   surprises where the compound file parses but doesn't match the
///   `SmartPlant` schema we support.
/// - `ParseFailure { context, message }` — curated parser errors
///   raised by individual decoders with a caller-facing context
///   string (e.g. `"PSMroots"` or `"Dynamic Attributes Metadata"`).
#[derive(Debug, Error)]
pub enum PidError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

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
