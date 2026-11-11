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
//! A page has two domains — the drawing's own records and the border
//! template's — and each domain's account ([`DomainAccount`]) is five layers,
//! each with an equation of its own:
//!
//! ```text
//! parse      a record family in one stream  decoded  = emitted  + out_of_range + shape_rejected + unattributed
//! entity     an entity of the geometry      total    = on_page  + Σ dropped + unattributed
//! body       a symbol body's primitive      total    = produced + Σ dropped + unattributed
//! primitive  a page primitive               laid_out = on_page  + Σ dropped + unattributed
//! segment    a stroke of a primitive        total    = written  + Σ dropped + unattributed
//! ```
//!
//! The parse layer's equation is the emitters' own, summed over every record
//! family and Sheet stream; beside it the layer lists the records that never
//! became entities, by appendix A.1's reasons. An entity's `dropped` is the
//! layout layer's reason (A.2) or, when it laid primitives out and none
//! reached the page, the first of theirs. A body's is A.3's. A primitive's
//! holds `off_sheet` — the primitives the final `retain` removed (A.4) — with
//! the writer's codes (A.5), counted once. A segment's is A.5's three
//! `segment_*` codes.
//!
//! Each layer carries an `unattributed` remainder. It has to be zero: a
//! non-zero remainder is a batch of things that neither reached the page nor
//! earned a name, which is exactly the gap the ledger exists to expose.
//!
//! What a page states about itself — where its size came from, the template
//! it wears, a template it names that no file answers — is a sixth account,
//! [`PageAccount`]. It has reasons (A.8) and no equation, so no remainder.
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
    BodyAccount, CorpusDescriptor, DomainAccount, EntityAccount, ExportLedger, LedgerSummary,
    PageAccount, PageSizeSource, ParseAccount, PrimitiveAccount, RecordItem, SegmentAccount,
    SuspectSite, TemplateFact, TemplateSourceKind,
};
pub use baseline::{
    GateInput, GateReport, LedgerBaseline, LedgerCorpus, SCHEMA as EXPORT_LEDGER_SCHEMA,
    descriptor_for, run_gate,
};
pub use layout::{
    BodyItemLayout, BodyTally, Confidence, EntityKind, EntityLayout, EntityOutcome, EntityRecord,
    LayoutLedger, LedgerRecorder, PageInstead, PageSources, PaperRefused, ParseCounts, ParseRecord,
    PlacementAccount, PlacementLayout, SheetRectangle, Site, SourceKind, SourceName, TemplateTrace,
    TextNormalization, UnplacedRecords, UnplacedWhy, micrometres,
};
pub use reason::{
    Asymmetry, DomainPair, DropReason, LedgerDomain, PartialMark, PlacementSource, ReasonLayer,
    Tier,
};
