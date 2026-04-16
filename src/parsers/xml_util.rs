use std::collections::BTreeMap;

/// Extract simple `<Tag>value</Tag>` pairs from an XML-like string.
/// Skips nested tags (values containing `<`).
pub fn collect_simple_tags(input: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' || i + 1 >= bytes.len() || bytes[i + 1] == b'/' {
            i += 1;
            continue;
        }

        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < bytes.len() && bytes[name_end] != b'>' {
            name_end += 1;
        }
        if name_end >= bytes.len() {
            break;
        }

        let name = input[name_start..name_end].trim();
        let close = format!("</{}>", name);
        let value_start = name_end + 1;
        if let Some(rel_end) = input[value_start..].find(&close) {
            let value_end = value_start + rel_end;
            let value = input[value_start..value_end].trim();
            if !name.is_empty() && !value.contains('<') {
                out.insert(name.to_string(), value.to_string());
                i = value_end + close.len();
            } else {
                i = name_end + 1;
            }
        } else {
            i = name_end + 1;
        }
    }

    out
}
