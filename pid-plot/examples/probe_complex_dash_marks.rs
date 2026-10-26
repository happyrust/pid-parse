//! Where a complex dash stamps its marks along each run, against where the
//! reference plot puts them.
//!
//! ```text
//! cargo run -p pid-plot --example probe_complex_dash_marks -- <drawing.pid> <reference.pdf>
//! ```
//!
//! A `JStyleSimpleDashType` in its complex form stamps a glyph once per fitted
//! cell of its advance; `model::stamp_complex_dash` owns that layout. This probe
//! reads the glyph back off the reference plot — a 3–4 mm 45° stroke is a mark,
//! its pair is the glyph — and prints, per run, the run's length, the cell count
//! three candidate rules give, and the mark centres the reference inks.
//!
//! The `our centres` line reports where the current layout stamps: one per
//! fitted cell, centred in it, except that a run fitting a single glyph keeps
//! the pattern's own advance and puts that glyph half a period in.
//!
//! The two walls of a pipe carry the same glyph, and either wall's marks fall
//! within a few millimetres of the other's line; the `side` filter keeps a run's
//! own marks only (the glyph half-width is 1.8 mm, so the neighbouring wall sits
//! ~3.8 mm out).

use std::path::Path;

use pid_parse::style_link::line_styles_for_document;
use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};
use pid_plot::read::read_plot;

type Stroke = ([f64; 2], [f64; 2]);

fn length(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// Signed distance of `point` from the line through `a` and `b`, positive on the
/// left of `a -> b`.
fn side(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let square = dx * dx + dy * dy;
    if square <= 1e-12 {
        return 0.0;
    }
    ((point[0] - a[0]) * dy - (point[1] - a[1]) * dx) / square.sqrt()
}

/// A run's own marks, as the centres of the glyph's stroke pairs.
fn clusters(theirs: &[f64]) -> Vec<f64> {
    let mut sorted = theirs.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    let mut group: Vec<f64> = Vec::new();
    for t in sorted {
        if let Some(last) = group.last()
            && (t - last).abs() > 2.2
        {
            out.push(group.iter().sum::<f64>() / group.len() as f64);
            group.clear();
        }
        group.push(t);
    }
    if !group.is_empty() {
        out.push(group.iter().sum::<f64>() / group.len() as f64);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (pid, pdf) = (&args[0], &args[1]);
    let doc = PidParser::new().parse_file(Path::new(pid)).unwrap();
    let geometry = build_normalized_geometry(&doc);
    let styles = line_styles_for_document(&doc);
    let plot = read_plot(&std::fs::read(pdf).unwrap()).unwrap();
    let marks: Vec<Stroke> = plot
        .segments
        .iter()
        .map(|segment| (segment.start_mm, segment.end_mm))
        .filter(|(a, b)| (3.0..=4.2).contains(&length(*a, *b)))
        .collect();

    for entity in &geometry.entities {
        let (Some(stream), Some(oid)) = (entity.source.stream_path.as_ref(), entity.graphic_oid)
        else {
            continue;
        };
        let Some(complex) = styles
            .get(&(stream.clone(), oid))
            .and_then(|style| style.complex_dash.as_ref())
        else {
            continue;
        };
        let path: Vec<[f64; 2]> = match &entity.kind {
            PidGraphicKind::Line { start, end } => {
                vec![
                    [start.x * 1000.0, start.y * 1000.0],
                    [end.x * 1000.0, end.y * 1000.0],
                ]
            }
            PidGraphicKind::Polyline { points, .. } => points
                .iter()
                .map(|point| [point.x * 1000.0, point.y * 1000.0])
                .collect(),
            _ => continue,
        };
        for (index, pair) in path.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            let run = length(a, b);
            if run < 2.0 {
                continue;
            }
            let period = complex.period_mm();
            // The room the glyph claims along the run, as `model` reads it: the
            // widest tangential extent of any stroke it is drawn from.
            let span = complex
                .strokes()
                .iter()
                .filter(|mark| !mark.is_degenerate())
                .map(|mark| (mark.end.0 - mark.start.0).abs() * 1000.0)
                .fold(0.0_f64, f64::max);
            let u = [(b[0] - a[0]) / run, (b[1] - a[1]) / run];
            let mut theirs: Vec<f64> = Vec::new();
            for (c, d) in &marks {
                let mid = [(c[0] + d[0]) / 2.0, (c[1] + d[1]) / 2.0];
                // The run's own wall only: the glyph half-width is 1.8 mm, so a
                // mark of the neighbouring wall sits ~3.8 mm out.
                if side(mid, a, b).abs() > 2.2 {
                    continue;
                }
                let t = (mid[0] - a[0]) * u[0] + (mid[1] - a[1]) * u[1];
                if t < -2.0 || t > run + 2.0 {
                    continue;
                }
                theirs.push(t);
            }
            let ref_marks = clusters(&theirs);
            if ref_marks.is_empty() {
                continue;
            }
            let cells = (run / period).round() as usize;
            let reach = (run - 2.54) / period;
            // The rule `stamp_complex_dash` now applies, so `our centres` is
            // where the page really inks rather than a candidate's answer.
            let fitted = if period > 1e-6 {
                ((run - span).max(0.0) / period).round() as usize
            } else {
                1
            }
            .max(1);
            println!(
                "oid {oid} seg {}/{}  L {run:6.2}  L/p {:6.3}  round {:2}  floor {:2}  reach {:5.3}->{:2}  ref {:2}  \n    ref centres {}\n    our centres {}",
                index + 1,
                path.len() - 1,
                run / period,
                cells,
                (run / period).floor().max(1.0) as usize,
                reach,
                (reach.round() as usize).max(1),
                ref_marks.len(),
                ref_marks
                    .iter()
                    .map(|t| format!("{t:6.2}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                if fitted <= 1 {
                    // A stub that fits one glyph keeps the pattern's own
                    // advance, so its mark sits half a period in — unless the
                    // run stops short of that, in which case no glyph is
                    // stamped at all.
                    if run < period * 0.5 {
                        "none".to_string()
                    } else {
                        format!("{:6.2}", period * 0.5)
                    }
                } else {
                    let step = run / fitted as f64;
                    (0..fitted)
                        .map(|cell| format!("{:6.2}", (cell as f64 + 0.5) * step))
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            );
        }
    }
}
