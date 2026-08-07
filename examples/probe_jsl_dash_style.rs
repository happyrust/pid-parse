//! Read the `0x002F` JSL Simple Dash Type Style records.
//!
//! Unlike the other style families this one is not a fixed-width row, which is
//! why column-wise bucketing never cracked it. `style.dll`'s serializer says
//! why: `JStyleSimpleDashType` writes a **count** and then that many `f64`
//! dash lengths, so every record is as long as its own pattern.
//!
//! The order the native reader writes (version 2, after the base part) is
//! `u32`, then a count, then that many `f64`, then flags. The `.pid` frames
//! the same fields more tightly than the DLL's own stream does -- the count
//! arrives as a `u16` and the record simply ends after the pattern -- so the
//! layout on disk is:
//!
//! ```text
//! +0 .. +47   fixed head (style id at +14, as in the other JSL families)
//! +48   u16   N, the dash segment count
//! +50   f64[N]  the dash pattern
//! ```
//!
//! which makes every record exactly `50 + 8N` bytes. That is the test, and it
//! is a strict one: a wrong reading would leave a remainder on some record.
//! The raw bytes are dumped too, so a failure can be read by eye.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_SIMPLE_DASH_TYPE: u16 = 0x002F;
const PSM_SIMPLE_LINE_STYLE: u16 = 0x002E;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;

/// Offset of the segment count; everything before it is the fixed head.
const COUNT_AT: usize = 48;
/// The style id, at the same offset as in `0x002C` and `0x002E`.
const STYLE_ID_AT: usize = 14;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f64_at(d: &[u8], at: usize) -> Option<f64> {
    let s = d.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

fn payloads_of(data: &[u8], want: u16) -> Vec<Vec<u8>> {
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

/// The dash pattern, if the record's own length agrees with its count.
///
/// The agreement is the whole point: `50 + 8N == len` has to hold exactly, so
/// a misread count or a misplaced field shows up as a rejected record rather
/// than as plausible-looking numbers.
fn dash_pattern(p: &[u8]) -> Option<(u16, Vec<f64>)> {
    let n = u16_at(p, COUNT_AT)?;
    let expect = COUNT_AT + 2 + 8 * n as usize;
    if expect != p.len() {
        return None;
    }
    let values: Vec<f64> = (0..n as usize)
        .filter_map(|i| f64_at(p, COUNT_AT + 2 + 8 * i))
        .collect();
    // A zero-length element is a dot, not a decode failure.
    (values.len() == n as usize && values.iter().all(|v| v.is_finite())).then_some((n, values))
}

/// Which `u32` column of the stream's `0x002E` line styles names a dash type.
///
/// A line style has to say which dash pattern it draws with, and the only
/// vocabulary it can use is the dash style ids living in the same cluster.
/// So the column is the one whose every value is either 0 (solid) or an id
/// that is actually present -- and which is not always 0, or it says nothing.
/// Where in a `0x002E` payload a dash style id actually appears.
///
/// `style.dll` says a `JStyleSimpleLine` holds a dash reference as an **id**
/// that it resolves lazily, so the id has to be written somewhere in the line
/// record. Demanding a column whose every value is a dash id is too strict --
/// a line that draws solid names no dash, and the same column may also carry
/// ids of other style families. So count hits per offset instead and let the
/// histogram show whether one offset stands out.
fn dash_id_sightings(
    lines: &[Vec<u8>],
    dash_ids: &BTreeSet<u32>,
) -> BTreeMap<(&'static str, usize), usize> {
    let mut hits: BTreeMap<(&'static str, usize), usize> = BTreeMap::new();
    for p in lines {
        for at in 0..p.len().saturating_sub(1) {
            if let Some(v) = u16_at(p, at) {
                if v != 0 && dash_ids.contains(&u32::from(v)) {
                    *hits.entry(("u16", at)).or_default() += 1;
                }
            }
            if let Some(v) = u32_at(p, at) {
                if v != 0 && dash_ids.contains(&v) {
                    *hits.entry(("u32", at)).or_default() += 1;
                }
            }
        }
    }
    hits
}

fn hexdump(p: &[u8]) {
    for (row, chunk) in p.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        println!("        +{:<3} {}", row * 16, hex.join(" "));
    }
}

fn probe(path: &Path) -> (usize, usize) {
    let mut fitted = 0usize;
    let Ok(file) = std::fs::File::open(path) else {
        return (0, 0);
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return (0, 0);
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();

    println!("\n=== {} ===", path.display());
    let mut total = 0usize;
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let payloads = payloads_of(&data, PSM_SIMPLE_DASH_TYPE);
        if payloads.is_empty() {
            continue;
        }
        total += payloads.len();
        let sizes: BTreeMap<usize, usize> = payloads.iter().fold(BTreeMap::new(), |mut m, p| {
            *m.entry(p.len()).or_default() += 1;
            m
        });
        println!(
            "\n  -- {name}: {} record(s), sizes {:?} --",
            payloads.len(),
            sizes.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
        );
        for (i, p) in payloads.iter().enumerate() {
            let id = u32_at(p, STYLE_ID_AT);
            match dash_pattern(p) {
                Some((n, values)) => {
                    let mm: Vec<String> = values
                        .iter()
                        .map(|v| format!("{:+.4}", v * 1000.0))
                        .collect();
                    println!(
                        "    [{i}] {} bytes  style id {:<5} N={n}  pattern(mm) [{}]",
                        p.len(),
                        id.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                        mm.join(", ")
                    );
                    fitted += 1;
                }
                None => {
                    println!(
                        "    [{i}] {} bytes  style id {id:?}  DOES NOT FIT 50+8N",
                        p.len()
                    );
                    hexdump(p);
                }
            }
        }

        let dash_ids: BTreeSet<u32> = payloads
            .iter()
            .filter(|p| dash_pattern(p).is_some())
            .filter_map(|p| u32_at(p, STYLE_ID_AT))
            .collect();
        let lines = payloads_of(&data, PSM_SIMPLE_LINE_STYLE);
        if !lines.is_empty() && !dash_ids.is_empty() {
            let hits = dash_id_sightings(&lines, &dash_ids);
            println!(
                "    join hunt: {} 0x002E record(s), dash ids {dash_ids:?}",
                lines.len()
            );
            if hits.is_empty() {
                println!("      no dash id appears anywhere in any line style record");
            }
            let mut ranked: Vec<_> = hits.into_iter().collect();
            ranked.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            for ((kind, at), n) in ranked.into_iter().take(6) {
                println!("      {kind} +{at:<3} seen in {n}/{} records", lines.len());
            }
        }
    }
    (total, fitted)
}

fn main() {
    let mut total = 0usize;
    let mut fitted = 0usize;
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/D06.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            let (t, f) = probe(path);
            total += t;
            fitted += f;
        }
    }
    println!("\n{fitted}/{total} 0x002F record(s) across the corpus fit 50+8N");

    // End to end: what a renderer actually receives once the chain is joined.
    println!("\n=== resolved through style_link ===");
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/D06.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        let Ok(index) = pid_parse::style_link::line_styles_for_file(path) else {
            continue;
        };
        let dashed: Vec<_> = index.values().filter_map(|s| s.dash).collect();
        let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
        for d in &dashed {
            let key: Vec<String> = d.segments_mm().iter().map(|v| format!("{v:+.4}")).collect();
            *shapes.entry(key.join(", ")).or_default() += 1;
        }
        println!(
            "  {fixture}: {}/{} drawn records dashed",
            dashed.len(),
            index.len()
        );
        for (shape, n) in shapes {
            println!("      {n:>4} x [{shape}]");
        }
    }
}
