//! How far off the reference the symbol bodies' own labels letter.
//!
//! The five-drawing comparison leaves a class of unmatched runs that come
//! from symbol bodies rather than from the sheet: nine `M` labels in the I
//! drawing, and one `S` each in the L and H drawings, all about 1 mm out.
//! This joins `pid-plot`'s laid-out text primitives (which carry the library
//! path the shape came from) to the reference plot's spans by text and
//! nearest position, then reports the error of our baseline against the
//! reference's: `our origin − line × our height` versus the span's own
//! baseline, measured **in the span's own writing frame** — `along` the
//! writing direction and `down` the text's descent — so a rotated label's
//! error is not mistaken for a paper-axis shift. `down` is also given as a
//! fraction of the reference body size, which is the vertical-anchor reading.
//!
//! ```text
//! cargo run -p pid-plot --example probe_symbol_label_offsets -- \
//!     "<drawing.pid>" "<reference.pdf>" "<corpus>"
//! ```
//!
//! Set `DETAIL=1` to list every paired label. Read-only.

use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::process::ExitCode;

use pid_plot::model::{PaperShape, TextLine, lay_out_drawing};
use pid_plot::read::read_plot;

struct Row {
    text: String,
    rotation_rad: f64,
    ref_deg: f64,
    our_height: f64,
    ref_height: f64,
    along: f64,
    down: f64,
    ratio: f64,
}

fn down(line: TextLine) -> f64 {
    match line {
        TextLine::Bottom => 0.0,
        TextLine::Middle => 0.5,
        TextLine::Top => 1.0,
        TextLine::Ascender => 1.32,
    }
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(pid), Some(reference), Some(corpus)) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: probe_symbol_label_offsets <drawing.pid> <reference.pdf> <corpus>");
        return ExitCode::from(2);
    };
    let model = match lay_out_drawing(Path::new(&pid), Path::new(&corpus)) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    let bytes = match std::fs::read(&reference) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{reference}: {error}");
            return ExitCode::from(1);
        }
    };
    let ink = match read_plot(&bytes) {
        Ok(ink) => ink,
        Err(error) => {
            eprintln!("{reference}: {error}");
            return ExitCode::from(1);
        }
    };
    let mut used = vec![false; ink.spans.len()];
    // (symbol, anchor, line) -> paired labels
    let mut classes: BTreeMap<(String, String, String), Vec<Row>> = BTreeMap::new();
    let mut unmatched = 0usize;
    for primitive in &model.primitives {
        let Some(symbol) = primitive.symbol.as_deref() else {
            continue;
        };
        let PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            anchor,
            line,
            rotation,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        let wanted: String = value.split_whitespace().collect::<Vec<_>>().join("");
        if wanted.is_empty() {
            continue;
        }
        let found = ink.spans.iter().enumerate().find(|(index, span)| {
            !used[*index]
                && span.text.split_whitespace().collect::<Vec<_>>().join("") == wanted
                && (span.origin_mm[0] - origin_mm[0]).abs() < 25.0
                && (span.origin_mm[1] - origin_mm[1]).abs() < 25.0
        });
        let Some((index, span)) = found else {
            unmatched += 1;
            continue;
        };
        used[index] = true;
        let our_height = height_mm.unwrap_or(0.0);
        // Our writing frame: `down` is the right-hand normal pointing at
        // descenders (paper y up), so the line position moves the insertion
        // point to the baseline the writer will use.
        let (sin, cos) = rotation.sin_cos();
        let down_dir = [sin, -cos];
        let our_baseline = [
            origin_mm[0] + down_dir[0] * our_height * down(*line),
            origin_mm[1] + down_dir[1] * our_height * down(*line),
        ];
        let delta = [
            our_baseline[0] - span.origin_mm[0],
            our_baseline[1] - span.origin_mm[1],
        ];
        // Project onto the *reference* span's frame, which is the frame the
        // mismatch is judged in: `along` follows its glyphs, `down` is its
        // right-hand normal (`(d.y, -d.x)` with paper y up).
        let ref_dir = span.direction;
        let along = delta[0] * ref_dir[0] + delta[1] * ref_dir[1];
        let down = delta[0] * ref_dir[1] - delta[1] * ref_dir[0];
        let ratio = if span.height_mm > 0.2 {
            down / span.height_mm
        } else {
            f64::NAN
        };
        classes
            .entry((
                symbol
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(symbol)
                    .to_string(),
                format!("{anchor:?}"),
                format!("{line:?}"),
            ))
            .or_default()
            .push(Row {
                text: value.clone(),
                rotation_rad: *rotation,
                ref_deg: span.direction[1].atan2(span.direction[0]).to_degrees(),
                our_height,
                ref_height: span.height_mm,
                along,
                down,
                ratio,
            });
    }
    println!("no reference span: {unmatched}");
    if env::var_os("DETAIL").is_some() {
        println!("symbol\ttext\tanchor\tline\trot_deg\tref_deg\tour_h\tref_h\talong_mm\tdown_mm");
        for ((symbol, anchor, line), rows) in &classes {
            for row in rows {
                println!(
                    "{symbol}\t{}\t{anchor}\t{line}\t{:.1}\t{:.1}\t{:.2}\t{:.2}\t{:+.2}\t{:+.2}",
                    row.text,
                    row.rotation_rad.to_degrees(),
                    row.ref_deg,
                    row.our_height,
                    row.ref_height,
                    row.along,
                    row.down
                );
            }
        }
    }
    println!("symbol\tanchor\tline\tn\talong_mm\tdown_mm\tdown/ref_height");
    for ((symbol, anchor, line), rows) in classes {
        let n = rows.len();
        let mean = |pick: fn(&Row) -> f64| rows.iter().map(pick).sum::<f64>() / n as f64;
        let mut ratios: Vec<f64> = rows.iter().map(|row| row.ratio).collect();
        ratios.sort_by(f64::total_cmp);
        println!(
            "{symbol}\t{anchor}\t{line}\t{n}\t{:+.2}\t{:+.2}\t{}",
            mean(|row| row.along),
            mean(|row| row.down),
            ratios
                .iter()
                .map(|ratio| format!("{ratio:+.2}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    ExitCode::SUCCESS
}
