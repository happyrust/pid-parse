//! One-shot diagnostic: scan an Oracle `exp`-format `.dmp` file and
//! list every `CREATE TABLE` statement found inside. Useful for
//! comparing the DWG-fixture schema against TEST02 without running
//! Oracle's `imp` tool.
//!
//! Output: one line per table → `TABLE_NAME: col1 (type1), col2 (type2), ...`
//!
//! Note: this scanner only extracts the **DDL** (CREATE TABLE
//! statements as plain ASCII). Row-level data is in Oracle's
//! proprietary binary `exp` row format and is **not** decoded
//! here; recovering it would require an Oracle exp parser proper
//! (or running `imp` against a live Oracle server).
//!
//! Usage:
//!   cargo run --example oracle_exp_schema -- path/to/Export.dmp

use std::collections::BTreeMap;
use std::path::PathBuf;

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

    // Oracle exp embeds CREATE TABLE as plain ASCII surrounded by
    // newlines. Find every "\nCREATE TABLE " offset and walk until
    // the matching ")  PCTFREE" / "TABLESPACE" close.
    let needle = b"\nCREATE TABLE \"";
    let mut tables: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut pos = 0usize;
    while let Some(off) = find_subslice(&bytes[pos..], needle) {
        let start = pos + off + 1;
        let line_end = find_subslice(&bytes[start..], b"\n")
            .map(|o| start + o)
            .unwrap_or(bytes.len());
        if line_end <= start {
            pos = start + 1;
            continue;
        }
        let line = std::str::from_utf8(&bytes[start..line_end]).unwrap_or("");
        if let Some((table_name, cols)) = parse_create_table(line) {
            tables.insert(table_name, cols);
        }
        pos = line_end;
    }
    eprintln!("Found {} CREATE TABLE statements", tables.len());

    // Print T_* tables first, then others.
    let mut t_tables: Vec<&String> = tables.keys().filter(|n| n.starts_with("T_")).collect();
    t_tables.sort();
    for name in &t_tables {
        let cols = tables.get(*name).unwrap();
        let col_str = cols
            .iter()
            .map(|(c, t)| format!("{c}:{t}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("[T_] {name} ({} cols): {col_str}", cols.len());
    }
    let other: Vec<&String> = tables.keys().filter(|n| !n.starts_with("T_")).collect();
    if !other.is_empty() {
        eprintln!();
        eprintln!("Other tables (non-T_):");
        for name in &other {
            eprintln!("  {name} ({} cols)", tables[*name].len());
        }
    }
    Ok(())
}

fn parse_create_table(line: &str) -> Option<(String, Vec<(String, String)>)> {
    // Expected shape:
    //   CREATE TABLE "NAME" ("COL1" TYPE1, "COL2" TYPE2, ...)  PCTFREE ...
    let rest = line.strip_prefix("CREATE TABLE \"")?;
    let close_name = rest.find('"')?;
    let name = rest[..close_name].to_string();
    let after_name = &rest[close_name + 1..];
    let paren_open = after_name.find('(')?;
    // Find the matching closing paren of the column list. Oracle nests
    // parens inside type specs (e.g. NUMBER(10, 0)), so we walk.
    let inside_start = paren_open + 1;
    let bytes = after_name.as_bytes();
    let mut depth = 1i32;
    let mut idx = inside_start;
    while idx < bytes.len() {
        match bytes[idx] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        idx += 1;
    }
    if depth != 0 {
        return None;
    }
    let cols_str = &after_name[inside_start..idx];
    // Now split on top-level commas (paren-aware again).
    let mut cols: Vec<(String, String)> = Vec::new();
    let mut last = 0usize;
    let cb = cols_str.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in cb.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                push_col(&cols_str[last..i], &mut cols);
                last = i + 1;
            }
            _ => {}
        }
    }
    push_col(&cols_str[last..], &mut cols);
    Some((name, cols))
}

fn push_col(piece: &str, out: &mut Vec<(String, String)>) {
    let piece = piece.trim();
    if piece.is_empty() {
        return;
    }
    let after_q = match piece.strip_prefix('"') {
        Some(s) => s,
        None => return,
    };
    let Some(end_q) = after_q.find('"') else {
        return;
    };
    let col = after_q[..end_q].to_string();
    let rest = after_q[end_q + 1..].trim();
    // Type spec may contain spaces (NOT NULL ENABLE etc). Keep until
    // first comma or end.
    let ty = rest.trim().to_string();
    out.push((col, ty));
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
