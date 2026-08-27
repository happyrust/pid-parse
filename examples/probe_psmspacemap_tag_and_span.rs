//! `PSMspacemap` walks clean, but two of its fields are still unread: the
//! `u16` this crate calls `span`, and the `u16` tag on every member.
//!
//! What the previous round left standing:
//!
//! * 1948 entries, 4446 members, tags drawn from a fixed 14-value set
//!   `{0, 181, 182, 183, 184, 185, 188, 190, 201, 205, 225, 239, 249, 261}`.
//! * `span` equals the entry's member count on 1631 of the 1948 entries, so it
//!   is not a copy of the count -- but nobody has said what the other 317 are.
//! * `sub_5647AB70`, the entry reader these files are *not* in, writes two
//!   members of its own with the tags `181` and `182` hardcoded. That is what
//!   rules out "tag is a length" for those two, and it is the only thing anyone
//!   has on tags so far.
//!
//! This probe asks the corpus rather than the disassembly. Four questions, each
//! with a shape of answer that would settle it:
//!
//! 1. **Is `span` the count of something narrower than "members"?** If it is
//!    the members that carry a nonzero tag, then `span == count - zeros` holds
//!    on all 1948 and the 317 "mismatches" were never mismatches.
//! 2. **Does the tag depend on the entry rather than the member?** A tag that
//!    is a per-object property (a class, a size) would show up as a small set
//!    of whole-entry tag shapes. A tag that names a slot within the object
//!    would show up as a fixed order.
//! 3. **Is `value` an address or a reference?** A reference resolves: it is a
//!    persist id an entry in the same container actually defines. An address
//!    does not, and is close to unique.
//! 4. **Do all four documents draw on the same tag set?** A per-document set
//!    would mean the tag is allocated by the file (a cluster id, a table row).
//!    One shared set means it is baked into the reader.
//! 5. **Does the tag belong to the target or to the edge?** This is the test
//!    that separates the two readings, and it needs nothing outside the map:
//!    take every persist id that more than one member points at, and ask
//!    whether all those members agree on a tag. If they do, the tag says what
//!    the target *is*. If they disagree, the tag says what the reference is
//!    *for*.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// `'tseg'`, the magic every segment stream in this corpus opens with.
const SEGMENT_MAGIC: u32 = 0x6765_7374;
/// `'sseg'`, the legacy form `Segment::Load` also accepts.
const SEGMENT_MAGIC_LEGACY: u32 = 0x6765_7373;
/// Bytes a segment addresses, and the shift its stream name is built with.
const SEGMENT_SPAN: u32 = 0x2000;
const SEGMENT_SHIFT: u32 = 13;
/// The bit `sub_5647A900` refuses an entry without. It is what ends the walk.
const ENTRY_LIVE: u32 = 0x0002_0000;
/// The bit that makes the reader add two to the span it stores.
const ENTRY_EXTENDED: u32 = 0x0001_0000;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

struct Entry {
    offset: usize,
    head_high: u16,
    index: u16,
    span: u16,
    members: Vec<(u32, u16)>,
    len: usize,
}

impl Entry {
    fn zero_tags(&self) -> usize {
        self.members.iter().filter(|(_, tag)| *tag == 0).count()
    }

    fn tag_shape(&self) -> Vec<u16> {
        self.members.iter().map(|(_, tag)| *tag).collect()
    }
}

fn read_entry(data: &[u8], at: usize) -> Option<Entry> {
    let head = u32_at(data, at)?;
    if head & ENTRY_LIVE == 0 {
        return None;
    }
    let span = u16_at(data, at + 4)?;
    let count = usize::from(u16_at(data, at + 6)?);
    let len = 8 + 6 * count;
    if at + len > data.len() {
        return None;
    }
    let members = (0..count)
        .filter_map(|i| {
            let base = at + 8 + 6 * i;
            Some((u32_at(data, base)?, u16_at(data, base + 4)?))
        })
        .collect();
    Some(Entry {
        offset: at,
        head_high: (head >> 16) as u16,
        index: (head & 0xFFFF) as u16,
        span,
        members,
        len,
    })
}

/// One `PSMspacemap` member stream, walked.
struct Segment {
    fixture: &'static str,
    stream: String,
    segment: u32,
    /// `m_iNext`: one past the highest index the segment has ever handed out.
    next_free_index: u16,
    /// Indices that were handed out and given back. The reader refills from
    /// here, so an index in this list names no live object.
    free_list: BTreeSet<u16>,
    entries: Vec<Entry>,
}

fn read_segment(fixture: &'static str, stream: &str, data: &[u8], segment: u32) -> Option<Segment> {
    let magic = u32_at(data, 0)?;
    if magic != SEGMENT_MAGIC && magic != SEGMENT_MAGIC_LEGACY {
        return None;
    }
    let next_free_index = u16_at(data, 8)?;
    let free_len = usize::from(u16_at(data, 10)?);
    let free_list: BTreeSet<u16> = (0..free_len)
        .filter_map(|i| u16_at(data, 12 + 2 * i))
        .collect();
    let mut at = 12 + 2 * free_len;
    let mut entries = Vec::new();
    while let Some(entry) = read_entry(data, at) {
        at += entry.len;
        entries.push(entry);
    }
    Some(Segment {
        fixture,
        stream: stream.to_string(),
        segment,
        next_free_index,
        free_list,
        entries,
    })
}

/// The index space a persist id belongs to: a document, and the storage the
/// `PSMspacemap` hangs under inside it. The top-level map and each `JSite`
/// registry's map number their objects independently, and so does every
/// document -- four of these files have a `/PSMspacemap` and they mean four
/// different things. A reference only resolves inside its own scope.
type Scope = (&'static str, String);

/// A stream named so the reader can tell the four documents apart.
fn label(segment: &Segment) -> String {
    let leaf = segment
        .fixture
        .rsplit('/')
        .next()
        .unwrap_or(segment.fixture);
    format!("{leaf}{}", segment.stream)
}

fn scope_of(segment: &Segment) -> Scope {
    let container = match segment.stream.rfind("PSMspacemap") {
        Some(at) => segment.stream[..at].to_string(),
        None => segment.stream.clone(),
    };
    (segment.fixture, container)
}

/// The segment number a member stream's name states.
fn segment_of(stream: &str) -> Option<u32> {
    let leaf = stream.rsplit(['/', '\\']).next()?;
    let value = u32::from_str_radix(leaf.strip_prefix("0x")?, 16).ok()?;
    (value % SEGMENT_SPAN == 0).then_some(value >> SEGMENT_SHIFT)
}

fn spacemap_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|p| p.contains("PSMspacemap"))
        .collect();
    let mut out = Vec::new();
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

/// Per-tag accounting of what the `u32` beside the tag looks like.
#[derive(Default)]
struct ValueShape {
    count: usize,
    zero: usize,
    distinct: BTreeSet<u32>,
    min: u32,
    max: u32,
    below_segment_span: usize,
    resolves_in_container: usize,
    resolves_in_file: usize,
    equals_own_persist_id: usize,
    /// `head_hi` of the entries this tag's resolved references land on.
    target_head_high: BTreeMap<u16, usize>,
}

impl ValueShape {
    fn observe(&mut self, value: u32, resolves: bool, resolves_in_file: bool, own: bool) {
        if self.count == 0 {
            self.min = value;
            self.max = value;
        }
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.distinct.insert(value);
        if value == 0 {
            self.zero += 1;
        }
        if value < SEGMENT_SPAN {
            self.below_segment_span += 1;
        }
        if resolves {
            self.resolves_in_container += 1;
        }
        if resolves_in_file {
            self.resolves_in_file += 1;
        }
        if own {
            self.equals_own_persist_id += 1;
        }
    }
}

fn main() {
    let mut segments: Vec<Segment> = Vec::new();
    let mut per_fixture_tags: BTreeMap<&str, BTreeSet<u16>> = BTreeMap::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        for (stream, data) in spacemap_streams(path) {
            let Some(segment) = segment_of(&stream) else {
                continue;
            };
            let Some(read) = read_segment(fixture, &stream, &data, segment) else {
                continue;
            };
            let tags = per_fixture_tags.entry(fixture).or_default();
            for entry in &read.entries {
                for (_, tag) in &entry.members {
                    tags.insert(*tag);
                }
            }
            segments.push(read);
        }
    }

    // Every persist id the corpus defines, grouped by the container that owns
    // the index space. A member value only counts as a reference if it lands
    // on an id its own container defines.
    let mut ids_by_scope: BTreeMap<Scope, BTreeSet<u32>> = BTreeMap::new();
    let mut ids_by_file: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for segment in &segments {
        let id_of = |entry: &Entry| (segment.segment << SEGMENT_SHIFT) | u32::from(entry.index);
        let scope = ids_by_scope.entry(scope_of(segment)).or_default();
        for entry in &segment.entries {
            scope.insert(id_of(entry));
        }
        let file = ids_by_file.entry(segment.fixture).or_default();
        for entry in &segment.entries {
            file.insert(id_of(entry));
        }
    }

    span_report(&segments);
    tag_shape_report(&segments);
    value_report(&segments, &ids_by_scope, &ids_by_file);
    println!("\n=== 4. is the tag set per document or baked into the reader? ===");
    for (fixture, tags) in &per_fixture_tags {
        let leaf = fixture.rsplit('/').next().unwrap_or(fixture);
        println!("  {leaf}: {tags:?}");
    }
    target_agreement_report(&segments);
    liveness_report(&segments);
    jsite_tag_report(&segments);
    raw_dump(&segments);
}

/// Question 8: name one of the classes.
///
/// A `JSite` storage is called `JSite` followed by a number, and that number
/// is a persist id in the document's top-level index space -- the same space
/// the top-level `PSMspacemap` numbers its entries in. So the corpus hands us
/// seven objects whose class we already know, across four unrelated documents.
/// If the tag is the target's class, every reference to those seven carries
/// one tag, and no reference to anything else carries it.
fn jsite_tag_report(segments: &[Segment]) {
    println!("\n=== 8. the tag on the objects whose class we already know ===");
    let mut jsite_ids: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for segment in segments {
        for part in segment.stream.split(['/', '\\']) {
            if let Some(id) = part
                .strip_prefix("JSite")
                .and_then(|n| n.parse::<u32>().ok())
            {
                jsite_ids.entry(segment.fixture).or_default().insert(id);
            }
        }
    }

    let mut tags_on_jsites: BTreeMap<u16, usize> = BTreeMap::new();
    let mut tag_used_elsewhere: BTreeMap<u16, usize> = BTreeMap::new();
    let mut own_entries: Vec<String> = Vec::new();

    for segment in segments {
        let (fixture, container) = scope_of(segment);
        // Only the top-level map numbers `JSite` ids, but any of its segments
        // may hold an entry that references one, so all of them are searched.
        let top_level = container == "/";
        let sites = jsite_ids.get(fixture).cloned().unwrap_or_default();
        for entry in &segment.entries {
            let own = (segment.segment << SEGMENT_SHIFT) | u32::from(entry.index);
            if top_level && sites.contains(&own) {
                own_entries.push(format!(
                    "{} JSite{}: head_hi {}, span {}, members {:?}",
                    fixture.rsplit('/').next().unwrap_or(fixture),
                    own,
                    entry.head_high,
                    entry.span,
                    entry.members
                ));
            }
            for (value, tag) in &entry.members {
                if *tag == 0 {
                    continue;
                }
                if top_level && sites.contains(value) {
                    *tags_on_jsites.entry(*tag).or_default() += 1;
                } else {
                    *tag_used_elsewhere.entry(*tag).or_default() += 1;
                }
            }
        }
    }

    for (fixture, sites) in &jsite_ids {
        let leaf = fixture.rsplit('/').next().unwrap_or(fixture);
        println!("  {leaf}: JSite ids {sites:?}");
    }
    println!("  tags carried by references to a JSite id: {tags_on_jsites:?}");
    for tag in tags_on_jsites.keys() {
        println!(
            "  tag {tag} also appears on something that is not a JSite {} times",
            tag_used_elsewhere.get(tag).copied().unwrap_or_default()
        );
    }
    println!(
        "  JSite ids that carry an entry of their own: {}",
        own_entries.len()
    );
    for line in own_entries.iter().take(8) {
        println!("     {line}");
    }
    hub_report(segments);
}

/// The objects the most references point at, with the tag they all carry.
/// If the tag is a class, the hubs are the document's few registry objects and
/// each keeps one tag no matter how many thousands of references land on it.
fn hub_report(segments: &[Segment]) {
    println!("\n=== 9. what the references point at, most-pointed-at first ===");
    let mut in_degree: BTreeMap<Scope, BTreeMap<u32, (u16, usize)>> = BTreeMap::new();
    let mut has_entry: BTreeMap<Scope, BTreeSet<u32>> = BTreeMap::new();
    for segment in segments {
        let scope = scope_of(segment);
        for entry in &segment.entries {
            has_entry
                .entry(scope.clone())
                .or_default()
                .insert((segment.segment << SEGMENT_SHIFT) | u32::from(entry.index));
            for (value, tag) in &entry.members {
                if *tag == 0 {
                    continue;
                }
                let slot = in_degree
                    .entry(scope.clone())
                    .or_default()
                    .entry(*value)
                    .or_insert((*tag, 0));
                slot.1 += 1;
            }
        }
    }

    for ((fixture, container), targets) in &in_degree {
        if targets.len() < 40 {
            continue;
        }
        let leaf = fixture.rsplit('/').next().unwrap_or(fixture);
        let entries = has_entry
            .get(&(fixture, container.clone()))
            .map(BTreeSet::len)
            .unwrap_or_default();
        let with_entry = targets
            .keys()
            .filter(|id| {
                has_entry
                    .get(&(fixture, container.clone()))
                    .is_some_and(|set| set.contains(id))
            })
            .count();
        println!(
            "  {leaf}{container}: {} objects are pointed at, {with_entry} of them carry an entry \
             (the map has {entries})",
            targets.len()
        );
        let mut ranked: Vec<_> = targets.iter().collect();
        ranked.sort_by_key(|(id, (_, hits))| (std::cmp::Reverse(*hits), **id));
        let top: Vec<String> = ranked
            .iter()
            .take(6)
            .map(|(id, (tag, hits))| format!("id {id} (tag {tag}) x{hits}"))
            .collect();
        println!("     {}", top.join(", "));
    }
}

/// Question 6: does every member value name a *live* object?
///
/// The header of each segment stream states `m_iNext` and lists the indices
/// that have been handed back. Between them they partition the index space
/// into three: never allocated (`>= m_iNext`), allocated and freed (on the
/// list), and live. Most of these segments are mostly free -- so if the
/// member values were anything but object references, four fifths of them
/// would land in the freed range. Whether they do is the test.
fn liveness_report(segments: &[Segment]) {
    println!("\n=== 6. is every value a live object id? ===");
    let mut by_scope: BTreeMap<Scope, BTreeMap<u32, &Segment>> = BTreeMap::new();
    for segment in segments {
        by_scope
            .entry(scope_of(segment))
            .or_default()
            .insert(segment.segment, segment);
    }

    // Per tag: live, freed, past `m_iNext`.
    let mut verdicts: BTreeMap<u16, (usize, usize, usize)> = BTreeMap::new();
    let mut over_the_top: Vec<String> = Vec::new();
    let mut dangling: Vec<String> = Vec::new();

    for segment in segments {
        let Some(peers) = by_scope.get(&scope_of(segment)) else {
            continue;
        };
        for entry in &segment.entries {
            for (value, tag) in &entry.members {
                if *tag == 0 {
                    continue;
                }
                let verdict = verdicts.entry(*tag).or_default();
                let target_segment = value >> SEGMENT_SHIFT;
                let index = (value & (SEGMENT_SPAN - 1)) as u16;
                let peer = peers.get(&target_segment);
                match peer {
                    Some(peer) if index < peer.next_free_index => {
                        if peer.free_list.contains(&index) {
                            verdict.1 += 1;
                            dangling.push(format!(
                                "{} entry {} -> {value} (tag {tag}) is on segment {target_segment}'s free list",
                                label(segment),
                                entry.index
                            ));
                        } else {
                            verdict.0 += 1;
                        }
                    }
                    _ => {
                        verdict.2 += 1;
                        if over_the_top.len() < 8 && *value >= SEGMENT_SPAN {
                            let ceiling = peer.map(|peer| peer.next_free_index);
                            over_the_top.push(format!(
                                "{} entry {} -> {value} = segment {target_segment} index {index}, \
                                 that segment's m_iNext is {ceiling:?}",
                                label(segment),
                                entry.index
                            ));
                        }
                    }
                }
            }
        }
    }

    let (freed_indices, allocated_indices) = segments.iter().fold((0, 0), |acc, segment| {
        (
            acc.0 + segment.free_list.len(),
            acc.1 + usize::from(segment.next_free_index),
        )
    });
    println!(
        "  {} of the {} indices these segments ever handed out are on a free list -- so {:.0}% \
         of a random id would read as dead",
        freed_indices,
        allocated_indices,
        100.0 * freed_indices as f64 / allocated_indices.max(1) as f64,
    );
    println!(
        "  {:>5} {:>7} {:>7} {:>13}  share live",
        "tag", "live", "freed", "past m_iNext"
    );
    for (tag, (live, freed, past)) in &verdicts {
        let total = live + freed + past;
        println!(
            "  {:>5} {:>7} {:>7} {:>13}  {:.0}%",
            tag,
            live,
            freed,
            past,
            100.0 * *live as f64 / total.max(1) as f64
        );
    }
    for line in &over_the_top {
        println!("     {line}");
    }
    for line in dangling.iter().take(8) {
        println!("     {line}");
    }
    free_list_sanity(segments);
}

/// The liveness test above only means something if the free list really is a
/// list of dead indices. An index that is on the list *and* carries an entry
/// would say it is not.
fn free_list_sanity(segments: &[Segment]) {
    println!("\n=== 7. is the free list a list of dead indices? ===");
    for segment in segments {
        if segment.free_list.is_empty() && segment.entries.is_empty() {
            continue;
        }
        let indices: BTreeSet<u16> = segment.entries.iter().map(|entry| entry.index).collect();
        let both = indices.intersection(&segment.free_list).count();
        let above_next = segment
            .free_list
            .iter()
            .filter(|index| **index >= segment.next_free_index)
            .count();
        println!(
            "  {}: m_iNext {}, {} free, {} entries -- {} indices are on the free list AND carry \
             an entry; {} free entries are at or above m_iNext",
            label(segment),
            segment.next_free_index,
            segment.free_list.len(),
            segment.entries.len(),
            both,
            above_next,
        );
    }
}

/// Question 1: what does `span` count?
fn span_report(segments: &[Segment]) {
    println!("=== 1. span ===");
    let mut delta_by_head: BTreeMap<u16, BTreeMap<i64, usize>> = BTreeMap::new();
    let mut span_is_count = 0usize;
    let mut span_is_nonzero_tags = 0usize;
    let mut span_is_count_plus_two = 0usize;
    let mut total = 0usize;
    let mut zero_tag_is_zero_value = 0usize;
    let mut zero_members = 0usize;
    let mut entries_with_a_hole = 0usize;
    let mut counterexamples: Vec<String> = Vec::new();

    for segment in segments {
        for entry in &segment.entries {
            total += 1;
            let count = entry.members.len();
            let nonzero = count - entry.zero_tags();
            let delta = i64::from(entry.span) - count as i64;
            *delta_by_head
                .entry(entry.head_high)
                .or_default()
                .entry(delta)
                .or_default() += 1;
            if usize::from(entry.span) == count {
                span_is_count += 1;
            }
            if usize::from(entry.span) == nonzero {
                span_is_nonzero_tags += 1;
            } else if counterexamples.len() < 8 {
                counterexamples.push(format!(
                    "{} +{:#x}: head_hi {}, span {}, {} members, {} nonzero, tags {:?}",
                    label(segment),
                    entry.offset,
                    entry.head_high,
                    entry.span,
                    count,
                    nonzero,
                    entry.tag_shape()
                ));
            }
            if usize::from(entry.span) == count + 2 {
                span_is_count_plus_two += 1;
            }
            zero_members += entry.zero_tags();
            zero_tag_is_zero_value += entry
                .members
                .iter()
                .filter(|(value, tag)| (*tag == 0) == (*value == 0))
                .count();
            // A hole is an empty slot with a live one after it: it says the
            // member array is addressed by position, not compacted on delete.
            let last_live = entry.members.iter().rposition(|(_, tag)| *tag != 0);
            if let Some(last_live) = last_live {
                if entry.members[..last_live].iter().any(|(_, tag)| *tag == 0) {
                    entries_with_a_hole += 1;
                }
            }
        }
    }

    println!("  entries: {total}");
    println!(
        "  members whose tag and value are zero together: {zero_tag_is_zero_value} of {}",
        segments
            .iter()
            .flat_map(|segment| segment.entries.iter())
            .map(|entry| entry.members.len())
            .sum::<usize>()
    );
    println!("  empty member slots: {zero_members}");
    println!("  entries with an empty slot before a live one: {entries_with_a_hole}");
    println!("  span == member count                      : {span_is_count}");
    println!("  span == members whose tag is nonzero       : {span_is_nonzero_tags}");
    println!("  span == member count + 2                   : {span_is_count_plus_two}");
    for (head_high, deltas) in &delta_by_head {
        println!("  head_hi {head_high}: span - count histogram {deltas:?}");
    }
    if span_is_nonzero_tags == total {
        println!("  -> span counts the members that carry a tag. No counterexample.");
    } else {
        println!(
            "  -> {} counterexamples to 'span counts tagged members':",
            total - span_is_nonzero_tags
        );
        for line in &counterexamples {
            println!("     {line}");
        }
    }
}

/// Question 2: is the tag a property of the entry, or a slot inside it?
fn tag_shape_report(segments: &[Segment]) {
    println!("\n=== 2. tag ===");
    let mut shape_census: BTreeMap<(u16, Vec<u16>), usize> = BTreeMap::new();
    let mut first_tag: BTreeMap<u16, usize> = BTreeMap::new();
    let mut tag_by_position: BTreeMap<usize, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut tag_by_head: BTreeMap<u16, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut nondecreasing = 0usize;
    let mut entries_with_repeat_tag = 0usize;
    let mut total = 0usize;

    for segment in segments {
        for entry in &segment.entries {
            total += 1;
            let shape = entry.tag_shape();
            *shape_census
                .entry((entry.head_high, shape.clone()))
                .or_default() += 1;
            if let Some(first) = shape.first() {
                *first_tag.entry(*first).or_default() += 1;
            }
            for (position, tag) in shape.iter().enumerate() {
                *tag_by_position
                    .entry(position.min(4))
                    .or_default()
                    .entry(*tag)
                    .or_default() += 1;
                *tag_by_head
                    .entry(entry.head_high)
                    .or_default()
                    .entry(*tag)
                    .or_default() += 1;
            }
            if shape.windows(2).all(|w| w[0] <= w[1]) {
                nondecreasing += 1;
            }
            let distinct: BTreeSet<u16> = shape.iter().copied().collect();
            if distinct.len() != shape.len() {
                entries_with_repeat_tag += 1;
            }
        }
    }

    println!("  entries whose tags never decrease left to right: {nondecreasing} / {total}");
    println!("  entries that use one tag twice                 : {entries_with_repeat_tag}");
    println!("  tag of the first member: {first_tag:?}");
    for (position, census) in &tag_by_position {
        println!("  tags at member position {position}{}: {census:?}", {
            if *position == 4 {
                "+"
            } else {
                ""
            }
        });
    }
    for (head_high, census) in &tag_by_head {
        println!("  tags under head_hi {head_high}: {census:?}");
    }

    let mut shapes: Vec<_> = shape_census.iter().collect();
    shapes.sort_by(|a, b| b.1.cmp(a.1));
    println!(
        "  whole-entry tag shapes, most common first ({} distinct):",
        shapes.len()
    );
    for ((head_high, shape), hits) in shapes.iter().take(18) {
        println!("    {hits:5} x head_hi {head_high} {shape:?}");
    }
}

/// Question 3: is the `u32` beside the tag an address or a reference?
fn value_report(
    segments: &[Segment],
    ids_by_scope: &BTreeMap<Scope, BTreeSet<u32>>,
    ids_by_file: &BTreeMap<&str, BTreeSet<u32>>,
) {
    println!("\n=== 3. value ===");
    let mut by_tag: BTreeMap<u16, ValueShape> = BTreeMap::new();
    let mut global_pairs: BTreeMap<(u16, u32), usize> = BTreeMap::new();
    let head_high_by_id = head_high_by_id(segments);

    for segment in segments {
        let scope = scope_of(segment);
        let known = ids_by_scope.get(&scope);
        let in_file = ids_by_file.get(segment.fixture);
        for entry in &segment.entries {
            let own = (segment.segment << SEGMENT_SHIFT) | u32::from(entry.index);
            for (value, tag) in &entry.members {
                let resolves = known.is_some_and(|set| set.contains(value));
                let shape = by_tag.entry(*tag).or_default();
                shape.observe(
                    *value,
                    resolves,
                    in_file.is_some_and(|set| set.contains(value)),
                    *value == own,
                );
                if let Some(head_high) = head_high_by_id.get(&(scope.clone(), *value)) {
                    *shape.target_head_high.entry(*head_high).or_default() += 1;
                }
                *global_pairs.entry((*tag, *value)).or_default() += 1;
            }
        }
    }

    println!(
        "  {:>5} {:>6} {:>8} {:>6} {:>7} {:>7} {:>9} {:>11} {:>10} {:>5}  target head_hi",
        "tag",
        "count",
        "distinct",
        "zero",
        "min",
        "max",
        "<0x2000",
        "in own map",
        "in my file",
        "self"
    );
    for (tag, shape) in &by_tag {
        println!(
            "  {:>5} {:>6} {:>8} {:>6} {:>7} {:>7} {:>9} {:>11} {:>10} {:>5}  {:?}",
            tag,
            shape.count,
            shape.distinct.len(),
            shape.zero,
            shape.min,
            shape.max,
            shape.below_segment_span,
            shape.resolves_in_container,
            shape.resolves_in_file,
            shape.equals_own_persist_id,
            shape.target_head_high
        );
    }

    let repeated = global_pairs.values().filter(|hits| **hits > 1).count();
    println!(
        "  distinct (tag, value) pairs: {} of {} members; {repeated} pairs occur more than once",
        global_pairs.len(),
        global_pairs.values().sum::<usize>()
    );
}

/// `head_hi` of every entry, keyed the way a reference has to be resolved:
/// by container, then by persist id.
fn head_high_by_id(segments: &[Segment]) -> BTreeMap<(Scope, u32), u16> {
    let mut out = BTreeMap::new();
    for segment in segments {
        let scope = scope_of(segment);
        for entry in &segment.entries {
            let id = (segment.segment << SEGMENT_SHIFT) | u32::from(entry.index);
            out.insert((scope.clone(), id), entry.head_high);
        }
    }
    out
}

/// Question 5: when several members point at the same object, do they agree
/// on a tag?
///
/// Agreement means the tag is a property of the target -- its class, its size,
/// anything intrinsic. Disagreement means the tag belongs to the reference,
/// and the same object can be pointed at two different ways.
fn target_agreement_report(segments: &[Segment]) {
    println!("\n=== 5. does the tag belong to the target or to the edge? ===");
    let mut tags_per_target: BTreeMap<(Scope, u32), BTreeMap<u16, usize>> = BTreeMap::new();
    for segment in segments {
        let scope = scope_of(segment);
        for entry in &segment.entries {
            for (value, tag) in &entry.members {
                if *tag == 0 {
                    continue;
                }
                *tags_per_target
                    .entry((scope.clone(), *value))
                    .or_default()
                    .entry(*tag)
                    .or_default() += 1;
            }
        }
    }

    let mut distinct_tag_histogram: BTreeMap<usize, usize> = BTreeMap::new();
    let mut multi_reference_targets = 0usize;
    let mut disagreeing = Vec::new();
    for (((fixture, container), value), tags) in &tags_per_target {
        let references: usize = tags.values().sum();
        if references > 1 {
            multi_reference_targets += 1;
        }
        *distinct_tag_histogram.entry(tags.len()).or_default() += 1;
        if tags.len() > 1 && disagreeing.len() < 10 {
            let leaf = fixture.rsplit('/').next().unwrap_or(fixture);
            disagreeing.push(format!("{leaf}{container} id {value}: {tags:?}"));
        }
    }

    println!(
        "  targets pointed at by more than one member: {multi_reference_targets} of {}",
        tags_per_target.len()
    );
    println!("  distinct tags per target: {distinct_tag_histogram:?}");
    let agreeing = distinct_tag_histogram.get(&1).copied().unwrap_or_default();
    println!(
        "  -> {agreeing} of {} targets are always referenced with the same tag",
        tags_per_target.len()
    );
    for line in &disagreeing {
        println!("     disagrees: {line}");
    }
}

/// The first few entries of the two largest segments, byte for byte, so the
/// numbers above can be checked against what is actually on the wire.
fn raw_dump(segments: &[Segment]) {
    println!("\n=== 5. the first entries of the two largest segments ===");
    let mut largest: Vec<&Segment> = segments.iter().collect();
    largest.sort_by_key(|segment| std::cmp::Reverse(segment.entries.len()));
    for segment in largest.iter().take(2) {
        println!("  {} ({} entries)", label(segment), segment.entries.len());
        for entry in segment.entries.iter().take(8) {
            let members: Vec<String> = entry
                .members
                .iter()
                .map(|(value, tag)| format!("({value}, tag {tag})"))
                .collect();
            println!(
                "    +{:#06x} head_hi {} index {:5} span {:5} x{} {}",
                entry.offset,
                entry.head_high,
                entry.index,
                entry.span,
                entry.members.len(),
                members.join(" ")
            );
        }
    }
    let extended_bit_agrees = segments
        .iter()
        .flat_map(|segment| segment.entries.iter())
        .all(|entry| {
            (u32::from(entry.head_high) << 16) & ENTRY_EXTENDED != 0 || entry.head_high == 2
        });
    println!("  head_hi is only ever 2 or 3 (3 = extended bit set): {extended_bit_agrees}");
}
