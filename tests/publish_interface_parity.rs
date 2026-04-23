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
const DWG_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml";
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

/// Load the SmartPlant-produced reference DWG XML. Returns
/// `None` when the fixture is missing.
fn load_reference_dwg_xml() -> Option<String> {
    let p = std::path::Path::new(DWG_REFERENCE_DATA_XML);
    if !p.exists() {
        eprintln!("skipping: reference fixture {DWG_REFERENCE_DATA_XML} not found");
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

/// Known cross-fixture shape divergences between A01 and DWG.
///
/// Each entry is `(tag, (only_in_a01, only_in_dwg), milestone,
/// rationale)`. Documented divergences are tolerated; unknown
/// new divergences fail the test. When a divergence closes
/// (e.g., A25 lands tank-variant emission), remove its entry
/// and the test will confirm the whitelist drained to empty.
///
/// SPPID's per-tag interface list is mostly shape-static but in
/// a few edge cases it varies with domain-enum columns
/// (e.g., vessel type, pipeline class). Those are real variant
/// shapes — not fixture drift — and need conditional writer
/// paths to close. We pin the known gaps here so they can't
/// silently drift from the writer's current assumption set.
#[allow(clippy::type_complexity)]
const KNOWN_A01_VS_DWG_DIVERGENCES: &[(&str, (&[&str], &[&str]), &str, &str)] = &[
    (
        "PIDProcessVessel",
        (&[], &["ILowPressureTank", "ILowPressureTankOcc"]),
        "A25",
        "SmartPlant emits ILowPressureTank + ILowPressureTankOcc on \
         'Open top tank' EqType1/EqType0 vessel variants (DWG fixture \
         V F1081777… / EqType1=\"@EE793\") but not on 'Horizontal Drum' \
         variants (A01 fixture V C57494A1… / EqTypeDescription=\"Horizontal Drum\"). \
         A25 will close this gap by routing tank-type detection from \
         the T_ProcessEquipment EqType enum columns into a conditional \
         ILowPressureTank + ILowPressureTankOcc emit path. Until then, \
         the writer matches A01's 15-interface shape universally.",
    ),
];

/// A24 · Cross-fixture shape universality check.
///
/// Motivation: our writer is shape-static per tag — it emits the
/// same interface list regardless of which plant / drawing owns
/// the tag. The A23 parity gate verifies writer ⊇ A01 reference.
/// This test verifies A01 reference == DWG reference for each
/// supported tag's interface set (modulo known variant deltas
/// whitelisted in [`KNOWN_A01_VS_DWG_DIVERGENCES`]), which means
/// "writer ⊇ A01" is transitively "writer ⊇ DWG" too for
/// non-whitelisted tags. Any future SmartPlant export that
/// emits a *new* interface mismatch on DWG vs A01 for the same
/// tag would invalidate the shape-static assumption and break
/// this assertion, forcing us to either close the gap via a
/// conditional emit path or add a fresh whitelist entry with
/// documented rationale.
///
/// Works without the DWG SQLite mirror: operates purely on the
/// two reference XML fixtures that ARE bundled in the repo.
/// Soft-skips when either fixture is absent.
#[test]
fn a01_and_dwg_reference_interfaces_agree_for_every_shared_supported_tag() {
    let Some(a01_xml) = load_reference_a01_xml() else {
        return;
    };
    let Some(dwg_xml) = load_reference_dwg_xml() else {
        return;
    };

    let a01_ifaces = parse_interfaces_per_tag(&a01_xml);
    let dwg_ifaces = parse_interfaces_per_tag(&dwg_xml);

    // Build a lookup from tag → (expected only_in_a01, expected
    // only_in_dwg). An absent key means "no divergence tolerated".
    let whitelist: std::collections::BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>)> =
        KNOWN_A01_VS_DWG_DIVERGENCES
            .iter()
            .map(|(tag, (a01_only, dwg_only), _milestone, _rationale)| {
                (
                    *tag,
                    (
                        a01_only.iter().copied().collect::<BTreeSet<&str>>(),
                        dwg_only.iter().copied().collect::<BTreeSet<&str>>(),
                    ),
                )
            })
            .collect();

    let mut unexpected_deltas: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    let mut closed_gaps: Vec<String> = Vec::new();
    let mut agreements: Vec<String> = Vec::new();
    let mut tolerated_gaps: Vec<String> = Vec::new();
    let mut a01_only: Vec<String> = Vec::new();
    let mut dwg_only: Vec<String> = Vec::new();

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        match (a01_ifaces.get(tag), dwg_ifaces.get(tag)) {
            (Some(a01_set), Some(dwg_set)) => {
                let a01_minus_dwg: BTreeSet<String> =
                    a01_set.difference(dwg_set).cloned().collect();
                let dwg_minus_a01: BTreeSet<String> =
                    dwg_set.difference(a01_set).cloned().collect();

                match whitelist.get(tag) {
                    Some((expected_a01_only, expected_dwg_only)) => {
                        let actual_a01_only: BTreeSet<&str> =
                            a01_minus_dwg.iter().map(|s| s.as_str()).collect();
                        let actual_dwg_only: BTreeSet<&str> =
                            dwg_minus_a01.iter().map(|s| s.as_str()).collect();
                        if &actual_a01_only == expected_a01_only
                            && &actual_dwg_only == expected_dwg_only
                        {
                            tolerated_gaps.push(tag.to_string());
                        } else if actual_a01_only.is_empty() && actual_dwg_only.is_empty() {
                            // Someone closed the gap — whitelist
                            // entry is now stale.
                            closed_gaps.push(tag.to_string());
                        } else {
                            let unexpected_in_a01: Vec<String> = actual_a01_only
                                .difference(expected_a01_only)
                                .map(|s| s.to_string())
                                .collect();
                            let unexpected_in_dwg: Vec<String> = actual_dwg_only
                                .difference(expected_dwg_only)
                                .map(|s| s.to_string())
                                .collect();
                            unexpected_deltas.push((
                                tag.to_string(),
                                unexpected_in_a01,
                                unexpected_in_dwg,
                            ));
                        }
                    }
                    None => {
                        if a01_minus_dwg.is_empty() && dwg_minus_a01.is_empty() {
                            agreements.push(tag.to_string());
                        } else {
                            unexpected_deltas.push((
                                tag.to_string(),
                                a01_minus_dwg.into_iter().collect(),
                                dwg_minus_a01.into_iter().collect(),
                            ));
                        }
                    }
                }
            }
            (Some(_), None) => a01_only.push(tag.to_string()),
            (None, Some(_)) => dwg_only.push(tag.to_string()),
            (None, None) => {}
        }
    }

    eprintln!("--- A24 cross-fixture shape universality summary ---");
    eprintln!("Tags agreeing on interface set in both A01 and DWG: {agreements:?}");
    eprintln!("Tags with tolerated known divergences: {tolerated_gaps:?}");
    if !a01_only.is_empty() {
        eprintln!("Tags only present in A01 (no DWG coverage): {a01_only:?}");
    }
    if !dwg_only.is_empty() {
        eprintln!("Tags only present in DWG (no A01 coverage): {dwg_only:?}");
    }
    if !closed_gaps.is_empty() {
        eprintln!("Tags with CLOSED whitelist entries (celebrate + drop): {closed_gaps:?}");
    }
    if !unexpected_deltas.is_empty() {
        eprintln!("Unexpected interface deltas:");
        for (tag, only_a01, only_dwg) in &unexpected_deltas {
            eprintln!("  [{tag}] only_in_A01: {only_a01:?}  only_in_DWG: {only_dwg:?}");
        }
    }

    assert!(
        closed_gaps.is_empty(),
        "A24 whitelist-sync regression: these tags no longer \
         diverge between A01 and DWG references, which means \
         someone closed the fidelity gap. Remove the entry from \
         KNOWN_A01_VS_DWG_DIVERGENCES (and celebrate): {closed_gaps:?}",
    );

    assert!(
        unexpected_deltas.is_empty(),
        "A24 shape universality regression: A01 and DWG references \
         disagree on interface sets for these supported tags, which \
         breaks the 'writer is shape-static per tag' invariant for \
         non-whitelisted tags. Either add a KNOWN_A01_VS_DWG_DIVERGENCES \
         entry documenting the variant (with milestone + rationale) \
         or close the gap via a conditional writer emit. \
         Details:\n{:#?}",
        unexpected_deltas,
    );
}

/// Guard: the whitelist must reference real tags. A typo in the
/// tag name would make an entry silently inert — both halves
/// of the assert would evaluate on an empty delta from the
/// wrong side of the comparison, masking a real divergence.
#[test]
fn a01_vs_dwg_whitelist_tags_are_all_under_parity() {
    let parity_set: BTreeSet<&str> = TAGS_UNDER_PARITY.iter().copied().collect();
    let stale: Vec<&str> = KNOWN_A01_VS_DWG_DIVERGENCES
        .iter()
        .filter_map(|(tag, _, _, _)| {
            if parity_set.contains(tag) {
                None
            } else {
                Some(*tag)
            }
        })
        .collect();
    assert!(
        stale.is_empty(),
        "KNOWN_A01_VS_DWG_DIVERGENCES references tags not in \
         TAGS_UNDER_PARITY, which would make the whitelist entry \
         inert: {stale:?}",
    );
}

/// Guard: each whitelist entry must carry at least one interface
/// on one side (empty+empty is an agreement, not a divergence).
#[test]
fn a01_vs_dwg_whitelist_entries_carry_at_least_one_interface() {
    let empty: Vec<&str> = KNOWN_A01_VS_DWG_DIVERGENCES
        .iter()
        .filter_map(|(tag, (a01_only, dwg_only), _, _)| {
            if a01_only.is_empty() && dwg_only.is_empty() {
                Some(*tag)
            } else {
                None
            }
        })
        .collect();
    assert!(
        empty.is_empty(),
        "KNOWN_A01_VS_DWG_DIVERGENCES has empty entries (should be \
         full agreements, not whitelist rows): {empty:?}",
    );
}
