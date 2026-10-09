//! The Oracle Database Dump in the store (S2d): the 154 tables the
//! Manifest lists, created empty from the `CREATE TABLE` statements of
//! the `exp` dump and marked undecoded (`dump_table.decoded = 0`,
//! `expected_rows` NULL), with `dump_schema`, `dump_column` and
//! `dump_view` filled as for SQL Server. Rows are in exp's binary
//! format and wait for the second version (Q3).
//!
//! Each statement belongs to the `CONNECT <OWNER>` line before it
//! (P12); an owner is matched to a Manifest schema by an ASCII
//! case-insensitive comparison with `PlantConnInfo` field 4, and
//! `dump_schema.schema_name` keeps the dump's upper-case spelling
//! (Q20). Column types follow Q19: `NVARCHAR2` / `VARCHAR2` TEXT,
//! `NUMBER(p, 0)` INTEGER, `FLOAT(126)` REAL, `DATE` TEXT, `BLOB`
//! BLOB; `NOT NULL` is recorded in `dump_column` only. The tables get
//! the same `_src_page` / `_src_slot` columns as SQL Server's so every
//! dumped table has one shape; an Oracle dump has no pages, so they
//! admit NULL and the second version will say what goes there.

use std::collections::BTreeSet;

use rusqlite::{params, Connection};

use crate::backup::oracle_exp::{scan_create_tables, ExpColumn, ExpTable};
use crate::backup::Manifest;

use super::mssql::{quote, SRC_PAGE_COLUMN, SRC_SLOT_COLUMN};
use super::{write_schemas_and_views, BackupStoreError, DumpSchema, StoreSummary};

/// The SQLite type a column of an Oracle table is declared with
/// (Q19), from its type name and arguments as `exp` wrote them.
///
/// `NUMBER` with a scale of 0 (written or implied by a precision
/// alone) is INTEGER; a `NUMBER` with another scale or no arguments
/// is TEXT, so a value of either keeps its digits when rows arrive.
/// `FLOAT`, `BINARY_FLOAT`, `BINARY_DOUBLE`, `REAL` and `DOUBLE` are
/// REAL; `BLOB`, `RAW` and `LONG RAW` BLOB; `INTEGER` / `INT` /
/// `SMALLINT` INTEGER; everything else (`NVARCHAR2`, `VARCHAR2`,
/// `CHAR`, `CLOB`, `DATE`, `TIMESTAMP`, ...) TEXT.
pub fn sqlite_type_of_oracle(type_name: &str, arguments: &[&str]) -> &'static str {
    match type_name.to_ascii_uppercase().as_str() {
        "NUMBER" | "NUMERIC" | "DECIMAL" | "DEC" => match arguments {
            [_precision] => "INTEGER",
            [_precision, scale] if scale.trim() == "0" => "INTEGER",
            _ => "TEXT",
        },
        "INTEGER" | "INT" | "SMALLINT" => "INTEGER",
        "FLOAT" | "BINARY_FLOAT" | "BINARY_DOUBLE" | "REAL" | "DOUBLE" => "REAL",
        "BLOB" | "RAW" | "LONG RAW" => "BLOB",
        _ => "TEXT",
    }
}

/// `dump_column`'s length, precision and scale of an Oracle column:
/// the type's arguments where they mean that -- a character or raw
/// length for the string and `RAW` types, precision and scale for
/// `NUMBER`, precision for `FLOAT` -- and NULL otherwise.
fn length_precision_scale(column: &ExpColumn) -> (Option<i64>, Option<i64>, Option<i64>) {
    let arguments = column.type_arguments();
    let leading_integer =
        |text: &str| -> Option<i64> { text.split_whitespace().next()?.parse().ok() };
    match column.type_name().to_ascii_uppercase().as_str() {
        "NVARCHAR2" | "VARCHAR2" | "VARCHAR" | "CHAR" | "NCHAR" | "RAW" => (
            arguments.first().and_then(|a| leading_integer(a)),
            None,
            None,
        ),
        "NUMBER" | "NUMERIC" | "DECIMAL" | "DEC" => match arguments.as_slice() {
            [precision] => (None, leading_integer(precision), Some(0)),
            [precision, scale] => (None, leading_integer(precision), leading_integer(scale)),
            _ => (None, None, None),
        },
        "FLOAT" => (
            None,
            arguments.first().and_then(|a| leading_integer(a)),
            None,
        ),
        _ => (None, None, None),
    }
}

/// Writes the catalogue of an Oracle `exp` dump and creates every table
/// the Manifest lists, empty; returns how many.
pub(super) fn write_oracle_dump(
    conn: &Connection,
    dump: &[u8],
    manifest: &Manifest,
    schemas: &[DumpSchema],
    summary: &mut StoreSummary,
) -> Result<usize, BackupStoreError> {
    let tables = scan_create_tables(dump)?;
    let owners: BTreeSet<&str> = tables.iter().map(|table| table.owner.as_str()).collect();

    // P12: each Manifest schema is one CONNECT owner, compared without
    // regard to ASCII case; the dump's spelling is what the store keeps.
    let mut schemas = schemas.to_vec();
    for schema in &mut schemas {
        let matches: Vec<&str> = owners
            .iter()
            .copied()
            .filter(|owner| owner.eq_ignore_ascii_case(&schema.schema_name))
            .collect();
        match matches.as_slice() {
            [owner] => schema.dump_name = (*owner).to_string(),
            [] => {
                return Err(BackupStoreError::DumpShape {
                    what: "the Manifest names a schema no CONNECT line of the dump spells",
                    name: schema.schema_name.clone(),
                })
            }
            _ => {
                return Err(BackupStoreError::DumpShape {
                    what: "two CONNECT owners of the dump spell the same Manifest schema",
                    name: schema.schema_name.clone(),
                })
            }
        }
    }
    write_schemas_and_views(conn, Some(manifest), &schemas)?;

    let mut written = 0usize;
    let mut listed: BTreeSet<(&str, &str)> = BTreeSet::new();
    for entry in manifest.tables() {
        let schema = DumpSchema::of_manifest_name(
            &schemas,
            &entry.database,
            "a Table line names a schema outside the four plant schemas",
        )?;
        let table = tables
            .iter()
            .find(|table| table.owner == schema.dump_name && table.name == entry.name)
            .ok_or_else(|| BackupStoreError::DumpShape {
                what: "the Manifest lists a table the dump's DDL does not hold",
                name: format!("{}.{}", entry.database, entry.name),
            })?;
        listed.insert((table.owner.as_str(), table.name.as_str()));
        write_empty_table(conn, schema, table)?;
        written += 1;
    }
    summary.tables += written;

    let unlisted = tables
        .iter()
        .filter(|table| {
            schemas.iter().any(|schema| schema.dump_name == table.owner)
                && !listed.contains(&(table.owner.as_str(), table.name.as_str()))
        })
        .count();
    if unlisted > 0 {
        summary.warnings.push(format!(
            "the dump's DDL creates {unlisted} tables under the four plant schemas that the \
             Manifest does not list; they are not registered"
        ));
    }
    Ok(written)
}

/// Creates `<role>__<TABLE>` empty, typed by Q19, and records the
/// table and its columns.
fn write_empty_table(
    conn: &Connection,
    schema: &DumpSchema,
    table: &ExpTable,
) -> Result<(), BackupStoreError> {
    let store_name = schema.role.store_table_name(&table.name);
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
                sqlite_type_of_oracle(column.type_name(), &column.type_arguments())
            )
        })
        .collect();
    columns_sql.push(format!("{} INTEGER", quote(SRC_PAGE_COLUMN)));
    columns_sql.push(format!("{} INTEGER", quote(SRC_SLOT_COLUMN)));
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
        let (length, precision, scale) = length_precision_scale(column);
        insert_column.execute(params![
            schema.role.as_str(),
            table.name,
            ordinal as i64 + 1,
            column.name,
            column.type_spec,
            length,
            precision,
            scale,
            i64::from(!column.not_null()),
        ])?;
    }
    conn.execute(
        "INSERT INTO dump_table (role, source_name, store_name, decoded, expected_rows, rows, \
         ghost_rows) VALUES (?1, ?2, ?3, 0, NULL, 0, 0)",
        params![schema.role.as_str(), table.name, store_name],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(type_spec: &str, modifiers: &str) -> ExpColumn {
        ExpColumn {
            name: "C".to_string(),
            type_spec: type_spec.to_string(),
            modifiers: modifiers.to_string(),
        }
    }

    #[test]
    fn oracle_types_map_by_q19() {
        let of = |spec: &str| {
            let column = column(spec, "");
            sqlite_type_of_oracle(column.type_name(), &column.type_arguments())
        };
        assert_eq!("TEXT", of("NVARCHAR2(32)"));
        assert_eq!("TEXT", of("VARCHAR2(4000)"));
        assert_eq!("INTEGER", of("NUMBER(11, 0)"));
        assert_eq!("INTEGER", of("NUMBER(10)"));
        assert_eq!("TEXT", of("NUMBER(10, 2)"));
        assert_eq!("TEXT", of("NUMBER"));
        assert_eq!("REAL", of("FLOAT(126)"));
        assert_eq!("REAL", of("BINARY_DOUBLE"));
        assert_eq!("TEXT", of("DATE"));
        assert_eq!("TEXT", of("TIMESTAMP(6)"));
        assert_eq!("BLOB", of("BLOB"));
        assert_eq!("BLOB", of("RAW(16)"));
        assert_eq!("TEXT", of("CLOB"));
    }

    #[test]
    fn length_precision_and_scale_come_from_the_type_arguments() {
        assert_eq!(
            (Some(32), None, None),
            length_precision_scale(&column("NVARCHAR2(32)", "NOT NULL ENABLE"))
        );
        assert_eq!(
            (Some(4000), None, None),
            length_precision_scale(&column("VARCHAR2(4000 BYTE)", ""))
        );
        assert_eq!(
            (None, Some(11), Some(0)),
            length_precision_scale(&column("NUMBER(11, 0)", ""))
        );
        assert_eq!(
            (None, Some(10), Some(0)),
            length_precision_scale(&column("NUMBER(10)", ""))
        );
        assert_eq!(
            (None, None, None),
            length_precision_scale(&column("NUMBER", ""))
        );
        assert_eq!(
            (None, Some(126), None),
            length_precision_scale(&column("FLOAT(126)", ""))
        );
        assert_eq!(
            (None, None, None),
            length_precision_scale(&column("DATE", ""))
        );
        assert_eq!(
            (None, None, None),
            length_precision_scale(&column("BLOB", ""))
        );
    }
}
