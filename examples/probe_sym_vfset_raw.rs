//! Raw dump of every 0x0057 record in a .sym's PSMcluster0 — including
//! the ones the strict decoder refuses — to see whether each SheetN has
//! its own view state.
use std::path::Path;

use pid_parse::parsers::view_filter_sets::decode_view_filter_sets;
use pid_parse::PidParser;

fn main() {
    for arg in std::env::args().skip(1) {
        let package = PidParser::new()
            .parse_package(Path::new(&arg))
            .expect("compound file");
        println!(
            "== {}",
            Path::new(&arg).file_name().unwrap().to_string_lossy()
        );
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() != Some("PSMcluster0") {
                continue;
            }
            let decoded = decode_view_filter_sets(&raw.data);
            let mut at = 8usize;
            while at + 6 <= raw.data.len() {
                let t = u16::from_le_bytes([raw.data[at], raw.data[at + 1]]) & 0x0FFF;
                let len = u32::from_le_bytes([
                    raw.data[at + 2],
                    raw.data[at + 3],
                    raw.data[at + 4],
                    raw.data[at + 5],
                ]) as usize;
                if t == 0 || at + 6 + len > raw.data.len() {
                    break;
                }
                if t == 0x057 {
                    let ok = decoded.iter().any(|d| d.byte_range.start == at + 6);
                    let payload = &raw.data[at + 6..at + 6 + len];
                    println!("  0x057 +{at} len={len} decoded={ok}");
                    for (i, chunk) in payload.chunks(16).take(6).enumerate() {
                        let hexs: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
                        println!("      {:04x}  {}", i * 16, hexs.join(" "));
                    }
                }
                at += 6 + len;
            }
        }
    }
}
