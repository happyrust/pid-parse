//! Hex-dump a byte range of one stream, so a primitive's recorded provenance
//! can be read back down to the record that produced it.
//!
//! ```text
//! cargo run -p pid-parse --example probe_stream_bytes -- "<file.pid>" "/Sheet6" 976 32
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: <file> <stream> <start> <len>");
    let stream = args.next().expect("usage: <file> <stream> <start> <len>");
    let start: usize = args.next().expect("start").parse()?;
    let len: usize = args.next().expect("len").parse()?;
    let package = PidParser::new().parse_package(PathBuf::from(&path).as_path())?;
    let Some((_, raw)) = package
        .streams
        .iter()
        .find(|(name, _)| name.as_str() == stream)
    else {
        eprintln!("no stream {stream:?}; have:");
        for name in package.streams.keys() {
            eprintln!("  {name}");
        }
        return Ok(());
    };
    let bytes = raw
        .data
        .get(start..(start + len).min(raw.data.len()))
        .unwrap_or(&[]);
    println!(
        "{stream} {} bytes, showing {start}..{}",
        raw.data.len(),
        start + bytes.len()
    );
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let at = start + row * 16;
        let hex: Vec<String> = chunk.iter().map(|byte| format!("{byte:02X}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|byte| {
                if byte.is_ascii_graphic() || *byte == b' ' {
                    char::from(*byte)
                } else {
                    '.'
                }
            })
            .collect();
        println!("{at:08X}  {:<47}  {ascii}", hex.join(" "));
    }
    Ok(())
}
