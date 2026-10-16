//! Integration gate: inventory a real `TEST02_p` backup folder in one call.
//!
//! Unit tests in `src/backup/folder.rs` cover synthetic directories.
//! This file checks that `scan_backup_folder` sees the same manifest,
//! `Export.dmp`, and `RefData` set the individual parsers already
//! ratchet, and that every on-disk `PlantData` file shows up. When
//! `test-file/` is absent the test soft-skips.

use std::fs;
use std::path::Path;

use pid_parse::backup::{
    parse_plant_data_filename, scan_backup_folder, ExportDumpFormat, MtfBlockType, RefDataFormat,
    ZipListing,
};

const FIXTURE_DIR: &str = "test-file/backup-test/TEST02_p";

const FIXTURE_MISSING_HINT: &str = "skipping: TEST02_p plant backup fixture not found at \
     `test-file/backup-test/TEST02_p` — \
     backup-folder inventory coverage is NOT validated on this run. \
     Drop the backup folder under `test-file/` to enable.";

#[test]
fn scan_real_test02_folder_inventories_manifest_export_refdata_and_plant_data() {
    let dir = Path::new(FIXTURE_DIR);
    if !dir.exists() {
        eprintln!("{FIXTURE_MISSING_HINT}");
        return;
    }

    let scan = scan_backup_folder(dir).expect("scan TEST02_p");
    assert!(
        scan.problems.is_empty(),
        "fixture members should all read: {:?}",
        scan.problems
    );

    let manifest = scan.manifest.expect("Manifest.txt");
    assert_eq!(manifest.lines.len(), 319);
    assert_eq!(manifest.first_field("Name"), Some("TEST02"));
    assert_eq!(manifest.first_field("Version"), Some("7.02"));

    let export = scan.export.expect("Export.dmp");
    assert!(
        export.size > 4096,
        "Export.dmp is sniffed, not fully loaded"
    );
    match export.format {
        ExportDumpFormat::SqlServerMtf(header) => {
            assert_eq!(header.block_type, MtfBlockType::Tape);
        }
        other => panic!("TEST02 Export.dmp is SQL Server MTF, got {other:?}"),
    }

    let ref_ids: Vec<u32> = scan.refdata.iter().map(|file| file.entry.id).collect();
    assert_eq!(
        ref_ids,
        vec![680, 681, 682, 683, 684, 685, 703, 709, 804, 809]
    );
    let catalogue = scan
        .refdata
        .iter()
        .find(|file| file.entry.id == 681)
        .expect("symbol catalogue");
    match &catalogue.zip {
        ZipListing::Listed(entries) => assert_eq!(entries.len(), 703),
        other => panic!("RefData~4~681.zip should list, got {other:?}"),
    }
    for file in &scan.refdata {
        match file.entry.format {
            RefDataFormat::Zip => assert!(
                matches!(file.zip, ZipListing::Listed(_)),
                "{} should list its central directory",
                file.entry.file_name
            ),
            _ => assert_eq!(file.zip, ZipListing::NotAZip),
        }
    }

    let mut expected_plant = Vec::new();
    for dirent in fs::read_dir(dir).expect("re-read fixture") {
        let dirent = dirent.expect("dirent");
        if !dirent.file_type().expect("file type").is_file() {
            continue;
        }
        let name = dirent.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Some((schema, id)) = parse_plant_data_filename(name) {
            expected_plant.push((schema, id, name.to_string()));
        }
    }
    expected_plant.sort();
    let got_plant: Vec<(u32, u32, String)> = scan
        .plant_data
        .iter()
        .map(|file| (file.schema, file.id, file.file_name.clone()))
        .collect();
    assert_eq!(got_plant, expected_plant);
    assert!(
        !scan.plant_data.is_empty(),
        "TEST02_p ships at least one PlantData archive"
    );
}
