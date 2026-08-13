//! Where, if anywhere, does an `igTextBox` record carry a rotation angle?
//!
//! `OpenCADStudio` letters every `.pid` label horizontally because the
//! `IgTextBoxEmitter` hard-codes `rotation: 0.0` — the decoder resolves an
//! insertion point and text but no angle. Labels that run up a vertical pipe
//! come in lying flat. Before wiring an angle we have to find one, and prove
//! it, and this probe is the "find" half: it measures the record's own bytes
//! for a plausible angle field, on the accepted records across the corpus.
//!
//! The shipped payload layout (see `decode_igtextbox_payload`) is:
//!
//! ```text
//!   0..31    head (oid, aux pair, sub_type, index, 12 undecoded, +30 length)
//!   32..     UTF-16LE text, `text_length` code units
//!   text_end +0..7   f64  trailing_double_1  (insertion.x)
//!   text_end +8..15  f64  trailing_double_2  (insertion.y)
//!   text_end +16..23 f64  trailing_double_3  ("often 1.0" — scale/marker)
//!   text_end +24..35 12 bytes, undecoded
//! ```
//!
//! A rotation angle would read as a finite double in `[0, 2π)` (or degrees in
//! `[0, 360)`) that is non-zero on at least some labels. The candidates are:
//! `trailing_double_3`, the 8 bytes at `text_end+24` read as an f64, and the
//! 12 undecoded head bytes at `+18`. This prints each candidate's
//! distribution so the reader can see whether any of them varies the way an
//! angle would, or whether the angle is not in the record at all (in which
//! case it is on the style chain, like character height —
//! `style_link::text_heights_for_file`).
//!
//! ```powershell
//! cargo run --example probe_igtextbox_rotation_candidate
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// Payload offset where the text begins.
const TEXT_START: usize = 32;

fn sheet_streams(path: &Path) -> Vec<Vec<u8>> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push(bytes);
        }
    }
    out
}

/// A rounded bucket key for a double, so near-identical values group.
/// Scientific notation so a noise-level value near zero does not read as a
/// clean `0.0000`.
fn bucket(value: f64) -> String {
    if !value.is_finite() {
        return "non-finite".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    format!("{:.3e}", value)
}

/// Would this value read as a rotation angle? An angle is continuous, so it
/// must be finite, non-zero, below `2π`, and *not* one of the discrete
/// `{±1}` markers a scale/flag field takes. A near-zero noise value is
/// excluded by the lower bound.
fn is_angle_like(value: f64) -> bool {
    value.is_finite()
        && value.abs() > 1e-4
        && value.abs() < std::f64::consts::TAU
        && (value.abs() - 1.0).abs() > 1e-4
}

fn f64_at(payload: &[u8], at: usize) -> Option<f64> {
    let chunk = payload.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
    ]))
}

fn main() {
    let mut total = 0usize;
    let mut d3: BTreeMap<String, usize> = BTreeMap::new();
    // f64 read from the 12 undecoded bytes at text_end+24.
    let mut d4: BTreeMap<String, usize> = BTreeMap::new();
    // How many labels have a non-zero value in each candidate — an angle
    // field must be non-zero on at least the rotated labels.
    let mut d3_nonzero = 0usize;
    let mut d4_nonzero = 0usize;
    // Values that would be a plausible angle: finite, |v| < 2π+ε, non-zero.
    let mut d3_angle_like = 0usize;
    let mut d4_angle_like = 0usize;
    // The pair hypothesis: (d3, d4) as a unit direction vector.
    let mut pair_unit = 0usize;
    let mut pair_angles: BTreeMap<i64, usize> = BTreeMap::new();
    let mut off_unit_samples: Vec<String> = Vec::new();
    // Text quality: a label that over-reads past its end picks up the binary
    // that follows, which shows up as control characters.
    let mut text_has_controls = 0usize;
    let mut controls_and_off_unit = 0usize;

    for fixture in FIXTURES {
        for bytes in sheet_streams(Path::new(fixture)) {
            for at in sheet_record_starts(&bytes) {
                let Some(raw) = bytes.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(record) = decode_igtextbox_at(&bytes, at) else {
                    continue;
                };
                total += 1;
                let dirty = record.text.chars().any(char::is_control);
                if dirty {
                    text_has_controls += 1;
                }

                let payload_start = record.byte_range.start + PSM_ENVELOPE_LEN;
                let payload = &bytes[payload_start..record.byte_range.end];
                let text_end = TEXT_START + 2 * record.text_length as usize;

                *d3.entry(bucket(record.trailing_double_3)).or_default() += 1;
                if record.trailing_double_3 != 0.0 {
                    d3_nonzero += 1;
                }
                if is_angle_like(record.trailing_double_3) {
                    d3_angle_like += 1;
                }

                if let Some(v) = f64_at(payload, text_end + 24) {
                    *d4.entry(bucket(v)).or_default() += 1;
                    if v != 0.0 {
                        d4_nonzero += 1;
                    }
                    if is_angle_like(v) {
                        d4_angle_like += 1;
                    }

                    // (d3, d4) as a direction vector: unit length is the test
                    // that separates "two unrelated flags" from "one angle".
                    let (cos, sin) = (record.trailing_double_3, v);
                    if (cos.hypot(sin) - 1.0).abs() < 1e-6 {
                        pair_unit += 1;
                        let degrees = sin.atan2(cos).to_degrees().round() as i64;
                        *pair_angles.entry(degrees).or_default() += 1;
                    } else {
                        if dirty {
                            controls_and_off_unit += 1;
                        }
                        off_unit_samples.push(format!(
                            "len={:.3e} cos={cos:.3e} sin={sin:.3e} text={:?}",
                            cos.hypot(sin),
                            record.text.chars().take(24).collect::<String>()
                        ));
                    }
                }
            }
        }
    }

    println!("accepted igTextBox records across corpus: {total}\n");

    println!("=== the pair hypothesis: (text_end+16, text_end+24) as (cos, sin) ===");
    println!("Neither slot is an angle on its own -- both take only {{0, +-1}}. But a");
    println!("rotation can be stored as a direction vector instead of an angle, and");
    println!("their counts are complementary. If that is what they are, every record");
    println!("satisfies d3^2 + d4^2 == 1 and the pair names an angle.");
    println!("{:>10} {:>10}  angles seen (deg x count)", "unit", "off-unit");
    println!(
        "{:>10} {:>10}  {}",
        pair_unit,
        total - pair_unit,
        pair_angles
            .iter()
            .map(|(deg, count)| format!("{deg}:{count}"))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!(
        "\n  text quality: {text_has_controls}/{total} accepted records carry control\n  \
         characters, i.e. the stated length over-reads past the label into binary.\n  \
         Of the {} off-unit records, {controls_and_off_unit} are such records -- the unit test\n  \
         and the text test agree on which lengths are wrong.",
        total - pair_unit
    );
    if !off_unit_samples.is_empty() {
        println!("\n  the off-unit records:");
        for sample in &off_unit_samples {
            println!("    {sample}");
        }
    }
    println!();

    println!("=== trailing_double_3 (text_end+16) ===");
    println!("non-zero: {d3_nonzero}/{total}   angle-like (finite, !=0, <2pi): {d3_angle_like}");
    for (value, count) in &d3 {
        println!("  {value:>12} : {count}");
    }

    println!("\n=== f64 at text_end+24 (first 8 of the 12 undecoded tail bytes) ===");
    println!("non-zero: {d4_nonzero}/{total}   angle-like (finite, !=0, <2pi): {d4_angle_like}");
    for (value, count) in &d4 {
        println!("  {value:>12} : {count}");
    }

    println!(
        "\nReading: a rotation field must be non-zero on the rotated labels and \n\
         zero on the flat ones. A candidate that is 1.0 everywhere is a scale or \n\
         marker, not an angle; one that is 0.0 everywhere is padding or absent, \n\
         which points at the style chain (like character height) rather than the \n\
         record."
    );
}
