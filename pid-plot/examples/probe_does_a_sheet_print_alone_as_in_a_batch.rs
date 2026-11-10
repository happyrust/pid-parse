//! Does a sheet print the same page when it is exported alone and when it is
//! exported in a batch?
//!
//! `pid_plot` prepares its faces for the one drawing it is given; a gate
//! prepares them for every drawing it lays out. The two PDFs carry different
//! font programs, so they cannot be compared byte for byte. This reads both
//! for what they print ([`PageInk`]) and holds them to each other outright,
//! with no tolerance: the inked strokes, the strokes painted in the paper's
//! colour, the strokes along the sheet with their widths, the caps and joins
//! with every piece under them, the colours, and every run of lettering with
//! its text, its start, its height, its advance and its direction.
//!
//! `probe_does_a_sheet_letter_alone_as_in_a_batch` asks the narrower question
//! of which face each run is set in, and reads the faces off the PDF; this one
//! asks whether anything on the paper moved.
//!
//! ```text
//! cargo run -p pid-plot --example probe_does_a_sheet_print_alone_as_in_a_batch -- [<name part> ...]
//! ```
//!
//! Read-only on the corpus (`PID_ACCEPTANCE_CORPUS`). Exit code 1 when a
//! sheet prints two ways, 2 when the corpus cannot be read.

use std::process::ExitCode;

use pid_plot::ink::PageInk;
use pid_plot::read::TextSpan;

#[path = "support/acceptance.rs"]
mod acceptance;

use acceptance::{pages, short, write_alone};

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let mut apart = 0usize;
    for page in &pages {
        let alone_pdf = match write_alone(&page.model) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{}: alone: {error}", page.name);
                return ExitCode::from(2);
            }
        };
        let (batch, alone) = match (PageInk::read(&page.ours), PageInk::read(&alone_pdf)) {
            (Ok(batch), Ok(alone)) => (batch, alone),
            (Err(error), _) | (_, Err(error)) => {
                eprintln!("{}: {error}", page.name);
                return ExitCode::from(2);
            }
        };
        let strokes = batch.ink.segments == alone.ink.segments
            && batch.paper.segments == alone.paper.segments
            && batch.pens == alone.pens
            && batch.ink.page_mm == alone.ink.page_mm;
        let moved: Vec<(&TextSpan, Option<&TextSpan>)> = batch
            .ink
            .spans
            .iter()
            .enumerate()
            .filter(|(index, span)| alone.ink.spans.get(*index) != Some(span))
            .map(|(index, span)| (span, alone.ink.spans.get(index)))
            .collect();
        let labels = moved.is_empty()
            && batch.ink.spans.len() == alone.ink.spans.len()
            && batch.paper.spans == alone.paper.spans;
        println!(
            "== {}: {} bytes in the batch, {} alone; strokes {} ({} inked, {} in the paper's \
             colour, {} along the sheet, {} cap-and-join pair(s)); lettering {} ({} run(s), {} \
             alone)",
            short(page.name),
            page.ours.len(),
            alone_pdf.len(),
            if strokes { "the same" } else { "APART" },
            batch.ink.segments.len(),
            batch.paper.segments.len(),
            batch.pens.sheet_edges.len(),
            batch.pens.ends.len(),
            if labels { "the same" } else { "APART" },
            batch.ink.spans.len(),
            alone.ink.spans.len()
        );
        for (span, other) in moved.iter().take(12) {
            match other {
                Some(other) => println!(
                    "   {:?} at ({:.4}, {:.4}) {:.4} mm high, {:.4} mm long in the batch; {:?} \
                     at ({:.4}, {:.4}) {:.4} mm high, {:.4} mm long alone",
                    span.text,
                    span.origin_mm[0],
                    span.origin_mm[1],
                    span.height_mm,
                    span.advance_mm,
                    other.text,
                    other.origin_mm[0],
                    other.origin_mm[1],
                    other.height_mm,
                    other.advance_mm
                ),
                None => println!("   {:?} has no run alone", span.text),
            }
        }
        if moved.len() > 12 {
            println!("   … and {} more run(s)", moved.len() - 12);
        }
        if !(strokes && labels) {
            apart += 1;
        }
    }
    println!("{} sheet(s): {apart} print two ways", pages.len());
    if apart > 0 {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
