//! Do the 46 space-map members tagged `188` land in the reference slots
//! `0x0115 JDim` was read to have?
//!
//! Two rounds left this as an unsettled join.
//!
//! * `2026-08-27-the-spacemap-is-an-incoming-reference-index.md` §3 turned
//!   the map around: a member is an *incoming* reference, so an entry is the
//!   object being pointed at and `member.value` is the referrer. Its per-tag
//!   table says tag `188`'s referrer family is `0x0115` and that the entry's
//!   id shows up in the referrer's payload "at `+49..+280`, several places",
//!   42 of 46. That row is the only one in the table without a named offset,
//!   and the four misses were never accounted for.
//! * `2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`
//!   §5 then read the JDim record from the other side and named the slots:
//!   `+92` is the geometry the dimension measures (`Line` / `Point`, marker
//!   `0x00CB` / `0x00F0`), `+140` is what the dimension belongs to
//!   (`JDimGroup` / `Vertical Constraint` / `JSheet` / `FreeFormAttrSet`,
//!   marker `0x0067` / `0x008C`), with `+202` / `+250` repeating the pair in
//!   a second block and `+280` overlapping the closing dword.
//!
//! Those two readings were produced independently and have never been put
//! next to each other. If `+92` really is the measured geometry, then every
//! tag-188 member whose entry is a `Line` or a `Point` has to land at `+92`
//! (or `+202`, its second-block twin) and nowhere else -- and conversely no
//! `JDimGroup` may land there. If the offset and the entry's record class
//! turn out to be independent, the §5 naming is refuted and the two slots
//! are something else.
//!
//! So this probe joins the two directly, member by member, with no
//! sampling:
//!
//! 1. Walk every `PSMspacemap` segment of the four sheet documents and take
//!    every live member whose tag is `188`.
//! 2. For each, resolve `member.value` to a record of the *same storage* --
//!    the index space a persist id is only meaningful inside -- and check it
//!    really is `0x0115`.
//! 3. Scan that JDim payload for the entry's own id as a `u32`, at every
//!    offset, and report each hit with the `u16` sitting four bytes behind
//!    it: the marker word §5 pairs with the slot.
//! 4. Resolve the entry's id to *its* record, so each row also states what
//!    class of object the dimension is pointing at.
//!
//! What would settle it, and what each shape of answer means:
//!
//! * every hit at a slot §5 named, with the class/marker pairing holding ->
//!   the two readings are one reading, and `+92` / `+140` keep their names;
//! * hits spread over offsets §5 does not name -> the "several places" of
//!   the 08-27 row is literal and the slots are not slots;
//! * the class at `+92` mixing geometry with groups -> `+92` is refuted.
//!
//! The misses get their own section, dumping the entry's record, the JDim's
//! frame and every slot the JDim does hold, because "42 of 46" is only an
//! answer once the other four have one.
//!
//! Section 8 turns on the other named slot. `+140` was called the owner on
//! the strength of resolving to *some* record 14 times out of 18; the map
//! never recorded a tag-188 member behind it. So it gets the two tests a
//! reference has to pass -- does the raw value vary with the storage's own
//! index space, and does the map record the edge under any tag -- with the
//! closing dword put through the same two for contrast.
//!
//! Findings are written up in
//! `docs/analysis/2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`.
//!
//! ```powershell
//! cargo run --example probe_tag188_members_land_in_jdim_slots
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::rad_class_name;
use pid_parse::PidParser;

/// The four sheet documents the 2026-08-27 space-map corpus is drawn from.
/// The 46 is theirs; A01 is not in it.
const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// The export fixture. It carries four more JDim records, so it can say
/// whether the slot grammar is a property of these four files or of the
/// format -- but its members are reported apart from the 46.
const CROSS_CHECK: &str = "test-file/export-test/publish-data/A01/A01.pid";

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
/// Bits of a persist id that address the index inside its segment.
const SEGMENT_SHIFT: u32 = 13;
/// PSM type code of the dimension record.
const JDIM: u16 = 0x0115;
/// The space-map tag whose referrer family is [`JDIM`].
const TAG: u16 = 188;

/// The reference slots the 2026-09-14 reading named, with the role it gave
/// each one. Offsets are payload-relative.
const NAMED_SLOTS: [(usize, &str); 5] = [
    (92, "measured geometry"),
    (140, "owner"),
    (202, "measured geometry, block two"),
    (250, "owner, block two"),
    (280, "measured geometry, tail"),
];

/// The classes `+92` / `+202` / `+280` were read as pointing at.
const GEOMETRY_CLASSES: [u16; 2] = [0x0018, 0x005E];
/// The classes `+140` / `+250` were read as pointing at.
const OWNER_CLASSES: [u16; 4] = [0x0058, 0x0085, 0x0089, 0x0114];

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

/// `0x0018 Line Object`, or `0x0018 ?` when the registry has no name.
fn class_label(type_code: u16) -> String {
    format!(
        "0x{type_code:04X} {}",
        rad_class_name(type_code).unwrap_or("?")
    )
}

/// The role `at` plays, if the 2026-09-14 reading named it.
fn slot_role(at: usize) -> Option<&'static str> {
    NAMED_SLOTS
        .iter()
        .find_map(|(offset, role)| (*offset == at).then_some(*role))
}

/// One record of a chain stream, payload kept whole so any offset can be
/// asked about later.
struct Record {
    /// Leaf name of the stream the record sits in (`PSMcluster0`, `Sheet6`…).
    stream: String,
    type_code: u16,
    payload: Vec<u8>,
}

/// PSM type code of the dimension group the closing dword names.
const JDIM_GROUP: u16 = 0x0058;

/// The dimensions a `0x0058 JDimGroup` record lists: a `u16` count at `+24`,
/// then one `u16` oid every four bytes from `+26`.
///
/// Read off the corpus, and only worth reading because it closes the loop on
/// [`group_of`] from the other side: the dimension names the group in its
/// closing dword, the group names the dimension back.
fn group_members(record: &Record) -> BTreeSet<u32> {
    let count = usize::from(u16_at(&record.payload, 24).unwrap_or_default());
    (0..count)
        .filter_map(|index| u16_at(&record.payload, 26 + 4 * index))
        .map(u32::from)
        .collect()
}

/// The `JDimGroup` a dimension's closing dword names, if it has one.
///
/// `radsrvitem.dll!sub_564BB990` reads that dword at `34 + main_len` and only
/// under flag `0x0100` (2026-09-14 §2). What it holds had no reading; on this
/// corpus it is a `0x0058 JDimGroup Object` on all 13 records that carry it.
fn group_of(record: &Record) -> Option<u32> {
    let flags = u16_at(&record.payload, 26)?;
    if flags & 0x0100 == 0 {
        return None;
    }
    let main_len = u32_at(&record.payload, 30)? as usize;
    u32_at(&record.payload, 34 + main_len)
}

/// One space-map entry, reduced to what the join needs.
struct MapEntry {
    id: u32,
    /// `(value, tag)` for the live members only.
    members: Vec<(u32, u16)>,
}

/// Everything one storage scope holds. A persist id only resolves inside
/// its own scope: the top-level map and each `JSite` registry number their
/// objects independently.
#[derive(Default)]
struct Scope {
    records: BTreeMap<u32, Vec<Record>>,
    entries: Vec<MapEntry>,
}

struct Doc {
    name: &'static str,
    scopes: BTreeMap<String, Scope>,
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

/// Every record-chain stream of the file, walked.
fn chain_streams(path: &Path) -> Vec<ChainStream> {
    let mut chains = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return chains;
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return chains;
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
        let records = sheet_record_starts(&data)
            .iter()
            .filter_map(|&at| {
                let type_code = u16_at(&data, at)? & 0x3FFF;
                let len = u32_at(&data, at + 2)? as usize;
                Some((type_code, data.get(at + 6..at + 6 + len)?.to_vec()))
            })
            .collect();
        chains.push((stream_path, records));
    }
    chains
}

fn load(fixture: &'static str) -> Option<Doc> {
    let path = Path::new(fixture);
    if !path.exists() {
        println!("skip: {fixture} is absent");
        return None;
    }
    let doc = match PidParser::new().parse_file(fixture) {
        Ok(doc) => doc,
        Err(error) => {
            println!("skip: {fixture} did not parse: {error}");
            return None;
        }
    };

    let mut scopes: BTreeMap<String, Scope> = BTreeMap::new();
    for (stream_path, records) in chain_streams(path) {
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

/// Where one tag-188 member's entry id turns up inside the referring JDim.
struct Hit {
    at: usize,
    /// The `u16` four bytes behind the reference: §5's marker word.
    marker: u16,
}

impl Hit {
    /// A reference slot, by §5's own definition: a `u32` naming an object of
    /// the same storage with a non-zero marker word behind it. A zero marker
    /// means the four bytes are not a slot at all -- see [`aliases`].
    fn is_slot(&self) -> bool {
        self.marker != 0
    }

    fn role(&self) -> &'static str {
        match (slot_role(self.at), self.is_slot()) {
            (Some(role), true) => role,
            (Some(_), false) => "named offset, zero marker",
            (None, true) => "UNNAMED SLOT",
            (None, false) => "not a slot, zero marker",
        }
    }
}

/// One tag-188 member, with both ends resolved.
struct Landing {
    fixture: &'static str,
    storage: String,
    /// The object the dimension points at, i.e. the entry carrying the member.
    entry_id: u32,
    /// Its record's type code and home stream, when it has a record.
    entry_class: Option<u16>,
    entry_stream: Option<String>,
    /// The referrer: `member.value`.
    jdim_oid: u32,
    /// The referrer's record type code, when it has a record. The 08-27 table
    /// says this is always `0x0115`; the probe checks rather than assumes.
    jdim_class: Option<u16>,
    /// Every payload offset of the JDim holding `entry_id`.
    hits: Vec<Hit>,
}

impl Landing {
    /// The hits that are a reference slot the 2026-09-14 reading named.
    fn slot_hits(&self) -> Vec<&Hit> {
        self.hits
            .iter()
            .filter(|hit| hit.is_slot() && slot_role(hit.at).is_some())
            .collect()
    }

    /// The hits that are not a slot: the `u32` is there, but the marker word
    /// behind it is zero, so nothing distinguishes it from bytes that happen
    /// to spell the id. [`aliases`] shows what they really are.
    fn aliases(&self) -> Vec<&Hit> {
        self.hits.iter().filter(|hit| !hit.is_slot()).collect()
    }
}

/// Every tag-188 member of one document, both ends resolved.
fn collect_landings(doc: &Doc) -> Vec<Landing> {
    let mut out = Vec::new();
    for (storage, scope) in &doc.scopes {
        for entry in &scope.entries {
            for (value, tag) in &entry.members {
                if *tag != TAG {
                    continue;
                }
                let referrer = scope.records.get(value).and_then(|records| records.first());
                let target = scope
                    .records
                    .get(&entry.id)
                    .and_then(|records| records.first());
                let hits = referrer
                    .map(|record| {
                        (0..record.payload.len().saturating_sub(3))
                            .filter(|at| u32_at(&record.payload, *at) == Some(entry.id))
                            .map(|at| Hit {
                                at,
                                marker: u16_at(&record.payload, at + 4).unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(Landing {
                    fixture: doc.name,
                    storage: storage.clone(),
                    entry_id: entry.id,
                    entry_class: target.map(|record| record.type_code),
                    entry_stream: target.map(|record| record.stream.clone()),
                    jdim_oid: *value,
                    jdim_class: referrer.map(|record| record.type_code),
                    hits,
                });
            }
        }
    }
    out
}

/// Deliverable one: the row-by-row table.
fn table(landings: &[Landing]) {
    println!(
        "\n=== 1. every tag-{TAG} member, one row each ({} members) ===\n",
        landings.len()
    );
    println!(
        "  {:<26} {:<12} {:>7} {:>7} {:>6} {:<30} {:>8}  entry record class",
        "document", "storage", "entry", "JDim", "slot", "role", "marker"
    );
    for landing in landings {
        let first = landing
            .slot_hits()
            .first()
            .copied()
            .or_else(|| landing.hits.first());
        let (slot, role, marker) = match first {
            Some(hit) => (
                format!("+{}", hit.at),
                hit.role().to_string(),
                format!("0x{:04X}", hit.marker),
            ),
            None => (
                "--".to_string(),
                "NOT IN PAYLOAD".to_string(),
                "--".to_string(),
            ),
        };
        println!(
            "  {:<26} {:<12} {:>7} {:>7} {:>6} {:<30} {:>8}  {}",
            leaf(landing.fixture),
            landing.storage,
            landing.entry_id,
            landing.jdim_oid,
            slot,
            role,
            marker,
            landing
                .entry_class
                .map_or_else(|| "no record".to_string(), class_label),
        );
        let shown = first.map(|hit| hit.at);
        for extra in landing.hits.iter().filter(|hit| Some(hit.at) != shown) {
            println!(
                "  {:<26} {:<12} {:>7} {:>7} {:>6} {:<30} {:>8}  (same member, another hit)",
                "",
                "",
                "",
                "",
                format!("+{}", extra.at),
                extra.role(),
                format!("0x{:04X}", extra.marker),
            );
        }
    }
}

/// The account the 08-27 row owes: how many of the members land, where, and
/// what the referrer turned out to be.
fn account(landings: &[Landing]) {
    println!("\n=== 2. the account ===");
    let landed = landings
        .iter()
        .filter(|landing| !landing.hits.is_empty())
        .count();
    let in_slot = landings
        .iter()
        .filter(|landing| !landing.slot_hits().is_empty())
        .count();
    let alias_only = landings
        .iter()
        .filter(|landing| landing.slot_hits().is_empty() && !landing.hits.is_empty())
        .count();
    println!(
        "  {} members; {landed} have the entry id somewhere in the referrer's payload \
         (the 08-27 count); of those, {in_slot} sit in a reference slot the 2026-09-14 reading \
         named and {alias_only} only in bytes that are not a slot at all",
        landings.len()
    );

    let mut referrer_classes: BTreeMap<String, usize> = BTreeMap::new();
    for landing in landings {
        *referrer_classes
            .entry(
                landing
                    .jdim_class
                    .map_or_else(|| "no record".to_string(), class_label),
            )
            .or_default() += 1;
    }
    println!("  the referrer's record class: {referrer_classes:?}");

    let mut per_offset: BTreeMap<usize, usize> = BTreeMap::new();
    for landing in landings {
        for hit in &landing.hits {
            *per_offset.entry(hit.at).or_default() += 1;
        }
    }
    println!("  hits per offset:");
    for (at, hits) in &per_offset {
        println!(
            "    +{at:<5} x{hits:<3} {}",
            slot_role(*at).unwrap_or("NOT A NAMED SLOT")
        );
    }

    let mut per_storage: BTreeMap<(&str, &str), (usize, usize)> = BTreeMap::new();
    for landing in landings {
        let slot = per_storage
            .entry((leaf(landing.fixture), landing.storage.as_str()))
            .or_default();
        slot.0 += 1;
        if !landing.hits.is_empty() {
            slot.1 += 1;
        }
    }
    println!("  per storage (members, of which landed):");
    for ((fixture, storage), (members, landed)) in &per_storage {
        println!("    {fixture:<26} {storage:<12} {members:>3} {landed:>3}");
    }
}

/// The test the two readings can fail: does the offset predict the class of
/// the object at the other end, and does the marker word go with it?
fn slot_vs_class(landings: &[Landing]) {
    println!("\n=== 3. does the slot predict what the dimension points at? ===");
    let mut per_slot: BTreeMap<usize, BTreeMap<String, usize>> = BTreeMap::new();
    let mut per_slot_marker: BTreeMap<usize, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut marker_vs_class: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    for landing in landings {
        for hit in &landing.hits {
            let class = landing
                .entry_class
                .map_or_else(|| "no record".to_string(), class_label);
            *per_slot
                .entry(hit.at)
                .or_default()
                .entry(class.clone())
                .or_default() += 1;
            *per_slot_marker
                .entry(hit.at)
                .or_default()
                .entry(hit.marker)
                .or_default() += 1;
            *marker_vs_class
                .entry(hit.marker)
                .or_default()
                .entry(class)
                .or_default() += 1;
        }
    }
    for (at, classes) in &per_slot {
        let markers: Vec<String> = per_slot_marker
            .get(at)
            .map(|census| {
                census
                    .iter()
                    .map(|(marker, hits)| format!("0x{marker:04X} x{hits}"))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "  +{at:<5} {:<30} markers {}",
            slot_role(*at).unwrap_or("NOT A NAMED SLOT"),
            markers.join(", ")
        );
        for (class, hits) in classes {
            println!("        {class:<34} x{hits}");
        }
    }
    println!("\n  the marker word against the class at the other end:");
    for (marker, classes) in &marker_vs_class {
        let text: Vec<String> = classes
            .iter()
            .map(|(class, hits)| format!("{class} x{hits}"))
            .collect();
        println!("    0x{marker:04X} -> {}", text.join(", "));
    }

    let geometry_slots: BTreeSet<usize> = [92usize, 202, 280].into_iter().collect();
    let owner_slots: BTreeSet<usize> = [140usize, 250].into_iter().collect();
    let mut counterexamples = Vec::new();
    for landing in landings {
        for hit in landing.hits.iter().filter(|hit| hit.is_slot()) {
            let Some(class) = landing.entry_class else {
                continue;
            };
            let wrong = (geometry_slots.contains(&hit.at) && !GEOMETRY_CLASSES.contains(&class))
                || (owner_slots.contains(&hit.at) && !OWNER_CLASSES.contains(&class));
            if wrong {
                counterexamples.push(format!(
                    "    +{} holds {} (entry {} of {}{})",
                    hit.at,
                    class_label(class),
                    landing.entry_id,
                    leaf(landing.fixture),
                    landing.storage
                ));
            }
        }
    }
    if counterexamples.is_empty() {
        println!(
            "\n  no counterexample: every geometry slot points at {:?}, every owner slot at {:?}",
            GEOMETRY_CLASSES.map(class_label),
            OWNER_CLASSES.map(class_label)
        );
    } else {
        println!("\n  counterexamples to the 2026-09-14 naming:");
        for line in &counterexamples {
            println!("{line}");
        }
    }
}

/// What the zero-marker hits really are.
///
/// The 08-27 row counted "the entry id appears in the referrer's payload"
/// at any offset, which is a weaker test than §5's slot: four bytes spelling
/// a small id can fall out of a double. Each of these gets the `f64` that
/// *ends* at the hit printed next to it, because that is the whole story --
/// an IEEE-754 double in `[2^-15, 1)` has `0x3F` as its top byte, so a
/// double in that range followed by three zero bytes reads back as the u32
/// `0x0000003F` = 63.
fn aliases(landings: &[Landing], docs: &[Doc]) {
    println!("\n=== 4. the hits that are not slots ===");
    let mut any = false;
    for landing in landings {
        let alias_hits = landing.aliases();
        if alias_hits.is_empty() {
            continue;
        }
        any = true;
        let payload = docs
            .iter()
            .find(|doc| doc.name == landing.fixture)
            .and_then(|doc| doc.scopes.get(&landing.storage))
            .and_then(|scope| scope.records.get(&landing.jdim_oid))
            .and_then(|records| records.first())
            .map(|record| record.payload.as_slice())
            .unwrap_or_default();
        println!(
            "\n  {}{} entry {} <- JDim {}: {} of its {} hits carry a zero marker",
            leaf(landing.fixture),
            landing.storage,
            landing.entry_id,
            landing.jdim_oid,
            alias_hits.len(),
            landing.hits.len()
        );
        for hit in alias_hits {
            let bytes: Vec<String> = payload
                .get(hit.at..hit.at + 4)
                .unwrap_or_default()
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect();
            println!(
                "    +{:<5} bytes {} = {}; the f64 ending at this byte (+{}) is {:e}",
                hit.at,
                bytes.join(" "),
                landing.entry_id,
                hit.at.saturating_sub(7),
                f64_at(payload, hit.at.saturating_sub(7)).unwrap_or_default()
            );
        }
    }
    if !any {
        println!("  none: every hit has a non-zero marker word behind it");
    }
}

/// The members whose entry id is nowhere in the referrer's payload. "42 of
/// 46" is only an answer once these have one, so each gets its record, its
/// referrer's frame, every slot that referrer does hold, and a sweep of the
/// whole storage for the id -- if some other record carries the reference,
/// the map is not inventing an edge, it is crediting it to the dimension.
fn misses(landings: &[Landing], docs: &[Doc]) {
    let missed: Vec<&Landing> = landings
        .iter()
        .filter(|landing| landing.slot_hits().is_empty())
        .collect();
    println!(
        "\n=== 5. the {} members that sit in no reference slot ===",
        missed.len()
    );
    for landing in &missed {
        println!(
            "\n  {}{} entry {} <- JDim {}",
            leaf(landing.fixture),
            landing.storage,
            landing.entry_id,
            landing.jdim_oid
        );
        println!(
            "    the entry's own record : {} in {}",
            landing.entry_class.map_or_else(
                || "NONE -- no record with this oid".to_string(),
                class_label
            ),
            landing.entry_stream.as_deref().unwrap_or("--")
        );
        let Some(doc) = docs.iter().find(|doc| doc.name == landing.fixture) else {
            continue;
        };
        let Some(scope) = doc.scopes.get(&landing.storage) else {
            continue;
        };
        if let Some(target) = scope
            .records
            .get(&landing.entry_id)
            .and_then(|records| records.first())
        {
            let head: Vec<String> = (0..6)
                .filter_map(|word| u32_at(&target.payload, word * 4))
                .map(|word| word.to_string())
                .collect();
            println!(
                "    its payload            : {} bytes, first words {}",
                target.payload.len(),
                head.join(" ")
            );
        }
        let Some(referrer) = scope
            .records
            .get(&landing.jdim_oid)
            .and_then(|records| records.first())
        else {
            println!("    the referrer           : NONE -- no record with this oid");
            continue;
        };
        println!(
            "    the referrer           : {} in {}, {} bytes, flags 0x{:04X}, main_len {}",
            class_label(referrer.type_code),
            referrer.stream,
            referrer.payload.len(),
            u16_at(&referrer.payload, 26).unwrap_or_default(),
            u32_at(&referrer.payload, 30).unwrap_or_default()
        );
        println!("    the slots it does hold :");
        for (at, role) in NAMED_SLOTS {
            let Some(oid) = u32_at(&referrer.payload, at) else {
                println!("      +{at:<5} {role:<32} past the end of this payload");
                continue;
            };
            let class = scope
                .records
                .get(&oid)
                .and_then(|records| records.first())
                .map(|record| class_label(record.type_code))
                .unwrap_or_else(|| "resolves to no record".to_string());
            println!(
                "      +{at:<5} {role:<32} oid {oid:<6} marker 0x{:04X}  {class}",
                u16_at(&referrer.payload, at + 4).unwrap_or_default()
            );
        }
        if let Some(group) = group_of(referrer) {
            let record = scope
                .records
                .get(&group)
                .and_then(|records| records.first());
            println!(
                "    the group its tail names: oid {group} {}, {} bytes",
                record.map_or_else(
                    || "no record".to_string(),
                    |record| class_label(record.type_code)
                ),
                record.map_or(0, |record| record.payload.len())
            );
            let spelled: Vec<String> = record
                .map(|record| {
                    (0..record.payload.len().saturating_sub(5))
                        .filter(|at| u32_at(&record.payload, *at) == Some(landing.entry_id))
                        .map(|at| {
                            format!(
                                "+{at} marker 0x{:04X}",
                                u16_at(&record.payload, at + 4).unwrap_or_default()
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            println!(
                "    the group's payload spells the entry id at: {}",
                if spelled.is_empty() {
                    "nowhere".to_string()
                } else {
                    spelled.join(", ")
                }
            );
        }
        let members: Vec<String> = scope
            .entries
            .iter()
            .find(|entry| entry.id == landing.entry_id)
            .map(|entry| {
                entry
                    .members
                    .iter()
                    .map(|(value, tag)| {
                        let class = scope
                            .records
                            .get(value)
                            .and_then(|records| records.first())
                            .map_or_else(
                                || "no record".to_string(),
                                |record| class_label(record.type_code),
                            );
                        format!("({value}, t{tag}) {class}")
                    })
                    .collect()
            })
            .unwrap_or_default();
        println!("    the entry's referrers, in map order:");
        for member in &members {
            println!("      {member}");
        }
        println!("    every slot-shaped reference to the entry in this storage:");
        let mut found = 0usize;
        for (oid, records) in &scope.records {
            for record in records {
                for at in 0..record.payload.len().saturating_sub(5) {
                    if u32_at(&record.payload, at) != Some(landing.entry_id) {
                        continue;
                    }
                    let marker = u16_at(&record.payload, at + 4).unwrap_or_default();
                    if marker == 0 {
                        continue;
                    }
                    found += 1;
                    println!(
                        "      oid {oid:<6} {:<30} +{at:<5} marker 0x{marker:04X}",
                        class_label(record.type_code),
                    );
                }
            }
        }
        if found == 0 {
            println!("      none");
        }
    }
}

/// The other direction, and the reason the two counts differ: every JDim of
/// the corpus, with the slots it fills and whether each one came back as a
/// tag-188 member. A filled slot with no member means the map did not record
/// the edge; a member with no filled slot is a §4 miss.
fn slots_without_members(docs: &[Doc], landings: &[Landing]) {
    println!("\n=== 6. the other direction: every JDim slot, member or not ===");
    // How many members each (referrer, target) pair produced. The count is
    // the point: a slot the payload states shows up twice, which is what
    // makes a lone member stand out as an edge with no slot behind it.
    let mut member_count: BTreeMap<(&str, &str, u32, u32), usize> = BTreeMap::new();
    for landing in landings {
        *member_count
            .entry((
                landing.fixture,
                landing.storage.as_str(),
                landing.jdim_oid,
                landing.entry_id,
            ))
            .or_default() += 1;
    }
    let mut filled = 0usize;
    let mut with_member = 0usize;
    for doc in docs {
        for (storage, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                for record in records.iter().filter(|record| record.type_code == JDIM) {
                    let main_len = u32_at(&record.payload, 30).unwrap_or_default() as usize;
                    let flags = u16_at(&record.payload, 26).unwrap_or_default();
                    let mut slot_targets: BTreeSet<u32> = BTreeSet::new();
                    let mut line = Vec::new();
                    for (at, _) in NAMED_SLOTS {
                        let Some(target) = u32_at(&record.payload, at) else {
                            continue;
                        };
                        let Some(class) = scope
                            .records
                            .get(&target)
                            .and_then(|hit| hit.first())
                            .map(|hit| hit.type_code)
                        else {
                            continue;
                        };
                        filled += 1;
                        slot_targets.insert(target);
                        let members = member_count
                            .get(&(doc.name, storage.as_str(), *oid, target))
                            .copied()
                            .unwrap_or_default();
                        if members > 0 {
                            with_member += 1;
                        }
                        line.push(format!("+{at}->{target} {} x{members}", class_label(class)));
                    }
                    let orphans: Vec<String> = member_count
                        .iter()
                        .filter(|((fixture, home, referrer, target), _)| {
                            *fixture == doc.name
                                && *home == storage.as_str()
                                && referrer == oid
                                && !slot_targets.contains(target)
                        })
                        .map(|((_, _, _, target), count)| {
                            let class = scope
                                .records
                                .get(target)
                                .and_then(|records| records.first())
                                .map_or_else(
                                    || "no record".to_string(),
                                    |hit| class_label(hit.type_code),
                                );
                            format!("{target} x{count} {class}")
                        })
                        .collect();
                    let tail_text = match group_of(record) {
                        Some(value) => {
                            let class = scope
                                .records
                                .get(&value)
                                .and_then(|records| records.first())
                                .map_or_else(
                                    || "no record".to_string(),
                                    |hit| class_label(hit.type_code),
                                );
                            format!("+{} = {value} {class}", 34 + main_len)
                        }
                        None => "none".to_string(),
                    };
                    println!(
                        "  {}{storage} JDim {oid:<6} {} bytes, main_len {main_len}, flags \
                         0x{flags:04X}, tail {tail_text}",
                        leaf(doc.name),
                        record.payload.len(),
                    );
                    println!("      slots: {}", line.join("  "));
                    if !orphans.is_empty() {
                        println!(
                            "      members with no slot behind them: {}",
                            orphans.join(", ")
                        );
                    }
                }
            }
        }
    }
    println!(
        "  {filled} filled slots across the corpus, {with_member} of them recorded as a tag-{TAG} \
         member"
    );
    member_arithmetic(docs, &member_count);
}

/// How many members one slot is worth.
///
/// The per-record lines above show the same number twice over: a geometry
/// slot at `+92` or `+202` produces *two* members on its target, a `+280`
/// reference produces one. Stating it as arithmetic is what makes a lone
/// member visible as an edge with no slot behind it, so it gets counted
/// rather than eyeballed.
fn member_arithmetic(docs: &[Doc], member_count: &BTreeMap<(&str, &str, u32, u32), usize>) {
    let mut agree = 0usize;
    let mut disagree = Vec::new();
    for ((fixture, storage, referrer, target), members) in member_count {
        let Some(record) = docs
            .iter()
            .find(|doc| doc.name == *fixture)
            .and_then(|doc| doc.scopes.get(*storage))
            .and_then(|scope| scope.records.get(referrer))
            .and_then(|records| records.first())
        else {
            continue;
        };
        let at = |offset: usize| u32_at(&record.payload, offset) == Some(*target);
        let blocks = usize::from(at(92)) + usize::from(at(202));
        let tail = usize::from(at(280));
        let expected = 2 * blocks + tail;
        if expected == *members {
            agree += 1;
        } else {
            disagree.push(format!(
                "    {}{storage} JDim {referrer} -> {target}: {members} members, \
                 {blocks} block slots + {tail} tail slots predicts {expected}",
                leaf(fixture)
            ));
        }
    }
    println!("  a +92 or +202 slot is worth two members, a +280 reference one:");
    println!("    {agree} pairs agree, {} do not", disagree.len());
    for line in &disagree {
        println!("{line}");
    }
}

/// The reading the misses suggest, put as a question the corpus can answer
/// no to: is every slotless member's target a geometry that *another*
/// dimension of the same `JDimGroup` measures in its own payload?
///
/// If it is, the map is not inventing an edge and the dimension is not
/// hiding one: a dimension in a group is associated with the whole group's
/// reference geometry, and only the association it measures itself gets a
/// slot.
fn orphans_are_the_group(docs: &[Doc], landings: &[Landing]) {
    println!("\n=== 7. is every slotless member the group's shared geometry? ===");
    let mut held = 0usize;
    let mut refuted = 0usize;
    for landing in landings {
        if !landing.slot_hits().is_empty() {
            continue;
        }
        let Some(scope) = docs
            .iter()
            .find(|doc| doc.name == landing.fixture)
            .and_then(|doc| doc.scopes.get(&landing.storage))
        else {
            continue;
        };
        let group = scope
            .records
            .get(&landing.jdim_oid)
            .and_then(|records| records.first())
            .and_then(group_of);
        // Every other dimension of the same group, and the slot (if any) in
        // which it measures this member's target.
        let siblings: Vec<String> = scope
            .records
            .iter()
            .filter(|(oid, _)| **oid != landing.jdim_oid)
            .flat_map(|(oid, records)| records.iter().map(move |record| (oid, record)))
            .filter(|(_, record)| record.type_code == JDIM && group_of(record) == group)
            .flat_map(|(oid, record)| {
                NAMED_SLOTS
                    .iter()
                    .filter(move |(at, _)| u32_at(&record.payload, *at) == Some(landing.entry_id))
                    .map(move |(at, role)| format!("JDim {oid} measures it at +{at} ({role})"))
            })
            .collect();
        if siblings.is_empty() {
            refuted += 1;
        } else {
            held += 1;
        }
        println!(
            "  {}{} entry {} <- JDim {} (group {}): {}",
            leaf(landing.fixture),
            landing.storage,
            landing.entry_id,
            landing.jdim_oid,
            group.map_or_else(|| "none".to_string(), |group| group.to_string()),
            if siblings.is_empty() {
                "NO sibling of this group measures it".to_string()
            } else {
                siblings.join("; ")
            }
        );
        // The other place the reference could live. It does not: these
        // records are the 18-byte envelope and a list of dimensions.
        if let Some(record) = group
            .and_then(|group| scope.records.get(&group))
            .and_then(|records| records.first())
        {
            let words: Vec<String> = (0..record.payload.len() / 4)
                .filter_map(|word| u32_at(&record.payload, word * 4))
                .map(|word| word.to_string())
                .collect();
            println!(
                "      the group record is {} bytes, every word of it: {}",
                record.payload.len(),
                words.join(" ")
            );
        }
    }
    println!("  holds on {held}, refuted on {refuted}");
    group_membership_is_mutual(docs);
}

/// The closing dword's reading, checked from both ends.
///
/// `group_of` says a dimension names its group; if that is right, the group
/// lists exactly the dimensions that name it. Nothing forces the two to
/// agree, so agreement is evidence and disagreement would retract the
/// reading.
fn group_membership_is_mutual(docs: &[Doc]) {
    println!("\n  the closing dword, checked from the group's side:");
    let mut mutual = 0usize;
    let mut broken = 0usize;
    for doc in docs {
        for (storage, scope) in &doc.scopes {
            let mut by_group: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
            for (oid, records) in &scope.records {
                for record in records.iter().filter(|record| record.type_code == JDIM) {
                    if let Some(group) = group_of(record) {
                        by_group.entry(group).or_default().insert(*oid);
                    }
                }
            }
            for (group, dimensions) in &by_group {
                let listed = scope
                    .records
                    .get(group)
                    .and_then(|records| records.first())
                    .filter(|record| record.type_code == JDIM_GROUP)
                    .map(group_members)
                    .unwrap_or_default();
                if listed == *dimensions {
                    mutual += 1;
                } else {
                    broken += 1;
                }
                println!(
                    "    {}{storage} group {group}: named by {dimensions:?}, lists {listed:?} {}",
                    leaf(doc.name),
                    if listed == *dimensions {
                        "-- mutual"
                    } else {
                        "-- DISAGREE"
                    }
                );
            }
        }
    }
    println!("    {mutual} groups agree, {broken} do not");
}

/// The tags under which `entry` lists `referrer` as one of its members.
///
/// The space map is an incoming-reference index (2026-08-27 §3): if a
/// record really points at an object, the object's entry carries a member
/// whose `value` is that record, under the tag of the edge. So "which tags"
/// is the map's own answer to "is this a reference".
fn tags_naming(scope: &Scope, entry: u32, referrer: u32) -> Vec<u16> {
    scope
        .entries
        .iter()
        .find(|candidate| candidate.id == entry)
        .map(|candidate| {
            candidate
                .members
                .iter()
                .filter(|(value, _)| *value == referrer)
                .map(|(_, tag)| *tag)
                .collect()
        })
        .unwrap_or_default()
}

fn tag_list(tags: &[u16]) -> String {
    if tags.is_empty() {
        "nothing".to_string()
    } else {
        tags.iter()
            .map(|tag| format!("t{tag}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Is `+140` a reference at all?
///
/// The 2026-09-14 reading (§5) called `+140` the owner slot because 14 of
/// its 18 values resolve to *some* record. Section 6 already shows the map
/// never recorded a tag-188 member behind it. This asks the wider question:
/// does the map record the dimension as a referrer of that object under
/// *any* tag -- the way it does for `+92` under tag 188 -- and what does
/// the raw value look like across storages? A real oid is drawn from the
/// storage's own index space and varies with it; a constant that happens
/// to be small resolves to a different class in every file. The closing
/// dword goes through the same test for contrast.
fn is_plus_140_a_reference(docs: &[Doc]) {
    println!("\n=== 8. is +140 a reference at all? the map's answer, under any tag ===");
    let mut values: BTreeMap<u32, usize> = BTreeMap::new();
    let mut total = 0usize;
    let mut resolved = 0usize;
    let mut backed = 0usize;
    let mut tail_total = 0usize;
    let mut tail_backed: BTreeMap<u16, usize> = BTreeMap::new();
    for doc in docs {
        for (storage, scope) in &doc.scopes {
            for (oid, records) in &scope.records {
                for record in records.iter().filter(|record| record.type_code == JDIM) {
                    let main_len = u32_at(&record.payload, 30).unwrap_or_default() as usize;
                    for at in [140usize, 250] {
                        if at + 6 > 34 + main_len {
                            continue;
                        }
                        let Some(target) = u32_at(&record.payload, at) else {
                            continue;
                        };
                        total += 1;
                        *values.entry(target).or_default() += 1;
                        let class = scope
                            .records
                            .get(&target)
                            .and_then(|records| records.first())
                            .map(|record| record.type_code);
                        if class.is_some() {
                            resolved += 1;
                        }
                        let tags = tags_naming(scope, target, *oid);
                        if !tags.is_empty() {
                            backed += 1;
                        }
                        println!(
                            "  {}{storage} JDim {oid:<6} +{at:<4} = {target:<4} {:<30} the map lists \
                             this JDim on that entry under: {}",
                            leaf(doc.name),
                            class.map_or_else(|| "no record".to_string(), class_label),
                            tag_list(&tags)
                        );
                    }
                    if let Some(group) = group_of(record) {
                        tail_total += 1;
                        let tags = tags_naming(scope, group, *oid);
                        for tag in &tags {
                            *tail_backed.entry(*tag).or_default() += 1;
                        }
                        println!(
                            "  {}{storage} JDim {oid:<6} tail = {group:<4} {:<30} the map lists \
                             this JDim on that entry under: {}",
                            leaf(doc.name),
                            scope
                                .records
                                .get(&group)
                                .and_then(|records| records.first())
                                .map_or_else(
                                    || "no record".to_string(),
                                    |record| class_label(record.type_code)
                                ),
                            tag_list(&tags)
                        );
                    }
                }
            }
        }
    }
    let histogram: Vec<String> = values
        .iter()
        .map(|(value, count)| format!("{value} x{count}"))
        .collect();
    println!(
        "  +140 / +250: {total} values inside the main area, {resolved} resolve to some record, \
         the map backs {backed} of them as an edge from the dimension under any tag"
    );
    println!(
        "  raw values across every storage: {}",
        histogram.join(", ")
    );
    let tail_summary: Vec<String> = tail_backed
        .iter()
        .map(|(tag, count)| format!("t{tag} x{count}"))
        .collect();
    println!(
        "  closing dword: {tail_total} records carry one; the group's entry lists the dimension \
         under {}",
        if tail_summary.is_empty() {
            "nothing".to_string()
        } else {
            tail_summary.join(", ")
        }
    );
}

fn main() {
    let docs: Vec<Doc> = FIXTURES
        .iter()
        .filter_map(|fixture| load(fixture))
        .collect();
    if docs.is_empty() {
        println!("no fixture available");
        return;
    }
    let landings: Vec<Landing> = docs.iter().flat_map(collect_landings).collect();

    table(&landings);
    account(&landings);
    slot_vs_class(&landings);
    aliases(&landings, &docs);
    misses(&landings, &docs);
    slots_without_members(&docs, &landings);
    orphans_are_the_group(&docs, &landings);
    is_plus_140_a_reference(&docs);

    println!("\n=== 9. cross-check: the export fixture, outside the 46 ===");
    match load(CROSS_CHECK) {
        Some(doc) => {
            let extra = collect_landings(&doc);
            let only = [doc];
            table(&extra);
            account(&extra);
            slot_vs_class(&extra);
            misses(&extra, &only);
            slots_without_members(&only, &extra);
            orphans_are_the_group(&only, &extra);
            is_plus_140_a_reference(&only);
        }
        None => println!("  absent"),
    }
}
