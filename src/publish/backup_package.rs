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
//! index is `_Index.json` in the output directory; every `.pid` entry the
//! run could list is accounted in it exactly once — paired to a model row,
//! classified as a template under a `Template Files` folder, or listed
//! under `unmatched_pid_entries`.
//!
//! `failed` in the index counts more than drawings. It is one for each
//! drawing that was not written, which has a row of `drawings` whose
//! `files` is empty, and one for each `PlantData` zip that could not be
//! opened and each entry of a zip whose header could not be read. Those
//! two have no row and are in none of the `.pid` counts: a sentence of
//! `problems` is all that says them. A zip that cannot be opened counts
//! one however many drawings it holds, and an entry that cannot be read
//! counts one whether it is a `.pid` or not. For a SQL Server package the
//! zips are only listed, so `failed` can be above 0 with every drawing
//! written.
//!
//! A `.pid` entry is unpacked into memory. One that would unpack to more
//! than 512 MiB is not unpacked: it is a drawing that fails, by name.
//!
//! A drawing's three files and the index each take their name only once
//! written whole ([`write_atomic`]), so a run that is stopped leaves no
//! part of a document under a document's name; it can leave temporary
//! files, and a drawing with only some of its three files. When one of
//! the three cannot be written, those already in place are removed again:
//! a drawing the index lists without files has none from this run. The
//! output directory is not emptied first: files an earlier run left there
//! are not removed, and `_Index.json` is what says which files this run
//! wrote.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::diff::pid_publish_census;
use super::model::DatabaseShortfalls;
use super::oracle_exp::{
    attach_dump, backup_path_key, backup_uid_key, plant_group_path, read_plant_groups,
    BackupDrawing, DumpAttachment, ExpTables, PlantGroup,
};
use super::pid_load::{
    read_publish_from_reader, PidLoadError, PidPublishDrawing, PidPublishStats, PublishChainFault,
};
use super::pid_writer::{
    json_file_text, pid_coverage_json, write_pid_data_xml, write_pid_meta_xml,
};
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
    /// What the run could not do, counted: the `failed` of `_Index.json`.
    /// One for each drawing that was not written, which has a row of the
    /// index's `drawings` whose `files` is empty, and one for each
    /// `PlantData` zip that could not be opened and each entry of a zip
    /// whose header could not be read. Those two have no row: a sentence
    /// of the index's `problems` is all that says them. So this is not a
    /// number of drawings. A zip that cannot be opened counts one however
    /// many drawings it holds, and an entry that cannot be read counts one
    /// whether it is a `.pid` or not. For a SQL Server package, whose zips
    /// are only listed, it can be above 0 with every drawing written.
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
/// A `.pid` entry of a `PlantData` zip that would unpack to more than
/// 512 MiB is not unpacked: that drawing fails, `_Index.json` names it
/// among the `problems`, and the others are published all the same.
///
/// # Errors
///
/// Returns [`BackupPublishError::Usage`] when the directory holds no
/// `Manifest.txt`, when `Export.dmp` has an unrecognised header, or when
/// it is a SQL Server MTF tape while this build lacks the `backup-mdf`
/// feature. Returns [`BackupPublishError::Io`] when the directory, the
/// dump or the data map of the package cannot be read, or when the output
/// directory or `_Index.json` cannot be written. A drawing that fails
/// returns no error, and neither does a `PlantData` zip or an entry of
/// one that cannot be opened: each is counted in
/// [`BackupPublishReport::failed`] and said among the `problems` of
/// `_Index.json`.
///
/// `reference_dir` 给定时，每个写出的图都会与 `<reference_dir>/<图名>_Data.xml` 打分。
pub fn publish_backup_package(
    package_dir: &Path,
    out_dir: &Path,
    plant_name: &str,
    reference_dir: Option<&Path>,
) -> Result<BackupPublishReport, BackupPublishError> {
    publish_package(
        package_dir,
        out_dir,
        plant_name,
        reference_dir,
        MAX_PID_ENTRY_BYTES,
    )
}

/// [`publish_backup_package`], with the most bytes a `.pid` entry may
/// unpack to as an argument, so that a test can set it low.
fn publish_package(
    package_dir: &Path,
    out_dir: &Path,
    plant_name: &str,
    reference_dir: Option<&Path>,
    max_pid_entry_bytes: u64,
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

    let mut tally = PublishTally {
        problems: scan
            .problems
            .iter()
            .map(|problem| format!("{}: {}", problem.path.display(), problem.detail))
            .collect(),
        gate: reference_dir.map(PackageGate::new),
        ..PublishTally::default()
    };

    for plant in &scan.plant_data {
        if !matches!(plant.format, crate::backup::RefDataFormat::Zip) {
            continue;
        }
        if let Err(problem) = publish_zip(
            plant,
            out_dir,
            plant_name,
            loaded.as_ref(),
            max_pid_entry_bytes,
            &mut tally,
        ) {
            tally.fail_unlisted(problem);
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
    let excluded_from_totals = excluded_from_totals(&tally.drawings);
    // Every `.pid` entry is counted exactly once — paired to a model row
    // (`paired`), classified as a template (`template_pid_entries`), or
    // listed here: the package shipped the `.pid`, but the dump's
    // `T_DRAWING` does not hold it. With no dump at all every entry lands
    // in `unmatched_pid_entries`; that is the intended, machine-checkable
    // accounting for a `.pid`-only package.
    let mut unmatched_pid_entries: Vec<String> = tally
        .entry_names
        .iter()
        .filter(|entry| !tally.paired_entries.contains(*entry))
        .cloned()
        .collect();
    unmatched_pid_entries.sort();
    let index = json!({
        "package": package_dir.display().to_string(),
        "export": export_kind,
        "pid_entries": tally.pid_entries,
        "drawing_rows": drawing_rows,
        "paired": tally.paired,
        "failed": tally.failed,
        "shells": tally.shells,
        "objects": tally.objects,
        "values": tally.values,
        "relationships": tally.relationships,
        "excluded_from_totals": excluded_from_totals,
        "piping_ports": tally.piping_ports,
        "signal_ports": tally.signal_ports,
        "connectivity_rels": tally.connectivity_rels,
        "opc_branch_points": tally.opc_branch_points,
        "equipment_composition_rels": tally.equipment_composition_rels,
        "item_note_rels": tally.item_note_rels,
        "pipelines": tally.pipelines,
        "piping_connectors": tally.piping_connectors,
        "pipeline_uids": if dump_path.is_some() {
            "synthesized"
        } else {
            "not synthesized"
        },
        "template_pid_entries": tally.template_pid_entries,
        "unmatched_pid_entries": unmatched_pid_entries,
        "drawings": tally.drawings,
        "problems": tally.problems,
    });
    let index_path = out_dir.join("_Index.json");
    write_atomic(&index_path, json_file_text(&index).as_bytes()).map_err(|error| {
        BackupPublishError::Io(format!("write {}: {error}", index_path.display()))
    })?;
    Ok(BackupPublishReport {
        failed: tally.failed,
        written: tally.written,
        index_path,
        pid_entries: tally.pid_entries,
        shells: tally.shells,
        gate: tally.gate.map(|gate| gate.report),
        chain_faults: tally.chain_faults,
        // No drawing of this branch goes through the database line.
        shortfalls: Vec::new(),
        duplicate_uids: tally.duplicate_uids,
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

/// Where the UID of a drawing listed in `_Index.json` came from: the
/// drawing's `uid_from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UidFrom {
    /// The `DrawingNo` the `.pid` states. No row of a dump holds the
    /// drawing: the package has no dump, or the dump is another revision's.
    DrawingNo,
    /// The `DrawingNo` the `.pid` states, and it is the `SP_ID` of a
    /// `T_DRAWING` row of the dump.
    DumpSpId,
    /// The `SP_ID` of the `T_DRAWING` row whose `PATH` is the zip entry.
    /// The `.pid` states no `DrawingNo`, and the drawing is a shell.
    DumpPath,
    /// The `SP_ID` of a `T_Drawing` row of the SQL Server branch, which
    /// publishes every drawing from its row.
    #[cfg(feature = "backup-mdf")]
    DatabaseSpId,
}

impl UidFrom {
    /// What `_Index.json` states, letter for letter as it always has. The
    /// Oracle branch spells the table `T_DRAWING` and the SQL Server branch
    /// `T_Drawing`, and a reader of the index may match on either.
    const fn as_str(self) -> &'static str {
        match self {
            Self::DrawingNo => "DrawingNo",
            Self::DumpSpId => "T_DRAWING.SP_ID",
            Self::DumpPath => "T_DRAWING.PATH",
            #[cfg(feature = "backup-mdf")]
            Self::DatabaseSpId => "T_Drawing.SP_ID",
        }
    }
}

/// What a run over a package's `PlantData` zips has counted and listed so
/// far: the totals and the lists of `_Index.json` and of the report.
#[derive(Default)]
struct PublishTally {
    failed: usize,
    written: usize,
    shells: usize,
    pid_entries: usize,
    paired: usize,
    objects: usize,
    values: usize,
    relationships: usize,
    piping_ports: usize,
    signal_ports: usize,
    connectivity_rels: usize,
    opc_branch_points: usize,
    equipment_composition_rels: usize,
    item_note_rels: usize,
    pipelines: usize,
    piping_connectors: usize,
    template_pid_entries: usize,
    entry_names: Vec<String>,
    paired_entries: HashSet<String>,
    written_names: HashSet<String>,
    drawings: Vec<Value>,
    /// The sentences `_Index.json` lists as `problems`.
    problems: Vec<String>,
    /// The chain faults of the drawings read, for the report. Each is also
    /// one of `problems`.
    chain_faults: Vec<PackageChainFault>,
    /// The drawings written with a UID on more than one block, for the
    /// report. Each is also one of `problems`.
    duplicate_uids: Vec<PackageDuplicateUids>,
    gate: Option<PackageGate>,
}

impl PublishTally {
    /// One of `problems`, under the name of the `.pid` entry it is about.
    fn problem(&mut self, zip_file: &str, zip_entry: &str, detail: &str) {
        self.problems
            .push(format!("{zip_file} {zip_entry}: {detail}"));
    }

    /// The `.pid` entry `zip_entry` of `zip_file` fails for `detail`. That
    /// is one failure, one of `problems` under the entry's name, and a row
    /// of `drawings` that holds the entry's name and `detail` and nothing
    /// else.
    fn fail(&mut self, zip_file: &str, zip_entry: &str, detail: &str) {
        self.fail_listed_as(zip_file, zip_entry, detail, detail);
    }

    /// [`Self::fail`], where the drawing's row words the reason as
    /// `listed` and not as `detail`.
    fn fail_listed_as(&mut self, zip_file: &str, zip_entry: &str, detail: &str, listed: &str) {
        self.problem(zip_file, zip_entry, detail);
        self.failed += 1;
        self.drawings
            .push(failed_drawing(zip_file, zip_entry, listed));
    }

    /// A failure that has no `.pid` entry to be listed under: a zip that
    /// cannot be opened, or an entry of one whose header cannot be read.
    /// That is one failure and one of `problems`, `problem` being the whole
    /// sentence, and no row of `drawings`.
    fn fail_unlisted(&mut self, problem: String) {
        self.problems.push(problem);
        self.failed += 1;
    }

    /// A chain stream of a drawing that does not hold: one for the report
    /// and one of `problems`. The index says it too because the drawing may
    /// well be written and counted as written, and a reader of the index
    /// alone would not know its delivery is short.
    fn chain_fault(&mut self, fault: PackageChainFault) {
        self.problems.push(fault.describe());
        self.chain_faults.push(fault);
    }

    /// A drawing written with a UID on more than one block: one for the
    /// report and one of `problems`. The drawing still counts as written.
    fn duplicates(&mut self, duplicates: PackageDuplicateUids) {
        self.problems.push(duplicates.describe());
        self.duplicate_uids.push(duplicates);
    }

    /// A drawing whose three files are written, counted. It is paired when
    /// a row of the dump holds it. When that row was found by the entry's
    /// path the drawing is a shell, and its objects, values and
    /// relationships (`counts`) stay out of the totals:
    /// [`excluded_from_totals`] declares them. `oracle` is the `oracle_exp`
    /// of the drawing's coverage report, whose counts add to the totals of
    /// the same names.
    fn count_written(
        &mut self,
        zip_file: &str,
        zip_entry: &str,
        uid_from: UidFrom,
        counts: (usize, usize, usize),
        oracle: Option<&Value>,
    ) {
        self.written += 1;
        if matches!(uid_from, UidFrom::DumpSpId | UidFrom::DumpPath) {
            self.paired += 1;
            self.paired_entries.insert(entry_key(zip_file, zip_entry));
        }
        if uid_from == UidFrom::DumpPath {
            self.shells += 1;
        } else {
            self.objects += counts.0;
            self.values += counts.1;
            self.relationships += counts.2;
        }
        if let Some(oracle) = oracle {
            self.piping_ports += json_usize(oracle, "piping_ports");
            self.signal_ports += json_usize(oracle, "signal_ports");
            self.connectivity_rels += json_usize(oracle, "connectivity_rels");
            self.opc_branch_points += json_usize(oracle, "opc_branch_points");
            self.equipment_composition_rels += json_usize(oracle, "equipment_composition_rels");
            self.item_note_rels += json_usize(oracle, "item_note_rels");
            self.pipelines += json_usize(oracle, "pipelines");
            self.piping_connectors += json_usize(oracle, "piping_connectors");
        }
    }
}

/// The most bytes a `.pid` entry of a `PlantData` zip may unpack to.
///
/// A drawing is unpacked into memory before it is read, so an entry that
/// unpacks without bound would take the run down, and with it every other
/// drawing of the package. The `.pid` files met so far are a few megabytes.
/// An entry over the limit is a drawing that fails, by name, and the rest
/// are still published.
const MAX_PID_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

/// What `entry` unpacks to, when that is `limit` bytes or fewer.
///
/// `stated` is the size the zip states for the entry. An entry that states
/// more than `limit` is refused before a byte of it is unpacked. What a zip
/// states is its word and not a fact, so whatever it states the read itself
/// stops one byte past `limit`, and an entry that got that far is refused
/// too. The error is the sentence for the drawing's `problems`.
fn read_entry_within(entry: &mut impl Read, stated: u64, limit: u64) -> Result<Vec<u8>, String> {
    if stated > limit {
        return Err(format!(
            "would unpack to {stated} bytes, over the limit of {limit} for a .pid entry"
        ));
    }
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if usize::try_from(limit).is_ok_and(|limit| bytes.len() > limit) {
        return Err(format!(
            "states {stated} bytes and unpacks to more than the limit of {limit} for a .pid entry"
        ));
    }
    Ok(bytes)
}

/// Publish the `.pid` entries of one `PlantData` zip, counting and listing
/// each in `tally`. The error is that the zip could not be opened, as the
/// sentence for the index's `problems`; counting it is the caller's.
fn publish_zip(
    plant: &PlantDataFile,
    out_dir: &Path,
    plant_name: &str,
    loaded: Option<&LoadedDump>,
    max_pid_entry_bytes: u64,
    tally: &mut PublishTally,
) -> Result<(), String> {
    let file = fs::File::open(&plant.full_path)
        .map_err(|error| format!("open {}: {error}", plant.full_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("open {}: {error}", plant.full_path.display()))?;
    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                tally.fail_unlisted(format!("{} entry {index}: {error}", plant.file_name));
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
        tally.pid_entries += 1;
        if is_template_pid_entry(&zip_entry) {
            // A template ships in the reference-data tree: it is not a
            // drawing, so it is neither published nor reported as a `.pid`
            // the model store lost.
            tally.template_pid_entries += 1;
            continue;
        }
        tally
            .entry_names
            .push(entry_key(&plant.file_name, &zip_entry));
        let stated = entry.size();
        let bytes = match read_entry_within(&mut entry, stated, max_pid_entry_bytes) {
            Ok(bytes) => bytes,
            Err(detail) => {
                // The drawing's row says `read: ` before the reason, and
                // the index's `problems` does not.
                tally.fail_listed_as(
                    &plant.file_name,
                    &zip_entry,
                    &detail,
                    &format!("read: {detail}"),
                );
                continue;
            }
        };
        drop(entry);
        if let Err(detail) = publish_one(
            plant, &zip_entry, &bytes, out_dir, plant_name, loaded, tally,
        ) {
            tally.fail(&plant.file_name, &zip_entry, &detail);
        }
    }
    Ok(())
}

/// Publish one `.pid` entry of a `PlantData` zip: write the drawing's three
/// files, and count and list it in `tally`. The error is why the entry
/// fails, as the sentence for its `problems`; counting and listing the
/// failure is the caller's.
fn publish_one(
    plant: &PlantDataFile,
    zip_entry: &str,
    bytes: &[u8],
    out_dir: &Path,
    plant_name: &str,
    loaded: Option<&LoadedDump>,
    tally: &mut PublishTally,
) -> Result<(), String> {
    let stem = entry_stem(zip_entry);
    // One read: the drawing, and the chain streams of it that do not hold.
    // Those are kept whatever becomes of the drawing below — a stream that
    // stops short is often why it has no `DrawingNo` or publishes little.
    let loaded_drawing = match read_publish_from_reader(std::io::Cursor::new(bytes), &stem) {
        Ok(read) => {
            for fault in read.chain_faults {
                tally.chain_fault(PackageChainFault {
                    zip_file: plant.file_name.clone(),
                    zip_entry: zip_entry.to_string(),
                    fault,
                });
            }
            read.drawing
        }
        Err(error) => Err(error),
    };
    // The drawing, where its UID came from, and the row of the dump that
    // holds it when one does.
    let (mut drawing, uid_from, row) = match loaded_drawing {
        Ok(mut drawing) => {
            if let Some(row) = loaded.and_then(|loaded| row_by_id(loaded, &drawing.drawing_uid)) {
                if !row.name.is_empty() {
                    drawing.drawing_name = row.name.clone();
                }
                (drawing, UidFrom::DumpSpId, Some(row))
            } else {
                (drawing, UidFrom::DrawingNo, None)
            }
        }
        Err(PidLoadError::NoDrawing) => {
            let Some(row) = loaded.and_then(|loaded| row_by_path(loaded, zip_entry)) else {
                return Err(
                    "no P&IDAttributes row carries a DrawingNo, and T_DRAWING.PATH does not match"
                        .to_string(),
                );
            };
            (
                shell_drawing(row.id.clone(), row.name.clone()),
                UidFrom::DumpPath,
                Some(row),
            )
        }
        Err(error) => return Err(error.to_string()),
    };

    // Only a sheet the dump lists is this dump's revision: another revision
    // could share model item UIDs with it and pick up ports, compositions or
    // pipelines that are not its own, so it is published from the `.pid` alone.
    let dump_in_run = loaded.is_some() && uid_from == UidFrom::DumpSpId;
    let mut notes = Vec::new();
    if loaded.is_some() && uid_from == UidFrom::DrawingNo {
        let note = format!(
            "DocUID {} has no T_DRAWING row: the dump is another revision of this sheet, \
             so it was published from the .pid alone",
            drawing.drawing_uid
        );
        tally.problem(&plant.file_name, zip_entry, &note);
        notes.push(note);
    }
    if let Some(loaded) = loaded.filter(|_| dump_in_run) {
        let mut done = DumpAttachment::default();
        let attached = attach_dump(
            &mut drawing,
            &loaded.tables,
            &loaded.path,
            plant_name,
            || Ok::<_, std::convert::Infallible>(loaded.datamap.as_ref()),
            &mut done,
        );
        if let Err(error) = attached {
            return Err(error.to_string());
        }
        if let Some(stored) = done.stored_values {
            for reason in stored.unread {
                notes.push(format!("a stored value column was not read: {reason}"));
            }
        }
    }

    if drawing.drawing_name.is_empty() {
        drawing.drawing_name = if drawing.drawing_uid.is_empty() {
            stem
        } else {
            drawing.drawing_uid.clone()
        };
    }
    let file_stem = output_stem(&drawing.drawing_name);
    if !tally.written_names.insert(file_stem.to_lowercase()) {
        return Err(format!("output name {file_stem} is already used"));
    }
    let xml = write_pid_data_xml(&drawing, plant_name);
    let meta = write_pid_meta_xml(&drawing, plant_name).map_err(|error| error.to_string())?;
    let source = PathBuf::from(format!("{}:{zip_entry}", plant.file_name));
    let report = pid_coverage_json(&drawing, &source, &file_stem);
    let census = pid_publish_census(&xml).map_err(|error| error.to_string())?;
    let (object_count, value_count, rel_count) = census.counts();
    write_three(out_dir, &file_stem, &xml, &meta, &report)?;
    if let Some(gate) = tally.gate.as_mut() {
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
        notes.push(duplicates.sentence());
        tally.duplicates(duplicates);
    }
    let oracle = coverage_oracle(&report);
    tally.count_written(
        &plant.file_name,
        zip_entry,
        uid_from,
        (object_count, value_count, rel_count),
        oracle.as_ref(),
    );
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
        "uid_from": uid_from.as_str(),
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
    Ok(())
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
///
/// This is all or none against a write that fails, not against a run that
/// is stopped: stopped between two of the renames, it leaves the files
/// that took their names, each whole, and the others under their
/// temporary names.
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
    let shell = UidFrom::DumpPath.as_str();
    let rows: Vec<Value> = drawings
        .iter()
        .filter(|row| row["uid_from"] == shell)
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
        "rule": format!("uid_from == \"{shell}\""),
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
    write_atomic(&index_path, json_file_text(&index).as_bytes()).map_err(|error| {
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
        "uid_from": UidFrom::DatabaseSpId.as_str(),
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
        let held = names_in(&out);
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

    /// A staged package published into a directory that holds an earlier
    /// run's files writes into none of them: the drawing's three files and
    /// the index each take their name as a new file. Each earlier file has
    /// a second name here, outside the output directory, and reads under it
    /// as it did, so whoever was reading it never sees a part of the new
    /// one.
    #[cfg(feature = "backup-mdf")]
    #[test]
    fn a_staged_package_written_again_takes_each_name_as_a_new_file() {
        const NAMES: [&str; 4] = [
            "SHORT-01_Coverage.json",
            "SHORT-01_Data.xml",
            "SHORT-01_Meta.xml",
            "_Index.json",
        ];
        const EARLIER: &str = "an earlier run's";
        let (root, report) = publish_staged_into("again", SHORTFALLS_SQL, |out| {
            let kept = out.with_file_name("earlier");
            fs::create_dir_all(out).expect("the output directory");
            fs::create_dir_all(&kept).expect("a directory for the second names");
            for name in NAMES {
                fs::write(out.join(name), EARLIER).expect("an earlier file");
                fs::hard_link(out.join(name), kept.join(name)).expect("its second name");
            }
        });
        let read = |dir: &str| -> Vec<String> {
            NAMES
                .iter()
                .map(|name| {
                    fs::read_to_string(root.join(dir).join(name))
                        .unwrap_or_else(|error| panic!("{dir}/{name}: {error}"))
                })
                .collect()
        };
        let (earlier, written) = (read("earlier"), read("out"));
        let held = names_in(&root.join("out"));
        let _ = fs::remove_dir_all(&root);

        assert_eq!((report.written, report.failed), (1, 0));
        assert_eq!(earlier, [EARLIER; 4], "no earlier file is written into");
        for (name, text) in NAMES.iter().zip(&written) {
            assert_ne!(text, EARLIER, "{name} is this run's");
        }
        assert_eq!(held, NAMES, "nothing is left beside them");
    }

    /// The names `dir` holds, sorted.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("a directory that can be listed")
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
            names_in(&self.0)
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

    /// The name goes to a new file: the file that held it is not written
    /// into, so whoever still reads that one, here under a second name it
    /// has, sees it whole.
    #[test]
    fn write_atomic_gives_the_name_to_a_new_file() {
        let scratch = WriteScratch::new("atomic");
        let path = scratch.0.join("_Index.json");
        write_atomic(&path, b"first").expect("nothing is there yet");
        assert_eq!(fs::read(&path).expect("the file"), b"first");
        assert_eq!(
            scratch.names(),
            ["_Index.json"],
            "nothing is left beside it"
        );

        let earlier = scratch.0.join("earlier");
        fs::hard_link(&path, &earlier).expect("a second name for the file");
        write_atomic(&path, b"second, and longer").expect("a file is there");
        assert_eq!(fs::read(&path).expect("the file"), b"second, and longer");
        assert_eq!(
            fs::read(&earlier).expect("the file that held the name"),
            b"first",
            "it is not written into"
        );
        write_atomic(&path, b"3").expect("a file is there");
        assert_eq!(fs::read(&path).expect("the file"), b"3");
        assert_eq!(scratch.names(), ["_Index.json", "earlier"]);
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

    /// A zip of one deflated entry per pair, in the order given.
    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options: zip::write::FileOptions<'_, '_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, bytes) in entries {
                writer.start_file(*name, options).expect("start an entry");
                writer.write_all(bytes).expect("the entry's bytes");
            }
            writer.finish().expect("finish the zip");
        }
        buffer.into_inner()
    }

    /// Make the entry `name` of `zip` state `size` as what it unpacks to,
    /// in its local header and in the central directory alike. What the
    /// entry holds stays as it is.
    fn state_unpacked_size(zip: &mut [u8], name: &str, size: u32) {
        // The signature of a header, and where in it the unpacked size,
        // the length of the name and the name are.
        let headers: [(&[u8], usize, usize, usize); 2] =
            [(b"PK\x03\x04", 22, 26, 30), (b"PK\x01\x02", 24, 28, 46)];
        let name_len = u16::try_from(name.len())
            .expect("a short name")
            .to_le_bytes();
        for (signature, size_at, name_len_at, name_at) in headers {
            let at = (0..zip.len())
                .find(|&at| {
                    zip[at..].starts_with(signature)
                        && zip.get(at + name_len_at..at + name_len_at + 2) == Some(&name_len[..])
                        && zip.get(at + name_at..at + name_at + name.len()) == Some(name.as_bytes())
                })
                .unwrap_or_else(|| panic!("a header of {name}"));
            zip[at + size_at..at + size_at + 4].copy_from_slice(&size.to_le_bytes());
        }
    }

    /// A committed `.pid`, by its path in the repository.
    fn committed_pid(path: &str) -> Vec<u8> {
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    /// A package in `scratch` of the one `PlantData` zip and no dump.
    fn package_of_one_zip(scratch: &WriteScratch, zip: &[u8]) -> PathBuf {
        let package = scratch.0.join("package");
        fs::create_dir_all(&package).expect("the package directory");
        fs::write(package.join("Manifest.txt"), "BackupType<<|>>2\n").expect("the manifest");
        fs::write(package.join("PlantData~2~1.zip"), zip).expect("the zip");
        package
    }

    /// The `_Index.json` in `out`, and the sentences of its `problems`.
    fn index_and_problems(out: &Path) -> (Value, Vec<String>) {
        let index: Value = serde_json::from_str(
            &fs::read_to_string(out.join("_Index.json")).expect("_Index.json"),
        )
        .expect("index json");
        let problems = index["problems"]
            .as_array()
            .expect("problems")
            .iter()
            .map(|problem| problem.as_str().expect("a sentence").to_string())
            .collect();
        (index, problems)
    }

    /// A `.pid` entry that would unpack to more than the limit is a drawing
    /// that fails by name, and the drawings after it are still published.
    /// The limit is set here to the size of the third entry. The first
    /// states more than that and is not unpacked; the second holds as much
    /// as the first and states less, and is not read through; the third,
    /// exactly at the limit, and the fourth are written. Under the limit
    /// the publisher runs with, the first is a drawing like the others.
    #[test]
    fn a_pid_entry_over_the_limit_fails_by_name_and_the_rest_are_published() {
        const ENTRIES: [&str; 4] = [
            "U01/OVER.pid",
            "U01/UNDERSTATED.pid",
            "U01/AT-THE-LIMIT.pid",
            "U01/UNDER.pid",
        ];
        let over = committed_pid("test-file/DWG-0201GP06-01.pid");
        let at_the_limit = committed_pid("test-file/DWG-0202GP06-01.pid");
        let under = committed_pid("test-file/D06.pid");
        assert!(over.len() > at_the_limit.len() && at_the_limit.len() > under.len());
        let limit = u64::try_from(at_the_limit.len()).expect("a size");

        let mut zip = zip_of(&[
            (ENTRIES[0], &over),
            (ENTRIES[1], &over),
            (ENTRIES[2], &at_the_limit),
            (ENTRIES[3], &under),
        ]);
        state_unpacked_size(&mut zip, ENTRIES[1], 1000);
        let scratch = WriteScratch::new("entry-limit");
        let package = package_of_one_zip(&scratch, &zip);

        let out = scratch.0.join("out");
        let report =
            publish_package(&package, &out, "P01", None, limit).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!((report.written, report.failed), (2, 2), "{index}");
        assert_eq!(report.pid_entries, 4);
        let refused = format!(
            "would unpack to {} bytes, over the limit of {limit} for a .pid entry",
            over.len()
        );
        let [stated_over, read_over] = problems.as_slice() else {
            panic!("two problems: {problems:?}");
        };
        assert_eq!(
            *stated_over,
            format!("PlantData~2~1.zip {}: {refused}", ENTRIES[0])
        );
        // Whether the zip reader stops the second or this module does, it
        // is named.
        assert!(
            read_over.starts_with(&format!("PlantData~2~1.zip {}: ", ENTRIES[1])),
            "{read_over}"
        );
        let drawings = index["drawings"].as_array().expect("drawings");
        assert_eq!(drawings.len(), 4, "{index}");
        for (drawing, entry) in drawings.iter().zip(ENTRIES) {
            assert_eq!(drawing["zip_entry"], entry, "{drawing}");
        }
        assert_eq!(
            drawings[0]["problems"],
            json!([format!("read: {refused}")]),
            "{index}"
        );
        let mut held = vec!["_Index.json".to_string()];
        for (drawing, written) in drawings.iter().zip([false, false, true, true]) {
            let files = drawing["files"].as_array().expect("files");
            let own = drawing["problems"].as_array().expect("its own problems");
            if written {
                assert_eq!((files.len(), own.len()), (3, 0), "{drawing}");
                held.extend(
                    files
                        .iter()
                        .map(|file| file.as_str().expect("a file name").to_string()),
                );
            } else {
                assert_eq!((files.len(), own.len()), (0, 1), "{drawing}");
                assert!(
                    own[0]
                        .as_str()
                        .is_some_and(|said| said.starts_with("read: ")),
                    "{drawing}"
                );
            }
        }
        held.sort();
        assert_eq!(names_in(&out), held, "{index}");

        let out = scratch.0.join("out-as-shipped");
        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!((report.written, report.failed), (3, 1), "{index}");
        let [read_over] = problems.as_slice() else {
            panic!("one problem: {problems:?}");
        };
        assert!(
            read_over.starts_with(&format!("PlantData~2~1.zip {}: ", ENTRIES[1])),
            "{read_over}"
        );
    }

    /// The limit a package is published with is 512 MiB. Two entries hold
    /// the same drawing: the one that states a byte more than that is
    /// refused, in a sentence that says both sizes, and the limit is not
    /// what the one that states exactly that is held against.
    #[test]
    fn the_limit_a_package_is_published_with_is_512_mib() {
        const LIMIT: u32 = 512 * 1024 * 1024;
        let pid = committed_pid("test-file/D06.pid");
        let mut zip = zip_of(&[
            ("U01/A-BYTE-OVER.pid", &pid),
            ("U01/AT-THE-LIMIT.pid", &pid),
        ]);
        state_unpacked_size(&mut zip, "U01/A-BYTE-OVER.pid", LIMIT + 1);
        state_unpacked_size(&mut zip, "U01/AT-THE-LIMIT.pid", LIMIT);
        let scratch = WriteScratch::new("entry-limit-shipped");
        let package = package_of_one_zip(&scratch, &zip);
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!(report.pid_entries, 2, "{index}");
        let refused = format!(
            "would unpack to {} bytes, over the limit of {LIMIT} for a .pid entry",
            u64::from(LIMIT) + 1
        );
        assert_eq!(
            problems.first(),
            Some(&format!("PlantData~2~1.zip U01/A-BYTE-OVER.pid: {refused}")),
            "{index}"
        );
        // The second holds less than it states. Whether the zip reader
        // takes that is the zip reader's affair.
        assert!(
            problems.iter().skip(1).all(|said| !said.contains("limit")),
            "{problems:?}"
        );
    }

    /// An entry of a committed `PlantData` zip, by the zip's path in the
    /// repository and the entry's name.
    fn committed_zip_entry(archive_path: &str, name: &str) -> Vec<u8> {
        let file = fs::File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(archive_path))
            .unwrap_or_else(|error| panic!("{archive_path}: {error}"));
        let mut archive =
            zip::ZipArchive::new(file).unwrap_or_else(|error| panic!("{archive_path}: {error}"));
        let mut entry = archive
            .by_name(name)
            .unwrap_or_else(|error| panic!("{archive_path} {name}: {error}"));
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("the entry's bytes");
        bytes
    }

    /// What `_Index.json` lists for the `.pid` entry `entry` of
    /// `PlantData~2~1.zip` when it fails: the entry, the name it has in
    /// the zip, and the reason.
    fn row_of_a_failed_entry(entry: &str, name: &str, reason: &str) -> Value {
        json!({
            "doc_uid": "",
            "name": name,
            "zip_file": "PlantData~2~1.zip",
            "zip_entry": entry,
            "drawing_number": "",
            "path": "",
            "template": "",
            "plant_group": "",
            "uid_from": "",
            "objects": 0,
            "values": 0,
            "relationships": 0,
            "files": [],
            "oracle_exp": null,
            "problems": [reason],
        })
    }

    /// A `.pid` entry that cannot be published is a drawing that fails:
    /// the index counts it, says why under the entry's name, and lists it
    /// with its name and the reason and nothing else. The entries after it
    /// are still published. The package here has no dump, so a drawing is
    /// named after its entry. The second entry is no compound file; the
    /// third has the first's name in another folder, and the first has
    /// taken the output name; the fourth states no `DrawingNo`, and without
    /// a dump there is no `T_DRAWING` row to place it by its path. The
    /// first and the last are written, each under the `DrawingNo` its
    /// `.pid` states, and no row of a dump holds either.
    #[test]
    fn a_pid_entry_that_cannot_be_published_fails_by_name_and_reason() {
        const ZIP: &str = "PlantData~2~1.zip";
        const ENTRIES: [&str; 5] = [
            "U01/FIRST.pid",
            "U01/NO-COMPOUND-FILE.pid",
            "U02/FIRST.pid",
            "U02/NO-DRAWING-NO.pid",
            "U03/LAST.pid",
        ];
        const WRITTEN: [usize; 2] = [0, 4];
        let first = committed_pid("test-file/D06.pid");
        let no_drawing_no = committed_zip_entry(
            "test-file/backup-test/DWG-0202GP06-01_p/PlantData~2~711.zip",
            "zcgc/A3jqz/001.pid",
        );
        let last = committed_pid("test-file/DWG-0202GP06-01.pid");
        let zip = zip_of(&[
            (ENTRIES[0], &first),
            (ENTRIES[1], b"no compound file"),
            (ENTRIES[2], &first),
            (ENTRIES[3], &no_drawing_no),
            (ENTRIES[4], &last),
        ]);
        let scratch = WriteScratch::new("entries-that-fail");
        let package = package_of_one_zip(&scratch, &zip);
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!(
            (
                report.written,
                report.failed,
                report.pid_entries,
                report.shells
            ),
            (2, 3, 5, 0),
            "{index}"
        );
        for (key, count) in [
            ("pid_entries", 5),
            ("failed", 3),
            ("paired", 0),
            ("shells", 0),
        ] {
            assert_eq!(index[key], count, "{key}: {index}");
        }
        let drawings = index["drawings"].as_array().expect("drawings");
        assert_eq!(drawings.len(), 5, "{index}");

        // The two that are written, and what the index adds up of them.
        let mut held = vec!["_Index.json".to_string()];
        for at in WRITTEN {
            let drawing = &drawings[at];
            assert_eq!(drawing["zip_entry"], ENTRIES[at], "{drawing}");
            assert_eq!(drawing["uid_from"], "DrawingNo", "{drawing}");
            assert_eq!(drawing["problems"], json!([]), "{drawing}");
            let files = drawing["files"].as_array().expect("files");
            assert_eq!(files.len(), 3, "{drawing}");
            held.extend(
                files
                    .iter()
                    .map(|file| file.as_str().expect("a file name").to_string()),
            );
        }
        held.sort();
        assert_eq!(names_in(&out), held, "{index}");
        for key in ["objects", "values", "relationships"] {
            let sum: u64 = WRITTEN
                .iter()
                .map(|&at| drawings[at][key].as_u64().expect("a count"))
                .sum();
            assert!(sum > 0, "{key}: {index}");
            assert_eq!(index[key], sum, "{key}: {index}");
        }
        // No row of a dump holds any of the five.
        let mut unmatched: Vec<String> = ENTRIES
            .iter()
            .map(|entry| format!("{ZIP}:{entry}"))
            .collect();
        unmatched.sort();
        assert_eq!(index["unmatched_pid_entries"], json!(unmatched), "{index}");

        // The three that fail, in the order of the entries.
        let [no_compound_file, name_taken, no_drawing_no] = problems.as_slice() else {
            panic!("three problems: {problems:?}");
        };
        // Why the second is no compound file is the reader's to word. The
        // index says it under the entry's name, and the drawing's own row
        // says the same.
        let reason = no_compound_file
            .strip_prefix(&format!("{ZIP} {}: ", ENTRIES[1]))
            .unwrap_or_else(|| panic!("the entry is named: {no_compound_file}"));
        assert!(!reason.is_empty(), "{no_compound_file}");
        assert_eq!(
            drawings[1],
            row_of_a_failed_entry(ENTRIES[1], "NO-COMPOUND-FILE", reason)
        );
        let first_stem = drawings[0]["files"][0]
            .as_str()
            .and_then(|file| file.strip_suffix("_Data.xml"))
            .expect("the first drawing's _Data.xml");
        let reason = format!("output name {first_stem} is already used");
        assert_eq!(*name_taken, format!("{ZIP} {}: {reason}", ENTRIES[2]));
        assert_eq!(
            drawings[2],
            row_of_a_failed_entry(ENTRIES[2], "FIRST", &reason)
        );
        let reason = "no P&IDAttributes row carries a DrawingNo, and T_DRAWING.PATH does not match";
        assert_eq!(*no_drawing_no, format!("{ZIP} {}: {reason}", ENTRIES[3]));
        assert_eq!(
            drawings[3],
            row_of_a_failed_entry(ENTRIES[3], "NO-DRAWING-NO", reason)
        );
    }

    /// Where the local header of the entry `name` starts in `zip`.
    fn local_header_of(zip: &[u8], name: &str) -> usize {
        (0..zip.len())
            .find(|&at| {
                zip[at..].starts_with(b"PK\x03\x04")
                    && zip.get(at + 30..at + 30 + name.len()) == Some(name.as_bytes())
            })
            .unwrap_or_else(|| panic!("a local header of {name}"))
    }

    /// A zip that cannot be opened and an entry whose header cannot be
    /// read are each one failure, said among the index's `problems`, and
    /// no drawing: there is no name to list either under. What can be read
    /// is still published.
    #[test]
    fn a_zip_or_an_entry_that_cannot_be_opened_is_a_failure_and_no_drawing() {
        let pid = committed_pid("test-file/D06.pid");
        // The second entry's header no longer starts as one. The first is
        // left as it is: a file is taken for a zip by its first four bytes.
        let mut zip = zip_of(&[("U01/READ.pid", &pid), ("U01/HEADER-GONE.pid", &pid)]);
        let header = local_header_of(&zip, "U01/HEADER-GONE.pid");
        zip[header..header + 4].copy_from_slice(b"GONE");
        let scratch = WriteScratch::new("unopened");
        let package = package_of_one_zip(&scratch, &zip);
        // A second zip that starts as one and holds nothing of one.
        let unopened = package.join("PlantData~2~2.zip");
        fs::write(&unopened, b"PK\x03\x04 and no zip after that").expect("the second zip");
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!(
            (report.written, report.failed, report.pid_entries),
            (1, 2, 1),
            "{index}"
        );
        assert_eq!(index["failed"], 2, "{index}");
        assert_eq!(index["pid_entries"], 1, "{index}");
        let drawings = index["drawings"].as_array().expect("drawings");
        let [read] = drawings.as_slice() else {
            panic!("one drawing: {index}");
        };
        assert_eq!(read["zip_entry"], "U01/READ.pid", "{read}");
        assert_eq!(read["files"].as_array().map(Vec::len), Some(3), "{read}");

        // The scan of the package says first that it could list neither
        // zip. Then the run says the entry it could not open, by its place
        // in the first zip, and the zip it could not open.
        let [first_unlisted, second_unlisted, entry, zip_unopened] = problems.as_slice() else {
            panic!("four problems: {problems:?}");
        };
        assert!(
            first_unlisted.starts_with(&format!(
                "{}: ",
                package.join("PlantData~2~1.zip").display()
            )),
            "{first_unlisted}"
        );
        assert!(
            second_unlisted.starts_with(&format!("{}: ", unopened.display())),
            "{second_unlisted}"
        );
        assert!(entry.starts_with("PlantData~2~1.zip entry 1: "), "{entry}");
        assert!(
            zip_unopened.starts_with(&format!("open {}: ", unopened.display())),
            "{zip_unopened}"
        );
    }

    /// What the index lists for a drawing a dump holds. The package is the
    /// committed one: seven `.pid` entries, and an Oracle `exp` whose
    /// `T_DRAWING` has a row for each. Six state a `DrawingNo` that is the
    /// `SP_ID` of their row. The first states none and is found by the
    /// `PATH` of its row: a shell. Either way the drawing's row of
    /// `drawings` says which it was, and carries what the row of the dump
    /// says of the drawing: its name, its number, its path, its template,
    /// and the plant group it is filed under. The sixth is numbered
    /// otherwise than it is named, in the dump itself.
    #[test]
    fn a_drawing_a_dump_holds_is_listed_with_what_its_row_says() {
        // The entry of `PlantData~2~711.zip`, the UID, the name, the
        // number, the template, and where the UID came from.
        const ROWS: [[&str; 6]; 7] = [
            [
                "zcgc/A3jqz/001.pid",
                "33A297A41D2D404B9B306BB4750BEBD6",
                "001",
                "001",
                "CPECCHBA2-new.pid",
                "T_DRAWING.PATH",
            ],
            [
                "zcgc/A3jqz/002.pid",
                "A4E1825567814FDF99165927A022A4BE",
                "002",
                "002",
                "qsmcqtaz13A2.pid",
                "T_DRAWING.SP_ID",
            ],
            [
                "zcgc/A3jqz/222.pid",
                "5B0BBC7189D64D039196DDC82A22B8D2",
                "222",
                "222",
                "qsmcqtaz13A2.pid",
                "T_DRAWING.SP_ID",
            ],
            [
                "zcgc/A3jqz/DWG-0201GP01-01.pid",
                "6D7C349C77894315BA5AF07D9428F216",
                "DWG-0201GP01-01",
                "DWG-0201GP01-01",
                "CPECCHBA2-new.pid",
                "T_DRAWING.SP_ID",
            ],
            [
                "zcgc/A3jqz/DWG-0201GP06-01.pid",
                "0F7B8ABD0C4E493FA3C7F06FD03AD6AA",
                "DWG-0201GP06-01",
                "DWG-0201GP06-01",
                "XIONGANA2.pid",
                "T_DRAWING.SP_ID",
            ],
            [
                "zcgc/A3jqz/DWG-0202GP06-01.pid",
                "9D1F3F232C47409D8C54EFB22FC3682A",
                "DWG-0202GP06-01",
                "DWG-0202GP06-02",
                "CPECCHBA2-new.pid",
                "T_DRAWING.SP_ID",
            ],
            [
                "zcgc/A4jqz/DWG-0104GP01-08.pid",
                "3F0E92BA81334AEDBC3B83BA05920C73",
                "DWG-0104GP01-08",
                "DWG-0104GP01-08",
                "CPECCHBA2-new.pid",
                "T_DRAWING.SP_ID",
            ],
        ];
        let package =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/backup-test/DWG-0202GP06-01_p");
        let scratch = WriteScratch::new("dump-rows");
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!(problems, Vec::<String>::new(), "{index}");
        assert_eq!(
            (
                report.written,
                report.failed,
                report.pid_entries,
                report.shells
            ),
            (7, 0, 7, 1),
            "{index}"
        );
        for (key, count) in [
            ("pid_entries", 7),
            ("drawing_rows", 7),
            ("paired", 7),
            ("failed", 0),
            ("shells", 1),
        ] {
            assert_eq!(index[key], count, "{key}: {index}");
        }
        assert_eq!(index["unmatched_pid_entries"], json!([]), "{index}");
        let drawings = index["drawings"].as_array().expect("drawings");
        assert_eq!(drawings.len(), ROWS.len(), "{index}");
        for (drawing, [entry, uid, name, number, template, uid_from]) in drawings.iter().zip(ROWS) {
            // The dump files `zcgc/<unit>/<name>.pid` of the zip as
            // `\zcgc\<unit>\<name>.pid`, under the unit of that name.
            let unit = entry.split('/').nth(1).expect("the folder of a unit");
            let listed = json!({
                "doc_uid": uid,
                "name": name,
                "zip_file": "PlantData~2~711.zip",
                "zip_entry": entry,
                "drawing_number": number,
                "path": format!("\\{}", entry.replace('/', "\\")),
                "template": template,
                "plant_group": format!("QSMCQTAZ13_PLANT \\ zcgc \\ {unit}"),
                "uid_from": uid_from,
            });
            for (key, value) in listed.as_object().expect("an object") {
                assert_eq!(&drawing[key], value, "{key}: {drawing}");
            }
        }
    }

    /// A drawing the dump lists and cannot be attached to fails by name
    /// and reason, and the rest of the package is still published. The
    /// package is the manifest, the `PlantData` zip and the dump of the
    /// committed one, with one byte of the dump changed: the `INSERT` of
    /// `T_PIPINGPOINT` no longer names the column the port projection
    /// reads a point's number from. The dump is still read, so the
    /// package publishes. The port tables are refused, so the attach
    /// fails, in the words of that refusal, for each of the six drawings
    /// found by the `SP_ID` of their row. The first entry is the shell: it
    /// is found by `PATH`, attaches nothing, and is written.
    #[test]
    fn a_drawing_the_dump_cannot_be_attached_to_fails_by_name_and_reason() {
        const SHELL: &str = "zcgc/A3jqz/001.pid";
        const ATTACHED: [&str; 6] = [
            "zcgc/A3jqz/002.pid",
            "zcgc/A3jqz/222.pid",
            "zcgc/A3jqz/DWG-0201GP01-01.pid",
            "zcgc/A3jqz/DWG-0201GP06-01.pid",
            "zcgc/A3jqz/DWG-0202GP06-01.pid",
            "zcgc/A4jqz/DWG-0104GP01-08.pid",
        ];
        let committed =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/backup-test/DWG-0202GP06-01_p");
        let mut dump = fs::read(committed.join("Export.dmp")).expect("the committed dump");
        // One letter of the column's name, in the column list of the
        // table's one `INSERT`: no byte moves, and the table still frames.
        let find = |bytes: &[u8], wanted: &[u8]| {
            bytes
                .windows(wanted.len())
                .position(|window| window == wanted)
        };
        let insert = find(&dump, br#"INSERT INTO "T_PIPINGPOINT" ("#).expect("the INSERT");
        let list = find(&dump[insert..], b")").expect("the end of the column list");
        let column = find(&dump[insert..insert + list], br#""PIPINGPOINTNUMBER""#)
            .expect("the column, in the list");
        dump[insert + column + 1] = b'X';
        let tables = ExpTables::read(&dump).expect("the dump is still read");
        let Err(refusal) = tables.port_tables() else {
            panic!("the port tables were read");
        };
        let refusal = refusal.to_string();
        assert!(refusal.starts_with("T_PIPINGPOINT: "), "{refusal}");

        let scratch = WriteScratch::new("attach-fails");
        let package = scratch.0.join("package");
        fs::create_dir_all(&package).expect("the package directory");
        for name in ["Manifest.txt", "PlantData~2~711.zip"] {
            fs::copy(committed.join(name), package.join(name)).expect("a file of the package");
        }
        fs::write(package.join("Export.dmp"), &dump).expect("the dump");
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, problems) = index_and_problems(&out);
        assert_eq!(
            (
                report.written,
                report.failed,
                report.pid_entries,
                report.shells
            ),
            (1, 6, 7, 1),
            "{index}"
        );
        for (key, count) in [
            ("pid_entries", 7),
            ("drawing_rows", 7),
            ("paired", 1),
            ("failed", 6),
            ("shells", 1),
        ] {
            assert_eq!(index[key], count, "{key}: {index}");
        }
        // One sentence for each drawing that fails, by its zip and entry.
        let sentences: Vec<String> = ATTACHED
            .iter()
            .map(|entry| format!("PlantData~2~711.zip {entry}: {refusal}"))
            .collect();
        assert_eq!(problems, sentences, "{index}");

        let drawings = index["drawings"].as_array().expect("drawings");
        let [shell, failed @ ..] = drawings.as_slice() else {
            panic!("the shell and the six: {index}");
        };
        assert_eq!(shell["zip_entry"], SHELL, "{shell}");
        assert_eq!(shell["uid_from"], "T_DRAWING.PATH", "{shell}");
        assert_eq!(shell["files"].as_array().map(Vec::len), Some(3), "{shell}");
        assert_eq!(failed.len(), ATTACHED.len(), "{index}");
        for (drawing, entry) in failed.iter().zip(ATTACHED) {
            assert_eq!(drawing["zip_entry"], entry, "{drawing}");
            // The row says the reason alone.
            assert_eq!(drawing["problems"], json!([refusal]), "{drawing}");
            assert_eq!(drawing["files"], json!([]), "{drawing}");
        }
        // Nothing of a drawing that failed is in the output directory.
        assert_eq!(
            names_in(&out),
            [
                "001_Coverage.json",
                "001_Data.xml",
                "001_Meta.xml",
                "_Index.json"
            ]
        );
    }

    /// A column the stored values stage is sent to and the dump does not
    /// hold is noted on the row of each drawing the stage looked in it
    /// for, and every drawing is written. The package is the manifest, the
    /// `PlantData` zip, the data map's zip and the dump of the committed
    /// one, with two bytes of the dump changed: the `INSERT`s of
    /// `T_PIPERUN` and `T_INLINECOMP` no longer name the columns the dump's
    /// own dictionary places a pipe run's `SP_ConnectorsZeroLength` and an
    /// inline component's `FlowDirection` in. No stage refuses such a
    /// dump. The shell attaches nothing; two drawings hold neither a pipe
    /// run nor an inline component; three hold both, and the last a pipe
    /// run alone.
    #[test]
    fn a_stored_column_the_dump_does_not_hold_is_noted_on_the_drawings_it_was_read_for() {
        const PIPE_RUN: &str = "a stored value column was not read: \
                                T_PipeRun.SP_ConnectorsZeroLength: the dump holds no such column";
        const INLINE: &str = "a stored value column was not read: \
                              T_InlineComp.FlowDirection: the dump holds no such column";
        let noted: [(&str, &[&str]); 7] = [
            ("zcgc/A3jqz/001.pid", &[]),
            ("zcgc/A3jqz/002.pid", &[]),
            ("zcgc/A3jqz/222.pid", &[]),
            ("zcgc/A3jqz/DWG-0201GP01-01.pid", &[INLINE, PIPE_RUN]),
            ("zcgc/A3jqz/DWG-0201GP06-01.pid", &[INLINE, PIPE_RUN]),
            ("zcgc/A3jqz/DWG-0202GP06-01.pid", &[INLINE, PIPE_RUN]),
            ("zcgc/A4jqz/DWG-0104GP01-08.pid", &[PIPE_RUN]),
        ];
        let committed =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/backup-test/DWG-0202GP06-01_p");
        let mut dump = fs::read(committed.join("Export.dmp")).expect("the committed dump");
        // One letter of each column's name, in the column list of its
        // table's one `INSERT`: no byte moves, and the tables still frame.
        let find = |bytes: &[u8], wanted: &[u8]| {
            bytes
                .windows(wanted.len())
                .position(|window| window == wanted)
        };
        for (table, column) in [
            ("T_PIPERUN", "SP_CONNECTORSZEROLENGTH"),
            ("T_INLINECOMP", "FLOWDIRECTION"),
        ] {
            let insert =
                find(&dump, format!("INSERT INTO \"{table}\" (").as_bytes()).expect("the INSERT");
            let list = find(&dump[insert..], b")").expect("the end of the column list");
            let name = find(
                &dump[insert..insert + list],
                format!("\"{column}\"").as_bytes(),
            )
            .expect("the column, in the list");
            dump[insert + name + 2] = b'X';
        }

        let scratch = WriteScratch::new("stored-column-not-read");
        let package = scratch.0.join("package");
        fs::create_dir_all(&package).expect("the package directory");
        for name in ["Manifest.txt", "PlantData~2~711.zip", "RefData~4~804.zip"] {
            fs::copy(committed.join(name), package.join(name)).expect("a file of the package");
        }
        fs::write(package.join("Export.dmp"), &dump).expect("the dump");
        let out = scratch.0.join("out");

        let report =
            publish_backup_package(&package, &out, "P01", None).expect("the package publishes");
        let (index, _) = index_and_problems(&out);
        assert_eq!((report.written, report.failed), (7, 0), "{index}");
        let drawings = index["drawings"].as_array().expect("drawings");
        assert_eq!(drawings.len(), noted.len(), "{index}");
        for (drawing, (entry, notes)) in drawings.iter().zip(noted) {
            assert_eq!(drawing["zip_entry"], entry, "{drawing}");
            assert_eq!(drawing["problems"], json!(notes), "{entry}");
            assert_eq!(
                drawing["files"].as_array().map(Vec::len),
                Some(3),
                "{entry}"
            );
        }
    }

    /// The UID and the name of every `<PIDPipeline>` a `_Data.xml` states,
    /// in document order: the two attributes of the `<IObject>` the element
    /// opens with, as they are written. A name with a character XML
    /// escapes would be read here in its escaped spelling.
    fn pipelines_stated(data_xml: &str) -> Vec<(String, String)> {
        let attribute = |object: &str, name: &str| {
            let (_, rest) = object.split_once(&format!(" {name}=\""))?;
            let (value, _) = rest.split_once('"')?;
            Some(value.to_string())
        };
        let mut lines = data_xml.lines();
        let mut stated = Vec::new();
        while let Some(line) = lines.next() {
            if line.trim() != "<PIDPipeline>" {
                continue;
            }
            let object = lines.next().expect("the <IObject> of a pipeline");
            stated.push((
                attribute(object, "UID").expect("the UID of a pipeline"),
                attribute(object, "Name").expect("the name of a pipeline"),
            ));
        }
        stated
    }

    /// What a package run hands the attach of its dump, seen in the files
    /// it writes. The package is the committed one, published under two
    /// plants. A drawing found by the `SP_ID` of its row has the dump on
    /// it: its coverage report names the package's `Export.dmp` as the
    /// dump it was read from, and each pipeline its `_Data.xml` states has
    /// the UID `pipeline_uid` makes of the run's plant and the pipeline's
    /// name. Three of the six publish pipelines, twelve between them. The
    /// shell attaches nothing: its report has no dump, and it states no
    /// pipeline.
    #[test]
    fn a_package_drawing_names_the_packages_dump_and_its_pipelines_take_the_runs_plant() {
        // The drawing's name, whether the dump is on it, and how many
        // pipelines it publishes.
        const DRAWINGS: [(&str, bool, usize); 7] = [
            ("001", false, 0),
            ("002", true, 0),
            ("222", true, 0),
            ("DWG-0201GP01-01", true, 0),
            ("DWG-0201GP06-01", true, 5),
            ("DWG-0202GP06-01", true, 6),
            ("DWG-0104GP01-08", true, 1),
        ];
        let package =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/backup-test/DWG-0202GP06-01_p");
        let dump = package.join("Export.dmp").display().to_string();
        let scratch = WriteScratch::new("plant-and-dump");

        for plant in ["P01", "Unit 7"] {
            let out = scratch.0.join(plant);
            let report =
                publish_backup_package(&package, &out, plant, None).expect("the package publishes");
            assert_eq!((report.written, report.failed), (7, 0), "{plant}");
            for (name, attached, pipelines) in DRAWINGS {
                let read = |suffix: &str| {
                    let path = out.join(format!("{name}{suffix}"));
                    fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
                };
                let coverage: Value =
                    serde_json::from_str(&read("_Coverage.json")).expect("a coverage report");
                let named = coverage
                    .get("oracle_exp")
                    .map(|section| section["dump"].clone());
                assert_eq!(named, attached.then(|| json!(dump)), "{plant}: {name}");

                let stated = pipelines_stated(&read("_Data.xml"));
                assert_eq!(stated.len(), pipelines, "{plant}: {name}");
                for (uid, pipeline) in stated {
                    assert_eq!(
                        uid,
                        super::super::pipeline::pipeline_uid(plant, &pipeline),
                        "{plant}: {name}: {pipeline}"
                    );
                }
            }
        }
    }

    /// A drawing written with a UID on more than one block is one of the
    /// index's `problems`, as `describe` words it, and one for the report.
    /// Nothing else moves: the drawing is counted where it is written. No
    /// committed `.pid` publishes such a document, so this is seen at the
    /// tally and not through a package; that `publish_one` hands the
    /// finding to the tally is seen by no test.
    #[test]
    fn the_tally_says_a_drawing_that_states_a_uid_twice() {
        let duplicates = PackageDuplicateUids {
            drawing_uid: "DWG-1".to_string(),
            drawing_name: "D1".to_string(),
            uids: vec![("V-1".to_string(), 2)],
        };
        let mut tally = PublishTally::default();
        tally.duplicates(duplicates.clone());
        assert_eq!(tally.problems, ["D1: 1 UID written more than once: V-1 ×2"]);
        assert_eq!(tally.duplicate_uids, [duplicates]);
        assert_eq!((tally.failed, tally.written), (0, 0));
        assert!(tally.drawings.is_empty());
    }

    /// A reader that counts the bytes it has handed out.
    struct Counted<R> {
        inner: R,
        handed_out: usize,
    }

    impl<R: Read> Read for Counted<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let count = self.inner.read(buf)?;
            self.handed_out += count;
            Ok(count)
        }
    }

    /// A reader that fails.
    struct Broken;

    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("the disk is gone"))
        }
    }

    /// What an entry states is looked at before a byte of it is read, and
    /// what it holds is read to one byte past the limit and no further.
    #[test]
    fn an_entry_is_read_up_to_the_limit_and_not_past_it() {
        // An entry that holds `held` sevens.
        let entry = |held: u64| Counted {
            inner: io::repeat(7).take(held),
            handed_out: 0,
        };

        // At the limit and under it, the entry is read whole.
        assert_eq!(
            read_entry_within(&mut entry(100), 100, 100),
            Ok(vec![7; 100])
        );
        assert_eq!(read_entry_within(&mut entry(99), 99, 100), Ok(vec![7; 99]));
        assert_eq!(read_entry_within(&mut entry(0), 0, 100), Ok(Vec::new()));
        assert_eq!(read_entry_within(&mut entry(0), 0, 0), Ok(Vec::new()));

        // It states more than the limit: refused, and not a byte is read.
        let mut over = entry(101);
        assert_eq!(
            read_entry_within(&mut over, 101, 100),
            Err("would unpack to 101 bytes, over the limit of 100 for a .pid entry".to_string())
        );
        assert_eq!(over.handed_out, 0);

        // It holds more than the limit and states less: the read stops a
        // byte past the limit, and the entry is refused.
        let mut understated = entry(1 << 20);
        assert_eq!(
            read_entry_within(&mut understated, 10, 100),
            Err(
                "states 10 bytes and unpacks to more than the limit of 100 for a .pid entry"
                    .to_string()
            )
        );
        assert_eq!(understated.handed_out, 101);
        let mut a_byte_over = entry(1);
        assert!(read_entry_within(&mut a_byte_over, 0, 0).is_err());
        assert_eq!(a_byte_over.handed_out, 1);

        // What it states and what it holds differ, within the limit: that
        // is for the zip reader to say, and here the bytes are the answer.
        assert_eq!(read_entry_within(&mut entry(60), 10, 100), Ok(vec![7; 60]));
        assert_eq!(read_entry_within(&mut entry(10), 60, 100), Ok(vec![7; 10]));

        // A limit of all there is does not wrap round to none.
        assert_eq!(
            read_entry_within(&mut entry(5), u64::MAX, u64::MAX),
            Ok(vec![7; 5])
        );

        // A read that fails says so in its own words.
        assert_eq!(
            read_entry_within(&mut Broken, 1, 100),
            Err("the disk is gone".to_string())
        );
    }
}
