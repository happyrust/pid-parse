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
    AttributeClassSummary, AttributeValue, ClusterCoverage, CrossReferenceGraph, EntryKind,
    PidDocument, RootPresence, StorageNode, SymbolUsage,
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
    let declared: Vec<String> = doc
        .psm_cluster_table
        .as_ref()
        .map(|t| t.entries.iter().map(|e| e.name.clone()).collect())
        .unwrap_or_default();

    let mut found_set: BTreeSet<String> = BTreeSet::new();
    for c in &doc.clusters {
        found_set.insert(c.name.clone());
    }
    for s in &doc.sheet_streams {
        found_set.insert(s.name.clone());
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

    ClusterCoverage {
        declared,
        found,
        matched,
        declared_missing,
        found_extra,
    }
}

fn build_symbol_usage(doc: &PidDocument) -> Vec<SymbolUsage> {
    let mut by_path: BTreeMap<String, (Option<String>, BTreeSet<String>)> = BTreeMap::new();

    for js in &doc.jsites {
        let Some(ref path) = js.symbol_path else {
            continue;
        };
        let entry = by_path
            .entry(path.clone())
            .or_insert_with(|| (js.symbol_name.clone(), BTreeSet::new()));
        if entry.0.is_none() {
            entry.0 = js.symbol_name.clone();
        }
        entry.1.insert(js.name.clone());
    }

    by_path
        .into_iter()
        .map(|(symbol_path, (symbol_name, names))| {
            let jsite_names: Vec<String> = names.into_iter().collect();
            let usage_count = jsite_names.len();
            SymbolUsage {
                symbol_path,
                symbol_name,
                jsite_names,
                usage_count,
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

        for field in &rec.attributes {
            bucket.names.insert(field.name.clone());
            if let AttributeValue::Text(v) = &field.value {
                if v.is_empty() {
                    continue;
                }
                match field.name.as_str() {
                    "DrawingID" | "DrawingNo" => {
                        bucket.drawing_ids.insert(v.clone());
                    }
                    "ModelID" => {
                        bucket.model_ids.insert(v.clone());
                    }
                    _ => {}
                }
            }
        }
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
                    },
                    PsmClusterEntry {
                        name: "MissingCluster".into(),
                        name_offset: 0,
                    },
                    PsmClusterEntry {
                        name: "Sheet6".into(),
                        name_offset: 0,
                    },
                ],
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

        let b = usage
            .iter()
            .find(|u| u.symbol_path.ends_with("B.sym"))
            .unwrap();
        assert_eq!(b.usage_count, 1);
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

        let pr = classes.iter().find(|c| c.class_name == "PipeRun").unwrap();
        assert!(pr.drawing_ids.is_empty(), "PipeRun has no DrawingID attrs");
        assert_eq!(
            pr.unique_attribute_names,
            vec!["Service".to_string(), "Size".to_string()]
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
