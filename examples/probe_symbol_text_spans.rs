//! Which library labels are lettered in more than one style, and in what.
//!
//! A `.sym` text record carries a run partition (`igTextBox` runs), and
//! `symbol_library::SymbolLettering` now states it as `spans` — one entry per
//! character-style run, with its own size, face and colour. That partition is
//! only worth drawing when a run differs from the label's own single style:
//! most of the library's partitions are per-character style ids that resolve
//! to the same size and face, which a renderer merges back into one run.
//!
//! This probe counts both, and separates the labels that are pure `NULL`
//! template placeholders — those are never drawn (the plot drops them), so a
//! partition inside one can never reach a page.
//!
//! ```text
//! cargo run --example probe_symbol_text_spans -- [--root DIR]...
//! ```
//!
//! Read-only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{read_symbol_geometry, SymbolLettering, SymbolPrimitive};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// The library roots pid-plot searches, in the same order: the corpus's own
/// `Ref/*symbol*` directories, then the library bundled with this repository.
fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for candidate in [
        Some(corpus.join("Ref")),
        corpus.parent().map(|p| p.join("Ref")),
    ]
    .into_iter()
    .flatten()
    {
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
        for root in found {
            if !roots.iter().any(|seen| seen == &root) {
                roots.push(root);
            }
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() && !roots.iter().any(|seen| seen == &bundled) {
        roots.push(bundled);
    }
    roots
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

/// The plot's own rule: whitespace removed, one or more `NULL`s and nothing
/// else. Such a label is a template placeholder and is never inked.
fn is_null_placeholder(text: &str) -> bool {
    let collapsed: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    let mut rest = collapsed.as_str();
    if rest.is_empty() {
        return false;
    }
    while let Some(next) = rest.strip_prefix("NULL") {
        rest = next;
    }
    rest.is_empty()
}

/// Whether a partition states anything the label's own single style does not.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
struct Change {
    height: bool,
    face: bool,
}

impl Change {
    fn none(self) -> bool {
        !self.height && !self.face
    }

    fn label(self) -> &'static str {
        match (self.height, self.face) {
            (false, false) => "same size and face (merged back into one run)",
            (true, false) => "another size",
            (false, true) => "another face",
            (true, true) => "another size and face",
        }
    }
}

#[derive(Default)]
struct Tally {
    symbols: usize,
    labels: usize,
    partitioned: usize,
    partitioned_placeholder: usize,
    /// Per change, counted over the inked labels only.
    changes: BTreeMap<&'static str, usize>,
    /// One line per inked label whose partition really changes how it letters.
    samples: Vec<String>,
}

fn change_of(lettering: &SymbolLettering) -> Change {
    let own_font = lettering
        .font_name
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let own_height = lettering.height_m.unwrap_or_default();
    let mut change = Change::default();
    for span in &lettering.spans {
        if span
            .height_m
            .is_some_and(|height| (height - own_height).abs() > 1e-9)
        {
            change.height = true;
        }
        if span
            .font_name
            .as_deref()
            .is_some_and(|face| face.to_lowercase() != own_font)
        {
            change.face = true;
        }
    }
    change
}

fn main() {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                if let Some(root) = args.next() {
                    roots.push(PathBuf::from(root));
                }
            }
            other => roots.push(PathBuf::from(other)),
        }
    }
    if roots.is_empty() {
        roots = library_roots(&corpus_dir());
    }
    let mut symbols = Vec::new();
    for root in &roots {
        println!("root {}", root.display());
        collect_syms(root, &mut symbols);
    }

    let mut tally = Tally {
        symbols: symbols.len(),
        ..Tally::default()
    };
    for path in &symbols {
        let Ok(geometry) = read_symbol_geometry(path) else {
            continue;
        };
        for styled in &geometry.primitives {
            let Some(lettering) = &styled.lettering else {
                continue;
            };
            tally.labels += 1;
            if lettering.spans.is_empty() {
                continue;
            }
            tally.partitioned += 1;
            let SymbolPrimitive::Text { text, .. } = &styled.primitive else {
                continue;
            };
            if is_null_placeholder(text) {
                tally.partitioned_placeholder += 1;
                continue;
            }
            let change = change_of(lettering);
            *tally.changes.entry(change.label()).or_default() += 1;
            if !change.none() && tally.samples.len() < 40 {
                let spans: Vec<String> = lettering
                    .spans
                    .iter()
                    .map(|span| {
                        format!(
                            "{}x#{} {} {}",
                            span.len,
                            span.style_id,
                            span.height_m.map_or("?".to_string(), |height| format!(
                                "{:.4} mm",
                                height * 1000.0
                            )),
                            span.font_name.as_deref().unwrap_or("?")
                        )
                    })
                    .collect();
                tally.samples.push(format!(
                    "{:?} {:?} own[{:?} {}] runs[{}]",
                    path.file_name().unwrap_or_default(),
                    text,
                    lettering.height_m.map(|h| h * 1000.0),
                    lettering.font_name.as_deref().unwrap_or("?"),
                    spans.join(" | ")
                ));
            }
        }
    }

    println!("\n== library labels (a `.sym` text record per label) ==");
    println!("  symbols {}  labels {}", tally.symbols, tally.labels);
    println!(
        "  with a run partition {} ({} of them pure `NULL` placeholders, never inked)",
        tally.partitioned, tally.partitioned_placeholder
    );
    println!(
        "  inked labels with a partition: {}",
        tally.partitioned - tally.partitioned_placeholder
    );
    println!("  what the partition states beyond the label's own style:");
    for change in [
        "same size and face (merged back into one run)",
        "another size",
        "another face",
        "another size and face",
    ] {
        println!(
            "    {:<52} {}",
            change,
            tally.changes.get(change).copied().unwrap_or_default()
        );
    }
    println!("\n== inked labels the partition really letters differently ==");
    for line in &tally.samples {
        println!("  {line}");
    }
}
