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
///
/// Phase 29 Slice B extends the walk past the string table: when the
/// fixed-length prologue plus a strict body record chain explains every
/// remaining byte (see [`decode_psm_cluster0_body_records`] for the
/// full-coverage gate), the prologue is traced `Probed`, each record
/// envelope `Decoded`, and each record payload `Probed`. When the gate
/// fails, the post-table region stays leftover — no partial claims.
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
    let (_, table_end) = parse_string_table_with_trace(data, table_start, trace);

    if let Some((prologue, records)) = psm_cluster0_chain_after_table(data, table_end) {
        // Prologue: byte position known, semantics unexplained.
        trace.consume(
            ByteRange::new(prologue.start as u64, prologue.end as u64),
            TraceConfidence::Probed,
        );
        for record in &records {
            let envelope_end = record.byte_range.start + 6;
            // Envelope semantics (type code + bytes_to_follow) are
            // corroborated by the cross-fixture `record_count - 2`
            // invariant; payload fields remain unnamed.
            trace.consume(
                ByteRange::new(record.byte_range.start as u64, envelope_end as u64),
                TraceConfidence::Decoded,
            );
            trace.consume(
                ByteRange::new(envelope_end as u64, record.byte_range.end as u64),
                TraceConfidence::Probed,
            );
        }
    }
    Some(header)
}

// Phase 29 Slice B: audit-only `/PSMcluster0` body record-chain walker.
//
// Cross-fixture triage
// (`docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`)
// proved the post-string-table body is a single continuous chain of
// PSM-style records (6-byte envelope + payload) preceded by a fixed
// 10-byte prologue, with `chain_records == header.record_count - 2` on
// all 6 local fixtures. The walker below follows the Phase 18
// audit-only template: envelope decoded, payload raw, no semantic type
// names, no `PidGraphicKind` emission. PSMcluster0 type codes must NOT
// be mapped through the Sheet ig* table without IDA confirmation of a
// shared namespace.

/// Fixed length of the unexplained prologue between the string-table
/// sentinel and the first `/PSMcluster0` body-chain record. Identical
/// 10-byte sequence observed on all 6 local fixtures (Phase 29 Slice B
/// triage); semantics unknown, traced as `Probed`.
pub const PSM_CLUSTER0_PROLOGUE_LEN: usize = 10;

/// Upper bound on `bytes_to_follow` accepted for one `/PSMcluster0`
/// body-chain record. Mirrors the Sheet-family record cap; rejects
/// wide-scan false positives whose length field reads as a giant value.
pub const CLUSTER_BODY_RECORD_MAX_BYTES_TO_FOLLOW: u32 = 100_000;

/// One audit-only record from the `/PSMcluster0` post-string-table body
/// chain (Phase 29 Slice B).
///
/// Exposes the 6-byte PSM-style envelope (`type_code`, `type_flags`,
/// `bytes_to_follow`) and the raw payload, with full byte provenance.
/// **No semantic type name is claimed for any code**: whether the
/// `PSMcluster0` record namespace matches the Sheet/IGDS table is an
/// open IDA question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterBodyRecordDecoded {
    /// Byte range covering the record (6-byte envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM-style 14-bit type code (`type_word & 0x3FFF`). Never 0.
    pub type_code: u16,
    /// Top 2 bits of the type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the envelope. Equals `raw_payload.len()`
    /// by construction.
    pub bytes_to_follow: u32,
    /// Raw payload bytes (audit-only; field semantics deferred until
    /// IDA names the record types).
    pub raw_payload: Vec<u8>,
}

/// Try to decode a single `/PSMcluster0` body-chain record at `offset`.
///
/// Validation rules: 6-byte envelope must fit, `type_code != 0`,
/// `bytes_to_follow <=` [`CLUSTER_BODY_RECORD_MAX_BYTES_TO_FOLLOW`],
/// and the payload must fit inside `data`. Bounds-checked and
/// panic-free: out-of-range offsets return `None`.
pub fn decode_cluster_body_record_at(
    data: &[u8],
    offset: usize,
) -> Option<ClusterBodyRecordDecoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code == 0 {
        return None;
    }
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if bytes_to_follow > CLUSTER_BODY_RECORD_MAX_BYTES_TO_FOLLOW {
        return None;
    }
    let payload_end = header_end.checked_add(bytes_to_follow as usize)?;
    if payload_end > data.len() {
        return None;
    }
    let raw_payload = data.get(header_end..payload_end)?.to_vec();
    Some(ClusterBodyRecordDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags: type_word >> 14,
        bytes_to_follow,
        raw_payload,
    })
}

/// Strictly walk a `/PSMcluster0` body record chain starting at `start`.
///
/// No resync: the walk stops at the first offset that fails
/// [`decode_cluster_body_record_at`] validation (zero type code,
/// oversized `bytes_to_follow`, truncated envelope or payload). The
/// triage evidence shows real fixture bodies form one such chain from
/// prologue end to end-of-stream.
pub fn decode_cluster_body_records(data: &[u8], start: usize) -> Vec<ClusterBodyRecordDecoded> {
    let mut out = Vec::new();
    let mut cursor = start;
    while let Some(record) = decode_cluster_body_record_at(data, cursor) {
        cursor = record.byte_range.end;
        out.push(record);
    }
    out
}

/// Decode the `/PSMcluster0` body record chain behind the header,
/// locator gap, string table, and fixed prologue.
///
/// Applies the **full-coverage gate**: the chain is returned only when
/// at least one record decodes at `table_end +`
/// [`PSM_CLUSTER0_PROLOGUE_LEN`] and the chain runs exactly to the end
/// of the stream. Anything else returns an empty vector so callers
/// (and the byte-audit trace) never claim a partial, unproven walk.
pub fn decode_psm_cluster0_body_records(data: &[u8]) -> Vec<ClusterBodyRecordDecoded> {
    if parse_header(data).is_none() || data.len() <= 32 {
        return Vec::new();
    }
    let table_start = find_string_table_start(data);
    let (_, table_end) = parse_string_table(data, table_start);
    psm_cluster0_chain_after_table(data, table_end)
        .map(|(_, records)| records)
        .unwrap_or_default()
}

/// Shared core for the post-table walk: compute the prologue range and
/// run the full-coverage gate described on
/// [`decode_psm_cluster0_body_records`].
fn psm_cluster0_chain_after_table(
    data: &[u8],
    table_end: usize,
) -> Option<(std::ops::Range<usize>, Vec<ClusterBodyRecordDecoded>)> {
    let chain_start = table_end.checked_add(PSM_CLUSTER0_PROLOGUE_LEN)?;
    if chain_start >= data.len() {
        return None;
    }
    let records = decode_cluster_body_records(data, chain_start);
    let last = records.last()?;
    if last.byte_range.end != data.len() {
        return None;
    }
    Some((table_end..chain_start, records))
}

// Phase 29 Slice B follow-up: audit-only `/StyleCluster` body record-chain
// walker.
//
// The same probe that triaged `/PSMcluster0`
// (`examples/probe_phase29_psmcluster0_body_triage.rs`, run with the
// `StyleCluster` argument) shows every local fixture's `/StyleCluster`
// stream is: 16-byte cluster header (`stream_type = 0x005A`,
// `flags = 0x2000`) + a variable-length unparsed prefix (GUID-table-like:
// ten zero bytes, a `u16` count, then consecutive 16-byte CLSID-shaped
// entries, plus further uncharacterized structure) + a single PSM-style
// record chain that runs exactly to end-of-stream with zero resync.
// Unlike `/PSMcluster0`, `header.record_count` matches the chain length
// on only 2 of 6 fixtures, so record envelopes here stay `Probed`
// (audit-only), not `Decoded`. The prefix is intentionally left as
// leftover until a dedicated slice characterizes it.

/// Minimum number of records a candidate `/StyleCluster` body chain
/// must contain before the walker commits to it. Guards the
/// earliest-offset locator against committing to a short coincidental
/// tail chain.
pub const STYLE_CLUSTER_CHAIN_MIN_RECORDS: usize = 3;

/// Decode the `/StyleCluster` body record chain.
///
/// Locator + gate: starting right after the 16-byte cluster header,
/// find the **earliest** offset from which a strict
/// [`decode_cluster_body_records`] chain of at least
/// [`STYLE_CLUSTER_CHAIN_MIN_RECORDS`] records runs **exactly** to
/// end-of-stream. Returns that chain, or an empty vector when no offset
/// qualifies (no partial claims). The unparsed prefix between the
/// header and the chain start is not described by this function.
pub fn decode_style_cluster_body_records(data: &[u8]) -> Vec<ClusterBodyRecordDecoded> {
    if parse_header(data).is_none() {
        return Vec::new();
    }
    style_cluster_chain(data)
        .map(|(_, records)| records)
        .unwrap_or_default()
}

/// Trace-aware `/StyleCluster` walker: 16-byte header (`Decoded`) plus
/// the body record chain from [`decode_style_cluster_body_records`],
/// claimed entirely as `TraceConfidence::Probed` (byte layout isolated,
/// record semantics unnamed). The prefix between header and chain start
/// stays leftover so the byte-audit keeps flagging it for a future
/// slice.
pub fn parse_style_cluster_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<ClusterHeader> {
    let header = parse_header_with_trace(data, trace)?;
    if let Some((_, records)) = style_cluster_chain(data) {
        for record in &records {
            trace.consume(
                ByteRange::new(record.byte_range.start as u64, record.byte_range.end as u64),
                TraceConfidence::Probed,
            );
        }
    }
    Some(header)
}

// Phase 29 Slice C: audit-only `/Unclustered Dynamic Attributes` body
// record-chain walker.
//
// Cross-fixture triage
// (`docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`)
// proved the DA stream is a minimal cluster-family member: an 8-byte
// prologue (the cluster magic plus a u32 record counter) followed by a
// single end-anchored chain of PSM-style records, every one masking to
// type code `0x0089`, on all 6 local fixtures. The 31-byte "record
// trailer" decoded by `dynamic_attr_records::extract_record_trailers`
// is byte-identical to the next record's envelope head, so this walker
// coexists with (and never replaces) the landmark scanner. The prologue
// counter equals the strict chain length on only 5 of 6 fixtures (one
// fixture carries a flagged head whose literal bytes are not
// `89 00`), so the counter is reported by probes but never gated on,
// and every claim stays `Probed` (StyleCluster precedent).

/// Fixed length of the `/Unclustered Dynamic Attributes` stream
/// prologue: the cluster-family magic [`CLUSTER_MAGIC`] plus a `u32`
/// record counter. Identical shape observed on all 6 local fixtures
/// (Phase 29 Slice C triage); counter semantics stay unnamed.
pub const UNCLUSTERED_DA_PROLOGUE_LEN: usize = 8;

/// Decode the `/Unclustered Dynamic Attributes` body record chain
/// behind the 8-byte prologue.
///
/// Applies the **full-coverage gate**: the chain is returned only when
/// the stream opens with [`CLUSTER_MAGIC`], at least one record decodes
/// at [`UNCLUSTERED_DA_PROLOGUE_LEN`], and the chain runs exactly to
/// the end of the stream. Anything else returns an empty vector so
/// callers (and the byte-audit trace) never claim a partial, unproven
/// walk. The prologue counter is intentionally **not** validated
/// against the chain length — it mismatches the strict chain on one
/// local fixture and its semantics are unnamed.
pub fn decode_unclustered_da_body_records(data: &[u8]) -> Vec<ClusterBodyRecordDecoded> {
    if data.len() < UNCLUSTERED_DA_PROLOGUE_LEN {
        return Vec::new();
    }
    if u32_le(data, 0) != CLUSTER_MAGIC {
        return Vec::new();
    }
    let records = decode_cluster_body_records(data, UNCLUSTERED_DA_PROLOGUE_LEN);
    match records.last() {
        Some(last) if last.byte_range.end == data.len() => records,
        _ => Vec::new(),
    }
}

/// Trace-aware `/Unclustered Dynamic Attributes` walker: the 8-byte
/// prologue plus every body record from
/// [`decode_unclustered_da_body_records`], all claimed as
/// [`TraceConfidence::Probed`] (byte layout proven end-anchored,
/// record semantics unnamed). When the full-coverage gate fails the
/// builder is left untouched — the landmark scanner
/// (`dynamic_attr_records::scan_da_landmarks_with_trace`) remains the
/// only claim source in that case.
///
/// Returns the number of chain records claimed — useful for unit
/// assertions.
pub fn parse_unclustered_da_with_trace(data: &[u8], trace: &mut ParserTraceBuilder) -> usize {
    let records = decode_unclustered_da_body_records(data);
    if records.is_empty() {
        return 0;
    }
    trace.consume(
        ByteRange::new(0, UNCLUSTERED_DA_PROLOGUE_LEN as u64),
        TraceConfidence::Probed,
    );
    for record in &records {
        trace.consume(
            ByteRange::new(record.byte_range.start as u64, record.byte_range.end as u64),
            TraceConfidence::Probed,
        );
    }
    records.len()
}

/// Earliest-offset chain locator shared by the `/StyleCluster` entry
/// points; see [`decode_style_cluster_body_records`] for the gate.
fn style_cluster_chain(data: &[u8]) -> Option<(usize, Vec<ClusterBodyRecordDecoded>)> {
    for start in 16..data.len() {
        // Cheap pre-filter: a chain can only start where one record
        // decodes; skip offsets that fail immediately.
        let Some(first) = decode_cluster_body_record_at(data, start) else {
            continue;
        };
        if first.byte_range.end == data.len() {
            // Single record to end-of-stream: below the minimum chain
            // length, keep scanning.
            continue;
        }
        let records = decode_cluster_body_records(data, start);
        if records.len() >= STYLE_CLUSTER_CHAIN_MIN_RECORDS
            && records.last().map(|r| r.byte_range.end) == Some(data.len())
        {
            return Some((start, records));
        }
    }
    None
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

    fn make_record(type_word: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&type_word.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    /// Header + locator gap + 2-entry string table + sentinel, the same
    /// construction `trace_aware_psm_cluster0_marks_locator_prefix_probed`
    /// uses; returns the buffer and the table-end offset.
    fn make_psm_cluster0_prefix() -> (Vec<u8>, usize) {
        let mut data = make_header(0, 0x75, 113, 1);
        data.extend_from_slice(&[0u8; 16]); // locator gap
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("AB"));
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend(utf16le("CD"));
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        let table_end = data.len();
        (data, table_end)
    }

    #[test]
    fn psm_cluster0_record_at_decodes_canonical_record() {
        let data = make_record(0x4089, &[0xAA, 0xBB, 0xCC]);
        let record = decode_cluster_body_record_at(&data, 0).expect("valid record");
        assert_eq!(record.byte_range, 0..9);
        assert_eq!(record.type_code, 0x0089);
        assert_eq!(record.type_flags, 0x1);
        assert_eq!(record.bytes_to_follow, 3);
        assert_eq!(record.raw_payload, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn psm_cluster0_record_at_rejects_zero_type_code() {
        // type_word 0x4000 has flags but a zero 14-bit type code.
        let data = make_record(0x4000, &[0xAA]);
        assert!(decode_cluster_body_record_at(&data, 0).is_none());
    }

    #[test]
    fn psm_cluster0_record_at_rejects_oversized_bytes_to_follow() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x0089u16.to_le_bytes());
        data.extend_from_slice(&(CLUSTER_BODY_RECORD_MAX_BYTES_TO_FOLLOW + 1).to_le_bytes());
        data.extend_from_slice(&[0u8; 8]);
        assert!(decode_cluster_body_record_at(&data, 0).is_none());
    }

    #[test]
    fn psm_cluster0_record_at_rejects_truncated_payload_and_header() {
        let data = make_record(0x0089, &[0xAA, 0xBB, 0xCC, 0xDD]);
        // Claimed 4 payload bytes, only 2 present.
        assert!(decode_cluster_body_record_at(&data[..8], 0).is_none());
        // Truncated header.
        assert!(decode_cluster_body_record_at(&data[..5], 0).is_none());
        // Out-of-range offsets are bounds-safe.
        assert!(decode_cluster_body_record_at(&data, data.len()).is_none());
        assert!(decode_cluster_body_record_at(&data, usize::MAX).is_none());
    }

    #[test]
    fn psm_cluster0_records_walks_chain_and_stops_at_first_invalid() {
        let mut data = make_record(0x0002, &[0x01; 4]);
        data.extend(make_record(0x0089, &[0x02; 7]));
        data.extend(make_record(0x0003, &[]));
        let chain_end = data.len();
        // Trailing bytes that do not parse as a record (zero type code).
        data.extend_from_slice(&[0u8; 6]);

        let records = decode_cluster_body_records(&data, 0);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].type_code, 0x0002);
        assert_eq!(records[1].type_code, 0x0089);
        assert_eq!(records[2].type_code, 0x0003);
        assert_eq!(records[2].byte_range.end, chain_end);
    }

    #[test]
    fn psm_cluster0_body_records_applies_full_coverage_gate() {
        let (mut data, table_end) = make_psm_cluster0_prefix();
        data.extend_from_slice(&[0u8; PSM_CLUSTER0_PROLOGUE_LEN]);
        data.extend(make_record(0x0002, &[0x01; 4]));
        data.extend(make_record(0x0089, &[0x02; 7]));

        let records = decode_psm_cluster0_body_records(&data);
        assert_eq!(records.len(), 2, "chain covering stream end must decode");
        assert_eq!(
            records[0].byte_range.start,
            table_end + PSM_CLUSTER0_PROLOGUE_LEN
        );
        assert_eq!(records[1].byte_range.end, data.len());

        // Trailing garbage breaks the full-coverage gate: no partial claim.
        let mut with_garbage = data.clone();
        with_garbage.extend_from_slice(&[0u8; 3]);
        assert!(decode_psm_cluster0_body_records(&with_garbage).is_empty());

        // Headerless data never walks.
        assert!(decode_psm_cluster0_body_records(&data[16..]).is_empty());
    }

    #[test]
    fn trace_aware_psm_cluster0_walks_body_chain_when_it_covers_stream() {
        let (mut data, table_end) = make_psm_cluster0_prefix();
        data.extend_from_slice(&[0u8; PSM_CLUSTER0_PROLOGUE_LEN]);
        data.extend(make_record(0x0002, &[0x01; 4]));
        data.extend(make_record(0x0089, &[0x02; 7]));

        let mut b = ParserTraceBuilder::new("parse_psm_cluster0");
        let header = parse_psm_cluster0_with_trace(&data, &mut b).expect("valid");
        assert_eq!(header.stream_type, 0x75);
        let trace = b.build("/PSMcluster0", data.len() as u64);

        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "prologue + chain must cover the whole stream"
        );
        assert!(trace.leftover_ranges.is_empty());

        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        let prologue_range = ByteRange::new(
            table_end as u64,
            (table_end + PSM_CLUSTER0_PROLOGUE_LEN) as u64,
        );
        assert!(
            probed
                .iter()
                .any(|r| r.start <= prologue_range.start && prologue_range.end <= r.end),
            "prologue must be claimed as Probed: {probed:?}"
        );

        let decoded = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Decoded)
            .cloned()
            .unwrap_or_default();
        let first_envelope_start = (table_end + PSM_CLUSTER0_PROLOGUE_LEN) as u64;
        assert!(
            decoded
                .iter()
                .any(|r| r.start <= first_envelope_start && first_envelope_start + 6 <= r.end),
            "record envelopes must be claimed as Decoded: {decoded:?}"
        );
    }

    #[test]
    fn trace_aware_psm_cluster0_leaves_body_leftover_when_gate_fails() {
        let (mut data, table_end) = make_psm_cluster0_prefix();
        data.extend_from_slice(&[0u8; PSM_CLUSTER0_PROLOGUE_LEN]);
        data.extend(make_record(0x0002, &[0x01; 4]));
        // Trailing garbage after the chain breaks the full-coverage gate.
        data.extend_from_slice(&[0u8; 5]);

        let mut b = ParserTraceBuilder::new("parse_psm_cluster0");
        let _ = parse_psm_cluster0_with_trace(&data, &mut b).expect("valid header");
        let trace = b.build("/PSMcluster0", data.len() as u64);

        let post_table = (data.len() - table_end) as u64;
        assert_eq!(
            trace.leftover_bytes(),
            post_table,
            "gate failure must leave the whole post-table region leftover"
        );
    }

    /// Header + uncharacterized prefix + a 3-record chain to stream end,
    /// mirroring the fixture `/StyleCluster` shape. Returns the buffer
    /// and the chain start offset.
    fn make_style_cluster_stream() -> (Vec<u8>, usize) {
        let mut data = make_header(47, 0x5A, 2368, 0x2000);
        data.extend_from_slice(&[0u8; 10]); // zero prologue
        data.extend_from_slice(&13u16.to_le_bytes());
        data.extend_from_slice(&[0xAB; 32]); // GUID-table-like prefix bytes
        let chain_start = data.len();
        data.extend(make_record(0x002C, &[0x01; 5]));
        data.extend(make_record(0x002D, &[0x02; 9]));
        data.extend(make_record(0x002E, &[0x03; 2]));
        (data, chain_start)
    }

    #[test]
    fn style_cluster_body_records_finds_earliest_chain_to_stream_end() {
        let (data, chain_start) = make_style_cluster_stream();
        let records = decode_style_cluster_body_records(&data);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].byte_range.start, chain_start);
        assert_eq!(records[0].type_code, 0x002C);
        assert_eq!(records[2].byte_range.end, data.len());
    }

    #[test]
    fn style_cluster_body_records_rejects_chain_not_reaching_stream_end() {
        let (mut data, _) = make_style_cluster_stream();
        // Trailing garbage detaches the chain from end-of-stream.
        data.extend_from_slice(&[0u8; 4]);
        assert!(decode_style_cluster_body_records(&data).is_empty());
    }

    #[test]
    fn style_cluster_body_records_rejects_chain_below_min_records() {
        let mut data = make_header(2, 0x5A, 0, 0x2000);
        data.extend_from_slice(&[0u8; 12]); // prefix
        data.extend(make_record(0x002C, &[0x01; 5]));
        data.extend(make_record(0x002D, &[0x02; 3]));
        assert!(
            decode_style_cluster_body_records(&data).is_empty(),
            "2-record chain is below STYLE_CLUSTER_CHAIN_MIN_RECORDS"
        );
    }

    #[test]
    fn style_cluster_body_records_rejects_headerless_stream() {
        let (data, _) = make_style_cluster_stream();
        assert!(decode_style_cluster_body_records(&data[16..]).is_empty());
    }

    #[test]
    fn trace_aware_style_cluster_claims_chain_probed_and_leaves_prefix_leftover() {
        let (data, chain_start) = make_style_cluster_stream();
        let mut b = ParserTraceBuilder::new("parse_style_cluster");
        let header = parse_style_cluster_with_trace(&data, &mut b).expect("valid header");
        assert_eq!(header.stream_type, 0x5A);
        let trace = b.build("/StyleCluster", data.len() as u64);

        // Header (16, Decoded) + chain (Probed); prefix stays leftover.
        let chain_bytes = (data.len() - chain_start) as u64;
        assert_eq!(trace.consumed_bytes(), 16 + chain_bytes);
        assert_eq!(trace.leftover_bytes(), (chain_start - 16) as u64);

        let probed = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default();
        let probed_total: u64 = probed.iter().map(ByteRange::len).sum();
        assert_eq!(
            probed_total, chain_bytes,
            "every chain record must be claimed as Probed: {probed:?}"
        );
    }

    #[test]
    fn trace_aware_style_cluster_claims_only_header_when_no_chain_qualifies() {
        let mut data = make_header(0, 0x5A, 0, 0x2000);
        data.extend_from_slice(&[0u8; 64]); // body that never chains
        let mut b = ParserTraceBuilder::new("parse_style_cluster");
        let _ = parse_style_cluster_with_trace(&data, &mut b).expect("valid header");
        let trace = b.build("/StyleCluster", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 16);
        assert_eq!(trace.leftover_bytes(), 64);
    }

    /// 8-byte DA prologue (magic + counter) followed by the given
    /// records, mirroring the fixture `/Unclustered Dynamic Attributes`
    /// shape.
    fn make_unclustered_da_stream(counter: u32, payloads: &[(u16, &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&CLUSTER_MAGIC.to_le_bytes());
        data.extend_from_slice(&counter.to_le_bytes());
        for (type_word, payload) in payloads {
            data.extend(make_record(*type_word, payload));
        }
        data
    }

    #[test]
    fn unclustered_da_body_records_decodes_end_anchored_chain() {
        let data = make_unclustered_da_stream(2, &[(0x0089, &[0x01; 40]), (0x0089, &[0x02; 7])]);
        let records = decode_unclustered_da_body_records(&data);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].byte_range.start, UNCLUSTERED_DA_PROLOGUE_LEN);
        assert_eq!(records[0].type_code, 0x0089);
        assert_eq!(records[1].byte_range.end, data.len());
    }

    #[test]
    fn unclustered_da_body_records_ignores_counter_mismatch() {
        // Counter says 99, chain has 2 records: the counter is reported
        // by probes but never gated on (it mismatches the strict chain
        // on one real fixture).
        let data = make_unclustered_da_stream(99, &[(0x0089, &[0x01; 4]), (0x4089, &[0x02; 3])]);
        let records = decode_unclustered_da_body_records(&data);
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].type_code, 0x0089, "flagged head masks to 0x0089");
        assert_eq!(records[1].type_flags, 0x1);
    }

    #[test]
    fn unclustered_da_body_records_rejects_wrong_magic() {
        let mut data = make_unclustered_da_stream(1, &[(0x0089, &[0x01; 4])]);
        data[0] ^= 0xFF;
        assert!(decode_unclustered_da_body_records(&data).is_empty());
    }

    #[test]
    fn unclustered_da_body_records_rejects_non_end_anchored_chain() {
        let mut data = make_unclustered_da_stream(1, &[(0x0089, &[0x01; 4])]);
        // Trailing garbage that does not parse as a record (zero type
        // code) breaks the full-coverage gate: no partial claim.
        data.extend_from_slice(&[0u8; 3]);
        assert!(decode_unclustered_da_body_records(&data).is_empty());
    }

    #[test]
    fn unclustered_da_body_records_rejects_truncated_or_empty_stream() {
        assert!(decode_unclustered_da_body_records(&[]).is_empty());
        let data = make_unclustered_da_stream(1, &[(0x0089, &[0x01; 4])]);
        assert!(
            decode_unclustered_da_body_records(&data[..7]).is_empty(),
            "stream shorter than the prologue must not walk"
        );
        assert!(
            decode_unclustered_da_body_records(&data[..UNCLUSTERED_DA_PROLOGUE_LEN]).is_empty(),
            "prologue-only stream has no records and must not claim"
        );
    }

    #[test]
    fn trace_aware_unclustered_da_claims_prologue_and_chain_probed() {
        let data = make_unclustered_da_stream(2, &[(0x0089, &[0x01; 40]), (0x0089, &[0x02; 7])]);
        let mut b = ParserTraceBuilder::new("parse_unclustered_da");
        let claimed = parse_unclustered_da_with_trace(&data, &mut b);
        assert_eq!(claimed, 2);
        let trace = b.build("/Unclustered Dynamic Attributes", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "prologue + chain must cover the whole stream"
        );
        assert!(trace.leftover_ranges.is_empty());
        let probed_total: u64 = trace
            .ranges_by_confidence
            .get(&TraceConfidence::Probed)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(ByteRange::len)
            .sum();
        assert_eq!(
            probed_total,
            data.len() as u64,
            "every claim must be Probed; no Decoded promotion"
        );
    }

    #[test]
    fn trace_aware_unclustered_da_claims_nothing_when_gate_fails() {
        let mut data = make_unclustered_da_stream(1, &[(0x0089, &[0x01; 4])]);
        data.extend_from_slice(&[0u8; 3]);
        let mut b = ParserTraceBuilder::new("parse_unclustered_da");
        let claimed = parse_unclustered_da_with_trace(&data, &mut b);
        assert_eq!(claimed, 0);
        let trace = b.build("/Unclustered Dynamic Attributes", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 0);
        assert_eq!(trace.leftover_bytes(), data.len() as u64);
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
