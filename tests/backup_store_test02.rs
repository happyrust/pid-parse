//! The Backup Store built from the TEST02 plant backup (SQL Server),
//! plan `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`:
//! the file list (S2a), then the Manifest (S2b) and the dump (S2c).
//! Skips when the `<Plant>_p.zip` original is absent.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::backup_store::{check_manifest, ManifestExpectation};
use pid_parse::backup::{build_backup_store, build_backup_store_in_memory, StoreOptions};
use rusqlite::Connection;

const TEST02_ZIP: &str = "test-file/backup-test/TEST02_p.zip";

/// The Manifest: 319 lines; the default redaction replaces `DBUids`,
/// `DBPwds` and fields 1 and 5 of the six connection lines with their
/// SHA-256 (14 fields); the SQL Server `BACKUP DATABASE` command
/// carries no password and stays.
const MANIFEST: ManifestExpectation = ManifestExpectation {
    lines: 319,
    redactions: 14,
    passwords_masked: 0,
    tables: 154,
    views: 35,
    conn_infos: 6,
    files: 14,
    roles: 1,
    rights: 78,
    plant_name: "TEST02",
};

/// SHA-256 of the `<Plant>_p.zip` original, as `store_info` records it.
const TEST02_ZIP_SHA256_PREFIX: &str = "662ba5fd1f0f2019";

/// The 14 outer entries of the archive, in central directory order,
/// with their sizes and the format label the store derives from their
/// first bytes.
const OUTER: [(&str, u64, &str); 14] = [
    ("Export.dmp", 20_012_544, "ascii"),
    ("Manifest.txt", 34_326, "unknown:fffe4200"),
    ("PlantConfig.xml", 5_085, "unknown:efbbbf3c"),
    ("PlantData~2~711.zip", 11_513_686, "zip"),
    ("RefData~4~680", 332_275, "ascii"),
    ("RefData~4~681.zip", 5_365_701, "zip"),
    ("RefData~4~682.zip", 352_974, "zip"),
    ("RefData~4~683", 122_880, "cfb"),
    ("RefData~4~684.zip", 519_563, "zip"),
    ("RefData~4~685.zip", 316, "zip"),
    ("RefData~4~703", 9_435, "zip"),
    ("RefData~4~709", 27_958, "xml"),
    ("RefData~4~804.zip", 4_357_748, "zip"),
    ("RefData~4~809.zip", 166_744, "zip"),
];

/// Entries of each Option Archive.
const ARCHIVE_ENTRIES: [(&str, usize); 7] = [
    ("PlantData~2~711.zip", 782),
    ("RefData~4~681.zip", 703),
    ("RefData~4~682.zip", 21),
    ("RefData~4~684.zip", 12),
    ("RefData~4~685.zip", 3),
    ("RefData~4~804.zip", 10),
    ("RefData~4~809.zip", 9),
];

/// `.pid` files whose SHA-256 the format document (section 8) lists.
const PID_SHA256_PREFIXES: [(&str, &str, u64, &str); 3] = [
    (
        "PlantData~2~711.zip",
        "01/01/A01.pid",
        106_496,
        "8cccc7342c74",
    ),
    ("RefData~4~682.zip", "A2-W-New.pid", 57_344, "ca07fb6e45c0"),
    (
        "RefData~4~682.zip",
        "CPECCHBA2-new.pid",
        28_672,
        "70356068b7d2",
    ),
];

fn fixture() -> Option<&'static Path> {
    let path = Path::new(TEST02_ZIP);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skip: {TEST02_ZIP} is absent");
        None
    }
}

fn store_info(conn: &Connection) -> BTreeMap<String, String> {
    let mut stmt = conn.prepare("SELECT key, value FROM store_info").unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// `(path, size, format)` of the outer files, in id order.
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

/// Entries per Option Archive, keyed by the archive's outer path.
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

/// `(size, sha256)` of the entry `path` inside the archive `archive`.
fn inner_file(conn: &Connection, archive: &str, path: &str) -> (u64, String) {
    conn.query_row(
        "SELECT entry.size, entry.sha256 FROM backup_file entry \
         JOIN backup_file container ON entry.container_id = container.id \
         WHERE container.path = ?1 AND entry.path = ?2",
        [archive, path],
        |row| Ok((row.get::<_, i64>(0)? as u64, row.get(1)?)),
    )
    .unwrap_or_else(|err| panic!("{archive} / {path}: {err}"))
}

/// Every row of `backup_file` as text, for comparing two builds.
fn backup_file_rows(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, container_id, entry_index, path, hex(path_raw), path_encoding, is_dir, \
             size, sha256, format FROM backup_file ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        let mut text = String::new();
        for i in 0..10 {
            let value: rusqlite::types::Value = row.get(i)?;
            text.push_str(&format!("{value:?}|"));
        }
        Ok(text)
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

#[test]
fn test02_store_lists_the_outer_files_and_every_option_archive_entry() {
    let Some(zip) = fixture() else {
        return;
    };
    let (conn, summary) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store");

    let info = store_info(&conn);
    assert_eq!(
        Some("pid_backup_store"),
        info.get("tool_name").map(String::as_str)
    );
    assert_eq!(
        Some(env!("CARGO_PKG_VERSION")),
        info.get("tool_version").map(String::as_str)
    );
    assert_eq!(Some("zip"), info.get("input_kind").map(String::as_str));
    assert!(
        info["input_sha256"].starts_with(TEST02_ZIP_SHA256_PREFIX),
        "{}",
        info["input_sha256"]
    );
    assert_eq!(Some("1"), info.get("redacted").map(String::as_str));
    assert_eq!(Some("0"), info.get("files_embedded").map(String::as_str));
    assert_eq!(None, info.get("input_note"));

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
    let total = 14
        + ARCHIVE_ENTRIES
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>();
    assert_eq!(total, summary.files);
    assert_eq!(0, summary.files_embedded);
    assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
    let content_rows: i64 = conn
        .query_row("SELECT count(*) FROM backup_file_content", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(0, content_rows);

    for (archive, path, size, sha256_prefix) in PID_SHA256_PREFIXES {
        let (got_size, sha256) = inner_file(&conn, archive, path);
        assert_eq!(size, got_size, "{archive} / {path}");
        assert!(
            sha256.starts_with(sha256_prefix),
            "{archive} / {path}: {sha256}"
        );
    }

    // Directory entries carry no hash and no format; every file does.
    let (dirs_without, files_without): (i64, i64) = conn
        .query_row(
            "SELECT sum(is_dir = 1 AND sha256 IS NULL AND format IS NULL), \
                    sum(is_dir = 0 AND (sha256 IS NULL OR format IS NULL)) FROM backup_file",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let dirs: i64 = conn
        .query_row(
            "SELECT count(*) FROM backup_file WHERE is_dir = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dirs, dirs_without);
    assert_eq!(0, files_without);
    // Every name in the TEST02 archives is ASCII.
    let non_ascii: i64 = conn
        .query_row(
            "SELECT count(*) FROM backup_file WHERE path_encoding <> 'ascii'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(0, non_ascii);
}

#[test]
fn test02_store_embeds_file_bytes_only_when_asked() {
    let Some(zip) = fixture() else {
        return;
    };
    let options = StoreOptions {
        embed_files: true,
        ..StoreOptions::default()
    };
    let (conn, summary) = build_backup_store_in_memory(zip, &options).expect("build store");

    assert_eq!(
        Some("1"),
        store_info(&conn).get("files_embedded").map(String::as_str)
    );
    let files: i64 = conn
        .query_row(
            "SELECT count(*) FROM backup_file WHERE is_dir = 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let embedded: i64 = conn
        .query_row("SELECT count(*) FROM backup_file_content", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(files, embedded);
    assert_eq!(summary.files_embedded, embedded as usize);

    // The embedded bytes are what the hash was taken over.
    let (bytes, sha256): (Vec<u8>, String) = conn
        .query_row(
            "SELECT content.bytes, file.sha256 FROM backup_file_content content \
             JOIN backup_file file ON file.id = content.file_id \
             WHERE file.path = '01/01/A01.pid'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(106_496, bytes.len());
    assert!(bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]), "a CFB file");
    assert!(sha256.starts_with("8cccc7342c74"));
}

#[test]
fn test02_store_keeps_the_manifest_line_by_line_and_redacts_by_default() {
    let Some(zip) = fixture() else {
        return;
    };
    check_manifest(zip, "test02", &MANIFEST);

    // The SQL Server command stays as written: no password in it.
    let (conn, _) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store");
    let command: String = conn
        .query_row(
            "SELECT value FROM manifest_value WHERE key = 'BackupCommand'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!("BACKUP DATABASE SP3DTrain_RDB_SCHEMA TO TEST02", command);
    let database_files: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT logical_name, filegroup, content FROM manifest_database_file \
                 ORDER BY line_no",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        vec![
            (
                "SP3DTrain_RDB_SCHEMA_dat".to_string(),
                "PRIMARY".to_string(),
                "data only".to_string()
            ),
            (
                "SP3DTrain_RDB_SCHEMA_log".to_string(),
                String::new(),
                "log only".to_string()
            ),
        ],
        database_files
    );
    // The 711 archive's File line and its FileSize line agree with the archive.
    let (status, archive, files, dirs): (String, String, String, String) = conn
        .query_row(
            "SELECT f.status, f.archive_name_or_error, s.file_count, s.directory_count \
             FROM manifest_file f JOIN manifest_file_size s \
             ON s.schema_code = f.schema_code AND s.option_id = f.option_id \
             WHERE f.option_id = '711'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        ("1", "PlantData~2~711.zip", "676", "106"),
        (
            status.as_str(),
            archive.as_str(),
            files.as_str(),
            dirs.as_str()
        )
    );
}

#[test]
fn test02_store_is_written_through_a_temporary_file_and_builds_the_same_twice() {
    let Some(zip) = fixture() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("pid_parse_backup_store_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("TEST02.sqlite");
    let _ = std::fs::remove_file(&out);

    let summary = build_backup_store(zip, &out, &StoreOptions::default()).expect("build store");
    assert!(out.exists());
    assert!(
        !dir.join("TEST02.sqlite.tmp").exists(),
        "the temporary file is renamed away"
    );
    assert_eq!(1554, summary.files);

    let on_disk = Connection::open(&out).unwrap();
    let (again, _) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store again");
    assert_eq!(store_info(&on_disk), store_info(&again));
    assert_eq!(backup_file_rows(&on_disk), backup_file_rows(&again));

    drop(on_disk);
    let _ = std::fs::remove_dir_all(&dir);
}
