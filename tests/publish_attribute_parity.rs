//! Attribute-name-level parity tests between the writer's
//! generated `A01_Data.xml` and the SmartPlant-produced
//! reference XML (A01 + DWG fixtures).
//!
//! ## Layered fidelity gates
//!
//! ```text
//! A12  parse_pid_tag_counts          tag-count diff
//! A15  coverage_against_reference    tag-coverage classifier
//! A23  parse_interfaces_per_tag      interface-set per tag
//! A24  cross-fixture interface gate  A01 ⇄ DWG interface universality
//! A26  parse_attrs_per_interface_per_tag  attr-name per (tag,iface)
//! A27  THIS FILE — writer ⊇ A01 ref (per (tag, interface) attr set)
//! A27b THIS FILE — A01 ref ⇄ DWG ref attr universality
//! ```
//!
//! A23 verified the writer emits the same INTERFACE list as the
//! SmartPlant reference. A27 goes one layer deeper: for every
//! `(PID tag, interface)` pair the reference exposes, the
//! writer must declare at least the same set of ATTRIBUTE
//! NAMES. A new SmartPlant attribute the writer doesn't know
//! about (e.g. an EngineeringCode column on IPipingNetworkSystem)
//! shows up here as a `(tag, interface, missing_attr)`
//! triplet diagnostic, pinning the gap concretely instead of
//! just "interface present, content unknown".
//!
//! A27b is the cross-fixture analog of A24: it verifies A01
//! and DWG references agree on the attribute name set for each
//! shared `(tag, interface)` pair. Documented variant
//! divergences (typically driven by SmartPlant domain enums
//! like ProcessVessel.EqType) are pinned in
//! [`KNOWN_A01_VS_DWG_ATTR_DIVERGENCES`] so they don't trigger
//! noise; any newly-emerged divergence breaks the test and
//! forces an explicit whitelist entry or a writer change.
//!
//! Both tests soft-skip when their fixtures are absent so CI
//! workers without the SmartPlant TEST02 backup bundle stay
//! green.

use std::collections::{BTreeMap, BTreeSet};

use pid_parse::publish::parse_attrs_per_interface_per_tag;

mod common;
use common::{
    generate_a01_xml, load_reference_a01_xml, load_reference_dwg_xml, TAGS_UNDER_PARITY,
};

/// A27 · Writer == A01 reference at attribute granularity.
///
/// For every `(tag, interface)` pair present in the
/// SmartPlant A01 reference for an in-scope PID tag, the
/// writer must declare the same set of attribute names on
/// that interface. A01 is now the only backup-backed
/// correctness baseline, so writer-side extras are no longer
/// informational drift — they are output regressions.
///
/// Failure mode produces a per-tag, per-interface report of
/// missing attribute names so the gap pinpoints exactly which
/// writer arm needs extending.
#[test]
fn attribute_parity_on_a01_writer_matches_reference_superset_per_interface() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let generated_attrs = parse_attrs_per_interface_per_tag(&generated_xml);
    let reference_attrs = parse_attrs_per_interface_per_tag(&reference_xml);

    // (tag, interface, missing_attrs)
    let mut missing: Vec<(String, String, Vec<String>)> = Vec::new();
    // (tag, interface, extra_attrs)
    let mut extras: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut checked_pairs = 0usize;

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let Some(ref_ifaces) = reference_attrs.get(tag) else {
            continue;
        };
        let Some(gen_ifaces) = generated_attrs.get(tag) else {
            // A23 already covers "writer missing entire tag"; we
            // skip here because A27 is per-(tag, interface) attr
            // contract, not a tag-presence contract.
            continue;
        };
        for (iface, ref_attrs) in ref_ifaces {
            let Some(gen_attrs) = gen_ifaces.get(iface) else {
                // A23 already covers "writer missing entire
                // interface"; skip per scope-of-this-gate.
                continue;
            };
            checked_pairs += 1;
            let miss: Vec<String> = ref_attrs.difference(gen_attrs).cloned().collect();
            let ext: Vec<String> = gen_attrs.difference(ref_attrs).cloned().collect();
            if !miss.is_empty() {
                missing.push((tag.to_string(), iface.clone(), miss));
            }
            if !ext.is_empty() {
                extras.push((tag.to_string(), iface.clone(), ext));
            }
        }
    }

    eprintln!("--- A27 attribute-parity (writer vs A01 reference) ---");
    eprintln!("Checked (tag, interface) pairs: {checked_pairs}");
    if !extras.is_empty() {
        eprintln!("Extra attrs on writer (HARD CONTRACT):");
        for (tag, iface, ext) in &extras {
            eprintln!("  [{tag} / {iface}] extra: {ext:?}");
        }
    }
    if !missing.is_empty() {
        eprintln!("Missing attrs on writer (HARD CONTRACT):");
        for (tag, iface, miss) in &missing {
            eprintln!("  [{tag} / {iface}] missing: {miss:?}");
        }
    }

    assert!(
        missing.is_empty(),
        "A27 attribute-parity regression: writer is missing \
         attributes the SmartPlant A01 reference declares. \
         Each entry below is one `(PID tag, interface, \
         attr_name[s])` triplet that needs a writer-side \
         emit:\n{:#?}",
        missing,
    );
    assert!(
        extras.is_empty(),
        "A27 attribute-parity regression: writer is emitting \
         attributes the SmartPlant A01 reference does not \
         declare. Each entry below is one `(PID tag, \
         interface, attr_name[s])` triplet that needs an \
         A01-side writer suppression or style-normalization:\n{:#?}",
        extras,
    );
}

/// Sanity sub-test: confirm A27's contract surface is
/// non-empty. If `parse_attrs_per_interface_per_tag` regresses
/// to returning empty maps (or the writer stops emitting
/// PIDPipeline entirely), the main A27 test would false-pass
/// because there's nothing to check. This guard pins the
/// contract surface to a sensible lower bound.
#[test]
fn attribute_parity_a01_reference_exposes_at_least_one_attr_per_supported_tag() {
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let attrs = parse_attrs_per_interface_per_tag(&reference_xml);
    let mut empty_tags: Vec<&str> = Vec::new();
    for tag in TAGS_UNDER_PARITY {
        let Some(ifaces) = attrs.get(*tag) else {
            // A23 sanity already covers "tag missing"; we only
            // care about "tag present but every interface has
            // an empty attr set", which would silently disable
            // the A27 contract.
            continue;
        };
        let any_attr = ifaces.values().any(|s| !s.is_empty());
        if !any_attr && !ifaces.is_empty() {
            empty_tags.push(*tag);
        }
    }
    assert!(
        empty_tags.is_empty(),
        "A27 sanity: these supported tags ship no attrs \
         anywhere in A01 reference, which would silently \
         disable the attribute-parity gate: {empty_tags:?}",
    );
}

/// Known cross-fixture attribute-shape divergences between
/// A01 and DWG references. Each entry pins a `(tag,
/// interface)` pair and the expected per-side attribute
/// extras, plus a milestone tag and rationale. New
/// unwhitelisted divergences fail [`a27b_a01_and_dwg_reference_attrs_agree_for_every_shared_tag_interface`].
///
/// Format: `(tag, interface, only_in_a01, only_in_dwg,
/// milestone, rationale)`.
///
/// ## Two divergence classes observed at A27b landing
///
/// 1. **IObject identifier rename** — the SmartPlant exporter
///    on A01 publishes the legacy `ItemTag` attribute on
///    `IObject` while the DWG export (taken from a different
///    plant + site config) uses `Name`. Affects three tags:
///    PIDPipeline, PIDPipingConnector, PIDProcessVessel. The
///    underlying value is the same business identifier; only
///    the attribute key differs. The writer follows A01 and
///    emits `ItemTag` everywhere for now. Closing this gap
///    requires either an A27c-style cross-fixture
///    site-config switch or a writer flag that tracks which
///    SmartPlant project flavor a drawing came from. Note
///    that PIDProcessVessel.IObject is asymmetric: A01 ships
///    `ItemTag` but DWG ships NEITHER `ItemTag` nor `Name` —
///    the DWG vessel was authored without a tag in the
///    source data, so the divergence reduces to a single
///    `only_in_a01: ["ItemTag"]` entry.
/// 2. **DWG-side enriched attribute coverage** — twelve
///    interfaces ship more attributes on DWG than on A01.
///    Every one of these maps to a SmartPlant column that
///    is populated on the DWG plant's source data but
///    NULL/empty on the A01 plant's source. The writer is
///    correct in not emitting them when the underlying
///    column is empty (SmartPlant convention: omit attr
///    rather than emit `attr=""`); closing the cross-fixture
///    gap therefore requires loader-side enrichment from
///    DWG's SQLite mirror, which is not currently bundled
///    in the repo. Tagged as A27b-discovery so a future
///    A28-series milestone can pick them off interface-by-
///    interface as the DWG mirror lands.
#[allow(clippy::type_complexity)]
const KNOWN_A01_VS_DWG_ATTR_DIVERGENCES: &[(
    &str,
    &str,
    &[&str],
    &[&str],
    &str,
    &str,
)] = &[
    (
        "PIDPipeline",
        "IObject",
        &["ItemTag"],
        &["Name"],
        "A27b",
        "IObject identifier rename: A01 export uses ItemTag, \
         DWG export uses Name on the same business identifier. \
         Site-config / project-flavor variant.",
    ),
    (
        "PIDPipingConnector",
        "IObject",
        &["ItemTag"],
        &["Name"],
        "A27b",
        "IObject identifier rename (see PIDPipeline/IObject entry).",
    ),
    (
        "PIDProcessVessel",
        "IObject",
        &["ItemTag"],
        &[],
        "A27b",
        "IObject identifier asymmetry: A01 vessel carries \
         ItemTag; DWG vessel was authored without a tag, so \
         neither ItemTag nor Name appears. Reduces to a one-\
         sided divergence rather than the rename pattern.",
    ),
    (
        "PIDPipeline",
        "IFluidSystem",
        &[],
        &["FluidCode", "FluidSystem"],
        "A27b",
        "DWG fixture's pipelines populate FluidCode + \
         FluidSystem (T_FluidSystem references); A01 fixture's \
         pipelines have these columns NULL so SmartPlant omits \
         the attrs. Loader-side gap, not a writer bug.",
    ),
    (
        "PIDNozzle",
        "IEquipmentComponent",
        &[],
        &["ProcessEqCompType1", "ProcessEqCompType2"],
        "A27b",
        "DWG nozzles populate ProcessEqCompType1/2 domain \
         enums; A01 nozzles do not. Loader-side enrichment \
         once DWG SQLite mirror is bundled.",
    ),
    (
        "PIDNozzle",
        "IPipingSpecifiedItem",
        &[],
        &["PipingMaterialsClass"],
        "A27b",
        "DWG nozzles carry a PipingMaterialsClass spec ref; \
         A01 nozzles do not. Loader-side gap.",
    ),
    (
        "PIDPipingConnector",
        "IConnector",
        &[],
        &["FlowDirection", "RepresentationsAreAllZeroLength"],
        "A27b",
        "DWG connectors carry FlowDirection (domain enum) + \
         RepresentationsAreAllZeroLength (boolean); A01 \
         connectors omit both. Loader-side gap.",
    ),
    (
        "PIDPipingConnector",
        "IInsulatedItem",
        &[],
        &["InsulThickSrc", "TotalInsulThick"],
        "A27b",
        "DWG connectors track insulation thickness fields; \
         A01 has none. Loader-side gap (T_InsulationSpec).",
    ),
    (
        "PIDPipingConnector",
        "IPipingConnector",
        &[],
        &["PipingConnectorType"],
        "A27b",
        "DWG connectors carry a PipingConnectorType domain \
         enum; A01 omits it. Loader-side gap.",
    ),
    (
        "PIDPipingConnector",
        "ISlopedPipingItem",
        &[],
        &["SlopedPipeDirection", "SlopedPipingAngle"],
        "A27b",
        "DWG models pipe slope (direction + angle); A01 does \
         not. Loader-side gap.",
    ),
    (
        "PIDPipingPort",
        "IConnection",
        &[],
        &["ConnectionFlowDirection"],
        "A27b",
        "DWG ports carry ConnectionFlowDirection; A01 omits \
         it. Loader-side gap (T_PipingPoint enrichment).",
    ),
    (
        "PIDProcessVessel",
        "IEquipment",
        &[],
        &["EqType0", "EqType1", "EqType2", "EqType3", "EquipmentTrimSpec"],
        "A27b",
        "DWG vessels populate the full EqType0..3 + \
         EquipmentTrimSpec domain-enum stack; A01 vessels \
         omit them. Same column family that drives the A25 \
         tank-variant detection — the loader-side A25b will \
         start emitting these as a side-effect of pulling the \
         EqType columns from T_ProcessEquipment.",
    ),
    (
        "PIDProcessVessel",
        "IPBSItem",
        &[],
        &["HeightRelativeToGrade"],
        "A27b",
        "DWG vessels publish HeightRelativeToGrade; A01 \
         omits it. Loader-side gap.",
    ),
    (
        "PIDProcessVessel",
        "IProcessVessel",
        &[],
        &["ProcessVessel_VesselVolumetricCapacity"],
        "A27b",
        "DWG vessels publish a volumetric capacity; A01 \
         omits it. Loader-side gap.",
    ),
    (
        "PIDProcessVessel",
        "ISpecifiedMatlItem",
        &[],
        &["LongMaterialDescription"],
        "A27b",
        "DWG vessels carry LongMaterialDescription on the \
         material-spec interface; A01 omits it. Loader-side \
         gap.",
    ),
];

/// A27b · Cross-fixture attribute-shape universality.
///
/// For every PID tag in [`TAGS_UNDER_PARITY`] AND every
/// interface present in BOTH A01 and DWG references for that
/// tag, the attribute name set must agree (modulo
/// [`KNOWN_A01_VS_DWG_ATTR_DIVERGENCES`] entries). Cases
/// where an interface is on one side only are out of scope —
/// those belong to the A24 interface-level gate.
///
/// Soft-skips when either reference fixture is missing.
#[test]
fn a27b_a01_and_dwg_reference_attrs_agree_for_every_shared_tag_interface() {
    let Some(a01_xml) = load_reference_a01_xml() else {
        return;
    };
    let Some(dwg_xml) = load_reference_dwg_xml() else {
        return;
    };

    let a01_attrs = parse_attrs_per_interface_per_tag(&a01_xml);
    let dwg_attrs = parse_attrs_per_interface_per_tag(&dwg_xml);

    // (tag, interface) -> (a01_only, dwg_only)
    type Key = (&'static str, &'static str);
    let whitelist: BTreeMap<Key, (BTreeSet<&str>, BTreeSet<&str>)> =
        KNOWN_A01_VS_DWG_ATTR_DIVERGENCES
            .iter()
            .map(|(tag, iface, a01_only, dwg_only, _ms, _rat)| {
                (
                    (*tag, *iface),
                    (
                        a01_only.iter().copied().collect::<BTreeSet<&str>>(),
                        dwg_only.iter().copied().collect::<BTreeSet<&str>>(),
                    ),
                )
            })
            .collect();

    let mut unexpected: Vec<(String, String, Vec<String>, Vec<String>)> = Vec::new();
    let mut closed_gaps: Vec<(String, String)> = Vec::new();
    let mut tolerated: Vec<(String, String)> = Vec::new();
    let mut agreements = 0usize;
    let mut compared_pairs = 0usize;

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let (Some(a01_ifaces), Some(dwg_ifaces)) =
            (a01_attrs.get(tag), dwg_attrs.get(tag))
        else {
            continue;
        };
        // Iterate the intersection of interface keys; one-side-
        // only interfaces are A24 territory.
        let a01_keys: BTreeSet<&str> =
            a01_ifaces.keys().map(|s| s.as_str()).collect();
        let dwg_keys: BTreeSet<&str> =
            dwg_ifaces.keys().map(|s| s.as_str()).collect();
        for iface in a01_keys.intersection(&dwg_keys).copied() {
            let a01_set = a01_ifaces.get(iface).expect("interface present in a01");
            let dwg_set = dwg_ifaces.get(iface).expect("interface present in dwg");
            compared_pairs += 1;
            let a01_only: BTreeSet<&str> =
                a01_set.difference(dwg_set).map(|s| s.as_str()).collect();
            let dwg_only: BTreeSet<&str> =
                dwg_set.difference(a01_set).map(|s| s.as_str()).collect();

            // Resolve any matching whitelist entry. The whitelist
            // key uses `&'static str`; cast iface to &str via
            // a leak only when needed for lookup.
            let key_lookup: Option<&(BTreeSet<&str>, BTreeSet<&str>)> = whitelist
                .iter()
                .find(|((wtag, wiface), _)| *wtag == tag && *wiface == iface)
                .map(|(_, v)| v);

            match key_lookup {
                Some((expected_a01_only, expected_dwg_only)) => {
                    if &a01_only == expected_a01_only && &dwg_only == expected_dwg_only {
                        tolerated.push((tag.to_string(), iface.to_string()));
                    } else if a01_only.is_empty() && dwg_only.is_empty() {
                        closed_gaps.push((tag.to_string(), iface.to_string()));
                    } else {
                        let unexpected_a01: Vec<String> = a01_only
                            .difference(expected_a01_only)
                            .map(|s| s.to_string())
                            .collect();
                        let unexpected_dwg: Vec<String> = dwg_only
                            .difference(expected_dwg_only)
                            .map(|s| s.to_string())
                            .collect();
                        unexpected.push((
                            tag.to_string(),
                            iface.to_string(),
                            unexpected_a01,
                            unexpected_dwg,
                        ));
                    }
                }
                None => {
                    if a01_only.is_empty() && dwg_only.is_empty() {
                        agreements += 1;
                    } else {
                        unexpected.push((
                            tag.to_string(),
                            iface.to_string(),
                            a01_only.into_iter().map(String::from).collect(),
                            dwg_only.into_iter().map(String::from).collect(),
                        ));
                    }
                }
            }
        }
    }

    eprintln!("--- A27b cross-fixture attribute-shape universality ---");
    eprintln!(
        "Compared (tag, interface) pairs: {compared_pairs}; agreements: {agreements}; \
         tolerated: {}; closed: {}; unexpected: {}",
        tolerated.len(),
        closed_gaps.len(),
        unexpected.len(),
    );
    if !tolerated.is_empty() {
        eprintln!("Tolerated divergences (matched whitelist):");
        for (tag, iface) in &tolerated {
            eprintln!("  [{tag} / {iface}]");
        }
    }
    if !closed_gaps.is_empty() {
        eprintln!("Closed gaps (whitelist entries are now stale):");
        for (tag, iface) in &closed_gaps {
            eprintln!("  [{tag} / {iface}]");
        }
    }
    if !unexpected.is_empty() {
        eprintln!("Unexpected attribute divergences:");
        for (tag, iface, a01_only, dwg_only) in &unexpected {
            eprintln!(
                "  [{tag} / {iface}] only_in_A01: {a01_only:?}  only_in_DWG: {dwg_only:?}"
            );
        }
    }

    assert!(
        closed_gaps.is_empty(),
        "A27b whitelist-sync regression: these (tag, interface) \
         pairs no longer diverge between A01 and DWG references — \
         remove the matching KNOWN_A01_VS_DWG_ATTR_DIVERGENCES \
         entry (and celebrate): {closed_gaps:?}",
    );
    assert!(
        unexpected.is_empty(),
        "A27b attribute-shape universality regression: A01 and \
         DWG references disagree on attribute names for these \
         (tag, interface) pairs. Either add a \
         KNOWN_A01_VS_DWG_ATTR_DIVERGENCES entry documenting \
         the variant (with milestone + rationale) or close the \
         gap via a writer / loader change. \
         Details:\n{:#?}",
        unexpected,
    );
}

/// Guard: every whitelist entry must be on a tag in scope.
/// A typo in the tag name would render the entry inert,
/// silently masking a real divergence.
#[test]
fn a27b_whitelist_tags_are_all_under_parity() {
    let parity_set: BTreeSet<&str> = TAGS_UNDER_PARITY.iter().copied().collect();
    let stale: Vec<&str> = KNOWN_A01_VS_DWG_ATTR_DIVERGENCES
        .iter()
        .filter_map(|(tag, _iface, _a, _b, _m, _r)| {
            if parity_set.contains(tag) {
                None
            } else {
                Some(*tag)
            }
        })
        .collect();
    assert!(
        stale.is_empty(),
        "KNOWN_A01_VS_DWG_ATTR_DIVERGENCES references tags not \
         in TAGS_UNDER_PARITY (entries would be inert): {stale:?}",
    );
}

/// Guard: every whitelist entry must carry at least one
/// attribute on one side. Empty + empty is an agreement, not
/// a divergence — silently inert whitelist entries are a
/// maintenance hazard.
#[test]
fn a27b_whitelist_entries_carry_at_least_one_attribute() {
    let empty: Vec<(String, String)> = KNOWN_A01_VS_DWG_ATTR_DIVERGENCES
        .iter()
        .filter_map(|(tag, iface, a01_only, dwg_only, _m, _r)| {
            if a01_only.is_empty() && dwg_only.is_empty() {
                Some((tag.to_string(), iface.to_string()))
            } else {
                None
            }
        })
        .collect();
    assert!(
        empty.is_empty(),
        "KNOWN_A01_VS_DWG_ATTR_DIVERGENCES has empty entries \
         (should be full agreements, not whitelist rows): {empty:?}",
    );
}
