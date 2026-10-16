//! Per-Sheet-stream primitive inventory for one .sym: which records each
//! Sheet stream carries, with each primitive's layer ref and the name the
//! root PSMcluster0 gives it.
use std::path::Path;

use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::symbol_library::read_symbol_geometry;
use pid_parse::PidParser;

fn main() {
    let path = std::env::args().nth(1).expect("usage: <file.sym>");
    let package = PidParser::new()
        .parse_package(Path::new(&path))
        .expect("compound file");
    let mut layers = Vec::new();
    for (stream_path, raw) in &package.streams {
        if stream_path.rsplit('/').next() == Some("PSMcluster0") {
            layers = decode_sheet_layers(&raw.data);
        }
    }
    for (stream_path, raw) in &package.streams {
        let name = stream_path.rsplit('/').next().unwrap_or("");
        if !name.starts_with("Sheet") {
            continue;
        }
        println!("== {stream_path} ({} bytes)", raw.data.len());
        let body = read_symbol_geometry(Path::new(&path));
        let _ = body; // per-stream walk below instead
                      // walk this stream's records
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
            let payload = &raw.data[at + 6..at + 6 + len];
            let layer_ref = u32::from_le_bytes([
                *payload.get(8).unwrap_or(&0),
                *payload.get(9).unwrap_or(&0),
                *payload.get(10).unwrap_or(&0),
                *payload.get(11).unwrap_or(&0),
            ]);
            let lname = layers
                .iter()
                .find(|l| l.oid == layer_ref)
                .map(|l| l.name.as_str())
                .unwrap_or("?");
            println!("  +{at:4} t=0x{t:03X} len={len:4} layer={layer_ref:<5} {lname}");
            at += 6 + len;
        }
    }
}
