//! Is `0x0030 JStyleOverride` how geometry reaches a style?
//!
//! `2026-08-04-geometry-to-style-link-negative.md` ruled out the dependency
//! records, `sub_type_word` and `index`. The name is the reason this one is
//! next: an *override* implies geometry normally takes a default and only the
//! exceptions are written down, which would explain why no per-line style id
//! exists to be found.
//!
//! The prediction that distinguishes it from the failed candidates is
//! different, and worth stating before looking. A per-line id field has to
//! cover nearly every line. An override table must NOT -- it should cover a
//! minority, and each entry should name both a piece of geometry and a style
//! the cluster defines, at a *fixed* offset. Sparse coverage is a pass here,
//! where it was a failure before; scattered offsets are still a failure.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols, decode_igtextboxes,
};

const PSM_JSTYLEOVERRIDE: u16 = 0x0030;
const PSM_TEXT_CHAR_STYLE: u16 = 0x002C;
const PSM_SIMPLE_LINE_STYLE: u16 = 0x002E;
const STYLE_ID_AT: usize = 14;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn all_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push((name, data));
        }
    }
    out
}

/// Records of one type from a stream that is a plain PSM chain.
fn chain_records(data: &[u8], want: u16) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = STREAM_HEADER;
    while at + ENVELOPE <= data.len() {
        let (Some(type_word), Some(btf)) = (u16_at(data, at), u32_at(data, at + 2)) else {
            break;
        };
        let end = at + ENVELOPE + btf as usize;
        if type_word & 0x3FFF == 0 || btf == 0 || end > data.len() {
            break;
        }
        if type_word & 0x3FFF == want {
            out.push(data[at + ENVELOPE..end].to_vec());
        }
        at = end;
    }
    out
}

/// Records of one type found by sliding-window scan, for streams whose framing
/// is not a plain chain (the Sheet streams interleave families).
fn scan_records(data: &[u8], want: u16, min: usize, max: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + ENVELOPE <= data.len() {
        let (Some(type_word), Some(btf)) = (u16_at(data, at), u32_at(data, at + 2)) else {
            break;
        };
        let btf = btf as usize;
        if type_word & 0x3FFF == want
            && (min..=max).contains(&btf)
            && at + ENVELOPE + btf <= data.len()
        {
            out.push(data[at + ENVELOPE..at + ENVELOPE + btf].to_vec());
            at += ENVELOPE + btf;
            continue;
        }
        at += 1;
    }
    out
}

fn oid_families(data: &[u8]) -> BTreeMap<u32, &'static str> {
    let mut map = BTreeMap::new();
    for r in decode_iglines(data) {
        map.insert(r.oid, "igLine2d");
    }
    for r in decode_igpoints(data) {
        map.insert(r.oid, "igPoint2d");
    }
    for r in decode_iglinestrings(data) {
        map.insert(r.oid, "igLineString2d");
    }
    for r in decode_igtextboxes(data) {
        map.insert(r.oid, "igTextBox");
    }
    for r in decode_igsymbols(data) {
        map.insert(r.oid, "igSymbol2d");
    }
    map
}

fn probe(path: &Path) {
    println!("\n=== {} ===", path.display());
    let streams = all_streams(path);

    // Style ids the cluster defines.
    let mut line_ids = BTreeSet::new();
    let mut text_ids = BTreeSet::new();
    for (name, data) in &streams {
        if !name.to_lowercase().contains("style") {
            continue;
        }
        for (code, set) in [
            (PSM_SIMPLE_LINE_STYLE, &mut line_ids),
            (PSM_TEXT_CHAR_STYLE, &mut text_ids),
        ] {
            for payload in chain_records(data, code) {
                if let Some(id) = u32_at(&payload, STYLE_ID_AT) {
                    set.insert(id);
                }
            }
        }
    }
    println!(
        "  defined: {} line style id(s), {} text style id(s)",
        line_ids.len(),
        text_ids.len()
    );

    // Where do the overrides live, and how many?
    println!("  0x0030 records per stream:");
    let mut sheet: Option<(&String, &Vec<u8>)> = None;
    for (name, data) in &streams {
        let chained = chain_records(data, PSM_JSTYLEOVERRIDE).len();
        let scanned = scan_records(data, PSM_JSTYLEOVERRIDE, 40, 400).len();
        if chained == 0 && scanned == 0 {
            continue;
        }
        println!("    {name:<40} chain={chained:<4} scan={scanned}");
        if name.contains("Sheet") && sheet.is_none() {
            sheet = Some((name, data));
        }
    }

    let Some((name, data)) = sheet else {
        println!("  no Sheet stream carries 0x0030 -- overrides are style-cluster only");
        return;
    };

    let families = oid_families(data);
    let lines: BTreeSet<u32> = families
        .iter()
        .filter(|(_, f)| **f == "igLine2d")
        .map(|(o, _)| *o)
        .collect();
    let overrides = scan_records(data, PSM_JSTYLEOVERRIDE, 40, 400);
    println!(
        "\n  -- {name}: {} override(s), {} igLine2d, {} known OID(s) --",
        overrides.len(),
        lines.len(),
        families.len()
    );
    if overrides.is_empty() {
        return;
    }

    // Fixed-offset test: does one offset consistently name geometry, and
    // another consistently name a style the cluster defines?
    let width = overrides.iter().map(Vec::len).min().unwrap_or(0);
    println!(
        "    payload sizes: {:?}",
        overrides.iter().map(Vec::len).collect::<BTreeSet<_>>()
    );
    println!("    offsets naming known geometry:");
    for at in 0..width.saturating_sub(3) {
        let hits: Vec<&'static str> = overrides
            .iter()
            .filter_map(|p| u32_at(p, at))
            .filter_map(|v| families.get(&v).copied())
            .collect();
        if hits.len() * 2 < overrides.len() {
            continue;
        }
        let kinds: BTreeSet<&str> = hits.iter().copied().collect();
        println!(
            "      +{at:<3} {}/{} -> {:?}",
            hits.len(),
            overrides.len(),
            kinds
        );
    }
    println!("    offsets naming a defined style id:");
    let mut any = false;
    for at in 0..width.saturating_sub(3) {
        let values: Vec<u32> = overrides.iter().filter_map(|p| u32_at(p, at)).collect();
        let hits = values
            .iter()
            .filter(|v| line_ids.contains(v) || text_ids.contains(v))
            .count();
        if hits * 2 < overrides.len() {
            continue;
        }
        any = true;
        let distinct: BTreeSet<u32> = values.iter().copied().collect();
        println!(
            "      +{at:<3} {hits}/{} ; {} distinct: {:?}",
            overrides.len(),
            distinct.len(),
            distinct.iter().take(10).collect::<Vec<_>>()
        );
    }
    if !any {
        println!("      none");
    }
}

fn main() {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path);
        }
    }
}
