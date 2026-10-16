//! Census of layer names across the .sym library, and per-layer-name
//! primitive counts — to test whether non-"Default" layers are the
//! suppressed-content family.
use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::PidParser;

fn main() {
    let root = std::env::args().nth(1).expect("usage: <symbols-dir>");
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
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
            } else if p.extension().is_some_and(|e| e == "sym") {
                files += 1;
                if let Ok(package) = PidParser::new().parse_package(Path::new(&p)) {
                    for (stream_path, raw) in &package.streams {
                        if stream_path.rsplit('/').next() == Some("PSMcluster0") {
                            for l in decode_sheet_layers(&raw.data) {
                                *names.entry(l.name.clone()).or_default() += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{files} .sym files");
    for (name, n) in &names {
        println!("  {n:4}  {name}");
    }
}
