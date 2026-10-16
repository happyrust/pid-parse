//! Census: which SheetN stream names .sym files carry, and the type-code
//! histogram of each sheet name family — is Sheet6 the body and higher
//! numbers auxiliary (labels/alternates)?
use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::PidParser;

fn main() {
    let root = std::env::args().nth(1).expect("usage: <symbols-dir>");
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut hist: BTreeMap<String, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut combo: BTreeMap<String, usize> = BTreeMap::new();
    let mut files = 0usize;
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
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
            files += 1;
            let Ok(package) = PidParser::new().parse_package(Path::new(&p)) else {
                continue;
            };
            let mut set = Vec::new();
            for (stream_path, raw) in &package.streams {
                let name = match stream_path.rsplit('/').next() {
                    Some(n) if n.starts_with("Sheet") => n.to_owned(),
                    _ => continue,
                };
                *names.entry(name.clone()).or_default() += 1;
                set.push(name.clone());
                let h = hist.entry(name).or_default();
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
                    *h.entry(t).or_default() += 1;
                    at += 6 + len;
                }
            }
            set.sort();
            *combo.entry(set.join(" + ")).or_default() += 1;
        }
    }
    println!("{files} .sym files");
    println!("-- sheet stream names --");
    for (n, c) in &names {
        println!("  {c:5}  {n}");
    }
    println!("-- sheet combos per file --");
    for (n, c) in &combo {
        println!("  {c:5}  {n}");
    }
    println!("-- type histograms per sheet name --");
    for (name, h) in &hist {
        let mut e: Vec<_> = h.iter().collect();
        e.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        let line: Vec<String> = e.iter().map(|(t, c)| format!("0x{t:03X}x{c}")).collect();
        println!("  {name}: {}", line.join(" "));
    }
}
