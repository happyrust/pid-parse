//! How far below its insertion point the reference letters each `igTextBox`.
//!
//! Joins the parsed geometry's text records to the reference plot's spans by
//! text and position, then reports `(reference baseline − insertion point) /
//! letter height` for each vertical anchor nibble. A class whose ratio is 0 is
//! lettered at the point, 0.5 is lettered from the body's middle, 1 from its
//! top and 1.32 from the font's ascender line — the mode per class is the
//! placement rule, and the ratio `pid-plot` does not apply is a misread
//! nibble. Set `DETAIL=1` for one row per record.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};
use pid_plot::read::read_plot;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(pid), Some(reference)) = (args.next(), args.next()) else {
        eprintln!("usage: probe_text_anchor_modes <drawing.pid> <reference.pdf>");
        return ExitCode::from(2);
    };
    let doc = match PidParser::new().parse_file(PathBuf::from(&pid).as_path()) {
        Ok(doc) => doc,
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
    let detail = env::var("DETAIL").is_ok_and(|value| value != "0");
    let styles = pid_parse::style_link::text_styles_for_document(&doc);
    let geometry = build_normalized_geometry(&doc);
    let mut used = vec![false; ink.spans.len()];
    // nibble -> ratios of offset over height
    let mut classes: BTreeMap<u8, Vec<f64>> = BTreeMap::new();
    let mut unmatched = 0usize;
    for entity in &geometry.entities {
        let PidGraphicKind::Text {
            insertion,
            value,
            anchor_code,
            ..
        } = &entity.kind
        else {
            continue;
        };
        let wanted: String = value.split_whitespace().collect::<Vec<_>>().join("");
        if wanted.is_empty() {
            continue;
        }
        let x_mm = insertion.x * 1000.0;
        let y_mm = insertion.y * 1000.0;
        let found = ink.spans.iter().enumerate().find(|(index, span)| {
            !used[*index]
                && span.text.split_whitespace().collect::<Vec<_>>().join("") == wanted
                && (span.origin_mm[0] - x_mm).abs() < 20.0
                && (span.origin_mm[1] - y_mm).abs() < 20.0
        });
        let Some((index, span)) = found else {
            unmatched += 1;
            continue;
        };
        used[index] = true;
        let height = entity
            .source
            .stream_path
            .as_ref()
            .and_then(|stream| {
                entity.graphic_oid.and_then(|oid| {
                    styles
                        .get(&(stream.clone(), oid))
                        .and_then(|style| style.effective())
                        .map(|resolved| resolved.height_mm())
                })
            })
            .unwrap_or(0.0);
        if height <= 0.2 {
            continue;
        }
        // Reader origins are paper millimetres with y up; the insertion point
        // is too, so a baseline below it is a positive offset.
        let offset = y_mm - span.origin_mm[1];
        if detail {
            println!(
                "v{:<2} ratio {:.2} offset {:>5.2} height {:>4.2} ref {:>4.2} at [{:.1},{:.1}] {:?}",
                anchor_code & 0x0F,
                offset / height,
                offset,
                height,
                span.height_mm,
                x_mm,
                y_mm,
                value.trim()
            );
        }
        classes
            .entry(anchor_code & 0x0F)
            .or_default()
            .push(offset / height);
    }
    println!("no reference span: {unmatched}");
    println!("nibble  n    ratio(offset/height) sorted");
    for (nibble, mut ratios) in classes {
        ratios.sort_by(f64::total_cmp);
        let shown: Vec<String> = ratios.iter().map(|r| format!("{r:.2}")).collect();
        println!("v{nibble:<6} {:>3}  {}", ratios.len(), shown.join(" "));
    }
    ExitCode::SUCCESS
}
