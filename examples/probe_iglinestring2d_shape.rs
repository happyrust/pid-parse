//! Probe PSM type 0x0084 (igLineString2d candidate) record byte shape.
//!
//! `examples/probe_psm_type_code_histogram.rs` revealed 131
//! cross-fixture records with PSM type code `0x0084` (= IGDS class
//! tag for `igLineString2d`). This probe dumps raw payload bytes
//! to validate the expected polyline layout:
//!
//! - 18-byte sub-header (parallel to igLine2d layout):
//!   - oid (u32) + parent_ref (u32) + remaining_header (u32) +
//!     sub_type_word (u16) + index (u32)
//! - vertex_count (u32 LE)
//! - form (u8) + scope (u8) + padding (2 bytes for alignment)
//! - vertex_count × 16 bytes of `(f64 x, f64 y)` pairs
//!
//! Or alternative: just count + form/scope + vertex array, with no
//! 18-byte preamble. Dump first 96 bytes per hit to identify.

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
    if bytes.len() < header_len + 32 {
        return Ok(());
    }
    let max_offset = bytes.len() - (header_len + 32);
    let mut hits = 0;
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != 0x0084 {
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
        let payload_start = off + header_len;
        let payload_end = (payload_start + bytes_to_follow as usize).min(bytes.len());
        let payload_len = payload_end - payload_start;
        if hits <= 5 {
            println!(
                "  HIT[{}] @ 0x{:06x} bytes_to_follow={} payload_len={}",
                hits, off, bytes_to_follow, payload_len
            );
            let dump_len = payload_len.min(128);
            for chunk_start in (0..dump_len).step_by(16) {
                let chunk_end = (chunk_start + 16).min(dump_len);
                let hex: String = bytes[payload_start + chunk_start..payload_start + chunk_end]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("           +{:03}: {}", chunk_start, hex);
            }
            // Try parsing igLine2d-style header (18 bytes) + then vertex_count
            // at offset 18.
            if payload_len >= 22 {
                let oid = u32::from_le_bytes([
                    bytes[payload_start],
                    bytes[payload_start + 1],
                    bytes[payload_start + 2],
                    bytes[payload_start + 3],
                ]);
                let parent_ref = u32::from_le_bytes([
                    bytes[payload_start + 4],
                    bytes[payload_start + 5],
                    bytes[payload_start + 6],
                    bytes[payload_start + 7],
                ]);
                let remaining_header = u32::from_le_bytes([
                    bytes[payload_start + 8],
                    bytes[payload_start + 9],
                    bytes[payload_start + 10],
                    bytes[payload_start + 11],
                ]);
                let sub_type_word =
                    u16::from_le_bytes([bytes[payload_start + 12], bytes[payload_start + 13]]);
                let index = u32::from_le_bytes([
                    bytes[payload_start + 14],
                    bytes[payload_start + 15],
                    bytes[payload_start + 16],
                    bytes[payload_start + 17],
                ]);
                let vc_off = payload_start + 18;
                if vc_off + 4 <= bytes.len() {
                    let vc = u32::from_le_bytes([
                        bytes[vc_off],
                        bytes[vc_off + 1],
                        bytes[vc_off + 2],
                        bytes[vc_off + 3],
                    ]);
                    println!(
                        "           interpret: oid={} parent={} remaining=0x{:X} sub_type=0x{:04X} index={} vc_at_18={}",
                        oid, parent_ref, remaining_header, sub_type_word, index, vc
                    );
                    // Expected: vc small (2..100) for polyline
                    // payload_len should == 18 + 4 (vc) + 4 (form/scope/pad) + vc * 16 = 26 + vc*16
                    if (2..=10000).contains(&vc) {
                        let expected = 26 + (vc as usize) * 16;
                        if expected == payload_len {
                            println!(
                                "           SHAPE MATCH! payload_len {} == 26 + {}*16",
                                payload_len, vc
                            );
                        }
                    }
                }
            }
        }
    }
    println!("  total 0x0084 hits: {}", hits);
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
