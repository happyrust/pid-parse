//! Which pen does the reference plotter print each declared stroke width on?
//!
//! A drawing declares its stroke widths in millimetres and the vendor's
//! plots print them on a handful of pens, each a whole number of device
//! units — a unit is 1/150 inch, 0.1693 mm, `0.48 w`. The writer's
//! [`pen_width_mm`] says which. This reads the rule off the plots: for
//! every straight piece our page model strokes — a line, or one side of a
//! polyline — it finds the stroke the reference plot paints at the same
//! place and notes that stroke's width, in device units, under the width the
//! model's primitive declares. A stroke along the sheet is read too
//! ([`read_plot_sequence`]), which is where the widest pen is.
//!
//! For each declared width it prints the pieces found, the device units the
//! reference prints them on, and what two statements of the rule say:
//!
//! * `cut` — the declared width in device units, cut to a whole unit and
//!   never under one: `max(1, trunc(width / 0.16933))`;
//! * `pen` — what [`pen_width_mm`] prints, in units.
//!
//! A width whose pieces the reference prints on more than one pen shows
//! each; the count beside a pen is what it rests on. Pieces whose place
//! holds strokes of two widths on the reference are left out — two records
//! drawn on each other, of which the plot shows one — as are dashed pieces,
//! which no single reference stroke answers to.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_pen_a_declared_width_prints_on -- <corpus> [width ...]
//! ```
//!
//! `<corpus>` holds the drawings, each with its reference plot of the same
//! name beside it: the acceptance corpus. Any further arguments are widths
//! in millimetres no plot was read for, printed with what the two statements
//! say of them. Exit code 1 if the pen most of a width's pieces are printed
//! on is not the one `cut` gives, or not the one `pen` gives. Read-only.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::{PaperShape, lay_out_drawing};
use pid_plot::read::read_plot_sequence;
use pid_plot::write::pen_width_mm;

/// One device unit of the reference plotter, millimetres.
const UNIT_MM: f64 = 25.4 / 150.0;

/// How far apart two midpoints may lie and still be the same place.
const NEAR_MM: f64 = 0.3;

fn units(width_mm: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (width_mm / UNIT_MM).round().max(0.0) as u32
    }
}

/// The declared width in device units, cut to a whole unit, never under one.
fn cut(declared_mm: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (declared_mm / UNIT_MM).trunc().max(1.0) as u32
    }
}

fn paired_sheets(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
                && path.with_extension("pdf").is_file()
        })
        .collect();
    paths.sort();
    paths
}

/// The straight pieces a primitive strokes.
fn pieces(shape: &PaperShape) -> Vec<([f64; 2], [f64; 2])> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![(*start_mm, *end_mm)],
        PaperShape::Polyline { points_mm, closed } => {
            let mut pieces: Vec<_> = points_mm
                .windows(2)
                .map(|pair| (pair[0], pair[1]))
                .collect();
            if *closed && points_mm.len() > 2 {
                pieces.push((points_mm[points_mm.len() - 1], points_mm[0]));
            }
            pieces
        }
        _ => Vec::new(),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(corpus) = args.next().map(PathBuf::from) else {
        eprintln!("usage: probe_which_pen_a_declared_width_prints_on <corpus> [width ...]");
        return ExitCode::from(2);
    };
    let unread: Vec<f64> = args.filter_map(|width| width.parse().ok()).collect();
    let sheets = paired_sheets(&corpus);
    if sheets.is_empty() {
        eprintln!("{}: no .pid with a plot beside it", corpus.display());
        return ExitCode::from(2);
    }
    // declared width (tenths of a micrometre) -> reference units -> pieces
    let mut tally: BTreeMap<u64, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut two_widths = 0usize;
    for path in &sheets {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let reference = match std::fs::read(path.with_extension("pdf"))
            .map_err(|error| error.to_string())
            .and_then(|bytes| read_plot_sequence(&bytes).map_err(|error| error.to_string()))
        {
            Ok(reference) => reference,
            Err(error) => {
                println!("{name} | the reference does not read: {error}");
                continue;
            }
        };
        let model = match lay_out_drawing(path, &corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("{name} | does not lay out: {error}");
                continue;
            }
        };
        // The reference's stroked ink by the millimetre cell its midpoint is in.
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        #[allow(clippy::cast_possible_truncation)]
        let cell = |point: [f64; 2]| (point[0].floor() as i64, point[1].floor() as i64);
        let middle = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        for (index, stroke) in reference.strokes.iter().enumerate() {
            if stroke.filled || stroke.paper {
                continue;
            }
            cells
                .entry(cell(middle(stroke.segment.start_mm, stroke.segment.end_mm)))
                .or_default()
                .push(index);
        }
        let mut found = 0usize;
        for primitive in &model.primitives {
            let Some(pen) = primitive.stroke.as_ref() else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let declared = (pen.width_mm * 10_000.0).round().max(0.0) as u64;
            for (a, b) in pieces(&primitive.shape) {
                let direction = [b[0] - a[0], b[1] - a[1]];
                let length = direction[0].hypot(direction[1]);
                if length < 0.5 {
                    continue;
                }
                let at = middle(a, b);
                let (x, y) = cell(at);
                let mut widths: Vec<u32> = Vec::new();
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for index in cells.get(&(x + dx, y + dy)).into_iter().flatten() {
                            let segment = &reference.strokes[*index].segment;
                            let there = middle(segment.start_mm, segment.end_mm);
                            if (there[0] - at[0]).hypot(there[1] - at[1]) > NEAR_MM {
                                continue;
                            }
                            let other = [
                                segment.end_mm[0] - segment.start_mm[0],
                                segment.end_mm[1] - segment.start_mm[1],
                            ];
                            let other_length = other[0].hypot(other[1]);
                            if other_length < 1e-9 {
                                continue;
                            }
                            let along = (direction[0] * other[0] + direction[1] * other[1]).abs()
                                / (length * other_length);
                            let ratio = (other_length / length).min(length / other_length);
                            if along >= 0.995 && ratio >= 0.75 {
                                widths.push(units(segment.width_mm));
                            }
                        }
                    }
                }
                widths.sort_unstable();
                widths.dedup();
                match widths[..] {
                    [printed] => {
                        *tally
                            .entry(declared)
                            .or_default()
                            .entry(printed)
                            .or_default() += 1;
                        found += 1;
                    }
                    [] => {}
                    _ => two_widths += 1,
                }
            }
        }
        println!("{name} | {found} piece(s) with one reference stroke at their place");
    }
    println!(
        "\ndeclared mm | pieces | reference prints on (units: pieces) | cut | pen | \
         ({two_widths} piece(s) left out: two widths at their place)"
    );
    let mut wrong = 0usize;
    for (declared, printed) in &tally {
        #[allow(clippy::cast_precision_loss)]
        let declared_mm = *declared as f64 / 10_000.0;
        let pieces: usize = printed.values().sum();
        let most = printed
            .iter()
            .max_by_key(|(_, count)| **count)
            .map_or(0, |(units, _)| *units);
        let shown: Vec<String> = printed
            .iter()
            .map(|(units, count)| format!("{units}: {count}"))
            .collect();
        let by_cut = cut(declared_mm);
        let by_pen = units(pen_width_mm(declared_mm));
        let verdict = if most == by_cut && most == by_pen {
            ""
        } else {
            wrong += 1;
            "  <- not what the reference prints"
        };
        println!(
            "{declared_mm:>11.4} | {pieces:>6} | {} | {by_cut} | {by_pen}{verdict}",
            shown.join(", ")
        );
    }
    if !unread.is_empty() {
        println!("\nwidths no plot was read for: declared mm | cut | pen");
        for width in unread {
            println!(
                "{width:>11.4} | {} | {}",
                cut(width),
                units(pen_width_mm(width))
            );
        }
    }
    println!(
        "\nin all: {} sheet(s), {} declared width(s), {wrong} whose pen is not the one the \
         reference prints most of its pieces on",
        sheets.len(),
        tally.len()
    );
    if wrong == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
