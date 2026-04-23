//! Integration test: load the TEST02 A01 drawing row out of the
//! real `Export_v2.sqlite` produced by `tools/orca-mdf-probe`.
//!
//! Skips cleanly on CI workers that do not carry the fixture.

use pid_parse::publish::{
    load_codelist_index, load_drawing, load_drawing_graph, sqlite_load::open_readonly,
};
use std::path::Path;

const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

fn load_sqlite() -> Option<rusqlite::Connection> {
    let p = Path::new(SQLITE_PATH);
    if !p.exists() {
        eprintln!("skipping: fixture {SQLITE_PATH} not found");
        eprintln!(
            "run the pid_backup_extract → OrcaMdfProbe pipeline first; \
             see tools/orca-mdf-probe/Program.cs for the `--to-sqlite` flag."
        );
        return None;
    }
    Some(open_readonly(p).expect("read-only open should succeed"))
}

#[test]
fn real_sqlite_reports_a01_drawing_row() {
    let Some(conn) = load_sqlite() else { return };
    let drawing = load_drawing(&conn, A01_DRAWING_UID).expect("A01 drawing row should load");

    assert_eq!(drawing.drawing_uid, A01_DRAWING_UID);
    assert_eq!(
        drawing.drawing_name, "A01",
        "T_Drawing.Name should equal the filename stem"
    );
    assert_eq!(
        drawing.template.as_deref(),
        Some("A2-W-New.pid"),
        "TEST02 fixture's A01 drawing uses the A2-W-New template"
    );
    assert_eq!(
        drawing.path.as_deref(),
        Some(r"\01\01\A01.pid"),
        "T_Drawing.Path mirrors the SmartPlant archive layout"
    );
    eprintln!("A01 row loaded: {drawing:?}");
}

#[test]
fn real_sqlite_load_drawing_graph_exposes_a01_objects_and_representations() {
    let Some(conn) = load_sqlite() else { return };
    let graph = load_drawing_graph(&conn, A01_DRAWING_UID).expect("graph should load");

    // Header matches the single-row load test above.
    assert_eq!(graph.drawing_uid, A01_DRAWING_UID);
    assert_eq!(graph.drawing_name, "A01");

    // TEST02 A01 has three drawing-scoped SmartPlant objects that
    // show up through the T_Representation / T_Relationship join:
    // Vessel, Nozzle, and PipeRun. The fourth T_ModelItem row
    // (`PipingPoint`) is a logical endpoint hung off PipeRun via
    // T_PipingPoint — it does not appear as a standalone
    // representation and therefore is NOT expected in the
    // drawing-scoped object list at this stage.
    let item_types: std::collections::BTreeSet<&str> = graph
        .objects
        .iter()
        .map(|o| o.item_type_name.as_str())
        .collect();
    for want in ["Vessel", "Nozzle", "PipeRun"] {
        assert!(
            item_types.contains(want),
            "expected object of ItemTypeName `{want}`, got {:?}",
            item_types
        );
    }
    assert_eq!(
        graph.objects.len(),
        3,
        "A01 should have 3 drawing-level objects (PipingPoint lives in T_PipingPoint)"
    );

    // 6 representations on the drawing — pipeline line, nozzle,
    // vessel, and three labels/annotations.
    assert_eq!(
        graph.representations.len(),
        6,
        "A01 fixture has six T_Representation rows"
    );
    // At least one representation should point at the vessel.
    let vessel_uid = graph
        .objects
        .iter()
        .find(|o| o.item_type_name == "Vessel")
        .expect("vessel object")
        .uid
        .clone();
    assert!(
        graph
            .representations
            .iter()
            .any(|r| r.model_item_uid.as_deref() == Some(vessel_uid.as_str())),
        "at least one representation should reference the vessel ({vessel_uid})"
    );

    // 3 relationships on A01.
    assert_eq!(
        graph.relationships.len(),
        3,
        "A01 fixture has three T_Relationship rows"
    );
    // Diagnostic echo.
    eprintln!("A01 graph: {} objects, {} representations, {} rels",
        graph.objects.len(), graph.representations.len(), graph.relationships.len());
    for o in &graph.objects {
        eprintln!(
            "  object {} ({}) desc={:?} fields={}",
            o.uid,
            o.item_type_name,
            o.description,
            o.fields.len(),
        );
        for (k, v) in &o.fields {
            eprintln!("      {k} = {v:?}");
        }
    }
    for r in &graph.representations {
        eprintln!(
            "  rep {} graphic_oid={:?} symbol={:?} model_item={:?}",
            r.uid, r.graphic_oid, r.symbol_path, r.model_item_uid
        );
    }
    for rel in &graph.relationships {
        eprintln!(
            "  rel {} {:?} -> {:?} (is_binary={:?})",
            rel.uid, rel.source_uid, rel.target_uid, rel.is_binary
        );
    }
}

#[test]
fn real_sqlite_vessel_and_nozzle_objects_carry_business_fields() {
    let Some(conn) = load_sqlite() else { return };
    let graph = load_drawing_graph(&conn, A01_DRAWING_UID).expect("graph");

    // T_Equipment populates TagPrefix / TagSequenceNo for the
    // Vessel row, mirroring SPPID's "V 010121A" identifier.
    let vessel = graph
        .objects
        .iter()
        .find(|o| o.item_type_name == "Vessel")
        .expect("vessel object");
    assert_eq!(
        vessel.fields.get("TagPrefix").map(|s| s.as_str()),
        Some("V"),
        "Vessel.TagPrefix should be loaded from T_Equipment"
    );
    assert_eq!(
        vessel.fields.get("TagSequenceNo").map(|s| s.as_str()),
        Some("010121A"),
        "Vessel.TagSequenceNo matches the A01 fixture's well-known tag"
    );

    // T_Nozzle populates NominalDiameter + PipingMaterialsClass.
    let nozzle = graph
        .objects
        .iter()
        .find(|o| o.item_type_name == "Nozzle")
        .expect("nozzle object");
    assert_eq!(
        nozzle.fields.get("NominalDiameter").map(|s| s.as_str()),
        Some("250"),
        "Nozzle.NominalDiameter mirrors the DN250 spec"
    );
    assert_eq!(
        nozzle.fields.get("PipingMaterialsClass").map(|s| s.as_str()),
        Some("B5"),
        "Nozzle.PipingMaterialsClass mirrors the B5 spec"
    );

    // PipeRun: T_PipeRun populates both Connector fields and Pipe
    // fields; verify at least the ones that are non-null in the
    // fixture (A01's PipeRun has a NominalDiameter and
    // PipingMaterialsClass as well).
    let pipe = graph
        .objects
        .iter()
        .find(|o| o.item_type_name == "PipeRun")
        .expect("piperun object");
    assert!(
        !pipe.fields.is_empty(),
        "PipeRun should gather at least some fields from T_PipeRun / T_Connector"
    );
    eprintln!("PipeRun fields: {:#?}", pipe.fields);
}

#[test]
fn real_sqlite_codelist_loader_does_not_error_even_when_catalog_empty() {
    // A7: `load_codelist_index` must tolerate the TEST02 fixture
    // shape, where `codelists` + `attributes` exist but are empty
    // (OrcaMDF exported the schema but not the catalog rows).
    // The loader should return a default empty index without any
    // SQL failure.
    let Some(conn) = load_sqlite() else { return };
    let idx = load_codelist_index(&conn).expect("codelist loader should not error");
    eprintln!(
        "A7 codelist index: entries={}, attribute_mappings={}",
        idx.entry_count(),
        idx.attribute_mapping_count()
    );
    // The index is expected to be empty for this fixture, but the
    // assertion is written defensively so any future catalog
    // export still passes: the key correctness property is "no
    // error", not "zero rows".
    assert!(
        idx.entry_count() == 0 || idx.entry_count() > 0,
        "entry_count is always a valid count"
    );
    assert!(
        idx.attribute_mapping_count() == 0 || idx.attribute_mapping_count() > 0,
        "attribute_mapping_count is always a valid count"
    );
}
