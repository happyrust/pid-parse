//! What a `.igr` border template states about its own lettering — and what it
//! does not.
//!
//! The reference plot draws a template's labels at 0.78 of the em they are set
//! in while drawing the sheet's own labels at the full em, so this probe looks
//! for the field that could source that ratio: every `igTextBox` the file
//! carries, its letter height, its text-box width and height (all `0.0` on a
//! template — a boxed field is a `.pid` title-block record), its anchor code
//! and rotation, plus every f32/f64 anywhere in the record's payload that
//! lands in a plausible width-factor window.
//!
//! On `CPE-SQ-A0.igr` / `CPE-SQ-A1.igr` the window comes up empty and no box
//! is stated, which is why the ratio is a plotter rule in this crate rather
//! than a read field. See
//! `docs/analysis/2026-10-23-the-furniture-text-height-already-matched.md`.
//!
//! ```text
//! cargo run --example probe_template_text_fields -- <a.igr|a.pid>
//! ```

use std::io::Read;

use pid_parse::parsers::sheet_records::decode_igtextboxes;

/// Lowest and highest a text width factor could plausibly be.
const WIDTH_FACTOR_WINDOW: std::ops::RangeInclusive<f64> = 0.60..=0.95;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: probe_template_text_fields <file>");
        std::process::exit(2);
    };
    let Ok(file) = std::fs::File::open(&path) else {
        eprintln!("cannot open {path}");
        std::process::exit(2);
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        eprintln!("not a compound file: {path}");
        std::process::exit(2);
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .collect();
    let mut boxes = 0usize;
    let mut factors = 0usize;
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(stream_path.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let records = decode_igtextboxes(&data);
        if records.is_empty() {
            continue;
        }
        println!("\n######## {stream_path}  ({} texts)", records.len());
        // A hit in the window only means something when it is not one of the
        // record's own placement numbers: the insertion point and the letter
        // height are stated as trailing doubles and are themselves inside any
        // plausible width-factor window, so they are named here first.
        for record in &records {
            let boxed = if record.text_box_width_m > 0.0 {
                boxes += 1;
                format!(
                    "box {:.4} x {:.4}",
                    record.text_box_width_m, record.text_box_height_m
                )
            } else {
                "no box".to_string()
            };
            println!(
                "  oid {:5} h={:.4} {boxed} anchor={:02x} rot={:.3} runs={:?} {:?}",
                record.oid,
                record.letter_height_m,
                record.anchor_code,
                record.rotation_rad,
                record
                    .runs
                    .iter()
                    .map(|run| (run.len, run.selector, run.style_id))
                    .collect::<Vec<_>>(),
                record.text
            );
            let placed = [
                record.trailing_double_1,
                record.trailing_double_2,
                record.letter_height_m,
            ];
            let placed = |value: f64| placed.iter().any(|known| (known - value).abs() < 1e-9);
            // Every float this format states is an `f64` on its own eight-byte
            // grid, so a byte-wise `f32` read only ever lands on half a double
            // — the letter height and the insertion point come back out of
            // such a slice, which is noise rather than a field.
            let payload = &data[record.byte_range.start + 6..record.byte_range.end];
            for offset in 0..payload.len().saturating_sub(7) {
                let bytes: [u8; 8] = payload[offset..offset + 8].try_into().unwrap();
                let candidate = f64::from_le_bytes(bytes);
                if candidate.is_finite()
                    && WIDTH_FACTOR_WINDOW.contains(&candidate)
                    && !placed(candidate)
                {
                    factors += 1;
                    println!("    +{offset:03} f64 {candidate:.6}");
                }
            }
        }
    }
    println!("\n{boxes} texts state a box; {factors} width-factor candidates");
}
