use crate::model::{ClusterHeader, IndexedString};

pub const CLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Parse the common header shared by all cluster-family streams.
/// Returns None if the data is too short or the magic doesn't match.
pub fn parse_header(data: &[u8]) -> Option<ClusterHeader> {
    if data.len() < 14 {
        return None;
    }
    let magic = u32_le(data, 0);
    if magic != CLUSTER_MAGIC {
        return None;
    }
    Some(ClusterHeader {
        magic,
        record_count: u32_le(data, 4),
        stream_type: u16_le(data, 8),
        body_len: u32_le(data, 10),
        flags: u16_le(data, 14),
    })
}

/// Parse the indexed UTF-16LE string table found in PSMcluster0.
/// Starts scanning from `offset`; each entry is: u32 index + u32 byte_len + UTF-16LE payload.
/// Stops when it encounters an index of 0 followed by a zero-length entry, or runs out of data.
pub fn parse_string_table(data: &[u8], start: usize) -> (Vec<IndexedString>, usize) {
    let mut out = Vec::new();
    let mut pos = start;

    while pos + 8 <= data.len() {
        let index = u32_le(data, pos);
        let byte_len = u32_le(data, pos + 4) as usize;
        pos += 8;

        if byte_len == 0 {
            out.push(IndexedString {
                index,
                value: String::new(),
            });
            break;
        }

        if pos + byte_len > data.len() {
            break;
        }

        let char_count = byte_len / 2;
        let words: Vec<u16> = (0..char_count)
            .map(|i| u16_le(data, pos + i * 2))
            .take_while(|&w| w != 0)
            .collect();
        let value = String::from_utf16_lossy(&words);

        out.push(IndexedString { index, value });
        pos += byte_len;
    }

    (out, pos)
}

fn u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
