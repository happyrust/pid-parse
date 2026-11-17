//! Shared helpers for the publish-related integration tests
//! (`tests/publish_*.rs`).
//!
//! Cargo treats every `tests/*.rs` file as an independent
//! integration-test binary and does NOT auto-link sibling
//! modules across binaries. We work around that with the
//! `tests/common/mod.rs` pattern — each test file that wants
//! the helpers writes `mod common;` and Cargo resolves the
//! reference to this directory's `mod.rs`.
//!
//! The `#[allow(dead_code)]` on the module body is required:
//! Cargo independently compiles `common` into every binary
//! that imports it, so any helper used by only a subset of
//! the tests would otherwise trip a dead-code warning in the
//! binaries that do not exercise it. The contract is that
//! every public item in this file is exercised by AT LEAST
//! ONE integration test in the workspace.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use pid_parse::publish::{
    load_drawing_graph_from_mdf, write_data_xml, write_meta_xml, PublishError,
};

// -----------------------------------------------------------------
// Scratch directories
// -----------------------------------------------------------------

/// A path of this process's own under the system temp dir. Whatever is at
/// it when the value drops is removed, so what a test writes there is gone
/// when the test ends — on a normal return and while a failed assertion
/// unwinds alike. A removal at the end of the test body would only run on
/// the first of the two.
///
/// It reads as a `&Path` (`scratch.join(..)`, `scratch.exists()`), and
/// [`Scratch::path`] hands the path to `Command::arg` and `std::fs`. It is
/// deliberately neither `AsRef<OsStr>` nor `AsRef<Path>`: taken by value
/// by `Command::arg` it would drop, removing nothing, before the command
/// ran and created the directory.
pub struct Scratch(PathBuf);

impl Scratch {
    /// Names a directory and creates it.
    pub fn new(tag: &str) -> Self {
        let scratch = Self::unmade(tag);
        std::fs::create_dir_all(&scratch.0)
            .unwrap_or_else(|error| panic!("create {}: {error}", scratch.0.display()));
        scratch
    }

    /// Names a directory without creating it: the output directory of a run
    /// that creates its own, or of one that must be shown to write nothing.
    pub fn unmade(tag: &str) -> Self {
        // Two guards of one tag made in the same clock tick still differ.
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "pid-parse-{tag}-{}-{nanos}-{sequence}",
            std::process::id()
        )))
    }

    /// The path, for an argument that takes one by reference.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// -----------------------------------------------------------------
// 8.3 aliases
// -----------------------------------------------------------------

/// The 8.3 alias the file system keeps for the file at `long`, as a path,
/// or `None` when there is none to reach the file by: off Windows, on a
/// volume that keeps no short names, and for a name that is its own short
/// name. A `skipping:` line then says so.
///
/// The alias is what `cmd` expands `%~sI` to; the standard library has no
/// call that gives it.
#[cfg(windows)]
pub fn short_path_of(long: &Path) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::os::windows::process::CommandExt;

    // The line goes to `cmd` as it stands: `cmd` does not undo the quoting
    // `Command::arg` would add. `/u` has `echo` answer in UTF-16, so the
    // path comes back whole whatever the console code page is.
    let line = format!("/d /u /c for %I in (\"{}\") do @echo %~sI", long.display());
    let output = match std::process::Command::new("cmd").raw_arg(line).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("skipping: cmd did not start, so no 8.3 alias can be asked for: {error}");
            return None;
        }
    };
    let units: Vec<u16> = output
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let short = PathBuf::from(String::from_utf16_lossy(&units).trim());
    let same_name = short.file_name().map(OsStr::to_ascii_lowercase)
        == long.file_name().map(OsStr::to_ascii_lowercase);
    if same_name || !short.is_file() {
        eprintln!(
            "skipping: {} has no 8.3 alias to be reached by (cmd answered {short:?})",
            long.display()
        );
        return None;
    }
    Some(short)
}

/// See the Windows definition: only a Windows volume keeps 8.3 aliases.
#[cfg(not(windows))]
pub fn short_path_of(long: &Path) -> Option<PathBuf> {
    eprintln!(
        "skipping: only a Windows volume keeps an 8.3 alias, so {} has none",
        long.display()
    );
    None
}

// -----------------------------------------------------------------
// Fixture paths and identifiers — A01 plant (TEST02)
// -----------------------------------------------------------------

/// MDF extracted from the TEST02 plant backup. This is the
/// single source-of-truth path for publish fidelity tests.
pub const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";

/// Legacy `SQLite` mirror kept only for `sqlite_load` compatibility
/// tests. New publish fidelity gates must use [`A01_MDF_PATH`].
pub const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";

/// `T_Drawing.SP_ID` of the A01 drawing inside the TEST02
/// MDF fixture.
pub const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

/// Plant name passed to `write_data_xml(_)` when generating
/// XML in tests. Matches the TEST02 plant in the MDF fixture
/// so any plant-level cross-checks line up.
pub const PLANT_NAME: &str = "TEST02";

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the A01 drawing. Bundled in `test-file/export-test/`.
pub const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";

/// Path to the SmartPlant-produced reference `_Meta.xml` for
/// the A01 drawing. Sibling of [`A01_REFERENCE_DATA_XML`];
/// drives the `_Meta.xml` parity gates in
/// `tests/publish_meta_parity.rs`.
pub const A01_REFERENCE_META_XML: &str = "test-file/export-test/publish-data/A01/A01_Meta.xml";

// -----------------------------------------------------------------
// Fixture paths and identifiers — DWG plant (P01)
// -----------------------------------------------------------------

/// Path to the SmartPlant-produced reference `_Data.xml` for
/// the DWG-0202GP06-01 drawing. Used by every cross-fixture
/// universality test (A24 / A27b) and by the A28 backlog
/// inventory.
pub const DWG_REFERENCE_DATA_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml";

/// Path to the SmartPlant-produced reference `_Meta.xml` for
/// the DWG-0202GP06-01 drawing.
pub const DWG_REFERENCE_META_XML: &str =
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Meta.xml";

/// The `.pid` of the DWG-0202GP06-01 drawing, committed twice
/// (byte-identical apart from the container's own store) under
/// `test-file/` and beside its reference export.
pub const DWG_PID_PATH: &str = "test-file/DWG-0202GP06-01.pid";

/// The package's Oracle `exp` dump — the DWG plant's only database
/// export in the repository.
pub const DWG_ORACLE_EXP_PATH: &str = "test-file/backup-test/DWG-0202GP06-01_p/Export.dmp";

/// The package's `SPPIDDataMap.xml` archive (the data map entry the
/// `.pid` line reads to spell codelist values and pipeline fluids).
pub const DWG_DATAMAP_ARCHIVE: &str = "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~804.zip";

/// `T_Drawing.SP_ID` of the DWG-0202GP06-01 drawing, sourced
/// from the reference `_Meta.xml`'s `DocUID` attribute so the
/// constant stays aligned with the bundled reference fixture.
pub const DWG_DRAWING_UID: &str = "9D1F3F232C47409D8C54EFB22FC3682A";

/// Plant name the DWG reference fixture publishes under.
/// Sourced from the reference `_Meta.xml`'s `Plant`
/// attribute; passed as `--plant` to the run that publishes the
/// package's `.pid`.
pub const DWG_PLANT_NAME: &str = "P01";

/// Soft-skip message the DWG gates print when a piece of the
/// committed package is missing. Every piece is committed, so this
/// only fires on a checkout that dropped one.
pub const DWG_PACKAGE_MISSING_HINT: &str = "skipping: the DWG package's `.pid`, `Export.dmp` or \
     `RefData~4~804.zip` is not in the repository — the DWG publish gates read \
     `test-file/DWG-0202GP06-01.pid` against \
     `test-file/backup-test/DWG-0202GP06-01_p/{Export.dmp,RefData~4~804.zip}`.";

// -----------------------------------------------------------------
// Parity scope
// -----------------------------------------------------------------

/// PID tag varieties under fidelity parity (A23 / A24 / A27 /
/// A27b). This is the full writer-supported tag surface as of
/// Stage-4, including the two formerly-backlogged branch-point
/// varieties. A23 scores a tag the A01 reference does not carry
/// against the DWG reference instead; keeping the union here means the
/// shared whitelist / drift guards stay aligned with
/// `publish::supported_pid_tags()`.
///
/// Sorted alphabetically for stability across whitelists and
/// diff readers.
pub const TAGS_UNDER_PARITY: &[&str] = &[
    "PIDBranchPoint",
    "PIDControlSystemFunction",
    "PIDDrawing",
    "PIDNote",
    "PIDNozzle",
    "PIDPipeline",
    "PIDPipingBranchPoint",
    "PIDPipingComponent",
    "PIDPipingConnector",
    "PIDPipingPort",
    "PIDProcessPoint",
    "PIDProcessVessel",
    "PIDRepresentation",
    "PIDSignalConnector",
    "PIDSignalPort",
];

// -----------------------------------------------------------------
// Fixture loaders
// -----------------------------------------------------------------

/// Generate `A01_Data.xml` through the real publish pipeline:
/// open the TEST02 MDF with the Rust reader, load the drawing
/// graph for the A01 drawing UID, and emit the XML as a String.
/// Returns `None` when the MDF fixture is missing.
pub fn generate_a01_xml() -> Option<Result<String, PublishError>> {
    let mdf_path = std::path::Path::new(A01_MDF_PATH);
    if !mdf_path.exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return None;
    }
    let drawing = match load_drawing_graph_from_mdf(mdf_path, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_data_xml(&drawing, PLANT_NAME))
}

/// Generate `A01_Meta.xml` through the real publish pipeline.
/// Sibling of [`generate_a01_xml`] but targets the
/// document-versioning `_Meta.xml` surface. Soft-skips when the
/// A01 MDF fixture is missing.
pub fn generate_a01_meta_xml() -> Option<Result<String, PublishError>> {
    let mdf_path = std::path::Path::new(A01_MDF_PATH);
    if !mdf_path.exists() {
        eprintln!("skipping: MDF fixture {A01_MDF_PATH} not found");
        return None;
    }
    let drawing = match load_drawing_graph_from_mdf(mdf_path, A01_DRAWING_UID) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    Some(write_meta_xml(&drawing, PLANT_NAME))
}

/// Generate `DWG-0202GP06-01_Data.xml` through the Oracle line: the
/// committed `.pid` published against its package's own `Export.dmp`
/// and data map. This is the line the CLI's `--oracle-exp` runs, so
/// what the test reads is what a delivery writes.
///
/// Returns `None` (with a `skipping: ` line) when a piece is absent.
pub fn generate_dwg_data_xml() -> Option<Result<String, PublishError>> {
    generate_dwg_documents().map(|documents| Ok(documents.data))
}

/// Generate `DWG-0202GP06-01_Meta.xml` through the same Oracle line.
/// The meta document is the CLI's `--out-dir` companion; it carries
/// no dump content of its own, but taking it from the same run keeps
/// the pair one document set.
pub fn generate_dwg_meta_xml() -> Option<Result<String, PublishError>> {
    generate_dwg_documents().map(|documents| Ok(documents.meta))
}

/// The two documents one `--oracle-exp` run of the committed package
/// writes.
struct DwgDocuments {
    data: String,
    meta: String,
}

/// Run the CLI over the committed package and read back the two
/// documents it writes. One run produces the pair, so a test that
/// needs both documents takes them from one call to this.
fn generate_dwg_documents() -> Option<DwgDocuments> {
    let pid = Path::new(DWG_PID_PATH);
    let dump = Path::new(DWG_ORACLE_EXP_PATH);
    let datamap = Path::new(DWG_DATAMAP_ARCHIVE);
    for piece in [pid, dump, datamap] {
        if !piece.is_file() {
            eprintln!("{DWG_PACKAGE_MISSING_HINT} missing: {}", piece.display());
            return None;
        }
    }
    let out_dir = Scratch::unmade("dwg-oracle-exp");
    let output = Command::new(env!("CARGO_BIN_EXE_pid_publish_xml"))
        .arg(pid)
        .arg("--oracle-exp")
        .arg(dump)
        .arg("--datamap")
        .arg(datamap)
        .arg("--out-dir")
        .arg(out_dir.path())
        .args(["--plant", DWG_PLANT_NAME])
        .output()
        .expect("spawn pid_publish_xml --oracle-exp on the DWG package");
    assert!(
        output.status.success(),
        "the DWG package's own dump must publish: exit {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let read = |suffix: &str| {
        let path = out_dir.join(format!("DWG-0202GP06-01{suffix}"));
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    };
    Some(DwgDocuments {
        data: read("_Data.xml"),
        meta: read("_Meta.xml"),
    })
}

/// Load the SmartPlant-produced reference A01 `_Data.xml`.
/// Returns `None` when the fixture is missing.
pub fn load_reference_a01_xml() -> Option<String> {
    load_reference_xml(A01_REFERENCE_DATA_XML)
}

/// Load the SmartPlant-produced reference DWG `_Data.xml`.
/// Returns `None` when the fixture is missing.
pub fn load_reference_dwg_xml() -> Option<String> {
    load_reference_xml(DWG_REFERENCE_DATA_XML)
}

/// Load the SmartPlant-produced reference A01 `_Meta.xml`.
pub fn load_reference_a01_meta_xml() -> Option<String> {
    load_reference_xml(A01_REFERENCE_META_XML)
}

/// Load the SmartPlant-produced reference DWG `_Meta.xml`.
pub fn load_reference_dwg_meta_xml() -> Option<String> {
    load_reference_xml(DWG_REFERENCE_META_XML)
}

/// Generic UTF-8 reader that prints a soft-skip notice when
/// the file is missing. Centralized so adding new reference
/// fixtures only requires a one-liner wrapper.
fn load_reference_xml(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        eprintln!("skipping: reference fixture {path} not found");
        return None;
    }
    Some(std::fs::read_to_string(p).expect("reference should be utf-8"))
}

/// Narrow A01 delivery-contract normalization: only the three
/// A39 evidence-guarded publish-time synthetic residual classes are
/// rewritten.
///
/// Stable object identities — drawing, vessel, nozzle, pipeline,
/// publishable representation UIDs, and every concrete `IRel`
/// endpoint — are left raw. This keeps the contract aligned with
/// the real burn-down target. The ignored A39 probe scans the full
/// Rust MDF table inventory exposed by `oxidized-mdf` and confirms
/// connector-family UID, `<Rel><IObject UID="..."/>`, and
/// `GraphicOID` are not present in the complete TEST02 MDF table
/// scan or MDF raw text / UUID byte forms.
#[derive(Clone, Copy)]
pub struct A01SyntheticMaskOptions {
    pub connector_family: bool,
    pub rel_iobject_uid: bool,
    pub graphic_oid: bool,
}

impl A01SyntheticMaskOptions {
    pub const ALL: Self = Self {
        connector_family: true,
        rel_iobject_uid: true,
        graphic_oid: true,
    };
}

pub fn normalize_a01_synthetic_slots(xml: &str, opts: A01SyntheticMaskOptions) -> String {
    let mut current_block: Option<&'static str> = None;
    let mut connector_uid: Option<String> = None;
    let mut graphic_count = 0usize;
    let mut rel_count = 0usize;
    let mut out = Vec::new();

    for raw in xml.replace("\r\n", "\n").lines() {
        let trimmed = raw.trim_start();
        match trimmed {
            "<PIDPipingConnector>" => current_block = Some("PIDPipingConnector"),
            "<Rel>" => current_block = Some("Rel"),
            _ => {}
        }

        let mut line = raw.to_string();

        if opts.connector_family
            && current_block == Some("PIDPipingConnector")
            && trimmed.starts_with("<IObject ")
        {
            if let Some(uid) = extract_attr(&line, "UID") {
                connector_uid = Some(uid.clone());
                line = replace_attr_value(&line, "UID", "@CONNECTOR@");
            }
        }

        if opts.connector_family {
            if let Some(base) = connector_uid.as_deref() {
                for (from, to) in [
                    (base.to_string(), "@CONNECTOR@"),
                    (format!("{base}.1"), "@PORT1@"),
                    (format!("{base}.2"), "@PORT2@"),
                    (format!("{base}.PPT"), "@PROCESS_POINT@"),
                ] {
                    line = line.replace(&from, to);
                }
            }
        }

        if opts.graphic_oid && trimmed.starts_with("<IDrawingRepresentation ") {
            graphic_count += 1;
            line = replace_attr_value(&line, "GraphicOID", &format!("@GRAPHIC{graphic_count}@"));
        }

        if opts.rel_iobject_uid && current_block == Some("Rel") && trimmed.starts_with("<IObject ")
        {
            rel_count += 1;
            line = replace_attr_value(&line, "UID", &format!("@REL{rel_count}@"));
        }

        out.push(line);

        match trimmed {
            "</PIDPipingConnector>" | "</Rel>" => current_block = None,
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
