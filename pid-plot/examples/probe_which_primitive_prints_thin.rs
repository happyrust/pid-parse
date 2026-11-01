//! Which primitive of ours is the stroke the reference prints with a different
//! pen?
//!
//! `probe_pen_width_pairs` says 305 strokes on the five reference pages carry a
//! different width than the plotter gave the same locus. That probe tallies
//! widths, not authorship, so it cannot say *what* drew them. This one pairs the
//! same way and then walks our own model to name the primitive that lies under
//! each mismatching stroke: the symbol it belongs to, its shape, and the width
//! it declares.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_primitive_prints_thin
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_plot::compare::template_masks;
use pid_plot::compare::{Fit, compare_plots};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{InkSegment, PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

const NAMES: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn hundredths(width: f64) -> u64 {
    (width * 100.0).round().max(0.0) as u64
}

fn length(segment: &InkSegment) -> f64 {
    (segment.start_mm[0] - segment.end_mm[0]).hypot(segment.start_mm[1] - segment.end_mm[1])
}

/// Walk a shape into its vertex chains, so a stroke can be attributed to it.
fn chains(shape: &PaperShape) -> Vec<Vec<[f64; 2]>> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![vec![*start_mm, *end_mm]],
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
            clockwise,
        } => {
            let sweep = if *clockwise {
                let mut sweep = start_angle - end_angle;
                while sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                -sweep
            } else {
                let mut sweep = end_angle - start_angle;
                while sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                sweep
            };
            const STEPS: usize = 24;
            let points = (0..=STEPS)
                .map(|step| {
                    let angle = start_angle + sweep * step as f64 / STEPS as f64;
                    [
                        center_mm[0] + radius_mm * angle.cos(),
                        center_mm[1] + radius_mm * angle.sin(),
                    ]
                })
                .collect();
            vec![points]
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => {
            const STEPS: usize = 32;
            let points = (0..=STEPS)
                .map(|step| {
                    let angle = std::f64::consts::TAU * step as f64 / STEPS as f64;
                    [
                        center_mm[0] + radius_mm * angle.cos(),
                        center_mm[1] + radius_mm * angle.sin(),
                    ]
                })
                .collect();
            vec![points]
        }
        PaperShape::Fill { chain_mm, .. } => vec![chain_mm.clone()],
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && points.first() != points.last() {
                points.push(points[0]);
            }
            vec![points]
        }
        PaperShape::Text { .. } => Vec::new(),
    }
}

fn point_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-18 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let t = (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    let projected = [a[0] + t * dx, a[1] + t * dy];
    (point[0] - projected[0]).hypot(point[1] - projected[1])
}

fn distance_to(chain: &[[f64; 2]], point: [f64; 2]) -> f64 {
    chain
        .windows(2)
        .map(|pair| point_to_segment(point, pair[0], pair[1]))
        .fold(f64::INFINITY, f64::min)
}

/// The primitive of ours nearest `point` (paper millimetres, our own frame).
///
/// A primitive that inks nothing — a zero-length line, a one-vertex chain — is
/// skipped: it sits at a point without drawing, so it would otherwise claim
/// every stroke that passes near where it stands. `Remarks.sym`'s cached body
/// is exactly that: three zero-length lines.
fn owner(model: &PaperModel, point: [f64; 2]) -> Option<(&PaperShape, Option<f64>, Option<&str>)> {
    let mut best: Option<(f64, &PaperShape, Option<f64>, Option<&str>)> = None;
    for primitive in &model.primitives {
        let width = primitive.stroke.as_ref().map(|stroke| stroke.width_mm);
        for chain in chains(&primitive.shape) {
            let inked: f64 = chain
                .windows(2)
                .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
                .sum();
            if inked < 0.05 {
                continue;
            }
            let distance = distance_to(&chain, point);
            if distance > 0.35 {
                continue;
            }
            if best.is_none_or(|(closest, ..)| distance < closest) {
                best = Some((
                    distance,
                    &primitive.shape,
                    width,
                    primitive.symbol.as_deref(),
                ));
            }
        }
    }
    best.map(|(_, shape, width, symbol)| (shape, width, symbol))
}

fn shape_name(shape: &PaperShape) -> &'static str {
    match shape {
        PaperShape::Line { .. } => "line",
        PaperShape::Arc { .. } => "arc",
        PaperShape::Circle { .. } => "circle",
        PaperShape::Fill { .. } => "fill",
        PaperShape::Polyline { .. } => "polyline",
        PaperShape::Text { .. } => "text",
    }
}

fn leaf(symbol: Option<&str>) -> String {
    symbol.map_or_else(
        || "(the sheet's own ink)".to_string(),
        |path| path.rsplit('\\').next().unwrap_or(path).to_string(),
    )
}

/// Pair our segments with the reference's the way `probe_pen_width_pairs` does,
/// keeping the mismatches so they can be attributed.
fn mismatches(
    model: &PaperModel,
    reference: &PlotInk,
    ours: &PlotInk,
    fit: &Fit,
    tally: &mut BTreeMap<(String, String, u64, u64), usize>,
) -> usize {
    let mut found = 0usize;
    for segment in &ours.segments {
        if length(segment) < 0.2 {
            continue;
        }
        let mapped = [
            fit.scale * segment.start_mm[0] + fit.translation_mm[0],
            fit.scale * segment.start_mm[1] + fit.translation_mm[1],
        ];
        let mapped_end = [
            fit.scale * segment.end_mm[0] + fit.translation_mm[0],
            fit.scale * segment.end_mm[1] + fit.translation_mm[1],
        ];
        let direction = [mapped_end[0] - mapped[0], mapped_end[1] - mapped[1]];
        let len = direction[0].hypot(direction[1]);
        if len < 1e-9 {
            continue;
        }
        let unit = [direction[0] / len, direction[1] / len];
        let midpoint = [
            (mapped[0] + mapped_end[0]) / 2.0,
            (mapped[1] + mapped_end[1]) / 2.0,
        ];
        let mut widths: Vec<u64> = Vec::new();
        for candidate in &reference.segments {
            let candidate_mid = [
                (candidate.start_mm[0] + candidate.end_mm[0]) / 2.0,
                (candidate.start_mm[1] + candidate.end_mm[1]) / 2.0,
            ];
            if (candidate_mid[0] - midpoint[0]).hypot(candidate_mid[1] - midpoint[1]) > 1.0 {
                continue;
            }
            let other = [
                candidate.end_mm[0] - candidate.start_mm[0],
                candidate.end_mm[1] - candidate.start_mm[1],
            ];
            let other_len = other[0].hypot(other[1]);
            if other_len < 1e-9 {
                continue;
            }
            let dot = (unit[0] * other[0] / other_len + unit[1] * other[1] / other_len).abs();
            if dot < 0.99 {
                continue;
            }
            let ratio = (other_len / len).min(len / other_len);
            if ratio < 0.5 {
                continue;
            }
            widths.push(hundredths(candidate.width_mm));
        }
        if widths.is_empty() {
            continue;
        }
        let ours_width = hundredths(segment.width_mm);
        if widths.contains(&ours_width) {
            continue;
        }
        // The reference states a width here and none of them is ours.
        let reference_width = widths[0];
        let our_mid = [
            (segment.start_mm[0] + segment.end_mm[0]) / 2.0,
            (segment.start_mm[1] + segment.end_mm[1]) / 2.0,
        ];
        let (shape, declared, symbol) = owner(model, our_mid).map_or_else(
            || ("(none)".to_string(), None, None),
            |(shape, width, symbol)| (shape_name(shape).to_string(), width, symbol),
        );
        *tally
            .entry((
                format!(
                    "{shape} | {} | ours {:>5.2}",
                    leaf(symbol),
                    ours_width as f64 / 100.0
                ),
                String::new(),
                hundredths(declared.unwrap_or(0.0)),
                reference_width,
            ))
            .or_default() += 1;
        found += 1;
    }
    found
}

fn main() {
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    let mut tally: BTreeMap<(String, String, u64, u64), usize> = BTreeMap::new();
    for name in NAMES {
        let reference =
            read_plot(&std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference"))
                .expect("read reference");
        let model = lay_out_drawing(&corpus.join(format!("{name}.pid")), &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let pdf = write_plot(&model, &fonts).expect("write");
        let ours = read_plot(&pdf).expect("read ours");
        let masks = template_masks(reference.page_mm);
        let report = compare_plots(&reference, &ours, &masks);
        let mut here: BTreeMap<(String, String, u64, u64), usize> = BTreeMap::new();
        let found = mismatches(&model, &reference, &ours, &report.fit, &mut here);
        println!("\n{name}: {found} stroke(s) the reference prints with another pen");
        let mut rows: Vec<_> = here.iter().collect();
        rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        for ((label, _, declared, reference_width), count) in rows {
            println!(
                "     {count:>4} x {label}  declares {:>5.2} mm, reference {:>5.2} mm",
                *declared as f64 / 100.0,
                *reference_width as f64 / 100.0
            );
            *tally
                .entry((label.clone(), String::new(), *declared, *reference_width))
                .or_default() += count;
        }
    }

    println!("\nWHO DRAWS THEM");
    let mut rows: Vec<_> = tally.iter().collect();
    rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for ((label, _, declared, reference_width), count) in rows {
        println!(
            "   {count:>4} x {label}  declares {:>5.2} mm, reference {:>5.2} mm",
            *declared as f64 / 100.0,
            *reference_width as f64 / 100.0
        );
    }
}
