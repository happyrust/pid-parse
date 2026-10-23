//! Ask the reference plot which arc between a symbol body's two angles it
//! inks: the minor sweep the writer draws, or the major one the other reading
//! of `start`/`end` states.
//!
//! Every paper arc is stored as two angles plus "sweep clockwise, the short
//! way", and the writer flattens it that way. For an arc past half a turn the
//! short way is its complement, so a body record that means the long way
//! prints mirrored. The reference plot settles which one the file meant: score
//! both chains against its ink.
//!
//! ```text
//! cargo run -p pid-plot --example probe_arc_sweep_side
//! ```
//!
//! Read-only: layout and the reference PDF, no plot written.

use std::collections::HashMap;
use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;

const TAU: f64 = std::f64::consts::TAU;
const COVER_MM: f64 = 0.5;
const CELL_MM: f64 = 5.0;

type Stroke = ([f64; 2], [f64; 2]);

struct Grid {
    buckets: HashMap<(i32, i32), Vec<Stroke>>,
}

impl Grid {
    fn new(segments: &[Stroke]) -> Self {
        let mut grid = Self {
            buckets: HashMap::new(),
        };
        for segment in segments {
            let (a, b) = *segment;
            let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
            let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
            for cx in ((x0 / CELL_MM).floor() as i32 - 1)..=((x1 / CELL_MM).floor() as i32 + 1) {
                for cy in ((y0 / CELL_MM).floor() as i32 - 1)..=((y1 / CELL_MM).floor() as i32 + 1)
                {
                    grid.buckets.entry((cx, cy)).or_default().push(*segment);
                }
            }
        }
        grid
    }

    fn distance(&self, point: [f64; 2]) -> f64 {
        let key = (
            (point[0] / CELL_MM).floor() as i32,
            (point[1] / CELL_MM).floor() as i32,
        );
        let mut best = f64::MAX;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) {
                    for (a, b) in bucket {
                        best = best.min(distance_to_segment(point, *a, *b));
                    }
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

/// Points along an arc swept `sweep` radians from `start`, sampled every
/// `0.2 mm` of chord.
fn chain(center: [f64; 2], radius: f64, start: f64, sweep: f64) -> Vec<[f64; 2]> {
    let steps = ((sweep.abs() * radius / 0.2).ceil() as usize).clamp(4, 400);
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

/// How much of `points` sits within [`COVER_MM`] of reference ink.
fn covered(points: &[[f64; 2]], grid: &Grid) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let hits = points
        .iter()
        .filter(|point| grid.distance(**point) <= COVER_MM)
        .count();
    hits as f64 / points.len() as f64
}

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    );
    // `probe_arc_sweep_side <x_mm> <y_mm>` also prints every arc within 12 mm
    // of that point in the first drawing, so a stroke on the page can be read
    // back to the shape that drew it.
    let mut args = std::env::args().skip(1);
    let verbose_at: [f64; 2] = match (args.next(), args.next()) {
        (Some(x), Some(y)) => [x.parse()?, y.parse()?],
        _ => [f64::NAN, f64::NAN],
    };
    let verbose = verbose_at[0].is_finite();
    for (index, name) in DRAWINGS.iter().enumerate() {
        let verbose = verbose && index == 0;
        let name = *name;
        let reference = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf")))?)?;
        let strokes: Vec<Stroke> = reference
            .segments
            .iter()
            .map(|segment| (segment.start_mm, segment.end_mm))
            .collect();
        let grid = Grid::new(&strokes);
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)?;
        let mut arcs = 0usize;
        let mut swapped = 0usize;
        let mut major_gap = Vec::new();
        for primitive in &model.primitives {
            let PaperShape::Arc {
                center_mm,
                radius_mm,
                start_angle,
                end_angle,
                clockwise,
            } = &primitive.shape
            else {
                continue;
            };
            arcs += 1;
            // The arc as the writer draws it: the record's own direction.
            let sweep = if *clockwise {
                -(*start_angle - *end_angle).rem_euclid(TAU)
            } else {
                (*end_angle - *start_angle).rem_euclid(TAU)
            };
            let minor = chain(*center_mm, *radius_mm, *start_angle, sweep);
            // The arc between the same two endpoints on the other side: leave
            // the same end and run the rest of the turn.
            let other_sweep = if sweep > 0.0 {
                sweep - TAU
            } else {
                sweep + TAU
            };
            let other = chain(*center_mm, *radius_mm, *start_angle, other_sweep);
            let minor_cover = covered(&minor, &grid);
            let major_cover = covered(&other, &grid);
            if verbose && (center_mm[0] - verbose_at[0]).hypot(center_mm[1] - verbose_at[1]) < 12.0
            {
                println!(
                    "   arc at ({:.2},{:.2}) r {:.2} sweep {:.1} deg: drawn {minor_cover:.2} other {major_cover:.2}",
                    center_mm[0],
                    center_mm[1],
                    radius_mm,
                    sweep.to_degrees()
                );
            }
            if major_cover > minor_cover + 0.05 {
                swapped += 1;
                major_gap.push((
                    major_cover - minor_cover,
                    *radius_mm,
                    *center_mm,
                    sweep.to_degrees(),
                    primitive.symbol.clone(),
                ));
            }
        }
        major_gap.sort_by(|left, right| right.0.total_cmp(&left.0));
        println!("{name}: {arcs} paper arcs, {swapped} that the reference inks the long way round");
        for (gap, radius, center, sweep, symbol) in major_gap.iter().take(8) {
            let symbol = symbol
                .as_deref()
                .map(|path| path.rsplit(['\\', '/']).next().unwrap_or(path))
                .unwrap_or("-");
            println!(
                "   +{gap:.2} cover  r {radius:.2} at ({:.2},{:.2}) sweep {sweep:.1} deg  {symbol}",
                center[0], center[1]
            );
        }
    }
    Ok(())
}
