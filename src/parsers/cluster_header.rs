//! Cluster-family stream header decoder.
//!
//! Every `PSMcluster0` / `StyleCluster` / similar cluster stream
//! opens with a common fixed header carrying a magic number, row
//! counts, and an indexed-string table. This module exposes
//! [`parse_header`] and the magic constant so the orchestrating
//! layer in [`crate::streams::cluster`] can quickly tell cluster
//! streams apart from the rest.
//!
//! Phase 12b-1f layered trace-aware variants ([`parse_header_with_trace`],
//! [`parse_string_table_with_trace`], [`parse_psm_cluster0_with_trace`])
//! mirror the back-compat thin-wrapper pattern used elsewhere in
//! `parsers/*` so [`crate::byte_audit::aggregate`] can register
//! cluster-family streams without forcing the orchestrating layer to
//! take a builder argument.

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{ClusterHeader, IndexedString};

/// `u32` LE signature that introduces every cluster-family stream
/// (PSM clusters, style cluster, Dynamic Attributes metadata, sheets).
pub const CLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Parse the common header shared by all cluster-family streams.
/// Returns None if the data is too short or the magic doesn't match.
///
/// Thin back-compat wrapper around [`parse_header_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_header(data: &[u8]) -> Option<ClusterHeader> {
    let mut trace = ParserTraceBuilder::new("parse_cluster_header");
    parse_header_with_trace(data, &mut trace)
}

/// Phase 12b-1f trace-aware variant of [`parse_header`].
///
/// Trace schema:
/// - `[0..16]` — full 16-byte header (magic / `record_count` /
///   `stream_type` / `body_len` / flags) — `TraceConfidence::Decoded`
///   when the magic matches.
///
/// Magic mismatch / short stream short-circuit before any consume call,
/// so the leftover view cleanly attributes every byte to the expected
/// next parser.
pub fn parse_header_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<ClusterHeader> {
    if data.len() < 16 {
        return None;
    }
    let magic = u32_le(data, 0);
    if magic != CLUSTER_MAGIC {
        return None;
    }
    trace.consume(ByteRange::new(0, 16), TraceConfidence::Decoded);
    Some(ClusterHeader {
        magic,
        record_count: u32_le(data, 4),
        stream_type: u16_le(data, 8),
        body_len: u32_le(data, 10),
        flags: u16_le(data, 14),
    })
}

/// Parse the indexed UTF-16LE string table found in `PSMcluster0`.
/// Starts scanning from `offset`; each entry is: u32 index + u32 `byte_len` + UTF-16LE payload.
/// Stops when it encounters an index of 0 followed by a zero-length entry, or runs out of data.
///
/// Thin back-compat wrapper around [`parse_string_table_with_trace`];
/// discards the trace output for callers that do not opt into byte
/// auditing.
pub fn parse_string_table(data: &[u8], start: usize) -> (Vec<IndexedString>, usize) {
    let mut trace = ParserTraceBuilder::new("parse_cluster_string_table");
    parse_string_table_with_trace(data, start, &mut trace)
}

/// Phase 12b-1f trace-aware variant of [`parse_string_table`].
///
/// Trace schema:
/// - per entry at `pos`:
///   - `[pos..pos+8]` — entry header (`index` + `byte_len`) —
///     `TraceConfidence::Decoded`.
///   - `[pos+8..pos+8+byte_len]` — UTF-16LE payload (when
///     `byte_len > 0`) — `TraceConfidence::Decoded`.
/// - sentinel (`index == 0 && byte_len == 0`): the 8-byte header is
///   consumed; the loop exits leaving the trailing region as leftover.
///
/// Truncated final entry (`byte_len` runs past `data.len()`) breaks out
/// of the loop **after** consuming the header — the body bytes that
/// could not be read remain in the leftover view, mirroring the legacy
/// "stop at the last clean entry" behaviour.
pub fn parse_string_table_with_trace(
    data: &[u8],
    start: usize,
    trace: &mut ParserTraceBuilder,
) -> (Vec<IndexedString>, usize) {
    let mut out = Vec::new();
    let mut pos = start;

    while pos + 8 <= data.len() {
        let index = u32_le(data, pos);
        let byte_len = u32_le(data, pos + 4) as usize;
        let header_end = pos + 8;
        trace.consume(
            ByteRange::new(pos as u64, header_end as u64),
            TraceConfidence::Decoded,
        );
        pos = header_end;

        if byte_len == 0 {
            // Sentinel: index==0 with zero-length payload signals end of
            // table. Non-zero index with zero-length is a legitimate
            // empty string entry.
            if index == 0 {
                break;
            }
            out.push(IndexedString {
                index,
                value: String::new(),
            });
            continue;
        }

        if pos + byte_len > data.len() {
            break;
        }

        let body_end = pos + byte_len;
        trace.consume(
            ByteRange::new(pos as u64, body_end as u64),
            TraceConfidence::Decoded,
        );
        let char_count = byte_len / 2;
        let words: Vec<u16> = (0..char_count)
            .map(|i| u16_le(data, pos + i * 2))
            .take_while(|&w| w != 0)
            .collect();
        let value = String::from_utf16_lossy(&words);

        out.push(IndexedString { index, value });
        pos = body_end;
    }

    (out, pos)
}

/// Phase 12b-1f high-level trace walker for `/PSMcluster0`.
///
/// Combines the cluster-family header probe with the indexed
/// string-table walker, and marks the heuristic prefix between the
/// header and the located string table as `TraceConfidence::Probed` so
/// the byte-audit consumer sees this region as "byte position known,
/// field semantics still being reverse-engineered" rather than as
/// generic leftover. Returns the parsed header on success (the string
/// table itself is not surfaced — callers needing the typed table go
/// through [`parse_string_table`]).
pub fn parse_psm_cluster0_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<ClusterHeader> {
    let header = parse_header_with_trace(data, trace)?;
    if data.len() <= 32 {
        return Some(header);
    }
    let table_start = find_string_table_start(data);
    if table_start > 16 {
        // Heuristic locator region — record as Probed so it stops
        // showing up as leftover but its semantic role stays explicit.
        trace.consume(
            ByteRange::new(16, table_start as u64),
            TraceConfidence::Probed,
        );
    }
    let _ = parse_string_table_with_trace(data, table_start, trace);
    Some(header)
}

/// Heuristic copy of `streams::cluster::find_string_table_start` —
/// kept here so the `parsers` layer is self-contained when running the
/// trace walker. Returns `16` (right after the header) as a fallback
/// when no plausible entry-2 marker is found, mirroring the orchestrator
/// behaviour but capping the leftover-Probed gap to zero in that case.
fn find_string_table_start(data: &[u8]) -> usize {
    for i in 20..data.len().saturating_sub(12) {
        let val = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if val == 2 {
            let blen =
                u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
            if (4..512).contains(&blen) && blen.is_multiple_of(2) && i + 8 + blen <= data.len() {
                let first_char = u16::from_le_bytes([data[i + 8], data[i + 9]]);
                if (0x20..=0x7e).contains(&first_char) {
                    if let Some(entry1_start) = find_entry1_before(data, i) {
                        return entry1_start;
                    }
                    return i;
                }
            }
        }
    }
    32
}

fn find_entry1_before(data: &[u8], entry2_pos: usize) -> Option<usize> {
    for blen in (4..=256).step_by(2) {
        let str_start = entry2_pos.checked_sub(blen)?;
        let blen_pos = str_start.checked_sub(4)?;
        let idx_pos = blen_pos.checked_sub(4)?;
        if idx_pos < 16 {
            continue;
        }
        let stored_blen = u32::from_le_bytes([
            data[blen_pos],
            data[blen_pos + 1],
            data[blen_pos + 2],
            data[blen_pos + 3],
        ]) as usize;
        if stored_blen != blen {
            continue;
        }
        let first_char = u16::from_le_bytes([data[str_start], data[str_start + 1]]);
        if (0x20..=0x7e).contains(&first_char) {
            let idx_val = u32::from_le_bytes([
                data[idx_pos],
                data[idx_pos + 1],
                data[idx_pos + 2],
                data[idx_pos + 3],
            ]);
            if idx_val <= 10 {
                return Some(idx_pos);
            }
            for extra in 1..=4 {
                let alt_start = idx_pos.checked_sub(extra)?;
                if alt_start >= 16 {
                    return Some(alt_start);
                }
            }
        }
    }
    None
}

fn u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn make_header(record_count: u32, stream_type: u16, body_len: u32, flags: u16) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&CLUSTER_MAGIC.to_le_bytes());
        out.extend_from_slice(&record_count.to_le_bytes());
        out.extend_from_slice(&stream_type.to_le_bytes());
        out.extend_from_slice(&body_len.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out
    }

    #[test]
    fn trace_aware_header_consumes_full_16_bytes_when_magic_matches() {
        let mut data = make_header(354, 0x00CE, 121, 0);
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // body trailing
        let mut b = ParserTraceBuilder::new("parse_cluster_header");
        let header = parse_header_with_trace(&data, &mut b).expect("valid header");
        assert_eq!(header.magic, CLUSTER_MAGIC);
        assert_eq!(header.record_count, 354);
        assert_eq!(header.stream_type, 0x00CE);
        let trace = b.build("/PSMcluster0", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            16,
            "header trace claims exactly 16 bytes; body remains leftover"
        );
        assert_eq!(trace.leftover_bytes(), 3);
    }

    #[test]
    fn trace_aware_header_short_circuits_on_wrong_magic_without_consume() {
        let mut data = vec![0xFFu8; 16];
        data[0] = 0; // break magic
        let mut b = ParserTraceBuilder::new("parse_cluster_header");
        let r = parse_header_with_trace(&data, &mut b);
        assert!(r.is_none());
        let trace = b.build("/PSMcluster0", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
    }

    #[test]
    fn trace_aware_header_short_circuits_on_truncated_stream() {
        let data = vec![0u8; 12];
        let mut b = ParserTraceBuilder::new("parse_cluster_header");
        let r = parse_header_with_trace(&data, &mut b);
        assert!(r.is_none());
        let trace = b.build("/PSMcluster0", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
    }

    #[test]
    fn trace_aware_string_table_covers_each_entry_header_and_payload() {
        let mut data: Vec<u8> = Vec::new();
        // entry 1: index=1, payload "AB" (4 bytes UTF-16LE)
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("AB"));
        // entry 2: index=2, payload "CD" (4 bytes UTF-16LE)
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("CD"));
        // sentinel
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&[0xDE, 0xAD]); // trailing leftover

        let mut b = ParserTraceBuilder::new("parse_cluster_string_table");
        let (entries, pos) = parse_string_table_with_trace(&data, 0, &mut b);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].value, "AB");
        assert_eq!(entries[1].value, "CD");
        let trace = b.build("/PSMcluster0", data.len() as u64);
        // 2 entries × (8B header + 4B body) + 8B sentinel = 32 consumed.
        assert_eq!(trace.consumed_bytes(), 32);
        assert_eq!(trace.leftover_bytes(), 2);
        assert_eq!(pos, 32, "cursor sits right after sentinel");
    }

    #[test]
    fn trace_aware_string_table_keeps_truncated_body_as_leftover() {
        let mut data: Vec<u8> = Vec::new();
        // entry 1: index=1, claims 8 bytes but only 2 follow
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&[0xAA, 0xBB]);

        let mut b = ParserTraceBuilder::new("parse_cluster_string_table");
        let (entries, _) = parse_string_table_with_trace(&data, 0, &mut b);
        assert!(entries.is_empty());
        let trace = b.build("/PSMcluster0", data.len() as u64);
        // Only the 8-byte header gets consumed; the truncated 2-byte
        // body stays as leftover.
        assert_eq!(trace.consumed_bytes(), 8);
        assert_eq!(trace.leftover_bytes(), 2);
    }

    #[test]
    fn trace_aware_psm_cluster0_marks_locator_prefix_probed() {
        // Header + 16-byte locator gap + entry-2-anchored table.
        let mut data = make_header(0, 0, 0, 0);
        // locator gap (16 bytes)
        data.extend_from_slice(&[0u8; 16]);
        let table_start = data.len();
        // entry 1: index=1, "AB" (4B body)
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("AB"));
        // entry 2: index=2, "CD" (4B body) — required so
        // find_string_table_start returns table_start
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("CD"));
        // sentinel
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let mut b = ParserTraceBuilder::new("parse_psm_cluster0");
        let header = parse_psm_cluster0_with_trace(&data, &mut b).expect("valid");
        assert_eq!(header.magic, CLUSTER_MAGIC);
        let trace = b.build("/PSMcluster0", data.len() as u64);

        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        assert!(
            !probed.is_empty(),
            "locator prefix must be Probed: {probed:?}"
        );
        // Locator gap covers [16..table_start].
        let gap_len: u64 = probed.iter().map(ByteRange::len).sum();
        assert_eq!(gap_len, (table_start - 16) as u64);

        let decoded: Vec<ByteRange> = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        // Header + every entry header + every payload + sentinel are
        // Decoded — the union must cover the entire stream.
        assert_eq!(trace.consumed_bytes(), data.len() as u64);
        assert!(trace.leftover_ranges.is_empty());
        assert!(decoded.iter().any(|r| r.start == 0 && r.end == 16));
    }

    #[test]
    fn back_compat_parse_header_matches_trace_variant_byte_for_byte() {
        let data = make_header(7, 0xCE, 121, 0);
        let without_trace = parse_header(&data).expect("legacy fn");
        let mut b = ParserTraceBuilder::new("parse_cluster_header");
        let with_trace = parse_header_with_trace(&data, &mut b).expect("trace fn");
        assert_eq!(without_trace.magic, with_trace.magic);
        assert_eq!(without_trace.record_count, with_trace.record_count);
        assert_eq!(without_trace.stream_type, with_trace.stream_type);
        assert_eq!(without_trace.body_len, with_trace.body_len);
        assert_eq!(without_trace.flags, with_trace.flags);
    }

    #[test]
    fn back_compat_parse_string_table_matches_trace_variant_byte_for_byte() {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("AB"));
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let (legacy, legacy_pos) = parse_string_table(&data, 0);
        let mut b = ParserTraceBuilder::new("parse_cluster_string_table");
        let (modern, modern_pos) = parse_string_table_with_trace(&data, 0, &mut b);
        assert_eq!(legacy_pos, modern_pos);
        assert_eq!(legacy.len(), modern.len());
        for (l, m) in legacy.iter().zip(modern.iter()) {
            assert_eq!(l.index, m.index);
            assert_eq!(l.value, m.value);
        }
    }
}
