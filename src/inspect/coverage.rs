//! SPPID parse coverage inventory (Phase 10a foundation + Phase 10b
//! dynamic classification).
//!
//! This module answers the "how much of this `.pid` file has pid-parse
//! actually decoded?" question as a structured [`CoverageReport`]
//! rather than the earlier binary "known vs unidentified" view (see
//! [`super::unidentified_top_level_streams`], which stays in place for
//! backward compatibility).
//!
//! **v0.6.0 (Phase 10a)**: each top-level stream or storage name in
//! [`KNOWN_TOP_LEVEL_STREAM_NAMES`] / [`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`]
//! mapped to a static [`ParseCoverageStatus`].
//!
//! **v0.6.1 (Phase 10b)**: the classifier now additionally consults the
//! [`PidDocument`] itself. A stream that is statically declared
//! "FullyDecoded" but whose corresponding model field is `None` or empty
//! is **downgraded** to `IdentifiedOnly` with a note explaining why.
//! This surfaces silent parser failures — if the stream is present but
//! the model field is empty, the parser tolerated an input it should
//! have understood, and the coverage report will stop pretending
//! otherwise.
//!
//! **v0.6.2 (Phase 10c)**: cluster & dynamic-attrs probes wired. The
//! four names parked in v0.6.1 (`PSMcluster0`, `StyleCluster`, `Dynamic
//! Attributes Metadata`, `Unclustered Dynamic Attributes`) now consult
//! `doc.clusters` (by `ClusterKind`) and `doc.dynamic_attributes` for
//! downgrade decisions, completing dynamic coverage for every current
//! `KNOWN_TOP_LEVEL_STREAM_NAMES` entry.
//!
//! See:
//! - `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` (strategy)
//! - `docs/plans/2026-04-21-sppid-coverage-inventory-implementation-plan.md`
//!   (Phase 10a tactical plan)
//! - `docs/plans/2026-04-21-phase-10b-dynamic-coverage.md`
//!   (Phase 10b tactical plan)

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
        .map(|name| {
            let mut entry = classify(&name, doc);
            entry.stream_size = size_for_top_level(doc, &name);
            entry
        })
        .collect()
}

/// Phase 10f (v0.6.5+): aggregate stream sizes across every
/// `doc.streams` entry whose top-level name matches `top_level_name`.
/// A top-level stream (`"/DocVersion3"`) contributes its own size; a
/// storage prefix (`"Sheet1"`) sums every child (`"/Sheet1/Foo"`,
/// `"/Sheet1/Bar"`, …). Returns `None` when no matching stream exists
/// — indicating `doc.streams` became inconsistent with the name
/// derivation pipeline.
fn size_for_top_level(doc: &PidDocument, top_level_name: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut found_any = false;
    for stream in &doc.streams {
        let trimmed = stream.path.trim_start_matches('/');
        let head = match trimmed.find('/') {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        };
        if head == top_level_name {
            total = total.saturating_add(stream.size);
            found_any = true;
        }
    }
    found_any.then_some(total)
}

/// Map a bare top-level name to a full [`CoverageEntry`], considering
/// both the static stream-name table (Phase 10a) and the actual
/// [`PidDocument`] field population (Phase 10b).
///
/// The static tier decides the **ceiling**: if a name is not known at
/// all, we can only ever report `Unknown`; if a storage prefix matches,
/// we report `IdentifiedOnly`. For statically `FullyDecoded` /
/// `PartiallyDecoded` names, the dynamic tier can *downgrade* to
/// `IdentifiedOnly` when the corresponding model field is `None` /
/// empty, revealing parser silent-failures or fixtures the decoder has
/// not yet grown to handle.
fn classify(name: &str, doc: &PidDocument) -> CoverageEntry {
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
            // Filled in by `top_level_coverage_entries` after classify.
            stream_size: None,
        };
    }

    if KNOWN_TOP_LEVEL_STREAM_NAMES.contains(&name) {
        let (static_status, parser, field, note) = known_stream_state(name);
        // Phase 10b: apply dynamic downgrade based on doc population.
        let (status, note) = apply_dynamic_downgrade(name, static_status, note, doc);
        return CoverageEntry {
            name: name.to_string(),
            kind: CoverageNodeKind::TopLevelStream,
            status,
            parser,
            document_field: field,
            note,
            stream_size: None,
        };
    }

    CoverageEntry {
        name: name.to_string(),
        kind: CoverageNodeKind::TopLevelStream,
        status: ParseCoverageStatus::Unknown,
        parser: None,
        document_field: None,
        note: Some("no decoder; not in KNOWN_TOP_LEVEL_STREAM_NAMES".into()),
        stream_size: None,
    }
}

/// Phase 10b: decide whether `static_status` should be downgraded in
/// light of `doc`'s actual decoded state.
///
/// Rules:
///
/// - `FullyDecoded` stays `FullyDecoded` **only if** the corresponding
///   model field is populated (and non-empty where "empty" is
///   meaningful, e.g. a `VersionHistory` with zero records is a parser
///   silent-failure).
/// - `PartiallyDecoded` stays as is if the corresponding field is
///   populated; otherwise downgrades to `IdentifiedOnly`.
/// - Any other status is returned unchanged (the downgrade rule only
///   targets the static Full/Partial tier).
///
/// When a downgrade happens the note is replaced with a diagnostic
/// string pointing at the missing / empty field so users immediately
/// see the gap.
fn apply_dynamic_downgrade(
    name: &str,
    static_status: ParseCoverageStatus,
    static_note: Option<String>,
    doc: &PidDocument,
) -> (ParseCoverageStatus, Option<String>) {
    use ParseCoverageStatus::*;
    let populated = stream_is_populated(name, doc);
    match (static_status, populated) {
        (FullyDecoded, Some(true)) | (PartiallyDecoded, Some(true)) => (static_status, static_note),
        (FullyDecoded, Some(false)) => (
            IdentifiedOnly,
            Some(format!(
                "stream present but parser did not populate the expected \
                 `{}` field — downgraded from FullyDecoded",
                document_field_for_known_stream(name).unwrap_or("(field)")
            )),
        ),
        (PartiallyDecoded, Some(false)) => (
            IdentifiedOnly,
            Some(format!(
                "stream present but parser produced no `{}` decoding — \
                 downgraded from PartiallyDecoded",
                document_field_for_known_stream(name).unwrap_or("(field)")
            )),
        ),
        // Either the name has no dynamic probe wired up yet, or the
        // static tier was already at/below IdentifiedOnly. Pass through.
        _ => (static_status, static_note),
    }
}

/// Phase 10b probe: given a known top-level stream name, determine
/// whether the corresponding `PidDocument` field is populated to a
/// degree that backs up the static classification.
///
/// Returns:
///
/// - `Some(true)` — field populated (and non-empty where that matters)
/// - `Some(false)` — parser *should have* populated this but did not;
///   downgrade candidate
/// - `None` — no dynamic probe for this name yet (covered by static
///   classification only; Phase 10c will add more)
fn stream_is_populated(name: &str, doc: &PidDocument) -> Option<bool> {
    match name {
        "\u{5}SummaryInformation" | "\u{5}DocumentSummaryInformation" => {
            Some(doc.summary.is_some())
        }
        "PSMroots" => Some(
            doc.psm_roots
                .as_ref()
                .is_some_and(|r| !r.entries.is_empty()),
        ),
        "PSMclustertable" => Some(doc.psm_cluster_table.is_some()),
        "PSMsegmenttable" => Some(doc.psm_segment_table.is_some()),
        "DocVersion2" => Some(doc.doc_version2_decoded.is_some()),
        "DocVersion3" => Some(
            doc.version_history
                .as_ref()
                .is_some_and(|v| !v.records.is_empty()),
        ),
        "AppObject" => Some(doc.app_object_registry.is_some()),
        "JTaggedTxtStgList" => Some(doc.tagged_storages.is_some()),
        // Phase 10c: cluster & dynamic-attrs probes. These streams
        // feed into the shared `doc.clusters` vector (tagged by
        // `ClusterKind`) instead of individual `Option<...>` fields,
        // so the probe filters by kind.
        "PSMcluster0" => Some(
            doc.clusters
                .iter()
                .any(|c| matches!(c.kind, crate::model::ClusterKind::PsmCluster)),
        ),
        "StyleCluster" => Some(
            doc.clusters
                .iter()
                .any(|c| matches!(c.kind, crate::model::ClusterKind::StyleCluster)),
        ),
        "Dynamic Attributes Metadata" => Some(
            doc.clusters
                .iter()
                .any(|c| matches!(c.kind, crate::model::ClusterKind::DynamicAttributesMetadata)),
        ),
        // Unclustered Dynamic Attributes flows through two possible
        // surfaces: a dedicated `doc.dynamic_attributes` blob (when the
        // stream was parsed by the DAB reader) or a cluster entry with
        // `UnclusteredDynamicAttributes` kind. Either one populated
        // means the stream has been read.
        "Unclustered Dynamic Attributes" => Some(
            doc.dynamic_attributes.is_some()
                || doc.clusters.iter().any(|c| {
                    matches!(
                        c.kind,
                        crate::model::ClusterKind::UnclusteredDynamicAttributes
                    )
                }),
        ),
        _ => None,
    }
}

/// Mirror of [`known_stream_state`]'s `document_field` output, provided
/// separately so the downgrade path can quote it without allocating
/// a full tuple. Returns the raw canonical field name (no `"` quoting).
fn document_field_for_known_stream(name: &str) -> Option<&'static str> {
    match name {
        "\u{5}SummaryInformation" | "\u{5}DocumentSummaryInformation" => Some("summary"),
        "PSMroots" => Some("psm_roots"),
        "PSMclustertable" => Some("psm_cluster_table"),
        "PSMsegmenttable" => Some("psm_segment_table"),
        "DocVersion2" => Some("doc_version2_decoded"),
        "DocVersion3" => Some("version_history"),
        "AppObject" => Some("app_object_registry"),
        "JTaggedTxtStgList" => Some("tagged_storages"),
        // Phase 10c: cluster / dynamic-attrs. The field names reference
        // the `ClusterKind` discriminator used by the dynamic probe so
        // downgrade notes stay actionable.
        "PSMcluster0" => Some("clusters (kind=PsmCluster)"),
        "StyleCluster" => Some("clusters (kind=StyleCluster)"),
        "Dynamic Attributes Metadata" => Some("clusters (kind=DynamicAttributesMetadata)"),
        "Unclustered Dynamic Attributes" => {
            Some("clusters (kind=UnclusteredDynamicAttributes) / dynamic_attributes")
        }
        _ => None,
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
    use crate::model::{
        AppObjectRegistry, DocVersion2, PidDocument, PsmClusterTable, PsmRootEntry, PsmRoots,
        PsmSegmentTable, StreamEntry, SummaryInfo, TaggedTextStorageList, VersionHistory,
        VersionRecord,
    };
    use std::collections::BTreeMap;

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

    /// Phase 10b: attach populated model fields to `doc` for every
    /// known top-level stream so dynamic classification keeps the
    /// static `FullyDecoded` / `PartiallyDecoded` verdict. Used by
    /// tests that want to pin the **static** behavior without having
    /// the dynamic tier downgrade under them.
    fn populate_all_known_fields(doc: &mut PidDocument) {
        doc.summary = Some(SummaryInfo {
            creating_application: None,
            template: None,
            title: Some("T".into()),
            created_time: None,
            modified_time: None,
            raw: BTreeMap::new(),
            user_properties: BTreeMap::new(),
        });
        doc.psm_roots = Some(PsmRoots {
            size: 16,
            entries: vec![PsmRootEntry {
                id: 1,
                offset: 0,
                name: "root".into(),
            }],
            trailing_bytes: 0,
        });
        doc.psm_cluster_table = Some(PsmClusterTable {
            size: 0,
            count: 0,
            entries: vec![],
            trailing_bytes: 0,
        });
        doc.psm_segment_table = Some(PsmSegmentTable {
            size: 0,
            count: 0,
            flags: vec![],
            entries: vec![],
            trailing_bytes: 0,
        });
        doc.doc_version2_decoded = Some(DocVersion2 {
            magic_u32_le: 0x0001_0034,
            reserved_all_zero: true,
            records: vec![],
        });
        doc.version_history = Some(VersionHistory {
            size: 48,
            records: vec![VersionRecord {
                product: "TestProduct".into(),
                version: "0.0.1".into(),
                operation: "SA".into(),
                timestamp: "01/01/26 00:00".into(),
                offset: 0,
            }],
            record_size: 48,
            trailing_bytes: 0,
        });
        doc.app_object_registry = Some(AppObjectRegistry {
            size: 0,
            leading_u32: 0,
            entries: vec![],
            trailing_bytes: 0,
        });
        doc.tagged_storages = Some(TaggedTextStorageList {
            size: 0,
            list_name: "TaggedTxtStorages".into(),
            entries: vec![],
        });
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
        let mut doc = doc_with_paths(&[
            "/DocVersion3",
            "/PSMsegmenttable",
            "/AppObject",
            "/PSMclustertable",
        ]);
        populate_all_known_fields(&mut doc);
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
        let mut doc = doc_with_paths(&[
            "/PSMroots",      // FullyDecoded (with model)
            "/GhostStream",   // Unknown
            "/Sheet1/Nested", // Sheet storage
            "/Random42",      // Unknown
        ]);
        populate_all_known_fields(&mut doc);
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
        let mut doc = doc_with_paths(&[
            "/DocVersion3",     // Full
            "/AppObject",       // Full
            "/PSMsegmenttable", // Partial
            "/Sheet1/x",        // Identified
            "/Sheet2/y",        // Identified
            "/GhostStream",     // Unknown
        ]);
        populate_all_known_fields(&mut doc);
        let report = coverage_report(&doc);
        let [full, partial, ident, unk] = report.status_counts();
        assert_eq!((full, partial, ident, unk), (2, 1, 2, 1));
    }

    // ------------------------------------------------------------------
    // Phase 10b dynamic-classification tests
    // ------------------------------------------------------------------

    #[test]
    fn coverage_downgrades_docversion3_when_parser_did_not_populate() {
        // /DocVersion3 stream is present but doc.version_history is None.
        // That models the "parser silent-failure" case Phase 10b exists
        // to expose: the stream looks FullyDecoded by the static table
        // but in reality we have zero decoded records.
        let doc = doc_with_paths(&["/DocVersion3"]);
        let report = coverage_report(&doc);
        let entry = find(&report, "DocVersion3");
        assert_eq!(
            entry.status,
            ParseCoverageStatus::IdentifiedOnly,
            "empty model field must downgrade FullyDecoded -> IdentifiedOnly",
        );
        let note = entry.note.as_deref().unwrap_or("");
        assert!(
            note.contains("stream present") && note.contains("version_history"),
            "downgrade note should name the empty field; got: {note}",
        );
    }

    #[test]
    fn coverage_downgrades_psm_cluster_table_when_empty_model() {
        let doc = doc_with_paths(&["/PSMclustertable"]);
        let report = coverage_report(&doc);
        let entry = find(&report, "PSMclustertable");
        assert_eq!(
            entry.status,
            ParseCoverageStatus::IdentifiedOnly,
            "empty psm_cluster_table must downgrade PartiallyDecoded -> IdentifiedOnly",
        );
        let note = entry.note.as_deref().unwrap_or("");
        assert!(
            note.contains("psm_cluster_table"),
            "downgrade note should name the empty field; got: {note}",
        );
    }

    #[test]
    fn coverage_keeps_fully_decoded_when_model_populated() {
        let mut doc = doc_with_paths(&["/DocVersion3"]);
        doc.version_history = Some(VersionHistory {
            size: 48,
            records: vec![VersionRecord {
                product: "TestProduct".into(),
                version: "0.0.1".into(),
                operation: "SA".into(),
                timestamp: "01/01/26 00:00".into(),
                offset: 0,
            }],
            record_size: 48,
            trailing_bytes: 0,
        });
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "DocVersion3").status,
            ParseCoverageStatus::FullyDecoded,
            "stream + non-empty records must stay FullyDecoded",
        );
    }

    #[test]
    fn coverage_unknown_and_identified_unaffected_by_model_state() {
        // Phase 10b only downgrades Full/Partial. Unknown and
        // IdentifiedOnly are pass-through regardless of doc state.
        let doc = doc_with_paths(&["/GhostStream", "/Sheet1/x", "/PSMroots"]);
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "GhostStream").status,
            ParseCoverageStatus::Unknown,
        );
        assert_eq!(
            find(&report, "Sheet1").status,
            ParseCoverageStatus::IdentifiedOnly,
        );
        // PSMroots is a known stream; with no psm_roots field it SHOULD
        // get downgraded to IdentifiedOnly — assert that explicitly so
        // the test also guards the "downgrade applies to PSMroots" path.
        assert_eq!(
            find(&report, "PSMroots").status,
            ParseCoverageStatus::IdentifiedOnly,
        );
    }

    // ------------------------------------------------------------------
    // Phase 10c: cluster & dynamic-attrs dynamic probes.
    // ------------------------------------------------------------------

    use crate::model::{ClusterInfo, ClusterKind, DynamicAttributesBlob};

    /// Build a minimally-populated `ClusterInfo` with the requested
    /// `ClusterKind`. Every field other than `kind` and `name` is a
    /// harmless zero-ish default — the Phase 10c probe only looks at
    /// `kind`, so these placeholder values stay invisible to the
    /// coverage classifier.
    fn cluster_of(kind: ClusterKind, name: &str) -> ClusterInfo {
        ClusterInfo {
            name: name.into(),
            path: format!("/{name}"),
            size: 0,
            magic_u32_le: None,
            extracted_strings: vec![],
            kind,
            header: None,
            string_table: None,
            probe_info: None,
        }
    }

    #[test]
    fn coverage_downgrades_psm_cluster0_when_no_cluster_kind_psmcluster() {
        // Phase 10c: stream present, but no ClusterInfo with
        // `kind=PsmCluster` in doc.clusters — parser didn't claim it.
        let doc = doc_with_paths(&["/PSMcluster0"]);
        let report = coverage_report(&doc);
        let entry = find(&report, "PSMcluster0");
        assert_eq!(entry.status, ParseCoverageStatus::IdentifiedOnly);
        let note = entry.note.as_deref().unwrap_or("");
        assert!(
            note.contains("clusters (kind=PsmCluster)"),
            "downgrade note should name the cluster kind; got: {note}",
        );
    }

    #[test]
    fn coverage_keeps_psm_cluster0_partial_when_cluster_kind_psmcluster_populated() {
        let mut doc = doc_with_paths(&["/PSMcluster0"]);
        doc.clusters
            .push(cluster_of(ClusterKind::PsmCluster, "PSMcluster0"));
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "PSMcluster0").status,
            ParseCoverageStatus::PartiallyDecoded,
            "populated cluster keeps the static PartiallyDecoded verdict"
        );
    }

    #[test]
    fn coverage_downgrades_style_cluster_when_no_cluster_kind_style() {
        let doc = doc_with_paths(&["/StyleCluster"]);
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "StyleCluster").status,
            ParseCoverageStatus::IdentifiedOnly,
        );
    }

    #[test]
    fn coverage_downgrades_dynamic_attrs_metadata_when_no_cluster_kind_dam() {
        let doc = doc_with_paths(&["/Dynamic Attributes Metadata"]);
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "Dynamic Attributes Metadata").status,
            ParseCoverageStatus::IdentifiedOnly,
        );
    }

    #[test]
    fn coverage_keeps_unclustered_dynamic_attrs_when_blob_populated() {
        // UnclusteredDynamicAttributes has TWO surfaces: the blob field
        // or a cluster kind. Either one present should keep the static
        // PartiallyDecoded verdict.
        let mut doc = doc_with_paths(&["/Unclustered Dynamic Attributes"]);
        doc.dynamic_attributes = Some(DynamicAttributesBlob {
            path: "/Unclustered Dynamic Attributes".into(),
            size: 0,
            magic_u32_le: None,
            strings: vec![],
            relationships: vec![],
            class_names: vec![],
            raw_preview_hex: String::new(),
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            relationship_probes: vec![],
            record_trailers: vec![],
        });
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "Unclustered Dynamic Attributes").status,
            ParseCoverageStatus::PartiallyDecoded,
        );
    }

    #[test]
    fn coverage_keeps_unclustered_dynamic_attrs_when_cluster_kind_populated() {
        let mut doc = doc_with_paths(&["/Unclustered Dynamic Attributes"]);
        doc.clusters.push(cluster_of(
            ClusterKind::UnclusteredDynamicAttributes,
            "Unclustered Dynamic Attributes",
        ));
        let report = coverage_report(&doc);
        assert_eq!(
            find(&report, "Unclustered Dynamic Attributes").status,
            ParseCoverageStatus::PartiallyDecoded,
        );
    }

    // ------------------------------------------------------------------
    // Phase 10e: JSON helpers on CoverageReport.
    // ------------------------------------------------------------------

    #[test]
    fn coverage_report_json_round_trip_default() {
        let original = CoverageReport::default();
        let json = original.to_json().expect("to_json default");
        let restored = CoverageReport::from_json(&json).expect("from_json default");
        assert!(restored.entries.is_empty());
        assert_eq!(restored.status_counts(), [0, 0, 0, 0]);
    }

    #[test]
    fn coverage_report_json_round_trip_preserves_entries() {
        // Build a non-trivial report via the classifier, round-trip
        // through pretty-printed JSON, and assert the bucket counts
        // survive byte-for-byte.
        let mut doc = doc_with_paths(&[
            "/DocVersion3",
            "/Sheet1/x",
            "/GhostStream",
            "/PSMclustertable",
        ]);
        populate_all_known_fields(&mut doc);
        let report_before = coverage_report(&doc);
        let json = report_before.to_json_pretty().expect("to_json_pretty");
        let report_after = CoverageReport::from_json(&json).expect("from_json");
        assert_eq!(report_before.entries.len(), report_after.entries.len());
        assert_eq!(report_before.status_counts(), report_after.status_counts());
    }

    #[test]
    fn coverage_report_from_json_rejects_invalid_syntax_with_pid_error() {
        let err = CoverageReport::from_json("this is not json").expect_err("must reject");
        let msg = format!("{err}");
        assert!(
            msg.contains("coverage report JSON"),
            "expected PidError context 'coverage report JSON'; got: {msg}"
        );
    }

    // ------------------------------------------------------------------
    // Phase 10f: coverage bytes / stream_size aggregation.
    // ------------------------------------------------------------------

    fn doc_with_paths_and_sizes(paths_and_sizes: &[(&str, u64)]) -> PidDocument {
        PidDocument {
            streams: paths_and_sizes
                .iter()
                .map(|(p, sz)| StreamEntry {
                    path: (*p).to_string(),
                    size: *sz,
                    preview_ascii: vec![],
                    magic_u32_le: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn coverage_entry_carries_stream_size_for_single_top_level_stream() {
        let doc = doc_with_paths_and_sizes(&[("/DocVersion3", 48)]);
        let report = coverage_report(&doc);
        let entry = find(&report, "DocVersion3");
        assert_eq!(entry.stream_size, Some(48));
    }

    #[test]
    fn coverage_entry_aggregates_sizes_across_storage_children() {
        // `Sheet1` is a storage prefix; its `stream_size` must be the
        // sum of every `/Sheet1/*` member, not the size of any single
        // child alone.
        let doc = doc_with_paths_and_sizes(&[
            ("/Sheet1/Foo", 100),
            ("/Sheet1/Bar", 250),
            ("/Sheet1/Baz", 50),
        ]);
        let report = coverage_report(&doc);
        assert_eq!(find(&report, "Sheet1").stream_size, Some(400));
    }

    #[test]
    fn coverage_report_total_bytes_by_status_matches_entries() {
        let mut doc = doc_with_paths_and_sizes(&[
            ("/DocVersion3", 96),      // FullyDecoded (once model populated)
            ("/AppObject", 32),        // FullyDecoded
            ("/PSMsegmenttable", 220), // PartiallyDecoded
            ("/Sheet1/x", 500),        // IdentifiedOnly
            ("/Sheet2/y", 500),        // IdentifiedOnly
            ("/GhostStream", 18),      // Unknown
        ]);
        populate_all_known_fields(&mut doc);
        let report = coverage_report(&doc);
        let [full, partial, ident, unk] = report.total_bytes_by_status();
        assert_eq!(
            (full, partial, ident, unk),
            (128, 220, 1000, 18),
            "byte totals per status bucket",
        );
    }

    #[test]
    fn coverage_report_to_json_pretty_is_multiline_and_indented() {
        let mut doc = doc_with_paths(&["/DocVersion3"]);
        populate_all_known_fields(&mut doc);
        let pretty = coverage_report(&doc).to_json_pretty().expect("pretty");
        assert!(
            pretty.contains('\n'),
            "pretty output must be multi-line; got: {pretty}"
        );
        assert!(
            pretty.contains("  \""),
            "pretty output must be indented; got: {pretty}"
        );
    }
}
