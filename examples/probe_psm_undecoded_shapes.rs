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
        0x00FA => "DependencyObject",
        0x0010 => "0x0010-frag",
        _ => "?",
    }
}

fn record_summary(rec: &Rec) -> String {
    format!(
        "{} range=0x{:06X}..0x{:06X} flags={} btf={}",
        type_label(rec.type_code),
        rec.offset,
        rec.end,
        rec.type_flags,
        rec.btf
    )
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

fn read_f64(data: &[u8], offset: usize) -> Option<f64> {
    let b = data.get(offset..offset.checked_add(8)?)?;
    Some(f64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

#[derive(Clone)]
struct LineGeometry {
    offset: usize,
    end_offset: usize,
    oid: u32,
    start: (f64, f64),
    end: (f64, f64),
    length: f64,
}

impl LineGeometry {
    fn summary(&self) -> String {
        format!(
            "range=0x{:06X}..0x{:06X} oid={} start=({:.12},{:.12}) end=({:.12},{:.12}) length={:.12}",
            self.offset,
            self.end_offset,
            self.oid,
            self.start.0,
            self.start.1,
            self.end.0,
            self.end.1,
            self.length
        )
    }

    fn anchors(&self) -> [(&'static str, f64); 11] {
        let min_x = self.start.0.min(self.end.0);
        let max_x = self.start.0.max(self.end.0);
        let min_y = self.start.1.min(self.end.1);
        let max_y = self.start.1.max(self.end.1);
        [
            ("start.x", self.start.0),
            ("start.y", self.start.1),
            ("end.x", self.end.0),
            ("end.y", self.end.1),
            ("bbox.min.x", min_x),
            ("bbox.max.x", max_x),
            ("bbox.min.y", min_y),
            ("bbox.max.y", max_y),
            ("extent.x", max_x - min_x),
            ("extent.y", max_y - min_y),
            ("length", self.length),
        ]
    }
}

fn igline_geometry(data: &[u8], rec: &Rec) -> Option<LineGeometry> {
    let line = pid_parse::parsers::sheet_records::decode_igline_at(data, rec.offset)?;
    Some(LineGeometry {
        offset: rec.offset,
        end_offset: rec.end,
        oid: line.oid,
        start: line.start,
        end: line.end,
        length: line.length(),
    })
}

/// A `0x0018 btf=50` record read with the strict `remaining_header == 12`
/// magic gate **relaxed**. The strict `decode_igline_at` rejects the neighbours
/// of every `0x0020` hit (their `payload+8` word is not `12`), so the prior
/// neighbour-correlation found "no decoded igLine2d neighbor". This relaxed
/// read pulls the same `(start, end)` f64 quad those records would carry *if*
/// they used the canonical igLine2d layout, so we can test whether the
/// `0x0020` candidate f64s line up with real neighbour endpoints/bbox or not.
#[derive(Clone)]
struct RelaxedLine {
    offset: usize,
    end_offset: usize,
    oid: u32,
    remaining_header: u32,
    finite_in_range: bool,
    line: LineGeometry,
}

impl RelaxedLine {
    fn summary(&self) -> String {
        format!(
            "range=0x{:06X}..0x{:06X} oid={} remaining_header={} finite_in_range={} start=({:.12},{:.12}) end=({:.12},{:.12}) length={:.12}",
            self.offset,
            self.end_offset,
            self.oid,
            self.remaining_header,
            self.finite_in_range,
            self.line.start.0,
            self.line.start.1,
            self.line.end.0,
            self.line.end.1,
            self.line.length
        )
    }
}

/// Read a `0x0018 btf=50` record's canonical `(start, end)` f64 quad without
/// enforcing the `remaining_header == 12` magic. Returns `None` for records
/// that are not `0x0018 btf=50` or that are truncated.
fn relaxed_igline_geometry(data: &[u8], rec: &Rec) -> Option<RelaxedLine> {
    if rec.type_code != 0x0018 || rec.btf != 50 {
        return None;
    }
    let payload = rec.offset.checked_add(PSM_HEADER_LEN)?;
    let oid = read_u32(data, payload)?;
    let remaining_header = read_u32(data, payload.checked_add(8)?)?;
    let sx = read_f64(data, payload.checked_add(18)?)?;
    let sy = read_f64(data, payload.checked_add(26)?)?;
    let ex = read_f64(data, payload.checked_add(34)?)?;
    let ey = read_f64(data, payload.checked_add(42)?)?;
    let vals = [sx, sy, ex, ey];
    let finite_in_range = vals.iter().all(|v| v.is_finite() && v.abs() <= 1.0e9);
    let dx = ex - sx;
    let dy = ey - sy;
    Some(RelaxedLine {
        offset: rec.offset,
        end_offset: rec.end,
        oid,
        remaining_header,
        finite_in_range,
        line: LineGeometry {
            offset: rec.offset,
            end_offset: rec.end,
            oid,
            start: (sx, sy),
            end: (ex, ey),
            length: (dx * dx + dy * dy).sqrt(),
        },
    })
}

fn candidate_matches(
    candidates: &[(usize, f64)],
    neighbors: &[(&str, &LineGeometry)],
) -> Vec<String> {
    candidates
        .iter()
        .map(|(offset, candidate)| {
            let best = neighbors
                .iter()
                .flat_map(|(side, line)| {
                    line.anchors().into_iter().map(move |(field, value)| {
                        (side, line.oid, field, value, (candidate - value).abs())
                    })
                })
                .min_by(|a, b| a.4.total_cmp(&b.4));
            match best {
                Some((side, oid, field, value, delta)) => format!(
                    "+{offset}:{candidate:.12} -> {side} oid={oid} {field}={value:.12} delta={delta:.12}"
                ),
                None => format!("+{offset}:{candidate:.12} -> no decoded igLine2d neighbor"),
            }
        })
        .collect()
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
    offset: usize,
    end: usize,
    type_flags: u16,
    btf: u32,
    payload: Vec<u8>,
    prev: Option<Rec>,
    next: Option<Rec>,
    prev_geometry: Option<LineGeometry>,
    next_geometry: Option<LineGeometry>,
    prev_relaxed: Option<RelaxedLine>,
    next_relaxed: Option<RelaxedLine>,
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
                    let prev = i.checked_sub(1).map(|j| records[j].clone());
                    let next = records.get(i + 1).cloned();
                    let prev_geometry = records[..i]
                        .iter()
                        .rev()
                        .find_map(|neighbor| igline_geometry(&bytes, neighbor));
                    let next_geometry = records
                        .get(i + 1..)
                        .unwrap_or_default()
                        .iter()
                        .find_map(|neighbor| igline_geometry(&bytes, neighbor));
                    let prev_relaxed = records[..i]
                        .iter()
                        .rev()
                        .find_map(|neighbor| relaxed_igline_geometry(&bytes, neighbor));
                    let next_relaxed = records
                        .get(i + 1..)
                        .unwrap_or_default()
                        .iter()
                        .find_map(|neighbor| relaxed_igline_geometry(&bytes, neighbor));
                    bucket.push(Sample {
                        fixture,
                        stream: sp.to_string_lossy().to_string(),
                        offset: rec.offset,
                        end: rec.end,
                        type_flags: rec.type_flags,
                        btf: rec.btf,
                        payload: bytes[rec.offset + PSM_HEADER_LEN..rec.end].to_vec(),
                        prev,
                        next,
                        prev_geometry,
                        next_geometry,
                        prev_relaxed,
                        next_relaxed,
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
                let doubles: Vec<String> = (0..9)
                    .filter_map(|w| {
                        let offset = w * 8;
                        read_f64(&s.payload, offset)
                            .filter(|v| v.is_finite())
                            .map(|v| format!("+{offset}:{v:.12}"))
                    })
                    .collect();
                let finite_double_candidates: Vec<(usize, f64)> =
                    (0..=s.payload.len().saturating_sub(8))
                        .filter_map(|offset| {
                            read_f64(&s.payload, offset)
                                .filter(|v| {
                                    v.is_finite() && (-10.0..=10.0).contains(v) && v.abs() >= 1.0e-6
                                })
                                .map(|v| (offset, v))
                        })
                        .take(12)
                        .collect();
                let candidate_text: Vec<String> = finite_double_candidates
                    .iter()
                    .map(|(offset, v)| format!("+{offset}:{v:.12}"))
                    .collect();
                let mut neighbors = Vec::new();
                if let Some(line) = &s.prev_geometry {
                    neighbors.push(("prev", line));
                }
                if let Some(line) = &s.next_geometry {
                    neighbors.push(("next", line));
                }
                let matches = candidate_matches(&finite_double_candidates, &neighbors);
                println!(
                    "\n  SAMPLE[{}] {} {}  range=0x{:06X}..0x{:06X} ({}..{}) flags={} btf={} payload_len={}",
                    k,
                    s.fixture,
                    s.stream,
                    s.offset,
                    s.end,
                    s.offset,
                    s.end,
                    s.type_flags,
                    s.btf,
                    s.payload.len()
                );
                println!(
                    "    context: prev={} next={}",
                    s.prev
                        .as_ref()
                        .map(record_summary)
                        .unwrap_or_else(|| "<none>".to_string()),
                    s.next
                        .as_ref()
                        .map(record_summary)
                        .unwrap_or_else(|| "<none>".to_string()),
                );
                if let Some(summary) = &s.prev_geometry {
                    println!("    prev decoded igLine2d geometry: {}", summary.summary());
                }
                if let Some(summary) = &s.next_geometry {
                    println!("    next decoded igLine2d geometry: {}", summary.summary());
                }
                if let Some(relaxed) = &s.prev_relaxed {
                    println!(
                        "    prev RELAXED igLine2d (no magic gate): {}",
                        relaxed.summary()
                    );
                }
                if let Some(relaxed) = &s.next_relaxed {
                    println!(
                        "    next RELAXED igLine2d (no magic gate): {}",
                        relaxed.summary()
                    );
                }
                let mut relaxed_lines: Vec<(&'static str, LineGeometry)> = Vec::new();
                if let Some(relaxed) = &s.prev_relaxed {
                    relaxed_lines.push(("prev~", relaxed.line.clone()));
                }
                if let Some(relaxed) = &s.next_relaxed {
                    relaxed_lines.push(("next~", relaxed.line.clone()));
                }
                let relaxed_refs: Vec<(&str, &LineGeometry)> = relaxed_lines
                    .iter()
                    .map(|(side, line)| (*side, line))
                    .collect();
                let relaxed_matches = candidate_matches(&finite_double_candidates, &relaxed_refs);
                if relaxed_matches.is_empty() {
                    println!("    RELAXED neighbor candidate matches: <none>");
                } else {
                    println!("    RELAXED neighbor candidate matches:");
                    for m in relaxed_matches {
                        println!("      {m}");
                    }
                }
                println!("    u32 words: {}", words.join("  "));
                println!("    f64 words: {}", doubles.join("  "));
                println!(
                    "    f64 finite [-10,10] candidates: {}",
                    candidate_text.join("  ")
                );
                if matches.is_empty() {
                    println!("    neighbor candidate matches: <none>");
                } else {
                    println!("    neighbor candidate matches:");
                    for m in matches {
                        println!("      {m}");
                    }
                }
                hexdump(&s.payload, 96);
            }
        }
    }

    Ok(())
}
