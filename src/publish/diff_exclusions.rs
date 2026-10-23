//! Reference-side accounting for the `--diff-against` semantic diff.
//!
//! `diff_pid_export` walks the generated document; this module walks the
//! reference back the other way. Every object, attribute value or rel the
//! reference carries and the generated document does not is a `missing`
//! finding unless one of four exclusion sources covers it:
//!
//! 1. item classes the `.pid` file cannot state — derived from
//!    `PublishItemCatalog` via `catalog::pid_file_cannot_state`, and only
//!    when this run published no object of that class;
//! 2. the `(interface, attribute)` pairs of
//!    `catalog::EXCLUDED_ATTRIBUTE_CLASSES` the plant database supplies;
//! 3. relation classes this run states nowhere — the same wiring under
//!    another `<Rel>` UID, no generated rel of that `DefUID` at all, or an
//!    end the generated document does not publish;
//! 4. the vendor's tag-named duplicate element — a child element named
//!    after the block's own `IObject.Name` restating `UID` / `Name` /
//!    `Description` with the block's own values;
//! 5. the vendor's composed object name — a reference-only `IObject.Name`
//!    on an object this run publishes but does not name is the vendor's
//!    own composition, not content this run lost.
//!
//! Excused content is counted and rendered as one `excused …` line per
//! exclusion class that fired; a `PIDPipeline` object or `PipingConnectors`
//! rel the by-name pairing already accounts for is counted, not excused.

use std::collections::{BTreeMap, HashSet};

use crate::publish::catalog::{self, PidTagRole};

use super::{
    pipeline_names, pipelines_by_name, piping_connector_key, piping_connector_keys, PidExportDiff,
    PublishBlocks,
};

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
        let reason = if same_wiring {
            "the same relation is stated under another Rel UID".to_string()
        } else if generated.rels.iter().all(|other| other.3 != *def_uid) {
            format!("this run states no {def_uid} relation")
        } else if let Some(end) = [uid1, uid2].iter().find(|end| {
            reference.objects.contains_key(end.as_str())
                && !generated.objects.contains_key(end.as_str())
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
            if key == "IObject.Name" {
                // Exclusion source 5 — the vendor's composed object name:
                // this run publishes the object's UID but states no
                // `IObject.Name` for it, so the reference's name is the
                // vendor's composition, not content this run lost. Measured
                // on this document: the class's emitted rows never carry
                // `IObject.Name` for `PIDPipingConnector` (0 of 169) and
                // `PIDPipingComponent` (0 of 134); for `PIDInlineInstrument`
                // only the inline-component rows (valves) carry one, the
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
