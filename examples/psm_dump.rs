//! Hex-dump arbitrary top-level streams of a `.pid` file.
//! Usage: `cargo run --example psm_dump -- file.pid [stream_name...]`
//! If no stream names are given, dumps PSM index streams by default.

use std::io::Read;

fn dump_all(data: &[u8]) {
    for (row, chunk) in data.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
            .collect();
        println!("  {:04X}: {:<48}  {}", row * 16, hex.join(" "), ascii);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: psm_dump <file.pid> [stream...]");
    let extra: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();
    let names: Vec<&str> = if extra.is_empty() {
        vec!["PSMroots", "PSMclustertable", "PSMsegmenttable"]
    } else {
        extra
    };
    let mut cfb = cfb::open(path).expect("open cfb");
    for name in names {
        let stream_path = format!("/{}", name);
        let mut stream = match cfb.open_stream(&stream_path) {
            Ok(s) => s,
            Err(_) => {
                println!("\n=== {} (NOT FOUND) ===", name);
                continue;
            }
        };
        let mut data = Vec::new();
        stream.read_to_end(&mut data).unwrap();
        println!("\n=== {} ({} bytes) ===", name, data.len());

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let tag: Vec<char> = magic
            .to_le_bytes()
            .iter()
            .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
            .collect();
        println!("magic=0x{:08X} tag=\"{}\"", magic, tag.iter().collect::<String>());

        dump_all(&data);

        // Try to walk as [u32 id][u32 char_count][UTF-16LE chars] records after
        // the 4-byte magic.
        println!("\n  Structured walk (u32 id / u32 char_count / utf16):");
        let mut pos = 4usize;
        let mut idx = 0;
        while pos + 8 <= data.len() {
            let id = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let cc = u32::from_le_bytes([
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]) as usize;
            let byte_len = cc * 2;
            let name_start = pos + 8;
            if name_start + byte_len > data.len() || cc > 512 {
                println!(
                    "    [{}] @+{:04X} id=0x{:08X} cc={} BAD (truncated or absurd)",
                    idx, pos, id, cc
                );
                break;
            }
            let name_bytes = &data[name_start..name_start + byte_len];
            let utf16: Vec<u16> = name_bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let name = String::from_utf16_lossy(&utf16);
            println!(
                "    [{}] @+{:04X} id=0x{:08X} cc={:2} name=\"{}\"",
                idx, pos, id, cc, name
            );
            pos = name_start + byte_len;
            idx += 1;
        }
        if pos < data.len() {
            println!("    ... {} trailing bytes from +{:04X}", data.len() - pos, pos);
        }
    }
}
