//! Phase 34-E corpus scanner — walk every CFB container under a
//! directory and count chain-validated top-level PSM hits for the five
//! curve families that the Sheet-only fixture histogram reports as
//! zero: `0x0059 igCircle2d`, `0x0061 igArc2d`, `0x0063 igEllipse2d`,
//! `0x007E igEllipticalArc2d`, `0x005D igBSplineCurve2d`.
//!
//! Unlike `probe_psm_type_code_histogram`, this walks **all** streams
//! of each container, not just `Sheet*`-named ones. That difference is
//! the Phase 34-E discovery: curve records live in nested
//! `/JSite*\PSMcluster0` streams of the registered `.pid` fixtures, in
//! `/StyleCluster` of the raw `RefData~4~683` container, and in the
//! `Sheet*` streams of `.sym` symbol-definition files inside the
//! backup `RefData~4~681.zip` — none of which the Sheet-filtered
//! histogram scans. See
//! `docs/analysis/2026-07-07-phase34e-missing-geometry-fixture-plan.md`.
//!
//! Read-only. Usage:
//! `cargo run --quiet --example probe_curve_family_corpus_scan -- <dir>`

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

const PSM_HEADER_LEN: usize = 6;
const CURVE_TARGETS: &[(u16, &str)] = &[
    (0x0059, "igCircle2d"),
    (0x0061, "igArc2d"),
    (0x0063, "igEllipse2d"),
    (0x007E, "igEllipticalArc2d"),
    (0x005D, "igBSplineCurve2d"),
];

fn psm_record_end(bytes: &[u8], off: usize) -> Option<usize> {
    let header_end = off.checked_add(PSM_HEADER_LEN)?;
    if header_end > bytes.len() {
        return None;
    }
    let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
    if type_word & 0x3FFF == 0 {
        return None;
    }
    let btf = u32::from_le_bytes([
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
    ]) as usize;
    if !(8..=100_000).contains(&btf) {
        return None;
    }
    let end = header_end.checked_add(btf)?;
    (end <= bytes.len()).then_some(end)
}

fn walk_type_codes(bytes: &[u8]) -> BTreeMap<u16, usize> {
    let mut out: BTreeMap<u16, usize> = BTreeMap::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN <= bytes.len() {
        if let Some(end) = psm_record_end(bytes, off) {
            if end == bytes.len() || psm_record_end(bytes, end).is_some() {
                let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
                *out.entry(type_word & 0x3FFF).or_insert(0) += 1;
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

fn collect_files(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .expect("usage: probe_34e_curve_scan_tmp <dir>");
    let mut files = Vec::new();
    collect_files(&PathBuf::from(&root), &mut files);
    files.sort();

    let mut scanned = 0usize;
    let mut cfb_ok = 0usize;
    let mut curve_files = 0usize;
    let mut curve_totals: BTreeMap<u16, usize> = BTreeMap::new();
    let mut other_totals: BTreeMap<u16, usize> = BTreeMap::new();

    for file in &files {
        scanned += 1;
        let Ok(handle) = std::fs::File::open(file) else {
            continue;
        };
        let Ok(mut cfb) = cfb::CompoundFile::open(handle) else {
            continue;
        };
        cfb_ok += 1;
        let stream_paths: Vec<_> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_path_buf())
            .collect();
        let mut per_file: BTreeMap<u16, usize> = BTreeMap::new();
        let mut curve_streams: Vec<String> = Vec::new();
        for sp in &stream_paths {
            let Ok(mut stream) = cfb.open_stream(sp) else {
                continue;
            };
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let counts = walk_type_codes(&bytes);
            let has_curve = CURVE_TARGETS.iter().any(|(tc, _)| counts.contains_key(tc));
            if has_curve {
                curve_streams.push(format!("{} ({} bytes)", sp.to_string_lossy(), bytes.len()));
            }
            for (tc, n) in counts {
                *per_file.entry(tc).or_insert(0) += n;
            }
        }
        let curve_hits: Vec<String> = CURVE_TARGETS
            .iter()
            .filter_map(|(tc, name)| per_file.get(tc).map(|n| format!("{name}(0x{tc:04X})×{n}")))
            .collect();
        for (tc, n) in &per_file {
            if CURVE_TARGETS.iter().any(|(c, _)| c == tc) {
                *curve_totals.entry(*tc).or_insert(0) += n;
            } else {
                *other_totals.entry(*tc).or_insert(0) += n;
            }
        }
        if !curve_hits.is_empty() {
            curve_files += 1;
            println!(
                "CURVE HIT {}\n    {}\n    streams: {}",
                file.display(),
                curve_hits.join("  "),
                curve_streams.join(" | ")
            );
        }
    }

    println!("\n==================== SUMMARY ====================");
    println!("files scanned: {scanned}, valid CFB containers: {cfb_ok}");
    println!("files with curve-family hits: {curve_files}");
    println!("curve-family totals:");
    for (tc, name) in CURVE_TARGETS {
        let n = curve_totals.get(tc).copied().unwrap_or(0);
        println!("  0x{tc:04X} {name}: {n}");
    }
    println!("top other type codes across corpus:");
    let mut others: Vec<_> = other_totals.into_iter().collect();
    others.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (tc, n) in others.into_iter().take(20) {
        println!("  0x{tc:04X}: {n}");
    }
    Ok(())
}
