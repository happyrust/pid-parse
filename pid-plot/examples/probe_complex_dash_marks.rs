//! Where a complex dash stamps its marks along each run, against where the
//! reference plot puts them.
//!
//! ```text
//! cargo run -p pid-plot --example probe_complex_dash_marks -- <drawing.pid> <reference.pdf>
//! ```
//!
//! A `JStyleSimpleDashType` in its complex form stamps a glyph once per fitted
//! cell of its advance; `model::stamp_complex_dash` owns that layout. This probe
//! reads the glyph back off the reference plot — a 2.5–5 mm stroke that meets
//! the run at 45° is a mark, its pair is the glyph — and prints, per run, the
//! run's length, the cell count three candidate rules give, and the mark centres
//! the reference inks.
//!
//! The filter asks the glyph the run carries, not a fixed length or angle: the
//! CD sheets' glyph is a pair of 3.592 mm slashes at 45° to the run, `I井场`'s
//! is an L of two 2.540 mm arms, one along the run and one across it. A band
//! that held one held neither — the CD sheets' wall stroke measures 4.233 mm
//! against their 3.592 mm glyph, 0.03 mm of margin, and the band that admits
//! `I井场`'s glyph also admits the symbol-body diagonals that are not marks
//! (`docs/analysis/2026-10-27-the-slash-surplus-was-the-probes-own-band.md`).
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

use pid_parse::style_link::{ComplexDash, line_styles_for_document};
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

/// The glyph's arms in the page's own frame, with their lengths in mm.
///
/// A glyph is stored in its own frame — +x along the run, +y across it — so an
/// arm has to be rotated by the run's direction before its direction can be
/// compared with a stroke on the page. The CD sheets' pair reads (2.5, 2.6) mm
/// locally and prints at 135° on this sheet's vertical runs, not at 46°.
fn arms_in_page(complex: &ComplexDash, a: [f64; 2], b: [f64; 2]) -> Vec<([f64; 2], f64)> {
    let run = b[0] - a[0];
    let rise = b[1] - a[1];
    let run_len = run.hypot(rise);
    if run_len <= 1e-9 {
        return Vec::new();
    }
    let u = [run / run_len, rise / run_len];
    let n = [-u[1], u[0]];
    complex
        .strokes()
        .iter()
        .filter(|mark| !mark.is_degenerate())
        .map(|mark| {
            let (dx, dy) = (mark.end.0 - mark.start.0, mark.end.1 - mark.start.1);
            (
                [dx * u[0] + dy * n[0], dx * u[1] + dy * n[1]],
                dx.hypot(dy) * 1000.0,
            )
        })
        .collect()
}

/// Whether a reference stroke is one of the glyph's own arms.
///
/// Asked of the glyph the run actually carries, because the two complex-dash
/// styles in this package ink different glyphs: the CD sheets' is a pair of
/// 3.61 mm slashes at 45° to the run, `I井场`'s is an L of two 2.6 mm arms,
/// one along the run and one across it. A length band cannot serve both — and
/// on the CD sheets it only held the glyph by 0.03 mm against the wall stroke
/// it had to drop, while on `I井场` the band that admitted the reference's
/// 4.19 mm diagonals also admitted symbol-body diagonals that are not marks.
///
/// The length window is the glyph's own size class rather than a tolerance: an
/// arm of a mark cannot be much longer than the arm that drew it, and the
/// window has to start well below 1 because the reader cuts a stroke where
/// another line crosses it.
fn matches_an_arm(stroke: &Stroke, arms: &[([f64; 2], f64)]) -> bool {
    let ink = [stroke.1[0] - stroke.0[0], stroke.1[1] - stroke.0[1]];
    let ink_len = ink[0].hypot(ink[1]);
    if ink_len <= 1e-9 {
        return false;
    }
    arms.iter().any(|(arm, arm_len_mm)| {
        if *arm_len_mm <= 1e-9 || !(0.7..=1.3).contains(&(ink_len / arm_len_mm)) {
            return false;
        }
        let arm_len = arm[0].hypot(arm[1]);
        if arm_len <= 1e-9 {
            return false;
        }
        let cosine = ((arm[0] * ink[0] + arm[1] * ink[1]) / (arm_len * ink_len)).abs();
        cosine.min(1.0).acos().to_degrees() <= 8.0
    })
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
        .map(|segment| (segment.start_mm, segment.end_mm)) // Loose enough for every glyph in the package and tight enough to
        // leave the 2.032 mm dashes out; which of these crosses the run at
        // 45° is asked per run, below.
        .filter(|(a, b)| (2.5..=5.0).contains(&length(*a, *b)))
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
            let arms = arms_in_page(complex, a, b);
            for (c, d) in marks.iter().filter(|s| matches_an_arm(s, &arms)) {
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
