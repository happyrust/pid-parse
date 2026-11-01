//! Does our pen agree with the plotter's, stroke by stroke, on the sheets that
//! have a reference?
//!
//! The comparison that gate-keeps a page ([`pid_plot::compare`]) measures
//! geometry: where ink lies, how much of the reference it covers, whether the
//! text pairs. It also paired the two pages' widths — [`CompareReport::pen`] —
//! because a page can cover the reference exactly while every stroke carried
//! the wrong pen; this probe prints that section across all five drawings at
//! once, and adds the one measurement the report does not carry.
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
use std::path::PathBuf;

use pid_plot::compare::template_masks;
use pid_plot::compare::{CompareReport, Fit, compare_plots};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

/// The five drawings whose own plot sits beside them. Each is one project's
/// sheet, and together they are the only pages a pen can be tested against.
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
    tally: &mut BTreeMap<(u64, u64, bool), usize>,
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
        let is_symbol = primitive.symbol.is_some();
        *tally.entry((declared, printed, is_symbol)).or_default() += 1;
    }
}

fn main() {
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));

    let mut agreed = 0usize;
    let mut disagreed = 0usize;
    let mut unpaired = 0usize;
    let mut overall: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    let mut by_declared: BTreeMap<(u64, u64, bool), usize> = BTreeMap::new();
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
    for ((declared, printed, is_symbol), count) in &by_declared {
        println!(
            "   declared {:>5.2} mm  prints {:>5.2} mm  {} {count:>6}",
            *declared as f64 / 100.0,
            *printed as f64 / 100.0,
            if *is_symbol {
                "symbol body"
            } else {
                "drawing ink"
            }
        );
    }
}
