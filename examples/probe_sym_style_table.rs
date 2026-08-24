//! Probe: does a `.sym` carry its own style table, and does it hold the
//! colours the symbol is meant to draw in?
//!
//! A placed symbol comes into OpenCADStudio with no colour or width, because
//! the placement record (`igSymbol2d`) carries no style link: the payload slot
//! its siblings use for `index` holds the `JSite` id instead. The question
//! this answers is whether the colour lives in the symbol file rather than in
//! the drawing -- every `.sym` turns out to hold a `StyleCluster` stream, and
//! that is the same table `DocumentStyleTable` already reads for a `.pid`.
//!
//! Usage: `cargo run --example probe_sym_style_table -- <path-to.sym>...`

use std::io::Read;

use pid_parse::style_link::DocumentStyleTable;

/// Style ids are small in this corpus; the walk stops well past the highest
/// one any fixture defines rather than asking the table to enumerate itself.
const HIGHEST_STYLE_ID: u32 = 4096;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: probe_sym_style_table <path-to.sym>...");
        return;
    }

    for path in &paths {
        let Ok(mut cfb) = cfb::open(path) else {
            println!("{path}: not a compound file");
            continue;
        };
        let streams: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .filter(|name| name.contains("StyleCluster"))
            .collect();

        println!("\n=== {path}");
        println!("    {} StyleCluster stream(s)", streams.len());
        for stream_path in streams {
            let mut bytes = Vec::new();
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let table = DocumentStyleTable::from_stylecluster_bytes(&bytes);
            println!(
                "    {stream_path} ({} bytes): {} style record(s)",
                bytes.len(),
                table.len()
            );
            for id in 0..=HIGHEST_STYLE_ID {
                let Some(record) = table.get(id) else {
                    continue;
                };
                let Some(symbology) = record.symbology else {
                    println!("        id {id:<4} type 0x{:04X}  (no line symbology)", record.type_code);
                    continue;
                };
                let [r, g, b] = symbology.rgb();
                println!(
                    "        id {id:<4} type 0x{:04X}  #{r:02X}{g:02X}{b:02X}  {:.2}mm",
                    record.type_code,
                    symbology.width_mm()
                );
            }
        }
    }
}
