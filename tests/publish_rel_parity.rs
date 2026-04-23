//! A33 · Rel-level fidelity gates.
//!
//! SmartPlant Publish Data XML carries two top-level
//! element families:
//!
//! 1. `<PIDxxx>` business objects — covered by the A12 /
//!    A23 / A27 / A27b / A28 fidelity ratchet in
//!    `publish_interface_parity.rs` and friends.
//! 2. `<Rel>` relationship records — until A33 these were
//!    not directly gated; A12's tag-count diff happened to
//!    surface gross missing-Rel regressions because the
//!    overall PID tag totals shift, but a writer that
//!    swapped one Rel's DefUID for another (e.g. emitted
//!    `DwgRepresentationComposition` where SmartPlant
//!    expects `DrawingItems`) would slip past every gate.
//!
//! A33 closes that hole by adding two Rel-specific gates
//! built on the new
//! [`pid_parse::publish::parse_rel_defuid_counts`] helper:
//!
//! * **A33** — for every DefUID present in the SmartPlant
//!   A01 reference, the writer must emit at least the
//!   reference count. Extras are tolerated and only
//!   logged (the writer may emit additional derived Rels
//!   that SmartPlant skips on annotation-only items, etc.,
//!   without breaking downstream).
//! * **A33b** — A01 reference and DWG reference must
//!   declare the SAME set of DefUIDs (modulo
//!   [`KNOWN_A01_VS_DWG_REL_DEFUID_DIVERGENCES`]). DefUIDs
//!   are domain enums, not fixture-driven values, so
//!   cross-fixture set agreement is the expected baseline.
//!   Any divergence here flags a real SmartPlant export
//!   convention difference worth investigating.
//!
//! Both tests soft-skip when their fixtures are missing.

use std::collections::BTreeSet;

use pid_parse::publish::parse_rel_defuid_counts;

mod common;
use common::{generate_a01_xml, load_reference_a01_xml, load_reference_dwg_xml};

/// Known writer-side Rel DefUID gaps the A33 gate tolerates.
///
/// These are SmartPlant DefUIDs the A01 reference declares
/// but the writer does not yet emit. They were discovered
/// when the A33 gate first ran on the A01 fixture; A34 will
/// close them by extending `write_derived_connector_endpoints`
/// (and adding the missing T_Relationship-driven arms) to
/// emit the corresponding derived `<Rel>` blocks.
///
/// Entry format: `(defuid, milestone, rationale)`.
///
/// Whitelist semantics: the A33 gate ignores reference
/// counts on these DefUIDs entirely (rather than gating
/// `generated >= reference - tolerance`). The `closed_gaps`
/// detection in [`rel_defuid_parity_on_a01_writer_matches_reference_supersets`]
/// fails the test if a DefUID listed here is now actually
/// emitted in `>= reference_count`, so the whitelist cannot
/// silently drift out of date.
const KNOWN_WRITER_REL_DEFUID_GAPS: &[(&str, &str, &str)] = &[
    (
        "EquipmentComponentComposition",
        "A33-discovery (A34 will fix)",
        "T_Relationship has Vessel ↔ Nozzle classified, but A01 \
         row for this rel doesn't survive the loader's source/target \
         population. Will land alongside the loader's \
         vessel-nozzle relationship pickup in A34.",
    ),
    (
        "PipingConnectors",
        "A33-discovery (A34 will fix)",
        "Pipeline ↔ PipeRun composition rel is classified by \
         classify_relationship() but the loader's \
         T_Relationship pickup doesn't surface this row on A01. \
         A34 closes the gap together with EquipmentComponentComposition.",
    ),
    (
        "PipingPortComposition",
        "A33-discovery (A34 will fix)",
        "PipingConnector → PIDPipingPort.{1,2} derived rels. \
         write_derived_connector_endpoints emits the PIDPipingPort \
         elements but does not emit the corresponding Rel rows yet. \
         A34 adds the two-rel emit path next to the .1/.2 PIDPipingPort \
         emit.",
    ),
    (
        "ProcessPointCollection",
        "A33-discovery (A34 will fix)",
        "PipingConnector → PIDProcessPoint.PPT derived rel. Same \
         scope as PipingPortComposition (companion derived emit).",
    ),
    (
        "PipingEnd1Conn",
        "A33-discovery (A34 will fix)",
        "PIDPipingPort.1 → connected endpoint derived rel. Requires \
         knowledge of which model item the .1 port connects to (Nozzle \
         on the upstream side, typically). A34 will derive this from \
         T_PipingPoint endpoint columns the loader already pulls.",
    ),
    (
        "PipingEnd2Conn",
        "A33-discovery (A34 will fix)",
        "PIDPipingPort.2 → connected endpoint derived rel. Same \
         scope as PipingEnd1Conn.",
    ),
];

/// A33 · Writer ⊇ A01 reference at Rel DefUID granularity,
/// modulo [`KNOWN_WRITER_REL_DEFUID_GAPS`].
///
/// Three failure modes:
///
/// 1. **Unwhitelisted under-emit** — a DefUID is missing or
///    under-counted relative to the A01 reference and is NOT
///    in the whitelist. Forces explicit classification (add
///    a whitelist entry with milestone + rationale, or close
///    the gap via a writer change).
/// 2. **Closed gap stale entry** — a DefUID listed in
///    [`KNOWN_WRITER_REL_DEFUID_GAPS`] is now actually
///    emitted in `>= reference_count`. The whitelist entry
///    is stale and should be removed (and the milestone
///    pointer celebrated).
/// 3. **Extras** — writer emits a DefUID the reference
///    does not declare. Tolerated and only logged; SmartPlant's
///    composition emitter sometimes skips derived rels for
///    annotation-only items, so writer supersets are valid.
#[test]
fn rel_defuid_parity_on_a01_writer_matches_reference_supersets() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let generated_counts = parse_rel_defuid_counts(&generated_xml);
    let reference_counts = parse_rel_defuid_counts(&reference_xml);
    let whitelist: BTreeSet<&str> = KNOWN_WRITER_REL_DEFUID_GAPS
        .iter()
        .map(|(d, _, _)| *d)
        .collect();

    let mut unwhitelisted_under_emit: Vec<(String, usize, usize)> = Vec::new();
    let mut closed_gaps: Vec<(String, usize, usize)> = Vec::new();
    let mut tolerated_gaps: Vec<(String, usize, usize)> = Vec::new();
    let mut extras: Vec<(String, usize)> = Vec::new();

    for (defuid, ref_count) in &reference_counts {
        let gen_count = generated_counts.get(defuid).copied().unwrap_or(0);
        if gen_count < *ref_count {
            if whitelist.contains(defuid.as_str()) {
                tolerated_gaps.push((defuid.clone(), gen_count, *ref_count));
            } else {
                unwhitelisted_under_emit.push((defuid.clone(), gen_count, *ref_count));
            }
        } else if whitelist.contains(defuid.as_str()) {
            // Whitelist entry exists but the writer is now
            // meeting / exceeding the reference — gap is closed.
            closed_gaps.push((defuid.clone(), gen_count, *ref_count));
        }
    }
    for (defuid, gen_count) in &generated_counts {
        if !reference_counts.contains_key(defuid) {
            extras.push((defuid.clone(), *gen_count));
        }
    }

    eprintln!("--- A33 Rel-level parity (writer vs A01 reference) ---");
    eprintln!(
        "DefUIDs in reference: {}; in generated: {}; whitelist size: {}; \
         tolerated: {}; closed: {}; extras: {}",
        reference_counts.len(),
        generated_counts.len(),
        whitelist.len(),
        tolerated_gaps.len(),
        closed_gaps.len(),
        extras.len(),
    );
    if !tolerated_gaps.is_empty() {
        eprintln!("Tolerated gaps (matched whitelist):");
        for (defuid, gen_count, ref_count) in &tolerated_gaps {
            eprintln!("  [{defuid}] generated={gen_count}  reference={ref_count}");
        }
    }
    if !closed_gaps.is_empty() {
        eprintln!("Closed gaps (whitelist entries are now stale):");
        for (defuid, gen_count, ref_count) in &closed_gaps {
            eprintln!("  [{defuid}] generated={gen_count}  reference={ref_count}");
        }
    }
    if !extras.is_empty() {
        eprintln!("Extra DefUIDs on writer (informational, tolerated):");
        for (defuid, count) in &extras {
            eprintln!("  [{defuid}] x{count}");
        }
    }
    if !unwhitelisted_under_emit.is_empty() {
        eprintln!("Unwhitelisted under-emits (HARD CONTRACT):");
        for (defuid, gen_count, ref_count) in &unwhitelisted_under_emit {
            eprintln!("  [{defuid}] generated={gen_count}  reference={ref_count}");
        }
    }

    assert!(
        closed_gaps.is_empty(),
        "A33 whitelist-sync regression: these DefUIDs are now \
         emitted in >= reference count — remove the matching \
         KNOWN_WRITER_REL_DEFUID_GAPS entry (and celebrate): \
         {closed_gaps:?}",
    );
    assert!(
        unwhitelisted_under_emit.is_empty(),
        "A33 Rel parity regression: writer is missing or \
         under-emitting these DefUIDs vs the SmartPlant A01 \
         reference and they are NOT in KNOWN_WRITER_REL_DEFUID_GAPS. \
         Either add a whitelist entry (with milestone + rationale) \
         or close the gap via a writer change. \
         Each entry: `(DefUID, generated_count, reference_count)`:\n{:#?}",
        unwhitelisted_under_emit,
    );
}

/// Guard: every whitelist entry must carry a non-empty
/// rationale and milestone tag — silently inert entries
/// would mask real regressions.
#[test]
fn a33_whitelist_entries_carry_milestone_and_rationale() {
    let bad: Vec<&str> = KNOWN_WRITER_REL_DEFUID_GAPS
        .iter()
        .filter_map(|(d, m, r)| {
            if m.trim().is_empty() || r.trim().is_empty() {
                Some(*d)
            } else {
                None
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "KNOWN_WRITER_REL_DEFUID_GAPS has entries with empty \
         milestone or rationale (silently inert): {bad:?}"
    );
}

/// Sanity sub-test: confirm the contract surface is not
/// empty. If the parser regresses to returning empty maps
/// (or the writer stops emitting Rels entirely), the main
/// A33 test would false-pass.
#[test]
fn rel_defuid_parity_a01_reference_exposes_nonempty_inventory() {
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let counts = parse_rel_defuid_counts(&reference_xml);
    assert!(
        !counts.is_empty(),
        "A33 sanity: A01 reference must declare at least one Rel \
         DefUID; an empty map would silently disable the parity gate"
    );
}

/// Cross-fixture DefUID divergences known to be valid
/// SmartPlant variants. Format:
/// `(only_in_a01, only_in_dwg, milestone, rationale)`.
///
/// As of A33 landing the whitelist documents 4 DWG-only
/// DefUIDs the A01 reference does not declare. They all
/// derive from data SmartPlant ships only on DWG-flavor
/// fixtures (instrument signal connectors, piping tap
/// fittings) so are real SmartPlant variants rather than
/// drift.
#[allow(clippy::type_complexity)]
const KNOWN_A01_VS_DWG_REL_DEFUID_DIVERGENCES: &[(&[&str], &[&str], &str, &str)] = &[(
    &[],
    &[
        "PipingTapOrFitting",
        "SignalEnd1Conn",
        "SignalEnd2Conn",
        "SignalPortComposition",
    ],
    "A33",
    "DWG fixture ships instrument signal connectors \
     (`<PIDSignalConnector>`) and piping tap fittings; A01 \
     fixture has neither, so the corresponding Rel DefUIDs \
     never appear on A01. SmartPlant fixture-side variant; \
     no writer / loader work needed — when DWG-flavor SQLite \
     mirrors land in the repo, the writer's existing arms \
     (signal connector emit + T_Relationship classifier) \
     should naturally produce these DefUIDs.",
)];

/// A33b · Cross-fixture Rel DefUID set agreement.
///
/// Both A01 and DWG reference must declare identical
/// DefUID sets (modulo whitelist). DefUIDs are SmartPlant
/// domain enums (DrawingItems, PipingConnectors,
/// DwgRepresentationComposition, ...), not data-driven
/// values, so cross-fixture set agreement is the
/// expected baseline. A divergence here flags a
/// SmartPlant convention difference between plants — those
/// are real and need explicit documentation.
#[test]
fn a33b_a01_and_dwg_reference_rel_defuids_agree_set_wise() {
    let Some(a01_xml) = load_reference_a01_xml() else {
        return;
    };
    let Some(dwg_xml) = load_reference_dwg_xml() else {
        return;
    };

    let a01_set: BTreeSet<String> =
        parse_rel_defuid_counts(&a01_xml).into_keys().collect();
    let dwg_set: BTreeSet<String> =
        parse_rel_defuid_counts(&dwg_xml).into_keys().collect();

    let a01_only: BTreeSet<&str> = a01_set.difference(&dwg_set).map(|s| s.as_str()).collect();
    let dwg_only: BTreeSet<&str> = dwg_set.difference(&a01_set).map(|s| s.as_str()).collect();

    // Resolve whitelist coverage: the only-in-X sets must
    // either be empty OR fully accounted for by the
    // whitelist. The whitelist itself starts EMPTY at A33
    // landing so the strict-agreement path is the
    // happy-case.
    let mut tolerated_a01: BTreeSet<&str> = BTreeSet::new();
    let mut tolerated_dwg: BTreeSet<&str> = BTreeSet::new();
    for (a01_white, dwg_white, _ms, _rat) in KNOWN_A01_VS_DWG_REL_DEFUID_DIVERGENCES {
        for s in *a01_white {
            tolerated_a01.insert(*s);
        }
        for s in *dwg_white {
            tolerated_dwg.insert(*s);
        }
    }
    let unexpected_a01: Vec<&str> =
        a01_only.difference(&tolerated_a01).copied().collect();
    let unexpected_dwg: Vec<&str> =
        dwg_only.difference(&tolerated_dwg).copied().collect();

    eprintln!("--- A33b Cross-fixture Rel DefUID universality ---");
    eprintln!(
        "A01 DefUIDs: {} ; DWG DefUIDs: {} ; agreement: {} ; only_in_a01: {} ; only_in_dwg: {}",
        a01_set.len(),
        dwg_set.len(),
        a01_set.intersection(&dwg_set).count(),
        a01_only.len(),
        dwg_only.len(),
    );
    if !a01_only.is_empty() {
        eprintln!("Only in A01: {a01_only:?}");
    }
    if !dwg_only.is_empty() {
        eprintln!("Only in DWG: {dwg_only:?}");
    }

    assert!(
        unexpected_a01.is_empty() && unexpected_dwg.is_empty(),
        "A33b cross-fixture Rel DefUID divergence: A01 and DWG \
         references disagree on DefUID set membership. Either add a \
         KNOWN_A01_VS_DWG_REL_DEFUID_DIVERGENCES whitelist entry \
         documenting the variant (with milestone + rationale) or \
         close the gap via a writer / loader change. \
         only_in_A01 (unexpected): {unexpected_a01:?}; only_in_DWG \
         (unexpected): {unexpected_dwg:?}",
    );
}
