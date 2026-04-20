//! Phase 10a (v0.6.0) — SPPID parse coverage inventory.
//!
//! This module answers the "how much of this `.pid` file has pid-parse
//! actually decoded?" question as a structured [`CoverageReport`]
//! rather than the earlier binary "known vs unidentified" view (see
//! [`super::unidentified_top_level_streams`], which stays in place for
//! backward compatibility).
//!
//! The v0.6.0 classification is **static**: each top-level stream or
//! storage name in [`KNOWN_TOP_LEVEL_STREAM_NAMES`] /
//! [`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`] is hard-coded to a
//! [`ParseCoverageStatus`] based on the current parser implementation
//! state. A future iteration (Phase 10b+) can upgrade the classifier to
//! consult the `PidDocument` itself (e.g. "model field non-None implies
//! decoded", "byte consumption ratio implies partial") once a byte-
//! consumption framework lands. For now, a stable static mapping is
//! sufficient to drive the roadmap's next priority calls.
//!
//! See:
//! - `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` (strategy)
//! - `docs/plans/2026-04-21-sppid-coverage-inventory-implementation-plan.md`
//!   (tactical Task 1..6 breakdown this file implements).

use super::{KNOWN_TOP_LEVEL_STORAGE_PREFIXES, KNOWN_TOP_LEVEL_STREAM_NAMES};
use crate::model::{
    CoverageEntry, CoverageNodeKind, CoverageReport, ParseCoverageStatus, PidDocument,
};
use std::collections::BTreeSet;

/// Build a [`CoverageReport`] for `doc`. Entries are sorted ascending by
/// `name` so callers get deterministic output regardless of
/// `doc.streams` insertion order.
pub fn coverage_report(doc: &PidDocument) -> CoverageReport {
    CoverageReport {
        entries: top_level_coverage_entries(doc),
    }
}

/// Compute one [`CoverageEntry`] per distinct top-level name reachable
/// from `doc.streams`. Prefer [`coverage_report`] for most callers;
/// this lower-level fn is public so tooling can slice the result.
pub fn top_level_coverage_entries(doc: &PidDocument) -> Vec<CoverageEntry> {
    // 1. Collect every distinct top-level name by stripping leading `/`
    //    and cutting at the first path separator.
    let mut top_level_names: BTreeSet<String> = BTreeSet::new();
    for stream in &doc.streams {
        let trimmed = stream.path.trim_start_matches('/');
        let head = match trimmed.find('/') {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        };
        if head.is_empty() {
            continue;
        }
        top_level_names.insert(head.to_string());
    }

    // 2. Classify each. `BTreeSet` iteration is already name-sorted.
    top_level_names
        .into_iter()
        .map(|name| classify(&name))
        .collect()
}

/// Map a bare top-level name to a full [`CoverageEntry`]. Visible under
/// `pub(super)` so Phase 10b's future classifier can slide in next to
/// us without disturbing the public API.
fn classify(name: &str) -> CoverageEntry {
    // Known storage prefix check first: a stream like "Sheet1" that
    // matches a storage prefix should be classified as storage even if
    // it coincidentally appears in `KNOWN_TOP_LEVEL_STREAM_NAMES`.
    if is_known_storage_prefix(name) {
        return CoverageEntry {
            name: name.to_string(),
            kind: CoverageNodeKind::TopLevelStorage,
            status: ParseCoverageStatus::IdentifiedOnly,
            parser: parser_for_storage(name),
            document_field: document_field_for_storage(name),
            note: Some("storage prefix recognized; member streams carry the data".into()),
        };
    }

    if KNOWN_TOP_LEVEL_STREAM_NAMES.contains(&name) {
        let (status, parser, field, note) = known_stream_state(name);
        return CoverageEntry {
            name: name.to_string(),
            kind: CoverageNodeKind::TopLevelStream,
            status,
            parser,
            document_field: field,
            note,
        };
    }

    CoverageEntry {
        name: name.to_string(),
        kind: CoverageNodeKind::TopLevelStream,
        status: ParseCoverageStatus::Unknown,
        parser: None,
        document_field: None,
        note: Some("no decoder; not in KNOWN_TOP_LEVEL_STREAM_NAMES".into()),
    }
}

/// True if `name` starts with any entry of
/// [`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`]. Uses prefix match so e.g.
/// `"Sheet1"` / `"Sheet42"` / `"JSite0123"` all land as storage.
fn is_known_storage_prefix(name: &str) -> bool {
    KNOWN_TOP_LEVEL_STORAGE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn parser_for_storage(name: &str) -> Option<String> {
    if name.starts_with("Sheet") {
        Some("streams::sheets".into())
    } else if name.starts_with("TaggedTxtData") {
        Some("streams::tagged_text".into())
    } else if name.starts_with("JSite") {
        Some("streams::jsite".into())
    } else {
        None
    }
}

fn document_field_for_storage(name: &str) -> Option<String> {
    if name.starts_with("Sheet") {
        Some("sheet_streams".into())
    } else if name.starts_with("TaggedTxtData") {
        Some("drawing_meta / general_meta".into())
    } else if name.starts_with("JSite") {
        Some("jsites".into())
    } else {
        None
    }
}

/// Phase 10a static mapping — each known top-level stream gets a
/// hard-coded `(status, parser, document_field, note)` quadruple.
/// Updates to this table are the ONE place Phase 10b+ needs to
/// re-review once dynamic classification comes online.
fn known_stream_state(
    name: &str,
) -> (
    ParseCoverageStatus,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    use ParseCoverageStatus::*;
    match name {
        "\u{5}SummaryInformation" => (
            FullyDecoded,
            Some("streams::summary".into()),
            Some("summary".into()),
            None,
        ),
        "\u{5}DocumentSummaryInformation" => (
            FullyDecoded,
            Some("streams::summary".into()),
            Some("summary".into()),
            None,
        ),
        "PSMroots" => (
            FullyDecoded,
            Some("streams::psm_roots".into()),
            Some("psm_roots".into()),
            None,
        ),
        "PSMclustertable" => (
            PartiallyDecoded,
            Some("streams::psm_tables".into()),
            Some("psm_cluster_table".into()),
            Some("record count + ids known; per-record field semantics still audit-only".into()),
        ),
        "PSMsegmenttable" => (
            PartiallyDecoded,
            Some("streams::psm_tables".into()),
            Some("psm_segment_table".into()),
            Some("segment record shape partially mapped; no stable field naming yet".into()),
        ),
        "DocVersion2" => (
            FullyDecoded,
            Some("parsers::doc_version2".into()),
            Some("doc_version2_decoded".into()),
            Some("Phase 9f reversed; 9-byte stride records with op_type + version number".into()),
        ),
        "DocVersion3" => (
            FullyDecoded,
            Some("streams::version_history".into()),
            Some("version_history".into()),
            None,
        ),
        "AppObject" => (
            FullyDecoded,
            Some("streams::app_object".into()),
            Some("app_object_registry".into()),
            None,
        ),
        "JTaggedTxtStgList" => (
            FullyDecoded,
            Some("streams::tagged_txt_list".into()),
            Some("tagged_storages".into()),
            None,
        ),
        "PSMcluster0" | "StyleCluster" => (
            PartiallyDecoded,
            Some("streams::cluster".into()),
            Some("clusters".into()),
            Some("header + record boundaries known; inner fields audit-only".into()),
        ),
        "Dynamic Attributes Metadata" | "Unclustered Dynamic Attributes" => (
            PartiallyDecoded,
            Some("streams::dyn_attrs".into()),
            Some("dynamic_attrs".into()),
            Some(
                "class/attribute tables extracted; per-object binding inferred at graph level"
                    .into(),
            ),
        ),
        // Any KNOWN_TOP_LEVEL_STREAM_NAMES entry we forgot to list above
        // falls through here — should never happen in practice because
        // `classify()` only calls us for names already in that list.
        _ => (
            IdentifiedOnly,
            None,
            None,
            Some("known name but coverage mapping not listed; update inspect::coverage".into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PidDocument, StreamEntry};

    fn doc_with_paths(paths: &[&str]) -> PidDocument {
        PidDocument {
            streams: paths
                .iter()
                .map(|p| StreamEntry {
                    path: (*p).to_string(),
                    size: 0,
                    preview_ascii: vec![],
                    magic_u32_le: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn find<'a>(report: &'a CoverageReport, name: &str) -> &'a CoverageEntry {
        report
            .entries
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("entry '{name}' missing; have: {:?}", report.entries))
    }

    #[test]
    fn coverage_marks_known_top_level_streams_with_expected_status() {
        let doc = doc_with_paths(&[
            "/DocVersion3",
            "/PSMsegmenttable",
            "/AppObject",
            "/PSMclustertable",
        ]);
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "DocVersion3").status,
            ParseCoverageStatus::FullyDecoded,
        );
        assert_eq!(
            find(&report, "AppObject").status,
            ParseCoverageStatus::FullyDecoded,
        );
        assert_eq!(
            find(&report, "PSMsegmenttable").status,
            ParseCoverageStatus::PartiallyDecoded,
        );
        assert_eq!(
            find(&report, "PSMclustertable").status,
            ParseCoverageStatus::PartiallyDecoded,
        );
    }

    #[test]
    fn coverage_marks_known_storage_prefixes_as_identified() {
        let doc = doc_with_paths(&[
            "/Sheet1/Foo",
            "/Sheet42/Bar",
            "/TaggedTxtData/Drawing",
            "/JSite0001/JProperties",
        ]);
        let report = coverage_report(&doc);
        for name in ["Sheet1", "Sheet42", "TaggedTxtData", "JSite0001"] {
            let entry = find(&report, name);
            assert_eq!(
                entry.status,
                ParseCoverageStatus::IdentifiedOnly,
                "storage prefix '{name}' should be IdentifiedOnly; got {:?}",
                entry.status,
            );
            assert_eq!(entry.kind, CoverageNodeKind::TopLevelStorage);
        }
    }

    #[test]
    fn coverage_marks_unknown_top_level_entries_as_unknown() {
        let doc = doc_with_paths(&[
            "/PSMroots",      // FullyDecoded
            "/GhostStream",   // Unknown
            "/Sheet1/Nested", // Sheet storage
            "/Random42",      // Unknown
        ]);
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "PSMroots").status,
            ParseCoverageStatus::FullyDecoded,
        );
        assert_eq!(
            find(&report, "GhostStream").status,
            ParseCoverageStatus::Unknown,
        );
        assert_eq!(
            find(&report, "Random42").status,
            ParseCoverageStatus::Unknown,
        );
        assert_eq!(
            find(&report, "Sheet1").kind,
            CoverageNodeKind::TopLevelStorage,
        );
    }

    #[test]
    fn coverage_entries_sorted_by_name_deterministic_across_input_orders() {
        let doc_a = doc_with_paths(&["/Zeta", "/Alpha", "/Mid"]);
        let doc_b = doc_with_paths(&["/Mid", "/Zeta", "/Alpha"]);
        let names_a: Vec<String> = coverage_report(&doc_a)
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        let names_b: Vec<String> = coverage_report(&doc_b)
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(names_a, names_b);
        assert_eq!(names_a, vec!["Alpha", "Mid", "Zeta"]);
    }

    #[test]
    fn coverage_report_empty_for_default_document() {
        let doc = PidDocument::default();
        let report = coverage_report(&doc);
        assert!(report.entries.is_empty());
        assert_eq!(report.status_counts(), [0, 0, 0, 0]);
    }

    #[test]
    fn coverage_status_counts_matches_entries() {
        let doc = doc_with_paths(&[
            "/DocVersion3",     // Full
            "/AppObject",       // Full
            "/PSMsegmenttable", // Partial
            "/Sheet1/x",        // Identified
            "/Sheet2/y",        // Identified
            "/GhostStream",     // Unknown
        ]);
        let report = coverage_report(&doc);
        let [full, partial, ident, unk] = report.status_counts();
        assert_eq!((full, partial, ident, unk), (2, 1, 2, 1));
    }
}
