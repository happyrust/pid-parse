//! Renders the two mermaid diagram flavors against a small synthesized
//! `PidDocument` — no `.pid` sample needed.
//!
//! Run:
//! ```bash
//! cargo run --example mermaid_demo
//! ```
//!
//! The output is pure mermaid text; pipe it into a `.mmd` file and open in
//! Mermaid Live Editor (https://mermaid.live), Obsidian, or VS Code.

use pid_parse::{
    inspect::mermaid::{crossref_mermaid, object_graph_mermaid},
    model::{
        AttributeClassSummary, ClusterCoverage, CrossReferenceGraph, ObjectGraph, PidDocument,
        PidObject, PidRelationship, RootPresence, SymbolUsage,
    },
};
use std::collections::BTreeMap;

fn synth_document() -> PidDocument {
    let mut doc = PidDocument::default();

    // -- ObjectGraph -----------------------------------------------------
    let mut g = ObjectGraph {
        drawing_no: Some("DWG-0201GP06-01".into()),
        project_number: Some("SQLPlant1401".into()),
        ..ObjectGraph::default()
    };

    // A tiny connected component: Valve ──connects──▶ PipeRun ──connects──▶ Nozzle
    g.objects.push(PidObject {
        drawing_id: "A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1".into(),
        item_type: "Valve".into(),
        drawing_item_type: Some("Symbol".into()),
        model_id: Some("V-001".into()),
        extra: BTreeMap::new(),
        record_id: Some(10),
        field_x: Some(0x40),
    });
    g.objects.push(PidObject {
        drawing_id: "B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2".into(),
        item_type: "PipeRun".into(),
        drawing_item_type: None,
        model_id: Some("PR-12".into()),
        extra: BTreeMap::new(),
        record_id: Some(11),
        field_x: Some(0x44),
    });
    g.objects.push(PidObject {
        drawing_id: "C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3".into(),
        item_type: "Nozzle".into(),
        drawing_item_type: Some("Symbol".into()),
        model_id: Some("N-200".into()),
        extra: BTreeMap::new(),
        record_id: Some(12),
        field_x: Some(0x4A),
    });
    g.relationships.push(PidRelationship {
        model_id: "Relationship.BEEFF00D00000000000000000000BEEF".into(),
        guid: "BEEFF00D00000000000000000000BEEF".into(),
        record_id: Some(50),
        field_x: Some(0x100),
        source_drawing_id: Some("A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1".into()),
        target_drawing_id: Some("B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2".into()),
    });
    g.relationships.push(PidRelationship {
        model_id: "Relationship.FEEDFACE00000000000000000FEEDFACE".into(),
        guid: "FEEDFACE00000000000000000FEEDFACE".into(),
        record_id: Some(51),
        field_x: Some(0x102),
        source_drawing_id: Some("B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2B2".into()),
        target_drawing_id: Some("C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3".into()),
    });
    // One off-drawing relationship (only target resolved) — demonstrates
    // the dashed `off-drawing` placeholder.
    g.relationships.push(PidRelationship {
        model_id: "Relationship.DEAD0000000000000000000000000000".into(),
        guid: "DEAD0000000000000000000000000000".into(),
        record_id: Some(52),
        field_x: Some(0x104),
        source_drawing_id: None,
        target_drawing_id: Some("C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3C3".into()),
    });

    g.counts_by_type.insert("Valve".into(), 1);
    g.counts_by_type.insert("PipeRun".into(), 1);
    g.counts_by_type.insert("Nozzle".into(), 1);
    g.counts_by_type.insert("Relationship".into(), 3);
    doc.object_graph = Some(g);

    // -- CrossReferenceGraph --------------------------------------------
    let mut xr = CrossReferenceGraph {
        cluster_coverage: ClusterCoverage {
            declared: vec![
                "PSMcluster0".into(),
                "StyleCluster".into(),
                "Sheet6".into(),
                "Dynamic Attributes Metadata".into(),
                "Unclustered Dynamic Attributes".into(),
            ],
            found: vec![
                "PSMcluster0".into(),
                "StyleCluster".into(),
                "Sheet6".into(),
                "Dynamic Attributes Metadata".into(),
                "Unclustered Dynamic Attributes".into(),
            ],
            matched: vec![
                "PSMcluster0".into(),
                "StyleCluster".into(),
                "Sheet6".into(),
                "Dynamic Attributes Metadata".into(),
                "Unclustered Dynamic Attributes".into(),
            ],
            declared_missing: vec![],
            found_extra: vec![],
        },
        ..CrossReferenceGraph::default()
    };
    xr.symbol_usage.push(SymbolUsage {
        symbol_path: "\\\\server\\share\\Symbols\\Valve.sym".into(),
        symbol_name: Some("Valve".into()),
        jsite_names: vec!["JSite0".into(), "JSite4".into(), "JSite7".into()],
        usage_count: 3,
    });
    xr.symbol_usage.push(SymbolUsage {
        symbol_path: "\\\\server\\share\\Symbols\\Nozzle.sym".into(),
        symbol_name: Some("Nozzle".into()),
        jsite_names: vec!["JSite2".into()],
        usage_count: 1,
    });
    xr.attribute_classes.push(AttributeClassSummary {
        class_name: "P&IDAttributes".into(),
        record_count: 140,
        drawing_ids: vec!["DWG-0201GP06-01".into()],
        model_ids: vec!["V-001".into(), "PR-12".into(), "N-200".into()],
        unique_attribute_names: vec!["DrawingID".into(), "ModelID".into(), "ModelItemType".into()],
    });
    xr.attribute_classes.push(AttributeClassSummary {
        class_name: "PipeRun".into(),
        record_count: 53,
        drawing_ids: vec![],
        model_ids: vec![],
        unique_attribute_names: vec!["Service".into(), "Size".into()],
    });
    xr.root_presence.push(RootPresence {
        name: "Imagineer Document".into(),
        id: 0x018C,
        found_as_storage: true,
        found_as_stream: false,
    });
    xr.root_presence.push(RootPresence {
        name: "DocStore".into(),
        id: 0x0001,
        found_as_storage: true,
        found_as_stream: false,
    });
    xr.root_presence.push(RootPresence {
        name: "_SupportOnlyList".into(),
        id: 0x0019,
        found_as_storage: false,
        found_as_stream: false,
    });
    doc.cross_reference = Some(xr);

    doc
}

fn main() {
    let doc = synth_document();

    println!("%% =====================================================");
    println!("%% Object Graph (synthesized demo)");
    println!("%% =====================================================");
    print!("{}", object_graph_mermaid(&doc));
    println!();

    println!("%% =====================================================");
    println!("%% Cross Reference Graph (synthesized demo)");
    println!("%% =====================================================");
    print!("{}", crossref_mermaid(&doc));
}
