//! Verify the native `JStyleTextChar` read order against the corpus.
//!
//! `style.dll`'s serialiser (`sub_10030A20`) reads the record field by field,
//! and the order gives the payload layout outright. Two offsets in it are
//! already documented at native-reader level -- the `+30` keyboard layout
//! (the read falls back to `GetKeyboardLayout(0)` when it comes back zero)
//! and the `+42` character height -- and the reads between them account for
//! every byte:
//!
//! ```text
//!   +26  u32
//!   +30  u16   language / keyboard layout   <-- known anchor
//!   +32  u16
//!   +34  u32   character colour (COLORREF)
//!   +38  u8 x4
//!   +42  f64   character height, metres     <-- known anchor
//!   +50  u16
//!   +52  u16
//!   +54  u32
//!   +58  u8 x6
//!   +64  u16
//!   +66  u8 x2
//!   +68  u16   font name length, UTF-16 code units
//!   +70  ...   font name
//! ```
//!
//! `30 + 2 + 2 + 4 + 4 = 42` closes with no slack, which is what pins `+34`.
//!
//! This probe checks the two consequences a corpus can see: every payload is
//! exactly `70 + 2 * count` bytes long, and the name carved at `+70` reads as
//! a font name rather than as the byte noise the old "longest UTF-16 run"
//! heuristic returned.
//!
//! ```powershell
//! cargo run --example probe_text_char_layout
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_TEXT_CHAR_STYLE: u16 = 0x002C;
const STREAM_HEADER: usize = 8;
const ENVELOPE: usize = 6;
/// Bytes before the font name: everything the serialiser reads ahead of it.
const FONT_NAME_START: usize = 70;
/// Where the name's length in UTF-16 code units sits.
const FONT_NAME_COUNT_AT: usize = 68;

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

/// The font name the native layout carves, if the payload holds it.
fn font_name(payload: &[u8]) -> Option<(u16, String)> {
    let count = u16_at(payload, FONT_NAME_COUNT_AT)?;
    let mut units = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        units.push(u16_at(payload, FONT_NAME_START + i * 2)?);
    }
    Some((count, String::from_utf16_lossy(&units)))
}

/// Does this read as a font name rather than as bytes that happen to decode?
fn reads_as_name(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('\u{FFFD}')
        && text.chars().all(|c| !c.is_control())
        && text.chars().any(char::is_alphanumeric)
}

fn main() {
    let mut total = 0usize;
    let mut size_formula_holds = 0usize;
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut unreadable = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        for data in style_streams(path) {
            for payload in text_char_payloads(&data) {
                total += 1;
                let Some((count, name)) = font_name(&payload) else {
                    unreadable += 1;
                    continue;
                };
                let expected_len = FONT_NAME_START + 2 * count as usize;
                if expected_len == payload.len() {
                    size_formula_holds += 1;
                } else if mismatches.len() < 5 {
                    mismatches.push(format!(
                        "payload {} bytes, count {count} implies {expected_len}",
                        payload.len()
                    ));
                }
                if reads_as_name(&name) {
                    *names.entry(name).or_default() += 1;
                } else {
                    unreadable += 1;
                }
            }
        }
    }

    println!("=== native JStyleTextChar layout, checked against the corpus ===");
    println!("0x002C records: {total}\n");
    println!(
        "payload length == 70 + 2*count : {size_formula_holds}/{total}"
    );
    for m in &mismatches {
        println!("  mismatch: {m}");
    }
    println!("\nfont names carved at +70 ({} distinct):", names.len());
    for (name, count) in &names {
        println!("  {count:>4} x {name:?}");
    }
    println!("\nrecords whose name does not read: {unreadable}");
    println!(
        "\nBoth checks passing means the read order is the layout: the size\n\
         formula leaves no unexplained bytes, and the name lands where the\n\
         serialiser says it does rather than where a scan guessed."
    );
}
