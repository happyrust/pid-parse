//! Load Publish XML source rows directly from a SQL Server MDF file
//! using the vendored `oxidized-mdf` Rust reader.
//!
//! The existing publish loader already knows how to turn a table set
//! shaped like `SmartPlant`'s SQL tables into [`super::model::PublishDrawing`].
//! This module keeps that contract without a C# probe step: rows are
//! read from MDF and staged into an in-memory `SQLite` connection, then
//! the proven `sqlite_load` path builds the DTO.

use std::path::Path;
use std::time::Instant;

use log::info;
use oxidized_mdf::{MdfDatabase, Value};
use rusqlite::{params_from_iter, Connection};

use super::model::{PublishDrawing, PublishError};
use super::sqlite_load;

/// The tables [`open_mdf_as_sqlite`] stages, in the order it reads them. A
/// table the `.mdf` does not hold is skipped.
///
/// These are the tables [`sqlite_load`] asks for: the subtables the publish
/// item catalog gives its item types, and the ones the loader's own
/// statements name. A table left out here reads as empty there, and one
/// added here that nothing asks for is staged for nothing; the tests of
/// [`sqlite_load`] hold the two to each other.
pub const PUBLISH_TABLES: &[&str] = &[
    "T_Drawing",
    "T_Representation",
    "T_Relationship",
    "T_ModelItem",
    "T_PipingPoint",
    "T_PlantItem",
    "T_Equipment",
    "T_ProcessEquipment",
    "T_Vessel",
    "T_EquipComponent",
    "T_Nozzle",
    "T_Connector",
    "T_PipeRun",
    // `T_Pipeline` shares its `SP_ID` with the owning PipeRun in
    // the simplified single-Pipeline-per-PipeRun model SPPID uses
    // for Publish XML. Staging it here lets `subtables_for_item_type`
    // attach `OperFluidCode` / `FluidSystem` / `TagSequenceNo` /
    // `TagSuffix` onto the PipeRun's `PublishObject.fields` so the
    // PIDPipeline writer arm can read them.
    "T_Pipeline",
    "T_InlineComp",
    "T_PipingComp",
    "T_Instrument",
    "T_InstrFunction",
    "T_ItemNote",
    "T_Exchanger",
    "T_Mechanical",
    "T_SignalRun",
    "codelists",
    "attributes",
];

/// Open `path` with `oxidized-mdf`, stage the publish-relevant
/// `SmartPlant` tables into an in-memory `SQLite` connection, and return
/// that connection for reuse by the existing query loader.
///
/// The `.mdf` is opened once for all the tables, and the rows of a table go
/// in within one transaction.
pub fn open_mdf_as_sqlite(path: &Path) -> Result<Connection, PublishError> {
    let t0 = Instant::now();
    let mut conn = Connection::open_in_memory()?;
    let mut db = MdfDatabase::open(path)?;
    let mut tables_staged = 0u32;
    let mut total_rows = 0usize;
    for table_name in PUBLISH_TABLES {
        let rows = stage_table(&mut db, &mut conn, table_name)?;
        if rows > 0 {
            tables_staged += 1;
        }
        total_rows += rows;
    }
    info!(
        "MDF staged: {} tables, {} rows in {:.1}ms ({})",
        tables_staged,
        total_rows,
        t0.elapsed().as_secs_f64() * 1000.0,
        path.display(),
    );
    Ok(conn)
}

/// Load one drawing graph directly from an MDF file.
pub fn load_drawing_graph_from_mdf(
    path: &Path,
    drawing_uid: &str,
) -> Result<PublishDrawing, PublishError> {
    let conn = open_mdf_as_sqlite(path)?;
    sqlite_load::load_drawing_graph(&conn, drawing_uid)
}

/// Strip the MTF envelope and the SQL Server backup-stream prefix
/// off an `Export.dmp`, returning the embedded MDF image bytes.
///
/// The `MSDA` stream body located by
/// [`crate::backup::mtf::msda_stream_body`] still carries a leading
/// backup-stream header;
/// [`crate::backup::mdf_page::detect_backup_stream_header_len`]
/// finds where the first MDF page starts (with a `0x3F0` fallback).
///
/// # Errors
///
/// Returns [`PublishError::Mdf`] when the dump has no `MSDA` stream
/// or the detected header offset already runs past the body end.
pub fn mdf_bytes_from_mtf_dump(dump: &[u8]) -> Result<Vec<u8>, PublishError> {
    mdf_image_in_mtf_dump(dump).map(<[u8]>::to_vec)
}

/// The MDF image of an `Export.dmp`, where it lies in `dump`: what
/// [`mdf_bytes_from_mtf_dump`] returns a copy of.
fn mdf_image_in_mtf_dump(dump: &[u8]) -> Result<&[u8], PublishError> {
    let body = crate::backup::mtf::msda_stream_body(dump).ok_or_else(|| {
        PublishError::Mdf("no MSDA (SQL data) stream in the MTF dump".to_string())
    })?;
    let offset = crate::backup::mdf_page::detect_backup_stream_header_len(body);
    if offset >= body.len() {
        return Err(PublishError::Mdf(format!(
            "MSDA stream is shorter than its detected {offset}-byte header ({} bytes)",
            body.len()
        )));
    }
    Ok(&body[offset..])
}

/// Stage an MTF-wrapped SQL Server `Export.dmp` into the same
/// in-memory `SQLite` staging database [`open_mdf_as_sqlite`]
/// produces for a plain `.mdf` file.
///
/// [`open_mdf_as_sqlite`] takes a path, so the MDF image is written to a
/// temp file first, straight from where it lies in `dump`, and removed
/// again once staging completes (the returned connection no longer
/// touches it).
///
/// # Errors
///
/// Returns [`PublishError::Mdf`] when the dump yields no MDF image,
/// [`PublishError::Io`] when the temp file cannot be written, and
/// whatever [`open_mdf_as_sqlite`] reports while staging.
pub fn open_mtf_dump_as_sqlite(dump: &[u8]) -> Result<Connection, PublishError> {
    let staged = StagedMdf::write(mdf_image_in_mtf_dump(dump)?)?;
    open_mdf_as_sqlite(&staged.path)
}

/// A temp `.mdf` that is removed when it goes out of scope — after staging,
/// after a failed or partial write, and on unwind — so a plant-sized image
/// is never left in the temp directory.
struct StagedMdf {
    path: std::path::PathBuf,
}

impl StagedMdf {
    fn write(mdf: &[u8]) -> Result<Self, PublishError> {
        use std::io::Write as _;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "pid-parse-mtf-{}-{nanos}-{}.mdf",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| PublishError::Io(format!("create {}: {error}", path.display())))?;
        let staged = Self { path };
        file.write_all(mdf).map_err(|error| {
            PublishError::Io(format!("write {}: {error}", staged.path.display()))
        })?;
        Ok(staged)
    }
}

impl Drop for StagedMdf {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            log::warn!("could not remove {}: {error}", self.path.display());
        }
    }
}

fn stage_table(
    db: &mut MdfDatabase,
    conn: &mut Connection,
    table_name: &str,
) -> Result<usize, PublishError> {
    let Some(columns) = db.column_names(table_name) else {
        info!("  {table_name}: not found in MDF, skipped");
        return Ok(0);
    };
    if columns.is_empty() {
        info!("  {table_name}: 0 columns, skipped");
        return Ok(0);
    }

    let ddl = format!(
        "CREATE TABLE {} ({})",
        quote_ident(table_name),
        columns
            .iter()
            .map(|c| format!("{} TEXT", quote_ident(c)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    conn.execute(&ddl, [])?;

    let placeholders = (0..columns.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(table_name),
        columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders
    );

    let Some(rows) = db.try_rows(table_name) else {
        info!("  {table_name}: 0 rows");
        return Ok(0);
    };
    // Outside a transaction every `INSERT` is a transaction of its own.
    let transaction = conn.transaction()?;
    let mut row_count = 0usize;
    {
        let mut stmt = transaction.prepare(&insert_sql)?;
        for row in rows {
            let row = row?;
            let values = columns
                .iter()
                .map(|column| row.value(column).cloned().and_then(value_to_text))
                .collect::<Vec<_>>();
            stmt.execute(params_from_iter(values.iter()))?;
            row_count += 1;
        }
    }
    transaction.commit()?;
    info!(
        "  {}: {} rows, {} cols",
        table_name,
        row_count,
        columns.len()
    );

    Ok(row_count)
}

fn value_to_text(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::DateTime(dt) => Some(dt.format("%Y/%-m/%-d %H:%M:%S").to_string()),
        Value::Binary(bytes) => Some(bytes.iter().map(|b| format!("{b:02X}")).collect()),
        other => Some(other.to_string()),
    }
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
