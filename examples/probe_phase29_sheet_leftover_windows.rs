//! Phase 29-A probe: emit bounded byte windows for Sheet-related byte-audit
//! leftovers across the local PID fixture set.
//!
//! This is read-only investigation tooling. It groups leftover ranges by a
//! conservative local shape: stream kind, nearby PSM-like header, size bucket,
//! and prefix bytes. It prints markdown for human review, does not name fields,
//! and does not promote parser semantics.

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::{byte_audit_report, ByteRange, PidParser};

const WINDOW_LEN: usize = 96;
const HEADER_SCAN_LEN: usize = 32;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ShapeKey {
    stream_kind: &'static str,
    type_code: Option<u16>,
    bytes_to_follow_bucket: String,
    prefix: String,
}

#[derive(Debug, Clone)]
struct Sample {
    fixture: &'static str,
    path: String,
    range: ByteRange,
    window_start: usize,
    window_end: usize,
    header_offset: Option<usize>,
    hex_prefix: String,
}

#[derive(Debug, Clone)]
struct Group {
    total_leftover_bytes: u64,
    samples: Vec<Sample>,
    paths: BTreeMap<String, usize>,
    fixtures: BTreeMap<&'static str, usize>,
}

#[derive(Debug, Clone, Copy)]
struct CandidateHeader {
    offset: usize,
    type_code: u16,
    bytes_to_follow: usize,
}

fn type_label(type_code: u16) -> &'static str {
    match type_code {
        0x0010 => "0x0010",
        0x0018 => "igLine2d",
        0x0030 => "JStyleOverride",
        0x004D => "igTextBox",
        0x005E => "igPoint2d",
        0x0084 => "igLineString2d",
        0x00CE => "igSymbol2d",
        0x00FA => "DependencyObject",
        0x3FE6 => "GLine2d",
        _ => "unknown",
    }
}

fn stream_kind(path: &str) -> &'static str {
    if path.starts_with("/Sheet") {
        "top-level Sheet"
    } else if path.contains("/Sheet") {
        "nested Sheet-like"
    } else {
        "other"
    }
}

fn bytes_to_follow_bucket(bytes_to_follow: Option<usize>) -> String {
    let Some(value) = bytes_to_follow else {
        return "no-header".to_string();
    };

    match value {
        0..=15 => "0000-0015".to_string(),
        16..=31 => "0016-0031".to_string(),
        32..=63 => "0032-0063".to_string(),
        64..=127 => "0064-0127".to_string(),
        128..=255 => "0128-0255".to_string(),
        256..=511 => "0256-0511".to_string(),
        512..=1023 => "0512-1023".to_string(),
        _ => "1024+".to_string(),
    }
}

fn hex_prefix(bytes: &[u8], max_len: usize) -> String {
    bytes
        .iter()
        .take(max_len)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn candidate_header_at(bytes: &[u8], offset: usize) -> Option<CandidateHeader> {
    let header = bytes.get(offset..offset.checked_add(6)?)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code == 0 {
        return None;
    }

    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
    if !(8..=100_000).contains(&bytes_to_follow) {
        return None;
    }

    let record_end = offset.checked_add(6)?.checked_add(bytes_to_follow)?;
    (record_end <= bytes.len()).then_some(CandidateHeader {
        offset,
        type_code,
        bytes_to_follow,
    })
}

fn find_candidate_header(bytes: &[u8], range: ByteRange) -> Option<CandidateHeader> {
    let start = usize::try_from(range.start).ok()?;
    let end = usize::try_from(range.end).ok()?.min(bytes.len());
    let scan_end = start.saturating_add(HEADER_SCAN_LEN).min(end);
    (start..scan_end).find_map(|offset| candidate_header_at(bytes, offset))
}

fn sample_for_range(
    fixture: &'static str,
    path: &str,
    bytes: &[u8],
    range: ByteRange,
    header: Option<CandidateHeader>,
) -> Option<Sample> {
    let start = usize::try_from(range.start).ok()?.min(bytes.len());
    let end_bound = usize::try_from(range.end).ok()?.min(bytes.len());
    if start >= end_bound {
        return None;
    }
    let window_end = start.saturating_add(WINDOW_LEN).min(end_bound);
    let window = bytes.get(start..window_end)?;
    Some(Sample {
        fixture,
        path: path.to_string(),
        range,
        window_start: start,
        window_end,
        header_offset: header.map(|h| h.offset),
        hex_prefix: hex_prefix(window, 32),
    })
}

fn sheet_leftover_ranges(
    pkg: &pid_parse::PidPackage,
    path: &str,
    total_bytes: u64,
) -> Vec<ByteRange> {
    let report = byte_audit_report(pkg);
    if let Some(trace) = report.traces.iter().find(|trace| trace.stream_path == path) {
        return trace.leftover_ranges.clone();
    }
    vec![ByteRange::new(0, total_bytes)]
}

fn collect_groups() -> BTreeMap<ShapeKey, Group> {
    let parser = PidParser::new();
    let mut groups: BTreeMap<ShapeKey, Group> = BTreeMap::new();

    for (fixture_id, fixture_path) in FIXTURES {
        if !Path::new(fixture_path).exists() {
            eprintln!("skip: fixture not found: {fixture_path}");
            continue;
        }
        let Ok(pkg) = parser.parse_package(fixture_path) else {
            eprintln!("skip: failed to parse fixture: {fixture_path}");
            continue;
        };

        let sheet_paths: Vec<_> = pkg
            .streams
            .iter()
            .filter(|(path, _)| path.contains("Sheet"))
            .map(|(path, stream)| (path.clone(), stream.data.len() as u64))
            .collect();

        for (path, total_bytes) in sheet_paths {
            let Some(stream) = pkg.streams.get(&path) else {
                continue;
            };
            for range in sheet_leftover_ranges(&pkg, &path, total_bytes) {
                if range.is_empty() {
                    continue;
                }
                let header = find_candidate_header(&stream.data, range);
                let prefix_start = usize::try_from(range.start)
                    .ok()
                    .unwrap_or_default()
                    .min(stream.data.len());
                let prefix_end = prefix_start.saturating_add(4).min(stream.data.len());
                let key = ShapeKey {
                    stream_kind: stream_kind(&path),
                    type_code: header.map(|h| h.type_code),
                    bytes_to_follow_bucket: bytes_to_follow_bucket(
                        header.map(|h| h.bytes_to_follow),
                    ),
                    prefix: hex_prefix(&stream.data[prefix_start..prefix_end], 4),
                };

                let Some(sample) = sample_for_range(fixture_id, &path, &stream.data, range, header)
                else {
                    continue;
                };

                let group = groups.entry(key).or_insert_with(|| Group {
                    total_leftover_bytes: 0,
                    samples: Vec::new(),
                    paths: BTreeMap::new(),
                    fixtures: BTreeMap::new(),
                });
                group.total_leftover_bytes += range.len();
                *group.paths.entry(path.clone()).or_insert(0) += 1;
                *group.fixtures.entry(*fixture_id).or_insert(0) += 1;
                if group.samples.len() < 3 {
                    group.samples.push(sample);
                }
            }
        }
    }

    groups
}

fn shape_label(key: &ShapeKey) -> String {
    match key.type_code {
        Some(type_code) => format!(
            "{} / 0x{type_code:04X} {} / btf {} / prefix {}",
            key.stream_kind,
            type_label(type_code),
            key.bytes_to_follow_bucket,
            key.prefix
        ),
        None => format!(
            "{} / no PSM-like header / prefix {}",
            key.stream_kind, key.prefix
        ),
    }
}

fn main() {
    let groups = collect_groups();
    let mut sorted: Vec<_> = groups.iter().collect();
    sorted.sort_by_key(|(_, group)| std::cmp::Reverse(group.total_leftover_bytes));

    println!("# Phase 29-A Sheet Leftover Windows");
    println!();
    println!("> Generated by `cargo run --example probe_phase29_sheet_leftover_windows`.");
    println!("> This is a bounded byte-window inventory for investigation only.");
    println!();
    println!("## Top Shape Groups");
    println!();
    println!(
        "| Rank | Shape | Leftover bytes | Fixture hits | Path hits | Sample ranges | Next action |"
    );
    println!("|---:|---|---:|---:|---:|---|---|");
    for (idx, (key, group)) in sorted.iter().take(20).enumerate() {
        let sample_ranges = group
            .samples
            .iter()
            .map(|sample| {
                format!(
                    "{} {}:{}..{}",
                    sample.fixture, sample.path, sample.range.start, sample.range.end
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        let action = if key.type_code.is_some() {
            "Map to existing typed/audit decoder or mark NeedsIDA"
        } else if key.stream_kind == "nested Sheet-like" {
            "Decide byte-audit registration / ownership first"
        } else {
            "Needs bounded record-shape probe"
        };
        println!(
            "| {} | `{}` | {} | {} | {} | {} | {} |",
            idx + 1,
            shape_label(key),
            group.total_leftover_bytes,
            group.fixtures.len(),
            group.paths.len(),
            sample_ranges,
            action
        );
    }

    println!();
    println!("## Sample Windows");
    println!();
    println!("| Shape | Fixture | Path | Range | Window | Header offset | Hex prefix |");
    println!("|---|---|---|---|---|---|---|");
    for (key, group) in sorted.iter().take(12) {
        for sample in &group.samples {
            let header_offset = sample
                .header_offset
                .map(|offset| format!("0x{offset:06X}"))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "| `{}` | `{}` | `{}` | {}..{} | {}..{} | {} | `{}` |",
                shape_label(key),
                sample.fixture,
                sample.path,
                sample.range.start,
                sample.range.end,
                sample.window_start,
                sample.window_end,
                header_offset,
                sample.hex_prefix
            );
        }
    }

    println!();
    println!("## Guardrails");
    println!();
    println!("- Treat these groups as byte-shape evidence only.");
    println!("- Do not infer field names from prefix bytes, size buckets, or offsets.");
    println!(
        "- `0x0010`, `DependencyObject`, page transform, and text placement remain no-promotion."
    );
    println!("- Nested `JSite*/Sheet*` streams need ownership and byte-audit registration decisions before parser work.");
}
