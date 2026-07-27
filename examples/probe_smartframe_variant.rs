//! Check radsrvitem's igSmartFrame2d classifier against the fixture bytes.
//!
//! ROADMAP-SMARTFRAME-003D keeps `0x003D` at `IdentifiedOnly` and will only
//! take named fields on native-reader or controlled-fixture evidence; the
//! A2-shaped scalars at payload `+76/+84` are explicitly not enough. IDA now
//! supplies the reader half. `radsrvitem.dll sub_564464D0` gates on the record
//! itself:
//!
//! ```c
//! if ( *(WORD *)a2 != 61 ) return E_INVALIDARG;      // 61 == 0x003D
//! v6 = *(DWORD *)(a2 + 32);
//! if ( (*(DWORD *)(a2 + 20) & 0x8000) == 0 )  -> "_Empty SmartFrame2d"      igOLENone
//! else if ( !(v6 & 0x40) && !(v6 & 0x20000) ) -> "_Embedded SmartFrame2d"   igOLEEmbedded
//! else if ( v6 & 0x20000 )                    -> "_Locally Linked ..."      igOLELinked
//! else                                        -> externally linked          igOLELinked
//! ```
//!
//! So the record is an **OLE container frame**, and two bounded flag words
//! decide which of three states it is in. That also explains why the payload
//! carries page-shaped scalars without being a page transform: they are the
//! embedded object's own extent.
//!
//! `a2` is asserted to be the record start because the type word is read at
//! `+0`, which is where the PSM envelope puts it. This probe tests that claim
//! the only way it can be tested from here: read those two words out of the
//! real records and see whether the classification comes out coherent. Random
//! bits would mean the reader is looking at an in-memory object instead.
//!
//! Read-only: no parser, decoder, schema or model change.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_HEADER_LEN: usize = 6;
const TYPE_SMARTFRAME: u16 = 0x003D;

/// Record offsets the native classifier reads, and the bits it tests.
const FLAGS_A_AT: usize = 20;
const FLAGS_B_AT: usize = 32;
const HAS_CONTENT: u32 = 0x8000;
const LINKED: u32 = 0x40;
const LOCALLY_LINKED: u32 = 0x2_0000;

/// Payload offsets the earlier evidence closeout called out, for context.
const EXTENT_W_AT: usize = 76;
const EXTENT_H_AT: usize = 84;
const ASPECT_AT: usize = 148;

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
    let end = header_end.checked_add(btf)?;
    (end <= bytes.len()).then_some(end)
}

/// Greedy + chain-validated top-level walk, as the other sheet probes use.
fn walk_records(bytes: &[u8]) -> Vec<(usize, usize, u16)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN <= bytes.len() {
        if let Some(end) = psm_record_end(bytes, off) {
            if end == bytes.len() || psm_record_end(bytes, end).is_some() {
                let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
                out.push((off, end, type_word & 0x3FFF));
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    bytes
        .get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

/// The classifier, transcribed from `sub_564464D0`.
fn variant(flags_a: u32, flags_b: u32) -> &'static str {
    if flags_a & HAS_CONTENT == 0 {
        "Empty        (igOLENone)"
    } else if flags_b & LINKED == 0 && flags_b & LOCALLY_LINKED == 0 {
        "Embedded     (igOLEEmbedded)"
    } else if flags_b & LOCALLY_LINKED != 0 {
        "LocallyLinked(igOLELinked)"
    } else {
        "Linked       (igOLELinked)"
    }
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

    let mut variants: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0usize;

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

        let name = Path::new(fixture)
            .file_name()
            .map_or(fixture, |n| n.to_str().unwrap_or(fixture));
        println!("\n=== {name} ===");

        for stream_path in sheet_paths {
            let mut bytes = Vec::new();
            cfb.open_stream(&stream_path)?.read_to_end(&mut bytes)?;
            for (off, end, type_code) in walk_records(&bytes) {
                if type_code != TYPE_SMARTFRAME {
                    continue;
                }
                total += 1;
                let record = &bytes[off..end];
                let payload = &record[PSM_HEADER_LEN..];
                let flags_a = u32_at(record, FLAGS_A_AT).unwrap_or(0);
                let flags_b = u32_at(record, FLAGS_B_AT).unwrap_or(0);
                let kind = variant(flags_a, flags_b);
                *variants.entry(kind).or_default() += 1;

                let w = f64_at(payload, EXTENT_W_AT).unwrap_or(f64::NAN);
                let h = f64_at(payload, EXTENT_H_AT).unwrap_or(f64::NAN);
                let aspect = f64_at(payload, ASPECT_AT).unwrap_or(f64::NAN);
                println!(
                    "  {} 0x{off:06X} btf={:<4} rec+20={flags_a:#010x} rec+32={flags_b:#010x}  {kind}",
                    stream_path.to_string_lossy(),
                    record.len() - PSM_HEADER_LEN,
                );
                println!(
                    "      payload +76={w:.6} +84={h:.6} +148={aspect:.6}  w/h={:.4}",
                    w / h
                );
            }
        }
    }

    println!("\n=== classification across {total} records ===");
    for (kind, count) in &variants {
        println!("  {kind:<28} {count}");
    }
    println!(
        "\nA coherent split (few distinct states, each with a plausible payload) supports\n\
         reading the flags off the record; scattered bits would mean the reader sees an\n\
         in-memory object rather than these bytes."
    );
    Ok(())
}
