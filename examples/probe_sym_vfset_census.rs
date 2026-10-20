//! Corpus census of decoded 0x0057 sheet_refs in .sym files vs the SheetN
//! streams each file carries: which sheet does the file's own view state
//! mark as the displayed one?
use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::parsers::view_filter_sets::decode_view_filter_sets;
use pid_parse::PidParser;

fn main() {
    let root = std::env::args().nth(1).expect("usage: <symbols-dir>");
    let mut stack = vec![std::path::PathBuf::from(root)];
    let mut tally: BTreeMap<u32, usize> = BTreeMap::new();
    let mut sets_per_file: BTreeMap<usize, usize> = BTreeMap::new();
    let mut multi: Vec<String> = Vec::new();
    let mut files = 0usize;
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
            let sheets: Vec<String> = package
                .streams
                .keys()
                .filter_map(|s| s.rsplit('/').next())
                .filter(|n| n.starts_with("Sheet"))
                .map(str::to_owned)
                .collect();
            let mut refs = Vec::new();
            for (stream_path, raw) in &package.streams {
                if stream_path.rsplit('/').next() == Some("PSMcluster0") {
                    for set in decode_view_filter_sets(&raw.data) {
                        refs.push(set.sheet_ref);
                    }
                }
            }
            *sets_per_file.entry(refs.len()).or_default() += 1;
            for r in &refs {
                *tally.entry(*r).or_default() += 1;
            }
            if sheets.len() > 1 {
                multi.push(format!(
                    "{}  sheets={sheets:?} refs={refs:?}",
                    p.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    println!("{files} files; sets-per-file histogram: {sets_per_file:?}");
    println!("sheet_ref tally (top): ");
    let mut t: Vec<_> = tally.into_iter().collect();
    t.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for (r, c) in t.iter().take(15) {
        println!("  ref {r:4}  x{c}");
    }
    println!("-- multi-sheet files ({}) --", multi.len());
    for line in multi.iter().take(40) {
        println!("  {line}");
    }
}
