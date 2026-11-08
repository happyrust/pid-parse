//! The export ledger: one account per page of every record, entity, primitive
//! and stroke the plot pipeline handles.
//!
//! The specification is `.kiro/specs/pdf-export-ledger/`. This module is its
//! implementation; the reading it is built on is that **the ledger only
//! observes**. No page may change because of it — the guardrail is
//! `pid-plot/examples/probe_export_bytes_unchanged.rs`, which compares 71 pages
//! (53 backup sheets, 7 committed fixtures, 11 reference sheets) byte for byte
//! before and after every step.
//!
//! # Five layers, five conservation equations
//!
//! ```text
//! parse     record  -> entity      total = entities + Σ A.1 reasons
//! layout    entity  -> primitive   total = on_page  + Σ A.2 reasons
//! body      body item -> primitive total = produced + Σ A.3 reasons
//! write     primitive -> segment   total = written  + Σ A.5 reasons
//! page      template / sheet size / cross-checks
//! ```
//!
//! Each layer carries an `unattributed` remainder. It has to be zero: a
//! non-zero remainder is a batch of things that neither reached the page nor
//! earned a name, which is exactly the gap the ledger exists to expose.
//!
//! # Not to be confused with `ExportBundleConfidenceLedger`
//!
//! The parser crate has a type with a similar name
//! (`pid_parse::export_bundle::ExportBundleConfidenceLedger`). It is a
//! different account of a different thing: that one lists how far each file in
//! a publish bundle can be trusted, built statically from the bundle plan and
//! checked only for existence. [`ExportLedger`](assemble::ExportLedger) is
//! computed per page from a laid-out `PaperModel` and a written PDF, and a gate
//! compares it page by page against a committed baseline.
//!
//! # Panic-free
//!
//! Every count here comes from untrusted drawing bytes, so the module denies
//! the four ways a count goes wrong silently: `unwrap`, `expect`, `panic!` and
//! indexing. Counts use `saturating_add`; an index conversion that cannot be
//! narrowed counts as unattributed rather than as a panic.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod assemble;
pub mod baseline;
pub mod layout;
pub mod reason;

pub use assemble::{
    BodyAccount, CorpusDescriptor, DomainAccount, EntityAccount, ExportLedger, PageAccount,
    PageSizeSource, ParseAccount, PrimitiveAccount, RecordItem, SegmentAccount, SuspectSite,
    TemplateFact, TemplateSourceKind,
};
pub use layout::{
    BodyItemLayout, BodyTally, Confidence, EntityKind, EntityLayout, EntityRecord, LayoutLedger,
    PageSources, ParseCounts, ParseRecord, PlacementAccount, PlacementLayout, Site, SourceKind,
    SourceName, TemplateTrace, TextNormalization,
};
pub use reason::{
    Asymmetry, DomainPair, DropReason, LedgerDomain, PartialMark, PlacementSource, ReasonLayer,
    Tier,
};
