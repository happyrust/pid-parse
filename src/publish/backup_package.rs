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
//!
//! A drawing's three files are written all of them or none, and they and
//! the index each take their name only once written whole
//! ([`write_atomic`]), so a run that is stopped leaves no part of a
//! document under a document's name. The output directory is not emptied
//! first: files an earlier run left there are not removed, and
//! `_Index.json` is what says which files this run wrote.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::diff::pid_publish_census;
use super::model::DatabaseShortfalls;
use super::oracle_exp::{
    attach_oracle_ports, backup_path_key, backup_uid_key, plant_group_path, read_plant_groups,
    BackupDrawing, ExpTables, PlantGroup,
};
use super::pid_load::{
    read_publish_from_reader, PidLoadError, PidPublishDrawing, PidPublishStats, PublishChainFault,
};
use super::pid_writer::{pid_coverage_json, write_pid_data_xml, write_pid_meta_xml};
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
    /// Chain streams of the package's drawings whose record chain does not
    /// hold, in the order the drawings were read. The publish reader takes
    /// a stream's records all or nothing, so each of these is a drawing
    /// whose delivery may be short of what that stream holds while it
    /// counts as written. `_Index.json` lists each among its `problems`,
    /// as [`PackageChainFault::describe`] words it — the one kind of
    /// problem there that is not a drawing or an archive the run could not
    /// read; its `failed` count and the coverage reports do not change. A
    /// caller that delivers the package says them as well, as
    /// `pid_publish_xml` does. Always empty for a SQL Server package,
    /// whose drawings are read from the database and not from the `.pid`.
    pub chain_faults: Vec<PackageChainFault>,
    /// Drawings of a SQL Server package that the database line did not
    /// write in full, in the order they were written. Each is written and
    /// counts as written, with an item left as a `<PIDItem>` placeholder
    /// or a relationship row defined from the item types of its ends alone
    /// ([`DatabaseShortfalls`]). `_Index.json` lists each sentence of
    /// [`PackageDrawingShortfalls::describe`] among its `problems`, and the
    /// same sentences without the drawing's name among that drawing's own;
    /// the drawing's `_Coverage.json` holds the counts under `shortfalls`;
    /// the `failed` count does not change. A caller that delivers the
    /// package says them as well, as `pid_publish_xml` does, where
    /// `--strict` turns them into exit 1. Always empty for an Oracle `exp`
    /// package and for one with no dump: their drawings are read from the
    /// `.pid`, and say what they leave out in their own coverage reports.
    pub shortfalls: Vec<PackageDrawingShortfalls>,
    /// Drawings whose `_Data.xml` states a UID on more than one block —
    /// objects and `<Rel>`s alike — in the order they were written. Each is
    /// written and counts as written. `_Index.json` lists
    /// [`PackageDuplicateUids::describe`] among its `problems` and
    /// [`PackageDuplicateUids::sentence`] among that drawing's own; the
    /// `failed` count and the coverage reports do not change. A caller that
    /// delivers the package says them as well, as `pid_publish_xml` does,
    /// whose exit code they do not change, `--strict` or not. Every drawing
    /// of every kind of package is checked, with or without a reference
    /// directory; where the gate compares a drawing with its reference it
    /// makes a finding of each such UID as well, and the drawing is one of
    /// [`PackageGateReport::dirty`].
    pub duplicate_uids: Vec<PackageDuplicateUids>,
}

/// A chain stream of one drawing of a backup package whose record chain
/// does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageChainFault {
    /// File name of the `PlantData` zip the drawing is in.
    pub zip_file: String,
    /// The drawing's entry in that zip.
    pub zip_entry: String,
    /// The stream, what is wrong with its framing, and what the publish
    /// reader made of it.
    pub fault: PublishChainFault,
}

impl PackageChainFault {
    /// The drawing — zip and entry, as `_Index.json` names a problem's —
    /// and the fault in one sentence ([`PublishChainFault::describe`]).
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{} {}: {}",
            self.zip_file,
            self.zip_entry,
            self.fault.describe()
        )
    }
}

/// One drawing of a SQL Server backup package that the database line did
/// not write in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDrawingShortfalls {
    /// `T_Drawing.SP_ID` of the drawing — `doc_uid` of its `_Index.json`
    /// entry.
    pub drawing_uid: String,
    /// The drawing's name, as its `_Index.json` entry has it.
    pub drawing_name: String,
    /// What was written in less than full form. Never
    /// [empty](DatabaseShortfalls::is_empty): a drawing written in full is
    /// not listed.
    pub shortfalls: DatabaseShortfalls,
}

impl PackageDrawingShortfalls {
    /// One sentence for each kind of shortfall
    /// ([`DatabaseShortfalls::describe`]), each under the drawing's name,
    /// as `_Index.json` lists them among its `problems`.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        self.shortfalls
            .describe()
            .into_iter()
            .map(|sentence| format!("{}: {sentence}", self.drawing_name))
            .collect()
    }
}

/// One drawing of a backup package whose `_Data.xml` states a UID on more
/// than one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDuplicateUids {
    /// `doc_uid` of the drawing's `_Index.json` entry.
    pub drawing_uid: String,
    /// The drawing's name, as its `_Index.json` entry has it.
    pub drawing_name: String,
    /// Each UID more than one block states — objects and `<Rel>`s alike —
    /// with the number of blocks that state it, in the order the document
    /// first states one again. Never empty: a drawing whose document states
    /// each UID once is not listed.
    pub uids: Vec<(String, usize)>,
}

impl PackageDuplicateUids {
    /// The drawing with the UIDs its document states again — the
    /// [`duplicate_uids`](super::diff::PidPublishCensus::duplicate_uids) of
    /// its census, one entry for each block that does — or `None` when
    /// there is none.
    fn of(drawing_uid: &str, drawing_name: &str, stated_again: &[String]) -> Option<Self> {
        let mut uids: Vec<(String, usize)> = Vec::new();
        let mut places: HashMap<&str, usize> = HashMap::new();
        for uid in stated_again {
            let place = *places.entry(uid.as_str()).or_insert_with(|| {
                // The block that stated it first is one of them.
                uids.push((uid.clone(), 1));
                uids.len() - 1
            });
            uids[place].1 += 1;
        }
        (!uids.is_empty()).then(|| Self {
            drawing_uid: drawing_uid.to_string(),
            drawing_name: drawing_name.to_string(),
            uids,
        })
    }

    /// How many UIDs, and each with the number of blocks that state it: the
    /// sentence the drawing's `_Index.json` entry lists among its
    /// `problems`.
    #[must_use]
    pub fn sentence(&self) -> String {
        let uids = self
            .uids
            .iter()
            .map(|(uid, blocks)| format!("{uid} ×{blocks}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} {} written more than once: {uids}",
            self.uids.len(),
            if self.uids.len() == 1 { "UID" } else { "UIDs" }
        )
    }

    /// [`Self::sentence`] under the drawing's name, as `_Index.json` lists
    /// it among its `problems`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{}: {}", self.drawing_name, self.sentence())
    }
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
    /// 参考文件在，但两侧 `<Container DocUID>` 不同的图数。
    ///
    /// 这种配对不是「有发现」而是「配错版本」：参考是同一张图名的另一版图纸，
    /// 逐条比对只会得到上千条与版本差有关的 finding。它仍然不算 clean ——
    /// 门禁没有验证过这张图，退出码照样是 1。
    pub revision_mismatch: usize,
}

impl PackageGateReport {
    /// 门禁没能判 clean 的图数：`dirty` 加 `unpaired` 加 `revision_mismatch`。
    pub fn not_clean(&self) -> usize {
        self.dirty + self.unpaired + self.revision_mismatch
    }
}

/// Publish `package_dir` into `out_dir`.
///
/// A drawing whose `_Data.xml` states a UID on more than one block is
/// written all the same and counts as written: `_Index.json` says so among
/// the drawing's `problems` and among its own, and the report lists the
/// drawing under [`BackupPublishReport::duplicate_uids`]. That holds for
/// every kind of package, with or without `reference_dir`.
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
    let mut chain_faults: Vec<PackageChainFault> = Vec::new();
    let mut duplicate_uids: Vec<PackageDuplicateUids> = Vec::new();
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
                chain_faults: &mut chain_faults,
                duplicate_uids: &mut duplicate_uids,
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
    write_atomic(&index_path, text.as_bytes()).map_err(|error| {
        BackupPublishError::Io(format!("write {}: {error}", index_path.display()))
    })?;
    Ok(BackupPublishReport {
        failed,
        written,
        index_path,
        pid_entries,
        shells,
        gate,
        chain_faults,
        // No drawing of this branch goes through the database line.
        shortfalls: Vec::new(),
        duplicate_uids,
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
    /// The sentences `_Index.json` lists as `problems`.
    problems: &'a mut Vec<String>,
    /// The chain faults of the drawings read, for the report. Each is also
    /// one of `problems`.
    chain_faults: &'a mut Vec<PackageChainFault>,
    /// The drawings written with a UID on more than one block, for the
    /// report. Each is also one of `problems`.
    duplicate_uids: &'a mut Vec<PackageDuplicateUids>,
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
    // One read: the drawing, and the chain streams of it that do not hold.
    // Those are kept whatever becomes of the drawing below — a stream that
    // stops short is often why it has no `DrawingNo` or publishes little.
    let loaded_drawing = match read_publish_from_reader(std::io::Cursor::new(bytes), &stem) {
        Ok(read) => {
            for fault in read.chain_faults {
                let fault = PackageChainFault {
                    zip_file: plant.file_name.clone(),
                    zip_entry: zip_entry.to_string(),
                    fault,
                };
                // The index says it too: the drawing below may well be
                // written and counted as written, and a reader of the
                // index alone would not know its delivery is short.
                tally.problems.push(fault.describe());
                tally.chain_faults.push(fault);
            }
            read.drawing
        }
        Err(error) => Err(error),
    };
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

    // Only a sheet the dump lists is this dump's revision: another revision
    // could share model item UIDs with it and pick up ports, compositions or
    // pipelines that are not its own, so it is published from the `.pid` alone.
    let dump_in_run = loaded.is_some() && uid_from == "T_DRAWING.SP_ID";
    let mut notes = Vec::new();
    if loaded.is_some() && uid_from == "DrawingNo" {
        let note = format!(
            "DocUID {} has no T_DRAWING row: the dump is another revision of this sheet, \
             so it was published from the .pid alone",
            drawing.drawing_uid
        );
        tally
            .problems
            .push(format!("{} {zip_entry}: {note}", plant.file_name));
        notes.push(note);
    }
    if let Some(loaded) = loaded.filter(|_| dump_in_run) {
        let attached = attach_exp(&mut drawing, loaded)
            .and_then(|()| {
                super::pipeline::attach_pipelines(
                    &mut drawing,
                    &loaded.tables,
                    plant_name,
                    loaded.datamap.as_ref(),
                )
            })
            .and_then(|()| {
                super::codelist_values::attach_codelist_values(
                    &mut drawing,
                    &loaded.tables,
                    loaded.datamap.as_ref(),
                )
                .map(|_| ())
            });
        if let (Ok(()), Some(map)) = (&attached, loaded.datamap.as_ref()) {
            let stored =
                super::stored_values::attach_stored_values(&mut drawing, &loaded.tables, map);
            for reason in stored.unread {
                notes.push(format!("a stored value column was not read: {reason}"));
            }
        }
        if let Err(error) = attached {
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
    let file_stem = output_stem(&drawing.drawing_name);
    if !tally.written_names.insert(file_stem.to_lowercase()) {
        let detail = format!("output name {file_stem} is already used");
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
    let report = pid_coverage_json(&drawing, &source, &file_stem);
    let census = match pid_publish_census(&xml) {
        Ok(census) => census,
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
    let (object_count, value_count, rel_count) = census.counts();
    if let Err(error) = write_three(out_dir, &file_stem, &xml, &meta, &report) {
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
        gate.score(&file_stem, &xml, dump_in_run);
    }
    // What the document states on more than one block. The drawing is
    // written and counts as written; the index says it, in the sentence the
    // caller is handed with the report.
    if let Some(duplicates) = PackageDuplicateUids::of(
        &drawing.drawing_uid,
        &drawing.drawing_name,
        &census.duplicate_uids,
    ) {
        tally.problems.push(duplicates.describe());
        notes.push(duplicates.sentence());
        tally.duplicate_uids.push(duplicates);
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
            format!("{file_stem}_Data.xml"),
            format!("{file_stem}_Meta.xml"),
            format!("{file_stem}_Coverage.json"),
        ],
        "oracle_exp": oracle,
        "problems": notes,
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
    /// 参考文件不存在时记 unpaired（**不是** clean、也不是跳过）。参考文件在、
    /// 但两侧 `<Container DocUID>` 不同时记 `revision_mismatch`：那是同一张图名的
    /// 另一版图纸，逐条比对只会产出与版本差有关的 finding，门禁不再列出它们。
    ///
    /// `dump_in_run`：这张图挂了 dump（或出自数据库）。此时 dump 该提供、却被整类
    /// 豁免的类算 finding，与单图 `--oracle-exp` 同一判定
    /// （[`super::diff::PidExportDiff::is_clean_with_dump`]）。
    fn score(&mut self, name: &str, xml: &str, dump_in_run: bool) {
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
        if let Some(line) = revision_mismatch_line(name, xml, &reference, &reference_path) {
            self.report.revision_mismatch += 1;
            self.report.lines.push(line);
            return;
        }
        match super::diff::diff_pid_export(xml, &reference) {
            Ok(diff) if diff.is_clean_with_dump(dump_in_run) => {
                self.report.clean += 1;
                self.report.lines.push(format!(
                    "{name}: clean (objects {}, values {}, rels {})",
                    diff.objects, diff.values, diff.rels
                ));
            }
            Ok(diff) => {
                let dump_findings: &[(String, usize)] = if dump_in_run {
                    &diff.dump_supplied_excused
                } else {
                    &[]
                };
                self.report.dirty += 1;
                self.report.lines.push(format!(
                    "{name}: dirty (objects {}, values {}, rels {}, findings {})",
                    diff.objects,
                    diff.values,
                    diff.rels,
                    diff.findings.len() + dump_findings.len()
                ));
                for finding in &diff.findings {
                    self.report.lines.push(format!("  {name}: {finding}"));
                }
                for (class, count) in dump_findings {
                    self.report.lines.push(format!(
                        "  {name}: excused {class} ×{count} the dump should supply"
                    ));
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

/// The `revision_mismatch` verdict line, or `None` when the pair is one
/// revision.
///
/// Both documents state their own `<Container DocUID>`. When the two differ,
/// the reference file is another revision of the same drawing name — the
/// `.pid` and the reference disagree about the sheet's identity, not about
/// its content. Scoring them object by object then reports the whole
/// version gap (thousands of findings) and hides that one fact.
fn revision_mismatch_line(
    name: &str,
    generated: &str,
    reference: &str,
    reference_path: &Path,
) -> Option<String> {
    let generated_uid = super::diff::publish_container_doc_uid(generated)?;
    let reference_uid = super::diff::publish_container_doc_uid(reference)?;
    if backup_uid_key(&generated_uid) == backup_uid_key(&reference_uid) {
        return None;
    }
    Some(format!(
        "{name}: revision mismatch — the drawing is DocUID {generated_uid} but {} is \
         DocUID {reference_uid}; the reference is another revision of this name, so the \
         drawing was not compared",
        reference_path.display()
    ))
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
    super::oracle_exp::attach_dump_only_components(drawing, &loaded.tables)?;
    let ports = loaded.tables.port_tables()?;
    attach_oracle_ports(drawing, &ports, &loaded.path);
    drawing.connectivity_ends = loaded.tables.connectivity_ends(&drawing.drawing_uid)?;
    drawing.exp_relations = loaded.tables.composition_relations()?;
    super::unit_values::attach_unit_values(drawing, &loaded.tables)?;
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
        drawing_attributes: Vec::new(),
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

/// Write a drawing's three files into `dir` under `stem`, all of them or
/// none. Each is first written whole under a temporary name, so whatever
/// fails while they are being written fails with the directory as it was.
/// Then they take their names one after the other, and when one cannot,
/// those that did are removed again. The error names the file that could
/// not be written, and any of the three that could not be removed.
///
/// All three take their names by rename, whatever holds the names: they
/// are names this run gives out inside its own output directory. Files an
/// earlier run left under them are replaced one by one, so when a later
/// name cannot be taken, the earlier run's files under the names already
/// taken are gone with this run's, and those under the others stay.
fn write_three(dir: &Path, stem: &str, xml: &str, meta: &str, report: &str) -> Result<(), String> {
    let parts = [
        ("_Data.xml", xml),
        ("_Meta.xml", meta),
        ("_Coverage.json", report),
    ];
    let mut staged = Vec::with_capacity(parts.len());
    for (suffix, contents) in parts {
        let path = dir.join(format!("{stem}{suffix}"));
        let file = StagedFile::stage(&path, contents.as_bytes())
            .map_err(|error| format!("write {}: {error}", path.display()))?;
        staged.push(file);
    }
    let mut in_place: Vec<PathBuf> = Vec::with_capacity(staged.len());
    for file in staged {
        let path = file.target.clone();
        if let Err(error) = file.commit() {
            let mut detail = format!("write {}: {error}", path.display());
            for earlier in &in_place {
                if let Err(error) = fs::remove_file(earlier) {
                    detail = format!("{detail}; {} is left behind: {error}", earlier.display());
                }
            }
            return Err(detail);
        }
        in_place.push(path);
    }
    Ok(())
}

/// Write `contents` to `path` so that `path` never holds a part of them:
/// they are written to a file of another name beside it, which then takes
/// the name. A run that fails or is stopped part-way leaves `path` as it
/// was — absent, or whole with what an earlier run wrote — and what it may
/// leave under the other name, `<file name>.<process id>.tmp`, ends in no
/// suffix a document is looked for by. The contents are not flushed to the
/// disk before they take the name, so this holds against a run that fails
/// or is interrupted, not against a power cut.
///
/// A `path` that names something other than a plain file — a link, a
/// device such as `NUL` or `/dev/null`, a directory — is written the way
/// [`fs::write`] writes it: taking the name would put a file in the place
/// of the link or the device, and a directory fails either way.
///
/// # Errors
///
/// The error of the step that failed: creating or writing the file beside
/// `path`, or giving it the name. That file is removed again.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if !takes_its_name_by_rename(path) {
        return fs::write(path, contents);
    }
    StagedFile::stage(path, contents)?.commit()
}

/// Whether [`write_atomic`] writes `path` beside itself and renames: when
/// nothing is there yet, or a plain file is.
fn takes_its_name_by_rename(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(found) => found.is_file(),
        // A device may refuse to say what it is, and is there all the same.
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

/// A file written whole under a temporary name beside the name it is to
/// take. Dropped before [`StagedFile::commit`], it removes itself.
struct StagedFile {
    temp: PathBuf,
    target: PathBuf,
    in_place: bool,
}

impl StagedFile {
    /// Write `contents` beside `target` as `<file name>.<process id>.tmp`:
    /// in the directory `target` is in, so that taking the name is a rename
    /// within one file system, and under a name that ends in none of the
    /// suffixes a reader of the output directory looks for.
    fn stage(target: &Path, contents: &[u8]) -> io::Result<Self> {
        let mut name = target
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path names no file"))?
            .to_os_string();
        name.push(format!(".{}.tmp", std::process::id()));
        let temp = target.with_file_name(name);
        let mut file = fs::File::create(&temp)?;
        // The file is there now: should the write fail, `staged` drops and
        // the file goes with it, once it is closed.
        let staged = Self {
            temp,
            target: target.to_path_buf(),
            in_place: false,
        };
        let written = file.write_all(contents);
        drop(file);
        written.map(|()| staged)
    }

    /// Give the file its name, in the place of whatever file holds it.
    fn commit(mut self) -> io::Result<()> {
        fs::rename(&self.temp, &self.target)?;
        self.in_place = true;
        Ok(())
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.in_place {
            let _ = fs::remove_file(&self.temp);
        }
    }
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

/// The stem a drawing's three files are written under: its name with each
/// path separator and each character a Windows file name cannot hold
/// replaced by `_`, so a row's name can neither leave `--out-dir` nor fail
/// the write. The name inside the documents is not touched.
fn output_stem(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
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
///
/// A drawing the database line does not write in full is written all the
/// same and counts as written. Its `_Coverage.json` counts what is short
/// under `shortfalls`, `_Index.json` says it among the drawing's
/// `problems` and among its own, and the report lists the drawing under
/// [`BackupPublishReport::shortfalls`].
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
    publish_staged_database(
        &conn,
        dump_path,
        package_dir,
        out_dir,
        plant_name,
        scan,
        reference_dir,
    )
}

/// [`publish_mtf_package`] from the staged database on: every `T_Drawing`
/// row of `conn` is published into `out_dir`, and `_Index.json` is written
/// there. `dump_path` only names the database in an error and
/// `package_dir` the package in the index; the `PlantData` zips `scan`
/// found are listed to pair each drawing with its `.pid` entry.
///
/// Split from the reading of `Export.dmp` so that the tests can publish a
/// database made on the spot: no SQL Server package in the repository
/// holds a drawing the database line writes short.
#[cfg(feature = "backup-mdf")]
fn publish_staged_database(
    conn: &rusqlite::Connection,
    dump_path: &Path,
    package_dir: &Path,
    out_dir: &Path,
    plant_name: &str,
    scan: &BackupFolderScan,
    reference_dir: Option<&Path>,
) -> Result<BackupPublishReport, BackupPublishError> {
    fs::create_dir_all(out_dir).map_err(|error| {
        BackupPublishError::Io(format!("create {}: {error}", out_dir.display()))
    })?;
    let rows = mtf_drawing_rows(conn).map_err(|error| {
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
    let mut shortfalls: Vec<PackageDrawingShortfalls> = Vec::new();
    let mut duplicate_uids: Vec<PackageDuplicateUids> = Vec::new();
    let mut gate = reference_dir.map(PackageGate::new);

    for row in &rows {
        let path_key = backup_path_key(&row.path);
        let (zip_file, zip_entry) = zip_pid
            .get(&path_key)
            .map(|(file, entry)| (file.clone(), entry.clone()))
            .unwrap_or_default();
        let (graph, graph_name, xml, meta) = match mtf_render_drawing(conn, &row.uid, plant_name) {
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
        let file_stem = output_stem(&name);
        if !written_names.insert(file_stem.to_lowercase()) {
            let detail = format!("output name {file_stem} is already used");
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
        let census = match pid_publish_census(&xml) {
            Ok(census) => census,
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
        let counts = census.counts();
        let source = PathBuf::from(format!("{zip_file}:{zip_entry}"));
        let report =
            super::pid_writer::write_database_coverage_json(&graph, &source, counts, &file_stem);
        if let Err(detail) = write_three(out_dir, &file_stem, &xml, &meta, &report) {
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
            // The plant database is in the run: it supplies what the dump would.
            gate.score(&file_stem, &xml, true);
        }
        objects += counts.0;
        values += counts.1;
        relationships += counts.2;
        pipelines += xml.matches("<PIDPipeline>").count();
        // `piping_connectors` counts relations in both branches.
        piping_connectors += xml.matches(r#"DefUID="PipingConnectors""#).count();
        if zip_pid.contains_key(&path_key) {
            paired += 1;
            paired_keys.insert(path_key);
        }
        // What the writer could not write of the drawing in full. It is
        // written and counts as written; the index says what is short of
        // it, in the sentences the caller is handed with the report.
        let short = PackageDrawingShortfalls {
            drawing_uid: row.uid.clone(),
            drawing_name: name.clone(),
            shortfalls: super::xml_writer::database_shortfalls(&graph),
        };
        problems.extend(short.describe());
        let mut own_problems = short.shortfalls.describe();
        // What the document states on more than one block is said the same
        // way, after what is short of it.
        if let Some(duplicates) = PackageDuplicateUids::of(&row.uid, &name, &census.duplicate_uids)
        {
            problems.push(duplicates.describe());
            own_problems.push(duplicates.sentence());
            duplicate_uids.push(duplicates);
        }
        drawings.push(mtf_drawing_entry(
            row,
            &name,
            (&zip_file, &zip_entry),
            counts,
            graph.template.clone().unwrap_or_default(),
            vec![
                format!("{file_stem}_Data.xml"),
                format!("{file_stem}_Meta.xml"),
                format!("{file_stem}_Coverage.json"),
            ],
            own_problems,
        ));
        if !short.shortfalls.is_empty() {
            shortfalls.push(short);
        }
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
    write_atomic(&index_path, text.as_bytes()).map_err(|error| {
        BackupPublishError::Io(format!("write {}: {error}", index_path.display()))
    })?;
    Ok(BackupPublishReport {
        failed,
        written,
        index_path,
        pid_entries,
        shells: 0,
        gate: gate.map(|gate| gate.report),
        // The drawings of this branch come out of the database: no `.pid`
        // is inflated, so no chain is walked.
        chain_faults: Vec::new(),
        shortfalls,
        duplicate_uids,
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
    fn the_package_gate_holds_a_dump_run_to_the_classes_the_dump_supplies() {
        let dir = std::env::temp_dir().join(format!("pid-parse-gate-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("reference dir");
        fs::write(
            dir.join("X_Data.xml"),
            r#"<Container DocUID="D">
                <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
                <PIDSignalPort><IObject UID="A.1"/></PIDSignalPort>
            </Container>"#,
        )
        .expect("reference");
        let generated = r#"<Container DocUID="D">
            <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        </Container>"#;
        let mut pid_only = PackageGate::new(&dir);
        pid_only.score("X", generated, false);
        let mut with_dump = PackageGate::new(&dir);
        with_dump.score("X", generated, true);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!((pid_only.report.clean, pid_only.report.dirty), (1, 0));
        assert_eq!((with_dump.report.clean, with_dump.report.dirty), (0, 1));
        assert!(
            with_dump
                .report
                .lines
                .iter()
                .any(|line| line.ends_with("excused PIDSignalPort ×1 the dump should supply")),
            "{:?}",
            with_dump.report.lines
        );
    }

    #[test]
    fn an_output_stem_stays_inside_the_output_directory() {
        assert_eq!(
            output_stem("I井场 注采阀组工艺及自控流程图"),
            "I井场 注采阀组工艺及自控流程图"
        );
        assert_eq!(output_stem(r"..\..\Windows\x"), ".._.._Windows_x");
        assert_eq!(output_stem("C:/plant/A01"), "C__plant_A01");
        assert_eq!(output_stem("a*b?c\"d<e>f|g\u{1}"), "a_b_c_d_e_f_g_");
        let out = Path::new("out");
        assert_eq!(
            out.join(format!("{}_Data.xml", output_stem(r"C:\x")))
                .parent(),
            Some(out)
        );
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

    /// One drawing the database line cannot write in full: an `Exchanger`,
    /// which has no writer of its own, and a `T_Relationship` row from a
    /// nozzle to it. `tests/publish_xml_cli.rs` publishes the same file
    /// through the command line.
    #[cfg(feature = "backup-mdf")]
    const SHORTFALLS_SQL: &str = include_str!("../../tests/fixtures/publish-shortfalls.sql");

    #[cfg(feature = "backup-mdf")]
    const PLACEHOLDER_SENTENCE: &str =
        "1 object written as a <PIDItem> placeholder, class and attributes left out: Exchanger 1";

    #[cfg(feature = "backup-mdf")]
    const RELATIONSHIP_SENTENCE: &str = "1 relationship row defined from the item types of its \
                                         ends alone: Nozzle -> Exchanger as Relationship 1";

    /// What one [`publish_staged_database`] run left: the report, and the
    /// files of the one drawing the fixture holds.
    #[cfg(feature = "backup-mdf")]
    struct StagedRun {
        report: BackupPublishReport,
        index: Value,
        coverage: Value,
        data_xml: String,
    }

    /// Publish a database made on the spot from `sql` the way the staged
    /// database of a SQL Server package is published. The package
    /// directory is empty, so no `.pid` entry pairs with the drawing.
    #[cfg(feature = "backup-mdf")]
    fn publish_staged(tag: &str, sql: &str) -> StagedRun {
        let (root, report) = publish_staged_into(tag, sql, |_| {});
        let out = root.join("out");
        let read = |file: &str| {
            fs::read_to_string(out.join(file)).unwrap_or_else(|error| panic!("{file}: {error}"))
        };
        let json = |file: &str| -> Value {
            serde_json::from_str(&read(file)).unwrap_or_else(|error| panic!("{file}: {error}"))
        };
        let run = StagedRun {
            report,
            index: json("_Index.json"),
            coverage: json("SHORT-01_Coverage.json"),
            data_xml: read("SHORT-01_Data.xml"),
        };
        let _ = fs::remove_dir_all(&root);
        run
    }

    /// [`publish_staged`] up to the reading of what it wrote. `prepare` is
    /// handed the output directory before the run creates it; the caller
    /// reads `<root>/out` and removes `<root>`, which is what comes back
    /// with the report.
    #[cfg(feature = "backup-mdf")]
    fn publish_staged_into(
        tag: &str,
        sql: &str,
        prepare: impl FnOnce(&Path),
    ) -> (PathBuf, BackupPublishReport) {
        let root =
            std::env::temp_dir().join(format!("pid-parse-staged-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let package = root.join("package");
        fs::create_dir_all(&package).expect("package dir");
        let scan = scan_backup_folder(&package).expect("scan the empty package");
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(sql).expect("fixture sql");
        let out = root.join("out");
        prepare(&out);
        let report = publish_staged_database(
            &conn,
            Path::new("Export.dmp"),
            &package,
            &out,
            "P01",
            &scan,
            None,
        )
        .expect("the staged database publishes");
        (root, report)
    }

    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_written_short_says_so_in_its_coverage_report() {
        let run = publish_staged("coverage", SHORTFALLS_SQL);
        assert_eq!(
            run.coverage["shortfalls"],
            json!({
                "placeholder_objects": { "total": 1, "by_item_type": { "Exchanger": 1 } },
                "relationship_rows_defined_by_item_type": {
                    "total": 1,
                    "rows": [{
                        "item1": "Nozzle",
                        "item2": "Exchanger",
                        "written_as": "Relationship",
                        "count": 1,
                    }],
                },
            }),
            "{}",
            run.coverage
        );
        // The counts are those of the tags in the document beside it.
        assert_eq!(run.data_xml.matches("<PIDItem>").count(), 1);
        assert_eq!(run.data_xml.matches(r#"DefUID="Relationship""#).count(), 1);
        // The rest of the report stands.
        assert_eq!(
            run.coverage["document_counts"],
            json!({ "objects": 7, "values": 15, "relationships": 7 }),
            "{}",
            run.coverage
        );
    }

    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_written_short_is_written_and_listed_among_the_problems() {
        let run = publish_staged("index", SHORTFALLS_SQL);
        let index = &run.index;
        assert_eq!(index["failed"], 0, "{index}");
        assert_eq!(index["drawing_rows"], 1, "{index}");
        assert_eq!(index["objects"], 7, "its counts are in the totals: {index}");
        let drawing = &index["drawings"][0];
        assert_eq!(drawing["name"], "SHORT-01", "{index}");
        assert_eq!(
            drawing["files"],
            json!([
                "SHORT-01_Data.xml",
                "SHORT-01_Meta.xml",
                "SHORT-01_Coverage.json"
            ]),
            "the drawing is written: {index}"
        );
        assert_eq!(
            drawing["problems"],
            json!([PLACEHOLDER_SENTENCE, RELATIONSHIP_SENTENCE]),
            "{index}"
        );
        assert_eq!(
            index["problems"],
            json!([
                format!("SHORT-01: {PLACEHOLDER_SENTENCE}"),
                format!("SHORT-01: {RELATIONSHIP_SENTENCE}"),
            ]),
            "{index}"
        );
        assert_eq!((run.report.written, run.report.failed), (1, 0));
    }

    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_written_short_is_named_in_the_report() {
        let run = publish_staged("report", SHORTFALLS_SQL);
        let mut expected = DatabaseShortfalls::default();
        expected.placeholder_objects.insert("Exchanger".into(), 1);
        expected.relationship_rows_by_item_type.insert(
            ("Nozzle".into(), "Exchanger".into(), "Relationship".into()),
            1,
        );
        assert_eq!(
            run.report.shortfalls,
            vec![PackageDrawingShortfalls {
                drawing_uid: "DWG-SHORT-01".into(),
                drawing_name: "SHORT-01".into(),
                shortfalls: expected,
            }]
        );
        assert_eq!(
            run.report.shortfalls[0].describe(),
            [
                format!("SHORT-01: {PLACEHOLDER_SENTENCE}"),
                format!("SHORT-01: {RELATIONSHIP_SENTENCE}"),
            ],
            "the sentences `_Index.json` lists"
        );
        assert!(run.report.chain_faults.is_empty());
    }

    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_written_in_full_has_the_section_and_no_problem() {
        // The fixture without the exchanger and the row that ends at it.
        let sql = format!(
            "{SHORTFALLS_SQL}\n\
             DELETE FROM T_Relationship;\n\
             DELETE FROM T_Representation WHERE SP_ModelItemID = 'EX-1';\n\
             DELETE FROM T_ModelItem WHERE SP_ID = 'EX-1';\n"
        );
        let run = publish_staged("full", &sql);
        assert_eq!(
            run.coverage["shortfalls"],
            json!({
                "placeholder_objects": { "total": 0, "by_item_type": {} },
                "relationship_rows_defined_by_item_type": { "total": 0, "rows": [] },
            }),
            "{}",
            run.coverage
        );
        assert!(!run.data_xml.contains("<PIDItem>"), "{}", run.data_xml);
        let index = &run.index;
        assert_eq!(index["problems"], json!([]), "{index}");
        assert_eq!(index["drawings"][0]["problems"], json!([]), "{index}");
        assert_eq!(index["failed"], 0, "{index}");
        assert!(run.report.shortfalls.is_empty());
        assert!(run.report.duplicate_uids.is_empty());
        assert_eq!((run.report.written, run.report.failed), (1, 0));
    }

    /// What is said of the one UID [`sql_with_a_uid_stated_twice`] makes the
    /// document state twice.
    #[cfg(feature = "backup-mdf")]
    const DUPLICATE_SENTENCE: &str = "1 UID written more than once: V-1 ×2";

    /// The fixture written in full, with a second representation of the
    /// nozzle whose `SP_ID` is the vessel's: the vessel's block and that
    /// representation's then state one UID. A plant database keeps an
    /// `SP_ID` to one row of all its tables, so no real one writes this.
    #[cfg(feature = "backup-mdf")]
    fn sql_with_a_uid_stated_twice() -> String {
        format!(
            "{SHORTFALLS_SQL}\n\
             DELETE FROM T_Relationship;\n\
             DELETE FROM T_Representation WHERE SP_ModelItemID = 'EX-1';\n\
             DELETE FROM T_ModelItem WHERE SP_ID = 'EX-1';\n\
             INSERT INTO T_Representation VALUES ('V-1', 'N-1', 'DWG-SHORT-01', '104', \
             '\\Equipment Components\\Nozzles\\Flanged Nozzle.sym', '13');\n"
        )
    }

    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_that_states_a_uid_twice_is_written_and_named() {
        let run = publish_staged("duplicate", &sql_with_a_uid_stated_twice());
        assert_eq!(
            run.data_xml.matches(r#"<IObject UID="V-1""#).count(),
            2,
            "the document states the UID twice: {}",
            run.data_xml
        );
        let index = &run.index;
        assert_eq!(index["failed"], 0, "{index}");
        let drawing = &index["drawings"][0];
        assert_eq!(
            drawing["files"],
            json!([
                "SHORT-01_Data.xml",
                "SHORT-01_Meta.xml",
                "SHORT-01_Coverage.json"
            ]),
            "the drawing is written: {index}"
        );
        assert_eq!((run.report.written, run.report.failed), (1, 0));
        assert_eq!(
            index["problems"],
            json!([format!("SHORT-01: {DUPLICATE_SENTENCE}")]),
            "{index}"
        );
        assert_eq!(drawing["problems"], json!([DUPLICATE_SENTENCE]), "{index}");
        let listed = PackageDuplicateUids {
            drawing_uid: "DWG-SHORT-01".into(),
            drawing_name: "SHORT-01".into(),
            uids: vec![("V-1".into(), 2)],
        };
        assert_eq!(listed.sentence(), DUPLICATE_SENTENCE);
        assert_eq!(run.report.duplicate_uids, vec![listed]);
        assert!(run.report.shortfalls.is_empty());
    }

    #[test]
    fn the_uids_stated_again_are_counted_by_block_and_said_under_the_drawings_name() {
        // What the document reader hands over: one entry for each block
        // that states a UID again.
        let stated_again = ["B".to_string(), "A".to_string(), "B".to_string()];
        let duplicates =
            PackageDuplicateUids::of("DWG-1", "D1", &stated_again).expect("two UIDs stated again");
        assert_eq!(
            duplicates.uids,
            [("B".to_string(), 3), ("A".to_string(), 2)],
            "each with the blocks that state it, the first one included"
        );
        assert_eq!(
            duplicates.sentence(),
            "2 UIDs written more than once: B ×3, A ×2"
        );
        assert_eq!(
            duplicates.describe(),
            "D1: 2 UIDs written more than once: B ×3, A ×2"
        );
        assert_eq!(PackageDuplicateUids::of("DWG-1", "D1", &[]), None);
    }

    /// A staged drawing one of whose three files cannot be written leaves
    /// none of them. The name of `SHORT-01`'s coverage report is taken by a
    /// directory here, so the two files that reached their names are removed
    /// again and nothing stays under a temporary name. The index says which
    /// file it was and lists the drawing without files, and the drawing
    /// after it is written.
    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them() {
        // The fixture with a second drawing, which holds nothing.
        let sql = format!(
            "{SHORTFALLS_SQL}\n\
             INSERT INTO T_Drawing VALUES ('DWG-OTHER-01', 'OTHER-01', 'P&ID Documents', \
             'P&ID', 'A2-W-New.pid', '\\01\\01\\OTHER-01.pid', '2026/11/16 10:00:00');\n"
        );
        let (root, report) = publish_staged_into("unwritten", &sql, |out| {
            fs::create_dir_all(out.join("SHORT-01_Coverage.json"))
                .expect("a directory under the coverage report's name");
        });
        let out = root.join("out");
        let mut held: Vec<String> = fs::read_dir(&out)
            .expect("the output directory")
            .map(|entry| {
                entry
                    .expect("a directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        held.sort();
        let in_the_way = fs::read_dir(out.join("SHORT-01_Coverage.json"))
            .map(Iterator::count)
            .ok();
        let index: Value = serde_json::from_str(
            &fs::read_to_string(out.join("_Index.json")).expect("_Index.json"),
        )
        .expect("index json");
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            held,
            [
                "OTHER-01_Coverage.json",
                "OTHER-01_Data.xml",
                "OTHER-01_Meta.xml",
                "SHORT-01_Coverage.json",
                "_Index.json",
            ],
            "{index}"
        );
        assert_eq!(in_the_way, Some(0), "the directory in the way stays empty");
        assert_eq!((report.written, report.failed), (1, 1));
        assert_eq!(index["failed"], 1, "{index}");
        let problems: Vec<&str> = index["problems"]
            .as_array()
            .expect("problems")
            .iter()
            .map(|problem| problem.as_str().expect("a sentence"))
            .collect();
        let [problem] = problems.as_slice() else {
            panic!("one problem: {problems:?}");
        };
        let detail = problem
            .strip_prefix("SHORT-01: ")
            .unwrap_or_else(|| panic!("the drawing is named: {problem}"));
        assert!(
            detail.starts_with("write ") && detail.contains("SHORT-01_Coverage.json: "),
            "{problem}"
        );
        let drawings = index["drawings"].as_array().expect("drawings");
        assert_eq!(drawings.len(), 2, "{index}");
        assert_eq!(drawings[0]["name"], "SHORT-01");
        assert_eq!(drawings[0]["files"], json!([]), "{index}");
        assert_eq!(drawings[0]["problems"], json!([detail]), "{index}");
        assert_eq!(drawings[1]["name"], "OTHER-01");
        assert_eq!(
            drawings[1]["files"],
            json!([
                "OTHER-01_Data.xml",
                "OTHER-01_Meta.xml",
                "OTHER-01_Coverage.json"
            ]),
            "{index}"
        );
        assert_eq!(drawings[1]["problems"], json!([]), "{index}");
        // A drawing that failed is not also said to be written short.
        assert!(report.shortfalls.is_empty());
    }

    /// A directory of one test's own under the system temp dir, removed
    /// again when the value drops.
    struct WriteScratch(PathBuf);

    impl WriteScratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("pid-parse-write-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch directory");
            Self(dir)
        }

        /// The names the directory holds, sorted.
        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.0)
                .expect("scratch directory")
                .map(|entry| {
                    entry
                        .expect("a directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for WriteScratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// What a run stopped between the writing of a file and its taking its
    /// name leaves: one file, beside the target, whose name ends in none of
    /// the suffixes the output directory is read by.
    #[test]
    fn a_staged_file_waits_under_a_name_no_document_is_looked_for_by() {
        let scratch = WriteScratch::new("staged");
        for suffix in ["_Data.xml", "_Meta.xml", "_Coverage.json", "_Index.json"] {
            let target = scratch.0.join(format!("A01{suffix}"));
            let staged = StagedFile::stage(&target, b"whole").expect("staged");
            let names = scratch.names();
            let [name] = names.as_slice() else {
                panic!("one file while staged: {names:?}");
            };
            assert_eq!(
                *name,
                format!("A01{suffix}.{}.tmp", std::process::id()),
                "the target's name is not taken yet"
            );
            assert_eq!(staged.temp, scratch.0.join(name));
            for looked_for in ["_Data.xml", "_Meta.xml", "_Coverage.json", "_Index.json"] {
                assert!(!name.ends_with(looked_for), "{name}");
            }
            assert_eq!(fs::read(&staged.temp).expect("the staged file"), b"whole");

            staged.commit().expect("the file takes its name");
            assert_eq!(scratch.names(), [format!("A01{suffix}")]);
            assert_eq!(fs::read(&target).expect("the target"), b"whole");
            fs::remove_file(&target).expect("cleared for the next suffix");
        }
        // Dropped before it took its name, as when a later step of the run
        // fails, it leaves nothing.
        let target = scratch.0.join("A01_Data.xml");
        drop(StagedFile::stage(&target, b"whole").expect("staged"));
        assert_eq!(scratch.names(), Vec::<String>::new());
    }

    #[test]
    fn write_atomic_puts_the_whole_contents_in_the_place_of_a_file() {
        let scratch = WriteScratch::new("atomic");
        let path = scratch.0.join("_Index.json");
        write_atomic(&path, b"first").expect("nothing is there yet");
        assert_eq!(fs::read(&path).expect("the file"), b"first");
        write_atomic(&path, b"second, and longer").expect("a file is there");
        assert_eq!(fs::read(&path).expect("the file"), b"second, and longer");
        write_atomic(&path, b"3").expect("a file is there");
        assert_eq!(fs::read(&path).expect("the file"), b"3");
        assert_eq!(
            scratch.names(),
            ["_Index.json"],
            "nothing is left beside it"
        );
    }

    #[test]
    fn write_atomic_fails_without_leaving_a_file_behind() {
        let scratch = WriteScratch::new("atomic-fails");
        // The name is a directory's, so nothing can be written under it;
        let taken = scratch.0.join("A01_Coverage.json");
        fs::create_dir(&taken).expect("a directory under the name");
        assert!(write_atomic(&taken, b"report").is_err());
        // and there is no directory to write this one into.
        let nowhere = scratch.0.join("missing").join("A01_Data.xml");
        assert!(write_atomic(&nowhere, b"document").is_err());
        assert_eq!(scratch.names(), ["A01_Coverage.json"]);
        assert_eq!(
            fs::read_dir(&taken).map(Iterator::count).ok(),
            Some(0),
            "the directory is left as it was"
        );
    }

    #[cfg(windows)]
    const NULL_DEVICE: &str = "NUL";
    #[cfg(unix)]
    const NULL_DEVICE: &str = "/dev/null";

    /// Renaming over a name would put a plain file where a directory or a
    /// device is, so only a free name and a plain file are renamed over.
    #[cfg(any(unix, windows))]
    #[test]
    fn only_a_free_name_or_a_plain_file_is_renamed_over() {
        let scratch = WriteScratch::new("route");
        let file = scratch.0.join("A01_Data.xml");
        fs::write(&file, b"earlier").expect("a file");
        assert!(takes_its_name_by_rename(&file), "a plain file");
        assert!(
            takes_its_name_by_rename(&scratch.0.join("free")),
            "a free name"
        );
        assert!(!takes_its_name_by_rename(&scratch.0), "a directory");
        assert!(
            !takes_its_name_by_rename(Path::new(NULL_DEVICE)),
            "the null device"
        );
    }

    /// The null device takes what is written to it, as it did while the
    /// files were written in place: it is not a plain file, so nothing is
    /// renamed over it.
    #[cfg(windows)]
    #[test]
    fn write_atomic_writes_a_device_in_place() {
        for device in [NULL_DEVICE, r"\\.\NUL"] {
            let device = Path::new(device);
            assert!(!takes_its_name_by_rename(device), "{}", device.display());
            write_atomic(device, b"discarded").expect("the null device takes it");
        }
    }

    #[test]
    fn three_files_are_written_all_of_them_or_none() {
        let scratch = WriteScratch::new("three");
        write_three(&scratch.0, "A01", "data", "meta", "report").expect("all three");
        let written = ["A01_Coverage.json", "A01_Data.xml", "A01_Meta.xml"];
        assert_eq!(scratch.names(), written);
        for (file, contents) in written.iter().zip(["report", "data", "meta"]) {
            assert_eq!(
                fs::read_to_string(scratch.0.join(file)).expect("a written file"),
                contents
            );
        }

        // The third name is a directory's: the two files that took their
        // names are removed again, and nothing waits under another name.
        fs::create_dir(scratch.0.join("B02_Coverage.json")).expect("a directory under the name");
        let error = write_three(&scratch.0, "B02", "data", "meta", "report")
            .expect_err("the third file cannot take its name");
        assert!(
            error.starts_with("write ") && error.contains("B02_Coverage.json: "),
            "{error}"
        );
        assert!(!error.contains("left behind"), "{error}");
        let mut left = written.to_vec();
        left.push("B02_Coverage.json");
        assert_eq!(scratch.names(), left);

        // The first name is a directory's: no file took a name.
        fs::create_dir(scratch.0.join("C03_Data.xml")).expect("a directory under the name");
        let error = write_three(&scratch.0, "C03", "data", "meta", "report")
            .expect_err("the first file cannot take its name");
        assert!(error.contains("C03_Data.xml: "), "{error}");
        left.push("C03_Data.xml");
        assert_eq!(scratch.names(), left);
    }
}
