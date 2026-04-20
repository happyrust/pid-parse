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

use crate::model::{PsmClusterEntry, PsmClusterTable, PsmRootEntry, PsmRoots, PsmSegmentTable};

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

/// Parse `PSMclustertable`. Extracts the canonical list of cluster names.
///
/// The per-record header layout (length / flags / cluster index) is still
/// being reverse-engineered, so this parser only extracts the UTF-16LE ASCII
/// names (runs of ≥ 4 printable ASCII chars encoded as UTF-16LE). In the
/// sampled file this recovers 5/5 cluster names cleanly.
pub fn parse_psm_cluster_table(data: &[u8]) -> Option<PsmClusterTable> {
    let magic = read_u32_le(data, 0)?;
    if magic != CLST_MAGIC {
        return None;
    }
    let count = read_u32_le(data, 4)?;
    let mut entries = Vec::new();
    let mut i = 8;
    while i + 2 <= data.len() {
        if is_ascii_utf16le(data, i) {
            let start = i;
            let mut name = String::new();
            while i + 2 <= data.len() && is_ascii_utf16le(data, i) {
                name.push(data[i] as char);
                i += 2;
            }
            if name.len() >= 4 {
                entries.push(PsmClusterEntry {
                    name,
                    name_offset: start,
                });
            }
        } else {
            i += 1;
        }
    }
    Some(PsmClusterTable {
        size: data.len() as u64,
        count,
        entries,
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
    Some(PsmSegmentTable {
        size: data.len() as u64,
        count,
        flags: data[flags_start..flags_end].to_vec(),
    })
}

/// Detect a 2-byte UTF-16LE code unit encoding a printable ASCII character.
fn is_ascii_utf16le(data: &[u8], i: usize) -> bool {
    i + 1 < data.len() && (0x20..=0x7e).contains(&data[i]) && data[i + 1] == 0
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
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x01, 0, 0]); // filler
        data.extend(utf16_bytes("PSMcluster0"));
        data.extend_from_slice(&[0, 0, 0, 0]); // separator
        data.extend(utf16_bytes("StyleCluster"));
        let t = parse_psm_cluster_table(&data).expect("valid");
        assert_eq!(t.count, 2);
        assert_eq!(t.entries.len(), 2);
        assert_eq!(t.entries[0].name, "PSMcluster0");
        assert_eq!(t.entries[1].name, "StyleCluster");
    }

    #[test]
    fn cluster_table_ignores_short_runs() {
        // Single-char UTF-16LE 'A' should not be reported as a name.
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
    fn segment_table_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&[0x01, 0x01, 0x01, 0x01]);
        let t = parse_psm_segment_table(&data).expect("valid");
        assert_eq!(t.count, 4);
        assert_eq!(t.flags, vec![0x01, 0x01, 0x01, 0x01]);
    }

    #[test]
    fn segment_table_truncated_returns_none() {
        let mut data = Vec::new();
        data.extend_from_slice(&STAB_MAGIC.to_le_bytes());
        data.extend_from_slice(&10u32.to_le_bytes()); // claims 10 flags
        data.extend_from_slice(&[0x01; 3]); // only 3 present
        assert!(parse_psm_segment_table(&data).is_none());
    }
}
