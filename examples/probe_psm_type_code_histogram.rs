//! Phase 0 census — histogram every plausible PSM type code across **all**
//! `Sheet*` streams of every fixture, and split the result into
//! already-decoded vs still-undecoded families.
//!
//! For each offset that looks like a valid PSM record header
//! (`type_code != 0`, `bytes_to_follow ∈ 8..=100_000`, and the record fits
//! the stream), extract the 14-bit type code (`u16 LE & 0x3FFF`) and count
//! occurrences. This is a deliberately heuristic sliding-window scan (it can
//! over-count headers that coincidentally appear inside another record's
//! payload), so the cross-fixture filter (`≥ 2 fixtures`, `total ≥ 3`) is the
//! signal that matters, not the absolute per-offset counts.
//!
//! Goal: produce an empirical, frequency-ranked "what still needs parsing"
//! list — i.e. the dominant type codes that are NOT yet covered by the
//! Phase 14–26 decoders (`GLine2d` 0x3FE6, `GArc2d` 0x0030, `igLine2d`
//! 0x0018, `igLineString2d` 0x0084, `igPoint2d` 0x005E, `igTextBox` 0x004D,
//! `igSymbol2d` 0x00CE, `0x0010` attribute/sub-record). Run with:
//! `cargo run --example probe_psm_type_code_histogram`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;

type TypeCodeHistogram = BTreeMap<u16, usize>;
type FixtureHistogramResult = Result<(TypeCodeHistogram, usize, usize), Box<dyn std::error::Error>>;

/// Human label for the PSM type codes already covered by a Phase 14–26
/// decoder. `None` means the type code is still undecoded.
fn decoded_label(type_code: u16) -> Option<&'static str> {
    match type_code {
        0x3FE6 => Some("GLine2d (SmartPlant ext.)"),
        0x0030 => Some("GArc2d / GEllipse2d + JStyleOverride (shared 0x0030)"),
        0x0018 => Some("igLine2d (IGDS)"),
        0x0084 => Some("igLineString2d (IGDS)"),
        0x005E => Some("igPoint2d (IGDS)"),
        0x004D => Some("igTextBox (IGDS)"),
        0x00CE => Some("igSymbol2d (IGDS)"),
        0x00FA => Some("DependencyObject (Phase 15, audit-only header + raw tail)"),
        0x0010 => Some("0x0010 sub-record / attribute-fragment (audit-only)"),
        _ => None,
    }
}

/// If a valid PSM record header starts at `bytes[off]`, return the record's
/// end offset (`off + 6 + bytes_to_follow`); otherwise `None`. Valid means
/// `type_code != 0`, `bytes_to_follow ∈ 8..=100_000`, and the whole record
/// fits the stream.
fn psm_record_end(bytes: &[u8], off: usize) -> Option<usize> {
    let header_end = off.checked_add(6)?;
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

/// Histogram PSM type codes across every `Sheet*` stream of one fixture
/// using a **greedy + chain-validated record walk**: a candidate header is
/// only accepted (counted and stepped over) when the position it advances to
/// is either end-of-stream or itself a valid header. This prevents a single
/// false-positive header carrying a huge `bytes_to_follow` from swallowing
/// the rest of the stream (the failure mode of a naive greedy walk), while
/// staying far less noisy than a raw per-offset sliding window. The last
/// real record before any trailing padding may be missed (off-by-one per
/// stream), which is immaterial for the cross-fixture priority signal.
///
/// Returns `(histogram, sheet_stream_count, total_records_walked)`.
fn count_type_codes(path: &Path) -> FixtureHistogramResult {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let sheet_paths: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();

    let mut histogram: BTreeMap<u16, usize> = BTreeMap::new();
    let mut total_records = 0usize;
    for sp in &sheet_paths {
        let mut stream = cfb.open_stream(sp)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        let mut off = 0usize;
        while off + 6 <= bytes.len() {
            if let Some(end) = psm_record_end(&bytes, off) {
                if end == bytes.len() || psm_record_end(&bytes, end).is_some() {
                    let type_code = u16::from_le_bytes([bytes[off], bytes[off + 1]]) & 0x3FFF;
                    *histogram.entry(type_code).or_insert(0) += 1;
                    total_records += 1;
                    off = end;
                    continue;
                }
            }
            off += 1;
        }
    }
    Ok((histogram, sheet_paths.len(), total_records))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];

    // type_code -> (fixture -> count)
    let mut combined: BTreeMap<u16, BTreeMap<&str, usize>> = BTreeMap::new();

    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        let (histogram, sheet_count, record_count) = count_type_codes(path)?;
        println!("\n=== {fixture}  ({sheet_count} Sheet* streams, {record_count} PSM records) ===");
        let mut sorted: Vec<_> = histogram.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        for (type_code, count) in sorted.iter().take(20) {
            let tag = decoded_label(**type_code)
                .map(|l| format!(" [DECODED: {l}]"))
                .unwrap_or_default();
            println!(
                "  0x{:04X} ({:5}): {:7} hits{}",
                type_code, type_code, count, tag
            );
        }
        for (type_code, count) in &histogram {
            *combined
                .entry(*type_code)
                .or_default()
                .entry(fixture)
                .or_insert(0) += count;
        }
    }

    // Cross-fixture signal: appears in >= 2 fixtures with total >= 3 hits.
    let mut cross: Vec<_> = combined
        .iter()
        .filter(|(_, by_fix)| by_fix.len() >= 2 && by_fix.values().sum::<usize>() >= 3)
        .collect();
    cross.sort_by_key(|(_, by_fix)| std::cmp::Reverse(by_fix.values().sum::<usize>()));

    println!("\n========================================================");
    println!("CROSS-FIXTURE DECODED type codes (already covered)");
    println!("========================================================");
    for (type_code, by_fix) in cross.iter().filter(|(tc, _)| decoded_label(**tc).is_some()) {
        let total: usize = by_fix.values().sum();
        println!(
            "  0x{:04X} ({:5}): total {:7} hits / {} fixtures  [{}]",
            type_code,
            type_code,
            total,
            by_fix.len(),
            decoded_label(**type_code).unwrap_or("")
        );
    }

    println!("\n========================================================");
    println!("CROSS-FIXTURE UNDECODED type codes — PARSE PRIORITY LIST");
    println!("(ranked by total hits; >= 2 fixtures, total >= 3)");
    println!("========================================================");
    for (type_code, by_fix) in cross.iter().filter(|(tc, _)| decoded_label(**tc).is_none()) {
        let total: usize = by_fix.values().sum();
        println!(
            "  0x{:04X} ({:5}): total {:7} hits across {} fixtures",
            type_code,
            type_code,
            total,
            by_fix.len(),
        );
    }

    Ok(())
}
