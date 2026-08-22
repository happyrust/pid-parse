//! Is `JStyleTextChar +34` a text colour, or only ever its sentinel?
//!
//! Every label a `.pid` imports today renders in its layer's default green:
//! `style_link` reads the character height off `0x002C` (`+42`) but nothing
//! reads a colour, and `OpenCADStudio`'s `apply_symbology` deliberately skips
//! the text layer. The format guide lists `+34` as the candidate, at
//! `hypothesis` level: "carries a `-1` sentinel; the shape fits a text
//! colour".
//!
//! That wording hides the question that decides whether any of this is worth
//! doing. A field which is *always* the sentinel has no colour behind it, and
//! wiring it would change nothing on any drawing. So before reading the
//! native serialiser, measure: across every `0x002C` record in the corpus,
//! what does `+34` actually hold?
//!
//! Read as a Win32 `COLORREF` (`0x00BBGGRR`), the same encoding the line and
//! fill colours use, a real colour has a zero high byte. `0xFFFFFFFF` is the
//! sentinel. Anything else is a value worth explaining.
//!
//! ```powershell
//! cargo run --example probe_text_colour_candidate
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

/// PSM type code of `JStyleTextChar`.
const PSM_TEXT_CHAR_STYLE: u16 = 0x002C;
/// Bytes of `StyleCluster` stream header before the first record.
const STREAM_HEADER: usize = 8;
/// PSM record envelope: `u16` type word + `u32` bytes-to-follow.
const ENVELOPE: usize = 6;
/// The candidate offset.
const COLOUR_CANDIDATE: usize = 34;
/// The offset the height is read from, for a per-record cross-check.
const HEIGHT_OFFSET: usize = 42;

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

/// Every `0x002C` payload in a `StyleCluster` stream, by walking the chain.
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

/// How a `COLORREF` reads: `0x00BBGGRR`, high byte zero.
fn as_colorref(value: u32) -> Option<[u8; 3]> {
    if value >> 24 != 0 {
        return None;
    }
    Some([
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
    ])
}

fn describe(value: u32) -> String {
    if value == 0xFFFF_FFFF {
        return "-1 (sentinel)".to_string();
    }
    match as_colorref(value) {
        Some([r, g, b]) => format!("0x{value:08X}  #{r:02X}{g:02X}{b:02X}"),
        None => format!("0x{value:08X}  (high byte set -- not a COLORREF)"),
    }
}

fn main() {
    let mut values: BTreeMap<u32, usize> = BTreeMap::new();
    let mut total = 0usize;
    // A record that resolves a plausible height is one the shipped chain
    // actually uses; a colour that only appears on unused records is worth
    // less than one that rides with real lettering.
    let mut with_usable_height: BTreeMap<u32, usize> = BTreeMap::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        for data in style_streams(path) {
            for payload in text_char_payloads(&data) {
                let Some(value) = u32_at(&payload, COLOUR_CANDIDATE) else {
                    continue;
                };
                total += 1;
                *values.entry(value).or_default() += 1;
                let height_ok = f64_at(&payload, HEIGHT_OFFSET)
                    .is_some_and(|h| h.is_finite() && (0.0005..=0.02).contains(&h));
                if height_ok {
                    *with_usable_height.entry(value).or_default() += 1;
                }
            }
        }
    }

    println!("=== JStyleTextChar +34 across the corpus ===");
    println!("0x002C records read: {total}\n");
    println!("{:>6}  {:>6}  value at +34", "count", "w/ hgt");
    for (value, count) in &values {
        println!(
            "{count:>6}  {:>6}  {}",
            with_usable_height.get(value).copied().unwrap_or(0),
            describe(*value)
        );
    }

    let sentinel = values.get(&0xFFFF_FFFF).copied().unwrap_or(0);
    let colourlike: usize = values
        .iter()
        .filter(|(v, _)| **v != 0xFFFF_FFFF && as_colorref(**v).is_some())
        .map(|(_, c)| *c)
        .sum();

    println!("\n=== verdict ===");
    println!("sentinel (-1): {sentinel}/{total}");
    println!("reads as a COLORREF: {colourlike}/{total}");
    if colourlike == 0 {
        println!(
            "\nNothing but the sentinel. There is no colour behind this field on\n\
             this corpus, so wiring it would change no drawing -- stop here and\n\
             record the negative result."
        );
    } else {
        println!(
            "\n{colourlike} record(s) carry something that reads as a colour. Worth\n\
             taking to the native serialiser for a read-order confirmation."
        );
    }
}
