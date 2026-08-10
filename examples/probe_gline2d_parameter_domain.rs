//! Why `GLine2d` (PSM `0x3FE6`) is not a record family.
//!
//! Phase 39 called these gap F and blamed an unresolved parameter domain: a
//! handful of "records" decoding to a segment one source unit long from near
//! the origin, kept off the drawing on a hidden `PID-UNRESOLVED` layer because
//! a 1000 mm rule across an A2 sheet ruins the framing. The parameter domain
//! was never unresolved — the decoder reads a `param_start` / `param_end` pair
//! and validates the order — so this probe printed the values, then asked the
//! question one level up: are these offsets where records start?
//!
//! They are not. Each sits 160 bytes inside an `igSmartFrame2d`, on the top two
//! bytes of its `1/√2` page-ratio `f64`, which spell `E6 3F`. The probe shows
//! both halves: each match in full (chain membership, the `f64` its type word
//! is part of, the six doubles, the trailing bytes), then a corpus tally of
//! every page frame against the payload rules.
//!
//! `decode_primitive_lines` now requires chain membership and finds none of
//! this, so the scan below is kept locally — the evidence has to stay
//! reproducible after the thing it describes is fixed.
//!
//! ```powershell
//! cargo run --example probe_gline2d_parameter_domain
//! ```

use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_primitive_line_at, SheetPrimitiveLineDecoded, GLINE2D_PAYLOAD_LEN, PSM_RECORD_HEADER_LEN,
};

/// Every offset whose bytes satisfy the `GLine2d` payload rules, found the
/// way the decoder used to look: try each offset, jump past a hit, slide one
/// byte on a miss.
///
/// `decode_primitive_lines` no longer works this way — it requires chain
/// membership, which is what retired this family — so this probe keeps the
/// old scan to reproduce the artifact it retired. Deleting the scan would
/// leave the analysis note pointing at a probe that prints nothing.
fn scan_for_payload_matches(data: &[u8]) -> Vec<SheetPrimitiveLineDecoded> {
    let mut out: Vec<SheetPrimitiveLineDecoded> = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if let Some(record) = decode_primitive_line_at(data, at) {
            at = record.byte_range.end.max(at + 1);
            out.push(record);
            continue;
        }
        at += 1;
    }
    out
}

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    // The gongyi fixture, escaped so this source stays pure ASCII.
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Millimetres in the metre the decoded sheet coordinates come in, so the
/// numbers can be read against a sheet the way the importer sees them.
const MM_PER_METRE: f64 = 1000.0;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with("Sheet"))
        })
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name, bytes));
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    let s = data.get(at..at.checked_add(8)?)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// Walk a `Sheet*` stream as the strict record chain it is: every record is
/// `u16 type_word`, `u32 bytes_to_follow`, then exactly that many payload
/// bytes, nose to tail. No sliding, no re-syncing.
///
/// This is what tells a record from a byte pattern that merely looks like one.
/// The decoders in `sheet_records` *scan* — they try every offset — so a header
/// shape occurring inside another record's payload is accepted as a record.
/// The chain says where records actually start.
///
/// Returns `(record starts, offset the walk stalled at)`.
fn walk_record_chain(data: &[u8], start: usize) -> (Vec<(usize, u16, usize)>, usize) {
    let mut records = Vec::new();
    let mut at = start;
    while at + 6 <= data.len() {
        let type_word = u16::from_le_bytes([data[at], data[at + 1]]);
        let btf =
            u32::from_le_bytes([data[at + 2], data[at + 3], data[at + 4], data[at + 5]]) as usize;
        let Some(end) = at.checked_add(6).and_then(|body| body.checked_add(btf)) else {
            break;
        };
        if btf == 0 || end > data.len() {
            break;
        }
        records.push((at, type_word & 0x3FFF, end));
        at = end;
    }
    (records, at)
}

/// Is this offset where a record starts, or somewhere inside one?
fn chain_verdict(data: &[u8], offset: usize) -> String {
    // A Sheet stream opens with an 8-byte header on the cluster families; try
    // both plausible starts and keep whichever walk reaches furthest.
    let (from_zero, end_zero) = walk_record_chain(data, 0);
    let (from_eight, end_eight) = walk_record_chain(data, 8);
    let (records, reached, start) = if end_eight >= end_zero {
        (from_eight, end_eight, 8)
    } else {
        (from_zero, end_zero, 0)
    };
    let coverage = if data.is_empty() {
        0.0
    } else {
        reached as f64 * 100.0 / data.len() as f64
    };
    let mut verdict = format!(
        "chain from +{start}: {} record(s), reached {reached}/{} bytes ({coverage:.1}%)",
        records.len(),
        data.len()
    );
    if records.iter().any(|(at, _, _)| *at == offset) {
        verdict.push_str("; the offset IS a record start");
        return verdict;
    }
    match records
        .iter()
        .find(|(at, _, end)| *at < offset && offset < *end)
    {
        Some((at, type_code, end)) => verdict.push_str(&format!(
            "; the offset sits INSIDE the 0x{type_code:04X} record at {at}..{end}, {} byte(s) in",
            offset - at
        )),
        None => verdict.push_str("; the chain does not reach the offset"),
    }
    verdict
}

/// Type code of the OLE container frame the page border arrives as.
const IG_SMART_FRAME_2D: u16 = 0x003D;

/// Offset of the frame's width/height ratio within its payload, per
/// `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`: constant
/// ≈ 0.7072 = 1/√2 on every ISO A-series frame in the corpus.
const SMART_FRAME_RATIO_OFFSET: usize = 148;

/// Where the ratio's top two bytes land inside the record: payload `+148`
/// starts at record `+154`, so its last two bytes are record `+160`.
const RATIO_HIGH_WORD_IN_RECORD: usize = SMART_FRAME_RATIO_OFFSET + 6 + 6;

/// Count how the phantom arises: which frames carry a ratio whose top two
/// bytes spell `0x3FE6`, and which of those go on to satisfy every remaining
/// `GLine2d` rule.
///
/// This is what separates "the offsets coincide" from "the offsets coincide
/// because of this field". If the ratio is the source, every phantom sits at
/// exactly this offset in a frame, and every frame that does not produce one
/// fails a rule further along rather than lacking the `E6 3F` pair.
fn report_frame_mechanism(fixtures: &[PathBuf]) {
    println!("\n=== the frames these come out of ===\n");
    let mut frames = 0usize;
    let mut ratio_spells_the_type_code = 0usize;
    let mut decodes_as_a_line = 0usize;

    for fixture in fixtures {
        for (stream_path, sheet) in sheet_streams(fixture) {
            let (records, _) = walk_record_chain(&sheet, 8);
            for (at, type_code, _) in records
                .iter()
                .filter(|(_, code, _)| *code == IG_SMART_FRAME_2D)
            {
                frames += 1;
                let ratio = f64_at(&sheet, at + 6 + SMART_FRAME_RATIO_OFFSET);
                let phantom_at = at + RATIO_HIGH_WORD_IN_RECORD;
                let spells = sheet
                    .get(phantom_at..phantom_at + 2)
                    .is_some_and(|pair| pair == [0xE6, 0x3F]);
                if spells {
                    ratio_spells_the_type_code += 1;
                }
                let decodes =
                    pid_parse::parsers::sheet_records::decode_primitive_line_at(&sheet, phantom_at)
                        .is_some();
                if decodes {
                    decodes_as_a_line += 1;
                }
                println!(
                    "  0x{type_code:04X} at {at:>6} in {stream_path:<24} ratio={} top word spells 0x3FE6: {} decodes as a line: {}",
                    ratio.map_or_else(|| "     ?    ".to_string(), |v| format!("{v:.9}")),
                    if spells { "yes" } else { "no " },
                    if decodes { "yes" } else { "no" }
                );
            }
        }
    }

    println!(
        "\n  {frames} frame(s); {ratio_spells_the_type_code} carry a ratio whose top word reads \
         0x3FE6; {decodes_as_a_line} of those pass every remaining GLine2d rule."
    );
}

fn main() {
    let root = test_file_root();
    let mut total = 0usize;

    for name in FIXTURES {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        for (stream_path, sheet) in sheet_streams(&path) {
            let records = scan_for_payload_matches(&sheet);
            if records.is_empty() {
                continue;
            }
            println!("\n=== {name}  {stream_path} ===");
            for record in &records {
                total += 1;
                let (origin_x, origin_y) = record.origin;
                let (dir_x, dir_y) = record.direction;
                let (ax, ay) = record.endpoint_a();
                let (bx, by) = record.endpoint_b();
                let span = record.param_end - record.param_start;

                println!(
                    "\n  oid={} type_flags=0x{:X} bytes_to_follow={} bytes {}..{}",
                    record.oid,
                    record.type_flags,
                    record.bytes_to_follow,
                    record.byte_range.start,
                    record.byte_range.end
                );
                println!("    origin    = ({origin_x:.9}, {origin_y:.9})");
                println!(
                    "    direction = ({dir_x:.9}, {dir_y:.9})  |d| = {:.12}",
                    dir_x.hypot(dir_y)
                );
                println!(
                    "    param     = [{:.9}, {:.9}]  span = {span:.9} ({:.3}mm)",
                    record.param_start,
                    record.param_end,
                    span * MM_PER_METRE
                );
                println!(
                    "    endpoints = ({:.3}mm, {:.3}mm) -> ({:.3}mm, {:.3}mm)",
                    ax * MM_PER_METRE,
                    ay * MM_PER_METRE,
                    bx * MM_PER_METRE,
                    by * MM_PER_METRE
                );

                println!("    {}", chain_verdict(&sheet, record.byte_range.start));
                // If `E6 3F` is the tail of a double rather than a type word,
                // the six bytes before it complete one.
                if record.byte_range.start >= 6 {
                    let at = record.byte_range.start - 6;
                    if let Some(value) = f64_at(&sheet, at) {
                        println!(
                            "    the 8 bytes ending at the type word read {value:.9} as f64: {}",
                            hex(sheet.get(at..at + 8).unwrap_or_default())
                        );
                    }
                }

                let Some(whole) = sheet.get(record.byte_range.clone()) else {
                    continue;
                };
                let payload_end = PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN;
                println!("    header    = {}", hex(&whole[..PSM_RECORD_HEADER_LEN]));
                for (i, chunk) in whole[PSM_RECORD_HEADER_LEN..payload_end.min(whole.len())]
                    .chunks(8)
                    .enumerate()
                {
                    println!("    double {i}  = {}", hex(chunk));
                }
                match whole.get(payload_end..) {
                    None | Some([]) => {
                        println!("    trailer   = none; the record is header + 6 doubles exactly");
                    }
                    Some(trailer) => {
                        println!(
                            "    trailer   = {} byte(s): {}",
                            trailer.len(),
                            hex(trailer)
                        );
                        // A trailing double in the sheet's own coordinate range
                        // would be the extent the parametric form is missing.
                        for at in 0..trailer.len().saturating_sub(7) {
                            let slice = &trailer[at..at + 8];
                            let value = f64::from_le_bytes([
                                slice[0], slice[1], slice[2], slice[3], slice[4], slice[5],
                                slice[6], slice[7],
                            ]);
                            if value.is_normal() && (1e-4..=2.0).contains(&value.abs()) {
                                println!(
                                    "      +{at:<3} reads {value:.9} ({:.3}mm) — sheet-sized",
                                    value * MM_PER_METRE
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    println!(
        "\n{total} offset(s) in the corpus satisfy every GLine2d payload rule; \
         none of them is a record."
    );

    let fixtures: Vec<PathBuf> = FIXTURES
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect();
    report_frame_mechanism(&fixtures);
}
