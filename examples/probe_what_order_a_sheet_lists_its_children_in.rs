//! What order does a sheet list its children in — and is the list the
//! sheet's own?
//!
//! The census behind
//! `docs/analysis/2026-11-11-a-sheet-draws-its-children-in-the-order-it-lists-them.md`.
//! For every top-level `Sheet<N>` stream of every drawing it is given, it
//! reads the `0x0114 JSheet` record of the root `PSMcluster0`
//! (`parsers::sheet_display_order`) and prints one line:
//!
//! | column | what |
//! |---|---|
//! | `listed` | children the record lists |
//! | `owned` | live records of the sheet stream whose parent is the sheet |
//! | `both` | children that are such records — `listed = owned = both` is a list that names exactly the sheet's own records |
//! | `scale` | the `f64` at payload `+12`, the reach of a child's range box in metres |
//! | `paper` | the sheet's paper as the record's tail states it: height, a size code, width |
//! | `frame at` | the place of the border frame (`0x003D`) in the list, `-` for a sheet with none |
//! | `first` | the first child: type code and oid |
//! | `rising` | adjacent entries whose oids rise — a list in oid order reads `n-1 of n-1` |
//! | `ranked` | live records of the stream the list reaches, of all its live records |
//!
//! A stream the cluster holds no table for is printed as such; on the
//! corpus those are extra `Sheet<N>` streams that own no record.
//!
//! The paper is not part of what `sheet_display_order` decodes. It is read
//! here from the 164 bytes behind the entries — two counted UTF-16 names,
//! then at `+102` an `f64` height, at `+110` a `u32`, at `+114` an `f64`
//! width — because the eleven reference plots are printed on exactly that
//! paper, the A2 sheet included, whose border template is 0.33 × 0.57 mm
//! larger.
//!
//! Run (PowerShell):
//!
//! ```powershell
//! cargo run --example probe_what_order_a_sheet_lists_its_children_in -- <drawing.pid | directory> [...]
//! ```
//!
//! A directory is walked for `.pid` files. Read-only. Exits non-zero when a
//! sheet that owns records has no table, or a table that does not name
//! exactly the sheet's records.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::parsers::sheet_display_order::{
    find_sheet_child_table, sheet_display_ranks, SheetChildTable, SHEET_CHILD_ENTRY_LEN,
    SHEET_CHILD_TABLE_AT,
};
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use pid_parse::PidPackage;

/// The border frame's type code, `igSmartFrame2d`.
const SMART_FRAME: u16 = 0x003D;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
    {
        out.push(path.to_path_buf());
    }
}

/// One live record of a sheet stream: `(type code, oid, parent)`.
fn live_records(sheet: &[u8]) -> Vec<(u16, u32, u32)> {
    let mut records = Vec::new();
    for at in sheet_record_starts(sheet) {
        let Some(header) = parse_live_psm_header(sheet, at) else {
            continue;
        };
        let (Some(oid), Some(parent)) = (
            u32_at(sheet, header.body_start),
            u32_at(sheet, header.body_start + 4),
        ) else {
            continue;
        };
        records.push((header.type_code, oid, parent));
    }
    records
}

/// The paper the record's tail states, `(height m, size code, width m)`.
///
/// The table is looked up again by its bytes: the decoder keeps the entries
/// and not what follows them.
fn paper(cluster: &[u8], table: &SheetChildTable) -> Option<(f64, u32, f64)> {
    let needle = table.sheet_oid.to_le_bytes();
    let entries_end = SHEET_CHILD_TABLE_AT + table.children.len() * SHEET_CHILD_ENTRY_LEN;
    let count = u32::try_from(table.children.len()).ok()?.to_le_bytes();
    let mut from = 0usize;
    while let Some(found) = cluster
        .get(from..)?
        .windows(4)
        .position(|window| window == needle)
    {
        let body = from + found;
        from = body + 1;
        let Some(header) = body
            .checked_sub(6)
            .and_then(|envelope| parse_live_psm_header(cluster, envelope))
        else {
            continue;
        };
        if header.type_code != 0x0114 || cluster.get(body + 24..body + 28) != Some(&count[..]) {
            continue;
        }
        let tail = body + entries_end;
        if tail + 122 > body + header.bytes_to_follow as usize {
            continue;
        }
        return Some((
            f64_at(cluster, tail + 102)?,
            u32_at(cluster, tail + 110)?,
            f64_at(cluster, tail + 114)?,
        ));
    }
    None
}

fn main() -> ExitCode {
    let mut drawings = Vec::new();
    for arg in std::env::args().skip(1) {
        collect(Path::new(&arg), &mut drawings);
    }
    if drawings.is_empty() {
        eprintln!(
            "usage: probe_what_order_a_sheet_lists_its_children_in <drawing.pid | directory> [...]"
        );
        return ExitCode::from(2);
    }
    println!(
        "drawing | sheet | listed | owned | both | scale | paper | frame at | first | rising | ranked"
    );
    let (mut sheets, mut tables, mut exact, mut children, mut bare) = (0, 0, 0, 0usize, 0);
    let mut failed = false;
    for path in &drawings {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(package) = PidPackage::from_path(path) else {
            println!("{name} | does not parse");
            failed = true;
            continue;
        };
        let Some(cluster) = package.get_stream("/PSMcluster0") else {
            println!("{name} | no /PSMcluster0");
            failed = true;
            continue;
        };
        for (stream, raw) in &package.streams {
            let Some(sheet_oid) = stream
                .strip_prefix("/Sheet")
                .and_then(|oid| oid.parse::<u32>().ok())
            else {
                continue;
            };
            sheets += 1;
            let records = live_records(&raw.data);
            let owned: BTreeSet<u32> = records
                .iter()
                .filter(|(_, _, parent)| *parent == sheet_oid)
                .map(|(_, oid, _)| *oid)
                .collect();
            // The first table that decodes for this oid and names a record of
            // the stream — a looser test than the parser's, so that a table
            // that is not exactly the sheet's is seen and counted.
            let all: BTreeSet<u32> = records.iter().map(|(_, oid, _)| *oid).collect();
            let table = find_sheet_child_table(&cluster.data, sheet_oid, |table| {
                table.children.iter().any(|child| all.contains(&child.oid))
            });
            let Some(table) = table else {
                println!(
                    "{name} | {sheet_oid} | no table | {} | - | - | - | - | - | - | 0 of {}",
                    owned.len(),
                    records.len()
                );
                if owned.is_empty() {
                    bare += 1;
                } else {
                    failed = true;
                }
                continue;
            };
            tables += 1;
            children += table.children.len();
            let both = table
                .children
                .iter()
                .filter(|child| owned.contains(&child.oid))
                .count();
            if both == table.children.len() && both == owned.len() {
                exact += 1;
            } else {
                failed = true;
            }
            let kind = |oid: u32| {
                records
                    .iter()
                    .find(|(_, record, _)| *record == oid)
                    .map(|(code, _, _)| *code)
            };
            let frame = table
                .children
                .iter()
                .position(|child| kind(child.oid) == Some(SMART_FRAME))
                .map_or_else(|| "-".to_string(), |place| place.to_string());
            let first = table.children.first().map_or_else(
                || "-".to_string(),
                |child| format!("{:#06x} oid {}", kind(child.oid).unwrap_or(0), child.oid),
            );
            let rising = table
                .children
                .windows(2)
                .filter(|pair| pair[1].oid > pair[0].oid)
                .count();
            let paper = paper(&cluster.data, &table).map_or_else(
                || "-".to_string(),
                |(height, code, width)| format!("{height} x {width} m, code {code}"),
            );
            let ranked = sheet_display_ranks(&raw.data, &table).len();
            println!(
                "{name} | {sheet_oid} | {} | {} | {both} | {} | {paper} | {frame} | {first} | {rising} of {} | {ranked} of {}",
                table.children.len(),
                owned.len(),
                table.range_scale,
                table.children.len().saturating_sub(1),
                records.len()
            );
        }
    }
    println!(
        "in all: {} drawing(s), {sheets} sheet stream(s), {tables} table(s), {exact} naming exactly \
         the sheet's records, {children} children; {bare} stream(s) with no table own no record",
        drawings.len()
    );
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
