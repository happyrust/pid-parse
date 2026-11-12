//! Which paint on a plot leaves no ink, and what does the comparison read
//! once it is left out?
//!
//! `read::read_plot` reads every painted path as ink, whatever colour it is
//! painted in, and `reference_plots_acceptance` compares two such readings. A
//! path stroked in white on white paper prints nothing, so two pages can read
//! as the same ink and look different. This asks the eleven `验收数据` sheets
//! the questions that reading cannot:
//!
//! - how much each page paints in the paper's colour — the vendor's plot and
//!   the plot this crate writes for the same sheet;
//! - where the vendor paints paper and we print ink, and where we paint paper
//!   and the vendor prints ink, each attributed to the page primitive it lies
//!   on;
//! - what the gate's own comparison reads, masked and whole, when both pages
//!   are read as ink only (`read::Painted::Inked`).
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_paint_leaves_no_ink -- [<sheet name part> ...]
//! ```
//!
//! Exit code 1 when any sheet's ink-only reading differs from its paint
//! reading, which is the finding: the two are meant to be the same page.
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::collections::BTreeMap;
use std::process::ExitCode;

use pid_plot::compare::{CompareReport, Fit, compare_plots_with_model, template_masks};
use pid_plot::model::PaperModel;
use pid_plot::read::{InkSegment, Painted, PlotInk, read_plot_painted};

use acceptance::{Near, missing_masks, name_of, pages, short, strokes_of, uncovered_mm};

/// How close two pages' paint has to lie to be the same place, millimetres.
/// Both are the same drawing at the same size; the vendor snaps to a 1/150 in
/// grid (0.169 mm), so this is one grid step and a half.
const SAME_PLACE_MM: f64 = 0.25;
/// Sampling step along a stroke, millimetres.
const STEP_MM: f64 = 0.05;

fn length(segments: &[InkSegment]) -> f64 {
    segments
        .iter()
        .map(|segment| {
            (segment.end_mm[0] - segment.start_mm[0]).hypot(segment.end_mm[1] - segment.start_mm[1])
        })
        .sum()
}

/// Our paper millimetres, in the reference's page.
fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
}

fn pairs(segments: &[InkSegment]) -> Vec<([f64; 2], [f64; 2])> {
    segments
        .iter()
        .map(|segment| (segment.start_mm, segment.end_mm))
        .collect()
}

fn pairs_in_reference(segments: &[InkSegment], fit: &Fit) -> Vec<([f64; 2], [f64; 2])> {
    segments
        .iter()
        .map(|segment| {
            (
                into_reference(segment.start_mm, fit),
                into_reference(segment.end_mm, fit),
            )
        })
        .collect()
}

/// The page's own strokes in the reference's frame, each with its name.
fn named_strokes(model: &PaperModel, fit: &Fit) -> (Near, Vec<String>) {
    let mut segments = Vec::new();
    let mut names = Vec::new();
    for primitive in &model.primitives {
        let name = name_of(primitive, model);
        for points in strokes_of(&primitive.shape) {
            for pair in points.windows(2) {
                segments.push((into_reference(pair[0], fit), into_reference(pair[1], fit)));
                names.push(name.clone());
            }
        }
    }
    (Near::new(segments), names)
}

/// Walk `paper` — paint in the paper's colour, reference frame — and total
/// the length that `other_ink` inks at the same place, by the name of the
/// page primitive it lies on.
fn inked_by_the_other(
    paper: &[([f64; 2], [f64; 2])],
    other_ink: &Near,
    strokes: &Near,
    names: &[String],
) -> (f64, BTreeMap<String, (usize, f64)>) {
    let mut total = 0.0;
    let mut by_name: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for (start, end) in paper {
        let span = (end[0] - start[0]).hypot(end[1] - start[1]);
        if span < 1e-9 {
            continue;
        }
        let pieces = ((span / STEP_MM).round() as usize).max(1);
        let weight = span / pieces as f64;
        let mut hit = 0.0;
        let mut name: Option<String> = None;
        for piece in 0..pieces {
            let t = (piece as f64 + 0.5) / pieces as f64;
            let point = [
                start[0] + (end[0] - start[0]) * t,
                start[1] + (end[1] - start[1]) * t,
            ];
            if !other_ink.within(point, SAME_PLACE_MM) {
                continue;
            }
            hit += weight;
            if name.is_none() {
                name = strokes
                    .nearest(point, 1.0)
                    .map(|(index, _)| names[index].clone());
            }
        }
        if hit > 0.0 {
            total += hit;
            let entry = by_name
                .entry(name.unwrap_or_else(|| "no page primitive within 1 mm".to_string()))
                .or_default();
            entry.0 += 1;
            entry.1 += hit;
        }
    }
    (total, by_name)
}

fn row(view: &str, read: &str, report: &CompareReport) -> String {
    format!(
        "   {view:<6} {read:<5} coverage {:.6}  uncovered {:>9.3} mm  extra {:>9.3} mm  labels {:>4}/{:<4} \
         pen {:>5} agree / {} differ / {:>3} unpaired",
        report.coverage,
        uncovered_mm(report),
        report.extra_length_mm,
        report.text_matched,
        report.text_reference,
        report.pen.agreed,
        report.pen.differed,
        report.pen.unpaired,
    )
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
    let read = |bytes: &[u8], painted: Painted| -> Option<PlotInk> {
        read_plot_painted(bytes, painted).ok()
    };
    let mut differing = 0usize;
    let mut totals = [0.0f64; 6];
    for page in &pages {
        let (
            Some(ref_all),
            Some(ref_ink),
            Some(ref_paper),
            Some(our_all),
            Some(our_ink),
            Some(our_paper),
        ) = (
            read(&page.reference, Painted::All),
            read(&page.reference, Painted::Inked),
            read(&page.reference, Painted::Paper),
            read(&page.ours, Painted::All),
            read(&page.ours, Painted::Inked),
            read(&page.ours, Painted::Paper),
        )
        else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        let masks = template_masks(ref_all.page_mm);
        let missing = missing_masks(&page.model);
        let compare = |reference: &PlotInk, ours: &PlotInk, masked: bool| {
            compare_plots_with_model(
                reference,
                ours,
                if masked { &masks } else { &[] },
                &missing,
                &page.model,
            )
        };
        let whole_paint = compare(&ref_all, &our_all, false);
        let masked_paint = compare(&ref_all, &our_all, true);
        let whole_ink = compare(&ref_ink, &our_ink, false);
        let masked_ink = compare(&ref_ink, &our_ink, true);

        println!("== {}", short(page.name));
        println!(
            "   painted in the paper's colour: the reference {} piece(s) {:.3} mm of {:.3} mm stroked; \
             ours {} piece(s) {:.3} mm of {:.3} mm",
            ref_paper.segments.len(),
            length(&ref_paper.segments),
            length(&ref_all.segments),
            our_paper.segments.len(),
            length(&our_paper.segments),
            length(&our_all.segments),
        );
        println!("{}", row("masked", "paint", &masked_paint));
        println!("{}", row("masked", "ink", &masked_ink));
        println!("{}", row("whole", "paint", &whole_paint));
        println!("{}", row("whole", "ink", &whole_ink));
        for group in &whole_ink.uncovered_reference {
            println!(
                "      ink, whole: reference ink we do not ink, near {:<34} {:>9.3} mm",
                group.label, group.length_mm
            );
        }
        for group in &whole_ink.extra_outside_by_symbol {
            println!(
                "      ink, whole: our ink the reference does not ink, from {:<28} {:>9.3} mm",
                group.label, group.length_mm
            );
        }

        // Where one page paints paper and the other inks, in the frame of the
        // paint comparison — the fit both readings above share to 1e-4 mm.
        let fit = &whole_paint.fit;
        let (strokes, names) = named_strokes(&page.model, fit);
        let our_ink_near = Near::new(pairs_in_reference(&our_ink.segments, fit));
        let ref_ink_near = Near::new(pairs(&ref_ink.segments));
        let (we_ink, we_ink_by) =
            inked_by_the_other(&pairs(&ref_paper.segments), &our_ink_near, &strokes, &names);
        let (they_ink, they_ink_by) = inked_by_the_other(
            &pairs_in_reference(&our_paper.segments, fit),
            &ref_ink_near,
            &strokes,
            &names,
        );
        println!(
            "   the reference paints paper where we print ink: {we_ink:.3} mm of its {:.3} mm",
            length(&ref_paper.segments)
        );
        for (name, (count, mm)) in &we_ink_by {
            println!("      {name:<44} {count:>4} piece(s) {mm:>9.3} mm");
        }
        println!(
            "   we paint paper where the reference prints ink: {they_ink:.3} mm of our {:.3} mm",
            length(&our_paper.segments)
        );
        for (name, (count, mm)) in &they_ink_by {
            println!("      {name:<44} {count:>4} piece(s) {mm:>9.3} mm");
        }

        let same = |paint: &CompareReport, ink: &CompareReport| {
            (paint.coverage - ink.coverage).abs() < 5e-7
                && (uncovered_mm(paint) - uncovered_mm(ink)).abs() < 5e-4
                && (paint.extra_length_mm - ink.extra_length_mm).abs() < 5e-4
        };
        if !same(&whole_paint, &whole_ink) || !same(&masked_paint, &masked_ink) {
            differing += 1;
        }
        totals[0] += length(&ref_paper.segments);
        totals[1] += length(&our_paper.segments);
        totals[2] += we_ink;
        totals[3] += they_ink;
        totals[4] += uncovered_mm(&whole_ink) - uncovered_mm(&whole_paint);
        totals[5] += whole_ink.extra_length_mm - whole_paint.extra_length_mm;
    }
    println!(
        "{} sheet(s): the references paint {:.3} mm in the paper's colour and ours {:.3} mm; \
         we ink {:.3} mm the references paint paper, and paint paper over {:.3} mm they ink; \
         read as ink the whole pages gain {:+.3} mm uncovered and {:+.3} mm extra; \
         {} sheet(s) read differently as ink than as paint",
        pages.len(),
        totals[0],
        totals[1],
        totals[2],
        totals[3],
        totals[4],
        totals[5],
        differing
    );
    if differing > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
