//! Probe PSM type 0x0018 (igLine2d candidate) record byte shape.
//!
//! `examples/probe_psm_type_code_histogram.rs` revealed 309
//! cross-fixture records with PSM type code `0x0018` (= IGDS class
//! tag for `igLine2d`). This probe inspects their byte content
//! looking for canonical line shape:
//!
//! - 4 × `f64` LE: `(start.x, start.y, end.x, end.y)` (32 bytes)
//! - or 2 × `f64` LE: `(end.x, end.y)` (16 bytes, relative to origin)
//! - or some other compact representation
//!
//! Dumps first 64 bytes of payload (after 18-byte PSM header) for
//! the first 10 hits per fixture to help spot the field pattern.

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

    let header_len = 18;
    let min_payload = 16;
    if bytes.len() < header_len + min_payload {
        return Ok(());
    }
    let max_offset = bytes.len() - (header_len + min_payload);
    let mut hits = 0;
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != 0x0018 {
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
        // Payload starts at off+6 (header is just type + bytes_to_follow).
        let payload_start = off + 6;
        let payload_end = (payload_start + bytes_to_follow as usize).min(bytes.len());
        let payload_len = payload_end - payload_start;
        if hits <= 5 {
            print!(
                "  HIT[{:2}] @ 0x{:06x} bytes_to_follow={:>5} payload_len={}",
                hits, off, bytes_to_follow, payload_len
            );
            println!();
            // Full hex dump of payload, 16 bytes per line.
            for chunk_start in (0..payload_len).step_by(16) {
                let chunk_end = (chunk_start + 16).min(payload_len);
                let hex: String = bytes[payload_start + chunk_start..payload_start + chunk_end]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("           +{:02}: {}", chunk_start, hex);
            }
            // Try parsing 4 doubles starting at payload offset 6
            // (skipping the 6-byte sub-prefix).
            if payload_len >= 6 + 32 {
                let mut d = [0f64; 4];
                for (i, slot) in d.iter_mut().enumerate() {
                    let pos = payload_start + 6 + i * 8;
                    *slot = f64::from_le_bytes([
                        bytes[pos],
                        bytes[pos + 1],
                        bytes[pos + 2],
                        bytes[pos + 3],
                        bytes[pos + 4],
                        bytes[pos + 5],
                        bytes[pos + 6],
                        bytes[pos + 7],
                    ]);
                }
                let all_finite = d.iter().all(|x| x.is_finite() && x.abs() < 1e6);
                if all_finite {
                    println!(
                        "           if 4xf64 @+6: ({:+.4}, {:+.4}) -> ({:+.4}, {:+.4})",
                        d[0], d[1], d[2], d[3]
                    );
                }
            }
        }
    }
    println!("  total 0x0018 hits: {}", hits);
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
