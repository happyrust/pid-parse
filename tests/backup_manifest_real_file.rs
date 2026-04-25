//! Integration gate: parse the real `TEST02_p/Manifest.txt` fixture
//! and assert the line-level model lines up with the on-disk shape.
//!
//! `crate::backup::manifest` ships a tolerant line-based parser
//! plus inline unit tests on small synthetic fixtures. This file
//! complements those with a *production-shape* check against the
//! 30 KB / 319-line manifest that ships with the bundled
//! `TEST02_p` plant backup. When the fixture is absent (the
//! `test-file/` tree is git-ignored — see `AGENTS.md`), the test
//! soft-skips with an explicit hint so the suite stays green for
//! contributors who haven't downloaded the backup samples.
//!
//! The assertions are tight on purpose: the manifest is a
//! checked-in artefact of one specific SmartPlant export run, so
//! every count below is exact rather than a `>=` lower bound. Any
//! drift in `parse_manifest` that drops or doubles lines will
//! show up immediately as a failed equality, not a fuzzy "got
//! more / fewer than expected" hint.

use std::collections::BTreeSet;
use std::path::Path;

use pid_parse::backup::parse_manifest_bytes;

/// Path to the bundled real-shape fixture. Lives under
/// `test-file/backup-test/TEST02_p/` along with the `Export.dmp`
/// MTF backup the rest of the offline pipeline consumes.
const MANIFEST_PATH: &str = "test-file/backup-test/TEST02_p/Manifest.txt";

/// Soft-skip hint emitted when the fixture is missing. Mirrors the
/// pattern of [`tests/common::DWG_MDF_MISSING_HINT`] so the suite
/// stays green for a fresh checkout but the blockage is visible
/// in the test log.
const MANIFEST_MISSING_HINT: &str = "skipping: TEST02_p/Manifest.txt fixture not found at \
     `test-file/backup-test/TEST02_p/Manifest.txt` — \
     plant-restore Step 1 integration coverage is NOT validated \
     on this run. Drop the backup folder under `test-file/` to \
     enable.";

#[test]
fn parse_real_test02_manifest_matches_on_disk_counts() {
    if !Path::new(MANIFEST_PATH).exists() {
        eprintln!("{MANIFEST_MISSING_HINT}");
        return;
    }

    // Read the file as raw bytes — `SmartPlant` writes
    // `Manifest.txt` as UTF-16 LE with a `FF FE` BOM, so
    // `read_to_string` would reject it as invalid UTF-8. The
    // byte-level entry point sniffs the BOM and decodes
    // transparently.
    let body = std::fs::read(MANIFEST_PATH).expect("read TEST02_p/Manifest.txt");
    let manifest = parse_manifest_bytes(&body);

    // Total parsed lines mirrors the file's non-blank line count
    // (319 raw lines, all populated — the parser drops nothing).
    assert_eq!(manifest.lines.len(), 319, "total parsed lines");

    // Singleton header keys.
    assert_eq!(manifest.first_field("BackupType"), Some("2"));
    assert_eq!(manifest.first_field("Version"), Some("7.02"));
    assert_eq!(
        manifest.first_field("DateCreated"),
        Some("04/20/2026 12:06:10")
    );
    assert_eq!(manifest.first_field("Name"), Some("TEST02"));
    assert_eq!(
        manifest.first_field("Spid"),
        Some("BB77101618824907AFAE785E6A863597"),
    );
    assert_eq!(manifest.first_field("ProjectType"), Some("0"));
    assert_eq!(manifest.first_field("BackupRefData"), Some("2"));
    assert_eq!(manifest.first_field("Serial_ID"), Some("260420120610"));
    assert_eq!(manifest.first_field("PidIsAssociated"), Some("2"));
    assert_eq!(manifest.first_field("SpelIsAssociated"), Some("1"));
    assert_eq!(manifest.first_field("SPIIsAssociated"), Some("1"));
    assert_eq!(manifest.first_field("DbaExport"), Some("2"));
    assert_eq!(manifest.first_field("ExportFileSize"), Some("20905983"));
    assert_eq!(manifest.first_field("ArchiveFileSize"), Some("139603035"));

    // Repeating-key counts — exact, see the PowerShell scan in
    // the PR description for how the numbers were derived.
    assert_eq!(manifest.tables().len(), 154, "Table line count");
    assert_eq!(manifest.views().len(), 35, "View line count");
    assert_eq!(manifest.all("Role").count(), 1, "Role line count");
    assert_eq!(manifest.all("Right").count(), 78, "Right line count");
    assert_eq!(manifest.all("File").count(), 14, "File line count");
    assert_eq!(manifest.all("FileSize").count(), 10, "FileSize line count");
    assert_eq!(
        manifest.all("PlantConnInfo").count(),
        4,
        "PlantConnInfo line count",
    );
    assert_eq!(
        manifest.all("SiteConnInfo").count(),
        2,
        "SiteConnInfo line count",
    );
    assert_eq!(
        manifest.all("DatabaseFiles").count(),
        2,
        "DatabaseFiles line count",
    );

    // The four logical databases the backup covers.
    let tables = manifest.tables();
    let databases: BTreeSet<String> = tables.iter().map(|t| t.database.clone()).collect();
    let expected_dbs: BTreeSet<String> = ["TEST02", "TEST02d", "TEST02pid", "TEST02pidd"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        databases, expected_dbs,
        "expected the four canonical SmartPlant databases",
    );

    // Essential P&ID drawing-side tables that downstream Step 3
    // (RestoredPlant) will need — drift here means the manifest
    // shape changed in a way that probably breaks restore.
    let pid_table_names: BTreeSet<String> = tables
        .iter()
        .filter(|t| t.database == "TEST02pid")
        .map(|t| t.name.clone())
        .collect();
    for required in [
        "T_Drawing",
        "T_Equipment",
        "T_Instrument",
        "T_PipeRun",
        "T_PipingComp",
        "T_PipingPoint",
        "T_Nozzle",
        "T_ModelItem",
        "T_Relationship",
        "T_Symbol",
        "T_Label",
    ] {
        assert!(
            pid_table_names.contains(required),
            "TEST02pid missing essential table `{required}`",
        );
    }
}
