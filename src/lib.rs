// Keep the curated subset of pedantic lints we already baked in via the
// `cargo clippy --fix` passes (see CHANGELOG "Clippy 清理" sections).
// These warn on new code only — existing violations are already fixed.
#![warn(
    clippy::uninlined_format_args,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::unreadable_literal,
    clippy::bool_to_int_with_if,
    clippy::implicit_clone,
    clippy::explicit_iter_loop,
    clippy::unnecessary_map_or
)]

pub mod api;
pub mod backup;
pub mod byte_audit;
pub mod cfb;
pub mod crossref;
pub mod error;
pub mod import_view;
pub mod inspect;
pub mod layout;
pub mod model;
pub mod package;
pub mod parsers;
pub mod publish;
pub mod schema;
pub mod streams;
pub mod writer;

pub use api::{ParseOptions, PidParser};
pub use byte_audit::{
    byte_audit_report, ByteAuditReport, ByteRange, ParserTrace, ParserTraceBuilder,
    StreamAuditSummary, TraceConfidence,
};
pub use error::PidError;
pub use import_view::*;
pub use layout::*;
pub use model::*;
pub use package::{
    diff_packages, PackageDiff, PidPackage, RawStream, StateBitsDiff, StorageClsidDiff,
    StorageTimestampDiff, StorageTimestamps, StreamDiff,
};
pub use writer::{
    EncodedString, MetadataUpdates, PidWriter, SheetChunkPatch, SheetPatch, StreamReplacement,
    WritePlan,
};

/// Re-export of [`uuid::Uuid`] for ergonomic access to the root CLSID
/// carried by [`PidPackage::root_clsid`] without forcing consumers to
/// pin their own `uuid` crate version.
pub use uuid::Uuid;
