//! Type-code census of a .sym's PSMcluster0 — is there a 0x0057
//! ViewFilterSet (or similar) that states which layers/sheets draw?
use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::PidParser;

fn main() {
    for arg in std::env::args().skip(1) {
        let package = PidParser::new()
            .parse_package(Path::new(&arg))
            .expect("compound file");
        println!("== {arg}");
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() != Some("PSMcluster0") {
                continue;
            }
            let mut counts: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
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
                let e = counts.entry(t).or_default();
                e.0 += 1;
                e.1 = len;
                at += 6 + len;
            }
            for (t, (n, len)) in &counts {
                println!("  t=0x{t:03X} x{n} (last len {len})");
            }
        }
    }
}
