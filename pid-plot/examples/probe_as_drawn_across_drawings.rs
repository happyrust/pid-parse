//! Is the style the librarian calls `As Drawn` the same record in every
//! drawing, and how many placements name it?
//!
//! The pen disagreement on the five reference pages is exactly the set of
//! symbol families whose placement names style id 10, and the style table says
//! id 10 is a 0.35 mm black `JStyleSimpleLine` the librarian calls
//! `As Drawn`. Every other family names a real item-class style and matches the
//! reference. That makes the name — not the width — the thing to check across
//! drawings before any rule is written around it.
//!
//! ```text
//! cargo run -p pid-plot --example probe_as_drawn_across_drawings
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::geometry::{PidGraphicKind, build_normalized_geometry};
use pid_parse::style_link::line_styles_for_document;
use pid_parse::{ParseOptions, PidParser};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(&arg);
        if path.is_dir() {
            collect_pids(&path, &mut paths);
        } else {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        collect_pids(&corpus, &mut paths);
    }

    println!(
        "{:<44} {:>5} {:>9} {:>8} {:>10} {:>10}",
        "drawing", "id", "width", "named", "placements", "other pens"
    );
    let mut ids: BTreeMap<u32, usize> = BTreeMap::new();
    for path in &paths {
        let Ok(doc) = PidParser::with_options(ParseOptions::geometry()).parse_file(path) else {
            continue;
        };
        let geometry = build_normalized_geometry(&doc);
        let styles = line_styles_for_document(&doc);

        // The record(s) the librarian names `As Drawn`, wherever they sit.
        let mut named: Vec<(u32, f64, [u8; 3], String)> = Vec::new();
        for (storage, table) in &doc.style_tables {
            for record in table.records() {
                let Some(name) = record.name.as_deref() else {
                    continue;
                };
                if !name.eq_ignore_ascii_case("as drawn") {
                    continue;
                }
                let (width, rgb) = record
                    .symbology
                    .map_or((f64::NAN, [0, 0, 0]), |s| (s.width_mm(), s.rgb()));
                named.push((record.style_id, width, rgb, storage.clone()));
            }
        }
        if named.is_empty() {
            continue;
        }

        let mut placements = 0usize;
        let mut others: BTreeMap<u32, usize> = BTreeMap::new();
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance { .. } = &entity.kind else {
                continue;
            };
            let resolved = entity
                .source
                .stream_path
                .as_ref()
                .zip(entity.graphic_oid)
                .and_then(|(stream, oid)| styles.get(&(stream.clone(), oid)));
            let Some(resolved) = resolved else { continue };
            if named.iter().any(|(id, ..)| *id == resolved.style_id) {
                placements += 1;
            } else {
                *others.entry(resolved.style_id).or_default() += 1;
            }
        }
        for (id, width, rgb, storage) in &named {
            *ids.entry(*id).or_default() += 1;
            let leaf = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            println!(
                "{leaf:<44} {id:>5} {width:>9.4} {:>8} {placements:>10} {:>10}",
                format!("{rgb:?}"),
                others.len()
            );
            if storage != "/" {
                println!("     (in storage {storage})");
            }
        }
    }
    println!("\nthe id the name lands on, across drawings: {ids:?}");
    Ok(())
}
