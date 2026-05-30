//! Phase 26 Slice A probe — PSM 0x0010 attribute-fragment text extraction.
//!
//! Reads each fixture's Sheet streams via proper CFB extraction (NOT a
//! whole-file byte scan), scans `type & 0x3FFF == 0x10` records, and parses
//! the `marker(4) + aux(8) + [u16 len + UTF-16LE]*` structure documented in
//! `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`. Reports the
//! number of extractable attribute strings per fixture, plus the single- vs
//! multi-string split, to seed the Phase 26 cross-fixture ratchet baseline.

use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;

/// Decode a byte slice as UTF-16LE, returning `None` on odd length or any
/// unpaired surrogate.
fn decode_utf16le(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut out = String::new();
    for r in char::decode_utf16(units) {
        match r {
            Ok(c) => out.push(c),
            Err(_) => return None,
        }
    }
    Some(out)
}

/// Parse consecutive length-prefixed UTF-16LE strings from a 0x0010 payload,
/// starting after `marker(4) + aux(8)`. Exploratory (reads multiple runs) to
/// measure single- vs multi-string layout for the Slice B decision.
fn parse_attribute_strings(payload: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 12usize;
    while pos + 2 <= payload.len() {
        let len = u16::from_le_bytes([payload[pos], payload[pos + 1]]) as usize;
        if len == 0 || len > 4096 {
            break;
        }
        let start = pos + 2;
        let Some(end) = start.checked_add(len * 2) else {
            break;
        };
        if end > payload.len() {
            break;
        }
        let Some(text) = decode_utf16le(&payload[start..end]) else {
            break;
        };
        let clean = text
            .chars()
            .all(|c| !c.is_control() || c == ' ' || c == '\t');
        let has_visible = text.chars().any(|c| !c.is_whitespace());
        if !clean || !has_visible {
            break;
        }
        out.push(text);
        pos = end;
    }
    out
}

/// Returns `(total_0x0010_records, records_with_string, total_strings)`.
fn probe_fixture(path: &Path) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let sheet_paths: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();

    let mut total_recs = 0usize;
    let mut recs_with_str = 0usize;
    let mut total_strs = 0usize;
    let mut single = 0usize;
    let mut multi = 0usize;
    let mut samples: Vec<String> = Vec::new();

    for sp in &sheet_paths {
        let mut stream = cfb.open_stream(sp)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        let mut off = 0usize;
        while off + 6 <= bytes.len() {
            let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
            if (type_word & 0x3FFF) == 0x0010 {
                let btf = u32::from_le_bytes([
                    bytes[off + 2],
                    bytes[off + 3],
                    bytes[off + 4],
                    bytes[off + 5],
                ]) as usize;
                if (8..=100_000).contains(&btf) && off + 6 + btf <= bytes.len() {
                    let payload = &bytes[off + 6..off + 6 + btf];
                    total_recs += 1;
                    let strs = parse_attribute_strings(payload);
                    if !strs.is_empty() {
                        recs_with_str += 1;
                        total_strs += strs.len();
                        if strs.len() == 1 {
                            single += 1;
                        } else {
                            multi += 1;
                        }
                        for s in &strs {
                            if samples.len() < 15 && s.trim().chars().count() >= 2 {
                                samples.push(s.clone());
                            }
                        }
                    }
                    off += 6 + btf;
                    continue;
                }
            }
            off += 1;
        }
    }

    println!("\n=== {} ===", path.display());
    println!(
        "  sheet_streams={}  0x0010_records={}  with_string={}  total_strings={}  (single={} multi={})",
        sheet_paths.len(),
        total_recs,
        recs_with_str,
        total_strs,
        single,
        multi
    );
    for s in samples.iter().take(15) {
        println!("    [{}]", s.trim());
    }
    Ok((total_recs, recs_with_str, total_strs))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/D06.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ];
    let mut g_recs = 0usize;
    let mut g_with = 0usize;
    let mut g_str = 0usize;
    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        let (r, w, s) = probe_fixture(path)?;
        g_recs += r;
        g_with += w;
        g_str += s;
    }
    println!(
        "\n=== CROSS-FIXTURE TOTAL: 0x0010_records={} with_string={} total_strings={} ===",
        g_recs, g_with, g_str
    );
    Ok(())
}
