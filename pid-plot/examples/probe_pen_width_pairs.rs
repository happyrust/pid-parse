//! Does our pen agree with the plotter's, stroke by stroke, on the sheets that
//! have a reference?
//!
//! The comparison that gate-keeps a page ([`pid_plot::compare`]) measures
//! geometry: where ink lies, how much of the reference it covers, whether the
//! text pairs. It also paired the two pages' widths — [`CompareReport::pen`] —
//! because a page can cover the reference exactly while every stroke carried
//! the wrong pen; this probe prints that section for every drawing in the
//! corpus that has a plot beside it, and adds the one measurement the report
//! does not carry.
//!
//! The tally is how the table's boundaries are measured: the declared widths
//! the reference pages do show are the only widths any boundary can be tested
//! against, and a declared width that no reference page shows is a width the
//! table's bands have never been checked on.
//!
//! ```text
//! cargo run -p pid-plot --example probe_pen_width_pairs
//! ```
//!
//! Read-only: both PDFs are read into memory. Our page is written to memory
//! from the model, never to disk.
//!
//! The corpus is `PID_PARSE_SELF_CHECK_DIR`, or `D:\work\cad\PID自研软件验证`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::compare::template_masks;
use pid_plot::compare::{CompareReport, Fit, compare_plots};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn hundredths(width: f64) -> u64 {
    (width * 100.0).round().max(0.0) as u64
}

/// Every top-level `.pid` that has a plot of the same name beside it.
///
/// The directory is flat: the five original sheets and the eleven acceptance
/// sheets both sit next to their plots, so a recursive walk would only pick
/// up copies filed under `plot/`.
fn paired_sheets(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "pid")
                && path.with_extension("pdf").is_file()
        })
        .collect();
    paths.sort();
    paths
}

/// Where a straight stroke's declaration comes from.
///
/// Furniture wins over a symbol name: a template symbol is furniture, and the
/// widths this probe exists to settle live on the border, not on the drawing.
fn ink_kind(furniture: bool, symbol: bool) -> &'static str {
    if furniture {
        "furniture"
    } else if symbol {
        "symbol body"
    } else {
        "drawing ink"
    }
}

/// Which pen the reference prints at the same place as each **declared**
/// width, straight-line strokes only.
///
/// The report's `pen` section says whether two pages agree; this says which
/// declaration they agree *about*, because a straight stroke's midpoint and
/// direction name it exactly and the model still knows the width it declared.
/// Arcs, circles and polylines are left out: their flattening differs between
/// the two pages, and a mismatch there would be an artefact of the sampling
/// rather than of the pen.
fn declared_pairs(
    model: &PaperModel,
    reference: &PlotInk,
    fit: &Fit,
    tally: &mut BTreeMap<(u64, u64, &'static str), usize>,
) {
    for primitive in &model.primitives {
        let PaperShape::Line { start_mm, end_mm } = &primitive.shape else {
            continue;
        };
        let declared = hundredths(primitive.stroke.as_ref().map_or(0.25, |s| s.width_mm));
        let back = |point: [f64; 2]| {
            [
                (point[0] - fit.translation_mm[0]) / fit.scale,
                (point[1] - fit.translation_mm[1]) / fit.scale,
            ]
        };
        let (a, b) = (back(*start_mm), back(*end_mm));
        let direction = [b[0] - a[0], b[1] - a[1]];
        let len = direction[0].hypot(direction[1]);
        if len < 0.5 {
            continue;
        }
        let unit = [direction[0] / len, direction[1] / len];
        let midpoint = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let mut widths_here: Vec<u64> = Vec::new();
        for candidate in &reference.segments {
            let candidate_mid = [
                (candidate.start_mm[0] + candidate.end_mm[0]) / 2.0,
                (candidate.start_mm[1] + candidate.end_mm[1]) / 2.0,
            ];
            if (candidate_mid[0] - midpoint[0]).hypot(candidate_mid[1] - midpoint[1]) > 0.3 {
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
            if dot < 0.995 {
                continue;
            }
            let ratio = (other_len / len).min(len / other_len);
            if ratio < 0.75 {
                continue;
            }
            widths_here.push(hundredths(candidate.width_mm));
        }
        // Only a locus where the reference states one width is evidence: a
        // filled region's outline sits on its own stroke, and picking between
        // them would report the pen the page did not use.
        widths_here.sort_unstable();
        widths_here.dedup();
        let [printed] = widths_here[..] else {
            continue;
        };
        let kind = ink_kind(primitive.furniture, primitive.symbol.is_some());
        *tally.entry((declared, printed, kind)).or_default() += 1;
    }
}

fn main() {
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));

    let mut agreed = 0usize;
    let mut disagreed = 0usize;
    let mut unpaired = 0usize;
    let mut overall: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    let mut by_declared: BTreeMap<(u64, u64, &'static str), usize> = BTreeMap::new();
    let sheets = paired_sheets(&corpus);
    if sheets.is_empty() {
        eprintln!("{}: no .pid with a plot beside it", corpus.display());
        std::process::exit(2);
    }
    for path in &sheets {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
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
        let pdf = write_plot(&model, &fonts).expect("write");
        let ours = read_plot(&pdf).expect("read ours");
        let masks = template_masks(reference.page_mm);
        let report: CompareReport = compare_plots(&reference, &ours, &masks);
        declared_pairs(&model, &reference, &report.fit, &mut by_declared);
        let pen = &report.pen;
        for pair in &pen.pairs {
            *overall
                .entry((pair.ours_centi_mm, pair.reference_centi_mm))
                .or_default() += pair.strokes;
        }
        agreed += pen.agreed;
        disagreed += pen.differed;
        unpaired += pen.unpaired;
        println!(
            "{name}\n   {} segments paired; {} agree with the reference's width, {} differ; \
             {} with no partner",
            pen.paired, pen.agreed, pen.differed, pen.unpaired
        );
        for pair in &pen.pairs {
            println!(
                "      ours {:>5.2} mm  reference {:>5.2} mm  {:>5} strokes{}",
                pair.ours_mm(),
                pair.reference_mm(),
                pair.strokes,
                if pair.agrees() { "" } else { "  <- differs" }
            );
        }
    }
    println!("\nPEN TABLE ON THE REFERENCE PAGES");
    for ((our_width, reference_width), count) in &overall {
        println!(
            "   ours {:>5.2} mm  reference {:>5.2} mm  {count:>6}",
            *our_width as f64 / 100.0,
            *reference_width as f64 / 100.0
        );
    }
    println!("\n{agreed} strokes agree, {disagreed} differ, {unpaired} unpaired");
    println!("\nBY DECLARED WIDTH (straight strokes, reference width at the same place)");
    for ((declared, printed, kind), count) in &by_declared {
        println!(
            "   declared {:>5.2} mm  prints {:>5.2} mm  {kind:<12} {count:>6}",
            *declared as f64 / 100.0,
            *printed as f64 / 100.0,
        );
    }
}
