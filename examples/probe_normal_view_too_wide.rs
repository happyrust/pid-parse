//! Is the normal-view classification too wide?
//!
//! `SymbolGeometry::displayed_primitives` prefers the sheet whose `0x0057
//! Top ViewFilterSet` files the supplementary layers off; the fattest sheet is
//! only consulted when no sheet has such a set. Scored against the bodies the
//! drawings' caches carry, taking the fattest sheet regardless beats that rule
//! on the placements that do have a normal view, which is only possible if some
//! of those "normal" sheets are not the body.
//!
//! This probe names the mechanism. For every placement whose cached body is
//! reproduced exactly by some library sheet, it compares
//!
//! - what ships today (`displayed_primitives`, whole-body fallback included),
//! - the normal-view pick, the fattest pick, and two repairs of the normal
//!   rule (hollow normal sheets fall through; stubs fall through),
//!
//! and classifies the normal sheet that lost as **hollow** (its own set hides
//! every stroke it holds), **stub** (it holds a handful against a body-sized
//! sheet) or **body** (it holds the body and the fattest only adds ink the
//! view files off).
//!
//! ```text
//! cargo run --example probe_normal_view_too_wide -- <drawing.pid | dir>... [--corpus DIR] [--verbose N]
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::symbol_library::{
    StyledPrimitive, SymbolGeometry, SymbolLibrary, SymbolPrimitive, SymbolSheetView,
};
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
    let ref_dir = corpus.join("Ref");
    if ref_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ref_dir) {
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

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

/// Everything one symbol files on each of its sheets: the layer table with
/// each layer's bit in the governing set, and every primitive with the bit and
/// the name that decide whether it draws.
fn detail(corpus: &Path, needle: &str) {
    let mut files = Vec::new();
    for root in library_roots(corpus) {
        collect_syms(&root, &mut files);
    }
    for path in files
        .iter()
        .filter(|path| path.to_string_lossy().contains(needle))
    {
        let Ok(body) = pid_parse::symbol_library::read_symbol_geometry(path) else {
            println!("{}: unreadable", path.display());
            continue;
        };
        println!("\n== {}", path.display());
        for view in &body.sheet_views {
            println!("   Sheet{} set: {}", view.sheet_ref, view_summary(view),);
            println!(
                "      classified {}",
                if supplementary(view) {
                    "ALTERNATE (a hidden-named layer is on)"
                } else {
                    "normal (no hidden-named layer is on)"
                }
            );
        }
        let sheets: BTreeSet<u32> = sheets_with_strokes(&body)
            .into_iter()
            .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
            .collect();
        for sheet in &sheets {
            let view = view_of(&body, *sheet);
            println!(
                "   Sheet{sheet}: {} primitives raw",
                body.primitives
                    .iter()
                    .filter(|styled| styled.sheet_ref == *sheet)
                    .count()
            );
            let mut raw: BTreeMap<(String, u32), (usize, usize, usize)> = BTreeMap::new();
            for styled in body.primitives.iter().filter(|s| s.sheet_ref == *sheet) {
                let name = styled
                    .sheet_layer_name
                    .clone()
                    .unwrap_or_else(|| "<unresolved>".to_owned());
                let bit = match (view, u16::try_from(styled.sheet_layer_number)) {
                    (Some(view), Ok(number)) => bit_is_set(&view.display, number),
                    _ => true,
                };
                let off_by_name = styled.sheet_layer_name.as_deref().is_some_and(hidden);
                let entry = raw
                    .entry((name, styled.sheet_layer_number))
                    .or_insert((0, 0, 0));
                entry.0 += 1;
                if off_by_name {
                    entry.2 += 1;
                }
                if bit {
                    entry.1 += 1;
                }
            }
            for ((name, number), (count, bits_on, off_by_name)) in &raw {
                println!(
                    "      layer {name:24} number {number:>3} bit {}  hidden-name {}  {count} strokes (bit on: {bits_on}, off by name: {off_by_name})",
                    if *number == u32::MAX {
                        "-".to_owned()
                    } else if match (view, u16::try_from(*number)) {
                        (Some(view), Ok(n)) => bit_is_set(&view.display, n),
                        _ => true,
                    } {
                        "on".to_owned()
                    } else {
                        "off".to_owned()
                    },
                    if body
                        .sheet_layers
                        .iter()
                        .any(|layer| layer.layer_number == *number && hidden(&layer.name))
                    {
                        "yes"
                    } else {
                        "no"
                    },
                );
            }
            println!(
                "      => draws {} ({} strokes)",
                describe(&sheet_census(&body, *sheet)),
                sheet_strokes(&body, *sheet).len(),
            );
        }
        println!(
            "   layer table: {:?}",
            body.sheet_layers
                .iter()
                .map(|layer| (layer.layer_number, layer.name.as_str()))
                .collect::<Vec<_>>()
        );
    }
}

/// How often the clean set governs a sheet that is not the body, across the
/// whole library rather than the handful of symbols a drawing places.
fn library_census(roots: &[PathBuf]) {
    let mut files = Vec::new();
    for root in roots {
        collect_syms(root, &mut files);
    }
    let mut with_views = 0usize;
    let mut with_alternate = 0usize;
    let mut alternate_with_ink = 0usize;
    let mut alternate_with_geometry = 0usize;
    let mut with_clean = 0usize;
    let mut clean_on_empty_sheet = 0usize;
    let mut clean_draws_nothing = 0usize;
    let mut clean_thinner_than_fattest = 0usize;
    let mut read = 0usize;
    for path in &files {
        let Ok(body) = pid_parse::symbol_library::read_symbol_geometry(path) else {
            continue;
        };
        read += 1;
        if body.sheet_views.is_empty() {
            continue;
        }
        with_views += 1;
        let normal = normal_governed(&body);
        if normal.is_empty() {
            continue;
        }
        with_clean += 1;
        for sheet in sheets_with_strokes(&body) {
            if let Some(view) = view_of(&body, sheet) {
                if supplementary(view) {
                    with_alternate += 1;
                    if body.primitives.iter().any(|s| {
                        s.sheet_ref == sheet && s.sheet_layer_name.as_deref().is_some_and(hidden)
                    }) {
                        alternate_with_ink += 1;
                    }
                    if body.primitives.iter().any(|s| {
                        s.sheet_ref == sheet
                            && s.sheet_layer_name.as_deref().is_some_and(hidden)
                            && !matches!(s.primitive, SymbolPrimitive::Text { .. })
                    }) {
                        alternate_with_geometry += 1;
                    }
                }
            }
        }
        let holding = sheets_with_strokes(&body);
        if normal.iter().all(|sheet| !holding.contains(sheet)) {
            clean_on_empty_sheet += 1;
        }
        let clean_strokes: usize = normal.iter().map(|s| sheet_strokes(&body, *s).len()).sum();
        if clean_strokes == 0 {
            clean_draws_nothing += 1;
        }
        if let Some((_, fat)) = fattest(&body) {
            if clean_strokes < fat {
                clean_thinner_than_fattest += 1;
            }
        }
    }
    println!("\n{read} library symbols read, {with_views} carry a view set");
    println!("  {with_clean} have a clean (non-supplementary) set");
    println!(
        "  {with_alternate} alternate (all-on) sheets hold strokes, {alternate_with_ink} of them keep any on a hidden-named layer ({alternate_with_geometry} keep geometry there)"
    );
    println!("  {clean_on_empty_sheet} of those govern a sheet with no primitives");
    println!("  {clean_draws_nothing} draw nothing once their own set has had its say");
    println!("  {clean_thinner_than_fattest} are thinner than the fattest sheet");
}

type Census = [usize; 6];

fn add(out: &mut Census, primitive: &SymbolPrimitive) {
    let slot = match primitive {
        SymbolPrimitive::Line { .. } => 0,
        SymbolPrimitive::Circle { .. } => 1,
        SymbolPrimitive::Arc { .. } => 2,
        SymbolPrimitive::Polyline { .. } => 3,
        SymbolPrimitive::BSpline { .. } => 4,
        SymbolPrimitive::Boundary { .. } => 5,
        SymbolPrimitive::Text { .. } => return,
    };
    out[slot] += 1;
}

fn census<'a>(primitives: impl IntoIterator<Item = &'a SymbolPrimitive>) -> Census {
    let mut out = [0usize; 6];
    for primitive in primitives {
        add(&mut out, primitive);
    }
    out
}

fn describe(census: &Census) -> String {
    let labels = ["line", "circle", "arc", "polyline", "bspline", "boundary"];
    let parts: Vec<String> = census
        .iter()
        .zip(labels)
        .filter(|(count, _)| **count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect();
    if parts.is_empty() {
        "<nothing>".to_owned()
    } else {
        parts.join(" + ")
    }
}

/// Whether one stroke of `sheet` draws: its set's bit, then the hidden-name
/// convention — the two rules `displayed_primitives` stacks.
fn stroke_shown(styled: &StyledPrimitive, view: Option<&SymbolSheetView>) -> bool {
    let bit = match (view, u16::try_from(styled.sheet_layer_number)) {
        (Some(view), Ok(number)) => bit_is_set(&view.display, number),
        _ => true,
    };
    bit && styled
        .sheet_layer_name
        .as_deref()
        .is_none_or(|name| !hidden(name))
}

fn view_of(body: &SymbolGeometry, sheet: u32) -> Option<&SymbolSheetView> {
    body.sheet_views.iter().find(|view| view.sheet_ref == sheet)
}

fn sheet_strokes(body: &SymbolGeometry, sheet: u32) -> Vec<&StyledPrimitive> {
    let view = view_of(body, sheet);
    body.primitives
        .iter()
        .filter(|styled| styled.sheet_ref == sheet)
        .filter(|styled| stroke_shown(styled, view))
        .collect()
}

fn sheet_census(body: &SymbolGeometry, sheet: u32) -> Census {
    census(sheet_strokes(body, sheet).into_iter().map(|s| &s.primitive))
}

fn sheets_with_strokes(body: &SymbolGeometry) -> BTreeSet<u32> {
    body.primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .collect()
}

/// Every sheet a clean (non-supplementary) view set governs, whether or not
/// the sheet holds anything — the classification `displayed_primitives` uses.
fn normal_governed(body: &SymbolGeometry) -> BTreeSet<u32> {
    let present: BTreeSet<u32> = sheets_with_strokes(body)
        .into_iter()
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .collect();
    body.sheet_views
        .iter()
        .filter(|view| !supplementary(view))
        .map(|view| view.sheet_ref)
        .filter(|sheet| present.contains(sheet))
        .collect()
}

/// A sheet's body geometry, lettering excluded: the catalogue `NULL`/`PG`
/// runs a `.sym` keeps on its `Label` layer are not strokes of the body, and
/// a label's editing view is fat with them.
fn geometry_strokes(body: &SymbolGeometry, sheet: u32) -> usize {
    sheet_strokes(body, sheet)
        .into_iter()
        .filter(|styled| !matches!(styled.primitive, SymbolPrimitive::Text { .. }))
        .count()
}

fn fattest(body: &SymbolGeometry) -> Option<(u32, usize)> {
    let mut best: Option<(u32, usize)> = None;
    for sheet in sheets_with_strokes(body) {
        let strokes = geometry_strokes(body, sheet);
        if best.is_none_or(|(_, most)| strokes > most) {
            best = Some((sheet, strokes));
        }
    }
    best
}

/// The census a sheet pick yields under `displayed_primitives`' own fallback:
/// a pick that leaves nothing visible draws the whole body.
fn picked_census(body: &SymbolGeometry, pick: &BTreeSet<u32>) -> Census {
    let mut chosen: Vec<&SymbolPrimitive> = Vec::new();
    for styled in &body.primitives {
        if !pick.contains(&styled.sheet_ref) {
            continue;
        }
        if stroke_shown(styled, view_of(body, styled.sheet_ref)) {
            chosen.push(&styled.primitive);
        }
    }
    if chosen.is_empty() {
        return census(body.primitives.iter().map(|styled| &styled.primitive));
    }
    census(chosen)
}

/// A sheet's drawn extent, as a width and height in millimetres.
fn sheet_extent(body: &SymbolGeometry, sheet: u32) -> Option<(f64, f64)> {
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut add = |x: f64, y: f64| {
        min.0 = min.0.min(x);
        min.1 = min.1.min(y);
        max.0 = max.0.max(x);
        max.1 = max.1.max(y);
    };
    for styled in sheet_strokes(body, sheet) {
        match &styled.primitive {
            SymbolPrimitive::Line { start, end } => {
                add(start.0, start.1);
                add(end.0, end.1);
            }
            SymbolPrimitive::Circle { center, radius } => {
                add(center.0 - radius, center.1 - radius);
                add(center.0 + radius, center.1 + radius);
            }
            SymbolPrimitive::Arc { center, radius, .. } => {
                add(center.0 - radius, center.1 - radius);
                add(center.0 + radius, center.1 + radius);
            }
            SymbolPrimitive::Polyline { vertices, .. } => {
                for (x, y) in vertices {
                    add(*x, *y);
                }
            }
            SymbolPrimitive::BSpline { poles, .. } => {
                for (x, y) in poles {
                    add(*x, *y);
                }
            }
            SymbolPrimitive::Boundary { members, .. } => {
                for member in members {
                    match member {
                        SymbolPrimitive::Circle { center, radius } => {
                            add(center.0 - radius, center.1 - radius);
                            add(center.0 + radius, center.1 + radius);
                        }
                        SymbolPrimitive::Line { start, end } => {
                            add(start.0, start.1);
                            add(end.0, end.1);
                        }
                        _ => {}
                    }
                }
            }
            SymbolPrimitive::Text { .. } => {}
        }
    }
    (min.0.is_finite() && max.0.is_finite()).then_some((max.0 - min.0, max.1 - min.1))
}

/// One stroke's shape, rounded to a micrometre, so two sheets that hold the
/// same drawing compare equal and a copy that added or moved a stroke does
/// not. Text is left out: the two sheets of a body differ in catalogue
/// lettering that the hidden-name rule drops anyway.
fn stroke_key(primitive: &SymbolPrimitive) -> Option<String> {
    let at = |v: f64| (v * 1e6).round() as i64;
    Some(match primitive {
        SymbolPrimitive::Line { start, end } => {
            let a = (at(start.0), at(start.1));
            let b = (at(end.0), at(end.1));
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            format!("L {} {} {} {}", a.0, a.1, b.0, b.1)
        }
        SymbolPrimitive::Circle { center, radius } => {
            format!("C {} {} {}", at(center.0), at(center.1), at(*radius))
        }
        SymbolPrimitive::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => format!(
            "A {} {} {} {} {}",
            at(center.0),
            at(center.1),
            at(*radius),
            at(*start_angle),
            at(*end_angle)
        ),
        SymbolPrimitive::Polyline {
            vertices,
            is_closed,
        } => format!(
            "P{}{}",
            if *is_closed { "c" } else { "o" },
            vertices
                .iter()
                .map(|(x, y)| format!("{},{};", at(*x), at(*y)))
                .collect::<String>()
        ),
        SymbolPrimitive::BSpline {
            poles,
            weights,
            knots,
        } => format!(
            "B{}|{}",
            poles
                .iter()
                .map(|(x, y)| format!("{},{};", at(*x), at(*y)))
                .collect::<String>(),
            knots
                .iter()
                .map(|k| at(*k).to_string())
                .collect::<Vec<_>>()
                .join(",")
                + &weights
                    .iter()
                    .map(|w| at(*w).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
        ),
        SymbolPrimitive::Boundary { members, .. } => format!(
            "F{}",
            members
                .iter()
                .filter_map(stroke_key)
                .collect::<Vec<_>>()
                .join("|")
        ),
        SymbolPrimitive::Text { .. } => return None,
    })
}

/// The drawn strokes of a sheet as a multiset of shapes.
fn sheet_shapes(body: &SymbolGeometry, sheet: u32) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for styled in sheet_strokes(body, sheet) {
        if let Some(key) = stroke_key(&styled.primitive) {
            *out.entry(key).or_default() += 1;
        }
    }
    out
}

/// Whether `fat` draws everything `thin` draws and more: two sheets holding
/// the same shapes are two views of one body, while a sheet that holds every
/// shape the other holds plus a few more is that body's later revision.
fn strictly_contains(fat: &BTreeMap<String, usize>, thin: &BTreeMap<String, usize>) -> bool {
    if fat == thin {
        return false;
    }
    thin.iter()
        .all(|(key, count)| fat.get(key).is_some_and(|have| have >= count))
}

fn displayed_listed(view: &SymbolSheetView) -> Vec<String> {
    let mut names: Vec<String> = view
        .layers
        .iter()
        .filter(|(number, _)| bit_is_set(&view.display, *number))
        .map(|(_, name)| name.clone())
        .collect();
    names.sort();
    names
}

fn view_summary(view: &SymbolSheetView) -> String {
    format!(
        "{} bits, listed [{}], shown [{}]",
        view.display.len() * 8,
        view.layers
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join("/"),
        displayed_listed(view).join("/"),
    )
}

fn sheet_summary(body: &SymbolGeometry, sheet: u32) -> String {
    let strokes = sheet_strokes(body, sheet);
    let mut layers: BTreeMap<String, usize> = BTreeMap::new();
    for styled in &strokes {
        *layers
            .entry(
                styled
                    .sheet_layer_name
                    .clone()
                    .unwrap_or_else(|| "<none>".to_owned()),
            )
            .or_default() += 1;
    }
    format!(
        "Sheet{sheet} {} ({} strokes on {:?}) [{}]",
        describe(&sheet_census(body, sheet)),
        strokes.len(),
        layers.keys().collect::<Vec<_>>(),
        view_of(body, sheet).map_or("<no set>".to_owned(), view_summary),
    )
}

fn main() -> ExitCode {
    let mut drawings: Vec<PathBuf> = Vec::new();
    let mut corpus = corpus_dir();
    let mut verbose = 0usize;
    let mut fat_table = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--corpus" {
            if let Some(dir) = args.next() {
                corpus = PathBuf::from(dir);
            }
        } else if arg == "--verbose" {
            verbose = args.next().and_then(|n| n.parse().ok()).unwrap_or(20);
        } else if arg == "--library-census" {
            library_census(&library_roots(&corpus));
            return ExitCode::SUCCESS;
        } else if arg == "--fat-table" {
            fat_table = true;
        } else if arg == "--detail" {
            let needle = args.next().unwrap_or_default();
            detail(&corpus, &needle);
            return ExitCode::SUCCESS;
        } else {
            drawings.push(PathBuf::from(arg));
        }
    }
    if drawings.is_empty() {
        eprintln!(
            "usage: probe_normal_view_too_wide <drawing | dir>... [--corpus DIR] [--verbose N]"
        );
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
    let mut placements = 0usize;
    let mut undecidable = 0usize;
    let mut scored = 0usize;
    let mut with_normal = 0usize;
    let mut normal_on_empty_sheet = 0usize;
    let mut hollow_normal = 0usize;
    let mut multi_normal = 0usize;
    let mut fat_kinds: BTreeMap<(&'static str, bool), usize> = BTreeMap::new();
    let mut fat_rows: Vec<String> = Vec::new();
    let mut flips_better = 0usize;
    let mut flips_worse = 0usize;
    let mut flip_cases: Vec<String> = Vec::new();
    let mut candidates: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut loser_classes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut loser_winners: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut cases: Vec<String> = Vec::new();

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
            placements += 1;
            let cache = census(
                definition
                    .visible_styled()
                    .map(|(primitive, _, _)| primitive),
            );
            let matching: BTreeSet<u32> = sheets_with_strokes(body)
                .into_iter()
                .filter(|sheet| sheet_census(body, *sheet) == cache)
                .collect();
            if matching.is_empty() {
                undecidable += 1;
                continue;
            }
            scored += 1;
            let normal = normal_governed(body);
            let Some((fat, fat_strokes)) = fattest(body) else {
                continue;
            };
            let normal_pick: BTreeSet<u32> = if normal.is_empty() {
                BTreeSet::from([fat])
            } else {
                normal.clone()
            };
            let fat_pick = BTreeSet::from([fat]);
            let normal_strokes: usize = normal.iter().map(|s| sheet_strokes(body, *s).len()).sum();
            let normal_geometry: usize = normal.iter().map(|s| geometry_strokes(body, *s)).sum();
            // A view set that turns a hidden-named layer on is only an
            // alternate if that layer carries something: an all-on map over
            // empty layers draws exactly what a clean map would.
            let ink_on_hidden = |sheet: u32| {
                body.primitives
                    .iter()
                    .filter(|styled| styled.sheet_ref == sheet)
                    .filter(|styled| styled.sheet_layer_name.as_deref().is_some_and(hidden))
                    .count()
            };
            let clean_or_vacuous: BTreeSet<u32> = sheets_with_strokes(body)
                .into_iter()
                .filter(|sheet| match view_of(body, *sheet) {
                    Some(view) if supplementary(view) => ink_on_hidden(*sheet) == 0,
                    Some(_) => true,
                    None => false,
                })
                .collect();
            let vacuous_pick: BTreeSet<u32> = {
                let mut best: Option<(u32, usize)> = None;
                for sheet in &clean_or_vacuous {
                    let geometry = geometry_strokes(body, *sheet);
                    if best.is_none_or(|(_, most)| geometry > most) {
                        best = Some((*sheet, geometry));
                    }
                }
                match best {
                    Some((sheet, geometry)) if geometry > 0 => BTreeSet::from([sheet]),
                    _ => normal_pick.clone(),
                }
            };
            let has_normal = !normal.is_empty();
            let more_geometry_pick: BTreeSet<u32> = if !has_normal {
                normal_pick.clone()
            } else if normal_geometry == 0 || fat_strokes > normal_geometry {
                BTreeSet::from([fat])
            } else {
                normal.clone()
            };
            if has_normal {
                with_normal += 1;
                if normal.len() > 1 {
                    multi_normal += 1;
                }
                if normal
                    .iter()
                    .all(|sheet| !sheets_with_strokes(body).contains(sheet))
                {
                    normal_on_empty_sheet += 1;
                }
                if normal_strokes == 0 {
                    hollow_normal += 1;
                }
            }
            let hollow_falls_through: BTreeSet<u32> = if has_normal && normal_strokes > 0 {
                normal.clone()
            } else if has_normal {
                BTreeSet::from([fat])
            } else {
                normal_pick.clone()
            };
            let stub_falls_through: BTreeSet<u32> =
                if has_normal && normal_strokes * 2 >= fat_strokes {
                    normal.clone()
                } else if has_normal {
                    BTreeSet::from([fat])
                } else {
                    normal_pick.clone()
                };
            let mut normal_shapes: BTreeMap<String, usize> = BTreeMap::new();
            for sheet in &normal {
                for (key, count) in sheet_shapes(body, *sheet) {
                    *normal_shapes.entry(key).or_default() += count;
                }
            }
            let fat_shapes = sheet_shapes(body, fat);
            let contains = strictly_contains(&fat_shapes, &normal_shapes);
            let same_body_pick: BTreeSet<u32> = if !has_normal {
                normal_pick.clone()
            } else if normal_strokes == 0 || fat_shapes != normal_shapes {
                BTreeSet::from([fat])
            } else {
                normal.clone()
            };
            let containment_pick: BTreeSet<u32> = if !has_normal {
                normal_pick.clone()
            } else if normal_strokes == 0 || contains {
                BTreeSet::from([fat])
            } else {
                normal.clone()
            };
            let hollow_pick: BTreeSet<u32> = if has_normal && normal_strokes > 0 {
                normal.clone()
            } else if has_normal {
                BTreeSet::from([fat])
            } else {
                normal_pick.clone()
            };
            let thinner_falls_through: BTreeSet<u32> =
                if has_normal && normal_strokes >= fat_strokes {
                    normal.clone()
                } else if has_normal {
                    BTreeSet::from([fat])
                } else {
                    normal_pick.clone()
                };

            let shipped = census(
                body.displayed_primitives()
                    .into_iter()
                    .map(|s| &s.primitive),
            );
            for (label, hit) in [
                ("shipped", shipped == cache),
                ("normal-view", picked_census(body, &normal_pick) == cache),
                ("fattest-sheet", picked_census(body, &fat_pick) == cache),
                (
                    "hollow normal falls through to fattest",
                    picked_census(body, &hollow_falls_through) == cache,
                ),
                (
                    "stub normal falls through to fattest",
                    picked_census(body, &stub_falls_through) == cache,
                ),
                (
                    "thinner normal falls through to fattest",
                    picked_census(body, &thinner_falls_through) == cache,
                ),
                (
                    "hollow, then a strict superset wins",
                    picked_census(body, &containment_pick) == cache,
                ),
                (
                    "normal unless a sheet draws more geometry",
                    picked_census(body, &more_geometry_pick) == cache,
                ),
                (
                    "a vacuous all-on set is a clean candidate",
                    picked_census(body, &vacuous_pick) == cache,
                ),
                (
                    "hollow, then a different body takes the fuller sheet",
                    picked_census(body, &same_body_pick) == cache,
                ),
            ] {
                if hit {
                    *candidates.entry(label).or_default() += 1;
                } else {
                    candidates.entry(label).or_default();
                }
            }

            let hollow_ok = picked_census(body, &hollow_pick) == cache;
            if has_normal && fat_strokes > normal_strokes {
                let hidden_ink = body
                    .primitives
                    .iter()
                    .filter(|styled| styled.sheet_ref == fat)
                    .filter(|styled| styled.sheet_layer_name.as_deref().is_some_and(hidden))
                    .count();
                let kind = match view_of(body, fat) {
                    None => "no set",
                    Some(view) if !supplementary(view) => "a clean set",
                    Some(_) if hidden_ink > 0 => "all-on, hidden layers carry ink",
                    Some(_) => "all-on, hidden layers empty",
                };
                let entry = fat_kinds.entry((kind, hollow_ok)).or_insert(0);
                *entry += 1;
                if fat_table {
                    let normal_sheet = normal.iter().next().copied().unwrap_or(fat);
                    let normal_extent = sheet_extent(body, normal_sheet);
                    let fat_extent = sheet_extent(body, fat);
                    let ratio = match (normal_extent, fat_extent) {
                        (Some((nw, nh)), Some((fw, fh))) if nw > 0.0 && nh > 0.0 => {
                            format!(
                                "{:.2}x{:.2}",
                                (fw / nw).max(fh / nh),
                                (fw / nw).min(fh / nh)
                            )
                        }
                        _ => "-".to_owned(),
                    };
                    fat_rows.push(format!(
                        "{:5} normal Sheet{normal_sheet}({normal_strokes}, {} geom) fat Sheet{fat}({fat_strokes}, {} geom) extent {:.1}x{:.1} vs {:.1}x{:.1} ratio {ratio}  {}",
                        if hollow_ok { "right" } else { "WRONG" },
                        normal_geometry,
                        geometry_strokes(body, fat),
                        normal_extent.map(|e| e.0 * 1000.0).unwrap_or(-1.0),
                        normal_extent.map(|e| e.1 * 1000.0).unwrap_or(-1.0),
                        fat_extent.map(|e| e.0 * 1000.0).unwrap_or(-1.0),
                        fat_extent.map(|e| e.1 * 1000.0).unwrap_or(-1.0),
                        symbol_path.rsplit('\\').next().unwrap_or(symbol_path),
                    ));
                }
            }
            let containment_ok = picked_census(body, &containment_pick) == cache;
            if hollow_pick != containment_pick && hollow_ok != containment_ok {
                if containment_ok {
                    flips_better += 1;
                } else {
                    flips_worse += 1;
                }
                flip_cases.push(format!(
                    "{} {}\n   cache    {}\n   normal   {}\n   fattest  {}\n   {}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    symbol_path,
                    describe(&cache),
                    describe(&sheet_census(
                        body,
                        normal.iter().next().copied().unwrap_or(fat)
                    )),
                    describe(&sheet_census(body, fat)),
                    if containment_ok {
                        "containment is right"
                    } else {
                        "containment is WRONG"
                    },
                ));
            }

            // Only the placements whose two picks disagree say anything about
            // the normal rule; both-right cases cannot prefer either.
            let fat_matches = matching.contains(&fat);
            if !(has_normal && normal_pick != fat_pick) {
                continue;
            }
            if !(fat_matches && !picked_census(body, &normal_pick).eq(&cache)) {
                continue;
            }
            let normal_sheet = normal.iter().next().copied().unwrap_or(fat);
            let class = if normal_strokes == 0 && normal.contains(&normal_sheet) {
                "hollow: the set hides every stroke the sheet holds"
            } else if normal_strokes * 2 < fat_strokes {
                "stub: the sheet holds far fewer strokes than the fattest"
            } else if normal_strokes > 0 {
                "body: the normal sheet holds the body"
            } else {
                "no normal sheet holds primitives"
            };
            *loser_classes.entry(class).or_default() += 1;
            let fat_kind = match view_of(body, fat) {
                None => "the winner has no view set",
                Some(view) if supplementary(view) => "the winner's set is the all-on alternate",
                Some(_) => "the winner's set is clean too",
            };
            *loser_winners.entry(fat_kind).or_default() += 1;
            if cases.len() < verbose {
                cases.push(format!(
                    "{} {}\n   cache   {}\n   normal  {}\n   fattest {}\n   shipped {}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    symbol_path,
                    describe(&cache),
                    sheet_summary(body, normal_sheet),
                    sheet_summary(body, fat),
                    describe(&shipped),
                ));
            }
        }
    }

    println!("\n{placements} placements with both a cached body and a library body");
    println!("  {undecidable} undecidable (no sheet reproduces the cache)");
    println!("  {scored} scored, of which {with_normal} have a normal view");
    println!("  {normal_on_empty_sheet} of those govern a sheet with no primitives");
    println!("  {hollow_normal} of those normal sheets draw nothing at all");
    println!("  {multi_normal} pick more than one normal sheet");
    println!("\nexact-match counts against the cached body ({scored} placements):");
    for (label, hit) in &candidates {
        let percent = 100.0 * *hit as f64 / scored.max(1) as f64;
        println!("  {hit:5}/{scored} ({percent:5.1} %)  {label}");
    }
    println!("\nwhere the normal pick and the fattest pick disagree and the cache sides with the fattest:");
    for (class, count) in &loser_classes {
        println!("  {count:5} {class}");
    }
    println!("  and who the winner is:");
    for (kind, count) in &loser_winners {
        println!("  {count:5} {kind}");
    }
    println!("\nwhen a fatter sheet exists, what is it? (hollow pick right / wrong)");
    for ((kind, right), count) in &fat_kinds {
        println!(
            "  {count:5}  {kind}  ({})",
            if *right { "pick right" } else { "pick wrong" }
        );
    }
    if fat_table {
        println!("\nevery placement with a fatter sheet:");
        for row in &fat_rows {
            println!("  {row}");
        }
    }
    println!("\nthe strict-superset test on top of the hollow rule:");
    println!("  {flips_better} placements it fixes, {flips_worse} it breaks");
    for case in &flip_cases {
        println!("  {case}");
    }
    if !cases.is_empty() {
        println!("\nexamples:");
        for case in &cases {
            println!("  {case}");
        }
    }
    ExitCode::SUCCESS
}
