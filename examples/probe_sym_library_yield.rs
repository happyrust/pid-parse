//! Phase 36-D: acceptance run for [`pid_parse::symbol_library`].
//!
//! Reads every `.sym` in the local reference library through the shipped
//! decoder and reports what comes out: how many symbols yield a body, the
//! primitive mix, and which type codes were stepped over. Then it replays
//! the symbol paths the drawing fixtures actually place, so the number that
//! matters -- how many placements can now be drawn rather than marked -- is
//! measured against real drawings instead of the library alone.
//!
//! Read-only.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{read_symbol_geometry, SymbolLibrary, SymbolPrimitive};
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
];

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

/// Where to read the library from: the arguments, else `PID_SYMBOL_LIBRARY`
/// (the same override the importer honours), else the unpacked corpus, else
/// the two committed samples.
///
/// Several roots are allowed because the fixtures come from different
/// projects citing different reference shares, and a machine normally holds
/// a partial copy of each rather than one merged tree.
fn symbols_roots() -> Vec<PathBuf> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if !args.is_empty() {
        return args;
    }
    if let Some(env) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        return std::env::split_paths(&env).collect();
    }
    let full = test_file_root().join("symbols-full");
    if full.is_dir() {
        vec![full]
    } else {
        vec![test_file_root().join("symbols")]
    }
}

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let roots = symbols_roots();
    let mut syms = Vec::new();
    for root in &roots {
        collect_syms(root, &mut syms);
    }
    syms.sort();
    println!(
        "library: {} symbols under {} root(s)",
        syms.len(),
        roots.len()
    );
    for root in &roots {
        println!("  {}", root.display());
    }

    let (mut lines, mut circles, mut arcs, mut polylines) = (0usize, 0usize, 0usize, 0usize);
    let mut empty = 0usize;
    let mut failed = 0usize;
    let mut skipped: BTreeMap<u16, usize> = BTreeMap::new();
    let mut richest: Vec<(usize, String)> = Vec::new();

    for sym in &syms {
        let Ok(body) = read_symbol_geometry(sym) else {
            failed += 1;
            continue;
        };
        if body.primitives.is_empty() {
            empty += 1;
        }
        for primitive in &body.primitives {
            match primitive {
                SymbolPrimitive::Line { .. } => lines += 1,
                SymbolPrimitive::Circle { .. } => circles += 1,
                SymbolPrimitive::Arc { .. } => arcs += 1,
                SymbolPrimitive::Polyline { .. } => polylines += 1,
            }
        }
        for (code, count) in &body.skipped_records {
            *skipped.entry(*code).or_default() += count;
        }
        let name = roots
            .iter()
            .find_map(|root| sym.strip_prefix(root).ok())
            .unwrap_or(sym.as_path())
            .display()
            .to_string();
        richest.push((body.primitives.len(), name));
    }

    let total = lines + circles + arcs + polylines;
    println!("\n-- library yield --");
    println!("  read failures      : {failed}");
    println!(
        "  symbols with a body: {} / {}",
        syms.len() - empty - failed,
        syms.len()
    );
    println!("  empty bodies       : {empty}");
    println!("  primitives         : {total}");
    println!("    lines     {lines}");
    println!("    circles   {circles}");
    println!("    arcs      {arcs}");
    println!("    polylines {polylines}");

    println!("\n  stepped-over record types (no drawable shape):");
    let mut rows: Vec<(&u16, &usize)> = skipped.iter().collect();
    rows.sort_by_key(|(_, count)| Reverse(**count));
    for (code, count) in rows.iter().take(8) {
        println!("    {code:#06x}  {count}");
    }

    richest.sort_by_key(|(count, _)| Reverse(*count));
    println!("\n  richest symbols:");
    for (count, name) in richest.iter().take(5) {
        println!("    {count:>4}  {name}");
    }

    println!("\n-- placements in the drawing fixtures --");
    let mut library = SymbolLibrary::with_roots(roots.clone());
    let (mut placed, mut drawable, mut drawable_primitives) = (0usize, 0usize, 0usize);
    let mut unnamed = 0usize;

    for fixture in FIXTURES {
        let path = test_file_root().join(fixture);
        if !path.is_file() {
            println!("  {fixture}: missing, skipped");
            continue;
        }
        let Ok(parsed) = PidParser::new().parse_file(&path) else {
            println!("  {fixture}: parse failed");
            continue;
        };
        let geometry = build_normalized_geometry(&parsed);
        let (mut n, mut ok, mut prims) = (0usize, 0usize, 0usize);
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance { symbol_path, .. } = &entity.kind else {
                continue;
            };
            n += 1;
            let Some(symbol_path) = symbol_path else {
                unnamed += 1;
                continue;
            };
            if let Some(body) = library.resolve(symbol_path) {
                if !body.primitives.is_empty() {
                    ok += 1;
                    prims += body.primitives.len();
                }
            }
        }
        placed += n;
        drawable += ok;
        drawable_primitives += prims;
        let name = Path::new(fixture)
            .file_name()
            .map_or(*fixture, |n| n.to_str().unwrap_or(*fixture));
        println!(
            "  {name:<32} placements={n:<4} drawable={ok:<4} ({:.0}%)  primitives={prims}",
            100.0 * ok as f64 / n.max(1) as f64
        );
    }

    println!(
        "\n  total: {drawable} / {placed} placements drawable ({:.1}%), {drawable_primitives} primitives",
        100.0 * drawable as f64 / placed.max(1) as f64
    );
    if unnamed > 0 {
        println!("  placements with no library path: {unnamed}");
    }
    println!(
        "  distinct symbols looked up: {} ({} resolved)",
        library.lookups(),
        library.resolved()
    );
    for miss in library.missing() {
        println!("    unresolved: {miss}");
    }

    Ok(())
}
