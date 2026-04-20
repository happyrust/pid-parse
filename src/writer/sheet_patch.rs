//! Experimental byte-range patcher for sheet streams.
//!
//! This is *deliberately* low-level: a patch is just a half-open
//! `[start..end)` range plus a replacement byte vector. No semantic
//! understanding of the sheet record format is involved — that lives behind
//! `crate::streams::sheet` probes and is not yet stable enough to drive
//! writes. Callers willing to do their own decode/re-encode can use this
//! API to land surgical changes today.
//!
//! Patches inside a single [`SheetPatch`] are applied in **reverse order
//! by `start`** so that earlier offsets stay valid when later splices change
//! the buffer length.

use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::{SheetChunkPatch, SheetPatch};

/// Run the supplied patches against an in-memory byte buffer.
/// Returns the new buffer (length may differ from input).
///
/// Validation rules: `start <= end` and `end <= buffer.len()` for every
/// patch. Violations produce a [`PidError::ParseFailure`] with `context =
/// "sheet_patch"` and a message that pinpoints the offending offsets, so
/// failures stay actionable.
pub fn apply_sheet_patch(
    sheet_path: &str,
    buffer: &[u8],
    patches: &[SheetChunkPatch],
) -> Result<Vec<u8>, PidError> {
    let mut sorted = patches.to_vec();
    sorted.sort_by(|a, b| b.start.cmp(&a.start));

    let mut out = buffer.to_vec();
    for patch in &sorted {
        if patch.start > patch.end || patch.end > out.len() {
            return Err(PidError::ParseFailure {
                context: "sheet_patch".to_string(),
                message: format!(
                    "patch range [{}..{}) out of bounds for sheet '{}' (len={})",
                    patch.start,
                    patch.end,
                    sheet_path,
                    out.len()
                ),
            });
        }
        out.splice(patch.start..patch.end, patch.replacement.iter().copied());
    }
    Ok(out)
}

/// Convenience: pull the named sheet stream out of the package, run
/// [`apply_sheet_patch`] on its bytes, then write the result back via
/// [`PidPackage::replace_stream`].
pub fn apply_sheet_patch_to_package(
    package: &mut PidPackage,
    patch: &SheetPatch,
) -> Result<(), PidError> {
    let current = package
        .get_stream(&patch.sheet_path)
        .ok_or_else(|| PidError::MissingStream(patch.sheet_path.clone()))?
        .data
        .clone();
    let next = apply_sheet_patch(&patch.sheet_path, &current, &patch.chunk_patches)?;
    package.replace_stream(&patch.sheet_path, next);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splice_in_place_same_length() {
        let buf = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
        let patches = vec![SheetChunkPatch {
            start: 2,
            end: 5,
            replacement: vec![0xAA, 0xBB, 0xCC],
        }];
        let out = apply_sheet_patch("/X", &buf, &patches).unwrap();
        assert_eq!(out, vec![0, 1, 0xAA, 0xBB, 0xCC, 5, 6, 7]);
    }

    #[test]
    fn multiple_patches_apply_in_reverse_order() {
        let buf = vec![0u8; 10];
        let patches = vec![
            SheetChunkPatch {
                start: 0,
                end: 2,
                replacement: vec![0x11, 0x22],
            },
            SheetChunkPatch {
                start: 6,
                end: 8,
                replacement: vec![0x33, 0x44],
            },
        ];
        let out = apply_sheet_patch("/X", &buf, &patches).unwrap();
        assert_eq!(out, vec![0x11, 0x22, 0, 0, 0, 0, 0x33, 0x44, 0, 0]);
    }

    #[test]
    fn growing_patch_extends_buffer() {
        let buf = vec![1u8, 2, 3, 4];
        let patches = vec![SheetChunkPatch {
            start: 1,
            end: 2,
            replacement: vec![0xA, 0xB, 0xC],
        }];
        let out = apply_sheet_patch("/X", &buf, &patches).unwrap();
        assert_eq!(out, vec![1, 0xA, 0xB, 0xC, 3, 4]);
    }

    #[test]
    fn out_of_range_patch_errors() {
        let buf = vec![0u8; 4];
        let patches = vec![SheetChunkPatch {
            start: 2,
            end: 8,
            replacement: vec![],
        }];
        let err = apply_sheet_patch("/X", &buf, &patches).unwrap_err();
        match err {
            PidError::ParseFailure { context, .. } => assert_eq!(context, "sheet_patch"),
            other => panic!("expected ParseFailure, got {:?}", other),
        }
    }

    #[test]
    fn inverted_range_errors() {
        let buf = vec![0u8; 4];
        let patches = vec![SheetChunkPatch {
            start: 3,
            end: 1,
            replacement: vec![],
        }];
        assert!(apply_sheet_patch("/X", &buf, &patches).is_err());
    }
}
