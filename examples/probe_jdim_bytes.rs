//! The `0x0115 JDim` record laid out field by field: what does a driving
//! dimension of a parametric symbol definition actually persist?
//!
//! What was known (plan 2026-09-07, background line one, from a throwaway
//! dump on 09-07): 18 records across the corpus, all of them inside a
//! symbol definition cache (`/JSite<N>/PSMcluster0`) and all of them on a
//! layer named `Dimension`; six payload lengths (166 / 194 / 198 / 276 /
//! 288 / 308); the 18-byte envelope the seven graphic families share;
//! `+18 u32 2`; `+26` a flag word (`0x0351` / `0x0051` / `0x0341` /
//! `0x0361`); `+30 u32` equal to `payload - 38` on five of the six
//! lengths; `+42 f64` an inch multiple, read as the dimension value; three
//! f64 pairs around `+108` / `+124` / `+146`; `+162/+170 = ±1.0`.
//!
//! None of that is a reading yet -- it is the shape the bytes show. This
//! probe is step one of plan 2026-09-07 J1: put all 18 records side by side
//! under one candidate layout, and report where the layout closes and where
//! it does not, so the native reader (`imagdex.dex`'s `JDim Object`, step
//! two) has something specific to confirm or refute.
//!
//! The candidate layout it reads and checks:
//!
//! ```text
//! +0   u32 oid ; +4 u32 parent_ref ; +8 u32 sheet_layer_ref ;
//! +12  u16 sub_type 0 ; +14 u32 index 1            -- the shared envelope
//! +18  u32 2 ; +22 u32 6 ; +26 u16 flags ; +28 u16 0 ; +30 u32 main_len
//! +34  f64 ~3.05e-5 ; +42 f64 the dimension value ; +50 f64 ~3.05e-5 ;
//! +58  f64 ~4.88e-4 ; +66 f64 0 ; +74 f64 0     -- +34..+81 is one block,
//!                                                  48 bytes on this corpus
//! +82  u16 blocks ; +84 f64
//! then blocks x { u32 oid ; u16 assoc class ; u16 kind ; 8 zero bytes ;
//!                 f64 x,y ; f64 x,y ; u32 oid ; u16 assoc class ;
//!                 f64 x,y ; f64 x,y ; f64 x,y }  -- 102 bytes, 8 more
//!                 between one block and the next
//! then u32 oid   -- only when the flag word has 0x0100 set
//! ```
//!
//! Two things make this more than a guess about where a field starts. The
//! record has to close: `34 + main_len + tail` must be the payload length,
//! on all six lengths. And a reference slot has to resolve: the u32 must
//! be the oid of an object in the same storage.
//!
//! The native reader confirms the frame (J1 step three,
//! `tools/idalib_radsrv_igdimension.py`): `radsrvitem.dll!sub_564BB990`
//! takes the record header as its base, so its `a2+32` is the payload's
//! `+26`; it reads the closing dword at `a2+40+*(u32*)(a2+36)`, which is
//! `34 + main_len`, and only under bit `0x100`. `a2+40` -- the payload's
//! `+34` -- is where the block starts, and `sub_56446B50` sizes that block
//! at 48 bytes for the dimension kind this corpus uses, which is why `+82`
//! is where the next section begins. The u16 at the payload's `+14`, read
//! here as an index because it is 1 on all 18 records, is that kind: the
//! native reader switches on it over eight values, one block reader each.
//!
//! What it prints, per record: the envelope, the header words, the value in
//! mm and in inches, the blocks with their typed references and points,
//! every plausible f64 with its offset, and the annotated hex dump. Across
//! the corpus: the length account, the flag-word histogram against the
//! lengths and the tail, and which offsets hold a typed reference.
//!
//! ```powershell
//! cargo run --example probe_jdim_bytes
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::rad_class_name;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

const CHAIN_MAGIC: u32 = 0x6C90_F544;
const JDIM: u16 = 0x0115;
/// The envelope the seven graphic families share, in bytes.
const ENVELOPE: usize = 18;
/// Metres to millimetres: the unit the file's coordinates are in, and the
/// one every reading of a dimension value is easier to recognise in.
const MM: f64 = 1000.0;
/// Millimetres in an inch, because every dimension value so far is a
/// multiple of one.
const MM_PER_INCH: f64 = 25.4;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

fn leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

/// The storage key the parsed document uses: `/` for the root, `/JSite…`
/// without a trailing slash for a nested storage.
fn normalize_storage(storage: &str) -> String {
    let trimmed = storage.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

/// One record-chain stream of the file: where it sits and what it carries.
struct Chain {
    stream: String,
    storage: String,
    data: Vec<u8>,
    records: Vec<(u16, usize, Vec<u8>)>,
}

fn chains_of(path: &Path) -> Vec<Chain> {
    let mut out = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return out;
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return out;
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let mut records = Vec::new();
        for at in sheet_record_starts(&data) {
            let Some(type_code) = u16_at(&data, at).map(|word| word & 0x3FFF) else {
                continue;
            };
            let Some(len) = u32_at(&data, at + 2).map(|len| len as usize) else {
                continue;
            };
            let Some(payload) = data.get(at + 6..at + 6 + len) else {
                continue;
            };
            records.push((type_code, at, payload.to_vec()));
        }
        out.push(Chain {
            storage: normalize_storage(stream_path.rsplit_once('/').map_or("/", |(head, _)| head)),
            stream: stream_path,
            data,
            records,
        });
    }
    out
}

/// A candidate reference slot: a u32 of the main block that is the oid of
/// an object in the same storage, the type code of what it names, and the
/// u16 sitting right behind it -- the probe's question is whether that u16
/// is the target's class.
struct Reference {
    at: usize,
    oid: u32,
    target_type: u16,
    following: u16,
}

/// One `JDim` record with everything the probe reads off it.
struct Jdim {
    fixture: String,
    storage: String,
    oid: u32,
    parent_ref: u32,
    layer_ref: u32,
    layer_name: String,
    layer_number: Option<u32>,
    sub_type: u16,
    index: u32,
    payload: Vec<u8>,
    /// Every object of the same storage by oid, with its type code, for
    /// resolving the payload's reference slots.
    storage_types: BTreeMap<u32, u16>,
}

impl Jdim {
    fn len(&self) -> usize {
        self.payload.len()
    }

    fn flags(&self) -> u16 {
        u16_at(&self.payload, 26).unwrap_or_default()
    }

    fn main_len(&self) -> usize {
        u32_at(&self.payload, 30).unwrap_or_default() as usize
    }

    /// The four closing bytes are there when the flag word has `0x0100`
    /// set, and only then. The corpus alone cannot separate that bit from
    /// `0x0200` -- every record carrying one carries the other -- but the
    /// native reader can: `radsrvitem.dll!sub_564BB990` reads a dword at
    /// `34 + main_len` under `0x100` and nothing under `0x200`
    /// (plan 2026-09-07, J1 step three; `tools/idalib_radsrv_igdimension.py`).
    fn tail_len(&self) -> usize {
        if self.flags() & 0x0100 == 0 {
            0
        } else {
            4
        }
    }

    /// Whether `34 + main_len + tail` is the payload length.
    fn closes(&self) -> bool {
        ENVELOPE + 16 + self.main_len() + self.tail_len() == self.len()
    }

    /// The reference slot candidate at `at`, if the u32 there names an
    /// object of this storage.
    fn reference_at(&self, at: usize) -> Option<Reference> {
        let oid = u32_at(&self.payload, at)?;
        let target_type = *self.storage_types.get(&oid)?;
        (oid > 1).then_some(Reference {
            at,
            oid,
            target_type,
            following: u16_at(&self.payload, at + 4)?,
        })
    }

    /// Where the blocks live: from `+92` to the end of what `main_len`
    /// accounts for.
    fn block_area(&self) -> std::ops::Range<usize> {
        92..(ENVELOPE + 16 + self.main_len()).min(self.len())
    }

    /// Every reference slot of the block area: a u32 naming an object of
    /// this storage and followed by a non-zero word. That second condition
    /// is what separates a slot from the block's small `kind` numbers,
    /// which happen to equal an oid often enough to drown the signal --
    /// every slot the corpus shows is followed by one of a handful of
    /// marker words (`0x00CB` / `0x00F0` / `0x0067` / `0x008C`).
    fn references(&self) -> Vec<Reference> {
        self.block_area()
            .step_by(2)
            .filter_map(|at| self.reference_at(at))
            .filter(|reference| reference.following != 0)
            .collect()
    }

    /// Every offset of the payload holding a double that could be a length
    /// or a coordinate in metres: finite, and either exactly zero, exactly
    /// ±1, or between a micrometre and ten metres.
    fn plausible_f64s(&self) -> Vec<(usize, f64)> {
        let mut out = Vec::new();
        for at in ENVELOPE..self.len().saturating_sub(7) {
            let Some(value) = f64_at(&self.payload, at) else {
                continue;
            };
            let magnitude = value.abs();
            let plausible = value == 0.0
                || magnitude == 1.0
                || (value.is_finite() && (1e-6..=10.0).contains(&magnitude));
            if plausible && value != 0.0 {
                out.push((at, value));
            }
        }
        out
    }
}

fn hex_dump(payload: &[u8], notes: &BTreeMap<usize, String>) {
    for start in (0..payload.len()).step_by(16) {
        let end = (start + 16).min(payload.len());
        let row: Vec<String> = payload[start..end]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect();
        let ascii: String = payload[start..end]
            .iter()
            .map(|byte| {
                if (0x20..0x7F).contains(byte) {
                    *byte as char
                } else {
                    '.'
                }
            })
            .collect();
        let note: Vec<&str> = (start..end)
            .filter_map(|at| notes.get(&at).map(String::as_str))
            .collect();
        println!(
            "      +{start:<4} {:<47} | {ascii:<16} | {}",
            row.join(" "),
            note.join("  ")
        );
    }
}

fn collect(path: &Path, fixture: &str) -> Vec<Jdim> {
    let mut out = Vec::new();
    for chain in chains_of(path) {
        if !chain.records.iter().any(|(code, _, _)| *code == JDIM) {
            continue;
        }
        let layers = decode_sheet_layers(&chain.data);
        let storage_types: BTreeMap<u32, u16> = chain
            .records
            .iter()
            .filter_map(|(type_code, _, payload)| Some((u32_at(payload, 0)?, *type_code)))
            .collect();
        println!(
            "\n--- {} {} ({} records, {} of them JDim) ---",
            leaf(fixture),
            chain.stream,
            chain.records.len(),
            chain
                .records
                .iter()
                .filter(|(code, _, _)| *code == JDIM)
                .count()
        );
        for (type_code, _, payload) in &chain.records {
            if *type_code != JDIM || payload.len() < ENVELOPE {
                continue;
            }
            let layer_ref = u32_at(payload, 8).unwrap_or_default();
            let layer = layers.iter().find(|layer| layer.oid == layer_ref);
            out.push(Jdim {
                fixture: leaf(fixture).to_string(),
                storage: chain.storage.clone(),
                oid: u32_at(payload, 0).unwrap_or_default(),
                parent_ref: u32_at(payload, 4).unwrap_or_default(),
                layer_ref,
                layer_name: layer.map_or_else(|| "?".to_string(), |layer| layer.name.clone()),
                layer_number: layer.map(|layer| layer.layer_number),
                sub_type: u16_at(payload, 12).unwrap_or_default(),
                index: u32_at(payload, 14).unwrap_or_default(),
                payload: payload.clone(),
                storage_types: storage_types.clone(),
            });
        }
    }
    out
}

fn report(record: &Jdim, ordinal: usize) {
    println!(
        "\n  [{ordinal}] {} {} oid={} payload={}",
        record.fixture,
        record.storage,
        record.oid,
        record.len()
    );
    println!(
        "      envelope: parent_ref={} layer={} ({} #{}) sub_type=0x{:04X} index={}",
        record.parent_ref,
        record.layer_ref,
        record.layer_name,
        record
            .layer_number
            .map_or_else(|| "?".to_string(), |number| number.to_string()),
        record.sub_type,
        record.index
    );
    let word = |at: usize| {
        u32_at(&record.payload, at).map_or_else(|| "-".to_string(), |value| value.to_string())
    };
    println!(
        "      header:   +18={} +20={} +22={} flags=0x{:04X} +28={} main_len={} tail={} -> {}",
        u16_at(&record.payload, 18).unwrap_or_default(),
        u16_at(&record.payload, 20).unwrap_or_default(),
        word(22),
        record.flags(),
        u16_at(&record.payload, 28).unwrap_or_default(),
        record.main_len(),
        record.tail_len(),
        if record.closes() {
            format!("closes at {}", record.len())
        } else {
            format!(
                "DOES NOT CLOSE: 34+{}+{} != {}",
                record.main_len(),
                record.tail_len(),
                record.len()
            )
        }
    );
    let scalar = |at: usize| f64_at(&record.payload, at).unwrap_or_default();
    if let Some(value) = f64_at(&record.payload, 42) {
        println!(
            "      value:    {:.6} m = {:.3} mm = {:.4} in     (+34 {:.3} mm, +50 {:.3} mm, +58 {:.3} mm, +66 {:.3}, +74 {:.3})",
            value,
            value * MM,
            value * MM / MM_PER_INCH,
            scalar(34) * MM,
            scalar(50) * MM,
            scalar(58) * MM,
            scalar(66),
            scalar(74)
        );
    }
    let area = record.block_area();
    println!(
        "      blocks:   +82={} +84={:.3}  area +{}..+{} ({} bytes)",
        u16_at(&record.payload, 82).unwrap_or_default(),
        scalar(84),
        area.start,
        area.end,
        area.len()
    );
    for reference in record.references() {
        println!(
            "        +{:<4} -> oid {:<5} is 0x{:04X} {:<20} marker 0x{:04X} then {}",
            reference.at,
            reference.oid,
            reference.target_type,
            rad_class_name(reference.target_type).unwrap_or("?"),
            reference.following,
            u16_at(&record.payload, reference.at + 6).unwrap_or_default()
        );
    }
    let doubles = record.plausible_f64s();
    println!("      f64 sweep ({} plausible):", doubles.len());
    for chunk in doubles.chunks(4) {
        let row: Vec<String> = chunk
            .iter()
            .map(|(at, value)| format!("+{at:<3} {:>11.4} mm", value * MM))
            .collect();
        println!("        {}", row.join("  "));
    }

    let mut notes: BTreeMap<usize, String> = BTreeMap::new();
    notes.insert(0, "oid".to_string());
    notes.insert(4, "parent".to_string());
    notes.insert(8, "layer".to_string());
    notes.insert(12, "sub_type".to_string());
    notes.insert(14, "index".to_string());
    notes.insert(26, "flags".to_string());
    notes.insert(30, "main_len".to_string());
    notes.insert(42, "value".to_string());
    notes.insert(82, "blocks".to_string());
    for reference in record.references() {
        notes.insert(reference.at, format!("ref {}", reference.oid));
    }
    if record.tail_len() == 4 {
        notes.insert(record.len() - 4, "tail".to_string());
    }
    hex_dump(&record.payload, &notes);
}

fn corpus(records: &[Jdim]) {
    println!("\n=== corpus ===");
    println!("\n  the length account, per length:");
    let mut by_length: BTreeMap<usize, Vec<&Jdim>> = BTreeMap::new();
    for record in records {
        by_length.entry(record.len()).or_default().push(record);
    }
    for (length, group) in &by_length {
        let mains: BTreeSet<usize> = group.iter().map(|record| record.main_len()).collect();
        let tails: BTreeSet<usize> = group.iter().map(|record| record.tail_len()).collect();
        let flags: BTreeSet<String> = group
            .iter()
            .map(|record| format!("0x{:04X}", record.flags()))
            .collect();
        println!(
            "    {length:>3} bytes x{:<2}  34 + main_len {:?} + tail {:?}  flags {:?}  {}",
            group.len(),
            mains.iter().collect::<Vec<_>>(),
            tails.iter().collect::<Vec<_>>(),
            flags.iter().collect::<Vec<_>>(),
            if group.iter().all(|record| record.closes()) {
                "closes"
            } else {
                "DOES NOT CLOSE"
            }
        );
    }
    println!(
        "    {} of {} records close exactly.",
        records.iter().filter(|record| record.closes()).count(),
        records.len()
    );

    println!("\n  one line per record:");
    for record in records {
        println!(
            "    {:<24} {:<10} oid={:<5} len={:<4} flags=0x{:04X} blocks={} refs={} value={:>9.3} mm",
            record.fixture,
            record.storage,
            record.oid,
            record.len(),
            record.flags(),
            u16_at(&record.payload, 82).unwrap_or_default(),
            record.references().len(),
            f64_at(&record.payload, 42).unwrap_or_default() * MM
        );
    }

    println!("\n  the reference slots, by offset -- what they name and the marker behind them:");
    /// What the corpus shows at one offset of the block area.
    #[derive(Default)]
    struct Slot {
        records: usize,
        classes: BTreeSet<String>,
        markers: BTreeSet<String>,
    }
    let mut slots: BTreeMap<usize, Slot> = BTreeMap::new();
    for record in records {
        for reference in record.references() {
            let slot = slots.entry(reference.at).or_default();
            slot.records += 1;
            slot.classes.insert(format!(
                "0x{:04X} {}",
                reference.target_type,
                rad_class_name(reference.target_type).unwrap_or("?")
            ));
            slot.markers
                .insert(format!("0x{:04X}", reference.following));
        }
    }
    let mut common: Vec<(usize, Slot)> = slots.into_iter().collect();
    common.sort_by_key(|(at, slot)| (std::cmp::Reverse(slot.records), *at));
    for (at, slot) in &common {
        println!(
            "    +{at:<4} in {:>2} of {} records, marker {:?}: {:?}",
            slot.records,
            records.len(),
            slot.markers.iter().collect::<Vec<_>>(),
            slot.classes.iter().collect::<Vec<_>>()
        );
    }

    println!("\n  does the marker say which class the slot points at?");
    let mut pairing: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    for record in records {
        for reference in record.references() {
            *pairing
                .entry(reference.following)
                .or_default()
                .entry(format!(
                    "0x{:04X} {}",
                    reference.target_type,
                    rad_class_name(reference.target_type).unwrap_or("?")
                ))
                .or_default() += 1;
        }
    }
    for (marker, classes) in &pairing {
        let names: Vec<String> = classes
            .iter()
            .map(|(class, count)| format!("{class} x{count}"))
            .collect();
        println!("    0x{marker:04X} -> {}", names.join(", "));
    }
}

fn main() {
    let mut records = Vec::new();
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("\n=== {} (missing) ===", leaf(fixture));
            continue;
        }
        println!("\n=== {} ===", leaf(fixture));
        let found = collect(path, fixture);
        if found.is_empty() {
            println!("  no 0x0115 record");
        }
        records.extend(found);
    }
    println!("\n=== {} JDim records ===", records.len());
    for (ordinal, record) in records.iter().enumerate() {
        report(record, ordinal + 1);
    }
    corpus(&records);
}
