//! Parser for the `JTaggedTxtStgList` stream — a small index that maps a
//! logical "storage list" name (e.g. "TaggedTxtStorages") to the actual
//! storage directory names inside the document (e.g. "TaggedTxtData").
//!
//! Observed layout (sampled file is 70 bytes total):
//!
//! ```text
//! +00..+X    `list_name` as a run of UTF-16LE printable-ASCII code units
//!            (NOT null-terminated; bounded by the following u32)
//! +X..+X+3   u32 entry count (observed `0x00000001`)
//! For each entry:
//!   [u32 char_count including L'\0']
//!   [UTF-16LE storage_name of char_count chars, last word is 0x0000]
//! ```
//!
//! The parser is defensive: if layout does not match, returns `None`.

use crate::model::{TaggedTextStorageEntry, TaggedTextStorageList};

pub fn parse_tagged_stg_list(data: &[u8]) -> Option<TaggedTextStorageList> {
    if data.len() < 8 {
        return None;
    }
    // Read a UTF-16LE run of printable-ASCII characters at offset 0; it ends
    // when we hit a u16 outside the printable-ASCII range (typically the
    // low byte of the following u32 count).
    let (list_name, after_name) = read_utf16le_ascii_run(data, 0);
    if list_name.is_empty() || after_name + 4 > data.len() {
        return None;
    }
    let count = u32::from_le_bytes([
        data[after_name],
        data[after_name + 1],
        data[after_name + 2],
        data[after_name + 3],
    ]);
    let mut pos = after_name + 4;
    let mut entries = Vec::new();
    for _ in 0..count {
        if pos + 4 > data.len() {
            break;
        }
        let char_count =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let name_start = pos + 4;
        let name_end = name_start.checked_add(char_count.checked_mul(2)?)?;
        if name_end > data.len() || char_count > 256 {
            break;
        }
        let name_bytes = &data[name_start..name_end];
        let words: Vec<u16> = name_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&w| w != 0)
            .collect();
        let storage_name = String::from_utf16_lossy(&words);
        entries.push(TaggedTextStorageEntry { storage_name });
        pos = name_end;
    }
    Some(TaggedTextStorageList {
        size: data.len() as u64,
        list_name,
        entries,
    })
}

fn read_utf16le_ascii_run(data: &[u8], start: usize) -> (String, usize) {
    let mut i = start;
    let mut words: Vec<u16> = Vec::new();
    while i + 2 <= data.len() {
        let w = u16::from_le_bytes([data[i], data[i + 1]]);
        if !(0x20..=0x7e).contains(&w) {
            break;
        }
        words.push(w);
        i += 2;
    }
    (String::from_utf16_lossy(&words), i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn parses_single_entry() {
        let mut data = Vec::new();
        // list_name: plain UTF-16LE ASCII run (no L'\0' terminator)
        data.extend(utf16("TaggedTxtStorages"));
        // u32 count = 1 (first byte 0x01 breaks the ascii-run scan above)
        data.extend_from_slice(&1u32.to_le_bytes());
        // storage_name: char_count including L'\0'
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]); // L'\0' terminator
        let r = parse_tagged_stg_list(&data).expect("valid");
        assert_eq!(r.list_name, "TaggedTxtStorages");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].storage_name, "TaggedTxtData");
    }

    #[test]
    fn rejects_short_stream() {
        assert!(parse_tagged_stg_list(&[]).is_none());
        assert!(parse_tagged_stg_list(&[1, 2, 3, 4]).is_none());
    }

    #[test]
    fn truncated_entry_stops_gracefully() {
        let mut data = Vec::new();
        data.extend(utf16("TaggedTxtStorages"));
        data.extend_from_slice(&2u32.to_le_bytes()); // declared 2
        data.extend_from_slice(&14u32.to_le_bytes());
        data.extend(utf16("TaggedTxtData"));
        data.extend_from_slice(&[0, 0]);
        // Second entry truncated (only 3 bytes of a u32 count)
        data.extend_from_slice(&[5, 0, 0]);
        let r = parse_tagged_stg_list(&data).expect("valid");
        assert_eq!(r.entries.len(), 1);
    }
}
