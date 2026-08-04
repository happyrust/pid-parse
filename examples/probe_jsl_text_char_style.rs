//! Read the `0x002C` JSL Text Character Style records.
//!
//! `2026-08-04-stylecluster-record-chain.md` put text height here: the style
//! cluster is a PSM record chain, and `0x002C` is `style.dll`'s Text Character
//! Style with 21-48 instances per drawing. This reads those records.
//!
//! Two targeted tests beyond the usual column analysis, because a text style
//! has two things worth finding and both have a recognisable shape:
//!
//! * **height** -- ISO 3098 body text is 2.5mm and a P&ID's range is roughly
//!   1..20mm. Decoded sheet coordinates are metres, so a height field is
//!   either an `f64` in `0.001..0.02` or one in `1..20`, depending on which
//!   unit the style table uses. Both are far from the values noise produces.
//! * **the style id** -- phase 35-D found `igTextBox` trailers carry a stable
//!   id (56 Chinese, 64 pipe numbers, 21 general annotation). If a record
//!   holds its own id, joining the two is what unblocks height end to end.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_TEXT_CHAR_STYLE: u16 = 0x002C;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;

/// Millimetre text on a metre-denominated sheet, and the same range expressed
/// in millimetres. A hit in either is worth reporting.
const HEIGHT_METRES: (f64, f64) = (0.001, 0.02);
const HEIGHT_MM: (f64, f64) = (1.0, 20.0);

/// The ids phase 35-D proved stable across drawings.
const PHASE35D_IDS: [u32; 3] = [21, 56, 64];

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

/// Every `0x002C` payload in the stream, by walking the record chain.
fn text_char_payloads(data: &[u8]) -> Vec<Vec<u8>> {
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
        if type_word & 0x3FFF == PSM_TEXT_CHAR_STYLE {
            out.push(data[at + ENVELOPE..end].to_vec());
        }
        at = end;
    }
    out
}

fn in_range(v: f64, (lo, hi): (f64, f64)) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

/// The longest printable UTF-16 run in a payload, which is where the font name
/// sits. Runs of two or three characters are byte noise that happens to decode.
fn longest_utf16_run(payload: &[u8]) -> String {
    let mut best = String::new();
    let mut at = 0;
    while at + 2 <= payload.len() {
        let mut run = String::new();
        let mut k = at;
        while k + 2 <= payload.len() {
            let ch = u16_at(payload, k).unwrap_or(0);
            if ch == 0 || !(0x20..0xFFFD).contains(&ch) {
                break;
            }
            match char::from_u32(u32::from(ch)) {
                Some(c) => run.push(c),
                None => break,
            }
            k += 2;
        }
        if run.chars().count() > best.chars().count() {
            best = run;
        }
        at += 2;
    }
    if best.chars().count() >= 4 {
        best
    } else {
        String::new()
    }
}

fn analyse(payloads: &[Vec<u8>]) {
    let width = payloads[0].len();
    if payloads.iter().any(|p| p.len() != width) {
        println!("      (mixed widths in this bucket, skipped)");
        return;
    }
    let n = payloads.len();

    // Which byte columns are fixed, and which carry data.
    let mut constants = 0usize;
    let mut varying: Vec<usize> = Vec::new();
    for col in 0..width {
        let distinct: BTreeSet<u8> = payloads.iter().map(|p| p[col]).collect();
        if distinct.len() == 1 {
            constants += 1;
        } else {
            varying.push(col);
        }
    }
    println!("      {constants}/{width} byte columns constant; varying at {varying:?}");

    // f64 candidates for the height. Scanned byte by byte rather than on an
    // alignment: the payload length varies with an embedded name, so a field
    // sitting after that name need not stay 2-byte aligned between variants.
    println!("      f64 windows in a text-height range:");
    let mut found_height = false;
    for at in 0..width.saturating_sub(7) {
        let values: Vec<f64> = payloads.iter().filter_map(|p| f64_at(p, at)).collect();
        if values.len() != n {
            continue;
        }
        let metres = values
            .iter()
            .filter(|v| in_range(**v, HEIGHT_METRES))
            .count();
        let mm = values.iter().filter(|v| in_range(**v, HEIGHT_MM)).count();
        let (hits, in_metres) = if metres >= mm {
            (metres, true)
        } else {
            (mm, false)
        };
        // A clean sweep, or a near miss -- a height field can hold a sentinel
        // in one record out of a dozen.
        if hits * 4 < n * 3 {
            continue;
        }
        found_height = true;
        let distinct: BTreeSet<String> = values
            .iter()
            .filter(|v| in_range(**v, HEIGHT_METRES) || in_range(**v, HEIGHT_MM))
            .map(|v| format!("{:.3}", if in_metres { v * 1000.0 } else { *v }))
            .collect();
        let shown: Vec<&str> = distinct.iter().take(9).map(String::as_str).collect();
        println!(
            "        +{at:<3} {hits}/{n} in {} range; {} distinct (mm): {}{}",
            if in_metres { "metres" } else { "mm" },
            distinct.len(),
            shown.join(", "),
            if distinct.len() > 9 { " ..." } else { "" }
        );
    }
    if !found_height {
        println!("        none");
    }

    // Does a record carry an id phase 35-D would recognise?
    println!("      words matching phase 35-D ids (21/56/64):");
    let mut found_id = false;
    for at in 0..width.saturating_sub(3) {
        let values: Vec<u32> = payloads.iter().filter_map(|p| u32_at(p, at)).collect();
        if values.len() != n {
            continue;
        }
        let hits = values.iter().filter(|v| PHASE35D_IDS.contains(v)).count();
        if hits == 0 {
            continue;
        }
        found_id = true;
        let distinct: BTreeSet<u32> = values.iter().copied().collect();
        println!(
            "        +{at:<3} {hits}/{n} record(s) match; {} distinct value(s){}",
            distinct.len(),
            if distinct.len() == n {
                " (unique per record -- looks like an id)"
            } else {
                ""
            }
        );
    }
    if !found_id {
        println!("        none");
    }

    // A style normally has a name; a UTF-16 run would confirm which record is which.
    // The two fields the column analysis singles out, per record.
    println!("      per-record: id@+14, height@+42, font name");
    for payload in payloads.iter().take(10) {
        let id = u32_at(payload, 14).unwrap_or(0);
        let height_m = f64_at(payload, 42).unwrap_or(f64::NAN);
        let font = longest_utf16_run(payload);
        println!(
            "        id={id:<12} height={:.3}mm  font={font:?}",
            height_m * 1000.0
        );
    }

    let mut names = Vec::new();
    for payload in payloads.iter().take(6) {
        let mut best = String::new();
        let mut at = 0;
        while at + 2 <= payload.len() {
            let mut run = String::new();
            let mut k = at;
            while k + 2 <= payload.len() {
                let ch = u16_at(payload, k).unwrap_or(0);
                if ch == 0 || !(0x20..0xFFFD).contains(&ch) {
                    break;
                }
                if let Some(c) = char::from_u32(u32::from(ch)) {
                    run.push(c);
                }
                k += 2;
            }
            if run.chars().count() > best.chars().count() {
                best = run;
            }
            at += 2;
        }
        if best.chars().count() >= 3 {
            names.push(best);
        }
    }
    if !names.is_empty() {
        println!("      longest UTF-16 run per record (first few): {names:?}");
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
        let payloads = text_char_payloads(&data);
        if payloads.is_empty() {
            continue;
        }
        let mut by_len: BTreeMap<usize, Vec<Vec<u8>>> = BTreeMap::new();
        for p in payloads {
            by_len.entry(p.len()).or_default().push(p);
        }
        println!(
            "\n  -- {name}: {} 0x002C record(s), payload sizes {:?} --",
            by_len.values().map(Vec::len).sum::<usize>(),
            by_len
                .iter()
                .map(|(k, v)| (*k, v.len()))
                .collect::<Vec<_>>()
        );
        // The join check: phase 35-D's ids have to exist in this table for
        // `igTextBox` to be pointing at it.
        let ids: BTreeSet<u32> = by_len
            .values()
            .flatten()
            .filter_map(|p| u32_at(p, 14))
            .collect();
        let present: Vec<u32> = PHASE35D_IDS
            .iter()
            .copied()
            .filter(|i| ids.contains(i))
            .collect();
        println!(
            "    id@+14 across all records: {} distinct, range {:?}..{:?}; phase 35-D ids present: {present:?} of {:?}",
            ids.len(),
            ids.iter().next(),
            ids.iter().next_back(),
            PHASE35D_IDS
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
