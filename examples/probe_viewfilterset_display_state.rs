//! The `0x0057 Top ViewFilterSet` record: where does the layer display state
//! live, and does every byte of every record account for itself?
//!
//! What was known (08-27, `tag-184-is-the-sheet-layer-view-hierarchy`): `+0`
//! oid, `+12` form number 2, `+16` the `JSheet`, `+20..+28` constants
//! `1 1 0`, `+32` unidentified (not the layer count), then "a run of
//! display-state bytes" and a table of `{u32 chars; UTF-16 name}`. Nothing
//! after `+32` had a reading.
//!
//! Step one of plan 2026-09-07 L1 is the length account: a reading of the
//! tail that closes every record exactly. A first dump of the 53 records
//! (four main fixtures + A01) showed the shape; this probe reads it strictly
//! and checks it against the rest of the layer subsystem:
//!
//! ```text
//! +0   u32 oid ; +4 u32 0 ; +8 u32 0 ; +12 u32 2 ; +16 u32 JSheet ; +20 u32 1 ;
//! +24  u32 1 ; +28 u32 0 ; +32 u32 ? ; +36 u16 0
//! then 6 x { u8 0xFF ; u16 len ; len bytes }        -- bitmaps over layer numbers
//! then u16 n ; u16 2 ; n x override                 -- per-layer display overrides
//!      override = { u16 layer ; u8 kind ; u8 1 ; u16 0 ;
//!                   [kind & 2: u32 COLORREF ; f64] ; u32 }
//! then 12 x u8 0
//! then u32 count ; count x { u32 chars ; UTF-16 name ; u16 layer number }
//! ```
//!
//! Checks: (1) the account closes on every record; (2) each `(name, number)`
//! entry is exactly one `JSheetLayer` object of the same storage; (3) the
//! first bitmap, read at each named layer's number, against the name
//! criterion the importer uses today (`Hidden` / `HiddenObjects` /
//! `Invisible` off); (4) what the second bitmap differs in; (5) candidates
//! for `+32`.
//!
//! ```powershell
//! cargo run --example probe_viewfilterset_display_state
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::parsers::sheet_records::sheet_record_starts;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

const CHAIN_MAGIC: u32 = 0x6C90_F544;
const VIEW_FILTER_SET: u16 = 0x0057;
const LAYER_GROUP: u16 = 0x0088;
const MASKS: usize = 6;

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

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One record-chain stream of the file: the storage it belongs to and the
/// records it carries.
struct Chain {
    storage: String,
    data: Vec<u8>,
    records: Vec<(u16, Vec<u8>)>,
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
            records.push((type_code, payload.to_vec()));
        }
        out.push(Chain {
            storage: stream_path
                .rsplit_once('/')
                .map_or("/", |(s, _)| s)
                .to_string(),
            data,
            records,
        });
    }
    out
}

/// A per-layer display override in the set: the layer's number, the kind
/// byte, the colour and width when the kind carries them, and the closing
/// word. Field names beyond `layer` are the probe's, not the writer's.
#[derive(Debug)]
struct Override {
    layer: u16,
    kind: u8,
    one: u8,
    zero: u16,
    colour: Option<u32>,
    width: Option<f64>,
    tail: u32,
}

#[derive(Debug)]
struct ViewFilterSet {
    oid: u32,
    jsheet: u32,
    word_32: u32,
    /// The six length-prefixed byte runs, in order.
    masks: Vec<Vec<u8>>,
    overrides: Vec<Override>,
    /// `(name, layer number)` in record order.
    layers: Vec<(String, u16)>,
}

impl ViewFilterSet {
    fn bit(mask: &[u8], layer: u16) -> Option<bool> {
        let byte = mask.get(usize::from(layer / 8))?;
        Some(byte & (1 << (layer % 8)) != 0)
    }
}

/// Read a record under the layout above; `Err` names the first place the
/// bytes refuse it, so a failing record says where the account breaks.
fn read(payload: &[u8]) -> Result<ViewFilterSet, String> {
    let need = |at: usize, n: usize| -> Result<(), String> {
        if at + n > payload.len() {
            Err(format!(
                "+{at}: {n} bytes wanted, {} left",
                payload.len() - at
            ))
        } else {
            Ok(())
        }
    };
    need(0, 38)?;
    let word = |at: usize| u32_at(payload, at).unwrap();
    for (at, expected) in [(4, 0), (8, 0), (12, 2), (20, 1), (24, 1), (28, 0)] {
        if word(at) != expected {
            return Err(format!("+{at}: {} where {expected} was expected", word(at)));
        }
    }
    if u16_at(payload, 36) != Some(0) {
        return Err(format!(
            "+36: {:?} where u16 0 was expected",
            u16_at(payload, 36)
        ));
    }
    let mut at = 38usize;
    let mut masks = Vec::new();
    for i in 0..MASKS {
        need(at, 3)?;
        if payload[at] != 0xFF {
            return Err(format!("+{at}: mask {i} flag {:02X}, not FF", payload[at]));
        }
        let len = usize::from(u16_at(payload, at + 1).unwrap());
        need(at + 3, len)?;
        masks.push(payload[at + 3..at + 3 + len].to_vec());
        at += 3 + len;
    }
    need(at, 4)?;
    let n = usize::from(u16_at(payload, at).unwrap());
    let two = u16_at(payload, at + 2).unwrap();
    if two != 2 {
        return Err(format!(
            "+{}: {two} where the override table's 2 was expected",
            at + 2
        ));
    }
    at += 4;
    let mut overrides = Vec::new();
    for _ in 0..n {
        need(at, 6)?;
        let layer = u16_at(payload, at).unwrap();
        let kind = payload[at + 2];
        let one = payload[at + 3];
        let zero = u16_at(payload, at + 4).unwrap();
        at += 6;
        let (colour, width) = if kind & 2 != 0 {
            need(at, 12)?;
            let c = word(at);
            let w = f64_at(payload, at + 4).unwrap();
            at += 12;
            (Some(c), Some(w))
        } else {
            (None, None)
        };
        need(at, 4)?;
        let tail = word(at);
        at += 4;
        overrides.push(Override {
            layer,
            kind,
            one,
            zero,
            colour,
            width,
            tail,
        });
    }
    need(at, 12)?;
    let reserved = payload[at..at + 12].to_vec();
    if reserved.iter().any(|b| *b != 0) {
        return Err(format!(
            "+{at}: reserved run is not all zero: {}",
            hex(&reserved)
        ));
    }
    at += 12;
    need(at, 4)?;
    let count = word(at) as usize;
    at += 4;
    let mut layers = Vec::new();
    for _ in 0..count {
        need(at, 4)?;
        let chars = word(at) as usize;
        need(at + 4, chars * 2 + 2)?;
        let units: Vec<u16> = (0..chars)
            .map(|u| u16_at(payload, at + 4 + u * 2).unwrap())
            .collect();
        let name = String::from_utf16(&units).map_err(|e| format!("+{at}: {e}"))?;
        let number = u16_at(payload, at + 4 + chars * 2).unwrap();
        layers.push((name, number));
        at += 4 + chars * 2 + 2;
    }
    if at != payload.len() {
        return Err(format!(
            "account ends at +{at}, record is {} bytes",
            payload.len()
        ));
    }
    Ok(ViewFilterSet {
        oid: word(0),
        jsheet: word(16),
        word_32: word(32),
        masks,
        overrides,
        layers,
    })
}

/// The importer's current name criterion, for comparison.
fn hidden_by_name(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(n.as_str(), "hidden" | "hiddenobjects" | "invisible")
}

fn main() {
    let mut closed = 0usize;
    let mut total = 0usize;
    let mut mask_lengths: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    let mut entry_matches = (0usize, 0usize);
    let mut entry_ambiguous = 0usize;
    let mut criterion = BTreeMap::<&str, usize>::new();
    let mut plus32: BTreeMap<&str, usize> = BTreeMap::new();
    let mut overrides_seen = Vec::new();
    let mut via_manager = (0usize, 0usize, 0usize);
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let chains = chains_of(path);
        // The parsed document, for the manager each layer is registered with
        // (tag 183) and, from the space map, which JSheet each manager lists.
        let doc = pid_parse::PidParser::new().parse_file(fixture).ok();
        let manager_of_sheet: BTreeMap<(String, u32), BTreeSet<u32>> = doc
            .as_ref()
            .map(|doc| {
                let mut out: BTreeMap<(String, u32), BTreeSet<u32>> = BTreeMap::new();
                let sheets: BTreeSet<(String, u32)> = chains
                    .iter()
                    .flat_map(|chain| {
                        chain
                            .records
                            .iter()
                            .filter(|(code, _)| *code == 0x0114)
                            .filter_map(|(_, payload)| u32_at(payload, 0))
                            .map(move |oid| (normalize_storage(&chain.storage), oid))
                    })
                    .collect();
                for (map_path, map) in &doc.psm_space_maps {
                    let normalized = map_path.replace('\\', "/");
                    let Some(segment) = normalized
                        .rsplit(['/', '\\'])
                        .next()
                        .and_then(|leaf| u32::from_str_radix(leaf.strip_prefix("0x")?, 16).ok())
                        .map(|address| address >> 13)
                    else {
                        continue;
                    };
                    let storage = match normalized.rfind("PSMspacemap") {
                        Some(at) => normalize_storage(&normalized[..at]),
                        None => "/".to_string(),
                    };
                    for entry in &map.entries {
                        let target = (segment << 13) | u32::from(entry.index);
                        if !sheets.contains(&(storage.clone(), target)) {
                            continue;
                        }
                        for member in entry.live_members().iter().filter(|m| m.tag == 183) {
                            out.entry((storage.clone(), target))
                                .or_default()
                                .insert(member.value);
                        }
                    }
                }
                out
            })
            .unwrap_or_default();
        for chain in &chains {
            let layers = decode_sheet_layers(&chain.data);
            let storage_key = normalize_storage(&chain.storage);
            let model_layers: Vec<&pid_parse::model::SheetLayer> = doc
                .as_ref()
                .and_then(|doc| doc.sheet_layers.get(&storage_key))
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            for (type_code, payload) in &chain.records {
                if *type_code != VIEW_FILTER_SET {
                    continue;
                }
                total += 1;
                let set = match read(payload) {
                    Ok(set) => set,
                    Err(why) => {
                        println!(
                            "\nXX {} {} oid {} len {}: {why}",
                            leaf(fixture),
                            chain.storage,
                            u32_at(payload, 0).unwrap_or(0),
                            payload.len()
                        );
                        continue;
                    }
                };
                closed += 1;
                *mask_lengths
                    .entry(set.masks.iter().map(Vec::len).collect())
                    .or_default() += 1;
                let top = chain.storage.is_empty() || chain.storage == "/";
                println!(
                    "\n== {} {} oid {} len {} jsheet {} +32={} {}",
                    leaf(fixture),
                    if top { "(top)" } else { chain.storage.as_str() },
                    set.oid,
                    payload.len(),
                    set.jsheet,
                    set.word_32,
                    if set.overrides.is_empty() {
                        String::new()
                    } else {
                        format!("overrides {:?}", set.overrides)
                    }
                );
                println!(
                    "   masks: {}",
                    set.masks
                        .iter()
                        .map(|m| hex(m))
                        .collect::<Vec<_>>()
                        .join(" | ")
                );
                // Per layer: number, mask bits, and the JSheetLayer objects
                // of this storage that carry the same name and number.
                let mut a_off = 0usize;
                let mut b_differs = Vec::new();
                let mut off_named = Vec::new();
                for (name, number) in &set.layers {
                    let a = ViewFilterSet::bit(&set.masks[0], *number);
                    let b = ViewFilterSet::bit(&set.masks[1], *number);
                    let same: Vec<u32> = layers
                        .iter()
                        .filter(|l| l.name == *name && l.layer_number == u32::from(*number))
                        .map(|l| l.oid)
                        .collect();
                    // Sharper: the same name and number *and* the layer's
                    // parent is this set's sheet.
                    let on_sheet: Vec<u32> = layers
                        .iter()
                        .filter(|l| {
                            l.name == *name
                                && l.layer_number == u32::from(*number)
                                && l.parent_ref == set.jsheet
                        })
                        .map(|l| l.oid)
                        .collect();
                    entry_matches.1 += 1;
                    match on_sheet.len() {
                        1 => entry_matches.0 += 1,
                        0 => {}
                        _ => entry_ambiguous += 1,
                    }
                    // Via the manager: the manager(s) listing this set's
                    // sheet (tag 183), and the layer of the same name and
                    // number registered with that manager.
                    let managers = manager_of_sheet
                        .get(&(storage_key.clone(), set.jsheet))
                        .cloned()
                        .unwrap_or_default();
                    let via: Vec<u32> = model_layers
                        .iter()
                        .filter(|l| {
                            l.name == *name
                                && l.layer_number == u32::from(*number)
                                && l.manager_oid.is_some_and(|m| managers.contains(&m))
                        })
                        .map(|l| l.oid)
                        .collect();
                    via_manager.2 += 1;
                    match via.len() {
                        1 => via_manager.0 += 1,
                        0 => {}
                        _ => via_manager.1 += 1,
                    }
                    if via.len() != 1 {
                        println!(
                            "     ! {name} #{number}: sheet {} managers {:?} -> layers {:?}",
                            set.jsheet, managers, via
                        );
                    }
                    if a == Some(false) {
                        a_off += 1;
                        off_named.push(name.as_str());
                    }
                    if a != b {
                        b_differs.push(format!("{name}({number}) a={a:?} b={b:?}"));
                    }
                    let key = match (hidden_by_name(name), a) {
                        (true, Some(false)) => "hidden-by-name & mask off",
                        (true, Some(true)) => "hidden-by-name & mask ON",
                        (false, Some(false)) => "other name & mask off",
                        (false, Some(true)) => "other name & mask on",
                        _ => "mask too short",
                    };
                    *criterion.entry(key).or_default() += 1;
                    let show = |bit: Option<bool>| match bit {
                        Some(true) => "on ",
                        Some(false) => "OFF",
                        None => "?  ",
                    };
                    println!(
                        "   {name:<20} #{number:<3} a={} b={} objects {:?}",
                        show(a),
                        show(b),
                        same
                    );
                }
                if !b_differs.is_empty() {
                    println!("   second mask differs: {}", b_differs.join("; "));
                }
                // Candidates for +32.
                let named = set.layers.len();
                let with_edges_guess = named.saturating_sub(1);
                let default_number = set
                    .layers
                    .iter()
                    .find(|(name, _)| name == "Default")
                    .map_or(usize::MAX, |(_, n)| usize::from(*n));
                for (label, value) in [
                    ("names", named),
                    ("names-1", with_edges_guess),
                    ("mask-a off among named", a_off),
                    (
                        "max number",
                        set.layers
                            .iter()
                            .map(|(_, n)| usize::from(*n))
                            .max()
                            .unwrap_or(0),
                    ),
                    ("number of the layer named Default", default_number),
                ] {
                    if value == set.word_32 as usize {
                        *plus32.entry(label).or_default() += 1;
                    }
                }
                let _ = off_named;
                for o in &set.overrides {
                    overrides_seen.push(format!(
                        "{} {} layer {} ({}) kind {:#04x} one {} zero {} colour {:?} width {:?} tail {}",
                        leaf(fixture),
                        if top { "top" } else { chain.storage.as_str() },
                        o.layer,
                        set.layers
                            .iter()
                            .find(|(_, n)| *n == o.layer)
                            .map_or("?", |(name, _)| name.as_str()),
                        o.kind,
                        o.one,
                        o.zero,
                        o.colour.map(|c| format!("{c:08X}")),
                        o.width,
                        o.tail
                    ));
                }
            }
            // Step 4 of L1, in passing: what a JSheetLayerGroup record holds.
            for (type_code, payload) in &chain.records {
                if *type_code == LAYER_GROUP {
                    println!(
                        "\n-- {} {} JSheetLayerGroup oid {} len {}: {}",
                        leaf(fixture),
                        chain.storage,
                        u32_at(payload, 0).unwrap_or(0),
                        payload.len(),
                        hex(payload)
                    );
                }
            }
        }
    }
    println!("\n=== length account: {closed} of {total} records close exactly");
    println!(
        "=== (name, number) resolved through the sheet's manager to exactly one JSheetLayer: {} of {} ({} ambiguous)",
        via_manager.0, via_manager.2, via_manager.1
    );
    println!("=== mask lengths (six runs) -> records: {mask_lengths:?}");
    println!(
        "=== (name, number) entries matching exactly one JSheetLayer of the storage: {} of {} ({entry_ambiguous} ambiguous)",
        entry_matches.0, entry_matches.1
    );
    println!("=== first mask against the name criterion: {criterion:?}");
    println!("=== +32 equals: {plus32:?} (of {closed})");
    println!("=== overrides:");
    for line in &overrides_seen {
        println!("   {line}");
    }
    let _ = BTreeSet::<u32>::new();
}
