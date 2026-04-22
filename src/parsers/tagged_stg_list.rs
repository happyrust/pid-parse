//! Parser for the `JTaggedTxtStgList` stream — a small index that maps a
//! logical "storage list" name (e.g. "TaggedTxtStorages") to the actual
//! storage directory names inside the document (e.g. "TaggedTxtData").
//!
//! Observed layout (sampled file is 70 bytes total):
//!
//! ```text
//! +00..+X    `list_name` as a run of UTF-16LE printable-ASCII code units
//!            (NOT null-terminated; bounded by the following u32)
//! +X..+X+3   u32 entry count (observed `0x00000001`)
//! For each entry:
//!   [u32 char_count including L'\0']
//!   [UTF-16LE storage_name of char_count chars, last word is 0x0000]
//! ```
//!
//! The parser is defensive: if layout does not match, returns `None`.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{TaggedTextStorageEntry, TaggedTextStorageList};

/// Parse `JTaggedTxtStgList`.
///
/// Thin back-compat wrapper around [`parse_tagged_stg_list_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_tagged_stg_list(data: &[u8]) -> Option<TaggedTextStorageList> {
    let mut trace = ParserTraceBuilder::new("parse_tagged_stg_list");
    parse_tagged_stg_list_with_trace(data, &mut trace)
}

/// Phase 12b-1d trace-aware variant of [`parse_tagged_stg_list`].
///
/// Trace schema:
/// - `[0..after_name]` — UTF-16LE printable-ASCII `list_name` run —
///   `TraceConfidence::Decoded`
/// - `[after_name..after_name+4]` — `count` u32 LE — `Decoded`
/// - per entry at offset `pos`:
///   - `[pos..pos+4]` — entry char_count — `Decoded`
///   - `[pos+4..name_end]` — UTF-16LE storage_name (including L'\0') —
///     `Decoded`
/// - Bytes past the last successfully-read entry (e.g. truncated
///   entries or real trailing garbage) surface as leftover.
pub fn parse_tagged_stg_list_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<TaggedTextStorageList> {
    if data.len() < 8 {
        return None;
    }
    // Read a UTF-16LE run of printable-ASCII characters at offset 0; it ends
    // when we hit a u16 outside the printable-ASCII range (typically the
    // low byte of the following u32 count).
    let (list_name, after_name) = read_utf16le_ascii_run(data, 0);
    if list_name.is_empty() || after_name + 4 > data.len() {
        return None;
    }
    trace.consume(
        ByteRange::new(0, after_name as u64),
        TraceConfidence::Decoded,
    );
    let count = u32::from_le_bytes([
        data[after_name],
        data[after_name + 1],
        data[after_name + 2],
        data[after_name + 3],
    ]);
    trace.consume(
        ByteRange::new(after_name as u64, (after_name + 4) as u64),
        TraceConfidence::Decoded,
    );
    let mut pos = after_name + 4;
    let mut entries = Vec::new();
    for _ in 0..count {
        if pos + 4 > data.len() {
            break;
        }
        let char_count =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let name_start = pos + 4;
        let name_end = name_start.checked_add(char_count.checked_mul(2)?)?;
        if name_end > data.len() || char_count > 256 {
            break;
        }
        let name_bytes = &data[name_start..name_end];
        let words: Vec<u16> = name_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&w| w != 0)
            .collect();
        let storage_name = String::from_utf16_lossy(&words);
        trace.consume(
            ByteRange::new(pos as u64, name_start as u64),
            TraceConfidence::Decoded,
        );
        trace.consume(
            ByteRange::new(name_start as u64, name_end as u64),
            TraceConfidence::Decoded,
        );
        entries.push(TaggedTextStorageEntry { storage_name });
        pos = name_end;
    }
    Some(TaggedTextStorageList {
        size: data.len() as u64,
        list_name,
        entries,
    })
}

fn read_utf16le_ascii_run(data: &[u8], start: usize) -> (String, usize) {
    let mut i = start;
    let mut words: Vec<u16> = Vec::new();
    while i + 2 <= data.len() {
        let w = u16::from_le_bytes([data[i], data[i + 1]]);
        if !(0x20..=0x7e).contains(&w) {
            break;
        }
        words.push(w);
        i += 2;
    }
    (String::from_utf16_lossy(&words), i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn parses_single_entry() {
        let mut data = Vec::new();
        // list_name: plain UTF-16LE ASCII run (no L'\0' terminator)
        data.extend(utf16("TaggedTxtStorages"));
        // u32 count = 1 (first byte 0x01 breaks the ascii-run scan above)
        data.extend_from_slice(&1u32.to_le_bytes());
        // storage_name: char_count including L'\0'
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]); // L'\0' terminator
        let r = parse_tagged_stg_list(&data).expect("valid");
        assert_eq!(r.list_name, "TaggedTxtStorages");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].storage_name, "TaggedTxtData");
    }

    #[test]
    fn rejects_short_stream() {
        assert!(parse_tagged_stg_list(&[]).is_none());
        assert!(parse_tagged_stg_list(&[1, 2, 3, 4]).is_none());
    }

    #[test]
    fn trace_aware_tagged_stg_list_consumes_header_and_each_entry() {
        let mut data = Vec::new();
        data.extend(utf16("TaggedTxtStorages"));
        let after_name = data.len(); // 34
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]);

        let mut b = ParserTraceBuilder::new("parse_tagged_stg_list");
        let list =
            parse_tagged_stg_list_with_trace(&data, &mut b).expect("valid");
        assert_eq!(list.entries.len(), 1);

        let trace = b.build("/JTaggedTxtStgList", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "every byte of this fixture should be consumed",
        );
        assert!(trace.leftover_ranges.is_empty());
        // Builder merges every same-confidence adjacent range, and
        // every byte is Decoded → one unified range.
        let decoded = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            decoded,
            vec![ByteRange::new(0, data.len() as u64)],
            "expected single merged Decoded range; got {decoded:?}"
        );
        // Sanity: the `count=1` u32 sits right after `after_name`.
        assert_eq!(after_name, 34);
    }

    #[test]
    fn trace_aware_tagged_stg_list_leaves_truncated_tail_as_leftover() {
        let mut data = Vec::new();
        data.extend(utf16("TaggedTxtStorages"));
        data.extend_from_slice(&2u32.to_le_bytes()); // declared 2 entries
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]);
        let first_entry_end = data.len();
        // Second entry is truncated (only 3 bytes of its u32 count).
        data.extend_from_slice(&[5, 0, 0]);

        let mut b = ParserTraceBuilder::new("parse_tagged_stg_list");
        let list =
            parse_tagged_stg_list_with_trace(&data, &mut b).expect("valid");
        assert_eq!(list.entries.len(), 1);

        let trace = b.build("/JTaggedTxtStgList", data.len() as u64);
        // The 3 truncated bytes are not consumed → leftover.
        assert_eq!(trace.consumed_bytes(), first_entry_end as u64);
        assert_eq!(trace.leftover_bytes(), 3);
        assert_eq!(
            trace.leftover_ranges,
            vec![ByteRange::new(first_entry_end as u64, data.len() as u64)]
        );
    }

    #[test]
    fn back_compat_parse_tagged_stg_list_matches_trace_variant_byte_for_byte() {
        let mut data = Vec::new();
        data.extend(utf16("TaggedTxtStorages"));
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]);

        let without_trace = parse_tagged_stg_list(&data).expect("old API works");

        let mut b = ParserTraceBuilder::new("parse_tagged_stg_list");
        let with_trace = parse_tagged_stg_list_with_trace(&data, &mut b).expect("new API works");

        assert_eq!(without_trace.size, with_trace.size);
        assert_eq!(without_trace.list_name, with_trace.list_name);
        assert_eq!(without_trace.entries.len(), with_trace.entries.len());
        for (a, b_entry) in without_trace.entries.iter().zip(with_trace.entries.iter()) {
            assert_eq!(a.storage_name, b_entry.storage_name);
        }
    }

    #[test]
    fn truncated_entry_stops_gracefully() {
        let mut data = Vec::new();
        data.extend(utf16("TaggedTxtStorages"));
        data.extend_from_slice(&2u32.to_le_bytes()); // declared 2
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]);
        // Second entry truncated (only 3 bytes of a u32 count)
        data.extend_from_slice(&[5, 0, 0]);
        let r = parse_tagged_stg_list(&data).expect("valid");
        assert_eq!(r.entries.len(), 1);
    }
}
