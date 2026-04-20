//! Declarative description of edits to apply when re-emitting a
//! [`crate::package::PidPackage`] via [`super::PidWriter`].
//!
//! A [`WritePlan`] is composed of three orthogonal layers:
//! 1. **`metadata_updates`** — high-level intent (replace `Drawing` /
//!    `General` XML, future: tweak `SummaryInformation`). Resolved by
//!    [`super::metadata_write`].
//! 2. **`stream_replacements`** — low-level path → bytes substitutions.
//!    Applied verbatim; the writer does no validation.
//! 3. **`sheet_patches`** — surgical byte-range edits inside an existing
//!    sheet stream. Marked `experimental` because semantic re-encoding is
//!    not yet in scope.
//!
//! Order of application (see [`super::PidWriter::write_to`]):
//! `metadata_updates` → `stream_replacements` → `sheet_patches`.
//! Later layers can therefore overwrite earlier ones; the writer does not
//! reject conflicts (callers compose plans with knowledge of intent).

use std::collections::BTreeMap;

/// Top-level write description. Construct with `WritePlan::default()` for a
/// pure passthrough (no edits, just re-emit the package as-is).
#[derive(Debug, Clone, Default)]
pub struct WritePlan {
    pub metadata_updates: MetadataUpdates,
    pub stream_replacements: Vec<StreamReplacement>,
    pub sheet_patches: Vec<SheetPatch>,
}

/// High-level metadata edits. First version supports the two
/// `/TaggedTxtData` XML streams that SmartPlant uses to store drawing-level
/// metadata; `summary_updates` is reserved for a future
/// `SummaryInformation` property-set rewrite (currently ignored by the
/// writer).
#[derive(Debug, Clone, Default)]
pub struct MetadataUpdates {
    /// Replacement bytes for `/TaggedTxtData/Drawing` (encoded UTF-8 / UTF-16
    /// per caller's responsibility — see writer-layer-plan risk note).
    pub drawing_xml: Option<String>,
    /// Replacement bytes for `/TaggedTxtData/General`.
    pub general_xml: Option<String>,
    /// Reserved for future `SummaryInformation` support. Currently
    /// preserved across writes but **not applied**.
    pub summary_updates: BTreeMap<String, String>,
}

/// Replace one stream wholesale with the supplied bytes. The writer will
/// insert the entry if it didn't previously exist.
#[derive(Debug, Clone)]
pub struct StreamReplacement {
    pub path: String,
    pub new_data: Vec<u8>,
}

/// Apply a list of byte-range patches to an existing sheet stream.
///
/// `experimental: true` is a soft flag — the writer will still execute the
/// patch — but downstream tooling can use it to decide whether to surface
/// the result to end users.
#[derive(Debug, Clone)]
pub struct SheetPatch {
    pub sheet_path: String,
    pub chunk_patches: Vec<SheetChunkPatch>,
    pub experimental: bool,
}

/// Splice `[start..end)` of the target stream with `replacement`.
/// `replacement.len()` may differ from `end - start`; the writer adjusts
/// the surrounding bytes accordingly.
#[derive(Debug, Clone)]
pub struct SheetChunkPatch {
    pub start: usize,
    pub end: usize,
    pub replacement: Vec<u8>,
}
