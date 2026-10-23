//! Look for shapes a page table should not carry: the same stroke twice,
//! degenerate geometry, coordinates the writer silently drops, labels stacked
//! on each other, run partitions that do not cover their text.
//!
//! ```text
//! cargo run -p pid-plot --example probe_page_defects -- <a.pid|dir> [more ...]
//! ```
//!
//! Read-only: layout only, no PDF is written.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use pid_plot::model::{
    PaperModel, PaperPrimitive, PaperShape, PaperStroke, PaperTextSpan, lay_out_drawing,
};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn finite_point(point: &[f64; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

/// Whether the shape is one the writer draws nothing for.
fn degenerate(shape: &PaperShape) -> bool {
    match shape {
        PaperShape::Line { start_mm, end_mm } => {
            (start_mm[0] - end_mm[0]).hypot(start_mm[1] - end_mm[1]) < 1e-6
        }
        PaperShape::Arc { radius_mm, .. } | PaperShape::Circle { radius_mm, .. } => {
            !radius_mm.is_finite() || *radius_mm <= 1e-6
        }
        PaperShape::Polyline { points_mm, .. } => points_mm.len() < 2,
        PaperShape::Fill { chain_mm, .. } => chain_mm.len() < 3,
        PaperShape::Text { .. } => false,
    }
}

/// Whether the writer refuses this shape for a reason it never reports: a
/// non-finite coordinate makes `show_text` and the path builder skip it.
fn not_finite(shape: &PaperShape) -> bool {
    match shape {
        PaperShape::Line { start_mm, end_mm } => !finite_point(start_mm) || !finite_point(end_mm),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            start_angle,
            end_angle,
        } => {
            !finite_point(center_mm)
                || !radius_mm.is_finite()
                || !start_angle.is_finite()
                || !end_angle.is_finite()
        }
        PaperShape::Circle {
            center_mm,
            radius_mm,
        } => !finite_point(center_mm) || !radius_mm.is_finite(),
        PaperShape::Polyline { points_mm, .. } => {
            points_mm.iter().any(|point| !finite_point(point))
        }
        PaperShape::Fill { chain_mm, .. } => chain_mm.iter().any(|point| !finite_point(point)),
        PaperShape::Text {
            origin_mm,
            height_mm,
            rotation,
            ..
        } => {
            !finite_point(origin_mm)
                || !rotation.is_finite()
                || height_mm.is_some_and(|height| !height.is_finite())
        }
    }
}

/// A key with the provenance stripped: two records that state the same shape
/// with the same pen are the same ink on the page.
fn shape_key(primitive: &PaperPrimitive) -> String {
    serde_json::to_string(&(&primitive.stroke, &primitive.shape)).unwrap_or_default()
}

fn kind_name(shape: &PaperShape) -> &'static str {
    match shape {
        PaperShape::Line { .. } => "line",
        PaperShape::Arc { .. } => "arc",
        PaperShape::Circle { .. } => "circle",
        PaperShape::Polyline { .. } => "polyline",
        PaperShape::Fill { .. } => "fill",
        PaperShape::Text { .. } => "text",
    }
}

/// Span problems a page table can state on its own.
fn span_problems(value: &str, spans: &[PaperTextSpan]) -> Vec<String> {
    let mut out = Vec::new();
    let chars = value.chars().count();
    if spans.is_empty() {
        return out;
    }
    let mut covered = vec![0u8; chars];
    let total: usize = spans.iter().map(|span| usize::from(span.len)).sum();
    if total != chars {
        out.push(format!("spans cover {total} of {chars} chars"));
    }
    for span in spans {
        let start = usize::from(span.start);
        let end = start.saturating_add(usize::from(span.len));
        if end > chars {
            out.push(format!("span {start}..{end} runs past {chars}"));
            continue;
        }
        for slot in &mut covered[start..end] {
            *slot += 1;
        }
        if span.len == 0 {
            out.push(format!("empty span at {start}"));
        }
    }
    if covered.contains(&0) {
        let missing = covered.iter().filter(|count| **count == 0).count();
        out.push(format!("{missing} chars in no span"));
    }
    if covered.iter().any(|count| *count > 1) {
        let doubled = covered.iter().filter(|count| **count > 1).count();
        out.push(format!("{doubled} chars in two spans"));
    }
    out
}

struct Report {
    name: String,
    page: Option<[f64; 2]>,
    primitives: usize,
    duplicates: usize,
    duplicate_samples: Vec<String>,
    degenerate: usize,
    degenerate_samples: Vec<String>,
    not_finite: usize,
    texts: usize,
    empty_texts: usize,
    no_height: usize,
    stacked_texts: usize,
    stacked_samples: Vec<String>,
    span_issues: Vec<String>,
    widths: BTreeMap<String, usize>,
    dashes: usize,
    dashed_duplicates: usize,
    wide_samples: Vec<String>,
    dash_samples: Vec<String>,
    missing_symbols: usize,
    height_min: Option<f64>,
    height_max: Option<f64>,
    odd_heights: Vec<String>,
    fonts: BTreeMap<String, usize>,
    ink_margins: Option<[f64; 4]>,
    outside: Vec<String>,
}

/// Left/bottom/right/top margins from the ink to the declared page edges.
fn ink_margins(model: &PaperModel) -> Option<[f64; 4]> {
    let [width, height] = model.page_mm?;
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    let mut grow = |point: [f64; 2]| {
        if point[0].is_finite() && point[1].is_finite() {
            min[0] = min[0].min(point[0]);
            min[1] = min[1].min(point[1]);
            max[0] = max[0].max(point[0]);
            max[1] = max[1].max(point[1]);
        }
    };
    for primitive in &model.primitives {
        for point in shape_points(&primitive.shape) {
            grow(point);
        }
    }
    if min[0] > max[0] {
        return None;
    }
    Some([min[0], min[1], width - max[0], height - max[1]])
}

fn shape_points(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        }
        | PaperShape::Circle {
            center_mm,
            radius_mm,
        } => vec![
            [center_mm[0] - radius_mm, center_mm[1] - radius_mm],
            [center_mm[0] + radius_mm, center_mm[1] + radius_mm],
        ],
        PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
}

fn audit(name: String, model: &PaperModel) -> Report {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut duplicate_samples = Vec::new();
    let mut duplicates = 0usize;
    let mut degenerate_count = 0usize;
    let mut degenerate_samples = Vec::new();
    let mut non_finite = 0usize;
    let mut texts = 0usize;
    let mut empty_texts = 0usize;
    let mut no_height = 0usize;
    let mut stacked: HashMap<String, Vec<String>> = HashMap::new();
    let mut span_issues = Vec::new();
    let mut widths: BTreeMap<String, usize> = BTreeMap::new();
    let mut dashes = 0usize;
    let mut patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut dashed_duplicates = 0usize;
    let mut wide_samples = Vec::new();
    let mut height_min: Option<f64> = None;
    let mut height_max: Option<f64> = None;
    let mut odd_heights = Vec::new();
    let mut fonts: BTreeMap<String, usize> = BTreeMap::new();
    let mut outside = Vec::new();
    for primitive in &model.primitives {
        let key = shape_key(primitive);
        let count = seen.entry(key).or_default();
        *count += 1;
        if *count > 1 {
            duplicates += 1;
            if primitive
                .stroke
                .as_ref()
                .is_some_and(|stroke| !stroke.dash_mm.is_empty())
            {
                dashed_duplicates += 1;
            }
            if duplicate_samples.len() < 4 {
                duplicate_samples.push(format!(
                    "{} ({}x{})",
                    kind_name(&primitive.shape),
                    primitive.symbol.as_deref().unwrap_or("-"),
                    primitive
                        .stream_path
                        .as_deref()
                        .map_or("-".to_string(), |path| path
                            .rsplit('/')
                            .next()
                            .unwrap_or(path)
                            .to_string())
                ));
            }
        }
        if degenerate(&primitive.shape) {
            degenerate_count += 1;
            if degenerate_samples.len() < 4 {
                degenerate_samples.push(kind_name(&primitive.shape).to_string());
            }
        }
        if not_finite(&primitive.shape) {
            non_finite += 1;
        }
        match &primitive.stroke {
            Some(PaperStroke {
                width_mm, dash_mm, ..
            }) => {
                *widths.entry(format!("{width_mm:.3}")).or_default() += 1;
                if !dash_mm.is_empty() {
                    dashes += 1;
                    let pattern: Vec<String> =
                        dash_mm.iter().map(|step| format!("{step:.2}")).collect();
                    *patterns.entry(pattern.join("/")).or_default() += 1;
                }
                if *width_mm >= 1.0 && wide_samples.len() < 6 {
                    wide_samples.push(format!(
                        "{width_mm:.3} mm {} {}\n        {:?}",
                        kind_name(&primitive.shape),
                        primitive.symbol.as_deref().unwrap_or("-"),
                        primitive.shape
                    ));
                }
            }
            None => *widths.entry("default".into()).or_default() += 1,
        }
        if let PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            font,
            spans,
            ..
        } = &primitive.shape
        {
            texts += 1;
            *fonts
                .entry(font.clone().unwrap_or_else(|| "-".into()))
                .or_default() += 1;
            if let Some(height) = height_mm {
                height_min = Some(height_min.map_or(*height, |min| min.min(*height)));
                height_max = Some(height_max.map_or(*height, |max| max.max(*height)));
                if (*height < 0.5 || *height > 12.0) && odd_heights.len() < 4 {
                    odd_heights.push(format!("{height:.3} mm {:?}", value.trim()));
                }
            }
            for span in spans {
                if let Some(height) = span.height_mm {
                    height_min = Some(height_min.map_or(height, |min| min.min(height)));
                    height_max = Some(height_max.map_or(height, |max| max.max(height)));
                    if (height < 0.5 || height > 12.0) && odd_heights.len() < 4 {
                        odd_heights.push(format!("span {height:.3} mm {:?}", value.trim()));
                    }
                }
            }
            if value.trim().is_empty() {
                empty_texts += 1;
            }
            if height_mm.is_none() {
                no_height += 1;
            }
            let origin_key = format!("{:.2},{:.2}|{}", origin_mm[0], origin_mm[1], value.trim());
            if !value.trim().is_empty() {
                stacked.entry(origin_key).or_default().push(format!(
                    "{} {:?} {:?}",
                    primitive.symbol.as_deref().unwrap_or("-"),
                    primitive
                        .stream_path
                        .as_deref()
                        .map_or("-".to_string(), |path| path
                            .rsplit('/')
                            .next()
                            .unwrap_or(path)
                            .to_string()),
                    primitive.byte_range
                ));
            }
            for problem in span_problems(value, spans) {
                if span_issues.len() < 6 {
                    span_issues.push(format!("{:?}: {problem}", value.trim()));
                }
            }
        }
    }
    if let Some([width, height]) = model.page_mm {
        for primitive in &model.primitives {
            let points = shape_points(&primitive.shape);
            let over = points.iter().any(|point| {
                point[0].is_finite()
                    && point[1].is_finite()
                    && (point[0] < -0.5
                        || point[1] < -0.5
                        || point[0] > width + 0.5
                        || point[1] > height + 0.5)
            });
            if over && outside.len() < 6 {
                outside.push(format!(
                    "{} {} {} {:?}",
                    kind_name(&primitive.shape),
                    primitive.symbol.as_deref().unwrap_or("-"),
                    primitive
                        .stream_path
                        .as_deref()
                        .map_or("-".to_string(), |path| path
                            .rsplit('/')
                            .next()
                            .unwrap_or(path)
                            .to_string()),
                    primitive.shape
                ));
            }
        }
    }
    let mut stacked_texts = 0usize;
    let mut stacked_samples = Vec::new();
    for (key, origins) in &stacked {
        if origins.len() > 1 {
            stacked_texts += origins.len();
            if stacked_samples.len() < 4 {
                stacked_samples.push(format!(
                    "{key} x{} [{}]",
                    origins.len(),
                    origins.join(" | ")
                ));
            }
        }
    }
    Report {
        name,
        page: model.page_mm,
        primitives: model.primitives.len(),
        duplicates,
        duplicate_samples,
        degenerate: degenerate_count,
        degenerate_samples,
        not_finite: non_finite,
        texts,
        empty_texts,
        no_height,
        stacked_texts,
        stacked_samples,
        span_issues,
        dash_samples: patterns
            .iter()
            .map(|(pattern, count)| format!("{pattern} x{count}"))
            .collect(),
        dashed_duplicates,
        wide_samples,
        widths,
        dashes,
        missing_symbols: model.missing_symbols.len(),
        height_min,
        height_max,
        odd_heights,
        fonts,
        ink_margins: ink_margins(model),
        outside,
    }
}

fn main() {
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            collect_pids(&path, &mut paths);
        } else {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        eprintln!("usage: probe_page_defects <a.pid|dir> [more ...]");
        std::process::exit(2);
    }
    let corpus = corpus_dir();
    let mut reports = Vec::new();
    let mut failed = 0usize;
    for path in &paths {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        match lay_out_drawing(path, &corpus) {
            Ok(model) => reports.push(audit(name, &model)),
            Err(error) => {
                println!("FAIL {name}: {error}");
                failed += 1;
            }
        }
    }
    let mut totals = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for report in &reports {
        totals.0 += report.primitives;
        totals.1 += report.duplicates;
        totals.2 += report.degenerate;
        totals.3 += report.not_finite;
        totals.4 += report.texts;
        totals.5 += report.empty_texts;
        totals.6 += report.stacked_texts;
        let page = report.page.map_or("none".to_string(), |page| {
            format!("{:.2}x{:.2}", page[0], page[1])
        });
        let pens: Vec<String> = report
            .widths
            .iter()
            .map(|(width, count)| format!("{width}:{count}"))
            .collect();
        let flagged = report.duplicates > 0
            || report.degenerate > 0
            || report.not_finite > 0
            || report.empty_texts > 0
            || report.stacked_texts > 0
            || !report.span_issues.is_empty();
        let heights = match (report.height_min, report.height_max) {
            (Some(min), Some(max)) => format!("{min:.3}..{max:.3}"),
            _ => "none".to_string(),
        };
        println!(
            "{} {}: {page} mm  {} prims  {} text  h {heights}  dup {} degen {} nonfinite {} \
             empty-text {} stacked {} no-height {}  dashes {}  pens [{}]  missing {}",
            if flagged { "FLAG" } else { "PASS" },
            report.name,
            report.primitives,
            report.texts,
            report.duplicates,
            report.degenerate,
            report.not_finite,
            report.empty_texts,
            report.stacked_texts,
            report.no_height,
            report.dashes,
            pens.join(" "),
            report.missing_symbols
        );
        for sample in &report.outside {
            println!("      outside: {sample}");
        }
        if let Some(margins) = report.ink_margins {
            let closest = margins.iter().copied().fold(f64::MAX, f64::min);
            if closest > 60.0 {
                println!("      margins: {margins:?} — the frame may be missing");
            }
        }
        let font_list: Vec<String> = report
            .fonts
            .iter()
            .map(|(font, count)| format!("{font}:{count}"))
            .collect();
        println!("      fonts: {}", font_list.join(" "));
        for sample in &report.duplicate_samples {
            println!("      dup: {sample}");
        }
        for sample in &report.degenerate_samples {
            println!("      degen: {sample}");
        }
        for sample in &report.stacked_samples {
            println!("      stacked: {sample}");
        }
        for sample in &report.wide_samples {
            println!("      wide: {sample}");
        }
        if report.dashes > 0 {
            println!(
                "      dashes: {} ({} of them duplicated); patterns [{}]",
                report.dashes,
                report.dashed_duplicates,
                report.dash_samples.join(", ")
            );
        }
        for issue in &report.span_issues {
            println!("      spans: {issue}");
        }
        for sample in &report.odd_heights {
            println!("      height: {sample}");
        }
    }
    println!(
        "\n{} drawings, {} failed: {} prims, {} duplicates, {} degenerate, {} non-finite, \
         {} texts ({} empty, {} stacked)",
        reports.len(),
        failed,
        totals.0,
        totals.1,
        totals.2,
        totals.3,
        totals.4,
        totals.5,
        totals.6
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
