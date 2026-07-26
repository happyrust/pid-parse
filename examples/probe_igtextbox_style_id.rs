//! Phase 35-D, fourth iteration: is the `igTextBox` trailer a style id?
//!
//! `probe_igtextbox_field_sweep.rs` established two things:
//!
//! 1. no offset inside `igTextBox` holds a plausible text height on every
//!    record, so the height is *not* stored on the text;
//! 2. the 12-byte trailer is eleven zero bytes and one small non-zero byte
//!    (21 / 56 / 1 across the first fixture), which reads as a big-endian
//!    u32 -- an index, not a measurement.
//!
//! An index has to point somewhere. This probe tests whether it points at
//! `JStyleOverride`, the record that already carries a rotation-shaped f64
//! (`field_2_f64`, clustering on quadrant angles) and, per iteration 2, f64s
//! in the text-height domain inside its attribute tail.
//!
//! Decisive question: are the distinct trailer ids a subset of the style
//! records' oids? If yes, Phase 35-D has the same shape as 35-C -- an
//! in-record id resolving into a side table -- and both height and rotation
//! follow deterministically.
//!
//! Read-only: no parser, schema, or model change.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igtextboxes, decode_jstyle_overrides};
use pid_parse::PidPackage;

// "\u{5DE5}\u{827A}..." is the gongyi fixture (Chinese file name kept as
// escapes so this source stays pure ASCII).
const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
];

const PSM_ENVELOPE_LEN: usize = 6;
const TRAILER_LEN: usize = 12;

const HEIGHT_MIN: f64 = 0.0005;
const HEIGHT_MAX: f64 = 0.02;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn height_candidates(bytes: &[u8]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    for at in 0..bytes.len().saturating_sub(7) {
        let Ok(chunk) = <[u8; 8]>::try_from(&bytes[at..at + 8]) else {
            continue;
        };
        let v = f64::from_le_bytes(chunk);
        if v.is_finite() && (HEIGHT_MIN..=HEIGHT_MAX).contains(&v) {
            out.push((at, v));
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();
    let mut global_hit = 0usize;
    let mut global_ids = 0usize;

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;
        println!("\n=== {rel} ===");

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            if sheet.path.matches('/').count() != 1 {
                continue;
            }
            let bytes = raw.data.as_slice();
            let texts = decode_igtextboxes(bytes);
            let styles = decode_jstyle_overrides(bytes);
            if texts.is_empty() {
                continue;
            }

            // trailer id -> (count, one sample text)
            let mut ids: BTreeMap<u32, (usize, String)> = BTreeMap::new();
            let mut nonzero_prefix = 0usize;
            for rec in &texts {
                let start = rec.byte_range.start + PSM_ENVELOPE_LEN;
                let Some(payload) = bytes.get(start..rec.byte_range.end) else {
                    continue;
                };
                if payload.len() < TRAILER_LEN {
                    continue;
                }
                let trailer = &payload[payload.len() - TRAILER_LEN..];
                if trailer[..8].iter().any(|b| *b != 0) {
                    nonzero_prefix += 1;
                }
                let Ok(last4) = <[u8; 4]>::try_from(&trailer[8..12]) else {
                    continue;
                };
                let id = u32::from_be_bytes(last4);
                let entry = ids.entry(id).or_insert_with(|| (0, String::new()));
                entry.0 += 1;
                if entry.1.is_empty() {
                    entry.1 = rec.text.chars().take(16).collect();
                }
            }

            let style_oids: BTreeSet<u32> = styles.iter().map(|s| s.oid).collect();
            let hit = ids.keys().filter(|id| style_oids.contains(id)).count();
            global_hit += hit;
            global_ids += ids.len();

            println!(
                "\n  sheet {} | texts={} styles={} distinct_trailer_ids={} \
                 trailer_prefix_nonzero={nonzero_prefix}",
                sheet.path,
                texts.len(),
                styles.len(),
                ids.len()
            );
            println!("    trailer ids -> style oid match: {hit}/{}", ids.len());
            for (id, (count, sample)) in &ids {
                let mark = if style_oids.contains(id) {
                    "HIT "
                } else {
                    "miss"
                };
                println!("      id={id:<6} x{count:<4} {mark}  e.g. {sample:?}");
            }

            println!("    styles (oid, rotation-candidate, tail height candidates):");
            for style in styles.iter().take(12) {
                let heights = height_candidates(&style.raw_attribute_tail);
                let shown = heights
                    .iter()
                    .take(4)
                    .map(|(at, v)| format!("@{at}={:.4}mm", v * 1000.0))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "      oid={:<6} f2={:<9.5} f1={:<12.6} tail={:<4} {shown}",
                    style.oid,
                    style.field_2_f64,
                    style.field_1_f64,
                    style.raw_attribute_tail.len()
                );
            }
        }
    }

    println!("\n\n=== verdict ===");
    println!("trailer ids resolving to a JStyleOverride oid: {global_hit}/{global_ids}");
    if global_ids > 0 && global_hit == global_ids {
        println!("=> deterministic link confirmed");
    } else {
        println!("=> not a style-oid pointer; the id indexes something else");
    }

    Ok(())
}
