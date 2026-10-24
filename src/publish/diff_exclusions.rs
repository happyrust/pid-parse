//! Reference-side accounting for the `--diff-against` semantic diff.
//!
//! `diff_pid_export` walks the generated document; this module walks the
//! reference back the other way. Every object, attribute value or rel the
//! reference carries and the generated document does not is a `missing`
//! finding unless one of five exclusion sources covers it:
//!
//! 1. item classes the `.pid` file cannot state — derived from
//!    `PublishItemCatalog` via `catalog::pid_file_cannot_state`, and only
//!    when this run published no object of that class;
//! 2. the `(interface, attribute)` pairs of
//!    `catalog::EXCLUDED_ATTRIBUTE_CLASSES` the plant database supplies;
//! 3. relations this run states nowhere — the same wiring under another
//!    `<Rel>` UID, an end the generated document does not publish (a
//!    `PIDPipeline` that pairs by name counts as published), or no generated
//!    rel of that `DefUID` at all. That last excuse never covers a class the
//!    `.pid` alone yields (`FILE_RELATIONS`), and covers a class only the exp
//!    dump supplies (`DUMP_RELATIONS`) only while the run carries none of the
//!    dump's content, so a run that lost a whole class it writes is a finding;
//! 4. the vendor's tag-named duplicate element — a child element named
//!    after the block's own `IObject.Name` restating `UID` / `Name` /
//!    `Description` with the block's own values;
//! 5. the vendor's composed object name — a reference-only `IObject.Name`
//!    on an object this run publishes but does not name, when this line
//!    never names that class or names other objects of it in this run. A
//!    class this line names that comes out with no name at all is a finding.
//!
//! Excused content is counted and rendered as one `excused …` line per
//! exclusion class that fired; a `PIDPipeline` object or `PipingConnectors`
//! rel the by-name pairing already accounts for is counted, not excused.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::publish::catalog::{self, PidTagRole};
use crate::publish::pid_load;

use super::{
    pipeline_names, pipelines_by_name, piping_connector_key, piping_connector_keys, PidExportDiff,
    PublishBlocks,
};

/// Relation classes the `.pid` alone yields (`pid_writer::write_document`):
/// the drawing's representations, their items and the process points. A run
/// that states none of them has lost them.
const FILE_RELATIONS: [&str; 3] = [
    "DwgRepresentationComposition",
    "DrawingItems",
    "ProcessPointCollection",
];

/// Relation classes only the exp dump supplies: the port compositions, the
/// six connectivity definitions (`oracle_exp::connectivity_def_uid`), the
/// nozzle compositions and item notes, and the synthesized pipelines'
/// connectors.
const DUMP_RELATIONS: [&str; 11] = [
    "PipingPortComposition",
    "SignalPortComposition",
    "PipingTapOrFitting",
    "SignalTapOrFitting",
    "PipingEnd1Conn",
    "PipingEnd2Conn",
    "SignalEnd1Conn",
    "SignalEnd2Conn",
    "EquipmentComponentComposition",
    "PBSItemNotes",
    "PipingConnectors",
];

/// Reference-side pass over `reference`: count every object, attribute value
/// and rel the generated document does not carry, push a `missing …` finding
/// for each one the exclusion list does not cover, and append one
/// `excused …` line per exclusion class that fired to `diff.exclusions`.
///
/// Called once from `diff_pid_export` after the generated-side walk. Results
/// land on `diff`'s `*_not_in_generated` / `*_excused_by_design` /
/// `*_paired_by_name` fields; the summary counters (`objects` / `values` /
/// `rels`) are untouched so a clean run prints byte-identical output.
pub(super) fn account_reference_side(
    generated: &PublishBlocks,
    reference: &PublishBlocks,
    diff: &mut PidExportDiff,
) {
    let generated_pipelines = pipelines_by_name(generated);
    let generated_pipeline_names = pipeline_names(generated);
    let reference_pipeline_names = pipeline_names(reference);
    let generated_connectors = piping_connector_keys(generated, &generated_pipeline_names);
    let published_classes: HashSet<&str> = generated
        .objects
        .values()
        .map(|(class, _)| class.as_str())
        .collect();
    let named_classes: HashSet<&str> = generated
        .objects
        .values()
        .filter(|(_, values)| values.contains_key("IObject.Name"))
        .map(|(class, _)| class.as_str())
        .collect();
    // Content only an exp run writes: a port, an OPC branch point or a
    // synthesized pipeline, or a relation the dump supplies.
    let dump_objects = generated
        .objects
        .values()
        .any(|(class, _)| catalog::pid_file_cannot_state(class).is_some());
    let dump_rels = generated
        .rels
        .iter()
        .any(|rel| DUMP_RELATIONS.contains(&rel.3.as_str()));
    let dump_content = dump_objects || dump_rels;

    let mut missing_objects = Vec::new();
    let mut missing_rels = Vec::new();
    let mut missing_values = Vec::new();
    let mut excused_objects: BTreeMap<&str, (usize, String)> = BTreeMap::new();
    let mut excused_members: BTreeMap<String, (usize, &'static str, &'static str)> =
        BTreeMap::new();
    let mut tag_named = 0usize;
    let mut composed_names: BTreeMap<&str, usize> = BTreeMap::new();
    let mut excused_rels: BTreeMap<(String, String), usize> = BTreeMap::new();

    let mut uids: Vec<&String> = reference.objects.keys().collect();
    uids.sort();

    // Objects: only a UID the generated document lacks entirely is
    // reference-only — a UID it publishes under any class is already
    // accounted for (and reported, if the classes differ) by the
    // generated-side walk. `PIDPipeline` name-pairing fires only for a UID
    // the generated document lacks: that is how the synthesized exp-mode
    // pipelines pair.
    for &uid in &uids {
        let (class, values) = &reference.objects[uid];
        let mut accounted = generated.objects.contains_key(uid);
        if !accounted && class == "PIDPipeline" {
            let name = values.get("IObject.Name").cloned().unwrap_or_default();
            if generated_pipelines.contains_key(&name) {
                diff.objects_paired_by_name += 1;
                accounted = true;
            }
        }
        if accounted {
            continue;
        }
        let excusable = catalog::pid_file_cannot_state(class)
            .filter(|_| !published_classes.contains(class.as_str()));
        if let Some((owner, role)) = excusable {
            diff.objects_excused_by_design += 1;
            let words = match role {
                PidTagRole::Port => "a port row the exp dump supplies",
                _ => "enrichment the file cannot state",
            };
            let reason = format!(
                "PublishItemCatalog row \"{owner}\" gives it PidTagRole::{role:?} \
                 ({words}) and this run published no {class}"
            );
            excused_objects
                .entry(class.as_str())
                .or_insert((0, reason))
                .0 += 1;
        } else {
            diff.objects_not_in_generated += 1;
            missing_objects.push(format!("missing {class} {uid}"));
        }
    }

    // Rels: an exact tuple match first, then the by-name pairing for
    // `PipingConnectors`, then the three excuse rules in order.
    let mut rels: Vec<&(String, String, String, String)> = reference.rels.iter().collect();
    rels.sort();
    for rel in rels {
        if generated.rels.contains(rel) {
            continue;
        }
        let (rel_uid, uid1, uid2, def_uid) = rel;
        if def_uid == "PipingConnectors" {
            if let Some(key) = piping_connector_key(rel, &reference_pipeline_names) {
                if generated_connectors.contains(&key) {
                    diff.rels_paired_by_name += 1;
                    continue;
                }
            }
        }
        let same_wiring = generated.rels.iter().any(|other| {
            other.0 != *rel_uid && other.1 == *uid1 && other.2 == *uid2 && other.3 == *def_uid
        });
        let absence = if generated.rels.iter().all(|other| other.3 != *def_uid) {
            absence_reason(def_uid, dump_content)
        } else {
            None
        };
        let reason = if same_wiring {
            "the same relation is stated under another Rel UID".to_string()
        } else if let Some(reason) = absence {
            reason
        } else if let Some(end) = [uid1, uid2].iter().find(|end| {
            reference.objects.contains_key(end.as_str())
                && !publishes(generated, &generated_pipelines, reference, end.as_str())
        }) {
            format!("its end {end} is not published by this run")
        } else {
            diff.rels_not_in_generated += 1;
            missing_rels.push(format!("missing rel {rel:?}"));
            continue;
        };
        diff.rels_excused_by_design += 1;
        *excused_rels.entry((def_uid.clone(), reason)).or_insert(0) += 1;
    }

    // Values on exactly the objects the generated direction compares: a UID
    // the generated document publishes under any class, plus `PIDPipeline`
    // objects paired by `IObject.Name` when the generated document lacks the
    // UID entirely.
    for &uid in &uids {
        let (class, values) = &reference.objects[uid];
        let generated_values = match generated.objects.get(uid) {
            Some((_, found)) => Some(found),
            None if class == "PIDPipeline" => {
                let name = values.get("IObject.Name").cloned().unwrap_or_default();
                generated_pipelines.get(&name).map(|(_, found)| found)
            }
            None => None,
        };
        let Some(generated_values) = generated_values else {
            continue;
        };
        let own_name = values
            .get("IObject.Name")
            .map(String::as_str)
            .unwrap_or_default();
        for (key, value) in values {
            if generated_values.contains_key(key) {
                continue;
            }
            if key == "IObject.Name" && vendor_composes_name(class, &named_classes) {
                // Exclusion source 5 — the vendor's composed object name:
                // this run publishes the object's UID but states no
                // `IObject.Name` for it. Measured on this document: the
                // class's emitted rows never carry `IObject.Name` for
                // `PIDPipingConnector` (0 of 169) and `PIDPipingComponent`
                // (0 of 134); for `PIDInlineInstrument` only the
                // inline-component rows (valves) carry one, the
                // inline-instrument rows do not. The differing direction is
                // untouched — `compare_values` still reports a stated
                // `IObject.Name` that disagrees with the reference's.
                diff.values_excused_by_design += 1;
                *composed_names.entry(class.as_str()).or_insert(0) += 1;
                continue;
            }
            let (interface, attribute) = key.rsplit_once('.').unwrap_or((key.as_str(), ""));
            if let Some(excluded) = catalog::excluded_attribute(interface, attribute) {
                diff.values_excused_by_design += 1;
                excused_members
                    .entry(key.clone())
                    .or_insert((0, excluded.class, excluded.reason))
                    .0 += 1;
                continue;
            }
            if !own_name.is_empty()
                && interface == own_name
                && tag_value_matches(attribute, value, uid, values)
            {
                diff.values_excused_by_design += 1;
                tag_named += 1;
                continue;
            }
            diff.values_not_in_generated += 1;
            missing_values.push(format!(
                "missing value {class} {uid} {key}: {value:?} not in generated"
            ));
        }
    }

    diff.findings.extend(missing_objects);
    diff.findings.extend(missing_rels);
    diff.findings.extend(missing_values);

    for (class, (count, reason)) in excused_objects {
        diff.exclusions
            .push(format!("excused object {class} {count} — {reason}"));
    }
    for (member, (count, class, reason)) in excused_members {
        diff.exclusions.push(format!(
            "excused value {member} {count} — excluded class \"{class}\": {reason}"
        ));
    }
    for (class, count) in composed_names {
        diff.exclusions.push(format!(
            "excused value {class}.IObject.Name {count} — the vendor's composed tag: the vendor \
             composes the object's tag in the reference (BV1001406A, -PG-1001105-F9--F9--, \
             FT-753); this run publishes those rows with their UID and the interfaces it derives \
             (IPipeCrossSectionItem, IPipingSpecifiedItem, INamedPipingConnector) and states \
             IObject.Name only for the classes whose tag it reproduces from the sheet's labels; \
             a reference-only name on a row this run publishes unnamed is the vendor's own \
             composition, not content this run lost."
        ));
    }
    if tag_named > 0 {
        diff.exclusions.push(format!(
            "excused value (tag-named elements) {tag_named} — the reference restates each \
             object's own tag as a child element named after it (UID / Name / Description); \
             this line states the same values in IObject"
        ));
    }
    for ((def_uid, reason), count) in excused_rels {
        diff.exclusions
            .push(format!("excused rel {def_uid} {count} — {reason}"));
    }
}

/// The tag-named duplicate element's `<attribute>` restates the block's own
/// `IObject.<attribute>`: `UID` is the block's map key, `Name` and
/// `Description` its own values. Anything else is not a duplicate.
fn tag_value_matches(
    attribute: &str,
    value: &str,
    uid: &str,
    values: &BTreeMap<String, String>,
) -> bool {
    let key = match attribute {
        "UID" => return value == uid,
        "Name" => "IObject.Name",
        "Description" => "IObject.Description",
        _ => return false,
    };
    values.get(key).map(String::as_str) == Some(value)
}

/// Why a run that states no `def_uid` relation at all is excused for it, or
/// `None` when that absence is a loss: a class the `.pid` alone yields, or a
/// class only the exp dump supplies while the run carries the dump's content.
fn absence_reason(def_uid: &str, dump_content: bool) -> Option<String> {
    if FILE_RELATIONS.contains(&def_uid) {
        None
    } else if DUMP_RELATIONS.contains(&def_uid) {
        if dump_content {
            None
        } else {
            Some(format!(
                "only the exp dump states {def_uid}; this run has none of its content"
            ))
        }
    } else {
        Some(format!("this line writes no {def_uid} relation"))
    }
}

/// Whether the generated document publishes the reference object `end`: it
/// carries the UID, or `end` is a `PIDPipeline` that pairs with a generated
/// one by `IObject.Name` (a synthesized pipeline carries its own UID).
fn publishes(
    generated: &PublishBlocks,
    generated_pipelines: &HashMap<String, (String, BTreeMap<String, String>)>,
    reference: &PublishBlocks,
    end: &str,
) -> bool {
    if generated.objects.contains_key(end) {
        return true;
    }
    let Some((class, values)) = reference.objects.get(end) else {
        return false;
    };
    class == "PIDPipeline"
        && values
            .get("IObject.Name")
            .is_some_and(|name| generated_pipelines.contains_key(name))
}

/// Whether a reference-only `IObject.Name` on an object this run publishes
/// unnamed is the vendor's own composition: this line never names `class`
/// (it names only the classes `pid_load::composes_name` covers and the ports
/// the exp dump numbers), or it names other objects of `class` in this run,
/// so this row lacked a part of the rule.
fn vendor_composes_name(class: &str, named_classes: &HashSet<&str>) -> bool {
    let line_names = pid_load::composes_name(class)
        || matches!(
            catalog::pid_file_cannot_state(class),
            Some((_, PidTagRole::Port))
        );
    !line_names || named_classes.contains(class)
}
