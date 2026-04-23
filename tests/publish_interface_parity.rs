//! Interface-level parity test between the writer's generated
//! `A01_Data.xml` and the SmartPlant-produced reference
//! `A01_Data.xml`.
//!
//! A23 establishes a fixture-driven regression gate that sits
//! one layer below the tag-only coverage check in
//! `publish_writer_coverage.rs`. The earlier test verifies every
//! PID tag the reference emits is ONE the writer knows about.
//! This test goes further: for each supported tag, it pins the
//! interface set emitted by SmartPlant into an assertion table
//! and confirms the writer emits at least the same interfaces —
//! closing the "we counted PIDPipeline but emitted only 6/10
//! interfaces inside" gap that A19/A20/A21/A22 systematically
//! closed.
//!
//! The test reads:
//! * The TEST02 SQLite mirror at
//!   `test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite`
//! * The reference A01 XML at
//!   `test-file/export-test/publish-data/A01/A01_Data.xml`
//!
//! and generates our `A01_Data.xml` in-process via
//! `publish::sqlite_load::load_drawing_graph` +
//! `publish::xml_writer::write_data_xml`. It then uses
//! [`pid_parse::publish::parse_interfaces_per_tag`] on both sides
//! to build `(tag -> interface set)` maps and asserts the writer
//! side is a superset of the reference set for every tag in the
//! reference inventory.
//!
//! Both inputs skip cleanly when missing so CI workers without
//! the SmartPlant TEST02 backup bundle continue to pass.

use std::collections::BTreeSet;

use pid_parse::publish::sqlite_load::open_readonly;
use pid_parse::publish::{
    load_drawing_graph, parse_interfaces_per_tag, write_data_xml, PublishError,
};

const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";
const PLANT_NAME: &str = "TEST02";

/// Generate `A01_Data.xml` through the real publish pipeline:
/// open the TEST02 SQLite mirror, load the drawing graph for the
/// A01 drawing UID, and emit the XML as a String. Returns `None`
/// when the fixture is not available (soft-skip on CI workers
/// without the SmartPlant backup bundle).
fn generate_a01_xml() -> Option<Result<String, PublishError>> {
    let sqlite_path = std::path::Path::new(SQLITE_PATH);
    if !sqlite_path.exists() {
        eprintln!("skipping: SQLite fixture {SQLITE_PATH} not found");
        return None;
    }
    let conn = match open_readonly(sqlite_path) {
        Ok(c) => c,
        Err(e) => return Some(Err(e)),
    };
    let drawing = match load_drawing_graph(&conn, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_data_xml(&drawing, PLANT_NAME))
}

/// Load the SmartPlant-produced reference A01 XML. Returns
/// `None` when the fixture is missing.
fn load_reference_a01_xml() -> Option<String> {
    let p = std::path::Path::new(A01_REFERENCE_DATA_XML);
    if !p.exists() {
        eprintln!("skipping: reference fixture {A01_REFERENCE_DATA_XML} not found");
        return None;
    }
    Some(std::fs::read_to_string(p).expect("reference should be utf8"))
}

/// The subset of PID tags that the writer emits and we want to
/// assert interface-level parity on. Tags the writer does not
/// emit yet (PIDBranchPoint / PIDPipingBranchPoint) are omitted.
/// Tags that are meta-only (PIDDrawing, PIDRepresentation) are
/// included because they still exercise the wrapper emission
/// path.
const TAGS_UNDER_PARITY: &[&str] = &[
    "PIDControlSystemFunction",
    "PIDDrawing",
    "PIDNote",
    "PIDNozzle",
    "PIDPipeline",
    "PIDPipingComponent",
    "PIDPipingConnector",
    "PIDPipingPort",
    "PIDProcessPoint",
    "PIDProcessVessel",
    "PIDRepresentation",
    "PIDSignalConnector",
    "PIDSignalPort",
];

#[test]
fn interface_parity_on_a01_writer_matches_reference_superset_post_a22() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let generated_ifaces = parse_interfaces_per_tag(&generated_xml);
    let reference_ifaces = parse_interfaces_per_tag(&reference_xml);

    let mut missing_per_tag: Vec<(String, Vec<String>)> = Vec::new();
    let mut extra_per_tag: Vec<(String, Vec<String>)> = Vec::new();
    let mut checked_tags = Vec::new();

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let Some(ref_set) = reference_ifaces.get(tag) else {
            // A01 fixture doesn't exercise this tag — no parity
            // to check. Still log it for diagnostic visibility.
            eprintln!("[A23] tag `{tag}`: not present in A01 reference; skipping");
            continue;
        };
        let Some(gen_set) = generated_ifaces.get(tag) else {
            // We declare the tag supported but the generator didn't
            // emit a single instance for this drawing. Treat as
            // full-miss and continue.
            missing_per_tag.push((
                tag.to_string(),
                ref_set.iter().cloned().collect(),
            ));
            continue;
        };
        let missing: Vec<String> = ref_set.difference(gen_set).cloned().collect();
        let extra: Vec<String> = gen_set.difference(ref_set).cloned().collect();
        if !missing.is_empty() {
            missing_per_tag.push((tag.to_string(), missing));
        }
        if !extra.is_empty() {
            extra_per_tag.push((tag.to_string(), extra));
        }
        checked_tags.push(tag.to_string());
    }

    // Emit a diagnostic summary before any panic so the report
    // surfaces the full picture instead of the first failing tag.
    eprintln!("--- A23 interface-parity summary ---");
    eprintln!("Checked tags: {checked_tags:?}");
    if !missing_per_tag.is_empty() {
        eprintln!("Missing interfaces (reference has, generated lacks):");
        for (tag, miss) in &missing_per_tag {
            eprintln!("  [{tag}] missing: {miss:?}");
        }
    }
    if !extra_per_tag.is_empty() {
        eprintln!("Extra interfaces (generated has, reference lacks):");
        for (tag, ext) in &extra_per_tag {
            eprintln!("  [{tag}] extra: {ext:?}");
        }
    }

    // Missing-interface assertion is the hard contract. After
    // A19 / A20 / A21 / A22 every supported PID tag that
    // appears in A01 must emit the full reference interface
    // set. A regression that drops one interface will surface
    // as a non-empty `missing_per_tag`, pinning the exact tag
    // + interface combo.
    assert!(
        missing_per_tag.is_empty(),
        "A23 interface-parity regression: writer is missing interfaces \
         present in SmartPlant A01 reference:\n{:#?}",
        missing_per_tag,
    );

    // Extras are informational: the writer is a strict superset
    // of the reference in some places (A19 always-declared
    // `FluidCode=""`, etc.). We record them for visibility but
    // do not fail — they are downstream-valid XML supersets.
}

#[test]
fn interface_parity_generator_produces_every_supported_tag_on_a01_subset() {
    // Sanity sub-test: confirm the pipeline actually produces at
    // least one occurrence of every TAG_UNDER_PARITY entry that
    // A01 ships. If the generator started to miss PIDPipeline
    // entirely, the main parity test's "no missing interfaces"
    // contract could false-positive pass (nothing to compare).
    // This sub-test guards against that regression.
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let generated_set: BTreeSet<&str> = parse_interfaces_per_tag(&generated_xml)
        .keys()
        .map(|s| s.as_str())
        .map(|s| -> &'static str {
            // leak to gain a 'static view for this short-lived
            // comparison (test-only path, negligible leak).
            Box::leak(s.to_string().into_boxed_str())
        })
        .collect();
    let reference_set: BTreeSet<&str> = parse_interfaces_per_tag(&reference_xml)
        .keys()
        .map(|s| -> &'static str { Box::leak(s.to_string().into_boxed_str()) })
        .collect();

    let tags_under_parity: BTreeSet<&str> = TAGS_UNDER_PARITY.iter().copied().collect();
    let a01_expected: BTreeSet<&str> =
        reference_set.intersection(&tags_under_parity).copied().collect();

    let missing_from_generator: Vec<&str> = a01_expected
        .iter()
        .copied()
        .filter(|t| !generated_set.contains(t))
        .collect();

    assert!(
        missing_from_generator.is_empty(),
        "Generator is missing these supported PID tags from the A01 output: {missing_from_generator:?}",
    );
}
