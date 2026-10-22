//! Which sheet a bundled `.sym` body is drawn from. A stroke a clean-view sheet
//! shares with a twin stays on the clean sheet, so the sheet the pick lands on
//! draws the body together with its own lettering.
//!
//! Soft-skips where `test-file/symbols-full` is absent.

use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};

const SOLENOID_VALVES: &str = "Instrumentation/Off-Line/Without Implied Components/Solenoid Valves";
const SLOPE_LABELS: &str = "Piping/Labels - Piping Segments";

fn library_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full")
}

/// What a body draws: how many displayed primitives are geometry, and the
/// displayed lettering. `None` where the file is not in this checkout.
fn displayed(folder: &str, file: &str) -> Option<(usize, Vec<String>)> {
    let path = library_root().join(folder).join(file);
    if !path.is_file() {
        return None;
    }
    let body = read_symbol_geometry(&path).expect("a readable .sym");
    let mut geometry = 0;
    let mut lettering = Vec::new();
    for styled in body.displayed_primitives() {
        match &styled.primitive {
            SymbolPrimitive::Text { text, .. } => lettering.push(text.clone()),
            _ => geometry += 1,
        }
    }
    Some((geometry, lettering))
}

/// The four solenoid valves keep their `S` alone on the clean `Sheet6` once a
/// twin sorting ahead of it by stream name (`Sheet46`, `Sheet45`, `Sheet129`,
/// `Sheet48`) takes the shared body; they drew nothing but the `S` until the
/// clean sheet kept the strokes it shares.
#[test]
fn a_solenoid_valve_draws_its_body_beside_its_s() {
    let cases = [
        ("2-way Valve.sym", 9),
        ("3-way Valve.sym", 12),
        ("4-way Double Valve.sym", 18),
        ("4-way Single Valve.sym", 13),
    ];
    for (file, strokes) in cases {
        let Some((geometry, lettering)) = displayed(SOLENOID_VALVES, file) else {
            continue;
        };
        assert_eq!(geometry, strokes, "{file}: lettering {lettering:?}");
        assert!(
            lettering.iter().any(|text| text == "S"),
            "{file}: {lettering:?}"
        );
    }
}

/// Both slope-label sheets are clean: `Sheet37` holds the 11 strokes and
/// `Sheet6` the `SLOPE` lettering, and the pick keeps both. Counting geometry
/// alone would drop the word.
#[test]
fn a_slope_label_keeps_its_lettering_beside_its_strokes() {
    for file in [
        "Slope Dnwrd (Angle).sym",
        "Slope Dnwrd (Rise & Run).sym",
        "Slope Upward (Angle).sym",
        "Slope Upward (Rise & Run).sym",
    ] {
        let Some((geometry, lettering)) = displayed(SLOPE_LABELS, file) else {
            continue;
        };
        assert_eq!(geometry, 11, "{file}: lettering {lettering:?}");
        assert!(
            lettering.iter().any(|text| text == "SLOPE"),
            "{file}: {lettering:?}"
        );
    }
}
