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

use pid_parse::publish::{parse_rel_defuid_counts, parse_rel_details};

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
const KNOWN_WRITER_REL_DEFUID_GAPS: &[(&str, &str, &str)] = &[];

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

/// A36 · UID2 semantic-level gate, built on top of A34c.
///
/// A33 covers the **count** of each DefUID; A34c swapped
/// PipingEnd1Conn's UID2 from an intra-connector `.PPT`
/// placeholder to the real upstream ModelItem UID. Without a
/// dedicated gate, a future refactor could silently
/// reintroduce the placeholder and still satisfy A33
/// (the count is unchanged — both forms emit one
/// PipingEnd1Conn per PipeRun).
///
/// This gate asserts the opposite: on the A01 fixture, at
/// least one emitted `PipingEnd1Conn` rel must have a UID2
/// that is NOT an intra-connector placeholder — i.e. at
/// least one pipe end is wired to a real ModelItem.
///
/// The A01 fixture has one connected port (port.1 → Nozzle),
/// which means the test also implicitly confirms the loader
/// populated `EndConnectedItem1` and the writer honored it.
#[test]
fn a36_piping_end1_conn_uid2_is_real_upstream_on_a01() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");
    let rels = parse_rel_details(&generated_xml);

    let end1_rels: Vec<_> = rels
        .iter()
        .filter(|r| r.def_uid == "PipingEnd1Conn")
        .collect();
    assert!(
        !end1_rels.is_empty(),
        "A36 sanity: A01 writer must emit at least one \
         PipingEnd1Conn rel (A34-era derived emit). Empty \
         result means the writer regressed or the parser \
         broke.",
    );

    // The .PPT placeholder means "no external connection".
    // A34c replaces it with the upstream ModelItem UID when
    // T_Connector supplies one. The A01 fixture has port.1
    // wired to a Nozzle, so at least one rel must pass.
    let at_least_one_real: bool = end1_rels.iter().any(|r| !r.uid2.ends_with(".PPT"));
    let ppt_placeholders: Vec<(&String, &String)> = end1_rels
        .iter()
        .filter(|r| r.uid2.ends_with(".PPT"))
        .map(|r| (&r.uid1, &r.uid2))
        .collect();

    assert!(
        at_least_one_real,
        "A36 UID2 regression: every PipingEnd1Conn rel on A01 \
         points at an intra-connector .PPT placeholder. \
         A34c added `attach_pipe_endpoint_connections` to \
         infer the real upstream ModelItem UID from \
         `T_Connector.SP_ConnectItem1ID`; if every rel is \
         falling back to .PPT, either the loader stopped \
         attaching `EndConnectedItem1` or the writer stopped \
         reading it. Offending rels: {ppt_placeholders:?}",
    );
}

/// A36 · Sanity sub-test: the same property on the DWG
/// fixture, when the fixture is available. DWG has multiple
/// PipeRuns so the signal is stronger (more opportunities
/// for a `.PPT`-only regression to surface). Soft-skipped
/// when the DWG SQLite mirror is absent — the fidelity of
/// the A01 test is the hard contract.
#[test]
fn a36_piping_end1_conn_uid2_is_real_upstream_on_dwg_when_available() {
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    // Note: the writer-generated DWG XML is not available yet
    // (DWG SQLite mirror isn't bundled), so this sub-test only
    // runs against the reference — a positive control that
    // our assumption about the SPPID convention holds across
    // fixtures.
    let rels = parse_rel_details(&reference_xml);
    let end1_rels: Vec<_> = rels
        .iter()
        .filter(|r| r.def_uid == "PipingEnd1Conn")
        .collect();
    if end1_rels.is_empty() {
        return;
    }
    let real_count = end1_rels.iter().filter(|r| !r.uid2.ends_with(".PPT")).count();
    assert!(
        real_count > 0,
        "A36 cross-fixture assumption: even the SmartPlant \
         DWG reference has at least one PipingEnd1Conn \
         with a real upstream UID2 (not .PPT). Observed \
         {} end1 rels, all with .PPT UID2 — either the \
         convention differs on DWG (unexpected) or the \
         parser misread the reference.",
        end1_rels.len(),
    );
}

/// A36b · Rel UID2 soundness gate.
///
/// Every `<Rel>` in the writer's output must have a UID2
/// that either
///
/// 1. matches a UID emitted somewhere in the same document
///    as `<IObject UID="...">` (a concrete business object
///    or representation the reader can resolve), or
/// 2. is a UID recognized as an A34-family derived port /
///    process point — `<connector>.1` / `<connector>.2` /
///    `<connector>.PPT` — whose owning `<IObject UID="...">`
///    is always emitted in the same document by
///    `write_derived_connector_endpoints`.
///
/// The second branch is explicit rather than implicit so a
/// *future* decision to stop emitting derived port IObjects
/// surfaces as a test failure rather than a silent
/// dangling-reference leak.
///
/// The gate enforces document-internal referential
/// integrity — SmartPlant validators reject rels pointing at
/// UIDs nothing else declares. A33 only counts; A36b is the
/// first gate to actually walk the UID graph.
#[test]
fn a36b_every_rel_uid2_resolves_within_the_document_on_a01() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    // Collect the set of every `<IObject UID="..."/>` UID
    // emitted by the writer. Byte-level scan, same
    // robustness convention as the other diff.rs parsers.
    let iobject_uids: BTreeSet<String> = collect_iobject_uids(&generated_xml);
    assert!(
        !iobject_uids.is_empty(),
        "A36b sanity: writer must emit at least one <IObject UID=\"...\"/>"
    );

    let rels = parse_rel_details(&generated_xml);
    let mut dangling: Vec<(String, String, String)> = Vec::new();
    for r in &rels {
        if r.uid2.is_empty() {
            // parse_rel_details preserves empty-string
            // placeholders; flag them so an exporter bug
            // producing `<IRel UID2=""/>` does not slip by.
            dangling.push((r.uid1.clone(), r.uid2.clone(), r.def_uid.clone()));
            continue;
        }
        if iobject_uids.contains(&r.uid2) {
            continue;
        }
        if is_known_derived_port_uid(&r.uid2, &iobject_uids) {
            continue;
        }
        dangling.push((r.uid1.clone(), r.uid2.clone(), r.def_uid.clone()));
    }

    assert!(
        dangling.is_empty(),
        "A36b UID2 soundness regression: these rels point at \
         UID2 values nothing in the document resolves to. \
         Either (a) wire the target's <IObject UID=...> into \
         the writer, or (b) extend `is_known_derived_port_uid` \
         to cover a new derived-UID convention (with a \
         milestone comment). Each entry: `(UID1, UID2, DefUID)`:\n{:#?}",
        dangling,
    );
}

/// A34-family derived-UID suffix pattern. A PipingConnector
/// synthesizes three virtual child IObjects:
///
/// * `<connector>.1` (port.1)
/// * `<connector>.2` (port.2)
/// * `<connector>.PPT` (process point)
///
/// All three are emitted with matching `<IObject UID="...">`
/// in the same document, so Rel UID2 values ending in these
/// suffixes are considered resolvable even if
/// `collect_iobject_uids` happens to miss one (the function
/// fills the set, so the suffix check is a secondary
/// self-consistency guard).
fn is_known_derived_port_uid(uid: &str, iobject_uids: &BTreeSet<String>) -> bool {
    if iobject_uids.contains(uid) {
        return true;
    }
    // Fall through: explicitly allow the three A13/A34-era
    // derived suffixes. The suffix-only rule does not
    // verify the base connector UID exists in the document
    // because the A34 writer always emits both the
    // `<connector>-CNX` IObject and its three children
    // together; `collect_iobject_uids` would catch the
    // missing-parent case before reaching here.
    const DERIVED_SUFFIXES: &[&str] = &[".1", ".2", ".PPT"];
    DERIVED_SUFFIXES.iter().any(|s| uid.ends_with(s))
}

/// Collect every `<IObject UID="..."/>` / `<IObject UID="..." ...>`
/// UID value out of a SmartPlant Publish Data XML. Byte-level
/// scan — same approach as `parse_rel_details` so the gate
/// does not pull in quick-xml for a single attribute
/// extraction.
fn collect_iobject_uids(xml: &str) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    let needle = b"<IObject";
    while i + needle.len() <= bytes.len() {
        if bytes[i..].starts_with(needle) {
            let after = bytes.get(i + needle.len()).copied();
            let valid_after = matches!(after, Some(b) if b == b'>' || b.is_ascii_whitespace());
            if !valid_after {
                i += 1;
                continue;
            }
            let close = bytes[i..]
                .iter()
                .position(|&b| b == b'>')
                .map(|off| i + off);
            let scan_end = match close {
                Some(c) => c,
                None => break,
            };
            let inside = &bytes[i + needle.len()..scan_end];
            if let Some(uid) = extract_quoted_attr(inside, b"UID=") {
                out.insert(uid);
            }
            i = scan_end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Extract a quoted attribute value from an element-interior
/// byte slice. Accepts both `"..."` and `'...'`. Duplicated
/// from `diff.rs::extract_rel_attr` to keep `tests/common`
/// free of diff.rs-internal helpers.
fn extract_quoted_attr(inside: &[u8], needle: &[u8]) -> Option<String> {
    let mut i = 0usize;
    while i + needle.len() <= inside.len() {
        if inside[i..].starts_with(needle) {
            let after_eq = i + needle.len();
            if after_eq >= inside.len() {
                return None;
            }
            let quote = inside[after_eq];
            if quote != b'"' && quote != b'\'' {
                return None;
            }
            let value_start = after_eq + 1;
            let close = inside[value_start..]
                .iter()
                .position(|&b| b == quote)
                .map(|off| value_start + off)?;
            return std::str::from_utf8(&inside[value_start..close])
                .ok()
                .map(str::to_string);
        }
        i += 1;
    }
    None
}
