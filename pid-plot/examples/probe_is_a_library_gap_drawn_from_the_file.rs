//! Is a symbol the library does not carry missing from the page, or only
//! from the library?
//!
//! `PaperModel::library_gaps` lists every symbol file a drawing names that
//! no library source answers while the placement was drawn from its cached
//! body in the `.pid`. What is missing from the page is
//! `PaperModel::missing_symbols` / `PaperModel::missing_boxes` — a placement
//! neither source could draw — and only those are masked out of the acceptance
//! comparison.
//!
//! The three A0 wellsites of `验收数据` list `pulsation damper.sym`. This
//! prints, for every acceptance sheet that lists a symbol, each placement of
//! each listed symbol: how many primitives the page drew for it and how much
//! ink, the box they span, and what the gate's own comparison reads inside
//! that box grown by a millimetre — reference ink, uncovered reference ink and
//! extra ink — with both pages read as ink only, so a mark painted in the
//! paper's colour on either side cannot stand in for a stroke.
//!
//! ```text
//! cargo run -p pid-plot --example probe_is_a_library_gap_drawn_from_the_file -- [<sheet name part> ...]
//! ```
//!
//! Exit code 1 when a listed symbol has a placement the page did not draw, a
//! missing-symbol box, or a placement whose box does not read fully covered
//! with nothing extra.
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::collections::BTreeMap;
use std::process::ExitCode;

use pid_plot::compare::{MaskRect, compare_plots_with_model};
use pid_plot::read::{Painted, read_plot_painted};

use acceptance::{missing_masks, outside, pages, short, strokes_of, uncovered_mm};

/// How far past a placement's own ink its window reaches, millimetres.
const GROW_MM: f64 = 1.0;

fn main() -> ExitCode {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let mut clean = true;
    let (mut listed, mut placements, mut ink_total) = (0usize, 0usize, 0.0f64);
    for page in &pages {
        let model = &page.model;
        if model.library_gaps.is_empty() {
            println!(
                "== {}: the library answers every symbol the sheet names; {} missing-symbol box(es)",
                short(page.name),
                model.missing_boxes.len()
            );
            clean &= model.missing_boxes.is_empty();
            continue;
        }
        let (Ok(reference), Ok(ours)) = (
            read_plot_painted(&page.reference, Painted::Inked),
            read_plot_painted(&page.ours, Painted::Inked),
        ) else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        // The fit both pages share, taken on the whole page.
        let whole = compare_plots_with_model(&reference, &ours, &[], &missing_masks(model), model);
        println!(
            "== {}: the library lacks {:?}; {} missing-symbol box(es); fit scale {:.7}, translation \
             ({:+.4}, {:+.4}) mm",
            short(page.name),
            model.library_gaps,
            model.missing_boxes.len(),
            whole.fit.scale,
            whole.fit.translation_mm[0],
            whole.fit.translation_mm[1],
        );
        clean &= model.missing_boxes.is_empty();
        for name in &model.library_gaps {
            listed += 1;
            // The key is the library-relative path, lower case; a primitive
            // carries the path as the drawing spells it.
            let key = name.split(" (").next().unwrap_or(name).to_lowercase();
            let mut by_placement: BTreeMap<[usize; 2], (usize, f64, [f64; 4])> = BTreeMap::new();
            for primitive in &model.primitives {
                let Some(symbol) = &primitive.symbol else {
                    continue;
                };
                if !symbol.to_lowercase().replace('/', "\\").ends_with(&key) {
                    continue;
                }
                let entry = by_placement
                    .entry(primitive.byte_range.unwrap_or([0, 0]))
                    .or_insert((0, 0.0, [f64::MAX, f64::MAX, f64::MIN, f64::MIN]));
                entry.0 += 1;
                for points in strokes_of(&primitive.shape) {
                    for pair in points.windows(2) {
                        entry.1 += (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
                    }
                    for point in &points {
                        entry.2[0] = entry.2[0].min(point[0]);
                        entry.2[1] = entry.2[1].min(point[1]);
                        entry.2[2] = entry.2[2].max(point[0]);
                        entry.2[3] = entry.2[3].max(point[1]);
                    }
                }
            }
            if by_placement.is_empty() {
                clean = false;
                println!("   {name}: NO primitive on the page carries this symbol");
                continue;
            }
            println!(
                "   {name}: {} placement(s) drawn from the file's own body",
                by_placement.len()
            );
            for (bytes, (primitives, ink_mm, [x0, y0, x1, y1])) in &by_placement {
                placements += 1;
                ink_total += ink_mm;
                // The window is stated in the reference's page: our box moved
                // by the fit, then grown.
                let to_reference = |x: f64, y: f64| {
                    [
                        (x - whole.fit.translation_mm[0]) / whole.fit.scale,
                        (y - whole.fit.translation_mm[1]) / whole.fit.scale,
                    ]
                };
                let low = to_reference(*x0, *y0);
                let high = to_reference(*x1, *y1);
                let window = MaskRect {
                    label: name.clone(),
                    min_mm: [low[0] - GROW_MM, low[1] - GROW_MM],
                    max_mm: [high[0] + GROW_MM, high[1] + GROW_MM],
                };
                let inside =
                    compare_plots_with_model(&reference, &ours, &outside(&window), &[], model);
                let covered = uncovered_mm(&inside) < 5e-4 && inside.extra_length_mm < 5e-4;
                clean &= covered && *primitives > 0;
                println!(
                    "      /Sheet bytes {}..{}: {primitives} primitives, {ink_mm:.3} mm of ink, box x \
                     {x0:.3}..{x1:.3} y {y0:.3}..{y1:.3}; inside it grown {GROW_MM} mm the reference \
                     inks {:.3} mm, uncovered {:.3} mm, extra {:.3} mm{}",
                    bytes[0],
                    bytes[1],
                    inside.reference_length_mm,
                    uncovered_mm(&inside),
                    inside.extra_length_mm,
                    if covered { "" } else { "  ← DIFFERS" }
                );
            }
        }
    }
    println!(
        "{} sheet(s): {listed} symbol name(s) the library lacks, {placements} placement(s) of them on \
         the pages drawing {ink_total:.3} mm of ink; {}",
        pages.len(),
        if clean {
            "every one is drawn and reads as the reference inks it — the gap is the library's, not \
             the page's"
        } else {
            "at least one is not drawn as the reference inks it"
        }
    );
    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
