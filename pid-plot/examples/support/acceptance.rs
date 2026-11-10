//! The eleven `验收数据` sheets, laid out, lettered and written the way
//! `reference_plots_acceptance` does it, for the probes that ask what that
//! gate's readings rest on.
//!
//! Not an example of its own: cargo only looks for `examples/*.rs` and
//! `examples/*/main.rs`, so a file under `examples/support/` is source, not a
//! target. Each probe compiles it in with `#[path = "support/acceptance.rs"]`,
//! so the helper carries items no single probe reads.
//!
//! Read-only: nothing here writes to the corpus. In particular it does not
//! deliver plots — a probe compares the bytes it wrote in memory.
#![allow(dead_code)]

use std::path::PathBuf;

use pid_plot::compare::{CompareReport, MaskRect};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape, TextAnchor, lay_out_drawing};
use pid_plot::read::PlotInk;
use pid_plot::write::{PlotFont, write_plot};

/// The eleven acceptance sheets, in the gate's order.
pub const SHEETS: [&str; 11] = [
    "I井场 注采阀组工艺及自控流程图",
    "J K井场 注采阀组工艺及自控流程图",
    "L M井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
    "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
];

/// Anchor-point miss past which the gate counts a paired label
/// (`LABEL_POSITION_MM`), millimetres.
pub const LABEL_POSITION_MM: f64 = 0.15;
/// Run-width difference past which the gate counts a paired label
/// (`LABEL_WIDTH_MM`), millimetres.
pub const LABEL_WIDTH_MM: f64 = 0.05;

/// The corpus: `PID_ACCEPTANCE_CORPUS`, else the gate's own default.
pub fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// A sheet's name cut down to what tells it from the other ten.
pub fn short(name: &str) -> String {
    name.trim_start_matches("集注站 ")
        .trim_end_matches("工艺及自控流程图")
        .trim_end_matches(" 注采阀组")
        .to_string()
}

/// One acceptance sheet: its model, the vendor's plot and the plot this crate
/// writes for it, both as bytes so a probe can read them more than one way.
pub struct Page {
    /// The sheet's file stem.
    pub name: &'static str,
    /// The page as laid out against the corpus.
    pub model: PaperModel,
    /// The vendor's reference plot.
    pub reference: Vec<u8>,
    /// The plot written from `model`, lettered in the faces prepared for all
    /// the sheets loaded together — the gate's batch.
    pub ours: Vec<u8>,
}

/// Load the sheets whose names contain any of `wanted` (all eleven when
/// `wanted` is empty), written in faces prepared for all eleven together as
/// the gate prepares them.
///
/// # Errors
///
/// A message naming the file or the step that failed.
pub fn pages(wanted: &[String]) -> Result<Vec<Page>, String> {
    let corpus = corpus_dir();
    let mut models: Vec<PaperModel> = Vec::new();
    for name in SHEETS {
        let path = corpus.join(format!("{name}.pid"));
        models
            .push(lay_out_drawing(&path, &corpus).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    let mut dirs = font_dirs(Some(&corpus));
    dirs.push(PathBuf::from("D:/work/cad/PID自研软件验证/fonts"));
    let prepared = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs)
        .map_err(|e| format!("fonts: {e}"))?;
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let mut out = Vec::new();
    for (name, model) in SHEETS.into_iter().zip(models) {
        if !wanted.is_empty() && !wanted.iter().any(|needle| name.contains(needle.as_str())) {
            continue;
        }
        let reference_path = corpus.join(format!("{name}.pdf"));
        let reference = std::fs::read(&reference_path)
            .map_err(|e| format!("{}: {e}", reference_path.display()))?;
        let ours = write_plot(&model, &fonts).map_err(|e| format!("{name}: write: {e}"))?;
        out.push(Page {
            name,
            model,
            reference,
            ours,
        });
    }
    Ok(out)
}

/// Write `model` in faces prepared for it alone.
///
/// # Errors
///
/// A message naming the step that failed.
pub fn write_alone(model: &PaperModel) -> Result<Vec<u8>, String> {
    let corpus = corpus_dir();
    let mut dirs = font_dirs(Some(&corpus));
    dirs.push(PathBuf::from("D:/work/cad/PID自研软件验证/fonts"));
    let prepared = prepare_fonts(&[model], &dirs).map_err(|e| format!("fonts: {e}"))?;
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    write_plot(model, &fonts).map_err(|e| format!("write: {e}"))
}

/// The page's missing-symbol boxes as the gate hands them to the comparison.
pub fn missing_masks(model: &PaperModel) -> Vec<MaskRect> {
    model
        .missing_boxes
        .iter()
        .map(|missing| MaskRect {
            label: missing.path.clone(),
            min_mm: missing.min_mm,
            max_mm: missing.max_mm,
        })
        .collect()
}

/// Four rectangles that together mask everything on the page outside
/// `window`, so a comparison run with them reads only what lies inside it.
///
/// The comparison leaves out whatever a mask contains and has no notion of a
/// window; masking the complement is how the gate's own rules are asked about
/// one rectangle. The far edges reach well past the page so ink the fit
/// places a hair outside it is masked as well.
pub fn outside(window: &MaskRect) -> Vec<MaskRect> {
    const FAR: f64 = 1.0e6;
    // Just outside the window, so ink lying on its edge stays inside it.
    const HAIR: f64 = 1.0e-9;
    let [x0, y0] = window.min_mm;
    let [x1, y1] = window.max_mm;
    let rect = |min_mm: [f64; 2], max_mm: [f64; 2]| MaskRect {
        label: format!("outside {}", window.label),
        min_mm,
        max_mm,
    };
    vec![
        rect([-FAR, -FAR], [x0 - HAIR, FAR]),
        rect([x1 + HAIR, -FAR], [FAR, FAR]),
        rect([x0 - HAIR, -FAR], [x1 + HAIR, y0 - HAIR]),
        rect([x0 - HAIR, y1 + HAIR], [x1 + HAIR, FAR]),
    ]
}

/// Reference ink a comparison left uncovered, millimetres.
pub fn uncovered_mm(report: &CompareReport) -> f64 {
    report.reference_length_mm - report.covered_length_mm
}

/// The paired labels of `report` past the gate's two label limits, one line
/// each, counted the way the gate's `labels_off` counts them: a label's
/// anchor point is where its paper-table source says it hangs from, and a
/// pair with no source is measured from its start.
pub fn labels_off(report: &CompareReport, reference: &PlotInk) -> Vec<String> {
    report
        .text_pairs
        .iter()
        .filter_map(|pair| {
            let fraction = match pair.source.as_ref().map(|source| source.anchor) {
                Some(TextAnchor::Center) => 0.5,
                Some(TextAnchor::Right) => 1.0,
                Some(TextAnchor::Left) | None => 0.0,
            };
            let position_mm = (pair.along_mm + fraction * pair.advance_mm).hypot(pair.down_mm);
            if position_mm <= LABEL_POSITION_MM && pair.advance_mm.abs() <= LABEL_WIDTH_MM {
                return None;
            }
            let runs: Vec<&str> = pair
                .reference_runs
                .iter()
                .map(|index| reference.spans[*index].text.as_str())
                .collect();
            let at = reference.spans[pair.reference_runs[0]].origin_mm;
            Some(format!(
                "{} {runs:?} at ({:.1}, {:.1}): {position_mm:.4} mm off, {:+.4} mm wide",
                pair.class, at[0], at[1], pair.advance_mm
            ))
        })
        .collect()
}

/// The paper table's name for a primitive: its symbol file, else the border
/// template for furniture, else its stream.
pub fn name_of(primitive: &PaperPrimitive, model: &PaperModel) -> String {
    if let Some(symbol) = &primitive.symbol {
        return symbol
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(symbol)
            .to_string();
    }
    if primitive.furniture {
        let template = model.template.as_ref().map_or("template", |template| {
            template
                .path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&template.path)
        });
        return format!("{template} (furniture)");
    }
    primitive
        .stream_path
        .clone()
        .unwrap_or_else(|| "sheet".to_string())
}

/// Every polyline a primitive strokes, paper millimetres. Lettering strokes
/// nothing here.
pub fn strokes_of(shape: &PaperShape) -> Vec<Vec<[f64; 2]>> {
    const STEPS: usize = 48;
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
            vec![
                (0..=STEPS)
                    .map(|step| {
                        let angle = start_angle + span * step as f64 / STEPS as f64;
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
        } => vec![
            (0..=STEPS)
                .map(|step| {
                    let angle = std::f64::consts::TAU * step as f64 / STEPS as f64;
                    [
                        center_mm[0] + radius_mm * angle.cos(),
                        center_mm[1] + radius_mm * angle.sin(),
                    ]
                })
                .collect(),
        ],
        PaperShape::Text { .. } => Vec::new(),
    }
}

/// Distance from `point` to the segment `start`–`end`.
pub fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length2 = dx * dx + dy * dy;
    let (x, y) = if length2 <= 1e-18 {
        (start[0], start[1])
    } else {
        let t =
            (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length2).clamp(0.0, 1.0);
        (start[0] + dx * t, start[1] + dy * t)
    };
    (point[0] - x).hypot(point[1] - y)
}

/// Segments bucketed on a grid, for "is there one within `radius` of here".
pub struct Near {
    cell: f64,
    segments: Vec<([f64; 2], [f64; 2])>,
    buckets: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl Near {
    /// Index `segments`.
    pub fn new(segments: Vec<([f64; 2], [f64; 2])>) -> Self {
        let cell = 2.0;
        let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        for (index, (start, end)) in segments.iter().enumerate() {
            let length = (end[0] - start[0]).hypot(end[1] - start[1]);
            let steps = ((length / cell).ceil() as usize).max(1);
            let mut seen: Vec<(i64, i64)> = Vec::new();
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let key = (
                    ((start[0] + (end[0] - start[0]) * t) / cell).floor() as i64,
                    ((start[1] + (end[1] - start[1]) * t) / cell).floor() as i64,
                );
                if !seen.contains(&key) {
                    seen.push(key);
                    buckets.entry(key).or_default().push(index);
                }
            }
        }
        Self {
            cell,
            segments,
            buckets,
        }
    }

    /// The nearest indexed segment within `radius` of `point`, with its
    /// distance.
    pub fn nearest(&self, point: [f64; 2], radius: f64) -> Option<(usize, f64)> {
        let reach = (radius / self.cell).ceil() as i64 + 1;
        let key = (
            (point[0] / self.cell).floor() as i64,
            (point[1] / self.cell).floor() as i64,
        );
        let mut best: Option<(usize, f64)> = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let Some(bucket) = self.buckets.get(&(key.0 + dx, key.1 + dy)) else {
                    continue;
                };
                for index in bucket {
                    let (start, end) = self.segments[*index];
                    let distance = dist_point_seg(point, start, end);
                    if distance <= radius && best.is_none_or(|(_, so_far)| distance < so_far) {
                        best = Some((*index, distance));
                    }
                }
            }
        }
        best
    }

    /// Whether any indexed segment lies within `radius` of `point`.
    pub fn within(&self, point: [f64; 2], radius: f64) -> bool {
        self.nearest(point, radius).is_some()
    }
}
