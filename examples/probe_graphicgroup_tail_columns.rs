//! What is in the `0x00FA GraphicGroup` raw tail.
//!
//! `GraphicGroup` is the most numerous record family in every fixture (458
//! cross-fixture hits) and the only high-volume one whose payload past offset
//! 18 is still kept as raw bytes. Symbology -- line weight, colour, style --
//! cannot be in the geometry primitives, because `igLine2d` (50 bytes) and
//! `igPoint2d` (34 bytes) are both fixed-length and fully accounted for. That
//! leaves this tail and the style cluster.
//!
//! Method: bucket records by `(bytes_to_follow, group_kind_word)` so only
//! same-shaped records are compared, then read each bucket column-wise. A
//! column that never varies across dozens of records is a constant the format
//! writes; one that varies is data. Each 4-byte window is also tested against
//! the set of OIDs the sheet actually defines, which separates references from
//! values.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_graphic_groups, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igsymbols,
    decode_igtextboxes,
};

/// Buckets smaller than this cannot tell a constant from a coincidence.
const MIN_BUCKET: usize = 4;

/// A column with at most this many distinct values is an enum rather than a
/// payload; worth printing the values themselves.
const ENUM_MAX_DISTINCT: usize = 6;

struct Bucket {
    tails: Vec<Vec<u8>>,
    oids: Vec<u32>,
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for p in paths {
        let Ok(mut stream) = cfb.open_stream(&p) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((p, bytes));
        }
    }
    out
}

/// Every OID the sheet's decoded geometry defines. A tail word matching one of
/// these is a reference; one that matches nothing is a value.
fn known_oids(data: &[u8]) -> BTreeSet<u32> {
    let mut oids = BTreeSet::new();
    oids.extend(decode_iglines(data).iter().map(|r| r.oid));
    oids.extend(decode_igpoints(data).iter().map(|r| r.oid));
    oids.extend(decode_iglinestrings(data).iter().map(|r| r.oid));
    oids.extend(decode_igtextboxes(data).iter().map(|r| r.oid));
    oids.extend(decode_igsymbols(data).iter().map(|r| r.oid));
    oids.extend(decode_graphic_groups(data).iter().map(|r| r.oid));
    oids
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn report_bucket(label: &str, bucket: &Bucket, oids: &BTreeSet<u32>) {
    let width = bucket.tails[0].len();
    if bucket.tails.iter().any(|t| t.len() != width) {
        return;
    }
    println!(
        "\n  -- {label}  n={}  tail_width={width} (payload +18..+{}) --",
        bucket.tails.len(),
        18 + width
    );

    // Column-wise: constant vs enum vs wide.
    let mut constants: Vec<(usize, u8)> = Vec::new();
    let mut enums: Vec<(usize, Vec<u8>)> = Vec::new();
    let mut wide = 0usize;
    for column in 0..width {
        let values: BTreeSet<u8> = bucket.tails.iter().map(|t| t[column]).collect();
        match values.len() {
            1 => constants.push((column, *values.iter().next().unwrap())),
            n if n <= ENUM_MAX_DISTINCT => enums.push((column, values.into_iter().collect())),
            _ => wide += 1,
        }
    }

    let constant_run: Vec<String> = constants
        .iter()
        .map(|(c, v)| format!("+{}:{v:02X}", 18 + c))
        .collect();
    println!(
        "     constant columns ({}/{width}): {}",
        constants.len(),
        if constant_run.is_empty() {
            "none".into()
        } else {
            constant_run.join(" ")
        }
    );
    for (column, values) in &enums {
        let shown: Vec<String> = values.iter().map(|v| format!("{v:02X}")).collect();
        println!(
            "     enum column +{:<3} {} distinct: {}",
            18 + column,
            values.len(),
            shown.join(",")
        );
    }
    println!("     high-variety columns: {wide}");

    // 4-byte windows: reference to a real object, or a value?
    println!("     4-byte windows (aligned to tail start):");
    for at in (0..width.saturating_sub(3)).step_by(4) {
        let words: Vec<u32> = bucket.tails.iter().filter_map(|t| u32_at(t, at)).collect();
        if words.len() != bucket.tails.len() {
            continue;
        }
        let distinct: BTreeSet<u32> = words.iter().copied().collect();
        let hits = words.iter().filter(|w| oids.contains(w)).count();
        let verdict = if hits * 2 > words.len() {
            format!("REFERENCE ({hits}/{} match a real OID)", words.len())
        } else if distinct.len() == 1 {
            let only = *distinct.iter().next().unwrap();
            format!("constant 0x{only:08X} ({})", only as i32)
        } else if distinct.len() <= ENUM_MAX_DISTINCT {
            let shown: Vec<String> = distinct.iter().map(|v| format!("0x{v:X}")).collect();
            format!("enum {{{}}}", shown.join(","))
        } else {
            format!("{} distinct values", distinct.len())
        };
        println!("       +{:<3} {verdict}", 18 + at);
    }
}

/// Which family each OID belongs to, so a tail word that references one can be
/// asked what it points at.
fn oid_families(data: &[u8]) -> BTreeMap<u32, &'static str> {
    let mut map = BTreeMap::new();
    for r in decode_iglines(data) {
        map.insert(r.oid, "igLine2d");
    }
    for r in decode_igpoints(data) {
        map.insert(r.oid, "igPoint2d");
    }
    for r in decode_iglinestrings(data) {
        map.insert(r.oid, "igLineString2d");
    }
    for r in decode_igtextboxes(data) {
        map.insert(r.oid, "igTextBox");
    }
    for r in decode_igsymbols(data) {
        map.insert(r.oid, "igSymbol2d");
    }
    map
}

/// The decisive test for the `+50..+65` block: a display property varies
/// between objects of the same family; a type tag does not. Cross-tabulating
/// the candidate words against the family the record points at separates them.
fn correlate_with_family(bucket: &Bucket, families: &BTreeMap<u32, &'static str>) {
    // `+34` is the window that references a real OID in every record.
    const REF_AT: usize = 34 - 18;
    let candidates = [(50usize, "+50"), (54, "+54"), (58, "+58"), (62, "+62")];

    println!("     candidate word x referenced family:");
    for (payload_off, label) in candidates {
        let at = payload_off - 18;
        let mut table: BTreeMap<&'static str, BTreeSet<i32>> = BTreeMap::new();
        for tail in &bucket.tails {
            let (Some(target), Some(value)) = (u32_at(tail, REF_AT), u32_at(tail, at)) else {
                continue;
            };
            let family = families.get(&target).copied().unwrap_or("(unresolved)");
            table.entry(family).or_default().insert(value as i32);
        }
        let cells: Vec<String> = table
            .iter()
            .map(|(family, values)| {
                let shown: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                format!("{family}={{{}}}", shown.join(","))
            })
            .collect();
        let varies = table.values().any(|v| v.len() > 1);
        println!(
            "       {label} {:<9} {}",
            if varies { "PROPERTY" } else { "tag-like" },
            cells.join("  ")
        );
    }
}

/// Every record ends in a self-describing block: three `u16` -- a block type,
/// a small selector, and the block's byte length -- followed by exactly that
/// many bytes, running to the end of the record.
///
/// This is what makes the property block locatable without knowing the record
/// length: read the last `len` bytes, not a fixed offset. `0x0077` looked like
/// a fixed anchor only because the buckets it appears in happen to share a
/// layout; `0x0007` marks the same shape in the others.
struct Trailer {
    /// Tail offset of the 6-byte descriptor.
    at: usize,
    block_type: u16,
    selector: u16,
    len: u16,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// Find the descriptor whose declared length runs exactly to the record's end.
///
/// Scanned forward so the outermost block wins: a record ends in zeros, and a
/// zero-length descriptor sitting on them satisfies the equation trivially at
/// the back. A block also has to be wide enough to hold something.
fn find_trailer(tail: &[u8]) -> Option<Trailer> {
    const MIN_BLOCK: usize = 4;
    (0..tail.len().saturating_sub(5)).find_map(|at| {
        let len = u16_at(tail, at + 4)? as usize;
        (len >= MIN_BLOCK && at + 6 + len == tail.len()).then(|| Trailer {
            at,
            block_type: u16_at(tail, at).unwrap_or(0),
            selector: u16_at(tail, at + 2).unwrap_or(0),
            len: len as u16,
        })
    })
}

/// Test the trailer hypothesis on every bucket, and read the block it locates.
fn trailer_scan(buckets: &BTreeMap<(u32, u16), Bucket>) {
    println!("\n  == trailer scan: <type u16><selector u16><len u16> + len bytes ==");
    let (mut found, mut total) = (0usize, 0usize);
    for ((btf, kind), bucket) in buckets {
        let n = bucket.tails.len();
        total += n;
        let trailers: Vec<Option<Trailer>> = bucket.tails.iter().map(|t| find_trailer(t)).collect();
        let hits = trailers.iter().filter(|t| t.is_some()).count();
        found += hits;
        if hits == 0 {
            println!("  btf={btf:<4} kind={kind} n={n:<3} NO TRAILER");
            continue;
        }
        let types: BTreeSet<u16> = trailers.iter().flatten().map(|t| t.block_type).collect();
        let selectors: BTreeSet<u16> = trailers.iter().flatten().map(|t| t.selector).collect();
        let lens: BTreeSet<u16> = trailers.iter().flatten().map(|t| t.len).collect();
        let at: BTreeSet<usize> = trailers.iter().flatten().map(|t| t.at + 18).collect();
        println!(
            "  btf={btf:<4} kind={kind} n={n:<3} {hits}/{n}  type={:<8} selector={:<6} len={:<6} descriptor@payload{}",
            types.iter().map(|v| format!("0x{v:04X}")).collect::<Vec<_>>().join(","),
            selectors.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
            lens.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
            at.iter().map(|v| format!("+{v}")).collect::<Vec<_>>().join(",")
        );

        // Read the located block column-wise as u32 words.
        if at.len() != 1 || lens.len() != 1 || hits != n {
            continue;
        }
        let base = trailers[0].as_ref().unwrap().at + 6;
        let len = *lens.iter().next().unwrap() as usize;
        for word in 0..(len / 4).min(8) {
            let word_at = base + word * 4;
            let values: Vec<u32> = bucket
                .tails
                .iter()
                .filter_map(|t| u32_at(t, word_at))
                .collect();
            if values.len() != n {
                break;
            }
            let distinct: BTreeSet<u32> = values.iter().copied().collect();
            let verdict = if distinct.len() == 1 {
                format!("constant {}", *distinct.iter().next().unwrap() as i32)
            } else if distinct.len() <= 8 {
                let mut sorted: Vec<i32> = distinct.iter().map(|v| *v as i32).collect();
                sorted.sort_unstable();
                format!(
                    "enum {{{}}}",
                    sorted
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            } else {
                format!("{} distinct", distinct.len())
            };
            println!(
                "      block[{word}] (payload +{:<3}) {verdict}",
                word_at + 18
            );
        }
    }
    println!("  -> trailer located in {found}/{total} record(s)");
}

fn probe(path: &Path) {
    for (stream, data) in sheet_streams(path) {
        let groups = decode_graphic_groups(&data);
        if groups.is_empty() {
            continue;
        }
        let oids = known_oids(&data);
        let families = oid_families(&data);
        println!(
            "\n=== {} {stream} === {} GraphicGroup record(s), {} known OID(s)",
            path.display(),
            groups.len(),
            oids.len()
        );

        let mut buckets: BTreeMap<(u32, u16), Bucket> = BTreeMap::new();
        for group in &groups {
            let entry = buckets
                .entry((group.bytes_to_follow, group.group_kind_word))
                .or_insert_with(|| Bucket {
                    tails: Vec::new(),
                    oids: Vec::new(),
                });
            entry.tails.push(group.raw_reference_payload.clone());
            entry.oids.push(group.oid);
        }

        for ((btf, kind), bucket) in &buckets {
            if bucket.tails.len() < MIN_BUCKET {
                continue;
            }
            report_bucket(&format!("btf={btf} group_kind={kind}"), bucket, &oids);
            if *btf == 66 && *kind == 2 {
                correlate_with_family(bucket, &families);
            }
        }

        let skipped: usize = buckets
            .values()
            .filter(|b| b.tails.len() < MIN_BUCKET)
            .map(|b| b.tails.len())
            .sum();
        println!(
            "\n  ({skipped} record(s) in buckets smaller than {MIN_BUCKET}, not column-analysed)"
        );

        trailer_scan(&buckets);
    }
}

fn main() {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path);
        }
    }
}
