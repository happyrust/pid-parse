//! Package layer: keeps every CFB stream's raw bytes alongside the parsed
//! `PidDocument`, so downstream code can both *read* the structured view and
//! *re-emit* the file (with optional surgical edits) via [`crate::writer`].
//!
//! `PidPackage` is intentionally cheap to clone: bytes live in `Vec<u8>` and
//! parsing results live in `PidDocument`. Stream paths are normalized to
//! forward-slash form (`/Storage/Stream`) so downstream lookups don't need to
//! care about the host platform's `Path` separator.

use crate::model::PidDocument;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One CFB stream's raw bytes, plus a flag tracking whether the package has
/// been edited since parsing.
#[derive(Debug, Clone)]
pub struct RawStream {
    /// Forward-slash normalized CFB path, e.g. `/TaggedTxtData/Drawing`.
    pub path: String,
    pub data: Vec<u8>,
    /// `true` once `replace_stream` has touched this entry; passthrough
    /// streams stay `false`. Useful for callers that want to log "what
    /// changed" before writing.
    pub modified: bool,
}

/// Owning bundle of raw stream bytes + parser output.
///
/// Construct via [`crate::api::PidParser::parse_package`]. Mutations are
/// surgical: use [`PidPackage::replace_stream`] to swap one stream's bytes,
/// or feed a [`crate::writer::WritePlan`] to [`crate::writer::PidWriter`] for
/// composed edits (metadata XML / sheet patches).
#[derive(Debug, Clone)]
pub struct PidPackage {
    /// Original on-disk path if the package was loaded from a file.
    /// `None` for synthetic / in-memory packages.
    pub source_path: Option<PathBuf>,
    /// All CFB streams, keyed by normalized forward-slash path.
    pub streams: BTreeMap<String, RawStream>,
    /// Parser-derived structured view of the same file.
    pub parsed: PidDocument,
}

impl PidPackage {
    pub fn get_stream(&self, path: &str) -> Option<&RawStream> {
        self.streams.get(path)
    }

    pub fn get_stream_mut(&mut self, path: &str) -> Option<&mut RawStream> {
        self.streams.get_mut(path)
    }

    /// Replace a stream's bytes wholesale. Marks the entry `modified=true`.
    /// Inserts a new entry if the path was previously absent — callers who
    /// want strict "must already exist" semantics should check
    /// [`PidPackage::get_stream`] first.
    pub fn replace_stream(&mut self, path: &str, data: Vec<u8>) {
        self.streams
            .entry(path.to_string())
            .and_modify(|s| {
                s.data = data.clone();
                s.modified = true;
            })
            .or_insert_with(|| RawStream {
                path: path.to_string(),
                data,
                modified: true,
            });
    }

    /// Reset every stream's `modified` flag back to `false`. Typically called
    /// right after a successful write so the next edit pass starts from a
    /// clean baseline.
    pub fn mark_unmodified(&mut self) {
        for s in self.streams.values_mut() {
            s.modified = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_pkg() -> PidPackage {
        PidPackage {
            source_path: None,
            streams: BTreeMap::new(),
            parsed: PidDocument::default(),
        }
    }

    #[test]
    fn replace_stream_inserts_new_entry_marked_modified() {
        let mut pkg = empty_pkg();
        pkg.replace_stream("/A/B", vec![1, 2, 3]);
        let s = pkg.get_stream("/A/B").unwrap();
        assert_eq!(s.data, vec![1, 2, 3]);
        assert!(s.modified);
        assert_eq!(s.path, "/A/B");
    }

    #[test]
    fn replace_stream_overwrites_existing_and_flags_modified() {
        let mut pkg = empty_pkg();
        pkg.streams.insert(
            "/X".into(),
            RawStream {
                path: "/X".into(),
                data: vec![0xff; 4],
                modified: false,
            },
        );
        pkg.replace_stream("/X", vec![0xaa; 2]);
        let s = pkg.get_stream("/X").unwrap();
        assert_eq!(s.data, vec![0xaa; 2]);
        assert!(s.modified);
    }

    #[test]
    fn mark_unmodified_resets_all_flags() {
        let mut pkg = empty_pkg();
        pkg.replace_stream("/A", vec![1]);
        pkg.replace_stream("/B", vec![2]);
        pkg.mark_unmodified();
        assert!(pkg.streams.values().all(|s| !s.modified));
    }
}
