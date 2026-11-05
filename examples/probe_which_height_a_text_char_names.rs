//! Which height of the face does a `JStyleTextChar` height measure?
//!
//! `style.dll` stores a one-byte code at payload `+41`, immediately before the
//! height `f64` at `+42`, and converts the height between codes in its getter
//! (`docs/analysis/2026-11-07-the-text-height-names-which-height-it-is.md`).
//! This walks every `.pid` / `.sym` under the given directories and counts the
//! codes per kind of store: a drawing sheet's own `StyleCluster`, a placed
//! symbol's `JSite` copy, or a `.sym` body's.
//!
//! ```text
//! cargo run --example probe_which_height_a_text_char_names -- <dir> [<dir> ...]
//! ```
//!
//! On the three corpora (`验收数据`, `test-file`, `backup-plant-data`, 1283
//! files) it reads `0x0d` 5273 (sheet 505, jsite 368, sym 4400), `0x0c` 675
//! (sym 597, jsite 76, sheet 2), `0x0b` 118 (sym 114, jsite 4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::style_link::{DocumentStyleTable, PSM_TYPE_CODE_JSTYLE_TEXT_CHAR};
use pid_parse::PidParser;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pid") || ext.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn main() {
    let mut files = Vec::new();
    for dir in std::env::args().skip(1) {
        walk(Path::new(&dir), &mut files);
    }
    if files.is_empty() {
        eprintln!("usage: probe_which_height_a_text_char_names <dir> [<dir> ...]");
        std::process::exit(2);
    }
    let mut counts: BTreeMap<(Option<u8>, &str), usize> = BTreeMap::new();
    let mut parsed = 0usize;
    for path in &files {
        let Ok(package) = PidParser::new().parse_package(path) else {
            continue;
        };
        parsed += 1;
        let body = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sym"));
        for (stream, raw) in &package.streams {
            if !stream.ends_with("StyleCluster") {
                continue;
            }
            let kind = if body {
                "sym"
            } else if stream.contains("JSite") {
                "jsite"
            } else {
                "sheet"
            };
            let table = DocumentStyleTable::from_stylecluster_bytes(&raw.data);
            for record in table.records() {
                if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_CHAR {
                    *counts.entry((record.char_height_mode, kind)).or_default() += 1;
                }
            }
        }
    }
    println!("files {parsed} of {}", files.len());
    for ((code, kind), n) in counts {
        let code = code.map_or_else(|| "none".to_string(), |code| format!("0x{code:02x}"));
        println!("  +41 {code} {kind}: {n}");
    }
}
