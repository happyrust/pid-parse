//! Export every `.pid` given and report what each one laid out and wrote, so a
//! drawing that cannot be plotted is named rather than silently skipped.
//!
//! ```text
//! cargo run -p pid-plot --example probe_export_audit -- <a.pid> [b.pid ...]
//! ```
//!
//! Per drawing: page size, primitive census, lettering that carries control
//! characters (a multi-line label the writer never splits), missing symbols,
//! then the PDF actually written and read back. Exit code 1 when any drawing
//! fails at any stage.
//!
//! Read-only: the PDF is kept in memory.

use std::path::PathBuf;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn census(model: &PaperModel) -> (usize, usize, usize, usize, usize, usize) {
    let mut counts = (0, 0, 0, 0, 0, 0);
    for primitive in &model.primitives {
        match &primitive.shape {
            PaperShape::Line { .. } => counts.0 += 1,
            PaperShape::Arc { .. } => counts.1 += 1,
            PaperShape::Circle { .. } => counts.2 += 1,
            PaperShape::Polyline { .. } => counts.3 += 1,
            PaperShape::Fill { .. } => counts.4 += 1,
            PaperShape::Text { .. } => counts.5 += 1,
        }
    }
    counts
}

/// Labels whose record states a run partition, and how many of those really
/// letter in more than one size or face.
///
/// The partition is the record's own `igTextBox` runs (`SymbolLettering::spans`),
/// carried onto the page as `PaperShape::Text::spans`. A label counts as
/// **lettering differently** when one of those runs states a size or a face the
/// shape's own single style does not — the case the writer cuts the label for;
/// runs that agree with it (per-character style ids that resolve to the same
/// size and face, which is most of the library's partitions) are drawn as one
/// run and change nothing.
fn partitioned_texts(model: &PaperModel) -> (usize, usize) {
    let mut partitioned = 0usize;
    let mut differing = 0usize;
    for primitive in &model.primitives {
        let PaperShape::Text {
            height_mm,
            font,
            spans,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        if spans.is_empty() {
            continue;
        }
        partitioned += 1;
        let own_height = height_mm.unwrap_or_default();
        let own_font = font.as_deref().unwrap_or_default().to_lowercase();
        let changes = spans.iter().any(|span| {
            let height_differs = span
                .height_mm
                .is_some_and(|height| (height - own_height).abs() > 0.01);
            let font_differs = span
                .font
                .as_deref()
                .is_some_and(|face| face.to_lowercase() != own_font);
            height_differs || font_differs
        });
        differing += usize::from(changes);
    }
    (partitioned, differing)
}

fn control_texts(model: &PaperModel) -> Vec<String> {
    model
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text { value, .. } if value.chars().any(char::is_control) => {
                Some(value.clone())
            }
            _ => None,
        })
        .collect()
}

/// Bounding box of every point a primitive defines, in paper millimetres.
fn ink_extent(model: &PaperModel) -> Option<([f64; 2], [f64; 2])> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    let mut seen = false;
    let mut grow = |point: [f64; 2]| {
        for axis in 0..2 {
            if point[axis].is_finite() {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
                seen = true;
            }
        }
    };
    for primitive in &model.primitives {
        match &primitive.shape {
            PaperShape::Line { start_mm, end_mm } => {
                grow(*start_mm);
                grow(*end_mm);
            }
            PaperShape::Arc {
                center_mm,
                radius_mm,
                ..
            } => {
                grow([center_mm[0] - radius_mm, center_mm[1] - radius_mm]);
                grow([center_mm[0] + radius_mm, center_mm[1] + radius_mm]);
            }
            PaperShape::Circle {
                center_mm,
                radius_mm,
            } => {
                grow([center_mm[0] - radius_mm, center_mm[1] - radius_mm]);
                grow([center_mm[0] + radius_mm, center_mm[1] + radius_mm]);
            }
            PaperShape::Polyline { points_mm, .. } => {
                for point in points_mm {
                    grow(*point);
                }
            }
            PaperShape::Fill { chain_mm, .. } => {
                for point in chain_mm {
                    grow(*point);
                }
            }
            PaperShape::Text { origin_mm, .. } => grow(*origin_mm),
        }
    }
    seen.then_some((min, max))
}

fn main() {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        eprintln!("usage: probe_export_audit <a.pid> [b.pid ...]");
        std::process::exit(2);
    }
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    let mut failed = 0usize;
    let mut rows = Vec::new();
    for path in &paths {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let model = match lay_out_drawing(path, &corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("FAIL {name}: layout: {error}");
                failed += 1;
                continue;
            }
        };
        let (lines, arcs, circles, polys, fills, texts) = census(&model);
        let inferred = model
            .primitives
            .iter()
            .filter(|primitive| primitive.confidence == "inferred")
            .count();
        let controls = control_texts(&model);
        let (partitioned, differing) = partitioned_texts(&model);
        let prepared = match prepare_fonts(&[&model], &dirs) {
            Ok(prepared) => prepared,
            Err(error) => {
                println!("FAIL {name}: fonts: {error}");
                failed += 1;
                continue;
            }
        };
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
            })
            .collect();
        let pdf = match write_plot(&model, &fonts) {
            Ok(pdf) => pdf,
            Err(error) => {
                println!(
                    "FAIL {name}: write: {error} ({} lettering runs carry a control character)",
                    controls.len()
                );
                failed += 1;
                continue;
            }
        };
        let ink = match read_plot(&pdf) {
            Ok(ink) => ink,
            Err(error) => {
                println!("FAIL {name}: read back: {error}");
                failed += 1;
                continue;
            }
        };
        rows.push((
            name,
            model,
            (lines, arcs, circles, polys, fills, texts),
            controls.len(),
            (partitioned, differing),
            inferred,
            ink.segments.len(),
            ink.spans.len(),
            pdf.len(),
        ));
    }
    for (
        name,
        model,
        counts,
        controls,
        (partitioned, differing),
        inferred,
        segments,
        spans,
        bytes,
    ) in &rows
    {
        // The page the PDF got, and whether the drawing stated it: four
        // drawings in the package state none and are measured instead, which
        // is a different fact from a page the `.pid` carries.
        let page = match model.resolved_page_mm() {
            Some(page) => format!(
                "{:.2}x{:.2}{}",
                page[0],
                page[1],
                if model.page_mm.is_some() {
                    ""
                } else {
                    " (measured)"
                }
            ),
            None => "none".to_string(),
        };
        println!(
            "PASS {name}: {page} mm  line {} arc {} circle {} poly {} fill {} text {}  \
             controls {controls} off-sheet {} endpoint-pairs {} inferred {inferred}  \
             partitions {partitioned} ({differing} letter differently)  \
             read {segments} segments  {spans} spans  {bytes} bytes  \
             symbols {}/{} missing {} boxes {}",
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            counts.4,
            counts.5,
            model.off_sheet_primitives,
            model.skipped_endpoint_pairs,
            model.symbols_found,
            model.symbols_referenced,
            model.missing_symbols.len(),
            model.missing_boxes.len()
        );
    }
    for (name, model, ..) in &rows {
        for missing in &model.missing_symbols {
            println!("     {name}: no library file for {missing}");
        }
        if let (Some(page), Some((min, max))) = (model.resolved_page_mm(), ink_extent(model)) {
            // A micrometre of slack: circle and arc extents are `center ±
            // radius`, which lands within 3e-5 mm of the page edge on almost
            // every sheet and would otherwise bury the one real overshoot
            // (a vertical label 1.55 mm below the page).
            const SLACK_MM: f64 = 1.0e-3;
            let outside = min[0] < -SLACK_MM
                || min[1] < -SLACK_MM
                || max[0] > page[0] + SLACK_MM
                || max[1] > page[1] + SLACK_MM;
            if outside {
                println!("     {name}: ink {min:?}..{max:?} runs past the {page:?} page");
            }
        }
    }
    println!("\n{} drawings, {} failed", paths.len(), failed);
    if failed > 0 {
        std::process::exit(1);
    }
}
