//! How does a piece of geometry get to its style?
//!
//! Text already has an answer: phase 35-D showed `igTextBox` carries a style id
//! in its own trailer, and that id indexes the `0x002C` records in the style
//! cluster. Line work does not -- `igLine2d` is 50 fixed bytes with every one
//! accounted for, so the id has to come from somewhere else.
//!
//! Two candidates, both testable from the corpus:
//!
//! * **`0x00FA` Dependency Object** -- the most numerous record in every Sheet
//!   stream, and it carries two OID references. If it is what links geometry to
//!   style, some of those references should point at `igLine2d` while another
//!   field holds a value the style cluster actually defines.
//! * **`0x0010`** -- 638 cross-fixture hits, described as attribute fragments
//!   embedded in other records. If line style rides along with the geometry it
//!   would be here.
//!
//! The test both share: a style id has to be one the style cluster defines.
//! Checking candidate fields against the real id set is what separates a link
//! from a small integer that happens to be in range.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_dependency_objects, decode_iglines, decode_iglinestrings, decode_igpoints,
    decode_igsymbols, decode_igtextboxes,
};

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

fn streams(path: &Path, filter: impl Fn(&str) -> bool) -> Vec<(String, Vec<u8>)> {
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
        .filter(|p| filter(p))
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

/// Style ids the cluster actually defines, by style family.
fn defined_style_ids(path: &Path) -> BTreeMap<u16, BTreeSet<u32>> {
    let mut out: BTreeMap<u16, BTreeSet<u32>> = BTreeMap::new();
    for (_, data) in streams(path, |p| p.to_lowercase().contains("style")) {
        let mut at = STREAM_HEADER;
        while at + ENVELOPE <= data.len() {
            let (Some(type_word), Some(btf)) = (u16_at(&data, at), u32_at(&data, at + 2)) else {
                break;
            };
            let end = at + ENVELOPE + btf as usize;
            let code = type_word & 0x3FFF;
            if code == 0 || btf == 0 || end > data.len() {
                break;
            }
            if code == PSM_TEXT_CHAR_STYLE || code == PSM_SIMPLE_LINE_STYLE {
                if let Some(id) = u32_at(&data, at + ENVELOPE + STYLE_ID_AT) {
                    out.entry(code).or_default().insert(id);
                }
            }
            at = end;
        }
    }
    out
}

/// Which family each OID in a Sheet stream belongs to.
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

    let ids = defined_style_ids(path);
    let line_ids = ids.get(&PSM_SIMPLE_LINE_STYLE).cloned().unwrap_or_default();
    let text_ids = ids.get(&PSM_TEXT_CHAR_STYLE).cloned().unwrap_or_default();
    println!(
        "  style ids defined: {} line style(s) {:?}..{:?}, {} text style(s)",
        line_ids.len(),
        line_ids.iter().next(),
        line_ids.iter().next_back(),
        text_ids.len()
    );

    for (name, data) in streams(path, |p| p.contains("Sheet")) {
        let families = oid_families(&data);
        let lines: BTreeSet<u32> = families
            .iter()
            .filter(|(_, f)| **f == "igLine2d")
            .map(|(oid, _)| *oid)
            .collect();
        let deps = decode_dependency_objects(&data);
        if deps.is_empty() {
            continue;
        }
        println!(
            "\n  -- {name}: {} dependency record(s), {} igLine2d, {} known OID(s) --",
            deps.len(),
            lines.len(),
            families.len()
        );

        // Does any dependency reference a line, and if so what else does it hold?
        let mut refs_a_line = 0usize;
        let mut small_by_offset: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for dep in &deps {
            let tail = &dep.raw_reference_payload;
            let targets: Vec<(usize, u32)> = (0..tail.len().saturating_sub(3))
                .filter_map(|at| u32_at(tail, at).map(|v| (at, v)))
                .filter(|(_, v)| lines.contains(v))
                .collect();
            if targets.is_empty() {
                continue;
            }
            refs_a_line += 1;
            // Collect every word that could be a line style id.
            for at in 0..tail.len().saturating_sub(3) {
                if let Some(v) = u32_at(tail, at) {
                    if line_ids.contains(&v) {
                        small_by_offset.entry(at).or_default().push(v);
                    }
                }
            }
        }
        println!(
            "    dependency records referencing an igLine2d: {refs_a_line}/{}",
            deps.len()
        );
        if refs_a_line == 0 {
            println!("    -> dependencies do not point at line geometry in this stream");
            continue;
        }
        // The other candidate, and the cheaper one: `igLine2d` already has two
        // undecoded small fields of its own. `sub_type_word` takes values like
        // 1, 16, 31, 35, 43, 50 -- the same space the line style ids occupy.
        let records = decode_iglines(&data);
        let subtypes: Vec<u32> = records.iter().map(|r| u32::from(r.sub_type_word)).collect();
        let indices: Vec<u32> = records.iter().map(|r| r.index).collect();
        for (label, values) in [
            ("sub_type_word (+12)", &subtypes),
            ("index (+14)", &indices),
        ] {
            let distinct: BTreeSet<u32> = values.iter().copied().collect();
            let defined = values.iter().filter(|v| line_ids.contains(v)).count();
            println!(
                "    igLine2d {label}: {} distinct {:?}; {defined}/{} are a defined line style id",
                distinct.len(),
                distinct.iter().take(10).collect::<Vec<_>>(),
                values.len()
            );
        }

        println!("    tail offsets holding a value the line-style table defines:");
        let mut rows: Vec<(&usize, &Vec<u32>)> = small_by_offset.iter().collect();
        rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        if rows.is_empty() {
            println!("      none -- no tail word matches a defined line style id");
        }
        for (at, values) in rows.iter().take(8) {
            let distinct: BTreeSet<u32> = values.iter().copied().collect();
            println!(
                "      +{:<3} {}/{refs_a_line} record(s); {} distinct: {:?}",
                *at + 18,
                values.len(),
                distinct.len(),
                distinct.iter().take(8).collect::<Vec<_>>()
            );
        }
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
