//! Where do `A01_Data.xml`'s four `GraphicOID`s live in `A01.pid`, and which
//! hop of the S1 join breaks? (OpenCADStudio plan
//! `2026-10-08-pid-integration-unblock-and-next-steps.md`, P-D28 / task D1.)
//!
//! `A01_Data.xml` publishes four representations: 24601 (the vessel
//! `V 010121A`), 24606 (its nozzle), 24613 and 24615 (both owned by the
//! pipeline `PH- 0102102-…`). `PidSemanticIndex::resolve` hands none of them
//! to a drawn record, so no A01 entity in OpenCADStudio carries `class=` or
//! `label=`. The 08-07 note (§4) only found each number once in
//! `PSMspacemap` and once in `Unclustered Dynamic Attributes`. This probe
//! asks the file where each number is, every way the corpus has a reader for:
//!
//! 1. a chain record whose own oid (`payload+0`) it is, in any storage;
//! 2. the space-map entry with that id -- who references the object, under
//!    which tag;
//! 3. the space-map entries that list it as a member -- what the object
//!    references;
//! 4. every other chain record whose payload holds it as a 4-byte word;
//! 5. every stream outside the record chains that holds it.
//!
//! `DWG-0202GP06-01`, whose 39 published oids all resolve, runs alongside as
//! the control, one line per object.
//!
//! Then the question that matters to a consumer: which drawn entities
//! (`build_normalized_geometry`, the projection OpenCADStudio imports)
//! resolve to which published object (6) -- today, and with the naive third
//! hop: a published oid that is no Sheet record, and that exactly one
//! top-level space-map entry lists under tag `190`, stands for that entry's
//! record (the XML is rewritten to the entry's id and the crate's own
//! two-hop rule does the rest). The exhibit (7) names what the hop lands on,
//! and the UID scan (8) asks where each published UID and item tag sits in
//! the file.
//!
//! The answer (9): every published representation's UID sits as ASCII in
//! exactly one top-level `FreeFormAttrSet` (`0x0089`), and that set points at
//! its record under tag 190. On `DWG-0202GP06-01` this lands on the
//! `GraphicOID`'s own record 39 times out of 39. On A01 the four published
//! numbers are attribute sets that hold *other* representations' UIDs (the
//! drawing was saved after it was published), the naive hop swaps the vessel
//! and its nozzle, and the UID join gives the vessel its Horizontal Drum, the
//! nozzle its Flanged Nozzle and the pipeline its run.
//!
//! ```powershell
//! cargo run --example probe_a01_representation_uid_is_the_join
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::rad_class_name;
use pid_parse::{
    build_normalized_geometry, NormalizedPidGeometry, PidDocument, PidGraphicKind, PidParser,
    PidSemanticHit, PidSemanticIndex, PidSemanticObject,
};

const FIXTURES: [(&str, bool); 2] = [
    ("test-file/export-test/publish-data/A01/A01.pid", true),
    (
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        false,
    ),
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const SEGMENT_SHIFT: u32 = 13;
/// The type-word bit `PSMSerializeIn` seeks past a record on.
const NATIVE_SKIP: u16 = 0x8000;
/// The space-map tag under which an `Unclustered Dynamic Attributes` set
/// points at the record it describes (see the A01 output).
const ATTRIBUTE_SET_TAG: u16 = 190;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Every offset at which `value` sits as a little-endian 4-byte word.
fn word_offsets(data: &[u8], value: u32) -> Vec<usize> {
    let needle = value.to_le_bytes();
    data.windows(4)
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(at, _)| at)
        .collect()
}

struct Record {
    stream: String,
    at: usize,
    type_word: u16,
    payload: Vec<u8>,
}

impl Record {
    fn type_code(&self) -> u16 {
        self.type_word & 0x3FFF
    }

    fn oid(&self) -> Option<u32> {
        u32_at(&self.payload, 0)
    }

    fn family(&self) -> String {
        let code = self.type_code();
        let skip = if self.type_word & NATIVE_SKIP != 0 {
            " [native-skip]"
        } else {
            ""
        };
        format!(
            "0x{code:04X} {}{skip}",
            rad_class_name(code).unwrap_or("(unnamed)")
        )
    }

    fn describe(&self) -> String {
        format!(
            "{} @0x{:X} {} len {}",
            self.stream,
            self.at,
            self.family(),
            self.payload.len()
        )
    }
}

/// The storage that owns a stream. `/Sheet6` and `/PSMspacemap/…` belong to
/// `/`; `/JSite204/Sheet6` belongs to `/JSite204/`.
fn container_of(path: &str) -> String {
    match path.rfind("PSMspacemap") {
        Some(at) => path[..at].to_string(),
        None => match path.rfind('/') {
            Some(at) => path[..=at].to_string(),
            None => "/".to_string(),
        },
    }
}

/// The segment a space-map member stream's name states.
fn segment_of(path: &str) -> Option<u32> {
    let leaf = path.rsplit(['/', '\\']).next()?;
    u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
        .ok()
        .map(|address| address >> SEGMENT_SHIFT)
}

fn split_id(id: u32) -> String {
    format!("{id} (seg {} idx {})", id >> SEGMENT_SHIFT, id & 0x1FFF)
}

struct Streams {
    chains: Vec<Record>,
    /// Streams outside the record chains, and chain-magic streams that do
    /// not walk, kept whole for the raw scan.
    others: Vec<(String, Vec<u8>)>,
    sizes: Vec<(String, usize, bool)>,
}

fn read_streams(path: &Path) -> Streams {
    let mut out = Streams {
        chains: Vec::new(),
        others: Vec::new(),
        sizes: Vec::new(),
    };
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
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let starts = if u32_at(&data, 0) == Some(CHAIN_MAGIC) {
            sheet_record_starts(&data)
        } else {
            Vec::new()
        };
        out.sizes
            .push((stream_path.clone(), data.len(), !starts.is_empty()));
        if starts.is_empty() {
            out.others.push((stream_path, data));
            continue;
        }
        for at in starts {
            let (Some(type_word), Some(len)) = (u16_at(&data, at), u32_at(&data, at + 2)) else {
                continue;
            };
            let Some(payload) = data.get(at + 6..at + 6 + len as usize) else {
                continue;
            };
            out.chains.push(Record {
                stream: stream_path.clone(),
                at,
                type_word,
                payload: payload.to_vec(),
            });
        }
    }
    out
}

struct MapEntry {
    scope: String,
    id: u32,
    members: Vec<(u32, u16)>,
}

fn map_entries(doc: &PidDocument) -> Vec<MapEntry> {
    let mut entries = Vec::new();
    for (map_path, map) in &doc.psm_space_maps {
        let normalized = map_path.replace('\\', "/");
        let Some(segment) = segment_of(&normalized) else {
            continue;
        };
        let scope = container_of(&normalized);
        for entry in &map.entries {
            entries.push(MapEntry {
                scope: scope.clone(),
                id: (segment << SEGMENT_SHIFT) | u32::from(entry.index),
                members: entry
                    .live_members()
                    .iter()
                    .map(|member| (member.value, member.tag))
                    .collect(),
            });
        }
    }
    entries
}

/// One space-map member: its value, its tag, and the families the records
/// with that oid decode as.
type Member = (u32, u16, Vec<String>);

/// What the file says about one published oid, by question.
struct Findings<'a> {
    own: Vec<&'a Record>,
    referrers: Vec<(&'a MapEntry, Vec<Member>)>,
    references: Vec<(&'a MapEntry, u16, Vec<String>)>,
    mentions: Vec<(&'a Record, usize)>,
    raw: Vec<(String, Vec<usize>)>,
}

fn families_of(
    by_oid: &BTreeMap<(String, u32), Vec<&Record>>,
    scope: &str,
    oid: u32,
) -> Vec<String> {
    by_oid
        .get(&(scope.to_string(), oid))
        .map(|records| records.iter().map(|record| record.describe()).collect())
        .unwrap_or_default()
}

fn find<'a>(
    oid: u32,
    streams: &'a Streams,
    entries: &'a [MapEntry],
    by_oid: &BTreeMap<(String, u32), Vec<&'a Record>>,
) -> Findings<'a> {
    let own: Vec<&Record> = streams
        .chains
        .iter()
        .filter(|record| record.oid() == Some(oid))
        .collect();
    let referrers = entries
        .iter()
        .filter(|entry| entry.id == oid)
        .map(|entry| {
            let members = entry
                .members
                .iter()
                .map(|(value, tag)| (*value, *tag, families_of(by_oid, &entry.scope, *value)))
                .collect();
            (entry, members)
        })
        .collect();
    let references = entries
        .iter()
        .flat_map(|entry| {
            entry
                .members
                .iter()
                .filter(move |(value, _)| *value == oid)
                .map(move |(_, tag)| (entry, *tag, families_of(by_oid, &entry.scope, entry.id)))
        })
        .collect();
    let mentions = streams
        .chains
        .iter()
        .flat_map(|record| {
            word_offsets(&record.payload, oid)
                .into_iter()
                .filter(move |at| !(*at == 0 && record.oid() == Some(oid)))
                .map(move |at| (record, at))
        })
        .collect();
    let raw = streams
        .others
        .iter()
        .filter_map(|(path, data)| {
            let hits = word_offsets(data, oid);
            (!hits.is_empty()).then(|| (path.clone(), hits))
        })
        .collect();
    Findings {
        own,
        referrers,
        references,
        mentions,
        raw,
    }
}

/// The candidate third hop: published oid -> the one top-level entry that
/// lists it under tag 190, for a published oid that is no Sheet record.
fn attribute_set_aliases(
    index: &PidSemanticIndex,
    entries: &[MapEntry],
    by_oid: &BTreeMap<(String, u32), Vec<&Record>>,
) -> BTreeMap<u32, BTreeSet<u32>> {
    let mut out = BTreeMap::new();
    for object in index.objects() {
        let oid = object.graphic_oid;
        let is_sheet_record = by_oid
            .get(&("/".to_string(), oid))
            .is_some_and(|records| records.iter().any(|r| r.stream.starts_with("/Sheet")));
        if is_sheet_record {
            continue;
        }
        let targets: BTreeSet<u32> = entries
            .iter()
            .filter(|entry| {
                entry.scope == "/"
                    && entry
                        .members
                        .iter()
                        .any(|(value, tag)| *value == oid && *tag == ATTRIBUTE_SET_TAG)
            })
            .map(|entry| entry.id)
            .collect();
        out.insert(oid, targets);
    }
    out
}

/// `(stream, oid, kind, hop)` of every drawn entity that resolves, keyed by
/// the published oid it resolves to.
type Resolutions = BTreeMap<u32, Vec<(String, u32, String, String)>>;

fn resolutions(geometry: &NormalizedPidGeometry, index: &PidSemanticIndex) -> Resolutions {
    let mut out: Resolutions = BTreeMap::new();
    for entity in &geometry.entities {
        let Some(oid) = entity.graphic_oid else {
            continue;
        };
        let Some(hit) = index.resolve(oid) else {
            continue;
        };
        let hop = match hit {
            PidSemanticHit::Direct(_) => "direct".to_string(),
            PidSemanticHit::ViaDependency { dependency_oid, .. } => {
                format!("via dependency {dependency_oid}")
            }
        };
        let kind = format!("{:?}", entity.kind);
        let kind = kind
            .split([' ', '{', '('])
            .next()
            .unwrap_or_default()
            .to_string();
        out.entry(hit.object().graphic_oid).or_default().push((
            entity.source.stream_path.clone().unwrap_or_default(),
            oid,
            kind,
            hop,
        ));
    }
    out
}

fn print_resolutions(label: &str, table: &Resolutions, published: usize) {
    let entities: usize = table.values().map(Vec::len).sum();
    println!(
        "  {label}: {entities} drawn entities resolve, to {} of {published} published objects",
        table.len()
    );
    for (published_oid, hits) in table {
        let mut grouped: BTreeMap<(String, String, String), Vec<u32>> = BTreeMap::new();
        for (stream, oid, kind, hop) in hits {
            grouped
                .entry((stream.clone(), kind.clone(), hop.clone()))
                .or_default()
                .push(*oid);
        }
        for ((stream, kind, hop), oids) in grouped {
            println!(
                "     {published_oid} <- {stream} {kind} x{} ({hop}) oids {oids:?}",
                oids.len()
            );
        }
    }
}

fn object_line(object: &PidSemanticObject) -> String {
    format!(
        "{} {} {:?}",
        split_id(object.graphic_oid),
        object.class,
        object.label().unwrap_or("")
    )
}

fn main() {
    for (fixture, detailed) in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let doc = match PidParser::new().parse_file(fixture) {
            Ok(doc) => doc,
            Err(e) => {
                println!("skip: {fixture} did not parse: {e}");
                continue;
            }
        };
        let Some(index) = PidSemanticIndex::load_beside(path, &doc) else {
            println!("skip: no _Data.xml beside {fixture}");
            continue;
        };
        let streams = read_streams(path);
        let entries = map_entries(&doc);
        let mut by_oid: BTreeMap<(String, u32), Vec<&Record>> = BTreeMap::new();
        for record in &streams.chains {
            if let Some(oid) = record.oid() {
                by_oid
                    .entry((container_of(&record.stream), oid))
                    .or_default()
                    .push(record);
            }
        }

        println!("\n=== {fixture} ===");
        let mut scopes: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
        for entry in &entries {
            scopes
                .entry(entry.scope.clone())
                .or_default()
                .insert(entry.id >> SEGMENT_SHIFT);
        }
        println!("  space-map scopes and their segments: {scopes:?}");
        let mut chain_oids: BTreeMap<String, (usize, u32, u32)> = BTreeMap::new();
        for record in &streams.chains {
            let Some(oid) = record.oid() else { continue };
            let slot = chain_oids
                .entry(record.stream.clone())
                .or_insert((0, u32::MAX, 0));
            slot.0 += 1;
            slot.1 = slot.1.min(oid);
            slot.2 = slot.2.max(oid);
        }
        if detailed {
            println!("  streams (path, bytes, walks as a record chain):");
            for (stream, size, chain) in &streams.sizes {
                let oids = chain_oids
                    .get(stream)
                    .map(|(n, lo, hi)| format!("  {n} records, oid {lo}..{hi}"))
                    .unwrap_or_default();
                println!(
                    "    {stream} {size}{}{oids}",
                    if *chain { " chain" } else { "" }
                );
            }
        }

        for object in index.objects() {
            let found = find(object.graphic_oid, &streams, &entries, &by_oid);
            if !detailed {
                let own: BTreeSet<String> = found.own.iter().map(|r| r.family()).collect();
                let attribute_sets: Vec<String> = found
                    .referrers
                    .iter()
                    .flat_map(|(_, members)| members.iter())
                    .filter(|(_, tag, _)| *tag == ATTRIBUTE_SET_TAG)
                    .map(|(value, _, families)| format!("{} {families:?}", split_id(*value)))
                    .collect();
                println!(
                    "  {} | own {:?} | referrers {} | references {} | mentions {} | \
                     tag-190 referrers {attribute_sets:?}",
                    object_line(object),
                    own,
                    found.referrers.iter().map(|(_, m)| m.len()).sum::<usize>(),
                    found.references.len(),
                    found.mentions.len(),
                );
                continue;
            }
            println!("\n  -- {}", object_line(object));
            println!("     1. own record:");
            for record in &found.own {
                println!("        {}", record.describe());
            }
            if found.own.is_empty() {
                println!("        none in any chain stream");
            }
            println!("     2. space-map entry with this id (its referrers):");
            for (entry, members) in &found.referrers {
                println!("        scope {}", entry.scope);
                for (value, tag, families) in members {
                    println!(
                        "          referrer {} tag {tag}: {families:?}",
                        split_id(*value)
                    );
                }
            }
            if found.referrers.is_empty() {
                println!("        none");
            }
            println!("     3. entries listing it as a member (what it references):");
            for (entry, tag, families) in &found.references {
                println!(
                    "        scope {} entry {} under tag {tag}: {families:?}",
                    entry.scope,
                    split_id(entry.id)
                );
            }
            if found.references.is_empty() {
                println!("        none");
            }
            println!("     4. other chain records holding it as a word:");
            for (record, at) in &found.mentions {
                println!(
                    "        oid {:?} {} at payload+{at}",
                    record.oid(),
                    record.describe()
                );
            }
            if found.mentions.is_empty() {
                println!("        none");
            }
            println!("     5. streams outside the chains:");
            for (path, hits) in &found.raw {
                let shown: Vec<String> = hits.iter().map(|at| format!("0x{at:X}")).collect();
                println!("        {path} at {}", shown.join(", "));
            }
            if found.raw.is_empty() {
                println!("        none");
            }
        }

        println!("\n  6. drawn entities resolved, today and with the attribute-set hop");
        let aliases = attribute_set_aliases(&index, &entries, &by_oid);
        for (oid, targets) in &aliases {
            println!("     not a Sheet record: {oid} -> tag-190 entries {targets:?}");
        }
        let geometry = build_normalized_geometry(&doc);
        let published = index.len();
        print_resolutions("today", &resolutions(&geometry, &index), published);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let xml_path = path.with_file_name(format!("{stem}_Data.xml"));
        let Ok(mut xml) = std::fs::read_to_string(&xml_path) else {
            continue;
        };
        for (oid, targets) in &aliases {
            if let (1, Some(target)) = (targets.len(), targets.first()) {
                xml = xml.replace(
                    &format!("GraphicOID=\"{oid}\""),
                    &format!("GraphicOID=\"{target}\""),
                );
            }
        }
        let hopped = PidSemanticIndex::from_xml(&xml, &doc);
        let mut rekeyed: Resolutions = BTreeMap::new();
        for (target, hits) in resolutions(&geometry, &hopped) {
            let original = aliases
                .iter()
                .find(|(_, targets)| targets.len() == 1 && targets.first() == Some(&target))
                .map_or(target, |(oid, _)| *oid);
            rekeyed.entry(original).or_default().extend(hits);
        }
        print_resolutions("with the hop", &rekeyed, published);

        if detailed {
            exhibit(&doc, &geometry);
            uid_scan(&xml, &streams);
        }

        let joined = uid_join(&index, &streams, &entries);
        let Ok(mut xml) = std::fs::read_to_string(&xml_path) else {
            continue;
        };
        for (oid, target) in &joined {
            xml = xml.replace(
                &format!("GraphicOID=\"{oid}\""),
                &format!("GraphicOID=\"{target}\""),
            );
        }
        let by_uid = PidSemanticIndex::from_xml(&xml, &doc);
        let mut rekeyed: Resolutions = BTreeMap::new();
        for (target, hits) in resolutions(&geometry, &by_uid) {
            let original = joined
                .iter()
                .find(|(_, joined_target)| **joined_target == target)
                .map_or(target, |(oid, _)| *oid);
            rekeyed.entry(original).or_default().extend(hits);
        }
        print_resolutions("with the UID join", &rekeyed, published);
    }
}

/// The join the file itself affords: each published representation's UID
/// sits as ASCII inside one `FreeFormAttrSet` (`0x0089`) of the top-level
/// storage, and that set points at its record under tag 190. Returns the
/// published oids whose join lands on exactly one record, with that record.
fn uid_join(
    index: &PidSemanticIndex,
    streams: &Streams,
    entries: &[MapEntry],
) -> BTreeMap<u32, u32> {
    println!("\n  9. representation UID -> the attribute set holding it -> its tag-190 record");
    let mut joined = BTreeMap::new();
    let mut agree = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for object in index.objects() {
        let needle = object.representation_uid.as_bytes();
        let sets: BTreeSet<u32> = streams
            .chains
            .iter()
            .filter(|record| {
                container_of(&record.stream) == "/"
                    && record.type_code() == 0x0089
                    && record.payload.windows(needle.len()).any(|w| w == needle)
            })
            .filter_map(Record::oid)
            .collect();
        let targets: BTreeSet<u32> = entries
            .iter()
            .filter(|entry| {
                entry.scope == "/"
                    && entry
                        .members
                        .iter()
                        .any(|(value, tag)| sets.contains(value) && *tag == ATTRIBUTE_SET_TAG)
            })
            .map(|entry| entry.id)
            .collect();
        let verdict = match (sets.len(), targets.len()) {
            (0, _) => "no attribute set holds the UID".to_string(),
            (_, 1) if targets.contains(&object.graphic_oid) => {
                agree += 1;
                "= GraphicOID".to_string()
            }
            (_, 1) => "differs from GraphicOID".to_string(),
            (_, 0) => "the set points at nothing under tag 190".to_string(),
            _ => "several records".to_string(),
        };
        if let (1, Some(target)) = (targets.len(), targets.first()) {
            joined.insert(object.graphic_oid, *target);
        }
        if verdict != "= GraphicOID" {
            lines.push(format!(
                "     {} rep {} -> sets {sets:?} -> records {targets:?}: {verdict}",
                object_line(object),
                object.representation_uid
            ));
        }
    }
    println!(
        "     {agree} of {} published objects: the UID join lands on the GraphicOID's own record",
        index.len()
    );
    for line in lines {
        println!("{line}");
    }
    joined
}

/// Every `IObject/@UID` of the published XML, looked for in the file as
/// ASCII and as UTF-16LE, and placed in its chain record when it has one.
fn uid_scan(xml: &str, streams: &Streams) {
    println!("\n  8. published UIDs inside the .pid");
    let mut uids: Vec<(String, String)> = Vec::new();
    let mut element = String::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('<') {
            if !rest.starts_with('I') && !rest.starts_with('/') && !rest.starts_with('?') {
                element = rest
                    .split([' ', '>'])
                    .next()
                    .unwrap_or_default()
                    .to_string();
            }
        }
        if let Some(at) = trimmed.find("<IObject UID=\"") {
            let rest = &trimmed[at + 14..];
            if let Some(end) = rest.find('"') {
                uids.push((element.clone(), rest[..end].to_string()));
            }
        }
        if let Some(at) = trimmed.find("ItemTag=\"") {
            let rest = &trimmed[at + 9..];
            if let Some(end) = rest.find('"') {
                uids.push((format!("{element} ItemTag"), rest[..end].to_string()));
            }
        }
    }
    let mut chain_bytes: Vec<(&Record, Vec<u8>)> = Vec::new();
    for record in &streams.chains {
        chain_bytes.push((record, record.payload.clone()));
    }
    for (element, uid) in &uids {
        let ascii = uid.as_bytes().to_vec();
        let wide: Vec<u8> = uid.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut hits: Vec<String> = Vec::new();
        for (form, needle) in [("ascii", &ascii), ("utf16", &wide)] {
            for (record, payload) in &chain_bytes {
                for at in payload
                    .windows(needle.len())
                    .enumerate()
                    .filter(|(_, window)| window == needle)
                    .map(|(at, _)| at)
                {
                    hits.push(format!(
                        "{form} in oid {:?} {} at payload+{at}",
                        record.oid(),
                        record.describe()
                    ));
                }
            }
            for (path, data) in &streams.others {
                for at in data
                    .windows(needle.len())
                    .enumerate()
                    .filter(|(_, window)| window == needle)
                    .map(|(at, _)| at)
                {
                    hits.push(format!("{form} in {path} at 0x{at:X}"));
                }
            }
        }
        println!(
            "     {element} {uid}: {}",
            if hits.is_empty() {
                "nowhere".to_string()
            } else {
                hits.join("; ")
            }
        );
    }
}

/// What the hop lands on: the named roots, every top-level Sheet entity,
/// and every top-level `DependencyObject` with the pool oids its tail names.
fn exhibit(doc: &PidDocument, geometry: &NormalizedPidGeometry) {
    println!("\n  7. exhibit");
    if let Some(roots) = &doc.psm_roots {
        let named: Vec<String> = roots
            .entries
            .iter()
            .map(|root| format!("{} {}", root.id, root.name))
            .collect();
        println!("     PSMroots: {}", named.join(", "));
    }
    for entity in &geometry.entities {
        let stream = entity.source.stream_path.as_deref().unwrap_or_default();
        if !stream.starts_with("/Sheet") || entity.graphic_oid.is_none() {
            continue;
        }
        let kind = match &entity.kind {
            PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                ..
            } => format!(
                "SymbolInstance at ({:.4}, {:.4}) {}",
                insertion.x,
                insertion.y,
                symbol_path
                    .as_deref()
                    .and_then(|path| path.rsplit('\\').nth(1).zip(path.rsplit('\\').next()))
                    .map(|(folder, file)| format!("{folder}\\{file}"))
                    .unwrap_or_default()
            ),
            other => format!("{other:?}").chars().take(150).collect(),
        };
        println!("     {stream} oid {:?} {kind}", entity.graphic_oid);
    }
    for sheet in &doc.sheet_streams {
        if !sheet.path.starts_with("/Sheet") {
            continue;
        }
        let Some(sheet_geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for dependency in &sheet_geometry.decoded_dependency_objects {
            let tail = &dependency.raw_reference_payload;
            let words: Vec<String> = (0..tail.len().saturating_sub(3))
                .step_by(4)
                .filter_map(|at| {
                    let value = u32_at(tail, at)?;
                    (value != 0).then(|| format!("+{}={value}", at + 18))
                })
                .collect();
            println!(
                "     {} DependencyObject {} kind {} sub {} flags {}: {}",
                sheet.path,
                dependency.oid,
                dependency.group_kind_word,
                dependency.sub_type_word,
                dependency.type_flags,
                words.join(" ")
            );
        }
    }
}
