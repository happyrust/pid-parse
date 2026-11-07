//! Which strokes does the pen tally charge as a difference, and what draws them?
//!
//! [`CompareReport::pen`] pairs our strokes with the reference's at a locus and
//! forgives a width the reference states *anywhere* there. So a differing pair
//! is not "a stroke we got wrong" until you know what is at the locus; when the
//! five corpus pages last differed, thirteen of the fourteen were a filled
//! region's outline charged against a neighbouring hairline.
//!
//! This probe walks the same walk as `compare::pen_pairs` — same constants, same
//! forgiveness rules — and prints every stroke it counts as differing: the
//! reference's widths there, the model primitive behind the stroke, and the six
//! nearest reference segments with their distance and angle. It is the evidence
//! behind `docs/analysis/2026-10-30-a-fill-outline-is-not-a-pen.md`.
//!
//! ```text
//! cargo run -p pid-plot --example probe_pen_width_sites -- [drawing ...]
//! ```
//!
//! With no drawing names it walks the original five sheets. The corpus is
//! `PID_PARSE_SELF_CHECK_DIR`, or `D:\work\cad\PID自研软件验证`. Read-only.

use std::path::PathBuf;

use pid_plot::compare::{compare_plots, template_masks};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

const NAMES: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

// Mirror of `compare`'s pairing constants, which are private.
const PEN_WINDOW_MM: f64 = 1.0;
const PEN_DOT: f64 = 0.99;
const PEN_FILL_DOT: f64 = 0.95;
const PEN_MIN_MM: f64 = 0.2;
const PEN_LENGTH_RATIO: f64 = 0.5;
const PEN_ON_MM: f64 = 0.08;
const PEN_SAME_PEN_ON_MM: f64 = 0.20;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn centi_mm(width: f64) -> u64 {
    (width * 100.0).round().max(0.0) as u64
}

fn length(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn is_sub_piece(
    start: [f64; 2],
    end: [f64; 2],
    candidate_start: [f64; 2],
    candidate_end: [f64; 2],
    tolerance_mm: f64,
) -> bool {
    let along = [
        candidate_end[0] - candidate_start[0],
        candidate_end[1] - candidate_start[1],
    ];
    let candidate_len = along[0].hypot(along[1]);
    if candidate_len < 1e-9 {
        return false;
    }
    let unit = [along[0] / candidate_len, along[1] / candidate_len];
    [start, end].into_iter().all(|point| {
        let delta = [point[0] - candidate_start[0], point[1] - candidate_start[1]];
        let along_mm = delta[0] * unit[0] + delta[1] * unit[1];
        let across_mm = (delta[0] * unit[1] - delta[1] * unit[0]).abs();
        across_mm <= tolerance_mm
            && (-tolerance_mm..=candidate_len + tolerance_mm).contains(&along_mm)
    })
}

fn ring_edge(chain: &[[f64; 2]], start: [f64; 2], end: [f64; 2]) -> bool {
    if chain.len() < 2 {
        return false;
    }
    let mut edges: Vec<([f64; 2], [f64; 2])> =
        chain.windows(2).map(|pair| (pair[0], pair[1])).collect();
    edges.push((chain[chain.len() - 1], chain[0]));
    edges.iter().any(|(a, b)| {
        (length(*a, start) < 1e-3 && length(*b, end) < 1e-3)
            || (length(*a, end) < 1e-3 && length(*b, start) < 1e-3)
    })
}

fn owner_of(primitive: &pid_plot::model::PaperPrimitive, is_fill: bool) -> String {
    let declared = primitive
        .stroke
        .as_ref()
        .map_or(0.25, |stroke| stroke.width_mm);
    let kind = match primitive.shape {
        pid_plot::model::PaperShape::Line { .. } => "line",
        pid_plot::model::PaperShape::Arc { .. } => "arc",
        pid_plot::model::PaperShape::Circle { .. } => "circle",
        pid_plot::model::PaperShape::Polyline { .. } => "polyline",
        pid_plot::model::PaperShape::Fill { .. } => "fill",
        pid_plot::model::PaperShape::Text { .. } => "text",
    };
    let role = if is_fill { "fill" } else { "shape" };
    let name = primitive
        .symbol
        .clone()
        .or_else(|| primitive.stream_path.clone())
        .unwrap_or_else(|| "(unnamed)".into());
    let furniture = if primitive.furniture {
        " furniture"
    } else {
        ""
    };
    format!(
        "{kind} {role} {name} conf={} declared {declared:.4} mm{furniture}",
        primitive.confidence
    )
}

fn shape_gap(shape: &PaperShape, start: [f64; 2], end: [f64; 2]) -> Option<f64> {
    let mid = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
    match shape {
        PaperShape::Line { start_mm, end_mm } => Some(dist_point_seg(mid, *start_mm, *end_mm)),
        PaperShape::Polyline { points_mm, .. }
        | PaperShape::Fill {
            chain_mm: points_mm,
            ..
        } => points_mm
            .windows(2)
            .map(|pair| dist_point_seg(mid, pair[0], pair[1]))
            .reduce(f64::min),
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => Some((length(mid, *center_mm) - radius_mm).abs()),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        } => Some((length(mid, *center_mm) - radius_mm).abs()),
        _ => None,
    }
}

fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length2 = dx * dx + dy * dy;
    if length2 <= 1e-12 {
        return length(point, start);
    }
    let t = (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length2).clamp(0.0, 1.0);
    length(point, [start[0] + dx * t, start[1] + dy * t])
}

/// One stroke the tally charges as differing.
struct Site {
    ours_centi_mm: u64,
    reference_centi_mm: u64,
    len_mm: f64,
    start: [f64; 2],
    end: [f64; 2],
    reference_widths: Vec<u64>,
    owner: String,
    is_fill: bool,
    detail: String,
}

fn main() {
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    let mut grand_differed = 0usize;
    let mut grand_fill = 0usize;
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let names: Vec<&str> = if asked.is_empty() {
        NAMES.to_vec()
    } else {
        asked.iter().map(String::as_str).collect()
    };
    for name in names {
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
        let fit = &report.fit;

        let mut sites: Vec<Site> = Vec::new();
        for segment in &ours.segments {
            let len = length(segment.start_mm, segment.end_mm);
            if len < PEN_MIN_MM {
                continue;
            }
            let ours_width = centi_mm(segment.width_mm);
            let back = |point: [f64; 2]| {
                [
                    (point[0] - fit.translation_mm[0]) / fit.scale,
                    (point[1] - fit.translation_mm[1]) / fit.scale,
                ]
            };
            let mapped = back(segment.start_mm);
            let mapped_end = back(segment.end_mm);
            let direction = [mapped_end[0] - mapped[0], mapped_end[1] - mapped[1]];
            let plen = direction[0].hypot(direction[1]);
            if plen < 1e-9 {
                continue;
            }
            let unit = [direction[0] / plen, direction[1] / plen];
            let midpoint = [
                (mapped[0] + mapped_end[0]) / 2.0,
                (mapped[1] + mapped_end[1]) / 2.0,
            ];
            let mut widths: Vec<u64> = Vec::new();
            let mut near: Vec<(f64, u64, f64, f64, f64)> = Vec::new();
            let mut nearest: Option<(f64, u64)> = None;
            for candidate in &reference.segments {
                let candidate_mid = [
                    (candidate.start_mm[0] + candidate.end_mm[0]) / 2.0,
                    (candidate.start_mm[1] + candidate.end_mm[1]) / 2.0,
                ];
                let distance = length(candidate_mid, midpoint);
                if distance > PEN_WINDOW_MM {
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
                let ratio = (other_len / plen).min(plen / other_len);
                let width = centi_mm(candidate.width_mm);
                near.push((distance, width, dot, ratio, other_len));
                let dot_min = if ours_width == 0 {
                    PEN_FILL_DOT
                } else {
                    PEN_DOT
                };
                if dot < dot_min {
                    continue;
                }
                let tolerance = if width == ours_width {
                    PEN_SAME_PEN_ON_MM
                } else {
                    PEN_ON_MM
                };
                if ours_width != 0
                    && ratio < PEN_LENGTH_RATIO
                    && !is_sub_piece(
                        mapped,
                        mapped_end,
                        candidate.start_mm,
                        candidate.end_mm,
                        tolerance,
                    )
                {
                    continue;
                }
                widths.push(width);
                if nearest.is_none_or(|(best, _)| distance < best) {
                    nearest = Some((distance, width));
                }
            }
            let Some((_, fallback)) = nearest else {
                continue;
            };
            if widths.contains(&ours_width) {
                continue;
            }
            widths.sort_unstable();
            widths.dedup();
            let (start, end) = (segment.start_mm, segment.end_mm);
            let mut owner = String::from("(nothing in the model)");
            let mut is_fill = false;
            for primitive in &model.primitives {
                let on_shape = match &primitive.shape {
                    PaperShape::Fill { chain_mm, .. } => ring_edge(chain_mm, start, end),
                    PaperShape::Line { start_mm, end_mm } => {
                        (length(*start_mm, start) < 1e-3 && length(*end_mm, end) < 1e-3)
                            || (length(*start_mm, end) < 1e-3 && length(*end_mm, start) < 1e-3)
                    }
                    PaperShape::Polyline { points_mm, .. } => ring_edge(points_mm, start, end),
                    _ => false,
                };
                if !on_shape {
                    continue;
                }
                is_fill = matches!(primitive.shape, PaperShape::Fill { .. });
                owner = owner_of(primitive, is_fill);
                break;
            }
            if owner.starts_with("(nothing") {
                let mut nearest: Option<(f64, String, bool)> = None;
                for primitive in &model.primitives {
                    let Some(distance) = shape_gap(&primitive.shape, start, end) else {
                        continue;
                    };
                    if nearest.as_ref().is_none_or(|(best, _, _)| distance < *best) {
                        let fill = matches!(primitive.shape, PaperShape::Fill { .. });
                        nearest = Some((distance, owner_of(primitive, fill), fill));
                    }
                }
                if let Some((distance, named, fill)) = nearest {
                    is_fill = fill;
                    owner = format!("nearest {distance:.3} mm: {named}");
                }
            }
            near.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut detail = String::new();
            for (distance, width, dot, ratio, other_len) in near.iter().take(6) {
                detail.push_str(&format!(
                    "\n        near {distance:.2} mm  {:.2} mm  dot {dot:.3}  ratio {ratio:.2}  len {other_len:.2}",
                    *width as f64 / 100.0
                ));
            }
            sites.push(Site {
                ours_centi_mm: ours_width,
                reference_centi_mm: fallback,
                len_mm: len,
                start,
                end,
                reference_widths: widths,
                owner,
                is_fill,
                detail,
            });
        }
        grand_differed += sites.len();
        grand_fill += sites.iter().filter(|site| site.is_fill).count();
        println!(
            "{name}\n   {} strokes differ (report says {})",
            sites.len(),
            report.pen.differed
        );
        sites.sort_by(|left, right| right.len_mm.total_cmp(&left.len_mm));
        for site in &sites {
            let widths: Vec<String> = site
                .reference_widths
                .iter()
                .map(|width| format!("{:.2}", *width as f64 / 100.0))
                .collect();
            println!(
                "   ours {:.2} ref {:.2}  {:>6.2} mm  ({:>7.2},{:>7.2})->({:>7.2},{:>7.2})  ref widths [{}]  {}{}{}",
                site.ours_centi_mm as f64 / 100.0,
                site.reference_centi_mm as f64 / 100.0,
                site.len_mm,
                site.start[0],
                site.start[1],
                site.end[0],
                site.end[1],
                widths.join(" "),
                if site.is_fill { "FILL " } else { "" },
                site.owner,
                site.detail,
            );
        }
    }
    println!("\n{grand_differed} differing strokes, {grand_fill} of them fill outlines");
}
