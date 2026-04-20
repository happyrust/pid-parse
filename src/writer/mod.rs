//! Writer layer: apply a [`WritePlan`] to a [`PidPackage`] and emit a new
//! CFB file. The first release supports:
//!
//! - **Passthrough round-trip**: `PidWriter::write_to(&pkg, &WritePlan::default(), out)`
//!   rebuilds the container with identical stream bytes (order by
//!   lexicographic path).
//! - **Metadata-only updates**: replace `/TaggedTxtData/Drawing` and
//!   `/TaggedTxtData/General` XML bodies.
//! - **Stream replacements**: verbatim byte replacement of any stream.
//! - **Experimental Sheet byte-range patches** (see [`sheet_patch`]).
//!
//! See `docs/writer-layer-plan.md` for the intentional constraints (no
//! `SummaryInformation` property-set writer; no CLSID / timestamp
//! preservation; BOM / UTF-16 encoding is the caller's responsibility).
pub mod cfb_write;
pub mod metadata_helpers;
pub mod metadata_write;
pub mod plan;
pub mod sheet_patch;
pub mod xml_edit;

use crate::error::PidError;
use crate::package::PidPackage;
use std::path::Path;

pub use metadata_helpers::{
    get_drawing_attribute, get_general_element_text, list_drawing_attributes,
    list_general_elements, set_drawing_attribute, set_drawing_number, set_element_text,
    set_general_file_path, MetadataEditError,
};
pub use plan::{MetadataUpdates, SheetChunkPatch, SheetPatch, StreamReplacement, WritePlan};

/// High-level writer entry point. See [`Self::write_to`].
pub struct PidWriter;

impl PidWriter {
    /// Apply `plan` to a **clone** of `package` and write the result to
    /// `output`. The caller's package is not mutated, matching the
    /// "immutable parse result, declarative plan" model.
    pub fn write_to(package: &PidPackage, plan: &WritePlan, output: &Path) -> Result<(), PidError> {
        let mut working = package.clone();

        metadata_write::apply_metadata_updates(&mut working, &plan.metadata_updates)?;

        for repl in &plan.stream_replacements {
            working.replace_stream(repl.path.clone(), repl.new_data.clone());
        }

        for patch in &plan.sheet_patches {
            sheet_patch::apply_sheet_patch_to_package(&mut working, patch)?;
        }

        cfb_write::write_package(&working, output)
    }
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
