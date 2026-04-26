//! Record decoder for the `Unclustered Dynamic Attributes` stream.
//!
//! Parses the sequence of `AttributeRecord`s and their trailing
//! `DaRecordTrailer`s out of the DA stream bytes, which
//! [`crate::streams::dynamic_attrs`] then stitches into
//! [`crate::model::ObjectInventory`] / [`crate::model::ObjectGraph`].
//! The record layout is recovered heuristically from observed hex
//! dumps — see module-local comments for the magic-byte signatures.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{
    AttributeField, AttributeRecord, AttributeValue, DaRecordTrailer, ProbeSummary,
};

/// Parse structured attribute records from the Unclustered Dynamic Attributes stream body.
///
/// The stream follows the pattern (observed from hex analysis):
///   - Stream header: 16+ bytes (magic + meta), followed by zero padding
///   - Section body starts after padding with: u32 count, then 0xFFFF marker
///   - Each section: u32 `section_len`, then interleaved class name + attribute fields
///   - Attribute fields: 1-byte type tag + length-prefixed name + typed value
///
/// This parser uses a tolerant, heuristic approach suitable for reverse-engineering:
/// it scans for recognizable class-name markers and extracts name/value pairs.
pub fn parse_attribute_records(data: &[u8]) -> (Vec<AttributeRecord>, ProbeSummary) {
    let body_start = find_body_start(data);
    let marker_count = count_markers(data, body_start);
    let mut records = Vec::new();
    let mut pos = body_start;

    while pos < data.len() {
        if let Some((rec, next_pos)) = try_parse_record(data, pos) {
            records.push(rec);
            pos = next_pos;
        } else {
            pos += 1;
        }
    }

    let summary = ProbeSummary {
        body_start_offset: body_start,
        marker_count,
        records_extracted: records.len(),
        bytes_scanned: pos.saturating_sub(body_start),
    };

    (records, summary)
}

/// Count 0x89 0x00 markers in the stream body (probe-level info).
fn count_markers(data: &[u8], start: usize) -> usize {
    let mut count = 0;
    for i in start..data.len().saturating_sub(1) {
        if data[i] == 0x89 && data[i + 1] == 0x00 {
            count += 1;
        }
    }
    count
}

fn find_body_start(data: &[u8]) -> usize {
    // Skip header region: look for end of first zero run after offset 8
    let mut i = 8;
    while i < data.len() && i < 32 {
        if data[i] != 0 {
            break;
        }
        i += 1;
    }
    i
}

fn try_parse_record(data: &[u8], pos: usize) -> Option<(AttributeRecord, usize)> {
    // Look for pattern: 0x0089 type marker at current position or nearby
    if pos + 6 > data.len() {
        return None;
    }

    // Check for 0x89 0x00 (type 0x0089) which marks a new record section
    if data[pos] != 0x89 || data[pos + 1] != 0x00 {
        return None;
    }

    // After 0x89 0x00: scan for the class section
    let mut cursor = pos + 2;

    // Read body region length/flags (varies)
    if cursor + 4 > data.len() {
        return None;
    }
    let section_len = u32_le(data, cursor) as usize;
    cursor += 4;

    if section_len == 0 || cursor + section_len > data.len() + 4 {
        return None;
    }

    let section_end = (cursor + section_len).min(data.len());

    // Skip initial flags/counters until we hit readable content
    while cursor < section_end && cursor + 2 <= data.len() {
        if is_printable_run(data, cursor, 3) {
            break;
        }
        cursor += 1;
    }

    // Try to extract class name: look for a null-terminated ASCII string
    let class_name = if let Some((name, after)) = read_null_terminated_ascii(data, cursor) {
        cursor = after;
        name
    } else {
        return None;
    };

    // Now extract attribute fields until section end
    let mut attributes = Vec::new();
    let max_attrs = 64;

    while cursor < section_end && attributes.len() < max_attrs {
        // Skip non-printable bytes (type tags, length prefixes)
        let scan_start = cursor;
        while cursor < section_end && !is_printable_run(data, cursor, 3) {
            cursor += 1;
        }
        if cursor >= section_end {
            break;
        }
        // We may have skipped a type/length prefix
        let _skipped = cursor - scan_start;

        // Read attribute name
        let Some((attr_name, after_name)) = read_null_terminated_ascii(data, cursor) else {
            break;
        };
        cursor = after_name;

        if attr_name.is_empty() {
            continue;
        }

        let (value, raw_value) = read_attribute_value(data, &mut cursor, section_end);

        attributes.push(AttributeField {
            name: attr_name,
            value,
            raw_value,
        });
    }

    if attributes.is_empty() && class_name.is_empty() {
        return None;
    }

    Some((
        AttributeRecord {
            class_name,
            attributes,
            confidence: "heuristic".to_string(),
        },
        section_end,
    ))
}

/// Returns the decoded value plus an optional `raw_value` audit trail: if the
/// heuristic `strip_value_prefix` stripped a leading byte from a printable
/// string, the pre-strip string is returned so callers can detect false
/// positives (e.g. a legitimate `"1DWG-..."` that the heuristic collapsed
/// into `"DWG-..."`).
fn read_attribute_value(
    data: &[u8],
    cursor: &mut usize,
    end: usize,
) -> (AttributeValue, Option<String>) {
    if *cursor >= end {
        return (AttributeValue::Empty, None);
    }

    // Try 8-byte double FIRST — OLE dates and coordinates are common in .pid files.
    // Doubles have recognizable exponent patterns that help distinguish them from strings.
    if *cursor + 8 <= end {
        let bytes = &data[*cursor..*cursor + 8];
        let f = f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        if is_plausible_double(f, bytes) {
            *cursor += 8;
            return (AttributeValue::Float(f), None);
        }
    }

    // Check for printable string, but handle single-byte prefix artifacts.
    // Pattern observed: [1 byte prefix] [actual string\0] where the prefix is a
    // binary type/length tag that happens to be printable ASCII.
    if is_printable_run(data, *cursor, 2) {
        if let Some((raw_val, after)) = read_null_terminated_ascii(data, *cursor) {
            if raw_val.len() >= 2 {
                let (stripped, was_stripped) = strip_value_prefix(&raw_val);
                *cursor = after;
                let audit = if was_stripped {
                    Some(raw_val.clone())
                } else {
                    None
                };
                if stripped.is_empty() {
                    return (AttributeValue::Empty, audit);
                }
                if let Ok(n) = stripped.parse::<i64>() {
                    return (AttributeValue::Integer(n), audit);
                }
                if let Ok(f) = stripped.parse::<f64>() {
                    return (AttributeValue::Float(f), audit);
                }
                return (AttributeValue::Text(stripped.to_string()), audit);
            }
        }
    }

    if *cursor + 1 < end && is_printable_run(data, *cursor + 1, 3) {
        *cursor += 1;
        if let Some((val, after)) = read_null_terminated_ascii(data, *cursor) {
            if !val.is_empty() {
                *cursor = after;
                return (AttributeValue::Text(val), None);
            }
        }
    }

    if *cursor + 4 <= end {
        let n = u32_le(data, *cursor) as i64;
        if n < 1_000_000 && n > 0 {
            *cursor += 4;
            return (AttributeValue::Integer(n), None);
        }
    }

    if data[*cursor] == 0 {
        *cursor += 1;
        return (AttributeValue::Empty, None);
    }

    *cursor += 1;
    (AttributeValue::Empty, None)
}

fn is_plausible_double(f: f64, bytes: &[u8]) -> bool {
    if !f.is_finite() || f == 0.0 {
        return false;
    }
    // OLE Automation dates: ~30000 (1982) to ~55000 (2050+)
    if f > 25000.0 && f < 60000.0 {
        return true;
    }
    // Common coordinate range in P&ID drawings
    if f.abs() > 0.001 && f.abs() < 1e8 {
        let exponent = bytes[7] & 0x7F;
        let has_structured_exponent = (0x38..=0x44).contains(&exponent);
        if has_structured_exponent {
            return true;
        }
    }
    false
}

fn read_null_terminated_ascii(data: &[u8], pos: usize) -> Option<(String, usize)> {
    if pos >= data.len() {
        return None;
    }
    let mut end = pos;
    while end < data.len() && data[end] != 0 {
        if !is_printable_byte(data[end]) {
            if end == pos {
                return None;
            }
            break;
        }
        end += 1;
    }
    if end == pos {
        return None;
    }
    let s = String::from_utf8_lossy(&data[pos..end]).to_string();
    let after = if end < data.len() && data[end] == 0 {
        end + 1
    } else {
        end
    };
    Some((s, after))
}

/// Strip a single-byte binary prefix from a value string.
///
/// In .pid DA records, values sometimes have a 1-byte type tag prepended that
/// happens to land in printable ASCII (e.g. 0x32='2' before `"DWG-..."`,
/// 0x31='1' before `"DWG-..."`). Returns `(stripped, was_stripped)`; callers
/// should record the original string as `raw_value` when `was_stripped` is
/// true so a false positive (a legitimate `"1DWG-..."` etc.) stays auditable.
///
/// Guards:
/// - Strings shorter than 3 bytes are never touched.
/// - Hex-GUID-shaped payloads (>=16 hex digits after the candidate prefix) are
///   never stripped, since they're already their own namespace.
pub(crate) fn strip_value_prefix(raw: &str) -> (&str, bool) {
    let bytes = raw.as_bytes();
    if bytes.len() >= 3 {
        let first = bytes[0];
        let second = bytes[1];
        let first_is_prefix = first.is_ascii_digit()
            || (first.is_ascii_punctuation() && first != b'\\' && first != b'/');
        let rest_starts_valid = second.is_ascii_uppercase() || second == b'\\' || second == b'/';
        if first_is_prefix && rest_starts_valid {
            let rest = &raw[1..];
            let looks_like_guid = rest.len() >= 16 && rest.bytes().all(|b| b.is_ascii_hexdigit());
            if !looks_like_guid {
                return (rest, true);
            }
        }
    }
    (raw, false)
}

fn is_printable_byte(b: u8) -> bool {
    (0x20..=0x7e).contains(&b)
}

fn is_printable_run(data: &[u8], pos: usize, min_len: usize) -> bool {
    if pos + min_len > data.len() {
        return false;
    }
    data[pos..pos + min_len]
        .iter()
        .all(|&b| is_printable_byte(b))
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Extract the 31-byte per-record trailer that ends each `P&IDAttributes`
/// record in the Unclustered Dynamic Attributes stream.
///
/// The trailer layout was reverse-engineered from two real-world samples
/// (`DWG-0201GP06-01.pid` and `DWG-0202GP06-01.pid`) and is documented on
/// [`DaRecordTrailer`]. This routine is deliberately conservative: it only
/// returns a trailer when every byte of the expected signature matches
/// (`0x89 0x00` marker, 8 zero padding bytes, `0xFFFF` separator, and the
/// `0x14 0x00 0x00` tail), so false positives are effectively impossible.
///
/// Records without the signature (e.g. malformed or final padding) are
/// simply skipped.
pub fn extract_record_trailers(data: &[u8]) -> Vec<DaRecordTrailer> {
    let starts = find_pidattributes_record_starts(data);
    let mut out = Vec::with_capacity(starts.len());
    for w in 0..starts.len() {
        let start = starts[w];
        let boundary = starts.get(w + 1).copied().unwrap_or(data.len());
        if boundary < start + 31 {
            continue;
        }
        if data[boundary - 3] != 0x14 || data[boundary - 2] != 0x00 || data[boundary - 1] != 0x00 {
            continue;
        }
        let trailer_start = boundary - 31;
        if data[trailer_start] != 0x89 || data[trailer_start + 1] != 0x00 {
            continue;
        }
        if !data[trailer_start + 10..trailer_start + 18]
            .iter()
            .all(|&b| b == 0)
        {
            continue;
        }
        if data[trailer_start + 22] != 0xFF || data[trailer_start + 23] != 0xFF {
            continue;
        }
        let drawing_id = read_drawing_id_before(data, start, trailer_start);
        let class_id = u32_le(data, trailer_start + 24);
        out.push(DaRecordTrailer {
            record_start: start,
            trailer_offset: trailer_start,
            size: u32_le(data, trailer_start + 2),
            record_id: u32_le(data, trailer_start + 6),
            field_x: u32_le(data, trailer_start + 18),
            class_id,
            drawing_id,
            relationship_guid: None,
        });
    }
    // Spatially pair each `Relationship.<GUID>` ASCII tag with the first
    // class_id=0xF6 trailer whose `trailer_offset` falls after the tag,
    // one-to-one. This correctly handles the rare case where the ASCII
    // tag sits inside the previous record's body rather than strictly
    // between `record_start` and `trailer_offset`.
    let rel_tags: Vec<(usize, String)> = collect_relationship_guids(data);
    let mut tag_iter = rel_tags.into_iter().peekable();
    for t in out.iter_mut().filter(|t| t.class_id == 0x0000_00F6) {
        if let Some(&(pos, _)) = tag_iter.peek() {
            if pos < t.trailer_offset {
                let (_, g) = tag_iter.next().unwrap();
                t.relationship_guid = Some(g);
            }
        }
    }
    out
}

/// Scan the stream in byte order for every `Relationship.<GUID>` ASCII
/// occurrence and return `(offset, guid)` tuples. Used by
/// [`extract_record_trailers`] to pair relationship trailers with their
/// textual identifier.
fn collect_relationship_guids(data: &[u8]) -> Vec<(usize, String)> {
    let tag = b"Relationship.";
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + tag.len() + 32 <= data.len() {
        if &data[i..i + tag.len()] != tag {
            i += 1;
            continue;
        }
        let g_start = i + tag.len();
        let g_end = g_start + 32;
        let bytes = &data[g_start..g_end];
        if bytes.iter().all(u8::is_ascii_hexdigit) {
            out.push((i, String::from_utf8_lossy(bytes).to_string()));
            i = g_end;
        } else {
            i += 1;
        }
    }
    out
}

/// Locate the stream offsets where genuine `P&IDAttributes` records start.
///
/// The ASCII class name appears inside attribute values as well (the record
/// references its own class), so we require the preceding byte to be a
/// `0x00` (end of previous field) or `0x01` (class-introducer) and skip
/// near-duplicates within 32 bytes.
pub(crate) fn find_pidattributes_record_starts(data: &[u8]) -> Vec<usize> {
    let class = b"P&IDAttributes";
    let mut out: Vec<usize> = Vec::new();
    for i in 0..data.len().saturating_sub(class.len()) {
        if &data[i..i + class.len()] != class {
            continue;
        }
        if i == 0 {
            out.push(i);
            continue;
        }
        let prev = data[i - 1];
        if matches!(prev, 0x00 | 0x01) && out.last().is_none_or(|&l| i - l > 32) {
            out.push(i);
        }
    }
    out
}

/// Search an attribute record's body for the `DrawingID\0<32hex>` sequence
/// and return the 32-hex drawing identifier if present.
///
/// Used to build the `record_id -> drawing_id` map: for each record trailer
/// we look back to the preceding `DrawingID\0` marker and read the 32-hex
/// id that follows. Returns `None` if the marker is absent or the id bytes
/// aren't valid ASCII hex.
pub(crate) fn read_drawing_id_before(
    data: &[u8],
    record_start: usize,
    trailer_offset: usize,
) -> Option<String> {
    let di_tag = b"DrawingID\0";
    if trailer_offset <= record_start {
        return None;
    }
    let slice = &data[record_start..trailer_offset];
    let pos = find_last(slice, di_tag)?;
    let str_start = record_start + pos + di_tag.len();
    let str_end = str_start + 32;
    if str_end > data.len() {
        return None;
    }
    let bytes = &data[str_start..str_end];
    if !bytes.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    Some(String::from_utf8_lossy(bytes).to_string())
}

fn find_last(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    for i in (0..=hay.len() - needle.len()).rev() {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

/// Phase 12b-1h trace-aware scan for the 31-byte per-record trailers that
/// terminate every `P&IDAttributes` record in the
/// `/Unclustered Dynamic Attributes` stream.
///
/// The trailer signature is tight enough (`0x89 0x00` marker + 8 zero
/// padding bytes + `0xFFFF` separator + `0x14 0x00 0x00` tail; see
/// [`extract_record_trailers`]) to use as a self-contained byte-audit
/// landmark without external context. Every located trailer is consumed
/// as a 31-byte `Decoded` range; the surrounding heuristic record body
/// keeps surfacing as leftover, which is the desired Phase 11a-probe
/// behaviour.
///
/// Returns the number of trailers traced — useful for unit assertions.
pub fn scan_da_record_trailers_with_trace(data: &[u8], trace: &mut ParserTraceBuilder) -> usize {
    let trailers = extract_record_trailers(data);
    let total = data.len() as u64;
    let mut hits = 0usize;
    for t in &trailers {
        let start = t.trailer_offset as u64;
        let end = start + 31;
        if end <= total {
            trace.consume(ByteRange::new(start, end), TraceConfidence::Decoded);
            hits += 1;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_prefix_digit_before_uppercase() {
        let (out, stripped) = strip_value_prefix("1DWG-XYZ");
        assert_eq!(out, "DWG-XYZ");
        assert!(stripped);
    }

    #[test]
    fn strip_prefix_punctuation_before_uppercase() {
        let (out, stripped) = strip_value_prefix("#Name");
        assert_eq!(out, "Name");
        assert!(stripped);
    }

    #[test]
    fn strip_prefix_preserves_backslash_and_slash_first() {
        // Leading \ or / never counts as a prefix byte (paths).
        let (out, stripped) = strip_value_prefix("\\Server\\share");
        assert_eq!(out, "\\Server\\share");
        assert!(!stripped);
        let (out, stripped) = strip_value_prefix("/usr/bin");
        assert_eq!(out, "/usr/bin");
        assert!(!stripped);
    }

    #[test]
    fn strip_prefix_refuses_hex_guid_body() {
        // 32-char hex string is a GUID; leading digit must not be stripped.
        let raw = "0F7B8ABD0C4E493FA3C7F06FD03AD6AA";
        let (out, stripped) = strip_value_prefix(raw);
        assert_eq!(out, raw);
        assert!(!stripped);
    }

    #[test]
    fn strip_prefix_keeps_pure_uppercase_first_byte() {
        // Uppercase first byte is never treated as a tag — keeps real values
        // like "DWG-0201GP06-01" intact.
        let (out, stripped) = strip_value_prefix("DWG-0201GP06-01");
        assert_eq!(out, "DWG-0201GP06-01");
        assert!(!stripped);
    }

    #[test]
    fn strip_prefix_ignores_short_strings() {
        let (out, stripped) = strip_value_prefix("1A");
        assert_eq!(out, "1A");
        assert!(!stripped);
        let (out, stripped) = strip_value_prefix("X");
        assert_eq!(out, "X");
        assert!(!stripped);
    }

    #[test]
    fn strip_prefix_keeps_lowercase_second_byte() {
        // Lowercase second byte means the value probably already starts with
        // a real character — don't strip.
        let (out, stripped) = strip_value_prefix("1name");
        assert_eq!(out, "1name");
        assert!(!stripped);
    }

    #[test]
    fn read_attribute_value_audits_stripped_prefix() {
        // Feed a printable ASCII run that triggers the strip branch.
        // Buffer layout: "1DWG-XY\0" (8 bytes, last is null terminator).
        let data = b"1DWG-XY\0";
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(data, &mut cursor, data.len());
        match value {
            AttributeValue::Text(ref t) => assert_eq!(t, "DWG-XY"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(raw_value.as_deref(), Some("1DWG-XY"));
    }

    #[test]
    fn read_attribute_value_no_audit_when_no_strip() {
        // Uppercase-only value: strip_value_prefix must not fire.
        let data = b"DWG-XY\0\0";
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(data, &mut cursor, data.len());
        match value {
            AttributeValue::Text(ref t) => assert_eq!(t, "DWG-XY"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(
            raw_value.is_none(),
            "raw_value should be None when no stripping occurred, got {raw_value:?}"
        );
    }

    #[test]
    fn read_attribute_value_hex_guid_not_stripped() {
        // 32-char hex GUID: the leading '0' is a digit but must be preserved.
        let raw = b"0F7B8ABD0C4E493FA3C7F06FD03AD6AA\0";
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(raw, &mut cursor, raw.len());
        match value {
            AttributeValue::Text(ref t) => {
                assert_eq!(t, "0F7B8ABD0C4E493FA3C7F06FD03AD6AA");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(
            raw_value.is_none(),
            "GUID body must not be audited as stripped"
        );
    }

    #[test]
    fn read_attribute_value_detects_ole_date_double() {
        // 36526.0 = OLE automation date for 2000-01-01 — squarely inside the
        // 25000..60000 "plausible date" window.
        let bytes = 36526.0f64.to_le_bytes();
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(&bytes, &mut cursor, bytes.len());
        match value {
            AttributeValue::Float(f) => assert!((f - 36526.0).abs() < 1e-9),
            other => panic!("expected Float, got {other:?}"),
        }
        assert!(raw_value.is_none());
        assert_eq!(cursor, 8, "double consumes exactly 8 bytes");
    }

    #[test]
    fn read_attribute_value_rejects_zero_and_nan_as_double() {
        // Zero is explicitly rejected by `is_plausible_double` so it must
        // fall through to one of the non-double branches.
        let zero = 0.0f64.to_le_bytes();
        let mut cursor = 0usize;
        let (value, _) = read_attribute_value(&zero, &mut cursor, zero.len());
        assert!(
            !matches!(value, AttributeValue::Float(_)),
            "0.0 must not be decoded as Float, got {value:?}"
        );
    }

    #[test]
    fn read_attribute_value_short_text_falls_through_double_gate() {
        // 4-char printable + null terminator (5 bytes total) — too short for
        // the 8-byte double probe, must land in the printable-string branch.
        let data = b"Name\0";
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(data, &mut cursor, data.len());
        match value {
            AttributeValue::Text(ref t) => assert_eq!(t, "Name"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(raw_value.is_none());
    }

    #[test]
    fn read_attribute_value_empty_returns_empty() {
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(&[], &mut cursor, 0);
        assert!(matches!(value, AttributeValue::Empty));
        assert!(raw_value.is_none());
    }

    #[test]
    fn read_attribute_value_small_integer_branch() {
        // Non-printable leading byte (0x05), followed by a small u32 — should
        // hit the 4-byte integer branch with value below 1_000_000.
        // Layout: [0x05 0x00 0x00 0x00] = 5 little-endian.
        let data = [0x05u8, 0x00, 0x00, 0x00];
        let mut cursor = 0usize;
        let (value, raw_value) = read_attribute_value(&data, &mut cursor, data.len());
        match value {
            AttributeValue::Integer(n) => assert_eq!(n, 5),
            other => panic!("expected Integer(5), got {other:?}"),
        }
        assert!(raw_value.is_none());
        assert_eq!(cursor, 4);
    }

    #[test]
    fn read_attribute_value_preserves_cursor_on_null_byte() {
        // A single 0x00 byte — the null-terminated empty branch.
        let data = [0u8];
        let mut cursor = 0usize;
        let (value, _) = read_attribute_value(&data, &mut cursor, data.len());
        assert!(matches!(value, AttributeValue::Empty));
        assert_eq!(cursor, 1, "null byte should advance cursor by 1");
    }

    /// Build a synthetic `Unclustered Dynamic Attributes` body that contains
    /// the minimum bytes `extract_record_trailers` needs to recover one
    /// trailer:
    /// - record body opens with `\0P&IDAttributes` (15 bytes; the leading
    ///   NUL satisfies the "previous byte must be 0x00 / 0x01" guard
    ///   in `find_pidattributes_record_starts`)
    /// - 5 bytes of opaque body
    /// - 31-byte trailer matching the validated signature
    fn make_synthetic_da_body_with_one_trailer() -> Vec<u8> {
        let mut data = Vec::new();
        data.push(0x00); // start with 0x00 so PIDAttributes guard fires
        data.extend_from_slice(b"P&IDAttributes");
        data.extend_from_slice(&[0xAB; 5]); // opaque body
        let trailer_start = data.len();

        // Trailer (31 bytes):
        //   +0..2   0x89 0x00
        //   +2..6   size (u32 LE)
        //   +6..10  record_id (u32 LE)
        //   +10..18 8 zero pad bytes
        //   +18..22 field_x (u32 LE)
        //   +22..24 0xFF 0xFF
        //   +24..28 class_id (u32 LE)
        //   +28..31 0x14 0x00 0x00
        let mut trailer = Vec::with_capacity(31);
        trailer.extend_from_slice(&[0x89, 0x00]); // marker
        trailer.extend_from_slice(&100u32.to_le_bytes()); // size
        trailer.extend_from_slice(&7u32.to_le_bytes()); // record_id
        trailer.extend_from_slice(&[0u8; 8]); // padding
        trailer.extend_from_slice(&0x0000_03B7u32.to_le_bytes()); // field_x
        trailer.extend_from_slice(&[0xFF, 0xFF]); // separator
        trailer.extend_from_slice(&0x0000_00EAu32.to_le_bytes()); // class_id (drawing)
        trailer.extend_from_slice(&[0x14, 0x00, 0x00]); // tail
        assert_eq!(trailer.len(), 31);
        data.extend_from_slice(&trailer);
        // The current `extract_record_trailers` implementation derives the
        // trailer's end position from the next record start (or
        // `data.len()` for the last record); the trailer signature check
        // walks back 31 bytes from that boundary. So no further padding
        // is required — `data.len()` already aligns with the trailer end.
        let _ = trailer_start;
        data
    }

    #[test]
    fn trace_aware_da_trailer_scan_consumes_31_bytes_per_trailer() {
        let data = make_synthetic_da_body_with_one_trailer();

        // Sanity-check the synthetic fixture: extract_record_trailers
        // should find exactly one trailer.
        let trailers = extract_record_trailers(&data);
        assert_eq!(trailers.len(), 1, "fixture must yield exactly one trailer");

        let mut trace = ParserTraceBuilder::new("scan_da_record_trailers");
        let hits = scan_da_record_trailers_with_trace(&data, &mut trace);
        assert_eq!(hits, 1);

        let trace = trace.build("/Unclustered Dynamic Attributes", data.len() as u64);
        // Trailer = 31 bytes, decoded.
        let decoded = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].len(), 31);
        assert_eq!(trace.consumed_bytes(), 31);
        assert_eq!(trace.leftover_bytes(), data.len() as u64 - 31);
    }

    #[test]
    fn trace_aware_da_trailer_scan_returns_zero_when_no_trailers_present() {
        // Garbage data — no `P&IDAttributes` markers, no trailer signatures.
        let data = vec![0xCD; 64];
        let mut trace = ParserTraceBuilder::new("scan_da_record_trailers");
        let hits = scan_da_record_trailers_with_trace(&data, &mut trace);
        assert_eq!(hits, 0);
        let trace = trace.build("/Unclustered Dynamic Attributes", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
        assert_eq!(trace.leftover_bytes(), data.len() as u64);
    }
}
