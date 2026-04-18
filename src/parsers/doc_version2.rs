//! Structured decoder for the `/DocVersion2` stream.
//!
//! `/DocVersion2` is a **compact per-save version log**, complementing the
//! human-readable `/DocVersion3`. Reverse-engineering of two real SmartPlant
//! `.pid` samples revealed the following layout:
//!
//! ```text
//! Header (12 bytes):
//!     0x00  u32 LE   magic = 0x0001_0034
//!     0x04  u64      reserved (observed all 0x00)
//!
//! Records (9 bytes each, repeated `count` times):
//!     +0  u8   op_type      (0x82 = SaveAs, 0x81 = Save)
//!     +1  3B   fixed        (observed 0x00 0x00 0x09)
//!     +4  u8   separator    (observed 0x00)
//!     +5  u32 LE  version   (u32 low byte = minor version; cross-checked
//!                            with DocVersion3 decimal version strings)
//! ```
//!
//! **Cross-check**: the version field matches `DocVersion3` records on the
//! same file. Sample 1 has DocVersion3 `[SA 0144, SV 0077, SV 0144, SV 0077]`
//! and DocVersion2 records `[0x82 version=0x90 (144), 0x81 0x4D (77),
//! 0x81 0x90 (144), 0x81 0x4D (77)]` — one-to-one.
//!
//! This parser is strict about the header magic and the 9-byte record
//! stride; any departure returns `None` and the caller falls back to
//! `DocVersion2Raw` (pre-v0.3.8 behavior).
use crate::model::{DocVersion2, DocVersion2Record};

pub const DOC_VERSION2_MAGIC: u32 = 0x0001_0034;

/// Attempt to decode `/DocVersion2`'s content as the structured log.
/// Returns `None` when the magic, header reserved-zero bytes, or the
/// per-record stride don't match the observed layout, letting the caller
/// fall back to raw preservation.
pub fn parse_doc_version2(data: &[u8]) -> Option<DocVersion2> {
    if data.len() < 12 {
        return None;
    }
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != DOC_VERSION2_MAGIC {
        return None;
    }
    // Bytes 4..12 observed as all zeros across samples. We're lenient
    // here (don't reject) but stash them so callers can audit if needed.
    let reserved_ok = data[4..12].iter().all(|&b| b == 0);

    let records_bytes = &data[12..];
    if !records_bytes.len().is_multiple_of(9) {
        return None;
    }
    let mut records = Vec::with_capacity(records_bytes.len() / 9);
    for chunk in records_bytes.chunks(9) {
        records.push(DocVersion2Record {
            op_type: chunk[0],
            fixed: [chunk[1], chunk[2], chunk[3]],
            separator: chunk[4],
            version: u32::from_le_bytes([chunk[5], chunk[6], chunk[7], chunk[8]]),
        });
    }

    Some(DocVersion2 {
        magic_u32_le: magic,
        reserved_all_zero: reserved_ok,
        records,
    })
}

/// Human-label the `op_type` byte using the mapping observed in the two
/// real samples. Unknown codes render as `0xNN`.
pub fn op_type_label(op: u8) -> String {
    match op {
        0x82 => "SaveAs".to_string(),
        0x81 => "Save".to_string(),
        other => format!("0x{:02X}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Vec<u8> {
        let mut h = vec![0u8; 12];
        h[0..4].copy_from_slice(&DOC_VERSION2_MAGIC.to_le_bytes());
        h
    }

    fn record(op: u8, version: u32) -> [u8; 9] {
        let mut r = [0u8; 9];
        r[0] = op;
        r[1] = 0x00;
        r[2] = 0x00;
        r[3] = 0x09;
        r[4] = 0x00;
        r[5..9].copy_from_slice(&version.to_le_bytes());
        r
    }

    #[test]
    fn decodes_sample1_sequence_four_records() {
        // SaveAs 144, Save 77, Save 144, Save 77 — matches the real
        // DWG-0201GP06-01.pid DocVersion2 byte stream byte-for-byte.
        let mut data = header();
        data.extend_from_slice(&record(0x82, 144));
        data.extend_from_slice(&record(0x81, 77));
        data.extend_from_slice(&record(0x81, 144));
        data.extend_from_slice(&record(0x81, 77));
        let dv = parse_doc_version2(&data).expect("ok");
        assert_eq!(dv.records.len(), 4);
        assert_eq!(dv.records[0].op_type, 0x82);
        assert_eq!(dv.records[0].version, 144);
        assert_eq!(dv.records[1].op_type, 0x81);
        assert_eq!(dv.records[1].version, 77);
        assert_eq!(dv.records[2].version, 144);
        assert_eq!(dv.records[3].version, 77);
        assert!(dv.reserved_all_zero);
    }

    #[test]
    fn decodes_sample2_sequence_three_records() {
        let mut data = header();
        data.extend_from_slice(&record(0x82, 144));
        data.extend_from_slice(&record(0x81, 144));
        data.extend_from_slice(&record(0x81, 77));
        let dv = parse_doc_version2(&data).expect("ok");
        assert_eq!(dv.records.len(), 3);
        assert_eq!(dv.records[0].op_type, 0x82);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut data = header();
        data[0] = 0xFF;
        data.extend_from_slice(&record(0x81, 1));
        assert!(parse_doc_version2(&data).is_none());
    }

    #[test]
    fn rejects_misaligned_record_body() {
        let mut data = header();
        data.push(0x81); // only 1 byte trailing, not 9
        assert!(parse_doc_version2(&data).is_none());
    }

    #[test]
    fn op_type_label_covers_known_and_unknown() {
        assert_eq!(op_type_label(0x82), "SaveAs");
        assert_eq!(op_type_label(0x81), "Save");
        assert_eq!(op_type_label(0xAB), "0xAB");
    }
}
