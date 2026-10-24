//! Text records read off the bundled `.sym` library: one label of each kind
//! the reader refused before it asked the `igTextBox` shape table, and a
//! ratchet on how many text records the library still refuses.
//!
//! Soft-skips where `test-file/symbols-full` is absent.

use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};

const EQUIPMENT_LABELS: &str = "Equipment Components/Labels - Equipment Components";
const INSTRUMENT_LABELS: &str = "Instrumentation/Labels - General Instrument";
const PIPING_LABELS: &str = "Piping/Labels - Piping Components";

/// Where the label templates of `Fail Action.sym` and `Red Nominal
/// Diameter.sym` both sit, in metres: 4.25 in by 5.5 in.
const DEFAULT_LABEL_AT: (f64, f64) = (0.10795, 0.1397);

fn library_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full")
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
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

/// Every text a symbol's body carries, with its insertion point; `None`
/// where the file is not in this checkout.
fn texts(folder: &str, file: &str) -> Option<Vec<(String, (f64, f64))>> {
    let path = library_root().join(folder).join(file);
    if !path.is_file() {
        return None;
    }
    let body = read_symbol_geometry(&path).expect("a readable .sym");
    let found = body
        .primitives
        .into_iter()
        .filter_map(|styled| match styled.primitive {
            SymbolPrimitive::Text { text, at } => Some((text, at)),
            _ => None,
        })
        .collect();
    Some(found)
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

/// Three labels in shape 3 (count at `+26`, formatting runs between the text
/// and the tail) and one in shape 2 with 48 bytes of fields after its tail.
#[test]
fn labels_the_reader_used_to_refuse_read_off_the_bundled_library() {
    let cases = [
        (
            INSTRUMENT_LABELS,
            "Fail Action.sym",
            "CSC NULL",
            Some(DEFAULT_LABEL_AT),
        ),
        (
            PIPING_LABELS,
            "Red Nominal Diameter.sym",
            "1\"x1\"",
            Some(DEFAULT_LABEL_AT),
        ),
        (EQUIPMENT_LABELS, "Nozzle NPD.sym", "1\"", None),
        (EQUIPMENT_LABELS, "Nozzle Rating.sym", "TBD", None),
    ];
    for (folder, file, label, at) in cases {
        let Some(found) = texts(folder, file) else {
            continue;
        };
        let hit = found
            .iter()
            .any(|(text, point)| text == label && at.is_none_or(|want| near(*point, want)));
        assert!(hit, "{file}: no {label:?} at {at:?} among {found:?}");
    }
}

/// The bundled library refuses no text record but the ones stating an empty
/// text: two across both library copies when the shape table landed.
#[test]
fn the_bundled_library_refuses_no_text_record_but_the_empty_ones() {
    let root = library_root();
    if !root.is_dir() {
        return;
    }
    let mut files = Vec::new();
    collect_syms(&root, &mut files);
    let mut refused = 0usize;
    for path in &files {
        let Ok(body) = read_symbol_geometry(path) else {
            continue;
        };
        refused += body.skipped_records.get(&0x004D).copied().unwrap_or(0);
    }
    let count = files.len();
    assert!(
        refused <= 2,
        "{refused} text records refused across {count} files"
    );
}
