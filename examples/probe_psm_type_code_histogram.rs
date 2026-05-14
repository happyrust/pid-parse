//! Histogram all plausible PSM type codes in `Sheet*` streams.
//!
//! For every offset that looks like a valid PSM record header
//! (`bytes_to_follow >= 8` and fits the stream), extract the 14-bit
//! type code (`u16 LE & 0x3FFF`) and count occurrences. This builds
//! a distribution of "PSM-like" records by type code across all
//! Sheet-bearing fixtures.
//!
//! Goal: identify dominant type codes other than the already-known
//! `0x3FE6` (GLine2d) and `0x0030` (GArc2d / GEllipse2d). Candidates
//! that show up multiple times across fixtures are likely additional
//! PSM record types (igCircle2d / igLineString2d / igSymbol2d / etc.).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

fn count_type_codes(path: &Path) -> Result<BTreeMap<u16, usize>, Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let mut stream = cfb.open_stream("/Sheet6")?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;

    let mut histogram: BTreeMap<u16, usize> = BTreeMap::new();
    let header_len = 18;
    if bytes.len() < header_len + 8 {
        return Ok(histogram);
    }
    let max_offset = bytes.len() - (header_len + 8);
    for off in 0..=max_offset {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        // Filter: bytes_to_follow must be plausibly sized + fit in
        // the stream tail. Also require type_code != 0 to avoid
        // counting all-zero padding.
        if !(8..=100_000).contains(&bytes_to_follow) {
            continue;
        }
        if (bytes_to_follow as usize) > bytes.len() - off {
            continue;
        }
        if type_code == 0 {
            continue;
        }
        *histogram.entry(type_code).or_insert(0) += 1;
    }
    Ok(histogram)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ];

    let mut combined: BTreeMap<u16, BTreeMap<&str, usize>> = BTreeMap::new();

    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        println!("\n=== {fixture} /Sheet6 type code histogram ===");
        let histogram = count_type_codes(path)?;
        let mut sorted: Vec<_> = histogram.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        for (type_code, count) in sorted.iter().take(20) {
            let known = match **type_code {
                0x3FE6 => " (GLine2d, IGDS 0x18)",
                0x0030 => " (GArc2d / GEllipse2d, IGDS 0x61)",
                _ => "",
            };
            println!(
                "  0x{:04X} ({:5}): {} hits{}",
                type_code, type_code, count, known
            );
        }
        for (type_code, count) in &histogram {
            combined
                .entry(*type_code)
                .or_default()
                .entry(fixture)
                .and_modify(|c| *c += count)
                .or_insert(*count);
        }
    }

    println!("\n=== Combined histogram across fixtures (≥ 2 fixtures, count ≥ 3) ===");
    let mut cross_fixture: Vec<_> = combined
        .iter()
        .filter(|(_, by_fix)| by_fix.len() >= 2 && by_fix.values().sum::<usize>() >= 3)
        .collect();
    cross_fixture.sort_by_key(|(_, by_fix)| std::cmp::Reverse(by_fix.values().sum::<usize>()));
    for (type_code, by_fix) in cross_fixture.iter().take(30) {
        let total: usize = by_fix.values().sum();
        let known = match **type_code {
            0x3FE6 => " (GLine2d)",
            0x0030 => " (GArc2d / GEllipse2d)",
            _ => "",
        };
        println!(
            "  0x{:04X} ({:5}): total {} hits across {} fixtures{}",
            type_code,
            type_code,
            total,
            by_fix.len(),
            known
        );
    }

    Ok(())
}
