//! Does the unrounded start of `SP:` say where the reference puts its colon?
//!
//! Nine acceptance sheets letter `SP:`; on the three air-cooler sheets the
//! reference's run is one device unit (0.1693 mm) narrower than ours.
//! `probe_sp_colon_step` settled what differs: the reference writes `SP` and
//! `:` as two text objects, and the colon's own `Tm` sits 21 units after the
//! start on those three sheets and 22 on the other six, where the writer
//! steps 11 + 11 on every sheet. Why those three was left open.
//!
//! This asks whether rounding alone does it. A label's start on the
//! plotter's 150 dpi grid, before any rounding, is its insertion scaled by
//! the sheet's device scale (the sheet's width rounded down to whole units,
//! over its width). Each glyph of `SP:` advances 36 of the 72-unit cell at
//! `2.54 mm / 49.2` a unit — 10.9756 device units, the constants
//! `probe_sp_colon_step` states for SimSun. If the first object starts on
//! `round(start)` and the colon's on `round(start + two advances)`, the colon
//! sits 21 units after the start exactly when the start's fraction is in
//! [0.5, 0.5488), and 22 otherwise.
//!
//! One line a sheet: the start in units with its fraction, the colon offset
//! that predicts, and the colon offset the reference has.
//!
//! ```text
//! cargo run -p pid-plot --example probe_does_the_unrounded_start_place_the_colon_of_sp
//! ```
//!
//! Exit code 1 when a sheet's prediction differs from its reference, or no
//! sheet letters the label. The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;
#[path = "support/glyphs.rs"]
mod glyphs;

use std::process::ExitCode;

use pid_plot::model::PaperShape;

use acceptance::{pages, short};
use glyphs::{PIXEL_MM, read_glyphs};

/// The label this probe is about.
const LABEL: &str = "SP:";
/// SimSun's cap height at `lfHeight = 72`, pixels: what a cap-height style
/// divides its height by (`probe_sp_colon_step`'s `REFERENCE_PX`).
const SIMSUN_CAP_PX: f64 = 49.2;
/// The 72-pixel advance of an ASCII glyph in SimSun's 256-unit face.
const HALF_WIDTH_PX: f64 = 36.0;
/// How far from the model's start the reference's `S` may sit, along the
/// line and across it (the insertion is the label's middle line).
const START_WITHIN_MM: [f64; 2] = [0.5, 3.0];

fn main() -> ExitCode {
    let pages = match pages(&[]) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let (mut lettered, mut agreed) = (0usize, 0usize);
    for page in &pages {
        let labels: Vec<([f64; 2], f64)> = page
            .model
            .primitives
            .iter()
            .filter_map(|primitive| match &primitive.shape {
                PaperShape::Text {
                    origin_mm,
                    height_mm,
                    value,
                    rotation,
                    ..
                } if value.trim() == LABEL && rotation.abs() < 1e-6 => {
                    height_mm.map(|height_mm| (*origin_mm, height_mm))
                }
                _ => None,
            })
            .collect();
        if labels.is_empty() {
            println!("{:<44} — no {LABEL} on this sheet", short(page.name));
            continue;
        }
        let Some(page_mm) = page.model.resolved_page_mm() else {
            eprintln!("{}: the page states no size", page.name);
            return ExitCode::from(1);
        };
        let Some(reference) = read_glyphs(&page.reference) else {
            eprintln!("{}: the reference could not be read back", page.name);
            return ExitCode::from(1);
        };
        let width_units = page_mm[0] / PIXEL_MM;
        let scale = width_units.floor() / width_units;
        for (origin_mm, height_mm) in labels {
            lettered += 1;
            let advance = height_mm / PIXEL_MM / SIMSUN_CAP_PX * HALF_WIDTH_PX;
            let start = origin_mm[0] * scale / PIXEL_MM;
            let fraction = start - start.floor();
            let predicted = (start + 2.0 * advance).round() - start.round();
            // The reference's own `S` nearest the model's start, and the colon
            // that follows it on the same baseline.
            let first = reference
                .iter()
                .filter(|glyph| {
                    glyph.text == "S"
                        && (glyph.origin_mm[0] / PIXEL_MM - start).abs()
                            < START_WITHIN_MM[0] / PIXEL_MM + 0.5
                        && (glyph.origin_mm[1] - origin_mm[1]).abs() < START_WITHIN_MM[1]
                })
                .min_by(|a, b| {
                    let key = |glyph: &glyphs::PlacedGlyph| {
                        (glyph.origin_mm[0] - origin_mm[0]).hypot(glyph.origin_mm[1] - origin_mm[1])
                    };
                    key(a).total_cmp(&key(b))
                });
            let colon = first.and_then(|first| {
                reference
                    .iter()
                    .filter(|glyph| {
                        glyph.text == ":"
                            && (glyph.origin_mm[1] - first.origin_mm[1]).abs() < 1e-3
                            && glyph.origin_mm[0] > first.origin_mm[0]
                            && glyph.origin_mm[0] - first.origin_mm[0] < 4.0 * advance * PIXEL_MM
                    })
                    .min_by(|a, b| a.origin_mm[0].total_cmp(&b.origin_mm[0]))
                    .map(|colon| (first, colon))
            });
            let Some((first, colon)) = colon else {
                println!(
                    "{:<44} start {start:9.3} units — the reference letters no {LABEL} there",
                    short(page.name)
                );
                continue;
            };
            let measured = ((colon.origin_mm[0] - first.origin_mm[0]) / PIXEL_MM).round();
            let agrees = (predicted - measured).abs() < 0.5;
            agreed += usize::from(agrees);
            println!(
                "{:<44} start {start:9.3} units (fraction {fraction:.3}), advance {advance:.4}: \
                 colon on round({:.3}) = {:.0}, {predicted:.0} after round(start) | reference: \
                 start {:.2}, colon {measured:.0} after it | {}",
                short(page.name),
                start + 2.0 * advance,
                (start + 2.0 * advance).round(),
                first.origin_mm[0] / PIXEL_MM,
                if agrees { "agrees" } else { "DIFFERS" }
            );
        }
    }
    println!(
        "{lettered} {LABEL} label(s) on the standard sheets: rounding each text object's own start \
         predicts the colon of {agreed}"
    );
    if lettered > 0 && agreed == lettered {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
