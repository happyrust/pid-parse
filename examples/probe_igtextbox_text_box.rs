//! What a kind-2 `igTextBox` tail carries past the placement block: the text
//! box its string letters inside.
//!
//! The family's oldest open question was the tail: `radsrvitem.dll`'s
//! `IGDSFactoryTextPointRectShape::GetSize` states a kind-2 tail as 68 bytes
//! plus one 8-byte field per optional bit, so the "32 or 40 extra bytes" the
//! 2026-08-13 byte account could not name were three quarters of the tail
//! itself. Two of those fields turn out to be the box's width (tail `+40`)
//! and height (tail `+48`): every title-block field of the five drawing
//! fixtures carries `45.307 x 3.689` mm there, and the reference plots start
//! each of those strings exactly half a box width left of the insertion point
//! the record states, which is where the virgin `.pid` puts a centred
//! `igTextBox`. See
//! `docs/analysis/2026-10-19-igtextbox-box-is-where-the-text-starts.md`.
//!
//! Classes over the corpus: shape, tail kind, and the box each kind-2 record
//! states, with the per-drawing count so a record that stops decoding is
//! visible rather than silently missing.
//!
//! ```powershell
//! cargo run --example probe_igtextbox_text_box -- "<drawing name>"
//! ```
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGTEXTBOX,
};

/// Payload offset of the shape discriminator (record byte 24).
const SHAPE_AT: usize = 18;
/// Payload offset of the tail-kind selector (record byte 26).
const TAIL_KIND_AT: usize = 20;
/// Bytes a sub-type 2 record's body occupies, before the tail.
const BODY_PREFIX_AT: usize = 22;
/// Bytes the 22-byte header and a sub-type 2 body prefix occupy.
const SUBTYPE2_BODY: usize = 10;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

/// The `Sheet*` streams of one drawing.
fn sheet_streams(path: &Path) -> Vec<Vec<u8>> {
    let Ok(file) = std::fs::File::open(path) else {
        eprintln!("{}: not found", path.display());
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push(bytes);
        }
    }
    out
}

fn main() {
    let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    );
    for name in std::env::args().skip(1) {
        let mut texts = 0usize;
        let mut kinds: BTreeMap<u16, usize> = BTreeMap::new();
        let mut boxes: BTreeMap<(u64, u64), usize> = BTreeMap::new();
        for data in sheet_streams(&corpus.join(format!("{name}.pid"))) {
            for at in sheet_record_starts(&data) {
                let Some(raw) = data.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(record) = decode_igtextbox_at(&data, at) else {
                    continue;
                };
                texts += 1;
                let payload = &data[at + PSM_ENVELOPE_LEN..record.byte_range.end];
                let kind = u16_at(payload, TAIL_KIND_AT).unwrap_or(0);
                *kinds.entry(kind).or_default() += 1;
                if kind != 2 {
                    continue;
                }
                let shape = u16_at(payload, SHAPE_AT).unwrap_or(0);
                let tail = BODY_PREFIX_AT + SUBTYPE2_BODY + 2 * record.text_length as usize;
                let field = |offset: usize| -> f64 {
                    let at = tail + offset;
                    let Some(bytes) = payload.get(at..at + 8) else {
                        return f64::NAN;
                    };
                    f64::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])
                };
                let (width, height) = (field(40), field(48));
                *boxes
                    .entry(((width * 1e6).round() as u64, (height * 1e6).round() as u64))
                    .or_default() += 1;
                println!(
                    "  {:?} shape={shape} insert=({:.4},{:.4}) anchor={:#04X} box={:.4}x{:.4} m",
                    record.text,
                    record.trailing_double_1,
                    record.trailing_double_2,
                    record.anchor_code,
                    width,
                    height,
                );
            }
        }
        println!("{name}: {texts} igTextBox records, tail kinds {kinds:?}");
        for ((width, height), count) in boxes {
            println!(
                "  box {:.6} x {:.6} m on {count} record(s)",
                width as f64 / 1e6,
                height as f64 / 1e6
            );
        }
    }
}
