//! Does a `.sym` body's lettering decide which of its sheets is drawn?
//!
//! `SymbolGeometry::displayed_primitives` draws a clean view set's sheet when
//! that sheet shows anything, and otherwise the sheet that shows the most;
//! both counts include `Text`. Since the library reads all three `igTextBox`
//! shapes, a clean sheet whose body went to a twin sheet in the cross-sheet
//! de-duplication can still hold its own labels, and if they sit on a shown
//! layer the labels alone win it the pick.
//!
//! For every `.sym` under the library roots this probe compares the pick as
//! shipped (reproduced here and checked against `displayed_primitives`) with
//! the same pick counting geometry only, which falls back to the shipped pick
//! where no sheet shows any geometry. It lists every symbol whose picks
//! differ, with the shapes each draws and the lettering on the sheets only the
//! shipped pick takes. It also compares `bounds()`, which takes in every
//! primitive including hidden lettering, with the bounds of what is displayed.
//!
//! ```text
//! cargo run --example probe_sym_pick_with_and_without_text -- [--root DIR]... [--verbose N]
//! ```
//!
//! Read-only.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{
    read_symbol_geometry, StyledPrimitive, SymbolGeometry, SymbolPrimitive, SymbolSheetView,
};

type Bounds = (f64, f64, f64, f64);

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

fn view_of(body: &SymbolGeometry, sheet: u32) -> Option<&SymbolSheetView> {
    body.sheet_views.iter().find(|view| view.sheet_ref == sheet)
}

/// `displayed_primitives`' own layer rule: the sheet's set answers at the
/// layer's number, then the hidden-name convention.
fn shown(body: &SymbolGeometry, styled: &StyledPrimitive) -> bool {
    if styled.sheet_layer_ref == 0 {
        return true;
    }
    let view = view_of(body, styled.sheet_ref);
    let bit = match (view, u16::try_from(styled.sheet_layer_number)) {
        (Some(view), Ok(number)) => bit_is_set(&view.display, number),
        _ => true,
    };
    bit && styled
        .sheet_layer_name
        .as_deref()
        .is_none_or(|name| !hidden(name))
}

fn is_text(styled: &StyledPrimitive) -> bool {
    matches!(styled.primitive, SymbolPrimitive::Text { .. })
}

/// The sheets `displayed_primitives` draws when only what `counts` admits
/// counts as drawing; empty where nothing counts on any sheet.
fn pick(body: &SymbolGeometry, counts: impl Fn(&StyledPrimitive) -> bool) -> BTreeSet<u32> {
    let present: BTreeSet<u32> = body
        .primitives
        .iter()
        .map(|styled| styled.sheet_ref)
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .collect();
    let on_sheet = |sheet: u32| {
        body.primitives
            .iter()
            .filter(|&styled| styled.sheet_ref == sheet && shown(body, styled) && counts(styled))
            .count()
    };
    let drawing: BTreeSet<u32> = present
        .iter()
        .copied()
        .filter(|sheet| view_of(body, *sheet).is_some_and(|view| !supplementary(view)))
        .filter(|sheet| on_sheet(*sheet) > 0)
        .collect();
    if !drawing.is_empty() {
        return drawing;
    }
    let mut best: Option<(u32, usize)> = None;
    for sheet in &present {
        let strokes = on_sheet(*sheet);
        if best.is_none_or(|(_, most)| strokes > most) {
            best = Some((*sheet, strokes));
        }
    }
    match best {
        Some((sheet, strokes)) if strokes > 0 => BTreeSet::from([sheet]),
        _ => BTreeSet::new(),
    }
}

/// What a pick draws: its sheets' shown primitives, or the whole body where
/// that is nothing, which is `displayed_primitives`' own fallback.
fn drawn<'a>(body: &'a SymbolGeometry, sheets: &BTreeSet<u32>) -> Vec<&'a StyledPrimitive> {
    let visible: Vec<&StyledPrimitive> = body
        .primitives
        .iter()
        .filter(|&styled| sheets.contains(&styled.sheet_ref) && shown(body, styled))
        .collect();
    if visible.is_empty() {
        body.primitives.iter().collect()
    } else {
        visible
    }
}

fn same_primitives(a: &[&StyledPrimitive], b: &[&StyledPrimitive]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| std::ptr::eq(*x, *y))
}

fn geometry_count(drawn: &[&StyledPrimitive]) -> usize {
    drawn.iter().filter(|styled| !is_text(styled)).count()
}

/// A sheet's shown geometry, lettering excluded.
fn shown_geometry(body: &SymbolGeometry, sheet: u32) -> usize {
    body.primitives
        .iter()
        .filter(|&styled| styled.sheet_ref == sheet && shown(body, styled) && !is_text(styled))
        .count()
}

/// Whether some sheet of a pick shows lettering and no geometry.
fn won_on_lettering(body: &SymbolGeometry, sheets: &BTreeSet<u32>) -> bool {
    sheets.iter().any(|&sheet| shown_geometry(body, sheet) == 0)
}

/// The lettering a sheet shows, as `"text" [layer]`.
fn lettering_on(body: &SymbolGeometry, sheet: u32) -> Vec<String> {
    body.primitives
        .iter()
        .filter(|&styled| styled.sheet_ref == sheet && shown(body, styled))
        .filter_map(|styled| {
            let SymbolPrimitive::Text { text, .. } = &styled.primitive else {
                return None;
            };
            let layer = styled.sheet_layer_name.as_deref().unwrap_or("-");
            Some(format!("{text:?} [{layer}]"))
        })
        .collect()
}

fn bounds_of<'a>(primitives: impl IntoIterator<Item = &'a StyledPrimitive>) -> Option<Bounds> {
    SymbolGeometry {
        primitives: primitives.into_iter().cloned().collect(),
        ..SymbolGeometry::default()
    }
    .bounds()
}

fn extent(bounds: Bounds) -> f64 {
    (bounds.2 - bounds.0).max(bounds.3 - bounds.1)
}

fn mm(bounds: Bounds) -> String {
    let width = (bounds.2 - bounds.0) * 1000.0;
    let height = (bounds.3 - bounds.1) * 1000.0;
    format!("{width:.1} x {height:.1} mm")
}

fn main() {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut verbose = 20usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => roots.extend(args.next().map(PathBuf::from)),
            "--verbose" => {
                verbose = args.next().and_then(|n| n.parse().ok()).unwrap_or(verbose);
            }
            other => roots.push(PathBuf::from(other)),
        }
    }
    if roots.is_empty() {
        roots = library_roots(&corpus_dir());
    }
    if roots.is_empty() {
        println!("no symbol library on this machine; nothing to measure");
        return;
    }
    let mut files = Vec::new();
    for root in &roots {
        collect_syms(root, &mut files);
    }
    println!("{} .sym files under {} root(s)", files.len(), roots.len());

    let mut read = 0usize;
    let mut reproduced = 0usize;
    let mut not_reproduced: Vec<String> = Vec::new();
    let mut picks_differ: Vec<String> = Vec::new();
    let mut geometry_differs = 0usize;
    let mut lettering_alone = 0usize;
    let mut inflated: Vec<String> = Vec::new();
    let mut inflated_by_text = 0usize;

    for path in &files {
        let Ok(body) = read_symbol_geometry(path) else {
            continue;
        };
        read += 1;
        let name = path.display().to_string();

        let shipped = pick(&body, |_| true);
        let shipped_drawn = drawn(&body, &shipped);
        let displayed = body.displayed_primitives();
        if same_primitives(&shipped_drawn, &displayed) {
            reproduced += 1;
        } else {
            not_reproduced.push(name.clone());
        }

        let geometry = pick(&body, |styled| !is_text(styled));
        let geometry_first = if geometry.is_empty() {
            shipped.clone()
        } else {
            geometry
        };
        if geometry_first != shipped {
            let first_drawn = drawn(&body, &geometry_first);
            let before = geometry_count(&shipped_drawn);
            let after = geometry_count(&first_drawn);
            if before != after {
                geometry_differs += 1;
            }
            if won_on_lettering(&body, &shipped) {
                lettering_alone += 1;
            }
            let lettering: Vec<String> = shipped
                .difference(&geometry_first)
                .flat_map(|sheet| lettering_on(&body, *sheet))
                .collect();
            let lettering = lettering.join(", ");
            let row = format!("{name}: shipped {shipped:?} draws {before} shapes");
            let row = format!("{row}, geometry-first {geometry_first:?} draws {after}");
            picks_differ.push(format!("{row}; shipped-only lettering: {lettering}"));
        }

        let all = body.bounds();
        let on_page = bounds_of(displayed.iter().copied());
        if let (Some(all), Some(on_page)) = (all, on_page) {
            let limit = 2.0 * extent(on_page);
            if extent(on_page) > 1e-9 && extent(all) > limit {
                let geometry = bounds_of(body.primitives.iter().filter(|s| !is_text(s)));
                if geometry.is_some_and(|bounds| extent(bounds) <= limit) {
                    inflated_by_text += 1;
                }
                let (all, on_page) = (mm(all), mm(on_page));
                inflated.push(format!(
                    "{name}: every primitive {all}, displayed {on_page}"
                ));
            }
        }
    }

    println!("{read} read");
    println!("self-check: reproduced displayed_primitives on {reproduced} of {read}");
    for name in not_reproduced.iter().take(verbose) {
        println!("  not reproduced: {name}");
    }
    let differ = picks_differ.len();
    println!("picks differ on {differ} symbols; the geometry drawn differs on {geometry_differs}");
    println!("a sheet won the shipped pick on lettering alone: {lettering_alone}");
    for row in picks_differ.iter().take(verbose) {
        println!("  {row}");
    }
    let over = inflated.len();
    println!("bounds() is over twice the displayed extent on {over} symbols");
    println!("  of those, the excess is lettering on {inflated_by_text}");
    for row in inflated.iter().take(verbose) {
        println!("  {row}");
    }
}
