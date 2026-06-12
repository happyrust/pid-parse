//! Decoder for the top-level `/JSitesList` stream (Phase 29 Slice M).
//!
//! Cross-fixture probe evidence
//! (`examples/probe_phase29_unregistered_tails.rs`, 6/6 fixtures):
//! the stream is `"OLEM"` magic + `u32` entry count + a 4-byte-aligned
//! `u32` slot table. On 4 of 6 fixtures the table holds exactly
//! `count` slots; on `dwg0202` / `publish-dwg0202` it holds 16 slots
//! for `count = 13`, and the 3 trailing slots repeat values from the
//! logical table — a stale, non-truncated tail. The decoder therefore
//! reads the first `count` slots as the logical list and reports (but
//! never claims) any trailing slots. The entry values correlate
//! strongly with the numeric ids of `JSite<id>` storages in the same
//! package (e.g. `publish-a01` lists 204 / 39 / 121, all present as
//! `JSite204` / `JSite39` / `JSite121`), but that mapping is recorded
//! as evidence only — the field is named `entries`, not `jsite_ids`,
//! until the writer-side semantics are IDA-confirmed.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};

/// 4-byte magic that opens the `/JSitesList` stream.
pub const JSITES_LIST_MAGIC: [u8; 4] = *b"OLEM";

/// Upper bound accepted for the `/JSitesList` entry count. Real
/// fixtures carry 5–20 entries; the cap only rejects wide-scan false
/// positives with absurd counts.
pub const JSITES_LIST_MAX_ENTRIES: u32 = 100_000;

/// Decoded `/JSitesList` stream: `"OLEM"` magic + `u32` count + a
/// 4-byte-aligned `u32` slot table whose first `count` slots form the
/// logical list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JSitesListDecoded {
    /// Entry count read from offset 4. Always equals `entries.len()`.
    pub count: u32,
    /// The first `count` `u32` little-endian slot values. Probe
    /// evidence correlates them with `JSite<id>` storage ids; the
    /// semantic name stays unclaimed pending IDA confirmation.
    pub entries: Vec<u32>,
    /// Slot values found after the logical table (stale, non-truncated
    /// tail observed on `dwg0202` / `publish-dwg0202`, where they
    /// repeat logical values). Reported for audit purposes only; the
    /// byte-audit trace leaves these bytes as leftover.
    pub trailing_slots: Vec<u32>,
}

/// Parse the `/JSitesList` stream.
///
/// Validation gate: `"OLEM"` magic, `count <=`
/// [`JSITES_LIST_MAX_ENTRIES`], a 4-byte-aligned body
/// (`(len - 8) % 4 == 0`), and at least `count` slots present
/// (`len >= 8 + 4 * count`). Extra trailing slots are tolerated and
/// surfaced via [`JSitesListDecoded::trailing_slots`]. Any other
/// mismatch returns `None`.
///
/// Thin back-compat wrapper around [`parse_jsites_list_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_jsites_list(data: &[u8]) -> Option<JSitesListDecoded> {
    let mut trace = ParserTraceBuilder::new("parse_jsites_list");
    parse_jsites_list_with_trace(data, &mut trace)
}

/// Trace-aware variant of [`parse_jsites_list`].
///
/// Trace schema:
/// - `[0..8)` — magic + count — `TraceConfidence::Decoded` (structure
///   proven by the slot-table invariant across fixtures).
/// - `[8..8 + 4 * count)` — logical entry table —
///   `TraceConfidence::Probed` (byte layout proven, value semantics
///   unnamed).
/// - `[8 + 4 * count..len)` — stale trailing slots — **not claimed**;
///   they stay in the leftover view until their writer-side semantics
///   are proven.
///
/// Gate failure short-circuits before any consume call.
pub fn parse_jsites_list_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<JSitesListDecoded> {
    if data.len() < 8 || data[0..4] != JSITES_LIST_MAGIC {
        return None;
    }
    if !(data.len() - 8).is_multiple_of(4) {
        return None;
    }
    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if count > JSITES_LIST_MAX_ENTRIES {
        return None;
    }
    let table_end = 8usize.checked_add((count as usize).checked_mul(4)?)?;
    if data.len() < table_end {
        return None;
    }
    let mut slots = data[8..]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let entries: Vec<u32> = slots.by_ref().take(count as usize).collect();
    let trailing_slots: Vec<u32> = slots.collect();

    trace.consume(ByteRange::new(0, 8), TraceConfidence::Decoded);
    if table_end > 8 {
        trace.consume(ByteRange::new(8, table_end as u64), TraceConfidence::Probed);
    }
    Some(JSitesListDecoded {
        count,
        entries,
        trailing_slots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stream(count: u32, entries: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&JSITES_LIST_MAGIC);
        out.extend_from_slice(&count.to_le_bytes());
        for e in entries {
            out.extend_from_slice(&e.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_canonical_list_with_exact_size() {
        let data = make_stream(3, &[204, 39, 121]);
        let list = parse_jsites_list(&data).expect("canonical list");
        assert_eq!(list.count, 3);
        assert_eq!(list.entries, vec![204, 39, 121]);
        assert!(list.trailing_slots.is_empty());
    }

    #[test]
    fn parses_stale_trailing_slots_without_claiming_them() {
        // dwg0202 shape: count=2 logical entries + 2 stale tail slots.
        let data = make_stream(2, &[793, 4458, 793, 4458]);
        let list = parse_jsites_list(&data).expect("stale tail tolerated");
        assert_eq!(list.entries, vec![793, 4458]);
        assert_eq!(list.trailing_slots, vec![793, 4458]);

        let mut b = ParserTraceBuilder::new("parse_jsites_list");
        let _ = parse_jsites_list_with_trace(&data, &mut b).expect("valid");
        let trace = b.build("/JSitesList", data.len() as u64);
        assert_eq!(
            trace.leftover_bytes(),
            8,
            "two stale slots must stay leftover"
        );
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut data = make_stream(1, &[204]);
        data[0] ^= 0xFF;
        assert!(parse_jsites_list(&data).is_none());
    }

    #[test]
    fn rejects_size_mismatch() {
        // Unaligned trailing byte.
        let mut long = make_stream(1, &[204]);
        long.push(0);
        assert!(parse_jsites_list(&long).is_none());
        // Truncated entry table.
        let short = make_stream(2, &[204]);
        assert!(parse_jsites_list(&short).is_none());
    }

    #[test]
    fn rejects_absurd_count_and_short_stream() {
        let mut data = Vec::new();
        data.extend_from_slice(&JSITES_LIST_MAGIC);
        data.extend_from_slice(&(JSITES_LIST_MAX_ENTRIES + 1).to_le_bytes());
        assert!(parse_jsites_list(&data).is_none());
        assert!(parse_jsites_list(&data[..6]).is_none());
        assert!(parse_jsites_list(&[]).is_none());
    }

    #[test]
    fn accepts_empty_list() {
        let data = make_stream(0, &[]);
        let list = parse_jsites_list(&data).expect("empty list is exact-size");
        assert_eq!(list.count, 0);
        assert!(list.entries.is_empty());
    }

    #[test]
    fn trace_claims_header_decoded_and_entries_probed() {
        let data = make_stream(2, &[145, 151]);
        let mut b = ParserTraceBuilder::new("parse_jsites_list");
        let _ = parse_jsites_list_with_trace(&data, &mut b).expect("valid");
        let trace = b.build("/JSitesList", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
        assert!(trace.leftover_ranges.is_empty());
        let decoded: u64 = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(ByteRange::len)
            .sum();
        assert_eq!(decoded, 8, "only magic + count are Decoded");
    }

    #[test]
    fn trace_claims_nothing_on_gate_failure() {
        let mut data = make_stream(2, &[145, 151]);
        data.push(0xAA);
        let mut b = ParserTraceBuilder::new("parse_jsites_list");
        assert!(parse_jsites_list_with_trace(&data, &mut b).is_none());
        let trace = b.build("/JSitesList", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
    }
}
