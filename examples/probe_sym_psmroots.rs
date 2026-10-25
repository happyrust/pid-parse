//! Dump PSMroots / JSitesList / PSMclustertable of a .sym — do they name
//! the body's SheetN stream?
use std::path::Path;

use pid_parse::PidParser;

fn hex(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        let hexs: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        let asc: String = chunk
            .iter()
            .map(|b| {
                if b.is_ascii_graphic() {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("    {:04x}  {:47}  {asc}", i * 16, hexs.join(" "));
    }
}

fn main() {
    for arg in std::env::args().skip(1) {
        let package = PidParser::new()
            .parse_package(Path::new(&arg))
            .expect("compound file");
        println!("== {arg}");
        for (stream_path, raw) in &package.streams {
            let name = stream_path.rsplit('/').next().unwrap_or("");
            if matches!(name, "PSMroots" | "JSitesList" | "PSMclustertable") {
                println!("  -- {stream_path} ({} bytes)", raw.data.len());
                hex(&raw.data);
            }
        }
    }
}
