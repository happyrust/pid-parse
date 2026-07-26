//! Locate the real placement fields inside a PSM `igSymbol2d` record.
//!
//! `sheet_records.rs` reads the transform at payload offset 40 and the
//! insertion at 72, and every fixture answers with (0, 0) and denormal
//! noise, so those offsets are wrong. Rather than guess again, slide a
//! window over the whole payload and count, per offset, how many records
//! read as a plausible sheet coordinate there.
//!
//! Two encodings are tested. Sheet geometry elsewhere in the file is f64
//! in 0..1, but the `Inferred` coordinate hints `geometry.rs` emits are
//! raw i32 near +/-917504 -- which is 0.875 * 2^20 -- so a 20-bit
//! fixed-point integer is the other candidate.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const TYPE_IGSYMBOL2D: u16 = 0x00CE;
const PSM_HEADER_LEN: usize = 6;

/// A sheet coordinate lives in 0..1 with the page edge just past 1.0.
fn on_sheet(v: f64) -> bool {
    v.is_finite() && v > 1e-6 && v < 1.2
}

struct Record {
    offset: usize,
    payload: Vec<u8>,
}

fn collect(bytes: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();
    if bytes.len() < PSM_HEADER_LEN + 16 {
        return out;
    }
    let mut off = 0usize;
    while off + PSM_HEADER_LEN + 16 <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        if type_word & 0x3FFF != TYPE_IGSYMBOL2D {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]) as usize;
        if !(113..=200).contains(&btf) || off + PSM_HEADER_LEN + btf > bytes.len() {
            off += 1;
            continue;
        }
        let start = off + PSM_HEADER_LEN;
        out.push(Record {
            offset: off,
            payload: bytes[start..start + btf].to_vec(),
        });
        off = start + btf;
    }
    out
}

fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    let c = p.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
    ]))
}

fn read_i32(p: &[u8], at: usize) -> Option<i32> {
    let c = p.get(at..at + 4)?;
    Some(i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

fn probe(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut bytes = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut bytes)?;
    let records = collect(&bytes);
    println!("\n=== {} ===", path.display());
    println!("  igSymbol2d records: {}", records.len());
    if records.is_empty() {
        return Ok(());
    }
    let sizes: BTreeMap<usize, usize> =
        records
            .iter()
            .fold(BTreeMap::new(), |mut m: BTreeMap<usize, usize>, r| {
                *m.entry(r.payload.len()).or_insert(0) += 1;
                m
            });
    println!("  payload sizes: {sizes:?}");

    let shortest = records.iter().map(|r| r.payload.len()).min().unwrap();

    // How many records read as an on-sheet coordinate at each offset?
    let mut f64_hits: BTreeMap<usize, usize> = BTreeMap::new();
    let mut fx20_hits: BTreeMap<usize, usize> = BTreeMap::new();
    let mut fx16_hits: BTreeMap<usize, usize> = BTreeMap::new();
    for r in &records {
        for at in 0..shortest.saturating_sub(8) {
            if read_f64(&r.payload, at).is_some_and(on_sheet) {
                *f64_hits.entry(at).or_insert(0) += 1;
            }
        }
        for at in 0..shortest.saturating_sub(4) {
            if let Some(raw) = read_i32(&r.payload, at) {
                if on_sheet(f64::from(raw) / 1_048_576.0) {
                    *fx20_hits.entry(at).or_insert(0) += 1;
                }
                if on_sheet(f64::from(raw) / 65_536.0) {
                    *fx16_hits.entry(at).or_insert(0) += 1;
                }
            }
        }
    }

    let n = records.len();
    for (label, hits) in [
        ("f64", &f64_hits),
        ("i32/2^20", &fx20_hits),
        ("i32/2^16", &fx16_hits),
    ] {
        let full: Vec<usize> = hits
            .iter()
            .filter(|(_, c)| **c == n)
            .map(|(o, _)| *o)
            .collect();
        println!("  [{label}] offsets on-sheet in ALL {n} records: {full:?}");
    }

    // Print the raw values at every offset that held for every record, so a
    // pair 8 (or 4) bytes apart can be read off as (x, y).
    println!("  --- values at unanimous f64 offsets ---");
    for (at, _) in f64_hits.iter().filter(|(_, c)| **c == n) {
        let vals: Vec<String> = records
            .iter()
            .map(|r| format!("{:.5}", read_f64(&r.payload, *at).unwrap()))
            .collect();
        println!("    +{at:3}: {}", vals.join("  "));
    }
    println!("  --- values at unanimous i32/2^20 offsets ---");
    for (at, _) in fx20_hits.iter().filter(|(_, c)| **c == n) {
        let vals: Vec<String> = records
            .iter()
            .map(|r| {
                format!(
                    "{:.5}",
                    f64::from(read_i32(&r.payload, *at).unwrap()) / 1_048_576.0
                )
            })
            .collect();
        println!("    +{at:3}: {}", vals.join("  "));
    }

    // First record verbatim, so the field structure can be eyeballed.
    let first = &records[0];
    println!(
        "  --- record[0] @ 0x{:06x}, payload {} bytes ---",
        first.offset,
        first.payload.len()
    );
    for start in (0..first.payload.len()).step_by(16) {
        let end = (start + 16).min(first.payload.len());
        let hex: Vec<String> = first.payload[start..end]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        println!("    +{:03}: {}", start, hex.join(" "));
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path)?;
        }
    }
    Ok(())
}
