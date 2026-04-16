use crate::model::{AttributeField, AttributeRecord, AttributeValue};

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
pub fn parse_attribute_records(data: &[u8]) -> Vec<AttributeRecord> {
    let mut records = Vec::new();
    let mut pos = find_body_start(data);

    while pos < data.len() {
        if let Some((rec, next_pos)) = try_parse_record(data, pos) {
            records.push(rec);
            pos = next_pos;
        } else {
            pos += 1;
        }
    }

    records
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
        },
        section_end,
    ))
}

fn read_attribute_value(data: &[u8], cursor: &mut usize, end: usize) -> AttributeValue {
    if *cursor >= end {
        return AttributeValue::Empty;
    }

    // Check if the next content is a printable string
    if is_printable_run(data, *cursor, 1) {
        if let Some((val, after)) = read_null_terminated_ascii(data, *cursor) {
            *cursor = after;
            if val.is_empty() {
                return AttributeValue::Empty;
            }
            // Try to parse as number
            if let Ok(n) = val.parse::<i64>() {
                return AttributeValue::Integer(n);
            }
            if let Ok(f) = val.parse::<f64>() {
                return AttributeValue::Float(f);
            }
            return AttributeValue::Text(val);
        }
    }

    // Check for 8-byte double
    if *cursor + 8 <= end {
        let bytes = &data[*cursor..*cursor + 8];
        let f = f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        if f.is_finite() && f.abs() < 1e15 && f.abs() > 1e-15 {
            *cursor += 8;
            return AttributeValue::Float(f);
        }
    }

    // Check for 4-byte integer
    if *cursor + 4 <= end {
        let n = u32_le(data, *cursor) as i64;
        if n < 1_000_000 {
            *cursor += 4;
            return AttributeValue::Integer(n);
        }
    }

    // Skip to next recognizable content
    *cursor += 1;
    AttributeValue::Empty
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
