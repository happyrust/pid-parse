//! Mirror-backed DWG generated-vs-reference `_Data.xml` parity gates.
//!
//! A24 / A27b document how the bundled A01 and DWG reference
//! fixtures differ from each other. They are useful as fixture
//! facts, but they are NOT the same thing as "did our generated
//! DWG XML match the DWG reference once a real DWG MDF fixture
//! was available?".
//!
//! This file adds that missing acceptance layer:
//!
//! * generated DWG `_Data.xml` must emit at least the same
//!   interface set as the DWG reference for every in-scope tag,
//! * generated DWG `_Data.xml` must emit at least the same
//!   attribute set per `(tag, interface)` pair as the reference,
//! * the two branch-point tags must match the reference not only
//!   on count but also on first-occurrence shape and UID
//!   conventions.
//!
//! All tests soft-skip through `common::generate_dwg_data_xml()`
//! when the DWG MDF fixture is absent.

use std::collections::BTreeMap;

use pid_parse::publish::{parse_attrs_per_interface_per_tag, parse_interfaces_per_tag};

mod common;
use common::{generate_dwg_data_xml, load_reference_dwg_xml, TAGS_UNDER_PARITY};

#[test]
fn generated_dwg_interface_parity_matches_reference_superset_when_mirror_available() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on DWG mirror");

    let generated_ifaces = parse_interfaces_per_tag(&generated_xml);
    let reference_ifaces = parse_interfaces_per_tag(&reference_xml);

    let mut missing_per_tag: Vec<(String, Vec<String>)> = Vec::new();
    let mut extra_per_tag: Vec<(String, Vec<String>)> = Vec::new();
    let mut checked_tags: Vec<String> = Vec::new();

    for tag in TAGS_UNDER_PARITY {
        let tag = *tag;
        let Some(ref_set) = reference_ifaces.get(tag) else {
            continue;
        };
        let Some(gen_set) = generated_ifaces.get(tag) else {
            missing_per_tag.push((tag.to_string(), ref_set.iter().cloned().collect()));
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

    eprintln!("--- DWG generated-vs-reference interface parity ---");
    eprintln!("Checked tags: {checked_tags:?}");
    if !extra_per_tag.is_empty() {
        eprintln!("Extra interfaces on generated DWG XML (informational, tolerated):");
        for (tag, extra) in &extra_per_tag {
            eprintln!("  [{tag}] extra: {extra:?}");
        }
    }
    if !missing_per_tag.is_empty() {
        eprintln!("Missing interfaces on generated DWG XML (HARD CONTRACT):");
        for (tag, missing) in &missing_per_tag {
            eprintln!("  [{tag}] missing: {missing:?}");
        }
    }

    assert!(
        missing_per_tag.is_empty(),
        "DWG generated interface parity regression: writer is missing interfaces \
         present in the SmartPlant DWG reference:\n{:#?}",
        missing_per_tag,
    );
}

#[test]
fn generated_dwg_attribute_parity_matches_reference_superset_when_mirror_available() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on DWG mirror");

    let generated_attrs = parse_attrs_per_interface_per_tag(&generated_xml);
    let reference_attrs = parse_attrs_per_interface_per_tag(&reference_xml);

    let mut missing: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut extras: Vec<(String, String, Vec<String>)> = Vec::new();
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

    eprintln!("--- DWG generated-vs-reference attribute parity ---");
    eprintln!("Checked (tag, interface) pairs: {checked_pairs}");
    if !extras.is_empty() {
        eprintln!("Extra attrs on generated DWG XML (informational, tolerated):");
        for (tag, iface, ext) in &extras {
            eprintln!("  [{tag} / {iface}] extra: {ext:?}");
        }
    }
    if !missing.is_empty() {
        eprintln!("Missing attrs on generated DWG XML (HARD CONTRACT):");
        for (tag, iface, miss) in &missing {
            eprintln!("  [{tag} / {iface}] missing: {miss:?}");
        }
    }

    assert!(
        missing.is_empty(),
        "DWG generated attribute parity regression: writer is missing attributes \
         the SmartPlant DWG reference declares. Each entry is one \
         `(PID tag, interface, attr_name[s])` triplet:\n{:#?}",
        missing,
    );
}

#[test]
fn generated_dwg_pid_branch_point_shape_matches_reference_when_mirror_available() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on DWG mirror");

    assert_branch_point_shape_matches_reference(
        &generated_xml,
        &reference_xml,
        "PIDBranchPoint",
        false,
    );
}

#[test]
fn generated_dwg_piping_branch_point_shape_matches_reference_when_mirror_available() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_dwg_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on DWG mirror");

    assert_branch_point_shape_matches_reference(
        &generated_xml,
        &reference_xml,
        "PIDPipingBranchPoint",
        true,
    );
}

fn assert_branch_point_shape_matches_reference(
    generated_xml: &str,
    reference_xml: &str,
    tag: &str,
    require_bpt_suffix: bool,
) {
    let generated_count = generated_xml.matches(&format!("<{tag}>")).count();
    let reference_count = reference_xml.matches(&format!("<{tag}>")).count();
    assert_eq!(
        generated_count, reference_count,
        "{tag} count must match reference ({reference_count}); generated {generated_count}",
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
        assert!(
            !uids.is_empty(),
            "{tag} should emit at least one IObject UID on DWG output",
        );
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
