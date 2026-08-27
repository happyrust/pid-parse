//! What is in `PSMspacemap`, the largest thing this crate does not read?
//!
//! The byte audit puts 66712 unread bytes in `PSMspacemap` across the four
//! sheet fixtures -- 71% of everything left over, and the only stream family
//! at a flat 0%. Nothing here decodes it yet; this probe is the first look, and
//! it asks the cheapest questions that could tell one kind of content from
//! another:
//!
//! * the stream names look like page addresses -- `0x00000000`, `0x00002000`,
//!   `0x00004000` -- so are the sizes and the spacing consistent with a map of
//!   8 KiB pages?
//! * is the content dense or sparse? A free-space bitmap is mostly runs; a
//!   record chain has structure at a stride; a table of ids is monotonic.
//! * do any of the words in it look like the `oid`s the rest of the document
//!   uses, which would make this content rather than bookkeeping?

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Every `PSMspacemap` member stream, as `(path, bytes)`.
fn spacemap_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|p| p.contains("PSMspacemap"))
        .collect();
    let mut out = Vec::new();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push((stream_path, data));
        }
    }
    out
}

/// How many distinct byte values, and the longest run of one value. Together
/// these separate a bitmap from a record chain without decoding either.
fn texture(data: &[u8]) -> (usize, usize, f64) {
    let mut seen = [false; 256];
    let mut distinct = 0usize;
    let mut longest = 0usize;
    let mut run = 0usize;
    let mut ones = 0u32;
    let mut previous = None;
    for byte in data {
        if !seen[*byte as usize] {
            seen[*byte as usize] = true;
            distinct += 1;
        }
        ones += byte.count_ones();
        if previous == Some(*byte) {
            run += 1;
        } else {
            run = 1;
            previous = Some(*byte);
        }
        longest = longest.max(run);
    }
    let density = if data.is_empty() {
        0.0
    } else {
        f64::from(ones) / (data.len() as f64 * 8.0)
    };
    (distinct, longest, density)
}

fn main() {
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        println!("\n=== {fixture} ===");
        for (stream, data) in spacemap_streams(path) {
            let leaf = stream.rsplit('/').next().unwrap_or("").to_string();
            by_name.entry(leaf).or_default().push(data.len());
            let (distinct, longest, density) = texture(&data);
            println!(
                "  {stream}: {} bytes, {distinct} distinct byte values, \
                 longest run {longest}, bits set {:.1}%",
                data.len(),
                density * 100.0
            );
            let head: String = data
                .iter()
                .take(48)
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("      head: {head}");
            // A page map would state page numbers; content would state oids.
            let words: Vec<u32> = (0..data.len() / 4)
                .filter_map(|i| u32_at(&data, i * 4))
                .collect();
            let nonzero = words.iter().filter(|w| **w != 0).count();
            let ascending = words.windows(2).filter(|pair| pair[0] < pair[1]).count();
            println!(
                "      {} u32 words, {nonzero} non-zero, {ascending} ascending steps{}",
                words.len(),
                if words.len() > 1 && ascending * 10 > (words.len() - 1) * 9 {
                    "  <- nearly monotonic"
                } else {
                    ""
                }
            );
        }
    }

    println!("\n=== stream names across the corpus ===");
    for (name, sizes) in &by_name {
        println!("  {name}: sizes {sizes:?}");
    }
}
