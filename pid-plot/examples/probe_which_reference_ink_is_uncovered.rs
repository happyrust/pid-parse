//! Which reference ink does the plot still leave uncovered?
//!
//! The compare report keeps `uncovered_reference` to groups of at least 1 mm,
//! which hides the last few tenths of a millimetre. This probe walks the
//! reference ink at a finer step, keeps every sample farther than the cover
//! radius from our ink, joins consecutive samples into runs, and names each
//! run by the paper-table stroke nearest to it. A run that turns out to be a
//! stub of a stroke we already draw is a sampling artefact; one that sits on
//! geometry we never print is a real miss.
//!
//! Run with
//! `cargo run -p pid-plot --example probe_which_reference_ink_is_uncovered`.

use std::collections::HashMap;
use std::path::PathBuf;

use pid_plot::compare::{Fit, MaskRect, compare_plots_with_model, template_masks};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

/// The radius `compare.rs` counts as covered.
const COVER_MM: f64 = 0.5;
/// Finer than the report's 0.5 mm sample, so a 0.2 mm stub cannot hide.
const STEP_MM: f64 = 0.1;
/// A paper-table stroke this close to a run names it.
const ATTRIB_MM: f64 = 15.0;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

#[derive(Clone, Copy)]
struct Seg {
    a: [f64; 2],
    b: [f64; 2],
}

impl Seg {
    fn length(self) -> f64 {
        dist(self.a, self.b)
    }

    fn at(self, t: f64) -> [f64; 2] {
        [
            self.a[0] + (self.b[0] - self.a[0]) * t,
            self.a[1] + (self.b[1] - self.a[1]) * t,
        ]
    }
}

fn dist(start: [f64; 2], end: [f64; 2]) -> f64 {
    ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt()
}

fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length2 = dx * dx + dy * dy;
    if length2 <= 1e-12 {
        return dist(point, start);
    }
    let t = (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length2).clamp(0.0, 1.0);
    dist(point, [start[0] + dx * t, start[1] + dy * t])
}

struct Grid {
    cell: f64,
    lines: Vec<Seg>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl Grid {
    fn new(lines: Vec<Seg>) -> Self {
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

    fn within(&self, point: [f64; 2], radius: f64) -> Vec<usize> {
        let key = cell_key(point, self.cell);
        let reach = (radius / self.cell).ceil() as i32 + 1;
        let mut hits = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) else {
                    continue;
                };
                for index in bucket {
                    let line = self.lines[*index];
                    if dist_point_seg(point, line.a, line.b) <= radius {
                        hits.push(*index);
                    }
                }
            }
        }
        hits.sort_unstable();
        hits.dedup();
        hits
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

fn cells(line: Seg, cell: f64) -> Vec<(i32, i32)> {
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

/// Our paper millimetres, back into the reference's page.
fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
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

/// The two indexes a run is measured against.
struct Lookup<'a> {
    ours: &'a Grid,
    named: &'a Grid,
    labels: &'a [(String, Seg)],
}

struct Run {
    length_mm: f64,
    first_mm: [f64; 2],
    last_mm: [f64; 2],
    stroke_mm: f64,
    /// How far our nearest ink sits off the reference stroke's own line.
    lateral_mm: f64,
    /// How far our nearest ink sits along that line from the run's middle.
    along_mm: f64,
    our_distance_mm: f64,
    our_point_mm: Option<[f64; 2]>,
    label: String,
}

impl Lookup<'_> {
    fn name_at(&self, point: [f64; 2]) -> String {
        self.named.nearest(point, ATTRIB_MM).map_or_else(
            || "—".to_string(),
            |(_, index)| self.labels[index].0.clone(),
        )
    }

    fn our_ink(&self, point: [f64; 2]) -> Option<(f64, [f64; 2])> {
        self.ours
            .nearest(point, 60.0)
            .map(|(distance, index)| (distance, self.ours.lines[index].at(0.5)))
    }
}

fn main() {
    let corpus = corpus_dir();
    for name in DRAWINGS {
        let reference_path = corpus.join(format!("{name}.pdf"));
        let ours_path = corpus.join(format!("plot/{name}.pdf"));
        let (Ok(reference_bytes), Ok(our_bytes)) =
            (std::fs::read(&reference_path), std::fs::read(&ours_path))
        else {
            eprintln!("skipping {name}: a PDF is absent");
            continue;
        };
        let reference = read_plot(&reference_bytes).expect("read reference");
        let ours = read_plot(&our_bytes).expect("read ours");
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");

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
        let mut masks = template.clone();
        masks.extend(missing.iter().cloned());

        let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
        let fit = report.fit;

        let our_lines: Vec<Seg> = ours
            .segments
            .iter()
            .filter(|segment| dist(segment.start_mm, segment.end_mm) >= 1e-4)
            .map(|segment| Seg {
                a: into_reference(segment.start_mm, &fit),
                b: into_reference(segment.end_mm, &fit),
            })
            .collect();
        let our_grid = Grid::new(our_lines);
        let reference_grid = Grid::new(
            reference
                .segments
                .iter()
                .filter(|segment| dist(segment.start_mm, segment.end_mm) >= 1e-6)
                .map(|segment| Seg {
                    a: segment.start_mm,
                    b: segment.end_mm,
                })
                .collect(),
        );

        let mut named: Vec<(String, Seg)> = Vec::new();
        for (index, primitive) in model.primitives.iter().enumerate() {
            let label = name_of(&model, index);
            for points in strokes_of(&primitive.shape) {
                for pair in points.windows(2) {
                    named.push((
                        label.clone(),
                        Seg {
                            a: pair[0],
                            b: pair[1],
                        },
                    ));
                }
            }
        }
        let named_grid = Grid::new(named.iter().map(|(_, line)| *line).collect());
        let lookup = Lookup {
            ours: &our_grid,
            named: &named_grid,
            labels: &named,
        };

        let mut runs: Vec<Run> = Vec::new();
        let mut open = 0.0;
        let mut reference_length = 0.0;
        for segment in &reference.segments {
            let length = dist(segment.start_mm, segment.end_mm);
            if length < 1e-6 {
                continue;
            }
            let line = Seg {
                a: segment.start_mm,
                b: segment.end_mm,
            };
            let pieces = ((length / STEP_MM).round() as usize).max(1);
            let weight = length / pieces as f64;
            let mut first: Option<[f64; 2]> = None;
            let mut last: Option<[f64; 2]> = None;
            for piece in 0..pieces {
                let t = (piece as f64 + 0.5) / pieces as f64;
                let point = line.at(t);
                if masks.iter().any(|mask| mask.contains(point)) {
                    continue;
                }
                reference_length += weight;
                if our_grid.nearest(point, COVER_MM).is_some() {
                    flush(&mut runs, &mut first, &mut last, line, weight, &lookup);
                    continue;
                }
                open += weight;
                first.get_or_insert(point);
                last = Some(point);
            }
            flush(&mut runs, &mut first, &mut last, line, weight, &lookup);
        }

        let coverage = if reference_length > 0.0 {
            1.0 - open / reference_length
        } else {
            0.0
        };
        println!(
            "== {name}\n   reference {reference_length:.3} mm  uncovered {open:.3} mm  coverage {coverage:.7}  report coverage {:.7}  runs {}",
            report.coverage,
            runs.len()
        );
        let near: f64 = runs
            .iter()
            .filter(|run| run.our_distance_mm < 1.0)
            .map(|run| run.length_mm)
            .sum();
        let far: f64 = runs
            .iter()
            .filter(|run| run.our_distance_mm >= 1.0)
            .map(|run| run.length_mm)
            .sum();
        let edges: Vec<&Run> = runs
            .iter()
            .filter(|run| run.our_distance_mm < 1.0)
            .collect();
        let longest = edges.iter().map(|run| run.length_mm).fold(0.0, f64::max);
        let widest = edges
            .iter()
            .map(|run| run.lateral_mm.abs())
            .fold(0.0, f64::max);
        let on_dashes = edges.iter().filter(|run| run.stroke_mm < 3.0).count();
        println!(
            "   edge-of-a-stroke (<1 mm): {near:.3} mm in {} runs; 真缺 (>1 mm): {far:.3} mm in {} runs",
            edges.len(),
            runs.iter().filter(|run| run.our_distance_mm >= 1.0).count()
        );
        println!(
            "   edge runs: longest {longest:.3} mm, widest {widest:.3} mm off the line, {on_dashes} of {} on a reference stroke shorter than 3 mm",
            edges.len()
        );
        runs.sort_by(|left, right| right.length_mm.total_cmp(&left.length_mm));
        for run in runs
            .iter()
            .filter(|run| run.our_distance_mm >= 1.0)
            .take(12)
        {
            println!(
                "   {:>6.3} mm  ({:8.3},{:8.3}) → ({:8.3},{:8.3})  our ink {:.3} mm  [{}]  on a {:.3} mm stroke",
                run.length_mm,
                run.first_mm[0],
                run.first_mm[1],
                run.last_mm[0],
                run.last_mm[1],
                run.our_distance_mm,
                run.label,
                run.stroke_mm
            );
            if let Some(point) = run.our_point_mm {
                println!(
                    "          nearest our ink at ({:.3},{:.3})",
                    point[0], point[1]
                );
            }
        }
        if far == 0.0 {
            let mut worst: Vec<&Run> = runs.iter().collect();
            worst.sort_by(|left, right| {
                right
                    .our_distance_mm
                    .total_cmp(&left.our_distance_mm)
                    .then(right.length_mm.total_cmp(&left.length_mm))
            });
            println!("   the four runs whose nearest ink is farthest:");
            for run in worst.iter().take(4) {
                println!(
                    "     {:>6.3} mm  ({:8.3},{:8.3})  our ink {:.3} mm  along {:+.3}  lateral {:+.3}  [{}]",
                    run.length_mm,
                    run.first_mm[0],
                    run.first_mm[1],
                    run.our_distance_mm,
                    run.along_mm,
                    run.lateral_mm,
                    run.label
                );
            }
        }
        if let Some(top) = runs.first() {
            let mid = [
                (top.first_mm[0] + top.last_mm[0]) * 0.5,
                (top.first_mm[1] + top.last_mm[1]) * 0.5,
            ];
            println!(
                "   the longest run in context, strokes within 6 mm of ({:.3},{:.3}):",
                mid[0], mid[1]
            );
            for (side, grid) in [("reference", &reference_grid), ("ours    ", &our_grid)] {
                let mut hits = grid.within(mid, 6.0);
                hits.sort_by(|left, right| {
                    grid.lines[*left]
                        .a
                        .partial_cmp(&grid.lines[*right].a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for index in hits {
                    let line = grid.lines[index];
                    println!(
                        "     {side} ({:8.3},{:8.3}) → ({:8.3},{:8.3})  len {:.3}",
                        line.a[0],
                        line.a[1],
                        line.b[0],
                        line.b[1],
                        line.length()
                    );
                }
            }
        }
        println!();
    }
}

fn flush(
    runs: &mut Vec<Run>,
    first: &mut Option<[f64; 2]>,
    last: &mut Option<[f64; 2]>,
    stroke: Seg,
    weight: f64,
    lookup: &Lookup<'_>,
) {
    let (Some(start), Some(end)) = (*first, *last) else {
        *first = None;
        *last = None;
        return;
    };
    let mid = [(start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5];
    let (our_distance_mm, our_point_mm) = lookup
        .our_ink(mid)
        .map_or((f64::INFINITY, None), |(distance, point)| {
            (distance, Some(point))
        });
    let (lateral_mm, along_mm) = our_point_mm.map_or((0.0, 0.0), |point| {
        let length = stroke.length();
        let unit = if length > 1e-9 {
            [
                (stroke.b[0] - stroke.a[0]) / length,
                (stroke.b[1] - stroke.a[1]) / length,
            ]
        } else {
            [1.0, 0.0]
        };
        let dx = point[0] - mid[0];
        let dy = point[1] - mid[1];
        (-dx * unit[1] + dy * unit[0], dx * unit[0] + dy * unit[1])
    });
    runs.push(Run {
        length_mm: dist(start, end) + weight,
        first_mm: start,
        last_mm: end,
        stroke_mm: stroke.length(),
        lateral_mm,
        along_mm,
        our_distance_mm,
        our_point_mm,
        label: lookup.name_at(mid),
    });
    *first = None;
    *last = None;
}
