//! Do the duplicated paper strokes come from placements that coincide?
//!
//! `probe_page_defects` counts a stroke twice when two primitives carry the same
//! shape and pen. Inside a symbol body that can happen two ways: the drawing
//! places the same symbol twice at one point (two records, one transform), or
//! the page expands one placement twice. Both leave the same byte range on both
//! copies, so the audit cannot tell them apart — this probe puts the two
//! readings side by side per symbol: stroke duplicates on the page against the
//! placements the source stores twice at one point.
//!
//! A column with strokes but no coincident placement (or the reverse) names
//! which one it is.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_placements_coincide -- <dir|drawing.pid>...
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use pid_parse::geometry::{PidGraphicKind, build_normalized_geometry};
use pid_parse::{ParseOptions, PidParser};
use pid_plot::model::lay_out_drawing;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn drawings(path: &Path) -> Vec<PathBuf> {
    if path.is_dir() {
        let mut found: Vec<PathBuf> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pid"))
            })
            .collect();
        found.sort();
        found
    } else {
        vec![path.to_path_buf()]
    }
}

/// The symbol a stroke belongs to, as the record spells it.
fn stem(path: &str) -> String {
    Path::new(path).file_stem().map_or_else(
        || path.to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: probe_which_placements_coincide <dir|drawing.pid>...");
        return Ok(());
    }
    let corpus = corpus_dir();
    let mut roots: Vec<PathBuf> = Vec::new();
    for arg in &args {
        roots.extend(drawings(Path::new(arg)));
    }
    let mut pages = 0usize;
    let mut settled = 0usize;
    let mut unexplained_duplicates = 0usize;
    for path in &roots {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let Ok(model) = lay_out_drawing(path, &corpus) else {
            println!("{name}|layout failed");
            continue;
        };
        let Ok(doc) = PidParser::with_options(ParseOptions::geometry()).parse_file(path) else {
            println!("{name}|parse failed");
            continue;
        };
        pages += 1;
        let geometry = build_normalized_geometry(&doc);

        // Page side: per symbol, how many strokes the page draws twice. The
        // shape's own `Debug` is the fingerprint — `f64` round-trips through it
        // exactly, so two strokes share a key only when every coordinate agrees.
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut duplicate_by_symbol: BTreeMap<String, usize> = BTreeMap::new();
        for primitive in &model.primitives {
            let pen = primitive.stroke.as_ref().map_or_else(
                || "none".to_string(),
                |stroke| format!("{:.4}/{:?}", stroke.width_mm, stroke.dash_mm),
            );
            let key = format!("{:?}|{pen}", primitive.shape);
            let count = seen.entry(key).or_insert(0);
            *count += 1;
            if *count > 1
                && let Some(symbol) = primitive.symbol.as_deref()
            {
                *duplicate_by_symbol.entry(stem(symbol)).or_default() += 1;
            }
        }

        // Source side: placements the drawing stores twice at one point, keyed
        // to the micrometre and to the transform's own bits.
        let mut placements: HashMap<(String, i64, i64, u64, u64, u64), usize> = HashMap::new();
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                rotation,
                scale,
                ..
            } = &entity.kind
            else {
                continue;
            };
            let Some(symbol_path) = symbol_path else {
                continue;
            };
            let key = (
                stem(symbol_path),
                (insertion.x * 1e6).round() as i64,
                (insertion.y * 1e6).round() as i64,
                rotation.to_bits(),
                scale[0].to_bits(),
                scale[1].to_bits(),
            );
            *placements.entry(key).or_default() += 1;
        }
        let mut extra_by_symbol: BTreeMap<String, usize> = BTreeMap::new();
        for ((symbol, ..), count) in &placements {
            if *count > 1 {
                *extra_by_symbol.entry(symbol.clone()).or_default() += count - 1;
            }
        }

        let mut symbols: BTreeSet<&str> = BTreeSet::new();
        symbols.extend(duplicate_by_symbol.keys().map(String::as_str));
        symbols.extend(extra_by_symbol.keys().map(String::as_str));
        let total_duplicates: usize = duplicate_by_symbol.values().sum();
        if symbols.is_empty() {
            println!(
                "{name}: {} primitives, no symbol strokes duplicated",
                model.primitives.len()
            );
            continue;
        }
        println!(
            "{name}: {} primitives, {total_duplicates} duplicated symbol strokes",
            model.primitives.len()
        );
        for symbol in symbols {
            let duplicates = duplicate_by_symbol.get(symbol).copied().unwrap_or(0);
            let extra = extra_by_symbol.get(symbol).copied().unwrap_or(0);
            let verdict = match (duplicates, extra) {
                (0, _) => "coincident placements, no duplicated stroke",
                (_, 0) => "duplicated strokes, NO coincident placement",
                _ => "both",
            };
            println!(
                "    {symbol:<40} dup {duplicates:>4}  extra placements {extra:>2}  {verdict}"
            );
            if duplicates > 0 {
                if extra == 0 {
                    unexplained_duplicates += duplicates;
                } else {
                    settled += duplicates;
                }
            }
        }
    }
    println!(
        "\n{pages} pages: {settled} duplicated symbol strokes sit under a coincident placement, \
{unexplained_duplicates} do not"
    );
    Ok(())
}
