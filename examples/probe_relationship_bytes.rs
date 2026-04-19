//! One-shot reverse-engineering helper: locate every `Relationship.<GUID>`
//! occurrence in the Unclustered Dynamic Attributes stream and dump the
//! surrounding bytes so we can see which GUIDs are co-located with each
//! relationship record. Also searches other top-level streams for the same
//! Relationship GUIDs so we can identify where source/target endpoints live.
//!
//! Usage: `cargo run --example probe_relationship_bytes -- test-file/foo.pid`

use std::collections::BTreeMap;
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: probe_relationship_bytes <file.pid>");

    let mut cfb = cfb::open(path).expect("open cfb");
    let mut stream = cfb
        .open_stream("/Unclustered Dynamic Attributes")
        .expect("Unclustered Dynamic Attributes stream missing");
    let mut data = Vec::new();
    stream.read_to_end(&mut data).unwrap();

    println!("stream size: {} bytes (0x{:X})", data.len(), data.len());

    let needle = b"Relationship.";
    let hits: Vec<usize> = (0..data.len().saturating_sub(needle.len()))
        .filter(|&i| &data[i..i + needle.len()] == needle)
        .collect();
    println!("\n'Relationship.' ASCII hits: {}", hits.len());

    let rel_guids: Vec<String> = hits
        .iter()
        .filter_map(|&h| {
            let start = h + needle.len();
            let end = start + 32;
            if end <= data.len() && data[start..end].iter().all(|b| b.is_ascii_hexdigit()) {
                Some(String::from_utf8_lossy(&data[start..end]).to_string())
            } else {
                None
            }
        })
        .collect();
    println!("Extracted {} Relationship GUIDs", rel_guids.len());

    let all_stream_paths = list_all_stream_paths(&mut cfb);
    println!(
        "\n== cross-stream search: {} Relationship GUIDs across {} streams (ASCII + binary Windows GUID layout) ==",
        rel_guids.len(),
        all_stream_paths.len()
    );

    // Pre-compute both layouts we care about: the ASCII 32-char form and the
    // canonical Windows GUID 16-byte layout (first 3 fields little-endian, last
    // 8 bytes big-endian).
    let rel_guid_forms: Vec<(String, [u8; 16], [u8; 16])> = rel_guids
        .iter()
        .map(|g| {
            let raw = hex_to_bytes(g);
            let win = windows_guid_layout(&raw);
            (g.clone(), raw, win)
        })
        .collect();

    let mut ascii_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut binary_raw_hits: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    let mut binary_win_hits: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();

    for path in &all_stream_paths {
        let mut s = match cfb.open_stream(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut buf = Vec::new();
        if s.read_to_end(&mut buf).is_err() {
            continue;
        }

        for (g, raw, win) in &rel_guid_forms {
            let ascii = g.as_bytes();
            let mut count = 0;
            for i in 0..buf.len().saturating_sub(ascii.len()) {
                if &buf[i..i + ascii.len()] == ascii {
                    count += 1;
                }
            }
            if count > 0 {
                *ascii_hits.entry(path.clone()).or_default() += count;
            }

            if buf.len() >= 16 {
                for i in 0..buf.len() - 16 {
                    if &buf[i..i + 16] == raw {
                        binary_raw_hits
                            .entry(path.clone())
                            .or_default()
                            .push((g.clone(), i));
                    }
                    if &buf[i..i + 16] == win {
                        binary_win_hits
                            .entry(path.clone())
                            .or_default()
                            .push((g.clone(), i));
                    }
                }
            }
        }
    }

    println!("\n-- ASCII-form matches (32-char hex) --");
    for (p, n) in &ascii_hits {
        println!("  {}: {} hits", p, n);
    }
    println!(
        "\n-- binary raw-hex-layout matches (16 bytes, as-is) --   total streams: {}",
        binary_raw_hits.len()
    );
    for (p, hits) in &binary_raw_hits {
        println!("  {}: {} hits", p, hits.len());
        for (g, off) in hits.iter().take(3) {
            println!("    @0x{:06X}  {}", off, g);
        }
        if hits.len() > 3 {
            println!("    ... ({} more)", hits.len() - 3);
        }
    }
    println!(
        "\n-- binary Windows-GUID-layout matches (16 bytes, mixed-endian) --   total streams: {}",
        binary_win_hits.len()
    );
    for (p, hits) in &binary_win_hits {
        println!("  {}: {} hits", p, hits.len());
        for (g, off) in hits.iter().take(3) {
            println!("    @0x{:06X}  {}", off, g);
        }
        if hits.len() > 3 {
            println!("    ... ({} more)", hits.len() - 3);
        }
    }

    println!("\n== per-hit windows in /Unclustered Dynamic Attributes (first 4 hits) ==");

    let window = 160usize;
    for (n, &hit) in hits.iter().take(4).enumerate() {
        let lo = hit.saturating_sub(window);
        let hi = (hit + needle.len() + 48 + window).min(data.len());
        println!(
            "\n=== hit #{} @ offset 0x{:05X} ({}), window 0x{:05X}..0x{:05X} ===",
            n, hit, hit, lo, hi
        );

        let relationship_end = (hit + needle.len() + 32).min(data.len());
        if relationship_end <= data.len() {
            let full = &data[hit..relationship_end];
            let s = String::from_utf8_lossy(full);
            println!("  record tag: {:?}", s);
        }

        hex_dump_range(&data[lo..hi], lo);

        let guids = find_hex_guids(&data[lo..hi]);
        if guids.is_empty() {
            println!("  (no GUID-shaped substrings found in window)");
        } else {
            println!("  GUID-shaped substrings in window:");
            for (rel_off, g) in &guids {
                println!("    @+0x{:05X}  {}", lo + rel_off, g);
            }
        }
    }

    let all_guids = find_hex_guids(&data);
    println!(
        "\nTotal 32-char hex GUID substrings in full stream: {}",
        all_guids.len()
    );
}

fn hex_dump_range(chunk: &[u8], base: usize) {
    for (row, line) in chunk.chunks(16).enumerate() {
        let off = base + row * 16;
        let hex: Vec<String> = line.iter().map(|b| format!("{:02X}", b)).collect();
        let ascii: String = line
            .iter()
            .map(|&b| {
                if (0x20..=0x7e).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("  0x{:05X}: {:<48}  {}", off, hex.join(" "), ascii);
    }
}

fn find_hex_guids(data: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 32 <= data.len() {
        if data[i..i + 32].iter().all(|b| b.is_ascii_hexdigit()) {
            let s = String::from_utf8_lossy(&data[i..i + 32]).to_string();
            out.push((i, s));
            i += 32;
        } else {
            i += 1;
        }
    }
    out
}

fn list_all_stream_paths<R: Read + std::io::Seek>(cfb: &mut cfb::CompoundFile<R>) -> Vec<String> {
    cfb.walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .collect()
}

fn hex_to_bytes(hex: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = hex.as_bytes();
    for i in 0..16 {
        let hi = (bytes[i * 2] as char).to_digit(16).unwrap_or(0) as u8;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16).unwrap_or(0) as u8;
        out[i] = (hi << 4) | lo;
    }
    out
}

/// Re-pack a "raw hex layout" GUID into Windows/OLE on-disk layout: first 4
/// bytes little-endian, next 2 bytes little-endian, next 2 bytes little-endian,
/// last 8 bytes big-endian (unchanged).
fn windows_guid_layout(raw: &[u8; 16]) -> [u8; 16] {
    [
        raw[3], raw[2], raw[1], raw[0], raw[5], raw[4], raw[7], raw[6], raw[8], raw[9], raw[10],
        raw[11], raw[12], raw[13], raw[14], raw[15],
    ]
}
