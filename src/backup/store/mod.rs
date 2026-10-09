//! A Backup Store: one SQLite file holding what one Plant Backup
//! carries -- its file list, its `Manifest.txt`, the tables of its
//! Database Dump -- so the publish pipeline, a person or a script can
//! read the backup without the `SmartPlant` tools.
//!
//! Plan `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`
//! (ADR-0004: one store per backup, dumped tables named by Schema
//! Role). This module is being built up step by step:
//!
//! * S2a (this): the input ([`input`]) and the file list --
//!   `store_info`, `backup_file` and, behind
//!   [`StoreOptions::embed_files`], `backup_file_content`.
//! * S2b: `Manifest.txt` line by line, with the default redaction.
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

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::backup::refdata::{classify_format, RefDataFormat};
use crate::backup::zip_index::ZipNameEncoding;

pub use input::{BackupInput, BackupInputKind, InputFile, DIRECTORY_INPUT_NOTE};

/// The tool name `store_info.tool_name` records.
pub const TOOL_NAME: &str = "pid_backup_store";

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

    let mut summary = StoreSummary::default();
    let tx = conn.unchecked_transaction()?;
    write_store_info(&tx, &input, options)?;
    write_backup_files(&tx, &mut input, options, &mut summary)?;
    tx.commit()?;

    summary.warnings.extend(input.note().map(str::to_string));
    Ok(summary)
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
