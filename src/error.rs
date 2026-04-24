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
    /// Wrapped `std::io::Error` from file / stream I/O.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Wrapped `quick_xml::Error` raised by an XML-oriented decoder.
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Requested CFB stream was not present in the compound file.
    /// Carries the normalized stream path as payload.
    #[error("missing stream: {0}")]
    MissingStream(String),

    /// UTF-16LE decoding failed inside a stream where UTF-16 was
    /// expected. Carries a short context string describing the stream.
    #[error("invalid utf16 data in stream: {0}")]
    InvalidUtf16(String),

    /// Structural shape the crate doesn't support yet (e.g. a new
    /// cluster kind). Carries a human-facing hint.
    #[error("unsupported structure: {0}")]
    Unsupported(String),

    /// Curated parser failure with caller-facing context.
    #[error("parse failure in {context}: {message}")]
    ParseFailure {
        /// Short human label identifying the decoder that failed
        /// (e.g. `"PSMroots"`, `"Dynamic Attributes Metadata"`).
        context: String,
        /// Underlying reason, already pre-formatted for logs.
        message: String,
    },
}
