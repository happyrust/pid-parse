//! Which rule refuses the five real drawings' `igLineString2d` records?
//!
//! `probe_real5_undrawn` counts 36 refused `0x0084` LineString Objects across
//! the corpus -- 18 in the A0 I井场 sheet alone, which is also the drawing
//! whose geometry recall still trails (0.967 vs >=0.996 everywhere else).
//! Every rule below mirrors the shipped `IgLineString2dDecoder` in order, so
//! the first to fire is the reason the decoder said no.
//!
//! ```powershell
//! cargo run --release --example probe_real5_linestring_refusals
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_iglinestring_at, parse_live_psm_header, sheet_record_starts,
    PSM_TYPE_CODE_IGLINESTRING2D,
};

const FILES: &[&str] = &[
    r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
    r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
];

// Mirrored from `IgLineString2dDecoder` (private consts in the crate).
const MIN_PAYLOAD: usize = 24 + 2 * 16;
const MAX_VC: u32 = 10_000;
const FORM_MAX: u8 = 6;
const DOMAIN: f64 = 1e9;

fn read_u16(p: &[u8], at: usize) -> Option<u16> {
    p.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn read_u32(p: &[u8], at: usize) -> Option<u32> {
    p.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    p.get(at..at + 8)
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

fn attribute(data: &[u8], off: usize) -> String {
    let Some(header) = parse_live_psm_header(data, off) else {
        return "header-invalid".to_string();
    };
    let btf = header.bytes_to_follow as usize;
    if btf < MIN_PAYLOAD {
        return format!("btf-below-floor: btf={btf} < {MIN_PAYLOAD}");
    }
    if !(btf - 24).is_multiple_of(16) {
        return format!("btf-not-vertex-aligned: btf={btf}");
    }
    let computed_vc = ((btf - 24) / 16) as u32;
    if !(2..=MAX_VC).contains(&computed_vc) {
        return format!("vertex-count-range: {computed_vc}");
    }
    let Some(end) = header.body_start.checked_add(btf) else {
        return "overflow".to_string();
    };
    if end > data.len() {
        return format!("truncated: btf={btf} past stream {}", data.len());
    }
    let payload = &data[header.body_start..end];

    let remaining_header = read_u32(payload, 8).unwrap_or_default();
    if remaining_header > 0xFFFF {
        return format!("remaining-header: 0x{remaining_header:X}");
    }
    let sub_type_word = read_u16(payload, 12).unwrap_or_default();
    let inline_vc = read_u32(payload, 18).unwrap_or(u32::MAX);
    if inline_vc != computed_vc {
        return format!("inline-vc-mismatch: inline={inline_vc} computed={computed_vc} sub_type_word=0x{sub_type_word:04X} remaining_header=0x{remaining_header:X}");
    }
    let form = payload[22];
    let scope = payload[23];
    if form > FORM_MAX {
        return format!("form: {form} (sub_type_word=0x{sub_type_word:04X})");
    }
    if scope > 4 && scope != 6 {
        return format!("scope: {scope} (sub_type_word=0x{sub_type_word:04X} form={form})");
    }
    let first = (
        read_f64(payload, 24).unwrap_or(f64::NAN),
        read_f64(payload, 32).unwrap_or(f64::NAN),
    );
    let mut all_same = true;
    let mut last = first;
    for i in 0..computed_vc as usize {
        let pos = 24 + i * 16;
        let (x, y) = (
            read_f64(payload, pos).unwrap_or(f64::NAN),
            read_f64(payload, pos + 8).unwrap_or(f64::NAN),
        );
        if !x.is_finite() || !y.is_finite() {
            return format!("vertex-nonfinite: v{i}");
        }
        if x.abs() > DOMAIN || y.abs() > DOMAIN {
            return format!("vertex-domain: v{i} ({x},{y})");
        }
        if i > 0 && ((x - first.0).abs() > 1e-12 || (y - first.1).abs() > 1e-12) {
            all_same = false;
        }
        last = (x, y);
    }
    if all_same {
        return format!(
            "degenerate-all-same: vc={computed_vc} at ({:.4},{:.4}) form={} scope={} sub_type_word=0x{sub_type_word:04X}",
            first.0, first.1, form, scope
        );
    }
    format!(
        "NO-RULE-FIRED?!: vc={computed_vc} first=({:.4},{:.4}) last=({:.4},{:.4}) form={} scope={} sub_type_word=0x{sub_type_word:04X} btf={btf}",
        first.0, first.1, last.0, last.1, form, scope
    )
}

fn main() {
    for file in FILES {
        let path = Path::new(file);
        println!(
            "\n========== {}",
            path.file_name().unwrap().to_string_lossy()
        );
        let streams = sheet_streams(path);
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;
        let mut decoded = 0usize;
        for (name, bytes) in &streams {
            let mut starts = sheet_record_starts(bytes);
            if starts.is_empty() {
                starts = (0..bytes.len().saturating_sub(66)).collect();
            }
            for off in starts {
                let Some(header) = parse_live_psm_header(bytes, off) else {
                    continue;
                };
                if header.type_code != PSM_TYPE_CODE_IGLINESTRING2D {
                    continue;
                }
                total += 1;
                if decode_iglinestring_at(bytes, off).is_some() {
                    decoded += 1;
                    continue;
                }
                let reason = attribute(bytes, off);
                let rule = reason.split(':').next().unwrap_or("?").to_string();
                *tally.entry(rule).or_default() += 1;
                println!("  {name} @{off:#07x}: {reason}");
            }
        }
        println!(
            "  total={total} decoded={decoded} refused={}",
            total - decoded
        );
        for (rule, count) in &tally {
            println!("    {count:>3} {rule}");
        }
    }
}
