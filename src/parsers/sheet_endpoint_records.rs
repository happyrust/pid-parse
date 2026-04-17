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

use crate::model::SheetEndpointRecord;

/// Constant `u32` at offset `+4` that marks an endpoint record.
const DISCRIMINATOR: u32 = 0x0000_0006;
/// `u16` type tag at offset `+16` that marks an endpoint record.
const ENDPOINT_TYPE_TAG: u16 = 0x0002;

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
}
