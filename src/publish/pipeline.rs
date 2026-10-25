//! Pipelines synthesized from `T_PIPERUN`.
//!
//! Plan `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md`
//! N9-2. `T_PIPELINE` is empty. A published `PIDPipingConnector` whose
//! `TAGSEQUENCENO` is not empty becomes a member of the pipeline named
//!
//! `{nominal}-{fluid}-{sequence}-{class}[-{insulation}]`
//!
//! Nominal diameter below 5000 is `{n} mm`. At or above 5000,
//! `(code - 5000) / 32` is a mixed number of inches with an inch mark
//! (`1"`, `3/4"`, `1-1/2"`). An empty column stays an empty slot, so a
//! missing diameter is `-PG-1001105-F9`. A fluid code that is not in
//! codelist 19 is left out of the name and counted. Runs that share a
//! name are one pipeline. The UID is a UUID v5: the namespace is the
//! fixed pid-parse pipeline root mixed with the plant, and the name is
//! the pipeline name. Same plant and same name always yield the same UID.

use std::collections::BTreeMap;

use uuid::Uuid;

use super::oracle_exp::{ExpError, ExpTables, PipeRunRow};
use super::pid_load::{PidPipeline, PidPublishDrawing, PipelineSynthesis};
use crate::backup::{fluid_system_guid, DataMap};

/// DNS namespace UUID from RFC 4122. The pipeline root is version 5 of
/// this namespace over the name `pid-parse/pipeline`.
const DNS_NAMESPACE: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

/// Hang synthesized pipelines on connectors this drawing already publishes.
///
/// `datamap` supplies `FluidSystem`. Without it the pipelines are still
/// written and `FluidSystem` stays empty.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when `T_PIPERUN` or `CODELISTS` is not the
/// pinned shape.
pub fn attach_pipelines(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
    plant: &str,
    datamap: Option<&DataMap>,
) -> Result<(), ExpError> {
    drawing.pipelines_attempted = true;
    let runs = tables.pipe_runs()?;
    let fluids = tables.codelist_short_text(19)?;
    let by_id: BTreeMap<&str, &PipeRunRow> =
        runs.iter().map(|run| (run.id_key.as_str(), run)).collect();
    let mut synthesis = PipelineSynthesis::default();
    let mut grouped: BTreeMap<String, PidPipeline> = BTreeMap::new();
    for item in drawing
        .items
        .iter()
        .filter(|item| item.class == Some("PIDPipingConnector"))
    {
        let key = super::oracle_exp::backup_uid_key(&item.uid);
        let Some(run) = by_id.get(key.as_str()) else {
            synthesis.not_in_piperun += 1;
            continue;
        };
        if run.tag_sequence.is_empty() {
            synthesis.without_sequence += 1;
            continue;
        }
        let (fluid, missing) = fluid_slot(&fluids, run.fluid_code);
        if missing {
            synthesis.fluid_code_missing += 1;
        }
        let name = pipeline_name(run, fluid.as_deref());
        if let Some(entry) = grouped.get_mut(&name) {
            entry.connector_uids.push(item.uid.clone());
            continue;
        }
        let fluid_system = match (datamap, run.fluid_system) {
            (Some(map), Some(code)) => match fluid_system_guid(map, code) {
                Some(guid) => Some(guid.to_string()),
                None => {
                    synthesis.fluid_system_unmapped += 1;
                    None
                }
            },
            _ => None,
        };
        grouped.insert(
            name.clone(),
            PidPipeline {
                uid: pipeline_uid(plant, &name),
                name,
                fluid_system,
                connector_uids: vec![item.uid.clone()],
            },
        );
    }
    drawing.pipeline_synthesis = synthesis;
    drawing.pipelines = grouped.into_values().collect();
    Ok(())
}

/// The pipeline name for one run.
///
/// `fluid` is `None` when the code is not in the codelist: that slot is
/// omitted. `Some("")` keeps the empty slot.
#[must_use]
pub fn pipeline_name(run: &PipeRunRow, fluid: Option<&str>) -> String {
    let mut parts = vec![nominal_diameter(run.nominal_diameter)];
    if let Some(fluid) = fluid {
        parts.push(fluid.to_string());
    }
    parts.push(run.tag_sequence.clone());
    parts.push(run.materials_class.clone());
    let mut name = parts.join("-");
    if !run.insulation_spec.is_empty() {
        name.push('-');
        name.push_str(&run.insulation_spec);
    }
    name
}

/// `Some(text)` keeps the slot, including an empty one. `None` means the
/// code is not in the list and the slot is omitted. The bool is that miss.
fn fluid_slot(
    fluids: &std::collections::HashMap<i64, String>,
    code: Option<i64>,
) -> (Option<String>, bool) {
    match code {
        None => (Some(String::new()), false),
        Some(code) => match fluids.get(&code) {
            Some(text) => (Some(text.clone()), false),
            None => (None, true),
        },
    }
}

/// Millimetres, or a mixed number of inches with an inch mark.
#[must_use]
pub fn nominal_diameter(code: Option<i64>) -> String {
    let Some(code) = code else {
        return String::new();
    };
    if (1..5000).contains(&code) {
        return format!("{code} mm");
    }
    if code < 5000 {
        return String::new();
    }
    let ticks = code - 5000;
    let whole = ticks / 32;
    let mut numerator = ticks % 32;
    let mut denominator = 32i64;
    let divisor = gcd(numerator, denominator);
    numerator /= divisor;
    denominator /= divisor;
    if numerator == 0 {
        format!("{whole}\"")
    } else if whole == 0 {
        format!("{numerator}/{denominator}\"")
    } else {
        format!("{whole}-{numerator}/{denominator}\"")
    }
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a.abs().max(1)
}

/// 32 uppercase hex digits. Stable for one plant and one pipeline name.
#[must_use]
pub fn pipeline_uid(plant: &str, name: &str) -> String {
    let dns = Uuid::parse_str(DNS_NAMESPACE).unwrap_or_else(|_| Uuid::nil());
    let root = Uuid::new_v5(&dns, b"pid-parse/pipeline");
    let plant_namespace = Uuid::new_v5(&root, plant.as_bytes());
    let id = Uuid::new_v5(&plant_namespace, name.as_bytes());
    let mut buffer = [0u8; 32];
    id.simple().encode_upper(&mut buffer).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(nominal: Option<i64>, sequence: &str, class: &str, insulation: &str) -> PipeRunRow {
        PipeRunRow {
            id: "RUN".to_string(),
            id_key: "RUN".to_string(),
            nominal_diameter: nominal,
            fluid_code: None,
            materials_class: class.to_string(),
            tag_sequence: sequence.to_string(),
            fluid_system: None,
            insulation_spec: insulation.to_string(),
        }
    }

    #[test]
    fn names_cover_millimetres_inches_empty_slots_and_insulation() {
        assert_eq!(
            pipeline_name(&run(Some(25), "1901201", "B9", ""), Some("CI")),
            "25 mm-CI-1901201-B9"
        );
        assert_eq!(
            pipeline_name(&run(Some(5032), "1001901", "B9", ""), Some("CI")),
            "1\"-CI-1001901-B9"
        );
        assert_eq!(
            pipeline_name(&run(Some(5024), "1", "B9", ""), Some("CI")),
            "3/4\"-CI-1-B9"
        );
        assert_eq!(
            pipeline_name(&run(Some(5048), "1001403", "B5", "EP40"), Some("PC")),
            "1-1/2\"-PC-1001403-B5-EP40"
        );
        assert_eq!(
            pipeline_name(&run(None, "1001105", "F9", ""), Some("PG")),
            "-PG-1001105-F9"
        );
        assert_eq!(
            pipeline_name(&run(Some(100), "1001105", "F9", "P40"), Some("PG")),
            "100 mm-PG-1001105-F9-P40"
        );
        assert_eq!(
            pipeline_name(&run(Some(25), "1901201", "B9", ""), None),
            "25 mm-1901201-B9"
        );
    }

    #[test]
    fn the_uid_is_stable_for_one_plant_and_one_name() {
        let first = pipeline_uid("P01", "25 mm-CI-1901201-B9");
        let second = pipeline_uid("P01", "25 mm-CI-1901201-B9");
        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
        assert!(first.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(first, first.to_uppercase());
        assert_ne!(first, pipeline_uid("SQPlant", "25 mm-CI-1901201-B9"));
    }
}
