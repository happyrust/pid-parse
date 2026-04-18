//! Package layer: raw CFB stream bytes + parsed document in a single container.
//!
//! A [`PidPackage`] is produced by [`crate::PidParser::parse_package`]. It
//! preserves every stream's raw bytes (keyed by its normalized `/`-separated
//! path) alongside the structurally decoded [`PidDocument`]. This is the
//! input to [`crate::writer::PidWriter`]: by keeping the bytes verbatim we
//! can do passthrough round-trips and targeted metadata updates without
//! losing any stream we don't yet fully understand.
use crate::model::PidDocument;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Raw bytes of one CFB stream plus a dirty flag.
///
/// `path` is always normalized to use `/` separators and starts with `/`
/// (e.g. `/TaggedTxtData/Drawing`). `data` is the verbatim stream content
/// as read from the source CFB. `modified` is set to `true` the moment a
/// writer replaces the stream; consumers can use this flag to skip rewriting
/// streams that don't need it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStream {
    pub path: String,
    pub data: Vec<u8>,
    pub modified: bool,
}

/// Full package: raw stream bytes + parsed document.
///
/// Iteration order is deterministic because [`PidPackage::streams`] is a
/// [`BTreeMap`] keyed on the normalized stream path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidPackage {
    /// Source file on disk, if the package was read from a file. `None`
    /// for packages constructed in memory.
    pub source_path: Option<PathBuf>,
    /// All streams in the CFB, keyed by their normalized path.
    pub streams: BTreeMap<String, RawStream>,
    /// Structurally decoded model.
    pub parsed: PidDocument,
    /// CLSID of the root storage as read from the source CFB. `None` when
    /// the source didn't set one (cfb defaults to the nil UUID) or when
    /// the package was constructed in memory. The writer preserves this
    /// value via [`cfb::CompoundFile::set_storage_clsid`] on the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_clsid: Option<Uuid>,
    /// CLSIDs of non-root storages (directories inside the CFB) whose
    /// value is **not** the nil UUID. Keyed by the normalized storage
    /// path. Nil-valued storages are omitted to keep the map sparse —
    /// real SmartPlant samples almost never set non-root CLSIDs, so the
    /// typical real-file map is empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub storage_clsids: BTreeMap<String, Uuid>,
}

impl PidPackage {
    /// Build a fresh package from its three parts. Callers are responsible
    /// for providing a consistent `streams` / `parsed` pair (normally this
    /// is done by the parser).
    pub fn new(
        source_path: Option<PathBuf>,
        streams: BTreeMap<String, RawStream>,
        parsed: PidDocument,
    ) -> Self {
        Self {
            source_path,
            streams,
            parsed,
            root_clsid: None,
            storage_clsids: BTreeMap::new(),
        }
    }

    /// Attach a root CLSID, consuming and returning `self` for ergonomic
    /// builder-style use inside the parser.
    pub fn with_root_clsid(mut self, clsid: Option<Uuid>) -> Self {
        self.root_clsid = clsid;
        self
    }

    /// Attach the non-root storage CLSIDs map, builder-style.
    pub fn with_storage_clsids(mut self, clsids: BTreeMap<String, Uuid>) -> Self {
        self.storage_clsids = clsids;
        self
    }

    /// Look up a stream by its normalized path.
    pub fn get_stream(&self, path: &str) -> Option<&RawStream> {
        self.streams.get(path)
    }

    /// Mutable access to a stream (e.g. for in-place byte edits). Does not
    /// flip the `modified` flag on its own; use [`Self::replace_stream`] or
    /// flip the field manually if the data actually changes.
    pub fn get_stream_mut(&mut self, path: &str) -> Option<&mut RawStream> {
        self.streams.get_mut(path)
    }

    /// Replace (or insert) a stream's bytes and mark it as modified. The
    /// provided `path` is normalized just like the parser does.
    pub fn replace_stream(&mut self, path: impl Into<String>, data: Vec<u8>) {
        let path = normalize_path(&path.into());
        self.streams.insert(
            path.clone(),
            RawStream {
                path,
                data,
                modified: true,
            },
        );
    }

    /// Clear every `modified` flag. Useful after a successful write so the
    /// package can be re-used for further edits from a clean baseline.
    pub fn mark_unmodified(&mut self) {
        for raw in self.streams.values_mut() {
            raw.modified = false;
        }
    }

    /// Replace the text of a simple `<tag>...</tag>` element inside the
    /// UTF-8 XML stream at `stream_path`. Returns the old value on success
    /// so callers can log / diff the change.
    ///
    /// Fails with [`crate::error::PidError::MissingStream`] when the stream
    /// doesn't exist, with [`crate::error::PidError::ParseFailure`] when
    /// the stream isn't valid UTF-8, and propagates the errors of
    /// [`crate::writer::xml_edit::replace_simple_tag_text`] for missing or
    /// nested tags.
    pub fn set_xml_tag(
        &mut self,
        stream_path: &str,
        tag: &str,
        new_value: &str,
    ) -> Result<String, crate::error::PidError> {
        use crate::error::PidError;
        let raw = self
            .get_stream(stream_path)
            .ok_or_else(|| PidError::MissingStream(stream_path.to_string()))?;
        let old_xml =
            std::str::from_utf8(&raw.data).map_err(|e| PidError::ParseFailure {
                context: format!("set_xml_tag:{}", stream_path),
                message: format!("stream is not UTF-8: {}", e),
            })?;
        // Capture the old text before we rewrite so callers can report
        // what they replaced.
        let old_value = extract_simple_tag_text(old_xml, tag).unwrap_or_default();
        let new_xml = crate::writer::xml_edit::replace_simple_tag_text(old_xml, tag, new_value)?;
        self.replace_stream(stream_path, new_xml.into_bytes());
        Ok(old_value)
    }

    /// Shortcut for `set_xml_tag("/TaggedTxtData/Drawing", tag, value)`.
    pub fn set_drawing_xml_tag(
        &mut self,
        tag: &str,
        new_value: &str,
    ) -> Result<String, crate::error::PidError> {
        self.set_xml_tag("/TaggedTxtData/Drawing", tag, new_value)
    }

    /// Shortcut for `set_xml_tag("/TaggedTxtData/General", tag, value)`.
    pub fn set_general_xml_tag(
        &mut self,
        tag: &str,
        new_value: &str,
    ) -> Result<String, crate::error::PidError> {
        self.set_xml_tag("/TaggedTxtData/General", tag, new_value)
    }
}

/// Scan `xml` for the first occurrence of `<tag>...</tag>` (simple open form,
/// no attributes) and return the inner text verbatim. Used by
/// [`PidPackage::set_xml_tag`] to report the old value without re-running
/// the full XML parser.
fn extract_simple_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end_rel = xml[start..].find(&close)?;
    Some(xml[start..start + end_rel].to_string())
}

/// Stream-level diff between two packages.
///
/// Produced by [`diff_packages`]. `only_in_a` / `only_in_b` list paths
/// that are present in exactly one side; `modified` lists paths present in
/// both where the bytes differ. `root_clsid_match` is `false` if the
/// packages carry different root CLSIDs (including one having `Some` and
/// the other `None`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageDiff {
    pub only_in_a: Vec<String>,
    pub only_in_b: Vec<String>,
    pub modified: Vec<StreamDiff>,
    pub root_clsid_match: bool,
    pub root_clsid_a: Option<Uuid>,
    pub root_clsid_b: Option<Uuid>,
    /// Storage paths whose non-root CLSID differs between A and B. For
    /// each path, carries the two values (either side may be `None` when
    /// the storage wasn't CLSID-stamped on one side).
    pub storage_clsid_diffs: Vec<StorageClsidDiff>,
}

/// One-entry diff for a non-root storage CLSID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClsidDiff {
    pub path: String,
    pub a: Option<Uuid>,
    pub b: Option<Uuid>,
}

impl PackageDiff {
    /// `true` iff the two packages are stream-identical at the byte level,
    /// share the same root CLSID, and carry identical non-root storage
    /// CLSIDs.
    pub fn is_empty(&self) -> bool {
        self.only_in_a.is_empty()
            && self.only_in_b.is_empty()
            && self.modified.is_empty()
            && self.root_clsid_match
            && self.storage_clsid_diffs.is_empty()
    }

    /// Total number of differences (streams + non-root CLSID deltas).
    /// Useful for one-line "N diffs" summaries.
    pub fn diff_count(&self) -> usize {
        self.only_in_a.len()
            + self.only_in_b.len()
            + self.modified.len()
            + self.storage_clsid_diffs.len()
    }
}

/// Per-stream byte diff summary. `first_mismatch_offset` is the first
/// index where the two byte strings disagree (or `min(len_a, len_b)` when
/// one is a strict prefix of the other). `context_before` / `context_after`
/// are short hex previews around that offset for quick inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDiff {
    pub path: String,
    pub len_a: usize,
    pub len_b: usize,
    pub first_mismatch_offset: usize,
    pub context_before: String,
    pub context_after: String,
}

/// Compute a byte-level diff between two packages.
pub fn diff_packages(a: &PidPackage, b: &PidPackage) -> PackageDiff {
    use std::collections::BTreeSet;

    let paths_a: BTreeSet<&String> = a.streams.keys().collect();
    let paths_b: BTreeSet<&String> = b.streams.keys().collect();

    let only_in_a: Vec<String> = paths_a
        .difference(&paths_b)
        .map(|s| (*s).clone())
        .collect();
    let only_in_b: Vec<String> = paths_b
        .difference(&paths_a)
        .map(|s| (*s).clone())
        .collect();

    let mut modified: Vec<StreamDiff> = Vec::new();
    for path in paths_a.intersection(&paths_b) {
        let ra = a.streams.get(*path).expect("intersection");
        let rb = b.streams.get(*path).expect("intersection");
        if ra.data == rb.data {
            continue;
        }
        let offset = first_mismatch_offset(&ra.data, &rb.data);
        modified.push(StreamDiff {
            path: (*path).clone(),
            len_a: ra.data.len(),
            len_b: rb.data.len(),
            first_mismatch_offset: offset,
            context_before: hex_preview(&ra.data, offset),
            context_after: hex_preview(&rb.data, offset),
        });
    }

    // Non-root storage CLSID diffs: scan the union of keys from both
    // maps; produce one entry per path whose values don't match (either
    // one side missing, or both present with different values).
    let mut storage_clsid_diffs: Vec<StorageClsidDiff> = Vec::new();
    let clsid_paths: BTreeSet<&String> = a
        .storage_clsids
        .keys()
        .chain(b.storage_clsids.keys())
        .collect();
    for path in clsid_paths {
        let va = a.storage_clsids.get(path).copied();
        let vb = b.storage_clsids.get(path).copied();
        if va != vb {
            storage_clsid_diffs.push(StorageClsidDiff {
                path: path.clone(),
                a: va,
                b: vb,
            });
        }
    }

    PackageDiff {
        only_in_a,
        only_in_b,
        modified,
        root_clsid_match: a.root_clsid == b.root_clsid,
        root_clsid_a: a.root_clsid,
        root_clsid_b: b.root_clsid,
        storage_clsid_diffs,
    }
}

fn first_mismatch_offset(a: &[u8], b: &[u8]) -> usize {
    let common = a.len().min(b.len());
    for i in 0..common {
        if a[i] != b[i] {
            return i;
        }
    }
    common
}

/// Render 16 bytes starting at `offset` (or fewer if out of range) as
/// `xx xx xx ...`. Used for the `context_before` / `context_after`
/// diagnostics in [`StreamDiff`].
fn hex_preview(data: &[u8], offset: usize) -> String {
    if offset >= data.len() {
        return "(eof)".to_string();
    }
    let end = (offset + 16).min(data.len());
    data[offset..end]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize a CFB-style path: convert `\` to `/` and ensure it starts with
/// `/`. This mirrors how the parser stores paths.
pub(crate) fn normalize_path(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    if replaced.starts_with('/') {
        replaced
    } else {
        format!("/{}", replaced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> PidDocument {
        PidDocument::default()
    }

    #[test]
    fn normalize_path_adds_leading_slash() {
        assert_eq!(normalize_path("TaggedTxtData/Drawing"), "/TaggedTxtData/Drawing");
        assert_eq!(normalize_path("/TaggedTxtData/Drawing"), "/TaggedTxtData/Drawing");
        assert_eq!(normalize_path("\\foo\\bar"), "/foo/bar");
    }

    #[test]
    fn replace_stream_marks_modified_and_normalizes() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg.replace_stream("TaggedTxtData/Drawing", b"<Drawing/>".to_vec());
        let got = pkg.get_stream("/TaggedTxtData/Drawing").expect("stream");
        assert_eq!(got.path, "/TaggedTxtData/Drawing");
        assert_eq!(got.data, b"<Drawing/>");
        assert!(got.modified);
    }

    #[test]
    fn mark_unmodified_clears_dirty_flags() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg.replace_stream("/A", vec![1, 2, 3]);
        pkg.replace_stream("/B", vec![4]);
        pkg.mark_unmodified();
        assert!(pkg.streams.values().all(|s| !s.modified));
    }

    #[test]
    fn with_root_clsid_round_trips_value() {
        let clsid = Uuid::parse_str("00020906-0000-0000-C000-000000000046").expect("uuid");
        let pkg = PidPackage::new(None, BTreeMap::new(), sample_doc())
            .with_root_clsid(Some(clsid));
        assert_eq!(pkg.root_clsid, Some(clsid));
    }

    #[test]
    fn set_xml_tag_returns_old_value_and_updates_bytes() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg.replace_stream(
            "/TaggedTxtData/Drawing",
            b"<Drawing><Template>OLD</Template></Drawing>".to_vec(),
        );
        let old = pkg
            .set_xml_tag("/TaggedTxtData/Drawing", "Template", "NEW")
            .expect("ok");
        assert_eq!(old, "OLD");
        let after = pkg.get_stream("/TaggedTxtData/Drawing").expect("stream");
        assert_eq!(
            std::str::from_utf8(&after.data).unwrap(),
            "<Drawing><Template>NEW</Template></Drawing>"
        );
        assert!(after.modified);
    }

    #[test]
    fn set_xml_tag_missing_stream_returns_missing_stream() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        let err = pkg
            .set_xml_tag("/No/Such/Stream", "x", "y")
            .expect_err("missing");
        assert!(matches!(
            err,
            crate::error::PidError::MissingStream(ref p) if p == "/No/Such/Stream"
        ));
    }

    #[test]
    fn set_xml_tag_rejects_non_utf8_stream() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg.replace_stream("/TaggedTxtData/Drawing", vec![0xFF, 0xFE, 0x00]);
        let err = pkg
            .set_xml_tag("/TaggedTxtData/Drawing", "x", "y")
            .expect_err("utf8");
        match err {
            crate::error::PidError::ParseFailure { context, .. } => {
                assert!(context.contains("set_xml_tag"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn diff_empty_when_packages_identical() {
        let mut pkg_a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg_a.replace_stream("/A", b"hello".to_vec());
        pkg_a.replace_stream("/B/C", b"world".to_vec());
        let pkg_b = pkg_a.clone();
        let d = diff_packages(&pkg_a, &pkg_b);
        assert!(d.is_empty(), "identical packages should produce empty diff");
        assert_eq!(d.diff_count(), 0);
    }

    #[test]
    fn diff_reports_only_in_a_and_only_in_b() {
        let mut a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        a.replace_stream("/only-a", vec![1]);
        a.replace_stream("/shared", vec![9]);
        let mut b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        b.replace_stream("/shared", vec![9]);
        b.replace_stream("/only-b", vec![2]);

        let d = diff_packages(&a, &b);
        assert_eq!(d.only_in_a, vec!["/only-a".to_string()]);
        assert_eq!(d.only_in_b, vec!["/only-b".to_string()]);
        assert!(d.modified.is_empty());
        assert_eq!(d.diff_count(), 2);
    }

    #[test]
    fn diff_reports_byte_level_mismatch_with_context() {
        let mut a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        a.replace_stream("/s", (0u8..20).collect());
        let mut b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        let mut bdata: Vec<u8> = (0u8..20).collect();
        bdata[7] = 0xFF;
        b.replace_stream("/s", bdata);

        let d = diff_packages(&a, &b);
        assert_eq!(d.modified.len(), 1);
        let m = &d.modified[0];
        assert_eq!(m.path, "/s");
        assert_eq!(m.len_a, 20);
        assert_eq!(m.len_b, 20);
        assert_eq!(m.first_mismatch_offset, 7);
        assert!(m.context_before.starts_with("07 08 09"));
        assert!(m.context_after.starts_with("ff 08 09"));
    }

    #[test]
    fn with_storage_clsids_round_trips_map() {
        let mut map = BTreeMap::new();
        let clsid = Uuid::parse_str("00020906-0000-0000-C000-000000000046").unwrap();
        map.insert("/JSite0".to_string(), clsid);
        let pkg = PidPackage::new(None, BTreeMap::new(), sample_doc())
            .with_storage_clsids(map.clone());
        assert_eq!(pkg.storage_clsids, map);
    }

    #[test]
    fn diff_flags_non_root_storage_clsid_mismatch() {
        let clsid_a = Uuid::parse_str("00020906-0000-0000-C000-000000000046").unwrap();
        let clsid_b = Uuid::parse_str("00020907-0000-0000-C000-000000000046").unwrap();

        let mut a_map = BTreeMap::new();
        a_map.insert("/JSite0".to_string(), clsid_a);
        let a = PidPackage::new(None, BTreeMap::new(), sample_doc())
            .with_storage_clsids(a_map);

        let mut b_map = BTreeMap::new();
        b_map.insert("/JSite0".to_string(), clsid_b);
        let b = PidPackage::new(None, BTreeMap::new(), sample_doc())
            .with_storage_clsids(b_map);

        let d = diff_packages(&a, &b);
        assert_eq!(d.storage_clsid_diffs.len(), 1);
        assert_eq!(d.storage_clsid_diffs[0].path, "/JSite0");
        assert_eq!(d.storage_clsid_diffs[0].a, Some(clsid_a));
        assert_eq!(d.storage_clsid_diffs[0].b, Some(clsid_b));
        assert!(!d.is_empty(), "storage CLSID mismatch should make diff non-empty");
        assert_eq!(d.diff_count(), 1);
    }

    #[test]
    fn diff_reports_missing_non_root_clsid_on_one_side() {
        let clsid = Uuid::parse_str("00020906-0000-0000-C000-000000000046").unwrap();
        let mut a_map = BTreeMap::new();
        a_map.insert("/JSite0".to_string(), clsid);
        let a = PidPackage::new(None, BTreeMap::new(), sample_doc())
            .with_storage_clsids(a_map);
        let b = PidPackage::new(None, BTreeMap::new(), sample_doc());

        let d = diff_packages(&a, &b);
        assert_eq!(d.storage_clsid_diffs.len(), 1);
        let only = &d.storage_clsid_diffs[0];
        assert_eq!(only.a, Some(clsid));
        assert_eq!(only.b, None);
    }

    #[test]
    fn diff_flags_root_clsid_mismatch() {
        let clsid = Uuid::parse_str("00020906-0000-0000-C000-000000000046").unwrap();
        let a = PidPackage::new(None, BTreeMap::new(), sample_doc()).with_root_clsid(Some(clsid));
        let b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        let d = diff_packages(&a, &b);
        assert!(
            !d.root_clsid_match,
            "mismatched CLSIDs should flag root_clsid_match=false"
        );
        assert_eq!(d.root_clsid_a, Some(clsid));
        assert_eq!(d.root_clsid_b, None);
        assert!(
            !d.is_empty(),
            "CLSID-only differences should still make diff non-empty"
        );
    }

    #[test]
    fn diff_prefix_reports_first_mismatch_at_common_length() {
        let mut a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        a.replace_stream("/s", b"abc".to_vec());
        let mut b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        b.replace_stream("/s", b"abcdef".to_vec());
        let d = diff_packages(&a, &b);
        let m = &d.modified[0];
        assert_eq!(m.first_mismatch_offset, 3);
        assert_eq!(m.context_before, "(eof)");
        assert!(m.context_after.starts_with("64 65 66"));
    }

    #[test]
    fn set_drawing_xml_tag_shortcut_delegates_to_set_xml_tag() {
        let mut pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        pkg.replace_stream(
            "/TaggedTxtData/Drawing",
            b"<Drawing><Name>old</Name></Drawing>".to_vec(),
        );
        let old = pkg.set_drawing_xml_tag("Name", "new").expect("ok");
        assert_eq!(old, "old");
        let data = &pkg.get_stream("/TaggedTxtData/Drawing").unwrap().data;
        assert_eq!(
            std::str::from_utf8(data).unwrap(),
            "<Drawing><Name>new</Name></Drawing>"
        );
    }
}
