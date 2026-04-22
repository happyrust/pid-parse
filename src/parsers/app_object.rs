//! Parser for the `AppObject` stream — a registry of external COM / DLL
//! plugins that the source application linked into this drawing.
//!
//! Observed layout:
//!
//! ```text
//! +00..+03  u32 leading value (observed 5; possibly entry count or
//!           registry version; NOT necessarily the exact entry count)
//! Then a sequence of entries of variable length:
//!   +00..+0F   16-byte CLSID (COM class id, raw little-endian layout)
//!   +10..+13   u32 path_char_count (UTF-16LE character count,
//!              INCLUDING the trailing L'\0')
//!   +14..      UTF-16LE path (path_char_count * 2 bytes)
//!   +...       3 bytes of filler (observed `\0\0\0`) before the next CLSID
//! ```
//!
//! A trailing "orphan" record may appear at the very end (CLSID plus a few
//! bytes, without a valid path); we record any such bytes in `trailing_bytes`.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{AppObjectEntry, AppObjectRegistry};

/// Parse `AppObject`. Returns `None` if the stream is too short.
///
/// Thin back-compat wrapper around [`parse_app_object_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_app_object(data: &[u8]) -> Option<AppObjectRegistry> {
    let mut trace = ParserTraceBuilder::new("parse_app_object");
    parse_app_object_with_trace(data, &mut trace)
}

/// Phase 12b-1d trace-aware variant of [`parse_app_object`].
///
/// Trace schema per entry at offset `pos`:
/// - `[0..4]` — leading u32 (stream-level, consumed once) — `Decoded`
/// - `[pos..pos+16]` — CLSID — `Decoded`
/// - `[pos+16..pos+20]` — path_char_count — `Decoded`
/// - `[pos+20..path_end]` — UTF-16LE path (including L'\0') — `Decoded`
/// - `[path_end..new_pos]` — up to 3 bytes of zero filler between
///   entries — `Probed` (structural role known: "resync buffer", but
///   not a named semantic field)
///
/// Defensive abort conditions (`char_count == 0 || > 2048`,
/// `path_end > data.len()`) break out of the loop without consuming the
/// malformed header's 20 bytes — those surface as leftover so a
/// byte-audit consumer can see exactly where the parser gave up.
pub fn parse_app_object_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<AppObjectRegistry> {
    if data.len() < 4 + 16 + 4 {
        return None;
    }
    let leading = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    trace.consume(ByteRange::new(0, 4), TraceConfidence::Decoded);
    let mut entries = Vec::new();
    let mut pos = 4usize;
    while pos + 20 <= data.len() {
        let clsid_end = pos + 16;
        let char_count = u32::from_le_bytes([
            data[clsid_end],
            data[clsid_end + 1],
            data[clsid_end + 2],
            data[clsid_end + 3],
        ]) as usize;
        let path_start = clsid_end + 4;
        if char_count == 0 || char_count > 2048 {
            break;
        }
        let path_byte_len = char_count * 2;
        let path_end = match path_start.checked_add(path_byte_len) {
            Some(v) if v <= data.len() => v,
            _ => break,
        };
        let clsid = format_guid(&data[pos..clsid_end]);
        let path = read_utf16le_null_terminated(&data[path_start..path_end]);
        // Entry consume: CLSID + char_count + path, all Decoded.
        trace.consume(
            ByteRange::new(pos as u64, path_end as u64),
            TraceConfidence::Decoded,
        );
        entries.push(AppObjectEntry {
            offset: pos,
            clsid,
            path,
        });
        // The next CLSID follows the path. Two patterns observed:
        //   (a) path ends on an odd byte and is followed by 3 bytes filler
        //   (b) path ends on an even byte and is followed by 1 byte filler
        // We resync by advancing up to 3 zero bytes (or any byte run short
        // enough to reach a plausible next CLSID).
        let filler_start = path_end;
        pos = path_end;
        let skip_limit = pos + 3;
        while pos < data.len() && pos < skip_limit && data[pos] == 0 {
            pos += 1;
        }
        if pos > filler_start {
            trace.consume(
                ByteRange::new(filler_start as u64, pos as u64),
                TraceConfidence::Probed,
            );
        }
    }
    Some(AppObjectRegistry {
        size: data.len() as u64,
        leading_u32: leading,
        entries,
        trailing_bytes: data.len().saturating_sub(pos),
    })
}

/// Format 16 raw bytes as a canonical GUID string.
///
/// COM `CLSID` is stored in mixed endianness on disk (Microsoft's classic
/// binary GUID layout): the first three fields are little-endian, the last
/// two are big-endian.
fn format_guid(bytes: &[u8]) -> String {
    assert_eq!(bytes.len(), 16);
    let d1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let d2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let d3 = u16::from_le_bytes([bytes[6], bytes[7]]);
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        d1,
        d2,
        d3,
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

/// Decode a UTF-16LE byte slice, dropping any trailing L'\0'.
fn read_utf16le_null_terminated(bytes: &[u8]) -> String {
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&w| w != 0)
        .collect();
    String::from_utf16_lossy(&words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str) -> Vec<u8> {
        let mut out: Vec<u8> = s.encode_utf16().flat_map(|w| w.to_le_bytes()).collect();
        out.extend_from_slice(&[0, 0]); // L'\0' terminator
        out
    }

    fn make_entry(clsid: [u8; 16], path: &str) -> Vec<u8> {
        let path_bytes = utf16le(path);
        let char_count = (path_bytes.len() / 2) as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&clsid);
        buf.extend_from_slice(&char_count.to_le_bytes());
        buf.extend_from_slice(&path_bytes);
        // 3-byte filler observed in real streams
        buf.extend_from_slice(&[0, 0, 0]);
        buf
    }

    #[test]
    fn parses_two_entries() {
        let mut data = Vec::new();
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend(make_entry([0x11; 16], "C:\\Plugins\\A.dll"));
        data.extend(make_entry([0x22; 16], "C:\\Plugins\\B.dll"));
        let r = parse_app_object(&data).expect("valid");
        assert_eq!(r.leading_u32, 5);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].path, "C:\\Plugins\\A.dll");
        assert_eq!(r.entries[1].path, "C:\\Plugins\\B.dll");
        assert!(r.entries[0].clsid.starts_with("{11111111-"));
    }

    #[test]
    fn too_short_returns_none() {
        assert!(parse_app_object(&[0; 10]).is_none());
    }

    #[test]
    fn guid_formatting_matches_com_layout() {
        // Bytes that decode to {D69F42DF-7717-11D1-9790-08003655F302}
        let bytes = [
            0xDF, 0x42, 0x9F, 0xD6, 0x17, 0x77, 0xD1, 0x11, 0x97, 0x90, 0x08, 0x00, 0x36, 0x55,
            0xF3, 0x02,
        ];
        let s = format_guid(&bytes);
        assert_eq!(s, "{D69F42DF-7717-11D1-9790-08003655F302}");
    }

    #[test]
    fn trace_aware_app_object_consumes_leading_plus_entries_with_filler() {
        let mut data = Vec::new();
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend(make_entry([0x11; 16], "C:\\Plugins\\A.dll"));
        data.extend(make_entry([0x22; 16], "C:\\Plugins\\B.dll"));

        let mut b = ParserTraceBuilder::new("parse_app_object");
        let r = parse_app_object_with_trace(&data, &mut b).expect("valid");
        assert_eq!(r.entries.len(), 2);

        let trace = b.build("/AppObject", data.len() as u64);
        // Fixture: every entry is followed by 3 zero bytes of filler.
        // The parser resyncs through all 3 zero bytes, so the entire
        // stream should be consumed.
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
        assert!(trace.leftover_ranges.is_empty());

        // Filler zones are Probed; CLSID + char_count + path + leading
        // u32 are all Decoded. At least one Probed range must exist
        // (two entries -> two filler zones).
        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            probed.len(),
            2,
            "expected one Probed filler range per entry; got {probed:?}",
        );
        for range in &probed {
            assert_eq!(range.len(), 3, "filler should be exactly 3 bytes");
        }
    }

    #[test]
    fn trace_aware_app_object_leaves_absurd_tail_as_leftover() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x11; 16]);
        data.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // bogus char count

        let mut b = ParserTraceBuilder::new("parse_app_object");
        let r = parse_app_object_with_trace(&data, &mut b).expect("still returns");
        assert!(r.entries.is_empty());

        let trace = b.build("/AppObject", data.len() as u64);
        // Only the leading u32 was consumed; the 20-byte malformed
        // entry header must surface as leftover.
        assert_eq!(trace.consumed_bytes(), 4);
        assert_eq!(trace.leftover_bytes(), 20);
        assert_eq!(
            trace.leftover_ranges,
            vec![ByteRange::new(4, 24)],
        );
    }

    #[test]
    fn back_compat_parse_app_object_matches_trace_variant_byte_for_byte() {
        let mut data = Vec::new();
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend(make_entry([0x11; 16], "C:\\Plugins\\A.dll"));
        data.extend(make_entry([0x22; 16], "C:\\Plugins\\B.dll"));

        let without_trace = parse_app_object(&data).expect("old API works");

        let mut b = ParserTraceBuilder::new("parse_app_object");
        let with_trace = parse_app_object_with_trace(&data, &mut b).expect("new API works");

        assert_eq!(without_trace.size, with_trace.size);
        assert_eq!(without_trace.leading_u32, with_trace.leading_u32);
        assert_eq!(without_trace.trailing_bytes, with_trace.trailing_bytes);
        assert_eq!(without_trace.entries.len(), with_trace.entries.len());
        for (a, b_entry) in without_trace.entries.iter().zip(with_trace.entries.iter()) {
            assert_eq!(a.offset, b_entry.offset);
            assert_eq!(a.clsid, b_entry.clsid);
            assert_eq!(a.path, b_entry.path);
        }
    }

    #[test]
    fn absurd_char_count_aborts() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x11; 16]);
        data.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // bogus char count
        let r = parse_app_object(&data).expect("still returns");
        assert!(r.entries.is_empty());
    }
}
