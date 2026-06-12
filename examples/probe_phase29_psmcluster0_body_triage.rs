//! Phase 29 Slice B probe: triage the `/PSMcluster0` and `/StyleCluster`
//! post-header bodies across the local PID fixture set.
//!
//! This is read-only investigation tooling. For every fixture it prints the
//! cluster header fields, the byte-audit leftover ranges, and three bounded
//! shape analyses over each leftover range:
//!
//! - a strict PSM-envelope walk (6-byte header: 14-bit type code +
//!   `bytes_to_follow`) to test whether the body reuses the `Sheet*` record
//!   envelope;
//! - a UTF-16LE printable-run scan to estimate embedded text volume;
//! - a hex window of the first bytes for human review.
//!
//! It prints markdown, does not name fields, and does not promote parser
//! semantics.

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::parsers::cluster_header;
use pid_parse::{byte_audit_report, ByteRange, PidParser};

const FIXTURES: &[(&str, &str)] = &[
    ("d06", "test-file/D06.pid"),
    ("nonascii-process-1", "test-file/工艺管道及仪表流程-1.pid"),
    ("dwg0201", "test-file/DWG-0201GP06-01.pid"),
    ("dwg0202", "test-file/DWG-0202GP06-01.pid"),
    (
        "publish-a01",
        "test-file/export-test/publish-data/A01/A01.pid",
    ),
    (
        "publish-dwg0202",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ),
];

const HEX_WINDOW_LEN: usize = 64;
const MIN_TEXT_RUN_CHARS: usize = 6;
const MAX_SNIPPETS: usize = 8;
const MAX_RECORD_BYTES_TO_FOLLOW: usize = 100_000;

#[derive(Debug, Default)]
struct EnvelopeWalk {
    records: usize,
    covered_bytes: u64,
    type_histogram: BTreeMap<u16, usize>,
    first_types: Vec<u16>,
    stop_reason: &'static str,
    stop_offset: u64,
}

/// Strictly walk `bytes[start..end)` as a chain of PSM-style records
/// (`u16 type_word` + `u32 bytes_to_follow`). No resync: the walk stops at
/// the first offset that does not parse as a plausible record header.
fn walk_psm_envelope(bytes: &[u8], start: usize, end: usize) -> EnvelopeWalk {
    let mut walk = EnvelopeWalk {
        stop_reason: "end-of-range",
        ..EnvelopeWalk::default()
    };
    let mut cursor = start;
    let end = end.min(bytes.len());

    while cursor < end {
        if cursor + 6 > end {
            walk.stop_reason = "truncated-header";
            break;
        }
        let type_word = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
        let type_code = type_word & 0x3FFF;
        let btf = u32::from_le_bytes([
            bytes[cursor + 2],
            bytes[cursor + 3],
            bytes[cursor + 4],
            bytes[cursor + 5],
        ]) as usize;
        if type_code == 0 {
            walk.stop_reason = "zero-type-code";
            break;
        }
        if btf > MAX_RECORD_BYTES_TO_FOLLOW {
            walk.stop_reason = "btf-too-large";
            break;
        }
        let Some(record_end) = cursor.checked_add(6).and_then(|v| v.checked_add(btf)) else {
            walk.stop_reason = "btf-overflow";
            break;
        };
        if record_end > end {
            walk.stop_reason = "btf-past-range";
            break;
        }
        walk.records += 1;
        walk.covered_bytes += 6 + btf as u64;
        *walk.type_histogram.entry(type_code).or_insert(0) += 1;
        if walk.first_types.len() < 10 {
            walk.first_types.push(type_code);
        }
        cursor = record_end;
    }
    walk.stop_offset = cursor as u64;
    walk
}

#[derive(Debug, Default)]
struct ChainScan {
    chains: usize,
    records: usize,
    covered_bytes: u64,
    resync_bytes: u64,
    type_histogram: BTreeMap<u16, usize>,
    longest_chain_start: u64,
    longest_chain_records: usize,
    longest_chain_bytes: u64,
}

/// Resync chain scan: walk `bytes[start..end)` accepting PSM-style record
/// chains of at least `min_chain` consecutive records. On failure to form a
/// chain, advance one byte (counted as resync). This estimates how much of
/// the body could be explained by the `Sheet*` record envelope without
/// claiming semantics for any walked type code.
fn scan_psm_chains(bytes: &[u8], start: usize, end: usize, min_chain: usize) -> ChainScan {
    let mut scan = ChainScan::default();
    let end = end.min(bytes.len());
    let mut cursor = start;

    let record_at = |offset: usize| -> Option<(u16, usize)> {
        if offset + 6 > end {
            return None;
        }
        let type_word = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code == 0 {
            return None;
        }
        let btf = u32::from_le_bytes([
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
        ]) as usize;
        if btf > MAX_RECORD_BYTES_TO_FOLLOW {
            return None;
        }
        let record_end = offset.checked_add(6)?.checked_add(btf)?;
        (record_end <= end).then_some((type_code, record_end))
    };

    while cursor < end {
        // Try to grow a chain from `cursor`.
        let mut chain_records: Vec<u16> = Vec::new();
        let mut probe = cursor;
        while let Some((type_code, record_end)) = record_at(probe) {
            chain_records.push(type_code);
            probe = record_end;
        }
        if chain_records.len() >= min_chain {
            let chain_bytes = (probe - cursor) as u64;
            scan.chains += 1;
            scan.records += chain_records.len();
            scan.covered_bytes += chain_bytes;
            for t in &chain_records {
                *scan.type_histogram.entry(*t).or_insert(0) += 1;
            }
            if chain_records.len() > scan.longest_chain_records {
                scan.longest_chain_records = chain_records.len();
                scan.longest_chain_start = cursor as u64;
                scan.longest_chain_bytes = chain_bytes;
            }
            cursor = probe;
        } else {
            scan.resync_bytes += 1;
            cursor += 1;
        }
    }
    scan
}

#[derive(Debug, Default)]
struct TextRunScan {
    runs: usize,
    total_bytes: u64,
    snippets: Vec<String>,
}

/// Scan for UTF-16LE printable ASCII runs of at least
/// `MIN_TEXT_RUN_CHARS` characters inside `bytes[start..end)`.
fn scan_utf16_runs(bytes: &[u8], start: usize, end: usize) -> TextRunScan {
    let mut scan = TextRunScan::default();
    let end = end.min(bytes.len());
    let mut cursor = start;
    while cursor + 2 <= end {
        let mut run_chars = 0usize;
        let mut probe = cursor;
        while probe + 2 <= end {
            let unit = u16::from_le_bytes([bytes[probe], bytes[probe + 1]]);
            if (0x20..=0x7E).contains(&unit) {
                run_chars += 1;
                probe += 2;
            } else {
                break;
            }
        }
        if run_chars >= MIN_TEXT_RUN_CHARS {
            scan.runs += 1;
            scan.total_bytes += (run_chars * 2) as u64;
            if scan.snippets.len() < MAX_SNIPPETS {
                let units: Vec<u16> = (0..run_chars.min(48))
                    .map(|i| u16::from_le_bytes([bytes[cursor + i * 2], bytes[cursor + i * 2 + 1]]))
                    .collect();
                scan.snippets.push(String::from_utf16_lossy(&units));
            }
            cursor = probe;
        } else {
            cursor += 1;
        }
    }
    scan
}

fn hex_window(bytes: &[u8], start: usize, max_len: usize) -> String {
    bytes
        .iter()
        .skip(start)
        .take(max_len)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Byte-level stride autocorrelation over `bytes[start..end)` (capped):
/// for each candidate stride, the fraction of positions whose byte equals
/// the byte one stride later. A dominant stride hints at a fixed-size
/// record table without naming any field.
fn stride_autocorrelation(bytes: &[u8], start: usize, end: usize) -> Vec<(usize, f64)> {
    const MAX_SPAN: usize = 8192;
    let end = end.min(bytes.len()).min(start + MAX_SPAN);
    if end <= start + 128 {
        return Vec::new();
    }
    let window = &bytes[start..end];
    let mut scores: Vec<(usize, f64)> = (2..=64)
        .filter_map(|stride| {
            let pairs = window.len().checked_sub(stride)?;
            if pairs < 64 {
                return None;
            }
            let matches = (0..pairs)
                .filter(|&i| window[i] == window[i + stride])
                .count();
            Some((stride, matches as f64 / pairs as f64))
        })
        .collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    scores.truncate(5);
    scores
}

fn find_stream<'a>(pkg: &'a pid_parse::PidPackage, name: &str) -> Option<(&'a String, &'a [u8])> {
    pkg.streams
        .iter()
        .find(|(path, _)| path.trim_start_matches('/') == name)
        .map(|(path, stream)| (path, stream.data.as_slice()))
}

fn leftover_ranges_for(pkg: &pid_parse::PidPackage, stream_path: &str) -> Vec<ByteRange> {
    let report = byte_audit_report(pkg);
    report
        .traces
        .iter()
        .find(|trace| trace.stream_path.trim_start_matches('/') == stream_path)
        .map(|trace| trace.leftover_ranges.clone())
        .unwrap_or_default()
}

fn main() {
    let stream_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "PSMcluster0".to_string());

    println!("# Phase 29 Slice B: {stream_name} Body Triage");
    println!();
    println!(
        "> Generated by `cargo run --example probe_phase29_psmcluster0_body_triage [stream]`."
    );
    println!("> Investigation-only output; no parser semantics are promoted.");
    println!();

    let parser = PidParser::new();

    println!("## Header And Leftover Summary");
    println!();
    println!("| Fixture | Stream bytes | record_count | stream_type | body_len | flags | Leftover ranges | Leftover bytes |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");

    let mut detail_blocks: Vec<String> = Vec::new();

    for (fixture_id, fixture_path) in FIXTURES {
        if !Path::new(fixture_path).exists() {
            eprintln!("skip: fixture not found: {fixture_path}");
            continue;
        }
        let Ok(pkg) = parser.parse_package(fixture_path) else {
            eprintln!("skip: failed to parse fixture: {fixture_path}");
            continue;
        };
        let Some((stream_path, data)) = find_stream(&pkg, &stream_name) else {
            eprintln!("skip: no {stream_name} stream in {fixture_path}");
            continue;
        };

        let header = cluster_header::parse_header(data);
        let leftovers = leftover_ranges_for(&pkg, &stream_name);
        let leftover_bytes: u64 = leftovers.iter().map(ByteRange::len).sum();

        match &header {
            Some(h) => println!(
                "| `{}` | {} | {} | 0x{:04X} | {} | 0x{:04X} | {} | {} |",
                fixture_id,
                data.len(),
                h.record_count,
                h.stream_type,
                h.body_len,
                h.flags,
                leftovers.len(),
                leftover_bytes
            ),
            None => println!(
                "| `{}` | {} | - | - | - | - | {} | {} |",
                fixture_id,
                data.len(),
                leftovers.len(),
                leftover_bytes
            ),
        }

        let mut block = String::new();
        block.push_str(&format!("### Fixture `{fixture_id}` (`{stream_path}`)\n\n"));
        for range in &leftovers {
            let start = usize::try_from(range.start).unwrap_or(usize::MAX);
            let end = usize::try_from(range.end)
                .unwrap_or(usize::MAX)
                .min(data.len());
            if start >= end {
                continue;
            }
            let walk = walk_psm_envelope(data, start, end);
            let text = scan_utf16_runs(data, start, end);
            let range_len = (end - start) as u64;
            let walk_ratio = if range_len == 0 {
                0.0
            } else {
                walk.covered_bytes as f64 / range_len as f64
            };

            block.push_str(&format!(
                "- Range `{}..{}` ({} bytes)\n",
                range.start, range.end, range_len
            ));
            block.push_str(&format!(
                "  - PSM-envelope strict walk: {} records, {} bytes covered ({:.4} of range), stop=`{}` at offset {}\n",
                walk.records, walk.covered_bytes, walk_ratio, walk.stop_reason, walk.stop_offset
            ));
            if !walk.first_types.is_empty() {
                let first = walk
                    .first_types
                    .iter()
                    .map(|t| format!("0x{t:04X}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                block.push_str(&format!("  - First walked type codes: {first}\n"));
            }
            if !walk.type_histogram.is_empty() {
                let mut hist: Vec<_> = walk.type_histogram.iter().collect();
                hist.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
                let top = hist
                    .iter()
                    .take(8)
                    .map(|(t, c)| format!("0x{t:04X}×{c}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                block.push_str(&format!("  - Walked type histogram (top 8): {top}\n"));
            }

            let chains = scan_psm_chains(data, start, end, 3);
            let chain_ratio = if range_len == 0 {
                0.0
            } else {
                chains.covered_bytes as f64 / range_len as f64
            };
            block.push_str(&format!(
                "  - PSM resync chain scan (min chain 3): {} chains, {} records, {} bytes covered ({:.4} of range), {} resync bytes\n",
                chains.chains, chains.records, chains.covered_bytes, chain_ratio, chains.resync_bytes
            ));
            if chains.longest_chain_records > 0 {
                block.push_str(&format!(
                    "  - Longest chain: {} records / {} bytes starting at offset {}\n",
                    chains.longest_chain_records,
                    chains.longest_chain_bytes,
                    chains.longest_chain_start
                ));
            }
            if !chains.type_histogram.is_empty() {
                let mut hist: Vec<_> = chains.type_histogram.iter().collect();
                hist.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
                let top = hist
                    .iter()
                    .take(10)
                    .map(|(t, c)| format!("0x{t:04X}×{c}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                block.push_str(&format!("  - Chain type histogram (top 10): {top}\n"));
            }
            block.push_str(&format!(
                "  - UTF-16LE printable runs (≥{MIN_TEXT_RUN_CHARS} chars): {} runs, {} bytes\n",
                text.runs, text.total_bytes
            ));
            for snippet in &text.snippets {
                block.push_str(&format!("    - `{snippet}`\n"));
            }
            let strides = stride_autocorrelation(data, start, end);
            if !strides.is_empty() {
                let s = strides
                    .iter()
                    .map(|(stride, score)| format!("{stride}:{score:.3}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                block.push_str(&format!(
                    "  - Stride autocorrelation top 5 (first 8KB): {s}\n"
                ));
            }
            let mid = start + (end - start) / 2;
            let tail = end.saturating_sub(HEX_WINDOW_LEN).max(start);
            for (label, w_start) in [("start", start), ("mid", mid), ("tail", tail)] {
                let w_len = HEX_WINDOW_LEN.min(end - w_start);
                if w_len == 0 {
                    continue;
                }
                block.push_str(&format!(
                    "  - Hex window ({label}) `[{}..{}]`: `{}`\n",
                    w_start,
                    w_start + w_len,
                    hex_window(data, w_start, w_len)
                ));
            }
        }
        block.push('\n');
        detail_blocks.push(block);
    }

    println!();
    println!("## Per-Fixture Leftover Detail");
    println!();
    for block in &detail_blocks {
        print!("{block}");
    }

    println!("## Guardrails");
    println!();
    println!("- Walk results are byte-shape evidence only; type codes walked here are not semantic names.");
    println!("- A high walk ratio justifies a parser-backlog item, not a `Decoded` promotion.");
    println!("- Text snippets are bounded samples for human review, not extracted product data.");
}
