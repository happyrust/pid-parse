//! Ask the drawings which `.sym` sheet `SmartPlant` actually drew.
//!
//! A `.sym` often keeps its body on two sheets, and
//! `SymbolGeometry::displayed_primitives` has to choose one. This probe takes
//! the cached body each placement carries — the one the reference plots are
//! printed from — and looks for the sheet whose filtered strokes match it, so
//! the choice can be judged against what the vendor drew instead of against a
//! naming convention.
//!
//! For every placement with both a cached body and a library body:
//!
//! - the cached body's shape census,
//! - the census of each library sheet after that sheet's own view set and the
//!   hidden-name rule are applied,
//! - the sheet `displayed_primitives` picks today.
//!
//! ```text
//! cargo run --example probe_which_sheet_the_cache_matches -- <drawing.pid | dir>... [--corpus DIR]
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::symbol_library::{SymbolGeometry, SymbolLibrary, SymbolPrimitive, SymbolSheetView};
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

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

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = corpus.join("Ref").is_dir().then(|| corpus.join("Ref")) {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut found: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| {
                    entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("symbol")
                })
                .map(|entry| entry.path())
                .collect();
            found.sort();
            roots.extend(found);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    roots
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
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

type Census = [usize; 6];

fn census<'a>(primitives: impl IntoIterator<Item = &'a SymbolPrimitive>) -> Census {
    let mut out = [0usize; 6];
    for primitive in primitives {
        match primitive {
            SymbolPrimitive::Line { .. } => out[0] += 1,
            SymbolPrimitive::Circle { .. } => out[1] += 1,
            SymbolPrimitive::Arc { .. } => out[2] += 1,
            SymbolPrimitive::Polyline { .. } => out[3] += 1,
            SymbolPrimitive::BSpline { .. } => out[4] += 1,
            SymbolPrimitive::Boundary { .. } => out[5] += 1,
            SymbolPrimitive::Text { .. } => {}
        }
    }
    out
}

/// The strokes one sheet shows when its own view set and the hidden-name rule
/// are applied to it alone.
fn sheet_census(body: &SymbolGeometry, sheet: u32) -> Census {
    let view = body.sheet_views.iter().find(|view| view.sheet_ref == sheet);
    census(
        body.primitives
            .iter()
            .filter(|styled| styled.sheet_ref == sheet)
            .filter(|styled| {
                if styled.sheet_layer_name.as_deref().is_some_and(hidden) {
                    return false;
                }
                match (view, u16::try_from(styled.sheet_layer_number)) {
                    (Some(view), Ok(number)) => bit_is_set(&view.display, number),
                    _ => true,
                }
            })
            .map(|styled| &styled.primitive),
    )
}

fn picked_sheets(body: &SymbolGeometry) -> BTreeSet<u32> {
    let view_of = |sheet: u32| body.sheet_views.iter().find(|v| v.sheet_ref == sheet);
    let sheets: BTreeSet<u32> = body
        .primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .collect();
    let normal: BTreeSet<u32> = sheets
        .iter()
        .copied()
        .filter(|sheet| view_of(*sheet).is_some_and(|view| !supplementary(view)))
        .collect();
    let unknown: BTreeSet<u32> = sheets
        .iter()
        .copied()
        .filter(|sheet| view_of(*sheet).is_none())
        .collect();
    if !normal.is_empty() {
        normal
    } else if sheets.contains(&6) {
        [6].into_iter().collect()
    } else if !unknown.is_empty() {
        unknown
    } else {
        sheets
    }
}

/// The rule `displayed_primitives` ships: normal view, then `Sheet6`, then the
/// sheets no view set governs, then everything.
#[allow(dead_code)]
fn pick_shipping(body: &SymbolGeometry) -> BTreeSet<u32> {
    picked_sheets(body)
}

/// A candidate that, when no sheet has a normal view, takes the sheet a view
/// set governs instead of assuming `Sheet6` is the body.
fn pick_governed_before_sheet6(body: &SymbolGeometry) -> BTreeSet<u32> {
    let view_of = |sheet: u32| body.sheet_views.iter().find(|v| v.sheet_ref == sheet);
    let sheets: BTreeSet<u32> = body
        .primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .collect();
    let normal: BTreeSet<u32> = sheets
        .iter()
        .copied()
        .filter(|sheet| view_of(*sheet).is_some_and(|view| !supplementary(view)))
        .collect();
    if !normal.is_empty() {
        return normal;
    }
    let governed: BTreeSet<u32> = body
        .sheet_views
        .iter()
        .map(|view| view.sheet_ref)
        .filter(|sheet| sheets.contains(sheet))
        .collect();
    if !governed.is_empty() {
        return governed;
    }
    if sheets.contains(&6) {
        return [6].into_iter().collect();
    }
    sheets
}

/// A candidate that takes whichever sheet holds the most strokes once its own
/// view set is applied.
fn pick_fattest(body: &SymbolGeometry) -> BTreeSet<u32> {
    let mut best: Option<(u32, usize)> = None;
    for sheet in body
        .primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .collect::<BTreeSet<_>>()
    {
        let strokes = sheet_census(body, sheet).iter().sum::<usize>();
        if best.is_none_or(|(_, b)| strokes > b) {
            best = Some((sheet, strokes));
        }
    }
    best.map(|(sheet, _)| [sheet].into_iter().collect())
        .unwrap_or_default()
}

/// `displayed_primitives`' normal-view rule, with the fattest sheet standing
/// in when no sheet has one.
fn pick_normal_then_fattest(body: &SymbolGeometry) -> BTreeSet<u32> {
    let view_of = |sheet: u32| body.sheet_views.iter().find(|v| v.sheet_ref == sheet);
    let normal: BTreeSet<u32> = body
        .primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .filter(|sheet| view_of(*sheet).is_some_and(|view| !supplementary(view)))
        .collect();
    if !normal.is_empty() {
        return normal;
    }
    pick_fattest(body)
}

fn has_normal_view(body: &SymbolGeometry) -> bool {
    body.sheet_views.iter().any(|view| !supplementary(view))
}

fn main() -> ExitCode {
    let mut drawings: Vec<PathBuf> = Vec::new();
    let mut corpus = corpus_dir();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--corpus" {
            if let Some(dir) = args.next() {
                corpus = PathBuf::from(dir);
            }
        } else {
            drawings.push(PathBuf::from(arg));
        }
    }
    if drawings.is_empty() {
        eprintln!("usage: probe_which_sheet_the_cache_matches <drawing | dir>... [--corpus DIR]");
        return ExitCode::from(2);
    }
    let mut expanded = Vec::new();
    for path in drawings.drain(..) {
        if path.is_dir() {
            collect(&path, &mut expanded);
        } else {
            expanded.push(path);
        }
    }

    let mut library = SymbolLibrary::with_roots(library_roots(&corpus));
    let mut compared = BTreeSet::new();
    let mut total = 0usize;
    let mut cache_matches_pick = 0usize;
    let mut cache_matches_other = 0usize;
    let mut no_match = 0usize;
    let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
    let mut candidates: BTreeMap<(&'static str, bool), (usize, usize)> = BTreeMap::new();
    let mut examples: Vec<(String, String, u32, Vec<u32>)> = Vec::new();

    for path in &expanded {
        let doc = match PidParser::new().parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(1);
            }
        };
        let geometry = build_normalized_geometry(&doc);
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                symbol_path,
                definition,
                ..
            } = &entity.kind
            else {
                continue;
            };
            let (Some(symbol_path), Some(reference)) = (symbol_path, definition) else {
                continue;
            };
            if !compared.insert((
                symbol_path.to_ascii_lowercase(),
                reference.site,
                reference.sheet,
            )) {
                continue;
            }
            let Some(definition) = geometry.symbol_definition(*reference) else {
                continue;
            };
            let Some(body) = library.resolve(symbol_path) else {
                continue;
            };
            let cache = census(
                definition
                    .visible_styled()
                    .map(|(primitive, _, _)| primitive),
            );
            let pick = picked_sheets(body);
            let mut matched: Vec<u32> = Vec::new();
            let mut best: Option<(u32, usize)> = None;
            for sheet in body
                .primitives
                .iter()
                .map(|styled| styled.sheet_ref)
                .collect::<BTreeSet<_>>()
            {
                let sheet_census_now = sheet_census(body, sheet);
                if sheet_census_now == cache {
                    matched.push(sheet);
                }
                let distance: usize = sheet_census_now
                    .iter()
                    .zip(cache.iter())
                    .map(|(a, b)| a.abs_diff(*b))
                    .sum();
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((sheet, distance));
                }
            }
            let normal = has_normal_view(body);
            for (label, candidate) in [
                ("shipping", pick_shipping(body)),
                ("governed-before-Sheet6", pick_governed_before_sheet6(body)),
                ("fattest-sheet", pick_fattest(body)),
                ("normal-then-fattest", pick_normal_then_fattest(body)),
            ] {
                let entry = candidates.entry((label, normal)).or_insert((0, 0));
                if !matched.is_empty() {
                    entry.1 += 1;
                    if matched.iter().any(|sheet| candidate.contains(sheet)) {
                        entry.0 += 1;
                    }
                }
            }
            total += 1;
            let pick_matches = matched.iter().any(|sheet| pick.contains(sheet));
            if pick_matches {
                cache_matches_pick += 1;
                *shapes.entry("cache matches the pick".into()).or_default() += 1;
            } else if !matched.is_empty() {
                cache_matches_other += 1;
                *shapes
                    .entry("cache matches a sheet the pick refuses".into())
                    .or_default() += 1;
                if examples.len() < 15 {
                    examples.push((
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        symbol_path.clone(),
                        matched[0],
                        pick.iter().copied().collect(),
                    ));
                }
            } else {
                no_match += 1;
                *shapes
                    .entry("no sheet matches the cache".into())
                    .or_default() += 1;
            }
        }
    }

    println!("\n{total} placements with both a cached body and a library body:");
    for (shape, count) in &shapes {
        println!("  {count:5} {shape}");
    }
    println!(
        "  pick matches {cache_matches_pick}, other sheet matches {cache_matches_other}, no match {no_match}"
    );
    for (drawing, symbol, sheet, pick) in &examples {
        println!("  {drawing}: Sheet{sheet} matches the cache, pick {pick:?}  {symbol}");
    }
    println!("\nrules, scored against the cached body (pairs some sheet matches):");
    for (label, (hit, out_of)) in &candidates {
        let percent = if *out_of == 0 {
            0.0
        } else {
            100.0 * *hit as f64 / *out_of as f64
        };
        println!(
            "  {hit}/{out_of} ({percent:.1} %)  {} (has normal view: {})",
            label.0, label.1,
        );
    }
    let mut by_rule: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for ((label, _), (hit, out_of)) in &candidates {
        let entry = by_rule.entry(label).or_insert((0, 0));
        entry.0 += hit;
        entry.1 += out_of;
    }
    println!("\ntotals across both shapes:");
    for (label, (hit, out_of)) in &by_rule {
        let percent = if *out_of == 0 {
            0.0
        } else {
            100.0 * *hit as f64 / *out_of as f64
        };
        println!("  {hit}/{out_of} ({percent:.1} %)  {label}");
    }
    ExitCode::SUCCESS
}
