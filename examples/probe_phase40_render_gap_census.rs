//! Phase 40 census — where does every Sheet record actually go?
//!
//! Phase 39 closed with all three of its gap diagnoses overturned, each for
//! the same reason: the gap list was inherited rather than measured. This
//! re-measures it from the bottom, and it accounts for **every** record on the
//! chain rather than only the ones something already warns about.
//!
//! `undecoded_type_code_census` — the census the crate ships and OCS reports
//! — answers a narrower question than it appears to. It skips two buckets by
//! construction:
//!
//! ```text
//! if !claimed_here && !DECODED_TYPE_CODES.contains(&type_code) { count }
//! ```
//!
//! so a record whose type code *has* a decoder but whose bytes that decoder
//! **refused** is counted nowhere and warned about nowhere. Its own doc calls
//! that "a different diagnostic"; nothing in the crate emits that diagnostic.
//! A validation-rejected `igLine2d` therefore leaves the drawing in total
//! silence, which is the exact failure mode Phase 38 S2 set out to end for the
//! undecoded families.
//!
//! Every chain record is put in exactly one of four buckets:
//!
//! | bucket | meaning | who says so today |
//! |---|---|---|
//! | `claimed` | a decoder took the bytes | drawn, if the family emits |
//! | `rejected` | decoder exists, refused these bytes | **nobody — silent** |
//! | `warned` | no decoder, native predicate says it draws | named warning |
//! | `quiet` | no decoder, predicate says it draws nothing | quiet by design |
//!
//! ```powershell
//! cargo run --example probe_phase40_render_gap_census
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::model::SHEET_RECORD_FAMILIES;
use pid_parse::parsers::sheet_records::{decode_igline_at, sheet_record_starts};
use pid_parse::parsers::undecoded_census::{
    is_native_graphic_type_code, rad_class_name, DECODED_TYPE_CODES,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Which bucket a chain record falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Claimed,
    Rejected,
    Warned,
    Quiet,
}

impl Bucket {
    fn label(self) -> &'static str {
        match self {
            Self::Claimed => "claimed ",
            Self::Rejected => "REJECTED",
            Self::Warned => "warned  ",
            Self::Quiet => "quiet   ",
        }
    }
}

/// Same chain walk the shipped census uses, so the two cannot disagree about
/// what counts as a record.
fn psm_record_end(bytes: &[u8], off: usize) -> Option<usize> {
    let header_end = off.checked_add(6)?;
    if header_end > bytes.len() {
        return None;
    }
    if u16::from_le_bytes([bytes[off], bytes[off + 1]]) & 0x3FFF == 0 {
        return None;
    }
    let btf = u32::from_le_bytes([
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
    ]) as usize;
    if !(8..=100_000).contains(&btf) {
        return None;
    }
    let end = header_end.checked_add(btf)?;
    (end <= bytes.len()).then_some(end)
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name.to_string_lossy().into_owned(), bytes));
        }
    }
    out
}

/// Whether the family that owns `type_code` draws anything when it does
/// accept a record. `0x0013` was a no-op until Phase 39 S6; this reads the
/// registry rather than remembering.
fn family_emits(type_code: u16) -> Option<bool> {
    SHEET_RECORD_FAMILIES
        .iter()
        .find(|family| family.type_code == type_code)
        .map(|family| family.emits_geometry)
}

fn f64_at(payload: &[u8], at: usize) -> Option<f64> {
    let s = payload.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// Which of `igLine2d`'s four payload rules refuses this record, replayed in
/// the decoder's own order. The decoder only ever says `None`; a census that
/// stops there cannot tell "wrong record shape" from "coordinates the rule
/// distrusts", and those call for opposite work.
fn igline_refusal_reason(data: &[u8], off: usize) -> (&'static str, Option<[f64; 4]>) {
    let Some(payload) = data.get(off + 6..off + 6 + 50) else {
        return ("payload truncated", None);
    };
    let remaining_header = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    if remaining_header != 12 {
        return (
            match remaining_header {
                0 => "remaining_header == 0",
                4 => "remaining_header == 4",
                8 => "remaining_header == 8",
                16 => "remaining_header == 16",
                20 => "remaining_header == 20",
                _ => "remaining_header == other",
            },
            None,
        );
    }
    let mut d = [0f64; 4];
    for (i, slot) in d.iter_mut().enumerate() {
        match f64_at(payload, 18 + i * 8) {
            Some(value) => *slot = value,
            None => return ("payload truncated", None),
        }
    }
    if !d.iter().all(|x| x.is_finite()) {
        return ("coordinate not finite", Some(d));
    }
    if d.iter().any(|x| x.abs() > 1e9) {
        return ("coordinate beyond 1e9 domain", Some(d));
    }
    if (d[2] - d[0]).abs() < 1e-12 && (d[3] - d[1]).abs() < 1e-12 {
        return ("degenerate: start == end", Some(d));
    }
    ("no rule fires -- claimed elsewhere?", Some(d))
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // (type_code, bucket) -> count, across the whole corpus.
    let mut corpus: BTreeMap<(u16, Bucket), usize> = BTreeMap::new();
    // Streams where a rejected record appears, for the follow-up.
    let mut reject_sites: Vec<(String, String, u16, usize)> = Vec::new();
    // (type_code, bucket) -> bytes_to_follow -> count. A refused record whose
    // declared length differs from every accepted one is a shape the decoder
    // does not know, not a coincidence.
    let mut lengths: BTreeMap<(u16, Bucket), BTreeMap<usize, usize>> = BTreeMap::new();
    // Does the strict chain walk agree that the refused offsets are records?
    let mut strict_agreement: Vec<(String, String, usize, usize, usize)> = Vec::new();
    // Which igLine2d rule refuses, and one worked example per reason.
    let mut igline_reasons: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut igline_examples: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut decoder_disagrees = 0usize;

    for fixture in FIXTURES {
        let path = root.join(fixture);
        if !path.exists() {
            eprintln!("skip: {fixture} not present");
            continue;
        }
        println!("\n=== {fixture} ===");

        for (stream_name, data) in sheet_streams(&path) {
            let claimed: Vec<core::ops::Range<usize>> = SHEET_RECORD_FAMILIES
                .iter()
                .flat_map(|family| (family.decoded_ranges)(&data))
                .collect();

            // The strict chain: empty unless a single unsliding walk reaches
            // the stream's end. Phase 39 S5 made this the membership test that
            // retracted the `0x3FE6` family, so a refused offset that is on it
            // is a record by the same standard that unmade a phantom family.
            let strict = sheet_record_starts(&data);

            let mut per_stream: BTreeMap<(u16, Bucket), usize> = BTreeMap::new();
            let mut records = 0usize;
            let mut rejected_on_strict = 0usize;
            let mut rejected_off_strict = 0usize;
            let mut off = 0usize;
            while off + 6 <= data.len() {
                if let Some(end) = psm_record_end(&data, off) {
                    if end == data.len() || psm_record_end(&data, end).is_some() {
                        let type_code = u16::from_le_bytes([data[off], data[off + 1]]) & 0x3FFF;
                        let bucket = if claimed.iter().any(|range| range.contains(&off)) {
                            Bucket::Claimed
                        } else if DECODED_TYPE_CODES.contains(&type_code) {
                            Bucket::Rejected
                        } else if is_native_graphic_type_code(type_code) {
                            Bucket::Warned
                        } else {
                            Bucket::Quiet
                        };
                        *per_stream.entry((type_code, bucket)).or_default() += 1;
                        *corpus.entry((type_code, bucket)).or_default() += 1;
                        *lengths
                            .entry((type_code, bucket))
                            .or_default()
                            .entry(end - off - 6)
                            .or_default() += 1;
                        if bucket == Bucket::Rejected {
                            if strict.contains(&off) {
                                rejected_on_strict += 1;
                            } else {
                                rejected_off_strict += 1;
                            }
                            if type_code == 0x0018 {
                                // Ask the shipped decoder directly, so the
                                // replay below cannot drift from it.
                                if decode_igline_at(&data, off).is_some() {
                                    decoder_disagrees += 1;
                                }
                                let (reason, coords) = igline_refusal_reason(&data, off);
                                *igline_reasons.entry(reason).or_default() += 1;
                                igline_examples.entry(reason).or_insert_with(|| {
                                    match coords {
                                        Some(d) => format!(
                                            "{stream_name} +{off}: ({:.6}, {:.6}) -> ({:.6}, {:.6})",
                                            d[0], d[1], d[2], d[3]
                                        ),
                                        None => {
                                            let hex = data[off..end]
                                                .iter()
                                                .map(|b| format!("{b:02x}"))
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                            format!("{stream_name} +{off}\n               {hex}")
                                        }
                                    }
                                });
                            }
                        }
                        records += 1;
                        off = end;
                        continue;
                    }
                }
                off += 1;
            }
            if rejected_on_strict + rejected_off_strict > 0 {
                strict_agreement.push((
                    (*fixture).to_string(),
                    stream_name.clone(),
                    strict.len(),
                    rejected_on_strict,
                    rejected_off_strict,
                ));
            }

            let rejected: usize = per_stream
                .iter()
                .filter(|((_, bucket), _)| *bucket == Bucket::Rejected)
                .map(|(_, count)| *count)
                .sum();
            println!(
                "\n  {stream_name}  ({} bytes, {records} chain records, {rejected} REJECTED)",
                data.len()
            );
            for ((type_code, bucket), count) in &per_stream {
                let name = rad_class_name(*type_code).unwrap_or("?");
                let emits = match family_emits(*type_code) {
                    Some(true) => "",
                    Some(false) => "  [family emits nothing by policy]",
                    None => "",
                };
                println!(
                    "    0x{type_code:04X} {:<24} {} {count:5}{emits}",
                    name,
                    bucket.label()
                );
                if *bucket == Bucket::Rejected {
                    reject_sites.push((
                        (*fixture).to_string(),
                        stream_name.clone(),
                        *type_code,
                        *count,
                    ));
                }
            }
        }
    }

    println!("\n========================================================");
    println!("CORPUS ROLL-UP — every chain record, by type code and bucket");
    println!("========================================================");
    let mut by_code: BTreeMap<u16, BTreeMap<Bucket, usize>> = BTreeMap::new();
    for ((type_code, bucket), count) in &corpus {
        *by_code
            .entry(*type_code)
            .or_default()
            .entry(*bucket)
            .or_default() += count;
    }
    println!(
        "  {:<6} {:<24} {:>8} {:>9} {:>8} {:>7}",
        "code", "class", "claimed", "REJECTED", "warned", "quiet"
    );
    for (type_code, buckets) in &by_code {
        let get = |bucket: Bucket| buckets.get(&bucket).copied().unwrap_or(0);
        println!(
            "  0x{type_code:04X} {:<24} {:>8} {:>9} {:>8} {:>7}",
            rad_class_name(*type_code).unwrap_or("?"),
            get(Bucket::Claimed),
            get(Bucket::Rejected),
            get(Bucket::Warned),
            get(Bucket::Quiet),
        );
    }

    println!("\n========================================================");
    println!("THE SILENT BUCKET — decoder exists, refused these bytes,");
    println!("nothing warns, nothing draws");
    println!("========================================================");
    if reject_sites.is_empty() {
        println!("  (empty)");
    }
    for (fixture, stream, type_code, count) in &reject_sites {
        println!(
            "  {count:5} x 0x{type_code:04X} {:<22} {stream}  in {fixture}",
            rad_class_name(*type_code).unwrap_or("?"),
        );
    }

    println!("\n========================================================");
    println!("IS THE SILENT BUCKET REAL? — refused offsets vs the strict");
    println!("chain that retracted the 0x3FE6 phantom family in Phase 39");
    println!("========================================================");
    println!(
        "  {:<10} {:>7} {:>10} {:>11}  stream",
        "strict len", "on", "off", "verdict"
    );
    for (fixture, stream, strict_len, on, off) in &strict_agreement {
        let verdict = if *strict_len == 0 {
            "no chain"
        } else if *off == 0 {
            "all records"
        } else {
            "MIXED"
        };
        println!("  {strict_len:<10} {on:>7} {off:>10} {verdict:>11}  {stream} ({fixture})");
    }

    println!("\n========================================================");
    println!("WHICH RULE REFUSES THE 0x0018 LINES?");
    println!("========================================================");
    println!("  decoder accepts an offset the census called refused: {decoder_disagrees}");
    for (reason, count) in &igline_reasons {
        println!("  {count:5} x {reason}");
        if let Some(example) = igline_examples.get(reason) {
            println!("          e.g. {example}");
        }
    }

    println!("\n========================================================");
    println!("WHY REFUSED? — declared payload length, accepted vs refused");
    println!("========================================================");
    for type_code in [0x0018u16, 0x004D, 0x0084] {
        let show = |bucket: Bucket| -> String {
            lengths
                .get(&(type_code, bucket))
                .map(|hist| {
                    hist.iter()
                        .map(|(len, count)| format!("{len}x{count}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_else(|| "-".to_string())
        };
        println!(
            "  0x{type_code:04X} {:<20}\n    accepted: {}\n    refused : {}",
            rad_class_name(type_code).unwrap_or("?"),
            show(Bucket::Claimed),
            show(Bucket::Rejected),
        );
    }
}
