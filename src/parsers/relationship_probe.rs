//! Relationship record probe for `Unclustered Dynamic Attributes`.
//!
//! Status: reverse-engineering scaffold, NOT a decoder.
//!
//! Observed from sample DWG-0201GP06-01.pid / DWG-0202GP06-01.pid:
//!
//! * Each `Relationship.<GUID>` ASCII tag appears exactly once inside the
//!   Unclustered Dynamic Attributes stream and nowhere else in the CFB
//!   (checked across all 69 streams, both raw and Windows/OLE GUID layouts).
//! * Consequently, `source` / `target` endpoint identifiers are NOT present
//!   next to the Relationship record in this stream. They must be recovered
//!   from Sheet streams or another cluster and are outside the scope of this
//!   probe.
//!
//! This probe therefore only captures byte-level evidence adjacent to each
//! Relationship record:
//!
//! * Its 32-hex `guid`.
//! * The stream offset of the ASCII tag (`ascii_offset`).
//! * Any additional 32-char hex GUIDs that appear within `WINDOW` bytes of
//!   the tag (usually the enclosing record's `DrawingNo`).
//! * Two short little-endian `u16` tokens that follow the record separator
//!   (`89 00 0A 01 00 00`); these look like a monotonically increasing
//!   record ID plus an ordinal, but are only exposed as raw numbers.
//!
//! Future work: once a Sheet-level decoder exists, cross-reference each
//! Relationship `guid` against Sheet-resident binary GUID tables to resolve
//! `source` / `target`.

use crate::model::{RelationshipProbe, RelationshipTrailingToken};

/// Bytes scanned around each Relationship tag when collecting neighbouring
/// hex GUIDs. Kept small so we don't spill into adjacent records.
const WINDOW: usize = 192;

const TAG: &[u8] = b"Relationship.";

/// Build a `RelationshipProbe` list from the raw Unclustered Dynamic
/// Attributes stream bytes. Returns an empty vector if the stream contains
/// no `Relationship.<GUID>` ASCII tags.
pub fn probe_relationships(data: &[u8]) -> Vec<RelationshipProbe> {
    let mut probes = Vec::new();
    let mut i = 0usize;
    while i + TAG.len() + 32 <= data.len() {
        if &data[i..i + TAG.len()] != TAG {
            i += 1;
            continue;
        }
        let guid_start = i + TAG.len();
        let guid_end = guid_start + 32;
        if !data[guid_start..guid_end].iter().all(u8::is_ascii_hexdigit) {
            i += 1;
            continue;
        }
        let guid = String::from_utf8_lossy(&data[guid_start..guid_end]).to_string();

        let window_start = i.saturating_sub(WINDOW);
        let window_end = (guid_end + WINDOW).min(data.len());
        let nearby_ascii_guids = find_nearby_guids(data, window_start, window_end, i);

        let trailing_tokens = scan_trailing_tokens(data, guid_end);

        probes.push(RelationshipProbe {
            guid,
            ascii_offset: i,
            window_start,
            window_end,
            nearby_ascii_guids,
            trailing_tokens,
        });

        i = guid_end;
    }
    probes
}

/// Scan the window for 32-char hex substrings, excluding the Relationship's
/// own GUID tag at `exclude_offset`.
fn find_nearby_guids(
    data: &[u8],
    start: usize,
    end: usize,
    exclude_offset: usize,
) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::<usize>::new();
    let mut i = start;
    let exclude_start = exclude_offset + TAG.len();
    while i + 32 <= end {
        if i >= exclude_start && i < exclude_start + 32 {
            i += 1;
            continue;
        }
        if data[i..i + 32].iter().all(u8::is_ascii_hexdigit) {
            if seen.insert(i) {
                out.push((i, String::from_utf8_lossy(&data[i..i + 32]).to_string()));
            }
            i += 32;
        } else {
            i += 1;
        }
    }
    out
}

/// After the Relationship payload (`<GUID>\0`), the sample files show a
/// consistent separator followed by two interesting `u16` slots:
///
/// ```text
///   89 00 0A 01 00 00 | <u16 slot_a> | 00 x 10 bytes | <u16 slot_b>
/// ```
///
/// Slot A (at `marker+6`) is a monotonically increasing record id
/// (0x6086, 0x6087, 0x6088, ... in sample 1); slot B (at `marker+18`) is
/// the same value modulo 0x0003 in the tested samples but may carry a
/// record-type or ordinal meaning we don't yet understand. Both are
/// surfaced as raw numbers without interpretation.
fn scan_trailing_tokens(data: &[u8], guid_end: usize) -> Vec<RelationshipTrailingToken> {
    // Skip the null terminator right after the GUID.
    let after_null = guid_end + 1;
    if after_null >= data.len() {
        return Vec::new();
    }

    // The separator pattern; we search forward for the first `89 00` within
    // 64 bytes of the GUID and then read the two recognised token slots.
    const SEPARATOR: [u8; 2] = [0x89, 0x00];
    let scan_end = (after_null + 64).min(data.len());
    let Some(sep) = (after_null..scan_end.saturating_sub(SEPARATOR.len()))
        .find(|&p| data[p..p + SEPARATOR.len()] == SEPARATOR)
    else {
        return Vec::new();
    };

    let mut tokens = Vec::new();
    let slot_a = sep + 6;
    if slot_a + 2 <= data.len() {
        tokens.push(RelationshipTrailingToken {
            offset: slot_a,
            label: "after_marker+6".to_string(),
            value: u16::from_le_bytes([data[slot_a], data[slot_a + 1]]),
        });
    }
    let slot_b = sep + 18;
    if slot_b + 2 <= data.len() {
        tokens.push(RelationshipTrailingToken {
            offset: slot_b,
            label: "after_marker+18".to_string(),
            value: u16::from_le_bytes([data[slot_b], data[slot_b + 1]]),
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relationship_record(guid: &str, slot_a: u16, slot_b: u16) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"Relationship.");
        v.extend_from_slice(guid.as_bytes());
        v.push(0); // null terminator
        v.extend_from_slice(&[0x03, 0x00, 0x0D, 0x00]);
        v.extend_from_slice(b"Flag");
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
        // separator
        v.extend_from_slice(&[0x89, 0x00, 0x0A, 0x01, 0x00, 0x00]);
        v.extend_from_slice(&slot_a.to_le_bytes()); // sep+6..7
        v.extend_from_slice(&[0u8; 10]); // sep+8..17 — zero padding
        v.extend_from_slice(&slot_b.to_le_bytes()); // sep+18..19
        v
    }

    #[test]
    fn probe_finds_single_relationship() {
        let mut buf = vec![0u8; 32];
        let rec = relationship_record("C5CF946710BF4EBDB02808EBD6879B62", 0x0086, 0x03B7);
        let offset = buf.len();
        buf.extend_from_slice(&rec);

        let probes = probe_relationships(&buf);
        assert_eq!(probes.len(), 1);
        let p = &probes[0];
        assert_eq!(p.guid, "C5CF946710BF4EBDB02808EBD6879B62");
        assert_eq!(p.ascii_offset, offset);
        assert_eq!(p.trailing_tokens.len(), 2);
        assert_eq!(p.trailing_tokens[0].value, 0x0086);
        assert_eq!(p.trailing_tokens[1].value, 0x03B7);
    }

    #[test]
    fn probe_skips_non_guid_tag() {
        // "Relationship." followed by non-hex must be skipped, not panic.
        let mut buf = b"noise-".to_vec();
        buf.extend_from_slice(b"Relationship.NOTHEX_NOTHEX_NOTHEX_NOTHEX_NOT!!!\0");
        let probes = probe_relationships(&buf);
        assert!(probes.is_empty());
    }

    #[test]
    fn probe_finds_nearby_guids() {
        // Record tag + a "DrawingNo" GUID in the window.
        let drawing_guid = "0F7B8ABD0C4E493FA3C7F06FD03AD6AA";
        let mut buf = Vec::new();
        buf.extend_from_slice(b"DrawingNo\0");
        buf.extend_from_slice(drawing_guid.as_bytes());
        buf.extend_from_slice(&[0; 16]);
        buf.extend_from_slice(&relationship_record(
            "C5CF946710BF4EBDB02808EBD6879B62",
            0x0086,
            0x03B7,
        ));

        let probes = probe_relationships(&buf);
        assert_eq!(probes.len(), 1);
        let p = &probes[0];
        assert!(
            p.nearby_ascii_guids.iter().any(|(_, g)| g == drawing_guid),
            "probe should capture the DrawingNo GUID in its window, got {:?}",
            p.nearby_ascii_guids
        );
    }

    #[test]
    fn probe_records_multiple_relationships() {
        let mut buf = vec![0u8; 16];
        buf.extend_from_slice(&relationship_record(
            "C5CF946710BF4EBDB02808EBD6879B62",
            0x0086,
            0x03B7,
        ));
        buf.extend_from_slice(&relationship_record(
            "D8AE93D9CFA548C8AAB076FD101974F3",
            0x0087,
            0x03B9,
        ));

        let probes = probe_relationships(&buf);
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].trailing_tokens[0].value, 0x0086);
        assert_eq!(probes[1].trailing_tokens[0].value, 0x0087);
        assert_eq!(probes[0].trailing_tokens[1].value, 0x03B7);
        assert_eq!(probes[1].trailing_tokens[1].value, 0x03B9);
    }
}
