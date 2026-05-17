//! Phase 19 probe: scan PSM type codes `0x0029..=0x0035` (RAD
//! `47FCC330..47FCC33E` sibling family) across all four
//! Sheet-bearing fixtures.
//!
//! Phase 16 JStyleOverride reverse-engineering proved `0x0030`
//! belongs to the RAD `47FCC338` `style.dll` family (see
//! `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`). The
//! deferred Phase 19 hypothesis is that the contiguous CLSID range
//! `47FCC330..47FCC33E` maps 1:1 to PSM type codes `0x29..0x35`
//! and may contain other annotation-like records.
//!
//! This probe is non-advancing (counts every byte offset whose
//! 14-bit type word matches and whose `bytes_to_follow` fits the
//! stream) so it mirrors Phase 18's probe style.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

fn count_psm_type_in_stream(
    path: &Path,
    type_codes: &[u16],
) -> Result<BTreeMap<u16, usize>, Box<dyn std::error::Error>> {
    let mut hits: BTreeMap<u16, usize> = type_codes.iter().map(|t| (*t, 0)).collect();
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut stream = cfb.open_stream("/Sheet6")?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;

    let header_len = 6;
    if bytes.len() < header_len + 8 {
        return Ok(hits);
    }
    let max_offset = bytes.len() - (header_len + 8);
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if !type_codes.contains(&type_code) {
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
        *hits.entry(type_code).or_insert(0) += 1;
    }
    Ok(hits)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ];

    let sibling_codes: Vec<u16> = (0x0029..=0x0035).collect();

    let mut combined: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        let hits = count_psm_type_in_stream(path, &sibling_codes)?;
        println!("\n=== {fixture} /Sheet6 ===");
        for (type_code, count) in &hits {
            let marker = if *type_code == 0x0030 {
                " (JStyleOverride, Phase 16)"
            } else {
                ""
            };
            println!("  0x{:04X}: {:5} hits{}", type_code, count, marker);
            combined
                .entry(*type_code)
                .or_default()
                .insert(fixture.to_string(), *count);
        }
    }

    println!("\n=== Cross-fixture totals (0x0029..=0x0035) ===");
    for type_code in sibling_codes {
        let by_fix = combined.entry(type_code).or_default();
        let total: usize = by_fix.values().sum();
        let nonzero_fixtures = by_fix.values().filter(|c| **c > 0).count();
        let marker = if type_code == 0x0030 {
            " ← JStyleOverride (Phase 16 / 17)"
        } else {
            ""
        };
        println!(
            "  0x{:04X}: total {} hits across {} of 4 fixtures{}",
            type_code, total, nonzero_fixtures, marker
        );
    }

    Ok(())
}
