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

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{VersionHistory, VersionRecord};

pub const RECORD_SIZE: usize = 48;

/// Parse `DocVersion3`. Returns `None` if no records can be recovered.
///
/// Thin back-compat wrapper around [`parse_doc_version3_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_doc_version3(data: &[u8]) -> Option<VersionHistory> {
    let mut trace = ParserTraceBuilder::new("parse_doc_version3");
    parse_doc_version3_with_trace(data, &mut trace)
}

/// Phase 12b-1c trace-aware variant of [`parse_doc_version3`].
///
/// Every 48-byte record is split into its four named sub-fields
/// (`product[16]`, `version[12]`, `operation[4]`, `timestamp[16]`) and
/// each sub-field is consumed as `TraceConfidence::Decoded`. The
/// builder will merge them into a single contiguous range per record
/// because they are adjacent same-confidence bytes — that's fine; the
/// merging keeps `consumed_ranges` compact while `ranges_by_confidence`
/// still records every byte as Decoded.
///
/// Records that the parser rejects (non-printable leading byte or
/// empty `product` field) cause the loop to break without consuming
/// those 48 bytes — they surface as leftover instead, which is
/// deliberately how byte-audit flags "bytes we saw but declined to
/// interpret".
pub fn parse_doc_version3_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<VersionHistory> {
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
        let p = pos as u64;
        trace.consume(ByteRange::new(p, p + 16), TraceConfidence::Decoded);
        trace.consume(ByteRange::new(p + 16, p + 28), TraceConfidence::Decoded);
        trace.consume(ByteRange::new(p + 28, p + 32), TraceConfidence::Decoded);
        trace.consume(ByteRange::new(p + 32, p + 48), TraceConfidence::Decoded);
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
    fn trace_aware_doc_version3_consumes_every_byte_of_each_record() {
        let mut data = Vec::new();
        for ts in ["12/29/25 10:45", "03/10/26 15:17"] {
            data.extend(fixed_field("SmartPlantPID.a", 16));
            data.extend(fixed_field("090000.0144", 12));
            data.extend(fixed_field("SV", 4));
            data.extend(fixed_field(ts, 16));
        }

        let mut b = ParserTraceBuilder::new("parse_doc_version3");
        let h = parse_doc_version3_with_trace(&data, &mut b).expect("valid");
        assert_eq!(h.records.len(), 2);

        let trace = b.build("/DocVersion3", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
        assert!(trace.leftover_ranges.is_empty());
        // Builder merges the 4 sub-field ranges per record into one
        // contiguous Decoded range, and then merges consecutive records
        // because they are also adjacent same-confidence → single range.
        let decoded = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        assert_eq!(decoded, vec![ByteRange::new(0, data.len() as u64)]);
    }

    #[test]
    fn trace_aware_doc_version3_leaves_rejected_records_as_leftover() {
        let mut data = Vec::new();
        // One valid record, then a rejected one (leading 0xFF).
        data.extend(fixed_field("SmartPlantPID.a", 16));
        data.extend(fixed_field("090000.0144", 12));
        data.extend(fixed_field("SA", 4));
        data.extend(fixed_field("12/29/25 10:45", 16));
        data.extend(vec![0xFF; RECORD_SIZE]);

        let mut b = ParserTraceBuilder::new("parse_doc_version3");
        let h = parse_doc_version3_with_trace(&data, &mut b).expect("valid");
        assert_eq!(h.records.len(), 1);
        // Legacy trailing_bytes counts every byte past the last accepted
        // record, so 48 here.
        assert_eq!(h.trailing_bytes, 48);

        let trace = b.build("/DocVersion3", data.len() as u64);
        // The 48-byte rejected record surfaces in leftover because the
        // parser never consumed it.
        assert_eq!(trace.consumed_bytes(), RECORD_SIZE as u64);
        assert_eq!(trace.leftover_bytes(), RECORD_SIZE as u64);
        assert_eq!(
            trace.leftover_ranges,
            vec![ByteRange::new(
                RECORD_SIZE as u64,
                (RECORD_SIZE * 2) as u64
            )],
        );
    }

    #[test]
    fn back_compat_parse_doc_version3_matches_trace_variant_byte_for_byte() {
        let mut data = Vec::new();
        for ts in ["12/29/25 10:45", "03/10/26 15:17", "03/16/26 11:24"] {
            data.extend(fixed_field("SmartPlantPID.a", 16));
            data.extend(fixed_field("090000.0144", 12));
            data.extend(fixed_field("SV", 4));
            data.extend(fixed_field(ts, 16));
        }
        data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // trailing

        let without_trace = parse_doc_version3(&data).expect("old API works");

        let mut b = ParserTraceBuilder::new("parse_doc_version3");
        let with_trace = parse_doc_version3_with_trace(&data, &mut b).expect("new API works");

        assert_eq!(without_trace.size, with_trace.size);
        assert_eq!(without_trace.record_size, with_trace.record_size);
        assert_eq!(without_trace.trailing_bytes, with_trace.trailing_bytes);
        assert_eq!(without_trace.records.len(), with_trace.records.len());
        for (a, b_rec) in without_trace.records.iter().zip(with_trace.records.iter()) {
            assert_eq!(a.product, b_rec.product);
            assert_eq!(a.version, b_rec.version);
            assert_eq!(a.operation, b_rec.operation);
            assert_eq!(a.timestamp, b_rec.timestamp);
            assert_eq!(a.offset, b_rec.offset);
        }
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
