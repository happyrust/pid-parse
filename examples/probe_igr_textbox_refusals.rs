//! Which rule refuses the linked border template's `igTextBox` records?
//!
//! `CPE-SQ-A1.igr` is the OLE-linked title-block template the drawing's
//! `igSmartFrame2d` names. It decodes its frame drafting today, but all 34 of
//! its `0x004D` records are refused -- and a title block without its static
//! lettering ("TITLE", "DWG NO", the company name) is not the title block.
//!
//! Every rule below mirrors the shipped `IgTextBoxDecoder` in order, so the
//! first to fire is the reason the decoder said no.
//!
//! ```powershell
//! cargo run --example probe_igr_textbox_refusals
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igboundary_at, decode_igtextbox_at, parse_live_psm_header, sheet_record_starts,
    PSM_TYPE_CODE_IGBOUNDARY2D, PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURES: &[&str] = &[
    "test-file/Ref/Template Files/CPE-SQ-A1.igr",
    "test-file/Ref/Template Files/CPE-SQ-A0.igr",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

// Private in the crate; mirrored here so the attribution matches the shipped
// decoder rule for rule.
const IGTEXTBOX_BODY_START: usize = 22;
const IGTEXTBOX_TAIL_LEN: usize = 36;
const IGTEXTBOX_MIN_PAYLOAD_LEN: usize = IGTEXTBOX_BODY_START + 2 + IGTEXTBOX_TAIL_LEN;
const IGTEXTBOX_MAX_TEXT_LENGTH: u16 = 1024;
const DOMAIN_LIMIT: f64 = 1e9;
const DIRECTION_TOLERANCE: f64 = 1e-6;

fn read_u16(payload: &[u8], at: usize) -> Option<u16> {
    payload
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(payload: &[u8], at: usize) -> Option<u32> {
    payload
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_f64(payload: &[u8], at: usize) -> Option<f64> {
    payload
        .get(at..at + 8)
        .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
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

/// `igtextbox_body_shape`, mirrored. Returns `(count, text_start, body_len)`.
fn body_shape(payload: &[u8], sub: u16) -> Option<(u16, usize, usize)> {
    match sub {
        1 => {
            let count = read_u16(payload, 22)?;
            Some((count, 24, 2 + 2 * count as usize))
        }
        2 => {
            let packed = read_u32(payload, 22)?;
            let count = read_u16(payload, 30)?;
            if packed != (count as u32 | 0x1_0000) {
                return None;
            }
            Some((count, 32, 10 + 2 * count as usize))
        }
        3 => {
            let a = read_u16(payload, 22)? as usize;
            let b = read_u16(payload, 24)? as usize;
            let count = read_u16(payload, 26)?;
            Some((count, 28, 6 + 2 * count as usize + 8 * (a + b)))
        }
        _ => None,
    }
}

fn attribute(data: &[u8], off: usize) -> String {
    let Some(header) = parse_live_psm_header(data, off) else {
        return "header-invalid".to_string();
    };
    let btf = header.bytes_to_follow as usize;
    if btf < IGTEXTBOX_MIN_PAYLOAD_LEN {
        return format!("btf-below-floor: btf={btf} < {IGTEXTBOX_MIN_PAYLOAD_LEN}");
    }
    let Some(end) = header.body_start.checked_add(btf) else {
        return "overflow".to_string();
    };
    if end > data.len() {
        return format!("truncated: btf={btf} past stream {}", data.len());
    }
    let payload = &data[header.body_start..end];
    let sub_type_word = read_u16(payload, 12).unwrap_or_default();
    let sub = read_u16(payload, 18).unwrap_or(u16::MAX);
    let Some((count, text_start, body_len)) = body_shape(payload, sub) else {
        // For an unknown sub-type, show the bytes a guess would read as text.
        let guess: String = (0..16)
            .filter_map(|i| read_u16(payload, 24 + i * 2))
            .map(|c| char::from_u32(c as u32).unwrap_or('�'))
            .collect();
        return format!(
            "unknown-subtype: sub={sub} sub_type_word=0x{sub_type_word:04X} \
             +22..={:02X?} text-guess={guess:?}",
            &payload[22..payload.len().min(38)]
        );
    };
    if count > IGTEXTBOX_MAX_TEXT_LENGTH {
        return format!("text-too-long: {count}");
    }
    let tail_start = IGTEXTBOX_BODY_START + body_len;
    if tail_start + IGTEXTBOX_TAIL_LEN > payload.len() {
        return format!(
            "no-room-for-tail: tail {tail_start}+36 > {} (sub={sub} count={count})",
            payload.len()
        );
    }
    if text_start + 2 * count as usize > payload.len() {
        return "text-overruns-payload".to_string();
    }
    let mut trailing = [0f64; 4];
    for (i, slot) in trailing.iter_mut().enumerate() {
        let Some(v) = read_f64(payload, tail_start + i * 8) else {
            return "tail-truncated".to_string();
        };
        *slot = v;
    }
    if trailing.iter().any(|v| !v.is_finite()) {
        return "nonfinite-tail".to_string();
    }
    if trailing.iter().any(|v| v.abs() > DOMAIN_LIMIT) {
        return format!("tail-out-of-domain: {trailing:?}");
    }
    let (cos, sin) = (trailing[2], trailing[3]);
    if (cos.hypot(sin) - 1.0).abs() > DIRECTION_TOLERANCE {
        // Show the lettering the refusal is keeping off the page, and the
        // tail beyond the four doubles in case the layout runs longer.
        let text: String = (0..count as usize)
            .filter_map(|i| read_u16(payload, text_start + i * 2))
            .map(|c| char::from_u32(c as u32).unwrap_or('�'))
            .collect();
        let extra: Vec<String> = (0..2)
            .map(|i| {
                read_f64(payload, tail_start + 32 + i * 8)
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_else(|| "-".to_string())
            })
            .collect();
        return format!(
            "direction-not-unit: cos={cos} sin={sin} (hypot={}) sub={sub} count={count} \
             insertion=({:.4},{:.4}) text={text:?} tail_extra=[{}]",
            cos.hypot(sin),
            trailing[0],
            trailing[1],
            extra.join(",")
        );
    }
    "NO-RULE-FIRED".to_string()
}

/// `decode_igboundary_payload`'s rules in order.
fn attribute_boundary(data: &[u8], off: usize) -> String {
    let Some(header) = parse_live_psm_header(data, off) else {
        return "header-invalid".to_string();
    };
    let btf = header.bytes_to_follow as usize;
    let Some(end) = header.body_start.checked_add(btf) else {
        return "overflow".to_string();
    };
    if end > data.len() {
        return format!("truncated: btf={btf} past stream {}", data.len());
    }
    let payload = &data[header.body_start..end];
    let sub_type_word = read_u16(payload, 12).unwrap_or_default();
    if sub_type_word != 0x0010 {
        return format!("sub-type-word: 0x{sub_type_word:04X} != 0x0010");
    }
    if read_u32(payload, 18) != Some(1) {
        return format!("marker: +18={:?} != 1", read_u32(payload, 18));
    }
    let n = read_u32(payload, 22).unwrap_or_default();
    if !(1..=64).contains(&n) {
        return format!("segment-count: {n}");
    }
    let expected = 49 + 41 * n as usize;
    if btf != expected {
        return format!("length-mismatch: btf={btf} != 49+41*{n}={expected}");
    }
    let mut pos = 28usize;
    for i in 0..n as usize {
        if payload.get(pos) != Some(&0x67) {
            return format!(
                "segment-tag: segment {i} at +{pos} is {:02X?}",
                payload.get(pos)
            );
        }
        for (j, label) in [(1, "sx"), (9, "sy"), (17, "ex"), (25, "ey")] {
            match read_f64(payload, pos + j) {
                Some(v) if v.is_finite() && v.abs() <= 1e9 => {}
                other => return format!("segment {i} {label} bad: {other:?}"),
            }
        }
        pos += 33;
    }
    let member_count = read_u32(payload, pos + 17).unwrap_or(u32::MAX);
    if member_count != n {
        return format!("member-count: {member_count} != {n}");
    }
    "NO-RULE-FIRED".to_string()
}

fn main() {
    for fixture in FIXTURES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
        println!("== {fixture} ==");
        let streams = sheet_streams(&path);
        if streams.is_empty() {
            println!("  (no Sheet streams)");
            continue;
        }
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        let mut decoded = 0usize;
        let mut total = 0usize;
        for (name, bytes) in &streams {
            // Sliding scan: `sheet_record_starts` only walks chains whose
            // header it knows; a nested JSite sheet or a lone record in a
            // foreign stream still needs looking at.
            let mut starts = sheet_record_starts(bytes);
            if starts.is_empty() {
                starts = (0..bytes.len().saturating_sub(66)).collect();
            }
            for off in starts {
                let Some(header) = parse_live_psm_header(bytes, off) else {
                    continue;
                };
                let is_family = header.type_code == PSM_TYPE_CODE_IGTEXTBOX
                    || header.type_code == PSM_TYPE_CODE_IGBOUNDARY2D;
                if !is_family {
                    continue;
                }
                total += 1;
                let decoded_ok = if header.type_code == PSM_TYPE_CODE_IGTEXTBOX {
                    decode_igtextbox_at(bytes, off).is_some()
                } else {
                    decode_igboundary_at(bytes, off).is_some()
                };
                if decoded_ok {
                    decoded += 1;
                    continue;
                }
                let reason = if header.type_code == PSM_TYPE_CODE_IGTEXTBOX {
                    attribute(bytes, off)
                } else {
                    attribute_boundary(bytes, off)
                };
                let rule = reason.split(':').next().unwrap_or("?").to_string();
                *tally.entry(rule.clone()).or_default() += 1;
                println!("  {name} @{off:#07x}: {reason}");
            }
        }
        println!(
            "  total 0x004D={total} decoded={decoded} refused={}",
            total - decoded
        );
        for (rule, count) in &tally {
            println!("    {count:>3} {rule}");
        }
    }
}
