//! Canonical normalized paths for well-known CFB streams.
//!
//! These constants live outside parser, audit, and writer modules so each
//! layer can identify the same stream without depending on another layer's
//! implementation details.

/// Normalized CFB path of the OLE `/\x05SummaryInformation` stream.
pub const SUMMARY_INFO_PATH: &str = "/\u{5}SummaryInformation";

/// Normalized CFB path of the OLE `/\x05DocumentSummaryInformation` stream.
pub const DOC_SUMMARY_PATH: &str = "/\u{5}DocumentSummaryInformation";
