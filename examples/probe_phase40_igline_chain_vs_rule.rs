//! Phase 40 S5 — what actually guards `igLine2d`, the chain or the rule?
//!
//! S1 measured that 88 chain-resident `igLine2d` records are refused, all of
//! them on one condition: payload `+8..11` (`remaining_header`) is not `12`.
//! The native serializer now says what that field is:
//!
//! `PSMSerializeOut` (`0x56491E80`) writes every PSM record as
//! `type(2) + bytes_to_follow(4) + oid(4) + aux(8)`, and the second writer
//! path spells the same 18-byte header out as a struct
//! (`*v31 = type; +2 = btf; +6 = oid; +10 = aux_lo; +14 = aux_hi`).
//! `aux` is generic envelope, not family payload: it is `(0, 0)` unless a flag
//! on the object record is set, and `PSMSerializeIn` (`0x564915E0`) reads those
//! 8 bytes into a local it never looks at again.
//!
//! So `remaining_header` is `aux_hi` — opaque metadata the native reader
//! discards. `== 12` is this crate's invention, and it is also, by accident,
//! the only thing keeping the family's **sliding byte scan** from accepting
//! junk: unlike `decode_primitive_lines` (chain-gated since `e5ef8fb`),
//! `decode_iglines` walks every offset.
//!
//! This probe measures the swap before anyone makes it — replace the invented
//! field rule with the structural invariant the sibling family already uses:
//!
//! | column | meaning |
//! |---|---|
//! | `scan` | what ships today: sliding scan + `aux_hi == 12` |
//! | `off-chain` | of those, how many start off the record chain |
//! | `chain+rule` | chain-gated, rule kept — the cost of gating |
//! | `chain-rule` | chain-gated, rule dropped — the proposal |
//!
//! ```powershell
//! cargo run --example probe_phase40_igline_chain_vs_rule
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igline_at, decode_iglines, sheet_record_starts};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// PSM type code for `igLine2d` (IGDS class tag `0x18`).
const IGLINE2D: u16 = 0x0018;
/// Fixed `bytes_to_follow` for the family.
const IGLINE2D_PAYLOAD: usize = 50;
/// Same coordinate domain the shipped decoder enforces.
const DOMAIN_LIMIT: f64 = 1e9;

/// Every shipped rule for the family **except** `aux_hi == 12`. Returns the
/// record's `aux_hi` so the caller can see what the dropped rule was testing.
fn lenient_at(bytes: &[u8], off: usize) -> Option<u32> {
    let header_end = off.checked_add(6)?;
    if header_end > bytes.len() {
        return None;
    }
    let raw = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
    if raw & 0x3FFF != IGLINE2D {
        return None;
    }
    let btf = u32::from_le_bytes([
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
    ]) as usize;
    if btf != IGLINE2D_PAYLOAD {
        return None;
    }
    let payload = bytes.get(header_end..header_end.checked_add(IGLINE2D_PAYLOAD)?)?;
    let aux_hi = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);

    let mut d = [0f64; 4];
    for (i, slot) in d.iter_mut().enumerate() {
        let pos = 18 + i * 8;
        let chunk = payload.get(pos..pos + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !d.iter().all(|x| x.is_finite()) {
        return None;
    }
    if d.iter().any(|x| x.abs() > DOMAIN_LIMIT) {
        return None;
    }
    if (d[2] - d[0]).abs() < 1e-12 && (d[3] - d[1]).abs() < 1e-12 {
        return None;
    }
    Some(aux_hi)
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
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
            out.push((name.to_string_lossy().into_owned(), bytes));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn main() {
    let mut totals = [0usize; 4];
    let mut aux_hi_histogram: BTreeMap<u32, usize> = BTreeMap::new();
    let mut off_chain_samples: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        let streams = sheet_streams(path);
        if streams.is_empty() {
            println!("{fixture}: no Sheet streams (missing fixture?)");
            continue;
        }
        println!("\n=== {fixture} ===");
        println!(
            "{:<28} {:>6} {:>10} {:>11} {:>11}",
            "stream", "scan", "off-chain", "chain+rule", "chain-rule"
        );

        for (name, bytes) in streams {
            let chain: BTreeSet<usize> = sheet_record_starts(&bytes).into_iter().collect();

            let scanned = decode_iglines(&bytes);
            let scan = scanned.len();
            let off_chain = scanned
                .iter()
                .filter(|r| !chain.contains(&r.byte_range.start))
                .count();
            for record in &scanned {
                if !chain.contains(&record.byte_range.start) && off_chain_samples.len() < 10 {
                    off_chain_samples.push(format!(
                        "{name} @ 0x{:06X} oid={} start=({:.4}, {:.4})",
                        record.byte_range.start, record.oid, record.start.0, record.start.1
                    ));
                }
            }

            let chain_rule = chain
                .iter()
                .filter(|&&at| decode_igline_at(&bytes, at).is_some())
                .count();
            let mut chain_lenient = 0usize;
            for &at in &chain {
                if let Some(aux_hi) = lenient_at(&bytes, at) {
                    chain_lenient += 1;
                    *aux_hi_histogram.entry(aux_hi).or_default() += 1;
                }
            }

            if scan == 0 && chain_lenient == 0 {
                continue;
            }
            println!("{name:<28} {scan:>6} {off_chain:>10} {chain_rule:>11} {chain_lenient:>11}");
            totals[0] += scan;
            totals[1] += off_chain;
            totals[2] += chain_rule;
            totals[3] += chain_lenient;
        }
    }

    println!("\n=== corpus totals ===");
    println!("scan (ships today)      : {}", totals[0]);
    println!("  of which off-chain    : {}", totals[1]);
    println!("chain-gated, rule kept  : {}", totals[2]);
    println!("chain-gated, rule gone  : {}", totals[3]);
    println!(
        "delta on screen         : {:+}",
        totals[3] as i64 - totals[0] as i64
    );

    println!("\n=== aux_hi (payload +8..11) over chain-resident igLine2d ===");
    for (value, count) in &aux_hi_histogram {
        println!("  aux_hi = {value:>6} : {count:>4} record(s)");
    }

    if off_chain_samples.is_empty() {
        println!("\nno off-chain acceptances — the chain already covers every scan hit");
    } else {
        println!(
            "\noff-chain acceptances (first {}):",
            off_chain_samples.len()
        );
        for sample in &off_chain_samples {
            println!("  {sample}");
        }
    }
}
