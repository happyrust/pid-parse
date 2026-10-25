//! Survey which sheet of each `.sym` body `displayed_primitives` picks, and
//! whether the picker throws away the sheet that holds the strokes.
//!
//! The picker prefers a sheet whose `Top ViewFilterSet` is the normal view
//! (no supplementary layer shown), then `Sheet6`, then sheets no view set
//! governs. A body that keeps its strokes on a single sheet whose only view
//! set shows everything has no *normal* sheet at all, so the `Sheet6`
//! fallback decides — and `Sheet6` there can be a one-stroke stub while the
//! real body sits on the sheet that view set governs.
//!
//! ```text
//! cargo run --example probe_sym_sheet_choice -- <symbol library root>...
//! ```
//!
//! Prints a census of the choice shapes, then every file where the pick holds
//! fewer strokes than another sheet of the same body. Read-only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::symbol_library::{read_symbol_geometry, SymbolSheetView};

const HIDDEN: &[&str] = &[
    "heattrace",
    "hiddenobjects",
    "jacket",
    "label",
    "construction",
    "dimension",
    "deminsion",
];

fn hidden(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    HIDDEN.contains(&normalized.as_str())
}

fn bit_is_set(bitmap: &[u8], n: u16) -> bool {
    bitmap
        .get(usize::from(n / 8))
        .is_some_and(|byte| byte & (1 << (n % 8)) != 0)
}

fn supplementary(view: &SymbolSheetView) -> bool {
    view.layers
        .iter()
        .any(|(number, name)| hidden(name) && bit_is_set(&view.display, *number))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "sym") {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        eprintln!("usage: probe_sym_sheet_choice <symbol library root>...");
        return ExitCode::from(2);
    }
    let mut files = Vec::new();
    for root in &roots {
        collect(root, &mut files);
    }

    let mut shapes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut lost: Vec<(PathBuf, usize, usize, Vec<u32>)> = Vec::new();
    let mut read = 0usize;
    for path in &files {
        let Ok(body) = read_symbol_geometry(path) else {
            continue;
        };
        read += 1;
        if body.primitives.is_empty() {
            continue;
        }
        let view_of = |sheet: u32| body.sheet_views.iter().find(|v| v.sheet_ref == sheet);
        let mut per_sheet: BTreeMap<u32, usize> = BTreeMap::new();
        for styled in &body.primitives {
            *per_sheet.entry(styled.sheet_ref).or_default() += 1;
        }
        let sheets: BTreeSet<u32> = body
            .sheet_views
            .iter()
            .map(|view| view.sheet_ref)
            .chain(per_sheet.keys().copied())
            .collect();
        let normal: BTreeSet<u32> = sheets
            .iter()
            .copied()
            .filter(|sheet| view_of(*sheet).is_some_and(|view| !supplementary(view)))
            .collect();
        let alternate: BTreeSet<u32> = sheets
            .iter()
            .copied()
            .filter(|sheet| view_of(*sheet).is_some_and(supplementary))
            .collect();
        let selection: BTreeSet<u32> = if !normal.is_empty() {
            *shapes.entry("normal view").or_insert(0) += 1;
            normal.clone()
        } else if sheets.contains(&6) {
            *shapes.entry("Sheet6 fallback").or_insert(0) += 1;
            [6].into_iter().collect()
        } else if alternate != sheets {
            *shapes.entry("alternate complement").or_insert(0) += 1;
            sheets.difference(&alternate).copied().collect()
        } else {
            *shapes.entry("everything").or_insert(0) += 1;
            sheets.clone()
        };
        let picked: usize = selection
            .iter()
            .map(|sheet| per_sheet.get(sheet).copied().unwrap_or(0))
            .sum();
        let best = per_sheet.values().copied().max().unwrap_or(0);
        if picked < best {
            lost.push((
                path.clone(),
                picked,
                best,
                selection.iter().copied().collect(),
            ));
        }
    }

    println!("{read} symbols read from {} roots", roots.len());
    for (shape, count) in &shapes {
        println!("  {count:5} {shape}");
    }
    println!(
        "\n{} bodies where the pick holds fewer strokes than another sheet:",
        lost.len()
    );
    lost.sort_by_key(|(_, picked, best, _)| best - picked);
    for (path, picked, best, selection) in lost.iter().rev().take(25) {
        println!(
            "  {picked}/{best} shown, selected {selection:?}  {}",
            path.display()
        );
    }
    ExitCode::SUCCESS
}
