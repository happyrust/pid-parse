//! Attribute one symbol file's residual ink: the strokes the paper model
//! draws where the reference plot has none, and the reference ink the model
//! does not draw, printed as paper-millimetre segments for comparison.
//!
//!     cargo run -p pid-plot --example diagnose_symbol_residual -- <drawing> <symbol>
//!
//! `symbol` matches the basename the comparator reports, such as
//! `Diaphragm Actuator.sym`.

use std::collections::HashMap;
use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;

const SAMPLE_MM: f64 = 0.5;
const COVER_MM: f64 = 0.5;
const CELL_MM: f64 = 5.0;

/// One straight run of paper millimetres.
type Stroke = ([f64; 2], [f64; 2]);

struct Grid {
    buckets: HashMap<(i32, i32), Vec<Stroke>>,
}

impl Grid {
    fn new(segments: &[([f64; 2], [f64; 2])]) -> Self {
        let mut grid = Self {
            buckets: HashMap::new(),
        };
        for segment in segments {
            for key in cells(*segment, CELL_MM) {
                grid.buckets.entry(key).or_default().push(*segment);
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
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) {
                    for (a, b) in bucket {
                        best = best.min(dist(point, *a, *b));
                    }
                }
            }
        }
        best
    }
}

fn cells((a, b): ([f64; 2], [f64; 2]), cell: f64) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let steps = (len(a, b) / cell).ceil() as usize + 1;
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        out.push((
            ((a[0] + (b[0] - a[0]) * t) / cell).floor() as i32,
            ((a[1] + (b[1] - a[1]) * t) / cell).floor() as i32,
        ));
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn len(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn dist(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1e-12 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let t = (((point[0] - a[0]) * ab[0] + (point[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
    (point[0] - (a[0] + ab[0] * t)).hypot(point[1] - (a[1] + ab[1] * t))
}

/// Break one paper shape into straight pieces.
fn pieces(shape: &PaperShape) -> Vec<([f64; 2], [f64; 2])> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![(*start_mm, *end_mm)],
        PaperShape::Polyline { points_mm, closed } => chain(points_mm, *closed),
        PaperShape::Fill { chain_mm, .. } => chain(chain_mm, true),
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => arc(*center_mm, *radius_mm, 0.0, -std::f64::consts::TAU),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let sweep = if *clockwise {
                -(*start_angle - *end_angle).rem_euclid(std::f64::consts::TAU)
            } else {
                (*end_angle - *start_angle).rem_euclid(std::f64::consts::TAU)
            };
            arc(*center_mm, *radius_mm, *start_angle, sweep)
        }
        PaperShape::Text { .. } => Vec::new(),
    }
}

fn chain(points: &[[f64; 2]], closed: bool) -> Vec<([f64; 2], [f64; 2])> {
    let mut out: Vec<_> = points.windows(2).map(|pair| (pair[0], pair[1])).collect();
    if closed && points.len() >= 2 {
        out.push((points[points.len() - 1], points[0]));
    }
    out
}

fn arc(center: [f64; 2], radius: f64, start: f64, sweep: f64) -> Vec<([f64; 2], [f64; 2])> {
    let steps = ((sweep.abs() * radius / 0.15).ceil() as usize).clamp(6, 180);
    let at = |step: usize| {
        let angle = start + sweep * (step as f64 / steps as f64);
        [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]
    };
    (0..steps).map(|step| (at(step), at(step + 1))).collect()
}

/// Uncovered length of `segment`, and the first uncovered point.
fn uncovered(segment: ([f64; 2], [f64; 2]), grid: &Grid) -> (f64, Option<[f64; 2]>) {
    let (a, b) = segment;
    let length = len(a, b);
    let steps = ((length / SAMPLE_MM).round() as usize).max(1);
    let mut miss = 0;
    let mut first = None;
    for step in 0..steps {
        let t = (step as f64 + 0.5) / steps as f64;
        let point = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        if grid.distance(point) > COVER_MM {
            miss += 1;
            if first.is_none() {
                first = Some(point);
            }
        }
    }
    (length * miss as f64 / steps as f64, first)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let name = args
        .next()
        .expect("usage: <drawing name> <symbol basename>");
    let wanted = args
        .next()
        .expect("usage: <drawing name> <symbol basename>");
    let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    );
    let reference = read_plot(&std::fs::read(corpus.join(format!("{name}.pdf")))?)?;
    let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus)?;
    if wanted == "pdf" {
        let ours = read_plot(&std::fs::read(corpus.join(format!("plot/{name}.pdf")))?)?;
        let theirs: Vec<Stroke> = reference
            .segments
            .iter()
            .map(|segment| (segment.start_mm, segment.end_mm))
            .collect();
        let their_grid = Grid::new(&theirs);
        let mut by_label: HashMap<String, (f64, usize)> = HashMap::new();
        let mut rows: Vec<(f64, Stroke, String)> = Vec::new();
        for segment in &ours.segments {
            let stroke = (segment.start_mm, segment.end_mm);
            let (miss, _) = uncovered(stroke, &their_grid);
            if miss <= 0.6 {
                continue;
            }
            let mid = [
                (stroke.0[0] + stroke.1[0]) * 0.5,
                (stroke.0[1] + stroke.1[1]) * 0.5,
            ];
            let mut best = (f64::MAX, "图纸线".to_string());
            for primitive in &model.primitives {
                if primitive.confidence == "probe_only" {
                    continue;
                }
                let label = match (&primitive.symbol, &primitive.stream_path) {
                    (Some(symbol), _) => symbol
                        .rsplit(['\\', '/'])
                        .next()
                        .unwrap_or(symbol)
                        .to_string(),
                    (None, Some(stream)) => stream.clone(),
                    (None, None) => "图纸线".to_string(),
                };
                for piece in pieces(&primitive.shape) {
                    let away = dist(mid, piece.0, piece.1);
                    if away < best.0 {
                        best = (away, label.clone());
                    }
                }
            }
            by_label
                .entry(best.1.clone())
                .and_modify(|row| {
                    row.0 += miss;
                    row.1 += 1;
                })
                .or_insert((miss, 1));
            rows.push((miss, stroke, best.1));
        }
        let mut grouped: Vec<(String, f64, usize)> = by_label
            .into_iter()
            .map(|(label, (miss, count))| (label, miss, count))
            .collect();
        grouped.sort_by(|left, right| right.1.total_cmp(&left.1));
        println!(
            "PDF extra {:.2} mm in {} strokes",
            rows.iter().map(|row| row.0).sum::<f64>(),
            rows.len()
        );
        for (label, miss, count) in grouped.iter().take(15) {
            println!("  {miss:9.2} mm  {count:5} strokes  {label}");
        }
        rows.sort_by(|left, right| right.0.total_cmp(&left.0));
        for (miss, (a, b), label) in rows.iter().take(25) {
            println!(
                "  {miss:6.2} mm  ({:8.2},{:8.2}) -> ({:8.2},{:8.2})  {label}",
                a[0], a[1], b[0], b[1]
            );
        }
        return Ok(());
    }
    let theirs: Vec<([f64; 2], [f64; 2])> = reference
        .segments
        .iter()
        .map(|segment| (segment.start_mm, segment.end_mm))
        .collect();
    let their_grid = Grid::new(&theirs);
    if wanted == "-" {
        println!(
            "debug: {} primitives, {} reference segments, probe {:?}",
            model.primitives.len(),
            theirs.len(),
            their_grid.distance([-100.0, -100.0])
        );
        for primitive in model.primitives.iter().take(3) {
            let pieces = pieces(&primitive.shape);
            println!(
                "debug shape {:?} pieces {} first {:?} len {:.3} unc {:?}",
                std::mem::discriminant(&primitive.shape),
                pieces.len(),
                pieces.first(),
                pieces.first().map_or(0.0, |s| len(s.0, s.1)),
                pieces.first().map(|s| uncovered(*s, &their_grid))
            );
        }
        let mut by_source: HashMap<String, f64> = HashMap::new();
        let mut total = 0.0;
        for primitive in &model.primitives {
            if primitive.confidence == "probe_only" {
                continue;
            }
            let label = match (&primitive.symbol, &primitive.stream_path) {
                (Some(symbol), _) => symbol
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(symbol)
                    .to_string(),
                (None, Some(stream)) => stream.clone(),
                (None, None) => "图纸线".to_string(),
            };
            let mut miss = 0.0;
            for segment in pieces(&primitive.shape) {
                miss += uncovered(segment, &their_grid).0;
            }
            if miss > 0.01 {
                *by_source.entry(label).or_default() += miss;
                total += miss;
            }
        }
        let mut rows: Vec<(String, f64)> = by_source.into_iter().collect();
        rows.sort_by(|left, right| right.1.total_cmp(&left.1));
        println!("TOTAL un-attributed extra {total:.2} mm");
        for (label, miss) in rows.iter().take(25) {
            println!("  {miss:9.2} mm  {label}");
        }
        return Ok(());
    }
    let mut ours: Vec<([f64; 2], [f64; 2])> = Vec::new();
    let mut count = 0;
    for primitive in &model.primitives {
        let Some(symbol) = &primitive.symbol else {
            continue;
        };
        let basename = symbol.rsplit(['\\', '/']).next().unwrap_or(symbol);
        if basename != wanted {
            continue;
        }
        count += 1;
        ours.extend(pieces(&primitive.shape));
    }
    let our_grid = Grid::new(&ours);

    println!("{wanted}: {count} primitives, {} strokes", ours.len());
    let mut extra: Vec<f64> = Vec::new();
    let mut rows = Vec::new();
    for segment in &ours {
        let (miss, first) = uncovered(*segment, &their_grid);
        if miss > 0.6 {
            extra.push(miss);
            rows.push((miss, *segment, first));
        }
    }
    rows.sort_by(|left, right| right.0.total_cmp(&left.0));
    println!(
        "EXTRA {:.2} mm in {} strokes (of {:.2} mm total)",
        extra.iter().sum::<f64>(),
        rows.len(),
        ours.iter().map(|s| len(s.0, s.1)).sum::<f64>()
    );
    for (miss, (a, b), first) in rows.iter().take(30) {
        println!(
            "  {miss:6.2} mm  ({:8.2},{:8.2}) -> ({:8.2},{:8.2})  len {:5.2}  first-miss {:?}",
            a[0],
            a[1],
            b[0],
            b[1],
            len(*a, *b),
            first.map(|point| (round2(point[0]), round2(point[1])))
        );
    }

    // Reference ink inside the symbol's own neighbourhood that we do not draw.
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for (a, b) in &ours {
        for point in [a, b] {
            min[0] = min[0].min(point[0]);
            min[1] = min[1].min(point[1]);
            max[0] = max[0].max(point[0]);
            max[1] = max[1].max(point[1]);
        }
    }
    let mut missing = Vec::new();
    for segment in &theirs {
        let mid = [
            (segment.0[0] + segment.1[0]) * 0.5,
            (segment.0[1] + segment.1[1]) * 0.5,
        ];
        if mid[0] < min[0] - 20.0
            || mid[0] > max[0] + 20.0
            || mid[1] < min[1] - 20.0
            || mid[1] > max[1] + 20.0
        {
            continue;
        }
        let (gone, first) = uncovered(*segment, &our_grid);
        if gone > 0.6 {
            missing.push((gone, *segment, first));
        }
    }
    missing.sort_by(|left, right| right.0.total_cmp(&left.0));
    println!(
        "MISSING {:.2} mm in {} reference strokes near the symbol",
        missing.iter().map(|row| row.0).sum::<f64>(),
        missing.len()
    );
    for (gone, (a, b), first) in missing.iter().take(30) {
        println!(
            "  {gone:6.2} mm  ({:8.2},{:8.2}) -> ({:8.2},{:8.2})  len {:5.2}  first-miss {:?}",
            a[0],
            a[1],
            b[0],
            b[1],
            len(*a, *b),
            first.map(|point| (round2(point[0]), round2(point[1])))
        );
    }

    let mut texts = Vec::new();
    for primitive in &model.primitives {
        let Some(symbol) = &primitive.symbol else {
            continue;
        };
        let basename = symbol.rsplit(['\\', '/']).next().unwrap_or(symbol);
        if basename != wanted {
            continue;
        }
        if !matches!(primitive.shape, PaperShape::Text { .. }) {
            let pen = primitive.stroke.as_ref();
            println!(
                "  pen rgb={:?} w={:?} dash={:?}",
                pen.map(|pen| pen.rgb),
                pen.map(|pen| round2(pen.width_mm)),
                pen.map(|pen| pen.dash_mm.iter().map(|v| round2(*v)).collect::<Vec<_>>())
            );
        }
        if let PaperShape::Text {
            origin_mm,
            value,
            height_mm,
            rotation,
            font,
            ..
        } = &primitive.shape
        {
            texts.push((
                *origin_mm,
                value.clone(),
                *height_mm,
                *rotation,
                font.clone(),
            ));
        }
    }
    println!("TEXT {} runs", texts.len());
    for (origin, value, height, rotation, font) in texts {
        println!(
            "  ({:8.2},{:8.2}) h={:?} r={:.3} {font:?} {value:?}",
            origin[0],
            origin[1],
            height.map(round2),
            rotation
        );
    }
    Ok(())
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
