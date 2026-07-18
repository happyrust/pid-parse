//! Phase 34-D pre-decoder probe — pin the `0x67` tag grammar and the
//! vertex/segment-count field of `0x0013 igBoundary2d` (20 top-level hits,
//! all `bytes_to_follow = 172`, primary `/Sheet6` only; see
//! `docs/analysis/2026-06-30-phase34-0013-003d-evidence-closeout.md`).
//!
//! For every `0x0013` record in the six local fixtures this probe dumps:
//!
//! - the 18-byte common prefix (`oid`, `parent_ref`, `remaining_header`,
//!   `sub_type`, `index`) plus the sub-header bytes before the first tag;
//! - every `0x67` tag position and the deltas between consecutive tags;
//! - the hypothesized group parse `tag(1) + 4×f64(32)` per segment, with
//!   values and finite/domain checks;
//! - the trailer bytes after the last group;
//! - a segment-chaining test: do the four segments form a closed loop
//!   (`seg[i].end == seg[i+1].start`, `seg[3].end == seg[0].start`, exact
//!   f64 bit equality)?
//!
//! Read-only probe; run with
//! `cargo run --quiet --example probe_0013_igboundary2d_grammar`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_HEADER_LEN: usize = 6;
const TARGET_TYPE: u16 = 0x0013;
const TAG_BYTE: u8 = 0x67;

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

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let b = data.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

fn read_f64(data: &[u8], offset: usize) -> Option<f64> {
    let b = data.get(offset..offset.checked_add(8)?)?;
    Some(f64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn hexdump(payload: &[u8]) {
    for start in (0..payload.len()).step_by(16) {
        let end = (start + 16).min(payload.len());
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
        println!("      +{:03}: {:<47} | {}", start, hex, ascii);
    }
}

struct Segment {
    tag_offset: usize,
    start: (f64, f64),
    end: (f64, f64),
}

/// One 8-byte trailer reference: `u32 member_oid + u16 class + u16 sub`.
struct MemberRef {
    oid: u32,
    class: u16,
    sub: u16,
}

/// `(start, end)` endpoint pair of a relaxed `0x0018` line read.
type LineEndpoints = ((f64, f64), (f64, f64));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ];

    let mut total = 0usize;
    let mut tag_layout_sets: BTreeMap<String, usize> = BTreeMap::new();
    let mut subheader_patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut trailer_patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut closed_loops = 0usize;
    let mut open_chains = 0usize;
    let mut bad_parses = 0usize;
    let mut member_class_ok = 0usize;
    let mut member_class_bad = 0usize;
    let mut members_found = 0usize;
    let mut members_missing = 0usize;
    let mut member_geom_match = 0usize;
    let mut member_geom_mismatch = 0usize;
    let mut anchor_inside = 0usize;
    let mut anchor_outside = 0usize;

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
            // oid -> (start, end) for every 0x0018 btf=50 record in this
            // stream (relaxed read at payload +18.. like the Phase 34-B
            // probe; canonical records also satisfy remaining_header == 12).
            let mut lines_by_oid: BTreeMap<u32, LineEndpoints> = BTreeMap::new();
            for r in records
                .iter()
                .filter(|r| r.type_code == 0x0018 && r.btf == 50)
            {
                let p = r.offset + PSM_HEADER_LEN;
                if let (Some(oid), Some(sx), Some(sy), Some(ex), Some(ey)) = (
                    read_u32(&bytes, p),
                    read_f64(&bytes, p + 18),
                    read_f64(&bytes, p + 26),
                    read_f64(&bytes, p + 34),
                    read_f64(&bytes, p + 42),
                ) {
                    lines_by_oid.insert(oid, ((sx, sy), (ex, ey)));
                }
            }
            for rec in records.iter().filter(|r| r.type_code == TARGET_TYPE) {
                total += 1;
                let payload = &bytes[rec.offset + PSM_HEADER_LEN..rec.end];
                println!(
                    "\nRECORD[{total}] {fixture} {} range=0x{:06X}..0x{:06X} flags={} btf={} payload_len={}",
                    sp.to_string_lossy(),
                    rec.offset,
                    rec.end,
                    rec.type_flags,
                    rec.btf,
                    payload.len()
                );

                // --- 18-byte common prefix (igLine2d-shaped) ---
                let oid = read_u32(payload, 0).unwrap_or(u32::MAX);
                let parent_ref = read_u32(payload, 4).unwrap_or(u32::MAX);
                let remaining_header = read_u32(payload, 8).unwrap_or(u32::MAX);
                let sub_type = read_u16(payload, 12).unwrap_or(u16::MAX);
                let index = read_u32(payload, 14).unwrap_or(u32::MAX);
                println!(
                    "    prefix: oid={oid} parent_ref={parent_ref} remaining_header={remaining_header} sub_type=0x{sub_type:04X} index={index}"
                );

                // --- tag scan ---
                let tag_positions: Vec<usize> = payload
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| (*b == TAG_BYTE).then_some(i))
                    .collect();
                let deltas: Vec<String> = tag_positions
                    .windows(2)
                    .map(|w| (w[1] - w[0]).to_string())
                    .collect();
                println!(
                    "    0x67 tag positions: {:?} deltas: [{}]",
                    tag_positions,
                    deltas.join(", ")
                );
                tag_layout_sets
                    .entry(format!("{tag_positions:?}"))
                    .and_modify(|c| *c += 1)
                    .or_insert(1);

                // --- sub-header between prefix and first tag ---
                if let Some(&first_tag) = tag_positions.first() {
                    let sub = &payload[18..first_tag.min(payload.len())];
                    let sub_hex = sub
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("    sub-header +18..+{first_tag}: {sub_hex}");
                    subheader_patterns
                        .entry(sub_hex.clone())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    // candidate count fields (u8 / u16 / u32 at every sub-header offset)
                    let mut candidates = Vec::new();
                    for off in 18..first_tag {
                        let v8 = payload[off];
                        if (1..=16).contains(&v8) {
                            candidates.push(format!("+{off}:u8={v8}"));
                        }
                        if let Some(v16) = read_u16(payload, off) {
                            if (1..=16).contains(&v16) {
                                candidates.push(format!("+{off}:u16={v16}"));
                            }
                        }
                        if let Some(v32) = read_u32(payload, off) {
                            if (1..=16).contains(&v32) {
                                candidates.push(format!("+{off}:u32={v32}"));
                            }
                        }
                    }
                    println!(
                        "    count-field candidates (1..=16): {}",
                        candidates.join("  ")
                    );
                }

                // --- hypothesized group parse: tag(1) + 4 f64 per segment ---
                let mut segments: Vec<Segment> = Vec::new();
                let mut pos = tag_positions.first().copied().unwrap_or(payload.len());
                let mut parse_ok = true;
                while pos + 33 <= payload.len() && payload[pos] == TAG_BYTE {
                    let vals: Vec<f64> = (0..4)
                        .filter_map(|i| read_f64(payload, pos + 1 + i * 8))
                        .collect();
                    if vals.len() != 4 || !vals.iter().all(|v| v.is_finite() && v.abs() <= 10.0) {
                        parse_ok = false;
                        println!("    GROUP at +{pos}: NON-FINITE/OUT-OF-DOMAIN values {vals:?}");
                        break;
                    }
                    segments.push(Segment {
                        tag_offset: pos,
                        start: (vals[0], vals[1]),
                        end: (vals[2], vals[3]),
                    });
                    pos += 33;
                }
                for s in &segments {
                    println!(
                        "    seg@+{:03}: start=({:.12},{:.12}) end=({:.12},{:.12})",
                        s.tag_offset, s.start.0, s.start.1, s.end.0, s.end.1
                    );
                }
                // --- structured trailer parse ---
                // Hypothesis: after the last group at `pos` the payload holds
                //   f64 anchor.x, f64 anchor.y, u8 flag, u32 member_count,
                //   member_count × { u32 member_oid, u16 class, u16 sub }.
                let seg_count_field = read_u32(payload, 22).unwrap_or(0);
                let anchor_x = read_f64(payload, pos);
                let anchor_y = read_f64(payload, pos + 8);
                let flag = payload.get(pos + 16).copied();
                let member_count = read_u32(payload, pos + 17).unwrap_or(0) as usize;
                let mut members: Vec<MemberRef> = Vec::new();
                let member_base = pos + 21;
                for i in 0..member_count {
                    let off = member_base + i * 8;
                    if let (Some(oid), Some(class), Some(sub)) = (
                        read_u32(payload, off),
                        read_u16(payload, off + 4),
                        read_u16(payload, off + 6),
                    ) {
                        members.push(MemberRef { oid, class, sub });
                    }
                }
                let consumed = member_base + member_count * 8;
                println!(
                    "    trailer: anchor=({:?},{:?}) flag={:?} member_count={} seg_count_field(+22)={} consumed={} payload_len={}",
                    anchor_x,
                    anchor_y,
                    flag,
                    member_count,
                    seg_count_field,
                    consumed,
                    payload.len()
                );
                let member_text: Vec<String> = members
                    .iter()
                    .map(|m| format!("oid={} class=0x{:04X} sub={}", m.oid, m.class, m.sub))
                    .collect();
                println!("    members: {}", member_text.join("  "));
                trailer_patterns
                    .entry(format!(
                        "trailer_len={} flag={:?} member_count={} consumed_exact={}",
                        payload.len() - pos,
                        flag,
                        member_count,
                        consumed == payload.len()
                    ))
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                if members.iter().all(|m| m.class == 0x00CB) && !members.is_empty() {
                    member_class_ok += 1;
                } else {
                    member_class_bad += 1;
                }

                // --- member oid -> igLine2d record geometry correlation ---
                let close = |a: f64, b: f64| (a - b).abs() <= 1e-9;
                let pt_close = |a: (f64, f64), b: (f64, f64)| close(a.0, b.0) && close(a.1, b.1);
                for (i, m) in members.iter().enumerate() {
                    match lines_by_oid.get(&m.oid) {
                        Some((ls, le)) => {
                            members_found += 1;
                            let verdict = if let Some(seg) = segments.get(i) {
                                let fwd = pt_close(seg.start, *ls) && pt_close(seg.end, *le);
                                let rev = pt_close(seg.start, *le) && pt_close(seg.end, *ls);
                                if fwd || rev {
                                    member_geom_match += 1;
                                    if fwd {
                                        "MATCH seg[i] fwd"
                                    } else {
                                        "MATCH seg[i] rev"
                                    }
                                } else {
                                    member_geom_mismatch += 1;
                                    "NO seg[i] match"
                                }
                            } else {
                                member_geom_mismatch += 1;
                                "no segment[i]"
                            };
                            println!(
                                "    member[{i}] oid={} -> 0x0018 line start=({:.12},{:.12}) end=({:.12},{:.12}) {verdict}",
                                m.oid, ls.0, ls.1, le.0, le.1
                            );
                        }
                        None => {
                            members_missing += 1;
                            println!(
                                "    member[{i}] oid={} -> NOT FOUND among 0x0018 btf=50 records",
                                m.oid
                            );
                        }
                    }
                }

                // --- anchor inside segment bbox? ---
                if let (Some(ax), Some(ay)) = (anchor_x, anchor_y) {
                    if !segments.is_empty() {
                        let xs: Vec<f64> =
                            segments.iter().flat_map(|s| [s.start.0, s.end.0]).collect();
                        let ys: Vec<f64> =
                            segments.iter().flat_map(|s| [s.start.1, s.end.1]).collect();
                        let (min_x, max_x) = (
                            xs.iter().cloned().fold(f64::INFINITY, f64::min),
                            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                        );
                        let (min_y, max_y) = (
                            ys.iter().cloned().fold(f64::INFINITY, f64::min),
                            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                        );
                        let inside = (min_x..=max_x).contains(&ax) && (min_y..=max_y).contains(&ay);
                        println!(
                            "    anchor in segment bbox: {inside} (bbox x=[{min_x:.12},{max_x:.12}] y=[{min_y:.12},{max_y:.12}])"
                        );
                        if inside {
                            anchor_inside += 1;
                        } else {
                            anchor_outside += 1;
                        }
                    }
                }

                // --- chaining / closure with 1e-9 tolerance (ulp noise seen) ---
                if parse_ok && segments.len() >= 2 {
                    let chained = segments.windows(2).all(|w| pt_close(w[0].end, w[1].start));
                    let closed =
                        chained && pt_close(segments[segments.len() - 1].end, segments[0].start);
                    println!(
                        "    chain(1e-9): consecutive={} closed-loop={} segments={} seg_count_field_match={}",
                        chained,
                        closed,
                        segments.len(),
                        segments.len() == seg_count_field as usize
                    );
                    if closed {
                        closed_loops += 1;
                    } else {
                        open_chains += 1;
                    }
                } else {
                    bad_parses += 1;
                }

                hexdump(payload);
            }
        }
    }

    println!("\n==================== SUMMARY ====================");
    println!("total 0x0013 records: {total}");
    println!("closed loops (1e-9):  {closed_loops}");
    println!("open/non-chained:     {open_chains}");
    println!("bad group parses:     {bad_parses}");
    println!("member class all 0x00CB: {member_class_ok} / bad {member_class_bad}");
    println!("member oid resolved: {members_found} / missing {members_missing}");
    println!("member geometry match: {member_geom_match} / mismatch {member_geom_mismatch}");
    println!("anchor inside bbox: {anchor_inside} / outside {anchor_outside}");
    println!("distinct tag layouts:");
    for (layout, n) in &tag_layout_sets {
        println!("  ×{n}  {layout}");
    }
    println!("distinct sub-headers (+18..first tag):");
    for (pat, n) in &subheader_patterns {
        println!("  ×{n}  [{pat}]");
    }
    println!("distinct trailers:");
    for (pat, n) in &trailer_patterns {
        println!("  ×{n}  {pat}");
    }

    Ok(())
}
