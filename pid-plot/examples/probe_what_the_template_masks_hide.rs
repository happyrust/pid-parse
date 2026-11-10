//! What do the acceptance gate's template masks hide?
//!
//! `reference_plots_acceptance` compares each of the eleven `验收数据` sheets
//! twice: with the surveyed furniture rectangles masked
//! (`assets/template-masks.json` — the sign-off column and the title block)
//! and whole. "Ten pages read coverage 1.000000" is the masked view. This
//! prints, for every sheet, both views side by side and then each mask
//! rectangle on its own, read by the gate's own comparison with everything
//! outside the rectangle masked instead:
//!
//! - reference ink, uncovered reference ink and extra ink inside it;
//! - the labels inside it — how many the reference letters, how many pair,
//!   which do not, and which pair past the gate's two label limits
//!   (0.15 mm of anchor-point miss, 0.05 mm of width);
//! - how many of the paired labels are the border template's own and how many
//!   are the drawing's;
//! - our strokes inside it the pen pairing leaves unpaired, by reason.
//!
//! Every number is `compare_plots_with_model`'s: a rectangle is asked about by
//! masking its complement, so no rule is repeated here.
//!
//! The comparison reads paint, not ink: a path stroked in white counts like a
//! black one (`read::Painted::All`). What these readings look like when only
//! ink is read is `probe_which_paint_leaves_no_ink`'s question.
//!
//! ```text
//! cargo run -p pid-plot --example probe_what_the_template_masks_hide -- [<sheet name part> ...]
//! ```
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::process::ExitCode;

use pid_plot::compare::{
    CompareReport, MaskRect, UnpairedReason, compare_plots_with_model, template_masks,
};
use pid_plot::model::{PaperModel, PaperShape};
use pid_plot::read::{PlotInk, read_plot};

use acceptance::{labels_off, missing_masks, outside, pages, short, uncovered_mm};

/// One view's row of the side-by-side table.
fn row(view: &str, report: &CompareReport, off: usize) -> String {
    format!(
        "   {view:<7} coverage {:.6}  uncovered {:>9.3} mm  extra {:>9.3} mm  labels {:>4}/{:<4} \
         past limits {:>2}  pen {:>5} agree / {} differ / {:>3} unpaired",
        report.coverage,
        uncovered_mm(report),
        report.extra_length_mm,
        report.text_matched,
        report.text_reference,
        off,
        report.pen.agreed,
        report.pen.differed,
        report.pen.unpaired,
    )
}

/// The insertion points of the labels the border template letters.
fn furniture_label_origins(model: &PaperModel) -> Vec<[f64; 2]> {
    model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text { origin_mm, .. } => Some(*origin_mm),
            _ => None,
        })
        .collect()
}

/// Print what one mask rectangle holds.
fn print_mask(
    mask: &MaskRect,
    reference: &PlotInk,
    ours: &PlotInk,
    model: &PaperModel,
    whole: &CompareReport,
) {
    let mut masks = outside(mask);
    masks.extend(missing_masks(model));
    let inside = compare_plots_with_model(reference, ours, &masks, &[], model);
    let off = labels_off(&inside, reference);
    let furniture = furniture_label_origins(model);
    let (mut template_pairs, mut drawing_pairs, mut unsourced) = (0usize, 0usize, 0usize);
    for pair in &inside.text_pairs {
        match &pair.source {
            Some(source) => {
                if furniture.iter().any(|origin| {
                    (origin[0] - source.origin_mm[0]).abs() < 1e-6
                        && (origin[1] - source.origin_mm[1]).abs() < 1e-6
                }) {
                    template_pairs += 1;
                } else {
                    drawing_pairs += 1;
                }
            }
            None => unsourced += 1,
        }
    }
    println!(
        "   mask {:?} x {:.3}..{:.3} y {:.3}..{:.3}",
        mask.label, mask.min_mm[0], mask.max_mm[0], mask.min_mm[1], mask.max_mm[1]
    );
    println!(
        "      ink: reference {:.3} mm, uncovered {:.3} mm, extra {:.3} mm",
        inside.reference_length_mm,
        uncovered_mm(&inside),
        inside.extra_length_mm
    );
    for group in &inside.uncovered_reference {
        println!(
            "         uncovered near {:<40} {:>9.3} mm",
            group.label, group.length_mm
        );
    }
    for group in &inside.extra_outside_by_symbol {
        println!(
            "         extra from     {:<40} {:>9.3} mm",
            group.label, group.length_mm
        );
    }
    println!(
        "      labels: reference {} / ours {} / paired {} ({} the template's, {} the drawing's, {} with \
         no paper-table label); {} past the label limits",
        inside.text_reference,
        inside.text_ours,
        inside.text_matched,
        template_pairs,
        drawing_pairs,
        unsourced,
        off.len()
    );
    for text in &inside.unmatched_reference_text {
        println!(
            "         reference only: {:?} at ({:.2}, {:.2})",
            text.text, text.origin_mm[0], text.origin_mm[1]
        );
    }
    for text in &inside.unmatched_our_text {
        println!(
            "         ours only:      {:?} at ({:.2}, {:.2})",
            text.text, text.origin_mm[0], text.origin_mm[1]
        );
    }
    for line in &off {
        println!("         past limits:    {line}");
    }
    // The pen pairing is not masked, so the whole page's unpaired strokes are
    // sorted into this rectangle by their midpoint.
    let here = |reason: UnpairedReason, furniture: bool| {
        whole
            .pen
            .unpaired_strokes
            .iter()
            .filter(|stroke| stroke.reason == reason && stroke.furniture == furniture)
            .filter(|stroke| {
                mask.contains([
                    (stroke.start_mm[0] + stroke.end_mm[0]) / 2.0,
                    (stroke.start_mm[1] + stroke.end_mm[1]) / 2.0,
                ])
            })
            .fold((0usize, 0.0f64), |(count, length), stroke| {
                (count + 1, length + stroke.length_mm())
            })
    };
    let (bare_template, bare_template_mm) = here(UnpairedReason::Bare, true);
    let (bare, bare_mm) = here(UnpairedReason::Bare, false);
    let (partly, partly_mm) = {
        let a = here(UnpairedReason::PartlyBare, true);
        let b = here(UnpairedReason::PartlyBare, false);
        (a.0 + b.0, a.1 + b.1)
    };
    let (beside, beside_mm) = {
        let a = here(UnpairedReason::Beside, true);
        let b = here(UnpairedReason::Beside, false);
        (a.0 + b.0, a.1 + b.1)
    };
    println!(
        "      unpaired strokes of ours: template with no reference ink {bare_template} \
         ({bare_template_mm:.3} mm), other bare {bare} ({bare_mm:.3} mm), partly bare {partly} \
         ({partly_mm:.3} mm), beside {beside} ({beside_mm:.3} mm)"
    );
}

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let mut totals = [0.0f64; 4];
    for page in &pages {
        let (Ok(reference), Ok(ours)) = (read_plot(&page.reference), read_plot(&page.ours)) else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        let masks = template_masks(reference.page_mm);
        let missing = missing_masks(&page.model);
        let masked = compare_plots_with_model(&reference, &ours, &masks, &missing, &page.model);
        let whole = compare_plots_with_model(&reference, &ours, &[], &missing, &page.model);
        println!(
            "== {} — page {:.0} × {:.0} mm, template {}, {} mask(s), {} missing-symbol box(es), \
             library gaps {:?}",
            short(page.name),
            reference.page_mm[0],
            reference.page_mm[1],
            page.model.template.as_ref().map_or_else(
                || "none".to_string(),
                |template| template
                    .path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or_default()
                    .to_string()
            ),
            masks.len(),
            missing.len(),
            page.model.missing_symbols,
        );
        println!(
            "{}",
            row("masked", &masked, labels_off(&masked, &reference).len())
        );
        println!(
            "{}",
            row("whole", &whole, labels_off(&whole, &reference).len())
        );
        println!(
            "   the masks hold: reference ink {:.3} mm, {} reference label(s) and {} of ours; whole \
             − masked: uncovered {:+.3} mm, extra {:+.3} mm, unpaired labels {:+}",
            masked.masked_reference_length_mm,
            whole.text_reference - masked.text_reference,
            whole.text_ours - masked.text_ours,
            uncovered_mm(&whole) - uncovered_mm(&masked),
            whole.extra_length_mm - masked.extra_length_mm,
            (whole.text_reference - whole.text_matched) as i64
                - (masked.text_reference - masked.text_matched) as i64,
        );
        totals[0] += uncovered_mm(&whole) - uncovered_mm(&masked);
        totals[1] += whole.extra_length_mm - masked.extra_length_mm;
        totals[2] += labels_off(&whole, &reference).len() as f64;
        totals[3] += labels_off(&masked, &reference).len() as f64;
        for mask in &masks {
            print_mask(mask, &reference, &ours, &page.model, &whole);
        }
    }
    println!(
        "{} sheet(s): the masks hide {:.3} mm of uncovered reference ink and {:.3} mm of extra ink; \
         labels past the limits: {} whole, {} masked",
        pages.len(),
        totals[0],
        totals[1],
        totals[2],
        totals[3]
    );
    ExitCode::SUCCESS
}
