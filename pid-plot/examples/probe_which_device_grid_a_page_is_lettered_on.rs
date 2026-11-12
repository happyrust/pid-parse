//! Which mapping onto the plotter's 150 dpi device grid reproduces where a
//! reference plot starts each label?
//!
//! The reference plots start every run on a whole device unit (1/150 in). On
//! a standard sheet the writer knows the mapping — `write.rs`'s
//! `device_scale`: the sheet scaled so its width fills a whole number of
//! units, measured from the page's lower-left corner — and starts its own
//! runs on the same units. A page that is not a standard sheet has no mapping
//! there: the A2 sheet states 594.33 × 420.57 mm and was printed on
//! 594 × 420, so its runs are written where the layout puts them, off the
//! grid, and each sits up to half a unit (0.085 mm an axis) from the
//! reference before anything else is wrong with it.
//!
//! This reads both pages back, pairs every reference run with the run of ours
//! spelling the same text nearest to it, and searches, an axis at a time, for
//! the mapping
//!
//! ```text
//! reference units = round(scale × ours mm / unit + offset)
//! ```
//!
//! that reproduces the most reference starts: for each scale of a grid (±0.1 %
//! in steps of one in a million), the offset lying inside the most pairs'
//! unit-wide intervals. It prints the scales and offsets that reach the best
//! count on each axis, the scale both axes share, how many starts the
//! standard sheets' rule itself reproduces (the paper's width rounded down to
//! whole units, no offset) and how many the width rounded to nearest does,
//! and every start the best mapping does not reproduce.
//!
//! Our own starts are read back off a page written on a 1/2400 in grid, a
//! sixteenth of a device unit, so a start is known to a thirty-second of a
//! unit either way; a mapping is not held against a start it misses by less.
//!
//! On a standard sheet this recovers the writer's own mapping, since our runs
//! are already on it; what it counts there is the starts that differ from the
//! reference by a whole unit.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_device_grid_a_page_is_lettered_on -- \
//!     [<sheet name part> ...]
//! ```
//!
//! Exit code 1 when the standard sheets' rule misses more than 2 % of a
//! sheet's starts on either axis — a whole grid off misses about half. The
//! corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::process::ExitCode;

use pid_plot::read::{PlotInk, read_plot};

use acceptance::{pages, short};

/// One device unit of the 150 dpi driver that made the reference plots.
const UNIT_MM: f64 = 25.4 / 150.0;
/// How far from a whole unit a reference start may read and still be on the
/// grid.
const ON_GRID_UNITS: f64 = 0.02;
/// How well a start of ours is known: half of the 1/2400 in grid our page is
/// written on, in device units, and a hair for the arithmetic.
const SLACK_UNITS: f64 = 1.0 / 32.0 + 1.0e-6;
/// How far apart the two pages may start the same text and still be a pair.
const PAIR_WITHIN_MM: f64 = 1.2;
/// The scales searched: `1 ± SCALE_STEPS × SCALE_STEP`.
const SCALE_STEP: f64 = 1.0e-6;
const SCALE_STEPS: i32 = 1000;
/// A sheet is on the standard sheets' grid while the rule misses no more than
/// this share of its starts on either axis. A whole grid off misses about
/// half of them; the handful a sheet on the grid misses are runs that are not
/// label starts — the second piece of a label that changes face, which the
/// writer places after the first rather than on the grid.
const RULE_MISS_PERCENT: usize = 2;
/// How many of the rule's misses are worth naming.
const RULE_MISSES_LISTED: usize = 12;

/// A run both pages letter: where the reference starts it, in device units,
/// and where ours does, in our page's millimetres.
struct Pair {
    text: String,
    reference_units: [f64; 2],
    ours_mm: [f64; 2],
}

/// The reference's on-grid runs paired with ours by text; with them, how many
/// reference runs were off the grid and how many found no run of ours.
fn pair_runs(reference: &PlotInk, ours: &PlotInk) -> (Vec<Pair>, usize, usize) {
    let mut pairs = Vec::new();
    let (mut off_grid, mut unpaired) = (0usize, 0usize);
    for span in &reference.spans {
        if span.text.trim().is_empty() {
            continue;
        }
        let units = span.origin_mm.map(|mm| mm / UNIT_MM);
        if units
            .iter()
            .any(|unit| (unit - unit.round()).abs() > ON_GRID_UNITS)
        {
            off_grid += 1;
            continue;
        }
        let nearest = ours
            .spans
            .iter()
            .filter(|candidate| candidate.text == span.text)
            .map(|candidate| {
                let distance = (candidate.origin_mm[0] - span.origin_mm[0])
                    .hypot(candidate.origin_mm[1] - span.origin_mm[1]);
                (distance, candidate)
            })
            .filter(|(distance, _)| *distance < PAIR_WITHIN_MM)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        match nearest {
            Some((_, candidate)) => pairs.push(Pair {
                text: span.text.clone(),
                reference_units: units.map(f64::round),
                ours_mm: candidate.origin_mm,
            }),
            None => unpaired += 1,
        }
    }
    (pairs, off_grid, unpaired)
}

/// Where a mapping puts a pair's start of ours on one axis, device units.
fn mapped(pair: &Pair, axis: usize, scale: f64, offset: f64) -> f64 {
    scale * pair.ours_mm[axis] / UNIT_MM + offset
}

/// Whether a mapping rounds a pair's start onto its reference unit, within
/// what our own start is known to.
fn reproduced(pair: &Pair, axis: usize, scale: f64, offset: f64) -> bool {
    (mapped(pair, axis, scale, offset) - pair.reference_units[axis]).abs() <= 0.5 + SLACK_UNITS
}

/// How many pairs a mapping reproduces on each axis.
fn count(pairs: &[Pair], scale: f64, offset: [f64; 2]) -> [usize; 2] {
    [0, 1].map(|axis| {
        pairs
            .iter()
            .filter(|pair| reproduced(pair, axis, scale, offset[axis]))
            .count()
    })
}

/// For one axis and one scale: the most pairs any one offset reproduces, and
/// the offsets (`low..high`) of the first stretch that reaches it.
///
/// A pair is reproduced by every offset in the interval that rounds its
/// mapped start onto its reference unit, so the best offset is the point
/// inside the most intervals.
fn best_offset(pairs: &[Pair], axis: usize, scale: f64) -> (usize, f64, f64) {
    let mut events: Vec<(f64, i32)> = Vec::with_capacity(pairs.len() * 2);
    for pair in pairs {
        let middle = pair.reference_units[axis] - mapped(pair, axis, scale, 0.0);
        events.push((middle - 0.5 - SLACK_UNITS, 1));
        events.push((middle + 0.5 + SLACK_UNITS, -1));
    }
    // Starts before ends at the same point: the intervals are closed.
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let (mut inside, mut best) = (0i32, 0i32);
    let (mut low, mut high) = (0.0, 0.0);
    let mut open = false;
    for (position, step) in events {
        if open && step < 0 {
            high = position;
            open = false;
        }
        inside += step;
        if inside > best {
            best = inside;
            low = position;
            open = true;
        }
    }
    (best.max(0) as usize, low, high)
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
    let mut by_the_rule = 0usize;
    for page in &pages {
        let (Ok(reference), Ok(ours)) = (read_plot(&page.reference), read_plot(&page.ours)) else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        let (pairs, off_grid, unpaired) = pair_runs(&reference, &ours);
        // The paper, to the hundredth of a millimetre a page size is stated to.
        let paper_mm = reference.page_mm.map(|mm| (mm * 100.0).round() / 100.0);
        let width_units = paper_mm[0] / UNIT_MM;
        println!(
            "== {} — the reference's paper {:.2} × {:.2} mm ({:.3} units wide), our page {:.2} × \
             {:.2} mm; {} run(s) paired by text, {} reference run(s) off the grid, {} with no run \
             of ours",
            short(page.name),
            paper_mm[0],
            paper_mm[1],
            width_units,
            ours.page_mm[0],
            ours.page_mm[1],
            pairs.len(),
            off_grid,
            unpaired
        );
        if pairs.is_empty() {
            continue;
        }

        // Every scale's best count and offsets, both axes.
        let scales: Vec<f64> = (-SCALE_STEPS..=SCALE_STEPS)
            .map(|step| 1.0 + f64::from(step) * SCALE_STEP)
            .collect();
        let searched: Vec<[(usize, f64, f64); 2]> = scales
            .iter()
            .map(|scale| {
                [
                    best_offset(&pairs, 0, *scale),
                    best_offset(&pairs, 1, *scale),
                ]
            })
            .collect();
        let best: [usize; 2] = [0, 1].map(|axis| {
            searched
                .iter()
                .map(|found| found[axis].0)
                .max()
                .unwrap_or(0)
        });
        for (axis, name) in [(0usize, "x"), (1usize, "y")] {
            let reaching: Vec<usize> = (0..scales.len())
                .filter(|index| searched[*index][axis].0 == best[axis])
                .collect();
            let low = reaching
                .iter()
                .map(|index| searched[*index][axis].1)
                .fold(f64::MAX, f64::min);
            let high = reaching
                .iter()
                .map(|index| searched[*index][axis].2)
                .fold(f64::MIN, f64::max);
            println!(
                "   {name}: the best mapping reproduces {} of {} starts — scale {:.6}…{:.6}, offset \
                 {:+.3}…{:+.3} units",
                best[axis],
                pairs.len(),
                scales[reaching[0]],
                scales[reaching[reaching.len() - 1]],
                low,
                high
            );
        }

        // The scale both axes share: best on both where one exists, else the
        // one reproducing the most starts over the two axes together.
        let shared: Vec<usize> = (0..scales.len())
            .filter(|index| searched[*index][0].0 == best[0] && searched[*index][1].0 == best[1])
            .collect();
        let candidates = if shared.is_empty() {
            let most = searched
                .iter()
                .map(|found| found[0].0 + found[1].0)
                .max()
                .unwrap_or(0);
            println!("   no one scale is best on both axes; taking the best over the two together");
            (0..scales.len())
                .filter(|index| searched[*index][0].0 + searched[*index][1].0 == most)
                .collect()
        } else {
            shared
        };
        let (first, last) = (candidates[0], candidates[candidates.len() - 1]);
        let within = |scale: f64| scale >= scales[first] - 1e-12 && scale <= scales[last] + 1e-12;
        let side = |scale: f64| if within(scale) { "inside" } else { "outside" };
        let down = width_units.floor() / width_units;
        let nearest = width_units.round() / width_units;
        let by_rule = count(&pairs, down, [0.0, 0.0]);
        let by_nearest = count(&pairs, nearest, [0.0, 0.0]);
        println!(
            "   both axes: scale {:.6}…{:.6}",
            scales[first], scales[last]
        );
        println!(
            "   the paper's width rounded down, {:.0} units: scale {down:.7} ({}), no offset — \
             reproduces {} (x) and {} (y) of {}",
            width_units.floor(),
            side(down),
            by_rule[0],
            by_rule[1],
            pairs.len()
        );
        println!(
            "   the paper's width to nearest,   {:.0} units: scale {nearest:.7} ({}), no offset — \
             reproduces {} (x) and {} (y) of {}",
            width_units.round(),
            side(nearest),
            by_nearest[0],
            by_nearest[1],
            pairs.len()
        );
        // What the rule misses, named while it is a handful: on a standard
        // sheet these are runs the writer did not put on the grid itself.
        let rule_misses: Vec<String> = pairs
            .iter()
            .flat_map(|pair| {
                [(0usize, "x"), (1usize, "y")]
                    .into_iter()
                    .filter(|(axis, _)| !reproduced(pair, *axis, down, 0.0))
                    .map(|(axis, name)| {
                        format!(
                            "{name}: {:?} reference {:.0} units, the rule gives {:.2}",
                            pair.text,
                            pair.reference_units[axis],
                            mapped(pair, axis, down, 0.0)
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        if rule_misses.len() <= RULE_MISSES_LISTED {
            for line in &rule_misses {
                println!("      the rule misses {line}");
            }
        }
        let on_the_rule = by_rule
            .iter()
            .all(|reproduced| (pairs.len() - reproduced) * 100 <= pairs.len() * RULE_MISS_PERCENT);
        if on_the_rule {
            println!(
                "   → the standard sheets' rule is this sheet's grid ({} start(s) of {} a unit off)",
                rule_misses.len(),
                pairs.len() * 2
            );
            by_the_rule += 1;
            continue;
        }
        let middle = candidates[candidates.len() / 2];
        let scale = scales[middle];
        let offset = [0, 1].map(|axis| {
            let (_, low, high) = searched[middle][axis];
            (low + high) / 2.0
        });
        println!(
            "   → the standard sheets' rule is not this sheet's grid; the best found is scale \
             {scale:.6}, offset ({:+.3}, {:+.3}) units = ({:+.4}, {:+.4}) mm, which leaves:",
            offset[0],
            offset[1],
            offset[0] * UNIT_MM,
            offset[1] * UNIT_MM
        );
        let mut missed = 0usize;
        for pair in &pairs {
            for (axis, name) in [(0usize, "x"), (1usize, "y")] {
                if !reproduced(pair, axis, scale, offset[axis]) {
                    missed += 1;
                    println!(
                        "      {name}: {:?} reference {:.0} units, the mapping gives {:.2}",
                        pair.text,
                        pair.reference_units[axis],
                        mapped(pair, axis, scale, offset[axis])
                    );
                }
            }
        }
        if missed == 0 {
            println!("      nothing: every start reproduced");
        }
    }
    println!(
        "{} sheet(s): {} lettered on the grid the standard sheets' rule gives, {} on another",
        pages.len(),
        by_the_rule,
        pages.len() - by_the_rule
    );
    if by_the_rule == pages.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
