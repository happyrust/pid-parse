//! Does a page read the same when the same two plots are compared again?
//!
//! `reference_plots_ink` pins every reading as an equality, so a reading
//! that moves with nothing changed is a gate that fails one run in three.
//! One did. The comparison's fit is not the same to the last bit from one
//! call to the next — on the A2 sheet its scale read
//! `0.999987214907646`, `…6409` and `…6533` in three calls on the same two
//! pages — and `ink::ends_apart` held a stroke to the reference strokes
//! within 45° of it: the two legs of that sheet's break mark cross the line
//! they break at exactly 45°, so each was along the line or across it on the
//! last bit of the fit, and the reading was 19107.863 mm in one run and
//! 19108.337 mm in the next. The limit is 37.5° now (`ink::ALONG_DOT`).
//!
//! This asks the general question. Each sheet is laid out and written once;
//! the same two PDFs are then read and compared `--rounds N` times (3 by
//! default), and every reading the gate pins is printed once per distinct
//! outcome. One outcome per sheet is the answer the gate needs.
//!
//! ```text
//! cargo run -p pid-plot --example probe_does_a_page_read_the_same_twice -- [--rounds N] [<name part> ...]
//! ```
//!
//! The eleven sheets at five rounds read one outcome each. A debug build
//! takes about twenty minutes for that; `--release` a tenth of it.
//!
//! Read-only on the corpus (`PID_ACCEPTANCE_CORPUS`). Exit code 1 when a
//! sheet reads two ways, 2 when the corpus cannot be read.

use std::collections::BTreeMap;
use std::process::ExitCode;

use pid_plot::compare::{compare_plots_with_model, template_masks};
use pid_plot::ink::{PageInk, ends_apart, label_colours, sheet_edges, stroke_colours};

#[path = "support/acceptance.rs"]
mod acceptance;

use acceptance::{missing_masks, pages, short, uncovered_mm};

fn main() -> ExitCode {
    let mut rounds = 3usize;
    let mut wanted: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--rounds" {
            match args.next().and_then(|value| value.parse::<usize>().ok()) {
                Some(value) if value >= 2 => rounds = value,
                _ => {
                    eprintln!("--rounds takes a number of at least 2");
                    return ExitCode::from(2);
                }
            }
        } else {
            wanted.push(arg);
        }
    }
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let mut unstable = 0usize;
    for page in &pages {
        let reference = match PageInk::read(&page.reference) {
            Ok(ink) => ink,
            Err(error) => {
                eprintln!("{}: reference: {error}", page.name);
                return ExitCode::from(2);
            }
        };
        let ours = match PageInk::read(&page.ours) {
            Ok(ink) => ink,
            Err(error) => {
                eprintln!("{}: ours: {error}", page.name);
                return ExitCode::from(2);
            }
        };
        let template = template_masks(reference.ink.page_mm);
        let missing = missing_masks(&page.model);
        let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
        let mut scales: Vec<f64> = Vec::new();
        for _ in 0..rounds {
            let masked = compare_plots_with_model(
                &reference.ink,
                &ours.ink,
                &template,
                &missing,
                &page.model,
            );
            let whole =
                compare_plots_with_model(&reference.ink, &ours.ink, &[], &missing, &page.model);
            let strokes = stroke_colours(&reference, &ours, &whole);
            let labels = label_colours(&reference, &ours, &whole);
            let edges = sheet_edges(&reference.pens, &ours.pens, &whole.fit);
            let ends = ends_apart(&reference.pens, &ours.pens, &whole.fit);
            scales.push(whole.fit.scale);
            let digest = format!(
                "masked {:.6} / {:.3} / {:.3} mm, labels {} of {}, {} alone; whole {:.6} / {:.3} / \
                 {:.3} mm, labels {} of {}, {} alone; pen {} / {} / {}; strokes {} same {} other \
                 ({} / {} alone); labels {} same {} other, {} characters; edges {} same {} apart; \
                 ends {:.3} same {:.3} other {:.3} alone",
                masked.coverage,
                uncovered_mm(&masked),
                masked.extra_length_mm,
                masked.text_matched,
                masked.text_reference,
                masked.unmatched_our_text.len(),
                whole.coverage,
                uncovered_mm(&whole),
                whole.extra_length_mm,
                whole.text_matched,
                whole.text_reference,
                whole.unmatched_our_text.len(),
                whole.pen.agreed,
                whole.pen.differed,
                whole.pen.unpaired,
                strokes.same(),
                strokes.other(),
                strokes.ink_alone,
                strokes.paper_alone,
                labels.same(),
                labels.other(),
                labels.glyphs_ink_on_ink,
                edges.same,
                edges.apart(),
                ends.same_mm,
                ends.other_mm,
                ends.alone_mm
            );
            *outcomes.entry(digest).or_default() += 1;
        }
        scales.sort_by(f64::total_cmp);
        scales.dedup();
        if outcomes.len() > 1 {
            unstable += 1;
        }
        println!(
            "== {}: {} outcome(s) in {rounds} rounds; the fit's scale took {} value(s), {:?} … {:?}",
            short(page.name),
            outcomes.len(),
            scales.len(),
            scales.first().copied().unwrap_or(f64::NAN),
            scales.last().copied().unwrap_or(f64::NAN)
        );
        for (digest, count) in &outcomes {
            println!("   x{count}: {digest}");
        }
    }
    println!(
        "{} sheet(s), {rounds} rounds each: {unstable} read two ways",
        pages.len()
    );
    if unstable > 0 {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
