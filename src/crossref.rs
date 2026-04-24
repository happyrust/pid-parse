//! Cross-reference graph derivation.
//!
//! Takes an already-populated [`PidDocument`] and builds a small relational
//! view that stitches together the data pieces parsed by the stream handlers:
//!
//! * `ClusterCoverage` — PSM declarations vs. on-disk cluster/sheet streams.
//! * `SymbolUsage`     — `JSite` instances grouped by the symbol they reference.
//! * `AttributeClassSummary` — per-class aggregation of Dynamic Attributes.
//! * `RootPresence`    — whether each `PSMroots` name resolves in the CFB tree.
//!
//! Pure derivation: no I/O, no CFB access. Runs in memory on the decoded model.

use crate::model::{
    AttributeClassRecordRef, AttributeClassSummary, AttributeValue, ClusterCoverage,
    ClusterCoverageMatch, ClusterCoverageSourceKind, CrossReferenceGraph, DeclaredClusterRef,
    EndpointLinkCoverage, EntryKind, FoundClusterRef, ObjectSourceCoverage, ObjectSourceRef,
    PidDocument, ProvenanceChainBreak, ProvenanceChainCoverage, ProvenanceChainStage,
    RelationshipEndpointLink, RootPresence, SheetProvenanceCoverage, SheetProvenanceRef,
    StorageNode, SymbolReference, SymbolUsage,
};
use std::collections::{BTreeMap, BTreeSet};

/// Build the cross-reference graph from an already-decoded document.
pub fn build_graph(doc: &PidDocument) -> CrossReferenceGraph {
    let (relationship_endpoint_links, relationship_endpoint_coverage) =
        build_relationship_endpoint_links(doc);
    let (object_sources, object_source_coverage) = build_object_sources(doc);
    let mut graph = CrossReferenceGraph {
        cluster_coverage: build_cluster_coverage(doc),
        symbol_usage: build_symbol_usage(doc),
        attribute_classes: build_attribute_classes(doc),
        root_presence: build_root_presence(doc),
        relationship_endpoint_links,
        relationship_endpoint_coverage,
        object_sources,
        object_source_coverage,
        ..Default::default()
    };
    let (coverage, breaks) = build_provenance_chain(&graph);
    graph.provenance_chain_coverage = coverage;
    graph.provenance_chain_breaks = breaks;
    let (sheet_provenance, sheet_provenance_coverage) = build_sheet_provenance(doc, &graph);
    graph.sheet_provenance = sheet_provenance;
    graph.sheet_provenance_coverage = sheet_provenance_coverage;
    graph
}

fn build_cluster_coverage(doc: &PidDocument) -> ClusterCoverage {
    let declared_entries: Vec<DeclaredClusterRef> = doc
        .psm_cluster_table
        .as_ref()
        .map(|t| {
            t.entries
                .iter()
                .map(|e| DeclaredClusterRef {
                    name: e.name.clone(),
                    record_offset: e.record_offset,
                    name_offset: e.name_offset,
                    record_len: e.record_len,
                })
                .collect()
        })
        .unwrap_or_default();
    let declared: Vec<String> = declared_entries.iter().map(|e| e.name.clone()).collect();

    let mut found_entries = Vec::new();
    for c in &doc.clusters {
        found_entries.push(FoundClusterRef {
            name: c.name.clone(),
            source_kind: ClusterCoverageSourceKind::PsmCluster,
            path: c.path.clone(),
        });
    }
    for s in &doc.sheet_streams {
        found_entries.push(FoundClusterRef {
            name: s.name.clone(),
            source_kind: ClusterCoverageSourceKind::SheetStream,
            path: s.path.clone(),
        });
    }
    let mut found_set: BTreeSet<String> = BTreeSet::new();
    for entry in &found_entries {
        found_set.insert(entry.name.clone());
    }
    let found: Vec<String> = found_set.iter().cloned().collect();

    let declared_set: BTreeSet<&String> = declared.iter().collect();
    let matched: Vec<String> = declared
        .iter()
        .filter(|n| found_set.contains(*n))
        .cloned()
        .collect();
    let declared_missing: Vec<String> = declared
        .iter()
        .filter(|n| !found_set.contains(*n))
        .cloned()
        .collect();
    let found_extra: Vec<String> = found
        .iter()
        .filter(|n| !declared_set.contains(n))
        .cloned()
        .collect();
    let matches_detailed: Vec<ClusterCoverageMatch> = declared_entries
        .iter()
        .enumerate()
        .filter_map(|(declared_index, declared_entry)| {
            found_entries
                .iter()
                .position(|found_entry| found_entry.name == declared_entry.name)
                .map(|found_index| ClusterCoverageMatch {
                    name: declared_entry.name.clone(),
                    declared_index,
                    found_index,
                })
        })
        .collect();

    ClusterCoverage {
        declared,
        declared_entries,
        found,
        found_entries,
        matched,
        matches_detailed,
        declared_missing,
        found_extra,
    }
}

/// Accumulator bucket used while aggregating `JSite` entries into
/// `SymbolUsage`: `(symbol_name, unique_jsite_names, references)`.
type SymbolUsageBucket = (Option<String>, BTreeSet<String>, Vec<SymbolReference>);

fn build_symbol_usage(doc: &PidDocument) -> Vec<SymbolUsage> {
    let mut by_path: BTreeMap<String, SymbolUsageBucket> = BTreeMap::new();

    for js in &doc.jsites {
        let Some(ref path) = js.symbol_path else {
            continue;
        };
        let entry = by_path
            .entry(path.clone())
            .or_insert_with(|| (js.symbol_name.clone(), BTreeSet::new(), Vec::new()));
        if entry.0.is_none() {
            entry.0 = js.symbol_name.clone();
        }
        entry.1.insert(js.name.clone());
        entry.2.push(SymbolReference {
            jsite_name: js.name.clone(),
            jsite_path: js.path.clone(),
            local_symbol_path: js.local_symbol_path.clone(),
            has_ole_stream: js.has_ole_stream,
        });
    }

    by_path
        .into_iter()
        .map(|(symbol_path, (symbol_name, names, references))| {
            let jsite_names: Vec<String> = names.into_iter().collect();
            let usage_count = jsite_names.len();
            SymbolUsage {
                symbol_path,
                symbol_name,
                jsite_names,
                usage_count,
                references,
            }
        })
        .collect()
}

fn build_attribute_classes(doc: &PidDocument) -> Vec<AttributeClassSummary> {
    let Some(ref da) = doc.dynamic_attributes else {
        return Vec::new();
    };
    if da.attribute_records.is_empty() {
        return Vec::new();
    }

    let mut agg: BTreeMap<String, AttrAgg> = BTreeMap::new();

    for rec in &da.attribute_records {
        let bucket = agg.entry(rec.class_name.clone()).or_default();
        bucket.record_count += 1;
        let mut record_drawing_ids = BTreeSet::new();
        let mut record_model_ids = BTreeSet::new();

        for field in &rec.attributes {
            bucket.names.insert(field.name.clone());
            if let AttributeValue::Text(v) = &field.value {
                if v.is_empty() {
                    continue;
                }
                match field.name.as_str() {
                    "DrawingID" | "DrawingNo" => {
                        bucket.drawing_ids.insert(v.clone());
                        record_drawing_ids.insert(v.clone());
                    }
                    "ModelID" => {
                        bucket.model_ids.insert(v.clone());
                        record_model_ids.insert(v.clone());
                    }
                    _ => {}
                }
            }
        }
        bucket.records.push(AttributeClassRecordRef {
            class_name: rec.class_name.clone(),
            attribute_count: rec.attributes.len(),
            confidence: rec.confidence.clone(),
            drawing_ids: record_drawing_ids.into_iter().collect(),
            model_ids: record_model_ids.into_iter().collect(),
        });
    }

    agg.into_iter()
        .map(|(class_name, a)| {
            let mut model_ids: Vec<String> = a.model_ids.into_iter().collect();
            if model_ids.len() > 32 {
                model_ids.truncate(32);
            }
            AttributeClassSummary {
                class_name,
                record_count: a.record_count,
                drawing_ids: a.drawing_ids.into_iter().collect(),
                model_ids,
                unique_attribute_names: a.names.into_iter().collect(),
                records: a.records,
            }
        })
        .collect()
}

#[derive(Default)]
struct AttrAgg {
    record_count: usize,
    drawing_ids: BTreeSet<String>,
    model_ids: BTreeSet<String>,
    names: BTreeSet<String>,
    records: Vec<AttributeClassRecordRef>,
}

fn build_root_presence(doc: &PidDocument) -> Vec<RootPresence> {
    let Some(ref roots) = doc.psm_roots else {
        return Vec::new();
    };

    // Build a name -> (is_storage, is_stream) lookup from the top-level of
    // the CFB tree (one level below root). PSMroots names are announced at
    // root scope in every sampled .pid.
    let mut top_level: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    walk_one_level(&doc.cfb_tree, &mut top_level);

    roots
        .entries
        .iter()
        .map(|entry| {
            let (as_storage, as_stream) = top_level
                .get(entry.name.as_str())
                .copied()
                .unwrap_or((false, false));
            RootPresence {
                name: entry.name.clone(),
                id: entry.id,
                found_as_storage: as_storage,
                found_as_stream: as_stream,
            }
        })
        .collect()
}

fn walk_one_level<'a>(node: &'a StorageNode, out: &mut BTreeMap<&'a str, (bool, bool)>) {
    for child in &node.children {
        let is_storage = matches!(child.kind, EntryKind::Storage);
        let is_stream = matches!(child.kind, EntryKind::Stream);
        let slot = out.entry(child.name.as_str()).or_insert((false, false));
        slot.0 |= is_storage;
        slot.1 |= is_stream;
    }
}

/// Derive `(links, coverage)` that connect `ObjectGraph.relationships` to
/// their backing `SheetEndpointRecord` entries.
///
/// Matching is keyed by `PidRelationship.field_x == SheetEndpointRecord.rel_field_x`.
/// The sheet index is built once per `build_graph` invocation; if the same
/// `rel_field_x` appears in multiple sheet streams, the first encountered
/// record wins (source order across `doc.sheet_streams`, then
/// `endpoint_records`).
fn build_relationship_endpoint_links(
    doc: &PidDocument,
) -> (Vec<RelationshipEndpointLink>, EndpointLinkCoverage) {
    let Some(graph) = doc.object_graph.as_ref() else {
        return (Vec::new(), EndpointLinkCoverage::default());
    };

    let mut sheet_index: BTreeMap<u32, (&str, usize, u32, u32)> = BTreeMap::new();
    for sheet in &doc.sheet_streams {
        for record in &sheet.endpoint_records {
            sheet_index.entry(record.rel_field_x).or_insert((
                record.sheet_path.as_str(),
                record.offset,
                record.endpoint_a,
                record.endpoint_b,
            ));
        }
    }

    let mut links = Vec::with_capacity(graph.relationships.len());
    let mut coverage = EndpointLinkCoverage {
        total: graph.relationships.len(),
        ..Default::default()
    };

    for rel in &graph.relationships {
        let mut link = RelationshipEndpointLink {
            relationship_guid: rel.guid.clone(),
            relationship_record_id: rel.record_id,
            rel_field_x: rel.field_x,
            source_field_x: None,
            target_field_x: None,
            source_drawing_id: rel.source_drawing_id.clone(),
            target_drawing_id: rel.target_drawing_id.clone(),
            sheet_path: None,
            sheet_offset: None,
            missing_sheet_record: false,
        };

        match rel.field_x {
            None => {
                coverage.missing_field_x += 1;
            }
            Some(field_x) => {
                if let Some((sheet_path, offset, endpoint_a, endpoint_b)) =
                    sheet_index.get(&field_x).copied()
                {
                    link.sheet_path = Some(sheet_path.to_string());
                    link.sheet_offset = Some(offset);
                    link.source_field_x = Some(endpoint_a);
                    link.target_field_x = Some(endpoint_b);
                    coverage.linked += 1;
                } else {
                    link.missing_sheet_record = true;
                    coverage.missing_sheet_record += 1;
                }
            }
        }

        match (
            rel.source_drawing_id.is_some(),
            rel.target_drawing_id.is_some(),
        ) {
            (true, true) => coverage.fully_resolved += 1,
            (true, false) | (false, true) => coverage.partially_resolved += 1,
            (false, false) => {}
        }

        links.push(link);
    }

    (links, coverage)
}

/// Derive `(sources, coverage)` that link each `PidObject` to the
/// `AttributeRecord` inside `Unclustered Dynamic Attributes` carrying its
/// `DrawingID`. Produces exactly one [`ObjectSourceRef`] per object and
/// preserves `ObjectGraph.objects` source order; unlinked objects still
/// emit a ref with `missing_da_record = true` so downstream code can
/// index into the vector 1:1 without rebuilding the mapping.
///
/// Returns `(Vec::new(), ObjectSourceCoverage::default())` when
/// `object_graph` is absent. When only `dynamic_attributes` is missing
/// every object is flagged as `missing_da_record = true`.
///
/// The drawing-id index is built once by walking `attribute_records` in
/// order and keeping the first record that advertises a given drawing id
/// via an `AttributeField { name = "DrawingID" | "DrawingNo", Text(..) }`.
fn build_object_sources(doc: &PidDocument) -> (Vec<ObjectSourceRef>, ObjectSourceCoverage) {
    let Some(graph) = doc.object_graph.as_ref() else {
        return (Vec::new(), ObjectSourceCoverage::default());
    };
    let total_objects = graph.objects.len();

    let by_drawing_id: BTreeMap<&str, (usize, &str, &str)> = match doc.dynamic_attributes.as_ref() {
        Some(da) => {
            let mut idx: BTreeMap<&str, (usize, &str, &str)> = BTreeMap::new();
            for (i, rec) in da.attribute_records.iter().enumerate() {
                for field in &rec.attributes {
                    if !matches!(field.name.as_str(), "DrawingID" | "DrawingNo") {
                        continue;
                    }
                    let AttributeValue::Text(ref v) = field.value else {
                        continue;
                    };
                    if v.is_empty() {
                        continue;
                    }
                    idx.entry(v.as_str()).or_insert((
                        i,
                        rec.class_name.as_str(),
                        rec.confidence.as_str(),
                    ));
                }
            }
            idx
        }
        None => BTreeMap::new(),
    };

    let mut sources = Vec::with_capacity(total_objects);
    let mut coverage = ObjectSourceCoverage {
        total_objects,
        ..Default::default()
    };

    for obj in &graph.objects {
        let has_trailer_record_id = obj.record_id.is_some();
        match by_drawing_id.get(obj.drawing_id.as_str()).copied() {
            Some((record_index, class_name, confidence)) => {
                sources.push(ObjectSourceRef {
                    drawing_id: obj.drawing_id.clone(),
                    class_name: Some(class_name.to_string()),
                    attribute_record_index: Some(record_index),
                    confidence: Some(confidence.to_string()),
                    has_trailer_record_id,
                    missing_da_record: false,
                });
                coverage.linked += 1;
                if has_trailer_record_id {
                    coverage.with_trailer_record_id += 1;
                }
            }
            None => {
                sources.push(ObjectSourceRef {
                    drawing_id: obj.drawing_id.clone(),
                    class_name: None,
                    attribute_record_index: None,
                    confidence: None,
                    has_trailer_record_id,
                    missing_da_record: true,
                });
                coverage.missing_da_record += 1;
            }
        }
    }

    (sources, coverage)
}

/// Upper bound on the number of broken-chain samples retained for debug.
/// Keeps report output bounded without losing the "first few" evidence.
const PROVENANCE_CHAIN_BREAK_SAMPLE_CAP: usize = 10;

/// Phase 3 Step 3 — walk every [`RelationshipEndpointLink`] alongside the
/// already-populated [`ObjectSourceRef`] index and tally hop-by-hop
/// progress. Returns coverage counts plus up to
/// [`PROVENANCE_CHAIN_BREAK_SAMPLE_CAP`] break samples ordered by the
/// stage at which each chain first failed.
fn build_provenance_chain(
    graph: &CrossReferenceGraph,
) -> (ProvenanceChainCoverage, Vec<ProvenanceChainBreak>) {
    let mut coverage = ProvenanceChainCoverage {
        total_relationships: graph.relationship_endpoint_links.len(),
        ..Default::default()
    };
    let mut breaks: Vec<ProvenanceChainBreak> = Vec::new();

    if graph.relationship_endpoint_links.is_empty() {
        return (coverage, breaks);
    }

    let object_by_drawing_id: BTreeMap<&str, &ObjectSourceRef> = graph
        .object_sources
        .iter()
        .filter(|s| !s.missing_da_record)
        .map(|s| (s.drawing_id.as_str(), s))
        .collect();

    for link in &graph.relationship_endpoint_links {
        let mut failure: Option<(ProvenanceChainStage, String)> = None;

        if link.rel_field_x.is_some() {
            coverage.has_field_x += 1;
        } else {
            failure = Some((
                ProvenanceChainStage::MissingFieldX,
                "relationship trailer has no field_x; sheet lookup cannot start".into(),
            ));
        }

        let sheet_ok = if failure.is_none() {
            if link.sheet_path.is_some() {
                coverage.sheet_linked += 1;
                true
            } else {
                failure = Some((
                    ProvenanceChainStage::MissingSheetRecord,
                    format!(
                        "rel_field_x={:?} had no matching SheetEndpointRecord",
                        link.rel_field_x
                    ),
                ));
                false
            }
        } else {
            false
        };

        let source_ok = match link.source_drawing_id.as_deref() {
            Some(id) if object_by_drawing_id.contains_key(id) => {
                coverage.source_object_linked += 1;
                true
            }
            other => {
                if sheet_ok && failure.is_none() {
                    failure = Some((
                        ProvenanceChainStage::SourceObjectUnlinked,
                        match other {
                            Some(id) => format!("source drawing_id {id} not linked to DA record"),
                            None => "relationship has no source_drawing_id".into(),
                        },
                    ));
                }
                false
            }
        };

        let target_ok = match link.target_drawing_id.as_deref() {
            Some(id) if object_by_drawing_id.contains_key(id) => {
                coverage.target_object_linked += 1;
                true
            }
            other => {
                if sheet_ok && source_ok && failure.is_none() {
                    failure = Some((
                        ProvenanceChainStage::TargetObjectUnlinked,
                        match other {
                            Some(id) => format!("target drawing_id {id} not linked to DA record"),
                            None => "relationship has no target_drawing_id".into(),
                        },
                    ));
                }
                false
            }
        };

        if failure.is_none() && sheet_ok && source_ok && target_ok {
            coverage.fully_traced += 1;
        }

        if let Some((stage, reason)) = failure {
            if breaks.len() < PROVENANCE_CHAIN_BREAK_SAMPLE_CAP {
                breaks.push(ProvenanceChainBreak {
                    relationship_guid: link.relationship_guid.clone(),
                    stage,
                    reason,
                });
            }
        }
    }

    (coverage, breaks)
}

/// Phase 3 Step 4 — aggregate the already-built provenance signals per
/// `SheetStream`. Returns a ref per sheet (in source order) plus a
/// coverage summary. Does not re-walk the binary stream.
fn build_sheet_provenance(
    doc: &PidDocument,
    graph: &CrossReferenceGraph,
) -> (Vec<SheetProvenanceRef>, SheetProvenanceCoverage) {
    if doc.sheet_streams.is_empty() {
        return (Vec::new(), SheetProvenanceCoverage::default());
    }

    let mut sheet_match_by_path: BTreeMap<&str, usize> = BTreeMap::new();
    for m in &graph.cluster_coverage.matches_detailed {
        if let Some(found) = graph.cluster_coverage.found_entries.get(m.found_index) {
            if matches!(found.source_kind, ClusterCoverageSourceKind::SheetStream) {
                sheet_match_by_path.insert(found.path.as_str(), m.declared_index);
            }
        }
    }

    let linked_ids: BTreeSet<&str> = graph
        .object_sources
        .iter()
        .filter(|s| !s.missing_da_record)
        .map(|s| s.drawing_id.as_str())
        .collect();

    let mut links_by_sheet: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for link in &graph.relationship_endpoint_links {
        let Some(path) = link.sheet_path.as_deref() else {
            continue;
        };
        let entry = links_by_sheet.entry(path).or_insert((0, 0));
        entry.0 += 1;
        let src_ok = link
            .source_drawing_id
            .as_deref()
            .is_some_and(|id| linked_ids.contains(id));
        let tgt_ok = link
            .target_drawing_id
            .as_deref()
            .is_some_and(|id| linked_ids.contains(id));
        if src_ok && tgt_ok {
            entry.1 += 1;
        }
    }

    let mut refs = Vec::with_capacity(doc.sheet_streams.len());
    let mut coverage = SheetProvenanceCoverage {
        total_sheets: doc.sheet_streams.len(),
        ..Default::default()
    };

    for sheet in &doc.sheet_streams {
        let endpoint_record_count = sheet.endpoint_records.len();
        let matched_declared_index = sheet_match_by_path.get(sheet.path.as_str()).copied();
        let declared_in_psm = matched_declared_index.is_some();
        let (linked_relationship_count, fully_traced_relationship_count) = links_by_sheet
            .get(sheet.path.as_str())
            .copied()
            .unwrap_or((0, 0));

        if declared_in_psm {
            coverage.declared_sheets += 1;
        } else {
            coverage.orphan_sheets += 1;
        }
        if endpoint_record_count > 0 {
            coverage.sheets_with_endpoint_records += 1;
        }
        if declared_in_psm && endpoint_record_count == 0 {
            coverage.empty_declared_sheets += 1;
        }

        refs.push(SheetProvenanceRef {
            sheet_path: sheet.path.clone(),
            endpoint_record_count,
            declared_in_psm,
            matched_declared_index,
            linked_relationship_count,
            fully_traced_relationship_count,
        });
    }

    (refs, coverage)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::model::{
        AttributeField, AttributeRecord, ClusterInfo, ClusterKind, DynamicAttributesBlob,
        IndexedString, JProperties, JSite, ObjectGraph, PidDocument, PidObject, PidRelationship,
        PsmClusterEntry, PsmClusterTable, PsmRootEntry, PsmRoots, SheetEndpointRecord, SheetStream,
        StorageNode,
    };

    fn mk_storage(name: &str, kind: EntryKind) -> StorageNode {
        StorageNode {
            name: name.to_string(),
            path: format!("/{name}"),
            kind,
            children: vec![],
        }
    }

    fn mk_jsite(name: &str, symbol_path: Option<&str>, symbol_name: Option<&str>) -> JSite {
        JSite {
            name: name.to_string(),
            path: format!("/{name}"),
            symbol_name: symbol_name.map(String::from),
            symbol_path: symbol_path.map(String::from),
            local_symbol_path: None,
            has_ole_stream: false,
            ole_links: vec![],
            properties: JProperties::default(),
            raw_streams: vec![],
        }
    }

    fn text_attr(name: &str, value: &str) -> AttributeField {
        AttributeField {
            name: name.to_string(),
            value: AttributeValue::Text(value.to_string()),
            raw_value: None,
        }
    }

    fn mk_record(class_name: &str, attrs: Vec<AttributeField>) -> AttributeRecord {
        AttributeRecord {
            class_name: class_name.to_string(),
            attributes: attrs,
            confidence: "heuristic".to_string(),
        }
    }

    #[test]
    fn cluster_coverage_matches_declared_and_found() {
        let mut doc = PidDocument::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            doc.psm_cluster_table = Some(PsmClusterTable {
                size: 0,
                count: 3,
                entries: vec![
                    PsmClusterEntry {
                        name: "PSMcluster0".into(),
                        name_offset: 0,
                        record_offset: 0,
                        record_len: 0,
                        prefix_bytes: vec![],
                        probe: None,
                    },
                    PsmClusterEntry {
                        name: "MissingCluster".into(),
                        name_offset: 0,
                        record_offset: 0,
                        record_len: 0,
                        prefix_bytes: vec![],
                        probe: None,
                    },
                    PsmClusterEntry {
                        name: "Sheet6".into(),
                        name_offset: 0,
                        record_offset: 0,
                        record_len: 0,
                        prefix_bytes: vec![],
                        probe: None,
                    },
                ],
                trailing_bytes: 0,
            });
        }
        doc.clusters.push(ClusterInfo {
            name: "PSMcluster0".into(),
            path: "/PSMcluster0".into(),
            size: 0,
            magic_u32_le: None,
            extracted_strings: vec![],
            kind: ClusterKind::PsmCluster,
            header: None,
            string_table: None,
            probe_info: None,
        });
        doc.clusters.push(ClusterInfo {
            name: "UnexpectedCluster".into(),
            path: "/UnexpectedCluster".into(),
            size: 0,
            magic_u32_le: None,
            extracted_strings: vec![],
            kind: ClusterKind::Unknown,
            header: None,
            string_table: None,
            probe_info: None,
        });
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            endpoint_records: vec![],
        });

        let cov = build_cluster_coverage(&doc);
        assert_eq!(cov.matched, vec!["PSMcluster0", "Sheet6"]);
        assert_eq!(cov.declared_missing, vec!["MissingCluster"]);
        assert_eq!(cov.found_extra, vec!["UnexpectedCluster"]);
        assert_eq!(cov.declared_entries.len(), 3);
        assert_eq!(cov.found_entries.len(), 3);
        assert_eq!(cov.matches_detailed.len(), 2);
        assert_eq!(cov.declared_entries[0].name, "PSMcluster0");
        assert_eq!(
            cov.found_entries[0].source_kind,
            ClusterCoverageSourceKind::PsmCluster
        );
        assert_eq!(
            cov.found_entries[2].source_kind,
            ClusterCoverageSourceKind::SheetStream
        );
    }

    #[test]
    fn cluster_coverage_records_declared_entry_provenance() {
        let mut doc = PidDocument::default();
        doc.psm_cluster_table = Some(PsmClusterTable {
            size: 0,
            count: 1,
            entries: vec![PsmClusterEntry {
                name: "PSMcluster0".into(),
                name_offset: 0x14,
                record_offset: 0x08,
                record_len: 0x20,
                prefix_bytes: vec![0xAA, 0xBB],
                probe: None,
            }],
            trailing_bytes: 0,
        });

        let cov = build_cluster_coverage(&doc);
        assert_eq!(cov.declared_entries.len(), 1);
        let entry = &cov.declared_entries[0];
        assert_eq!(entry.name, "PSMcluster0");
        assert_eq!(entry.record_offset, 0x08);
        assert_eq!(entry.name_offset, 0x14);
        assert_eq!(entry.record_len, 0x20);
    }

    #[test]
    fn cluster_coverage_records_found_entry_provenance() {
        let mut doc = PidDocument::default();
        doc.psm_cluster_table = Some(PsmClusterTable {
            size: 0,
            count: 2,
            entries: vec![
                PsmClusterEntry {
                    name: "PSMcluster0".into(),
                    name_offset: 0,
                    record_offset: 0,
                    record_len: 0,
                    prefix_bytes: vec![],
                    probe: None,
                },
                PsmClusterEntry {
                    name: "Sheet6".into(),
                    name_offset: 0,
                    record_offset: 0,
                    record_len: 0,
                    prefix_bytes: vec![],
                    probe: None,
                },
            ],
            trailing_bytes: 0,
        });
        doc.clusters.push(ClusterInfo {
            name: "PSMcluster0".into(),
            path: "/PSMcluster0".into(),
            size: 0,
            magic_u32_le: None,
            extracted_strings: vec![],
            kind: ClusterKind::PsmCluster,
            header: None,
            string_table: None,
            probe_info: None,
        });
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            endpoint_records: vec![],
        });

        let cov = build_cluster_coverage(&doc);
        assert_eq!(cov.found_entries.len(), 2);
        assert_eq!(cov.matches_detailed.len(), 2);
        assert_eq!(cov.found_entries[0].path, "/PSMcluster0");
        assert_eq!(cov.found_entries[1].path, "/Sheet6");
        assert_eq!(
            cov.matches_detailed[0],
            ClusterCoverageMatch {
                name: "PSMcluster0".into(),
                declared_index: 0,
                found_index: 0,
            }
        );
    }

    #[test]
    fn symbol_usage_groups_jsites_by_path() {
        let mut doc = PidDocument::default();
        doc.jsites
            .push(mk_jsite("JSite0", Some("C:\\sym\\A.sym"), Some("A")));
        doc.jsites
            .push(mk_jsite("JSite1", Some("C:\\sym\\A.sym"), Some("A")));
        doc.jsites
            .push(mk_jsite("JSite2", Some("C:\\sym\\B.sym"), Some("B")));
        doc.jsites.push(mk_jsite("JSite3", None, None));

        let usage = build_symbol_usage(&doc);
        assert_eq!(usage.len(), 2, "two distinct symbols in use");

        let a = usage
            .iter()
            .find(|u| u.symbol_path.ends_with("A.sym"))
            .unwrap();
        assert_eq!(a.usage_count, 2);
        assert_eq!(
            a.jsite_names,
            vec!["JSite0".to_string(), "JSite1".to_string()]
        );
        assert_eq!(a.references.len(), 2);
        assert_eq!(a.references[0].jsite_name, "JSite0");
        assert_eq!(a.references[0].jsite_path, "/JSite0");

        let b = usage
            .iter()
            .find(|u| u.symbol_path.ends_with("B.sym"))
            .unwrap();
        assert_eq!(b.usage_count, 1);
    }

    #[test]
    fn symbol_usage_records_reference_provenance() {
        let mut doc = PidDocument::default();
        let mut js = mk_jsite("JSite0", Some("C:\\sym\\Valve.sym"), Some("Valve"));
        js.local_symbol_path = Some("D:\\cache\\Valve.sym".into());
        js.has_ole_stream = true;
        doc.jsites.push(js);

        let usage = build_symbol_usage(&doc);
        assert_eq!(usage.len(), 1);
        assert_eq!(
            usage[0].references[0],
            SymbolReference {
                jsite_name: "JSite0".into(),
                jsite_path: "/JSite0".into(),
                local_symbol_path: Some("D:\\cache\\Valve.sym".into()),
                has_ole_stream: true,
            }
        );
    }

    #[test]
    fn attribute_classes_summarise_records() {
        let mut doc = PidDocument::default();
        let records = vec![
            mk_record(
                "P&IDAttributes",
                vec![
                    text_attr("DrawingID", "D-001"),
                    text_attr("ModelID", "M-100"),
                    text_attr("ModelItemType", "PipeRun"),
                ],
            ),
            mk_record(
                "P&IDAttributes",
                vec![
                    text_attr("DrawingID", "D-001"), // dedup
                    text_attr("ModelID", "M-101"),
                    text_attr("Service", "Water"),
                ],
            ),
            mk_record(
                "PipeRun",
                vec![text_attr("Service", "Steam"), text_attr("Size", "6\"")],
            ),
        ];
        doc.dynamic_attributes = Some(DynamicAttributesBlob {
            path: "/Unclustered Dynamic Attributes".into(),
            size: 0,
            magic_u32_le: None,
            strings: vec![],
            relationships: vec![],
            class_names: vec![],
            raw_preview_hex: String::new(),
            header: None,
            attribute_records: records,
            probe_summary: None,
            relationship_probes: vec![],
            record_trailers: vec![],
        });

        let classes = build_attribute_classes(&doc);
        assert_eq!(classes.len(), 2);

        let pid = classes
            .iter()
            .find(|c| c.class_name == "P&IDAttributes")
            .unwrap();
        assert_eq!(pid.record_count, 2);
        assert_eq!(pid.drawing_ids, vec!["D-001".to_string()]);
        assert_eq!(
            pid.model_ids,
            vec!["M-100".to_string(), "M-101".to_string()]
        );
        assert!(pid.unique_attribute_names.iter().any(|n| n == "Service"));
        assert!(pid
            .unique_attribute_names
            .iter()
            .any(|n| n == "ModelItemType"));
        assert_eq!(pid.records.len(), 2);
        assert_eq!(pid.records[0].class_name, "P&IDAttributes");
        assert_eq!(pid.records[0].confidence, "heuristic");

        let pr = classes.iter().find(|c| c.class_name == "PipeRun").unwrap();
        assert!(pr.drawing_ids.is_empty(), "PipeRun has no DrawingID attrs");
        assert_eq!(
            pr.unique_attribute_names,
            vec!["Service".to_string(), "Size".to_string()]
        );
    }

    #[test]
    fn attribute_classes_record_provenance_per_record() {
        let mut doc = PidDocument::default();
        doc.dynamic_attributes = Some(DynamicAttributesBlob {
            path: "/Unclustered Dynamic Attributes".into(),
            size: 0,
            magic_u32_le: None,
            strings: vec![],
            relationships: vec![],
            class_names: vec![],
            raw_preview_hex: String::new(),
            header: None,
            attribute_records: vec![mk_record(
                "Instrument",
                vec![
                    text_attr("DrawingID", "DWG-01"),
                    text_attr("ModelID", "M-100"),
                    text_attr("Tag", "FIT-001"),
                ],
            )],
            probe_summary: None,
            relationship_probes: vec![],
            record_trailers: vec![],
        });

        let classes = build_attribute_classes(&doc);
        assert_eq!(classes.len(), 1);
        assert_eq!(
            classes[0].records[0],
            AttributeClassRecordRef {
                class_name: "Instrument".into(),
                attribute_count: 3,
                confidence: "heuristic".into(),
                drawing_ids: vec!["DWG-01".into()],
                model_ids: vec!["M-100".into()],
            }
        );
    }

    #[test]
    fn root_presence_marks_tree_entries() {
        let mut doc = PidDocument::default();
        doc.cfb_tree
            .children
            .push(mk_storage("JSite0", EntryKind::Storage));
        doc.cfb_tree
            .children
            .push(mk_storage("PSMcluster0", EntryKind::Stream));

        doc.psm_roots = Some(PsmRoots {
            size: 0,
            entries: vec![
                PsmRootEntry {
                    id: 0x18C,
                    offset: 0,
                    name: "JSite0".into(),
                },
                PsmRootEntry {
                    id: 0x001,
                    offset: 0,
                    name: "GhostEntry".into(),
                },
            ],
            trailing_bytes: 0,
        });

        let presence = build_root_presence(&doc);
        assert_eq!(presence.len(), 2);
        assert!(presence[0].found_as_storage);
        assert!(!presence[0].found_as_stream);
        assert!(!presence[1].found_as_storage);
        assert!(!presence[1].found_as_stream);
    }

    #[test]
    fn build_graph_wires_all_sections() {
        let doc = PidDocument::default();
        let g = build_graph(&doc);
        assert!(g.cluster_coverage.declared.is_empty());
        assert!(g.symbol_usage.is_empty());
        assert!(g.attribute_classes.is_empty());
        assert!(g.root_presence.is_empty());
    }

    #[test]
    fn cluster_coverage_no_psm_table_still_lists_found() {
        let mut doc = PidDocument::default();
        doc.clusters.push(ClusterInfo {
            name: "Orphan".into(),
            path: "/Orphan".into(),
            size: 0,
            magic_u32_le: None,
            extracted_strings: vec![],
            kind: ClusterKind::Unknown,
            header: None,
            string_table: None,
            probe_info: None,
        });
        let cov = build_cluster_coverage(&doc);
        assert!(cov.declared.is_empty());
        assert_eq!(cov.found, vec!["Orphan"]);
        assert_eq!(cov.found_extra, vec!["Orphan"]);

        // IndexedString import check — keep unused import guard
        let _probe = IndexedString {
            index: 0,
            value: String::new(),
        };
    }

    fn mk_relationship(
        guid: &str,
        field_x: Option<u32>,
        src: Option<&str>,
        tgt: Option<&str>,
    ) -> PidRelationship {
        PidRelationship {
            model_id: format!("Relationship.{guid}"),
            guid: guid.into(),
            record_id: Some(0x1000),
            field_x,
            source_drawing_id: src.map(str::to_string),
            target_drawing_id: tgt.map(str::to_string),
        }
    }

    fn mk_sheet_with_endpoint(
        name: &str,
        rel_field_x: u32,
        endpoint_a: u32,
        endpoint_b: u32,
        offset: usize,
    ) -> SheetStream {
        SheetStream {
            name: name.into(),
            path: format!("/{name}"),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            endpoint_records: vec![SheetEndpointRecord {
                sheet_path: format!("/{name}"),
                offset,
                rel_field_x,
                endpoint_a,
                endpoint_b,
            }],
        }
    }

    #[test]
    fn relationship_endpoint_links_record_sheet_provenance() {
        let mut doc = PidDocument::default();
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet6", 100, 42, 77, 0x1A0));
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet7", 200, 51, 88, 0x240));

        let mut graph = ObjectGraph::default();
        graph.relationships.push(mk_relationship(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            Some(100),
            Some("SRC1"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            Some(999),
            Some("SRC2"),
            None,
        ));
        graph.relationships.push(mk_relationship(
            "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
            None,
            None,
            None,
        ));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);

        assert_eq!(g.relationship_endpoint_links.len(), 3);

        let a = &g.relationship_endpoint_links[0];
        assert_eq!(a.relationship_guid, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert_eq!(a.rel_field_x, Some(100));
        assert_eq!(a.sheet_path.as_deref(), Some("/Sheet6"));
        assert_eq!(a.sheet_offset, Some(0x1A0));
        assert_eq!(a.source_field_x, Some(42));
        assert_eq!(a.target_field_x, Some(77));
        assert_eq!(a.source_drawing_id.as_deref(), Some("SRC1"));
        assert_eq!(a.target_drawing_id.as_deref(), Some("TGT1"));
        assert!(!a.missing_sheet_record);

        let b = &g.relationship_endpoint_links[1];
        assert_eq!(b.rel_field_x, Some(999));
        assert!(b.sheet_path.is_none());
        assert!(b.sheet_offset.is_none());
        assert!(b.missing_sheet_record);

        let c = &g.relationship_endpoint_links[2];
        assert_eq!(c.rel_field_x, None);
        assert!(c.sheet_path.is_none());
        assert!(
            !c.missing_sheet_record,
            "rel without field_x should not be flagged as missing sheet record"
        );

        let cov = &g.relationship_endpoint_coverage;
        assert_eq!(cov.total, 3);
        assert_eq!(cov.linked, 1);
        assert_eq!(cov.missing_sheet_record, 1);
        assert_eq!(cov.missing_field_x, 1);
        assert_eq!(cov.fully_resolved, 1);
        assert_eq!(cov.partially_resolved, 1);
    }

    #[test]
    fn relationship_endpoint_links_empty_without_object_graph() {
        let doc = PidDocument::default();
        let g = build_graph(&doc);
        assert!(g.relationship_endpoint_links.is_empty());
        assert_eq!(g.relationship_endpoint_coverage.total, 0);
        assert_eq!(g.relationship_endpoint_coverage.linked, 0);
    }

    #[test]
    fn relationship_endpoint_links_skip_field_x_without_sheet_records() {
        let mut doc = PidDocument::default();
        let mut graph = ObjectGraph::default();
        graph.relationships.push(mk_relationship(
            "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
            Some(5),
            None,
            None,
        ));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(g.relationship_endpoint_links.len(), 1);
        let d = &g.relationship_endpoint_links[0];
        assert_eq!(d.rel_field_x, Some(5));
        assert!(d.sheet_path.is_none());
        assert!(d.missing_sheet_record);

        let cov = &g.relationship_endpoint_coverage;
        assert_eq!(cov.total, 1);
        assert_eq!(cov.linked, 0);
        assert_eq!(cov.missing_sheet_record, 1);
        assert_eq!(cov.missing_field_x, 0);
        assert_eq!(cov.fully_resolved, 0);
        assert_eq!(cov.partially_resolved, 0);
    }

    fn mk_object(drawing_id: &str, item_type: &str, record_id: Option<u32>) -> PidObject {
        PidObject {
            drawing_id: drawing_id.into(),
            item_type: item_type.into(),
            drawing_item_type: None,
            model_id: None,
            extra: std::collections::BTreeMap::new(),
            record_id,
            field_x: None,
        }
    }

    fn mk_attribute_record(
        class_name: &str,
        drawing_id: Option<&str>,
        confidence: &str,
    ) -> AttributeRecord {
        let attributes = match drawing_id {
            Some(id) => vec![AttributeField {
                name: "DrawingID".into(),
                value: AttributeValue::Text(id.into()),
                raw_value: None,
            }],
            None => vec![],
        };
        AttributeRecord {
            class_name: class_name.into(),
            attributes,
            confidence: confidence.into(),
        }
    }

    fn mk_da_blob(records: Vec<AttributeRecord>) -> DynamicAttributesBlob {
        DynamicAttributesBlob {
            path: "/Unclustered Dynamic Attributes".into(),
            size: 0,
            magic_u32_le: None,
            strings: vec![],
            relationships: vec![],
            class_names: vec![],
            raw_preview_hex: String::new(),
            header: None,
            attribute_records: records,
            probe_summary: None,
            relationship_probes: vec![],
            record_trailers: vec![],
        }
    }

    #[test]
    fn object_sources_record_da_provenance() {
        let mut doc = PidDocument::default();
        doc.dynamic_attributes = Some(mk_da_blob(vec![
            mk_attribute_record("Instrument", Some("OBJ-1"), "decoded"),
            mk_attribute_record("Drawing", Some("OBJ-2"), "heuristic"),
            mk_attribute_record("Nozzle", None, "heuristic"),
        ]));

        let mut graph = ObjectGraph::default();
        graph
            .objects
            .push(mk_object("OBJ-1", "Instrument", Some(0x6009)));
        graph.objects.push(mk_object("OBJ-2", "Drawing", None));
        graph.objects.push(mk_object("OBJ-GHOST", "Symbol", None));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(
            g.object_sources.len(),
            3,
            "every object should produce exactly one source ref"
        );

        let first = &g.object_sources[0];
        assert_eq!(first.drawing_id, "OBJ-1");
        assert_eq!(first.class_name.as_deref(), Some("Instrument"));
        assert_eq!(first.attribute_record_index, Some(0));
        assert_eq!(first.confidence.as_deref(), Some("decoded"));
        assert!(first.has_trailer_record_id);
        assert!(!first.missing_da_record);

        let second = &g.object_sources[1];
        assert_eq!(second.drawing_id, "OBJ-2");
        assert_eq!(second.class_name.as_deref(), Some("Drawing"));
        assert_eq!(second.attribute_record_index, Some(1));
        assert_eq!(second.confidence.as_deref(), Some("heuristic"));
        assert!(
            !second.has_trailer_record_id,
            "object without record_id must not claim a trailer"
        );
        assert!(!second.missing_da_record);

        let ghost = &g.object_sources[2];
        assert_eq!(ghost.drawing_id, "OBJ-GHOST");
        assert!(ghost.class_name.is_none());
        assert!(ghost.attribute_record_index.is_none());
        assert!(ghost.confidence.is_none());
        assert!(!ghost.has_trailer_record_id);
        assert!(
            ghost.missing_da_record,
            "unlinked object must set missing_da_record"
        );

        let cov = &g.object_source_coverage;
        assert_eq!(cov.total_objects, 3);
        assert_eq!(cov.linked, 2);
        assert_eq!(cov.missing_da_record, 1);
        assert_eq!(cov.with_trailer_record_id, 1);
    }

    #[test]
    fn object_sources_first_da_record_wins_on_duplicate_drawing_id() {
        let mut doc = PidDocument::default();
        doc.dynamic_attributes = Some(mk_da_blob(vec![
            mk_attribute_record("Instrument", Some("OBJ-1"), "heuristic"),
            mk_attribute_record("Drawing", Some("OBJ-1"), "decoded"),
        ]));

        let mut graph = ObjectGraph::default();
        graph
            .objects
            .push(mk_object("OBJ-1", "Instrument", Some(0x1)));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(g.object_sources.len(), 1);
        assert_eq!(
            g.object_sources[0].class_name.as_deref(),
            Some("Instrument")
        );
        assert_eq!(g.object_sources[0].attribute_record_index, Some(0));
        assert_eq!(g.object_sources[0].confidence.as_deref(), Some("heuristic"));
    }

    #[test]
    fn object_sources_empty_without_object_graph() {
        let doc = PidDocument::default();
        let g = build_graph(&doc);
        assert!(g.object_sources.is_empty());
        assert_eq!(g.object_source_coverage.total_objects, 0);
        assert_eq!(g.object_source_coverage.linked, 0);
        assert_eq!(g.object_source_coverage.missing_da_record, 0);
        assert_eq!(g.object_source_coverage.with_trailer_record_id, 0);
    }

    #[test]
    fn object_sources_flag_missing_da_records_when_da_absent() {
        let mut doc = PidDocument::default();

        let mut graph = ObjectGraph::default();
        graph
            .objects
            .push(mk_object("OBJ-X", "Drawing", Some(0x10)));
        graph.objects.push(mk_object("OBJ-Y", "Nozzle", None));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(g.object_sources.len(), 2);
        assert!(g.object_sources.iter().all(|r| r.missing_da_record));
        let cov = &g.object_source_coverage;
        assert_eq!(cov.total_objects, 2);
        assert_eq!(cov.linked, 0);
        assert_eq!(cov.missing_da_record, 2);
        assert_eq!(cov.with_trailer_record_id, 0);
    }

    #[test]
    fn object_sources_also_match_drawing_no_alias() {
        let mut doc = PidDocument::default();
        let rec = AttributeRecord {
            class_name: "Drawing".into(),
            attributes: vec![
                AttributeField {
                    name: "DrawingNo".into(),
                    value: AttributeValue::Text("OBJ-AL".into()),
                    raw_value: None,
                },
                AttributeField {
                    name: "Tag".into(),
                    value: AttributeValue::Text("unused".into()),
                    raw_value: None,
                },
            ],
            confidence: "decoded".into(),
        };
        doc.dynamic_attributes = Some(mk_da_blob(vec![rec]));

        let mut graph = ObjectGraph::default();
        graph
            .objects
            .push(mk_object("OBJ-AL", "Drawing", Some(0x77)));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(g.object_sources.len(), 1);
        assert_eq!(g.object_sources[0].attribute_record_index, Some(0));
        assert_eq!(g.object_sources[0].class_name.as_deref(), Some("Drawing"));
        assert_eq!(g.object_source_coverage.linked, 1);
        assert_eq!(g.object_source_coverage.with_trailer_record_id, 1);
    }

    #[test]
    fn provenance_chain_coverage_counts_each_hop() {
        let mut doc = PidDocument::default();
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet6", 100, 42, 77, 0x100));
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet6", 200, 51, 99, 0x200));
        doc.dynamic_attributes = Some(mk_da_blob(vec![
            mk_attribute_record("Instrument", Some("SRC1"), "decoded"),
            mk_attribute_record("Drawing", Some("TGT1"), "decoded"),
            mk_attribute_record("Drawing", Some("SRC2"), "decoded"),
        ]));

        let mut graph = ObjectGraph::default();
        graph.objects.push(mk_object("SRC1", "Instrument", Some(1)));
        graph.objects.push(mk_object("TGT1", "Drawing", Some(2)));
        graph.objects.push(mk_object("SRC2", "Drawing", Some(3)));
        graph.relationships.push(mk_relationship(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            Some(100),
            Some("SRC1"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            Some(200),
            Some("SRC2"),
            Some("TGT-GHOST"),
        ));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        let cov = &g.provenance_chain_coverage;
        assert_eq!(cov.total_relationships, 2);
        assert_eq!(cov.has_field_x, 2);
        assert_eq!(cov.sheet_linked, 2);
        assert_eq!(cov.source_object_linked, 2);
        assert_eq!(cov.target_object_linked, 1);
        assert_eq!(cov.fully_traced, 1);
    }

    #[test]
    fn provenance_chain_breaks_point_at_first_failed_hop() {
        let mut doc = PidDocument::default();
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet6", 100, 42, 77, 0x100));
        doc.dynamic_attributes = Some(mk_da_blob(vec![mk_attribute_record(
            "Instrument",
            Some("SRC1"),
            "decoded",
        )]));

        let mut graph = ObjectGraph::default();
        graph.objects.push(mk_object("SRC1", "Instrument", Some(1)));
        graph.relationships.push(mk_relationship(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            None,
            Some("SRC1"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            Some(999),
            Some("SRC1"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
            Some(100),
            Some("SRC-GHOST"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
            Some(100),
            Some("SRC1"),
            Some("TGT-GHOST"),
        ));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        let breaks = &g.provenance_chain_breaks;
        assert_eq!(breaks.len(), 4);
        assert_eq!(breaks[0].stage, ProvenanceChainStage::MissingFieldX);
        assert_eq!(breaks[1].stage, ProvenanceChainStage::MissingSheetRecord);
        assert_eq!(breaks[2].stage, ProvenanceChainStage::SourceObjectUnlinked);
        assert_eq!(breaks[3].stage, ProvenanceChainStage::TargetObjectUnlinked);
        assert!(breaks[0].reason.contains("field_x"));
        assert!(breaks[1].reason.contains("SheetEndpointRecord"));
        assert!(breaks[2].reason.contains("SRC-GHOST"));
        assert!(breaks[3].reason.contains("TGT-GHOST"));
        assert_eq!(g.provenance_chain_coverage.fully_traced, 0);
    }

    #[test]
    fn provenance_chain_empty_without_object_graph() {
        let doc = PidDocument::default();
        let g = build_graph(&doc);
        let cov = &g.provenance_chain_coverage;
        assert_eq!(cov.total_relationships, 0);
        assert_eq!(cov.fully_traced, 0);
        assert!(g.provenance_chain_breaks.is_empty());
    }

    #[test]
    fn provenance_chain_sample_cap_holds() {
        let mut doc = PidDocument::default();
        let mut graph = ObjectGraph::default();
        for i in 0..(PROVENANCE_CHAIN_BREAK_SAMPLE_CAP + 5) {
            graph.relationships.push(mk_relationship(
                &format!("{i:032X}"),
                None,
                Some("UNLINKED"),
                Some("UNLINKED"),
            ));
        }
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(
            g.provenance_chain_coverage.total_relationships,
            PROVENANCE_CHAIN_BREAK_SAMPLE_CAP + 5
        );
        assert_eq!(
            g.provenance_chain_breaks.len(),
            PROVENANCE_CHAIN_BREAK_SAMPLE_CAP
        );
    }

    #[test]
    fn sheet_provenance_aggregates_endpoint_and_relationship_counts() {
        let mut doc = PidDocument::default();
        doc.psm_cluster_table = Some(PsmClusterTable {
            size: 0,
            count: 2,
            entries: vec![
                PsmClusterEntry {
                    name: "Sheet6".into(),
                    name_offset: 0,
                    record_offset: 0,
                    record_len: 0,
                    prefix_bytes: vec![],
                    probe: None,
                },
                PsmClusterEntry {
                    name: "Sheet7".into(),
                    name_offset: 0,
                    record_offset: 0,
                    record_len: 0,
                    prefix_bytes: vec![],
                    probe: None,
                },
            ],
            trailing_bytes: 0,
        });
        doc.sheet_streams
            .push(mk_sheet_with_endpoint("Sheet6", 100, 42, 77, 0x100));
        doc.sheet_streams.push(SheetStream {
            name: "Sheet7".into(),
            path: "/Sheet7".into(),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            endpoint_records: vec![],
        });
        doc.sheet_streams.push(SheetStream {
            name: "SheetOrphan".into(),
            path: "/SheetOrphan".into(),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            endpoint_records: vec![SheetEndpointRecord {
                sheet_path: "/SheetOrphan".into(),
                offset: 0x200,
                rel_field_x: 500,
                endpoint_a: 10,
                endpoint_b: 20,
            }],
        });
        doc.dynamic_attributes = Some(mk_da_blob(vec![
            mk_attribute_record("Instrument", Some("SRC1"), "decoded"),
            mk_attribute_record("Drawing", Some("TGT1"), "decoded"),
        ]));

        let mut graph = ObjectGraph::default();
        graph.objects.push(mk_object("SRC1", "Instrument", Some(1)));
        graph.objects.push(mk_object("TGT1", "Drawing", Some(2)));
        graph.relationships.push(mk_relationship(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            Some(100),
            Some("SRC1"),
            Some("TGT1"),
        ));
        graph.relationships.push(mk_relationship(
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            Some(500),
            Some("SRC-GHOST"),
            Some("TGT1"),
        ));
        doc.object_graph = Some(graph);

        let g = build_graph(&doc);
        assert_eq!(g.sheet_provenance.len(), 3);

        let sheet6 = g
            .sheet_provenance
            .iter()
            .find(|s| s.sheet_path == "/Sheet6")
            .expect("Sheet6 ref");
        assert_eq!(sheet6.endpoint_record_count, 1);
        assert!(sheet6.declared_in_psm);
        assert_eq!(sheet6.linked_relationship_count, 1);
        assert_eq!(sheet6.fully_traced_relationship_count, 1);

        let sheet7 = g
            .sheet_provenance
            .iter()
            .find(|s| s.sheet_path == "/Sheet7")
            .expect("Sheet7 ref");
        assert_eq!(sheet7.endpoint_record_count, 0);
        assert!(sheet7.declared_in_psm);
        assert_eq!(sheet7.linked_relationship_count, 0);

        let orphan = g
            .sheet_provenance
            .iter()
            .find(|s| s.sheet_path == "/SheetOrphan")
            .expect("orphan sheet ref");
        assert!(!orphan.declared_in_psm);
        assert_eq!(orphan.endpoint_record_count, 1);
        assert_eq!(orphan.linked_relationship_count, 1);
        assert_eq!(orphan.fully_traced_relationship_count, 0);

        let cov = &g.sheet_provenance_coverage;
        assert_eq!(cov.total_sheets, 3);
        assert_eq!(cov.declared_sheets, 2);
        assert_eq!(cov.orphan_sheets, 1);
        assert_eq!(cov.sheets_with_endpoint_records, 2);
        assert_eq!(cov.empty_declared_sheets, 1);
    }

    #[test]
    fn sheet_provenance_empty_without_sheet_streams() {
        let doc = PidDocument::default();
        let g = build_graph(&doc);
        assert!(g.sheet_provenance.is_empty());
        let cov = &g.sheet_provenance_coverage;
        assert_eq!(cov.total_sheets, 0);
        assert_eq!(cov.declared_sheets, 0);
        assert_eq!(cov.orphan_sheets, 0);
    }
}
