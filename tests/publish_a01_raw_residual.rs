//! A01 raw-file residual gates.
//!
//! The A01 delivery-contract test in `publish_xml_cli.rs` proves
//! the generated `_Data.xml` matches the bundled SmartPlant
//! reference once publish-time synthetic slots are normalized.
//! This file narrows the remaining gap further:
//!
//! 1. Assert that after masking ONLY the three known synthetic
//!    residual classes, the generated raw XML matches the
//!    reference byte-for-byte.
//! 2. Provide an ignored probe that logs candidate connector-UID
//!    seeds against the reference for manual reverse-engineering.
//!
//! The three currently-known synthetic residual classes are:
//!
//! * `PIDPipingConnector` base UID (and its derived `.1` / `.2`
//!   / `.PPT` children),
//! * `<Rel><IObject UID="..."/>` synthetic 32-hex values,
//! * `PIDRepresentation` `GraphicOID` numbering.

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::publish::sqlite_load::open_readonly;
use rusqlite::Connection;

mod common;
use common::{generate_a01_xml, load_reference_a01_xml, SQLITE_PATH};

#[test]
fn a01_raw_xml_diff_is_limited_to_known_synthetic_residual_slots() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    assert_eq!(
        normalize_a01_known_raw_residuals(&generated_xml),
        normalize_a01_known_raw_residuals(&reference_xml),
        "after masking only the known A01 synthetic residual slots \
         (connector-family UID, Rel IObject UID, GraphicOID), the \
         generated raw XML must match the SmartPlant reference exactly",
    );
}

#[test]
#[ignore = "manual reverse-engineering probe for connector synthetic UID derivation"]
fn connector_uid_reverse_engineering_probe_logs_current_candidates_against_reference() {
    if !Path::new(SQLITE_PATH).exists() {
        eprintln!("skipping: SQLite fixture {SQLITE_PATH} not found");
        return;
    }
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let reference_connector_uid = first_iobject_uid_for_tag(&reference_xml, "PIDPipingConnector")
        .expect("A01 reference should contain PIDPipingConnector/IObject UID");
    let generated_connector_uid = first_iobject_uid_for_tag(&generated_xml, "PIDPipingConnector")
        .expect("generated A01 XML should contain PIDPipingConnector/IObject UID");
    let pipeline_uid = first_iobject_uid_for_tag(&reference_xml, "PIDPipeline")
        .expect("A01 reference should contain PIDPipeline/IObject UID");
    let connector_item_tag = first_attr_for_tag(&reference_xml, "PIDPipingConnector", "ItemTag")
        .expect("A01 reference should carry connector ItemTag");

    let conn = open_readonly(Path::new(SQLITE_PATH)).expect("open TEST02 sqlite");
    let pipe_run_row = load_row_map(&conn, "T_PipeRun", &pipeline_uid);
    let plant_item_row = load_row_map(&conn, "T_PlantItem", &pipeline_uid);

    eprintln!("--- A01 connector UID reverse-engineering probe ---");
    eprintln!("Reference connector UID : {reference_connector_uid}");
    eprintln!("Current writer UID      : {generated_connector_uid}");
    eprintln!("Pipeline UID            : {pipeline_uid}");
    eprintln!("Connector ItemTag       : {connector_item_tag}");
    eprintln!("T_PlantItem row         : {plant_item_row:?}");
    eprintln!("T_PipeRun row           : {pipe_run_row:?}");

    let mut candidates = BTreeMap::new();
    for seed in candidate_seeds(&pipeline_uid, &connector_item_tag, &plant_item_row, &pipe_run_row) {
        for (algo, value) in derive_candidate_values(&seed) {
            candidates.insert(format!("{algo} :: {seed}"), value);
        }
    }
    for (label, value) in &candidates {
        let marker = if value == &reference_connector_uid {
            "  <== MATCH"
        } else if value == &generated_connector_uid {
            "  <== CURRENT"
        } else {
            ""
        };
        eprintln!("{label} => {value}{marker}");
    }

    assert!(
        !reference_connector_uid.is_empty() && !generated_connector_uid.is_empty(),
        "probe sanity: connector UIDs must be non-empty",
    );
}

fn normalize_a01_known_raw_residuals(xml: &str) -> String {
    let mut current_block: Option<&'static str> = None;
    let mut connector_uid: Option<String> = None;
    let mut graphic_count = 0usize;
    let mut rel_count = 0usize;
    let mut out = Vec::new();

    for raw in xml.replace("\r\n", "\n").lines() {
        let trimmed = raw.trim_start();
        match trimmed {
            "<PIDPipingConnector>" => current_block = Some("PIDPipingConnector"),
            "<PIDPipingPort>" => current_block = Some("PIDPipingPort"),
            "<PIDProcessPoint>" => current_block = Some("PIDProcessPoint"),
            "<PIDRepresentation>" => current_block = Some("PIDRepresentation"),
            "<Rel>" => current_block = Some("Rel"),
            _ => {}
        }

        let mut line = raw.to_string();

        if current_block == Some("PIDPipingConnector") && trimmed.starts_with("<IObject ") {
            if let Some(uid) = extract_attr(&line, "UID") {
                connector_uid = Some(uid.clone());
                line = replace_attr_value(&line, "UID", "@CONNECTOR@");
            }
        }

        if let Some(base) = connector_uid.as_deref() {
            let derived = [
                (format!("{base}.1"), "@PORT1@"),
                (format!("{base}.2"), "@PORT2@"),
                (format!("{base}.PPT"), "@PROCESS_POINT@"),
                (base.to_string(), "@CONNECTOR@"),
            ];
            for (from, to) in derived {
                line = line.replace(&from, to);
            }
        }

        if trimmed.starts_with("<IDrawingRepresentation ") {
            graphic_count += 1;
            line = replace_attr_value(&line, "GraphicOID", &format!("@GRAPHIC{graphic_count}@"));
        }

        if current_block == Some("Rel") && trimmed.starts_with("<IObject ") {
            rel_count += 1;
            line = replace_attr_value(&line, "UID", &format!("@REL{rel_count}@"));
        }

        out.push(line);

        match trimmed {
            "</PIDPipingConnector>"
            | "</PIDPipingPort>"
            | "</PIDProcessPoint>"
            | "</PIDRepresentation>"
            | "</Rel>" => current_block = None,
            _ => {}
        }
    }

    let mut normalized = out.join("\n");
    normalized.push('\n');
    normalized
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = line.find(&needle)? + needle.len();
    let end = start + line[start..].find('"')?;
    Some(line[start..end].to_string())
}

fn replace_attr_value(line: &str, attr: &str, replacement: &str) -> String {
    let needle = format!(r#"{attr}=""#);
    let Some(start) = line.find(&needle) else {
        return line.to_string();
    };
    let value_start = start + needle.len();
    let Some(rel_end) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + rel_end;
    format!(
        "{}{}{}",
        &line[..value_start],
        replacement,
        &line[value_end..]
    )
}

fn first_iobject_uid_for_tag(xml: &str, tag: &str) -> Option<String> {
    collect_tag_blocks(xml, tag)
        .into_iter()
        .find_map(|block| extract_attr_from_first_iobject(block, "UID"))
}

fn first_attr_for_tag(xml: &str, tag: &str, attr: &str) -> Option<String> {
    collect_tag_blocks(xml, tag)
        .into_iter()
        .find_map(|block| extract_attr_from_first_iobject(block, attr))
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

fn extract_attr_from_first_iobject(block: &str, attr: &str) -> Option<String> {
    block.lines()
        .find(|line| line.trim_start().starts_with("<IObject "))
        .and_then(|line| extract_attr(line, attr))
}

fn load_row_map(conn: &Connection, table_name: &str, uid: &str) -> BTreeMap<String, String> {
    let sql = format!(
        "SELECT * FROM \"{}\" WHERE SP_ID = ?1",
        table_name.replace('"', "\"\"")
    );
    let mut stmt = conn.prepare(&sql).expect("prepare row-map query");
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query([uid]).expect("query row-map");
    let Some(row) = rows.next().expect("read row-map result") else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for (idx, name) in col_names.iter().enumerate() {
        let value: Option<String> = row.get(idx).expect("read text cell");
        if let Some(v) = value {
            if !v.is_empty() {
                out.insert(name.clone(), v);
            }
        }
    }
    out
}

fn candidate_seeds(
    pipeline_uid: &str,
    connector_item_tag: &str,
    plant_item_row: &BTreeMap<String, String>,
    pipe_run_row: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut seeds = vec![
        pipeline_uid.to_string(),
        pipeline_uid.to_lowercase(),
        pipeline_uid.to_uppercase(),
        format!("{pipeline_uid}/piping-connector"),
        format!("{pipeline_uid}-CNX"),
        format!("{pipeline_uid}.CNX"),
        format!("{pipeline_uid}CNX"),
        connector_item_tag.to_string(),
    ];

    for key in ["Name", "TagPrefix", "TagSequenceNumber", "TagSuffix"] {
        if let Some(v) = plant_item_row.get(key).or_else(|| pipe_run_row.get(key)) {
            seeds.push(v.clone());
        }
    }
    if let (Some(prefix), Some(seq), Some(suffix)) = (
        pipe_run_row.get("TagPrefix"),
        pipe_run_row.get("TagSequenceNumber"),
        pipe_run_row.get("TagSuffix"),
    ) {
        seeds.push(format!("{prefix}{seq}{suffix}"));
        seeds.push(format!("{prefix}-{seq}-{suffix}"));
        seeds.push(format!("{prefix}/{seq}/{suffix}"));
    }
    seeds.sort();
    seeds.dedup();
    seeds
}

fn derive_candidate_values(seed: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "uuid5-oid",
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, seed.as_bytes())
                .simple()
                .to_string()
                .to_uppercase(),
        ),
        (
            "uuid5-url",
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, seed.as_bytes())
                .simple()
                .to_string()
                .to_uppercase(),
        ),
    ]
}
