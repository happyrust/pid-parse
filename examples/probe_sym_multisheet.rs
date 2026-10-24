//! For every multi-sheet .sym: per-sheet decoded-primitive counts and the
//! layer-name profile, so we can see whether the extra sheet is an
//! alternate/label sheet (duplicates Sheet6 or holds only labels) or body
//! geometry that must merge.
use std::path::Path;

use pid_parse::symbol_library::read_symbol_geometry;
use pid_parse::{PidParser, RawStream};

fn main() {
    let root = std::env::args().nth(1).expect("usage: <symbols-dir>");
    let max: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let mut stack = vec![std::path::PathBuf::from(root)];
    let mut shown = 0usize;
    'outer: while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|e| e == "sym") {
                continue;
            }
            let Ok(package) = PidParser::new().parse_package(Path::new(&p)) else {
                continue;
            };
            let sheet_streams: Vec<(&String, &RawStream)> = package
                .streams
                .iter()
                .filter(|(s, _)| s.rsplit('/').next().is_some_and(|n| n.starts_with("Sheet")))
                .collect();
            if sheet_streams.len() < 2 {
                continue;
            }
            // decode all sheets merged (gives per-primitive layer names)
            let Ok(body) = read_symbol_geometry(&p) else {
                continue;
            };
            // per-sheet record census
            let mut census = Vec::new();
            for (stream_path, raw) in &sheet_streams {
                let name = stream_path.rsplit('/').next().unwrap();
                let mut counts: std::collections::BTreeMap<u16, usize> = Default::default();
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
                    *counts.entry(t).or_default() += 1;
                    at += 6 + len;
                }
                census.push((name.to_string(), counts));
            }
            let fname = p.file_name().unwrap().to_string_lossy().to_string();
            println!("== {fname}");
            for (name, counts) in &census {
                let parts: Vec<String> = counts
                    .iter()
                    .map(|(t, c)| format!("0x{t:03X}x{c}"))
                    .collect();
                println!("   {name}: {}", parts.join(" "));
            }
            // layer-name profile of the whole body
            let mut by_layer: std::collections::BTreeMap<String, usize> = Default::default();
            for styled in &body.primitives {
                *by_layer
                    .entry(
                        styled
                            .sheet_layer_name
                            .clone()
                            .unwrap_or_else(|| format!("<ref {}>", styled.sheet_layer_ref)),
                    )
                    .or_default() += 1;
            }
            let profile: Vec<String> = by_layer.iter().map(|(n, c)| format!("{n}x{c}")).collect();
            println!("   layers: {}", profile.join(" "));
            shown += 1;
            if shown >= max {
                break 'outer;
            }
        }
    }
}
