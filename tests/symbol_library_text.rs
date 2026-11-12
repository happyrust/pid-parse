//! Text records read off the bundled `.sym` library: one label of each kind
//! the reader refused before it asked the `igTextBox` shape table, and a
//! ratchet on how many text records the library still refuses.
//!
//! The library is read out of the archives it is committed in
//! (`tests/support/library_symbols.rs`), so these run on every checkout.
//! Until 2026-11-12 they read `test-file/symbols-full`, a directory only a
//! machine that had unpacked the library holds, and returned without a word
//! where it was absent.

#[path = "support/library_symbols.rs"]
mod library_symbols;

use pid_parse::symbol_library::{read_symbol_geometry_from_bytes, SymbolPrimitive};

const EQUIPMENT_LABELS: &str = "Equipment Components/Labels - Equipment Components";
const INSTRUMENT_LABELS: &str = "Instrumentation/Labels - General Instrument";
const PIPING_LABELS: &str = "Piping/Labels - Piping Components";

/// Where the label templates of `Fail Action.sym` and `Red Nominal
/// Diameter.sym` both sit, in metres: 4.25 in by 5.5 in.
const DEFAULT_LABEL_AT: (f64, f64) = (0.10795, 0.1397);

/// Every text a symbol's body carries, with its insertion point; `None`,
/// after a `skipping: ` line, where the symbol is not in reach.
fn texts(folder: &str, file: &str) -> Option<Vec<(String, (f64, f64))>> {
    let bytes = library_symbols::library_symbol(&format!("{folder}/{file}"))?;
    let body = read_symbol_geometry_from_bytes(&bytes).expect("a readable .sym");
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
///
/// Every symbol of the library is read, and the walk says so: a walk that
/// found no file, or left one unread, used to pass on the bound alone.
#[test]
fn the_bundled_library_refuses_no_text_record_but_the_empty_ones() {
    let Some(symbols) = library_symbols::every_library_symbol() else {
        return;
    };
    let count = symbols.len();
    assert_eq!(
        count,
        library_symbols::LIBRARY_SYMBOLS,
        "the symbols the library holds"
    );
    let mut refused = 0usize;
    let mut read = 0usize;
    let mut unreadable: Vec<&str> = Vec::new();
    for (name, bytes) in &symbols {
        let Ok(body) = read_symbol_geometry_from_bytes(bytes) else {
            unreadable.push(name);
            continue;
        };
        refused += body.skipped_records.get(&0x004D).copied().unwrap_or(0);
        read += body
            .primitives
            .iter()
            .filter(|styled| matches!(styled.primitive, SymbolPrimitive::Text { .. }))
            .count();
    }
    assert!(
        unreadable.is_empty(),
        "symbols that are not readable compound files: {unreadable:?}"
    );
    assert!(
        refused <= 2,
        "{refused} text records refused across {count} files"
    );
    eprintln!("symbol library: {count} symbols, {read} text records read, {refused} refused");
}
