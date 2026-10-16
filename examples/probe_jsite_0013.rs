//! Do the drawing's own symbol-definition caches (`JSite*` storages)
//! carry `igBoundary2d` (0x0013) records the way the `.sym` does?

use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::PidParser;
use std::path::Path;

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let package = PidParser::new().parse_package(path).expect("package");
        println!("=== {}", path.file_name().unwrap().to_string_lossy());
        for (stream_path, raw) in &package.streams {
            if !stream_path.starts_with("/JSite") || !stream_path.contains("PSMcluster") {
                continue;
            }
            let bytes = &raw.data;
            let mut hist = std::collections::BTreeMap::new();
            for at in sheet_record_starts(bytes) {
                if at + 6 <= bytes.len() {
                    *hist.entry(u16le(bytes, at) & 0x0FFF).or_insert(0usize) += 1;
                }
            }
            let b13 = hist.get(&0x13).copied().unwrap_or(0);
            println!(
                "  {stream_path} ({}B): 0x13={b13} hist={:?}",
                bytes.len(),
                hist
            );
        }
    }
}
