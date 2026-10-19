//! How far off the reference the symbol bodies' own labels letter.
//!
//! The five-drawing comparison leaves a class of unmatched runs that come
//! from symbol bodies rather than from the sheet: nine `M` labels in the I
//! drawing, and one `S` each in the L and H drawings, all about 1 mm out.
//! This joins `pid-plot`'s laid-out text primitives (which carry the library
//! path the shape came from) to the reference plot's spans by text and
//! nearest position, then reports the error of our baseline against the
//! reference's: `our origin − line × our height` versus the span's own
//! baseline, in millimetres and as a fraction of the reference body size.
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
    our_height: f64,
    ref_height: f64,
    dx: f64,
    dy: f64,
    ratio: f64,
}

fn down(line: TextLine) -> f64 {
    match line {
        TextLine::Bottom => 0.0,
        TextLine::Middle => 0.5,
        TextLine::Top => 1.0,
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
        let our_baseline = origin_mm[1] - our_height * down(*line);
        let dy = our_baseline - span.origin_mm[1];
        let ratio = if span.height_mm > 0.2 {
            dy / span.height_mm
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
                our_height,
                ref_height: span.height_mm,
                dx: origin_mm[0] - span.origin_mm[0],
                dy,
                ratio,
            });
    }
    println!("no reference span: {unmatched}");
    if env::var_os("DETAIL").is_some() {
        println!("symbol\ttext\tanchor\tline\trot_deg\tour_h\tref_h\tdx_mm\tdy_mm");
        for ((symbol, anchor, line), rows) in &classes {
            for row in rows {
                println!(
                    "{symbol}\t{}\t{anchor}\t{line}\t{:.1}\t{:.2}\t{:.2}\t{:+.2}\t{:+.2}",
                    row.text,
                    row.rotation_rad.to_degrees(),
                    row.our_height,
                    row.ref_height,
                    row.dx,
                    row.dy
                );
            }
        }
    }
    println!("symbol\tanchor\tline\tn\tdx_mm\tdy_mm\tdy/ref_height");
    for ((symbol, anchor, line), rows) in classes {
        let n = rows.len();
        let mean = |pick: fn(&Row) -> f64| rows.iter().map(pick).sum::<f64>() / n as f64;
        let mut ratios: Vec<f64> = rows.iter().map(|row| row.ratio).collect();
        ratios.sort_by(f64::total_cmp);
        println!(
            "{symbol}\t{anchor}\t{line}\t{n}\t{:+.2}\t{:+.2}\t{}",
            mean(|row| row.dx),
            mean(|row| row.dy),
            ratios
                .iter()
                .map(|ratio| format!("{ratio:+.2}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    ExitCode::SUCCESS
}
