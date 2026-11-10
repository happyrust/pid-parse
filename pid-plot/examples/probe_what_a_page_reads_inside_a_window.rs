//! What does the acceptance comparison read inside a rectangle of a page, and
//! what is left once that rectangle is set aside?
//!
//! A page-wide reading says how much ink differs, not whether the difference
//! is one thing in one place. The A2 sheet reads coverage 0.9829 with
//! 208.671 mm uncovered and 187.091 mm extra, attributed to one instrument
//! cluster the `.pid` stores 16.8 mm from where the reference prints it. If
//! that is the whole of it, the page outside the cluster reads 1.000000 with
//! nothing uncovered, nothing extra and every label paired; this asks.
//!
//! Each window is read by `compare_plots_with_model` with everything outside
//! it masked, and the rest of the page with the windows themselves masked —
//! the gate's rules, asked about a rectangle. The template masks are not
//! applied: a window is stated in the reference's page millimetres and the
//! readings are whole-page ones.
//!
//! ```text
//! cargo run -p pid-plot --example probe_what_a_page_reads_inside_a_window -- \
//!     <sheet name part> <x0,y0,x1,y1> [<x0,y0,x1,y1> ...]
//! ```
//!
//! With no window given, the four the A2 sheet's differences lie in are
//! used. Exit code 1 when the page outside the windows does not read
//! coverage 1.000000 with 0.000 mm uncovered, 0.000 mm extra and every label
//! paired.
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::process::ExitCode;

use pid_plot::compare::{CompareReport, MaskRect, compare_plots_with_model};
use pid_plot::read::{PlotInk, read_plot};

use acceptance::{labels_off, missing_masks, outside, pages, short, uncovered_mm};

/// The A2 sheet, and the four rectangles its differences lie in: the
/// temperature loop's bubbles with the whole of their signal line, the two
/// pipeline labels whose text differs, and the sheet's lower left corner,
/// where the reference's sheet edge ends in a 0.169 mm stub of its own (one
/// device unit of the 150 dpi driver that made it; the edge itself is a stroke
/// the reader leaves out of both pages).
const A2: &str = "轻烃分离器I气出口";
const A2_WINDOWS: [(&str, [f64; 4]); 4] = [
    (
        "TT / TICA loop and its signal line",
        [436.0, 196.0, 520.0, 346.0],
    ),
    ("label 天然气自轻烃分…器I来", [50.0, 291.5, 96.0, 298.5]),
    ("label …-PC-0114301-A2-C40", [100.0, 312.0, 150.0, 319.0]),
    (
        "stub at the sheet's lower left corner",
        [0.0, 0.2, 0.4, 0.7],
    ),
];

fn parse_window(text: &str) -> Option<MaskRect> {
    let numbers: Vec<f64> = text
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    let [x0, y0, x1, y1] = numbers[..] else {
        return None;
    };
    Some(MaskRect {
        label: text.to_string(),
        min_mm: [x0.min(x1), y0.min(y1)],
        max_mm: [x0.max(x1), y0.max(y1)],
    })
}

fn print_reading(title: &str, report: &CompareReport, reference: &PlotInk) {
    let off = labels_off(report, reference);
    println!(
        "   {title}: reference ink {:.3} mm, coverage {:.6}, uncovered {:.3} mm, extra {:.3} mm, \
         labels {}/{} (ours {}), {} past the label limits",
        report.reference_length_mm,
        report.coverage,
        uncovered_mm(report),
        report.extra_length_mm,
        report.text_matched,
        report.text_reference,
        report.text_ours,
        off.len()
    );
    for group in &report.uncovered_reference {
        println!(
            "         uncovered near {:<40} {:>9.3} mm",
            group.label, group.length_mm
        );
    }
    for group in &report.extra_outside_by_symbol {
        println!(
            "         extra from     {:<40} {:>9.3} mm",
            group.label, group.length_mm
        );
    }
    for text in &report.unmatched_reference_text {
        println!(
            "         reference only: {:?} at ({:.2}, {:.2})",
            text.text, text.origin_mm[0], text.origin_mm[1]
        );
    }
    for text in &report.unmatched_our_text {
        println!(
            "         ours only:      {:?} at ({:.2}, {:.2})",
            text.text, text.origin_mm[0], text.origin_mm[1]
        );
    }
    for line in &off {
        println!("         past limits:    {line}");
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let sheet = args.next().unwrap_or_else(|| A2.to_string());
    let mut windows: Vec<MaskRect> = Vec::new();
    for text in args {
        let Some(window) = parse_window(&text) else {
            eprintln!("{text}: a window is x0,y0,x1,y1 in the reference's page millimetres");
            return ExitCode::from(2);
        };
        windows.push(window);
    }
    if windows.is_empty() {
        windows = A2_WINDOWS
            .iter()
            .map(|(label, [x0, y0, x1, y1])| MaskRect {
                label: (*label).to_string(),
                min_mm: [*x0, *y0],
                max_mm: [*x1, *y1],
            })
            .collect();
    }
    let pages = match pages(&[sheet]) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let mut clean = true;
    for page in &pages {
        let (Ok(reference), Ok(ours)) = (read_plot(&page.reference), read_plot(&page.ours)) else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        let missing = missing_masks(&page.model);
        println!("== {}", short(page.name));
        let whole = compare_plots_with_model(&reference, &ours, &[], &missing, &page.model);
        print_reading("the whole page", &whole, &reference);
        println!(
            "   fit: scale {:.7}, translation ({:+.4}, {:+.4}) mm",
            whole.fit.scale, whole.fit.translation_mm[0], whole.fit.translation_mm[1]
        );
        for window in &windows {
            let mut masks = outside(window);
            masks.extend(missing.iter().cloned());
            let inside = compare_plots_with_model(&reference, &ours, &masks, &[], &page.model);
            print_reading(
                &format!(
                    "inside {:?} x {:.1}..{:.1} y {:.1}..{:.1}",
                    window.label,
                    window.min_mm[0],
                    window.max_mm[0],
                    window.min_mm[1],
                    window.max_mm[1]
                ),
                &inside,
                &reference,
            );
        }
        let mut masks = windows.clone();
        masks.extend(missing.iter().cloned());
        let rest = compare_plots_with_model(&reference, &ours, &masks, &[], &page.model);
        print_reading("the page outside the windows", &rest, &reference);
        let outside_clean = rest.coverage >= 0.999_999_5
            && uncovered_mm(&rest) < 5e-4
            && rest.extra_length_mm < 5e-4
            && rest.text_matched == rest.text_reference
            && rest.text_ours == rest.text_reference;
        println!(
            "   → outside the windows the page {}",
            if outside_clean {
                "reads coverage 1.000000, nothing uncovered, nothing extra, every label paired"
            } else {
                "still differs"
            }
        );
        clean &= outside_clean;
    }
    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
