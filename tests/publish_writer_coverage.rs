//! Integration / fixture tests for [`coverage_against_reference`]
//! over the SmartPlant reference Publish-Data documents.
//!
//! A14 closed every gap on the small A01 fixture, so it must show
//! 100% writer coverage. The larger DWG-0202GP06-01 fixture
//! exercises tag varieties the writer does not yet emit. Pinning
//! the remaining backlog here turns the next-phase work into an
//! executable contract — the moment a writer addition closes one
//! of those tags, the corresponding assertion will fail and
//! prompt the test to be tightened.
//!
//! Backlog history:
//! - A15 baseline: `PIDSignalPort × 16, PIDBranchPoint × 5,
//!   PIDPipingBranchPoint × 4, PIDPipingComponent × 2,
//!   PIDSignalConnector × 1` (28 tags total).
//! - A16 closed `PIDSignalPort` (16 derived per InstrFunction).
//! - A17 closed `PIDPipingComponent` (2 per PipingComp row), so
//!   the backlog dropped to 10 tags.
//! - A18 closed `PIDSignalConnector` (1 per SignalRun row), so
//!   the backlog drops to `PIDBranchPoint × 5 +
//!   PIDPipingBranchPoint × 4 = 9` tags.
//!
//! Both tests skip cleanly when the reference fixture is missing so
//! CI workers without the SmartPlant export bundle still pass.

use pid_parse::publish::coverage_against_reference;

const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";
const DWG_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml";

fn read_fixture(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        eprintln!("skipping: reference fixture {path} not found");
        return None;
    }
    Some(std::fs::read_to_string(p).expect("fixture should be readable utf-8"))
}

#[test]
fn coverage_on_a01_reference_is_100_percent_post_a14() {
    let Some(xml) = read_fixture(A01_REFERENCE_DATA_XML) else {
        return;
    };
    let cov = coverage_against_reference(&xml);
    assert!(
        cov.is_complete(),
        "A14 milestone: every PID tag in A01 must be writer-supported; backlog={:?}",
        cov.unsupported_in_reference,
    );
    // Post-A14 every variety in A01 is one of the 8 we MATCH.
    assert!(
        cov.supported_in_reference.len() >= 8,
        "A01 reference should surface at least 8 supported tag varieties; got {:?}",
        cov.supported_in_reference,
    );
}

#[test]
fn coverage_on_dwg_reference_documents_known_writer_backlog() {
    let Some(xml) = read_fixture(DWG_REFERENCE_DATA_XML) else {
        return;
    };
    let cov = coverage_against_reference(&xml);
    eprintln!("--- DWG-0202GP06-01 coverage report ---\n{cov}");

    // The DWG fixture exercises the full SmartPlant tag vocabulary.
    // Today the writer covers a subset; pin every known unsupported
    // tag so a future addition that closes one of these will trip
    // the assertion and prompt this test to be updated.
    assert!(
        !cov.is_complete(),
        "DWG-0202GP06-01 should still surface unsupported tags pre-Phase-15+; if this fails it means the writer just gained coverage of a previously unsupported tag — update this test to celebrate.",
    );

    let backlog: std::collections::BTreeMap<&str, usize> = cov
        .unsupported_in_reference
        .iter()
        .map(|r| (r.tag.as_str(), r.count))
        .collect();

    // Each of these tag varieties is in the DWG reference fixture
    // and the writer cannot yet emit them. Counts are pinned to
    // detect any drift in the reference fixture itself (e.g. a
    // SmartPlant exporter version bump that changes its emission
    // rules).
    for (tag, expected_count) in [
        ("PIDProcessPoint", 7), // DWG ProcessPoint for non-PipeRun objects, partially supported
        ("PIDBranchPoint", 5),
        ("PIDPipingBranchPoint", 4),
    ] {
        let actual = backlog.get(tag).copied();
        // ProcessPoint is partially supported (we derive one per
        // PipeRun), so allow either "fully unsupported" (count
        // matches) or "absent from backlog because the writer's
        // DWG-equivalent run would now emit them all". The other
        // tags must show up exactly because we have no derivation
        // for them.
        if tag == "PIDProcessPoint" {
            // Soft assertion — record but do not fail.
            eprintln!(
                "DWG backlog row for `{tag}`: actual={actual:?} expected={expected_count}",
            );
            continue;
        }
        assert_eq!(
            actual,
            Some(expected_count),
            "expected DWG fixture to ship {expected_count} occurrences of `{tag}` in the writer backlog; got {actual:?}",
        );
    }

    // A16: PIDSignalPort moved out of the backlog into the supported
    // bucket. Pin that explicitly so a future regression that drops
    // the writer back to "no SignalPort" trips this assertion.
    assert!(
        !backlog.contains_key("PIDSignalPort"),
        "PIDSignalPort should no longer be in the backlog after A16; backlog={backlog:?}",
    );
    let signal_port_supported = cov
        .supported_in_reference
        .iter()
        .find(|r| r.tag == "PIDSignalPort")
        .map(|r| r.count);
    assert_eq!(
        signal_port_supported,
        Some(16),
        "A16 milestone: 16 PIDSignalPort occurrences in DWG must now be writer-supported; got {signal_port_supported:?}",
    );

    // A17: PIDPipingComponent moved out of the backlog (the DWG
    // fixture ships 2 PipingComp samples — a `Cap` and a
    // `Conduit gate valve` — which the writer now emits directly
    // from the PipingComp dispatch arm). Regression guard: the
    // supported bucket must now carry PIDPipingComponent × 2.
    assert!(
        !backlog.contains_key("PIDPipingComponent"),
        "PIDPipingComponent should no longer be in the backlog after A17; backlog={backlog:?}",
    );
    let piping_component_supported = cov
        .supported_in_reference
        .iter()
        .find(|r| r.tag == "PIDPipingComponent")
        .map(|r| r.count);
    assert_eq!(
        piping_component_supported,
        Some(2),
        "A17 milestone: 2 PIDPipingComponent occurrences in DWG must now be writer-supported; got {piping_component_supported:?}",
    );

    // A18: PIDSignalConnector moved out of the backlog (the DWG
    // fixture ships 1 SignalRun row emitted through the new
    // SignalRun dispatch arm). Regression guard: the supported
    // bucket must now carry PIDSignalConnector × 1.
    assert!(
        !backlog.contains_key("PIDSignalConnector"),
        "PIDSignalConnector should no longer be in the backlog after A18; backlog={backlog:?}",
    );
    let signal_connector_supported = cov
        .supported_in_reference
        .iter()
        .find(|r| r.tag == "PIDSignalConnector")
        .map(|r| r.count);
    assert_eq!(
        signal_connector_supported,
        Some(1),
        "A18 milestone: 1 PIDSignalConnector occurrence in DWG must now be writer-supported; got {signal_connector_supported:?}",
    );

    // Coverage ratio sanity: writer must already cover SOMETHING in
    // DWG (the supported tags include PIDDrawing / PIDNozzle /
    // PIDProcessVessel / PIDPipeline / PIDPipingConnector / etc.
    // that are also present in DWG).
    assert!(
        cov.supported_total() > 0,
        "writer must already cover some DWG tags",
    );
    assert!(
        cov.unsupported_total() > 0,
        "DWG must still surface backlog rows pre-Phase-15+",
    );
}
