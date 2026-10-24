//! Compare each drawing's cached symbol bodies against the `.sym` library.
//!
//! `lay_out_drawing` prefers the body a `.pid` carries over the one in a
//! library, so where both exist they describe the same symbol twice. This
//! probe asks whether they agree: per placed symbol, the census of primitive
//! kinds and the local bounding box, both in the symbol's own metres.
//!
//! A body can legitimately differ: a parametric instance (a tank, a manifold)
//! draws with the instance's variables, not the library's defaults, so a size
//! difference on a body that names a template is expected and reported as
//! `parametric`, never as a mismatch.
//!
//! ```text
//! cargo run -p pid-plot --example probe_library_vs_cached_bodies -- <a.pid | dir> ... [--corpus DIR]
//! ```
//!
//! Read-only. Exit 1 when a drawing cannot be parsed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::symbol_library::{SymbolLibrary, SymbolPrimitive};
use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};

/// Bounding-box slack, in metres. The two readers decode the same f64s, so a
/// body that agrees does so to the bit; 1 µm only absorbs the f64 order of
/// operations in a projected point.
const TOLERANCE_M: f64 = 1e-6;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for candidate in [
        Some(corpus.join("Ref")),
        corpus.parent().map(|parent| parent.join("Ref")),
    ]
    .into_iter()
    .flatten()
    {
        let Ok(entries) = std::fs::read_dir(&candidate) else {
            continue;
        };
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
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/symbols-full");
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

/// Primitive-kind census, in a fixed order.
#[derive(Default, Clone, PartialEq, Eq)]
struct Census {
    line: usize,
    circle: usize,
    arc: usize,
    polyline: usize,
    text: usize,
    bspline: usize,
    boundary: usize,
}

impl Census {
    fn of<'a>(primitives: impl IntoIterator<Item = &'a SymbolPrimitive>) -> Self {
        let mut census = Self::default();
        for primitive in primitives {
            match primitive {
                SymbolPrimitive::Line { .. } => census.line += 1,
                SymbolPrimitive::Circle { .. } => census.circle += 1,
                SymbolPrimitive::Arc { .. } => census.arc += 1,
                SymbolPrimitive::Polyline { .. } => census.polyline += 1,
                SymbolPrimitive::Text { .. } => census.text += 1,
                SymbolPrimitive::BSpline { .. } => census.bspline += 1,
                SymbolPrimitive::Boundary { .. } => census.boundary += 1,
            }
        }
        census
    }

    fn describe(&self) -> String {
        format!(
            "line={} circle={} arc={} polyline={} text={} bspline={} boundary={}",
            self.line, self.circle, self.arc, self.polyline, self.text, self.bspline, self.boundary
        )
    }
}

fn points_of(primitive: &SymbolPrimitive, out: &mut Vec<(f64, f64)>) {
    match primitive {
        SymbolPrimitive::Line { start, end } => {
            out.push(*start);
            out.push(*end);
        }
        SymbolPrimitive::Circle { center, radius } => {
            out.push((center.0 - radius, center.1 - radius));
            out.push((center.0 + radius, center.1 + radius));
        }
        SymbolPrimitive::Arc { center, radius, .. } => {
            out.push((center.0 - radius, center.1 - radius));
            out.push((center.0 + radius, center.1 + radius));
        }
        SymbolPrimitive::Polyline { vertices, .. } => out.extend(vertices.iter().copied()),
        SymbolPrimitive::Text { at, .. } => out.push(*at),
        SymbolPrimitive::BSpline { poles, .. } => out.extend(poles.iter().copied()),
        SymbolPrimitive::Boundary { members, .. } => {
            for member in members {
                points_of(member, out);
            }
        }
    }
}

fn bounds<'a>(primitives: impl IntoIterator<Item = &'a SymbolPrimitive>) -> Option<[f64; 4]> {
    let mut points = Vec::new();
    for primitive in primitives {
        points_of(primitive, &mut points);
    }
    let mut bounds: Option<[f64; 4]> = None;
    for (x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        match &mut bounds {
            None => bounds = Some([x, y, x, y]),
            Some(b) => {
                b[0] = b[0].min(x);
                b[1] = b[1].min(y);
                b[2] = b[2].max(x);
                b[3] = b[3].max(y);
            }
        }
    }
    bounds
}

#[derive(Default)]
struct Tally {
    pairs: usize,
    same: usize,
    parametric: usize,
    mismatched: usize,
    library_missing: usize,
    /// Placements the drawing does not carry a body for: the library is the
    /// only source, so its layer filter decides the whole shape.
    library_only: usize,
}

/// How much of a library body its own display filter keeps.
#[derive(Default)]
struct Filtering {
    bodies: usize,
    /// Bodies where the filter keeps under half the strokes.
    stripped: usize,
    /// Strokes dropped across every compared body.
    dropped: usize,
    total: usize,
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
        eprintln!("usage: probe_library_vs_cached_bodies <drawing.pid | dir> ... [--corpus DIR]");
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
    let mut tally = Tally::default();
    let mut filtering = Filtering::default();
    let mut stripped_symbols: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut compared: BTreeSet<(String, u32, u32)> = BTreeSet::new();
    let mut per_drawing: BTreeMap<String, Tally> = BTreeMap::new();

    for path in &expanded {
        let doc = match PidParser::new().parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(1);
            }
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let geometry = build_normalized_geometry(&doc);
        let mut local = Tally::default();
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
                if symbol_path.is_some() && definition.is_none() {
                    local.library_only += 1;
                }
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
                local.library_missing += 1;
                continue;
            };
            // Lettering is instance data -- the drawing supplies the value and
            // the library carries a `NULL` placeholder -- so it is not shape
            // and cannot agree or disagree.
            let is_shape =
                |primitive: &&SymbolPrimitive| !matches!(primitive, SymbolPrimitive::Text { .. });
            let cached: Vec<&SymbolPrimitive> = definition
                .visible_styled()
                .map(|(primitive, _, _)| primitive)
                .filter(is_shape)
                .collect();
            let all_library: Vec<&SymbolPrimitive> = body
                .primitives
                .iter()
                .map(|styled| &styled.primitive)
                .filter(is_shape)
                .collect();
            let from_library: Vec<&SymbolPrimitive> = body
                .displayed_primitives()
                .iter()
                .map(|styled| &styled.primitive)
                .filter(is_shape)
                .collect();
            filtering.bodies += 1;
            filtering.total += all_library.len();
            filtering.dropped += all_library.len().saturating_sub(from_library.len());
            if !all_library.is_empty() && from_library.len() * 2 < all_library.len() {
                filtering.stripped += 1;
                let entry = stripped_symbols
                    .entry(symbol_path.clone())
                    .or_insert((0, 0));
                entry.0 = all_library.len();
                entry.1 = from_library.len();
            }
            let cached_census = Census::of(cached.iter().copied());
            let library_census = Census::of(from_library.iter().copied());
            let cached_bounds = bounds(cached.iter().copied());
            let library_bounds = bounds(from_library.iter().copied());
            local.pairs += 1;

            let box_delta = match (cached_bounds, library_bounds) {
                (Some(a), Some(b)) => (a[0] - b[0])
                    .abs()
                    .max((a[1] - b[1]).abs())
                    .max((a[2] - b[2]).abs())
                    .max((a[3] - b[3]).abs()),
                (None, None) => 0.0,
                _ => f64::INFINITY,
            };
            // A body that names a template draws the instance's variables, so
            // its size is expected to differ from the library's defaults.
            let parametric = definition.template.is_some();
            if cached_census == library_census && box_delta <= TOLERANCE_M {
                local.same += 1;
                continue;
            }
            if parametric {
                local.parametric += 1;
                continue;
            }
            local.mismatched += 1;
            println!(
                "MISMATCH {name}: {symbol_path} (JSite{} sheet {})\n  cached  {}\n  library {}\n  box delta {:.6} mm",
                reference.site,
                reference.sheet,
                cached_census.describe(),
                library_census.describe(),
                box_delta * 1000.0
            );
        }
        tally.pairs += local.pairs;
        tally.same += local.same;
        tally.parametric += local.parametric;
        tally.mismatched += local.mismatched;
        tally.library_missing += local.library_missing;
        tally.library_only += local.library_only;
        per_drawing.insert(name, local);
    }

    for (name, local) in &per_drawing {
        if local.mismatched > 0 {
            println!(
                "{name}: {} pairs, {} same, {} parametric, {} mismatched",
                local.pairs, local.same, local.parametric, local.mismatched
            );
        }
    }
    println!(
        "\n{} drawings, {} distinct placed bodies: pairs={} same={} parametric={} mismatched={} library_missing={} library_only_placements={}",
        expanded.len(),
        compared.len(),
        tally.pairs,
        tally.same,
        tally.parametric,
        tally.mismatched,
        tally.library_missing,
        tally.library_only
    );
    println!(
        "library bodies: {} compared, {} strokes, {} dropped by the display filter, {} bodies lost over half",
        filtering.bodies, filtering.total, filtering.dropped, filtering.stripped
    );
    let mut stripped: Vec<(String, (usize, usize))> = stripped_symbols.into_iter().collect();
    stripped.sort_by_key(|(_, (all, shown))| (all - shown).max(1));
    for (symbol, (all, shown)) in stripped.iter().rev().take(12) {
        println!("  stripped {shown}/{all} {symbol}");
    }
    ExitCode::SUCCESS
}
