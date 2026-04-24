//! Integration / fixture tests for [`coverage_against_reference`]
//! over the SmartPlant reference Publish-Data documents.
//!
//! Both A01 and DWG-0202GP06-01 are now 100% writer-covered.
//!
//! Backlog history:
//! - A15 baseline: `PIDSignalPort × 16, PIDBranchPoint × 5,
//!   PIDPipingBranchPoint × 4, PIDPipingComponent × 2,
//!   PIDSignalConnector × 1` (28 tags total).
//! - A16 closed `PIDSignalPort` (16 derived per InstrFunction).
//! - A17 closed `PIDPipingComponent` (2 per PipingComp row).
//! - A18 closed `PIDSignalConnector` (1 per SignalRun row).
//! - Stage-4 closed `PIDBranchPoint × 5 +
//!   PIDPipingBranchPoint × 4`, reaching 0 backlog tags.
//!
//! Both tests skip cleanly when the reference fixture is missing so
//! CI workers without the SmartPlant export bundle still pass.

use pid_parse::publish::coverage_against_reference;

mod common;
use common::{load_reference_a01_xml, load_reference_dwg_xml};

#[test]
fn coverage_on_a01_reference_is_100_percent_post_a14() {
    let Some(xml) = load_reference_a01_xml() else {
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
    let Some(xml) = load_reference_dwg_xml() else {
        return;
    };
    let cov = coverage_against_reference(&xml);
    eprintln!("--- DWG-0202GP06-01 coverage report ---\n{cov}");

    // Stage-4 milestone: the DWG fixture is now 100% covered —
    // every PID tag in the reference is one the writer can emit.
    assert!(
        cov.is_complete(),
        "Stage-4 milestone: DWG-0202GP06-01 should be 100% writer-covered; backlog={:?}",
        cov.unsupported_in_reference,
    );

    // Regression guards — each graduated tag must be in the
    // supported bucket with its pinned instance count.
    let supported: std::collections::BTreeMap<&str, usize> = cov
        .supported_in_reference
        .iter()
        .map(|r| (r.tag.as_str(), r.count))
        .collect();

    for (tag, expected_count) in [
        ("PIDSignalPort", 16),       // A16
        ("PIDPipingComponent", 2),   // A17
        ("PIDSignalConnector", 1),   // A18
        ("PIDBranchPoint", 5),       // Stage-4
        ("PIDPipingBranchPoint", 4), // Stage-4
    ] {
        assert_eq!(
            supported.get(tag).copied(),
            Some(expected_count),
            "milestone regression: {tag} × {expected_count} must be writer-supported; got {:?}",
            supported.get(tag),
        );
    }

    assert!(
        cov.supported_total() > 0,
        "writer must cover all DWG tags",
    );
}
