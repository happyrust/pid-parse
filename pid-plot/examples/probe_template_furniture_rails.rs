//! Where the sheet template's own furniture rails sit, and what a mask edge
//! placed on one of them would take in.
//!
//! ```text
//! cargo run -p pid-plot --example probe_template_furniture_rails
//! ```
//!
//! Read-only: layout and reference reads only, no PDF is written.
//!
//! Settles the left column's mask edge. `template_masks`' rectangles were first
//! surveyed from the five reference plots' margin **text origins**
//! (`docs/analysis/2026-10-12-template-mask-samples.md`), so their right edge
//! followed the lettering to `x = 72` (A0) / `70` (A1) and left the frame's own
//! lines outside. The template hangs a 50 × 10 mm box from the top frame at
//! `x 25..75`; this prints that rail's as-drawn `x` and stroke width for both
//! sheets (`x = 74.999970`, 0.25 mm wide, so its paper edge is 75.125), then
//! lists the reference ink inside the band a mask edge moved to 75.125 would
//! cover — which is the box on the two sheets whose reference drew it and
//! nothing at all on the other three.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;

const A0: &str = "I井场 注采阀组工艺及自控流程图";
const A1: &str = "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图";

/// Every drawing with a reference plot, its mask's old right edge and its
/// sheet's mask rectangle in y.
const BANDS: [(&str, f64, (f64, f64)); 5] = [
    (A0, 72.0, (560.0, 841.0)),
    (A1, 70.0, (310.0, 594.0)),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        70.0,
        (310.0, 594.0),
    ),
    (
        "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        70.0,
        (310.0, 594.0),
    ),
    (
        "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        70.0,
        (310.0, 594.0),
    ),
];

/// The new right edge: the box rail's centre plus half its 0.25 mm stroke.
const RAIL_EDGE_MM: f64 = 75.125;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn segments(shape: &PaperShape) -> Vec<([f64; 2], [f64; 2])> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![(*start_mm, *end_mm)],
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && points.len() > 2 {
                points.push(points[0]);
            }
            points.windows(2).map(|pair| (pair[0], pair[1])).collect()
        }
        _ => Vec::new(),
    }
}

/// The vertical rails and horizontal rails the mask would swallow: for every
/// `x`/`y` in range, the ink of the model that lies there and how far it runs.
fn rails(model: &PaperModel, x_max: f64, y_min: f64, y_max: f64) {
    let mut verticals = BTreeMap::<String, (f64, f64, f64, usize)>::new();
    let mut horizontals = BTreeMap::<String, (f64, f64, f64, usize)>::new();
    for primitive in &model.primitives {
        let width = primitive
            .stroke
            .as_ref()
            .map_or(f64::NAN, |stroke| stroke.width_mm);
        for (start, end) in segments(&primitive.shape) {
            let dx = (end[0] - start[0]).abs();
            let dy = (end[1] - start[1]).abs();
            let vertical = dx < 1e-6 && dy > 1e-6;
            if vertical && (start[0] < 25.0 || start[0] > x_max) {
                continue;
            }
            let low_y = start[1].min(end[1]);
            let high_y = start[1].max(end[1]);
            if vertical && !(high_y < y_min || low_y > y_max) {
                let entry = verticals
                    .entry(format!("{:.6} w {width:.6}", start[0]))
                    .or_insert((low_y, high_y, 0.0, 0));
                entry.0 = entry.0.min(low_y);
                entry.1 = entry.1.max(high_y);
                entry.2 += dy;
                entry.3 += 1;
            } else if !vertical
                && (dy < 1e-6 && dx > 1e-6 && start[1] >= y_min && start[1] <= y_max)
            {
                let low_x = start[0].min(end[0]);
                if low_x > x_max {
                    continue;
                }
                let high_x = start[0].max(end[0]);
                let entry = horizontals
                    .entry(format!("{:.6} w {width:.6}", start[1]))
                    .or_insert((low_x, high_x, 0.0, 0));
                entry.0 = entry.0.min(low_x);
                entry.1 = entry.1.max(high_x);
                entry.2 += dx;
                entry.3 += 1;
            }
        }
    }
    println!("  vertical rails (x 25..{x_max}, y {y_min}..{y_max}):");
    for (x, (low, high, length, count)) in &verticals {
        println!("    x {x:>22}  n {count:>3}  y {low:8.3}..{high:8.3}  total {length:8.3} mm");
    }
    println!("  horizontal rails (y {y_min}..{y_max}, x 25..{x_max}):");
    for (y, (low, high, length, count)) in &horizontals {
        println!("    y {y:>22}  n {count:>3}  x {low:8.3}..{high:8.3}  total {length:8.3} mm");
    }
}

/// Reference ink inside the band a mask edge moved to [`RAIL_EDGE_MM`] covers.
///
/// Filtered on the segment's midpoint, so a rail that starts left of the band
/// and crosses it is listed only when its own middle is inside.
fn reference_band(corpus: &std::path::Path, name: &str, from: f64, y: (f64, f64)) {
    let Ok(bytes) = std::fs::read(corpus.join(format!("{name}.pdf"))) else {
        println!("  {name}: no reference plot");
        return;
    };
    let Ok(ink) = read_plot(&bytes) else {
        println!("  {name}: unreadable");
        return;
    };
    let mut rows: Vec<(f64, [f64; 2], [f64; 2])> = Vec::new();
    for segment in &ink.segments {
        let mid = [
            (segment.start_mm[0] + segment.end_mm[0]) / 2.0,
            (segment.start_mm[1] + segment.end_mm[1]) / 2.0,
        ];
        if mid[0] > from && mid[0] <= RAIL_EDGE_MM && mid[1] >= y.0 && mid[1] <= y.1 {
            let dx = segment.end_mm[0] - segment.start_mm[0];
            let dy = segment.end_mm[1] - segment.start_mm[1];
            rows.push(((dx * dx + dy * dy).sqrt(), segment.start_mm, segment.end_mm));
        }
    }
    rows.sort_by(|left, right| right.0.total_cmp(&left.0));
    println!(
        "  {name}: {} segments, {:.3} mm in x {from}..{RAIL_EDGE_MM} y {:.0}..{:.0}",
        rows.len(),
        rows.iter().map(|row| row.0).sum::<f64>(),
        y.0,
        y.1
    );
    for (length, start, end) in rows.iter().take(8) {
        println!(
            "      {length:8.3}  ({:8.3},{:8.3}) -> ({:8.3},{:8.3})",
            start[0], start[1], end[0], end[1]
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let model = lay_out_drawing(&corpus.join(format!("{A1}.pid")), &corpus)?;
    println!("== the A1 sheet's furniture in the left column's corner ==");
    rails(&model, 100.0, 280.0, 600.0);
    let model = lay_out_drawing(&corpus.join(format!("{A0}.pid")), &corpus)?;
    println!("== the A0 sheet's furniture in the left column's corner ==");
    rails(&model, 110.0, 520.0, 850.0);

    println!("== reference ink the wider rectangle takes in ==");
    for (name, from, y) in BANDS {
        reference_band(&corpus, name, from, y);
    }
    Ok(())
}
