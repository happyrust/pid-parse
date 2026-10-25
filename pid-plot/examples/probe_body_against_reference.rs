//! Score every stroke of a placed symbol's library body against the reference
//! plot's ink, so a body that looks stripped can be told apart from one the
//! vendor really printed nearly empty.
//!
//! `SymbolGeometry::displayed_primitives` keeps a subset of a `.sym` body's
//! strokes: the sheet that presents the body's normal view, and the layers
//! that view displays. When that subset is one stroke out of eight, the
//! question is not what the filter did — `probe_sym_layer_filter` answers that
//! per layer — but whether the reference plot inks the other seven. This probe
//! answers it: it places the whole body (every stroke, hidden layers included)
//! through the placement's own transform and reports, per stroke, how much of
//! it lies on reference ink.
//!
//! ```text
//! cargo run -p pid-plot --example probe_body_against_reference -- \
//!     <reference.pdf> <drawing.pid> [symbol basename]
//! cargo run -p pid-plot --example probe_body_against_reference -- --package
//! cargo run -p pid-plot --example probe_body_against_reference -- --residual
//! cargo run -p pid-plot --example probe_body_against_reference -- \
//!     --window <x0> <y0> <x1> <y1> <drawing.pid>
//! ```
//!
//! `--package` is the same screening over every sheet the plant backup holds:
//! each sheet's placed bodies whose filter keeps under half their strokes, with
//! the missing ink summed over the sheet's placements. Only the sheets that have
//! a reference plot beside the corpus can be scored — a sheet without one still
//! lists its halved bodies, marked `-` — and the list is sorted by missing ink.
//!
//! `--residual` turns the same reading around: rather than scoring our bodies, it
//! takes the reference ink no page stroke covers and splits it into contiguous
//! runs, each attributed to the page stroke nearest it (its shape, the symbol or
//! stream it came from, and how far the run strays). That is what tells a mark
//! that is merely off-lying from ink nothing on the page draws.
//!
//! A run that falls inside a border-template placement also names the body that
//! placement was drawn from, and the sweep totals those separately: ink inside a
//! placement the library has no `.sym` for is a source the corpus does not carry
//! (the page could only draw it from the file's own cache, or not at all), never
//! a decoding miss. That is the `missing sources` section, and `BLFH.sym` is the
//! case that put it there.
//!
//! `--window` prints one paper-millimetre window of a drawing and of its reference
//! plot side by side: every page stroke whose chain enters the box, with its
//! provenance, and every reference stroke whose midpoint does, with how far it is
//! from the page.
//!
//! The drawing is the reference plot's own source (md5-equal `.pid`), the
//! library is the share the drawing names, and the basename defaults to the
//! body whose filter keeps the smallest fraction of its strokes. Read-only.
//!
//! A mirrored placement negates the sweep of every arc, so the sampling is done
//! in the symbol's own space and every sample is mapped through
//! `place_metres`, never by transforming a centre and a sweep separately.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{NameMiss, SymbolLibrary, SymbolPrimitive};
use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};
use pid_plot::model::{PaperModel, PaperShape, SymbolBodySource, lay_out_drawing, place_metres};
use pid_plot::read::read_plot;

/// How near a sample must land to reference ink to count as covered, in mm.
///
/// The reference plot's own strokes are read back at print precision; 0.5 mm
/// is the pairing window the plot comparison uses for the same reason.
const COVER_MM: f64 = 0.5;
/// Sampling pitch, in mm of chord, spelled in the symbol's own unit: the
/// chains below are built in metres (that is what a body's coordinates are),
/// and 0.2 mm of chord is what [`COVER_MM`] can resolve at.
const SAMPLE_M: f64 = 0.000_2;
/// The same pitch in paper millimetres, for walking the page's own shapes.
const SAMPLE_MM: f64 = 0.2;
/// Spatial hash cell, in mm. Comfortably wider than [`COVER_MM`], so the
/// `±1` cell neighbourhood of a sample holds every candidate stroke.
const CELL_MM: f64 = 5.0;
/// Two strokes are the same shape when every sample of one lies this close to
/// the other, in mm. A tenth of the print's own line width: a retraced outline
/// matches, a stroke a pen-width away does not.
const SAME_MM: f64 = 0.05;
/// How near a page stroke a sample must land to count as already drawn, in mm.
/// Looser than [`SAME_MM`] because two different strokes may legitimately
/// coincide (a pipe under a label's frame), and the question there is whether
/// the page inks the place, not whether it inks it with this shape.
const PAGE_MM: f64 = 0.1;
/// The tight reading: a stroke the print really draws lies on ink this close,
/// in mm. A stroke that merely runs *parallel* to printed ink (a pipe wall, a
/// neighbouring outline, a glyph curve) reads high at [`COVER_MM`] and near
/// zero here — 0.5 mm is half a line width, wide enough for a line 0.3 mm off
/// to count along its whole length.
const TIGHT_MM: f64 = 0.1;
const TAU: f64 = std::f64::consts::TAU;

type Stroke = ([f64; 2], [f64; 2]);

struct Grid {
    buckets: HashMap<(i32, i32), Vec<Stroke>>,
}

impl Grid {
    fn new(segments: &[Stroke]) -> Self {
        let mut buckets: HashMap<(i32, i32), Vec<Stroke>> = HashMap::new();
        for segment in segments {
            let (a, b) = *segment;
            let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
            let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
            for cx in ((x0 / CELL_MM).floor() as i32 - 1)..=((x1 / CELL_MM).floor() as i32 + 1) {
                for cy in ((y0 / CELL_MM).floor() as i32 - 1)..=((y1 / CELL_MM).floor() as i32 + 1)
                {
                    buckets.entry((cx, cy)).or_default().push(*segment);
                }
            }
        }
        Self { buckets }
    }

    /// The strokes filed under any cell the box touches.
    ///
    /// A stroke may sit in several cells; the linear dedupe is cheap because a
    /// query box is a few millimetres across.
    fn strokes_in(&self, min: [f64; 2], max: [f64; 2]) -> Vec<Stroke> {
        let mut found: Vec<Stroke> = Vec::new();
        let (cx0, cx1) = (
            (min[0] / CELL_MM).floor() as i32,
            (max[0] / CELL_MM).floor() as i32,
        );
        let (cy0, cy1) = (
            (min[1] / CELL_MM).floor() as i32,
            (max[1] / CELL_MM).floor() as i32,
        );
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                let Some(bucket) = self.buckets.get(&(cx, cy)) else {
                    continue;
                };
                for stroke in bucket {
                    if !found.contains(stroke) {
                        found.push(*stroke);
                    }
                }
            }
        }
        found
    }

    fn distance(&self, point: [f64; 2]) -> f64 {
        let key = (
            (point[0] / CELL_MM).floor() as i32,
            (point[1] / CELL_MM).floor() as i32,
        );
        let mut best = f64::MAX;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) else {
                    continue;
                };
                for (a, b) in bucket {
                    best = best.min(distance_to_segment(point, *a, *b));
                }
            }
        }
        best
    }
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    if length <= 1e-12 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let t = (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length).clamp(0.0, 1.0);
    (point[0] - (a[0] + t * dx)).hypot(point[1] - (a[1] + t * dy))
}

fn line_chain(start: [f64; 2], end: [f64; 2], pitch: f64) -> Vec<[f64; 2]> {
    let length = (end[0] - start[0]).hypot(end[1] - start[1]);
    let steps = ((length / pitch).ceil() as usize).clamp(2, 20_000);
    (0..=steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            [
                start[0] + (end[0] - start[0]) * t,
                start[1] + (end[1] - start[1]) * t,
            ]
        })
        .collect()
}

fn arc_chain(center: [f64; 2], radius: f64, start: f64, sweep: f64, pitch: f64) -> Vec<[f64; 2]> {
    let steps = ((sweep.abs() * radius / pitch).ceil() as usize).clamp(4, 20_000);
    (0..=steps)
        .map(|step| {
            let angle = start + sweep * (step as f64 / steps as f64);
            [
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

fn covered(points: &[[f64; 2]], grid: &Grid) -> f64 {
    covered_within(points, grid, COVER_MM)
}

/// The fraction of `points` within `tolerance` mm of the grid's ink.
fn covered_within(points: &[[f64; 2]], grid: &Grid, tolerance: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let hits = points
        .iter()
        .filter(|point| grid.distance(**point) <= tolerance)
        .count();
    hits as f64 / points.len() as f64
}

/// One stroke scored against the reference: how much lies on ink at the loose
/// [`COVER_MM`] and at the tight [`TIGHT_MM`], and how long it is in paper
/// millimetres.
struct Scored {
    ink: f64,
    tight: f64,
    length: f64,
}

/// Score one shape: how much of it lies on reference ink, and how much ink it
/// would draw, both in paper millimetres.
fn score(
    local: &[[f64; 2]],
    insertion: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    grid: &Grid,
) -> Scored {
    let points: Vec<[f64; 2]> = local
        .iter()
        .map(|point| place_metres(*point, insertion, rotation, scale))
        .collect();
    let length = points
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .sum();
    Scored {
        ink: covered(&points, grid),
        tight: covered_within(&points, grid, TIGHT_MM),
        length,
    }
}

/// The `ink …` column, blank for a shape this probe does not walk.
fn ink_column(scored: Option<&Scored>) -> String {
    match scored {
        Some(scored) => format!(
            "  ink {:.2}  tight {:.2}  len {:6.1} mm",
            scored.ink, scored.tight, scored.length
        ),
        None => "  not scored (lettering or fill)".to_string(),
    }
}

/// How far `point` is from a stroke given as a polyline, in mm.
fn polyline_distance(point: [f64; 2], stroke: &[[f64; 2]]) -> f64 {
    stroke
        .windows(2)
        .map(|pair| distance_to_segment(point, pair[0], pair[1]))
        .fold(f64::MAX, f64::min)
}

/// Whether two strokes are the same shape: every point of `one` lies within
/// [`SAME_MM`] of `other`.
///
/// A heat-trace stroke is often the body's own outline traced again, and a
/// dropped copy of it is not missing ink — the page already draws the stroke it
/// lies on. Only a dropped stroke that matches nothing in the cache is ink the
/// page does not print.
fn same_stroke(one: &[[f64; 2]], other: &[[f64; 2]]) -> bool {
    !one.is_empty()
        && !other.is_empty()
        && one
            .iter()
            .all(|point| polyline_distance(*point, other) <= SAME_MM)
}

/// The cache stroke `local` coincides with, or `None`.
fn cache_match(translated: &[[f64; 2]], cache: &[(bool, Vec<[f64; 2]>)]) -> Option<(usize, bool)> {
    cache
        .iter()
        .position(|(_, stroke)| same_stroke(translated, stroke))
        .map(|index| (index, cache[index].0))
}

/// How much of a stroke the page already inks: the fraction of its samples
/// within [`PAGE_MM`] of any stroke the whole page draws.
///
/// This is the arbiter between the other two columns. A dropped stroke the
/// reference inks is harmless when this is high — the page draws that line as
/// some other entity's stroke — and is missing ink when it is low. Measuring
/// against the page and not against the body's own cache matters: a remarks
/// box, a leader or a pipe is drawn by an entity of its own.
fn drawn_cover(points: &[[f64; 2]], page: &Grid) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let hits = points
        .iter()
        .filter(|point| page.distance(**point) <= PAGE_MM)
        .count();
    hits as f64 / points.len() as f64
}

/// Reference ink near a stroke that nothing on the page draws, in millimetres.
///
/// This is the measure that survives the reference plot's small systematic
/// offset: rather than asking whether the *stroke* is on ink (which a line
/// running 0.3 mm parallel also passes), it asks whether the reference ink in
/// the stroke's neighbourhood is explained by some page stroke at all. A
/// printed-but-missing stroke leaves its own length unexplained; a stroke whose
/// "ink" is a neighbouring pipe leaves nothing.
fn unexplained_ink(points: &[[f64; 2]], reference: &Grid, page: &Grid) -> f64 {
    let Some(first) = points.first() else {
        return 0.0;
    };
    let mut min = *first;
    let mut max = *first;
    for point in points {
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
    }
    // The corridor a printed stroke of this size would occupy.
    let reach = 0.6;
    min = [min[0] - reach, min[1] - reach];
    max = [max[0] + reach, max[1] + reach];

    let mut unexplained = 0.0;
    for (a, b) in reference.strokes_in(min, max) {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let steps = ((length / SAMPLE_MM).ceil() as usize).clamp(2, 20_000);
        let pitch = length / steps as f64;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let point = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if point[0] < min[0] || point[0] > max[0] || point[1] < min[1] || point[1] > max[1] {
                continue;
            }
            // The tube around the stroke itself, not its bounding box: glyph
            // ink a label prints inside a disc is the page's own lettering,
            // and this probe's page grid carries no glyphs to explain it with.
            if polyline_distance(point, points) > reach {
                continue;
            }
            if page.distance(point) > COVER_MM {
                unexplained += pitch;
            }
        }
    }
    unexplained
}

/// Every reference stroke's length that no page stroke covers, in millimetres.
///
/// The same reading as [`unexplained_ink`], taken over the whole page: this is
/// the denominator the per-body figures are read against, and it also proves
/// the measure is alive (a page that misses nothing at all would make a
/// halved body's zero meaningless).
fn unexplained_segments(segments: &[Stroke], page: &Grid) -> f64 {
    let mut unexplained = 0.0;
    for (a, b) in segments {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let steps = ((length / SAMPLE_MM).ceil() as usize).clamp(2, 20_000);
        let pitch = length / steps as f64;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let point = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if page.distance(point) > COVER_MM {
                unexplained += pitch;
            }
        }
    }
    unexplained
}

/// The last `count` components of a symbol path, so two bodies that share a
/// basename under different shares stay apart in the table.
fn path_tail(path: &str, count: usize) -> String {
    let parts: Vec<&str> = path
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    parts[parts.len().saturating_sub(count)..].join("/")
}

/// The symbol's own points for one stroke, in metres, or `None` for a shape
/// this probe does not walk (lettering, fills, members of a boundary).
fn local_points(primitive: &SymbolPrimitive) -> Option<Vec<[f64; 2]>> {
    match primitive {
        SymbolPrimitive::Line { start, end } => {
            Some(line_chain([start.0, start.1], [end.0, end.1], SAMPLE_M))
        }
        SymbolPrimitive::Circle { center, radius } => {
            Some(arc_chain([center.0, center.1], *radius, 0.0, TAU, SAMPLE_M))
        }
        SymbolPrimitive::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let sweep = if *clockwise {
                -(*start_angle - *end_angle).rem_euclid(TAU)
            } else {
                (*end_angle - *start_angle).rem_euclid(TAU)
            };
            Some(arc_chain(
                [center.0, center.1],
                *radius,
                *start_angle,
                sweep,
                SAMPLE_M,
            ))
        }
        SymbolPrimitive::Polyline { vertices, .. } => {
            let mut points = Vec::new();
            for pair in vertices.windows(2) {
                points.extend(line_chain(
                    [pair[0].0, pair[0].1],
                    [pair[1].0, pair[1].1],
                    SAMPLE_M,
                ));
            }
            Some(points)
        }
        SymbolPrimitive::BSpline { .. } => {
            let points = primitive.bspline_points(8);
            let mut chain = Vec::new();
            for pair in points.windows(2) {
                chain.extend(line_chain(
                    [pair[0].0, pair[0].1],
                    [pair[1].0, pair[1].1],
                    SAMPLE_M,
                ));
            }
            Some(chain)
        }
        SymbolPrimitive::Text { .. } | SymbolPrimitive::Boundary { .. } => None,
    }
}

/// The page points of one paper shape, in millimetres, or `None` for the two
/// shapes this probe does not walk (a fill's ring is painted, and lettering is
/// glyphs rather than a stroke).
fn paper_points(shape: &PaperShape) -> Option<Vec<[f64; 2]>> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => Some(line_chain(*start_mm, *end_mm, SAMPLE_MM)),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let sweep = if *clockwise {
                -(*start_angle - *end_angle).rem_euclid(TAU)
            } else {
                (*end_angle - *start_angle).rem_euclid(TAU)
            };
            Some(arc_chain(
                *center_mm,
                *radius_mm,
                *start_angle,
                sweep,
                SAMPLE_MM,
            ))
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => Some(arc_chain(*center_mm, *radius_mm, 0.0, TAU, SAMPLE_MM)),
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = Vec::new();
            let mut chain = points_mm.clone();
            if *closed && chain.len() > 1 {
                chain.push(chain[0]);
            }
            for pair in chain.windows(2) {
                points.extend(line_chain(pair[0], pair[1], SAMPLE_MM));
            }
            Some(points)
        }
        PaperShape::Fill { chain_mm, .. } => {
            let mut points = Vec::new();
            let mut chain = chain_mm.clone();
            if chain.len() > 1 {
                chain.push(chain[0]);
            }
            for pair in chain.windows(2) {
                points.extend(line_chain(pair[0], pair[1], SAMPLE_MM));
            }
            Some(points)
        }
        PaperShape::Text { .. } => None,
    }
}

fn kind(primitive: &SymbolPrimitive) -> &'static str {
    match primitive {
        SymbolPrimitive::Line { .. } => "line",
        SymbolPrimitive::Circle { .. } => "circle",
        SymbolPrimitive::Arc { .. } => "arc",
        SymbolPrimitive::Polyline { .. } => "polyline",
        SymbolPrimitive::Text { .. } => "text",
        SymbolPrimitive::BSpline { .. } => "bspline",
        SymbolPrimitive::Boundary { .. } => "boundary",
    }
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// Where the shelled-out plant backup sits, if this machine has one.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(corpus_dir());
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

/// The share directories a corpus drawing names its symbols under.
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

fn base_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// One placement of a body on a sheet: the transform the page applies to it,
/// and whether the drawing caches a body for it.
struct Placement {
    insertion: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    cached: bool,
}

/// One placement's read of a dropped stroke.
struct Spot {
    /// Insertion point of the placement, in metres.
    insertion: [f64; 2],
    /// Reference ink the page leaves unexplained in this stroke's tube, in mm.
    left_mm: f64,
    /// Share of the dropped stroke the reference inks (0.5 mm window).
    ref_ink: f64,
    /// Share of it this page draws.
    page: f64,
}

/// One stroke a body's filter dropped, and how it reads over the placements.
struct Dropped {
    kind: &'static str,
    /// Ink left unexplained by this stroke, summed over the placements, in mm.
    left_mm: f64,
    /// The placements that contributed any, in placement order.
    spots: Vec<Spot>,
}

/// One screened body: a library body placed on one sheet whose display filter
/// keeps under half its strokes.
struct Screened {
    sheet: String,
    body: String,
    placements: usize,
    /// How many of those placements have no body cached in the drawing, so the
    /// page draws this filtered library body itself. A halved body only costs
    /// ink where its filter is the last word; where the drawing caches the body,
    /// the cache decides and the filter is moot.
    library_only: usize,
    strokes: usize,
    kept: usize,
    /// Reference ink the page does not explain near the dropped strokes, summed
    /// over every placement of this body on this sheet, in millimetres. Zero
    /// for a sheet with no reference plot to measure against.
    missing_mm: f64,
    /// Whether the sheet has a reference plot; without one the body is listed
    /// but not scored.
    scored: bool,
    /// Each dropped stroke, in body order.
    per_stroke: Vec<Dropped>,
}

/// The `--package` sweep: every sheet the plant backup holds, every body its
/// library filter halves, sorted by how much reference ink goes unprinted.
///
/// A body counts as halved when its own display filter keeps fewer than half the
/// strokes the `.sym` carries — the shape `Local Vent` has, not the ordinary
/// case of a label layer being filed off. The ink figure is the same
/// `unexplained_ink` reading the single-drawing probe makes: reference ink in
/// the corridor a dropped stroke would occupy that no page stroke draws.
fn package_sweep() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        println!("no backup-plant-data directory; nothing to sweep");
        return Ok(());
    };
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    println!("{} sheets under {}", drawings.len(), backup.display());

    let mut library = SymbolLibrary::with_roots(library_roots(&corpus));
    let mut rows: Vec<Screened> = Vec::new();
    // Each scored sheet: its name, its reference ink length and the length of
    // that ink no page stroke covers, in millimetres.
    let mut sheet_totals: Vec<(String, f64, f64)> = Vec::new();
    let (mut sheets_scored, mut bodies_seen, mut bodies_halved) = (0usize, 0usize, 0usize);
    for drawing in &drawings {
        let stem = drawing
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        let reference = corpus.join(format!("{stem}.pdf"));

        // Only a sheet with a reference plot needs the page laid out: the
        // scorer's "does anything print here" side is the whole page.
        let (grid, page_grid) = if reference.is_file() {
            sheets_scored += 1;
            let plot = read_plot(&std::fs::read(&reference)?)?;
            let segments: Vec<Stroke> = plot
                .segments
                .iter()
                .map(|segment| (segment.start_mm, segment.end_mm))
                .collect();
            let model = lay_out_drawing(drawing, &corpus)?;
            let mut page_segments: Vec<Stroke> = Vec::new();
            for primitive in &model.primitives {
                let Some(points) = paper_points(&primitive.shape) else {
                    continue;
                };
                page_segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
            }
            let page = Grid::new(&page_segments);
            // The sheet's own denominator: every reference stroke not covered
            // by a page stroke. A few millimetres of halved-body ink means
            // nothing without knowing what the page as a whole leaves.
            sheet_totals.push((
                stem.clone(),
                segments
                    .iter()
                    .map(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1]))
                    .sum(),
                unexplained_segments(&segments, &page),
            ));
            (Some(Grid::new(&segments)), Some(page))
        } else {
            (None, None)
        };

        let doc = PidParser::new().parse_file(drawing)?;
        let geometry = build_normalized_geometry(&doc);
        let mut by_path: BTreeMap<String, Vec<Placement>> = BTreeMap::new();
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                symbol_path: Some(path),
                insertion,
                rotation,
                scale,
                definition,
            } = &entity.kind
            else {
                continue;
            };
            by_path.entry(path.clone()).or_default().push(Placement {
                insertion: [insertion.x, insertion.y],
                rotation: *rotation,
                scale: *scale,
                cached: definition
                    .and_then(|reference| geometry.symbol_definition(reference))
                    .is_some(),
            });
        }

        for (path, placements) in &by_path {
            bodies_seen += 1;
            let Some(body) = library.resolve(path) else {
                continue;
            };
            let shown: Vec<usize> = body
                .displayed_primitives()
                .iter()
                .map(|styled| {
                    body.primitives
                        .iter()
                        .position(|candidate| std::ptr::eq(candidate, *styled))
                        .unwrap_or(usize::MAX)
                })
                .collect();
            if shown.len() * 2 >= body.primitives.len() {
                continue;
            }
            bodies_halved += 1;
            let mut row = Screened {
                sheet: stem.clone(),
                body: path_tail(path, 2),
                placements: placements.len(),
                library_only: placements.iter().filter(|placed| !placed.cached).count(),
                strokes: body.primitives.len(),
                kept: shown.len(),
                missing_mm: 0.0,
                scored: grid.is_some(),
                per_stroke: Vec::new(),
            };
            for (index, styled) in body.primitives.iter().enumerate() {
                if shown.contains(&index) {
                    continue;
                }
                let Some(local) = local_points(&styled.primitive) else {
                    continue;
                };
                let mut left_mm = 0.0;
                let mut spots: Vec<Spot> = Vec::new();
                for placed in placements {
                    let translated: Vec<[f64; 2]> = local
                        .iter()
                        .map(|point| {
                            place_metres(*point, placed.insertion, placed.rotation, placed.scale)
                        })
                        .collect();
                    if let (Some(grid), Some(page)) = (&grid, &page_grid) {
                        let here = unexplained_ink(&translated, grid, page);
                        if here > 0.0 {
                            spots.push(Spot {
                                insertion: placed.insertion,
                                left_mm: here,
                                ref_ink: covered(&translated, grid),
                                page: drawn_cover(&translated, page),
                            });
                        }
                        left_mm += here;
                    }
                }
                row.missing_mm += left_mm;
                row.per_stroke.push(Dropped {
                    kind: kind(&styled.primitive),
                    left_mm,
                    spots,
                });
            }
            rows.push(row);
        }
    }

    // A sheet without a reference plot cannot be ranked by ink, so its rows
    // follow every scored one; among themselves they rank by how much the
    // filter took.
    rows.sort_by(|a, b| {
        b.scored
            .cmp(&a.scored)
            .then_with(|| b.missing_mm.total_cmp(&a.missing_mm))
            .then_with(|| (b.strokes - b.kept).cmp(&(a.strokes - a.kept)))
            .then_with(|| a.sheet.cmp(&b.sheet))
            .then_with(|| a.body.cmp(&b.body))
    });

    println!(
        "\n{bodies_seen} placed bodies, {bodies_halved} halved by their filter; \
         {sheets_scored} of {} sheets have a reference plot",
        drawings.len()
    );
    let mut total = 0.0;
    println!(
        "\n{:<40} {:<44} {:>4} {:>4} {:>5} {:>4} {:>9}",
        "sheet", "body", "str", "keep", "inst", "lib", "missing"
    );
    for row in &rows {
        total += row.missing_mm;
        let missing = if row.scored {
            format!("{:.1} mm", row.missing_mm)
        } else {
            "-".to_string()
        };
        println!(
            "{:<40} {:<44} {:>4} {:>4} {:>5} {:>4} {:>9}",
            row.sheet, row.body, row.strokes, row.kept, row.placements, row.library_only, missing
        );
    }
    println!("\ntotal unexplained ink on scored sheets: {total:.1} mm");

    println!("\nper scored sheet (the denominator the body figures are read against):");
    println!(
        "{:<44} {:>10} {:>10} {:>7}",
        "sheet", "ref ink", "unexpl.", "share"
    );
    for (sheet, reference_mm, unexplained_mm) in &sheet_totals {
        println!(
            "{:<44} {:>8.0} mm {:>9.1} {:>6.2}%",
            sheet,
            reference_mm,
            unexplained_mm,
            if *reference_mm > 0.0 {
                100.0 * unexplained_mm / reference_mm
            } else {
                0.0
            }
        );
    }

    println!("\nthe strokes that carry it (>= 1 mm each):");
    for row in &rows {
        let heavy: Vec<&Dropped> = row
            .per_stroke
            .iter()
            .filter(|dropped| dropped.left_mm >= 1.0)
            .collect();
        if heavy.is_empty() {
            continue;
        }
        println!(
            "\n{} / {}  ({} of {} kept, {} placements, {} library-only)",
            row.sheet, row.body, row.kept, row.strokes, row.placements, row.library_only
        );
        for dropped in heavy {
            println!("    {:<9} {:6.1} mm", dropped.kind, dropped.left_mm);
            for spot in &dropped.spots {
                if spot.left_mm >= 0.3 {
                    println!(
                        "        at ({:.3},{:.3}) m  left {:.1} mm  ref ink {:.2}  page {:.2}",
                        spot.insertion[0], spot.insertion[1], spot.left_mm, spot.ref_ink, spot.page
                    );
                }
            }
        }
    }
    Ok(())
}

/// One page stroke kept with the provenance a residual is attributed to.
struct PageShape {
    chain: Vec<[f64; 2]>,
    kind: &'static str,
    symbol: Option<String>,
    stream: Option<String>,
    dashed: bool,
    furniture: bool,
    min: [f64; 2],
    max: [f64; 2],
}

fn bbox_of(points: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    let mut min = points.first().copied().unwrap_or([0.0; 2]);
    let mut max = min;
    for point in points {
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
    }
    (min, max)
}

fn box_gap(one: ([f64; 2], [f64; 2]), other: ([f64; 2], [f64; 2])) -> f64 {
    let dx = (other.0[0] - one.1[0]).max(one.0[0] - other.1[0]).max(0.0);
    let dy = (other.0[1] - one.1[1]).max(one.0[1] - other.1[1]).max(0.0);
    dx.hypot(dy)
}

/// A label's extent, as the box its baseline run and body size span.
///
/// Coarse on purpose: the question a residual asks of a label is whether one is
/// there at all (`read_plot` keeps glyphs out of `segments`, so a label can
/// never explain ink directly), not how wide each character is.
fn text_box(origin: [f64; 2], height: f64, rotation: f64, value: &str) -> Vec<[f64; 2]> {
    let width = value.chars().count() as f64 * height * 0.62;
    let (sin, cos) = rotation.sin_cos();
    let along = [cos, sin];
    let across = [-sin, cos];
    let corner = |u: f64, v: f64| {
        [
            origin[0] + along[0] * u + across[0] * v,
            origin[1] + along[1] * u + across[1] * v,
        ]
    };
    let ring = [
        corner(0.0, 0.0),
        corner(width, 0.0),
        corner(width, height),
        corner(0.0, height),
        corner(0.0, 0.0),
    ];
    ring.to_vec()
}

/// Every page stroke, walked into a chain and tagged with where it came from.
fn page_shapes(model: &PaperModel) -> Vec<PageShape> {
    let mut shapes: Vec<PageShape> = Vec::new();
    for primitive in &model.primitives {
        let (kind, chain) = match &primitive.shape {
            PaperShape::Line { .. } => ("line", paper_points(&primitive.shape).unwrap_or_default()),
            PaperShape::Arc { .. } => ("arc", paper_points(&primitive.shape).unwrap_or_default()),
            PaperShape::Circle { .. } => {
                ("circle", paper_points(&primitive.shape).unwrap_or_default())
            }
            PaperShape::Polyline { closed, .. } => (
                if *closed { "ring" } else { "polyline" },
                paper_points(&primitive.shape).unwrap_or_default(),
            ),
            PaperShape::Fill { .. } => ("fill", paper_points(&primitive.shape).unwrap_or_default()),
            PaperShape::Text {
                origin_mm,
                height_mm,
                rotation,
                value,
                ..
            } => (
                "text",
                text_box(*origin_mm, height_mm.unwrap_or(3.0), *rotation, value),
            ),
        };
        if chain.len() < 2 {
            continue;
        }
        let (min, max) = bbox_of(&chain);
        shapes.push(PageShape {
            chain,
            kind,
            symbol: primitive.symbol.as_deref().map(|path| path_tail(path, 2)),
            stream: primitive
                .stream_path
                .as_deref()
                .map(|path| path_tail(path, 1)),
            dashed: primitive
                .stroke
                .as_ref()
                .is_some_and(|stroke| !stroke.dash_mm.is_empty()),
            furniture: primitive.furniture,
            min,
            max,
        });
    }
    shapes
}

/// One stretch of reference ink no page stroke covers, with the page stroke
/// nearest to it.
struct Residual {
    a: [f64; 2],
    b: [f64; 2],
    mm: f64,
    near_kind: &'static str,
    near_symbol: Option<String>,
    near_stream: Option<String>,
    near_dashed: bool,
    near_furniture: bool,
    gap: f64,
    gap_far: f64,
    /// The border-template placement the run sits in, when that placement had
    /// to be drawn from something other than the `.sym` it names: the body it
    /// fell back to, the key it names, and — when the name is the reason —
    /// whether the library lacks the file or carries that name twice. Either
    /// way the gap is the source's business, not a decoder's.
    template_source: Option<(SymbolBodySource, String, Option<NameMiss>)>,
}

/// The template's own symbol account: what drew each placement, and which of
/// them had no `.sym` to be read from.
fn print_template_sources(model: &PaperModel) {
    let Some(template) = model.template.as_ref() else {
        return;
    };
    if template.symbols.is_empty() {
        return;
    }
    // A sheet whose every placement read its `.sym` has no source gap to name.
    if template
        .symbols
        .iter()
        .all(|symbol| symbol.body == SymbolBodySource::Library)
    {
        return;
    }
    let count = |wanted: fn(&SymbolBodySource) -> bool| {
        template
            .symbols
            .iter()
            .filter(|symbol| wanted(&symbol.body))
            .count()
    };
    let cached_only = template
        .symbols
        .iter()
        .filter(|symbol| symbol.body == SymbolBodySource::Cached && !symbol.library_available)
        .count();
    println!(
        "  template symbols: {} placed — {} from the .sym, {} drawn from the cache ({} of them \
         with no .sym at all), {} with nothing to draw",
        template.symbols.len(),
        count(|body| *body == SymbolBodySource::Library),
        count(|body| *body == SymbolBodySource::Cached),
        cached_only,
        count(|body| *body == SymbolBodySource::Missing),
    );
    for symbol in &template.symbols {
        let tag = match symbol.body {
            SymbolBodySource::Library => continue,
            SymbolBodySource::Cached if symbol.library_available => "cache (the .sym is here too)",
            SymbolBodySource::Cached => "cache only (the .sym is absent)",
            SymbolBodySource::Missing => "undrawn (no .sym, no cache)",
        };
        println!(
            "    {tag:<32} {}  at ({:.2},{:.2}) mm{}",
            symbol.key,
            symbol.insertion_mm[0],
            symbol.insertion_mm[1],
            symbol
                .extent_mm
                .map(|[min, max]| format!(
                    "  ink ({:.2},{:.2})..({:.2},{:.2})",
                    min[0], min[1], max[0], max[1]
                ))
                .unwrap_or_default()
        );
    }
}

/// The page stroke nearest a run, and how far the run strays from it.
fn attribute(run: &[[f64; 2]], shapes: &[PageShape]) -> Option<(usize, f64, f64)> {
    let (min, max) = bbox_of(run);
    let mut best: Option<(usize, f64, f64)> = None;
    for (index, shape) in shapes.iter().enumerate() {
        if box_gap((min, max), (shape.min, shape.max)) > 6.0 {
            continue;
        }
        let mut near = f64::MAX;
        let mut far = 0.0_f64;
        for point in run {
            let here = polyline_distance(*point, &shape.chain);
            near = near.min(here);
            far = far.max(here);
        }
        if best.is_none_or(|(_, gap, _)| near < gap) {
            best = Some((index, near, far));
        }
    }
    best
}

/// Split one reference stroke into the stretches its page leaves unexplained.
fn unexplained_runs(
    a: [f64; 2],
    b: [f64; 2],
    page: &Grid,
    shapes: &[PageShape],
    model: &PaperModel,
    out: &mut Vec<Residual>,
) {
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let steps = ((length / SAMPLE_MM).ceil() as usize).clamp(2, 20_000);
    let pitch = length / steps as f64;
    let points: Vec<[f64; 2]> = (0..=steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        })
        .collect();
    let open: Vec<bool> = points
        .iter()
        .map(|point| page.distance(*point) > COVER_MM)
        .collect();
    let mut start: Option<usize> = None;
    for index in 0..=steps {
        match (open[index], start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                let run = &points[from..index];
                push_run(run, pitch, shapes, model, out);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        push_run(&points[from..], pitch, shapes, model, out);
    }
}

fn push_run(
    run: &[[f64; 2]],
    pitch: f64,
    shapes: &[PageShape],
    model: &PaperModel,
    out: &mut Vec<Residual>,
) {
    let (Some(first), Some(last)) = (run.first(), run.last()) else {
        return;
    };
    // Every sample stands for one pitch of stroke, and a run keeps both ends.
    let mm = (last[0] - first[0]).hypot(last[1] - first[1]) + pitch;
    let (near_kind, near_symbol, near_stream, near_dashed, near_furniture, gap, gap_far) =
        match attribute(run, shapes) {
            Some((index, gap, far)) => {
                let shape = &shapes[index];
                (
                    shape.kind,
                    shape.symbol.clone(),
                    shape.stream.clone(),
                    shape.dashed,
                    shape.furniture,
                    gap,
                    far,
                )
            }
            None => (
                "nothing",
                None,
                None,
                false,
                false,
                f64::INFINITY,
                f64::INFINITY,
            ),
        };
    // A run that falls inside a border-template placement the file could only
    // draw from its own cache is a source the corpus does not carry. Naming it
    // keeps the next round from re-reading the same gap as a decoder's.
    let centre = [(first[0] + last[0]) / 2.0, (first[1] + last[1]) / 2.0];
    let template_source =
        model
            .template_symbol_at(centre, 1.0)
            .and_then(|symbol| match symbol.body {
                SymbolBodySource::Cached if !symbol.library_available => Some((
                    SymbolBodySource::Cached,
                    symbol.key.clone(),
                    symbol.name_gap,
                )),
                SymbolBodySource::Missing => Some((
                    SymbolBodySource::Missing,
                    symbol.key.clone(),
                    symbol.name_gap,
                )),
                _ => None,
            });
    out.push(Residual {
        a: *first,
        b: *last,
        mm,
        near_kind,
        near_symbol,
        near_stream,
        near_dashed,
        near_furniture,
        gap,
        gap_far,
        template_source,
    });
}

/// The border templates' own symbol account, over every sheet the backup holds.
///
/// A template placement names a `.sym`; the sheet can be drawn from that body,
/// from the copy the file carries, or not at all. The last two are sources the
/// corpus does not carry, and naming them once here keeps every later residual
/// in their neighbourhood from being read as a decoder's miss.
fn template_ledger(backup: &Path, corpus: &Path) {
    let mut drawings = Vec::new();
    collect_pids(backup, &mut drawings);
    let mut rows: BTreeMap<(String, u8), (usize, BTreeSet<String>)> = BTreeMap::new();
    let mut drawn_from_library = 0usize;
    let mut sheets = 0usize;
    let mut failed = 0usize;
    for drawing in &drawings {
        let stem = drawing
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        let Ok(model) = lay_out_drawing(drawing, corpus) else {
            failed += 1;
            continue;
        };
        sheets += 1;
        let Some(template) = model.template.as_ref() else {
            continue;
        };
        for symbol in &template.symbols {
            if symbol.body == SymbolBodySource::Library {
                drawn_from_library += 1;
                continue;
            }
            // Ways a placement can lose its source: the path names a slot in
            // the share and the file is missing there; the path never enters a
            // share at all (an absolute local drive, which is where a
            // title-block icon authored on the engineer's own machine points)
            // and the library carries no `.sym` of that name; the same, but the
            // library carries that name more than once and deliberately refuses
            // to pick. The last one is not a missing file — the corpus has it.
            let addressed = pid_parse::symbol_library::library_relative_path(&symbol.key).is_some();
            let tag = match symbol.body {
                SymbolBodySource::Cached if symbol.library_available => 3_u8,
                SymbolBodySource::Cached if symbol.name_gap == Some(NameMiss::NotUnique) => 4_u8,
                SymbolBodySource::Cached if addressed => 0_u8,
                SymbolBodySource::Cached if symbol.name_gap == Some(NameMiss::NoSuchFile) => 2_u8,
                SymbolBodySource::Cached => 5_u8,
                _ => 1_u8,
            };
            let name = symbol
                .key
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&symbol.key)
                .to_ascii_uppercase();
            let entry = rows.entry((name, tag)).or_default();
            entry.0 += 1;
            entry.1.insert(stem.clone());
        }
    }
    println!("\ntemplate furniture symbols, over {sheets} sheets laid out");
    if failed > 0 {
        println!("  ({failed} sheets could not be laid out and are missing here)");
    }
    if rows.is_empty() {
        println!("  every placement read its .sym from the library");
    } else {
        println!("{:<20} {:>5} {:>7}  body", "symbol", "count", "sheets");
        for ((name, tag), (count, where_drawn)) in &rows {
            let body = match tag {
                0 => "cache only — the share has no such .sym",
                3 => "cache only — the .sym is in the library too, and the cache still wins",
                4 => "cache only — the library carries that name twice, so it is refused",
                2 => "cache only — no share in the path, and no .sym of that name",
                5 => "cache only — no share in the path, and no readable .sym of that name",
                _ => "undrawn — no .sym and no cache",
            };
            println!("{name:<20} {count:>5} {:>7}  {body}", where_drawn.len());
        }
    }
    println!("  ({drawn_from_library} placements read their .sym from the library)");
}

/// The `--residual` sweep: every stretch of reference ink the page leaves
/// unexplained, on every sheet that has a reference plot, attributed to the
/// page stroke nearest it.
fn residual_sweep() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        println!("no backup-plant-data directory; nothing to sweep");
        return Ok(());
    };
    template_ledger(&backup, &corpus);
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    let mut all: Vec<(String, Residual)> = Vec::new();
    for drawing in &drawings {
        let stem = drawing
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        let reference = corpus.join(format!("{stem}.pdf"));
        if !reference.is_file() {
            continue;
        }
        let plot = read_plot(&std::fs::read(&reference)?)?;
        let segments: Vec<Stroke> = plot
            .segments
            .iter()
            .map(|segment| (segment.start_mm, segment.end_mm))
            .collect();
        let model = lay_out_drawing(drawing, &corpus)?;
        let mut page_segments: Vec<Stroke> = Vec::new();
        for primitive in &model.primitives {
            let Some(points) = paper_points(&primitive.shape) else {
                continue;
            };
            page_segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
        }
        let page = Grid::new(&page_segments);
        let shapes = page_shapes(&model);
        let mut runs: Vec<Residual> = Vec::new();
        for (a, b) in &segments {
            unexplained_runs(*a, *b, &page, &shapes, &model, &mut runs);
        }
        let total: f64 = runs.iter().map(|run| run.mm).sum();
        println!(
            "\n{stem}\n  {} reference strokes, {:.1} mm unexplained in {} runs",
            segments.len(),
            total,
            runs.len()
        );
        runs.sort_by(|a, b| b.mm.total_cmp(&a.mm));
        for run in &runs {
            if run.mm < 0.2 {
                continue;
            }
            println!(
                "  {:5.2} mm ({:7.2},{:7.2})->({:7.2},{:7.2})  near {:<8} gap {:5.2}..{:5.2}  {}{}{}{}",
                run.mm,
                run.a[0],
                run.a[1],
                run.b[0],
                run.b[1],
                run.near_kind,
                run.gap,
                run.gap_far,
                if run.near_dashed { "dash " } else { "" },
                if run.near_furniture { "furniture " } else { "" },
                run.near_symbol
                    .clone()
                    .or_else(|| run.near_stream.clone())
                    .map(|name| format!(" [{name}]"))
                    .unwrap_or_default(),
                run.template_source
                    .as_ref()
                    .map(|(source, key, gap)| match (source, gap) {
                        (SymbolBodySource::Cached, Some(NameMiss::NotUnique)) => {
                            format!("  <- {key} refused: the library carries that name twice")
                        }
                        (SymbolBodySource::Cached, Some(NameMiss::NoSuchFile)) => {
                            format!("  <- no {key} of that name in the library")
                        }
                        (SymbolBodySource::Cached, None) => format!("  <- no {key} in the library"),
                        _ => format!("  <- nothing draws {key}"),
                    })
                    .unwrap_or_default()
            );
        }
        all.extend(runs.into_iter().map(|run| (stem.clone(), run)));
    }

    // Ink inside a border-template placement the package cannot draw from the
    // library is a source the corpus does not carry. Kept apart from the decoders'
    // scoreboard so the next round does not re-read it as a decoding gap.
    let mut sources: BTreeMap<(String, String), (f64, usize)> = BTreeMap::new();
    let mut from_source = 0.0_f64;
    let mut from_source_runs = 0usize;
    for (_, run) in &all {
        let Some((source, key, gap)) = &run.template_source else {
            continue;
        };
        from_source += run.mm;
        from_source_runs += 1;
        let tag = match (source, gap) {
            (SymbolBodySource::Cached, Some(NameMiss::NotUnique)) => "name twice",
            (SymbolBodySource::Cached, _) => "cache only",
            _ => "undrawn",
        };
        let entry = sources
            .entry((tag.to_string(), key.clone()))
            .or_insert((0.0, 0));
        entry.0 += run.mm;
        entry.1 += 1;
    }
    println!(
        "\nmissing sources: {from_source:.1} mm in {from_source_runs} runs sit inside a\n\
         border-template placement whose body the library does not carry"
    );
    println!("{:<12} {:<28} {:>10} {:>6}", "body", "symbol", "mm", "runs");
    for ((tag, key), (mm, runs)) in &sources {
        println!("{tag:<12} {key:<28} {mm:>9.1} {runs:>6}");
    }
    if sources.is_empty() {
        println!("(none)");
    }

    // Group by what the run would have to be to print: the same cause leaves
    // the same signature, so the totals are read per signature.
    let mut groups: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for (_, run) in &all {
        let bucket = if run.gap == f64::INFINITY {
            "nothing within 6 mm".to_string()
        } else if run.gap <= 0.8 {
            format!("near {} within 0.8 mm", run.near_kind)
        } else if run.gap <= 2.0 {
            format!("near {} at 0.8-2 mm", run.near_kind)
        } else {
            format!("near {} beyond 2 mm", run.near_kind)
        };
        let entry = groups.entry(bucket).or_insert((0.0, 0));
        entry.0 += run.mm;
        entry.1 += 1;
    }
    println!("\nby signature (the page stroke nearest the residual):");
    let mut ranked: Vec<(&String, &(f64, usize))> = groups.iter().collect();
    ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
    println!("{:<40} {:>10} {:>6}", "signature", "mm", "runs");
    for (signature, (mm, runs)) in ranked {
        println!("{signature:<40} {mm:>9.1} {runs:>6}");
    }
    Ok(())
}

/// `--window x0 y0 x1 y1 <drawing.pid>`: one paper-millimetre window, with
/// every reference stroke and every page stroke that falls in it.
///
/// The residual sweep names where the page misses ink; this names what is
/// already there, so a residual can be read against its neighbours instead of
/// against a total.
fn window_dump(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let numbers: Vec<f64> = args
        .iter()
        .take(4)
        .filter_map(|value| value.parse().ok())
        .collect();
    let Some(path) = args.get(4) else {
        eprintln!("--window <x0> <y0> <x1> <y1> <drawing.pid>");
        return Ok(());
    };
    let [x0, y0, x1, y1]: [f64; 4] = numbers
        .as_slice()
        .try_into()
        .map_err(|_| "--window needs four numbers")?;
    let inside =
        |point: [f64; 2]| point[0] >= x0 && point[0] <= x1 && point[1] >= y0 && point[1] <= y1;

    let corpus = corpus_dir();
    let model = lay_out_drawing(Path::new(path), &corpus)?;
    println!("window ({x0},{y0})-({x1},{y1}) mm on {path}");
    println!("\nthe page draws:");
    for primitive in &model.primitives {
        let Some(chain) = paper_points(&primitive.shape) else {
            if let PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                ..
            } = &primitive.shape
                && inside(*origin_mm)
            {
                println!(
                    "  text   at ({:7.2},{:7.2}) h {:.2} {:?}",
                    origin_mm[0],
                    origin_mm[1],
                    height_mm.unwrap_or(0.0),
                    value
                );
            }
            continue;
        };
        if !chain.iter().any(|point| inside(*point)) {
            continue;
        }
        let kind = match &primitive.shape {
            PaperShape::Line { .. } => "line",
            PaperShape::Circle { .. } => "circle",
            PaperShape::Arc { .. } => "arc",
            PaperShape::Polyline { closed, .. } => {
                if *closed {
                    "ring"
                } else {
                    "polyline"
                }
            }
            PaperShape::Fill { .. } => "fill",
            PaperShape::Text { .. } => "text",
        };
        let (first, last) = (chain[0], chain[chain.len() - 1]);
        println!(
            "  {:<8} ({:7.2},{:7.2})->({:7.2},{:7.2})  {:<9} {}{}{}",
            kind,
            first[0],
            first[1],
            last[0],
            last[1],
            primitive.confidence,
            if primitive
                .stroke
                .as_ref()
                .is_some_and(|stroke| !stroke.dash_mm.is_empty())
            {
                "dash "
            } else {
                ""
            },
            if primitive.furniture {
                "furniture "
            } else {
                ""
            },
            primitive
                .symbol
                .as_deref()
                .or(primitive.stream_path.as_deref())
                .map(|name| format!("[{}]", path_tail(name, 2)))
                .unwrap_or_default()
        );
    }

    let stem = Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let reference = corpus.join(format!("{stem}.pdf"));
    if !reference.is_file() {
        println!("\nno reference plot beside the corpus");
        return Ok(());
    }
    let plot = read_plot(&std::fs::read(&reference)?)?;
    let mut page_segments: Vec<Stroke> = Vec::new();
    for primitive in &model.primitives {
        let Some(points) = paper_points(&primitive.shape) else {
            continue;
        };
        page_segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
    }
    let page = Grid::new(&page_segments);
    println!("\nthe reference inks:");
    for segment in plot.segments {
        let (a, b) = (segment.start_mm, segment.end_mm);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        if !inside(mid) {
            continue;
        }
        let samples = line_chain(a, b, SAMPLE_MM);
        let gap = samples
            .iter()
            .map(|point| page.distance(*point))
            .fold(f64::MAX, f64::min);
        let far = samples
            .iter()
            .map(|point| page.distance(*point))
            .fold(0.0_f64, f64::max);
        println!(
            "  ({:7.2},{:7.2})->({:7.2},{:7.2})  len {:5.2}  page {:4.2}..{:4.2}",
            a[0],
            a[1],
            b[0],
            b[1],
            (b[0] - a[0]).hypot(b[1] - a[1]),
            gap,
            far
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--package") {
        return package_sweep();
    }
    if args.first().map(String::as_str) == Some("--residual") {
        return residual_sweep();
    }
    if args.first().map(String::as_str) == Some("--window") {
        return window_dump(&args[1..]);
    }
    let mut args = args.into_iter();
    let (Some(reference), Some(drawing)) = (args.next(), args.next()) else {
        eprintln!(
            "usage: probe_body_against_reference <reference.pdf> <drawing.pid> [symbol basename]"
        );
        return Ok(());
    };
    let wanted = args.next();
    let corpus = corpus_dir();

    let plot = read_plot(&std::fs::read(&reference)?)?;
    let segments: Vec<Stroke> = plot
        .segments
        .iter()
        .map(|segment| (segment.start_mm, segment.end_mm))
        .collect();
    let grid = Grid::new(&segments);

    // The whole page, so `page` answers "does anything the drawing prints ink
    // this place" rather than "does this body's own cache".
    let model = lay_out_drawing(Path::new(&drawing), &corpus)?;
    print_template_sources(&model);
    let mut page_segments: Vec<Stroke> = Vec::new();
    for primitive in &model.primitives {
        let Some(points) = paper_points(&primitive.shape) else {
            continue;
        };
        page_segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
    }
    let page_grid = Grid::new(&page_segments);

    let doc = PidParser::new().parse_file(Path::new(&drawing))?;
    let geometry = build_normalized_geometry(&doc);

    // What the drawing's own definition cache holds for a placement: its
    // primitives with the file's display answer for each, and the layer table
    // that answer is read through. The page draws this body, not the library's,
    // so a stroke the library filter drops can still print — and a stroke the
    // cache hides is missing from the page whatever the library says.
    struct Cache {
        layers: Vec<(u32, Option<String>, Option<bool>)>,
        primitives: Vec<(bool, SymbolPrimitive)>,
    }

    impl Cache {
        fn of(body: &pid_parse::PidSymbolDefinition) -> Self {
            Self {
                layers: body
                    .sheet_layers
                    .iter()
                    .map(|layer| (layer.oid, layer.name.clone(), layer.displayed))
                    .collect(),
                primitives: body
                    .primitives
                    .iter()
                    .enumerate()
                    .map(|(index, primitive)| {
                        let displayed = body
                            .primitive_layers
                            .get(index)
                            .is_none_or(|layer| body.layer_is_displayed(*layer));
                        (displayed, primitive.clone())
                    })
                    .collect(),
            }
        }
    }

    // Collect the placements, one per distinct symbol path, with the body the
    // drawing caches for it.
    struct Placement {
        path: String,
        insertion: [f64; 2],
        rotation: f64,
        scale: [f64; 2],
        cache: Option<Cache>,
    }
    let mut placements: BTreeMap<String, Placement> = BTreeMap::new();
    for entity in &geometry.entities {
        let PidGraphicKind::SymbolInstance {
            symbol_path: Some(path),
            insertion,
            rotation,
            scale,
            definition,
        } = &entity.kind
        else {
            continue;
        };
        let cache = definition
            .and_then(|reference| geometry.symbol_definition(reference))
            .map(Cache::of);
        placements.entry(path.clone()).or_insert(Placement {
            path: path.clone(),
            insertion: [insertion.x, insertion.y],
            rotation: *rotation,
            scale: *scale,
            cache,
        });
    }

    let mut library = SymbolLibrary::with_roots(library_roots(&corpus));
    let mut judged = 0usize;
    for placement in placements.values() {
        let base = base_name(&placement.path).to_string();
        if wanted.as_deref().is_some_and(|wanted| wanted != base) {
            continue;
        }
        let Some(body) = library.resolve(&placement.path) else {
            continue;
        };
        let shown: Vec<usize> = body
            .displayed_primitives()
            .iter()
            .map(|styled| {
                body.primitives
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, *styled))
                    .unwrap_or(usize::MAX)
            })
            .collect();
        // Without a name, report only the bodies whose filter keeps under half.
        if wanted.is_none() && shown.len() * 2 >= body.primitives.len() {
            continue;
        }
        judged += 1;
        println!(
            "\n{base}: {} primitives, filter keeps {}",
            body.primitives.len(),
            shown.len()
        );
        println!(
            "  placement at ({:.5},{:.5}) m rotation {:.6} rad scale {:?}",
            placement.insertion[0], placement.insertion[1], placement.rotation, placement.scale
        );
        match &placement.cache {
            Some(cache) => {
                let displayed = cache
                    .primitives
                    .iter()
                    .filter(|(displayed, _)| *displayed)
                    .count();
                println!(
                    "  drawing's cache: {} primitives, {displayed} displayed",
                    cache.primitives.len()
                );
                for (oid, name, shown) in &cache.layers {
                    println!("    layer oid={oid:<6} name={name:?} displayed={shown:?}");
                }
                for (index, (shown, primitive)) in cache.primitives.iter().enumerate() {
                    let scored = local_points(primitive).map(|local| {
                        score(
                            &local,
                            placement.insertion,
                            placement.rotation,
                            placement.scale,
                            &grid,
                        )
                    });
                    println!(
                        "    [{index}] {} {:<8}{}",
                        if *shown { "displayed" } else { "hidden   " },
                        kind(primitive),
                        ink_column(scored.as_ref())
                    );
                }
            }
            None => println!("  drawing's cache: none (library-only placement)"),
        }
        // The cache's own strokes in paper millimetres, so a dropped library
        // stroke can be told apart from one the page already draws.
        let cache_curves: Vec<(bool, Vec<[f64; 2]>)> =
            placement.cache.as_ref().map_or_else(Vec::new, |cache| {
                cache
                    .primitives
                    .iter()
                    .filter_map(|(shown, primitive)| {
                        let local = local_points(primitive)?;
                        let points = local
                            .iter()
                            .map(|point| {
                                place_metres(
                                    *point,
                                    placement.insertion,
                                    placement.rotation,
                                    placement.scale,
                                )
                            })
                            .collect();
                        Some((*shown, points))
                    })
                    .collect()
            });
        println!("  library body:");
        for (index, styled) in body.primitives.iter().enumerate() {
            let picked = shown.contains(&index);
            let scored = local_points(&styled.primitive).map(|local| {
                score(
                    &local,
                    placement.insertion,
                    placement.rotation,
                    placement.scale,
                    &grid,
                )
            });
            // A dropped stroke the cache already carries is not missing ink,
            // and neither is one the page already inks through other strokes.
            let carries = match (&scored, picked) {
                (Some(_), false) => {
                    let local = local_points(&styled.primitive).unwrap_or_default();
                    let translated: Vec<[f64; 2]> = local
                        .iter()
                        .map(|point| {
                            place_metres(
                                *point,
                                placement.insertion,
                                placement.rotation,
                                placement.scale,
                            )
                        })
                        .collect();
                    let page = drawn_cover(&translated, &page_grid);
                    let left = unexplained_ink(&translated, &grid, &page_grid);
                    let same = cache_match(&translated, &cache_curves).map_or_else(
                        String::new,
                        |(at, shown)| {
                            format!(
                                "  = cache[{at}] {}",
                                if shown { "displayed" } else { "hidden" }
                            )
                        },
                    );
                    format!("  page {page:.2}  left {left:5.1} mm{same}")
                }
                _ => String::new(),
            };
            println!(
                "    [{index}] sheet={} layer={:?} {} {:<8}{}{carries}",
                styled.sheet_ref,
                styled.sheet_layer_name,
                if picked { "drawn  " } else { "dropped" },
                kind(&styled.primitive),
                ink_column(scored.as_ref())
            );
        }
    }
    if judged == 0 {
        println!("no placed body matched");
    }
    Ok(())
}
