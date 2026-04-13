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

    JProperties {
        strings,
        key_values,
        guids: Vec::new(),
        raw_len: data.len(),
    }
}

fn is_key_like(s: &str) -> bool {
    s.len() >= 3
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}
