pub fn scan_ascii_strings(data: &[u8], limit: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut buf = Vec::new();

    for &b in data {
        if (0x20..=0x7e).contains(&b) || b == b'\t' {
            buf.push(b);
        } else {
            if buf.len() >= 4 {
                result.push(String::from_utf8_lossy(&buf).to_string());
                if result.len() >= limit {
                    break;
                }
            }
            buf.clear();
        }
    }

    if buf.len() >= 4 && result.len() < limit {
        result.push(String::from_utf8_lossy(&buf).to_string());
    }

    result
}

/// Scan binary data for GUIDs in both text and raw 16-byte LE form.
pub fn scan_guids(data: &[u8], limit: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    // Text form: {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}
    let text = String::from_utf8_lossy(data);
    let mut start = 0;
    while let Some(pos) = text[start..].find('{') {
        let abs = start + pos;
        if abs + 38 <= text.len() {
            let candidate = &text[abs..abs + 38];
            if is_guid_text(candidate) {
                let upper = candidate.to_ascii_uppercase();
                if seen.insert(upper.clone()) {
                    out.push(upper);
                    if out.len() >= limit {
                        return out;
                    }
                }
            }
        }
        start = abs + 1;
    }

    // Raw 16-byte LE GUIDs (Microsoft mixed-endian layout)
    if data.len() >= 16 {
        for i in 0..=data.len() - 16 {
            let chunk = &data[i..i + 16];
            if chunk.iter().all(|&b| b == 0) {
                continue;
            }
            let formatted = format!(
                "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
                u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                u16::from_le_bytes([chunk[4], chunk[5]]),
                u16::from_le_bytes([chunk[6], chunk[7]]),
                chunk[8],
                chunk[9],
                chunk[10],
                chunk[11],
                chunk[12],
                chunk[13],
                chunk[14],
                chunk[15],
            );
            if has_plausible_guid_structure(chunk) && seen.insert(formatted.clone()) {
                out.push(formatted);
                if out.len() >= limit {
                    break;
                }
            }
        }
    }

    out
}

fn is_guid_text(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 38 || b[0] != b'{' || b[37] != b'}' {
        return false;
    }
    let dashes = [9, 14, 19, 24];
    for &d in &dashes {
        if b[d] != b'-' {
            return false;
        }
    }
    for (i, &byte) in b.iter().enumerate() {
        if i == 0 || i == 37 || dashes.contains(&i) {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn has_plausible_guid_structure(chunk: &[u8]) -> bool {
    let nonzero = chunk.iter().filter(|&&b| b != 0).count();
    let version_nibble = (chunk[7] >> 4) & 0x0F;
    nonzero >= 6 && (1..=5).contains(&version_nibble)
}

pub fn scan_utf16le_strings(data: &[u8], min_chars: usize, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;

    while i + 1 < data.len() && out.len() < limit {
        let start = i;
        let mut words = Vec::new();

        while i + 1 < data.len() {
            let w = u16::from_le_bytes([data[i], data[i + 1]]);
            if w == 0 {
                break;
            }
            let printable = (0x20..=0x7e).contains(&w) || w > 0x7f;
            if printable {
                words.push(w);
                i += 2;
            } else {
                break;
            }
        }

        if words.len() >= min_chars {
            out.push(String::from_utf16_lossy(&words));
        }

        i = if i == start { i + 2 } else { i + 2 };
    }

    out
}
