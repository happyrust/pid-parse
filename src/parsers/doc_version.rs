//! Parser for the `DocVersion3` stream (SmartPlant P&ID version history log).
//!
//! The stream begins with the ASCII magic `"SmartPlantPID.a"` (same as the
//! first record's product field) and is a plain array of fixed-size 48-byte
//! records:
//!
//! ```text
//! +00..+0F  product name, zero-padded ASCII (16 bytes, e.g. "SmartPlantPID.a\0")
//! +10..+1B  version string, zero-padded ASCII (12 bytes, e.g. "090000.0144\0")
//! +1C..+1F  operation code ASCII (4 bytes, e.g. "SA\0\0" or "SV\0\0")
//! +20..+2F  timestamp ASCII (16 bytes, e.g. "12/29/25 10:45\0\0")
//! ```
//!
//! The parser is tolerant: it stops on the first record that does not start
//! with a printable ASCII byte.

use crate::model::{VersionHistory, VersionRecord};

pub const RECORD_SIZE: usize = 48;

/// Parse `DocVersion3`. Returns `None` if no records can be recovered.
pub fn parse_doc_version3(data: &[u8]) -> Option<VersionHistory> {
    if data.len() < RECORD_SIZE {
        return None;
    }
    let mut records = Vec::new();
    let mut pos = 0usize;
    while pos + RECORD_SIZE <= data.len() {
        let chunk = &data[pos..pos + RECORD_SIZE];
        let first = chunk[0];
        if !(0x20..=0x7e).contains(&first) {
            break;
        }
        let product = zero_terminated_ascii(&chunk[0..16]);
        let version = zero_terminated_ascii(&chunk[16..28]);
        let operation = zero_terminated_ascii(&chunk[28..32]);
        let timestamp = zero_terminated_ascii(&chunk[32..48]);
        if product.trim().is_empty() {
            break;
        }
        records.push(VersionRecord {
            product,
            version,
            operation,
            timestamp,
            offset: pos,
        });
        pos += RECORD_SIZE;
    }
    if records.is_empty() {
        None
    } else {
        Some(VersionHistory {
            size: data.len() as u64,
            record_size: RECORD_SIZE,
            trailing_bytes: data.len().saturating_sub(pos),
            records,
        })
    }
}

/// Extract the printable-ASCII prefix of a fixed-size field, stopping at the
/// first null byte or non-printable character.
fn zero_terminated_ascii(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == 0 || !(0x20..=0x7e).contains(&b))
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_field(value: &str, size: usize) -> Vec<u8> {
        let mut v = value.as_bytes().to_vec();
        v.resize(size, 0);
        v
    }

    #[test]
    fn single_record() {
        let mut data = Vec::new();
        data.extend(fixed_field("SmartPlantPID.a", 16));
        data.extend(fixed_field("090000.0144", 12));
        data.extend(fixed_field("SA", 4));
        data.extend(fixed_field("12/29/25 10:45", 16));
        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.records.len(), 1);
        assert_eq!(h.records[0].product, "SmartPlantPID.a");
        assert_eq!(h.records[0].version, "090000.0144");
        assert_eq!(h.records[0].operation, "SA");
        assert_eq!(h.records[0].timestamp, "12/29/25 10:45");
    }

    #[test]
    fn multiple_records() {
        let mut data = Vec::new();
        for ts in ["12/29/25 10:45", "03/10/26 15:17", "03/16/26 11:24"] {
            data.extend(fixed_field("SmartPlantPID.a", 16));
            data.extend(fixed_field("090000.0144", 12));
            data.extend(fixed_field("SV", 4));
            data.extend(fixed_field(ts, 16));
        }
        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.records.len(), 3);
        assert_eq!(h.records[2].timestamp, "03/16/26 11:24");
    }

    #[test]
    fn empty_returns_none() {
        assert!(parse_doc_version3(&[]).is_none());
        assert!(parse_doc_version3(&[0u8; 10]).is_none());
    }

    #[test]
    fn stops_on_non_ascii_start() {
        let mut data = Vec::new();
        data.extend(fixed_field("SmartPlantPID.a", 16));
        data.extend(fixed_field("090000.0144", 12));
        data.extend(fixed_field("SA", 4));
        data.extend(fixed_field("12/29/25 10:45", 16));
        // Second record starts with a non-printable byte (0xFF)
        data.extend(vec![0xFF; RECORD_SIZE]);
        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.records.len(), 1);
    }

    #[test]
    fn doc_version3_records_expose_record_offset_and_trailing_bytes() {
        let mut data = Vec::new();
        for _ in 0..3 {
            data.extend(fixed_field("SmartPlantPID.a", 16));
            data.extend(fixed_field("090000.0144", 12));
            data.extend(fixed_field("SV", 4));
            data.extend(fixed_field("01/01/26 00:00", 16));
        }
        data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.records.len(), 3);
        assert_eq!(h.record_size, 48);
        assert_eq!(h.trailing_bytes, 4);
        assert_eq!(h.records[0].offset, 0);
        assert_eq!(h.records[1].offset, 48);
        assert_eq!(h.records[2].offset, 96);
    }

    #[test]
    fn doc_version3_rejects_record_with_empty_product() {
        let mut data = Vec::new();
        data.extend(fixed_field("SmartPlantPID.a", 16));
        data.extend(fixed_field("090000.0144", 12));
        data.extend(fixed_field("SA", 4));
        data.extend(fixed_field("12/29/25 10:45", 16));
        // Second record: product starts with space (0x20) but is otherwise empty
        let mut bad = vec![0x20u8];
        bad.resize(RECORD_SIZE, 0);
        data.extend(bad);
        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.records.len(), 1, "empty product stops parsing");
    }

    #[test]
    fn doc_version3_zero_trailing_when_exact_fit() {
        let mut data = Vec::new();
        data.extend(fixed_field("SmartPlantPID.a", 16));
        data.extend(fixed_field("090000.0144", 12));
        data.extend(fixed_field("SA", 4));
        data.extend(fixed_field("12/29/25 10:45", 16));
        let h = parse_doc_version3(&data).expect("valid");
        assert_eq!(h.trailing_bytes, 0);
    }
}
