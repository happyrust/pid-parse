//! Shared checks for the Backup Store tests (`tests/backup_store_*.rs`):
//! what every sample's store must satisfy, with the sample's own
//! numbers passed in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::backup::manifest::FIELD_SEP;
use pid_parse::backup::store::{
    build_backup_store, build_backup_store_in_memory, manifest::decode_manifest,
    manifest::split_lines, reassemble_manifest, redact_field, BackupInput, RedactionRule,
    StoreOptions, MANIFEST_FILE_NAME, MASK,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// What a sample's Manifest looks like in its store.
pub struct ManifestExpectation {
    /// Lines of `Manifest.txt`.
    pub lines: usize,
    /// Fields the default store replaces, and how many of those are a
    /// masked password (the rest are SHA-256 replacements).
    pub redactions: usize,
    pub passwords_masked: usize,
    /// Rows of the views.
    pub tables: usize,
    pub views: usize,
    pub conn_infos: usize,
    pub files: usize,
    pub roles: usize,
    pub rights: usize,
    /// `Name` as `manifest_value` shows it.
    pub plant_name: &'static str,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn count(conn: &Connection, sql: &str) -> usize {
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|err| panic!("{sql}: {err}")) as usize
}

fn store_info(conn: &Connection, key: &str) -> String {
    conn.query_row(
        "SELECT value FROM store_info WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .unwrap_or_else(|err| panic!("store_info.{key}: {err}"))
}

/// The original `Manifest.txt` bytes of the backup at `input`.
pub fn original_manifest(input: &Path) -> Vec<u8> {
    let mut backup = BackupInput::open(input).expect("open the backup");
    let index = backup
        .files()
        .iter()
        .find(|file| file.name == MANIFEST_FILE_NAME)
        .map(|file| file.entry_index)
        .expect("Manifest.txt among the outer files");
    backup.read(index).expect("read Manifest.txt")
}

/// A directory under the temp dir for one test's store files.
pub fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pid_parse_backup_store_{label}_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Builds the backup's store twice -- keeping secrets, in memory, and
/// with the default redaction, as a file -- and checks the Manifest
/// tables against `expect` and against the original `Manifest.txt`
/// (P9 both ways, Q15's "no original value left in the file").
pub fn check_manifest(input: &Path, label: &str, expect: &ManifestExpectation) {
    let original = original_manifest(input);
    let (encoding, bom, original_text, had_errors) = decode_manifest(&original);
    assert!(!had_errors, "{label}: the Manifest decodes cleanly");
    assert!(bom, "{label}: the Manifest carries a BOM");

    // Secrets kept: the file comes back byte for byte.
    let keep = StoreOptions {
        keep_secrets: true,
        ..StoreOptions::default()
    };
    let (kept, kept_summary) = build_backup_store_in_memory(input, &keep).expect("build (kept)");
    assert_eq!("0", store_info(&kept, "redacted"));
    assert_eq!(0, kept_summary.redactions);
    assert_eq!(0, count(&kept, "SELECT count(*) FROM store_redaction"));
    assert_eq!(
        original,
        reassemble_manifest(&kept).expect("reassemble"),
        "{label}: the kept store gives the Manifest back byte for byte"
    );

    // Redacted by default: the same file with the listed replacements.
    let dir = scratch_dir(label);
    let out = dir.join("store.sqlite");
    let summary = build_backup_store(input, &out, &StoreOptions::default()).expect("build");
    let conn = Connection::open(&out).expect("open the store");
    assert_eq!("1", store_info(&conn, "redacted"));
    assert_eq!(encoding.as_str(), store_info(&conn, "manifest_encoding"));
    assert_eq!("1", store_info(&conn, "manifest_bom"));
    assert_eq!(expect.lines, summary.manifest_lines, "{label}: lines");
    assert_eq!(
        expect.lines,
        count(&conn, "SELECT count(*) FROM manifest_line")
    );
    assert_eq!(
        expect.lines,
        count(
            &conn,
            "SELECT count(*) FROM manifest_line WHERE terminator = char(13, 10)"
        ),
        "{label}: every line ends in CRLF"
    );
    assert_eq!(expect.redactions, summary.redactions, "{label}: redactions");

    let original_lines = split_lines(&original_text);
    assert_eq!(expect.lines, original_lines.len());
    let mut rules: BTreeMap<String, usize> = BTreeMap::new();
    let mut secrets: Vec<String> = Vec::new();
    let mut replaced_lines: Vec<String> = original_lines
        .iter()
        .map(|(line, _)| (*line).to_string())
        .collect();
    let redaction_rows: Vec<(usize, usize, String, String)> = {
        let mut stmt = conn
            .prepare("SELECT line_no, position, rule, original_sha256 FROM store_redaction")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        rows
    };
    for (line_no, position, rule, original_sha256) in redaction_rows {
        *rules.entry(rule.clone()).or_default() += 1;
        let mut parts: Vec<String> = replaced_lines[line_no - 1]
            .split(FIELD_SEP)
            .map(str::to_string)
            .collect();
        let key = parts[0].trim().to_string();
        let original_value = parts[position].clone();
        assert_eq!(
            sha256_hex(original_value.as_bytes()),
            original_sha256,
            "{label}: line {line_no} field {position}: the SHA-256 of the original value"
        );
        let (replacement, expected_rule) =
            redact_field(&key, position, &original_value).expect("a redacted field");
        assert_eq!(
            expected_rule.as_str(),
            rule,
            "{label}: line {line_no} field {position}"
        );
        if expected_rule == RedactionRule::PasswordMasked {
            // The password: what the mask replaced.
            let masked_at = replacement.find(MASK).expect("the mask in the replacement");
            let tail = replacement.len() - masked_at - MASK.len();
            let password = &original_value[masked_at..original_value.len() - tail];
            assert!(!password.is_empty() && !password.contains(char::is_whitespace));
            secrets.push(password.to_string());
        } else if key == "PlantConnInfo" && position == 1 {
            // In every sample so far this field is the plant's name, which
            // the Name and Table lines carry anyway; Q15 replaces it all the
            // same, but it cannot be absent from the file.
            assert_eq!(expect.plant_name, original_value, "{label}: line {line_no}");
        } else if key == "DBUids" {
            // The database user ids: the four Plant schema names, comma
            // separated -- the OWNER list of an Oracle exp command line
            // repeats them. Replaced by Q15, but not a secret.
            let mut names: Vec<&str> = original_value.split(',').collect();
            names.sort_unstable();
            let mut schemas: Vec<&str> = original_lines
                .iter()
                .filter(|(line, _)| line.starts_with("PlantConnInfo<<|>>"))
                .map(|(line, _)| line.split(FIELD_SEP).nth(4).expect("field 4"))
                .collect();
            schemas.sort_unstable();
            assert_eq!(
                schemas, names,
                "{label}: DBUids lists the Plant schema names"
            );
        } else {
            secrets.push(original_value.clone());
        }
        parts[position] = replacement;
        replaced_lines[line_no - 1] = parts.join(FIELD_SEP);
    }
    assert_eq!(
        expect.redactions - expect.passwords_masked,
        rules.get("sha256").copied().unwrap_or(0),
        "{label}: {rules:?}"
    );
    assert_eq!(
        expect.passwords_masked,
        rules.get("password-masked").copied().unwrap_or(0),
        "{label}: {rules:?}"
    );
    assert_eq!(None, rules.get("value-masked"), "{label}: {rules:?}");

    let mut expected_text = String::new();
    for (line, (_, terminator)) in replaced_lines.iter().zip(&original_lines) {
        expected_text.push_str(line);
        expected_text.push_str(terminator);
    }
    let mut expected = encoding.bom().to_vec();
    expected.extend(encoding.encode(&expected_text));
    assert_eq!(
        expected,
        reassemble_manifest(&conn).expect("reassemble"),
        "{label}: the default store gives the Manifest back with the listed replacements"
    );

    // No original value, as UTF-8 or UTF-16LE, anywhere in the store file.
    drop(conn);
    let file = std::fs::read(&out).expect("read the store file");
    for secret in &secrets {
        let utf8 = secret.as_bytes();
        let utf16: Vec<u8> = secret
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        assert!(
            !contains(&file, utf8) && !contains(&file, &utf16),
            "{label}: an original value of {} chars is still in the store file",
            secret.chars().count()
        );
    }

    // The views and the meaning table.
    let conn = Connection::open(&out).expect("open the store");
    assert_eq!(
        expect.tables,
        count(&conn, "SELECT count(*) FROM manifest_table_entry")
    );
    assert_eq!(
        expect.views,
        count(&conn, "SELECT count(*) FROM manifest_view_entry")
    );
    assert_eq!(
        expect.conn_infos,
        count(&conn, "SELECT count(*) FROM manifest_conn_info")
    );
    assert_eq!(
        expect.files,
        count(&conn, "SELECT count(*) FROM manifest_file")
    );
    assert_eq!(
        expect.roles,
        count(&conn, "SELECT count(*) FROM manifest_role")
    );
    assert_eq!(
        expect.rights,
        count(&conn, "SELECT count(*) FROM manifest_right")
    );
    assert_eq!(
        vec![
            ("Plant".to_string(), "2".to_string()),
            ("Plant".to_string(), "4".to_string()),
            ("Plant".to_string(), "8".to_string()),
            ("Plant".to_string(), "9".to_string()),
            ("Site".to_string(), "1".to_string()),
            ("Site".to_string(), "7".to_string()),
        ],
        {
            let mut stmt = conn
                .prepare(
                    "SELECT scope, schema_type_code FROM manifest_conn_info \
                     ORDER BY scope, schema_type_code",
                )
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<Vec<(String, String)>, _>>()
                .unwrap()
        },
        "{label}: the six connections and their schema type codes"
    );
    let plant_name: String = conn
        .query_row(
            "SELECT value FROM manifest_value WHERE key = 'Name'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(expect.plant_name, plant_name);
    assert_eq!(
        0,
        count(
            &conn,
            "SELECT count(*) FROM manifest_field f JOIN manifest_line l ON l.line_no = f.line_no \
             LEFT JOIN manifest_field_meaning m ON m.key = l.key AND m.position = f.position \
             WHERE m.key IS NULL"
        ),
        "{label}: every field of every line has a meaning row"
    );
    assert_eq!(
        0,
        count(
            &conn,
            "SELECT count(*) FROM manifest_field_meaning \
             WHERE (name IS NULL) <> (evidence <> 'decoded')"
        ),
        "{label}: names go with decoded evidence only"
    );

    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// What an Oracle sample's dump looks like in its store (S2d): the
/// four `CONNECT` owners by role, columns per role, and columns by
/// Oracle type name over the 154 tables.
pub struct OracleDumpExpectation {
    /// The dump's upper-case owner of each role: plant, plantd, pid, pidd.
    pub owners: [&'static str; 4],
    /// Columns of each role's tables, in the same order.
    pub columns_per_role: [usize; 4],
    /// Columns by Oracle type name (`NUMBER`, `NVARCHAR2`, ...).
    pub columns_by_type: &'static [(&'static str, usize)],
    /// Columns declared `NOT NULL`.
    pub not_null_columns: usize,
}

/// The table counts every Oracle sample shares: 154 tables over the
/// four roles (22 / 25 / 82 / 25), 126 distinct names, 35 views.
const ORACLE_TABLES_PER_ROLE: [(&str, usize); 4] =
    [("plant", 22), ("plantd", 25), ("pid", 82), ("pidd", 25)];
const ORACLE_DISTINCT_TABLE_NAMES: usize = 126;

/// Checks the catalogue and the empty tables of an Oracle dump's store
/// (acceptance 11 and the dump part of 13).
pub fn check_oracle_dump(conn: &Connection, label: &str, expect: &OracleDumpExpectation) {
    assert_eq!("oracle-exp", store_info(conn, "dump_kind"), "{label}");

    // dump_schema: the dump's own spelling, type 2 (Oracle), from the Manifest.
    let schemas: Vec<(String, String, String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT role, schema_name, type_code, db_type, role_source FROM dump_schema \
                 ORDER BY CASE role WHEN 'plant' THEN 0 WHEN 'plantd' THEN 1 \
                 WHEN 'pid' THEN 2 ELSE 3 END",
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
    let roles = ["plant", "plantd", "pid", "pidd"];
    let codes = ["2", "8", "4", "9"];
    assert_eq!(
        (0..4)
            .map(|i| (
                roles[i].to_string(),
                expect.owners[i].to_string(),
                codes[i].to_string(),
                "2".to_string(),
                "manifest-conninfo".to_string(),
            ))
            .collect::<Vec<_>>(),
        schemas,
        "{label}: dump_schema"
    );

    // dump_table: 154 empty, undecoded tables, 126 distinct names.
    let per_role: BTreeMap<String, usize> = {
        let mut stmt = conn
            .prepare("SELECT role, count(*) FROM dump_table GROUP BY role")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        ORACLE_TABLES_PER_ROLE
            .iter()
            .map(|(role, n)| ((*role).to_string(), *n))
            .collect::<BTreeMap<_, _>>(),
        per_role,
        "{label}: tables per role"
    );
    assert_eq!(
        0,
        count(
            conn,
            "SELECT count(*) FROM dump_table WHERE decoded <> 0 OR expected_rows IS NOT NULL \
             OR rows <> 0 OR ghost_rows <> 0"
        ),
        "{label}: every table undecoded and empty"
    );
    assert_eq!(
        ORACLE_DISTINCT_TABLE_NAMES,
        count(conn, "SELECT count(DISTINCT source_name) FROM dump_table"),
        "{label}: distinct table names"
    );
    assert_eq!(
        vec![("MAX_ID".to_string(), 4), ("SPIDCACHE".to_string(), 2),],
        {
            let mut stmt = conn
                .prepare(
                    "SELECT source_name, count(*) FROM dump_table \
                     WHERE source_name IN ('MAX_ID', 'SPIDCACHE') GROUP BY source_name \
                     ORDER BY source_name",
                )
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)?)))
                .unwrap()
                .collect::<Result<Vec<(String, i64)>, _>>()
                .unwrap()
        },
        "{label}: same-named tables each under their own role"
    );
    assert_eq!(
        0,
        count(conn, "SELECT count(*) FROM dump_ghost_row")
            + count(conn, "SELECT count(*) FROM dump_lob"),
        "{label}: no ghost rows or LOBs without rows"
    );
    assert_eq!(
        35,
        count(conn, "SELECT count(*) FROM dump_view"),
        "{label}: views"
    );
    assert_eq!(
        0,
        count(
            conn,
            "SELECT count(*) FROM dump_view v LEFT JOIN dump_schema s \
             ON s.role = v.role AND s.schema_name = v.schema_name WHERE s.role IS NULL"
        ),
        "{label}: every view under a schema's dump spelling"
    );

    // dump_column: per role and by type; NOT NULL recorded there only.
    let columns_per_role: BTreeMap<String, usize> = {
        let mut stmt = conn
            .prepare("SELECT role, count(*) FROM dump_column GROUP BY role")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        (0..4)
            .map(|i| (roles[i].to_string(), expect.columns_per_role[i]))
            .collect::<BTreeMap<_, _>>(),
        columns_per_role,
        "{label}: columns per role"
    );
    let columns_by_type: BTreeMap<String, usize> = {
        let mut stmt = conn
            .prepare(
                "SELECT CASE WHEN instr(source_type, '(') > 0 \
                 THEN substr(source_type, 1, instr(source_type, '(') - 1) ELSE source_type END, \
                 count(*) FROM dump_column GROUP BY 1",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        expect
            .columns_by_type
            .iter()
            .map(|(t, n)| ((*t).to_string(), *n))
            .collect::<BTreeMap<_, _>>(),
        columns_by_type,
        "{label}: columns by type"
    );
    assert_eq!(
        expect.not_null_columns,
        count(conn, "SELECT count(*) FROM dump_column WHERE nullable = 0"),
        "{label}: NOT NULL columns"
    );
    // Q19 in the created tables: every column's declared SQLite type
    // follows its Oracle type, and the provenance columns come last.
    let tables: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT store_name, source_name FROM dump_table ORDER BY store_name")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let mut checked_columns = 0usize;
    for (store_name, source_name) in &tables {
        assert_eq!(
            0,
            count(conn, &format!("SELECT count(*) FROM \"{store_name}\"")),
            "{label}: {store_name} is empty"
        );
        let declared: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT name, type FROM pragma_table_info('{store_name}') ORDER BY cid"
                ))
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let sources: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT c.name, c.source_type FROM dump_column c JOIN dump_table t \
                     ON t.role = c.role AND t.source_name = c.table_name \
                     WHERE t.store_name = ?1 ORDER BY c.ordinal",
                )
                .unwrap();
            stmt.query_map([store_name], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(sources.len() + 2, declared.len(), "{label}: {store_name}");
        assert_eq!(
            [
                ("_src_page".to_string(), "INTEGER".to_string()),
                ("_src_slot".to_string(), "INTEGER".to_string())
            ],
            declared[declared.len() - 2..],
            "{label}: {store_name}"
        );
        for ((name, source_type), (declared_name, declared_type)) in
            sources.iter().zip(declared.iter())
        {
            assert_eq!(name, declared_name, "{label}: {store_name}");
            let type_name = source_type.split('(').next().unwrap();
            let expected = match type_name {
                "NUMBER" => "INTEGER",
                "FLOAT" => "REAL",
                "BLOB" => "BLOB",
                "NVARCHAR2" | "VARCHAR2" | "DATE" => "TEXT",
                other => panic!("{label}: {store_name}.{name}: unexpected type {other}"),
            };
            assert_eq!(
                expected, declared_type,
                "{label}: {store_name}.{name} {source_type}"
            );
            checked_columns += 1;
        }
        assert!(
            source_name.bytes().all(|b| !b.is_ascii_lowercase()),
            "{label}: {source_name} keeps the dump's upper case (Q20)"
        );
    }
    assert_eq!(
        expect.columns_per_role.iter().sum::<usize>(),
        checked_columns,
        "{label}: every column checked"
    );
    // NUMBER columns are all (p, 0) in the samples and FLOAT all (126).
    assert_eq!(
        0,
        count(
            conn,
            "SELECT count(*) FROM dump_column WHERE source_type LIKE 'NUMBER%' \
             AND (scale <> 0 OR precision NOT IN (10, 11))"
        ),
        "{label}: NUMBER(10, 0) / NUMBER(11, 0) only"
    );
    assert_eq!(
        0,
        count(
            conn,
            "SELECT count(*) FROM dump_column WHERE source_type LIKE 'FLOAT%' AND precision <> 126"
        ),
        "{label}: FLOAT(126) only"
    );
    assert_eq!(
        (Some(32i64), None::<i64>, None::<i64>, 0i64),
        conn.query_row(
            "SELECT length, precision, scale, nullable FROM dump_column \
             WHERE role = 'pid' AND table_name = 'T_DRAWING' AND name = 'SP_ID'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, i64>(3)?)),
        )
        .unwrap(),
        "{label}: T_DRAWING.SP_ID NVARCHAR2(32) NOT NULL"
    );
}
