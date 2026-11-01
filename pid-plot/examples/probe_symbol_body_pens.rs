//! What pen does each symbol body declare for itself?
//!
//! `body_stroke` lets a placement's style paint the whole body, and the
//! reference plot confirms that on the process symbols. The disagreements all
//! sit on small annotation marks, so this asks whether their bodies declare a
//! thinner pen than the placement does — the one difference that would explain
//! both readings without a new rule.
//!
//! ```text
//! cargo run -p pid-plot --example probe_symbol_body_pens
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_parse::geometry::build_normalized_geometry;
use pid_parse::symbol_library::SymbolLibrary;
use pid_parse::{ParseOptions, PidParser};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn roots(corpus: &std::path::Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for candidate in [corpus.join("Ref")] {
        let Ok(entries) = std::fs::read_dir(&candidate) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| {
                entry.path().is_dir()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains("symbol")
            })
            .map(|entry| entry.path())
            .collect();
        found.sort();
        roots.extend(found);
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    roots
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus_dir();
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "I井场 注采阀组工艺及自控流程图".to_string());
    let path = corpus.join(format!("{name}.pid"));
    let doc = PidParser::with_options(ParseOptions::geometry()).parse_file(&path)?;
    let geometry = build_normalized_geometry(&doc);

    let mut library = SymbolLibrary::with_roots(roots(&corpus));
    println!("== {}", path.display());

    // Every symbol path this page places, and the widths its body declares.
    let mut seen: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for entity in &geometry.entities {
        let pid_parse::geometry::PidGraphicKind::SymbolInstance { symbol_path, .. } = &entity.kind
        else {
            continue;
        };
        let Some(symbol_path) = symbol_path else {
            continue;
        };
        let leaf = symbol_path
            .rsplit('\\')
            .next()
            .unwrap_or(symbol_path)
            .to_string();
        if seen.contains_key(&leaf) {
            continue;
        }
        let Some(body) = library.resolve(symbol_path) else {
            println!("\n  {leaf}: not in the library");
            continue;
        };
        let mut widths: BTreeMap<String, usize> = BTreeMap::new();
        for styled in body.displayed_primitives() {
            let label = match &styled.style {
                Some(style) => format!("{:.4} mm {:?}", style.width_mm, style.rgb),
                None => "no style".to_string(),
            };
            *widths.entry(label).or_default() += 1;
        }
        seen.insert(leaf.clone(), widths);
    }
    for (leaf, widths) in &seen {
        println!("\n  {leaf}");
        for (width, count) in widths {
            println!("     {count:>3} primitive(s) declare {width}");
        }
    }
    Ok(())
}
