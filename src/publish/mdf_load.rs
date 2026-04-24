//! Load Publish XML source rows directly from a SQL Server MDF file
//! using the vendored `oxidized-mdf` Rust reader.
//!
//! The existing publish loader already knows how to turn a table set
//! shaped like SmartPlant's SQL tables into [`super::model::PublishDrawing`].
//! This module keeps that contract without a C# probe step: rows are
//! read from MDF and staged into an in-memory SQLite connection, then
//! the proven `sqlite_load` path builds the DTO.

use std::path::Path;

use async_std::task;
use futures_lite::stream::StreamExt;
use oxidized_mdf::{MdfDatabase, Value};
use rusqlite::{params_from_iter, Connection};

use super::model::{PublishDrawing, PublishError};
use super::sqlite_load;

const PUBLISH_TABLES: &[&str] = &[
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
/// SmartPlant tables into an in-memory SQLite connection, and return
/// that connection for reuse by the existing query loader.
pub fn open_mdf_as_sqlite(path: &Path) -> Result<Connection, PublishError> {
    task::block_on(open_mdf_as_sqlite_async(path))
}

/// Load one drawing graph directly from an MDF file.
pub fn load_drawing_graph_from_mdf(
    path: &Path,
    drawing_uid: &str,
) -> Result<PublishDrawing, PublishError> {
    let conn = open_mdf_as_sqlite(path)?;
    sqlite_load::load_drawing_graph(&conn, drawing_uid)
}

async fn open_mdf_as_sqlite_async(path: &Path) -> Result<Connection, PublishError> {
    let conn = Connection::open_in_memory()?;
    for table_name in PUBLISH_TABLES {
        stage_table(path, &conn, table_name).await?;
    }
    Ok(conn)
}

async fn stage_table(
    path: &Path,
    conn: &Connection,
    table_name: &str,
) -> Result<(), PublishError> {
    // `oxidized-mdf`'s page reader is forward-only. Re-opening per
    // table is slower but avoids cross-table page-order coupling and
    // keeps this adapter deterministic for small publish exports.
    let mut db = MdfDatabase::open(path).await?;
    let Some(columns) = db.column_names(table_name) else {
        return Ok(());
    };
    if columns.is_empty() {
        return Ok(());
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

    let Some(mut rows) = db.rows(table_name) else {
        return Ok(());
    };
    let mut stmt = conn.prepare(&insert_sql)?;
    while let Some(row) = rows.next().await {
        let values = columns
            .iter()
            .map(|column| row.value(column).cloned().and_then(value_to_text))
            .collect::<Vec<_>>();
        stmt.execute(params_from_iter(values.iter()))?;
    }

    Ok(())
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
