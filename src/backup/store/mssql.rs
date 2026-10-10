//! The SQL Server Database Dump in the store (S2c): the 154 tables the
//! Manifest lists, row by row, each row with the page and slot it was
//! read from; the catalogue of them in `dump_schema`, `dump_table`,
//! `dump_column` and `dump_view`; Ghost Rows byte for byte in
//! `dump_ghost_row` and every LOB's root in `dump_lob` (Q8, Q9, Q11,
//! Q12).
//!
//! Values go in by source type (Q9): `nvarchar` as TEXT with `""` and
//! NULL apart, `int` as INTEGER, `float` as REAL, `datetime` as TEXT
//! `YYYY-MM-DD HH:MM:SS.fff`, `image` as the LOB's bytes. Tables are
//! created in the order of the Manifest's `Table` lines and rows
//! inserted in page-chain and slot order (P8).

use std::collections::BTreeSet;

use oxidized_mdf::{ColumnInfo, MdfDatabase, ScannedRecord, TableInfo, Value};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection};

use crate::backup::Manifest;

use super::input::sha256_hex;
use super::{write_schemas_and_views, BackupStoreError, DumpSchema, SchemaRole, StoreSummary};

/// The catalogue tables of the Database Dump, shared with
/// [`super::oracle`], which fills them for an Oracle dump (S2d).
pub(super) const SCHEMA: &str = "
-- schema_name is the dump's own spelling (the Manifest's for SQL
-- Server, the upper-case CONNECT owner for Oracle: P12, Q20).
CREATE TABLE dump_schema (
    role        TEXT PRIMARY KEY,
    schema_name TEXT NOT NULL,
    type_code   TEXT NOT NULL,
    db_type     TEXT NOT NULL,
    role_source TEXT NOT NULL
) WITHOUT ROWID;

-- decoded 1: rows read (SQL Server, expected_rows = rcrows);
-- decoded 0: the table is registered from the dump's DDL and empty
-- (Oracle, expected_rows NULL, Q3).
CREATE TABLE dump_table (
    role          TEXT    NOT NULL,
    source_name   TEXT    NOT NULL,
    store_name    TEXT    NOT NULL UNIQUE,
    decoded       INTEGER NOT NULL,
    expected_rows INTEGER,
    rows          INTEGER NOT NULL,
    ghost_rows    INTEGER NOT NULL,
    PRIMARY KEY (role, source_name)
) WITHOUT ROWID;

-- source_type is the source's type as written (nvarchar, NUMBER(11, 0)).
-- length, precision and scale are the source's own: for SQL Server the
-- catalogue's byte length (sys.columns.max_length: an nvarchar(32) is
-- 64, -1 is max) and declared precision / scale; for Oracle the
-- arguments of the type (NVARCHAR2(32) is 32, NUMBER(11, 0) is 11 / 0,
-- FLOAT(126) is 126). nullable is what the column was declared, not
-- what its rows hold.
CREATE TABLE dump_column (
    role        TEXT    NOT NULL,
    table_name  TEXT    NOT NULL,
    ordinal     INTEGER NOT NULL,
    name        TEXT    NOT NULL,
    source_type TEXT    NOT NULL,
    length      INTEGER,
    precision   INTEGER,
    scale       INTEGER,
    nullable    INTEGER,
    PRIMARY KEY (role, table_name, ordinal)
) WITHOUT ROWID;

CREATE TABLE dump_view (
    role        TEXT NOT NULL,
    schema_name TEXT NOT NULL,
    name        TEXT NOT NULL,
    PRIMARY KEY (role, name)
) WITHOUT ROWID;

CREATE TABLE dump_ghost_row (
    role        TEXT    NOT NULL,
    table_name  TEXT    NOT NULL,
    page        INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    record_type INTEGER NOT NULL,
    bytes       BLOB    NOT NULL,
    PRIMARY KEY (role, table_name, page, slot)
) WITHOUT ROWID;

CREATE TABLE dump_lob (
    role        TEXT    NOT NULL,
    table_name  TEXT    NOT NULL,
    row_page    INTEGER NOT NULL,
    row_slot    INTEGER NOT NULL,
    column_name TEXT    NOT NULL,
    root_page   INTEGER NOT NULL,
    root_slot   INTEGER NOT NULL,
    length      INTEGER NOT NULL,
    sha256      TEXT    NOT NULL,
    PRIMARY KEY (role, table_name, row_page, row_slot, column_name)
) WITHOUT ROWID;
";

/// The two columns every dumped table gets after its source columns (Q11).
pub const SRC_PAGE_COLUMN: &str = "_src_page";
/// See [`SRC_PAGE_COLUMN`].
pub const SRC_SLOT_COLUMN: &str = "_src_slot";

/// The SQLite type a source column is declared with, by SQL Server
/// type name (Q9): integers INTEGER, floats REAL, binaries BLOB,
/// everything else TEXT.
pub fn sqlite_type_of(source_type: &str) -> &'static str {
    match source_type {
        "bit" | "tinyint" | "smallint" | "int" | "bigint" => "INTEGER",
        "real" | "float" => "REAL",
        "image" | "varbinary" | "binary" | "timestamp" => "BLOB",
        _ => "TEXT",
    }
}

/// A value as it goes into a dumped table (Q9).
pub fn store_value(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bit(bit) => SqlValue::Integer(i64::from(*bit)),
        Value::TinyInt(i) => SqlValue::Integer(i64::from(*i)),
        Value::SmallInt(i) => SqlValue::Integer(i64::from(*i)),
        Value::Int(i) => SqlValue::Integer(i64::from(*i)),
        Value::BigInt(i) => SqlValue::Integer(*i),
        Value::Real(f) => SqlValue::Real(f64::from(*f)),
        Value::Float(f) => SqlValue::Real(*f),
        Value::Decimal(d) => SqlValue::Text(d.to_string()),
        Value::String(s) => SqlValue::Text(s.clone()),
        Value::Binary(b) => SqlValue::Blob(b.clone()),
        Value::DateTime(dt) => SqlValue::Text(dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
        Value::Uuid(u) => SqlValue::Text(u.to_string().to_uppercase()),
    }
}

/// `identifier` as a double-quoted SQLite identifier.
pub(super) fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Writes the catalogue and the tables of a SQL Server dump: with a
/// Manifest, the tables its `Table` lines list, in that order (P8);
/// without one (an `Export.mdf` on its own, P6), every user table of
/// the four schemas in role and name order.
pub(super) fn write_sql_server_dump(
    conn: &Connection,
    db: &MdfDatabase,
    tables: &[TableInfo],
    manifest: Option<&Manifest>,
    schemas: &[DumpSchema],
    summary: &mut StoreSummary,
) -> Result<(), BackupStoreError> {
    let dump_schema_names: BTreeSet<&str> = tables
        .iter()
        .map(|table| table.schema_name.as_str())
        .collect();
    for schema in schemas {
        if !dump_schema_names.contains(schema.dump_name.as_str()) {
            return Err(BackupStoreError::DumpShape {
                what: "the Manifest names a schema the dump does not hold",
                name: schema.schema_name.clone(),
            });
        }
    }
    write_schemas_and_views(conn, manifest, schemas)?;

    let selected: Vec<(&DumpSchema, &TableInfo)> = match manifest {
        Some(manifest) => {
            let mut selected = Vec::new();
            for entry in manifest.tables() {
                let schema = DumpSchema::of_manifest_name(
                    schemas,
                    &entry.database,
                    "a Table line names a schema outside the four plant schemas",
                )?;
                let table = tables
                    .iter()
                    .find(|table| table.schema_name == schema.dump_name && table.name == entry.name)
                    .ok_or_else(|| BackupStoreError::DumpShape {
                        what: "the Manifest lists a table the dump does not hold",
                        name: format!("{}.{}", entry.database, entry.name),
                    })?;
                selected.push((schema, table));
            }
            selected
        }
        None => {
            let mut selected: Vec<(&DumpSchema, &TableInfo)> = tables
                .iter()
                .filter_map(|table| {
                    schemas
                        .iter()
                        .find(|schema| schema.dump_name == table.schema_name)
                        .map(|schema| (schema, table))
                })
                .collect();
            selected.sort_by(|(a_schema, a), (b_schema, b)| {
                a_schema
                    .role
                    .cmp(&b_schema.role)
                    .then_with(|| a.name.cmp(&b.name))
            });
            selected
        }
    };
    for (schema, table) in selected {
        write_table(conn, db, schema.role, table, summary)?;
    }
    Ok(())
}

/// Creates `<role>__<table>`, fills it from the dump and records the
/// table, its columns, its Ghost Rows and its LOBs.
fn write_table(
    conn: &Connection,
    db: &MdfDatabase,
    role: SchemaRole,
    table: &TableInfo,
    summary: &mut StoreSummary,
) -> Result<(), BackupStoreError> {
    let store_name = role.store_table_name(&table.name);
    for column in &table.columns {
        if column.name == SRC_PAGE_COLUMN || column.name == SRC_SLOT_COLUMN {
            return Err(BackupStoreError::DumpShape {
                what: "a source column is named like the provenance columns",
                name: format!("{}.{}", store_name, column.name),
            });
        }
    }

    let mut columns_sql: Vec<String> = table
        .columns
        .iter()
        .map(|column| {
            format!(
                "{} {}",
                quote(&column.name),
                sqlite_type_of(&column.type_name)
            )
        })
        .collect();
    columns_sql.push(format!("{} INTEGER NOT NULL", quote(SRC_PAGE_COLUMN)));
    columns_sql.push(format!("{} INTEGER NOT NULL", quote(SRC_SLOT_COLUMN)));
    conn.execute_batch(&format!(
        "CREATE TABLE {} ({});",
        quote(&store_name),
        columns_sql.join(", ")
    ))?;

    let mut insert_column = conn.prepare(
        "INSERT INTO dump_column (role, table_name, ordinal, name, source_type, length, \
         precision, scale, nullable) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for (ordinal, column) in table.columns.iter().enumerate() {
        let ColumnInfo {
            name,
            type_name,
            max_length,
            precision,
            scale,
            nullable,
        } = column;
        insert_column.execute(params![
            role.as_str(),
            table.name,
            ordinal as i64 + 1,
            name,
            type_name,
            i64::from(*max_length),
            i64::from(*precision),
            i64::from(*scale),
            i64::from(*nullable),
        ])?;
    }

    let placeholders = (1..=table.columns.len() + 2)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let column_list = table
        .columns
        .iter()
        .map(|column| quote(&column.name))
        .chain([quote(SRC_PAGE_COLUMN), quote(SRC_SLOT_COLUMN)])
        .collect::<Vec<_>>()
        .join(", ");
    let mut insert_row = conn.prepare(&format!(
        "INSERT INTO {} ({column_list}) VALUES ({placeholders})",
        quote(&store_name)
    ))?;
    let mut insert_ghost = conn.prepare(
        "INSERT INTO dump_ghost_row (role, table_name, page, slot, record_type, bytes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut insert_lob = conn.prepare(
        "INSERT INTO dump_lob (role, table_name, row_page, row_slot, column_name, root_page, \
         root_slot, length, sha256) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;

    let mut rows = 0i64;
    let mut ghost_rows = 0i64;
    for record in db.scan_table(table)? {
        match record? {
            ScannedRecord::Live {
                page_id,
                slot,
                row,
                lobs,
            } => {
                let mut values: Vec<SqlValue> = table
                    .columns
                    .iter()
                    .map(|column| row.value(&column.name).map_or(SqlValue::Null, store_value))
                    .collect();
                values.push(SqlValue::Integer(i64::from(page_id)));
                values.push(SqlValue::Integer(i64::from(slot)));
                insert_row.execute(params_from_iter(values.iter()))?;
                rows += 1;
                for lob in lobs {
                    // The hash is over the bytes as stored: an ntext value's
                    // text re-encoded is not them, so it is hashed as UTF-8.
                    let sha256 = match row.value(&lob.column) {
                        Some(Value::Binary(bytes)) => sha256_hex(bytes),
                        Some(Value::String(text)) => sha256_hex(text.as_bytes()),
                        _ => continue,
                    };
                    insert_lob.execute(params![
                        role.as_str(),
                        table.name,
                        i64::from(page_id),
                        i64::from(slot),
                        lob.column,
                        i64::from(lob.root_page),
                        i64::from(lob.root_slot),
                        i64::try_from(lob.length).unwrap_or(i64::MAX),
                        sha256,
                    ])?;
                    summary.lobs += 1;
                }
            }
            ScannedRecord::Ghost(ghost) => {
                insert_ghost.execute(params![
                    role.as_str(),
                    table.name,
                    i64::from(ghost.page_id),
                    i64::from(ghost.slot),
                    i64::from(ghost.record_type),
                    ghost.bytes,
                ])?;
                ghost_rows += 1;
            }
        }
    }

    conn.execute(
        "INSERT INTO dump_table (role, source_name, store_name, decoded, expected_rows, rows, \
         ghost_rows) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
        params![
            role.as_str(),
            table.name,
            store_name,
            table.rcrows,
            rows,
            ghost_rows
        ],
    )?;
    summary.tables += 1;
    summary.rows += u64::try_from(rows).unwrap_or(0);
    summary.ghost_rows += u64::try_from(ghost_rows).unwrap_or(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn values_go_in_by_source_type() {
        assert_eq!(SqlValue::Null, store_value(&Value::Null));
        assert_eq!(
            SqlValue::Text(String::new()),
            store_value(&Value::String(String::new()))
        );
        assert_eq!(SqlValue::Integer(250), store_value(&Value::Int(250)));
        assert_eq!(SqlValue::Real(250.0), store_value(&Value::Float(250.0)));
        assert_eq!(
            SqlValue::Text("2026-04-20 10:32:46.007".to_string()),
            store_value(&Value::DateTime(
                Utc.with_ymd_and_hms(2026, 4, 20, 10, 32, 46).unwrap()
                    + chrono::Duration::milliseconds(7)
            ))
        );
        assert_eq!(
            SqlValue::Blob(vec![0x50, 0x4B]),
            store_value(&Value::Binary(vec![0x50, 0x4B]))
        );
    }

    #[test]
    fn sqlite_types_follow_the_source_type() {
        assert_eq!("TEXT", sqlite_type_of("nvarchar"));
        assert_eq!("INTEGER", sqlite_type_of("int"));
        assert_eq!("REAL", sqlite_type_of("float"));
        assert_eq!("TEXT", sqlite_type_of("datetime"));
        assert_eq!("BLOB", sqlite_type_of("image"));
    }

    #[test]
    fn roles_name_store_tables() {
        assert_eq!(
            "pid__T_Drawing",
            SchemaRole::Pid.store_table_name("T_Drawing")
        );
        assert_eq!(
            Some(SchemaRole::PidDictionary),
            SchemaRole::from_type_code("9")
        );
        assert_eq!(None, SchemaRole::from_type_code("1"));
    }
}
