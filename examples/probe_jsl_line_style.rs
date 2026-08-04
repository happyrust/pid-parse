//! Read the `0x002E` JSL Simple Line Style records.
//!
//! `2026-08-04-stylecluster-record-chain.md` put line weight and colour here.
//! The approach that found text height applies: bucket by payload size, read
//! column-wise, then test the candidates against what the values would have to
//! look like if the guess were right.
//!
//! Line work has two recognisable signatures:
//!
//! * **weight** -- ISO 128 standardises 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0,
//!   1.4 and 2.0 mm. A field carrying a subset of those is a line weight and
//!   almost nothing else.
//! * **colour** -- either a small palette index, or a packed `u32` whose top
//!   byte is zero and whose three low bytes are RGB.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_SIMPLE_LINE_STYLE: u16 = 0x002E;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;

/// ISO 128 line widths in millimetres.
const ISO_WIDTHS: [f64; 9] = [0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];

/// Line weight expressed on a metre-denominated sheet, and in millimetres.
const WEIGHT_METRES: (f64, f64) = (0.00005, 0.005);
const WEIGHT_MM: (f64, f64) = (0.05, 5.0);

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

fn in_range(v: f64, (lo, hi): (f64, f64)) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

/// How many of these millimetre values sit on an ISO 128 step.
fn iso_matches(values: &BTreeSet<String>) -> usize {
    values
        .iter()
        .filter_map(|s| s.parse::<f64>().ok())
        .filter(|v| ISO_WIDTHS.iter().any(|w| (w - v).abs() < 0.005))
        .count()
}

fn analyse(payloads: &[Vec<u8>]) {
    let width = payloads[0].len();
    if payloads.iter().any(|p| p.len() != width) {
        return;
    }
    let n = payloads.len();

    let mut constants = 0usize;
    let mut varying: Vec<usize> = Vec::new();
    for col in 0..width {
        if payloads
            .iter()
            .map(|p| p[col])
            .collect::<BTreeSet<u8>>()
            .len()
            == 1
        {
            constants += 1;
        } else {
            varying.push(col);
        }
    }
    println!("      {constants}/{width} byte columns constant; varying at {varying:?}");

    println!("      f64 windows in a line-weight range:");
    let mut any_weight = false;
    for at in 0..width.saturating_sub(7) {
        let values: Vec<f64> = payloads.iter().filter_map(|p| f64_at(p, at)).collect();
        if values.len() != n {
            continue;
        }
        let metres = values
            .iter()
            .filter(|v| in_range(**v, WEIGHT_METRES))
            .count();
        let mm = values.iter().filter(|v| in_range(**v, WEIGHT_MM)).count();
        let (hits, in_metres) = if metres >= mm {
            (metres, true)
        } else {
            (mm, false)
        };
        if hits * 4 < n * 3 {
            continue;
        }
        let distinct: BTreeSet<String> = values
            .iter()
            .filter(|v| in_range(**v, WEIGHT_METRES) || in_range(**v, WEIGHT_MM))
            .map(|v| format!("{:.3}", if in_metres { v * 1000.0 } else { *v }))
            .collect();
        let iso = iso_matches(&distinct);
        any_weight = true;
        println!(
            "        +{at:<3} {hits}/{n} in {} range; {} distinct (mm): {}   [{iso}/{} on an ISO 128 step]",
            if in_metres { "metres" } else { "mm" },
            distinct.len(),
            distinct.iter().take(10).cloned().collect::<Vec<_>>().join(", "),
            distinct.len(),
        );
    }
    if !any_weight {
        println!("        none");
    }

    println!("      u32 windows that look like a packed colour or a small index:");
    let mut any_colour = false;
    for at in 0..width.saturating_sub(3) {
        let values: Vec<u32> = payloads.iter().filter_map(|p| u32_at(p, at)).collect();
        if values.len() != n {
            continue;
        }
        let distinct: BTreeSet<u32> = values.iter().copied().collect();
        if distinct.len() < 2 {
            continue;
        }
        // Packed 0x00BBGGRR: top byte clear, and not all values tiny.
        let rgb_shaped = values.iter().all(|v| v >> 24 == 0);
        let has_wide = distinct.iter().any(|v| *v > 0xFF);
        let index_shaped = values.iter().all(|v| *v <= 255);
        if !(rgb_shaped && has_wide) && !index_shaped {
            continue;
        }
        any_colour = true;
        let kind = if index_shaped {
            "index-like"
        } else {
            "colour-like"
        };
        let shown: Vec<String> = distinct
            .iter()
            .take(8)
            .map(|v| {
                if index_shaped {
                    v.to_string()
                } else {
                    format!(
                        "#{:02X}{:02X}{:02X}",
                        v & 0xFF,
                        (v >> 8) & 0xFF,
                        (v >> 16) & 0xFF
                    )
                }
            })
            .collect();
        println!(
            "        +{at:<3} {kind:<12} {} distinct: {}{}",
            distinct.len(),
            shown.join(", "),
            if distinct.len() > 8 { " ..." } else { "" }
        );
    }
    if !any_colour {
        println!("        none");
    }
}

fn probe(path: &Path) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return;
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.to_lowercase().contains("style"))
        .collect();

    println!("\n=== {} ===", path.display());
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let payloads = payloads_of(&data, PSM_SIMPLE_LINE_STYLE);
        if payloads.len() < 3 {
            continue;
        }
        let mut by_len: BTreeMap<usize, Vec<Vec<u8>>> = BTreeMap::new();
        for p in payloads {
            by_len.entry(p.len()).or_default().push(p);
        }
        println!(
            "\n  -- {name}: {} 0x002E record(s), sizes {:?} --",
            by_len.values().map(Vec::len).sum::<usize>(),
            by_len
                .iter()
                .map(|(k, v)| (*k, v.len()))
                .collect::<Vec<_>>()
        );
        for (len, group) in &by_len {
            if group.len() < 3 {
                continue;
            }
            println!("    payload {len} bytes, n={}", group.len());
            analyse(group);
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
