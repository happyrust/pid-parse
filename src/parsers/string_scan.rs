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
            let printable_ascii = (0x20..=0x7e).contains(&(w as u8));
            if printable_ascii || w > 0x7f {
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
