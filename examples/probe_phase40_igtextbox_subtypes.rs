//! Phase 40 S3-followup — verify the native `igTextBox` grammar against the corpus.
//!
//! `radsrvitem.dll`'s `igTextBox` Load (`sub_56498C00`) and its size helper
//! (`sub_5646D450`) settle the layout the shipped decoder only half-modelled.
//! The record is a 28-byte header, a sub-type-dependent body, then a fixed
//! placement/style tail. The **sub-type discriminator is a `u16` at record
//! byte 24 = payload+18**; the style-tail tag is at payload+20; the body
//! starts at payload+22. Native (payload-relative) layout:
//!
//! * sub-type 1: `[u16 count]` @+22, text @+24 ; body = `2*count + 2`
//! * sub-type 2: `[u32 count|0x10000]` @+22, `[u32 X]` @+26, `[u16 count]` @+30,
//!   text @+32 ; body = `2*count + 10`  (the layout the decoder ships)
//! * sub-type 3: `[u16 A]` @+22, `[u16 B]` @+24, `[u16 count]` @+26, text @+28,
//!   then `A` doubles then `B` doubles ; body = `6 + 2*count + 8*(A+B)`
//!
//! whatever follows the body is the placement/style tail, expected to be the
//! same 36 bytes (3 insertion doubles + a 12-byte style trailer) in every
//! sub-type. This probe walks every `0x004D` record on the chain, applies the
//! native formula, and checks: does the tail come out to 36, and does the text
//! the native count carves read cleanly?
//!
//! ```powershell
//! cargo run --example probe_phase40_igtextbox_subtypes
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// The placement/style tail every sub-type ends with, per the native Load.
const EXPECTED_TAIL: usize = 36;

fn u16_at(payload: &[u8], at: usize) -> Option<u16> {
    payload
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(payload: &[u8], at: usize) -> Option<u32> {
    payload
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn text_at(payload: &[u8], start: usize, count: u16) -> Option<String> {
    let mut units = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        units.push(u16_at(payload, start + i * 2)?);
    }
    Some(String::from_utf16_lossy(&units))
}

fn is_readable(text: &str) -> bool {
    !text.is_empty() && !text.contains('\u{FFFD}') && text.chars().all(|c| !c.is_control())
}

/// The native decode of one `igTextBox` payload: `(sub_type, count, text,
/// body_len, tail_len, dword0_ok)`. `None` when the payload is too short for
/// the fields the sub-type names (a genuinely malformed record).
fn native(payload: &[u8]) -> Option<(u16, u16, String, usize, isize, bool)> {
    let sub_type = u16_at(payload, 18)?;
    let (count, text_start, body, dword0_ok) = match sub_type {
        1 => {
            let count = u16_at(payload, 22)?;
            (count, 24usize, 2 + 2 * count as usize, true)
        }
        2 => {
            let dword0 = u32_at(payload, 22)?;
            let count = u16_at(payload, 30)?;
            let ok = dword0 == (count as u32 | 0x1_0000);
            (count, 32usize, 10 + 2 * count as usize, ok)
        }
        3 => {
            let a = u16_at(payload, 22)?;
            let b = u16_at(payload, 24)?;
            let count = u16_at(payload, 26)?;
            let body = 6 + 2 * count as usize + 8 * (a as usize + b as usize);
            (count, 28usize, body, true)
        }
        _ => return None,
    };
    let text = text_at(payload, text_start, count)?;
    // payload = 22 header bytes + body + tail
    let tail = payload.len() as isize - 22 - body as isize;
    Some((sub_type, count, text, body, tail, dword0_ok))
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name.to_string_lossy().into_owned(), bytes));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn main() {
    // (sub_type, accepted_by_shipped_decoder) -> count
    let mut by_kind: BTreeMap<(u16, bool), usize> = BTreeMap::new();
    // tail length distribution per sub-type
    let mut tails: BTreeMap<(u16, isize), usize> = BTreeMap::new();
    let mut readable: BTreeMap<u16, (usize, usize)> = BTreeMap::new(); // sub_type -> (readable, total)
    let mut dword0_bad = 0usize;
    let mut native_failed = 0usize;
    let mut samples: BTreeMap<u16, Vec<String>> = BTreeMap::new();
    let mut total = 0usize;

    for fixture in FIXTURES {
        for (name, bytes) in sheet_streams(Path::new(fixture)) {
            for at in sheet_record_starts(&bytes) {
                let Some(raw) = bytes.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(b) = bytes.get(at + 2..at + 6) else {
                    continue;
                };
                let btf = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
                let Some(payload) = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf)
                else {
                    continue;
                };
                total += 1;
                let accepted = decode_igtextbox_at(&bytes, at).is_some();

                let Some((sub_type, count, text, _body, tail, dword0_ok)) = native(payload) else {
                    native_failed += 1;
                    continue;
                };
                *by_kind.entry((sub_type, accepted)).or_default() += 1;
                *tails.entry((sub_type, tail)).or_default() += 1;
                let entry = readable.entry(sub_type).or_default();
                entry.1 += 1;
                if is_readable(&text) {
                    entry.0 += 1;
                }
                if !dword0_ok {
                    dword0_bad += 1;
                }
                let bucket = samples.entry(sub_type).or_default();
                if bucket.len() < 4 {
                    bucket.push(format!(
                        "{name} @ 0x{at:06X} btf={btf} count={count} tail={tail} text={text:?}"
                    ));
                }
            }
        }
    }

    println!("=== igTextBox records classified by the native sub-type (payload+18) ===");
    println!("total 0x004D records on the chain: {total}");
    println!("native formula could not fit the payload: {native_failed}");
    println!("sub-type 2 dword0 != count|0x10000: {dword0_bad}\n");

    println!("{:>8} {:>10} {:>10}", "sub_type", "accepted", "refused");
    let mut kinds: Vec<u16> = by_kind.keys().map(|(k, _)| *k).collect();
    kinds.sort_unstable();
    kinds.dedup();
    for k in &kinds {
        let a = by_kind.get(&(*k, true)).copied().unwrap_or(0);
        let r = by_kind.get(&(*k, false)).copied().unwrap_or(0);
        println!("{k:>8} {a:>10} {r:>10}");
    }

    println!("\n=== tail length after the native body (expected {EXPECTED_TAIL}) ===");
    println!("{:>8} {:>6} {:>8}", "sub_type", "tail", "count");
    for ((k, tail), count) in &tails {
        let flag = if *tail == EXPECTED_TAIL as isize {
            ""
        } else {
            "  <- not 36"
        };
        println!("{k:>8} {tail:>6} {count:>8}{flag}");
    }

    println!("\n=== does the native count carve clean text? ===");
    println!("{:>8} {:>10} {:>8}", "sub_type", "readable", "total");
    for (k, (ok, tot)) in &readable {
        println!("{k:>8} {ok:>10} {tot:>8}");
    }

    println!("\n=== samples ===");
    for (k, bucket) in &samples {
        println!("-- sub-type {k} --");
        for s in bucket {
            println!("  {s}");
        }
    }
}
