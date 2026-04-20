//! Writer layer: apply a [`WritePlan`] to a [`PidPackage`] and emit a new
//! CFB file. Supports:
//!
//! - **Passthrough round-trip**: `PidWriter::write_to(&pkg, &WritePlan::default(), out)`
//!   rebuilds the container with identical stream bytes (order by
//!   lexicographic path).
//! - **Metadata-only updates**: replace `/TaggedTxtData/Drawing` and
//!   `/TaggedTxtData/General` XML bodies.
//! - **SummaryInformation / DocumentSummaryInformation** string-property
//!   edits via `MetadataUpdates.summary_updates` (see [`summary_write`]).
//! - **Stream replacements**: verbatim byte replacement of any stream.
//! - **Experimental Sheet byte-range patches** (see [`sheet_patch`]).
//!
//! See `docs/writer-layer-plan.md` for remaining constraints (no CLSID /
//! timestamp preservation; BOM / UTF-16 encoding is the caller's
//! responsibility; property-set writing is scoped to string types only).
pub mod cfb_write;
pub mod metadata_helpers;
pub mod metadata_write;
pub mod plan;
pub mod sheet_patch;
pub mod summary_write;
pub mod xml_edit;

use crate::error::PidError;
use crate::package::PidPackage;
use std::io::Cursor;
use std::path::Path;

pub use metadata_helpers::{
    get_drawing_attribute, get_general_element_text, list_drawing_attributes,
    list_general_elements, set_drawing_attribute, set_drawing_number, set_element_text,
    set_general_file_path, MetadataEditError,
};
pub use plan::{
    EncodedString, MetadataUpdates, SheetChunkPatch, SheetPatch, StreamReplacement, WritePlan,
};

/// High-level writer entry point. See [`Self::write_to`].
pub struct PidWriter;

impl PidWriter {
    /// Apply `plan` to a **clone** of `package` and write the result to
    /// `output`. The caller's package is not mutated, matching the
    /// "immutable parse result, declarative plan" model.
    pub fn write_to(package: &PidPackage, plan: &WritePlan, output: &Path) -> Result<(), PidError> {
        let mut working = package.clone();
        apply_plan_to_package(&mut working, plan)?;
        cfb_write::write_package(&working, output)
    }

    /// Phase 9o (v0.5.3+): apply `plan` to a clone of `package` and
    /// return the resulting CFB as an in-memory byte buffer. Functionally
    /// equivalent to [`write_to`] followed by reading the file back, but
    /// skips the disk round-trip (useful for HTTP service paths, tests,
    /// or any caller that already has a `Vec<u8>` workflow).
    ///
    /// Overhead note: peak memory is ~2× the final CFB size because we
    /// clone `package` first, then materialize the output in a separate
    /// `Vec`. For multi-MB `.pid` files where memory matters, prefer
    /// [`write_to`].
    pub fn write_to_bytes(package: &PidPackage, plan: &WritePlan) -> Result<Vec<u8>, PidError> {
        let mut working = package.clone();
        apply_plan_to_package(&mut working, plan)?;
        let cursor = cfb_write::write_package_to_writer(&working, Cursor::new(Vec::new()))?;
        Ok(cursor.into_inner())
    }
}

/// Internal: mutate `package` in-place according to `plan`. Shared by
/// [`PidWriter::write_to`] and [`PidWriter::write_to_bytes`] so any
/// future pipeline change (metadata → stream_replacements → sheet_patches)
/// lands in both paths automatically.
fn apply_plan_to_package(package: &mut PidPackage, plan: &WritePlan) -> Result<(), PidError> {
    metadata_write::apply_metadata_updates(package, &plan.metadata_updates)?;
    for repl in &plan.stream_replacements {
        package.replace_stream(repl.path.clone(), repl.new_data.clone());
    }
    for patch in &plan.sheet_patches {
        sheet_patch::apply_sheet_patch_to_package(package, patch)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::{PidPackage, RawStream};
    use std::collections::BTreeMap;

    fn tmp_file(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        // Unique-ish per test to avoid collisions; the tests themselves
        // always overwrite, so this is just for parallelism.
        p.push(format!(
            "pid-parse-writer-{}-{:?}.pid",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    fn pkg_with_streams(streams: &[(&str, &[u8])]) -> PidPackage {
        let mut map = BTreeMap::new();
        for (p, data) in streams {
            map.insert(
                (*p).to_string(),
                RawStream {
                    path: (*p).to_string(),
                    data: data.to_vec(),
                    modified: false,
                },
            );
        }
        PidPackage::new(None, map, PidDocument::default())
    }

    #[test]
    fn passthrough_produces_readable_cfb() {
        let pkg = pkg_with_streams(&[
            ("/FlatStream", &[0x11, 0x22, 0x33]),
            ("/Nested/Blob", &[0xAA, 0xBB]),
        ]);
        let out = tmp_file("passthrough");
        PidWriter::write_to(&pkg, &WritePlan::default(), &out).expect("write");

        let mut cfb = ::cfb::open(&out).expect("reopen");
        use std::io::Read;
        let mut flat = Vec::new();
        cfb.open_stream("/FlatStream")
            .expect("stream")
            .read_to_end(&mut flat)
            .expect("read");
        assert_eq!(flat, vec![0x11, 0x22, 0x33]);

        let mut blob = Vec::new();
        cfb.open_stream("/Nested/Blob")
            .expect("stream")
            .read_to_end(&mut blob)
            .expect("read");
        assert_eq!(blob, vec![0xAA, 0xBB]);

        let _ = std::fs::remove_file(&out);
    }
}
