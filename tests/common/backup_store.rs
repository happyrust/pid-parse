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
