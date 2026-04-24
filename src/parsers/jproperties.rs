//! Decoder for `JSite` property blobs.
//!
//! Extracts ASCII / UTF-16 strings, `key=value` pairs, and 32-hex
//! GUIDs from a raw property payload and packages them into
//! [`JProperties`]. The input format is heuristic — `SmartPlant`
//! stores these as opaque CFB streams — so the output is
//! best-effort: the raw byte length is retained so consumers can
//! still sanity-check coverage.

use crate::model::JProperties;
use std::collections::BTreeMap;

pub fn parse_jproperties(data: &[u8]) -> JProperties {
    let mut strings = crate::parsers::string_scan::scan_ascii_strings(data, 256);

    for s in crate::parsers::string_scan::scan_utf16le_strings(data, 4, 256) {
        if !strings.contains(&s) {
            strings.push(s);
        }
    }

    let mut key_values = BTreeMap::new();
    let mut i = 0;
    while i + 1 < strings.len() {
        let key = &strings[i];
        let value = &strings[i + 1];
        if is_key_like(key) {
            key_values.entry(key.clone()).or_insert(value.clone());
        }
        i += 1;
    }

    let guids = crate::parsers::string_scan::scan_guids(data, 64);

    JProperties {
        strings,
        key_values,
        guids,
        raw_len: data.len(),
    }
}

fn is_key_like(s: &str) -> bool {
    s.len() >= 3
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}
