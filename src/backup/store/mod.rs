//! A Backup Store: one SQLite file holding what one Plant Backup
//! carries -- its file list, its `Manifest.txt`, the tables of its
//! Database Dump -- so the publish pipeline, a person or a script can
//! read the backup without the `SmartPlant` tools.
//!
//! Plan `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`
//! (ADR-0004: one store per backup, dumped tables named by Schema
//! Role). This module is being built up step by step:
//!
//! * S2a: the input ([`input`]) and the file list -- `store_info`,
//!   `backup_file` and, behind [`StoreOptions::embed_files`],
//!   `backup_file_content`.
//! * S2b: `Manifest.txt` line by line ([`manifest`]), with the
//!   default redaction ([`redact`]) unless
//!   [`StoreOptions::keep_secrets`].
//! * S2c: the SQL Server dump, table by table, row by row.
//! * S2d: the Oracle dump's tables, empty, from its DDL.
//!
//! # Determinism (P8)
//!
//! The same input gives the same table contents on every run and
//! every machine: files are listed in central-directory or name
//! order, ids are assigned in that order, and the store records no
//! time or path -- only the tool, its version and the input's
//! SHA-256. A store is written to `<out>.tmp` and renamed into place
//! when complete.

pub mod input;
pub mod manifest;
pub mod mssql;
pub mod redact;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::backup::refdata::{classify_format, RefDataFormat};
use crate::backup::zip_index::ZipNameEncoding;

pub use input::{BackupInput, BackupInputKind, InputFile, DIRECTORY_INPUT_NOTE};
pub use manifest::{reassemble_manifest, ManifestEncoding, MANIFEST_FILE_NAME};
pub use mssql::store_value;
pub use redact::{redact_backup_command, redact_field, RedactionRule, MASK};

/// The tool name `store_info.tool_name` records.
pub const TOOL_NAME: &str = "pid_backup_store";

/// The part a schema of the Database Dump plays for its plant (Q8,
/// ADR-0004): what names its dumped tables, `<role>__<table>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaRole {
    /// The plant schema (`<p>`), connection type code 2.
    Plant,
    /// The plant dictionary (`<p>d`), type code 8.
    PlantDictionary,
    /// The P&ID schema (`<p>pid`), type code 4.
    Pid,
    /// The P&ID dictionary (`<p>pidd`), type code 9.
    PidDictionary,
}

impl SchemaRole {
    /// Every role, in type-code order of the Manifest's connection lines.
    pub const ALL: [Self; 4] = [
        Self::Plant,
        Self::PlantDictionary,
        Self::Pid,
        Self::PidDictionary,
    ];

    /// The role a `PlantConnInfo` schema type code names.
    pub fn from_type_code(code: &str) -> Option<Self> {
        match code {
            "2" => Some(Self::Plant),
            "8" => Some(Self::PlantDictionary),
            "4" => Some(Self::Pid),
            "9" => Some(Self::PidDictionary),
            _ => None,
        }
    }

    /// The type code the role answers to.
    pub fn type_code(self) -> &'static str {
        match self {
            Self::Plant => "2",
            Self::PlantDictionary => "8",
            Self::Pid => "4",
            Self::PidDictionary => "9",
        }
    }

    /// The prefix of the role's dumped tables and its `dump_schema.role`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plant => "plant",
            Self::PlantDictionary => "plantd",
            Self::Pid => "pid",
            Self::PidDictionary => "pidd",
        }
    }

    /// The store table a source table of this role becomes.
    pub fn store_table_name(self, source_name: &str) -> String {
        format!("{}__{source_name}", self.as_str())
    }
}

/// One schema of the dump with its role, as `dump_schema` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpSchema {
    /// The role.
    pub role: SchemaRole,
    /// The schema's name in the Manifest (and, for SQL Server, in the dump).
    pub schema_name: String,
    /// The Manifest's database type (`1` SQL Server, `2` Oracle).
    pub db_type: String,
    /// How the role was found: `manifest-conninfo` or `schema-name-suffix` (P6).
    pub role_source: &'static str,
}

/// The four plant schemas of a Manifest, from its `PlantConnInfo`
/// lines (fields 2, 4 and 8); an error unless exactly the four roles
/// appear once each.
pub fn plant_schemas_from_manifest(
    manifest: &crate::backup::Manifest,
) -> Result<Vec<DumpSchema>, BackupStoreError> {
    let mut schemas = Vec::new();
    for line in manifest.all("PlantConnInfo") {
        let field = |position: usize| line.fields.get(position - 1).map(String::as_str);
        let (Some(code), Some(schema_name), Some(db_type)) = (field(2), field(4), field(8)) else {
            return Err(BackupStoreError::SchemaRoles {
                what: "a PlantConnInfo line is short of its 13 fields",
            });
        };
        let Some(role) = SchemaRole::from_type_code(code) else {
            return Err(BackupStoreError::SchemaRoles {
                what: "a PlantConnInfo line carries a schema type code other than 2 / 8 / 4 / 9",
            });
        };
        if schemas
            .iter()
            .any(|schema: &DumpSchema| schema.role == role)
        {
            return Err(BackupStoreError::SchemaRoles {
                what: "two PlantConnInfo lines carry the same schema type code",
            });
        }
        schemas.push(DumpSchema {
            role,
            schema_name: schema_name.to_string(),
            db_type: db_type.to_string(),
            role_source: "manifest-conninfo",
        });
    }
    if schemas.len() != SchemaRole::ALL.len() {
        return Err(BackupStoreError::SchemaRoles {
            what: "the Manifest does not name exactly four plant schemas",
        });
    }
    schemas.sort_by_key(|schema| schema.role);
    Ok(schemas)
}

/// What goes wrong building a store.
#[derive(Debug, Error)]
pub enum BackupStoreError {
    /// Reading or writing `path` failed.
    #[error("{path}: {source}")]
    Io {
        /// The file or directory in question.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The `zip` crate refused `archive`.
    #[error("zip archive {archive}: {source}")]
    Zip {
        /// The outer input or the Option Archive in question.
        archive: String,
        /// The underlying error.
        #[source]
        source: zip::result::ZipError,
    },
    /// SQLite refused a statement.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The input is neither a directory nor a ZIP archive.
    #[error("{path}: neither a directory nor a ZIP archive (`PK\\x03\\x04`)")]
    UnrecognisedInput {
        /// The input path.
        path: PathBuf,
    },
    /// An outer entry index beyond the input's files.
    #[error("no outer entry {entry_index} in the input")]
    NoSuchEntry {
        /// The index asked for.
        entry_index: usize,
    },
    /// The input has no `Manifest.txt` among its outer files.
    #[error("the input has no {MANIFEST_FILE_NAME} among its outer files")]
    MissingManifest,
    /// A store being read back is not shaped as this module writes it.
    #[error("store: {what}")]
    StoreShape {
        /// What was found wanting.
        what: &'static str,
    },
    /// The Manifest's connection lines do not give the four Schema Roles.
    #[error("schema roles: {what}")]
    SchemaRoles {
        /// What was found wanting.
        what: &'static str,
    },
    /// The Database Dump is an MTF file the SQL Server streams could
    /// not be taken from.
    #[error("Export.dmp: {0}")]
    Dump(#[from] crate::backup::mtf::SqlServerDumpError),
    /// The MDF inside the dump could not be read.
    #[error("Export.mdf: {0}")]
    Mdf(#[from] oxidized_mdf::error::Error),
    /// The Manifest lists a table the dump does not hold, or the dump a
    /// schema the Manifest does not name.
    #[error("dump: {what}: {name}")]
    DumpShape {
        /// What was found wanting.
        what: &'static str,
        /// The schema or table in question.
        name: String,
    },
}

/// What the caller decides about a store (Q2, Q15).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoreOptions {
    /// Keep the Manifest's credentials as written instead of
    /// replacing them with their SHA-256 (`--keep-secrets`).
    pub keep_secrets: bool,
    /// Store every file's bytes in `backup_file_content`
    /// (`--embed-files`).
    pub embed_files: bool,
}

/// What a build put into the store, for the command line to print.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreSummary {
    /// Rows of `backup_file`: outer files plus the entries of every
    /// Option Archive.
    pub files: usize,
    /// Of those, files whose bytes went into `backup_file_content`.
    pub files_embedded: usize,
    /// Lines of `Manifest.txt`, as `manifest_line` holds them.
    pub manifest_lines: usize,
    /// Manifest fields replaced, as `store_redaction` lists them.
    pub redactions: usize,
    /// Dumped tables (`dump_table` rows).
    pub tables: usize,
    /// Live rows written into the dumped tables.
    pub rows: u64,
    /// Ghost Rows kept in `dump_ghost_row`.
    pub ghost_rows: u64,
    /// LOB values read, as `dump_lob` lists them.
    pub lobs: u64,
    /// Notes worth showing: the directory-input caveat and the like.
    pub warnings: Vec<String>,
}

/// Builds the store of the Plant Backup at `input` into the SQLite
/// file `out`, writing `<out>.tmp` first and renaming it into place
/// (over an existing `out`; whether that is allowed is the caller's
/// call, see `pid_backup_store --force`).
pub fn build_backup_store(
    input: &Path,
    out: &Path,
    options: &StoreOptions,
) -> Result<StoreSummary, BackupStoreError> {
    let mut tmp_name: OsString = out.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    if tmp.exists() {
        fs::remove_file(&tmp).map_err(|source| BackupStoreError::Io {
            path: tmp.clone(),
            source,
        })?;
    }

    let summary = {
        let conn = Connection::open(&tmp)?;
        let summary = fill_store(&conn, input, options);
        conn.close().map_err(|(_, err)| err)?;
        summary
    };
    let summary = match summary {
        Ok(summary) => summary,
        Err(err) => {
            let _ = fs::remove_file(&tmp);
            return Err(err);
        }
    };

    fs::rename(&tmp, out).map_err(|source| BackupStoreError::Io {
        path: out.to_path_buf(),
        source,
    })?;
    Ok(summary)
}

/// Builds the store of the Plant Backup at `input` in memory, for a
/// reader that wants the tables and not a file.
pub fn build_backup_store_in_memory(
    input: &Path,
    options: &StoreOptions,
) -> Result<(Connection, StoreSummary), BackupStoreError> {
    let conn = Connection::open_in_memory()?;
    let summary = fill_store(&conn, input, options)?;
    Ok((conn, summary))
}

/// The tables S2a fills. Later steps add theirs.
const SCHEMA: &str = "
CREATE TABLE store_info (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE backup_file (
    id            INTEGER PRIMARY KEY,
    container_id  INTEGER REFERENCES backup_file(id),
    entry_index   INTEGER NOT NULL,
    path          TEXT    NOT NULL,
    path_raw      BLOB    NOT NULL,
    path_encoding TEXT    NOT NULL,
    is_dir        INTEGER NOT NULL,
    size          INTEGER NOT NULL,
    sha256        TEXT,
    format        TEXT,
    UNIQUE (container_id, entry_index)
);

CREATE TABLE backup_file_content (
    file_id INTEGER PRIMARY KEY REFERENCES backup_file(id),
    bytes   BLOB NOT NULL
);
";

/// Opens the input and fills every table of the store into `conn`.
fn fill_store(
    conn: &Connection,
    input: &Path,
    options: &StoreOptions,
) -> Result<StoreSummary, BackupStoreError> {
    let mut input = BackupInput::open(input)?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(manifest::SCHEMA)?;
    conn.execute_batch(mssql::SCHEMA)?;

    let mut summary = StoreSummary::default();
    let tx = conn.unchecked_transaction()?;
    write_store_info(&tx, &input, options)?;
    write_backup_files(&tx, &mut input, options, &mut summary)?;
    let manifest_bytes = input.read(outer_index(&input, MANIFEST_FILE_NAME)?)?;
    manifest::write_manifest(&tx, &manifest_bytes, options, &mut summary)?;
    write_dump(&tx, &mut input, &manifest_bytes, &mut summary)?;
    tx.commit()?;

    summary.warnings.extend(input.note().map(str::to_string));
    Ok(summary)
}

/// The entry index of the outer file named `name`.
fn outer_index(input: &BackupInput, name: &str) -> Result<usize, BackupStoreError> {
    input
        .files()
        .iter()
        .find(|file| !file.is_dir && file.name == name)
        .map(|file| file.entry_index)
        .ok_or(BackupStoreError::MissingManifest)
}

/// The Database Dump: the file the Manifest's connection lines name
/// (field 12, `Export.dmp`). A SQL Server MTF dump is read into its
/// tables (S2c); an Oracle `exp` dump is recognised and recorded as
/// `store_info.dump_kind` only, its tables waiting for S2d.
fn write_dump(
    conn: &Connection,
    input: &mut BackupInput,
    manifest_bytes: &[u8],
    summary: &mut StoreSummary,
) -> Result<(), BackupStoreError> {
    let manifest = crate::backup::parse_manifest_bytes(manifest_bytes);
    let schemas = plant_schemas_from_manifest(&manifest)?;
    let dump_name = manifest
        .all("PlantConnInfo")
        .filter_map(|line| line.fields.get(11))
        .find(|name| !name.is_empty() && *name != "None")
        .cloned()
        .unwrap_or_else(|| "Export.dmp".to_string());
    let Some(index) = input
        .files()
        .iter()
        .find(|file| !file.is_dir && file.name == dump_name)
        .map(|file| file.entry_index)
    else {
        summary.warnings.push(format!(
            "{dump_name}: the Database Dump the Manifest names is not among the outer files; \
             no table dumped"
        ));
        return Ok(());
    };
    let dump = input.read(index)?;

    let mut info = conn.prepare("INSERT INTO store_info (key, value) VALUES (?1, ?2)")?;
    match crate::backup::mtf::mdf_bytes_of_dump(&dump) {
        Ok(mdf) => {
            info.execute(params!["dump_kind", "sql-server-mtf"])?;
            let mdf = mdf.to_vec();
            drop(dump);
            mssql::write_sql_server_dump(conn, mdf, &manifest, &schemas, summary)?;
        }
        Err(crate::backup::mtf::SqlServerDumpError::NotMtf(diagnostic)) => {
            info.execute(params!["dump_kind", "oracle-exp"])?;
            // The diagnostic opens with what the file is; the advice after
            // the first comma is for the extraction tools, not for here.
            let what = diagnostic
                .split(", not a SQL Server")
                .next()
                .unwrap_or(&diagnostic);
            summary.warnings.push(format!(
                "{dump_name}: {what}; its tables are not decoded in this version"
            ));
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

fn write_store_info(
    conn: &Connection,
    input: &BackupInput,
    options: &StoreOptions,
) -> Result<(), BackupStoreError> {
    let mut insert = conn.prepare("INSERT INTO store_info (key, value) VALUES (?1, ?2)")?;
    let flag = |on: bool| if on { "1" } else { "0" };
    let mut rows: Vec<(&str, String)> = vec![
        ("tool_name", TOOL_NAME.to_string()),
        ("tool_version", env!("CARGO_PKG_VERSION").to_string()),
        ("input_kind", input.kind().as_str().to_string()),
        ("input_sha256", input.sha256().to_string()),
        ("redacted", flag(!options.keep_secrets).to_string()),
        ("files_embedded", flag(options.embed_files).to_string()),
    ];
    if let Some(note) = input.note() {
        rows.push(("input_note", note.to_string()));
    }
    for (key, value) in rows {
        insert.execute(params![key, value])?;
    }
    Ok(())
}

/// One row of `backup_file` before it is written.
struct FileRow<'a> {
    container_id: Option<i64>,
    entry_index: usize,
    path: &'a str,
    path_raw: &'a [u8],
    path_encoding: ZipNameEncoding,
    is_dir: bool,
    size: u64,
    bytes: &'a [u8],
}

/// Writes `backup_file` (and `backup_file_content`) rows with ids in
/// the order they are given.
struct FileWriter<'conn> {
    insert: rusqlite::Statement<'conn>,
    insert_content: rusqlite::Statement<'conn>,
    embed_files: bool,
    next_id: i64,
}

impl<'conn> FileWriter<'conn> {
    fn new(conn: &'conn Connection, options: &StoreOptions) -> Result<Self, BackupStoreError> {
        Ok(Self {
            insert: conn.prepare(
                "INSERT INTO backup_file (id, container_id, entry_index, path, path_raw, \
                 path_encoding, is_dir, size, sha256, format) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?,
            insert_content: conn
                .prepare("INSERT INTO backup_file_content (file_id, bytes) VALUES (?1, ?2)")?,
            embed_files: options.embed_files,
            next_id: 1,
        })
    }

    /// Writes one row and returns its id. A directory entry gets no
    /// SHA-256, format or content.
    fn write(
        &mut self,
        row: &FileRow<'_>,
        summary: &mut StoreSummary,
    ) -> Result<i64, BackupStoreError> {
        let id = self.next_id;
        self.next_id += 1;
        let (sha256, format) = if row.is_dir {
            (None, None)
        } else {
            (
                Some(input::sha256_hex(row.bytes)),
                Some(format_label(classify_format(row.bytes))),
            )
        };
        self.insert.execute(params![
            id,
            row.container_id,
            i64::try_from(row.entry_index).unwrap_or(i64::MAX),
            row.path,
            row.path_raw,
            row.path_encoding.as_str(),
            i64::from(row.is_dir),
            i64::try_from(row.size).unwrap_or(i64::MAX),
            sha256,
            format,
        ])?;
        summary.files += 1;
        if self.embed_files && !row.is_dir {
            self.insert_content.execute(params![id, row.bytes])?;
            summary.files_embedded += 1;
        }
        Ok(id)
    }
}

/// Writes the outer files, then the entries of each Option Archive
/// among them, one level deep (Q14): outer ids come first, then each
/// archive's entries in central directory order.
fn write_backup_files(
    conn: &Connection,
    input: &mut BackupInput,
    options: &StoreOptions,
    summary: &mut StoreSummary,
) -> Result<(), BackupStoreError> {
    let mut writer = FileWriter::new(conn, options)?;

    let outer = input.files().to_vec();
    let mut archives: Vec<(i64, String, Vec<u8>)> = Vec::new();
    for file in &outer {
        let bytes = if file.is_dir {
            Vec::new()
        } else {
            input.read(file.entry_index)?
        };
        let id = writer.write(
            &FileRow {
                container_id: None,
                entry_index: file.entry_index,
                path: &file.name,
                path_raw: &file.name_raw,
                path_encoding: file.name_encoding,
                is_dir: file.is_dir,
                size: file.size,
                bytes: &bytes,
            },
            summary,
        )?;
        if !file.is_dir && is_option_archive(&file.name) {
            archives.push((id, file.name.clone(), bytes));
        }
    }

    for (container_id, name, bytes) in archives {
        let mut archive = input::OptionArchive::open(&name, bytes)?;
        for index in 0..archive.len() {
            let (entry, content) = archive.read(index)?;
            writer.write(
                &FileRow {
                    container_id: Some(container_id),
                    entry_index: index,
                    path: &entry.name,
                    path_raw: &entry.name_raw,
                    path_encoding: entry.name_encoding,
                    is_dir: entry.is_dir,
                    size: entry.size,
                    bytes: &content,
                },
                summary,
            )?;
        }
    }
    Ok(())
}

/// `<PlantData|RefData>~<schema>~<id>.zip`: a directory `SmartPlant`
/// packed as one archive (format doc section 3). Single-file payloads
/// carry no `.zip`, whatever their bytes are (`RefData~4~703` is an
/// xlsx), and are not opened.
pub fn is_option_archive(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".zip") else {
        return false;
    };
    let rest = if let Some(rest) = stem.strip_prefix("PlantData~") {
        rest
    } else if let Some(rest) = stem.strip_prefix("RefData~") {
        rest
    } else {
        return false;
    };
    let mut parts = rest.split('~');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    };
    numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none()
}

/// The label `backup_file.format` holds for a classification.
pub fn format_label(format: RefDataFormat) -> String {
    match format {
        RefDataFormat::Zip => "zip".to_string(),
        RefDataFormat::Cfb => "cfb".to_string(),
        RefDataFormat::Xml => "xml".to_string(),
        RefDataFormat::AsciiText => "ascii".to_string(),
        RefDataFormat::Unknown(magic) => format!(
            "unknown:{:02x}{:02x}{:02x}{:02x}",
            magic[0], magic[1], magic[2], magic[3]
        ),
    }
}
