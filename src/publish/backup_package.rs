//! Publish every `.pid` in a `SmartPlant` backup directory.
//!
//! Plan `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md` N9-3.
//! A directory that contains `Manifest.txt` is one package. `Export.dmp`,
//! when it is an Oracle `exp`, is read once ([`super::oracle_exp::ExpTables`]).
//! Each `PlantData` zip's `.pid` entries are inflated in memory and published
//! with [`super::pid_load::load_publish_from_reader`]. A SQL Server MTF dump
//! is refused. A package with no dump still publishes the drawings from the
//! `.pid` files alone.
//!
//! One drawing that fails is recorded and the rest are still written. The
//! index is `_Index.json` in the output directory.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::diff::pid_publish_counts;
use super::oracle_exp::{
    attach_oracle_ports, backup_path_key, backup_uid_key, plant_group_path, read_plant_groups,
    BackupDrawing, ExpTables, PlantGroup,
};
use super::pid_load::{load_publish_from_reader, PidLoadError, PidPublishDrawing, PidPublishStats};
use super::pid_writer::{write_pid_coverage_json, write_pid_data_xml, write_pid_meta_xml};
use crate::backup::{scan_backup_folder, ExportDumpFormat, PlantDataFile};

/// Why a backup directory was not published.
#[derive(Debug)]
pub enum BackupPublishError {
    /// The invocation cannot be a backup publish. The process should exit 2.
    Usage(String),
    /// The package could not be opened. The process should exit 1.
    Io(String),
}

impl std::fmt::Display for BackupPublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(detail) | Self::Io(detail) => f.write_str(detail),
        }
    }
}

/// What [`publish_backup_package`] wrote.
#[derive(Debug)]
pub struct BackupPublishReport {
    /// Drawings that were not written.
    pub failed: usize,
    /// `_Index.json`.
    pub index_path: PathBuf,
    /// `.pid` entries seen in `PlantData` zips.
    pub pid_entries: usize,
    /// Drawings whose uid came from `T_DRAWING.PATH` because the `.pid`
    /// has no `DrawingNo`.
    pub shells: usize,
}

/// Publish `package_dir` into `out_dir`.
///
/// # Errors
///
/// Returns [`BackupPublishError::Usage`] when `Export.dmp` is not an Oracle
/// `exp` (a SQL Server tape, or an unrecognised header). Returns
/// [`BackupPublishError::Io`] when the directory, the dump, or a `PlantData`
/// zip cannot be opened. A single drawing's failure is counted on the
/// report and does not return an error.
pub fn publish_backup_package(
    package_dir: &Path,
    out_dir: &Path,
    plant_name: &str,
) -> Result<BackupPublishReport, BackupPublishError> {
    if !package_dir.join("Manifest.txt").is_file() {
        return Err(BackupPublishError::Usage(
            "a directory input is a backup package only when it contains Manifest.txt".into(),
        ));
    }
    let scan = scan_backup_folder(package_dir).map_err(|error| {
        BackupPublishError::Io(format!("read {}: {error}", package_dir.display()))
    })?;
    let export_path = package_dir.join("Export.dmp");
    if export_path.is_file() && scan.export.is_none() {
        let detail = scan
            .problems
            .iter()
            .map(|problem| problem.detail.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(BackupPublishError::Io(format!(
            "Export.dmp could not be sniffed: {detail}"
        )));
    }
    let dump_path = match scan.export.as_ref() {
        Some(export) => match &export.format {
            ExportDumpFormat::OracleExp { .. } => Some(export.path.clone()),
            ExportDumpFormat::SqlServerMtf(_) => {
                return Err(BackupPublishError::Usage(
                    "Export.dmp is a SQL Server MTF stream; use pid_backup_extract".into(),
                ));
            }
            ExportDumpFormat::Unrecognized { detail } => {
                return Err(BackupPublishError::Usage(format!(
                    "Export.dmp is not an Oracle exp dump: {detail}"
                )));
            }
        },
        None => None,
    };

    let mut loaded = match &dump_path {
        Some(path) => Some(load_dump(path)?),
        None => None,
    };
    if let Some(loaded) = loaded.as_mut() {
        if let Some(path) = scan.refdata.iter().find_map(|file| {
            (file.entry.schema == 4 && file.entry.id == 804).then_some(file.entry.full_path.clone())
        }) {
            loaded.datamap = Some(crate::backup::read_datamap(&path).map_err(|error| {
                BackupPublishError::Io(format!("read {}: {error}", path.display()))
            })?);
        }
    }
    fs::create_dir_all(out_dir).map_err(|error| {
        BackupPublishError::Io(format!("create {}: {error}", out_dir.display()))
    })?;

    let mut failed = 0usize;
    let mut shells = 0usize;
    let mut pid_entries = 0usize;
    let mut paired = 0usize;
    let mut objects = 0usize;
    let mut values = 0usize;
    let mut relationships = 0usize;
    let mut piping_ports = 0usize;
    let mut signal_ports = 0usize;
    let mut connectivity_rels = 0usize;
    let mut opc_branch_points = 0usize;
    let mut equipment_composition_rels = 0usize;
    let mut item_note_rels = 0usize;
    let mut pipelines = 0usize;
    let mut piping_connectors = 0usize;
    let mut written_names = HashSet::new();
    let mut drawings = Vec::new();
    let mut problems: Vec<String> = scan
        .problems
        .iter()
        .map(|problem| format!("{}: {}", problem.path.display(), problem.detail))
        .collect();

    for plant in &scan.plant_data {
        if !matches!(plant.format, crate::backup::RefDataFormat::Zip) {
            continue;
        }
        match publish_zip(
            plant,
            out_dir,
            plant_name,
            loaded.as_ref(),
            &mut PublishTally {
                failed: &mut failed,
                shells: &mut shells,
                pid_entries: &mut pid_entries,
                paired: &mut paired,
                objects: &mut objects,
                values: &mut values,
                relationships: &mut relationships,
                piping_ports: &mut piping_ports,
                signal_ports: &mut signal_ports,
                connectivity_rels: &mut connectivity_rels,
                opc_branch_points: &mut opc_branch_points,
                equipment_composition_rels: &mut equipment_composition_rels,
                item_note_rels: &mut item_note_rels,
                pipelines: &mut pipelines,
                piping_connectors: &mut piping_connectors,
                written_names: &mut written_names,
                drawings: &mut drawings,
                problems: &mut problems,
            },
        ) {
            Ok(()) => {}
            Err(detail) => {
                problems.push(detail);
                failed += 1;
            }
        }
    }

    let export_kind = if dump_path.is_some() {
        "OracleExp"
    } else {
        "PidOnly"
    };
    let drawing_rows = match loaded.as_ref() {
        Some(loaded) => loaded.drawings.len(),
        None => 0,
    };
    let index = json!({
        "package": package_dir.display().to_string(),
        "export": export_kind,
        "pid_entries": pid_entries,
        "drawing_rows": drawing_rows,
        "paired": paired,
        "failed": failed,
        "shells": shells,
        "objects": objects,
        "values": values,
        "relationships": relationships,
        "piping_ports": piping_ports,
        "signal_ports": signal_ports,
        "connectivity_rels": connectivity_rels,
        "opc_branch_points": opc_branch_points,
        "equipment_composition_rels": equipment_composition_rels,
        "item_note_rels": item_note_rels,
        "pipelines": pipelines,
        "piping_connectors": piping_connectors,
        "pipeline_uids": if dump_path.is_some() {
            "synthesized"
        } else {
            "not synthesized"
        },
        "drawings": drawings,
        "problems": problems,
    });
    let index_path = out_dir.join("_Index.json");
    let mut text = serde_json::to_string_pretty(&index).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    fs::write(&index_path, text).map_err(|error| {
        BackupPublishError::Io(format!("write {}: {error}", index_path.display()))
    })?;
    Ok(BackupPublishReport {
        failed,
        index_path,
        pid_entries,
        shells,
    })
}

struct LoadedDump {
    tables: ExpTables,
    path: PathBuf,
    drawings: Vec<BackupDrawing>,
    by_id: HashMap<String, usize>,
    by_path: HashMap<String, usize>,
    groups: Vec<PlantGroup>,
    datamap: Option<crate::backup::DataMap>,
}

fn load_dump(path: &Path) -> Result<LoadedDump, BackupPublishError> {
    let bytes = fs::read(path)
        .map_err(|error| BackupPublishError::Io(format!("read {}: {error}", path.display())))?;
    let tables = ExpTables::read(&bytes)
        .map_err(|error| BackupPublishError::Io(format!("read {}: {error}", path.display())))?;
    let drawings = tables
        .drawings()
        .map_err(|error| BackupPublishError::Io(format!("read {}: {error}", path.display())))?;
    let groups = read_plant_groups(&bytes)
        .map_err(|error| BackupPublishError::Io(format!("read {}: {error}", path.display())))?;
    let mut by_id = HashMap::new();
    let mut by_path = HashMap::new();
    for (index, drawing) in drawings.iter().enumerate() {
        by_id.insert(drawing.id_key.clone(), index);
        if !drawing.path_key.is_empty() {
            by_path.insert(drawing.path_key.clone(), index);
        }
    }
    Ok(LoadedDump {
        tables,
        path: path.to_path_buf(),
        drawings,
        by_id,
        by_path,
        groups,
        datamap: None,
    })
}

struct PublishTally<'a> {
    failed: &'a mut usize,
    shells: &'a mut usize,
    pid_entries: &'a mut usize,
    paired: &'a mut usize,
    objects: &'a mut usize,
    values: &'a mut usize,
    relationships: &'a mut usize,
    piping_ports: &'a mut usize,
    signal_ports: &'a mut usize,
    connectivity_rels: &'a mut usize,
    opc_branch_points: &'a mut usize,
    equipment_composition_rels: &'a mut usize,
    item_note_rels: &'a mut usize,
    pipelines: &'a mut usize,
    piping_connectors: &'a mut usize,
    written_names: &'a mut HashSet<String>,
    drawings: &'a mut Vec<Value>,
    problems: &'a mut Vec<String>,
}

fn publish_zip(
    plant: &PlantDataFile,
    out_dir: &Path,
    plant_name: &str,
    loaded: Option<&LoadedDump>,
    tally: &mut PublishTally<'_>,
) -> Result<(), String> {
    let file = fs::File::open(&plant.full_path)
        .map_err(|error| format!("open {}: {error}", plant.full_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("open {}: {error}", plant.full_path.display()))?;
    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                tally
                    .problems
                    .push(format!("{} entry {index}: {error}", plant.file_name));
                *tally.failed += 1;
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry.name_raw().to_vec();
        let zip_entry = decode_zip_entry_name(&raw_name);
        if !zip_entry.to_ascii_lowercase().ends_with(".pid") {
            continue;
        }
        *tally.pid_entries += 1;
        let mut bytes = Vec::new();
        if let Err(error) = entry.read_to_end(&mut bytes) {
            tally
                .problems
                .push(format!("{} {zip_entry}: {error}", plant.file_name));
            *tally.failed += 1;
            tally.drawings.push(failed_drawing(
                &plant.file_name,
                &zip_entry,
                &format!("read: {error}"),
            ));
            continue;
        }
        drop(entry);
        publish_one(
            plant, &zip_entry, &bytes, out_dir, plant_name, loaded, tally,
        );
    }
    Ok(())
}

fn publish_one(
    plant: &PlantDataFile,
    zip_entry: &str,
    bytes: &[u8],
    out_dir: &Path,
    plant_name: &str,
    loaded: Option<&LoadedDump>,
    tally: &mut PublishTally<'_>,
) {
    let stem = entry_stem(zip_entry);
    let loaded_drawing = load_publish_from_reader(std::io::Cursor::new(bytes), &stem);
    let mut uid_from = "DrawingNo";
    let mut drawing = match loaded_drawing {
        Ok(mut drawing) => {
            if let Some(row) = loaded.and_then(|loaded| row_by_id(loaded, &drawing.drawing_uid)) {
                if !row.name.is_empty() {
                    drawing.drawing_name = row.name.clone();
                }
                uid_from = "T_DRAWING.SP_ID";
            }
            drawing
        }
        Err(PidLoadError::NoDrawing) => {
            let Some(row) = loaded.and_then(|loaded| row_by_path(loaded, zip_entry)) else {
                let detail =
                    "no P&IDAttributes row carries a DrawingNo, and T_DRAWING.PATH does not match";
                tally
                    .problems
                    .push(format!("{} {zip_entry}: {detail}", plant.file_name));
                *tally.failed += 1;
                tally
                    .drawings
                    .push(failed_drawing(&plant.file_name, zip_entry, detail));
                return;
            };
            uid_from = "T_DRAWING.PATH";
            shell_drawing(row.id.clone(), row.name.clone())
        }
        Err(error) => {
            let detail = error.to_string();
            tally
                .problems
                .push(format!("{} {zip_entry}: {detail}", plant.file_name));
            *tally.failed += 1;
            tally
                .drawings
                .push(failed_drawing(&plant.file_name, zip_entry, &detail));
            return;
        }
    };

    if let Some(loaded) = loaded {
        if uid_from != "T_DRAWING.PATH" {
            if let Err(error) = attach_exp(&mut drawing, loaded) {
                let detail = error.to_string();
                tally
                    .problems
                    .push(format!("{} {zip_entry}: {detail}", plant.file_name));
                *tally.failed += 1;
                tally
                    .drawings
                    .push(failed_drawing(&plant.file_name, zip_entry, &detail));
                return;
            }
            if let Err(error) = super::pipeline::attach_pipelines(
                &mut drawing,
                &loaded.tables,
                plant_name,
                loaded.datamap.as_ref(),
            ) {
                let detail = error.to_string();
                tally
                    .problems
                    .push(format!("{} {zip_entry}: {detail}", plant.file_name));
                *tally.failed += 1;
                tally
                    .drawings
                    .push(failed_drawing(&plant.file_name, zip_entry, &detail));
                return;
            }
        }
    }

    let row = loaded.and_then(|loaded| match uid_from {
        "T_DRAWING.PATH" => row_by_path(loaded, zip_entry),
        "T_DRAWING.SP_ID" => row_by_id(loaded, &drawing.drawing_uid),
        _ => None,
    });
    if drawing.drawing_name.is_empty() {
        drawing.drawing_name = if drawing.drawing_uid.is_empty() {
            stem
        } else {
            drawing.drawing_uid.clone()
        };
    }
    let data_path = out_dir.join(format!("{}_Data.xml", drawing.drawing_name));
    if !tally.written_names.insert(drawing.drawing_name.clone()) {
        let detail = format!("output name {} is already used", drawing.drawing_name);
        tally
            .problems
            .push(format!("{} {zip_entry}: {detail}", plant.file_name));
        *tally.failed += 1;
        tally
            .drawings
            .push(failed_drawing(&plant.file_name, zip_entry, &detail));
        return;
    }
    let xml = write_pid_data_xml(&drawing, plant_name);
    let meta = match write_pid_meta_xml(&drawing, plant_name) {
        Ok(meta) => meta,
        Err(error) => {
            let detail = error.to_string();
            tally
                .problems
                .push(format!("{} {zip_entry}: {detail}", plant.file_name));
            *tally.failed += 1;
            tally
                .drawings
                .push(failed_drawing(&plant.file_name, zip_entry, &detail));
            return;
        }
    };
    let source = PathBuf::from(format!("{}:{zip_entry}", plant.file_name));
    let report = write_pid_coverage_json(&drawing, &source);
    if let Err(error) = write_three(&data_path, &xml, &meta, &report, &drawing.drawing_name) {
        tally
            .problems
            .push(format!("{} {zip_entry}: {error}", plant.file_name));
        *tally.failed += 1;
        tally
            .drawings
            .push(failed_drawing(&plant.file_name, zip_entry, &error));
        return;
    }
    let (object_count, value_count, rel_count) = pid_publish_counts(&xml);
    if uid_from == "T_DRAWING.SP_ID" || uid_from == "T_DRAWING.PATH" {
        *tally.paired += 1;
    }
    if uid_from == "T_DRAWING.PATH" {
        *tally.shells += 1;
    }
    if uid_from != "T_DRAWING.PATH" {
        *tally.objects += object_count;
        *tally.values += value_count;
        *tally.relationships += rel_count;
    }
    let oracle = coverage_oracle(&report);
    if let Some(oracle) = &oracle {
        *tally.piping_ports += json_usize(oracle, "piping_ports");
        *tally.signal_ports += json_usize(oracle, "signal_ports");
        *tally.connectivity_rels += json_usize(oracle, "connectivity_rels");
        *tally.opc_branch_points += json_usize(oracle, "opc_branch_points");
        *tally.equipment_composition_rels += json_usize(oracle, "equipment_composition_rels");
        *tally.item_note_rels += json_usize(oracle, "item_note_rels");
        *tally.pipelines += json_usize(oracle, "pipelines");
        *tally.piping_connectors += json_usize(oracle, "piping_connectors");
    }
    let groups = if let Some(loaded) = loaded {
        loaded.groups.as_slice()
    } else {
        &[]
    };
    let plant_group = row
        .map(|row| plant_group_path(groups, &row.plant_group_id))
        .unwrap_or_default();
    tally.drawings.push(json!({
        "doc_uid": drawing.drawing_uid,
        "name": drawing.drawing_name,
        "zip_file": plant.file_name,
        "zip_entry": zip_entry,
        "drawing_number": row.map(|row| row.drawing_number.clone()).unwrap_or_default(),
        "path": row.map(|row| row.path.clone()).unwrap_or_default(),
        "plant_group": plant_group,
        "uid_from": uid_from,
        "objects": object_count,
        "values": value_count,
        "relationships": rel_count,
        "oracle_exp": oracle,
        "problems": [],
    }));
}

fn row_by_id<'a>(loaded: &'a LoadedDump, drawing_uid: &str) -> Option<&'a BackupDrawing> {
    let index = *loaded.by_id.get(&backup_uid_key(drawing_uid))?;
    loaded.drawings.get(index)
}

fn row_by_path<'a>(loaded: &'a LoadedDump, zip_entry: &str) -> Option<&'a BackupDrawing> {
    let index = *loaded.by_path.get(&backup_path_key(zip_entry))?;
    loaded.drawings.get(index)
}

fn attach_exp(
    drawing: &mut PidPublishDrawing,
    loaded: &LoadedDump,
) -> Result<(), super::oracle_exp::ExpError> {
    let ports = loaded.tables.port_tables()?;
    attach_oracle_ports(drawing, &ports, &loaded.path);
    drawing.connectivity_ends = loaded.tables.connectivity_ends(&drawing.drawing_uid)?;
    drawing.exp_relations = loaded.tables.composition_relations()?;
    let mut opc_points = loaded.tables.opc_points(&drawing.drawing_uid)?;
    let representations: std::collections::HashSet<&str> = drawing
        .representations
        .iter()
        .map(|representation| representation.uid.as_str())
        .collect();
    opc_points.retain(|point| representations.contains(point.representation_uid.as_str()));
    drawing.opc_points = opc_points;
    Ok(())
}

fn shell_drawing(uid: String, name: String) -> PidPublishDrawing {
    PidPublishDrawing {
        drawing_uid: uid,
        drawing_name: name,
        items: Vec::new(),
        process_points: Vec::new(),
        branch_points: Vec::new(),
        ports: Vec::new(),
        connectivity_ends: Vec::new(),
        exp_relations: Vec::new(),
        opc_points: Vec::new(),
        exp: None,
        pipelines: Vec::new(),
        pipelines_attempted: false,
        pipeline_synthesis: super::pid_load::PipelineSynthesis::default(),
        representations: Vec::new(),
        stats: PidPublishStats::default(),
    }
}

fn write_three(
    data_path: &Path,
    xml: &str,
    meta: &str,
    report: &str,
    name: &str,
) -> Result<(), String> {
    let dir = data_path.parent().unwrap_or(Path::new("."));
    write_file(data_path, xml)?;
    write_file(&dir.join(format!("{name}_Meta.xml")), meta)?;
    write_file(&dir.join(format!("{name}_Coverage.json")), report)?;
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(())
}

fn failed_drawing(zip_file: &str, zip_entry: &str, detail: &str) -> Value {
    json!({
        "doc_uid": "",
        "name": entry_stem(zip_entry),
        "zip_file": zip_file,
        "zip_entry": zip_entry,
        "drawing_number": "",
        "path": "",
        "plant_group": "",
        "uid_from": "",
        "objects": 0,
        "values": 0,
        "relationships": 0,
        "oracle_exp": Value::Null,
        "problems": [detail],
    })
}

fn coverage_oracle(report: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(report).ok()?;
    let oracle = value.get("oracle_exp")?.clone();
    if oracle.is_null() {
        None
    } else {
        Some(oracle)
    }
}

fn json_usize(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

fn entry_stem(entry: &str) -> String {
    let name = entry.rsplit(['/', '\\']).next().unwrap_or(entry);
    name.strip_suffix(".pid")
        .or_else(|| name.strip_suffix(".PID"))
        .unwrap_or(name)
        .to_string()
}

/// Zip entry name from the raw central-directory bytes.
///
/// ASCII and valid UTF-8 stay as they are. Anything else is GBK. A GBK
/// decode that reports errors becomes the raw bytes in hex.
#[must_use]
pub fn decode_zip_entry_name(raw: &[u8]) -> String {
    if let Ok(utf8) = std::str::from_utf8(raw) {
        return utf8.to_string();
    }
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(raw);
    if had_errors {
        raw.iter().map(|byte| format!("{byte:02x}")).collect()
    } else {
        decoded.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gbk_zip_name_round_trips_and_ascii_stays_ascii() {
        let (encoded, _, errors) = encoding_rs::GBK.encode("00/00/井场.pid");
        assert!(!errors);
        assert_eq!(decode_zip_entry_name(&encoded), "00/00/井场.pid");
        assert_eq!(decode_zip_entry_name(b"D06.pid"), "D06.pid");
    }
}
