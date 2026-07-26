//! Probe whether "rotated / styled text" lives in nested JSite label
//! sheets rather than top-level `igTextBox` records (Phase 35-D,
//! third iteration).
//!
//! Hypothesis from the pairing probe: rotated `JStyleOverride`
//! records (rot = pi/2, 3pi/2) anchor in regions where no top-level
//! `igTextBox` exists. If pipe labels are label JSites, the text is
//! stored in a *nested* `JSite<n>/Sheet6` stream in local
//! coordinates, and the page placement (including rotation) comes
//! from the top-level `igSymbol2d` matrix.
//!
//! For each fixture this probe prints:
//! 1. nested sheets that contain `igTextBox` records (path, texts);
//! 2. top-level `igSymbol2d` records with their `jsite_ref`,
//!    insertion, and rotation angle recovered from the matrix;
//! 3. the cross-check: nested label texts whose owning JSite is
//!    placed with a non-zero rotation, plus how close that placement
//!    sits to a rotated `JStyleOverride` anchor.
//!
//! Read-only: no parser, schema, or model change.

use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igsymbols, decode_igtextboxes, decode_jstyle_overrides,
};
use pid_parse::PidPackage;

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

fn f64_from_u32_pair(low: u32, high: u32) -> f64 {
    f64::from_bits((u64::from(high) << 32) | u64::from(low))
}

/// Extract the leading `JSite<n>` id from a nested stream path like
/// `Root Entry/JSite123/Sheet6` (already stripped of the root name by
/// the package layer, so the shape is `JSite123/Sheet6`).
fn jsite_id_of_path(path: &str) -> Option<u32> {
    let first = path.trim_start_matches('/').split('/').next()?;
    let digits = first.strip_prefix("JSite")?;
    digits.parse().ok()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;
        println!("\n=== {rel} ===");

        // 1. Nested sheets containing text.
        let mut labels: Vec<(u32, String, String)> = Vec::new(); // (jsite_id, path, joined text)
        for sheet in &pkg.parsed.sheet_streams {
            if sheet.path.matches('/').count() < 2 {
                continue;
            }
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let texts = decode_igtextboxes(raw.data.as_slice());
            if texts.is_empty() {
                continue;
            }
            let joined = texts
                .iter()
                .map(|t| t.text.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            if let Some(id) = jsite_id_of_path(&sheet.path) {
                labels.push((id, sheet.path.clone(), joined));
            }
        }
        println!("  nested sheets with text: {}", labels.len());
        for (id, path, joined) in labels.iter().take(12) {
            println!("    JSite{id} ({path}): {joined:?}");
        }

        // 2. Top-level igSymbol2d placements + rotated style anchors.
        for sheet in &pkg.parsed.sheet_streams {
            if sheet.path.matches('/').count() != 1 {
                continue;
            }
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let symbols = decode_igsymbols(bytes);
            let styles = decode_jstyle_overrides(bytes);
            if symbols.is_empty() {
                continue;
            }
            let rotated_styles: Vec<(f64, f64, f64)> = styles
                .iter()
                .filter(|s| s.field_2_f64.abs() > 1e-9)
                .map(|s| {
                    (
                        f64_from_u32_pair(s.field_a_u32, s.field_b_u32),
                        f64_from_u32_pair(s.field_c_u32, s.field_d_u32),
                        s.field_2_f64,
                    )
                })
                .collect();

            println!(
                "  {}: {} igSymbol2d, {} rotated styles",
                sheet.path,
                symbols.len(),
                rotated_styles.len()
            );
            for sym in &symbols {
                // transform is a row-major 2x2 matrix [m00, m01, m10, m11];
                // the rotation angle of the placed x basis vector is
                // atan2(m10, m00).
                let angle = sym.transform[2].atan2(sym.transform[0]);
                let is_label = labels.iter().find(|(id, _, _)| *id == sym.jsite_ref);
                let near_style = rotated_styles
                    .iter()
                    .map(|(sx, sy, rot)| {
                        let d = (sym.insertion.0 - sx).hypot(sym.insertion.1 - sy);
                        (d, *rot)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if is_label.is_some() || angle.abs() > 1e-6 {
                    println!(
                        "    sym oid={} jsite_ref={} ins=({:.4}, {:.4}) angle={:.4}{}{}",
                        sym.oid,
                        sym.jsite_ref,
                        sym.insertion.0,
                        sym.insertion.1,
                        angle,
                        match is_label {
                            Some((_, _, text)) => format!("  LABEL text={text:?}"),
                            None => String::new(),
                        },
                        match near_style {
                            Some((d, rot)) if d < 0.02 =>
                                format!("  near rotated style (d={d:.4}, rot={rot:.4})"),
                            _ => String::new(),
                        },
                    );
                }
            }
        }
    }
    Ok(())
}
