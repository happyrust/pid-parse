//! What are A01's 18 refused `igTextBox` records, and is any of it lettering?
//!
//! `/JSite204/Sheet6` in `A01.pid` carries the corpus's largest single block of
//! refused text: 18 records, unmoved by retiring the fixed-68 overhead
//! (`docs/analysis/2026-08-12-igtextbox-overhead-is-a-floor-not-a-constant.md`).
//! That stream is established page content — 80 of its `igLine2d` records were
//! this drawing's border and they draw today — so text refused there is text
//! missing from the sheet, and worth attributing.
//!
//! The question this answers is the one that decides whether a fix is worth
//! writing: **is there readable lettering behind these refusals?** A record
//! whose `+30` is garbage and whose `+32` does not read as text has nothing to
//! put on the page even if the decoder accepted it; a record with a real label
//! in it is content the drawing is missing.
//!
//! ```powershell
//! cargo run --example probe_a01_refused_text
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, IGTEXTBOX_PAYLOAD_OVERHEAD, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURE: &str = "test-file/export-test/publish-data/A01/A01.pid";
/// Payload offset where the text begins.
const TEXT_START: usize = 32;

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

/// Read `count` UTF-16LE code units at `at`, if they are all inside `payload`.
fn utf16_at(payload: &[u8], at: usize, count: usize) -> Option<String> {
    let mut units = Vec::with_capacity(count);
    for i in 0..count {
        let pos = at + i * 2;
        let b = payload.get(pos..pos + 2)?;
        units.push(u16::from_le_bytes([b[0], b[1]]));
    }
    Some(String::from_utf16_lossy(&units))
}

/// Does this read as a label rather than as bytes that happen to decode?
fn reads_as_text(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('\u{FFFD}')
        && text.chars().all(|c| !c.is_control())
        && text.chars().any(char::is_alphanumeric)
}

fn main() {
    let path = Path::new(FIXTURE);
    if !path.exists() {
        println!("fixture {FIXTURE} not found");
        return;
    }

    let mut by_reason: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut readable = 0usize;
    let mut refused_total = 0usize;

    for (stream, bytes) in sheet_streams(path) {
        let mut printed_header = false;
        for at in sheet_record_starts(&bytes) {
            let Some(raw) = bytes.get(at..at + 2) else {
                continue;
            };
            if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                continue;
            }
            if decode_igtextbox_at(&bytes, at).is_some() {
                continue;
            }
            refused_total += 1;

            let Some(b) = bytes.get(at + 2..at + 6) else {
                continue;
            };
            let btf = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
            let payload = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf);

            // Attribute against the shipped rules, in their order.
            let (reason, inline) = if btf < IGTEXTBOX_PAYLOAD_OVERHEAD {
                ("btf below the 68-byte floor", None)
            } else {
                match payload.and_then(|p| {
                    p.get(30..32)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                }) {
                    None => ("payload truncated", None),
                    Some(inline) if inline > 1024 => ("stated length above the 1024 cap", Some(inline)),
                    Some(inline) => {
                        let text_end = TEXT_START + 2 * inline as usize;
                        let len = payload.map_or(0, <[u8]>::len);
                        if text_end + 36 > len {
                            ("no room for the stated text plus the tail", Some(inline))
                        } else {
                            ("a trailing double is non-finite or out of domain", Some(inline))
                        }
                    }
                }
            };
            *by_reason.entry(reason).or_default() += 1;

            // Whatever it states, is there a label sitting at +32?
            let sample = payload
                .and_then(|p| utf16_at(p, TEXT_START, inline.unwrap_or(0).min(40) as usize));
            let is_readable = sample.as_deref().is_some_and(reads_as_text);
            if is_readable {
                readable += 1;
            }

            if !printed_header {
                println!("\n=== {stream} ===");
                printed_header = true;
            }
            println!(
                "  @0x{at:06X} btf={btf:<6} +30={:<7} {reason}{}",
                inline.map_or_else(|| "-".to_string(), |v| v.to_string()),
                if is_readable {
                    format!("  text={:?}", sample.unwrap_or_default())
                } else {
                    String::new()
                }
            );
        }
    }

    println!("\n=== summary ===");
    println!("refused igTextBox records: {refused_total}");
    for (reason, count) in &by_reason {
        println!("  {count:>3}  {reason}");
    }
    println!(
        "\nrecords with readable lettering at +32: {readable}/{refused_total}\n\
         Zero here means accepting them would put nothing on the page, and the\n\
         refusals are a reporting matter rather than missing drawing content."
    );
}
