//! Cross-reference graph derivation.
//!
//! Takes an already-populated [`PidDocument`] and builds a small relational
//! view that stitches together the data pieces parsed by the stream handlers:
//!
//! * `ClusterCoverage` — PSM declarations vs. on-disk cluster/sheet streams.
//! * `SymbolUsage`     — JSite instances grouped by the symbol they reference.
//! * `AttributeClassSummary` — per-class aggregation of Dynamic Attributes.
//! * `RootPresence`    — whether each `PSMroots` name resolves in the CFB tree.
//!
//! Pure derivation: no I/O, no CFB access. Runs in memory on the decoded model.

use crate::model::{
    AttributeClassRecordRef, AttributeClassSummary, AttributeValue, ClusterCoverage, ClusterCoverageMatch,
    ClusterCoverageSourceKind, CrossReferenceGraph, DeclaredClusterRef, EntryKind, FoundClusterRef,
    PidDocument, RootPresence, StorageNode, SymbolReference, SymbolUsage,
};
use std::collections::{BTreeMap, BTreeSet};

/// Build the cross-reference graph from an already-decoded document.
pub fn build_graph(doc: &PidDocument) -> CrossReferenceGraph {
    CrossReferenceGraph {
        cluster_coverage: build_cluster_coverage(doc),
        symbol_usage: build_symbol_usage(doc),
        attribute_classes: build_attribute_classes(doc),
        root_presence: build_root_presence(doc),
    }
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

fn build_symbol_usage(doc: &PidDocument) -> Vec<SymbolUsage> {
    let mut by_path: BTreeMap<String, (Option<String>, BTreeSet<String>, Vec<SymbolReference>)> =
        BTreeMap::new();

    for js in &doc.jsites {
        let Some(ref path) = js.symbol_path else {
            continue;
        };
        let entry = by_path.entry(path.clone()).or_insert_with(|| {
            (js.symbol_name.clone(), BTreeSet::new(), Vec::new())
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AttributeField, AttributeRecord, ClusterInfo, ClusterKind, DynamicAttributesBlob,
        IndexedString, JProperties, JSite, PidDocument, PsmClusterEntry, PsmClusterTable,
        PsmRootEntry, PsmRoots, SheetStream, StorageNode,
    };

    fn mk_storage(name: &str, kind: EntryKind) -> StorageNode {
        StorageNode {
            name: name.to_string(),
            path: format!("/{}", name),
            kind,
            children: vec![],
        }
    }

    fn mk_jsite(name: &str, symbol_path: Option<&str>, symbol_name: Option<&str>) -> JSite {
        JSite {
            name: name.to_string(),
            path: format!("/{}", name),
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
                    },
                    PsmClusterEntry {
                        name: "MissingCluster".into(),
                        name_offset: 0,
                        record_offset: 0,
                        record_len: 0,
                        prefix_bytes: vec![],
                    },
                    PsmClusterEntry {
                        name: "Sheet6".into(),
                        name_offset: 0,
                        record_offset: 0,
                        record_len: 0,
                        prefix_bytes: vec![],
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
                },
                PsmClusterEntry {
                    name: "Sheet6".into(),
                    name_offset: 0,
                    record_offset: 0,
                    record_len: 0,
                    prefix_bytes: vec![],
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
}
