//! The Backup Store built from the TEST02 plant backup (SQL Server),
//! plan `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`:
//! the file list (S2a), then the Manifest (S2b) and the dump (S2c).
//! Skips when the `<Plant>_p.zip` original is absent.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::backup_store::{check_manifest, ManifestExpectation};
use oxidized_mdf::{MdfDatabase, ScannedRecord};
use pid_parse::backup::mtf::mdf_bytes_of_dump;
use pid_parse::backup::store::{store_value, BackupInput};
use pid_parse::backup::{build_backup_store, build_backup_store_in_memory, StoreOptions};
use rusqlite::types::Value as SqlValue;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

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

/// The four schemas of the dump, by role: Manifest name, type code.
const SCHEMAS: [(&str, &str, &str); 4] = [
    ("plant", "TEST02", "2"),
    ("plantd", "TEST02d", "8"),
    ("pid", "TEST02pid", "4"),
    ("pidd", "TEST02pidd", "9"),
];

/// Tables and views per role.
const TABLES_PER_ROLE: [(&str, usize); 4] =
    [("plant", 22), ("plantd", 25), ("pid", 82), ("pidd", 25)];
const VIEWS_PER_ROLE: [(&str, usize); 2] = [("plantd", 25), ("pid", 10)];

/// Columns over the 154 tables by source type (1,798 in all).
const COLUMNS_BY_TYPE: [(&str, usize); 5] = [
    ("datetime", 27),
    ("float", 263),
    ("image", 2),
    ("int", 650),
    ("nvarchar", 856),
];

/// NULLs over the dumped tables by source type (50,596 in all) and the
/// empty strings among the nvarchar columns.
const NULLS_BY_TYPE: [(&str, usize); 4] = [
    ("datetime", 8),
    ("float", 148),
    ("int", 29_009),
    ("nvarchar", 21_431),
];
const EMPTY_STRINGS: usize = 162;

/// The tables holding a Ghost Row (all in the pid schema).
const GHOST_TABLES: [&str; 5] = [
    "T_Equipment",
    "T_EquipmentOther",
    "T_PlantItem",
    "T_SmartFrameStorage",
    "T_Symbol",
];

/// The LOBs: table, root page and slot, length, first ZIP entry.
const LOBS: [(&str, u32, u16, usize, &str); 4] = [
    ("T_SmartFrameStorage", 2247, 1, 32_768, "A01-JSite204.tmp"),
    ("T_DrawingVersion", 2323, 3, 32_768, "Drawing.xml"),
    ("T_DrawingVersion", 2323, 1, 65_536, "Drawing.xml"),
    ("T_DrawingVersion", 2323, 5, 32_768, "Drawing.xml"),
];

fn query_pairs(conn: &Connection, sql: &str) -> Vec<(String, i64)> {
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|err| panic!("{sql}: {err}"));
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0))
        .unwrap_or_else(|err| panic!("{sql}: {err}"))
}

#[test]
fn test02_store_dumps_every_table_the_manifest_lists_with_its_provenance() {
    let Some(zip) = fixture() else {
        return;
    };
    let (conn, summary) =
        build_backup_store_in_memory(zip, &StoreOptions::default()).expect("build store");
    assert_eq!(
        Some("sql-server-mtf"),
        store_info(&conn).get("dump_kind").map(String::as_str)
    );
    assert_eq!(
        (154, 37_470, 5, 4),
        (
            summary.tables,
            summary.rows,
            summary.ghost_rows,
            summary.lobs
        )
    );

    // Acceptance 1: every table to its rcrows, 37,470 in all; the catalogue.
    assert_eq!(
        SCHEMAS
            .iter()
            .map(|(role, name, code)| (
                (*role).to_string(),
                (*name).to_string(),
                (*code).to_string()
            ))
            .collect::<Vec<_>>(),
        {
            let mut stmt = conn
                .prepare(
                    "SELECT role, schema_name, type_code FROM dump_schema \
                     WHERE db_type = '1' AND role_source = 'manifest-conninfo' \
                     ORDER BY CASE role WHEN 'plant' THEN 0 WHEN 'plantd' THEN 1 \
                     WHEN 'pid' THEN 2 ELSE 3 END",
                )
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<Vec<(String, String, String)>, _>>()
                .unwrap()
        }
    );
    assert_eq!(
        TABLES_PER_ROLE
            .iter()
            .map(|(role, n)| ((*role).to_string(), *n as i64))
            .collect::<BTreeMap<_, _>>(),
        query_pairs(&conn, "SELECT role, count(*) FROM dump_table GROUP BY role")
            .into_iter()
            .collect()
    );
    assert_eq!(37_470, count(&conn, "SELECT sum(rows) FROM dump_table"));
    assert_eq!(
        0,
        count(
            &conn,
            "SELECT count(*) FROM dump_table WHERE rows <> expected_rows OR decoded <> 1"
        )
    );
    assert_eq!(
        VIEWS_PER_ROLE
            .iter()
            .map(|(role, n)| ((*role).to_string(), *n as i64))
            .collect::<BTreeMap<_, _>>(),
        query_pairs(&conn, "SELECT role, count(*) FROM dump_view GROUP BY role")
            .into_iter()
            .collect()
    );
    assert_eq!(
        COLUMNS_BY_TYPE
            .iter()
            .map(|(t, n)| ((*t).to_string(), *n as i64))
            .collect::<BTreeMap<_, _>>(),
        query_pairs(
            &conn,
            "SELECT source_type, count(*) FROM dump_column GROUP BY source_type"
        )
        .into_iter()
        .collect()
    );
    // Every dumped table exists with its provenance columns, and the
    // row counts of the tables themselves agree with the catalogue.
    let store_tables: Vec<(String, i64)> = query_pairs(
        &conn,
        "SELECT store_name, rows FROM dump_table ORDER BY store_name",
    );
    assert_eq!(154, store_tables.len());
    for (store_name, rows) in &store_tables {
        assert_eq!(
            *rows,
            count(&conn, &format!("SELECT count(*) FROM \"{store_name}\"")),
            "{store_name}"
        );
        let columns: Vec<String> = {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT name FROM pragma_table_info('{store_name}')"
                ))
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            Some(&["_src_page".to_string(), "_src_slot".to_string()][..]),
            columns.get(columns.len() - 2..),
            "{store_name}"
        );
    }
    assert_eq!(
        154,
        count(
            &conn,
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name LIKE '%\\_\\_%' ESCAPE '\\'"
        )
    );
    // SP_ID is NOT NULL on T_Drawing; its Description admits NULL. 206
    // columns are declared NOT NULL (syscolpars.status bit 1 set), the
    // other 1,592 admit NULL.
    assert_eq!(
        vec![("Description".to_string(), 1), ("SP_ID".to_string(), 0)],
        query_pairs(
            &conn,
            "SELECT name, nullable FROM dump_column WHERE role = 'pid' AND table_name = 'T_Drawing' \
             AND name IN ('SP_ID', 'Description') ORDER BY name"
        )
    );
    assert_eq!(
        vec![("0".to_string(), 206), ("1".to_string(), 1_592)],
        query_pairs(
            &conn,
            "SELECT CAST(nullable AS TEXT), count(*) FROM dump_column GROUP BY nullable ORDER BY nullable"
        )
    );

    // Acceptance 4: empty strings and NULLs, counted in the dumped tables.
    let mut empty_strings = 0i64;
    let mut nulls_by_type: BTreeMap<String, i64> = BTreeMap::new();
    let mut nulls_in_not_null_columns: Vec<(String, String, i64)> = Vec::new();
    let columns: Vec<(String, String, String, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT t.store_name, c.name, c.source_type, c.nullable FROM dump_column c \
                 JOIN dump_table t ON t.role = c.role AND t.source_name = c.table_name",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    };
    assert_eq!(1_798, columns.len());
    for (store_name, column, source_type, nullable) in &columns {
        let (nulls, empties): (i64, i64) = conn
            .query_row(
                &format!(
                    "SELECT count(*) FILTER (WHERE \"{column}\" IS NULL), \
                     count(*) FILTER (WHERE \"{column}\" = '') FROM \"{store_name}\""
                ),
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        *nulls_by_type.entry(source_type.clone()).or_default() += nulls;
        if source_type == "nvarchar" {
            empty_strings += empties;
        }
        if *nullable == 0 && nulls > 0 {
            nulls_in_not_null_columns.push((store_name.clone(), column.clone(), nulls));
        }
    }
    nulls_by_type.retain(|_, n| *n > 0);
    assert_eq!(
        NULLS_BY_TYPE
            .iter()
            .map(|(t, n)| ((*t).to_string(), *n as i64))
            .collect::<BTreeMap<_, _>>(),
        nulls_by_type
    );
    assert_eq!(EMPTY_STRINGS as i64, empty_strings);
    // One row contradicts its declaration: T_Symbol's SP_ID is NOT NULL,
    // yet the record at 2300:1 has the column's null bit set (its fixed
    // columns hold pointer-like bytes, its variable part a 32-character
    // SP_ID). The store writes what the null bitmap says, as every other
    // NULL is counted (Q9); dump_column.nullable lets a reader find it.
    assert_eq!(
        vec![("pid__T_Symbol".to_string(), "SP_ID".to_string(), 1)],
        nulls_in_not_null_columns
    );
    assert_eq!(
        (2_300, 1),
        conn.query_row(
            "SELECT _src_page, _src_slot FROM pid__T_Symbol WHERE SP_ID IS NULL",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap()
    );
    // datetime as YYYY-MM-DD HH:MM:SS.fff; float and int as numbers.
    let (created, diameter, run_type): (String, f64, i64) = conn
        .query_row(
            "SELECT d.DateCreated, r.NominalDiameter, r.PipeRunType FROM pid__T_Drawing d, \
             pid__T_PipeRun r WHERE d.SP_ID = 'D9635C3C898840D1990B7E8BEE1D55DA' \
             AND r.SP_ID = '185EF98B03E844158E3BD8E82806E6CF'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        ("2026-04-20 10:32:46.000", 250.0, 20),
        (created.as_str(), diameter, run_type)
    );

    // Acceptance 2: the five Ghost Rows, byte for byte what the MDF holds.
    let mut backup = BackupInput::open(zip).unwrap();
    let dump_index = backup
        .files()
        .iter()
        .find(|file| file.name == "Export.dmp")
        .unwrap()
        .entry_index;
    let dump = backup.read(dump_index).unwrap();
    let mdf = mdf_bytes_of_dump(&dump).unwrap();
    let ghosts: Vec<(String, i64, i64, i64, Vec<u8>)> = {
        let mut stmt = conn
            .prepare(
                "SELECT table_name, page, slot, record_type, bytes FROM dump_ghost_row \
                 WHERE role = 'pid' ORDER BY table_name",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    };
    assert_eq!(5, count(&conn, "SELECT count(*) FROM dump_ghost_row"));
    assert_eq!(
        GHOST_TABLES
            .iter()
            .map(|t| (*t).to_string())
            .collect::<Vec<_>>(),
        ghosts.iter().map(|(t, ..)| t.clone()).collect::<Vec<_>>()
    );
    for (table, page, slot, record_type, bytes) in &ghosts {
        assert_eq!(6, *record_type, "{table}");
        let page_bytes = &mdf[*page as usize * 8192..(*page as usize + 1) * 8192];
        let entry = 8192 - 2 * (*slot as usize + 1);
        let offset = u16::from_le_bytes([page_bytes[entry], page_bytes[entry + 1]]) as usize;
        assert_eq!(
            &page_bytes[offset..offset + bytes.len()],
            bytes.as_slice(),
            "{table}"
        );
    }
    assert_eq!(
        5,
        count(&conn, "SELECT sum(ghost_rows) FROM dump_table"),
        "the catalogue counts them too"
    );

    // Acceptance 3: the four LOBs, ZIPs with the expected first entry,
    // hashed as stored.
    let lobs: Vec<(String, i64, i64, i64, i64, i64, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT table_name, row_page, row_slot, root_page, root_slot, length, sha256 \
                 FROM dump_lob WHERE role = 'pid' AND column_name = 'SP_Storage' \
                 ORDER BY row_page, row_slot",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    };
    assert_eq!(4, lobs.len());
    assert_eq!(
        LOBS.iter()
            .map(|(table, root_page, root_slot, length, _)| (
                (*table).to_string(),
                i64::from(*root_page),
                i64::from(*root_slot),
                *length as i64
            ))
            .collect::<Vec<_>>(),
        lobs.iter()
            .map(|(table, _, _, root_page, root_slot, length, _)| (
                table.clone(),
                *root_page,
                *root_slot,
                *length
            ))
            .collect::<Vec<_>>()
    );
    for ((table, row_page, row_slot, _, _, length, sha256), (_, _, _, _, first_entry)) in
        lobs.iter().zip(LOBS.iter())
    {
        let blob: Vec<u8> = conn
            .query_row(
                &format!(
                    "SELECT SP_Storage FROM \"pid__{table}\" WHERE _src_page = ?1 AND _src_slot = ?2"
                ),
                [row_page, row_slot],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(*length as usize, blob.len(), "{table}");
        assert!(blob.starts_with(b"PK\x03\x04"), "{table}: a ZIP");
        let name_len = u16::from_le_bytes([blob[26], blob[27]]) as usize;
        assert_eq!(first_entry.as_bytes(), &blob[30..30 + name_len], "{table}");
        let digest: String = Sha256::digest(&blob)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(&digest, sha256, "{table}");
    }

    // Acceptance 5: rows read back by _src_page / _src_slot decode to
    // the same values -- every row of three tables, 3,210 in all.
    let db = MdfDatabase::from_bytes(mdf.to_vec()).unwrap();
    let tables = db.user_tables().unwrap();
    let mut compared = 0usize;
    for (schema, table_name, store_name) in [
        ("TEST02pid", "T_Drawing", "pid__T_Drawing"),
        ("TEST02pid", "T_PlantItem", "pid__T_PlantItem"),
        ("TEST02pidd", "codelists", "pidd__codelists"),
    ] {
        let table = tables
            .iter()
            .find(|t| t.schema_name == schema && t.name == table_name)
            .unwrap();
        let column_list = table
            .columns
            .iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {column_list} FROM \"{store_name}\" WHERE _src_page = ?1 AND _src_slot = ?2"
            ))
            .unwrap();
        for record in db.scan_table(table).unwrap() {
            let ScannedRecord::Live {
                page_id, slot, row, ..
            } = record.unwrap()
            else {
                continue;
            };
            let stored: Vec<SqlValue> = stmt
                .query_row([i64::from(page_id), i64::from(slot)], |r| {
                    (0..table.columns.len()).map(|i| r.get(i)).collect()
                })
                .unwrap_or_else(|err| panic!("{store_name} {page_id}:{slot}: {err}"));
            let decoded: Vec<SqlValue> = table
                .columns
                .iter()
                .map(|c| row.value(&c.name).map_or(SqlValue::Null, store_value))
                .collect();
            assert_eq!(decoded, stored, "{store_name} {page_id}:{slot}");
            compared += 1;
        }
    }
    assert_eq!(1 + 3 + 3_206, compared);
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

    // Acceptance 9: every table, dumped ones included, the same row for row.
    let tables: Vec<String> = {
        let mut stmt = on_disk
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert!(tables.len() > 154 + 10, "{}", tables.len());
    for table in &tables {
        assert_eq!(
            table_rows(&on_disk, table),
            table_rows(&again, table),
            "{table}"
        );
    }

    drop(on_disk);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every row of `table` as text, sorted, for comparing two stores.
fn table_rows(conn: &Connection, table: &str) -> Vec<String> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
    let width = stmt.column_count();
    let mut rows: Vec<String> = stmt
        .query_map([], |row| {
            let mut text = String::new();
            for i in 0..width {
                let value: SqlValue = row.get(i)?;
                text.push_str(&format!("{value:?}|"));
            }
            Ok(text)
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows.sort();
    rows
}
