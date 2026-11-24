//! DWG generated-vs-reference `_Data.xml` parity gates, on the Oracle
//! line.
//!
//! The DWG plant is committed as a whole backup package: an Oracle
//! `exp` dump and the `SPPIDDataMap.xml` beside the drawing's `.pid`.
//! `common::generate_dwg_data_xml` publishes the `.pid` against them,
//! which is the CLI's `--oracle-exp` line, and this file scores that
//! document against the bundled `DWG-0202GP06-01_Data.xml`.
//!
//! What the Oracle line states and the reference does not, and the
//! reverse, is pinned below with the reason each gap has. The gaps are
//! content the `.pid` + dump pair cannot state — the plant register's
//! columns, the codelist levels above an enum's leaf, the vendor's
//! composed tags — or content this drawing's `.pid` does not place.

use std::collections::{BTreeMap, BTreeSet};

use pid_parse::publish::{parse_attrs_per_interface_per_tag, parse_interfaces_per_tag};

mod common;
use common::{generate_dwg_data_xml, load_reference_dwg_xml, TAGS_UNDER_PARITY};

/// Interfaces the reference states on a tag and the Oracle line does
/// not. The open-top tank's `ILowPressureTank` and `ILowPressureTankOcc`
/// are written when the dump states `EqType1` `@EE793` and `EqType2`
/// `@EE7A6`, so this list is empty.
const INTERFACES_THE_LINE_LEAVES_OUT: &[(&str, &[&str])] = &[];

/// `(tag, interface, attribute)` the reference states and the Oracle
/// line does not, each with what it is:
///
/// * `PIDDrawing/IDocument.DocTitle` — the document register's title;
///   the `.pid` states the drawing's name and UID, not the register
///   row (the CLI prints it among its excused classes).
/// * `PIDPipeline/IFluidSystem.FluidCode` — the reference's first
///   pipeline carries a fluid code the dump's data map does not map.
/// * `PIDPipingComponent/IPipingComponent.PipingComponentType1..3` —
///   the component type triple, on a component whose class the dump
///   does not state them for.
/// * `PIDPipingConnector/IObject.Name` — the vendor's composed tag
///   (`BV1001406A`, `-PG-1001105-F9--F9--`, `FT-753`); the line states
///   the UID and the derived interfaces, not the composed name.
/// * `PIDPipingConnector/IInsulatedItem.InsulThickSrc`,
///   `ISlopedPipingItem.SlopedPipeDirection` — the insulation and
///   slope source references, which the dump stores on the pipe run
///   and the reference states on the connector.
/// * `PIDProcessVessel/IObject.Description`,
///   `IEquipment.EqType0..3`, `IEquipment.EquipmentTrimSpec` — the
///   equipment register's columns.
const ATTRIBUTES_THE_LINE_LEAVES_OUT: &[(&str, &str, &[&str])] = &[
    ("PIDDrawing", "IDocument", &["DocTitle"]),
    ("PIDPipeline", "IFluidSystem", &["FluidCode"]),
    (
        "PIDPipingComponent",
        "IPipingComponent",
        &[
            "PipingComponentType1",
            "PipingComponentType2",
            "PipingComponentType3",
        ],
    ),
    ("PIDPipingConnector", "IObject", &["Name"]),
    (
        "PIDPipingConnector",
        "ISlopedPipingItem",
        &["SlopedPipeDirection"],
    ),
    ("PIDPipingConnector", "IInsulatedItem", &["InsulThickSrc"]),
    ("PIDProcessVessel", "IObject", &["Description"]),
    (
        "PIDProcessVessel",
        "IEquipment",
        &[
            "EqType0",
            "EqType1",
            "EqType2",
            "EqType3",
            "EquipmentTrimSpec",
        ],
    ),
];

/// `PIDBranchPoint` — the Oracle line writes one per OPC point the
/// drawing places (4); the reference carries 5. The fifth
/// (`799B9008DB6848F28127C78AC745C5A8`, name `278`) hangs on a
/// representation this `.pid` does not hold.
const DWG_PID_BRANCH_POINTS: usize = 4;
const DWG_REFERENCE_PID_BRANCH_POINTS: usize = 5;

/// The generated DWG `_Data.xml` must state every interface the
/// reference states. The open-top tank's two interfaces are among them.
#[test]
fn dwg_generated_interface_parity_is_the_reference_less_the_registered_variant() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("the package's data document renders");

    let generated_ifaces = parse_interfaces_per_tag(&generated_xml);
    let reference_ifaces = parse_interfaces_per_tag(&reference_xml);

    let registered: BTreeMap<&str, BTreeSet<&str>> = INTERFACES_THE_LINE_LEAVES_OUT
        .iter()
        .map(|(tag, ifaces)| (*tag, ifaces.iter().copied().collect()))
        .collect();

    let mut missing_per_tag: Vec<(String, Vec<String>)> = Vec::new();
    let mut checked_tags: Vec<String> = Vec::new();

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let Some(ref_set) = reference_ifaces.get(tag) else {
            continue;
        };
        checked_tags.push(tag.to_string());
        let Some(gen_set) = generated_ifaces.get(tag) else {
            missing_per_tag.push((tag.to_string(), ref_set.iter().cloned().collect()));
            continue;
        };
        let mut missing: Vec<String> = ref_set.difference(gen_set).cloned().collect();
        if let Some(excused) = registered.get(tag) {
            missing.retain(|iface| !excused.contains(iface.as_str()));
        }
        if !missing.is_empty() {
            missing_per_tag.push((tag.to_string(), missing));
        }
    }

    eprintln!("--- DWG generated-vs-reference interface parity (oracle-exp) ---");
    eprintln!("Checked tags: {checked_tags:?}");
    eprintln!("Registered variant: {INTERFACES_THE_LINE_LEAVES_OUT:?}");
    if !missing_per_tag.is_empty() {
        eprintln!("Missing interfaces on generated DWG XML (HARD CONTRACT):");
        for (tag, missing) in &missing_per_tag {
            eprintln!("  [{tag}] missing: {missing:?}");
        }
    }

    assert!(
        missing_per_tag.is_empty(),
        "DWG generated interface parity regression: the Oracle line is missing interfaces \
         the SmartPlant DWG reference states:\n\
         {missing_per_tag:#?}",
    );
}

/// The generated DWG `_Data.xml` must state every attribute the
/// reference states on an interface both documents carry, except the
/// pinned register / codelist / composed-tag set.
#[test]
fn dwg_generated_attribute_parity_is_the_reference_less_the_registered_columns() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("the package's data document renders");

    let generated_attrs = parse_attrs_per_interface_per_tag(&generated_xml);
    let reference_attrs = parse_attrs_per_interface_per_tag(&reference_xml);

    let registered: BTreeMap<(&str, &str), BTreeSet<&str>> = ATTRIBUTES_THE_LINE_LEAVES_OUT
        .iter()
        .map(|(tag, iface, attrs)| ((*tag, *iface), attrs.iter().copied().collect()))
        .collect();

    let mut missing: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut checked_pairs = 0usize;

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let Some(ref_ifaces) = reference_attrs.get(tag) else {
            continue;
        };
        let Some(gen_ifaces) = generated_attrs.get(tag) else {
            continue;
        };
        for (iface, ref_attrs) in ref_ifaces {
            let Some(gen_attrs) = gen_ifaces.get(iface) else {
                continue;
            };
            checked_pairs += 1;
            let mut miss: Vec<String> = ref_attrs.difference(gen_attrs).cloned().collect();
            if let Some(excused) = registered.get(&(tag, iface.as_str())) {
                miss.retain(|attr| !excused.contains(attr.as_str()));
            }
            if !miss.is_empty() {
                missing.push((tag.to_string(), iface.clone(), miss));
            }
        }
    }

    eprintln!("--- DWG generated-vs-reference attribute parity (oracle-exp) ---");
    eprintln!("Checked (tag, interface) pairs: {checked_pairs}");
    if !missing.is_empty() {
        eprintln!("Missing attrs on generated DWG XML (HARD CONTRACT):");
        for (tag, iface, miss) in &missing {
            eprintln!("  [{tag} / {iface}] missing: {miss:?}");
        }
    }

    assert!(
        missing.is_empty(),
        "DWG generated attribute parity regression: the Oracle line is missing attributes \
         the SmartPlant DWG reference declares, beyond the registered register / codelist / \
         composed-tag columns. Each entry is one `(PID tag, interface, attr_name[s])` \
         triplet:\n{missing:#?}",
    );
}

/// `PIDBranchPoint` shape and count: the count is the OPC points this
/// drawing places (4 against the reference's 5); the interface set and
/// attribute shape of the first occurrence are the reference's.
#[test]
fn dwg_generated_pid_branch_point_shape_matches_the_reference() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("the package's data document renders");

    assert_branch_point_shape_matches_reference(
        &generated_xml,
        &reference_xml,
        "PIDBranchPoint",
        false,
        DWG_PID_BRANCH_POINTS,
        DWG_REFERENCE_PID_BRANCH_POINTS,
    );
}

/// `PIDPipingBranchPoint` shape and count: 4 on both sides, each
/// `.BPT` UID.
#[test]
fn dwg_generated_piping_branch_point_shape_matches_the_reference() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("the package's data document renders");

    assert_branch_point_shape_matches_reference(
        &generated_xml,
        &reference_xml,
        "PIDPipingBranchPoint",
        true,
        4,
        4,
    );
}

fn assert_branch_point_shape_matches_reference(
    generated_xml: &str,
    reference_xml: &str,
    tag: &str,
    require_bpt_suffix: bool,
    generated_count: usize,
    reference_count: usize,
) {
    assert_eq!(
        generated_xml.matches(&format!("<{tag}>")).count(),
        generated_count,
        "{tag} count on the Oracle line",
    );
    assert_eq!(
        reference_xml.matches(&format!("<{tag}>")).count(),
        reference_count,
        "{tag} count in the DWG reference",
    );

    let generated_ifaces = parse_interfaces_per_tag(generated_xml);
    let reference_ifaces = parse_interfaces_per_tag(reference_xml);
    assert_eq!(
        generated_ifaces.get(tag),
        reference_ifaces.get(tag),
        "{tag} first-occurrence interface set drifted from the DWG reference",
    );

    let generated_attrs = parse_attrs_per_interface_per_tag(generated_xml);
    let reference_attrs = parse_attrs_per_interface_per_tag(reference_xml);
    let generated_attr_map = generated_attrs
        .get(tag)
        .cloned()
        .unwrap_or_else(BTreeMap::new);
    let reference_attr_map = reference_attrs
        .get(tag)
        .cloned()
        .unwrap_or_else(BTreeMap::new);
    assert_eq!(
        generated_attr_map, reference_attr_map,
        "{tag} first-occurrence attribute shape drifted from the DWG reference",
    );

    if require_bpt_suffix {
        let uids = collect_iobject_uids_for_tag(generated_xml, tag);
        assert_eq!(uids.len(), generated_count, "{tag} IObject count");
        let bad: Vec<String> = uids
            .into_iter()
            .filter(|uid| !uid.ends_with(".BPT"))
            .collect();
        assert!(
            bad.is_empty(),
            "{tag} IObject UIDs must use the `.BPT` suffix; offending UIDs: {bad:?}",
        );
    }
}

fn collect_iobject_uids_for_tag(xml: &str, tag: &str) -> Vec<String> {
    collect_tag_blocks(xml, tag)
        .into_iter()
        .filter_map(extract_iobject_uid)
        .collect()
}

fn collect_tag_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(start_off) = xml[cursor..].find(&open) {
        let start = cursor + start_off;
        let body_start = start + open.len();
        let Some(end_off) = xml[body_start..].find(&close) else {
            break;
        };
        let end = body_start + end_off + close.len();
        out.push(&xml[start..end]);
        cursor = end;
    }
    out
}

fn extract_iobject_uid(block: &str) -> Option<String> {
    let needle = "<IObject";
    let start = block.find(needle)?;
    let after_start = start + needle.len();
    let close = block[after_start..].find('>')? + after_start;
    extract_quoted_attr(&block.as_bytes()[after_start..close], b"UID=")
}

fn extract_quoted_attr(inside: &[u8], needle: &[u8]) -> Option<String> {
    let mut i = 0usize;
    while i + needle.len() <= inside.len() {
        if inside[i..].starts_with(needle) {
            let after_eq = i + needle.len();
            if after_eq >= inside.len() {
                return None;
            }
            let quote = inside[after_eq];
            let value_start = after_eq + 1;
            let end = inside[value_start..]
                .iter()
                .position(|&byte| byte == quote)
                .map(|off| value_start + off)?;
            return String::from_utf8(inside[value_start..end].to_vec()).ok();
        }
        i += 1;
    }
    None
}
