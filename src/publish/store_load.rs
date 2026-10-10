//! Publish reads a Backup Store (S4 of plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`:
//! Q13, Q18, P6, P7).
//!
//! The query loader ([`super::sqlite_load`]) wants the publish tables
//! as TEXT columns, named as `SmartPlant` names them. A Backup Store
//! holds them typed, under `<role>__<table>`. This module copies the
//! typed rows into an in-memory connection of TEXT tables with the
//! rules the MDF adapter used until S4 -- datetime as
//! `YYYY/M/D HH:MM:SS`, binary as upper-case hex, floats as Rust
//! prints them (P7) -- so the XML a drawing publishes to stays byte
//! for byte what it was. `codelists` and `attributes` come from the
//! `pidd__` tables, the other 22 from `pid__` (Q18: the source moves
//! to the P&ID dictionary; the join itself is a separately tracked
//! defect).
//!
//! It also names the inputs publish accepts ([`PublishInput`]): a
//! Backup Store file, a Plant Backup (zip or directory), an
//! `Export.mdf` on its own (read through an in-memory store whose
//! Schema Roles come from the schema names, P6), or a legacy
//! `Export_v2.sqlite` mirror.

use std::path::Path;
use std::time::Instant;

use log::info;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection, OpenFlags};

use crate::backup::{
    build_backup_store_from_mdf_in_memory, build_backup_store_in_memory, StoreOptions,
};

use super::model::{PublishDrawing, PublishError};
use super::sqlite_load;

/// The `SmartPlant` tables publish reads, in the order they are staged.
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

/// The store table a publish table is read from: the dictionary
/// tables from the P&ID dictionary (`pidd__`), the rest from the
/// P&ID schema (`pid__`).
pub fn store_table_name(publish_table: &str) -> String {
    match publish_table {
        "codelists" | "attributes" => format!("pidd__{publish_table}"),
        _ => format!("pid__{publish_table}"),
    }
}

/// What a path given to publish is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishInput {
    /// A SQL Server `Export.mdf` on its own (`.mdf`).
    Mdf,
    /// A Plant Backup: the `<Plant>_p.zip` or the directory it unpacks to.
    PlantBackup,
    /// A Backup Store `pid_backup_store` wrote (a SQLite file with `dump_table`).
    BackupStore,
    /// A legacy `Export_v2.sqlite` mirror: the publish tables as TEXT already.
    LegacySqlite,
}

impl PublishInput {
    /// The kind as a word for reports (`mdf`, `plant-backup`,
    /// `backup-store`, `sqlite`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mdf => "mdf",
            Self::PlantBackup => "plant-backup",
            Self::BackupStore => "backup-store",
            Self::LegacySqlite => "sqlite",
        }
    }
}

/// Which input `path` is: a directory or a file opening with the ZIP
/// signature is a Plant Backup, a `.mdf` an MDF, a SQLite file holding
/// `dump_table` a Backup Store, any other file a legacy mirror.
pub fn classify_publish_input(path: &Path) -> PublishInput {
    if path.is_dir() {
        return PublishInput::PlantBackup;
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdf"))
    {
        return PublishInput::Mdf;
    }
    let mut magic = [0u8; 16];
    let read = std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read(&mut file, &mut magic))
        .unwrap_or(0);
    if magic[..read].starts_with(b"PK\x03\x04") {
        return PublishInput::PlantBackup;
    }
    if magic[..read].starts_with(b"SQLite format 3\0") && sqlite_file_has_dump_table(path) {
        return PublishInput::BackupStore;
    }
    PublishInput::LegacySqlite
}

fn sqlite_file_has_dump_table(path: &Path) -> bool {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|conn| {
            conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'dump_table'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .is_ok_and(|n| n > 0)
}

/// Opens any publish input as the connection of TEXT tables the
/// query loader reads: an MDF or a Plant Backup through an in-memory
/// Backup Store, a Backup Store file through the copy, a legacy
/// mirror read-only as it is.
pub fn open_publish_input(path: &Path) -> Result<Connection, PublishError> {
    match classify_publish_input(path) {
        PublishInput::Mdf => open_mdf_as_sqlite(path),
        PublishInput::PlantBackup => {
            let t0 = Instant::now();
            let (store, summary) = build_backup_store_in_memory(path, &StoreOptions::default())?;
            info!(
                "Plant Backup read into a Backup Store: {} tables, {} rows in {:.1}ms ({})",
                summary.tables,
                summary.rows,
                t0.elapsed().as_secs_f64() * 1000.0,
                path.display(),
            );
            for warning in &summary.warnings {
                info!("  {warning}");
            }
            copy_publish_tables(&store)
        }
        PublishInput::BackupStore => {
            let store = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            copy_publish_tables(&store)
        }
        PublishInput::LegacySqlite => sqlite_load::open_readonly(path),
    }
}

/// Reads `path` (an `Export.mdf`) into an in-memory Backup Store and
/// copies the publish tables out of it.
pub fn open_mdf_as_sqlite(path: &Path) -> Result<Connection, PublishError> {
    let t0 = Instant::now();
    let mdf = std::fs::read(path)
        .map_err(|err| PublishError::Mdf(format!("read {}: {err}", path.display())))?;
    let (store, summary) = build_backup_store_from_mdf_in_memory(mdf)?;
    info!(
        "MDF read into a Backup Store: {} tables, {} rows in {:.1}ms ({})",
        summary.tables,
        summary.rows,
        t0.elapsed().as_secs_f64() * 1000.0,
        path.display(),
    );
    copy_publish_tables(&store)
}

/// Loads one drawing graph from an `Export.mdf` on its own.
pub fn load_drawing_graph_from_mdf(
    path: &Path,
    drawing_uid: &str,
) -> Result<PublishDrawing, PublishError> {
    let conn = open_mdf_as_sqlite(path)?;
    sqlite_load::load_drawing_graph(&conn, drawing_uid)
}

/// Copies the publish tables of `store` into a fresh in-memory
/// connection as TEXT tables named as `SmartPlant` names them. A
/// table the store does not hold is skipped, as the MDF adapter
/// skipped a table the MDF did not hold.
pub fn copy_publish_tables(store: &Connection) -> Result<Connection, PublishError> {
    let t0 = Instant::now();
    let conn = Connection::open_in_memory()?;
    let mut tables_staged = 0u32;
    let mut total_rows = 0usize;
    for table_name in PUBLISH_TABLES {
        let rows = copy_table(store, &conn, table_name)?;
        if rows > 0 {
            tables_staged += 1;
        }
        total_rows += rows;
    }
    info!(
        "store staged: {} tables, {} rows in {:.1}ms",
        tables_staged,
        total_rows,
        t0.elapsed().as_secs_f64() * 1000.0,
    );
    Ok(conn)
}

/// Copies one table; returns how many rows.
fn copy_table(
    store: &Connection,
    conn: &Connection,
    table_name: &str,
) -> Result<usize, PublishError> {
    let store_name = store_table_name(table_name);
    let columns: Vec<(String, String)> = {
        let mut stmt = store.prepare(
            "SELECT c.name, c.source_type FROM dump_column c \
             JOIN dump_table t ON t.role = c.role AND t.source_name = c.table_name \
             WHERE t.store_name = ?1 ORDER BY c.ordinal",
        )?;
        let columns = stmt
            .query_map([&store_name], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        columns
    };
    if columns.is_empty() {
        info!("  {table_name}: not in the store ({store_name}), skipped");
        return Ok(0);
    }

    conn.execute(
        &format!(
            "CREATE TABLE {} ({})",
            quote_ident(table_name),
            columns
                .iter()
                .map(|(name, _)| format!("{} TEXT", quote_ident(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        [],
    )?;
    let column_list = columns
        .iter()
        .map(|(name, _)| quote_ident(name))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (0..columns.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let mut insert = conn.prepare(&format!(
        "INSERT INTO {} ({column_list}) VALUES ({placeholders})",
        quote_ident(table_name)
    ))?;

    // The store's rowid order is the dump's page-chain and slot order
    // (P8), which is the order the MDF adapter staged rows in.
    let mut select = store.prepare(&format!(
        "SELECT {column_list} FROM {} ORDER BY rowid",
        quote_ident(&store_name)
    ))?;
    let mut rows = select.query([])?;
    let mut row_count = 0usize;
    while let Some(row) = rows.next()? {
        let mut values: Vec<Option<String>> = Vec::with_capacity(columns.len());
        for (index, (_, source_type)) in columns.iter().enumerate() {
            values.push(text_of(source_type, row.get::<_, SqlValue>(index)?));
        }
        insert.execute(params_from_iter(values.iter()))?;
        row_count += 1;
    }
    info!(
        "  {}: {} rows, {} cols",
        table_name,
        row_count,
        columns.len()
    );
    Ok(row_count)
}

/// A typed store value as the TEXT the publish tables hold, by the
/// column's SQL Server type: what `oxidized_mdf::Value`'s `Display`
/// gave the MDF adapter, with datetime as `YYYY/M/D HH:MM:SS`
/// (milliseconds dropped) and binary as upper-case hex.
pub fn text_of(source_type: &str, value: SqlValue) -> Option<String> {
    Some(match (source_type, value) {
        (_, SqlValue::Null) => return None,
        ("datetime" | "smalldatetime" | "date", SqlValue::Text(text)) => {
            datetime_as_the_adapter_wrote_it(&text).unwrap_or(text)
        }
        // `real` is a 4-byte float the store widened; print it at its
        // own width, as the reader's `Value::Real(f32)` did.
        ("real", SqlValue::Real(f)) => (f as f32).to_string(),
        // `Value::Bit(bool)` printed `true` / `false`.
        ("bit", SqlValue::Integer(i)) => (i != 0).to_string(),
        // The store keeps a GUID upper-case; `Value::Uuid` printed it
        // lower-case.
        ("uniqueidentifier", SqlValue::Text(text)) => text.to_lowercase(),
        (_, SqlValue::Integer(i)) => i.to_string(),
        (_, SqlValue::Real(f)) => f.to_string(),
        (_, SqlValue::Text(text)) => text,
        (_, SqlValue::Blob(bytes)) => bytes.iter().map(|b| format!("{b:02X}")).collect(),
    })
}

/// The store's `YYYY-MM-DD HH:MM:SS.fff` as the MDF adapter's
/// `%Y/%-m/%-d %H:%M:%S`: month and day without their leading zero,
/// the fraction dropped. `None` when `text` is not shaped like that.
fn datetime_as_the_adapter_wrote_it(text: &str) -> Option<String> {
    let (date, time) = text.split_once(' ')?;
    let time = time.split_once('.').map_or(time, |(whole, _)| whole);
    let [year, month, day] = <[&str; 3]>::try_from(date.split('-').collect::<Vec<_>>()).ok()?;
    let [hour, minute, second] = <[&str; 3]>::try_from(time.split(':').collect::<Vec<_>>()).ok()?;
    let digits = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
    if !(digits(year, 4)
        && digits(month, 2)
        && digits(day, 2)
        && digits(hour, 2)
        && digits(minute, 2)
        && digits(second, 2))
    {
        return None;
    }
    fn unpad(s: &str) -> &str {
        s.strip_prefix('0').unwrap_or(s)
    }
    Some(format!(
        "{year}/{}/{} {hour}:{minute}:{second}",
        unpad(month),
        unpad(day)
    ))
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_print_as_the_mdf_adapter_printed_them() {
        assert_eq!(None, text_of("nvarchar", SqlValue::Null));
        assert_eq!(
            Some(String::new()),
            text_of("nvarchar", SqlValue::Text(String::new()))
        );
        assert_eq!(
            Some("2026/4/20 10:32:46".to_string()),
            text_of(
                "datetime",
                SqlValue::Text("2026-04-20 10:32:46.007".to_string())
            )
        );
        // `%-m` / `%-d` drop the leading zero; `%H` keeps it.
        assert_eq!(
            Some("2026/12/1 00:00:00".to_string()),
            text_of(
                "datetime",
                SqlValue::Text("2026-12-01 00:00:00.000".to_string())
            )
        );
        // Not the store's shape: passed through as it is.
        assert_eq!(
            Some("2026-04-20".to_string()),
            text_of("datetime", SqlValue::Text("2026-04-20".to_string()))
        );
        assert_eq!(
            Some("250".to_string()),
            text_of("int", SqlValue::Integer(250))
        );
        assert_eq!(
            Some("250".to_string()),
            text_of("float", SqlValue::Real(250.0))
        );
        assert_eq!(
            Some("0.1".to_string()),
            text_of("real", SqlValue::Real(f64::from(0.1f32)))
        );
        assert_eq!(
            Some("0.10000000149011612".to_string()),
            text_of("float", SqlValue::Real(f64::from(0.1f32)))
        );
        assert_eq!(
            Some("true".to_string()),
            text_of("bit", SqlValue::Integer(1))
        );
        assert_eq!(
            Some("504B0304".to_string()),
            text_of("image", SqlValue::Blob(vec![0x50, 0x4B, 0x03, 0x04]))
        );
        assert_eq!(
            Some("6f9619ff-8b86-d011-b42d-00c04fc964ff".to_string()),
            text_of(
                "uniqueidentifier",
                SqlValue::Text("6F9619FF-8B86-D011-B42D-00C04FC964FF".to_string())
            )
        );
    }

    #[test]
    fn dictionary_tables_come_from_pidd_and_the_rest_from_pid() {
        assert_eq!("pidd__codelists", store_table_name("codelists"));
        assert_eq!("pidd__attributes", store_table_name("attributes"));
        assert_eq!("pid__T_Drawing", store_table_name("T_Drawing"));
    }

    #[test]
    fn a_store_without_a_table_skips_it_and_copies_the_others_as_text() {
        let store = Connection::open_in_memory().unwrap();
        store
            .execute_batch(
                "CREATE TABLE dump_table (role TEXT, source_name TEXT, store_name TEXT);
                 CREATE TABLE dump_column (role TEXT, table_name TEXT, ordinal INTEGER, \
                 name TEXT, source_type TEXT);
                 INSERT INTO dump_table VALUES ('pid', 'T_Drawing', 'pid__T_Drawing');
                 INSERT INTO dump_column VALUES ('pid', 'T_Drawing', 1, 'SP_ID', 'nvarchar');
                 INSERT INTO dump_column VALUES ('pid', 'T_Drawing', 2, 'DateCreated', 'datetime');
                 INSERT INTO dump_column VALUES ('pid', 'T_Drawing', 3, 'UpdateCount', 'int');
                 CREATE TABLE pid__T_Drawing (SP_ID TEXT, DateCreated TEXT, UpdateCount INTEGER, \
                 _src_page INTEGER, _src_slot INTEGER);
                 INSERT INTO pid__T_Drawing VALUES ('B', '2026-04-20 10:32:46.000', 3, 2194, 1);
                 INSERT INTO pid__T_Drawing VALUES ('A', NULL, NULL, 2194, 0);",
            )
            .unwrap();
        let conn = copy_publish_tables(&store).unwrap();
        let rows: Vec<(String, Option<String>, Option<String>)> = {
            let mut stmt = conn
                .prepare("SELECT SP_ID, DateCreated, UpdateCount FROM T_Drawing")
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        // Rows keep the store's order; every column is TEXT.
        assert_eq!(
            vec![
                (
                    "B".to_string(),
                    Some("2026/4/20 10:32:46".to_string()),
                    Some("3".to_string())
                ),
                ("A".to_string(), None, None),
            ],
            rows
        );
        let declared: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT type FROM pragma_table_info('T_Drawing')")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(vec!["TEXT"; 3], declared);
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(1, tables, "the 23 tables the store lacks are skipped");
    }

    #[test]
    fn a_file_is_classified_by_what_it_holds() {
        let dir = std::env::temp_dir().join(format!(
            "pid-parse-store-load-classify-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(PublishInput::PlantBackup, classify_publish_input(&dir));

        let zip = dir.join("plant_p.zip");
        std::fs::write(&zip, b"PK\x03\x04rest").unwrap();
        assert_eq!(PublishInput::PlantBackup, classify_publish_input(&zip));

        let mdf = dir.join("Export.MDF");
        assert_eq!(PublishInput::Mdf, classify_publish_input(&mdf));

        let store = dir.join("store.sqlite");
        Connection::open(&store)
            .unwrap()
            .execute_batch("CREATE TABLE dump_table (role TEXT);")
            .unwrap();
        assert_eq!(PublishInput::BackupStore, classify_publish_input(&store));

        let legacy = dir.join("Export_v2.sqlite");
        Connection::open(&legacy)
            .unwrap()
            .execute_batch("CREATE TABLE T_Drawing (SP_ID TEXT);")
            .unwrap();
        assert_eq!(PublishInput::LegacySqlite, classify_publish_input(&legacy));
        assert_eq!(
            PublishInput::LegacySqlite,
            classify_publish_input(&dir.join("absent.sqlite"))
        );
        assert_eq!("backup-store", PublishInput::BackupStore.as_str());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
