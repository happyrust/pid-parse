//! The Backup Store built from the DWG-0202GP06-01 plant backup
//! (Oracle), plan `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`:
//! the file list (S2a), then the Manifest (S2b) and the empty Oracle
//! tables (S2d). Skips when the `<Plant>_p.zip` original is absent.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::backup_store::{check_manifest, ManifestExpectation};
use pid_parse::backup::{build_backup_store_in_memory, StoreOptions};
use rusqlite::Connection;

const DWG_ZIP: &str = "test-file/backup-test/DWG-0202GP06-01_p.zip";

/// The Manifest: 402 lines (two roles, 156 rights); 14 SHA-256
/// replacements plus the password of the Oracle `exp` command line.
const MANIFEST: ManifestExpectation = ManifestExpectation {
    lines: 402,
    redactions: 15,
    passwords_masked: 1,
    tables: 154,
    views: 35,
    conn_infos: 6,
    files: 14,
    roles: 2,
    rights: 156,
    plant_name: "QSMCQTAZ13_PLANT",
};

/// SHA-256 of the `<Plant>_p.zip` original.
const DWG_ZIP_SHA256_PREFIX: &str = "208559916e5bf603";

/// The 16 outer entries, in central directory order, with sizes and
/// format labels. `Export.dmp` is an Oracle `exp` dump (`03 03 69 E`),
/// `Export.log` starts with CRLF.
const OUTER: [(&str, u64, &str); 16] = [
    ("Export.dmp", 8_802_304, "unknown:03036945"),
    ("Export.log", 15_715, "unknown:0d0a436f"),
    ("Manifest.txt", 48_880, "unknown:fffe4200"),
    ("PlantConfig.xml", 5_085, "unknown:efbbbf3c"),
    ("PlantData~2~711.zip", 283_099, "zip"),
    ("PlantData~2~722", 12_903, "unknown:efbbbf3c"),
    ("RefData~4~680", 332_275, "ascii"),
    ("RefData~4~681.zip", 5_614_375, "zip"),
    ("RefData~4~682.zip", 387_773, "zip"),
    ("RefData~4~683", 122_880, "cfb"),
    ("RefData~4~684.zip", 519_563, "zip"),
    ("RefData~4~685.zip", 35_102, "zip"),
    ("RefData~4~703", 9_435, "zip"),
    ("RefData~4~709", 27_958, "xml"),
    ("RefData~4~804.zip", 4_357_748, "zip"),
    ("RefData~4~809.zip", 166_744, "zip"),
];

/// Entries of each Option Archive.
const ARCHIVE_ENTRIES: [(&str, usize); 7] = [
    ("PlantData~2~711.zip", 12),
    ("RefData~4~681.zip", 715),
    ("RefData~4~682.zip", 24),
    ("RefData~4~684.zip", 12),
    ("RefData~4~685.zip", 4),
    ("RefData~4~804.zip", 10),
    ("RefData~4~809.zip", 9),
];

/// `.pid` files whose SHA-256 the format document (section 8) lists;
/// the assembly `wuyouchi.pid` sits in two archives with the same bytes.
const PID_SHA256_PREFIXES: [(&str, &str, u64, &str); 3] = [
    (
        "PlantData~2~711.zip",
        "zcgc/A3jqz/DWG-0202GP06-01.pid",
        360_448,
        "eafe73905f4e",
    ),
    (
        "RefData~4~681.zip",
        "Assemblies/Equipment/wuyouchi.pid",
        208_896,
        "1473297b3ef6",
    ),
    (
        "RefData~4~685.zip",
        "Equipment/wuyouchi.pid",
        208_896,
        "1473297b3ef6",
    ),
];

fn fixture() -> Option<&'static Path> {
    let path = Path::new(DWG_ZIP);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skip: {DWG_ZIP} is absent");
        None
    }
}

fn outer_files(conn: &Connection) -> Vec<(String, u64, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT path, size, format FROM backup_file WHERE container_id IS NULL ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? as u64,
            row.get::<_, String>(2)?,
        ))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

fn archive_entry_counts(conn: &Connection) -> BTreeMap<String, usize> {
    let mut stmt = conn
        .prepare(
            "SELECT container.path, count(*) FROM backup_file entry \
             JOIN backup_file container ON entry.container_id = container.id \
             GROUP BY container.path",
        )
        .unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn dwg_store_lists_the_outer_files_and_every_option_archive_entry() {
    let Some(zip) = fixture() else {
        return;
    };
    let (conn, summary) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store");

    let input_sha256: String = conn
        .query_row(
            "SELECT value FROM store_info WHERE key = 'input_sha256'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        input_sha256.starts_with(DWG_ZIP_SHA256_PREFIX),
        "{input_sha256}"
    );

    assert_eq!(
        OUTER
            .iter()
            .map(|(path, size, format)| ((*path).to_string(), *size, (*format).to_string()))
            .collect::<Vec<_>>(),
        outer_files(&conn)
    );
    assert_eq!(
        ARCHIVE_ENTRIES
            .iter()
            .map(|(archive, count)| ((*archive).to_string(), *count))
            .collect::<BTreeMap<_, _>>(),
        archive_entry_counts(&conn)
    );
    assert_eq!(
        16 + ARCHIVE_ENTRIES
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>(),
        summary.files
    );

    for (archive, path, size, sha256_prefix) in PID_SHA256_PREFIXES {
        let (got_size, sha256): (i64, String) = conn
            .query_row(
                "SELECT entry.size, entry.sha256 FROM backup_file entry \
                 JOIN backup_file container ON entry.container_id = container.id \
                 WHERE container.path = ?1 AND entry.path = ?2",
                [archive, path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|err| panic!("{archive} / {path}: {err}"));
        assert_eq!(size, got_size as u64, "{archive} / {path}");
        assert!(
            sha256.starts_with(sha256_prefix),
            "{archive} / {path}: {sha256}"
        );
    }
}

#[test]
fn dwg_store_keeps_the_manifest_and_masks_the_oracle_password() {
    let Some(zip) = fixture() else {
        return;
    };
    check_manifest(zip, "dwg", &MANIFEST);

    let (conn, _) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store");
    let command: String = conn
        .query_row(
            "SELECT value FROM manifest_value WHERE key = 'BackupCommand'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        command.starts_with("Exp.exe SYSTEM/***@SPIDDB CONSISTENT=y OWNER=(QSMCQTAZ13_PLANTpidd,"),
        "{command}"
    );
    // Oracle-only lines read through the views.
    let (characterset, version): (String, String) = conn
        .query_row(
            "SELECT (SELECT value FROM manifest_value WHERE key = 'Characterset'), \
                    (SELECT value FROM manifest_value WHERE key = 'OracleVersion')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        ("AL32UTF8", "12.1.0.2.0"),
        (characterset.as_str(), version.as_str())
    );
    let kinds: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT tablespace_kind FROM manifest_table_space ORDER BY line_no")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        vec!["Permanent".to_string(), "Temporary".to_string()],
        kinds
    );
}
