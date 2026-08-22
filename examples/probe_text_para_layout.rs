//! Does `JStyleTextPara` state a justification, and does any drawing use it?
//!
//! The paragraph style is the one record a text label reaches that we barely
//! read: `style_link` takes its `+38` character-style pointer and nothing
//! else. Alignment is a paragraph property, so if a `.pid` states one it
//! should be here -- and `OpenCADStudio` currently places every label
//! left-aligned at its insertion point, which would put centred labels half a
//! word off.
//!
//! `style.dll`'s serialiser (`sub_100337A0`) gives the layout outright. The
//! read order, anchored by the `+38` pointer we already rely on:
//!
//! ```text
//!   +26  u32   (only the high word is kept)
//!   +30  u32
//!   +34  u8
//!   +35  u8    <- has a get/put pair on IJStyleTextParaImp
//!   +36  u8    <- has a get/put pair on IJStyleTextParaImp
//!   +37  u8    (read and discarded)
//!   +38  u32   character style reference        <- known anchor
//!   +42  f64  } six values the interface refuses to set negative:
//!   +50  f64  } indents and spacing, in the paragraph's own units
//!   +58  f64  }
//!   +66  f64  }
//!   +74  f64  }
//!   +82  f64  }
//!   = 90 bytes
//! ```
//!
//! The two bytes at `+35` / `+36` are the alignment candidates: small enums
//! with both a getter and a setter on the interface. This probe measures
//! whether they vary. **A field that never varies cannot be the reason a
//! label looks misplaced**, and settles whether wiring alignment is worth
//! anything before any of it is written.
//!
//! ```powershell
//! cargo run --example probe_text_para_layout
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_TEXT_PARA_STYLE: u16 = 0x002D;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;
/// Payload length the native read order accounts for, byte for byte.
const NATIVE_PAYLOAD_LEN: usize = 90;
/// The two bytes the interface exposes with a get/put pair.
const ENUM_A: usize = 35;
const ENUM_B: usize = 36;
/// The six non-negative doubles.
const METRICS: [usize; 6] = [42, 50, 58, 66, 74, 82];

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

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

fn para_payloads(data: &[u8]) -> Vec<Vec<u8>> {
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
        if type_word & 0x3FFF == PSM_TEXT_PARA_STYLE {
            out.push(data[at + ENVELOPE..end].to_vec());
        }
        at = end;
    }
    out
}

fn style_streams(path: &Path) -> Vec<Vec<u8>> {
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
        .filter(|p| p.to_lowercase().contains("style"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push(data);
        }
    }
    out
}

fn main() {
    let mut total = 0usize;
    let mut right_length = 0usize;
    let mut enum_a: BTreeMap<u8, usize> = BTreeMap::new();
    let mut enum_b: BTreeMap<u8, usize> = BTreeMap::new();
    let mut metrics: Vec<BTreeMap<String, usize>> = vec![BTreeMap::new(); METRICS.len()];

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        for data in style_streams(path) {
            for payload in para_payloads(&data) {
                total += 1;
                if payload.len() == NATIVE_PAYLOAD_LEN {
                    right_length += 1;
                }
                if let Some(v) = payload.get(ENUM_A) {
                    *enum_a.entry(*v).or_default() += 1;
                }
                if let Some(v) = payload.get(ENUM_B) {
                    *enum_b.entry(*v).or_default() += 1;
                }
                for (slot, at) in METRICS.iter().enumerate() {
                    if let Some(v) = f64_at(&payload, *at) {
                        *metrics[slot]
                            .entry(format!("{v:.6}"))
                            .or_default() += 1;
                    }
                }
            }
        }
    }

    println!("=== JStyleTextPara across the corpus ===");
    println!("0x002D records: {total}");
    println!("payload == {NATIVE_PAYLOAD_LEN} bytes: {right_length}/{total}\n");

    println!("=== the two alignment candidates ===");
    println!("+{ENUM_A}: {enum_a:?}");
    println!("+{ENUM_B}: {enum_b:?}");
    let a_varies = enum_a.len() > 1;
    let b_varies = enum_b.len() > 1;

    println!("\n=== the six non-negative metrics ===");
    for (slot, at) in METRICS.iter().enumerate() {
        let distinct = metrics[slot].len();
        let sample: Vec<String> = metrics[slot]
            .iter()
            .take(6)
            .map(|(v, n)| format!("{v}x{n}"))
            .collect();
        println!("+{at:<3} {distinct:>3} distinct  {}", sample.join("  "));
    }

    println!("\n=== verdict ===");
    if a_varies || b_varies {
        println!(
            "At least one of the two enums varies, so the drawings do state\n\
             something per-paragraph. Worth identifying which is horizontal\n\
             alignment before wiring anything."
        );
    } else {
        println!(
            "Neither enum varies across the corpus. Whatever they mean, every\n\
             paragraph in every drawing shares the value -- so alignment is not\n\
             why any label looks misplaced, and wiring it would change nothing."
        );
    }
}
