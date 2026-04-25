//! Integration gate for `crate::backup::zip_index::list_zip_entries`
//! against real `RefData~4~*.zip` payloads from the bundled
//! `TEST02_p` plant backup.
//!
//! `crate::backup::zip_index` already has unit-level coverage on
//! synthetic in-memory archives. This file complements those with
//! production-shape checks against three of the ten `RefData~4~*`
//! files the bundled fixture ships:
//!
//! * `RefData~4~681.zip` — the Symbol Catalogue: 703 entries
//!   spanning `Assemblies/`, `Toolbars/`, individual `.sym`
//!   files, and the central `CatalogIndex.xml` index.
//! * `RefData~4~685.zip` — three empty directory entries
//!   (Equipment / Instrumentation / Piping). Smallest fixture so
//!   asserting full equality stays cheap.
//! * `RefData~4~703` — extension-less; magic-byte sniffs as
//!   ZIP and turns out to be an OOXML `.xlsx` package, exercising
//!   the "no `.zip` suffix" path that
//!   [`crate::backup::scan_refdata_dir`] discovered.
//!
//! When the fixture tree is missing (the `test-file/` directory
//! is git-ignored — see `AGENTS.md`), every test soft-skips with
//! an explicit hint mirroring the rest of the backup test suite.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pid_parse::backup::list_zip_entries;

const FIXTURE_DIR: &str = "test-file/backup-test/TEST02_p";

const FIXTURE_MISSING_HINT: &str = "skipping: TEST02_p plant backup fixture not found at \
     `test-file/backup-test/TEST02_p` — \
     plant-restore Step 2.2 ZIP entry index coverage is NOT \
     validated on this run. Drop the backup folder under \
     `test-file/` to enable.";

fn fixture(name: &str) -> Option<PathBuf> {
    let path = Path::new(FIXTURE_DIR).join(name);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("{FIXTURE_MISSING_HINT}");
        None
    }
}

#[test]
fn list_symbol_catalogue_contains_703_entries_and_catalog_index() {
    let Some(path) = fixture("RefData~4~681.zip") else {
        return;
    };
    let entries = list_zip_entries(&path).expect("list 681.zip");

    // Bundled Symbol Catalogue is exactly 703 entries.
    assert_eq!(entries.len(), 703, "symbol catalogue entry count");

    // First entry is the `Assemblies/` directory placeholder.
    let first = &entries[0];
    assert_eq!(first.name, "Assemblies/");
    assert!(first.is_dir, "Assemblies/ must be a directory entry");
    assert_eq!(first.size, 0);

    // The central XML index ships as `CatalogIndex.xml` at
    // 493_202 bytes uncompressed — Step 2.4 (typed RefData
    // indices) will lean on this file as the canonical map of
    // every symbol.
    let catalog_index = entries
        .iter()
        .find(|e| e.name == "CatalogIndex.xml")
        .expect("CatalogIndex.xml must be present in 681.zip");
    assert!(!catalog_index.is_dir);
    assert_eq!(catalog_index.size, 493_202);
    assert!(
        catalog_index.compressed_size < catalog_index.size,
        "CatalogIndex.xml should compress",
    );

    // Sanity: at least a few `.sym` files surface so the
    // catalogue is not just placeholder directories.
    let sym_count = entries
        .iter()
        .filter(|e| !e.is_dir && e.name.ends_with(".sym"))
        .count();
    assert!(
        sym_count >= 50,
        "expected many `.sym` symbol files; got {sym_count}",
    );
}

#[test]
fn list_minimal_skeleton_zip_has_three_empty_directory_entries() {
    let Some(path) = fixture("RefData~4~685.zip") else {
        return;
    };
    let entries = list_zip_entries(&path).expect("list 685.zip");

    assert_eq!(entries.len(), 3, "skeleton archive entry count");

    let names: BTreeSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    let expected: BTreeSet<&str> = ["Equipment/", "Instrumentation/", "Piping/"]
        .into_iter()
        .collect();
    assert_eq!(names, expected);

    for entry in &entries {
        assert!(entry.is_dir, "{} must be a directory entry", entry.name);
        assert_eq!(entry.size, 0, "{} must have zero size", entry.name);
        assert_eq!(entry.compressed_size, 0);
    }
}

#[test]
fn list_extension_less_zip_reads_ooxml_xlsx_package_layout() {
    // `RefData~4~703` carries no `.zip` suffix on disk yet
    // sniffs as ZIP via `classify_format`. Internally it is an
    // OOXML `.xlsx` package, so we expect the canonical OOXML
    // skeleton (`[Content_Types].xml`, `xl/workbook.xml`, etc.)
    // — confirming that `list_zip_entries` happily reads a ZIP
    // regardless of file extension.
    let Some(path) = fixture("RefData~4~703") else {
        return;
    };
    let entries = list_zip_entries(&path).expect("list 703");
    assert_eq!(entries.len(), 12, "xlsx package entry count");

    let names: BTreeSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "xl/workbook.xml",
        "xl/sharedStrings.xml",
        "xl/styles.xml",
        "xl/theme/theme1.xml",
        "xl/worksheets/sheet1.xml",
        "docProps/app.xml",
        "docProps/core.xml",
    ] {
        assert!(
            names.contains(required),
            "OOXML xlsx package missing `{required}`",
        );
    }

    let content_types = entries
        .iter()
        .find(|e| e.name == "[Content_Types].xml")
        .expect("[Content_Types].xml present");
    assert_eq!(content_types.size, 1440);
}
