//! Phase 29 Slice C probe: triage the `/Unclustered Dynamic Attributes`
//! stream body across the local PID fixture set.
//!
//! This is read-only investigation tooling. For every fixture it prints:
//!
//! - the stream / byte-audit / landmark summary (trailers, heuristic
//!   attribute records, `0x89 0x00` marker count);
//! - a strict PSM-envelope chain test (`u16 type` + `u32 bytes_to_follow`)
//!   over the whole stream, mirroring the Phase 29 `/PSMcluster0` and
//!   `/StyleCluster` body triage, including the earliest end-anchored
//!   chain locator;
//! - alignment counts between walked chain heads and the known 31-byte
//!   record-trailer landmarks plus `P&IDAttributes` class-name offsets;
//! - a cross-fixture attribute-name census from the existing heuristic
//!   decoder, to rank which body fields would benefit object /
//!   relationship semantics.
//!
//! It prints markdown, does not name new fields, and does not promote
//! parser semantics.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pid_parse::parsers::dynamic_attr_records::{extract_record_trailers, parse_attribute_records};
use pid_parse::{byte_audit_report, AttributeValue, ByteRange, PidParser};

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

const DA_STREAM_NAME: &str = "Unclustered Dynamic Attributes";
const HEX_WINDOW_LEN: usize = 64;
const MAX_RECORD_BYTES_TO_FOLLOW: usize = 100_000;
const CHAIN_START_SEARCH_LIMIT: usize = 8192;
const MIN_CHAIN_RECORDS: usize = 3;
const CLASS_NAME: &[u8] = b"P&IDAttributes";
const CENSUS_TOP_NAMES: usize = 40;
const EXAMPLE_VALUE_MAX_CHARS: usize = 40;

#[derive(Debug, Default)]
struct EnvelopeWalk {
    records: usize,
    covered_bytes: u64,
    heads: Vec<u64>,
    type_histogram: BTreeMap<u16, usize>,
    stop_reason: &'static str,
    stop_offset: u64,
}

/// Strictly walk `bytes[start..end)` as a chain of PSM-style records
/// (`u16 type_word` + `u32 bytes_to_follow`). No resync: the walk stops
/// at the first offset that does not parse as a plausible record header.
fn walk_envelope(bytes: &[u8], start: usize, end: usize) -> EnvelopeWalk {
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
        walk.heads.push(cursor as u64);
        *walk.type_histogram.entry(type_code).or_insert(0) += 1;
        cursor = record_end;
    }
    walk.stop_offset = cursor as u64;
    walk
}

/// Locate the earliest offset within the bounded search window whose
/// strict envelope walk reaches exactly the end of the stream with at
/// least `MIN_CHAIN_RECORDS` records (the Phase 29-G end-anchored chain
/// rule). Falls back to the start with the largest covered byte count.
fn best_chain_start(bytes: &[u8]) -> (usize, EnvelopeWalk, bool) {
    let limit = bytes.len().min(CHAIN_START_SEARCH_LIMIT);
    let mut fallback: Option<(usize, EnvelopeWalk)> = None;
    for start in 0..limit {
        let walk = walk_envelope(bytes, start, bytes.len());
        if walk.records >= MIN_CHAIN_RECORDS && walk.stop_offset == bytes.len() as u64 {
            return (start, walk, true);
        }
        let better = match &fallback {
            Some((_, best)) => walk.covered_bytes > best.covered_bytes,
            None => walk.records > 0,
        };
        if better {
            fallback = Some((start, walk));
        }
    }
    match fallback {
        Some((start, walk)) => (start, walk, false),
        None => (0, walk_envelope(bytes, 0, bytes.len()), false),
    }
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

fn count_markers(bytes: &[u8]) -> usize {
    bytes
        .windows(2)
        .filter(|w| w[0] == 0x89 && w[1] == 0x00)
        .count()
}

fn class_name_at(bytes: &[u8], offset: usize) -> bool {
    bytes
        .get(offset..offset + CLASS_NAME.len())
        .is_some_and(|slice| slice == CLASS_NAME)
}

#[derive(Debug, Default)]
struct NameStats {
    records: usize,
    text: usize,
    integer: usize,
    float: usize,
    empty: usize,
    example: Option<String>,
}

fn sanitize_example(value: &str) -> String {
    let mut out: String = value
        .chars()
        .take(EXAMPLE_VALUE_MAX_CHARS)
        .map(|c| match c {
            '|' => '/',
            '`' => '\'',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    if value.chars().count() > EXAMPLE_VALUE_MAX_CHARS {
        out.push('…');
    }
    out
}

fn find_stream<'a>(pkg: &'a pid_parse::PidPackage, name: &str) -> Option<(&'a String, &'a [u8])> {
    pkg.streams
        .iter()
        .find(|(path, _)| path.trim_start_matches('/') == name)
        .map(|(path, stream)| (path, stream.data.as_slice()))
}

fn leftover_ranges_for(pkg: &pid_parse::PidPackage, stream_name: &str) -> Vec<ByteRange> {
    let report = byte_audit_report(pkg);
    report
        .traces
        .iter()
        .find(|trace| trace.stream_path.trim_start_matches('/') == stream_name)
        .map(|trace| trace.leftover_ranges.clone())
        .unwrap_or_default()
}

fn main() {
    println!("# Phase 29 Slice C: Unclustered Dynamic Attributes Body Triage");
    println!();
    println!("> Generated by `cargo run --example probe_phase29_da_body_triage`.");
    println!("> Investigation-only output; no parser semantics are promoted.");
    println!();

    let parser = PidParser::new();

    let mut summary_rows: Vec<String> = Vec::new();
    let mut chain_rows: Vec<String> = Vec::new();
    let mut detail_blocks: Vec<String> = Vec::new();
    let mut census: BTreeMap<String, NameStats> = BTreeMap::new();
    let mut class_id_histogram: BTreeMap<u32, usize> = BTreeMap::new();
    let mut class_name_histogram: BTreeMap<String, usize> = BTreeMap::new();

    for (fixture_id, fixture_path) in FIXTURES {
        if !Path::new(fixture_path).exists() {
            eprintln!("skip: fixture not found: {fixture_path}");
            continue;
        }
        let Ok(pkg) = parser.parse_package(fixture_path) else {
            eprintln!("skip: failed to parse fixture: {fixture_path}");
            continue;
        };
        let Some((_, data)) = find_stream(&pkg, DA_STREAM_NAME) else {
            eprintln!("skip: no {DA_STREAM_NAME} stream in {fixture_path}");
            continue;
        };

        let leftovers = leftover_ranges_for(&pkg, DA_STREAM_NAME);
        let leftover_bytes: u64 = leftovers.iter().map(ByteRange::len).sum();
        let trailers = extract_record_trailers(data);
        let (records, probe_summary) = parse_attribute_records(data);
        let marker_count = count_markers(data);

        summary_rows.push(format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            fixture_id,
            data.len(),
            leftover_bytes,
            leftovers.len(),
            trailers.len(),
            records.len(),
            marker_count
        ));

        for t in &trailers {
            *class_id_histogram.entry(t.class_id).or_insert(0) += 1;
        }
        for rec in &records {
            *class_name_histogram
                .entry(rec.class_name.clone())
                .or_insert(0) += 1;
            for field in &rec.attributes {
                let stats = census.entry(field.name.clone()).or_default();
                stats.records += 1;
                match &field.value {
                    AttributeValue::Text(v) => {
                        stats.text += 1;
                        if stats.example.is_none() && !v.is_empty() {
                            stats.example = Some(sanitize_example(v));
                        }
                    }
                    AttributeValue::Integer(v) => {
                        stats.integer += 1;
                        if stats.example.is_none() {
                            stats.example = Some(v.to_string());
                        }
                    }
                    AttributeValue::Float(v) => {
                        stats.float += 1;
                        if stats.example.is_none() {
                            stats.example = Some(format!("{v:.4}"));
                        }
                    }
                    _ => stats.empty += 1,
                }
            }
        }

        let (chain_start, walk, end_anchored) = best_chain_start(data);
        let stream_ratio = if data.is_empty() {
            0.0
        } else {
            walk.covered_bytes as f64 / data.len() as f64
        };

        let trailer_offsets: BTreeSet<u64> =
            trailers.iter().map(|t| t.trailer_offset as u64).collect();
        let head_set: BTreeSet<u64> = walk.heads.iter().copied().collect();
        let trailer_head_matches = trailer_offsets.intersection(&head_set).count();
        let class_name_after_header = walk
            .heads
            .iter()
            .filter(|&&head| class_name_at(data, head as usize + 31))
            .count();

        chain_rows.push(format!(
            "| `{}` | {} | {} | {} | {:.4} | {} | `{}` | {}/{} | {}/{} |",
            fixture_id,
            chain_start,
            walk.records,
            walk.covered_bytes,
            stream_ratio,
            if end_anchored { "yes" } else { "no" },
            walk.stop_reason,
            trailer_head_matches,
            trailer_offsets.len(),
            class_name_after_header,
            walk.records
        ));

        let mut block = String::new();
        block.push_str(&format!("### Fixture `{fixture_id}`\n\n"));
        block.push_str(&format!(
            "- Heuristic probe summary: body_start={}, markers={}, records_extracted={}, bytes_scanned={}\n",
            probe_summary.body_start_offset,
            probe_summary.marker_count,
            probe_summary.records_extracted,
            probe_summary.bytes_scanned
        ));
        if !walk.type_histogram.is_empty() {
            let mut hist: Vec<_> = walk.type_histogram.iter().collect();
            hist.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
            let top = hist
                .iter()
                .take(8)
                .map(|(t, c)| format!("0x{t:04X}×{c}"))
                .collect::<Vec<_>>()
                .join(", ");
            block.push_str(&format!("- Chain type histogram (top 8): {top}\n"));
        }
        block.push_str(&format!(
            "- Prologue before chain start: {} bytes\n",
            chain_start
        ));
        if chain_start > 0 {
            block.push_str(&format!(
                "  - Hex `[0..{}]`: `{}`\n",
                chain_start.min(HEX_WINDOW_LEN),
                hex_window(data, 0, chain_start.min(HEX_WINDOW_LEN))
            ));
        }
        let tail_gap = data.len() as u64 - walk.stop_offset;
        block.push_str(&format!("- Tail bytes after chain stop: {tail_gap}\n"));
        if tail_gap > 0 {
            let tail_start = walk.stop_offset as usize;
            block.push_str(&format!(
                "  - Hex `[{}..{}]`: `{}`\n",
                tail_start,
                (tail_start + HEX_WINDOW_LEN).min(data.len()),
                hex_window(data, tail_start, HEX_WINDOW_LEN)
            ));
        }
        block.push('\n');
        detail_blocks.push(block);
    }

    println!("## Stream And Landmark Summary");
    println!();
    println!(
        "| Fixture | Stream bytes | Leftover bytes | Leftover ranges | Trailers | Heuristic records | `0x89 0x00` markers |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|");
    for row in &summary_rows {
        println!("{row}");
    }
    println!();

    println!("## Envelope Chain Test (whole stream)");
    println!();
    println!(
        "| Fixture | Chain start | Records | Covered bytes | Stream ratio | End-anchored | Stop | Trailer-offset∩heads | Class-name at head+31 |"
    );
    println!("|---|---:|---:|---:|---:|---|---|---:|---:|");
    for row in &chain_rows {
        println!("{row}");
    }
    println!();

    println!("## Per-Fixture Detail");
    println!();
    for block in &detail_blocks {
        print!("{block}");
    }

    println!("## Trailer class_id Histogram (cross-fixture)");
    println!();
    println!("| class_id | Trailers |");
    println!("|---|---:|");
    let mut class_rows: Vec<_> = class_id_histogram.iter().collect();
    class_rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (class_id, count) in class_rows {
        println!("| 0x{class_id:08X} | {count} |");
    }
    println!();

    println!("## Heuristic Record class_name Histogram (cross-fixture)");
    println!();
    println!("| class_name | Records |");
    println!("|---|---:|");
    let mut name_rows: Vec<_> = class_name_histogram.iter().collect();
    name_rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (name, count) in name_rows.iter().take(16) {
        println!("| `{name}` | {count} |");
    }
    println!();

    println!("## Attribute Name Census (cross-fixture, heuristic decoder)");
    println!();
    println!("| Attribute | Fields | Text | Int | Float | Empty | Example |");
    println!("|---|---:|---:|---:|---:|---:|---|");
    let mut census_rows: Vec<_> = census.iter().collect();
    census_rows.sort_by_key(|(_, stats)| std::cmp::Reverse(stats.records));
    for (name, stats) in census_rows.iter().take(CENSUS_TOP_NAMES) {
        println!(
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            name,
            stats.records,
            stats.text,
            stats.integer,
            stats.float,
            stats.empty,
            stats
                .example
                .as_deref()
                .map(|e| format!("`{e}`"))
                .unwrap_or_default()
        );
    }
    println!();
    println!(
        "Census rows shown: {} of {} distinct attribute names.",
        census_rows.len().min(CENSUS_TOP_NAMES),
        census_rows.len()
    );
    println!();

    println!("## Guardrails");
    println!();
    println!(
        "- Chain results are byte-shape evidence only; walked type codes are not semantic names."
    );
    println!("- A high chain ratio justifies an audit-only walker backlog item, not a `Decoded` promotion.");
    println!("- Attribute names come from ASCII strings observed in the stream via the existing heuristic decoder; they are reported, not invented.");
}
