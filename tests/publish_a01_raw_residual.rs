//! A01 raw-file residual gates.
//!
//! The A01 delivery-contract test in `publish_xml_cli.rs` proves
//! the generated `_Data.xml` matches the bundled SmartPlant
//! reference once publish-time synthetic slots are normalized.
//! This file narrows the remaining gap further:
//!
//! 1. Assert that after masking ONLY the three evidence-guarded
//!    synthetic residual classes, the generated raw XML matches the
//!    reference byte-for-byte.
//! 2. Provide ignored probes that log candidate connector-UID
//!    seeds and scan the full MDF table inventory for residual
//!    source values.
//!
//! A39's full Rust MDF probe scans every table name exposed by
//! `oxidized-mdf` and confirms these values are not present in the
//! TEST02 MDF table inventory or MDF raw text / UUID byte forms. The
//! three evidence-guarded synthetic residual classes are:
//!
//! * `PIDPipingConnector` base UID (and its derived `.1` / `.2`
//!   / `.PPT` children),
//! * `<Rel><IObject UID="..."/>` synthetic 32-hex values,
//! * `PIDRepresentation` `GraphicOID` numbering.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Mutex;

use oxidized_mdf::{MdfDatabase, Row, Value};
use pid_parse::publish::open_mdf_as_sqlite;
use rusqlite::Connection;

mod common;
use common::{
    generate_a01_xml, load_reference_a01_xml, normalize_a01_synthetic_slots,
    A01SyntheticMaskOptions, A01_MDF_PATH,
};

static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

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
        normalize_a01_synthetic_slots(&generated_xml, A01SyntheticMaskOptions::ALL),
        normalize_a01_synthetic_slots(&reference_xml, A01SyntheticMaskOptions::ALL),
        "after masking only the A39 evidence-guarded A01 synthetic residual slots \
         (connector-family UID, Rel IObject UID, GraphicOID), the \
         generated raw XML must match the SmartPlant reference exactly",
    );
}

#[test]
fn connector_family_uid_is_the_only_evidence_guarded_gap_after_masking_rel_and_graphicoid() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let without_connector = A01SyntheticMaskOptions {
        connector_family: false,
        rel_iobject_uid: true,
        graphic_oid: true,
    };
    assert_ne!(
        normalize_a01_synthetic_slots(&generated_xml, without_connector),
        normalize_a01_synthetic_slots(&reference_xml, without_connector),
        "after masking Rel IObject UID + GraphicOID, the remaining raw A01 gap should still be the connector-family publish-time synthetic UID strategy"
    );
    assert_eq!(
        normalize_a01_synthetic_slots(&generated_xml, A01SyntheticMaskOptions::ALL),
        normalize_a01_synthetic_slots(&reference_xml, A01SyntheticMaskOptions::ALL),
        "once connector-family UID is also masked, the A01 raw document should close again"
    );
}

#[test]
fn rel_iobject_uid_is_the_only_evidence_guarded_gap_after_masking_connector_and_graphicoid() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let without_rel = A01SyntheticMaskOptions {
        connector_family: true,
        rel_iobject_uid: false,
        graphic_oid: true,
    };
    assert_ne!(
        normalize_a01_synthetic_slots(&generated_xml, without_rel),
        normalize_a01_synthetic_slots(&reference_xml, without_rel),
        "after masking connector-family UID + GraphicOID, the remaining raw A01 gap should still be Rel IObject publish-time synthetic UID synthesis"
    );
    assert_eq!(
        normalize_a01_synthetic_slots(&generated_xml, A01SyntheticMaskOptions::ALL),
        normalize_a01_synthetic_slots(&reference_xml, A01SyntheticMaskOptions::ALL),
        "once Rel IObject UID is also masked, the A01 raw document should close again"
    );
}

#[test]
fn representation_graphicoid_remains_evidence_guarded_after_masking_connector_and_rel_uid() {
    let Some(generated_result) = generate_a01_xml() else {
        return;
    };
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let generated_xml = generated_result.expect("writer should succeed on A01");

    let without_graphic = A01SyntheticMaskOptions {
        connector_family: true,
        rel_iobject_uid: true,
        graphic_oid: false,
    };
    assert_ne!(
        normalize_a01_synthetic_slots(&generated_xml, without_graphic),
        normalize_a01_synthetic_slots(&reference_xml, without_graphic),
        "after masking connector-family UID + Rel IObject UID, the remaining raw A01 gap should still be GraphicOID publish-time remap"
    );
    assert_eq!(
        normalize_a01_synthetic_slots(&generated_xml, A01SyntheticMaskOptions::ALL),
        normalize_a01_synthetic_slots(&reference_xml, A01SyntheticMaskOptions::ALL),
        "once GraphicOID is also masked, the A01 raw document should close again"
    );
}

#[test]
#[ignore = "manual reverse-engineering probe for connector synthetic UID derivation"]
fn connector_uid_reverse_engineering_probe_logs_current_candidates_against_reference() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
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

    let conn = open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("open TEST02 MDF");
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

#[test]
#[ignore = "manual MDF full-table probe for A01 raw residual source values"]
fn raw_residual_source_probe_scans_full_mdf_and_confirms_synthetic_slots_are_absent() {
    if !Path::new(A01_MDF_PATH).exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return;
    }
    let Some(reference_xml) = load_reference_a01_xml() else {
        return;
    };
    let conn = open_mdf_as_sqlite(Path::new(A01_MDF_PATH)).expect("open TEST02 MDF");

    let mut probes = Vec::new();
    if let Some(uid) = first_iobject_uid_for_tag(&reference_xml, "PIDPipingConnector") {
        probes.push(ResidualProbe::new("connector_iobject_uid", uid));
    }
    if let Some(uid) = first_iobject_uid_for_tag(&reference_xml, "Rel") {
        probes.push(ResidualProbe::new("first_rel_iobject_uid", uid));
    }
    if let Some(graphic_oid) = first_drawing_representation_graphic_oid(&reference_xml) {
        probes.push(ResidualProbe::new(
            "first_representation_graphic_oid",
            graphic_oid,
        ));
    }

    eprintln!("--- A01 raw residual MDF source-value probe ---");
    eprintln!("staged publish-table scan:");
    for probe in &probes {
        let occurrences = find_text_occurrences(&conn, probe);
        eprintln!("{}: {}", probe.label, probe.value);
        if occurrences.is_empty() {
            eprintln!("  not present in currently staged publish tables");
        } else {
            for hit in occurrences {
                eprintln!("  {hit}");
            }
        }
    }

    let full_scan =
        scan_full_mdf_for_residual_values(Path::new(A01_MDF_PATH), &probes).expect("scan full MDF");
    let raw_byte_hits =
        scan_mdf_bytes_for_residual_values(Path::new(A01_MDF_PATH), &probes).expect("scan MDF bytes");
    eprintln!("full MDF scan:");
    eprintln!(
        "  tables_discovered={} tables_read={} tables_skipped={} columns_scanned={} rows_scanned={}",
        full_scan.stats.tables_discovered,
        full_scan.stats.tables_read,
        full_scan.stats.tables_skipped,
        full_scan.stats.columns_scanned,
        full_scan.stats.rows_scanned
    );
    if !full_scan.skipped_tables.is_empty() {
        eprintln!("  skipped tables:");
        for skipped in &full_scan.skipped_tables {
            eprintln!("    {skipped}");
        }
    }
    for probe in &probes {
        let hits = full_scan
            .hits
            .iter()
            .filter(|hit| hit.label == probe.label)
            .collect::<Vec<_>>();
        eprintln!("{}: {}", probe.label, probe.value);
        if hits.is_empty() {
            eprintln!("  not present in full Rust MDF scan");
        } else {
            for hit in hits {
                eprintln!(
                    "  {}.{} row={} {} stored_value={}",
                    hit.table, hit.column, hit.row_number, hit.row_identity, hit.stored_value
                );
            }
        }
    }
    eprintln!("raw MDF byte scan:");
    for probe in &probes {
        let hits = raw_byte_hits
            .iter()
            .filter(|hit| hit.label == probe.label)
            .collect::<Vec<_>>();
        eprintln!("{}: {}", probe.label, probe.value);
        if hits.is_empty() {
            eprintln!("  no ASCII / UTF-16LE / UUID byte-form hits in MDF file");
        } else {
            for hit in hits {
                eprintln!("  {} offset=0x{:X}", hit.variant, hit.offset);
            }
        }
    }

    assert!(
        !probes.is_empty(),
        "probe sanity: A01 reference must expose residual values"
    );
    assert!(
        full_scan.stats.tables_discovered > 0 && full_scan.stats.rows_scanned > 0,
        "probe sanity: full Rust MDF scan must read a non-empty table inventory"
    );
    assert_eq!(
        full_scan.stats.tables_discovered, 128,
        "A39 probe expects the TEST02 MDF table inventory to stay stable"
    );
    assert_eq!(
        full_scan.stats.tables_skipped, 0,
        "A39 full Rust MDF scan must not skip tables; skipped={:#?}",
        full_scan.skipped_tables
    );
    assert!(
        full_scan.hits.is_empty(),
        "A39 evidence changed: at least one raw residual value is present in the MDF and should be wired from source instead of kept synthetic: {:#?}",
        full_scan.hits
    );
    let raw_uid_hits = raw_byte_hits
        .iter()
        .filter(|hit| hit.label != "first_representation_graphic_oid")
        .collect::<Vec<_>>();
    assert!(
        raw_uid_hits.is_empty(),
        "A39 byte evidence changed: a residual UID exists in MDF raw bytes outside the decoded table scan and needs parser/source investigation: {raw_uid_hits:#?}"
    );
}

#[derive(Debug)]
struct ResidualProbe {
    label: &'static str,
    value: String,
    comparable_values: BTreeSet<String>,
}

impl ResidualProbe {
    fn new(label: &'static str, value: String) -> Self {
        let comparable_values = comparable_variants(&value);
        Self {
            label,
            value,
            comparable_values,
        }
    }

    fn matches(&self, stored_value: &str) -> bool {
        comparable_variants(stored_value)
            .iter()
            .any(|variant| self.comparable_values.contains(variant))
    }
}

#[derive(Debug, Default)]
struct FullMdfScanStats {
    tables_discovered: usize,
    tables_read: usize,
    tables_skipped: usize,
    columns_scanned: usize,
    rows_scanned: usize,
}

#[derive(Debug)]
struct ResidualHit {
    label: &'static str,
    table: String,
    column: String,
    row_number: usize,
    row_identity: String,
    stored_value: String,
}

#[derive(Debug)]
struct RawByteHit {
    label: &'static str,
    variant: String,
    offset: usize,
}

#[derive(Debug)]
struct FullMdfResidualScan {
    stats: FullMdfScanStats,
    hits: Vec<ResidualHit>,
    skipped_tables: Vec<String>,
}

#[derive(Debug, Default)]
struct TableResidualScan {
    columns_scanned: usize,
    rows_scanned: usize,
    hits: Vec<ResidualHit>,
}

fn scan_full_mdf_for_residual_values(
    path: &Path,
    probes: &[ResidualProbe],
) -> Result<FullMdfResidualScan, String> {
    let mut table_names = {
        let db = MdfDatabase::open(path)
            .map_err(|err| format!("open MDF for table inventory: {err}"))?;
        db.table_names()
    };
    table_names.sort();
    table_names.dedup();

    let mut stats = FullMdfScanStats {
        tables_discovered: table_names.len(),
        ..FullMdfScanStats::default()
    };
    let mut hits = Vec::new();
    let mut skipped_tables = Vec::new();

    for table_name in table_names {
        let table_scan = catch_table_scan_panic_silent(|| {
            scan_mdf_table_for_residual_values(
                path,
                &table_name,
                probes,
            )
        });
        match table_scan {
            Ok(Ok(table_scan)) => {
                if table_scan.columns_scanned == 0 {
                    continue;
                }
                stats.tables_read += 1;
                stats.columns_scanned += table_scan.columns_scanned;
                stats.rows_scanned += table_scan.rows_scanned;
                hits.extend(table_scan.hits);
            }
            Ok(Err(err)) => {
                stats.tables_skipped += 1;
                skipped_tables.push(format!("{table_name}: {err}"));
            }
            Err(_) => {
                stats.tables_skipped += 1;
                skipped_tables.push(format!("{table_name}: oxidized-mdf row parser panic"));
            }
        }
    }

    Ok(FullMdfResidualScan {
        stats,
        hits,
        skipped_tables,
    })
}

fn scan_mdf_bytes_for_residual_values(
    path: &Path,
    probes: &[ResidualProbe],
) -> Result<Vec<RawByteHit>, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("read MDF bytes: {err}"))?;
    let mut hits = Vec::new();
    for probe in probes {
        for (variant, needle) in raw_byte_needles(probe) {
            for offset in find_all_bytes(&bytes, &needle, 8) {
                hits.push(RawByteHit {
                    label: probe.label,
                    variant: variant.clone(),
                    offset,
                });
            }
        }
    }
    Ok(hits)
}

fn raw_byte_needles(probe: &ResidualProbe) -> Vec<(String, Vec<u8>)> {
    let mut needles = Vec::new();
    for variant in comparable_variants(&probe.value) {
        needles.push((format!("ascii:{variant}"), variant.as_bytes().to_vec()));
        needles.push((format!("utf16le:{variant}"), utf16le_bytes(&variant)));
    }

    if is_32_hex(&probe.value) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&probe.value) {
            needles.push(("uuid-rfc4122-bytes".to_string(), uuid.as_bytes().to_vec()));
            needles.push((
                "uuid-sqlserver-little-endian-bytes".to_string(),
                uuid.to_u128_le().to_le_bytes().to_vec(),
            ));
        }
    }

    needles.sort_by(|a, b| a.0.cmp(&b.0));
    needles.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    needles
}

fn utf16le_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn is_32_hex(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn find_all_bytes(haystack: &[u8], needle: &[u8], limit: usize) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut cursor = 0usize;
    while cursor + needle.len() <= haystack.len() && hits.len() < limit {
        let Some(offset) = haystack[cursor..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            break;
        };
        let absolute = cursor + offset;
        hits.push(absolute);
        cursor = absolute + 1;
    }
    hits
}

fn scan_mdf_table_for_residual_values(
    path: &Path,
    table_name: &str,
    probes: &[ResidualProbe],
) -> Result<TableResidualScan, String> {
    let mut db = MdfDatabase::open(path)
        .map_err(|err| format!("open MDF for table {table_name}: {err}"))?;
    let Some(columns) = db.column_names(table_name) else {
        return Ok(TableResidualScan::default());
    };
    if columns.is_empty() {
        return Ok(TableResidualScan::default());
    }

    let Some(rows) = db.rows(table_name) else {
        return Ok(TableResidualScan::default());
    };
    let mut out = TableResidualScan {
        columns_scanned: columns.len(),
        ..TableResidualScan::default()
    };
    for (row_index, row) in rows.enumerate() {
        let row_number = row_index + 1;
        out.rows_scanned += 1;
        let row_identity = row_identity(&row);
        for column in &columns {
            let Some(value) = row.value(column) else {
                continue;
            };
            let stored_value = value_to_probe_text(value);
            for probe in probes {
                if probe.matches(&stored_value) {
                    out.hits.push(ResidualHit {
                        label: probe.label,
                        table: table_name.to_string(),
                        column: column.clone(),
                        row_number,
                        row_identity: row_identity.clone(),
                        stored_value: stored_value.clone(),
                    });
                }
            }
        }
    }
    Ok(out)
}

fn catch_table_scan_panic_silent<F, T>(f: F) -> std::thread::Result<T>
where
    F: FnOnce() -> T,
{
    let _guard = PANIC_HOOK_LOCK.lock().expect("panic hook lock poisoned");
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(previous_hook);
    result
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = line.find(&needle)? + needle.len();
    let end = start + line[start..].find('"')?;
    Some(line[start..end].to_string())
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

fn first_drawing_representation_graphic_oid(xml: &str) -> Option<String> {
    collect_tag_blocks(xml, "PIDRepresentation")
        .into_iter()
        .find_map(|block| {
            block
                .lines()
                .find(|line| line.trim_start().starts_with("<IDrawingRepresentation "))
                .and_then(|line| extract_attr(line, "GraphicOID"))
        })
}

fn find_text_occurrences(conn: &Connection, probe: &ResidualProbe) -> Vec<String> {
    let mut out = Vec::new();
    let mut tables_stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("prepare sqlite_master query");
    let table_names = tables_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table names")
        .map(|r| r.expect("read table name"))
        .collect::<Vec<_>>();

    for table in table_names {
        let pragma = format!("PRAGMA table_info({})", quote_ident(&table));
        let mut col_stmt = conn.prepare(&pragma).expect("prepare table_info");
        let columns = col_stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query table_info")
            .map(|r| r.expect("read column name"))
            .collect::<Vec<_>>();

        for column in columns {
            let sql = format!(
                "SELECT rowid FROM {} WHERE {} = ?1 LIMIT 5",
                quote_ident(&table),
                quote_ident(&column)
            );
            let mut stmt = conn.prepare(&sql).expect("prepare value probe");
            let hits = stmt
                .query_map([probe.value.as_str()], |row| row.get::<_, i64>(0))
                .expect("query value probe")
                .map(|r| r.expect("read rowid"))
                .collect::<Vec<_>>();
            for rowid in hits {
                out.push(format!("{}.{} rowid={}", table, column, rowid));
            }
        }
    }
    out
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn comparable_variants(value: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.insert(value.to_string());
    out.insert(value.to_uppercase());
    out.insert(value.to_lowercase());
    let compact = value.replace('-', "");
    out.insert(compact.clone());
    out.insert(compact.to_uppercase());
    out.insert(compact.to_lowercase());
    out
}

fn value_to_probe_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::DateTime(dt) => dt.format("%Y/%-m/%-d %H:%M:%S").to_string(),
        Value::Binary(bytes) => bytes.iter().map(|b| format!("{b:02X}")).collect(),
        other => other.to_string(),
    }
}

fn row_identity(row: &Row) -> String {
    for key in [
        "SP_ID",
        "UID",
        "ID",
        "Name",
        "ItemTag",
        "SP_DrawingID",
        "SP_ModelItemID",
        "SP_RepresentationID",
    ] {
        let Some(value) = row.value(key) else {
            continue;
        };
        let text = value_to_probe_text(value);
        if !text.is_empty() {
            return format!("{key}={text}");
        }
    }
    "identity=(none)".to_string()
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

    for key in ["Name", "ItemTag", "TagPrefix", "TagSequenceNo", "TagSuffix"] {
        if let Some(v) = plant_item_row.get(key).or_else(|| pipe_run_row.get(key)) {
            seeds.push(v.clone());
        }
    }
    let tag_prefix = pipe_run_row
        .get("TagPrefix")
        .or_else(|| plant_item_row.get("TagPrefix"));
    let tag_sequence_no = pipe_run_row
        .get("TagSequenceNo")
        .or_else(|| plant_item_row.get("TagSequenceNo"));
    let tag_suffix = pipe_run_row
        .get("TagSuffix")
        .or_else(|| plant_item_row.get("TagSuffix"));

    if let (Some(prefix), Some(seq)) = (tag_prefix, tag_sequence_no) {
        seeds.push(format!("{prefix}{seq}"));
        seeds.push(format!("{prefix}-{seq}"));
        seeds.push(format!("{prefix}/{seq}"));
    }
    if let (Some(prefix), Some(seq), Some(suffix)) = (tag_prefix, tag_sequence_no, tag_suffix) {
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
