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

use crate::model::{
    PsmClusterEntry, PsmClusterRecordProbe, PsmClusterTable, PsmRootEntry, PsmRoots,
    PsmSegmentEntry, PsmSegmentTable,
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
pub fn parse_psm_segment_table(data: &[u8]) -> Option<PsmSegmentTable> {
    let magic = read_u32_le(data, 0)?;
    if magic != STAB_MAGIC {
        return None;
    }
    let count = read_u32_le(data, 4)?;
    let flags_start = 8usize;
    let flags_end = flags_start.checked_add(count as usize)?;
    if flags_end > data.len() {
        return None;
    }
    let flags: Vec<u8> = data[flags_start..flags_end].to_vec();
    let entries: Vec<PsmSegmentEntry> = flags
        .iter()
        .enumerate()
        .map(|(i, &flag)| PsmSegmentEntry {
            index: i,
            offset: flags_start + i,
            flag,
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
