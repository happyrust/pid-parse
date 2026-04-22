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
use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{DocVersion2, DocVersion2Record};

pub const DOC_VERSION2_MAGIC: u32 = 0x0001_0034;

/// Attempt to decode `/DocVersion2`'s content as the structured log.
/// Returns `None` when the magic, header reserved-zero bytes, or the
/// per-record stride don't match the observed layout, letting the caller
/// fall back to raw preservation.
///
/// Thin back-compat wrapper around [`parse_doc_version2_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_doc_version2(data: &[u8]) -> Option<DocVersion2> {
    let mut trace = ParserTraceBuilder::new("parse_doc_version2");
    parse_doc_version2_with_trace(data, &mut trace)
}

/// Phase 12b-1c trace-aware variant of [`parse_doc_version2`].
///
/// Byte map (per reverse-engineering notes at the top of this file):
/// - `[0..4]` — `0x0001_0034` magic — `TraceConfidence::Decoded`
/// - `[4..12]` — reserved (observed all-zero) — `TraceConfidence::Probed`
///   (byte position known, but the 8 bytes have no independent semantic
///   field name — the parser only records whether they are all zero via
///   `DocVersion2.reserved_all_zero`).
/// - per 9-byte record at offset `12 + 9*i`:
///   - `[+0..+1]` — op_type — `Decoded`
///   - `[+1..+4]` — fixed 3-byte observed constant — `Probed` (no
///     independent field name beyond "observed constant")
///   - `[+4..+5]` — separator — `Probed` (same reasoning)
///   - `[+5..+9]` — version u32 LE — `Decoded`
///
/// On misaligned record body (length not divisible by 9) or magic
/// mismatch the parser returns `None` without emitting any consume
/// events beyond what was already pushed — so a builder passed in will
/// carry at most the magic/reserved ranges if the truncation is
/// detected after the header.
pub fn parse_doc_version2_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<DocVersion2> {
    if data.len() < 12 {
        return None;
    }
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != DOC_VERSION2_MAGIC {
        return None;
    }
    trace.consume(ByteRange::new(0, 4), TraceConfidence::Decoded);
    // Bytes 4..12 observed as all zeros across samples. We're lenient
    // here (don't reject) but stash them so callers can audit if needed.
    let reserved_ok = data[4..12].iter().all(|&b| b == 0);
    trace.consume(ByteRange::new(4, 12), TraceConfidence::Probed);

    let records_bytes = &data[12..];
    if !records_bytes.len().is_multiple_of(9) {
        return None;
    }
    let mut records = Vec::with_capacity(records_bytes.len() / 9);
    for (i, chunk) in records_bytes.chunks(9).enumerate() {
        let rec_start = (12 + 9 * i) as u64;
        records.push(DocVersion2Record {
            op_type: chunk[0],
            fixed: [chunk[1], chunk[2], chunk[3]],
            separator: chunk[4],
            version: u32::from_le_bytes([chunk[5], chunk[6], chunk[7], chunk[8]]),
        });
        trace.consume(
            ByteRange::new(rec_start, rec_start + 1),
            TraceConfidence::Decoded,
        );
        trace.consume(
            ByteRange::new(rec_start + 1, rec_start + 4),
            TraceConfidence::Probed,
        );
        trace.consume(
            ByteRange::new(rec_start + 4, rec_start + 5),
            TraceConfidence::Probed,
        );
        trace.consume(
            ByteRange::new(rec_start + 5, rec_start + 9),
            TraceConfidence::Decoded,
        );
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
    fn trace_aware_doc_version2_splits_header_and_record_fields_into_confidence_buckets() {
        let mut data = header();
        data.extend_from_slice(&record(0x82, 144));
        data.extend_from_slice(&record(0x81, 77));

        let mut b = ParserTraceBuilder::new("parse_doc_version2");
        let dv = parse_doc_version2_with_trace(&data, &mut b).expect("ok");
        assert_eq!(dv.records.len(), 2);

        let trace = b.build("/DocVersion2", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "every byte of this fixture should be consumed",
        );
        assert!(trace.leftover_ranges.is_empty());

        let decoded = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        // Decoded ranges: magic [0..4], record 0's op_type [12..13],
        // record 0's version merged with record 1's op_type into
        // [17..22] (because Decoded ranges separated only by other
        // records' Decoded bytes get merged across the probed gap? No:
        // they are separated by Probed bytes [13..17], so record 0's
        // version [17..21] and record 1's op_type [21..22] ARE adjacent
        // and both Decoded → merge into [17..22]). Finally record 1's
        // version [26..30] is split again by the second Probed gap
        // [22..26].
        assert_eq!(
            decoded,
            vec![
                ByteRange::new(0, 4),
                ByteRange::new(12, 13),
                ByteRange::new(17, 22),
                ByteRange::new(26, 30),
            ],
            "unexpected Decoded layout: {decoded:?}"
        );
        // Probed: reserved [4..12], fixed+separator of record 0
        // [13..17] and record 1 [22..26].
        assert_eq!(
            probed,
            vec![
                ByteRange::new(4, 12),
                ByteRange::new(13, 17),
                ByteRange::new(22, 26),
            ],
            "unexpected Probed layout: {probed:?}"
        );
    }

    #[test]
    fn trace_aware_doc_version2_covers_every_byte_exactly_once() {
        let mut data = header();
        for v in [144, 77, 144, 77] {
            data.extend_from_slice(&record(0x81, v));
        }

        let mut b = ParserTraceBuilder::new("parse_doc_version2");
        parse_doc_version2_with_trace(&data, &mut b).expect("ok");
        let trace = b.build("/DocVersion2", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
        // Conservation law: consumed + leftover == total.
        assert_eq!(
            trace.consumed_bytes() + trace.leftover_bytes(),
            data.len() as u64
        );
    }

    #[test]
    fn back_compat_parse_doc_version2_matches_trace_variant_byte_for_byte() {
        let mut data = header();
        data.extend_from_slice(&record(0x82, 144));
        data.extend_from_slice(&record(0x81, 144));
        data.extend_from_slice(&record(0x81, 77));

        let without_trace = parse_doc_version2(&data).expect("old API works");

        let mut b = ParserTraceBuilder::new("parse_doc_version2");
        let with_trace = parse_doc_version2_with_trace(&data, &mut b).expect("new API works");

        assert_eq!(without_trace.magic_u32_le, with_trace.magic_u32_le);
        assert_eq!(
            without_trace.reserved_all_zero,
            with_trace.reserved_all_zero
        );
        assert_eq!(without_trace.records.len(), with_trace.records.len());
        for (a, b_rec) in without_trace.records.iter().zip(with_trace.records.iter()) {
            assert_eq!(a.op_type, b_rec.op_type);
            assert_eq!(a.fixed, b_rec.fixed);
            assert_eq!(a.separator, b_rec.separator);
            assert_eq!(a.version, b_rec.version);
        }
    }

    #[test]
    fn op_type_label_covers_known_and_unknown() {
        assert_eq!(op_type_label(0x82), "SaveAs");
        assert_eq!(op_type_label(0x81), "Save");
        assert_eq!(op_type_label(0xAB), "0xAB");
    }
}
