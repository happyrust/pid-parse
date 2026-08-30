//! Tag `184`'s edges: does a `ViewFilterSet` record hold its own membership?
//!
//! The incoming-edge round left one tag unexplained. Every other tag's
//! referrer keeps the reference in its own payload — `0x0089+12`, `0x00FA+16`,
//! `0x00CE+29` — but of the 366 members tagged `184`, only 60 are mentioned by
//! the record of the object that owns them. The remaining 306 exist **only**
//! in the space map, which would make the table primary storage rather than an
//! index, and that is a strong enough claim to deserve its own measurement.
//!
//! What is already known: the referrers are records of family `0x0057` and
//! `0x0060`, and those two type codes are exactly the two CLSIDs the RAD class
//! registry lists for `Top ViewFilterSet` (`viewfil.dex`) — the same class
//! `PSMroots` names `TopVFSet`. So a tag-184 edge reads "a view filter set
//! refers to me".
//!
//! Before concluding "the list is nowhere else", four cheaper explanations
//! have to die first:
//!
//! 1. **A different width.** A membership list has no need for full persist
//!    ids when every member shares the storage — a `u16` index would do. The
//!    forward scan only ever looked for the whole `u32`.
//! 2. **No room.** If a referrer's record is physically too small to list the
//!    edges it owns, "the list is elsewhere" stops being a hypothesis and
//!    becomes arithmetic. `TopVFSet`'s `0x0060` record is 28 bytes.
//! 3. **A count without a list.** If the record states how many members it has
//!    but not which, that names the missing structure rather than denying it.
//! 4. **Another stream.** The membership may sit in a stream that carries no
//!    record chain, so the join never saw it. Every byte of every stream gets
//!    asked.
//!
//! ```powershell
//! cargo run --example probe_psmspacemap_tag184_viewfilterset_edges
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::PidParser;

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const SEGMENT_SHIFT: u32 = 13;
const INDEX_MASK: u32 = 0x1FFF;
/// The tag under study.
const VF_TAG: u16 = 184;
/// The two `Top ViewFilterSet` type codes (RAD registry codes 87 and 96).
const VF_FAMILIES: [u16; 2] = [0x0057, 0x0060];

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
}

/// The class the RAD registry (`tools/psm_type_clsid.py`) gives each family
/// this probe meets, so the output reads as classes rather than numbers.
fn class_of(type_code: u16) -> &'static str {
    match type_code {
        0x0042 => "JSheetLayerManager",
        0x0057 => "Top ViewFilterSet (88E2AA20)",
        0x0060 => "Top ViewFilterSet (9A6C2C50)",
        0x0076 => "SheetView",
        0x0081 => "JSheetLayer",
        0x0114 => "JSheet",
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
    roots: BTreeMap<u32, String>,
    /// Every stream of the file, raw, for the "is it anywhere else" sweep.
    streams: Vec<(String, Vec<u8>)>,
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

    let roots = doc
        .psm_roots
        .as_ref()
        .map(|roots| {
            roots
                .entries
                .iter()
                .map(|root| (root.id, root.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    Some(Doc {
        name: fixture,
        scopes,
        roots,
        streams,
    })
}

/// One tag-184 edge, resolved on both ends.
struct Edge {
    doc: &'static str,
    scope: String,
    /// The object the edge points at — the space-map entry.
    target: u32,
    /// The view filter set that refers to it.
    referrer: u32,
    referrer_family: u16,
}

fn edges(docs: &[Doc]) -> Vec<Edge> {
    let mut out = Vec::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for entry in &scope.entries {
                for (value, tag) in &entry.members {
                    if *tag != VF_TAG {
                        continue;
                    }
                    let record = scope.records.get(value).and_then(|records| records.first());
                    out.push(Edge {
                        doc: doc.name,
                        scope: container.clone(),
                        target: entry.id,
                        referrer: *value,
                        referrer_family: record.map_or(0, |record| record.type_code),
                    });
                }
            }
        }
    }
    out
}

fn main() {
    let docs: Vec<Doc> = FIXTURES.iter().filter_map(|f| load(f)).collect();
    if docs.is_empty() {
        return;
    }
    let edges = edges(&docs);
    referrer_inventory(&docs, &edges);
    width_sweep(&docs, &edges);
    count_probe(&docs, &edges);
    target_report(&docs, &edges);
    elsewhere_report(&docs, &edges);
    shape_report(&docs, &edges);
    set_to_set_report(&docs, &edges);
    layer_manager_report(&docs);
    layer_report(&docs);
    membership_by_name_report(&docs, &edges);
    referrer_hexdump(&docs);
}

/// Question 11: the crux. A `0x0057` record carries a run of UTF-16 names and
/// a `JSheetLayer` record carries one. If the names in the set are the names
/// of the layers it points at, the membership is not missing at all — it is
/// written by name, and the space map is what resolves a name to an object.
fn membership_by_name_report(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 11. does the set name the layers it points at? ===");
    let mut sets = 0usize;
    let mut covered = 0usize;
    let mut edges_named = 0usize;
    let mut edges_total = 0usize;
    let mut jsheet_at_16 = (0usize, 0usize);
    let mut leftovers: BTreeMap<String, usize> = BTreeMap::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                let Some(record) = records.first() else {
                    continue;
                };
                if record.type_code != 0x0057 {
                    continue;
                }
                sets += 1;
                let names: BTreeSet<String> = utf16_strings(&record.payload)
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect();
                let mine: Vec<&Edge> = edges
                    .iter()
                    .filter(|edge| {
                        edge.doc == doc.name && edge.scope == *container && edge.referrer == *oid
                    })
                    .collect();
                let mut layer_names: BTreeSet<String> = BTreeSet::new();
                let mut all_named = true;
                for edge in &mine {
                    let Some(target) = scope
                        .records
                        .get(&edge.target)
                        .and_then(|records| records.first())
                    else {
                        continue;
                    };
                    match target.type_code {
                        0x0081 => {
                            edges_total += 1;
                            let name = utf16_strings(&target.payload)
                                .into_iter()
                                .map(|(_, text)| text)
                                .next()
                                .unwrap_or_default();
                            layer_names.insert(name.clone());
                            if names.contains(&name) {
                                edges_named += 1;
                            } else {
                                all_named = false;
                            }
                        }
                        0x0114 => {
                            jsheet_at_16.1 += 1;
                            if u32_at(&record.payload, 16) == Some(edge.target) {
                                jsheet_at_16.0 += 1;
                            }
                        }
                        _ => {}
                    }
                }
                if all_named {
                    covered += 1;
                }
                for extra in names.difference(&layer_names) {
                    *leftovers.entry(extra.clone()).or_default() += 1;
                }
            }
        }
    }
    println!(
        "  {edges_named} of {edges_total} JSheetLayer edges have their layer's name in the set's \
         own record; {covered} of {sets} sets name every layer they point at"
    );
    println!(
        "  the JSheet edge sits at the set's payload +16 in {} of {} cases",
        jsheet_at_16.0, jsheet_at_16.1
    );
    let mut ranked: Vec<(&String, &usize)> = leftovers.iter().collect();
    ranked.sort_by_key(|(name, hits)| (std::cmp::Reverse(**hits), (*name).clone()));
    let text: Vec<String> = ranked
        .iter()
        .map(|(name, hits)| format!("{name:?} x{hits}"))
        .collect();
    println!(
        "  names in a set with no edge of their own: {}",
        text.join(", ")
    );

    // Two header words look like they state the counts. Ask every referrer.
    let mut layers_at_32 = (0usize, 0usize, BTreeMap::<String, usize>::new());
    let mut sets_at_12 = (0usize, 0usize);
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                let Some(record) = records.first() else {
                    continue;
                };
                if !VF_FAMILIES.contains(&record.type_code) {
                    continue;
                }
                let mut per_family: BTreeMap<u16, usize> = BTreeMap::new();
                for edge in edges.iter().filter(|edge| {
                    edge.doc == doc.name && edge.scope == *container && edge.referrer == *oid
                }) {
                    let family = scope
                        .records
                        .get(&edge.target)
                        .and_then(|records| records.first())
                        .map_or(0, |record| record.type_code);
                    *per_family.entry(family).or_default() += 1;
                }
                if record.type_code == 0x0057 {
                    let layers = per_family.get(&0x0081).copied().unwrap_or_default();
                    layers_at_32.1 += 1;
                    if u32_at(&record.payload, 32) == Some(layers as u32) {
                        layers_at_32.0 += 1;
                    } else {
                        *layers_at_32
                            .2
                            .entry(format!(
                                "{}{container} id {oid} +32={:?} layers={layers}",
                                leaf(doc.name),
                                u32_at(&record.payload, 32)
                            ))
                            .or_default() += 1;
                    }
                } else {
                    let held = per_family.get(&0x0057).copied().unwrap_or_default();
                    sets_at_12.1 += 1;
                    if u32_at(&record.payload, 12) == Some(held as u32) {
                        sets_at_12.0 += 1;
                    }
                }
            }
        }
    }
    println!(
        "  0x0057 +32 == its layer count in {} of {} sets; 0x0060 +12 == the sets it holds in {} \
         of {}",
        layers_at_32.0, layers_at_32.1, sets_at_12.0, sets_at_12.1
    );
    for line in layers_at_32.2.keys() {
        println!("     off: {line}");
    }
}

/// Question 10: a layer is only useful to a renderer if something says which
/// graphics are on it. Dump what a `0x0081 JSheetLayer` record carries, and
/// ask whether any record in the same storage names a layer id.
fn layer_report(docs: &[Doc]) {
    println!("\n=== 10. what a JSheetLayer record holds, and who names one ===");
    let mut lengths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut listed_once = (0usize, 0usize);
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let layers: BTreeSet<u32> = scope
                .records
                .iter()
                .filter(|(_, records)| {
                    records
                        .first()
                        .is_some_and(|record| record.type_code == 0x0081)
                })
                .map(|(oid, _)| *oid)
                .collect();
            if layers.is_empty() {
                continue;
            }
            for oid in &layers {
                if let Some(record) = scope.records.get(oid).and_then(|records| records.first()) {
                    *lengths.entry(record.payload.len()).or_default() += 1;
                }
            }
            // How many managers list each layer, via tag 183?
            for oid in &layers {
                let listings = scope
                    .entries
                    .iter()
                    .filter(|entry| entry.id == *oid)
                    .flat_map(|entry| entry.members.iter())
                    .filter(|(_, tag)| *tag == 183)
                    .count();
                listed_once.0 += 1;
                if listings == 1 {
                    listed_once.1 += 1;
                }
            }
            // Does anything outside the layer subsystem mention a layer id?
            // Layer ids are small integers, and this corpus has burned us on
            // exactly that before, so the same question gets asked of a decoy
            // set: the same number of non-layer oids nearest each layer id.
            let non_layers: Vec<u32> = scope
                .records
                .keys()
                .copied()
                .filter(|oid| !layers.contains(oid))
                .collect();
            let decoys: BTreeSet<u32> = layers
                .iter()
                .filter_map(|layer| {
                    non_layers
                        .iter()
                        .min_by_key(|oid| oid.abs_diff(*layer))
                        .copied()
                })
                .collect();
            let mut mentions = 0usize;
            let mut decoy_mentions = 0usize;
            let mut scanned = 0usize;
            for records in scope.records.values() {
                for record in records {
                    if matches!(record.type_code, 0x0081 | 0x0042 | 0x0057 | 0x0060) {
                        continue;
                    }
                    scanned += 1;
                    let mentions_from = |wanted: &BTreeSet<u32>| {
                        (0..record.payload.len().saturating_sub(3)).any(|at| {
                            u32_at(&record.payload, at).is_some_and(|word| wanted.contains(&word))
                        })
                    };
                    if mentions_from(&layers) {
                        mentions += 1;
                    }
                    if mentions_from(&decoys) {
                        decoy_mentions += 1;
                    }
                }
            }
            println!(
                "  {}{container}: {} layers; of {scanned} records outside the layer subsystem, \
                 {mentions} name a layer id — against {decoy_mentions} naming one of {} decoy ids \
                 of the same magnitude",
                leaf(doc.name),
                layers.len(),
                decoys.len(),
            );
        }
    }
    println!("  JSheetLayer record lengths: {lengths:?}");
    println!(
        "  {} of {} layers are listed by exactly one manager",
        listed_once.1, listed_once.0
    );
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                let Some(record) = records.first() else {
                    continue;
                };
                if record.type_code != 0x0081 {
                    continue;
                }
                let names: Vec<String> = utf16_strings(&record.payload)
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect();
                let words: Vec<String> = (0..record.payload.len() / 4)
                    .filter_map(|word| u32_at(&record.payload, word * 4))
                    .map(|word| word.to_string())
                    .collect();
                println!(
                    "     {}{container} layer {oid:>6} len {:>3} {:?} u32 {}",
                    leaf(doc.name),
                    record.payload.len(),
                    names,
                    words.join(" ")
                );
            }
        }
    }
}

/// Every `u32 n` followed by `n` plausible UTF-16LE characters — the shape the
/// corpus uses for names everywhere else.
fn utf16_strings(payload: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 4 < payload.len() {
        let Some(count) = u32_at(payload, at).map(|count| count as usize) else {
            break;
        };
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
                        out.push((at, text));
                        at += 4 + count * 2;
                        continue;
                    }
                }
            }
        }
        at += 1;
    }
    out
}

/// Question 7: is a referrer's edge set shaped, or a bag? Break every
/// referrer's edges down by the class of what they point at.
fn shape_report(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 7. the shape of one view filter set's edge set ===");
    let mut per_referrer: BTreeMap<(&str, String, u32, u16), BTreeMap<u16, usize>> =
        BTreeMap::new();
    for edge in edges {
        let target_family = docs
            .iter()
            .find(|doc| doc.name == edge.doc)
            .and_then(|doc| doc.scopes.get(&edge.scope))
            .and_then(|scope| scope.records.get(&edge.target))
            .and_then(|records| records.first())
            .map_or(0, |record| record.type_code);
        *per_referrer
            .entry((
                edge.doc,
                edge.scope.clone(),
                edge.referrer,
                edge.referrer_family,
            ))
            .or_default()
            .entry(target_family)
            .or_default() += 1;
    }
    let mut shapes: BTreeMap<(u16, String), usize> = BTreeMap::new();
    for ((_, _, _, family), targets) in &per_referrer {
        let fixed: Vec<String> = targets
            .iter()
            .filter(|(_, hits)| **hits == 1)
            .map(|(target, _)| format!("1x {}", class_of(*target)))
            .collect();
        let variable: Vec<String> = targets
            .iter()
            .filter(|(_, hits)| **hits != 1)
            .map(|(target, _)| format!("Nx {}", class_of(*target)))
            .collect();
        let mut parts = fixed;
        parts.extend(variable);
        *shapes.entry((*family, parts.join(" + "))).or_default() += 1;
    }
    for ((family, shape), hits) in &shapes {
        println!(
            "  {hits:>2} referrers of class {} point at {shape}",
            class_of(*family)
        );
    }
    println!("  per-referrer detail (record length against layer count):");
    for ((doc_name, container, oid, family), targets) in &per_referrer {
        let len = docs
            .iter()
            .find(|doc| doc.name == *doc_name)
            .and_then(|doc| doc.scopes.get(container))
            .and_then(|scope| scope.records.get(oid))
            .and_then(|records| records.first())
            .map_or(0, |record| record.payload.len());
        let layers = targets.get(&0x0081).copied().unwrap_or_default();
        let names: Vec<String> = docs
            .iter()
            .find(|doc| doc.name == *doc_name)
            .and_then(|doc| doc.scopes.get(container))
            .and_then(|scope| scope.records.get(oid))
            .and_then(|records| records.first())
            .map(|record| {
                utf16_strings(&record.payload)
                    .into_iter()
                    .map(|(at, text)| format!("+{at} {text:?}"))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "     {}{container} id {oid:>6} 0x{family:04X} len {len:>4} {layers} layers  {}",
            leaf(doc_name),
            names.join(" ")
        );
    }
}

/// Question 8: every view filter set is pointed at by exactly one other view
/// filter set. One cycle per storage, a chain, or a star?
fn set_to_set_report(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 8. the view-filter-set to view-filter-set edges ===");
    for doc in docs {
        for (container, scope) in &doc.scopes {
            let mut links: Vec<(u32, u32)> = Vec::new();
            for edge in edges
                .iter()
                .filter(|edge| edge.doc == doc.name && edge.scope == *container)
            {
                let target_family = scope
                    .records
                    .get(&edge.target)
                    .and_then(|records| records.first())
                    .map_or(0, |record| record.type_code);
                if VF_FAMILIES.contains(&target_family) {
                    links.push((edge.referrer, edge.target));
                }
            }
            if links.is_empty() {
                continue;
            }
            links.sort_unstable();
            let text: Vec<String> = links
                .iter()
                .map(|(from, to)| format!("{from}->{to}"))
                .collect();
            let referrers: BTreeSet<u32> = links.iter().map(|(from, _)| *from).collect();
            let targets: BTreeSet<u32> = links.iter().map(|(_, to)| *to).collect();
            println!(
                "  {}{container}: {} edges, {} distinct sources, {} distinct targets — {}",
                leaf(doc.name),
                links.len(),
                referrers.len(),
                targets.len(),
                text.join(" ")
            );
        }
    }
}

/// Question 9: tag `183`'s referrers are `0x0042 JSheetLayerManager`, and the
/// reverse scan found the entry id in a stride-4 table at `+12`. If those
/// targets are layers, the manager is the structure that does list them, and
/// the view filter set's silence is a division of labour rather than a hole.
fn layer_manager_report(docs: &[Doc]) {
    println!("\n=== 9. tag 183: what the JSheetLayerManager's table lists ===");
    let mut pairs: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    let mut listed: BTreeMap<(&str, String, u32), usize> = BTreeMap::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for entry in &scope.entries {
                for (value, tag) in &entry.members {
                    if *tag != 183 {
                        continue;
                    }
                    let referrer_family = scope
                        .records
                        .get(value)
                        .and_then(|records| records.first())
                        .map_or(0, |record| record.type_code);
                    let target_family = scope
                        .records
                        .get(&entry.id)
                        .and_then(|records| records.first())
                        .map_or(0, |record| record.type_code);
                    *pairs.entry((referrer_family, target_family)).or_default() += 1;
                    *listed
                        .entry((doc.name, container.clone(), *value))
                        .or_default() += 1;
                }
            }
        }
    }
    for ((referrer, target), hits) in &pairs {
        println!(
            "  {hits:>4} edges: {} (0x{referrer:04X}) -> {} (0x{target:04X})",
            class_of(*referrer),
            class_of(*target)
        );
    }
    println!("  managers and how many objects each lists:");
    for ((doc_name, container, oid), hits) in &listed {
        let len = docs
            .iter()
            .find(|doc| doc.name == *doc_name)
            .and_then(|doc| doc.scopes.get(container))
            .and_then(|scope| scope.records.get(oid))
            .and_then(|records| records.first())
            .map_or(0, |record| record.payload.len());
        println!(
            "     {}{container} id {oid:>6} len {len:>4} lists {hits}",
            leaf(doc_name)
        );
    }
}

/// Question 1: who are the referrers, how big are their records, and how many
/// edges does each one own? A record that owns more edges than it has words is
/// not hiding the list — it does not have room for it.
fn referrer_inventory(docs: &[Doc], edges: &[Edge]) {
    println!("=== 1. every tag-184 referrer: record size against edges owned ===");
    let mut owned: BTreeMap<(&str, String, u32), usize> = BTreeMap::new();
    for edge in edges {
        *owned
            .entry((edge.doc, edge.scope.clone(), edge.referrer))
            .or_default() += 1;
    }
    println!("  {} edges from {} referrers", edges.len(), owned.len());
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                for record in records {
                    if !VF_FAMILIES.contains(&record.type_code) {
                        continue;
                    }
                    let owns = owned
                        .get(&(doc.name, container.clone(), *oid))
                        .copied()
                        .unwrap_or_default();
                    let words = record.payload.len() / 4;
                    let named = doc.roots.get(oid).cloned().unwrap_or_default();
                    let has_entry = scope.entries.iter().any(|entry| entry.id == *oid);
                    println!(
                        "  {}{container} id {oid:>6} 0x{:04X} len {:>4} ({words} words) owns \
                         {owns:>3} edges{}{}",
                        leaf(doc.name),
                        record.type_code,
                        record.payload.len(),
                        if named.is_empty() {
                            String::new()
                        } else {
                            format!("  [PSMroots: {named}]")
                        },
                        if has_entry { "  [has own entry]" } else { "" },
                    );
                    if owns > words {
                        println!(
                            "       -> cannot hold them: {owns} edges need {} bytes, the whole \
                             record is {}",
                            owns * 4,
                            record.payload.len()
                        );
                    }
                }
            }
        }
    }
}

/// Question 2: the forward scan only asked for the full `u32`. Ask for the
/// narrower encodings a same-storage membership list would plausibly use.
fn width_sweep(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 2. is the target id in the referrer's payload at any width? ===");
    let mut as_u32 = 0usize;
    let mut as_u16_index = 0usize;
    let mut as_u32_low13 = 0usize;
    let mut nowhere = 0usize;
    let mut offsets_u32: BTreeMap<(u16, usize), usize> = BTreeMap::new();
    let mut offsets_u16: BTreeMap<(u16, usize), usize> = BTreeMap::new();
    for edge in edges {
        let Some(scope) = docs
            .iter()
            .find(|doc| doc.name == edge.doc)
            .and_then(|doc| doc.scopes.get(&edge.scope))
        else {
            continue;
        };
        let Some(records) = scope.records.get(&edge.referrer) else {
            continue;
        };
        let index = edge.target & INDEX_MASK;
        let mut hit_u32 = false;
        let mut hit_u16 = false;
        let mut hit_low13 = false;
        for record in records {
            let payload = &record.payload;
            for at in 0..payload.len().saturating_sub(3) {
                let Some(word) = u32_at(payload, at) else {
                    continue;
                };
                if word == edge.target {
                    hit_u32 = true;
                    *offsets_u32.entry((record.type_code, at)).or_default() += 1;
                } else if word != 0 && (word & INDEX_MASK) == index {
                    hit_low13 = true;
                }
            }
            for at in 0..payload.len().saturating_sub(1) {
                if u16_at(payload, at).map(u32::from) == Some(index) && index != 0 {
                    hit_u16 = true;
                    *offsets_u16.entry((record.type_code, at)).or_default() += 1;
                }
            }
        }
        if hit_u32 {
            as_u32 += 1;
        }
        if hit_u16 {
            as_u16_index += 1;
        }
        if hit_low13 {
            as_u32_low13 += 1;
        }
        if !hit_u32 && !hit_u16 && !hit_low13 {
            nowhere += 1;
        }
    }
    println!(
        "  of {} edges: {as_u32} carry the target as a full u32, {as_u16_index} as a u16 index, \
         {as_u32_low13} as a u32 whose low 13 bits match, {nowhere} at no width at all",
        edges.len()
    );
    print_offsets("  full u32 sits at", &offsets_u32);
    print_offsets("  u16 index sits at", &offsets_u16);
}

fn print_offsets(label: &str, offsets: &BTreeMap<(u16, usize), usize>) {
    let mut ranked: Vec<(&(u16, usize), &usize)> = offsets.iter().collect();
    ranked.sort_by_key(|((family, at), hits)| (std::cmp::Reverse(**hits), *family, *at));
    let text: Vec<String> = ranked
        .iter()
        .take(10)
        .map(|((family, at), hits)| format!("0x{family:04X}+{at} x{hits}"))
        .collect();
    println!(
        "{label} {}",
        if text.is_empty() {
            "--".to_string()
        } else {
            text.join(", ")
        }
    );
}

/// Question 3: a set that does not list its members may still state how many
/// it has. Ask every word of the referrer's record whether it equals the
/// number of edges the corpus says it owns.
fn count_probe(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 3. does the referrer's record state its member count? ===");
    let mut owned: BTreeMap<(&str, String, u32), usize> = BTreeMap::new();
    for edge in edges {
        *owned
            .entry((edge.doc, edge.scope.clone(), edge.referrer))
            .or_default() += 1;
    }
    let mut offsets: BTreeMap<(u16, usize), usize> = BTreeMap::new();
    let mut referrers = 0usize;
    let mut stated = 0usize;
    for ((doc_name, container, oid), owns) in &owned {
        let Some(scope) = docs
            .iter()
            .find(|doc| doc.name == *doc_name)
            .and_then(|doc| doc.scopes.get(container))
        else {
            continue;
        };
        let Some(records) = scope.records.get(oid) else {
            continue;
        };
        referrers += 1;
        let mut found = false;
        for record in records {
            let payload = &record.payload;
            for at in 0..payload.len().saturating_sub(3) {
                if u32_at(payload, at) == Some(*owns as u32) {
                    found = true;
                    *offsets.entry((record.type_code, at)).or_default() += 1;
                }
            }
        }
        if found {
            stated += 1;
        }
    }
    println!("  {stated} of {referrers} referrers hold their own edge count somewhere");
    print_offsets("  the count sits at", &offsets);
}

/// Question 4: what does a view filter set point at? If the answer is "one
/// record family, all of it", the edge is a membership relation and the set is
/// a partition of the document; if it is scattered, it is something else.
fn target_report(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 4. what the tag-184 edges point at ===");
    let mut families: BTreeMap<u16, usize> = BTreeMap::new();
    let mut targeted: BTreeSet<(&str, String, u32)> = BTreeSet::new();
    for edge in edges {
        targeted.insert((edge.doc, edge.scope.clone(), edge.target));
        let family = docs
            .iter()
            .find(|doc| doc.name == edge.doc)
            .and_then(|doc| doc.scopes.get(&edge.scope))
            .and_then(|scope| scope.records.get(&edge.target))
            .and_then(|records| records.first())
            .map_or(0, |record| record.type_code);
        *families.entry(family).or_default() += 1;
    }
    let mut ranked: Vec<(&u16, &usize)> = families.iter().collect();
    ranked.sort_by_key(|(family, hits)| (std::cmp::Reverse(**hits), **family));
    let text: Vec<String> = ranked
        .iter()
        .map(|(family, hits)| format!("0x{family:04X} x{hits}"))
        .collect();
    println!("  target families: {}", text.join(", "));

    println!("  coverage — of every object of that family in the scope, how many are targeted:");
    let mut coverage: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                let Some(record) = records.first() else {
                    continue;
                };
                let slot = coverage.entry(record.type_code).or_default();
                slot.0 += 1;
                if targeted.contains(&(doc.name, container.clone(), *oid)) {
                    slot.1 += 1;
                }
            }
        }
    }
    let mut ranked: Vec<(&u16, &(usize, usize))> = coverage
        .iter()
        .filter(|(_, (_, hit))| *hit > 0)
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(family, (_, hit))| (std::cmp::Reverse(*hit), **family));
    for (family, (total, hit)) in ranked {
        println!("     0x{family:04X}: {hit} of {total} objects carry a tag-184 edge");
    }
}

/// Question 5: if the membership list is not in the referrer's record, is it
/// anywhere in the file at all? Ask every byte of every stream for the target
/// id, and separate the space map itself from everything else.
fn elsewhere_report(docs: &[Doc], edges: &[Edge]) {
    println!("\n=== 5. for edges the referrer does not mention, is the id anywhere else? ===");
    for doc in docs {
        let mut wanted: BTreeSet<u32> = BTreeSet::new();
        for edge in edges.iter().filter(|edge| edge.doc == doc.name) {
            let mentioned = doc
                .scopes
                .get(&edge.scope)
                .and_then(|scope| scope.records.get(&edge.referrer))
                .is_some_and(|records| {
                    records.iter().any(|record| {
                        (0..record.payload.len().saturating_sub(3))
                            .any(|at| u32_at(&record.payload, at) == Some(edge.target))
                    })
                });
            if !mentioned {
                wanted.insert(edge.target);
            }
        }
        if wanted.is_empty() {
            continue;
        }
        let mut homes: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for (stream_path, data) in &doc.streams {
            let bucket = if stream_path.contains("PSMspacemap") {
                "PSMspacemap".to_string()
            } else {
                stream_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(stream_path)
                    .to_string()
            };
            for at in 0..data.len().saturating_sub(3) {
                let Some(word) = u32_at(data, at) else {
                    continue;
                };
                if wanted.contains(&word) {
                    homes.entry(bucket.clone()).or_default().insert(word);
                }
            }
        }
        println!(
            "  {}: {} table-only targets; each id turns up in:",
            leaf(doc.name),
            wanted.len()
        );
        let mut ranked: Vec<(&String, &BTreeSet<u32>)> = homes.iter().collect();
        ranked.sort_by_key(|(name, ids)| (std::cmp::Reverse(ids.len()), (*name).clone()));
        for (bucket, ids) in ranked {
            println!("     {bucket}: {} of the {} ids", ids.len(), wanted.len());
        }
    }
}

/// Question 6: the records themselves, so the reader can see what a view
/// filter set does carry. `0x0060` is 28 bytes in all four fixtures.
fn referrer_hexdump(docs: &[Doc]) {
    println!("\n=== 6. the view filter set records ===");
    for doc in docs {
        for (container, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                for record in records {
                    if !VF_FAMILIES.contains(&record.type_code) {
                        continue;
                    }
                    println!(
                        "  {}{container} id {oid} 0x{:04X} len {}",
                        leaf(doc.name),
                        record.type_code,
                        record.payload.len()
                    );
                    let shown = record.payload.len().min(96);
                    for chunk in record.payload[..shown].chunks(16) {
                        let hex: Vec<String> =
                            chunk.iter().map(|byte| format!("{byte:02X}")).collect();
                        println!("     {}", hex.join(" "));
                    }
                    let words: Vec<String> = (0..shown / 4)
                        .filter_map(|word| u32_at(&record.payload, word * 4))
                        .map(|word| word.to_string())
                        .collect();
                    println!("     as u32: {}", words.join(" "));
                }
            }
        }
    }
}
