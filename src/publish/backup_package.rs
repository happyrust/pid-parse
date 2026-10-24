//! Publish every `.pid` in a `SmartPlant` backup directory.
//!
//! Plan `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md` N9-3
//! and N12. A directory that contains `Manifest.txt` is one package.
//! `Export.dmp`, when it is an Oracle `exp`, is read once
//! ([`super::oracle_exp::ExpTables`]). Each `PlantData` zip's `.pid`
//! entries are inflated in memory and published with
//! [`super::pid_load::load_publish_from_reader`]. A SQL Server MTF dump
//! instead goes through the MDF path: the `MSDA` stream is stripped to
//! the plain MDF image, staged once via `mdf_load`, and every
//! `T_Drawing` row publishes through the proven `sqlite_load` loader —
//! that branch needs the `backup-mdf` feature and is refused without
//! it. A package with no dump still publishes the drawings from the
//! `.pid` files alone.
//!
//! One drawing that fails is recorded and the rest are still written. The
//! index is `_Index.json` in the output directory; every `.pid` entry is
//! accounted in it exactly once — paired to a model row, classified as a
//! template under a `Template Files` folder, or listed under
//! `unmatched_pid_entries`.

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
use crate::backup::{scan_backup_folder, BackupFolderScan, ExportDumpFormat, PlantDataFile};

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
    /// Drawings successfully written (all three output files —
    /// `_Data.xml`, `_Meta.xml`, `_Coverage.json` — in both modes).
    pub written: usize,
    /// `_Index.json`.
    pub index_path: PathBuf,
    /// `.pid` entries seen in `PlantData` zips.
    pub pid_entries: usize,
    /// Drawings whose uid came from `T_DRAWING.PATH` because the `.pid`
    /// has no `DrawingNo`.
    pub shells: usize,
    /// 门禁的逐图结论；没给参考目录时为 `None`。
    pub gate: Option<PackageGateReport>,
}

/// 一次整包发布里 `--diff-against` 的逐图结论。
///
/// 参考按图名索引：`<参考目录>/<图名>_Data.xml`。
#[derive(Debug, Default)]
pub struct PackageGateReport {
    /// 逐图结论行，按写出顺序；dirty 的图，其 finding 行紧随其后。
    pub lines: Vec<String>,
    /// 参考与之一致的图数。
    pub clean: usize,
    /// 参考不一致的图数。
    pub dirty: usize,
    /// 参考目录里没有 `<图名>_Data.xml` 的图数。
    pub unpaired: usize,
}

impl PackageGateReport {
    /// 门禁没能判 clean 的图数：dirty 加 unpaired。
    pub fn not_clean(&self) -> usize {
        self.dirty + self.unpaired
    }
}

/// Publish `package_dir` into `out_dir`.
///
/// # Errors
///
/// Returns [`BackupPublishError::Usage`] when `Export.dmp` has an
/// unrecognised header — or is a SQL Server MTF tape while this build
/// lacks the `backup-mdf` feature. Returns [`BackupPublishError::Io`]
/// when the directory, the dump, or a `PlantData` zip cannot be opened.
/// A single drawing's failure is counted on the report and does not
/// return an error.
///
/// `reference_dir` 给定时，每个写出的图都会与 `<reference_dir>/<图名>_Data.xml` 打分。
pub fn publish_backup_package(
    package_dir: &Path,
    out_dir: &Path,
    plant_name: &str,
    reference_dir: Option<&Path>,
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
                return publish_mtf_package(
                    package_dir,
                    &export.path,
                    out_dir,
                    plant_name,
                    &scan,
                    reference_dir,
                );
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
    let mut written = 0usize;
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
    let mut template_pid_entries = 0usize;
    let mut entry_names: Vec<String> = Vec::new();
    let mut paired_entries: HashSet<String> = HashSet::new();
    let mut written_names = HashSet::new();
    let mut drawings = Vec::new();
    let mut problems: Vec<String> = scan
        .problems
        .iter()
        .map(|problem| format!("{}: {}", problem.path.display(), problem.detail))
        .collect();
    let mut gate = reference_dir.map(PackageGate::new);

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
                written: &mut written,
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
                template_pid_entries: &mut template_pid_entries,
                entry_names: &mut entry_names,
                paired_entries: &mut paired_entries,
                written_names: &mut written_names,
                drawings: &mut drawings,
                problems: &mut problems,
                gate: gate.as_mut(),
            },
        ) {
            Ok(()) => {}
            Err(detail) => {
                problems.push(detail);
                failed += 1;
            }
        }
    }

    let gate = gate.map(|gate| gate.report);
    let export_kind = if dump_path.is_some() {
        "OracleExp"
    } else {
        "PidOnly"
    };
    let drawing_rows = match loaded.as_ref() {
        Some(loaded) => loaded.drawings.len(),
        None => 0,
    };
    let excluded_from_totals = excluded_from_totals(&drawings);
    // Every `.pid` entry is counted exactly once — paired to a model row
    // (`paired`), classified as a template (`template_pid_entries`), or
    // listed here: the package shipped the `.pid`, but the dump's
    // `T_DRAWING` does not hold it. With no dump at all every entry lands
    // in `unmatched_pid_entries`; that is the intended, machine-checkable
    // accounting for a `.pid`-only package.
    let mut unmatched_pid_entries: Vec<String> = entry_names
        .iter()
        .filter(|entry| !paired_entries.contains(*entry))
        .cloned()
        .collect();
    unmatched_pid_entries.sort();
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
        "excluded_from_totals": excluded_from_totals,
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
        "template_pid_entries": template_pid_entries,
        "unmatched_pid_entries": unmatched_pid_entries,
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
        written,
        index_path,
        pid_entries,
        shells,
        gate,
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
    written: &'a mut usize,
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
    template_pid_entries: &'a mut usize,
    entry_names: &'a mut Vec<String>,
    paired_entries: &'a mut HashSet<String>,
    written_names: &'a mut HashSet<String>,
    drawings: &'a mut Vec<Value>,
    problems: &'a mut Vec<String>,
    gate: Option<&'a mut PackageGate>,
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
        if is_template_pid_entry(&zip_entry) {
            // A template ships in the reference-data tree: it is not a
            // drawing, so it is neither published nor reported as a `.pid`
            // the model store lost.
            *tally.template_pid_entries += 1;
            continue;
        }
        tally
            .entry_names
            .push(entry_key(&plant.file_name, &zip_entry));
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
    let (object_count, value_count, rel_count) = match pid_publish_counts(&xml) {
        Ok(counts) => counts,
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
    *tally.written += 1;
    if let Some(gate) = tally.gate.as_deref_mut() {
        gate.score(&drawing.drawing_name, &xml);
    }
    if uid_from == "T_DRAWING.SP_ID" || uid_from == "T_DRAWING.PATH" {
        *tally.paired += 1;
        tally
            .paired_entries
            .insert(entry_key(&plant.file_name, zip_entry));
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
        "template": row.map(|row| row.template.clone()).unwrap_or_default(),
        "plant_group": plant_group,
        "uid_from": uid_from,
        "objects": object_count,
        "values": value_count,
        "relationships": rel_count,
        "files": [
            format!("{}_Data.xml", drawing.drawing_name),
            format!("{}_Meta.xml", drawing.drawing_name),
            format!("{}_Coverage.json", drawing.drawing_name),
        ],
        "oracle_exp": oracle,
        "problems": [],
    }));
}

/// 把一次整包发布写出的每张图与一个 `<图名>_Data.xml` 参考目录打分。
struct PackageGate {
    reference_dir: PathBuf,
    report: PackageGateReport,
}

impl PackageGate {
    fn new(reference_dir: &Path) -> Self {
        Self {
            reference_dir: reference_dir.to_path_buf(),
            report: PackageGateReport::default(),
        }
    }

    /// 给一张刚写出的图打分；结论行进 `report`。
    ///
    /// 参考文件不存在时记 unpaired（**不是** clean、也不是跳过）。
    fn score(&mut self, name: &str, xml: &str) {
        let reference_path = self.reference_dir.join(format!("{name}_Data.xml"));
        if !reference_path.is_file() {
            self.report.unpaired += 1;
            self.report.lines.push(format!(
                "{name}: unpaired — no reference file {}; the drawing was not compared",
                reference_path.display()
            ));
            return;
        }
        let reference = match fs::read_to_string(&reference_path) {
            Ok(reference) => reference,
            Err(error) => {
                self.report.unpaired += 1;
                self.report.lines.push(format!(
                    "{name}: unpaired — reference {} could not be read: {error}",
                    reference_path.display()
                ));
                return;
            }
        };
        match super::diff::diff_pid_export(xml, &reference) {
            Ok(diff) if diff.is_clean() => {
                self.report.clean += 1;
                self.report.lines.push(format!(
                    "{name}: clean (objects {}, values {}, rels {})",
                    diff.objects, diff.values, diff.rels
                ));
            }
            Ok(diff) => {
                self.report.dirty += 1;
                self.report.lines.push(format!(
                    "{name}: dirty (objects {}, values {}, rels {}, findings {})",
                    diff.objects,
                    diff.values,
                    diff.rels,
                    diff.findings.len()
                ));
                for finding in &diff.findings {
                    self.report.lines.push(format!("  {name}: {finding}"));
                }
            }
            Err(malformed) => {
                self.report.dirty += 1;
                self.report
                    .lines
                    .push(format!("{name}: dirty — {malformed}"));
            }
        }
    }
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
        "template": "",
        "plant_group": "",
        "uid_from": "",
        "objects": 0,
        "values": 0,
        "relationships": 0,
        "files": [],
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

/// `_Index.json` `excluded_from_totals` declaration: which `drawings[]`
/// rows the top-level `objects` / `values` / `relationships` aggregates
/// do not count, and how much each of them holds — so
/// `aggregate + excluded == Σ drawings[]` stays machine-checkable.
///
/// The rule selects only `uid_from == "T_DRAWING.PATH"` rows — shell
/// drawings whose `.pid` carried no `P&IDAttributes` row, so the
/// published `_Data.xml` holds just the `PIDDrawing` container. A
/// `failed_drawing` row (`uid_from == ""`, every count 0) adds 0 to the
/// aggregates and to Σ `drawings[]` alike, so it is never a silent
/// exclusion channel.
fn excluded_from_totals(drawings: &[Value]) -> Value {
    let rows: Vec<Value> = drawings
        .iter()
        .filter(|row| row["uid_from"] == "T_DRAWING.PATH")
        .map(|row| {
            json!({
                "name": row["name"].clone(),
                "uid_from": row["uid_from"].clone(),
                "zip_file": row["zip_file"].clone(),
                "zip_entry": row["zip_entry"].clone(),
                "objects": row["objects"].clone(),
                "values": row["values"].clone(),
                "relationships": row["relationships"].clone(),
            })
        })
        .collect();
    let objects = rows
        .iter()
        .map(|row| json_usize(row, "objects"))
        .sum::<usize>();
    let values = rows
        .iter()
        .map(|row| json_usize(row, "values"))
        .sum::<usize>();
    let relationships = rows
        .iter()
        .map(|row| json_usize(row, "relationships"))
        .sum::<usize>();
    json!({
        "rule": "uid_from == \"T_DRAWING.PATH\"",
        "reason": "these drawings carry no P&IDAttributes row, so the publish wrote only the PIDDrawing container shell; their counts stay out of the top-level aggregates",
        "drawings": rows,
        "objects": objects,
        "values": values,
        "relationships": relationships,
    })
}

fn entry_stem(entry: &str) -> String {
    let name = entry.rsplit(['/', '\\']).next().unwrap_or(entry);
    name.strip_suffix(".pid")
        .or_else(|| name.strip_suffix(".PID"))
        .unwrap_or(name)
        .to_string()
}

/// `Template Files/` 下的 `.pid` 是整包随附的绘图模板，不是本包的图纸。
fn is_template_pid_entry(entry: &str) -> bool {
    entry
        .split(['/', '\\'])
        .any(|part| part.eq_ignore_ascii_case("Template Files"))
}

/// `_Index.json` 用来指名一个 `.pid` 条目的 `<zip>:<entry>` 键。
fn entry_key(zip_file: &str, entry: &str) -> String {
    format!("{zip_file}:{entry}")
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

/// One `T_Drawing` row of a staged SQL Server backup — every column
/// is TEXT because `mdf_load` stages it that way.
#[cfg(feature = "backup-mdf")]
struct MtfDrawingRow {
    uid: String,
    name: String,
    path: String,
    drawing_number: String,
}

/// SQL Server MTF branch of [`publish_backup_package`].
///
/// `Export.dmp` is an MTF tape stream: its `MSDA` body minus the
/// backup-stream prefix is the plant's MDF. The image is staged once
/// through [`super::mdf_load::open_mtf_dump_as_sqlite`], `T_Drawing`
/// rows enumerate the drawings, and each is loaded with the proven
/// `sqlite_load` path and written as `<Name>_Data.xml` +
/// `<Name>_Meta.xml` + `<Name>_Coverage.json`. `PlantData` zips are
/// listed but never inflated: `_Index.json` keeps the same shape as the
/// Oracle `exp` branch, the `T_Drawing.Path` → zip-entry map pairs each
/// drawing with its `.pid` entry, `.pid` entries under `Template Files`
/// are tallied under `template_pid_entries`, and entries with no model
/// row surface under `unmatched_pid_entries`.
#[cfg(feature = "backup-mdf")]
fn publish_mtf_package(
    package_dir: &Path,
    dump_path: &Path,
    out_dir: &Path,
    plant_name: &str,
    scan: &BackupFolderScan,
    reference_dir: Option<&Path>,
) -> Result<BackupPublishReport, BackupPublishError> {
    let dump = fs::read(dump_path).map_err(|error| {
        BackupPublishError::Io(format!("read {}: {error}", dump_path.display()))
    })?;
    let conn = super::mdf_load::open_mtf_dump_as_sqlite(&dump).map_err(|error| {
        BackupPublishError::Io(format!("stage {}: {error}", dump_path.display()))
    })?;
    fs::create_dir_all(out_dir).map_err(|error| {
        BackupPublishError::Io(format!("create {}: {error}", out_dir.display()))
    })?;
    let rows = mtf_drawing_rows(&conn).map_err(|error| {
        BackupPublishError::Io(format!(
            "read T_Drawing of {}: {error}",
            dump_path.display()
        ))
    })?;

    let mut failed = 0usize;
    let mut problems: Vec<String> = scan
        .problems
        .iter()
        .map(|problem| format!("{}: {}", problem.path.display(), problem.detail))
        .collect();

    // List the PlantData zips (never inflated): `pid_entries` keeps the
    // index shape identical to the Oracle `exp` branch and `T_Drawing.Path`
    // pairs each drawing with the `.pid` entry that carries the same
    // graphic data.
    let mut pid_entries = 0usize;
    let mut template_pid_entries = 0usize;
    let mut zip_pid: HashMap<String, (String, String)> = HashMap::new();
    for plant in &scan.plant_data {
        if !matches!(plant.format, crate::backup::RefDataFormat::Zip) {
            continue;
        }
        let file = match fs::File::open(&plant.full_path) {
            Ok(file) => file,
            Err(error) => {
                problems.push(format!("open {}: {error}", plant.full_path.display()));
                failed += 1;
                continue;
            }
        };
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(error) => {
                problems.push(format!("open {}: {error}", plant.full_path.display()));
                failed += 1;
                continue;
            }
        };
        for index in 0..archive.len() {
            let entry = match archive.by_index(index) {
                Ok(entry) => entry,
                Err(error) => {
                    problems.push(format!("{} entry {index}: {error}", plant.file_name));
                    failed += 1;
                    continue;
                }
            };
            if entry.is_dir() {
                continue;
            }
            let name = decode_zip_entry_name(entry.name_raw());
            if !name.to_ascii_lowercase().ends_with(".pid") {
                continue;
            }
            pid_entries += 1;
            if is_template_pid_entry(&name) {
                template_pid_entries += 1;
                continue;
            }
            zip_pid
                .entry(backup_path_key(&name))
                .or_insert((plant.file_name.clone(), name));
        }
    }

    let mut written = 0usize;
    let mut paired = 0usize;
    let mut objects = 0usize;
    let mut values = 0usize;
    let mut relationships = 0usize;
    let mut pipelines = 0usize;
    let mut piping_connectors = 0usize;
    let mut written_names = HashSet::new();
    let mut paired_keys = HashSet::new();
    let mut drawings: Vec<Value> = Vec::new();
    let mut gate = reference_dir.map(PackageGate::new);

    for row in &rows {
        let path_key = backup_path_key(&row.path);
        let (zip_file, zip_entry) = zip_pid
            .get(&path_key)
            .map(|(file, entry)| (file.clone(), entry.clone()))
            .unwrap_or_default();
        let (graph, graph_name, xml, meta) = match mtf_render_drawing(&conn, &row.uid, plant_name) {
            Ok(parts) => parts,
            Err(detail) => {
                problems.push(format!("{} {}: {detail}", row.name, row.uid));
                failed += 1;
                drawings.push(mtf_drawing_entry(
                    row,
                    &row.name,
                    (&zip_file, &zip_entry),
                    (0, 0, 0),
                    String::new(),
                    Vec::new(),
                    vec![detail],
                ));
                continue;
            }
        };
        let name = if !graph_name.is_empty() {
            graph_name
        } else if !row.name.is_empty() {
            row.name.clone()
        } else {
            row.uid.clone()
        };
        if !written_names.insert(name.clone()) {
            let detail = format!("output name {name} is already used");
            problems.push(format!("{} {}: {detail}", row.name, row.uid));
            failed += 1;
            drawings.push(mtf_drawing_entry(
                row,
                &name,
                (&zip_file, &zip_entry),
                (0, 0, 0),
                graph.template.clone().unwrap_or_default(),
                Vec::new(),
                vec![detail],
            ));
            continue;
        }
        let counts = match pid_publish_counts(&xml) {
            Ok(counts) => counts,
            Err(error) => {
                let detail = error.to_string();
                problems.push(format!("{name}: {detail}"));
                failed += 1;
                drawings.push(mtf_drawing_entry(
                    row,
                    &name,
                    (&zip_file, &zip_entry),
                    (0, 0, 0),
                    graph.template.clone().unwrap_or_default(),
                    Vec::new(),
                    vec![detail],
                ));
                continue;
            }
        };
        let source = PathBuf::from(format!("{zip_file}:{zip_entry}"));
        let report = super::pid_writer::write_database_coverage_json(&graph, &source, counts);
        if let Err(detail) = write_file(&out_dir.join(format!("{name}_Data.xml")), &xml)
            .and_then(|()| write_file(&out_dir.join(format!("{name}_Meta.xml")), &meta))
            .and_then(|()| write_file(&out_dir.join(format!("{name}_Coverage.json")), &report))
        {
            problems.push(format!("{name}: {detail}"));
            failed += 1;
            drawings.push(mtf_drawing_entry(
                row,
                &name,
                (&zip_file, &zip_entry),
                (0, 0, 0),
                graph.template.clone().unwrap_or_default(),
                Vec::new(),
                vec![detail],
            ));
            continue;
        }
        written += 1;
        if let Some(gate) = gate.as_mut() {
            gate.score(&name, &xml);
        }
        objects += counts.0;
        values += counts.1;
        relationships += counts.2;
        pipelines += xml.matches("<PIDPipeline>").count();
        piping_connectors += xml.matches("<PIDPipingConnector>").count();
        if zip_pid.contains_key(&path_key) {
            paired += 1;
            paired_keys.insert(path_key);
        }
        drawings.push(mtf_drawing_entry(
            row,
            &name,
            (&zip_file, &zip_entry),
            counts,
            graph.template.clone().unwrap_or_default(),
            vec![
                format!("{name}_Data.xml"),
                format!("{name}_Meta.xml"),
                format!("{name}_Coverage.json"),
            ],
            Vec::new(),
        ));
    }

    // `.pid` entries that never paired to a `T_Drawing` row: files the
    // package shipped that this backup's model store does not hold.
    let mut unmatched: Vec<String> = zip_pid
        .iter()
        .filter(|(key, _)| !paired_keys.contains(*key))
        .map(|(_, (file, entry))| format!("{file}:{entry}"))
        .collect();
    unmatched.sort();

    let excluded_from_totals = excluded_from_totals(&drawings);
    let index = json!({
        "package": package_dir.display().to_string(),
        "export": "SqlServerMtf",
        "pid_entries": pid_entries,
        "drawing_rows": rows.len(),
        "paired": paired,
        "failed": failed,
        "shells": 0,
        "objects": objects,
        "values": values,
        "relationships": relationships,
        "excluded_from_totals": excluded_from_totals,
        "piping_ports": 0,
        "signal_ports": 0,
        "connectivity_rels": 0,
        "opc_branch_points": 0,
        "equipment_composition_rels": 0,
        "item_note_rels": 0,
        "pipelines": pipelines,
        "piping_connectors": piping_connectors,
        "pipeline_uids": "database",
        "template_pid_entries": template_pid_entries,
        "unmatched_pid_entries": unmatched,
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
        written,
        index_path,
        pid_entries,
        shells: 0,
        gate: gate.map(|gate| gate.report),
    })
}

/// Without `backup-mdf` the MTF branch cannot stage the embedded MDF,
/// so a SQL Server directory input keeps reporting the usage error.
#[cfg(not(feature = "backup-mdf"))]
fn publish_mtf_package(
    _package_dir: &Path,
    _dump_path: &Path,
    _out_dir: &Path,
    _plant_name: &str,
    _scan: &BackupFolderScan,
    _reference_dir: Option<&Path>,
) -> Result<BackupPublishReport, BackupPublishError> {
    Err(BackupPublishError::Usage(
        "Export.dmp is a SQL Server MTF stream; use pid_backup_extract".into(),
    ))
}

/// Enumerate `T_Drawing` rows of a staged SQL Server backup in storage
/// order. `DrawingNumber` is not present in every `SmartPlant` schema
/// version, so the select list is built from `PRAGMA table_info`.
#[cfg(feature = "backup-mdf")]
fn mtf_drawing_rows(
    conn: &rusqlite::Connection,
) -> Result<Vec<MtfDrawingRow>, super::model::PublishError> {
    let mut has_drawing_number = false;
    {
        let mut pragma = conn.prepare("PRAGMA table_info(\"T_Drawing\")")?;
        let mut cols = pragma.query([])?;
        while let Some(col) = cols.next()? {
            let name: String = col.get(1)?;
            if name.eq_ignore_ascii_case("DrawingNumber") {
                has_drawing_number = true;
            }
        }
    }
    let sql = if has_drawing_number {
        "SELECT SP_ID, Name, Path, DrawingNumber FROM T_Drawing"
    } else {
        "SELECT SP_ID, Name, Path, NULL FROM T_Drawing"
    };
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(MtfDrawingRow {
            uid: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            path: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            drawing_number: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        });
    }
    out.retain(|row| !row.uid.is_empty());
    Ok(out)
}

/// One `T_Drawing` row → the loaded graph, the drawing's `T_Drawing.Name`
/// as loaded, and the rendered `_Data.xml` + `_Meta.xml` strings.
#[cfg(feature = "backup-mdf")]
fn mtf_render_drawing(
    conn: &rusqlite::Connection,
    uid: &str,
    plant_name: &str,
) -> Result<(super::model::PublishDrawing, String, String, String), String> {
    let graph =
        super::sqlite_load::load_drawing_graph(conn, uid).map_err(|error| error.to_string())?;
    let name = graph.drawing_name.clone();
    let xml =
        super::xml_writer::write_data_xml(&graph, plant_name).map_err(|error| error.to_string())?;
    let meta =
        super::xml_writer::write_meta_xml(&graph, plant_name).map_err(|error| error.to_string())?;
    Ok((graph, name, xml, meta))
}

/// `_Index.json` `drawings[]` entry for the SQL Server MTF branch —
/// same keys as the Oracle `exp` entry; `oracle_exp` stays null.
/// `source` is the `<PlantData zip>` / `<zip entry>` pair the drawing came
/// from; `files` is the file set this run wrote beside it, empty when the
/// drawing failed.
#[cfg(feature = "backup-mdf")]
fn mtf_drawing_entry(
    row: &MtfDrawingRow,
    name: &str,
    source: (&str, &str),
    counts: (usize, usize, usize),
    template: String,
    files: Vec<String>,
    problems: Vec<String>,
) -> Value {
    json!({
        "doc_uid": row.uid,
        "name": name,
        "zip_file": source.0,
        "zip_entry": source.1,
        "drawing_number": row.drawing_number,
        "path": row.path,
        "template": template,
        "plant_group": "",
        "uid_from": "T_Drawing.SP_ID",
        "objects": counts.0,
        "values": counts.1,
        "relationships": counts.2,
        "files": files,
        "oracle_exp": Value::Null,
        "problems": problems,
    })
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

    #[test]
    fn excluded_from_totals_names_only_the_shell_rows() {
        let drawings = vec![
            json!({
                "doc_uid": "d03-uid",
                "name": "D03",
                "zip_file": "PlantData~2~711.zip",
                "zip_entry": "test/U01/D03.pid",
                "drawing_number": "D03",
                "path": "test/U01/D03.pid",
                "plant_group": "",
                "uid_from": "T_DRAWING.PATH",
                "objects": 1,
                "values": 1,
                "relationships": 0,
                "oracle_exp": Value::Null,
                "problems": [],
            }),
            json!({
                "doc_uid": "d06-uid",
                "name": "D06",
                "zip_file": "PlantData~2~1.zip",
                "zip_entry": "D06.pid",
                "drawing_number": "",
                "path": "",
                "plant_group": "",
                "uid_from": "DrawingNo",
                "objects": 42,
                "values": 57,
                "relationships": 13,
                "oracle_exp": Value::Null,
                "problems": [],
            }),
            failed_drawing("PlantData~2~1.zip", "E01.pid", "read: boom"),
        ];
        let declaration = excluded_from_totals(&drawings);
        assert_eq!(declaration["rule"], "uid_from == \"T_DRAWING.PATH\"");
        let named = declaration["drawings"]
            .as_array()
            .expect("excluded drawings");
        assert_eq!(named.len(), 1);
        assert_eq!(named[0]["name"], "D03");
        assert_eq!(named[0]["zip_entry"], "test/U01/D03.pid");
        assert_eq!(declaration["objects"], 1);
        assert_eq!(declaration["values"], 1);
        assert_eq!(declaration["relationships"], 0);
    }
}
