//! Which rule refuses the refusals `tests/render_gap_census.rs` still counts,
//! and is each one right?
//!
//! The census names five records on the two DWG drawings without saying why:
//! four `igLineString2d` on DWG-0202 `/Sheet6` and one `DependencyObject` on
//! DWG-0201 `/Sheet6` (the gongyi drawing's eight line strings were judged a
//! correct refusal in `docs/analysis/2026-08-11-what-refuses-the-remaining-53.md`
//! and are here for comparison). This walks each `Sheet6` record chain the
//! way `parsers::undecoded_census` does, keeps the chain records of the two
//! codes no decoded record starts at, and replays the family's validation
//! rules in order to name the first that says no -- then prints what the
//! accepted records of the same family on the same sheet look like, so the
//! refusal can be judged against them.
//!
//! ```powershell
//! cargo run --example probe_the_last_refusals
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use pid_parse::parsers::sheet_records::{
    decode_dependency_objects, decode_iglinestrings, DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN,
    IGLINESTRING2D_MIN_PAYLOAD_LEN, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_DEPENDENCY_OBJECT,
    PSM_TYPE_CODE_IGLINESTRING2D,
};

/// The decoder's private bounds, restated so the replay can name them.
const IGLINESTRING2D_MAX_VERTEX_COUNT: usize = 10_000;
const IGLINESTRING2D_FORM_MAX: u8 = 6;
const DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN: usize = 512;
const COORDINATE_DOMAIN_LIMIT: f64 = 1e9;

/// The census's own bounds on a chain record's `bytes_to_follow`.
const CENSUS_MIN_BYTES_TO_FOLLOW: usize = 8;
const CENSUS_MAX_BYTES_TO_FOLLOW: usize = 100_000;

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
];

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn f64_at(d: &[u8], at: usize) -> Option<f64> {
    d.get(at..at + 8)
        .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

/// `undecoded_census::psm_record_end`, restated.
fn record_end(data: &[u8], off: usize) -> Option<usize> {
    let header_end = off.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    if u16_at(data, off)? & 0x3FFF == 0 {
        return None;
    }
    let btf = u32_at(data, off + 2)? as usize;
    if !(CENSUS_MIN_BYTES_TO_FOLLOW..=CENSUS_MAX_BYTES_TO_FOLLOW).contains(&btf) {
        return None;
    }
    let end = header_end.checked_add(btf)?;
    (end <= data.len()).then_some(end)
}

/// Every chain record `(start, end, type_code)`, the census's walk.
fn chain(data: &[u8]) -> Vec<(usize, usize, u16)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 6 <= data.len() {
        if let Some(end) = record_end(data, off) {
            if end == data.len() || record_end(data, end).is_some() {
                out.push((off, end, u16_at(data, off).unwrap_or(0) & 0x3FFF));
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

/// The first `igLineString2d` rule the record at `off` fails, or `None` if it
/// passes them all (in which case the scan skipped it for another reason).
fn linestring_refusal(data: &[u8], off: usize) -> Option<String> {
    let btf = u32_at(data, off + 2)? as usize;
    if btf < IGLINESTRING2D_MIN_PAYLOAD_LEN {
        return Some(format!("btf {btf} < {IGLINESTRING2D_MIN_PAYLOAD_LEN}"));
    }
    if !(btf - 24).is_multiple_of(16) {
        return Some(format!(
            "(btf {btf} - 24) is not a whole number of vertices"
        ));
    }
    let computed = (btf - 24) / 16;
    if !(2..=IGLINESTRING2D_MAX_VERTEX_COUNT).contains(&computed) {
        return Some(format!("{computed} vertices out of range"));
    }
    let p = &data[off + PSM_ENVELOPE_LEN..off + PSM_ENVELOPE_LEN + btf];
    let layer_word = u32_at(p, 8)?;
    if layer_word > 0xFFFF {
        return Some(format!("+8 {layer_word:#x} > 0xFFFF"));
    }
    let inline = u32_at(p, 18)? as usize;
    if inline != computed {
        return Some(format!(
            "inline vertex count {inline} != {computed} from btf"
        ));
    }
    let (form, scope) = (p[22], p[23]);
    if form > IGLINESTRING2D_FORM_MAX {
        return Some(format!("form {form} > {IGLINESTRING2D_FORM_MAX}"));
    }
    if scope > 4 && scope != 6 {
        return Some(format!("scope {scope} not in 0..=4 or 6"));
    }
    let mut vertices = Vec::new();
    for i in 0..computed {
        let (x, y) = (f64_at(p, 24 + i * 16)?, f64_at(p, 32 + i * 16)?);
        if !x.is_finite()
            || !y.is_finite()
            || x.abs() > COORDINATE_DOMAIN_LIMIT
            || y.abs() > COORDINATE_DOMAIN_LIMIT
        {
            return Some(format!("vertex {i} ({x}, {y}) not a coordinate"));
        }
        vertices.push((x, y));
    }
    let first = vertices[0];
    if vertices
        .iter()
        .all(|v| (v.0 - first.0).abs() <= 1e-12 && (v.1 - first.1).abs() <= 1e-12)
    {
        return Some(format!(
            "degenerate: all {computed} vertices at ({:.6}, {:.6})",
            first.0, first.1
        ));
    }
    None
}

/// The first `DependencyObject` rule the record at `off` fails.
fn dependency_refusal(data: &[u8], off: usize) -> Option<String> {
    let flags = u16_at(data, off)? >> 14;
    if flags != 0 {
        return Some(format!("type flags {flags} != 0"));
    }
    let btf = u32_at(data, off + 2)? as usize;
    if !(DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN..=DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN).contains(&btf) {
        return Some(format!(
            "btf {btf} outside [{DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN}, {DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN}]"
        ));
    }
    if !btf.is_multiple_of(2) {
        return Some(format!("btf {btf} odd"));
    }
    let p = &data[off + PSM_ENVELOPE_LEN..off + PSM_ENVELOPE_LEN + btf];
    if u32_at(p, 0)? == 0 {
        return Some("oid 0".to_string());
    }
    let parent = u32_at(p, 4)?;
    if parent != 6 {
        return Some(format!("parent_ref {parent} != 6"));
    }
    if p.get(8..14)? != [0u8; 6].as_slice() {
        return Some(format!("+8..14 not zero: {:02X?}", &p[8..14]));
    }
    let kind = u16_at(p, 14)?;
    if kind == 0 || kind > 16 {
        return Some(format!("group_kind_word {kind} outside 1..=16"));
    }
    None
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_stream(path: &str, stream: &str) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut cfb = cfb::CompoundFile::open(file).ok()?;
    let mut s = cfb.open_stream(stream).ok()?;
    let mut data = Vec::new();
    s.read_to_end(&mut data).ok()?;
    Some(data)
}

fn main() {
    for fixture in FIXTURES {
        let Some(data) = read_stream(fixture, "/Sheet6") else {
            println!("UNREAD {fixture}");
            continue;
        };
        let records = chain(&data);
        let lines: BTreeSet<usize> = decode_iglinestrings(&data)
            .iter()
            .map(|r| r.byte_range.start)
            .collect();
        let deps = decode_dependency_objects(&data);
        let dep_starts: BTreeSet<usize> = deps.iter().map(|r| r.byte_range.start).collect();
        let ends: BTreeMap<usize, usize> = records.iter().map(|(s, e, _)| (*s, *e)).collect();
        println!("\n=== {fixture} /Sheet6: {} chain records", records.len());

        for (start, end, code) in &records {
            let (claimed, refusal) = match *code {
                PSM_TYPE_CODE_IGLINESTRING2D => {
                    (lines.contains(start), linestring_refusal(&data, *start))
                }
                PSM_TYPE_CODE_DEPENDENCY_OBJECT => (
                    dep_starts.contains(start),
                    dependency_refusal(&data, *start),
                ),
                _ => continue,
            };
            if claimed {
                continue;
            }
            let payload = &data[start + PSM_ENVELOPE_LEN..*end];
            println!(
                "  refused 0x{code:04X} @ 0x{start:06X} btf={} oid={:?} parent={:?} +8={:?} -> {}",
                payload.len(),
                u32_at(payload, 0),
                u32_at(payload, 4),
                u32_at(payload, 8),
                refusal.unwrap_or_else(|| "passes every rule (skipped by the scan)".to_string())
            );
            if *code == PSM_TYPE_CODE_IGLINESTRING2D {
                let vc = u32_at(payload, 18).unwrap_or(0) as usize;
                let pts: Vec<String> = (0..vc.min(8))
                    .filter_map(|i| {
                        Some(format!(
                            "({:.4}, {:.4})",
                            f64_at(payload, 24 + i * 16)?,
                            f64_at(payload, 32 + i * 16)?
                        ))
                    })
                    .collect();
                println!(
                    "      index={:?} form={} scope={} vertices {vc}: {}",
                    u32_at(payload, 14),
                    payload.get(22).copied().unwrap_or(0),
                    payload.get(23).copied().unwrap_or(0),
                    pts.join(" ")
                );
            } else {
                println!("      payload: {}", hex(payload));
            }
            // The neighbours: what the chain holds right before and after.
            let before = records.iter().rev().find(|(_, e, _)| e == start);
            let after = ends.get(end).map(|e| (*end, *e));
            if let Some((s, _, c)) = before {
                println!("      previous record 0x{c:04X} @ 0x{s:06X}");
            }
            if let Some((s, _)) = after {
                println!(
                    "      next record 0x{:04X} @ 0x{s:06X}",
                    u16_at(&data, s).unwrap_or(0) & 0x3FFF
                );
            }
        }

        // What the accepted ones of the same families look like here.
        let mut dep_shapes: BTreeMap<(u32, u16, usize), usize> = BTreeMap::new();
        for d in &deps {
            *dep_shapes
                .entry((d.parent_ref, d.group_kind_word, d.bytes_to_follow as usize))
                .or_default() += 1;
        }
        println!("  accepted DependencyObject (parent, kind, btf) -> n: {dep_shapes:?}");
        let mut line_shapes: BTreeMap<(u8, u8, usize), usize> = BTreeMap::new();
        for l in decode_iglinestrings(&data) {
            *line_shapes
                .entry((l.form, l.scope, l.vertices.len()))
                .or_default() += 1;
        }
        println!("  accepted igLineString2d (form, scope, vertices) -> n: {line_shapes:?}");
    }
}
