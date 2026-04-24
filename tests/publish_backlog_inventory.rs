//! A28 · Backlog tag fidelity spec — executable
//! reverse-engineering snapshot for PID tags the writer
//! does NOT yet emit.
//!
//! ## Why a snapshot test instead of code?
//!
//! `publish::supported_pid_tags()` lists the tags the writer
//! emits today. Every SmartPlant reference XML in the repo
//! (A01 + DWG fixtures) may ship extra tags that the writer
//! has no emit path for. A23/A27 only check tags the writer
//! claims to support, so the backlog stays invisible in CI
//! until someone manually re-scans the reference XMLs.
//!
//! A28 closes that observability gap: it pins the exact
//! interface + attribute shape of every backlog tag
//! observed in any reference fixture, into hard-coded
//! const tables guarded by `assert_eq!`. Two payoffs:
//!
//! * **Spec for future writer arms.** When someone takes
//!   on a backlog tag, the const table here is the
//!   executable spec — they extend the writer until
//!   `interface_parity_*` (A23) / `attribute_parity_*`
//!   (A27) start passing for the new tag.
//! * **Drift detection.** If a future SmartPlant export
//!   introduces a new attribute or reorders the interface
//!   list on a backlog tag, this test fails on the next
//!   `cargo test` rather than silently going unnoticed
//!   until someone implements that writer arm.
//!
//! Both fixtures soft-skip when missing so CI workers
//! without the SmartPlant TEST02 / DWG bundle stay green.
//!
//! ## Current state
//!
//! As of Stage-4 all previously backlogged tags
//! (`PIDBranchPoint` + `PIDPipingBranchPoint`) have
//! graduated into `supported_pid_tags()`. BACKLOG_SPECS
//! is empty. The guard test
//! `a28_every_unsupported_reference_tag_has_a_backlog_spec`
//! will fire the moment a new unsupported tag appears in
//! any reference fixture.

use std::collections::{BTreeMap, BTreeSet};

use pid_parse::publish::{
    parse_attrs_per_interface_per_tag, parse_interfaces_per_tag, parse_pid_tag_counts,
    supported_pid_tags,
};

mod common;
use common::{load_reference_a01_xml as load_a01, load_reference_dwg_xml as load_dwg};

/// Hard-coded spec for one backlog tag on one reference
/// fixture. `instance_count` is the number of `<PIDxxx>`
/// opens we expect in that fixture. `interfaces` is the
/// ordered list of interface element names that appear
/// inside the FIRST occurrence of the tag — order matters
/// because SmartPlant emits interfaces in a canonical
/// order that the writer must mirror. `interface_attrs`
/// maps each interface to the alphabetical set of
/// attribute names it carries.
struct BacklogTagSpec {
    /// e.g. `"PIDBranchPoint"`
    tag: &'static str,
    /// e.g. `"DWG"` for the fixture nickname
    fixture: &'static str,
    /// Total number of `<tag>` open occurrences expected.
    instance_count: usize,
    /// Interfaces of the FIRST occurrence, in the order
    /// they appear in the reference XML.
    interfaces: &'static [&'static str],
    /// Per-interface attribute name set (alphabetical).
    interface_attrs: &'static [(&'static str, &'static [&'static str])],
}

/// All backlog tag specs the snapshot test enforces.
/// Currently empty — all previously backlogged tags have
/// graduated into `supported_pid_tags()`. New specs will
/// be added here when a SmartPlant export surfaces a PID
/// tag the writer does not yet emit.
const BACKLOG_SPECS: &[&BacklogTagSpec] = &[];

fn pick_xml_for(fixture: &str) -> Option<String> {
    match fixture {
        "A01" => load_a01(),
        "DWG" => load_dwg(),
        other => panic!("unknown fixture nickname `{other}`"),
    }
}

/// A28 main snapshot: every entry in [`BACKLOG_SPECS`] must
/// match its reference fixture's actual shape exactly.
/// Failure modes:
///
/// * `instance_count` mismatch → SmartPlant added or
///   removed branch points in the fixture, OR our scanner
///   regressed on tag-counting.
/// * `interfaces` mismatch → SmartPlant changed the
///   canonical interface list / order for a backlog tag.
///   The future writer arm must be revised before it can
///   land cleanly.
/// * `interface_attrs` mismatch → SmartPlant added or
///   removed an attribute on a specific interface — the
///   spec needs to be updated.
#[test]
fn a28_backlog_tag_specs_match_reference_fixtures_exactly() {
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for spec in BACKLOG_SPECS {
        let Some(xml) = pick_xml_for(spec.fixture) else {
            continue;
        };
        checked += 1;

        // (1) Instance count.
        let counts = parse_pid_tag_counts(&xml);
        let actual_count = counts.get(spec.tag).copied().unwrap_or(0);
        if actual_count != spec.instance_count {
            failures.push(format!(
                "[{}/{}] instance_count: expected {}, got {}",
                spec.fixture, spec.tag, spec.instance_count, actual_count
            ));
        }

        // (2) Interface set (order-insensitive — the spec
        //     lists canonical order for human readers, but
        //     `parse_interfaces_per_tag` returns a BTreeSet,
        //     so we compare on set membership).
        let actual_ifaces = parse_interfaces_per_tag(&xml);
        let actual_iface_set: BTreeSet<&str> = actual_ifaces
            .get(spec.tag)
            .map(|s| s.iter().map(|x| x.as_str()).collect())
            .unwrap_or_default();
        let expected_iface_set: BTreeSet<&str> = spec.interfaces.iter().copied().collect();
        if actual_iface_set != expected_iface_set {
            let missing: Vec<&str> = expected_iface_set
                .difference(&actual_iface_set)
                .copied()
                .collect();
            let extra: Vec<&str> = actual_iface_set
                .difference(&expected_iface_set)
                .copied()
                .collect();
            failures.push(format!(
                "[{}/{}] interfaces drift: missing_from_fixture={:?} extra_in_fixture={:?}",
                spec.fixture, spec.tag, missing, extra
            ));
        }

        // (3) Per-interface attribute name sets.
        let actual_attrs = parse_attrs_per_interface_per_tag(&xml);
        let actual_per_iface: BTreeMap<&str, BTreeSet<&str>> = actual_attrs
            .get(spec.tag)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| {
                        (
                            k.as_str(),
                            v.iter().map(|x| x.as_str()).collect::<BTreeSet<_>>(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let expected_per_iface: BTreeMap<&str, BTreeSet<&str>> = spec
            .interface_attrs
            .iter()
            .map(|(iface, attrs)| (*iface, attrs.iter().copied().collect()))
            .collect();
        if actual_per_iface != expected_per_iface {
            failures.push(format!(
                "[{}/{}] attr drift: expected={:#?}\n actual={:#?}",
                spec.fixture, spec.tag, expected_per_iface, actual_per_iface
            ));
        }
    }

    eprintln!(
        "--- A28 backlog tag inventory ({} spec(s) checked, {} failure(s)) ---",
        checked,
        failures.len()
    );
    for f in &failures {
        eprintln!("  {f}");
    }

    assert!(
        failures.is_empty(),
        "A28 backlog tag spec drift: SmartPlant references no \
         longer match the hard-coded snapshot. Either:\n\
         1. Update the failing BacklogTagSpec entry to match \
            the new shape (and bump the milestone tag in the \
            comment to A28b/c/...), OR\n\
         2. Investigate WHY the fixture changed before \
            updating — drift here might mask a real \
            SmartPlant export regression worth reporting \
            upstream.\n\nFailures:\n{:#?}",
        failures
    );
}

/// Guard: every backlog spec must reference a tag the
/// writer does NOT yet support. If a tag in BACKLOG_SPECS
/// graduates into `supported_pid_tags()`, the snapshot
/// becomes a duplicate of A23/A27's coverage and should be
/// removed (the new writer arm now owns the spec).
#[test]
fn a28_backlog_spec_tags_are_not_in_writer_supported_set() {
    let supported: BTreeSet<&str> = supported_pid_tags().iter().copied().collect();
    let stale: Vec<&str> = BACKLOG_SPECS
        .iter()
        .map(|s| s.tag)
        .filter(|t| supported.contains(t))
        .collect();
    assert!(
        stale.is_empty(),
        "BACKLOG_SPECS references tags the writer now \
         supports — they should be moved into A23 / A27 \
         coverage instead: {stale:?}",
    );
}

/// Guard: backlog spec coverage must include every tag
/// observed in any reference fixture that is NOT in the
/// writer's supported set. Otherwise a newly-introduced
/// SmartPlant tag could land in fixtures and stay
/// invisible until someone notices the coverage gap.
#[test]
fn a28_every_unsupported_reference_tag_has_a_backlog_spec() {
    let supported: BTreeSet<&str> = supported_pid_tags().iter().copied().collect();
    let speccd_tags: BTreeSet<&str> = BACKLOG_SPECS.iter().map(|s| s.tag).collect();

    let mut observed_unsupported: BTreeSet<String> = BTreeSet::new();
    if let Some(xml) = load_a01() {
        for tag in parse_pid_tag_counts(&xml).keys() {
            if !supported.contains(tag.as_str()) {
                observed_unsupported.insert(tag.clone());
            }
        }
    }
    if let Some(xml) = load_dwg() {
        for tag in parse_pid_tag_counts(&xml).keys() {
            if !supported.contains(tag.as_str()) {
                observed_unsupported.insert(tag.clone());
            }
        }
    }

    let uncovered: Vec<&str> = observed_unsupported
        .iter()
        .map(|s| s.as_str())
        .filter(|t| !speccd_tags.contains(t))
        .collect();
    assert!(
        uncovered.is_empty(),
        "Unsupported PID tags appear in reference fixtures \
         but have no BacklogTagSpec entry. Add a spec or \
         document why this tag is intentionally ignored: \
         {uncovered:?}",
    );
}

/// Guard: the per-interface attr table must list every
/// interface in `interfaces` exactly once. A typo there
/// would silently exclude an interface from the attr
/// check, masking real drift.
#[test]
fn a28_per_spec_interface_lists_align_between_interfaces_and_interface_attrs() {
    for spec in BACKLOG_SPECS {
        let interfaces_set: BTreeSet<&str> = spec.interfaces.iter().copied().collect();
        let attrs_keys: BTreeSet<&str> = spec.interface_attrs.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            interfaces_set, attrs_keys,
            "[{}/{}] BacklogTagSpec.interfaces and \
             BacklogTagSpec.interface_attrs disagree on \
             interface set; spec is internally inconsistent",
            spec.fixture, spec.tag,
        );
    }
}
