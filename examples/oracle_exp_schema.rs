//! One-shot diagnostic: list every `CREATE TABLE` statement an Oracle
//! `exp`-format `.dmp` carries as plain text, grouped by the `CONNECT`
//! owner each one sits under (`pid_parse::backup::oracle_exp`, P12 of
//! the Backup Store plan). Useful for comparing the DWG / SQPlant
//! schemas against TEST02 without running Oracle's `imp` tool.
//!
//! Output: a header per owner, then one line per table →
//! `  TABLE_NAME (n cols): COL1 TYPE1, COL2 TYPE2 NOT NULL, ...`
//!
//! Only the **DDL** is read. Row-level data is in Oracle's proprietary
//! binary `exp` row format and is **not** decoded (the Backup Store's
//! first version registers these tables empty; see Q3 of the plan).
//!
//! Usage:
//!   cargo run --example oracle_exp_schema -- path/to/Export.dmp

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_parse::backup::oracle_exp::{scan_create_tables, ExpTable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Usage: oracle_exp_schema <Export.dmp>");
        std::process::exit(2);
    };
    let path = PathBuf::from(path);
    let bytes = std::fs::read(&path)?;
    eprintln!(
        "{} bytes ({:.2} MB)",
        bytes.len(),
        bytes.len() as f64 / 1_048_576.0
    );

    let tables = scan_create_tables(&bytes)?;
    let mut by_owner: BTreeMap<&str, Vec<&ExpTable>> = BTreeMap::new();
    for table in &tables {
        by_owner
            .entry(table.owner.as_str())
            .or_default()
            .push(table);
    }
    let distinct_names = tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    eprintln!(
        "Found {} CREATE TABLE statements under {} owners ({} distinct table names)",
        tables.len(),
        by_owner.len(),
        distinct_names
    );

    for (owner, tables) in &by_owner {
        let columns: usize = tables.iter().map(|table| table.columns.len()).sum();
        println!("[{owner}] {} tables, {columns} columns", tables.len());
        let mut tables = tables.clone();
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        for table in tables {
            let column_list = table
                .columns
                .iter()
                .map(|column| {
                    if column.not_null() {
                        format!("{} {} NOT NULL", column.name, column.type_spec)
                    } else {
                        format!("{} {}", column.name, column.type_spec)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  {} ({} cols): {column_list}",
                table.name,
                table.columns.len()
            );
        }
    }
    Ok(())
}
