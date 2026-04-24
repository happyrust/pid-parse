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

//! `pid-parse` is a layered, panic-free reader / writer for the
//! `.pid` compound files produced by Intergraph / Hexagon
//! `SmartPlant` P&ID and the `Export.mdf` SQL Server payloads those
//! projects ship to downstream publishing.
//!
//! The crate is organised as eight collaborating layers — see
//! `docs/architecture-guide.md` for the full treatment. At the
//! public-API level the pieces you normally touch are:
//!
//! - [`PidParser`] / [`ParseOptions`] — turn a `.pid` path into
//!   either a [`PidDocument`] (decoded model only) or a
//!   [`PidPackage`] (decoded model + preserved raw streams,
//!   required for round-tripping through [`PidWriter`]).
//! - [`PidDocument`] — the canonical decoded model. Subsequent
//!   passes ([`crossref::build_graph`], [`layout::derive_layout`],
//!   [`import_view::build_import_view`]) enrich it in place.
//! - [`PidWriter`] + [`WritePlan`] — declarative round-trip of a
//!   [`PidPackage`] back to a `.pid` on disk, preserving unknown
//!   streams byte-for-byte.
//! - [`publish`] — a separate pipeline that takes a `SmartPlant`
//!   `Export.mdf` (via the vendored `oxidized-mdf` reader) and
//!   produces the `_Data.xml` / `_Meta.xml` files `SmartPlant`
//!   expects from its publishing stage. Entry points:
//!   [`publish::load_drawing_graph_from_mdf`],
//!   [`publish::write_data_xml`], [`publish::write_meta_xml`].
//!
//! Every failure path is surfaced through [`PidError`] (CFB / XML /
//! writer) or [`publish::PublishError`] (the MDF → XML pipeline);
//! the code base is `panic!`/`unwrap()`-free in production paths
//! and the workspace is gated on `cargo clippy --locked
//! --workspace --all-targets -- -D warnings`.
//!
//! ```no_run
//! use pid_parse::PidParser;
//!
//! let doc = PidParser::new()
//!     .parse_file("example.pid")
//!     .expect("valid SmartPlant .pid file");
//! println!("streams: {}", doc.streams.len());
//! ```
//!
//! End-to-end runnable walkthroughs live under `examples/`:
//!
//! - `parse_walkthrough.rs` — open a `.pid`, inspect streams /
//!   [`DrawingMeta`] / [`GeneralMeta`], emit a summary JSON.
//! - `publish_walkthrough.rs` — open an `Export.mdf`, load a
//!   drawing graph, emit `_Data.xml` + `_Meta.xml`.
//!
//! Both soft-skip when the local `test-file/` fixture is missing,
//! so `cargo run --example …` is safe on any checkout.

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
