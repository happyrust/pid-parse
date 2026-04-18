//! Byte-range patches for Sheet streams.
//!
//! This layer is **experimental**: it does not interpret the sheet body
//! (the v0.3.x parser only exposes `ClusterHeader` + endpoint records from
//! Sheet streams). The API lets callers splice arbitrary `[start, end)`
//! ranges so they can round-trip byte-level reverse-engineering work
//! without the writer silently corrupting unknown bytes.
//!
//! Contract recap (also in [`super::plan::SheetChunkPatch`]):
//!
//! - `[start, end)` is a half-open range over the existing stream bytes
//! - `start == end` is a pure insertion at `start`
//! - `end > data.len()` returns [`PidError::ParseFailure`]
//! - Patches within a single call are applied in **descending `start`** order
//!   so earlier offsets remain stable regardless of length deltas
use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::{SheetChunkPatch, SheetPatch};

/// Splice every [`SheetChunkPatch`] in `patches` into `data`. Validates each
/// range against the original length and returns the patched bytes. Keeps
/// `data` untouched on error.
pub fn apply_sheet_patch(
    data: &[u8],
    patches: &[SheetChunkPatch],
    context: &str,
) -> Result<Vec<u8>, PidError> {
    for p in patches {
        if p.start > p.end {
            return Err(PidError::ParseFailure {
                context: format!("sheet_patch:{}", context),
                message: format!("start {} > end {}", p.start, p.end),
            });
        }
        if p.end > data.len() {
            return Err(PidError::ParseFailure {
                context: format!("sheet_patch:{}", context),
                message: format!(
                    "end {} exceeds stream length {}",
                    p.end,
                    data.len()
                ),
            });
        }
    }

    // Apply descending by `start` so each splice's absolute offset remains
    // stable regardless of replacement length.
    let mut ordered: Vec<&SheetChunkPatch> = patches.iter().collect();
    ordered.sort_by_key(|p| std::cmp::Reverse(p.start));

    let mut out = data.to_vec();
    for p in ordered {
        out.splice(p.start..p.end, p.replacement.iter().cloned());
    }
    Ok(out)
}

/// High-level entry point: resolve the target sheet stream in `package`,
/// run [`apply_sheet_patch`], and write the result back. Returns
/// [`PidError::MissingStream`] when the sheet isn't present.
pub fn apply_sheet_patch_to_package(
    package: &mut PidPackage,
    patch: &SheetPatch,
) -> Result<(), PidError> {
    let raw = package
        .get_stream(&patch.sheet_path)
        .ok_or_else(|| PidError::MissingStream(patch.sheet_path.clone()))?;
    let new_bytes =
        apply_sheet_patch(&raw.data, &patch.chunk_patches, &patch.sheet_path)?;
    package.replace_stream(patch.sheet_path.clone(), new_bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::RawStream;
    use std::collections::BTreeMap;

    fn pkg_with_sheet(path: &str, data: &[u8]) -> PidPackage {
        let mut map = BTreeMap::new();
        map.insert(
            path.to_string(),
            RawStream {
                path: path.to_string(),
                data: data.to_vec(),
                modified: false,
            },
        );
        PidPackage::new(None, map, PidDocument::default())
    }

    #[test]
    fn single_range_replace_same_length() {
        let data = (0u8..16).collect::<Vec<_>>();
        let patches = vec![SheetChunkPatch {
            start: 4,
            end: 8,
            replacement: vec![0xAA; 4],
        }];
        let out = apply_sheet_patch(&data, &patches, "/Sheet0").expect("ok");
        assert_eq!(out.len(), 16);
        assert_eq!(&out[0..4], &[0, 1, 2, 3]);
        assert_eq!(&out[4..8], &[0xAA; 4]);
        assert_eq!(&out[8..16], &data[8..16]);
    }

    #[test]
    fn pure_insertion_extends_length() {
        let data = b"abcd".to_vec();
        let patches = vec![SheetChunkPatch {
            start: 2,
            end: 2,
            replacement: b"XY".to_vec(),
        }];
        let out = apply_sheet_patch(&data, &patches, "/Sheet0").expect("ok");
        assert_eq!(out, b"abXYcd");
    }

    #[test]
    fn multiple_patches_applied_descending() {
        // Without descending order, the second patch's offset would shift.
        let data = (0u8..10).collect::<Vec<_>>();
        let patches = vec![
            SheetChunkPatch {
                start: 2,
                end: 4,
                replacement: b"xx".to_vec(),
            },
            SheetChunkPatch {
                start: 6,
                end: 8,
                replacement: b"yy".to_vec(),
            },
        ];
        let out = apply_sheet_patch(&data, &patches, "/Sheet0").expect("ok");
        assert_eq!(out, b"\x00\x01xx\x04\x05yy\x08\x09");
    }

    #[test]
    fn out_of_range_end_errors() {
        let data = vec![0u8; 4];
        let patches = vec![SheetChunkPatch {
            start: 2,
            end: 99,
            replacement: vec![0xFF; 1],
        }];
        let err = apply_sheet_patch(&data, &patches, "/Sheet0").expect_err("oob");
        match err {
            PidError::ParseFailure { context, .. } => {
                assert!(context.starts_with("sheet_patch:"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_sheet_returns_missing_stream() {
        let mut pkg = pkg_with_sheet("/SheetA", &[0u8; 4]);
        let patch = SheetPatch {
            sheet_path: "/SheetB".to_string(),
            chunk_patches: vec![],
            experimental: true,
        };
        match apply_sheet_patch_to_package(&mut pkg, &patch) {
            Err(PidError::MissingStream(p)) => assert_eq!(p, "/SheetB"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn apply_to_package_writes_back_and_marks_modified() {
        let mut pkg = pkg_with_sheet("/Sheet0", &(0u8..8).collect::<Vec<_>>());
        let patch = SheetPatch {
            sheet_path: "/Sheet0".to_string(),
            chunk_patches: vec![SheetChunkPatch {
                start: 0,
                end: 0,
                replacement: b"HEAD".to_vec(),
            }],
            experimental: true,
        };
        apply_sheet_patch_to_package(&mut pkg, &patch).expect("ok");
        let s = pkg.get_stream("/Sheet0").expect("stream");
        assert_eq!(&s.data[..4], b"HEAD");
        assert_eq!(s.data.len(), 12);
        assert!(s.modified);
    }
}
