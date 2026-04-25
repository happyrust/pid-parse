//! Decoder for `JSite` property blobs.
//!
//! Extracts ASCII / UTF-16 strings, `key=value` pairs, and 32-hex
//! GUIDs from a raw property payload and packages them into
//! [`JProperties`]. The input format is heuristic — `SmartPlant`
//! stores these as opaque CFB streams — so the output is
//! best-effort: the raw byte length is retained so consumers can
//! still sanity-check coverage.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::JProperties;
use std::collections::BTreeMap;

/// Flatten a raw `JProperties` blob into the [`JProperties`] DTO —
/// extracts every recoverable string / key-value pair / GUID and
/// records the original byte length for audit.
pub fn parse_jproperties(data: &[u8]) -> JProperties {
    let mut trace = ParserTraceBuilder::new("parse_jproperties");
    parse_jproperties_with_trace(data, &mut trace)
}

/// Trace-aware variant of [`parse_jproperties`].
///
/// `JProperties` is still a heuristic parser, so it only claims bytes
/// that feed recovered ASCII / UTF-16LE text runs. Opaque binary prefix,
/// suffix, and gaps intentionally remain leftover.
pub fn parse_jproperties_with_trace(data: &[u8], trace: &mut ParserTraceBuilder) -> JProperties {
    let ascii_ranges = ascii_string_ranges(data, 4, 256);
    for &(start, end) in &ascii_ranges {
        trace.consume(ByteRange::new(start, end), TraceConfidence::Probed);
    }
    for (start, end) in utf16le_string_ranges(data, 4, 256) {
        if ascii_ranges
            .iter()
            .any(|&(ascii_start, ascii_end)| start < ascii_end && ascii_start < end)
        {
            continue;
        }
        trace.consume(ByteRange::new(start, end), TraceConfidence::Probed);
    }

    let mut strings = crate::parsers::string_scan::scan_ascii_strings(data, 256);

    for s in crate::parsers::string_scan::scan_utf16le_strings(data, 4, 256) {
        if !strings.contains(&s) {
            strings.push(s);
        }
    }

    let mut key_values = BTreeMap::new();
    let mut i = 0;
    while i + 1 < strings.len() {
        let key = &strings[i];
        let value = &strings[i + 1];
        if is_key_like(key) {
            key_values.entry(key.clone()).or_insert(value.clone());
        }
        i += 1;
    }

    let guids = crate::parsers::string_scan::scan_guids(data, 64);

    JProperties {
        strings,
        key_values,
        guids,
        raw_len: data.len(),
    }
}

fn ascii_string_ranges(data: &[u8], min_len: usize, limit: usize) -> Vec<(u64, u64)> {
    let mut result = Vec::new();
    let mut start: Option<usize> = None;

    for (idx, &byte) in data.iter().enumerate() {
        if (0x20..=0x7e).contains(&byte) || byte == b'\t' {
            start.get_or_insert(idx);
            continue;
        }

        if let Some(run_start) = start.take() {
            if idx - run_start >= min_len {
                result.push((run_start as u64, idx as u64));
                if result.len() >= limit {
                    return result;
                }
            }
        }
    }

    if let Some(run_start) = start {
        if data.len() - run_start >= min_len && result.len() < limit {
            result.push((run_start as u64, data.len() as u64));
        }
    }

    result
}

fn utf16le_string_ranges(data: &[u8], min_chars: usize, limit: usize) -> Vec<(u64, u64)> {
    let mut result = Vec::new();
    let mut i = 0usize;

    while i + 1 < data.len() && result.len() < limit {
        let run_start = i;
        let mut chars = 0usize;

        while i + 1 < data.len() {
            let word = u16::from_le_bytes([data[i], data[i + 1]]);
            if word == 0 {
                break;
            }
            let printable = (0x20..=0x7e).contains(&word) || word > 0x7f;
            if !printable {
                break;
            }
            chars += 1;
            i += 2;
        }

        if chars >= min_chars {
            result.push((run_start as u64, i as u64));
        }

        i += 2;
    }

    result
}

fn is_key_like(s: &str) -> bool {
    s.len() >= 3
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byte_audit::ParserTraceBuilder;

    fn utf16le(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn trace_consumes_recovered_text_runs_without_claiming_binary_bytes() {
        let mut data = vec![0xFF, 0x00];
        data.extend_from_slice(b"SymbolName");
        data.extend_from_slice(&[0x00, 0x00]);
        data.extend(utf16le("PUMP-101"));
        data.push(0xEE);

        let mut builder = ParserTraceBuilder::new("parse_jproperties");
        let parsed = parse_jproperties_with_trace(&data, &mut builder);
        let trace = builder.build("/JSite0001/JProperties", data.len() as u64);

        assert!(parsed.strings.iter().any(|s| s == "SymbolName"));
        assert!(parsed.strings.iter().any(|s| s == "PUMP-101"));
        assert_eq!(trace.consumed_bytes(), 10 + 16);
        assert_eq!(trace.leftover_bytes(), (data.len() - 26) as u64);
    }
}
