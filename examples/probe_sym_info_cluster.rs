//! SymbolInformationCluster + sheet list per .sym: does the info record
//! name the body sheet (a sheet number/oid) so multi-sheet files can pick
//! the drawn body deterministically?
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
        let sheets: Vec<&str> = package
            .streams
            .keys()
            .filter_map(|s| s.rsplit('/').next())
            .filter(|n| n.starts_with("Sheet"))
            .collect();
        println!(
            "== {}  sheets={sheets:?}",
            Path::new(&arg).file_name().unwrap().to_string_lossy()
        );
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() == Some("SymbolInformationCluster") {
                hex(&raw.data);
            }
        }
    }
}
