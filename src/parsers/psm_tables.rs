//! Parsers for the PSM index streams: `PSMroots`, `PSMclustertable`,
//! `PSMsegmenttable`. These small top-level streams index the document's
//! internal storage layout.
//!
//! Byte layouts (observed from reverse engineering real `.pid` files):
//!
//! * `PSMroots`
//!   - `[u32 magic 'root']`
//!   - N records of `[u32 id][u32 char_count][UTF-16LE name (char_count chars)]`
//!   - Trailing 4-byte sentinel `00 00 00 00` (or zero bytes if unused)
//!
//! * `PSMclustertable`
//!   - `[u32 magic 'clst']`
//!   - `[u32 count]`
//!   - Per entry: variable-size record ending with a UTF-16LE name. Internal
//!     header fields (length/flags/cluster-index) are not yet fully
//!     understood, so the parser extracts names by scanning UTF-16LE ASCII
//!     runs ≥ 4 chars.
//!
//! * `PSMsegmenttable`
//!   - `[u32 magic 'stab']`
//!   - `[u32 count]`
//!   - `[u8 × count]` per-segment flag bytes (observed: all `0x01`).

use crate::byte_audit::{ByteRange, ParserTraceBuilder, TraceConfidence};
use crate::model::{
    PsmClusterEntry, PsmClusterRecordProbe, PsmClusterTable, PsmRootEntry, PsmRoots,
    PsmSegmentEntry, PsmSegmentRecordProbe, PsmSegmentTable,
};

pub const ROOT_MAGIC: u32 = 0x746F_6F72; // 'root' (LE bytes: 72 6F 6F 74)
pub const CLST_MAGIC: u32 = 0x7473_6C63; // 'clst'
pub const STAB_MAGIC: u32 = 0x6261_7473; // 'stab'

fn read_u32_le(data: &[u8], pos: usize) -> Option<u32> {
    if pos + 4 > data.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        data[pos],
        data[pos + 1],
        data[pos + 2],
        data[pos + 3],
    ]))
}

fn read_utf16le_name(data: &[u8], start: usize, char_count: usize) -> Option<String> {
    let byte_len = char_count.checked_mul(2)?;
    let end = start.checked_add(byte_len)?;
    if end > data.len() {
        return None;
    }
    let words: Vec<u16> = data[start..end]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(String::from_utf16_lossy(&words))
}

/// Parse `PSMroots`. Returns `None` if the magic does not match.
pub fn parse_psm_roots(data: &[u8]) -> Option<PsmRoots> {
    let magic = read_u32_le(data, 0)?;
    if magic != ROOT_MAGIC {
        return None;
    }
    let mut entries = Vec::new();
    let mut pos = 4usize;
    while pos + 8 <= data.len() {
        let id = read_u32_le(data, pos)?;
        let cc = read_u32_le(data, pos + 4)? as usize;
        if cc == 0 && id == 0 {
            // likely sentinel
            break;
        }
        if cc > 512 {
            // implausible — stop to avoid infinite loops
            break;
        }
        let name_start = pos + 8;
        let Some(name) = read_utf16le_name(data, name_start, cc) else {
            break;
        };
        entries.push(PsmRootEntry {
            id,
            offset: pos,
            name,
        });
        pos = name_start + cc * 2;
    }
    Some(PsmRoots {
        size: data.len() as u64,
        entries,
        trailing_bytes: data.len().saturating_sub(pos),
    })
}

/// Parse `PSMclustertable`. Extracts the canonical list of cluster names
/// with per-record structural metadata.
///
/// Strategy: walk byte-by-byte from offset 8 (after magic + count header).
/// Each record is bounded by finding a UTF-16LE ASCII name run (≥ 4 chars).
/// Bytes between the previous record end and the name start are captured
/// as `prefix_bytes` for audit. The name run plus any trailing null
/// terminator complete the record.
pub fn parse_psm_cluster_table(data: &[u8]) -> Option<PsmClusterTable> {
    let magic = read_u32_le(data, 0)?;
    if magic != CLST_MAGIC {
        return None;
    }
    let count = read_u32_le(data, 4)?;
    let mut entries = Vec::new();
    let mut i = 8usize;
    let mut record_start = i;
    while i + 2 <= data.len() {
        if is_ascii_utf16le(data, i) {
            let name_start = i;
            let mut name = String::new();
            while i + 2 <= data.len() && is_ascii_utf16le(data, i) {
                name.push(data[i] as char);
                i += 2;
            }
            // Skip trailing null terminator if present
            if i + 2 <= data.len() && data[i] == 0 && data[i + 1] == 0 {
                i += 2;
            }
            if name.len() >= 4 {
                let prefix = data[record_start..name_start].to_vec();
                let record_len = i - record_start;
                let probe = build_cluster_record_probe(&data[record_start..i], &prefix, &name);
                entries.push(PsmClusterEntry {
                    name,
                    name_offset: name_start,
                    record_offset: record_start,
                    record_len,
                    prefix_bytes: prefix,
                    probe: Some(probe),
                });
                record_start = i;
            }
        } else {
            i += 1;
        }
    }
    let trailing_bytes = data.len().saturating_sub(record_start);
    Some(PsmClusterTable {
        size: data.len() as u64,
        count,
        entries,
        trailing_bytes,
    })
}

/// Parse `PSMsegmenttable`.
///
/// Layout: `[u32 magic 'stab'][u32 count][u8 × count flags]`. Returns `None`
/// if the magic/size are inconsistent.
///
/// This is a thin back-compat wrapper around
/// [`parse_psm_segment_table_with_trace`] — callers that do not need
/// byte-level coverage tracing can keep using this entry point
/// unchanged. The trace is silently discarded.
pub fn parse_psm_segment_table(data: &[u8]) -> Option<PsmSegmentTable> {
    let mut trace = ParserTraceBuilder::new("parse_psm_segment_table");
    parse_psm_segment_table_with_trace(data, &mut trace)
}

/// Phase 12b-1 trace-aware variant of [`parse_psm_segment_table`].
///
/// Every byte this parser consumes is reported to `trace`:
/// - `[0..4]` — `stab` magic — `TraceConfidence::Decoded`
/// - `[4..8]` — `count` u32 LE — `TraceConfidence::Decoded`
/// - `[8 + i .. 8 + i + 1]` for `i ∈ 0..count` — individual flag bytes,
///   marked `TraceConfidence::Probed` since their field semantics are
///   still being reverse-engineered (see Phase 11b-probe).
///
/// `trailing_bytes` past the flag table are **not** consumed — they
/// surface in the resulting `ParserTrace::leftover_ranges`.
pub fn parse_psm_segment_table_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<PsmSegmentTable> {
    let magic = read_u32_le(data, 0)?;
    if magic != STAB_MAGIC {
        return None;
    }
    trace.consume(ByteRange::new(0, 4), TraceConfidence::Decoded);
    let count = read_u32_le(data, 4)?;
    trace.consume(ByteRange::new(4, 8), TraceConfidence::Decoded);

    let flags_start = 8usize;
    let flags_end = flags_start.checked_add(count as usize)?;
    if flags_end > data.len() {
        return None;
    }
    let flags: Vec<u8> = data[flags_start..flags_end].to_vec();
    let entries: Vec<PsmSegmentEntry> = flags
        .iter()
        .enumerate()
        .map(|(i, &flag)| {
            let offset = flags_start + i;
            let off64 = offset as u64;
            trace.consume(
                ByteRange::new(off64, off64 + 1),
                TraceConfidence::Probed,
            );
            PsmSegmentEntry {
                index: i,
                offset,
                flag,
                probe: Some(build_segment_record_probe(data, offset, flag)),
            }
        })
        .collect();
    let trailing_bytes = data.len().saturating_sub(flags_end);
    Some(PsmSegmentTable {
        size: data.len() as u64,
        count,
        flags,
        entries,
        trailing_bytes,
    })
}

/// Detect a 2-byte UTF-16LE code unit encoding a printable ASCII character.
fn is_ascii_utf16le(data: &[u8], i: usize) -> bool {
    i + 1 < data.len() && (0x20..=0x7e).contains(&data[i]) && data[i + 1] == 0
}

/// Phase 11b-probe: **post-parse** step that backfills
/// [`PsmSegmentRecordProbe::owner_cluster_hint`] on every entry of
/// `segment_table` when a positional 1:1 mapping with `cluster_table` is
/// the most natural guess (`segment_table.entries.len() ==
/// cluster_table.entries.len()`).
///
/// When the counts disagree (or the cluster table is missing), all hints
/// stay `None` — deliberately conservative to avoid over-claiming
/// semantics before a second fixture arrives.
///
/// This helper lives here rather than in `streams::psm_tables` so it is
/// unit-testable without needing a full CFB fixture.
pub fn apply_segment_owner_hints(
    segment_table: &mut PsmSegmentTable,
    cluster_table: Option<&PsmClusterTable>,
) {
    let Some(ct) = cluster_table else {
        return;
    };
    if segment_table.entries.len() != ct.entries.len() {
        return;
    }
    for (segment, cluster) in segment_table.entries.iter_mut().zip(ct.entries.iter()) {
        if let Some(probe) = segment.probe.as_mut() {
            probe.owner_cluster_hint = Some(cluster.name.clone());
        }
    }
}

/// Phase 11b-probe: derive a [`PsmSegmentRecordProbe`] from a single flag
/// byte plus the raw `PSMsegmenttable` stream. Pure computation — the
/// `owner_cluster_hint` slot is left as `None` here; the dispatcher in
/// `streams::psm_tables` fills it once the cluster table context is in
/// scope.
fn build_segment_record_probe(stream: &[u8], offset: usize, flag: u8) -> PsmSegmentRecordProbe {
    let flag_hex = format!("{flag:02X}");
    let window_lo = offset.saturating_sub(3);
    let window_hi = stream.len().min(offset.saturating_add(4)); // +4 = inclusive-3 exclusive
    let neighbor_window_hex = if window_lo < window_hi {
        stream[window_lo..window_hi]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };
    PsmSegmentRecordProbe {
        flag_hex,
        neighbor_window_hex,
        stream_offset: offset,
        owner_cluster_hint: None,
    }
}

/// Phase 11a-probe: derive a [`PsmClusterRecordProbe`] from a cluster
/// record's raw bytes. Pure computation, no semantic claims.
fn build_cluster_record_probe(record: &[u8], prefix: &[u8], name: &str) -> PsmClusterRecordProbe {
    let first_u32_le = if prefix.len() >= 4 {
        read_u32_le(prefix, 0)
    } else {
        None
    };
    let last_u32_le = if record.len() >= 4 {
        read_u32_le(record, record.len() - 4)
    } else {
        None
    };
    let prefix_hex = prefix
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    let trailer_start = record.len().saturating_sub(8);
    let trailer_hex = record[trailer_start..]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    PsmClusterRecordProbe {
        first_u32_le,
        last_u32_le,
        prefix_hex,
        trailer_hex,
        name_char_count: name.chars().count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|w| w.to_le_bytes()).collect()
    }

    fn make_root_entry(id: u32, name: &str) -> Vec<u8> {
        let chars: Vec<u16> = name.encode_utf16().collect();
        let cc = chars.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&cc.to_le_bytes());
        buf.extend(utf16_bytes(name));
        buf
    }

    #[test]
    fn roots_two_entries() {
        let mut data = Vec::new();
        data.extend_from_slice(&ROOT_MAGIC.to_le_bytes());
        data.extend(make_root_entry(0x18C, "Imagineer Document"));
        data.extend(make_root_entry(0x149, "Server Document"));
        let r = parse_psm_roots(&data).expect("valid");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].id, 0x18C);
        assert_eq!(r.entries[0].name, "Imagineer Document");
        assert_eq!(r.entries[1].id, 0x149);
        assert_eq!(r.entries[1].name, "Server Document");
        assert_eq!(r.trailing_bytes, 0);
    }

    #[test]
    fn roots_wrong_magic() {
        let data = [0, 0, 0, 0, 1, 2, 3, 4];
        assert!(parse_psm_roots(&data).is_none());
    }

    #[test]
    fn roots_stops_on_sentinel() {
        let mut data = Vec::new();
        data.extend_from_slice(&ROOT_MAGIC.to_le_bytes());
        data.extend(make_root_entry(1, "Test"));
        // sentinel id=0 cc=0
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&[0xDE, 0xAD]); // trailing garbage

        let r = parse_psm_roots(&data).expect("valid");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].name, "Test");
        assert_eq!(r.trailing_bytes, 10); // 8 sentinel + 2 trailing
    }

    #[test]
    fn cluster_table_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes()); // count
        let prefix1 = [0x01, 0x00, 0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&prefix1); // filler before name
        let name1_offset = data.len();
        data.extend(utf16_bytes("PSMcluster0"));
        data.extend_from_slice(&[0, 0]); // null terminator
        let sep = [0x00, 0x00]; // separator between records
        data.extend_from_slice(&sep);
        let name2_offset = data.len();
        data.extend(utf16_bytes("StyleCluster"));
        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.count, 2);
        assert_eq!(t.entries.len(), 2);
        assert_eq!(t.entries[0].name, "PSMcluster0");
        assert_eq!(t.entries[0].name_offset, name1_offset);
        assert_eq!(t.entries[0].record_offset, 8);
        assert!(t.entries[0].record_len > 0);
        assert_eq!(t.entries[0].prefix_bytes, prefix1.to_vec());
        assert_eq!(t.entries[1].name, "StyleCluster");
        assert_eq!(t.entries[1].name_offset, name2_offset);
    }

    #[test]
    fn cluster_table_ignores_short_runs() {
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x41, 0x00, 0, 0, 0, 0]); // 'A' + padding
        data.extend(utf16_bytes("Realname"));
        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 1);
        assert_eq!(t.entries[0].name, "Realname");
    }

    #[test]
    fn cluster_table_entry_records_offsets_and_prefix_bytes() {
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        let prefix = [0xAB, 0xCD, 0xEF, 0x12];
        data.extend_from_slice(&prefix);
        let name_off = data.len();
        data.extend(utf16_bytes("TestCluster"));
        data.extend_from_slice(&[0, 0]); // null terminator
        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 1);
        let e = &t.entries[0];
        assert_eq!(e.record_offset, 8);
        assert_eq!(e.name_offset, name_off);
        assert_eq!(e.prefix_bytes, prefix.to_vec());
        assert!(e.record_len > 0);
        assert_eq!(t.trailing_bytes, 0);
    }

    #[test]
    fn cluster_table_reports_trailing_bytes() {
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend(utf16_bytes("TestName"));
        data.extend_from_slice(&[0, 0]); // null terminator
        data.extend_from_slice(&[0xDE, 0xAD, 0xBE]); // trailing
        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 1);
        assert_eq!(t.trailing_bytes, 3);
    }

    #[test]
    fn segment_table_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x01, 0x01, 0x01]);
        let t = parse_psm_segment_table(&data).expect("valid");
        assert_eq!(t.count, 4);
        assert_eq!(t.flags, vec![0x01, 0x01, 0x01, 0x01]);
        assert_eq!(t.entries.len(), 4);
        assert_eq!(t.entries[0].index, 0);
        assert_eq!(t.entries[0].offset, 8);
        assert_eq!(t.entries[0].flag, 0x01);
        assert_eq!(t.entries[3].index, 3);
        assert_eq!(t.entries[3].offset, 11);
        assert_eq!(t.trailing_bytes, 0);
    }

    #[test]
    fn segment_table_truncated_returns_none() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&10u32.to_le_bytes());
        data.extend_from_slice(&[0x01; 3]);
        assert!(parse_psm_segment_table(&data).is_none());
    }

    #[test]
    fn segment_table_exposes_indexed_entries() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x02, 0x03]);
        let t = parse_psm_segment_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 3);
        for (i, e) in t.entries.iter().enumerate() {
            assert_eq!(e.index, i);
            assert_eq!(e.offset, 8 + i);
        }
        assert_eq!(t.entries[1].flag, 0x02);
    }

    #[test]
    fn segment_table_reports_trailing_bytes() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x01]);
        data.extend_from_slice(&[0xAA, 0xBB]);
        let t = parse_psm_segment_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 2);
        assert_eq!(t.trailing_bytes, 2);
    }

    #[test]
    fn cluster_table_entry_probes_expose_byte_level_summary() {
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        let prefix = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        data.extend_from_slice(&prefix);
        data.extend(utf16_bytes("TestCluster"));
        data.extend_from_slice(&[0, 0]); // null terminator

        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 1);
        let probe = t.entries[0].probe.as_ref().expect("probe populated");
        assert_eq!(probe.first_u32_le, Some(0x4433_2211));
        assert_eq!(probe.name_char_count, "TestCluster".chars().count());
        assert_eq!(probe.prefix_hex, "11 22 33 44 55 66");
        // Record length = 6 prefix + 22 UTF-16 chars + 2 null = 30 bytes.
        // Last 4 bytes should be two trailing chars + null (2 'r' bytes + 00 00).
        assert!(probe.last_u32_le.is_some());
        // Trailer hex should contain the last 8 bytes — verify length is
        // shaped as 23 chars = 8 tokens joined by single spaces.
        assert_eq!(probe.trailer_hex.split_whitespace().count(), 8);
    }

    #[test]
    fn segment_table_entries_expose_byte_level_probe() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);

        let t = parse_psm_segment_table(&data).expect("valid");
        assert_eq!(t.entries.len(), 4);

        let probe0 = t.entries[0]
            .probe
            .as_ref()
            .expect("probe populated for first segment");
        assert_eq!(probe0.flag_hex, "01");
        assert_eq!(probe0.stream_offset, 8);
        assert_eq!(probe0.owner_cluster_hint, None);
        let tokens0: Vec<_> = probe0.neighbor_window_hex.split_whitespace().collect();
        assert!(
            (1..=7).contains(&tokens0.len()),
            "window token count out of range: {tokens0:?}"
        );

        let probe_last = t.entries[3].probe.as_ref().expect("probe populated");
        assert_eq!(probe_last.flag_hex, "04");
        assert_eq!(probe_last.stream_offset, 11);
    }

    #[test]
    fn apply_segment_owner_hints_backfills_matching_lengths() {
        // Build a 2-flag segment table with probes in place.
        let mut seg_bytes = Vec::new();
        seg_bytes.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        seg_bytes.extend_from_slice(&2u32.to_le_bytes());
        seg_bytes.extend_from_slice(&[0x01, 0x01]);
        let mut seg = parse_psm_segment_table(&seg_bytes).expect("valid");

        // Build a 2-entry cluster table.
        let mut cluster_bytes = Vec::new();
        cluster_bytes.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        cluster_bytes.extend_from_slice(&2u32.to_le_bytes());
        cluster_bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        cluster_bytes.extend(utf16_bytes("PSMcluster0"));
        cluster_bytes.extend_from_slice(&[0, 0]);
        cluster_bytes.extend(utf16_bytes("StyleCluster"));
        cluster_bytes.extend_from_slice(&[0, 0]);
        let cluster = parse_psm_cluster_table(&cluster_bytes).expect("valid");
        assert_eq!(cluster.entries.len(), 2, "cluster fixture precondition");

        apply_segment_owner_hints(&mut seg, Some(&cluster));

        let hints: Vec<_> = seg
            .entries
            .iter()
            .map(|e| {
                e.probe
                    .as_ref()
                    .and_then(|p| p.owner_cluster_hint.clone())
            })
            .collect();
        assert_eq!(
            hints,
            vec![
                Some("PSMcluster0".to_string()),
                Some("StyleCluster".to_string())
            ]
        );
    }

    #[test]
    fn apply_segment_owner_hints_skips_when_lengths_mismatch() {
        let mut seg_bytes = Vec::new();
        seg_bytes.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        seg_bytes.extend_from_slice(&3u32.to_le_bytes());
        seg_bytes.extend_from_slice(&[0x01, 0x01, 0x01]);
        let mut seg = parse_psm_segment_table(&seg_bytes).expect("valid");

        let mut cluster_bytes = Vec::new();
        cluster_bytes.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        cluster_bytes.extend_from_slice(&2u32.to_le_bytes());
        cluster_bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        cluster_bytes.extend(utf16_bytes("OnlyOne"));
        cluster_bytes.extend_from_slice(&[0, 0]);
        cluster_bytes.extend(utf16_bytes("OnlyTwo"));
        cluster_bytes.extend_from_slice(&[0, 0]);
        let cluster = parse_psm_cluster_table(&cluster_bytes).expect("valid");
        assert_eq!(cluster.entries.len(), 2);

        apply_segment_owner_hints(&mut seg, Some(&cluster));

        for entry in &seg.entries {
            assert_eq!(
                entry
                    .probe
                    .as_ref()
                    .and_then(|p| p.owner_cluster_hint.clone()),
                None
            );
        }
    }

    #[test]
    fn apply_segment_owner_hints_noops_when_cluster_table_missing() {
        let mut seg_bytes = Vec::new();
        seg_bytes.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        seg_bytes.extend_from_slice(&1u32.to_le_bytes());
        seg_bytes.extend_from_slice(&[0xAA]);
        let mut seg = parse_psm_segment_table(&seg_bytes).expect("valid");

        apply_segment_owner_hints(&mut seg, None);

        assert_eq!(
            seg.entries[0]
                .probe
                .as_ref()
                .and_then(|p| p.owner_cluster_hint.clone()),
            None
        );
    }

    #[test]
    fn segment_table_probe_window_clips_near_stream_tail() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0xAA, 0xBB]);
        // `flags_start = 8`, segment 1 sits at offset 9 which is also the
        // last byte of the stream — the probe window must clip to stop
        // before an out-of-range index.
        let t = parse_psm_segment_table(&data).expect("valid");
        let tail = t.entries[1]
            .probe
            .as_ref()
            .expect("probe populated for tail segment");
        let tokens: Vec<_> = tail.neighbor_window_hex.split_whitespace().collect();
        assert!(
            tokens.len() <= 4,
            "tail probe window should be clipped, got {tokens:?}"
        );
        assert_eq!(tail.flag_hex, "BB");
        assert_eq!(tail.stream_offset, 9);
    }

    #[test]
    fn trace_aware_segment_parser_reports_complete_coverage_for_header_and_flags() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x02, 0x03]);

        let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
        let table = parse_psm_segment_table_with_trace(&data, &mut b)
            .expect("trace-aware parser succeeds on valid input");
        assert_eq!(table.entries.len(), 3);

        let trace = b.build("/PSMsegmenttable", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            data.len() as u64,
            "header (8) + flags (3) should total 11 consumed bytes"
        );
        assert!(
            trace.leftover_ranges.is_empty(),
            "no trailing bytes expected for this fixture; got {:?}",
            trace.leftover_ranges
        );
        assert_eq!(trace.parser_name, "parse_psm_segment_table");
    }

    #[test]
    fn trace_aware_segment_parser_marks_header_decoded_and_flags_probed() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x01]);

        let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
        parse_psm_segment_table_with_trace(&data, &mut b).expect("valid");
        let trace = b.build("/PSMsegmenttable", data.len() as u64);

        // Decoded bucket should include [0..8] (magic + count). Builder
        // merges adjacent same-confidence ranges, so there's one entry.
        assert_eq!(
            trace.ranges_by_confidence.get(&TraceConfidence::Decoded),
            Some(&vec![ByteRange::new(0, 8)])
        );
        // Probed bucket: one range per flag byte at offsets 8, 9 —
        // builder merges them into one [8..10] because the code
        // emits them consecutively with the same confidence.
        assert_eq!(
            trace.ranges_by_confidence.get(&TraceConfidence::Probed),
            Some(&vec![ByteRange::new(8, 10)])
        );
        // No Raw ranges at all.
        assert!(!trace
            .ranges_by_confidence
            .contains_key(&TraceConfidence::Raw));
    }

    #[test]
    fn trace_aware_segment_parser_leaves_trailing_bytes_in_leftover() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x01]);
        data.extend_from_slice(&[0xDE, 0xAD, 0xBE]); // trailing bytes

        let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
        let table = parse_psm_segment_table_with_trace(&data, &mut b).expect("valid");
        assert_eq!(table.trailing_bytes, 3);

        let trace = b.build("/PSMsegmenttable", data.len() as u64);
        assert_eq!(trace.consumed_bytes(), 10); // 8 header + 2 flags
        assert_eq!(trace.leftover_bytes(), 3);
        assert_eq!(
            trace.leftover_ranges,
            vec![ByteRange::new(10, 13)],
            "3 trailing bytes should surface as a single leftover range"
        );
    }

    #[test]
    fn trace_aware_segment_parser_emits_no_events_on_bad_magic() {
        let data = [0u8; 16]; // magic is all-zero, not 'stab'
        let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
        let out = parse_psm_segment_table_with_trace(&data, &mut b);
        assert!(out.is_none());
        let trace = b.build("/PSMsegmenttable", data.len() as u64);
        assert_eq!(
            trace.consumed_bytes(),
            0,
            "magic mismatch must short-circuit before any consume() call"
        );
    }

    #[test]
    fn back_compat_parse_psm_segment_table_matches_trace_variant_byte_for_byte() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        data.extend_from_slice(&[0xAA, 0xBB]); // trailing

        let without_trace = parse_psm_segment_table(&data).expect("old API works");

        let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
        let with_trace =
            parse_psm_segment_table_with_trace(&data, &mut b).expect("new API works");

        // The thin wrapper around `_with_trace` must produce an
        // identical table — down to entry probes — even though the
        // wrapper throws the builder away.
        assert_eq!(without_trace.size, with_trace.size);
        assert_eq!(without_trace.count, with_trace.count);
        assert_eq!(without_trace.flags, with_trace.flags);
        assert_eq!(without_trace.trailing_bytes, with_trace.trailing_bytes);
        assert_eq!(without_trace.entries.len(), with_trace.entries.len());
        for (a, b_entry) in without_trace.entries.iter().zip(with_trace.entries.iter()) {
            assert_eq!(a.index, b_entry.index);
            assert_eq!(a.offset, b_entry.offset);
            assert_eq!(a.flag, b_entry.flag);
            assert_eq!(a.probe, b_entry.probe);
        }
    }

    #[test]
    fn cluster_table_probe_handles_short_prefix() {
        // Construct a cluster whose prefix is only 2 bytes — outside the
        // realistic SmartPlant layout but useful as a defensive check.
        let mut data = Vec::new();
        data.extend_from_slice(&CLST_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        // 2-byte prefix
        data.extend_from_slice(&[0xAA, 0xBB]);
        data.extend(utf16_bytes("AB12"));
        data.extend_from_slice(&[0, 0]);

        let t = parse_psm_cluster_table(&data).expect("valid");
        let probe = t.entries[0].probe.as_ref().expect("probe populated");
        assert_eq!(probe.first_u32_le, None);
        assert_eq!(probe.prefix_hex, "AA BB");
        assert_eq!(probe.name_char_count, 4);
    }
}
