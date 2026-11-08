//! Why do A2's two `仪表风管线` labels sit 0.198 mm higher on the reference
//! than the plotter model puts them?
//!
//! The census flags exactly two pairs on A2 and files both under
//! `unexplained`: the run's anchor point is where the model puts it
//! horizontally, the run is the reference's width to 0.0002 mm, and only the
//! baseline is off — ours 0.198 mm low. A2 is the one acceptance sheet whose
//! page is not a standard sheet size, so the device pixel grid the other ten
//! sheets are lettered on does not apply to it. This probe asks what does:
//!
//! * every `0x48` `仿宋` label on A2, with its stored value, its box width,
//!   its paragraph and the reference's own baseline drop against the model's;
//! * whether the reference's run origins on A2 land on the 1/150 in grid the
//!   other ten sheets' do (they should not);
//! * how far the two `仪表风管线` sit from the neighbours that share their
//!   face, anchor code and paragraph, so a rule has to explain the whole
//!   family and not just the two flagged labels.
//!
//! ```text
//! cargo run -p pid-plot --example probe_a2_device_grid
//! ```
//!
//! Read-only. The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`.
//!
//! Verdict vocabulary: `on-grid` — A2's run origins are on the same device
//! grid as the standard sheets, so a grid rule could move them; `off-grid` —
//! they are not, so no grid rule applies; `family-wide` — the offset is shared
//! by every label of the family; `two-labels` — only the flagged pair differs.

use std::path::PathBuf;
use std::process::ExitCode;

#[path = "support/glyphs.rs"]
mod glyphs;

use glyphs::{PageGlyphs, Run, read_glyphs};
use pid_plot::compare::{MaskRect, compare_plots_with_model};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot};
use pid_plot::write::{PlotFont, write_plot};

/// The sheet this probe is about.
const SHEET: &str = "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图";
/// The label the census flags.
const LABEL: &str = "仪表风管线";
/// The anchor code the flagged labels carry.
const CODE: u8 = 0x48;
/// The face those labels name.
const FACE: &str = "仿宋";

/// One laid-out label of the family, with both sides' readings.
struct Row {
    value: String,
    paragraph: String,
    line: String,
    height_mm: f64,
    /// Where the model puts the baseline below the insertion point, mm.
    model_drop_mm: f64,
    /// Where the reference's first inked glyph sits below it, mm.
    reference_drop_mm: f64,
    /// Ours, mm.
    our_drop_mm: f64,
    /// The reference's first glyph origin, paper millimetres.
    reference_origin_mm: [f64; 2],
    /// The record's stored box, millimetres: `(width, height)`, `None` when
    /// the record carries no box.
    box_mm: Option<(f64, f64)>,
}

/// One `igTextBox` record's own fields, read off the parsed `.pid`.
#[derive(Clone, Copy)]
struct Stored {
    /// The record's anchor code.
    anchor_code: u8,
}

/// Every `0x48` `仿宋` record of the sheet, off the parsed `.pid`: anchor
/// code from the geometry pass, box from the raw `igTextBox` bytes. The box
/// matters because the vertical drop is the only reading the census flags
/// here, and a boxed record's box is the one stored quantity the layout does
/// not use vertically yet.
fn codes(path: &std::path::Path) -> std::collections::HashMap<(String, usize), Stored> {
    let mut out = std::collections::HashMap::new();
    let Ok(doc) = pid_parse::PidParser::new().parse_file(path) else {
        return out;
    };
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::Text { anchor_code, .. } = &entity.kind else {
            continue;
        };
        let (Some(stream), Some(range)) = (&entity.source.stream_path, &entity.source.byte_range)
        else {
            continue;
        };
        out.insert(
            (stream.clone(), range.start),
            Stored {
                anchor_code: *anchor_code,
            },
        );
    }
    out
}

/// The stored box of every `igTextBox` on the sheet, keyed by its value in
/// millimetres. A record with no box is left out. Two records that spell the
/// same value carry the same box on this sheet, so a per-value map is enough.
fn boxes(path: &std::path::Path) -> std::collections::HashMap<String, (f64, f64)> {
    let mut out = std::collections::HashMap::new();
    let Ok(bytes) = std::fs::read(path) else {
        return out;
    };
    for record in pid_parse::parsers::sheet_records::decode_igtextboxes(&bytes) {
        if record.text_box_width_m <= 0.0 {
            continue;
        }
        out.insert(
            record.text.trim().to_string(),
            (
                record.text_box_width_m * 1000.0,
                record.text_box_height_m * 1000.0,
            ),
        );
    }
    out
}

/// A glyph's baseline drop below `insertion`, millimetres, along the page's
/// own y axis (down positive).
fn drop_below(page: &PageGlyphs, ink: &PlotInk, run: usize, insertion: [f64; 2]) -> Option<f64> {
    let reading = Run::of(page, ink, run);
    let first = reading
        .glyphs
        .iter()
        .find(|glyph| !glyph.text.trim().is_empty())?;
    Some(insertion[1] - first.origin_mm[1])
}

fn main() -> ExitCode {
    let corpus = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    let pid = corpus.join(format!("{SHEET}.pid"));
    if !pid.is_file() {
        eprintln!("{} is not there", pid.display());
        return ExitCode::from(2);
    }
    let Ok(model) = lay_out_drawing(&pid, &corpus) else {
        eprintln!("{SHEET}: could not lay the sheet out");
        return ExitCode::from(1);
    };
    let mut dirs = font_dirs(Some(&corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    let Ok(prepared) = prepare_fonts(&[&model], &dirs) else {
        eprintln!("no faces to letter with");
        return ExitCode::from(1);
    };
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let Ok(reference_bytes) = std::fs::read(corpus.join(format!("{SHEET}.pdf"))) else {
        eprintln!("{SHEET}: no reference plot");
        return ExitCode::from(1);
    };
    let Ok(our_bytes) = write_plot(&model, &fonts) else {
        eprintln!("{SHEET}: could not write the plot");
        return ExitCode::from(1);
    };
    let (Ok(reference), Ok(ours), Some(reference_glyphs), Some(our_glyphs)) = (
        read_plot(&reference_bytes),
        read_plot(&our_bytes),
        read_glyphs(&reference_bytes),
        read_glyphs(&our_bytes),
    ) else {
        eprintln!("{SHEET}: a plot could not be read back");
        return ExitCode::from(1);
    };
    println!(
        "page: reference {:.3} × {:.3} mm, ours {:.3} × {:.3} mm",
        reference.page_mm[0], reference.page_mm[1], ours.page_mm[0], ours.page_mm[1],
    );
    let reference_page = PageGlyphs::new(reference_glyphs);
    let our_page = PageGlyphs::new(our_glyphs);
    let anchor_codes = codes(&pid);
    let stored_boxes = boxes(&pid);
    // A2's page is not a standard sheet: the reference printed a 594 × 420 mm
    // sheet from a `.pid` that states 594.328 × 420.566 mm, so the two pages'
    // millimetres are not the same millimetres. Every comparison below maps
    // our point into the reference's frame with the same fit the acceptance
    // gate solves, or the page-size difference shows up as a phantom vertical
    // offset of up to 0.57 mm at the top of the sheet.
    let missing: Vec<MaskRect> = model
        .missing_boxes
        .iter()
        .map(|missing| MaskRect {
            label: missing.path.clone(),
            min_mm: missing.min_mm,
            max_mm: missing.max_mm,
        })
        .collect();
    let report = compare_plots_with_model(&reference, &ours, &[], &missing, &model);
    let fit = &report.fit;
    let into_reference = |point: [f64; 2]| {
        [
            (point[0] - fit.translation_mm[0]) / fit.scale,
            (point[1] - fit.translation_mm[1]) / fit.scale,
        ]
    };
    println!(
        "fit: scale {:.7}, translation ({:+.4}, {:+.4}) mm, within threshold {}",
        fit.scale, fit.translation_mm[0], fit.translation_mm[1], fit.within_threshold,
    );
    let mut rows: Vec<Row> = Vec::new();
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm,
            value,
            anchor,
            line,
            font,
            height_mm,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        if font.as_deref() != Some(FACE) {
            continue;
        }
        let code = primitive
            .byte_range
            .and_then(|range| anchor_codes.get(&(primitive.stream_path.clone()?, range[0])))
            .copied();
        if code.map(|stored| stored.anchor_code) != Some(CODE) {
            continue;
        }
        let Some(height_mm) = *height_mm else {
            continue;
        };
        let Some(reference_run) = nearest(&reference, &reference_page, value, *origin_mm, fit)
        else {
            continue;
        };
        let Some(our_run) = nearest(&ours, &our_page, value, *origin_mm, fit) else {
            continue;
        };
        let (Some(reference_drop_mm), Some(our_drop_mm)) = (
            drop_below(
                &reference_page,
                &reference,
                reference_run,
                into_reference(*origin_mm),
            ),
            drop_below(&our_page, &ours, our_run, *origin_mm),
        ) else {
            continue;
        };
        rows.push(Row {
            value: value.clone(),
            paragraph: format!("{anchor:?}"),
            line: format!("{line:?}"),
            height_mm,
            // A2 has no device grid, so ours is the model's own drop here.
            model_drop_mm: our_drop_mm,
            reference_drop_mm,
            our_drop_mm,
            reference_origin_mm: {
                let reading = Run::of(&reference_page, &reference, reference_run);
                reading
                    .glyphs
                    .iter()
                    .find(|glyph| !glyph.text.trim().is_empty())
                    .map_or([f64::NAN; 2], |glyph| glyph.origin_mm)
            },
            box_mm: stored_boxes.get(value.trim()).copied(),
        });
    }
    if rows.is_empty() {
        eprintln!("no {CODE:#04x} {FACE} labels on {SHEET}");
        return ExitCode::from(1);
    }
    println!(
        "{:<14} {:<8} {:<6} {:>6} {:>9} {:>9} {:>9} {:>9}  {:<13} grid",
        "value",
        "anchor",
        "line",
        "height",
        "model",
        "reference",
        "ours",
        "ref-model",
        "box w×h mm"
    );
    for row in &rows {
        let grid = on_grid(row.reference_origin_mm);
        println!(
            "{:<14} {:<8} {:<6} {:>6.3} {:>+9.4} {:>+9.4} {:>+9.4} {:>+9.4}  {:<13} {}",
            row.value,
            row.paragraph,
            row.line,
            row.height_mm,
            row.model_drop_mm,
            row.reference_drop_mm,
            row.our_drop_mm,
            row.reference_drop_mm - row.model_drop_mm,
            row.box_mm.map_or_else(
                || "none".to_string(),
                |(width, height)| format!("{width:.3}×{height:.3}")
            ),
            if grid { "on" } else { "off" },
        );
    }
    println!();
    // The grid question, over every inked run on the page.
    let on_grid_runs = reference
        .spans
        .iter()
        .filter(|span| !span.text.trim().is_empty())
        .filter(|span| on_grid(span.origin_mm))
        .count();
    let inked = reference
        .spans
        .iter()
        .filter(|span| !span.text.trim().is_empty())
        .count();
    println!("reference run origins on the 1/150 in grid: {on_grid_runs}/{inked}");
    // The family: does every label share the reference's offset?
    let offsets: Vec<f64> = rows
        .iter()
        .map(|row| row.reference_drop_mm - row.model_drop_mm)
        .collect();
    let flagged: Vec<&Row> = rows.iter().filter(|row| row.value == LABEL).collect();
    let mean = offsets.iter().sum::<f64>() / offsets.len() as f64;
    let spread = (offsets
        .iter()
        .map(|offset| (offset - mean).powi(2))
        .sum::<f64>()
        / offsets.len() as f64)
        .sqrt();
    println!(
        "reference minus model, over the {} labels of the family: mean {:+.4} mm, sd {:.4} mm, \
         range {:+.4} … {:+.4}",
        offsets.len(),
        mean,
        spread,
        offsets.iter().copied().fold(f64::INFINITY, f64::min),
        offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    );
    for row in &flagged {
        println!(
            "  flagged {:<12} height {:.3} mm: reference {:+.4}, model {:+.4}, offset {:+.4} mm \
             ({:+.3} of the body)",
            row.value,
            row.height_mm,
            row.reference_drop_mm,
            row.model_drop_mm,
            row.reference_drop_mm - row.model_drop_mm,
            (row.reference_drop_mm - row.model_drop_mm) / row.height_mm,
        );
    }
    println!();
    let grid_verdict = if on_grid_runs == inked {
        "on-grid"
    } else {
        "off-grid"
    };
    let family_verdict = if spread > 0.05 {
        "family-wide"
    } else {
        "two-labels"
    };
    println!(
        "verdict: {grid_verdict}, {family_verdict} — the flagged pair's offset is {:+.4} mm while \
         the family's other labels spread over {:+.4} … {:+.4} mm (sd {:.4}); every label carries \
         the same anchor code, face, height and paragraph, and the page's runs are {}on the \
         standard sheets' 1/150 in grid",
        flagged
            .first()
            .map_or(f64::NAN, |row| row.reference_drop_mm - row.model_drop_mm),
        offsets.iter().copied().fold(f64::INFINITY, f64::min),
        offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        spread,
        if on_grid_runs == inked { "" } else { "not " },
    );
    ExitCode::SUCCESS
}

/// Whether a point sits on the reference's 1/150 in device grid, counted from
/// the page's lower-left corner.
fn on_grid(point: [f64; 2]) -> bool {
    let step = 0.48 * 25.4 / 72.0;
    point
        .iter()
        .all(|mm| ((mm / step) - (mm / step).round()).abs() < 1e-3)
}

/// The run nearest `insertion` that spells `value`, `insertion` mapped into
/// the page's own frame by `fit`.
fn nearest(
    ink: &PlotInk,
    page: &PageGlyphs,
    value: &str,
    insertion: [f64; 2],
    fit: &pid_plot::compare::Fit,
) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (index, span) in ink.spans.iter().enumerate() {
        if span.text.trim() != value.trim() {
            continue;
        }
        let reading = Run::of(page, ink, index);
        let Some(first) = reading
            .glyphs
            .iter()
            .find(|glyph| !glyph.text.trim().is_empty())
        else {
            continue;
        };
        // `insertion` is ours; the reference's own frame is a different one.
        let at = [
            (insertion[0] - fit.translation_mm[0]) / fit.scale,
            (insertion[1] - fit.translation_mm[1]) / fit.scale,
        ];
        let distance = (first.origin_mm[0] - at[0]).hypot(first.origin_mm[1] - at[1]);
        if best.is_none_or(|(nearest, _)| distance < nearest) {
            best = Some((distance, index));
        }
    }
    best.map(|(_, index)| index)
}
