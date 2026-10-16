//! For .sym files WITHOUT a Sheet6: which sheets do they carry, and which
//! sheet's VFSet is the "normal view" (Default only) vs all-shown?
use std::path::Path;

use pid_parse::parsers::view_filter_sets::decode_view_filter_sets;
use pid_parse::PidParser;

fn main() {
    let root = std::env::args().nth(1).expect("usage: <symbols-dir>");
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
            let Ok(package) = PidParser::new().parse_package(Path::new(&p)) else {
                continue;
            };
            let sheets: Vec<String> = package
                .streams
                .iter()
                .filter_map(|(s, _)| s.rsplit('/').next())
                .filter(|n| n.starts_with("Sheet"))
                .map(str::to_owned)
                .collect();
            if sheets.iter().any(|s| s == "Sheet6") || sheets.is_empty() {
                continue;
            }
            // per-set summary: sheet_ref + whether the hidden-family is off
            let mut line = format!(
                "{}  sheets={sheets:?}",
                p.file_name().unwrap().to_string_lossy()
            );
            for (stream_path, raw) in &package.streams {
                if stream_path.rsplit('/').next() != Some("PSMcluster0") {
                    continue;
                }
                for set in decode_view_filter_sets(&raw.data) {
                    let hidden_family_off = set.layers.iter().all(|l| {
                        let n = l.name.replace(' ', "").to_lowercase();
                        let hidden = matches!(
                            n.as_str(),
                            "label"
                                | "hiddenobjects"
                                | "heattrace"
                                | "jacket"
                                | "construction"
                                | "dimension"
                                | "deminsion"
                        );
                        set.is_displayed(l.layer_number) != Some(true) || !hidden
                    });
                    line += &format!(
                        "  set(ref={}, default-only={hidden_family_off})",
                        set.sheet_ref
                    );
                }
            }
            println!("{line}");
        }
    }
}
