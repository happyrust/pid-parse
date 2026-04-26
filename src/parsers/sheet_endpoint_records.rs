//! Decode relationship endpoint-pair records from Sheet streams.
//!
//! Background
//! ----------
//! Each P&IDAttributes `Relationship.<GUID>` record in
//! `/Unclustered Dynamic Attributes` carries a `field_x` identifier in its
//! trailer (see [`crate::model::DaRecordTrailer`]). For every relationship,
//! Sheet streams (`/Sheet*`) contain a 28-byte header record that lists the
//! two endpoints as a pair of `field_x` values. Resolving those back to the
//! owning object's `drawing_id` yields the relationship's source/target.
//!
//! Record signature (first 26 bytes):
//!
//! ```text
//!   +0   u32   rel_field_x      ← matches the relationship's DA field_x
//!   +4   u32   0x00000006       ← constant (discriminator)
//!   +8   [u8;6] zero padding
//!   +14  u16   type = 0x0002    ← endpoint-record marker
//!   +16  u32   endpoint_a       ← first endpoint's field_x
//!   +20  u16   0x0001           ← delimiter
//!   +22  u32   endpoint_b       ← second endpoint's field_x
//! ```
//!
//! The signature is tight enough that false positives are effectively
//! impossible when the `rel_field_x` is drawn from a known relationship
//! list (the caller must supply the valid set via `rel_field_xs`).

use std::collections::HashSet;

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::SheetEndpointRecord;

/// Constant `u32` at offset `+4` that marks an endpoint record.
const DISCRIMINATOR: u32 = 0x0000_0006;
/// `u16` type tag at offset `+16` that marks an endpoint record.
const ENDPOINT_TYPE_TAG: u16 = 0x0002;
/// `u16` delimiter between the two endpoint `field_x` values.
const ENDPOINT_DELIMITER: u16 = 0x0001;

/// Parse all endpoint-pair records from a single Sheet stream body.
///
/// `rel_field_xs` is the set of relationship `field_x` values extracted
/// from the DA trailers. Supplying this set keeps the parser strictly
/// focused on the records we care about and immune to coincidental byte
/// patterns elsewhere in the sheet.
pub fn parse_endpoint_records(
    sheet_path: &str,
    data: &[u8],
    rel_field_xs: &HashSet<u32>,
) -> Vec<SheetEndpointRecord> {
    let mut out = Vec::new();
    if data.len() < 26 {
        return out;
    }
    let end = data.len() - 26;
    let mut i = 0usize;
    while i <= end {
        let field_x = u32_le(data, i);
        if !rel_field_xs.contains(&field_x) {
            i += 1;
            continue;
        }
        if u32_le(data, i + 4) != DISCRIMINATOR {
            i += 1;
            continue;
        }
        if !data[i + 8..i + 14].iter().all(|&b| b == 0) {
            i += 1;
            continue;
        }
        if u16_le(data, i + 14) != ENDPOINT_TYPE_TAG {
            i += 1;
            continue;
        }
        if u16_le(data, i + 20) != ENDPOINT_DELIMITER {
            i += 1;
            continue;
        }
        let endpoint_a = u32_le(data, i + 16);
        let endpoint_b = u32_le(data, i + 22);
        out.push(SheetEndpointRecord {
            sheet_path: sheet_path.to_string(),
            offset: i,
            rel_field_x: field_x,
            endpoint_a,
            endpoint_b,
        });
        // A real endpoint record occupies at least 26 bytes, skip past it.
        i += 26;
    }
    out
}

fn u32_le(data: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([data[p], data[p + 1], data[p + 2], data[p + 3]])
}

fn u16_le(data: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([data[p], data[p + 1]])
}

/// Phase 12b-1g self-contained byte-audit scan for endpoint records.
///
/// Unlike [`parse_endpoint_records`] this variant does **not** require
/// the caller to supply the `rel_field_xs` set — the 14 fixed bytes of
/// the 26-byte record (the `0x0000_0006` discriminator at `+4`, six
/// zero bytes at `+8..+14`, the `0x0002` type tag at `+14`, and the
/// `0x0001` delimiter at `+20`) are tight enough that random
/// occurrences inside a Sheet stream are extremely unlikely. Each
/// match is consumed as a single 26-byte `Probed` range, mirroring the
/// confidence the Phase 6 reverse-engineering notes attach to these
/// records.
///
/// Returns the count of records claimed; callers may ignore it but the
/// number is useful for unit assertions.
pub fn scan_endpoint_records_with_trace(data: &[u8], trace: &mut ParserTraceBuilder) -> usize {
    if data.len() < 26 {
        return 0;
    }
    let end = data.len() - 26;
    let mut hits = 0usize;
    let mut i = 0usize;
    while i <= end {
        if u32_le(data, i + 4) != DISCRIMINATOR {
            i += 1;
            continue;
        }
        if !data[i + 8..i + 14].iter().all(|&b| b == 0) {
            i += 1;
            continue;
        }
        if u16_le(data, i + 14) != ENDPOINT_TYPE_TAG {
            i += 1;
            continue;
        }
        if u16_le(data, i + 20) != ENDPOINT_DELIMITER {
            i += 1;
            continue;
        }
        trace.consume(
            ByteRange::new(i as u64, (i + 26) as u64),
            TraceConfidence::Probed,
        );
        hits += 1;
        i += 26;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint_bytes(rel_fx: u32, endpoint_a: u32, endpoint_b: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&rel_fx.to_le_bytes());
        v.extend_from_slice(&DISCRIMINATOR.to_le_bytes());
        v.extend_from_slice(&[0u8; 6]);
        v.extend_from_slice(&ENDPOINT_TYPE_TAG.to_le_bytes());
        v.extend_from_slice(&endpoint_a.to_le_bytes());
        v.extend_from_slice(&[0x01, 0x00]);
        v.extend_from_slice(&endpoint_b.to_le_bytes());
        v
    }

    #[test]
    fn parses_single_endpoint_record() {
        let mut buf = vec![0u8; 32];
        buf.extend_from_slice(&endpoint_bytes(0x03B7, 0x02E4, 0x008B));
        let mut set = HashSet::new();
        set.insert(0x03B7);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rel_field_x, 0x03B7);
        assert_eq!(r[0].endpoint_a, 0x02E4);
        assert_eq!(r[0].endpoint_b, 0x008B);
        assert_eq!(r[0].offset, 32);
        assert_eq!(r[0].sheet_path, "/Sheet6");
    }

    #[test]
    fn ignores_field_x_not_in_rel_set() {
        let mut buf = vec![0u8; 32];
        buf.extend_from_slice(&endpoint_bytes(0x03B7, 0x02E4, 0x008B));
        let set = HashSet::new();
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert!(r.is_empty(), "must skip when rel_field_xs is empty");
    }

    #[test]
    fn ignores_wrong_discriminator() {
        let mut buf = vec![0u8; 32];
        let mut bad = endpoint_bytes(0x03B7, 0x02E4, 0x008B);
        // Corrupt the discriminator u32 at +4.
        bad[4..8].copy_from_slice(&[0x07, 0x00, 0x00, 0x00]);
        buf.extend_from_slice(&bad);
        let mut set = HashSet::new();
        set.insert(0x03B7);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert!(r.is_empty());
    }

    #[test]
    fn ignores_non_zero_padding() {
        let mut buf = vec![0u8; 32];
        let mut bad = endpoint_bytes(0x03B7, 0x02E4, 0x008B);
        bad[10] = 0x01;
        buf.extend_from_slice(&bad);
        let mut set = HashSet::new();
        set.insert(0x03B7);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert!(r.is_empty());
    }

    #[test]
    fn ignores_wrong_type_tag() {
        let mut buf = vec![0u8; 32];
        let mut bad = endpoint_bytes(0x03B7, 0x02E4, 0x008B);
        bad[14] = 0x12;
        buf.extend_from_slice(&bad);
        let mut set = HashSet::new();
        set.insert(0x03B7);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert!(r.is_empty());
    }

    #[test]
    fn ignores_wrong_endpoint_delimiter() {
        let mut buf = vec![0u8; 32];
        let mut bad = endpoint_bytes(0x03B7, 0x02E4, 0x008B);
        bad[20..22].copy_from_slice(&0x0002u16.to_le_bytes());
        buf.extend_from_slice(&bad);
        let mut set = HashSet::new();
        set.insert(0x03B7);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert!(r.is_empty());
    }

    #[test]
    fn parses_multiple_endpoint_records() {
        let mut buf = vec![0u8; 8];
        buf.extend_from_slice(&endpoint_bytes(0x03B7, 0x02E4, 0x008B));
        buf.extend_from_slice(&[0xAA; 12]);
        buf.extend_from_slice(&endpoint_bytes(0x03B9, 0x008B, 0x0146));
        let mut set = HashSet::new();
        set.insert(0x03B7);
        set.insert(0x03B9);
        let r = parse_endpoint_records("/Sheet6", &buf, &set);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].rel_field_x, 0x03B7);
        assert_eq!(r[1].rel_field_x, 0x03B9);
        assert_eq!(r[1].endpoint_a, 0x008B);
        assert_eq!(r[1].endpoint_b, 0x0146);
    }

    #[test]
    fn trace_aware_scan_consumes_each_match_as_26b_probed_range() {
        let mut buf = vec![0u8; 16];
        buf.extend_from_slice(&endpoint_bytes(0x03B7, 0x02E4, 0x008B));
        buf.extend_from_slice(&[0xAA; 8]);
        buf.extend_from_slice(&endpoint_bytes(0x03B9, 0x008B, 0x0146));
        buf.extend_from_slice(&[0xBB; 4]); // trailing leftover

        let mut trace = ParserTraceBuilder::new("scan_endpoint_records");
        let hits = scan_endpoint_records_with_trace(&buf, &mut trace);
        assert_eq!(hits, 2);

        let trace = trace.build("/Sheet6", buf.len() as u64);
        // 2 hits × 26B = 52 bytes consumed as Probed.
        let probed_total: u64 = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(ByteRange::len)
            .sum();
        assert_eq!(probed_total, 52);
        assert_eq!(trace.consumed_bytes(), 52);
        assert_eq!(trace.leftover_bytes(), buf.len() as u64 - 52);
    }

    #[test]
    fn trace_aware_scan_skips_records_with_corrupt_discriminator() {
        let mut buf = vec![0u8; 8];
        let mut bad = endpoint_bytes(0x03B7, 0x02E4, 0x008B);
        bad[4..8].copy_from_slice(&[0x07, 0x00, 0x00, 0x00]); // wrong
        buf.extend_from_slice(&bad);
        buf.extend_from_slice(&endpoint_bytes(0x03B9, 0x008B, 0x0146));

        let mut trace = ParserTraceBuilder::new("scan_endpoint_records");
        let hits = scan_endpoint_records_with_trace(&buf, &mut trace);
        assert_eq!(hits, 1, "only the second (well-formed) record must claim");
        let trace = trace.build("/Sheet6", buf.len() as u64);
        assert_eq!(trace.consumed_bytes(), 26);
    }

    #[test]
    fn trace_aware_scan_short_streams_yield_no_consumes() {
        let buf = vec![0u8; 20]; // < 26
        let mut trace = ParserTraceBuilder::new("scan_endpoint_records");
        let hits = scan_endpoint_records_with_trace(&buf, &mut trace);
        assert_eq!(hits, 0);
        let trace = trace.build("/Sheet6", buf.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
    }
}
