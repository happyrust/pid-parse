//! Does the corpus agree that tag `181` is the source record's `parent_ref`?
//!
//! The IDA round found a writer. `sub_56495440` walks a cluster's record
//! chain (the §3 envelope: `u16 type_word`, `u32 bytes_to_follow`, payload)
//! and registers every record into the space map, and the entry constructor
//! `sub_56479290` fills the first two member slots from the record's **own
//! payload**: `payload+4` under tag `181`, `payload+8` under tag `182`. On
//! that path the tag names *which field of the source record the reference
//! was read from* — nothing about the target. That contradicts the label the
//! last commit put on the tag ("the class of the object the member points
//! at"), so before any documentation moves, the corpus gets to vote.
//!
//! Two things make this a real test rather than a formality:
//!
//! * The rebuild path writes tags `181` and `182` **in pairs**, but the
//!   corpus holds 84 members tagged `181` against ~519 tagged `182` — so
//!   these files were not (all) written by that path, and the mechanism
//!   found in the disassembly is so far only proven for a code path these
//!   files may never have taken.
//! * The seven `JSite` entries are shaped `[(2, tag 182), (X, tag 225|261)]`
//!   — no `181` in slot 0 — which the two-slot writer cannot have produced.
//!
//! The join the corpus affords: a space-map entry's persist id is
//! `(segment << 13) | index`, and every record chain in the **same storage**
//! carries records whose payload starts with that same id (`payload+0`,
//! the `oid` every decoded family states). So for each storage, index the
//! records by oid and put each entry next to its own record:
//!
//! 1. **Coverage.** Which entries have a source record at all, and in which
//!    chain stream? (If the map is the out-edge table of the record chains,
//!    most entries should resolve.)
//! 2. **The direct test.** For members tagged `181`: does the value equal
//!    the record's `payload+4`? For `182`: `payload+8`?
//! 3. **The converse, which is where the 84-vs-519 puzzle should resolve.**
//!    When a record's `payload+4` is nonzero, does its entry hold that value
//!    under `181`? When it is zero, is the `181` member simply absent?
//! 4. **The other eleven tags.** If a tag names a source field, each tag's
//!    member values should sit at one fixed payload offset of its source
//!    records. Scan every offset and histogram the hits per tag.
//! 5. **Exhibits.** Every `PSMroots`-named object of the top-level scope,
//!    with its entry members and its record's first words side by side —
//!    the seven `JSite`s among them.
//!
//! ```powershell
//! cargo run --example probe_psmspacemap_tag181_is_the_parent_ref
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

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One record of a chain stream, payload kept whole so any offset can be
/// asked about later.
struct Record {
    /// Leaf name of the stream the record sits in (`Sheet6`, `PSMcluster0`…).
    stream: String,
    type_code: u16,
    payload: Vec<u8>,
}

impl Record {
    fn word(&self, at: usize) -> Option<u32> {
        u32_at(&self.payload, at)
    }
}

/// One space-map entry, reduced to what the join needs.
struct MapEntry {
    id: u32,
    /// `(value, tag, slot)` for the live members only.
    members: Vec<(u32, u16, usize)>,
}

/// Everything one storage scope holds: its records by oid, its entries.
#[derive(Default)]
struct ScopeData {
    records: BTreeMap<u32, Vec<Record>>,
    entries: Vec<MapEntry>,
}

struct Doc {
    name: &'static str,
    scopes: BTreeMap<String, ScopeData>,
    roots: Vec<(u32, String)>,
    /// Chain streams whose walk did not consume the stream exactly.
    failed_walks: Vec<String>,
}

fn leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

/// The storage that owns a stream: everything up to and including the last
/// separator. `/Sheet6` and `/PSMspacemap/…` both belong to `/`;
/// `/JSite329/StyleCluster` belongs to `/JSite329/`.
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

/// One walked chain stream: its path, and every record as
/// `(type_code, payload)`.
type ChainStream = (String, Vec<(u16, Vec<u8>)>);

/// Every record-chain stream of the file, walked. A stream qualifies by its
/// magic alone; a qualifying stream that does not walk to zero residue is
/// reported rather than half-used.
fn chain_streams(path: &Path) -> (Vec<ChainStream>, Vec<String>) {
    let mut chains = Vec::new();
    let mut failed = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return (chains, failed);
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return (chains, failed);
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
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        if u32_at(&data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let starts = sheet_record_starts(&data);
        if starts.is_empty() {
            failed.push(stream_path);
            continue;
        }
        let records = starts
            .iter()
            .filter_map(|&at| {
                let type_code = u16_at(&data, at)? & 0x3FFF;
                let len = u32_at(&data, at + 2)? as usize;
                Some((type_code, data.get(at + 6..at + 6 + len)?.to_vec()))
            })
            .collect();
        chains.push((stream_path, records));
    }
    (chains, failed)
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

    let mut scopes: BTreeMap<String, ScopeData> = BTreeMap::new();
    let (chains, failed_walks) = chain_streams(path);
    for (stream_path, records) in chains {
        let scope = scopes.entry(container_of(&stream_path)).or_default();
        let stream_leaf = stream_path
            .rsplit('/')
            .next()
            .unwrap_or(&stream_path)
            .to_string();
        for (type_code, payload) in records {
            let Some(oid) = u32_at(&payload, 0) else {
                continue;
            };
            scope.records.entry(oid).or_default().push(Record {
                stream: stream_leaf.clone(),
                type_code,
                payload,
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
                    .enumerate()
                    .map(|(slot, member)| (member.value, member.tag, slot))
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
        failed_walks,
    })
}

fn main() {
    let docs: Vec<Doc> = FIXTURES.iter().filter_map(|f| load(f)).collect();
    if docs.is_empty() {
        return;
    }
    for doc in &docs {
        for failed in &doc.failed_walks {
            println!(
                "note: {} {failed} carries the chain magic but does not walk cleanly",
                leaf(doc.name)
            );
        }
    }
    coverage_report(&docs);
    direct_report(&docs);
    converse_report(&docs);
    offset_report(&docs);
    roots_exhibit(&docs);
    reciprocity_report(&docs);
    value_family_report(&docs);
    reverse_scan_report(&docs);
}

/// Question 6: members come in mirrored pairs — entry 23 holds `(22, 181)`
/// while entry 22 holds `(23, 182)`. If that is the rule rather than the
/// anecdote, the tags pair up as the two ends of one relationship. Count, for
/// every members' tag, under which tag (if any) the target entry points back.
fn reciprocity_report(docs: &[Doc]) {
    println!("\n=== 6. does the target point back, and under which tag? ===");
    let mut back_tag: BTreeMap<u16, BTreeMap<Option<u16>, usize>> = BTreeMap::new();
    let mut self_members: BTreeMap<u16, usize> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            let mut members_of: BTreeMap<u32, &[(u32, u16, usize)]> = BTreeMap::new();
            for entry in &scope.entries {
                members_of.insert(entry.id, &entry.members);
            }
            for entry in &scope.entries {
                for (value, tag, _) in &entry.members {
                    if *tag == 0 {
                        continue;
                    }
                    if *value == entry.id {
                        *self_members.entry(*tag).or_default() += 1;
                    }
                    let back = members_of.get(value).and_then(|members| {
                        members
                            .iter()
                            .find(|(back_value, _, _)| *back_value == entry.id)
                            .map(|(_, back_tag, _)| *back_tag)
                    });
                    *back_tag.entry(*tag).or_default().entry(back).or_default() += 1;
                }
            }
        }
    }
    for (tag, backs) in &back_tag {
        let text: Vec<String> = backs
            .iter()
            .map(|(back, hits)| match back {
                Some(back) => format!("tag {back} x{hits}"),
                None => format!("no back-edge x{hits}"),
            })
            .collect();
        println!(
            "  tag {tag:>3}: target holds my id under {}",
            text.join(", ")
        );
    }
    println!("  members whose value is the entry's own id: {self_members:?}");
}

/// Question 7: what kind of record does a member's *value* name? If the tag
/// tracks the record family of the object at `value`, the tag is (a coarse
/// version of) that object's class; if one tag covers several families, it is
/// not.
fn value_family_report(docs: &[Doc]) {
    println!("\n=== 7. the record family of the object each tag points at ===");
    let mut families: BTreeMap<u16, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut streams: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    let mut no_record: BTreeMap<u16, usize> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            for entry in &scope.entries {
                for (value, tag, _) in &entry.members {
                    if *tag == 0 {
                        continue;
                    }
                    let Some(records) = scope.records.get(value) else {
                        *no_record.entry(*tag).or_default() += 1;
                        continue;
                    };
                    let mut kinds: BTreeSet<u16> = BTreeSet::new();
                    let mut homes: BTreeSet<&str> = BTreeSet::new();
                    for record in records {
                        kinds.insert(record.type_code);
                        homes.insert(record.stream.as_str());
                    }
                    for kind in kinds {
                        *families.entry(*tag).or_default().entry(kind).or_default() += 1;
                    }
                    for home in homes {
                        *streams
                            .entry(*tag)
                            .or_default()
                            .entry(home.to_string())
                            .or_default() += 1;
                    }
                }
            }
        }
    }
    for (tag, kinds) in &families {
        let mut ranked: Vec<(&u16, &usize)> = kinds.iter().collect();
        ranked.sort_by_key(|(kind, hits)| (std::cmp::Reverse(**hits), **kind));
        let text: Vec<String> = ranked
            .iter()
            .map(|(kind, hits)| format!("0x{kind:04X} x{hits}"))
            .collect();
        println!(
            "  tag {tag:>3}: values are {} (no record x{})",
            text.join(", "),
            no_record.get(tag).copied().unwrap_or_default()
        );
        println!(
            "           value's record lives in {:?}",
            streams.get(tag).cloned().unwrap_or_default()
        );
    }
}

/// Question 8: the forward scan (question 4) found the member value almost
/// nowhere in the *entry's* record. Scan the other direction: does the
/// entry's id appear in the payload of the record at `value`? A hit means
/// the record at `value` is the one holding the reference, and the map
/// entry is its back-pointer.
fn reverse_scan_report(docs: &[Doc]) {
    println!("\n=== 8. does the value's record mention the entry (back-pointer test)? ===");
    struct Scan {
        members: usize,
        with_record: usize,
        mentioned: usize,
        nowhere: usize,
        offsets: BTreeMap<usize, usize>,
        /// `(value record family, offset)` pairs, to see per-family fields.
        family_offsets: BTreeMap<(u16, usize), usize>,
    }
    let mut per_tag: BTreeMap<u16, Scan> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            for entry in &scope.entries {
                for (value, tag, _) in &entry.members {
                    if *tag == 0 {
                        continue;
                    }
                    let scan = per_tag.entry(*tag).or_insert_with(|| Scan {
                        members: 0,
                        with_record: 0,
                        mentioned: 0,
                        nowhere: 0,
                        offsets: BTreeMap::new(),
                        family_offsets: BTreeMap::new(),
                    });
                    scan.members += 1;
                    let Some(records) = scope.records.get(value) else {
                        continue;
                    };
                    scan.with_record += 1;
                    let mut hit = false;
                    for record in records {
                        for at in 0..record.payload.len().saturating_sub(3) {
                            if record.word(at) == Some(entry.id) {
                                hit = true;
                                *scan.offsets.entry(at).or_default() += 1;
                                *scan
                                    .family_offsets
                                    .entry((record.type_code, at))
                                    .or_default() += 1;
                            }
                        }
                    }
                    if hit {
                        scan.mentioned += 1;
                    } else {
                        scan.nowhere += 1;
                    }
                }
            }
        }
    }
    for (tag, scan) in &per_tag {
        let mut ranked: Vec<(&(u16, usize), &usize)> = scan.family_offsets.iter().collect();
        ranked.sort_by_key(|(_, hits)| std::cmp::Reverse(**hits));
        let top: Vec<String> = ranked
            .iter()
            .take(8)
            .map(|((kind, at), hits)| format!("0x{kind:04X}+{at} x{hits}"))
            .collect();
        println!(
            "  tag {tag:>3}: {} of {} members' values have a record; the record mentions the \
             entry in {} cases, never in {}",
            scan.with_record, scan.members, scan.mentioned, scan.nowhere
        );
        println!(
            "           where: {}",
            if top.is_empty() {
                "--".to_string()
            } else {
                top.join(", ")
            }
        );
    }
}

/// Question 1: which entries have a source record, and where do the records
/// live?
fn coverage_report(docs: &[Doc]) {
    println!("=== 1. does a space-map entry have a record with its own id? ===");
    for doc in docs {
        for (container, scope) in &doc.scopes {
            if scope.entries.is_empty() {
                continue;
            }
            let resolved = scope
                .entries
                .iter()
                .filter(|entry| scope.records.contains_key(&entry.id))
                .count();
            let mut per_stream: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
            let entry_ids: BTreeSet<u32> = scope.entries.iter().map(|entry| entry.id).collect();
            for (oid, records) in &scope.records {
                for record in records {
                    let slot = per_stream.entry(record.stream.as_str()).or_default();
                    slot.0 += 1;
                    if entry_ids.contains(oid) {
                        slot.1 += 1;
                    }
                }
            }
            println!(
                "  {}{container}: {resolved} of {} entries have a record",
                leaf(doc.name),
                scope.entries.len()
            );
            for (stream, (records, with_entry)) in &per_stream {
                println!("     {stream}: {records} records, {with_entry} of them carry an entry");
            }
        }
    }
}

/// Question 2: member value against the record word the writer read it from.
fn direct_report(docs: &[Doc]) {
    println!("\n=== 2. tag 181 vs payload+4, tag 182 vs payload+8 ===");
    for (tag, offset) in [(181u16, 4usize), (182, 8)] {
        let mut members = 0usize;
        let mut no_record = 0usize;
        let mut matched = 0usize;
        let mut mismatches: Vec<String> = Vec::new();
        for doc in docs {
            for (container, scope) in &doc.scopes {
                for entry in &scope.entries {
                    for (value, member_tag, _) in &entry.members {
                        if *member_tag != tag {
                            continue;
                        }
                        members += 1;
                        let Some(records) = scope.records.get(&entry.id) else {
                            no_record += 1;
                            continue;
                        };
                        if records
                            .iter()
                            .any(|record| record.word(offset) == Some(*value))
                        {
                            matched += 1;
                        } else if mismatches.len() < 10 {
                            let seen: Vec<String> = records
                                .iter()
                                .map(|record| {
                                    format!(
                                        "{} 0x{:04X} +{offset}={:?}",
                                        record.stream,
                                        record.type_code,
                                        record.word(offset)
                                    )
                                })
                                .collect();
                            mismatches.push(format!(
                                "{}{container} id {} member ({value}, {tag}) vs {}",
                                leaf(doc.name),
                                entry.id,
                                seen.join("; ")
                            ));
                        }
                    }
                }
            }
        }
        let mismatched = members - no_record - matched;
        println!(
            "  tag {tag} vs payload+{offset}: {matched} match, {mismatched} mismatch, \
             {no_record} of {members} members have no source record"
        );
        for line in &mismatches {
            println!("     mismatch: {line}");
        }
    }
}

/// Question 3: the converse — every record word, asked whether the entry
/// carries it. This is where 84-vs-519 either resolves or does not.
fn converse_report(docs: &[Doc]) {
    println!("\n=== 3. every record's payload word, asked back ===");
    for (tag, offset) in [(181u16, 4usize), (182, 8)] {
        let mut entries_with_record = 0usize;
        let mut word_zero = 0usize;
        let mut too_short = 0usize;
        let mut held_under_tag = 0usize;
        let mut held_under_other: BTreeMap<u16, usize> = BTreeMap::new();
        let mut absent = 0usize;
        let mut absent_examples: Vec<String> = Vec::new();
        for doc in docs {
            for (container, scope) in &doc.scopes {
                for entry in &scope.entries {
                    let Some(records) = scope.records.get(&entry.id) else {
                        continue;
                    };
                    entries_with_record += 1;
                    let words: BTreeSet<u32> = records
                        .iter()
                        .filter_map(|record| record.word(offset))
                        .collect();
                    if words.is_empty() {
                        too_short += 1;
                        continue;
                    }
                    if words.iter().all(|word| *word == 0) {
                        word_zero += 1;
                        continue;
                    }
                    let mut verdict_absent = true;
                    for word in words.iter().filter(|word| **word != 0) {
                        if entry
                            .members
                            .iter()
                            .any(|(value, member_tag, _)| value == word && *member_tag == tag)
                        {
                            held_under_tag += 1;
                            verdict_absent = false;
                        } else if let Some((_, other, _)) =
                            entry.members.iter().find(|(value, _, _)| value == word)
                        {
                            *held_under_other.entry(*other).or_default() += 1;
                            verdict_absent = false;
                        }
                    }
                    if verdict_absent {
                        absent += 1;
                        if absent_examples.len() < 6 {
                            let record = &records[0];
                            absent_examples.push(format!(
                                "{}{container} id {} ({} 0x{:04X}) +{offset}={:?} not among members {:?}",
                                leaf(doc.name),
                                entry.id,
                                record.stream,
                                record.type_code,
                                words,
                                entry
                                    .members
                                    .iter()
                                    .map(|(value, member_tag, _)| (*value, *member_tag))
                                    .collect::<Vec<_>>()
                            ));
                        }
                    }
                }
            }
        }
        println!(
            "  payload+{offset} → tag {tag}: of {entries_with_record} entries with a record, \
             {word_zero} have the word zero, {too_short} too short a payload, \
             {held_under_tag} hold it under tag {tag}, {:?} under another tag, {absent} not at all",
            held_under_other
        );
        for line in &absent_examples {
            println!("     absent: {line}");
        }
    }
}

/// Question 4: if a tag names a source field, its values sit at one payload
/// offset. Scan every offset of every source record, per tag.
fn offset_report(docs: &[Doc]) {
    println!("\n=== 4. which payload offset does each tag's value sit at? ===");
    struct TagScan {
        members: usize,
        with_record: usize,
        nowhere: usize,
        offsets: BTreeMap<usize, usize>,
        source_types: BTreeMap<u16, usize>,
        slots: BTreeMap<usize, usize>,
    }
    let mut per_tag: BTreeMap<u16, TagScan> = BTreeMap::new();
    for doc in docs {
        for scope in doc.scopes.values() {
            for entry in &scope.entries {
                for (value, tag, slot) in &entry.members {
                    if *tag == 0 {
                        continue;
                    }
                    let scan = per_tag.entry(*tag).or_insert_with(|| TagScan {
                        members: 0,
                        with_record: 0,
                        nowhere: 0,
                        offsets: BTreeMap::new(),
                        source_types: BTreeMap::new(),
                        slots: BTreeMap::new(),
                    });
                    scan.members += 1;
                    *scan.slots.entry(*slot).or_default() += 1;
                    let Some(records) = scope.records.get(&entry.id) else {
                        continue;
                    };
                    scan.with_record += 1;
                    let mut hit_offsets: BTreeSet<usize> = BTreeSet::new();
                    let mut types: BTreeSet<u16> = BTreeSet::new();
                    for record in records {
                        types.insert(record.type_code);
                        for at in 0..record.payload.len().saturating_sub(3) {
                            if record.word(at) == Some(*value) {
                                hit_offsets.insert(at);
                            }
                        }
                    }
                    for type_code in types {
                        *scan.source_types.entry(type_code).or_default() += 1;
                    }
                    if hit_offsets.is_empty() {
                        scan.nowhere += 1;
                    }
                    for at in hit_offsets {
                        *scan.offsets.entry(at).or_default() += 1;
                    }
                }
            }
        }
    }
    for (tag, scan) in &per_tag {
        let mut ranked: Vec<(&usize, &usize)> = scan.offsets.iter().collect();
        ranked.sort_by_key(|(at, hits)| (std::cmp::Reverse(**hits), **at));
        let top: Vec<String> = ranked
            .iter()
            .take(6)
            .map(|(at, hits)| format!("+{at} x{hits}"))
            .collect();
        println!(
            "  tag {tag:>3}: {} members ({} with a record, {} whose value is nowhere in the \
             payload), member slot {:?}",
            scan.members, scan.with_record, scan.nowhere, scan.slots
        );
        println!(
            "           source records {:?}; value found at {}",
            scan.source_types,
            if top.is_empty() {
                "--".to_string()
            } else {
                top.join(", ")
            }
        );
    }
}

/// Question 5: the named objects, entry and record side by side.
fn roots_exhibit(docs: &[Doc]) {
    println!("\n=== 5. every PSMroots-named object: entry members vs record words ===");
    for doc in docs {
        let Some(scope) = doc.scopes.get("/") else {
            continue;
        };
        if doc.roots.is_empty() {
            continue;
        }
        println!("  {}", leaf(doc.name));
        for (id, name) in &doc.roots {
            let members: Option<&MapEntry> = scope.entries.iter().find(|entry| entry.id == *id);
            let member_text = members
                .map(|entry| {
                    let listed: Vec<String> = entry
                        .members
                        .iter()
                        .map(|(value, tag, _)| format!("({value}, t{tag})"))
                        .collect();
                    listed.join(" ")
                })
                .unwrap_or_else(|| "no entry".to_string());
            match scope.records.get(id) {
                Some(records) => {
                    for record in records {
                        println!(
                            "     id {id:>6} {name:<28} [{member_text}] <- {} 0x{:04X} len {} \
                             +4={:?} +8={:?} +12={:?}",
                            record.stream,
                            record.type_code,
                            record.payload.len(),
                            record.word(4),
                            record.word(8),
                            record.word(12),
                        );
                    }
                }
                None => println!("     id {id:>6} {name:<28} [{member_text}] <- no record"),
            }
        }
    }
}
