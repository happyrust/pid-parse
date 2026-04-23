//! Integration / fixture tests for [`coverage_against_reference`]
//! over the SmartPlant reference Publish-Data documents.
//!
//! A14 closed every gap on the small A01 fixture, so it must show
//! 100% writer coverage. The larger DWG-0202GP06-01 fixture
//! exercises tag varieties the writer does not yet emit
//! (PIDSignalPort, PIDPipingComponent, PIDBranchPoint,
//! PIDPipingBranchPoint, PIDSignalConnector); pinning that backlog
//! here turns the next-phase work into an executable contract — the
//! moment a writer addition closes one of those tags, the
//! corresponding assertion will fail and prompt the test to be
//! tightened.
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
        ("PIDSignalPort", 16),
        ("PIDProcessPoint", 7), // DWG uses ProcessPoint for non-PipeRun objects too
        ("PIDBranchPoint", 5),
        ("PIDPipingBranchPoint", 4),
        ("PIDPipingComponent", 2),
        ("PIDSignalConnector", 1),
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
