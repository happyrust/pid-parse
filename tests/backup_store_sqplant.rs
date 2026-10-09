//! The Backup Store built from the SQPlant plant backup (Oracle), the
//! external sample of plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`
//! (P10): a directory, not a zip, found through `PID_PARSE_SQPLANT_BACKUP`
//! or at `D:\work\cad\pid-test-data`; skips when neither is there.
//! The file list (S2a), then the Manifest (S2b) and the empty Oracle
//! tables (S2d).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::backup_store::{check_manifest, ManifestExpectation};
use pid_parse::backup::store::DIRECTORY_INPUT_NOTE;
use pid_parse::backup::{build_backup_store_in_memory, StoreOptions};
use rusqlite::Connection;

const SQPLANT_ENV: &str = "PID_PARSE_SQPLANT_BACKUP";
const SQPLANT_DEFAULT: &str = r"D:\work\cad\pid-test-data";

/// The Manifest: 322 lines; 14 SHA-256 replacements plus the password
/// of the Oracle `exp` command line, which the test takes from the
/// original Manifest at run time and never writes down.
const MANIFEST: ManifestExpectation = ManifestExpectation {
    lines: 322,
    redactions: 15,
    passwords_masked: 1,
    tables: 154,
    views: 35,
    conn_infos: 6,
    files: 14,
    roles: 1,
    rights: 78,
    plant_name: "SQPlant",
};

/// The 16 top-level files, in byte order of their names, with sizes
/// and format labels. Text files are the raw bytes `SmartPlant` wrote
/// (`RefData~4~680` 331,977, `PlantConfig.xml` 5,085).
const OUTER: [(&str, u64, &str); 16] = [
    ("Export.dmp", 74_489_856, "unknown:03036945"),
    ("Export.log", 15_571, "unknown:0d0a436f"),
    ("Manifest.txt", 34_730, "unknown:fffe4200"),
    ("PlantConfig.xml", 5_085, "unknown:efbbbf3c"),
    ("PlantData~2~711.zip", 21_412_367, "zip"),
    ("PlantData~2~722", 12_903, "unknown:efbbbf3c"),
    ("RefData~4~680", 331_977, "ascii"),
    ("RefData~4~681.zip", 5_510_126, "zip"),
    ("RefData~4~682.zip", 271_625, "zip"),
    ("RefData~4~683", 122_880, "cfb"),
    ("RefData~4~684.zip", 390_048, "zip"),
    ("RefData~4~685.zip", 316, "zip"),
    ("RefData~4~703", 9_435, "zip"),
    ("RefData~4~709", 27_958, "xml"),
    ("RefData~4~804.zip", 4_357_748, "zip"),
    ("RefData~4~809.zip", 20_528, "zip"),
];

/// Entries of each Option Archive.
const ARCHIVE_ENTRIES: [(&str, usize); 7] = [
    ("PlantData~2~711.zip", 60),
    ("RefData~4~681.zip", 742),
    ("RefData~4~682.zip", 20),
    ("RefData~4~684.zip", 10),
    ("RefData~4~685.zip", 3),
    ("RefData~4~804.zip", 10),
    ("RefData~4~809.zip", 4),
];

/// Names in the 711 archive stored without the UTF-8 flag and with
/// bytes above ASCII: GBK, 47 of the 60.
const GBK_NAMES_IN_711: usize = 47;

/// Two drawings of the 711 archive: one named in GBK, one in ASCII.
const PINNED_DRAWINGS: [(&str, &str, u64); 2] = [
    ("00/00/A井场 注采阀组工艺及自控流程图.pid", "gbk", 3_264_512),
    ("test/U01/D06.pid", "ascii", 229_376),
];

fn fixture() -> Option<PathBuf> {
    let path = std::env::var_os(SQPLANT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SQPLANT_DEFAULT));
    if path.join("Manifest.txt").is_file() {
        Some(path)
    } else {
        eprintln!(
            "skip: SQPlant backup is absent ({SQPLANT_ENV} unset, {} has no Manifest.txt)",
            path.display()
        );
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
fn sqplant_store_lists_the_directory_and_reads_gbk_entry_names() {
    let Some(dir) = fixture() else {
        return;
    };
    let (conn, summary) = build_backup_store_in_memory(Path::new(&dir), &StoreOptions::default())
        .expect("build store");

    let info: BTreeMap<String, String> = {
        let mut stmt = conn.prepare("SELECT key, value FROM store_info").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        Some("directory"),
        info.get("input_kind").map(String::as_str)
    );
    assert_eq!(
        Some(DIRECTORY_INPUT_NOTE),
        info.get("input_note").map(String::as_str)
    );
    assert_eq!(64, info["input_sha256"].len());
    assert_eq!(vec![DIRECTORY_INPUT_NOTE.to_string()], summary.warnings);

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

    // The 711 archive's names: 47 GBK, the rest ASCII, none with a
    // replacement character.
    let encodings: BTreeMap<String, usize> = {
        let mut stmt = conn
            .prepare(
                "SELECT entry.path_encoding, count(*) FROM backup_file entry \
                 JOIN backup_file container ON entry.container_id = container.id \
                 WHERE container.path = 'PlantData~2~711.zip' GROUP BY entry.path_encoding",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        BTreeMap::from([
            ("ascii".to_string(), 60 - GBK_NAMES_IN_711),
            ("gbk".to_string(), GBK_NAMES_IN_711),
        ]),
        encodings
    );
    let with_replacement: i64 = conn
        .query_row(
            "SELECT count(*) FROM backup_file WHERE instr(path, char(65533)) > 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(0, with_replacement);
    // Outside the 711 archive every name is ASCII.
    let non_ascii_elsewhere: i64 = conn
        .query_row(
            "SELECT count(*) FROM backup_file WHERE path_encoding <> 'ascii' \
             AND container_id IS NOT (SELECT id FROM backup_file WHERE path = 'PlantData~2~711.zip')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(0, non_ascii_elsewhere);

    for (path, encoding, size) in PINNED_DRAWINGS {
        let (got_encoding, got_size, raw, sha256): (String, i64, Vec<u8>, String) = conn
            .query_row(
                "SELECT entry.path_encoding, entry.size, entry.path_raw, entry.sha256 \
                 FROM backup_file entry \
                 JOIN backup_file container ON entry.container_id = container.id \
                 WHERE container.path = 'PlantData~2~711.zip' AND entry.path = ?1",
                [path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap_or_else(|err| panic!("{path}: {err}"));
        assert_eq!(encoding, got_encoding, "{path}");
        assert_eq!(size, got_size as u64, "{path}");
        assert_eq!(64, sha256.len(), "{path}");
        // The raw bytes decode back to the path under the recorded encoding.
        let decoded = if encoding == "gbk" {
            encoding_rs::GBK.decode(&raw).0.into_owned()
        } else {
            String::from_utf8(raw).unwrap()
        };
        assert_eq!(path, decoded);
    }
}

#[test]
fn sqplant_store_keeps_the_manifest_and_masks_the_oracle_password() {
    let Some(dir) = fixture() else {
        return;
    };
    check_manifest(Path::new(&dir), "sqplant", &MANIFEST);

    let (conn, _) = build_backup_store_in_memory(Path::new(&dir), &StoreOptions::default())
        .expect("build store");
    let command: String = conn
        .query_row(
            "SELECT value FROM manifest_value WHERE key = 'BackupCommand'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        command.starts_with("Exp.exe system/***@ORCL CONSISTENT=y OWNER=(SQPlantpidd,SQPlantd,"),
        "{command}"
    );
    // The Manifest spells the schemas SQPlant / SQPlantd / SQPlantpid /
    // SQPlantpidd while the dump's owners are upper-case (P12).
    let schemas: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT schema_name FROM manifest_conn_info WHERE scope = 'Plant' \
                 ORDER BY schema_type_code",
            )
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        vec!["SQPlant", "SQPlantpid", "SQPlantd", "SQPlantpidd"],
        schemas
    );
}
