//! Writer layer: turn a [`crate::package::PidPackage`] (parsed file +
//! every raw CFB stream) plus a [`WritePlan`] back into a `.pid` file on
//! disk.
//!
//! Composition pipeline (see [`PidWriter::write_to`]):
//!   1. clone the source package so the in-memory original stays intact;
//!   2. resolve `metadata_updates` into stream replacements;
//!   3. apply explicit `stream_replacements` verbatim;
//!   4. run `sheet_patches` byte-range edits;
//!   5. emit a fresh CFB container.
//!
//! Caveats — first-version scope (see `docs/writer-layer-plan.md`):
//! - The output is a brand-new CFB container; CLSID and storage
//!   timestamps are not preserved.
//! - `summary_updates` inside [`plan::MetadataUpdates`] are accepted but
//!   not yet applied.
//! - Sheet patches are byte-range only; semantic re-encoding lives in
//!   future work.

pub mod cfb_write;
pub mod metadata_helpers;
pub mod metadata_write;
pub mod plan;
pub mod sheet_patch;

pub use metadata_helpers::{
    get_drawing_attribute, get_general_element_text, list_drawing_attributes,
    list_general_elements, set_drawing_attribute, set_drawing_number, set_element_text,
    set_general_file_path, MetadataEditError,
};
pub use plan::{
    MetadataUpdates, SheetChunkPatch, SheetPatch, StreamReplacement, WritePlan,
};

use crate::error::PidError;
use crate::package::PidPackage;
use std::path::Path;

/// Stateless façade over the writer pipeline. Construction is free
/// (`PidWriter` is a unit struct); call [`PidWriter::write_to`] for each
/// emit operation.
pub struct PidWriter;

impl PidWriter {
    pub fn new() -> Self {
        Self
    }

    /// Apply `plan` to `package` and emit the resulting CFB to `output`.
    /// `package` is left untouched (the writer clones it internally so
    /// callers can keep using the original).
    pub fn write_to(
        package: &PidPackage,
        plan: &WritePlan,
        output: &Path,
    ) -> Result<(), PidError> {
        let mut working = package.clone();

        metadata_write::apply_metadata_updates(&mut working, &plan.metadata_updates)?;

        for replacement in &plan.stream_replacements {
            working.replace_stream(&replacement.path, replacement.new_data.clone());
        }

        for patch in &plan.sheet_patches {
            sheet_patch::apply_sheet_patch_to_package(&mut working, patch)?;
        }

        cfb_write::write_package(&working, output)
    }
}

impl Default for PidWriter {
    fn default() -> Self {
        Self::new()
    }
}
