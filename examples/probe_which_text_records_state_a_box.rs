//! Which `igTextBox` records of a drawing state a box, and how tall is it?
//!
//! A text record's tail either stops after its placement (kind 1) or goes on
//! to state a box (kind 2: a width and a height). The plot writer hangs a
//! boxed record from its box's edge along the line and leaves the box's
//! height unused: the lettered boxed records measured so far all state a box
//! 3.689 mm tall under 3.725 mm lettering, where the box's middle and the
//! letter's own middle are 0.018 mm apart and the reference plots cannot
//! tell the two apart. A record whose box is taller than its lettering can.
//!
//! This lists every boxed record of each drawing's top-level sheets — oid,
//! byte range, anchor code, box, insertion, text — and counts them by box
//! height, lettered and blank apart. On the eleven acceptance drawings the
//! only lettered records with another box are four on the A2 sheet
//! (`低压放空总管` ×2, `仪表风管线` ×2: 42.009 × 4.034 mm), which are the
//! four labels the reference prints one device unit above where the rest of
//! that sheet's lettering says they belong.
//!
//! ```text
//! cargo run --example probe_which_text_records_state_a_box -- <drawing.pid> [<drawing.pid> ...]
//! ```
//!
//! Read-only. Exit code 1 when a file cannot be read.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_parse::parsers::sheet_records::decode_igtextboxes;

fn main() -> ExitCode {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: probe_which_text_records_state_a_box <drawing.pid> [<drawing.pid> ...]");
        return ExitCode::from(2);
    }
    let mut failed = false;
    // Box height in micrometres -> (lettered, blank) over every file.
    let mut heights: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
    for file in &files {
        let mut compound = match std::fs::File::open(file).and_then(cfb::CompoundFile::open) {
            Ok(compound) => compound,
            Err(error) => {
                eprintln!("{file}: {error}");
                failed = true;
                continue;
            }
        };
        let sheets: Vec<PathBuf> = compound
            .walk()
            .filter(|entry| {
                entry.is_stream()
                    && entry.name().starts_with("Sheet")
                    && entry.path().components().count() == 2
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        println!("== {file}");
        for path in sheets {
            let mut data = Vec::new();
            if compound
                .open_stream(&path)
                .and_then(|mut stream| stream.read_to_end(&mut data))
                .is_err()
            {
                eprintln!("{file}: {} cannot be read", path.display());
                failed = true;
                continue;
            }
            let records = decode_igtextboxes(&data);
            let boxed: Vec<_> = records
                .iter()
                .filter(|record| record.text_box_width_m > 0.0)
                .collect();
            println!(
                "   {}: {} text record(s), {} state a box",
                path.display(),
                records.len(),
                boxed.len()
            );
            for record in boxed {
                let lettered = !record.text.trim().is_empty();
                let entry = heights
                    .entry((record.text_box_height_m * 1.0e6).round() as i64)
                    .or_default();
                if lettered {
                    entry.0 += 1;
                } else {
                    entry.1 += 1;
                }
                println!(
                    "      oid {:>6} bytes {:>6}..{:<6} anchor {:#04x} box {:.3} × {:.3} mm at \
                     ({:.4}, {:.4}) mm {:?}",
                    record.oid,
                    record.byte_range.start,
                    record.byte_range.end,
                    record.anchor_code,
                    record.text_box_width_m * 1000.0,
                    record.text_box_height_m * 1000.0,
                    record.trailing_double_1 * 1000.0,
                    record.trailing_double_2 * 1000.0,
                    record.text
                );
            }
        }
    }
    println!("{} file(s); boxed records by box height:", files.len());
    for (height_um, (lettered, blank)) in &heights {
        println!(
            "   {:.3} mm: {lettered} lettered, {blank} blank",
            *height_um as f64 / 1000.0
        );
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
