//! Per-storage JSheetLayer dump for a .sym — which PSMcluster0 says what
//! about each oid, and which sheet stream each primitive's layer ref
//! resolves against.
use std::path::Path;

use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::PidParser;

fn main() {
    let path = std::env::args().nth(1).expect("usage: <file.sym>");
    let package = PidParser::new()
        .parse_package(Path::new(&path))
        .expect("compound file");
    for (stream_path, raw) in &package.streams {
        let name = stream_path.rsplit('/').next().unwrap_or("");
        if name == "PSMcluster0" {
            println!("{stream_path}");
            for layer in decode_sheet_layers(&raw.data) {
                println!(
                    "  oid={:<5} num={:<3} name={:?}",
                    layer.oid, layer.layer_number, layer.name
                );
            }
        }
    }
    println!("--- streams ---");
    for (stream_path, raw) in &package.streams {
        println!("{stream_path}  {} bytes", raw.data.len());
    }
}
