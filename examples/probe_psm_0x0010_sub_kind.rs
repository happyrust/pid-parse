//! Phase 19 probe: scan every PSM `0x0010` sub-record across all four
//! Sheet-bearing fixtures and look for a stable sub-kind discriminator
//! byte / word that splits records into typed buckets WITHOUT naming
//! payload fields yet.
//!
//! Strategy (mirrors Phase 18 audit-only template + adds discriminator
//! histogramming):
//!
//! 1. Iterate every `0x0010` record (advancing scan, same as the
//!    Phase 18 `decode_sub_records_0x0010` decoder).
//! 2. Bucket records by `bytes_to_follow` (already known to be a
//!    polymorphic distribution: 13/16/21/28/31/43/45/46/50/70/74/76/86/94/99).
//! 3. For each bucket, dump a histogram of the first 2 bytes of the
//!    payload (most-likely discriminator-word position).
//! 4. Also dump a histogram of the byte at payload offset `+0` alone
//!    (most-likely discriminator-byte position) and at offset `+1`.
//! 5. Report a global "size -> top discriminator -> coverage %" table
//!    so we can see if a single discriminator cleanly partitions each
//!    size bucket.
//!
//! Phase 19 acceptance: discriminator candidate that splits ≥ 80 % of
//! records in each size bucket without overlapping across buckets
//! (i.e. discriminator distinguishes records IRRESPECTIVE of size).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

fn iter_records<F: FnMut(&[u8])>(bytes: &[u8], mut visit: F) {
    let header_len = 6;
    if bytes.len() < header_len + 1 {
        return;
    }
    let mut off = 0usize;
    while off + header_len <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != 0x0010 {
            off += 1;
            continue;
        }
        let bytes_to_follow = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
        if !(8..=100_000).contains(&bytes_to_follow) {
            off += 1;
            continue;
        }
        let payload_start = off + header_len;
        let payload_end = payload_start + bytes_to_follow as usize;
        if payload_end > bytes.len() {
            off += 1;
            continue;
        }
        visit(&bytes[payload_start..payload_end]);
        off = payload_end;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ];

    type ByteHist = BTreeMap<u8, usize>;
    type WordHist = BTreeMap<u16, usize>;
    type SizeBucket = (ByteHist, WordHist);
    // size_bucket -> (byte_at_offset_0_histogram, word_at_offset_0_histogram)
    let mut by_size: BTreeMap<usize, SizeBucket> = BTreeMap::new();
    // global histograms across all sizes
    let mut global_byte0: BTreeMap<u8, usize> = BTreeMap::new();
    let mut global_word0: BTreeMap<u16, usize> = BTreeMap::new();
    let mut global_byte1: BTreeMap<u8, usize> = BTreeMap::new();
    let mut record_count = 0usize;

    for fixture in fixtures {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
        let mut stream = cfb.open_stream("/Sheet6")?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        iter_records(&bytes, |payload| {
            record_count += 1;
            let size = payload.len();
            let entry = by_size.entry(size).or_default();
            if !payload.is_empty() {
                let b0 = payload[0];
                *entry.0.entry(b0).or_insert(0) += 1;
                *global_byte0.entry(b0).or_insert(0) += 1;
            }
            if payload.len() >= 2 {
                let w0 = u16::from_le_bytes([payload[0], payload[1]]);
                *entry.1.entry(w0).or_insert(0) += 1;
                *global_word0.entry(w0).or_insert(0) += 1;
                *global_byte1.entry(payload[1]).or_insert(0) += 1;
            }
        });
    }

    println!("\n=== Phase 19 PSM 0x0010 sub-kind discriminator probe ===");
    println!("Total records visited (advancing scan): {}", record_count);

    println!("\n--- Per-size bucket: top word@+0 (LE) ---");
    println!(
        "{:>5} | {:>5} | {:<40} | {:<40}",
        "size", "count", "top 3 word@+0 (LE)", "top 3 byte@+0"
    );
    for (size, (byte0_hist, word0_hist)) in &by_size {
        let bucket_count: usize = byte0_hist.values().sum();
        let mut top_words: Vec<_> = word0_hist.iter().collect();
        top_words.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        let mut top_bytes: Vec<_> = byte0_hist.iter().collect();
        top_bytes.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        let words: String = top_words
            .iter()
            .take(3)
            .map(|(w, c)| format!("0x{:04X}={} ({}%)", w, c, 100 * **c / bucket_count.max(1)))
            .collect::<Vec<_>>()
            .join(" / ");
        let bytes: String = top_bytes
            .iter()
            .take(3)
            .map(|(b, c)| format!("0x{:02X}={} ({}%)", b, c, 100 * **c / bucket_count.max(1)))
            .collect::<Vec<_>>()
            .join(" / ");
        println!("{:>5} | {:>5} | {:<40} | {:<40}", size, bucket_count, words, bytes);
    }

    println!("\n--- Global byte@+0 histogram (all sizes) ---");
    let mut sorted: Vec<_> = global_byte0.iter().collect();
    sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let total = sorted.iter().map(|(_, c)| **c).sum::<usize>();
    for (byte, count) in sorted.iter().take(12) {
        println!(
            "  0x{:02X} : {:>4}  ({}%)",
            byte,
            count,
            100 * **count / total.max(1)
        );
    }

    println!("\n--- Global byte@+1 histogram (all sizes) ---");
    let mut sorted: Vec<_> = global_byte1.iter().collect();
    sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let total = sorted.iter().map(|(_, c)| **c).sum::<usize>();
    for (byte, count) in sorted.iter().take(12) {
        println!(
            "  0x{:02X} : {:>4}  ({}%)",
            byte,
            count,
            100 * **count / total.max(1)
        );
    }

    println!("\n--- Global word@+0 (LE) histogram (all sizes) ---");
    let mut sorted: Vec<_> = global_word0.iter().collect();
    sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let total = sorted.iter().map(|(_, c)| **c).sum::<usize>();
    for (word, count) in sorted.iter().take(12) {
        println!(
            "  0x{:04X} : {:>4}  ({}%)",
            word,
            count,
            100 * **count / total.max(1)
        );
    }

    println!("\n--- Cross-size discriminator overlap matrix (top 6 words) ---");
    let mut top_words: Vec<_> = global_word0.iter().collect();
    top_words.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let top_words: Vec<u16> = top_words.iter().take(6).map(|(w, _)| **w).collect();
    print!("{:>5}", "size");
    for w in &top_words {
        print!(" | 0x{:04X}", w);
    }
    println!();
    for (size, (_, word0_hist)) in &by_size {
        let bucket_count: usize = word0_hist.values().sum();
        print!("{:>5}", size);
        for w in &top_words {
            let c = word0_hist.get(w).copied().unwrap_or(0);
            print!(" | {:>3} {:>2}%", c, 100 * c / bucket_count.max(1));
        }
        println!();
    }

    Ok(())
}
