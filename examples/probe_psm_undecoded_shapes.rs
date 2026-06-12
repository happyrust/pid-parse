//! Phase 0 mop-up — byte-shape probe for the still-undecoded top-level PSM
//! type codes surfaced by `probe_psm_type_code_histogram`:
//!
//! - `0x0013` (19) — top undecoded candidate (DWG-0202 + 工艺管道-1)
//! - `0x003D` (61) — appears 1-2× in every fixture; suspected per-sheet /
//!   per-group structural or container record
//! - `0x0020` (32) — possible IGDS `igRectangle2d`, low confidence (3 hits)
//!
//! For each target it enumerates the **top-level** records via the same
//! greedy + chain-validated walk the histogram uses (so record boundaries
//! line up with real records, not sliding-window noise), then dumps per
//! fixture: count, `bytes_to_follow` size distribution, the type of the
//! immediately preceding / following record (context), and a hex + u32-field
//! dump of the first few payloads to eyeball the layout. Read-only probe;
//! run with `cargo run --example probe_psm_undecoded_shapes`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_HEADER_LEN: usize = 6;

const TARGETS: &[(u16, &str)] = &[
    (0x0013, "0x0013 (19) — top undecoded candidate"),
    (0x003D, "0x003D (61) — per-sheet/per-group structural?"),
    (
        0x0020,
        "0x0020 (32) — possible igRectangle2d (low confidence)",
    ),
];

/// Decoded label for context (which neighbour record types surround a hit).
fn type_label(type_code: u16) -> &'static str {
    match type_code {
        0x3FE6 => "GLine2d",
        0x0030 => "GArc2d/JStyleOverride",
        0x0018 => "igLine2d",
        0x0084 => "igLineString2d",
        0x005E => "igPoint2d",
        0x004D => "igTextBox",
        0x00CE => "igSymbol2d",
        0x00FA => "GraphicGroup",
        0x0010 => "0x0010-frag",
        _ => "?",
    }
}

/// `Some(end)` if a valid PSM record header starts at `off`, else `None`.
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

#[derive(Clone)]
struct Rec {
    offset: usize,
    end: usize,
    type_code: u16,
    type_flags: u16,
    btf: u32,
}

/// Greedy + chain-validated top-level record walk (matches the histogram).
fn walk_records(bytes: &[u8]) -> Vec<Rec> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN <= bytes.len() {
        if let Some(end) = psm_record_end(bytes, off) {
            if end == bytes.len() || psm_record_end(bytes, end).is_some() {
                let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
                let btf = u32::from_le_bytes([
                    bytes[off + 2],
                    bytes[off + 3],
                    bytes[off + 4],
                    bytes[off + 5],
                ]);
                out.push(Rec {
                    offset: off,
                    end,
                    type_code: type_word & 0x3FFF,
                    type_flags: type_word >> 14,
                    btf,
                });
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn hexdump(payload: &[u8], max: usize) {
    let dump_len = payload.len().min(max);
    for start in (0..dump_len).step_by(16) {
        let end = (start + 16).min(dump_len);
        let raw = &payload[start..end];
        let hex = raw
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = raw
            .iter()
            .map(|b| {
                if (0x20..0x7F).contains(b) {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("             +{:03}: {:<47} | {}", start, hex, ascii);
    }
}

/// One captured sample of a target record plus its neighbours.
struct Sample {
    fixture: &'static str,
    stream: String,
    type_flags: u16,
    btf: u32,
    payload: Vec<u8>,
    prev: Option<u16>,
    next: Option<u16>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ];

    // target type_code -> (per-fixture count, btf distribution, samples)
    let mut per_fixture: BTreeMap<u16, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut btf_dist: BTreeMap<u16, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut samples: BTreeMap<u16, Vec<Sample>> = BTreeMap::new();

    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
        let sheet_paths: Vec<_> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_path_buf())
            .filter(|p| p.to_string_lossy().contains("Sheet"))
            .collect();

        for sp in &sheet_paths {
            let mut stream = cfb.open_stream(sp)?;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes)?;
            let records = walk_records(&bytes);
            for (i, rec) in records.iter().enumerate() {
                if !TARGETS.iter().any(|(tc, _)| *tc == rec.type_code) {
                    continue;
                }
                *per_fixture
                    .entry(rec.type_code)
                    .or_default()
                    .entry(fixture)
                    .or_insert(0) += 1;
                *btf_dist
                    .entry(rec.type_code)
                    .or_default()
                    .entry(rec.btf)
                    .or_insert(0) += 1;
                let bucket = samples.entry(rec.type_code).or_default();
                if bucket.len() < 4 {
                    bucket.push(Sample {
                        fixture,
                        stream: sp.to_string_lossy().to_string(),
                        type_flags: rec.type_flags,
                        btf: rec.btf,
                        payload: bytes[rec.offset + PSM_HEADER_LEN..rec.end].to_vec(),
                        prev: i.checked_sub(1).map(|j| records[j].type_code),
                        next: records.get(i + 1).map(|r| r.type_code),
                    });
                }
            }
        }
    }

    for (type_code, label) in TARGETS {
        println!("\n========================================================");
        println!("TARGET {label}");
        println!("========================================================");

        let counts = per_fixture.get(type_code);
        let total: usize = counts.map(|m| m.values().sum()).unwrap_or(0);
        println!("  total {total} hits");
        if let Some(counts) = counts {
            for (fix, n) in counts {
                println!("    {fix}: {n}");
            }
        }

        if let Some(dist) = btf_dist.get(type_code) {
            let mut sorted: Vec<_> = dist.iter().collect();
            sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
            print!("  bytes_to_follow sizes:");
            for (btf, c) in sorted.iter().take(8) {
                print!(" {btf}×{c}");
            }
            println!();
        }

        if let Some(bucket) = samples.get(type_code) {
            for (k, s) in bucket.iter().enumerate() {
                let words: Vec<String> = (0..6)
                    .filter_map(|w| {
                        read_u32(&s.payload, w * 4).map(|v| format!("+{}:{}", w * 4, v))
                    })
                    .collect();
                println!(
                    "\n  SAMPLE[{}] {} {}  flags={} btf={} payload_len={}",
                    k,
                    s.fixture,
                    s.stream,
                    s.type_flags,
                    s.btf,
                    s.payload.len()
                );
                println!(
                    "    context: prev={} next={}",
                    s.prev.map(type_label).unwrap_or("<none>"),
                    s.next.map(type_label).unwrap_or("<none>"),
                );
                println!("    u32 words: {}", words.join("  "));
                hexdump(&s.payload, 96);
            }
        }
    }

    Ok(())
}
