//! Mermaid diagram exporters for `ObjectGraph` and `CrossReferenceGraph`.
//!
//! Two entry points:
//!
//! * [`object_graph_mermaid`] — renders the P&ID object graph (objects +
//!   resolved relationships). Each node is colored by `item_type` and
//!   displays its `DrawingID` (truncated) + `ModelItemType`. Relationships are
//!   directed edges; unresolved endpoints are drawn to an explicit
//!   `off_drawing` pseudo-node so the picture stays lossless.
//! * [`crossref_mermaid`] — renders the cross-reference graph from
//!   `doc.cross_reference`: PSM-declared clusters vs. actual clusters,
//!   symbol → `JSite` fan-out, and `PSMroots` → CFB-tree resolution state.
//!
//! Both functions are **pure** and can be tested with a synthesized
//! `PidDocument`. They intentionally avoid any external mermaid crate —
//! we emit plain text because mermaid syntax is small and we want zero
//! extra dependencies.

use crate::model::{
    CrossReferenceGraph, ObjectGraph, PidDocument, PidObject, PidRelationship, RootPresence,
    SymbolUsage,
};
use std::collections::BTreeMap;
use std::fmt::Write;

// ────────────────────────────────────────────────────────────────────────────
// Object graph → Mermaid
// ────────────────────────────────────────────────────────────────────────────

/// Configuration for object-graph rendering.
#[derive(Debug, Clone)]
pub struct ObjectGraphOptions {
    /// Upper bound on nodes to render. Extra nodes are replaced with a
    /// single `... (N more)` placeholder to keep mermaid parseable for
    /// large drawings. `usize::MAX` disables the cap.
    pub max_nodes: usize,
    /// Upper bound on relationship edges to render.
    pub max_edges: usize,
    /// If true, skip template relationships (those without a concrete
    /// GUID). These are template records and rarely useful in diagrams.
    pub skip_template_relationships: bool,
}

impl Default for ObjectGraphOptions {
    fn default() -> Self {
        Self {
            max_nodes: 200,
            max_edges: 500,
            skip_template_relationships: true,
        }
    }
}

/// Render `doc.object_graph` as a mermaid `graph LR` diagram.
/// Returns an empty string when no graph is present.
pub fn object_graph_mermaid(doc: &PidDocument) -> String {
    object_graph_mermaid_with(doc, &ObjectGraphOptions::default())
}

/// Like [`object_graph_mermaid`] but with explicit options.
pub fn object_graph_mermaid_with(doc: &PidDocument, opts: &ObjectGraphOptions) -> String {
    let Some(graph) = doc.object_graph.as_ref() else {
        return String::new();
    };

    let mut out = String::new();
    writeln!(&mut out, "graph LR").ok();
    write_graph_header(&mut out, graph);

    let rendered_objects = render_object_nodes(&mut out, graph, opts);
    let rendered_edges = render_relationship_edges(&mut out, graph, &rendered_objects, opts);

    // Legend subgraph — small, always present, uses already-declared classes.
    writeln!(&mut out, "  subgraph LEGEND[\"Legend\"]").ok();
    writeln!(&mut out, "    LEG_REL[\"Relationship\"]:::rel").ok();
    writeln!(&mut out, "    LEG_OBJ[\"Object\"]:::obj").ok();
    writeln!(&mut out, "    LEG_OFF[\"off-drawing\"]:::offdrawing").ok();
    writeln!(&mut out, "  end").ok();

    write_graph_classes(&mut out);
    let _ = rendered_edges; // count reported via caller if needed

    out
}

fn write_graph_header(out: &mut String, graph: &ObjectGraph) {
    let counts: Vec<String> = graph
        .counts_by_type
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    if !counts.is_empty() {
        writeln!(out, "  %% objects: {}", counts.join(", ")).ok();
    }
    if let Some(ref d) = graph.drawing_no {
        writeln!(out, "  %% drawing_no: {d}").ok();
    }
    if let Some(ref p) = graph.project_number {
        writeln!(out, "  %% project: {p}").ok();
    }
}

/// Render every object as a mermaid node. Returns the set of `drawing_ids`
/// that were actually rendered so edges can skip dangling endpoints.
fn render_object_nodes(
    out: &mut String,
    graph: &ObjectGraph,
    opts: &ObjectGraphOptions,
) -> std::collections::BTreeSet<String> {
    let mut rendered = std::collections::BTreeSet::new();
    let total = graph.objects.len();
    let take = total.min(opts.max_nodes);
    for obj in graph.objects.iter().take(take) {
        writeln!(
            out,
            "  {}[\"{}\"]:::obj",
            node_id_for_drawing(&obj.drawing_id),
            node_label_for_object(obj)
        )
        .ok();
        rendered.insert(obj.drawing_id.clone());
    }
    if total > take {
        writeln!(
            out,
            "  obj_overflow[\"... ({} more objects omitted)\"]:::overflow",
            total - take
        )
        .ok();
    }
    rendered
}

fn render_relationship_edges(
    out: &mut String,
    graph: &ObjectGraph,
    rendered_objects: &std::collections::BTreeSet<String>,
    opts: &ObjectGraphOptions,
) -> usize {
    let mut off_drawing_needed = false;
    let mut edges_drawn = 0usize;
    let mut rel_idx = 0usize;
    for rel in &graph.relationships {
        if edges_drawn >= opts.max_edges {
            break;
        }
        if opts.skip_template_relationships && rel.guid.is_empty() {
            continue;
        }
        let Some((src, tgt, src_is_off, tgt_is_off)) = endpoint_nodes(rel, rendered_objects) else {
            continue;
        };
        if src_is_off || tgt_is_off {
            off_drawing_needed = true;
        }
        let label = rel_label(rel, rel_idx);
        rel_idx += 1;
        writeln!(out, "  {src} -->|\"{label}\"| {tgt}").ok();
        edges_drawn += 1;
    }
    if off_drawing_needed {
        writeln!(out, "  off_drawing[\"(off-drawing)\"]:::offdrawing").ok();
    }
    edges_drawn
}

fn endpoint_nodes(
    rel: &PidRelationship,
    rendered_objects: &std::collections::BTreeSet<String>,
) -> Option<(String, String, bool, bool)> {
    let (src, src_off) = match rel.source_drawing_id.as_deref() {
        Some(d) if rendered_objects.contains(d) => (node_id_for_drawing(d), false),
        Some(_) | None => ("off_drawing".to_string(), true),
    };
    let (tgt, tgt_off) = match rel.target_drawing_id.as_deref() {
        Some(d) if rendered_objects.contains(d) => (node_id_for_drawing(d), false),
        Some(_) | None => ("off_drawing".to_string(), true),
    };
    if src_off && tgt_off {
        // Both endpoints unresolved — no useful edge to draw.
        return None;
    }
    Some((src, tgt, src_off, tgt_off))
}

fn rel_label(rel: &PidRelationship, fallback_idx: usize) -> String {
    if !rel.guid.is_empty() {
        format!("rel#{}", &rel.guid[..rel.guid.len().min(6)])
    } else {
        format!("rel#{fallback_idx}")
    }
}

fn node_id_for_drawing(drawing_id: &str) -> String {
    // Mermaid node ids must be alphanumeric or underscore. DrawingIDs are
    // already 32 hex chars, so we prefix + truncate.
    let short: String = drawing_id.chars().take(12).collect();
    format!("o_{short}")
}

fn node_label_for_object(obj: &PidObject) -> String {
    let short_id: String = obj.drawing_id.chars().take(8).collect();
    // Mermaid renders `<br/>` as a line break inside `"..."` labels.
    format!(
        "{}<br/>{}",
        escape_mermaid(&obj.item_type),
        escape_mermaid(&short_id)
    )
}

fn write_graph_classes(out: &mut String) {
    writeln!(
        out,
        "  classDef obj fill:#e0f2ff,stroke:#0b7285,color:#0b3b5a;"
    )
    .ok();
    writeln!(
        out,
        "  classDef rel fill:#fff4e6,stroke:#b85c00,color:#5a2a00;"
    )
    .ok();
    writeln!(
        out,
        "  classDef offdrawing fill:#f3f4f6,stroke:#6b7280,color:#1f2937,stroke-dasharray: 4 2;"
    )
    .ok();
    writeln!(
        out,
        "  classDef overflow fill:#fef3c7,stroke:#92400e,color:#78350f;"
    )
    .ok();
}

// ────────────────────────────────────────────────────────────────────────────
// Cross-reference graph → Mermaid
// ────────────────────────────────────────────────────────────────────────────

/// Knobs that cap how large a cross-reference mermaid diagram grows
/// before it becomes unreadable. Passed to [`crossref_mermaid_with`];
/// [`Default`] values match the shipped [`crossref_mermaid`] preset.
#[derive(Debug, Clone)]
pub struct CrossRefOptions {
    /// Maximum number of symbols to render.
    pub max_symbols: usize,
    /// Maximum number of `JSite` fan-out edges per symbol.
    pub max_jsites_per_symbol: usize,
}

impl Default for CrossRefOptions {
    fn default() -> Self {
        Self {
            max_symbols: 20,
            max_jsites_per_symbol: 6,
        }
    }
}

/// Render `doc.cross_reference` as a mermaid diagram.
/// Produces a top-to-bottom layout with four subgraphs (one per sub-view).
/// Returns an empty string when no cross-reference data is available.
pub fn crossref_mermaid(doc: &PidDocument) -> String {
    crossref_mermaid_with(doc, &CrossRefOptions::default())
}

/// [`crossref_mermaid`] with caller-supplied caps. Returns an empty
/// string when `doc.cross_reference` is absent.
pub fn crossref_mermaid_with(doc: &PidDocument, opts: &CrossRefOptions) -> String {
    let Some(xr) = doc.cross_reference.as_ref() else {
        return String::new();
    };

    let mut out = String::new();
    writeln!(&mut out, "graph TB").ok();

    write_cluster_coverage_subgraph(&mut out, xr);
    write_symbol_usage_subgraph(&mut out, &xr.symbol_usage, opts);
    write_attribute_classes_subgraph(&mut out, xr);
    write_root_presence_subgraph(&mut out, &xr.root_presence);

    writeln!(&mut out, "  classDef declared fill:#e0f2ff,stroke:#0b7285;").ok();
    writeln!(
        &mut out,
        "  classDef missing fill:#ffe3e3,stroke:#c92a2a,color:#862e2e;"
    )
    .ok();
    writeln!(&mut out, "  classDef extra fill:#fff4e6,stroke:#b85c00;").ok();
    writeln!(&mut out, "  classDef symbol fill:#edfff2,stroke:#2b8a3e;").ok();
    writeln!(&mut out, "  classDef jsite fill:#f0f4ff,stroke:#3b5bdb;").ok();
    writeln!(&mut out, "  classDef cls fill:#fff5f5,stroke:#6a5d0a;").ok();
    writeln!(&mut out, "  classDef root fill:#f3f4f6,stroke:#4b5563;").ok();

    out
}

fn write_cluster_coverage_subgraph(out: &mut String, xr: &CrossReferenceGraph) {
    let cov = &xr.cluster_coverage;
    writeln!(out, "  subgraph CC[\"Cluster Coverage\"]").ok();
    for name in &cov.matched {
        writeln!(
            out,
            "    cc_m_{}[\"{}\"]:::declared",
            sanitize(name),
            escape_mermaid(name)
        )
        .ok();
    }
    for name in &cov.declared_missing {
        writeln!(
            out,
            "    cc_miss_{}[\"{} (missing)\"]:::missing",
            sanitize(name),
            escape_mermaid(name)
        )
        .ok();
    }
    for name in &cov.found_extra {
        writeln!(
            out,
            "    cc_extra_{}[\"{} (undeclared)\"]:::extra",
            sanitize(name),
            escape_mermaid(name)
        )
        .ok();
    }
    writeln!(out, "  end").ok();
}

fn write_symbol_usage_subgraph(out: &mut String, usages: &[SymbolUsage], opts: &CrossRefOptions) {
    if usages.is_empty() {
        return;
    }
    writeln!(
        out,
        "  subgraph SU[\"Symbol Usage (top {})\"]",
        opts.max_symbols
    )
    .ok();
    for (i, u) in usages.iter().take(opts.max_symbols).enumerate() {
        let sym_node = format!("sym_{i}");
        let label = u
            .symbol_name
            .clone()
            .unwrap_or_else(|| basename_of(&u.symbol_path));
        writeln!(
            out,
            "    {}[\"{} x{}\"]:::symbol",
            sym_node,
            escape_mermaid(&label),
            u.usage_count
        )
        .ok();
        for (j, js) in u
            .jsite_names
            .iter()
            .take(opts.max_jsites_per_symbol)
            .enumerate()
        {
            let js_node = format!("js_{i}_{j}");
            writeln!(out, "    {}[\"{}\"]:::jsite", js_node, escape_mermaid(js)).ok();
            writeln!(out, "    {sym_node} --> {js_node}").ok();
        }
        if u.jsite_names.len() > opts.max_jsites_per_symbol {
            let extra = u.jsite_names.len() - opts.max_jsites_per_symbol;
            let ov = format!("js_{i}_ov");
            writeln!(out, "    {ov}[\"... ({extra} more)\"]:::jsite").ok();
            writeln!(out, "    {sym_node} --> {ov}").ok();
        }
    }
    if usages.len() > opts.max_symbols {
        writeln!(
            out,
            "    sym_overflow[\"... ({} more symbols omitted)\"]:::symbol",
            usages.len() - opts.max_symbols
        )
        .ok();
    }
    writeln!(out, "  end").ok();
}

fn write_attribute_classes_subgraph(out: &mut String, xr: &CrossReferenceGraph) {
    if xr.attribute_classes.is_empty() {
        return;
    }
    writeln!(out, "  subgraph AC[\"Attribute Classes\"]").ok();
    for c in &xr.attribute_classes {
        writeln!(
            out,
            "    cls_{}[\"{}<br/>records={}, attrs={}\"]:::cls",
            sanitize(&c.class_name),
            escape_mermaid(&c.class_name),
            c.record_count,
            c.unique_attribute_names.len()
        )
        .ok();
    }
    writeln!(out, "  end").ok();
}

fn write_root_presence_subgraph(out: &mut String, roots: &[RootPresence]) {
    if roots.is_empty() {
        return;
    }
    writeln!(out, "  subgraph RP[\"PSMroots → CFB Tree\"]").ok();
    for r in roots {
        let state = if r.found_as_storage {
            "STORAGE"
        } else if r.found_as_stream {
            "STREAM"
        } else {
            "MISSING"
        };
        let class_ = if state == "MISSING" {
            "missing"
        } else {
            "root"
        };
        writeln!(
            out,
            "    rp_{}[\"{}<br/>id=0x{:08X} {}\"]:::{}",
            sanitize(&r.name),
            escape_mermaid(&r.name),
            r.id,
            state,
            class_
        )
        .ok();
    }
    writeln!(out, "  end").ok();
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Sanitize an arbitrary string into a mermaid-safe node-id fragment.
/// Keeps ASCII letters/digits/underscore, replaces every other char with `_`.
fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    // Mermaid ids cannot start with a digit in some dialects — prefix.
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'x');
    }
    out
}

/// Minimal escaping for label bodies. Mermaid labels accept most characters
/// as long as `"` and backslashes are escaped. We also avoid literal `|`
/// which would interfere with edge-label syntax.
fn escape_mermaid(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('|', "¦")
}

fn basename_of(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

// Silence unused-import warning from BTreeMap in `crossref_mermaid_with`
// when certain branches are empty; referenced by signature only.
#[allow(dead_code)]
fn _dummy_btree(_m: &BTreeMap<String, usize>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AttributeClassSummary, ClusterCoverage, CrossReferenceGraph, ObjectGraph, PidDocument,
        PidObject, PidRelationship, RootPresence, SymbolUsage,
    };
    use std::collections::BTreeMap;

    fn obj(did: &str, ty: &str) -> PidObject {
        PidObject {
            drawing_id: did.to_string(),
            item_type: ty.to_string(),
            drawing_item_type: None,
            model_id: None,
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        }
    }

    fn rel(guid: &str, src: Option<&str>, tgt: Option<&str>) -> PidRelationship {
        PidRelationship {
            model_id: format!("Relationship.{guid}"),
            guid: guid.to_string(),
            record_id: None,
            field_x: None,
            source_drawing_id: src.map(String::from),
            target_drawing_id: tgt.map(String::from),
        }
    }

    #[test]
    fn object_graph_empty_doc_returns_empty_string() {
        let doc = PidDocument::default();
        assert!(object_graph_mermaid(&doc).is_empty());
    }

    #[test]
    fn object_graph_emits_nodes_and_edges() {
        let mut doc = PidDocument::default();
        let mut g = ObjectGraph::default();
        g.objects
            .push(obj("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "PipeRun"));
        g.objects
            .push(obj("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB", "Nozzle"));
        g.relationships.push(rel(
            "DEADBEEF",
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"),
        ));
        doc.object_graph = Some(g);

        let s = object_graph_mermaid(&doc);
        assert!(s.contains("graph LR"));
        assert!(s.contains("PipeRun"));
        assert!(s.contains("Nozzle"));
        assert!(s.contains("-->|\"rel#DEADBE\"|"));
        assert!(s.contains("o_AAAAAAAAAAAA"));
        assert!(s.contains("o_BBBBBBBBBBBB"));
        assert!(s.contains("classDef obj"));
    }

    #[test]
    fn object_graph_off_drawing_endpoint_creates_placeholder() {
        let mut doc = PidDocument::default();
        let mut g = ObjectGraph::default();
        g.objects
            .push(obj("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "PipeRun"));
        g.relationships.push(rel(
            "F00DF00D",
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            None, // off-drawing target
        ));
        doc.object_graph = Some(g);
        let s = object_graph_mermaid(&doc);
        assert!(
            s.contains("off_drawing[\"(off-drawing)\"]"),
            "off-drawing placeholder missing from:\n{s}"
        );
        assert!(s.contains("--> off_drawing") || s.contains("|\"rel#F00DF0\"| off_drawing"));
    }

    #[test]
    fn object_graph_skips_template_relationships_by_default() {
        let mut doc = PidDocument::default();
        let mut g = ObjectGraph::default();
        g.objects
            .push(obj("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "PipeRun"));
        g.relationships.push(rel(
            "",
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ));
        doc.object_graph = Some(g);
        let s = object_graph_mermaid(&doc);
        assert!(
            !s.contains("rel#0"),
            "template relationship should be filtered out:\n{s}"
        );
    }

    #[test]
    fn crossref_empty_returns_empty() {
        let doc = PidDocument::default();
        assert!(crossref_mermaid(&doc).is_empty());
    }

    #[test]
    fn crossref_emits_all_four_subgraphs() {
        let mut doc = PidDocument::default();
        let mut xr = CrossReferenceGraph {
            cluster_coverage: ClusterCoverage {
                declared: vec!["A".into(), "Missing".into()],
                declared_entries: vec![],
                found: vec!["A".into(), "Extra".into()],
                found_entries: vec![],
                matched: vec!["A".into()],
                matches_detailed: vec![],
                declared_missing: vec!["Missing".into()],
                found_extra: vec!["Extra".into()],
            },
            ..CrossReferenceGraph::default()
        };
        xr.symbol_usage.push(SymbolUsage {
            symbol_path: "C:\\sym\\Valve.sym".into(),
            symbol_name: Some("Valve".into()),
            jsite_names: vec!["JSite0".into(), "JSite1".into()],
            usage_count: 2,
            references: vec![],
        });
        xr.attribute_classes.push(AttributeClassSummary {
            class_name: "P&IDAttributes".into(),
            record_count: 42,
            drawing_ids: vec![],
            model_ids: vec![],
            unique_attribute_names: vec!["DrawingID".into(), "ModelID".into()],
            records: vec![],
        });
        xr.root_presence.push(RootPresence {
            name: "DocStore".into(),
            id: 1,
            found_as_storage: true,
            found_as_stream: false,
        });
        xr.root_presence.push(RootPresence {
            name: "Ghost".into(),
            id: 2,
            found_as_storage: false,
            found_as_stream: false,
        });
        doc.cross_reference = Some(xr);

        let s = crossref_mermaid(&doc);
        assert!(s.contains("graph TB"));
        assert!(s.contains("Cluster Coverage"));
        assert!(s.contains("Symbol Usage"));
        assert!(s.contains("Attribute Classes"));
        assert!(s.contains("PSMroots"));
        // matched vs missing vs extra coloring classes are declared
        assert!(s.contains(":::declared"));
        assert!(s.contains(":::missing"));
        assert!(s.contains(":::extra"));
        // Symbol fan-out contains both JSite children
        assert!(s.contains("JSite0"));
        assert!(s.contains("JSite1"));
        // Root presence labels include STORAGE / MISSING
        assert!(s.contains("STORAGE"));
        assert!(s.contains("MISSING"));
    }

    #[test]
    fn sanitize_normalizes_unicode_and_specials() {
        assert_eq!(sanitize("ABC_123"), "ABC_123");
        assert_eq!(sanitize("P&IDAttributes"), "P_IDAttributes");
        assert_eq!(sanitize("123foo"), "x123foo");
        assert_eq!(sanitize(""), "_");
    }

    #[test]
    fn escape_mermaid_handles_pipes_and_quotes() {
        let e = escape_mermaid("a|b\"c\\d");
        assert_eq!(e, "a¦b\\\"c\\\\d");
    }

    #[test]
    fn object_graph_max_nodes_produces_overflow_marker() {
        let mut doc = PidDocument::default();
        let mut g = ObjectGraph::default();
        for i in 0..5 {
            g.objects.push(obj(
                &format!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA{i:1X}"),
                "Pipe",
            ));
        }
        doc.object_graph = Some(g);
        let opts = ObjectGraphOptions {
            max_nodes: 2,
            max_edges: 10,
            skip_template_relationships: false,
        };
        let s = object_graph_mermaid_with(&doc, &opts);
        assert!(s.contains("3 more objects omitted"));
    }
}
