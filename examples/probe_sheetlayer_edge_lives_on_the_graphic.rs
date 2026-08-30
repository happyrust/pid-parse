//! Which byte does a graphic use to say what sheet layer it is on?
//!
//! `shlyhp.dll` settled the direction. `AddObjectToSheetLayer` hands the
//! layer's outer `IUnknown` to the graphic (slot 13 of the interface
//! `204D4DD1-B174-11CE-B914-08003601C6EB`) and bumps a counter inside the
//! layer; the layer itself keeps no member list. So the edge — if it survives a
//! save at all — is written by the graphic, and the layer's `+12` is nothing
//! but a tally of it.
//!
//! The previous round asked the corpus "does any record mention a layer id"
//! and got an unusable answer: layer ids are small integers, and a decoy set of
//! non-layer oids of the same magnitude scored as well or better. Scanning a
//! whole payload for a bare integer cannot be made to pay.
//!
//! So this round asks a question with a shape. Every layer states how many
//! objects sit on it. A storage's layers therefore state a **multiset of
//! counts** — `D06`'s top level says `10 4 0 1 0 0 2 10 0 0 0 0`. If graphics
//! carry the reference at a fixed offset, then grouping the graphics of one
//! family by the word at that offset must **reproduce that multiset exactly**,
//! zeros included. That is a partition test, not a membership test: a stray
//! small integer cannot pass it, and no decoy set is needed because the null
//! hypothesis is priced into the shape itself.
//!
//! Four questions, in order:
//!
//! 1. **Does any space-map tag land on a layer besides 183 and 184?** The map
//!    indexes incoming references. If graphics referred to layers the way
//!    symbols refer to their site, every layer would carry a fan of edges.
//! 2. **What do the layers declare?** Name, number, object count, and — since
//!    same-named layers are one object per view filter set — whether the copies
//!    agree with each other.
//! 3. **Does the declared total have a match in the record census?** If the
//!    layers of a storage account for 27 objects and the storage holds 27
//!    graphics, the assignment is total; if it holds 3000, it is not.
//! 4. **The partition test itself**, over every family × offset × width, in two
//!    key spaces: the layer's persist id and the layer's own number.
//!
//! ```powershell
//! cargo run --example probe_sheetlayer_edge_lives_on_the_graphic
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::PidParser;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    // The fixture whose page border Phase 40 found in the refused pile.
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// The families this crate decodes, plus the two the census turns up that it
/// records as absent. `aux_hi` is read for each of them at the end.
const DECODED: [u16; 9] = [
    0x0013, 0x0018, 0x0020, 0x004D, 0x0059, 0x005E, 0x0061, 0x0084, 0x00CE,
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const SEGMENT_SHIFT: u32 = 13;
/// `0x0081 JSheetLayer`, the family under study.
const LAYER: u16 = 0x0081;
/// The layer subsystem's own families, reported apart from the graphics.
const LAYER_SUBSYSTEM: [u16; 5] = [0x0042, 0x0057, 0x0060, 0x0081, 0x0088];
/// How deep into a payload the partition test looks for the reference.
const MAX_OFFSET: usize = 256;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One record of a chain stream, payload kept whole.
struct Record {
    type_code: u16,
    payload: Vec<u8>,
    /// The stream it was read from, so a family can be placed as well as
    /// counted.
    stream: String,
}

/// The class the RAD registry (`tools/psm_type_clsid.py`) gives each family
/// this probe prints, so the output reads as classes rather than numbers.
fn class_of(type_code: u16) -> &'static str {
    match type_code {
        0x0013 => "igBoundary2d",
        0x0018 => "igLine2d",
        0x0030 => "JStyleOverride",
        0x0042 => "JSheetLayerManager",
        0x004D => "igTextBox",
        0x0057 => "Top ViewFilterSet",
        0x005E => "igPoint2d",
        0x0060 => "Top ViewFilterSet'",
        0x0076 => "SheetView",
        0x0081 => "JSheetLayer",
        0x0084 => "igLineString2d",
        0x0088 => "JSheetLayerGroup",
        0x0089 => "DA row",
        0x00CE => "igSymbol2d",
        0x00FA => "DependencyObject",
        0x0114 => "JSheet",
        0x3FE6 => "GLine2d",
        _ => "?",
    }
}

/// One space-map entry, reduced to what the join needs.
struct MapEntry {
    id: u32,
    members: Vec<(u32, u16)>,
}

/// Everything one storage scope holds.
#[derive(Default)]
struct ScopeData {
    records: BTreeMap<u32, Vec<Record>>,
    entries: Vec<MapEntry>,
}

struct Doc {
    name: &'static str,
    scopes: BTreeMap<String, ScopeData>,
}

fn leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

/// The storage that owns a stream: everything up to and including the last
/// separator, with `PSMspacemap/…` folded onto its parent.
fn container_of(path: &str) -> String {
    match path.rfind("PSMspacemap") {
        Some(at) => path[..at].to_string(),
        None => match path.rfind('/') {
            Some(at) => path[..=at].to_string(),
            None => "/".to_string(),
        },
    }
}

/// The segment a member stream's name states: `swprintf_s(L"0x%.8x", n << 13)`.
fn segment_of(path: &str) -> Option<u32> {
    let stream_leaf = path.rsplit(['/', '\\']).next()?;
    u32::from_str_radix(stream_leaf.strip_prefix("0x")?, 16)
        .ok()
        .map(|address| address >> SEGMENT_SHIFT)
}

/// Every stream of the file, raw.
fn all_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
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
        if stream.read_to_end(&mut data).is_ok() {
            out.push((stream_path, data));
        }
    }
    out
}

fn load(fixture: &'static str) -> Option<Doc> {
    let path = Path::new(fixture);
    if !path.exists() {
        println!("skip: {fixture} is absent");
        return None;
    }
    let doc = match PidParser::new().parse_file(fixture) {
        Ok(doc) => doc,
        Err(e) => {
            println!("skip: {fixture} did not parse: {e}");
            return None;
        }
    };

    let streams = all_streams(path);
    let mut scopes: BTreeMap<String, ScopeData> = BTreeMap::new();
    for (stream_path, data) in &streams {
        if u32_at(data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let starts = sheet_record_starts(data);
        if starts.is_empty() {
            continue;
        }
        let scope = scopes.entry(container_of(stream_path)).or_default();
        for at in starts {
            let Some(type_code) = u16_at(data, at).map(|word| word & 0x3FFF) else {
                continue;
            };
            let Some(len) = u32_at(data, at + 2).map(|len| len as usize) else {
                continue;
            };
            let Some(payload) = data.get(at + 6..at + 6 + len) else {
                continue;
            };
            let Some(oid) = u32_at(payload, 0) else {
                continue;
            };
            scope.records.entry(oid).or_default().push(Record {
                type_code,
                payload: payload.to_vec(),
                stream: stream_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(stream_path)
                    .to_string(),
            });
        }
    }

    for (map_path, map) in &doc.psm_space_maps {
        let normalized = map_path.replace('\\', "/");
        let Some(segment) = segment_of(&normalized) else {
            continue;
        };
        let scope = scopes.entry(container_of(&normalized)).or_default();
        for entry in &map.entries {
            scope.entries.push(MapEntry {
                id: (segment << SEGMENT_SHIFT) | u32::from(entry.index),
                members: entry
                    .live_members()
                    .iter()
                    .map(|member| (member.value, member.tag))
                    .collect(),
            });
        }
    }

    Some(Doc {
        name: fixture,
        scopes,
    })
}

/// What a `0x0081 JSheetLayer` record states about itself. Field names come
/// from `SheetLayer::IJPersistImp::Save` in `shlyhp.dll`, not from guesswork.
struct Layer {
    oid: u32,
    name: String,
    /// `+12`: how many objects the layer says are on it — the counter
    /// `IJLayerImp` bumps on add and drops on remove.
    objects: u32,
    /// `+16`: the layer's own number; `-1` when the constructor never assigned
    /// one.
    number: u32,
}

/// Every `u32 n` followed by `n` plausible UTF-16LE characters — the shape the
/// corpus uses for names everywhere else.
fn first_utf16_name(payload: &[u8]) -> String {
    let mut at = 0usize;
    while at + 4 < payload.len() {
        if let Some(count) = u32_at(payload, at).map(|count| count as usize) {
            if (1..=128).contains(&count) && at + 4 + count * 2 <= payload.len() {
                let units: Vec<u16> = (0..count)
                    .filter_map(|unit| u16_at(payload, at + 4 + unit * 2))
                    .collect();
                if units.len() == count {
                    if let Ok(text) = String::from_utf16(&units) {
                        if text
                            .chars()
                            .all(|c| c.is_alphanumeric() || " _-.()/&#".contains(c))
                        {
                            return text;
                        }
                    }
                }
            }
        }
        at += 1;
    }
    String::new()
}

fn layers_of(scope: &ScopeData) -> Vec<Layer> {
    let mut out = Vec::new();
    for (oid, records) in &scope.records {
        let Some(record) = records.first() else {
            continue;
        };
        if record.type_code != LAYER {
            continue;
        }
        out.push(Layer {
            oid: *oid,
            name: first_utf16_name(&record.payload),
            objects: u32_at(&record.payload, 12).unwrap_or_default(),
            number: u32_at(&record.payload, 16).unwrap_or_default(),
        });
    }
    out
}

fn signed(word: u32) -> String {
    if word == u32::MAX {
        "-1".to_string()
    } else {
        word.to_string()
    }
}

fn main() {
    let docs: Vec<Doc> = FIXTURES.iter().filter_map(|f| load(f)).collect();
    if docs.is_empty() {
        return;
    }
    tag_matrix(&docs);
    layer_inventory(&docs);
    census(&docs);
    partition_test(&docs);
    residue(&docs);
    anatomy(&docs);
    where_they_live(&docs);
    the_gate(&docs);
    the_layerless(&docs);
}

/// Question 9: a handful of records of otherwise layer-bearing families put
/// nothing at `+8`. If `+8` really is the sheet layer, those have to be
/// graphics that are not on a sheet — and the corpus has exactly one such
/// population, the glyph lines the style library owns (guide §5). Print where
/// they sit rather than asserting it.
fn the_layerless(docs: &[Doc]) {
    println!("\n=== 9. records of a layer-bearing family that name no layer ===");
    let mut homes: BTreeMap<(u16, String), usize> = BTreeMap::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers: BTreeSet<u32> = layers_of(scope).iter().map(|layer| layer.oid).collect();
            let carriers: BTreeSet<u16> = scope
                .records
                .values()
                .flatten()
                .filter(|record| {
                    u32_at(&record.payload, 8).is_some_and(|value| layers.contains(&value))
                })
                .map(|record| record.type_code)
                .collect();
            for records in scope.records.values() {
                for record in records {
                    if !carriers.contains(&record.type_code) && !DECODED.contains(&record.type_code)
                    {
                        continue;
                    }
                    let Some(value) = u32_at(&record.payload, 8) else {
                        continue;
                    };
                    if layers.contains(&value) {
                        continue;
                    }
                    let note = if value == 0 { "no layer" } else { "unresolved" };
                    *homes
                        .entry((
                            record.type_code,
                            format!(
                                "{}{container}{} ({note} {value})",
                                leaf(doc.name),
                                record.stream
                            ),
                        ))
                        .or_default() += 1;
                }
            }
        }
    }
    for ((family, place), hits) in &homes {
        println!("  0x{family:04X} {} — {place} x{hits}", class_of(*family));
    }
}

/// Question 8: this crate reads payload `+8` as `aux_hi` and `decode_igboundaries`
/// still refuses any record whose `aux_hi` is not `12`. Its own comment says
/// the rule stays only because nobody has measured the family without it. If
/// `+8` is the layer, then `== 12` is not a framing rule at all — it admits one
/// layer and refuses the rest. Print what each decoded family actually carries.
fn the_gate(docs: &[Doc]) {
    println!("\n=== 8. what the decoded families carry at +8 ===");
    for family in DECODED {
        let mut spread: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;
        let mut would_pass = 0usize;
        for doc in docs {
            for scope in doc.scopes.values() {
                let named: BTreeMap<u32, String> = layers_of(scope)
                    .into_iter()
                    .map(|layer| (layer.oid, layer.name))
                    .collect();
                for records in scope.records.values() {
                    for record in records {
                        if record.type_code != family {
                            continue;
                        }
                        let Some(value) = u32_at(&record.payload, 8) else {
                            continue;
                        };
                        total += 1;
                        if value == 12 {
                            would_pass += 1;
                        }
                        let label = match named.get(&value) {
                            Some(name) => format!("{value} {name:?}"),
                            None if value == 0 => "0 (no layer)".to_string(),
                            None => format!("{value} (no such layer here)"),
                        };
                        *spread.entry(label).or_default() += 1;
                    }
                }
            }
        }
        if total == 0 {
            continue;
        }
        let mut ranked: Vec<(&String, &usize)> = spread.iter().collect();
        ranked.sort_by_key(|(label, hits)| (std::cmp::Reverse(**hits), (*label).clone()));
        let text: Vec<String> = ranked
            .iter()
            .take(8)
            .map(|(label, hits)| format!("{label} x{hits}"))
            .collect();
        println!(
            "  0x{family:04X} {:>16}: {total} records, {would_pass} would pass an `aux_hi == 12` \
             gate — {}",
            class_of(family),
            text.join(", ")
        );
    }
}

/// Question 7: `+8` sorts the corpus into families that are on a layer and
/// families that are not, which makes it a census of what counts as a graphic.
/// Two of the families it turns up are ones the decoder suite records as
/// absent from this corpus, so each one gets placed: which fixture, which
/// storage, which stream.
fn where_they_live(docs: &[Doc]) {
    println!("\n=== 7. the families that carry a layer, and where they sit ===");
    let mut homes: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers: BTreeSet<u32> = layers_of(scope).iter().map(|layer| layer.oid).collect();
            for records in scope.records.values() {
                for record in records {
                    let carries = u32_at(&record.payload, 8)
                        .is_some_and(|value| value != 0 && layers.contains(&value));
                    if !carries {
                        continue;
                    }
                    *homes
                        .entry(record.type_code)
                        .or_default()
                        .entry(format!("{}{container}{}", leaf(doc.name), record.stream))
                        .or_default() += 1;
                }
            }
        }
    }
    for (family, places) in &homes {
        let total: usize = places.values().sum();
        println!(
            "  0x{family:04X} {} — {total} records on a layer",
            class_of(*family)
        );
        if total <= 30 {
            for (place, hits) in places {
                println!("     {place} x{hits}");
            }
        } else {
            println!("     across {} streams", places.len());
        }
    }
}

/// Question 5: where the winning offset and the layers disagree, by how much
/// and in which direction. A count that runs ahead of the records is a stale
/// tally; one that runs behind means objects are being reached that the probe
/// is not counting.
fn residue(docs: &[Doc]) {
    println!("\n=== 5. every layer where +8 and the declared count disagree ===");
    let mut declared_total = 0u32;
    let mut observed_total = 0u32;
    let mut off = 0usize;
    let mut splits = 0usize;
    let mut seen_layers = 0usize;
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers = layers_of(scope);
            if layers.is_empty() {
                continue;
            }
            let (values, split) = values_by_object(scope, 8, 32);
            splits += split;
            let mut observed: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for (oid, value) in &values {
                observed.entry(*value).or_default().push(*oid);
            }
            seen_layers += layers.len();
            for layer in &layers {
                let mine = observed.get(&layer.oid).map(Vec::as_slice).unwrap_or(&[]);
                let seen = mine.len() as u32;
                declared_total += layer.objects;
                observed_total += seen;
                if seen != layer.objects {
                    off += 1;
                    let mut families: BTreeMap<u16, usize> = BTreeMap::new();
                    for oid in mine {
                        if let Some(record) = scope.records.get(oid).and_then(|rs| rs.first()) {
                            *families.entry(record.type_code).or_default() += 1;
                        }
                    }
                    let text: Vec<String> = families
                        .iter()
                        .map(|(family, hits)| {
                            format!("{}(0x{family:04X}) x{hits}", class_of(*family))
                        })
                        .collect();
                    println!(
                        "  {}{container} layer {} {:?}: declares {}, {seen} objects point at it \
                         — {}",
                        leaf(doc.name),
                        layer.oid,
                        layer.name,
                        layer.objects,
                        text.join(", ")
                    );
                }
            }
        }
    }
    println!(
        "  {off} of {seen_layers} layers disagree; {observed_total} objects point at a layer \
         against {declared_total} declared, and {splits} objects have records that disagree at +8"
    );
}

/// Question 6: `+8` is a header word every family carries, so its meaning has
/// to hold for every family — not only the ones a layer counts. Ask what each
/// family puts there: nothing, a layer, or some other object of the storage.
fn anatomy(docs: &[Doc]) {
    println!("\n=== 6. what every family puts in +8 ===");
    let mut zero: BTreeMap<u16, usize> = BTreeMap::new();
    let mut to_layer: BTreeMap<u16, usize> = BTreeMap::new();
    let mut to_other: BTreeMap<u16, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut dangling: BTreeMap<u16, usize> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            let layers: BTreeSet<u32> = layers_of(scope).iter().map(|layer| layer.oid).collect();
            for records in scope.records.values() {
                for record in records {
                    let Some(value) = u32_at(&record.payload, 8) else {
                        continue;
                    };
                    if value == 0 {
                        *zero.entry(record.type_code).or_default() += 1;
                    } else if layers.contains(&value) {
                        *to_layer.entry(record.type_code).or_default() += 1;
                    } else if let Some(target) = scope
                        .records
                        .get(&value)
                        .and_then(|records| records.first())
                    {
                        *to_other
                            .entry(record.type_code)
                            .or_default()
                            .entry(target.type_code)
                            .or_default() += 1;
                    } else {
                        *dangling.entry(record.type_code).or_default() += 1;
                    }
                }
            }
        }
    }
    let families: BTreeSet<u16> = zero
        .keys()
        .chain(to_layer.keys())
        .chain(to_other.keys())
        .chain(dangling.keys())
        .copied()
        .collect();
    for family in families {
        let others: Vec<String> = to_other
            .get(&family)
            .map(|targets| {
                targets
                    .iter()
                    .map(|(target, hits)| format!("{}(0x{target:04X}) x{hits}", class_of(*target)))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "  0x{family:04X} {:>18}: zero {:>4}, a layer {:>4}, dangling {:>3}{}",
            class_of(family),
            zero.get(&family).copied().unwrap_or_default(),
            to_layer.get(&family).copied().unwrap_or_default(),
            dangling.get(&family).copied().unwrap_or_default(),
            if others.is_empty() {
                String::new()
            } else {
                format!(", other {}", others.join(" "))
            }
        );
    }
}

/// Question 1: the space map is an index of incoming references. Every tag,
/// with the family that refers and the family referred to. If a graphic held a
/// persisted reference to its layer the way a symbol holds one to its site,
/// `0x0081` would show up in the right-hand column under some tag.
fn tag_matrix(docs: &[Doc]) {
    println!("=== 1. every space-map tag: who refers -> what is referred to ===");
    let mut matrix: BTreeMap<u16, BTreeMap<(u16, u16), usize>> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            for entry in &scope.entries {
                for (value, tag) in &entry.members {
                    let referrer = scope
                        .records
                        .get(value)
                        .and_then(|records| records.first())
                        .map_or(0, |record| record.type_code);
                    let target = scope
                        .records
                        .get(&entry.id)
                        .and_then(|records| records.first())
                        .map_or(0, |record| record.type_code);
                    *matrix
                        .entry(*tag)
                        .or_default()
                        .entry((referrer, target))
                        .or_default() += 1;
                }
            }
        }
    }
    for (tag, pairs) in &matrix {
        let total: usize = pairs.values().sum();
        let mut ranked: Vec<(&(u16, u16), &usize)> = pairs.iter().collect();
        ranked.sort_by_key(|((referrer, target), hits)| {
            (std::cmp::Reverse(**hits), *referrer, *target)
        });
        let text: Vec<String> = ranked
            .iter()
            .take(6)
            .map(|((referrer, target), hits)| {
                format!(
                    "{}(0x{referrer:04X})->{}(0x{target:04X}) x{hits}",
                    class_of(*referrer),
                    class_of(*target)
                )
            })
            .collect();
        println!("  tag {tag:>3} ({total:>4}): {}", text.join(", "));
    }
    let into_layers: usize = matrix
        .values()
        .flat_map(|pairs| pairs.iter())
        .filter(|((_, target), _)| *target == LAYER)
        .map(|(_, hits)| *hits)
        .sum();
    let tags_into_layers: BTreeSet<u16> = matrix
        .iter()
        .filter(|(_, pairs)| pairs.keys().any(|(_, target)| *target == LAYER))
        .map(|(tag, _)| *tag)
        .collect();
    println!("  edges landing on a JSheetLayer: {into_layers}, under tags {tags_into_layers:?}");
}

/// Question 2: what the layers declare. Same-named layers are one object per
/// view filter set, so the copies are printed together — if the object count
/// were per-view state the copies would disagree, and if it is a property of
/// the layer they will not.
fn layer_inventory(docs: &[Doc]) {
    println!("\n=== 2. what every JSheetLayer declares ===");
    let mut agree = (0usize, 0usize);
    let mut assigned = (0usize, 0usize);
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers = layers_of(scope);
            if layers.is_empty() {
                continue;
            }
            let total: u32 = layers.iter().map(|layer| layer.objects).sum();
            println!(
                "  {}{container}: {} layers, {total} objects declared",
                leaf(doc.name),
                layers.len()
            );
            let mut by_name: BTreeMap<&str, Vec<&Layer>> = BTreeMap::new();
            for layer in &layers {
                by_name.entry(layer.name.as_str()).or_default().push(layer);
                assigned.1 += 1;
                if layer.number != u32::MAX {
                    assigned.0 += 1;
                }
            }
            for (name, copies) in &by_name {
                let text: Vec<String> = copies
                    .iter()
                    .map(|layer| {
                        format!(
                            "id {} n={} objs={}",
                            layer.oid,
                            signed(layer.number),
                            layer.objects
                        )
                    })
                    .collect();
                let counts: BTreeSet<u32> = copies.iter().map(|layer| layer.objects).collect();
                if copies.len() > 1 {
                    agree.1 += 1;
                    if counts.len() == 1 {
                        agree.0 += 1;
                    }
                }
                println!("     {name:>20}: {}", text.join(" | "));
            }
        }
    }
    println!(
        "  {} of {} layers carry a number other than -1; {} of {} same-name groups agree on the \
         object count",
        assigned.0, assigned.1, agree.0, agree.1
    );
}

/// Question 3: the layers of a storage account for N objects between them. Is
/// there anything in that storage of size N? A record census next to the
/// declared total says whether the assignment could be total, partial, or
/// aimed at something that is not a sheet record at all.
fn census(docs: &[Doc]) {
    println!("\n=== 3. declared object total against the record census ===");
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers = layers_of(scope);
            if layers.is_empty() {
                continue;
            }
            let declared: u32 = layers.iter().map(|layer| layer.objects).sum();
            let mut families: BTreeMap<u16, usize> = BTreeMap::new();
            for records in scope.records.values() {
                for record in records {
                    *families.entry(record.type_code).or_default() += 1;
                }
            }
            let graphics: usize = families
                .iter()
                .filter(|(family, _)| !LAYER_SUBSYSTEM.contains(family))
                .map(|(_, hits)| *hits)
                .sum();
            let mut ranked: Vec<(&u16, &usize)> = families
                .iter()
                .filter(|(family, _)| !LAYER_SUBSYSTEM.contains(family))
                .collect();
            ranked.sort_by_key(|(family, hits)| (std::cmp::Reverse(**hits), **family));
            let text: Vec<String> = ranked
                .iter()
                .take(8)
                .map(|(family, hits)| format!("0x{family:04X} x{hits}"))
                .collect();
            println!(
                "  {}{container}: declared {declared}, {graphics} records outside the layer \
                 subsystem — {}",
                leaf(doc.name),
                text.join(", ")
            );
            let exact: Vec<String> = families
                .iter()
                .filter(|(_, hits)| **hits as u32 == declared && declared > 0)
                .map(|(family, _)| format!("0x{family:04X} {}", class_of(*family)))
                .collect();
            if !exact.is_empty() {
                println!("     families of exactly that size: {}", exact.join(", "));
            }
        }
    }
}

/// How one candidate offset scores against the layers' declared counts.
///
/// The two halves are kept apart on purpose. Most layers hold nothing, so a
/// candidate that finds nothing at all still gets every zero right; counting
/// the two together is how a null result dresses up as a near miss.
struct Score {
    /// Layers that declare objects and got the right number.
    live_right: usize,
    /// Layers that declare objects at all.
    live_total: usize,
    /// Layers that declare nothing and were given nothing.
    empty_right: usize,
    /// Layers that declare nothing.
    empty_total: usize,
    /// Records whose value there is not any layer's key.
    outside: u32,
}

impl Score {
    fn exact(&self) -> bool {
        self.live_total > 0
            && self.live_right == self.live_total
            && self.empty_right == self.empty_total
    }

    fn rank(&self) -> (usize, usize) {
        (self.live_right, self.empty_right)
    }
}

/// What each object of the scope says at one offset.
///
/// An object is not a record: a chain can carry more than one record for the
/// same oid, and a layer counts objects. Every record of an oid is read and
/// the disagreements are counted separately, so folding them cannot quietly
/// invent or lose a member.
fn values_by_object(scope: &ScopeData, at: usize, width: u8) -> (BTreeMap<u32, u32>, usize) {
    let mut out: BTreeMap<u32, u32> = BTreeMap::new();
    let mut split = 0usize;
    for (oid, records) in &scope.records {
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        for record in records {
            if LAYER_SUBSYSTEM.contains(&record.type_code) {
                continue;
            }
            let value = if width == 16 {
                u16_at(&record.payload, at).map(u32::from)
            } else {
                u32_at(&record.payload, at)
            };
            if let Some(value) = value {
                seen.insert(value);
            }
        }
        match seen.len() {
            0 => {}
            1 => {
                out.insert(*oid, seen.into_iter().next().unwrap_or_default());
            }
            _ => split += 1,
        }
    }
    (out, split)
}

/// Tally, for one offset and width, how the values found there distribute over
/// the key space, then score that distribution against the declared counts.
fn score_at(scope: &ScopeData, wanted: &BTreeMap<u32, u32>, at: usize, width: u8) -> Score {
    let mut per_key: BTreeMap<u32, u32> = BTreeMap::new();
    let mut outside = 0u32;
    let (values, _) = values_by_object(scope, at, width);
    for value in values.values() {
        if wanted.contains_key(value) {
            *per_key.entry(*value).or_default() += 1;
        } else {
            outside += 1;
        }
    }
    let mut score = Score {
        live_right: 0,
        live_total: 0,
        empty_right: 0,
        empty_total: 0,
        outside,
    };
    for (key, declared) in wanted {
        let seen = per_key.get(key).copied().unwrap_or_default();
        if *declared > 0 {
            score.live_total += 1;
            if seen == *declared {
                score.live_right += 1;
            }
        } else {
            score.empty_total += 1;
            if seen == 0 {
                score.empty_right += 1;
            }
        }
    }
    score
}

/// Question 4: the partition test. The declared counts cover every object on a
/// layer, whatever family it belongs to, so the grouping has to be taken over
/// the whole storage at one offset — not one family at a time. A candidate
/// passes only when every layer that declares objects gets exactly that many
/// **and** every layer that declares none gets none.
fn partition_test(docs: &[Doc]) {
    println!("\n=== 4. does any fixed offset reproduce the declared counts? ===");
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers = layers_of(scope);
            let declared_total: u32 = layers.iter().map(|layer| layer.objects).sum();
            if layers.is_empty() || declared_total == 0 {
                continue;
            }
            let by_oid: BTreeMap<u32, u32> = layers
                .iter()
                .map(|layer| (layer.oid, layer.objects))
                .collect();
            let mut by_number: BTreeMap<u32, u32> = BTreeMap::new();
            for layer in &layers {
                *by_number.entry(layer.number).or_default() += layer.objects;
            }
            let deepest = scope
                .records
                .values()
                .flatten()
                .filter(|record| !LAYER_SUBSYSTEM.contains(&record.type_code))
                .map(|record| record.payload.len())
                .max()
                .unwrap_or_default()
                .min(MAX_OFFSET);
            println!(
                "  {}{container}: {} layers ({} of them hold objects), {declared_total} objects",
                leaf(doc.name),
                layers.len(),
                layers.iter().filter(|layer| layer.objects > 0).count(),
            );
            for (space, wanted) in [("id", &by_oid), ("number", &by_number)] {
                let mut exact: Vec<String> = Vec::new();
                let mut best: Option<((usize, usize), String)> = None;
                for at in 0..deepest {
                    for width in [16u8, 32u8] {
                        let score = score_at(scope, wanted, at, width);
                        let line = format!(
                            "+{at} u{width}: {}/{} layers with objects, {}/{} empty ones, {} \
                             objects outside",
                            score.live_right,
                            score.live_total,
                            score.empty_right,
                            score.empty_total,
                            score.outside
                        );
                        if score.exact() {
                            exact.push(line.clone());
                        }
                        if best.as_ref().is_none_or(|(rank, _)| score.rank() > *rank) {
                            best = Some((score.rank(), line));
                        }
                    }
                }
                if exact.is_empty() {
                    let text = best.map_or_else(|| "nothing scanned".into(), |(_, line)| line);
                    println!("     by {space}: none — closest {text}");
                } else {
                    for line in &exact {
                        println!("     by {space}: MATCH {line}");
                    }
                }
            }
        }
    }
}
