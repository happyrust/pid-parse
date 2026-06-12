//! Phase 29-C1 probe: classify `0x00CE` symbol-like records that remain in
//! Sheet byte-audit leftovers after existing Sheet decoder trace integration.
//!
//! This probe mirrors the conservative `decode_igsymbol_at` validation enough
//! to explain rejection reasons. It is read-only and does not relax parser
//! validation.

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::{byte_audit_report, ByteRange, PidParser};

const IGSYMBOL2D_TYPE_CODE: u16 = 0x00CE;
const IGSYMBOL2D_MIN_PAYLOAD_LEN: usize = 113;
const IGSYMBOL2D_MAX_PAYLOAD_LEN: usize = 200;
const COORDINATE_DOMAIN_LIMIT: f64 = 1.0e9;
const SCAN_LIMIT_PER_RANGE: usize = 512;

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
enum RejectReason {
    BytesToFollowOutOfRange,
    PayloadTruncated,
    MissingDoublePayload,
    NonFiniteDouble,
    OutOfDomainDouble,
    AcceptedButStillLeftover,
}

impl RejectReason {
    fn label(&self) -> &'static str {
        match self {
            RejectReason::BytesToFollowOutOfRange => "bytes_to_follow_out_of_range",
            RejectReason::PayloadTruncated => "payload_truncated",
            RejectReason::MissingDoublePayload => "missing_double_payload",
            RejectReason::NonFiniteDouble => "non_finite_double",
            RejectReason::OutOfDomainDouble => "out_of_domain_double",
            RejectReason::AcceptedButStillLeftover => "accepted_but_still_leftover",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RejectKey {
    reason: RejectReason,
    type_flags: u16,
    bytes_to_follow: u32,
    prefix: String,
}

#[derive(Debug, Clone)]
struct Sample {
    fixture: &'static str,
    path: String,
    offset: usize,
    range: ByteRange,
    hex_prefix: String,
}

#[derive(Debug, Clone)]
struct Group {
    count: usize,
    samples: Vec<Sample>,
}

fn hex_prefix(bytes: &[u8], max_len: usize) -> String {
    bytes
        .iter()
        .take(max_len)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn classify_igsymbol_candidate(data: &[u8], offset: usize) -> Option<RejectKey> {
    let header = data.get(offset..offset.checked_add(6)?)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != IGSYMBOL2D_TYPE_CODE {
        return None;
    }

    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let btf = bytes_to_follow as usize;
    let reason = if !(IGSYMBOL2D_MIN_PAYLOAD_LEN..=IGSYMBOL2D_MAX_PAYLOAD_LEN).contains(&btf) {
        RejectReason::BytesToFollowOutOfRange
    } else {
        let header_end = offset + 6;
        let Some(payload_end) = header_end.checked_add(btf) else {
            return Some(RejectKey {
                reason: RejectReason::PayloadTruncated,
                type_flags,
                bytes_to_follow,
                prefix: hex_prefix(&data[offset..data.len().min(offset + 8)], 8),
            });
        };
        if payload_end > data.len() {
            RejectReason::PayloadTruncated
        } else {
            let payload = data.get(header_end..payload_end)?;
            let mut reason = RejectReason::AcceptedButStillLeftover;
            for i in 0..6 {
                let pos = 40 + i * 8;
                let Some(chunk) = payload.get(pos..pos + 8) else {
                    reason = RejectReason::MissingDoublePayload;
                    break;
                };
                let value = f64::from_le_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
                ]);
                if !value.is_finite() {
                    reason = RejectReason::NonFiniteDouble;
                    break;
                }
                if value.abs() > COORDINATE_DOMAIN_LIMIT {
                    reason = RejectReason::OutOfDomainDouble;
                    break;
                }
            }
            reason
        }
    };

    Some(RejectKey {
        reason,
        type_flags,
        bytes_to_follow,
        prefix: hex_prefix(&data[offset..data.len().min(offset + 8)], 8),
    })
}

fn leftover_ranges_for(pkg: &pid_parse::PidPackage, path: &str) -> Vec<ByteRange> {
    let report = byte_audit_report(pkg);
    report
        .traces
        .iter()
        .find(|trace| trace.stream_path == path)
        .map(|trace| trace.leftover_ranges.clone())
        .unwrap_or_else(|| {
            pkg.get_stream(path)
                .map(|stream| vec![ByteRange::new(0, stream.data.len() as u64)])
                .unwrap_or_default()
        })
}

fn collect_groups() -> BTreeMap<RejectKey, Group> {
    let parser = PidParser::new();
    let mut groups: BTreeMap<RejectKey, Group> = BTreeMap::new();

    for (fixture_id, fixture_path) in FIXTURES {
        if !Path::new(fixture_path).exists() {
            eprintln!("skip: missing fixture {fixture_path}");
            continue;
        }
        let Ok(pkg) = parser.parse_package(fixture_path) else {
            eprintln!("skip: failed to parse {fixture_path}");
            continue;
        };

        let sheet_paths: Vec<String> = pkg
            .streams
            .keys()
            .filter(|path| path.starts_with("/Sheet"))
            .cloned()
            .collect();

        for path in sheet_paths {
            let Some(stream) = pkg.get_stream(&path) else {
                continue;
            };
            for range in leftover_ranges_for(&pkg, &path) {
                let Ok(range_start) = usize::try_from(range.start) else {
                    continue;
                };
                let Ok(range_end_raw) = usize::try_from(range.end) else {
                    continue;
                };
                let range_end = range_end_raw.min(stream.data.len());
                let scan_end = range_start
                    .saturating_add(SCAN_LIMIT_PER_RANGE)
                    .min(range_end);
                for offset in range_start..scan_end {
                    let Some(key) = classify_igsymbol_candidate(&stream.data, offset) else {
                        continue;
                    };
                    let hex_prefix =
                        hex_prefix(&stream.data[offset..stream.data.len().min(offset + 32)], 32);
                    let group = groups.entry(key).or_insert_with(|| Group {
                        count: 0,
                        samples: Vec::new(),
                    });
                    group.count += 1;
                    if group.samples.len() < 4 {
                        group.samples.push(Sample {
                            fixture: fixture_id,
                            path: path.clone(),
                            offset,
                            range,
                            hex_prefix,
                        });
                    }
                }
            }
        }
    }

    groups
}

fn main() {
    let groups = collect_groups();
    let mut sorted: Vec<_> = groups.iter().collect();
    sorted.sort_by_key(|(key, group)| {
        (
            std::cmp::Reverse(group.count),
            key.reason.clone(),
            key.type_flags,
            key.bytes_to_follow,
            key.prefix.clone(),
        )
    });

    println!("# Phase 29-C1 igSymbol2d Reject Probe");
    println!();
    println!("> Generated by `cargo run --example probe_phase29_igsymbol_rejects`.");
    println!("> This classifies `0x00CE` candidates that remain in Sheet leftovers.");
    println!();
    println!("## Reject Groups");
    println!();
    println!("| Rank | Reason | Type flags | Bytes-to-follow | Prefix | Count | Sample offsets |");
    println!("|---:|---|---:|---:|---|---:|---|");
    for (idx, (key, group)) in sorted.iter().enumerate() {
        let samples = group
            .samples
            .iter()
            .map(|sample| {
                format!(
                    "{} {} 0x{:06X} (range {}..{})",
                    sample.fixture,
                    sample.path,
                    sample.offset,
                    sample.range.start,
                    sample.range.end
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        println!(
            "| {} | `{}` | {} | {} | `{}` | {} | {} |",
            idx + 1,
            key.reason.label(),
            key.type_flags,
            key.bytes_to_follow,
            key.prefix,
            group.count,
            samples
        );
    }

    println!();
    println!("## Sample Hex Prefixes");
    println!();
    println!("| Reason | Fixture | Path | Offset | Hex prefix |");
    println!("|---|---|---|---:|---|");
    for (key, group) in sorted.iter().take(12) {
        for sample in &group.samples {
            println!(
                "| `{}` | `{}` | `{}` | 0x{:06X} | `{}` |",
                key.reason.label(),
                sample.fixture,
                sample.path,
                sample.offset,
                sample.hex_prefix
            );
        }
    }

    println!();
    println!("## Guardrails");
    println!();
    println!("- This probe does not relax `decode_igsymbols` validation.");
    println!("- `accepted_but_still_leftover` is a trace-accounting smell and should be investigated before parser changes.");
    println!("- Non-zero type flags require IDA or controlled fixture evidence before acceptance.");
}
