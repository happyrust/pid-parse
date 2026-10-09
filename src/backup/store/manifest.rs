//! `Manifest.txt` in the store, line by line (Q14, Q6): every line
//! as written -- its text and its terminator -- in `manifest_line`,
//! every `<<|>>`-separated field in `manifest_field`, what each
//! field is known to mean in `manifest_field_meaning`, and views
//! over the fields whose meaning is `decoded`. Joining
//! `raw_text || terminator` over `line_no` and encoding the result
//! as `store_info.manifest_encoding` says (with the BOM when
//! `manifest_bom` is `1`) gives the file back byte for byte -- after
//! the default redaction, with the replacements `store_redaction`
//! lists (P9).

use rusqlite::{params, Connection};

use crate::backup::manifest::FIELD_SEP;

use super::input::sha256_hex;
use super::redact::redact_field;
use super::{BackupStoreError, StoreOptions, StoreSummary};

/// The file the Manifest is, at the top of every Plant Backup.
pub const MANIFEST_FILE_NAME: &str = "Manifest.txt";

/// The tables and views S2b adds.
pub(super) const SCHEMA: &str = "
CREATE TABLE manifest_line (
    line_no    INTEGER PRIMARY KEY,
    key        TEXT NOT NULL,
    raw_text   TEXT NOT NULL,
    terminator TEXT NOT NULL
);

CREATE TABLE manifest_field (
    line_no  INTEGER NOT NULL REFERENCES manifest_line(line_no),
    position INTEGER NOT NULL,
    value    TEXT    NOT NULL,
    PRIMARY KEY (line_no, position)
) WITHOUT ROWID;

CREATE TABLE manifest_field_meaning (
    key      TEXT    NOT NULL,
    position INTEGER NOT NULL,
    name     TEXT,
    evidence TEXT    NOT NULL,
    PRIMARY KEY (key, position)
) WITHOUT ROWID;

CREATE TABLE store_redaction (
    line_no         INTEGER NOT NULL REFERENCES manifest_line(line_no),
    position        INTEGER NOT NULL,
    rule            TEXT    NOT NULL,
    original_sha256 TEXT    NOT NULL,
    PRIMARY KEY (line_no, position)
) WITHOUT ROWID;
";

/// Evidence levels of `manifest_field_meaning.evidence`, the words of
/// `CONTEXT.md`.
pub mod evidence {
    /// Field boundaries and meaning supported by repeatable evidence.
    pub const DECODED: &str = "decoded";
    /// A proven position whose meaning is kept conservative: a flag
    /// that is always `1`, a `0` whose role is not known.
    pub const TYPED_AUDIT: &str = "typed_audit";
    /// A recognised field without a decoder: an encrypted or obfuscated string.
    pub const IDENTIFIED_ONLY: &str = "identified_only";
    /// No classification: kept as is.
    pub const UNKNOWN: &str = "unknown";
}

/// What each field of each Manifest key is known to mean, from the
/// format write-up `docs/analysis/2026-10-08-sppid-backup-package-format-cn.md`
/// sections 4 and 10: `(key, position, name, evidence)`; `None` for
/// the name means the meaning is not confirmed (P5).
pub const FIELD_MEANINGS: &[(&str, usize, Option<&str>, &str)] = &[
    ("BackupType", 1, Some("backup_type"), evidence::DECODED),
    ("Version", 1, Some("version"), evidence::DECODED),
    ("DateCreated", 1, Some("date_created"), evidence::DECODED),
    ("Name", 1, Some("plant_name"), evidence::DECODED),
    ("Spid", 1, Some("plant_uid"), evidence::DECODED),
    (
        "BackupRefData",
        1,
        Some("backup_ref_data"),
        evidence::DECODED,
    ),
    ("ProjectType", 1, Some("project_type"), evidence::DECODED),
    ("Serial_ID", 1, Some("serial_id"), evidence::DECODED),
    ("Rootitem", 1, Some("uid"), evidence::DECODED),
    ("Rootitem", 2, Some("name"), evidence::DECODED),
    ("Rootitem", 3, Some("description"), evidence::DECODED),
    ("Rootitem", 4, Some("unc_path"), evidence::DECODED),
    ("Rootitem", 5, Some("kind"), evidence::DECODED),
    ("Rootitem", 6, None, evidence::TYPED_AUDIT),
    ("Rootitem", 7, None, evidence::TYPED_AUDIT),
    ("Rootitem", 8, None, evidence::UNKNOWN),
    ("Rootitem", 9, None, evidence::UNKNOWN),
    ("Rootitem", 10, Some("created"), evidence::DECODED),
    (
        "PidIsAssociated",
        1,
        Some("pid_is_associated"),
        evidence::DECODED,
    ),
    (
        "SpelIsAssociated",
        1,
        Some("spel_is_associated"),
        evidence::DECODED,
    ),
    (
        "SPIIsAssociated",
        1,
        Some("spi_is_associated"),
        evidence::DECODED,
    ),
    ("DbaExport", 1, Some("dba_export"), evidence::DECODED),
    ("SiteConnInfo", 1, None, evidence::IDENTIFIED_ONLY),
    (
        "SiteConnInfo",
        2,
        Some("schema_type_code"),
        evidence::DECODED,
    ),
    ("SiteConnInfo", 3, Some("server"), evidence::DECODED),
    ("SiteConnInfo", 4, Some("schema_name"), evidence::DECODED),
    ("SiteConnInfo", 5, None, evidence::IDENTIFIED_ONLY),
    ("SiteConnInfo", 6, Some("login"), evidence::DECODED),
    ("SiteConnInfo", 7, None, evidence::UNKNOWN),
    ("SiteConnInfo", 8, Some("database_type"), evidence::DECODED),
    (
        "SiteConnInfo",
        9,
        Some("sql_server_database"),
        evidence::DECODED,
    ),
    (
        "SiteConnInfo",
        10,
        Some("oracle_default_tablespace"),
        evidence::DECODED,
    ),
    (
        "SiteConnInfo",
        11,
        Some("oracle_temp_tablespace"),
        evidence::DECODED,
    ),
    ("SiteConnInfo", 12, Some("dump_file"), evidence::DECODED),
    ("SiteConnInfo", 13, Some("log_file"), evidence::DECODED),
    ("PlantConnInfo", 1, None, evidence::IDENTIFIED_ONLY),
    (
        "PlantConnInfo",
        2,
        Some("schema_type_code"),
        evidence::DECODED,
    ),
    ("PlantConnInfo", 3, Some("server"), evidence::DECODED),
    ("PlantConnInfo", 4, Some("schema_name"), evidence::DECODED),
    ("PlantConnInfo", 5, None, evidence::IDENTIFIED_ONLY),
    ("PlantConnInfo", 6, Some("login"), evidence::DECODED),
    ("PlantConnInfo", 7, None, evidence::UNKNOWN),
    ("PlantConnInfo", 8, Some("database_type"), evidence::DECODED),
    (
        "PlantConnInfo",
        9,
        Some("sql_server_database"),
        evidence::DECODED,
    ),
    (
        "PlantConnInfo",
        10,
        Some("oracle_default_tablespace"),
        evidence::DECODED,
    ),
    (
        "PlantConnInfo",
        11,
        Some("oracle_temp_tablespace"),
        evidence::DECODED,
    ),
    ("PlantConnInfo", 12, Some("dump_file"), evidence::DECODED),
    ("PlantConnInfo", 13, Some("log_file"), evidence::DECODED),
    ("DatabaseFiles", 1, Some("logical_name"), evidence::DECODED),
    ("DatabaseFiles", 2, Some("file_number"), evidence::DECODED),
    ("DatabaseFiles", 3, Some("physical_path"), evidence::DECODED),
    ("DatabaseFiles", 4, Some("filegroup"), evidence::DECODED),
    ("DatabaseFiles", 5, Some("size"), evidence::DECODED),
    ("DatabaseFiles", 6, Some("max_size"), evidence::DECODED),
    ("DatabaseFiles", 7, Some("growth"), evidence::DECODED),
    ("DatabaseFiles", 8, Some("content"), evidence::DECODED),
    ("Role", 1, Some("role_uid"), evidence::DECODED),
    ("Role", 2, None, evidence::TYPED_AUDIT),
    ("Role", 3, Some("windows_group"), evidence::DECODED),
    ("Role", 4, None, evidence::UNKNOWN),
    ("Role", 5, None, evidence::UNKNOWN),
    ("Right", 1, Some("role_uid"), evidence::DECODED),
    ("Right", 2, None, evidence::TYPED_AUDIT),
    ("Right", 3, None, evidence::TYPED_AUDIT),
    ("Right", 4, Some("right_id"), evidence::DECODED),
    (
        "Characterset",
        1,
        Some("oracle_characterset"),
        evidence::DECODED,
    ),
    (
        "OracleVersion",
        1,
        Some("oracle_version"),
        evidence::DECODED,
    ),
    ("TableSpace", 1, Some("tablespace_kind"), evidence::DECODED),
    ("TableSpace", 2, None, evidence::IDENTIFIED_ONLY),
    (
        "ExportFileSize",
        1,
        Some("export_file_size"),
        evidence::DECODED,
    ),
    (
        "ArchiveFileSize",
        1,
        Some("archive_file_size"),
        evidence::DECODED,
    ),
    ("SlotCount", 1, None, evidence::UNKNOWN),
    ("SlotCount", 2, None, evidence::UNKNOWN),
    ("DBUids", 1, None, evidence::IDENTIFIED_ONLY),
    ("DBPwds", 1, None, evidence::IDENTIFIED_ONLY),
    ("Table", 1, Some("schema_name"), evidence::DECODED),
    ("Table", 2, Some("table_name"), evidence::DECODED),
    ("View", 1, Some("schema_name"), evidence::DECODED),
    ("View", 2, Some("view_name"), evidence::DECODED),
    (
        "BackupCommand",
        1,
        Some("backup_command"),
        evidence::DECODED,
    ),
    ("File", 1, Some("schema_code"), evidence::DECODED),
    ("File", 2, Some("source_kind"), evidence::DECODED),
    ("File", 3, Some("option_id"), evidence::DECODED),
    ("File", 4, None, evidence::TYPED_AUDIT),
    ("File", 5, Some("source_path"), evidence::DECODED),
    ("File", 6, Some("status"), evidence::DECODED),
    ("File", 7, Some("archive_name_or_error"), evidence::DECODED),
    ("FileSize", 1, Some("schema_code"), evidence::DECODED),
    ("FileSize", 2, Some("option_id"), evidence::DECODED),
    ("FileSize", 3, Some("file_count"), evidence::DECODED),
    ("FileSize", 4, Some("directory_count"), evidence::DECODED),
    ("FileSize", 5, Some("total_bytes"), evidence::DECODED),
];

/// The views over decoded fields: `(view name, keys it covers)`. A
/// view has one row per line of those keys and one column per decoded
/// position, named as [`FIELD_MEANINGS`] says; `manifest_conn_info`
/// adds `scope` (`Site` / `Plant`) and `manifest_value` lists every
/// one-field key by `key` and `value`.
pub const VIEWS: &[(&str, &[&str])] = &[
    ("manifest_root_item", &["Rootitem"]),
    ("manifest_conn_info", &["SiteConnInfo", "PlantConnInfo"]),
    ("manifest_database_file", &["DatabaseFiles"]),
    ("manifest_role", &["Role"]),
    ("manifest_right", &["Right"]),
    ("manifest_table_entry", &["Table"]),
    ("manifest_view_entry", &["View"]),
    ("manifest_file", &["File"]),
    ("manifest_file_size", &["FileSize"]),
    ("manifest_table_space", &["TableSpace"]),
];

/// The keys `manifest_value` lists: every key with exactly one
/// decoded field at position 1 and no other position.
fn single_value_keys() -> Vec<&'static str> {
    let mut keys: Vec<&str> = Vec::new();
    for (key, position, name, level) in FIELD_MEANINGS {
        let only_position = FIELD_MEANINGS
            .iter()
            .all(|(other, other_position, _, _)| other != key || other_position == position);
        if *position == 1 && name.is_some() && *level == evidence::DECODED && only_position {
            keys.push(key);
        }
    }
    keys
}

/// How the Manifest's bytes read as text, as `store_info` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestEncoding {
    /// UTF-16 little-endian, the encoding `SmartPlant` writes.
    Utf16Le,
    /// UTF-16 big-endian.
    Utf16Be,
    /// UTF-8, with or without a BOM.
    Utf8,
}

impl ManifestEncoding {
    /// The label `store_info.manifest_encoding` holds.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::Utf8 => "utf-8",
        }
    }

    /// The encoding a label names, if any.
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "utf-16le" => Some(Self::Utf16Le),
            "utf-16be" => Some(Self::Utf16Be),
            "utf-8" => Some(Self::Utf8),
            _ => None,
        }
    }

    /// The byte order mark of the encoding.
    pub fn bom(self) -> &'static [u8] {
        match self {
            Self::Utf16Le => &[0xFF, 0xFE],
            Self::Utf16Be => &[0xFE, 0xFF],
            Self::Utf8 => &[0xEF, 0xBB, 0xBF],
        }
    }

    /// `text` in this encoding.
    pub fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf16Le => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Self::Utf16Be => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            Self::Utf8 => text.as_bytes().to_vec(),
        }
    }
}

/// The Manifest's bytes as text: the encoding its BOM names (UTF-8
/// without one), whether a BOM was there, the text, and whether any
/// byte sequence had to be replaced.
pub fn decode_manifest(bytes: &[u8]) -> (ManifestEncoding, bool, String, bool) {
    let (encoding, bom) = if bytes.starts_with(ManifestEncoding::Utf16Le.bom()) {
        (ManifestEncoding::Utf16Le, true)
    } else if bytes.starts_with(ManifestEncoding::Utf16Be.bom()) {
        (ManifestEncoding::Utf16Be, true)
    } else if bytes.starts_with(ManifestEncoding::Utf8.bom()) {
        (ManifestEncoding::Utf8, true)
    } else {
        (ManifestEncoding::Utf8, false)
    };
    let body = if bom {
        &bytes[encoding.bom().len()..]
    } else {
        bytes
    };
    let codec = match encoding {
        ManifestEncoding::Utf16Le => encoding_rs::UTF_16LE,
        ManifestEncoding::Utf16Be => encoding_rs::UTF_16BE,
        ManifestEncoding::Utf8 => encoding_rs::UTF_8,
    };
    let (text, had_errors) = codec.decode_without_bom_handling(body);
    (encoding, bom, text.into_owned(), had_errors)
}

/// `text` cut into lines, each with the terminator that ended it:
/// `\r\n`, `\n`, a lone `\r`, or nothing for a last line the file
/// does not end. Joining `line || terminator` gives `text` back.
pub fn split_lines(text: &str) -> Vec<(&str, &str)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                lines.push((&text[start..i], &text[i..i + 2]));
                i += 2;
                start = i;
            }
            b'\r' | b'\n' => {
                lines.push((&text[start..i], &text[i..=i]));
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < text.len() {
        lines.push((&text[start..], ""));
    }
    lines
}

/// Writes the Manifest tables, meanings and views; `summary` learns
/// the line and redaction counts.
pub(super) fn write_manifest(
    conn: &Connection,
    bytes: &[u8],
    options: &StoreOptions,
    summary: &mut StoreSummary,
) -> Result<(), BackupStoreError> {
    let (encoding, bom, text, had_errors) = decode_manifest(bytes);
    if had_errors {
        summary.warnings.push(format!(
            "{MANIFEST_FILE_NAME}: not every byte sequence decodes as {}; the replaced ones \
             will not come back on reassembly",
            encoding.as_str()
        ));
    }
    let mut info = conn.prepare("INSERT INTO store_info (key, value) VALUES (?1, ?2)")?;
    info.execute(params!["manifest_encoding", encoding.as_str()])?;
    info.execute(params!["manifest_bom", if bom { "1" } else { "0" }])?;

    let mut insert_line = conn.prepare(
        "INSERT INTO manifest_line (line_no, key, raw_text, terminator) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut insert_field =
        conn.prepare("INSERT INTO manifest_field (line_no, position, value) VALUES (?1, ?2, ?3)")?;
    let mut insert_redaction = conn.prepare(
        "INSERT INTO store_redaction (line_no, position, rule, original_sha256) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for (index, (line, terminator)) in split_lines(&text).into_iter().enumerate() {
        let line_no = i64::try_from(index + 1).unwrap_or(i64::MAX);
        let mut parts: Vec<String> = line.split(FIELD_SEP).map(str::to_string).collect();
        let key = parts[0].trim().to_string();
        let mut redactions = Vec::new();
        if !options.keep_secrets {
            for (position, part) in parts.iter_mut().enumerate().skip(1) {
                if let Some((replacement, rule)) = redact_field(&key, position, part) {
                    redactions.push((position, rule, sha256_hex(part.as_bytes())));
                    *part = replacement;
                }
            }
        }
        insert_line.execute(params![line_no, key, parts.join(FIELD_SEP), terminator])?;
        for (position, value) in parts.iter().enumerate().skip(1) {
            insert_field.execute(params![line_no, position as i64, value])?;
        }
        for (position, rule, original_sha256) in redactions {
            insert_redaction.execute(params![
                line_no,
                position as i64,
                rule.as_str(),
                original_sha256
            ])?;
            summary.redactions += 1;
        }
        summary.manifest_lines += 1;
    }

    let mut insert_meaning = conn.prepare(
        "INSERT INTO manifest_field_meaning (key, position, name, evidence) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (key, position, name, level) in FIELD_MEANINGS {
        insert_meaning.execute(params![key, *position as i64, name, level])?;
    }
    conn.execute_batch(&view_sql())?;
    Ok(())
}

/// The `CREATE VIEW` statements for [`VIEWS`] and `manifest_value`.
pub fn view_sql() -> String {
    let mut sql = String::new();
    for (view, keys) in VIEWS {
        let mut columns = vec!["line.line_no".to_string()];
        if keys.len() > 1 {
            columns.push("substr(line.key, 1, length(line.key) - 8) AS scope".to_string());
        }
        let mut joins = String::new();
        for (key, position, name, level) in FIELD_MEANINGS {
            if key != &keys[0] || *level != evidence::DECODED {
                continue;
            }
            let Some(name) = name else {
                continue;
            };
            columns.push(format!("f{position}.value AS {name}"));
            joins.push_str(&format!(
                " LEFT JOIN manifest_field f{position} ON f{position}.line_no = line.line_no \
                 AND f{position}.position = {position}"
            ));
        }
        let keys_list = keys
            .iter()
            .map(|key| format!("'{key}'"))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(
            "CREATE VIEW {view} AS SELECT {} FROM manifest_line line{joins} \
             WHERE line.key IN ({keys_list});\n",
            columns.join(", ")
        ));
    }
    let singles = single_value_keys()
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(", ");
    sql.push_str(&format!(
        "CREATE VIEW manifest_value AS SELECT line.line_no, line.key, f1.value \
         FROM manifest_line line JOIN manifest_field f1 ON f1.line_no = line.line_no \
         AND f1.position = 1 WHERE line.key IN ({singles});\n"
    ));
    sql
}

/// The Manifest's bytes rebuilt from a store: BOM (if recorded), then
/// every line's `raw_text || terminator` in `line_no` order, in the
/// recorded encoding. Byte for byte the input when the store kept its
/// secrets; otherwise the input with `store_redaction`'s replacements.
pub fn reassemble_manifest(conn: &Connection) -> Result<Vec<u8>, BackupStoreError> {
    let encoding: String = conn.query_row(
        "SELECT value FROM store_info WHERE key = 'manifest_encoding'",
        [],
        |row| row.get(0),
    )?;
    let bom: String = conn.query_row(
        "SELECT value FROM store_info WHERE key = 'manifest_bom'",
        [],
        |row| row.get(0),
    )?;
    let encoding = ManifestEncoding::parse(&encoding).ok_or(BackupStoreError::StoreShape {
        what: "store_info.manifest_encoding names no known encoding",
    })?;
    let mut text = String::new();
    let mut lines =
        conn.prepare("SELECT raw_text, terminator FROM manifest_line ORDER BY line_no")?;
    for line in lines.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (raw_text, terminator) = line?;
        text.push_str(&raw_text);
        text.push_str(&terminator);
    }
    let mut bytes = Vec::new();
    if bom == "1" {
        bytes.extend_from_slice(encoding.bom());
    }
    bytes.extend(encoding.encode(&text));
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_keep_their_terminators_and_join_back() {
        let text = "a<<|>>1\r\nb\n\rc";
        let lines = split_lines(text);
        assert_eq!(
            vec![("a<<|>>1", "\r\n"), ("b", "\n"), ("", "\r"), ("c", "")],
            lines
        );
        let joined: String = lines
            .iter()
            .map(|(line, terminator)| format!("{line}{terminator}"))
            .collect();
        assert_eq!(text, joined);
        assert!(split_lines("").is_empty());
        assert_eq!(vec![("x", "\r\n")], split_lines("x\r\n"));
    }

    #[test]
    fn the_bom_names_the_encoding_and_utf16le_round_trips() {
        let text = "Name<<|>>TEST02\r\n描述<<|>>沁水\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(ManifestEncoding::Utf16Le.encode(text));
        let (encoding, bom, decoded, had_errors) = decode_manifest(&bytes);
        assert_eq!(
            (ManifestEncoding::Utf16Le, true, false),
            (encoding, bom, had_errors)
        );
        assert_eq!(text, decoded);
        let mut again = encoding.bom().to_vec();
        again.extend(encoding.encode(&decoded));
        assert_eq!(bytes, again);

        let (encoding, bom, decoded, _) = decode_manifest(b"Name<<|>>X\n");
        assert_eq!((ManifestEncoding::Utf8, false), (encoding, bom));
        assert_eq!("Name<<|>>X\n", decoded);
    }

    #[test]
    fn every_meaning_row_is_unique_and_named_only_when_decoded() {
        let mut seen = std::collections::BTreeSet::new();
        for (key, position, name, level) in FIELD_MEANINGS {
            assert!(
                seen.insert((*key, *position)),
                "{key} {position} listed twice"
            );
            assert!(
                matches!(
                    *level,
                    evidence::DECODED
                        | evidence::TYPED_AUDIT
                        | evidence::IDENTIFIED_ONLY
                        | evidence::UNKNOWN
                ),
                "{key} {position}: {level}"
            );
            assert_eq!(
                name.is_some(),
                *level == evidence::DECODED,
                "{key} {position}: a name goes with decoded evidence only"
            );
        }
        assert!(single_value_keys().contains(&"Name"));
        assert!(!single_value_keys().contains(&"DBPwds"));
        assert!(!single_value_keys().contains(&"Rootitem"));
    }

    #[test]
    fn the_views_build_on_an_empty_store() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(&view_sql()).unwrap();
        let views: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'view'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(VIEWS.len() as i64 + 1, views);
    }
}
