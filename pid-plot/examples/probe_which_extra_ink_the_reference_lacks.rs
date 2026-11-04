//! Which of our strokes does the reference have no ink for?
//!
//! `CompareReport::extra_length_mm` totals our ink the reference lacks and
//! `extra_outside_by_symbol` groups it to 1 mm, but neither names the record
//! behind a run. This probe repeats the compare's own rule — our ink sampled as
//! drawn at 0.5 mm against the reference's *merged* centre lines (a dash gap is
//! bridged, a symbol's blank is not), masks and missing-symbol boxes excluded —
//! then joins the uncovered samples into runs and prints, for each, the paper
//! primitive nearest to it with its symbol.
//!
//! Repeating the rule is the point: sampling against the reference's strokes as
//! laid instead reports 7401 mm on the I drawing where the gate reads 0.305,
//! because every dash landing in a reference dash gap looks like ink with no
//! reference ink to cover it.
//!
//! The cover radius is an argument (`COVER_MM=0.55`), which is how the I
//! drawing's 0.305 mm answers itself: at 0.55 the same page reads 0 segments
//! and 0.000 mm, so the whole figure is the radius's own 0.05 mm shell rather
//! than ink the reference has no room for. The gate keeps reading 0.305 either
//! way — `compare.rs` holds its radius at 0.5, and this probe measures what
//! that radius charges.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_extra_ink_the_reference_lacks -- ["<drawing>"]
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use pid_plot::compare::{Fit, MaskRect, compare_plots_with_model, template_masks};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{read_plot, sheet_edge_mm};
use pid_plot::write::{PlotFont, write_plot};

const DEFAULT_DRAWING: &str = "I井场 注采阀组工艺及自控流程图";
/// The radius `compare.rs` counts as covered.
const COVER_MM: f64 = 0.5;
/// The compare's own sampling step; the weights add up to its figure only if it matches.
const SAMPLE_MM: f64 = 0.5;
/// How far the merge bridges a collinear gap (`JOIN_GAP_MM`).
const JOIN_GAP_MM: f64 = 3.0;
/// A paper-table stroke this close to a run names it.
const ATTRIB_MM: f64 = 15.0;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn hypot(start: [f64; 2], end: [f64; 2]) -> f64 {
    ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt()
}

#[derive(Clone, Copy)]
struct Line {
    a: [f64; 2],
    b: [f64; 2],
}

impl Line {
    fn length(self) -> f64 {
        hypot(self.a, self.b)
    }

    fn direction(self) -> [f64; 2] {
        let length = self.length();
        if length <= 1e-12 {
            return [1.0, 0.0];
        }
        [
            (self.b[0] - self.a[0]) / length,
            (self.b[1] - self.a[1]) / length,
        ]
    }

    fn at(self, t: f64) -> [f64; 2] {
        [
            self.a[0] + (self.b[0] - self.a[0]) * t,
            self.a[1] + (self.b[1] - self.a[1]) * t,
        ]
    }
}

fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length2 = dx * dx + dy * dy;
    if length2 <= 1e-12 {
        return hypot(point, start);
    }
    let t = (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length2).clamp(0.0, 1.0);
    hypot(point, [start[0] + dx * t, start[1] + dy * t])
}

struct Grid {
    cell: f64,
    lines: Vec<Line>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl Grid {
    fn new(lines: Vec<Line>) -> Self {
        let mut grid = Self {
            cell: 5.0,
            lines: Vec::with_capacity(lines.len()),
            buckets: HashMap::new(),
        };
        for line in lines {
            let index = grid.lines.len();
            grid.lines.push(line);
            for key in cells(line, grid.cell) {
                grid.buckets.entry(key).or_default().push(index);
            }
        }
        grid
    }

    fn nearest(&self, point: [f64; 2], radius: f64) -> Option<(f64, usize)> {
        let key = cell_key(point, self.cell);
        let reach = (radius / self.cell).ceil() as i32 + 1;
        let mut best: Option<(f64, usize)> = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) else {
                    continue;
                };
                for index in bucket {
                    let line = self.lines[*index];
                    let distance = dist_point_seg(point, line.a, line.b);
                    if distance <= radius && best.is_none_or(|(best, _)| distance < best) {
                        best = Some((distance, *index));
                    }
                }
            }
        }
        best
    }
}

fn cells(line: Line, cell: f64) -> Vec<(i32, i32)> {
    let mut keys = Vec::new();
    let steps = ((line.length() / cell).ceil() as usize).max(1);
    for step in 0..=steps {
        let point = line.at(step as f64 / steps as f64);
        let key = cell_key(point, cell);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

fn cell_key(point: [f64; 2], cell: f64) -> (i32, i32) {
    (
        (point[0] / cell).floor() as i32,
        (point[1] / cell).floor() as i32,
    )
}

fn find(parent: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parent[root] != root {
        root = parent[root];
    }
    let mut walk = index;
    while parent[walk] != walk {
        let next = parent[walk];
        parent[walk] = root;
        walk = next;
    }
    root
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}

/// `compare.rs`'s own join test, repeated: collinear to `dot >= 0.999`, within
/// `lateral_mm` of the line, a gap under 3 mm, and two ends that meet.
fn can_join(left: Line, right: Line, lateral_mm: f64) -> bool {
    let direction = left.direction();
    let other = right.direction();
    let dot = (direction[0] * other[0] + direction[1] * other[1]).abs();
    if dot < 0.999 {
        return false;
    }
    let perp = |point: [f64; 2]| {
        let vx = point[0] - left.a[0];
        let vy = point[1] - left.a[1];
        (vx * -direction[1] + vy * direction[0]).abs()
    };
    if perp(right.a) > lateral_mm || perp(right.b) > lateral_mm {
        return false;
    }
    let project = |point: [f64; 2]| {
        (point[0] - left.a[0]) * direction[0] + (point[1] - left.a[1]) * direction[1]
    };
    let left_end = left.length();
    let right_min = project(right.a).min(project(right.b));
    let right_max = project(right.a).max(project(right.b));
    let gap = if right_max < 0.0 {
        -right_max
    } else if left_end < right_min {
        right_min - left_end
    } else {
        0.0
    };
    gap <= JOIN_GAP_MM && ends_meet(left, right)
}

fn ends_meet(left: Line, right: Line) -> bool {
    [left.a, left.b].into_iter().any(|end| {
        [right.a, right.b]
            .into_iter()
            .any(|other| hypot(end, other) <= JOIN_GAP_MM)
    })
}

/// One line per group, from the group's two extreme endpoints.
fn merge_group(lines: &[Line], members: &[usize]) -> Option<Line> {
    let first = lines[*members.first()?];
    let direction = first.direction();
    let mut min_t = 0.0;
    let mut max_t = 0.0;
    let mut min_point = first.a;
    let mut max_point = first.a;
    for member in members {
        for point in [lines[*member].a, lines[*member].b] {
            let t = (point[0] - first.a[0]) * direction[0] + (point[1] - first.a[1]) * direction[1];
            if t < min_t {
                min_t = t;
                min_point = point;
            }
            if t > max_t {
                max_t = t;
                max_point = point;
            }
        }
    }
    (hypot(min_point, max_point) >= 1e-4).then_some(Line {
        a: min_point,
        b: max_point,
    })
}

/// `compare.rs`'s `join_centerlines`: a dashed route is one line to sample, and
/// a symbol's blank is not bridged (its two sides are not collinear across it).
fn join_centerlines(segments: &[Line], lateral_mm: f64) -> Vec<Line> {
    let lines: Vec<Line> = segments
        .iter()
        .copied()
        .filter(|line| line.length() >= 1e-4)
        .collect();
    if lines.is_empty() {
        return lines;
    }
    let mut parent: Vec<usize> = (0..lines.len()).collect();
    let mut endpoints: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        for end in [line.a, line.b] {
            endpoints
                .entry(cell_key(end, JOIN_GAP_MM))
                .or_default()
                .push(index);
        }
    }
    for (index, line) in lines.iter().enumerate() {
        for end in [line.a, line.b] {
            let key = cell_key(end, JOIN_GAP_MM);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(bucket) = endpoints.get(&(key.0 + dx, key.1 + dy)) else {
                        continue;
                    };
                    for other in bucket {
                        if *other <= index {
                            continue;
                        }
                        if can_join(*line, lines[*other], lateral_mm) {
                            union(&mut parent, index, *other);
                        }
                    }
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..lines.len() {
        let root = find(&mut parent, index);
        groups.entry(root).or_default().push(index);
    }
    groups
        .into_values()
        .filter_map(|members| merge_group(&lines, &members))
        .collect()
}

/// How far sideways a pair that `can_join` accepts at 0.4 mm actually sits.
fn print_join_laterals(lines: &[Line]) {
    let mut endpoints: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        for end in [line.a, line.b] {
            endpoints
                .entry(cell_key(end, JOIN_GAP_MM))
                .or_default()
                .push(index);
        }
    }
    let mut seen: HashMap<(usize, usize), f64> = HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        for end in [line.a, line.b] {
            let key = cell_key(end, JOIN_GAP_MM);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(bucket) = endpoints.get(&(key.0 + dx, key.1 + dy)) else {
                        continue;
                    };
                    for other in bucket {
                        if *other <= index {
                            continue;
                        }
                        let pair = (index, *other);
                        if seen.contains_key(&pair) {
                            continue;
                        }
                        let Some(perp) = join_perp(*line, lines[*other]) else {
                            continue;
                        };
                        if perp <= 0.4 {
                            seen.insert(pair, perp);
                        }
                    }
                }
            }
        }
    }
    let mut counts = [0usize; 8];
    let mut widest = 0.0_f64;
    for perp in seen.values() {
        widest = widest.max(*perp);
        let bucket = ((perp / 0.05).floor() as usize).min(counts.len() - 1);
        counts[bucket] += 1;
    }
    println!(
        "  joins at 0.4 mm: {} pairs, widest {widest:.3} mm",
        seen.len()
    );
    for (bucket, count) in counts.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        let lo = bucket as f64 * 0.05;
        println!("    {lo:.2}–{:.2} mm  {count}", lo + 0.05);
    }
}

/// The sideways offset of a pair the join would consider, when the pair is
/// collinear, gapped by at most 3 mm, and has ends that meet. `None` when
/// the pair fails one of those, so the offset is not a join at any tolerance.
fn join_perp(left: Line, right: Line) -> Option<f64> {
    let direction = left.direction();
    let other = right.direction();
    let dot = (direction[0] * other[0] + direction[1] * other[1]).abs();
    if dot < 0.999 {
        return None;
    }
    let perp = |point: [f64; 2]| {
        let vx = point[0] - left.a[0];
        let vy = point[1] - left.a[1];
        (vx * -direction[1] + vy * direction[0]).abs()
    };
    let offset = perp(right.a).max(perp(right.b));
    let project = |point: [f64; 2]| {
        (point[0] - left.a[0]) * direction[0] + (point[1] - left.a[1]) * direction[1]
    };
    let left_end = left.length();
    let right_min = project(right.a).min(project(right.b));
    let right_max = project(right.a).max(project(right.b));
    let gap = if right_max < 0.0 {
        -right_max
    } else if left_end < right_min {
        right_min - left_end
    } else {
        0.0
    };
    (gap <= JOIN_GAP_MM && ends_meet(left, right)).then_some(offset)
}

/// Our paper millimetres, back into the reference's page.
fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
}

fn in_masks(point: [f64; 2], masks: &[MaskRect]) -> bool {
    masks.iter().any(|rect| {
        point[0] >= rect.min_mm[0]
            && point[0] <= rect.max_mm[0]
            && point[1] >= rect.min_mm[1]
            && point[1] <= rect.max_mm[1]
    })
}

/// The paper table's name for a primitive: the symbol file, or its stream.
fn name_of(model: &PaperModel, index: usize) -> String {
    let primitive = &model.primitives[index];
    if let Some(symbol) = &primitive.symbol {
        return symbol
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(symbol)
            .to_string();
    }
    primitive
        .stream_path
        .clone()
        .unwrap_or_else(|| "图纸线".to_string())
}

/// Every polyline a primitive inks. Lettering inks nothing here.
fn strokes_of(shape: &PaperShape) -> Vec<Vec<[f64; 2]>> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![vec![*start_mm, *end_mm]],
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && points.len() > 2 {
                points.push(points[0]);
            }
            vec![points]
        }
        PaperShape::Fill { chain_mm, .. } => {
            let mut points = chain_mm.clone();
            if points.len() > 2 {
                points.push(points[0]);
            }
            vec![points]
        }
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let span = if *clockwise {
                -((start_angle - end_angle).rem_euclid(std::f64::consts::TAU))
            } else {
                (end_angle - start_angle).rem_euclid(std::f64::consts::TAU)
            };
            let steps = 48;
            vec![
                (0..=steps)
                    .map(|step| {
                        let angle = start_angle + span * step as f64 / steps as f64;
                        [
                            center_mm[0] + radius_mm * angle.cos(),
                            center_mm[1] + radius_mm * angle.sin(),
                        ]
                    })
                    .collect(),
            ]
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => {
            let steps = 48;
            vec![
                (0..=steps)
                    .map(|step| {
                        let angle = std::f64::consts::TAU * step as f64 / steps as f64;
                        [
                            center_mm[0] + radius_mm * angle.cos(),
                            center_mm[1] + radius_mm * angle.sin(),
                        ]
                    })
                    .collect(),
            ]
        }
        PaperShape::Text { .. } => Vec::new(),
    }
}

struct Run {
    length_mm: f64,
    first_mm: [f64; 2],
    last_mm: [f64; 2],
    nearest_mm: f64,
    nearest_ink: Option<Line>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_DRAWING.to_string());
    let reference = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf")))?)?;
    let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)?;

    let dirs = font_dirs(Some(&corpus));
    let prepared = prepare_fonts(&[&model], &dirs)?;
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let ours = read_plot(&write_plot(&model, &fonts)?)?;

    let template = template_masks(reference.page_mm);
    let missing: Vec<MaskRect> = model
        .missing_boxes
        .iter()
        .map(|box_| MaskRect {
            label: box_.path.clone(),
            min_mm: box_.min_mm,
            max_mm: box_.max_mm,
        })
        .collect();
    let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
    let fit = report.fit;
    // The cover radius is a threshold, so the probe takes it as an argument:
    // running the same page at 0.50 / 0.55 / 0.60 says whether a run is
    // geometry or the radius's own edge.
    let cover_mm: f64 = std::env::var("COVER_MM")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(COVER_MM);

    // The reference side of the rule: merged centre lines. `LATERAL_MM`
    // narrows the join's sideways tolerance (the extra sampler uses 0.20; the
    // fit still uses 0.4) so a reading can say whether a parallel stroke was
    // swallowed into the line beside it.
    let lateral_mm: f64 = std::env::var("LATERAL_MM")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.2);
    let reference_lines: Vec<Line> = reference
        .segments
        .iter()
        .map(|segment| Line {
            a: segment.start_mm,
            b: segment.end_mm,
        })
        .collect();
    print_join_laterals(&reference_lines);
    let reference_index = Grid::new(join_centerlines(&reference_lines, lateral_mm));
    // Our side: as drawn, mapped into the reference's frame.
    let our_lines: Vec<Line> = ours
        .segments
        .iter()
        .filter(|segment| hypot(segment.start_mm, segment.end_mm) >= 1e-4)
        .map(|segment| Line {
            a: into_reference(segment.start_mm, &fit),
            b: into_reference(segment.end_mm, &fit),
        })
        .collect();

    // Attribution: the paper table's strokes, in the same frame.
    let mut named: Vec<(String, Line)> = Vec::new();
    for (index, primitive) in model.primitives.iter().enumerate() {
        let label = name_of(&model, index);
        for points in strokes_of(&primitive.shape) {
            for pair in points.windows(2) {
                let line = Line {
                    a: into_reference(pair[0], &fit),
                    b: into_reference(pair[1], &fit),
                };
                if line.length() >= 1e-4 {
                    named.push((label.clone(), line));
                }
            }
        }
    }
    let named_index = Grid::new(named.iter().map(|(_, line)| *line).collect());

    let mut runs: Vec<(String, Run)> = Vec::new();
    let mut inside_missing = 0.0;
    for line in &our_lines {
        let length = line.length();
        let pieces = ((length / SAMPLE_MM).round() as usize).max(1);
        let weight = length / pieces as f64;
        let mut open: Option<Run> = None;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = line.at(t);
            if in_masks(point, &template) {
                open = None;
                continue;
            }
            let near = reference_index
                .nearest(point, cover_mm)
                .is_some_and(|(distance, _)| distance <= cover_mm);
            if near {
                if let Some(run) = open.take() {
                    runs.push((attribute(&named_index, &named, run.last_mm), run));
                }
                continue;
            }
            if in_masks(point, &missing) {
                inside_missing += weight;
                if let Some(run) = open.take() {
                    runs.push(("inside a missing-symbol box".to_string(), run));
                }
                continue;
            }
            let nearest = reference_index.nearest(point, 60.0);
            let run = open.get_or_insert(Run {
                length_mm: 0.0,
                first_mm: point,
                last_mm: point,
                nearest_mm: f64::INFINITY,
                nearest_ink: None,
            });
            run.length_mm += weight;
            run.last_mm = point;
            if let Some((distance, ink_index)) = nearest
                && distance < run.nearest_mm
            {
                run.nearest_mm = distance;
                run.nearest_ink = Some(reference_index.lines[ink_index]);
            }
        }
        if let Some(run) = open.take() {
            runs.push((attribute(&named_index, &named, run.last_mm), run));
        }
    }

    let mut by_symbol: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut total = 0.0;
    for (label, run) in &runs {
        let entry = by_symbol.entry(label.clone()).or_default();
        entry.0 += run.length_mm;
        entry.1 += 1;
        total += run.length_mm;
    }
    println!(
        "{name}: report extra {:.3} mm (outside a missing box {:.3} / inside one {:.3}), fit scale {:.6}, cover {cover_mm:.2} mm, lateral {lateral_mm:.2} mm",
        report.extra_length_mm,
        report.extra_outside_missing_mm,
        report.extra_inside_missing_mm,
        fit.scale
    );
    println!(
        "  this probe: {} runs, {total:.3} mm outside the masks and {inside_missing:.3} mm inside a missing box\n",
        runs.len()
    );
    for (label, (length, count)) in &by_symbol {
        println!("  {label:<46} {count:>3} runs  {length:8.3} mm");
    }
    println!("\n  the longest runs:");
    let mut sorted = runs;
    sorted.sort_by(|left, right| right.1.length_mm.total_cmp(&left.1.length_mm));
    for (label, run) in sorted.iter().take(12) {
        let ink = run.nearest_ink.map_or_else(
            || "no ink within 60 mm".to_string(),
            |line| {
                format!(
                    "nearest ink {:.3} mm: a {:.3} mm stroke ({:.2},{:.2})->({:.2},{:.2})",
                    run.nearest_mm,
                    line.length(),
                    line.a[0],
                    line.a[1],
                    line.b[0],
                    line.b[1]
                )
            },
        );
        println!(
            "    {:.3} mm  ({:.2},{:.2})->({:.2},{:.2})  {label}\n        {ink}",
            run.length_mm, run.first_mm[0], run.first_mm[1], run.last_mm[0], run.last_mm[1]
        );
    }
    println!("\n  named runs (/Sheet6, Flange, Remarks):");
    for (label, run) in sorted.iter().filter(|(label, _)| {
        label.contains("Sheet6") || label.contains("Flange") || label.contains("Remarks")
    }) {
        let edge = edge_distance(run.first_mm, run.last_mm, reference.page_mm);
        let frame = sheet_edge_mm(run.first_mm, run.last_mm, reference.page_mm);
        let band = mask_gap(run.first_mm, run.last_mm, &template);
        println!(
            "    {:.3} mm  ({:.2},{:.2})->({:.2},{:.2})  edge {edge:.2}  frame {frame}  {band}  {label}\n        {}",
            run.length_mm,
            run.first_mm[0],
            run.first_mm[1],
            run.last_mm[0],
            run.last_mm[1],
            nearest_text(run)
        );
    }
    Ok(())
}

fn nearest_text(run: &Run) -> String {
    run.nearest_ink.map_or_else(
        || "no ink within 60 mm".to_string(),
        |line| {
            format!(
                "nearest {:.3} mm ({:.2},{:.2})->({:.2},{:.2})",
                run.nearest_mm, line.a[0], line.a[1], line.b[0], line.b[1]
            )
        },
    )
}

fn edge_distance(start: [f64; 2], end: [f64; 2], page_mm: [f64; 2]) -> f64 {
    [start, end]
        .into_iter()
        .map(|point| {
            point[0]
                .min(point[1])
                .min(page_mm[0] - point[0])
                .min(page_mm[1] - point[1])
        })
        .fold(f64::MAX, f64::min)
}

fn mask_gap(start: [f64; 2], end: [f64; 2], masks: &[MaskRect]) -> String {
    let mut best: Option<(f64, &str)> = None;
    for point in [start, end] {
        for mask in masks {
            let gap = (mask.min_mm[0] - point[0])
                .max(point[0] - mask.max_mm[0])
                .max(mask.min_mm[1] - point[1])
                .max(point[1] - mask.max_mm[1])
                .max(0.0);
            if best.is_none_or(|(so_far, _)| gap < so_far) {
                best = Some((gap, mask.label.as_str()));
            }
        }
    }
    best.map_or_else(
        || "no mask".to_string(),
        |(gap, label)| format!("outside {label} by {gap:.2} mm"),
    )
}

fn attribute(named_index: &Grid, named: &[(String, Line)], point: [f64; 2]) -> String {
    named_index
        .nearest(point, ATTRIB_MM)
        .map_or_else(|| "—".to_string(), |(_, index)| named[index].0.clone())
}
