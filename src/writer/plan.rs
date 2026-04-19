//! Declarative write plan consumed by [`crate::writer::PidWriter::write_to`].
//!
//! A [`WritePlan`] describes **what** should change in a [`PidPackage`]
//! before it gets serialized back to CFB. The writer pipeline is:
//!
//! 1. Apply [`MetadataUpdates`] (drawing XML / general XML / future summary)
//! 2. Apply every [`StreamReplacement`] verbatim
//! 3. Apply every [`SheetPatch`] byte-range (experimental, no semantic checks)
//! 4. Write the resulting stream map to a new CFB container
//!
//! Each field is optional / may be empty; a `WritePlan::default()` is a
//! pure passthrough that re-serializes the package unchanged.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level write plan. An empty plan (`WritePlan::default()`) is a valid
/// passthrough request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WritePlan {
    /// Targeted metadata edits. See [`MetadataUpdates`] for supported
    /// streams.
    pub metadata_updates: MetadataUpdates,
    /// Verbatim stream replacements applied after metadata updates. Paths
    /// are normalized (leading `/`, `/` separator) by the writer.
    pub stream_replacements: Vec<StreamReplacement>,
    /// Experimental byte-range patches for Sheet streams. No semantic
    /// validation — the caller is responsible for producing a byte-valid
    /// Sheet body.
    pub sheet_patches: Vec<SheetPatch>,
}

/// Narrow metadata channel for the common "edit drawing number / project"
/// flow without hand-crafting byte replacements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataUpdates {
    /// New XML body for `/TaggedTxtData/Drawing`. `None` = untouched.
    pub drawing_xml: Option<String>,
    /// New XML body for `/TaggedTxtData/General`. `None` = untouched.
    pub general_xml: Option<String>,
    /// Placeholder for future `SummaryInformation` property-set updates.
    /// **Not implemented in the current release** — any value given here
    /// is silently ignored.
    pub summary_updates: BTreeMap<String, String>,
}

/// Replace (or insert) a single CFB stream with the provided bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamReplacement {
    /// Absolute path inside the CFB (e.g. `/TaggedTxtData/Drawing`).
    pub path: String,
    pub new_data: Vec<u8>,
}

/// Experimental byte-range patch for a Sheet stream. Not semantically
/// validated; callers opt into risk via `experimental = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPatch {
    /// Absolute sheet stream path, e.g. `/Sheet6`.
    pub sheet_path: String,
    /// Ordered list of byte-range edits. Patches may overlap across calls,
    /// but the writer applies them in descending-`start` order within a
    /// single patch to keep earlier offsets stable.
    pub chunk_patches: Vec<SheetChunkPatch>,
    /// Must be `true` in the current release. Present for future
    /// validation hooks.
    pub experimental: bool,
}

/// Half-open byte range `[start, end)` replaced by `replacement`. A
/// `start == end` patch is a pure insertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetChunkPatch {
    pub start: usize,
    pub end: usize,
    pub replacement: Vec<u8>,
}

impl WritePlan {
    /// Convenience constructor for the metadata-only flow: only the two
    /// `/TaggedTxtData/*` XML streams are touched.
    pub fn metadata_only(drawing_xml: Option<String>, general_xml: Option<String>) -> Self {
        Self {
            metadata_updates: MetadataUpdates {
                drawing_xml,
                general_xml,
                summary_updates: BTreeMap::new(),
            },
            ..Self::default()
        }
    }

    /// `true` iff the plan would not change any stream (a passthrough).
    pub fn is_passthrough(&self) -> bool {
        self.metadata_updates.drawing_xml.is_none()
            && self.metadata_updates.general_xml.is_none()
            && self.metadata_updates.summary_updates.is_empty()
            && self.stream_replacements.is_empty()
            && self.sheet_patches.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plan_is_passthrough() {
        assert!(WritePlan::default().is_passthrough());
    }

    #[test]
    fn metadata_only_sets_only_xml_fields() {
        let plan = WritePlan::metadata_only(Some("<Drawing/>".into()), None);
        assert!(!plan.is_passthrough());
        assert_eq!(
            plan.metadata_updates.drawing_xml.as_deref(),
            Some("<Drawing/>")
        );
        assert!(plan.metadata_updates.general_xml.is_none());
        assert!(plan.stream_replacements.is_empty());
        assert!(plan.sheet_patches.is_empty());
    }
}
