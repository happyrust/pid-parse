//! Which height a text record letters at: the one its tail states, or the one
//! its style join answers.
//!
//! The five-drawing comparison pairs every laid-out text primitive with a
//! reference span. One class it cannot pair is the `M` inside the electric
//! motor bubbles: our plot letters it at 2.04 mm and the reference at 4.55 mm,
//! while the `自带` beside it agrees to the hundredth (3.73 mm). Both heights
//! are available to the layout — a record's `letter_height_m` (a scaled
//! direction pair) wins when it is non-zero, the style's character height
//! answers otherwise — so the question is which one a record states and what
//! its style would have given.
//!
//! Reads a `.pid`'s sheet `igTextBox` records, or a `.sym`'s symbol-body text
//! records, and prints both heights side by side:
//!
//! ```text
//! cargo run -p pid-plot --example probe_lettering_heights -- "<file>" [text-filter]
//! ```
//!
//! Read-only: no parser, model or plot change.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use pid_parse::PidParser;
use pid_parse::style_link::text_styles_for_document;
use pid_parse::symbol_library::{SymbolPrimitive, read_symbol_geometry};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: probe_lettering_heights <drawing.pid|symbol.sym> [text-filter]");
        return ExitCode::from(2);
    };
    let filter = args.next().unwrap_or_default();
    let path = Path::new(&path);
    let is_symbol = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"));
    let result = if is_symbol {
        symbol_letters(path, &filter)
    } else {
        sheet_letters(path, &filter)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn sheet_letters(path: &Path, filter: &str) -> Result<(), String> {
    let doc = PidParser::new()
        .parse_file(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let styles = text_styles_for_document(&doc);
    println!(
        "stream\toid\ttext\tinsert_mm\trot_deg\tanchor\tbox_mm\ttail_mm\tstyle_id\tstyle_mm\tfont"
    );
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for record in &geometry.decoded_igtextboxes {
            let text = record.text.trim();
            if !filter.is_empty() && text != filter {
                continue;
            }
            let resolved = styles
                .get(&(sheet.path.clone(), record.oid))
                .and_then(|style| style.effective());
            println!(
                "{}\t{}\t{:?}\t[{:.2},{:.2}]\t{:.1}\t0x{:02X}\t[{}x{}]\t{:.2}\t{}\t{}\t{}",
                sheet.path,
                record.oid,
                text,
                record.trailing_double_1 * 1000.0,
                record.trailing_double_2 * 1000.0,
                record.rotation_rad.to_degrees(),
                record.anchor_code,
                record.text_box_width_m * 1000.0,
                record.text_box_height_m * 1000.0,
                record.letter_height_m * 1000.0,
                resolved
                    .as_ref()
                    .map_or("none".to_string(), |style| style.style_id.to_string()),
                resolved
                    .as_ref()
                    .map_or("none".to_string(), |style| format!(
                        "{:.2}",
                        style.height_mm()
                    )),
                resolved
                    .as_ref()
                    .and_then(|style| style.font_name.clone())
                    .unwrap_or_default(),
            );
        }
    }
    Ok(())
}

fn symbol_letters(path: &Path, filter: &str) -> Result<(), String> {
    let geometry =
        read_symbol_geometry(path).map_err(|error| format!("{}: {error}", path.display()))?;
    println!("text\tat_m\trot_deg\tanchor\ttail_mm\tstyle_mm\tletters_with");
    for item in &geometry.primitives {
        let SymbolPrimitive::Text { text, at } = &item.primitive else {
            continue;
        };
        let text = text.trim();
        if !filter.is_empty() && text != filter {
            continue;
        }
        let lettering = item.lettering.as_ref();
        println!(
            "{:?}\t[{:.5},{:.5}]\t{:.1}\t0x{:02X}\t{:.2}\t{}\t{}",
            text,
            at.0,
            at.1,
            lettering.map_or(0.0, |lettering| lettering.rotation_rad.to_degrees()),
            lettering.map_or(0u8, |lettering| lettering.anchor_code),
            lettering.map_or(0.0, |lettering| lettering.letter_height_m * 1000.0),
            lettering.map_or("none".to_string(), |lettering| lettering.height_m.map_or(
                "none".to_string(),
                |height| format!("{:.2}", height * 1000.0)
            )),
            lettering.map_or("tail", |lettering| {
                if lettering.letter_height_m.abs() > 1e-9 {
                    "tail"
                } else {
                    "style"
                }
            }),
        );
    }
    Ok(())
}
