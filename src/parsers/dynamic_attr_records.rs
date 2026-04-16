use crate::model::{AttributeField, AttributeRecord, AttributeValue, ProbeSummary};

/// Parse structured attribute records from the Unclustered Dynamic Attributes stream body.
///
/// The stream follows the pattern (observed from hex analysis):
///   - Stream header: 16+ bytes (magic + meta), followed by zero padding
///   - Section body starts after padding with: u32 count, then 0xFFFF marker
///   - Each section: u32 section_len, then interleaved class name + attribute fields
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
        let (attr_name, after_name) = match read_null_terminated_ascii(data, cursor) {
            Some(r) => r,
            None => break,
        };
        cursor = after_name;

        if attr_name.is_empty() {
            continue;
        }

        // Read attribute value
        let value = read_attribute_value(data, &mut cursor, section_end);

        attributes.push(AttributeField {
            name: attr_name,
            value,
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

fn read_attribute_value(data: &[u8], cursor: &mut usize, end: usize) -> AttributeValue {
    if *cursor >= end {
        return AttributeValue::Empty;
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
            return AttributeValue::Float(f);
        }
    }

    // Check for printable string, but handle single-byte prefix artifacts.
    // Pattern observed: [1 byte prefix] [actual string\0] where the prefix is a
    // binary type/length tag that happens to be printable ASCII.
    if is_printable_run(data, *cursor, 2) {
        if let Some((raw_val, after)) = read_null_terminated_ascii(data, *cursor) {
            if raw_val.len() >= 2 {
                // Detect prefix byte: a single digit/hex char followed by an uppercase-starting string
                let val = strip_value_prefix(&raw_val);
                *cursor = after;
                if val.is_empty() {
                    return AttributeValue::Empty;
                }
                if let Ok(n) = val.parse::<i64>() {
                    return AttributeValue::Integer(n);
                }
                if let Ok(f) = val.parse::<f64>() {
                    return AttributeValue::Float(f);
                }
                return AttributeValue::Text(val.to_string());
            }
        }
    }

    // Single non-printable or single printable byte followed by a string
    if *cursor + 1 < end && is_printable_run(data, *cursor + 1, 3) {
        *cursor += 1;
        if let Some((val, after)) = read_null_terminated_ascii(data, *cursor) {
            if !val.is_empty() {
                *cursor = after;
                return AttributeValue::Text(val);
            }
        }
    }

    // Check for 4-byte integer
    if *cursor + 4 <= end {
        let n = u32_le(data, *cursor) as i64;
        if n < 1_000_000 && n > 0 {
            *cursor += 4;
            return AttributeValue::Integer(n);
        }
    }

    // Null-terminated empty string
    if data[*cursor] == 0 {
        *cursor += 1;
        return AttributeValue::Empty;
    }

    *cursor += 1;
    AttributeValue::Empty
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
/// In .pid DA records, values sometimes have a 1-byte type tag prepended that falls
/// in the printable ASCII range (e.g., 0x32='2' before "DWG-...", 0x31='1' before "DWG-...").
fn strip_value_prefix(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() >= 3 {
        let first = bytes[0];
        let second = bytes[1];
        let first_is_prefix = first.is_ascii_digit()
            || (first.is_ascii_punctuation() && first != b'\\' && first != b'/');
        let rest_starts_valid = second.is_ascii_uppercase()
            || second == b'\\'
            || second == b'/';
        if first_is_prefix && rest_starts_valid {
            // Don't strip if the whole string looks like a hex GUID (all hex + no spaces)
            let rest = &raw[1..];
            let looks_like_guid = rest.len() >= 16
                && rest.bytes().all(|b| b.is_ascii_hexdigit());
            if !looks_like_guid {
                return rest;
            }
        }
    }
    raw
}

fn is_printable_byte(b: u8) -> bool {
    (0x20..=0x7e).contains(&b)
}

fn is_printable_run(data: &[u8], pos: usize, min_len: usize) -> bool {
    if pos + min_len > data.len() {
        return false;
    }
    data[pos..pos + min_len].iter().all(|&b| is_printable_byte(b))
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
