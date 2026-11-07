//! Why does each of our strokes that pairs with no reference stroke have no
//! partner?
//!
//! [`CompareReport::pen`] pairs our strokes with the reference's at the same
//! place and lists every stroke it could not pair, with how the reference's
//! ink lies around it ([`UnpairedReason`]) and whether the border template
//! drew it. This probe compares every drawing of the acceptance corpus the
//! way `reference_plots_acceptance` does, prints that accounting page by page,
//! and names the model primitive behind each unpaired stroke — the symbol or
//! stream, the shape, and its pen — so a count in the gate can be traced to
//! the ink it counts.
//!
//! ```text
//! cargo run -p pid-plot --example probe_why_a_stroke_has_no_pen_partner -- [name-filter ...]
//! ```
//!
//! With no filter it walks all eleven sheets. The corpus is
//! `PID_ACCEPTANCE_CORPUS`, or `D:\work\cad\pid-test-data\验收数据`; fonts come
//! from `PID_PLOT_FONTS` and the self-check corpus's own set. Read-only: both
//! plots are read into memory, ours is written from the model and never to
//! disk.
//!
//! The evidence behind
//! `docs/analysis/2026-11-07-every-unpaired-pen-stroke-has-a-reason.md`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use pid_plot::compare::{
    CompareReport, Fit, MaskRect, UnpairedReason, UnpairedStroke, compare_plots_with_model,
    template_masks,
};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, stroked_segments, write_plot};

/// Most strokes listed one by one in a category; a larger one is summarised
/// by owner.
const LISTED: usize = 24;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// Every `.pid` in the corpus with a plot of the same name beside it.
fn sheets(corpus: &Path, filters: &[String]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(corpus) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "pid")
                && path.with_extension("pdf").is_file()
                && path.file_stem().is_some_and(|stem| {
                    let stem = stem.to_string_lossy();
                    filters.is_empty()
                        || filters.iter().any(|filter| stem.contains(filter.as_str()))
                })
        })
        .collect();
    paths.sort();
    paths
}

fn hypot(start: [f64; 2], end: [f64; 2]) -> f64 {
    (start[0] - end[0]).hypot(start[1] - end[1])
}

fn into_ours(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        fit.scale * point[0] + fit.translation_mm[0],
        fit.scale * point[1] + fit.translation_mm[1],
    ]
}

/// Each segment the writer strokes, with the primitive it belongs to.
///
/// `stroked_segments` is asked about one primitive at a time, so the pieces a
/// dash or an arc's chain leaves are attributed to the shape that drew them.
fn attributed_segments(model: &PaperModel) -> Vec<([f64; 2], [f64; 2], usize)> {
    let mut single = model.clone();
    single.template = None;
    let mut out = Vec::new();
    for (index, primitive) in model.primitives.iter().enumerate() {
        if matches!(primitive.shape, PaperShape::Text { .. }) {
            continue;
        }
        single.primitives = vec![primitive.clone()];
        for (start, end) in stroked_segments(&single) {
            out.push((start, end, index));
        }
    }
    out
}

/// The primitive whose stroked segment is this stroke, ends within 0.05 mm.
fn owner(
    segments: &[([f64; 2], [f64; 2], usize)],
    grid: &HashMap<(i64, i64), Vec<usize>>,
    start: [f64; 2],
    end: [f64; 2],
) -> Option<usize> {
    let middle = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
    let key = (
        (middle[0] / 0.5).floor() as i64,
        (middle[1] / 0.5).floor() as i64,
    );
    let mut best: Option<(f64, usize)> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            for &slot in grid.get(&(key.0 + dx, key.1 + dy)).into_iter().flatten() {
                let (a, b, primitive) = segments[slot];
                let error = (hypot(a, start) + hypot(b, end)).min(hypot(a, end) + hypot(b, start));
                if error < 0.05 && best.is_none_or(|(least, _)| error < least) {
                    best = Some((error, primitive));
                }
            }
        }
    }
    best.map(|(_, primitive)| primitive)
}

fn describe(primitive: &PaperPrimitive) -> String {
    let kind = match primitive.shape {
        PaperShape::Line { .. } => "line",
        PaperShape::Polyline { .. } => "polyline",
        PaperShape::Arc { .. } => "arc",
        PaperShape::Circle { .. } => "circle",
        PaperShape::Fill { .. } => "fill",
        PaperShape::Text { .. } => "text",
    };
    let name = primitive
        .symbol
        .as_deref()
        .map(|symbol| symbol.rsplit(['\\', '/']).next().unwrap_or(symbol))
        .or(primitive.stream_path.as_deref())
        .unwrap_or("(sheet)");
    let pen = primitive.stroke.as_ref().map_or_else(
        || "default pen".to_string(),
        |stroke| {
            if stroke.dash_mm.is_empty() {
                format!("{:.2} mm", stroke.width_mm)
            } else {
                let dash: Vec<String> = stroke
                    .dash_mm
                    .iter()
                    .map(|step| format!("{step:.3}"))
                    .collect();
                format!(
                    "{:.2} mm dash {}{}",
                    stroke.width_mm,
                    dash.join("/"),
                    if stroke.dash_fit.is_some() {
                        " fitted"
                    } else {
                        ""
                    }
                )
            }
        },
    );
    format!("{kind} {name} ({pen})")
}

/// The gate's category for one unpaired stroke.
fn category(stroke: &UnpairedStroke) -> &'static str {
    match (stroke.reason, stroke.furniture) {
        (UnpairedReason::Bare, true) => "template the reference does not print",
        (UnpairedReason::Bare, false) => "bare: ink the reference does not print",
        (UnpairedReason::PartlyBare, _) => "partly bare",
        (UnpairedReason::Beside, _) => "beside the reference's ink",
    }
}

fn report_page(name: &str, model: &PaperModel, report: &CompareReport) {
    let pen = &report.pen;
    println!(
        "{name}\n  pen: paired {} (agreed {}, differed {}), unpaired {}, short {}",
        pen.paired, pen.agreed, pen.differed, pen.unpaired, pen.short
    );
    let segments = attributed_segments(model);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (slot, (start, end, _)) in segments.iter().enumerate() {
        let middle = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
        grid.entry((
            (middle[0] / 0.5).floor() as i64,
            (middle[1] / 0.5).floor() as i64,
        ))
        .or_default()
        .push(slot);
    }
    let mut by_category: BTreeMap<&'static str, Vec<(&UnpairedStroke, String)>> = BTreeMap::new();
    for stroke in &pen.unpaired_strokes {
        let start = into_ours(stroke.start_mm, &report.fit);
        let end = into_ours(stroke.end_mm, &report.fit);
        let drawn_by = owner(&segments, &grid, start, end).map_or_else(
            || "(no model stroke within 0.05 mm)".to_string(),
            |index| describe(&model.primitives[index]),
        );
        let drawn_by = if stroke.furniture {
            format!("[template] {drawn_by}")
        } else {
            drawn_by
        };
        by_category
            .entry(category(stroke))
            .or_default()
            .push((stroke, drawn_by));
    }
    let in_missing = pen
        .unpaired_strokes
        .iter()
        .filter(|stroke| stroke.in_missing_box)
        .count();
    println!("  in a missing-symbol box: {in_missing}");
    for (label, strokes) in &by_category {
        let length: f64 = strokes.iter().map(|(stroke, _)| stroke.length_mm()).sum();
        println!("  {label}: {} strokes, {length:.3} mm", strokes.len());
        if strokes.len() > LISTED {
            let mut owners: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
            for (stroke, drawn_by) in strokes {
                let entry = owners.entry(drawn_by.as_str()).or_default();
                entry.0 += 1;
                entry.1 += stroke.length_mm();
            }
            for (drawn_by, (count, length)) in owners {
                println!("      {count:>5}  {length:>9.3} mm  {drawn_by}");
            }
            continue;
        }
        for (stroke, drawn_by) in strokes {
            println!(
                "      w {:.2}  {:>7.3} mm  ({:.3}, {:.3}) -> ({:.3}, {:.3})  {drawn_by}",
                stroke.width_centi_mm as f64 / 100.0,
                stroke.length_mm(),
                stroke.start_mm[0],
                stroke.start_mm[1],
                stroke.end_mm[0],
                stroke.end_mm[1],
            );
        }
    }
}

fn main() {
    let filters: Vec<String> = std::env::args().skip(1).collect();
    let corpus = corpus_dir();
    let paths = sheets(&corpus, &filters);
    if paths.is_empty() {
        eprintln!("{}: no .pid with a plot beside it", corpus.display());
        std::process::exit(2);
    }
    let mut dirs = font_dirs(Some(&corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    for path in &paths {
        let name = path
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
        let reference = read_plot(&std::fs::read(path.with_extension("pdf")).expect("reference"))
            .expect("read reference");
        let model = lay_out_drawing(path, &corpus).expect("layout");
        let prepared = prepare_fonts(&[&model], &dirs).expect("fonts");
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let ours = read_plot(&write_plot(&model, &fonts).expect("write")).expect("read ours");
        let template = template_masks(reference.page_mm);
        let missing: Vec<MaskRect> = model
            .missing_boxes
            .iter()
            .map(|missing| MaskRect {
                label: missing.path.clone(),
                min_mm: missing.min_mm,
                max_mm: missing.max_mm,
            })
            .collect();
        let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
        report_page(&name, &model, &report);
    }
}
