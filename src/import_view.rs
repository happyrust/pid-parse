//! UI-oriented projection of a decoded [`PidDocument`].
//!
//! Where [`crate::model`] keeps the faithful decoded structure and
//! [`crate::crossref`] derives exact cross-references, this module
//! flattens the pieces a consuming UI or report typically cares
//! about — titled objects, relationship summaries, symbol usage,
//! cluster coverage, and a bucket of unresolved entities — into the
//! compact [`PidImportView`] DTO.
//!
//! Build one with [`build_import_view`]. The input [`PidDocument`]
//! is **not** mutated, so the same document can be projected under
//! multiple view policies without reparsing. This module does no
//! I/O and is safe to call from any thread that already owns the
//! document.

use crate::model::{
    ClusterCoverage, ObjectGraph, PidDocument, PidObject, PidRelationship, RootPresence,
    SheetStream, SymbolUsage,
};
use std::collections::BTreeMap;

/// Compact UI-oriented snapshot of a [`PidDocument`], produced by
/// [`build_import_view`]. Immutable view intended for reports, pickers
/// and imports; richer or byte-level detail stays on
/// [`crate::model::PidDocument`] / [`crate::model::CrossReferenceGraph`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PidImportView {
    /// Display title sourced from `DrawingMeta.drawing_number`, then
    /// `SummaryInfo.title`, falling back to `"Smart P&ID Import"`.
    pub title: String,
    /// Owning project number sourced from
    /// `ObjectGraph.project_number`, else the raw `ProjectNumber` tag
    /// on [`crate::model::GeneralMeta`].
    pub project_number: Option<String>,
    /// Flat list of modelled objects, one per [`PidObject`].
    pub objects: Vec<PidVisualObject>,
    /// Flat list of relationships, one per [`PidRelationship`].
    pub relationships: Vec<PidVisualRelationship>,
    /// Distinct symbol usages — one entry per `symbol_path`.
    pub symbols: Vec<PidSymbolSummary>,
    /// Mixed cluster / sheet / coverage summary lines, in the order
    /// `build_cluster_summaries` emits them.
    pub clusters: Vec<PidClusterSummary>,
    /// Human-readable diagnostics for data the reader could not fully
    /// resolve (dangling relationship endpoints, missing roots, etc.).
    pub unresolved: Vec<String>,
}

/// Slim view of a single [`PidObject`] — keeps just the fields a UI
/// typically needs for pickers, tables and diffs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVisualObject {
    /// 32-character hex drawing-scoped identifier.
    pub drawing_id: String,
    /// `ModelItemType` verbatim from the source object
    /// (e.g. `"PipeRun"`).
    pub item_type: String,
    /// `DrawingItemType` when present on the source object
    /// (e.g. `"Symbol"`, `"LabelPersist"`).
    pub drawing_item_type: Option<String>,
    /// `ModelID` when present on the source object.
    pub model_id: Option<String>,
    /// All `extra` `BTreeMap` entries from the source object sorted
    /// by key — flattened into a stable-ordered `Vec` so the view
    /// diffs deterministically.
    pub extra: Vec<(String, String)>,
}

/// Slim view of a single [`PidRelationship`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVisualRelationship {
    /// 32-character hex GUID portion of `model_id`.
    pub guid: String,
    /// Full `"Relationship.<GUID>"` identifier verbatim.
    pub model_id: String,
    /// Source endpoint drawing id if the relationship resolved it;
    /// surfaced here so unresolved endpoints can be filtered out at
    /// the UI layer.
    pub source_drawing_id: Option<String>,
    /// Target endpoint drawing id if the relationship resolved it.
    pub target_drawing_id: Option<String>,
}

/// Reverse-index summary of how a symbol is used across `JSite`
/// instances — slim view of [`crate::model::SymbolUsage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidSymbolSummary {
    /// Symbol basename (e.g. `"GateValve"`) when the `JSite` exposed
    /// one; else `None`.
    pub symbol_name: Option<String>,
    /// Full symbol-library path (absolute, as observed on the
    /// writer's filesystem).
    pub symbol_path: String,
    /// Number of references to this symbol — equal to
    /// `jsite_names.len()`.
    pub usage_count: usize,
    /// `JSite` storage names that reference this symbol
    /// (deduplicated, source order).
    pub jsite_names: Vec<String>,
}

/// One-line cluster / sheet / coverage summary used inside
/// [`PidImportView::clusters`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidClusterSummary {
    /// Display name — matches [`crate::model::ClusterInfo::name`] or
    /// a synthetic marker like `"coverage.declared_missing"`.
    pub name: String,
    /// Short kind tag (e.g. `"PsmCluster"`, `"Sheet"`, `"Coverage"`)
    /// used to colour / bucket the row at the UI layer.
    pub kind: String,
    /// Record count contributed by the cluster / sheet (string-table
    /// rows, endpoint records, declared-missing list length, …).
    pub record_count: usize,
    /// Free-form note describing header metadata or coverage gap
    /// context — displayed as a secondary line under the row.
    pub note: String,
}

/// Build a [`PidImportView`] from a decoded [`PidDocument`]. Does not
/// mutate `doc`; safe to call repeatedly.
pub fn build_import_view(doc: &PidDocument) -> PidImportView {
    let object_graph = doc.object_graph.as_ref();
    let cross = doc.cross_reference.as_ref();

    let objects = object_graph
        .map(|graph| graph.objects.iter().map(visual_object_from).collect())
        .unwrap_or_default();
    let relationships = object_graph
        .map(|graph| {
            graph
                .relationships
                .iter()
                .map(visual_relationship_from)
                .collect()
        })
        .unwrap_or_default();
    let symbols = cross.map_or_else(
        || fallback_symbols(doc),
        |cross| cross.symbol_usage.iter().map(symbol_summary_from).collect(),
    );
    let clusters = build_cluster_summaries(doc, cross.map(|c| &c.cluster_coverage));
    let unresolved = build_unresolved(doc, object_graph, cross.map(|c| &c.root_presence));

    PidImportView {
        title: doc
            .drawing_meta
            .as_ref()
            .and_then(|m| m.drawing_number.clone())
            .or_else(|| doc.summary.as_ref().and_then(|s| s.title.clone()))
            .unwrap_or_else(|| "Smart P&ID Import".into()),
        project_number: object_graph
            .and_then(|g| g.project_number.clone())
            .or_else(|| {
                doc.general_meta
                    .as_ref()
                    .and_then(|g| g.tags.get("ProjectNumber").cloned())
            }),
        objects,
        relationships,
        symbols,
        clusters,
        unresolved,
    }
}

fn visual_object_from(object: &PidObject) -> PidVisualObject {
    let mut extra: Vec<(String, String)> = object
        .extra
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    extra.sort_by(|a, b| a.0.cmp(&b.0));

    PidVisualObject {
        drawing_id: object.drawing_id.clone(),
        item_type: object.item_type.clone(),
        drawing_item_type: object.drawing_item_type.clone(),
        model_id: object.model_id.clone(),
        extra,
    }
}

fn visual_relationship_from(relationship: &PidRelationship) -> PidVisualRelationship {
    PidVisualRelationship {
        guid: relationship.guid.clone(),
        model_id: relationship.model_id.clone(),
        source_drawing_id: relationship.source_drawing_id.clone(),
        target_drawing_id: relationship.target_drawing_id.clone(),
    }
}

fn symbol_summary_from(symbol: &SymbolUsage) -> PidSymbolSummary {
    PidSymbolSummary {
        symbol_name: symbol.symbol_name.clone(),
        symbol_path: symbol.symbol_path.clone(),
        usage_count: symbol.usage_count,
        jsite_names: symbol.jsite_names.clone(),
    }
}

fn fallback_symbols(doc: &PidDocument) -> Vec<PidSymbolSummary> {
    let mut grouped: BTreeMap<String, PidSymbolSummary> = BTreeMap::new();
    for site in &doc.jsites {
        let Some(path) = &site.symbol_path else {
            continue;
        };
        let entry = grouped
            .entry(path.clone())
            .or_insert_with(|| PidSymbolSummary {
                symbol_name: site.symbol_name.clone(),
                symbol_path: path.clone(),
                usage_count: 0,
                jsite_names: Vec::new(),
            });
        entry.usage_count += 1;
        entry.jsite_names.push(site.name.clone());
    }
    grouped.into_values().collect()
}

fn build_cluster_summaries(
    doc: &PidDocument,
    coverage: Option<&ClusterCoverage>,
) -> Vec<PidClusterSummary> {
    let mut out = Vec::new();
    for cluster in &doc.clusters {
        out.push(PidClusterSummary {
            name: cluster.name.clone(),
            kind: format!("{:?}", cluster.kind),
            record_count: cluster
                .string_table
                .as_ref()
                .map_or(cluster.extracted_strings.len(), std::vec::Vec::len),
            note: cluster.header.as_ref().map_or_else(
                || "header=none".into(),
                |h| format!("type=0x{:04X} flags=0x{:04X}", h.stream_type, h.flags),
            ),
        });
    }
    for sheet in &doc.sheet_streams {
        out.push(sheet_summary_from(sheet));
    }
    if let Some(coverage) = coverage {
        if !coverage.declared_missing.is_empty() {
            out.push(PidClusterSummary {
                name: "coverage.declared_missing".into(),
                kind: "Coverage".into(),
                record_count: coverage.declared_missing.len(),
                note: coverage.declared_missing.join(", "),
            });
        }
    }
    out
}

fn sheet_summary_from(sheet: &SheetStream) -> PidClusterSummary {
    PidClusterSummary {
        name: sheet.name.clone(),
        kind: "Sheet".into(),
        record_count: sheet
            .endpoint_records
            .len()
            .max(sheet.attribute_records.len()),
        note: sheet.header.as_ref().map_or_else(
            || format!("endpoints={}", sheet.endpoint_records.len()),
            |h| {
                format!(
                    "type=0x{:04X} endpoints={}",
                    h.stream_type,
                    sheet.endpoint_records.len()
                )
            },
        ),
    }
}

fn build_unresolved(
    doc: &PidDocument,
    object_graph: Option<&ObjectGraph>,
    roots: Option<&Vec<RootPresence>>,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(graph) = object_graph {
        for relationship in &graph.relationships {
            if relationship.source_drawing_id.is_none() || relationship.target_drawing_id.is_none()
            {
                out.push(format!(
                    "relationship {} unresolved: {:?} -> {:?}",
                    relationship.guid,
                    relationship.source_drawing_id,
                    relationship.target_drawing_id
                ));
            }
        }
    }
    if let Some(roots) = roots {
        for root in roots {
            if !root.found_as_storage && !root.found_as_stream {
                out.push(format!("missing root {} (0x{:08X})", root.name, root.id));
            }
        }
    }
    if doc.object_graph.is_none() {
        out.push("object graph unavailable".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CrossReferenceGraph, PidDocument};

    #[test]
    fn build_import_view_collects_objects_symbols_and_unresolved() {
        let doc = PidDocument {
            object_graph: Some(ObjectGraph {
                drawing_no: Some("D-100".into()),
                project_number: Some("P-01".into()),
                objects: vec![PidObject {
                    drawing_id: "OBJ-1".into(),
                    item_type: "Instrument".into(),
                    drawing_item_type: Some("Symbol".into()),
                    model_id: Some("MID-1".into()),
                    extra: BTreeMap::from([("Tag".into(), "FIT-001".into())]),
                    record_id: None,
                    field_x: None,
                }],
                relationships: vec![PidRelationship {
                    model_id: "Relationship.R1".into(),
                    guid: "R1".into(),
                    record_id: None,
                    field_x: None,
                    source_drawing_id: Some("OBJ-1".into()),
                    target_drawing_id: None,
                }],
                by_drawing_id: BTreeMap::new(),
                counts_by_type: BTreeMap::new(),
            }),
            cross_reference: Some(CrossReferenceGraph {
                cluster_coverage: ClusterCoverage::default(),
                symbol_usage: vec![SymbolUsage {
                    symbol_path: r"\\srv\sym\Valve.sym".into(),
                    symbol_name: Some("Valve".into()),
                    jsite_names: vec!["JSite0".into()],
                    usage_count: 1,
                    references: vec![],
                }],
                attribute_classes: vec![],
                root_presence: vec![RootPresence {
                    name: "MissingRoot".into(),
                    id: 0x10,
                    found_as_storage: false,
                    found_as_stream: false,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let view = build_import_view(&doc);
        assert_eq!(view.project_number.as_deref(), Some("P-01"));
        assert_eq!(view.objects.len(), 1);
        assert_eq!(view.symbols.len(), 1);
        assert_eq!(view.unresolved.len(), 2);
    }
}
