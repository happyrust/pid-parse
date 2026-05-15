//! Probe bytes immediately after PSM igLine2d records — looking
//! for attribute tail (color / line style / layer / level refs).
//!
//! Each `igLine2d` is 56 bytes total (6 header + 50 payload). Dump
//! the 64 bytes *after* the record to see what follows.

use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const PSM_IGLINE2D: u16 = 0x0018;
const IGLINE2D_TOTAL_SIZE: usize = 56; // 6 header + 50 payload

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

    if bytes.len() < IGLINE2D_TOTAL_SIZE + 64 {
        return Ok(());
    }
    let max_offset = bytes.len() - (IGLINE2D_TOTAL_SIZE + 64);
    let mut hits = 0;
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != PSM_IGLINE2D {
            continue;
        }
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        if bytes_to_follow != 50 {
            continue;
        }
        hits += 1;
        if hits > 4 {
            break;
        }
        // Dump the 32 bytes after the igLine2d record ends.
        let after = off + IGLINE2D_TOTAL_SIZE;
        let dump_end = (after + 64).min(bytes.len());
        let dump_len = dump_end - after;
        println!(
            "  igLine2d HIT[{}] @ 0x{:06x}, oid_field={} - dumping {} bytes after record end:",
            hits,
            off,
            u32::from_le_bytes([
                bytes[off + 6],
                bytes[off + 7],
                bytes[off + 8],
                bytes[off + 9],
            ]),
            dump_len,
        );
        for chunk_start in (0..dump_len).step_by(16) {
            let chunk_end = (chunk_start + 16).min(dump_len);
            let raw: &[u8] = &bytes[after + chunk_start..after + chunk_end];
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
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        probe_fixture(path)?;
    }
    Ok(())
}
