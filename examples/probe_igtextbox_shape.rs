//! Probe PSM type 0x004D (igTextBox) record byte shape.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

fn probe_fixture(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut stream = cfb.open_stream("/Sheet6")?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    println!(
        "\n=== {} /Sheet6 (size {}) ===",
        path.display(),
        bytes.len()
    );

    let header_len = 6;
    if bytes.len() < header_len + 16 {
        return Ok(());
    }
    let max_offset = bytes.len() - (header_len + 16);
    let mut hits = 0;
    let mut size_dist: BTreeMap<u32, usize> = BTreeMap::new();
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != 0x004D {
            continue;
        }
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        if !(8..=100_000).contains(&bytes_to_follow) {
            continue;
        }
        if (bytes_to_follow as usize) > bytes.len() - off {
            continue;
        }

        hits += 1;
        *size_dist.entry(bytes_to_follow).or_insert(0) += 1;
        let payload_start = off + header_len;
        let payload_end = (payload_start + bytes_to_follow as usize).min(bytes.len());
        let payload_len = payload_end - payload_start;
        if hits <= 3 {
            println!(
                "  HIT[{}] @ 0x{:06x} bytes_to_follow={} payload_len={}",
                hits, off, bytes_to_follow, payload_len
            );
            let dump_len = payload_len.min(256);
            for chunk_start in (0..dump_len).step_by(16) {
                let chunk_end = (chunk_start + 16).min(dump_len);
                let raw: &[u8] = &bytes[payload_start + chunk_start..payload_start + chunk_end];
                let hex: String = raw
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ascii: String = raw
                    .iter()
                    .map(|b| {
                        if (0x20..0x7F).contains(b) {
                            *b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("           +{:03}: {:<48} | {}", chunk_start, hex, ascii);
            }
        }
    }
    println!("  total 0x004D hits: {}", hits);
    println!("  bytes_to_follow distribution (top 10):");
    let mut sorted: Vec<_> = size_dist.iter().collect();
    sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (size, count) in sorted.iter().take(10) {
        println!("    {} bytes: {} hits", size, count);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        probe_fixture(path)?;
    }
    Ok(())
}
