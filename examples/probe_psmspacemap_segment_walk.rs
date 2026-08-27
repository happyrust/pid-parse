//! Can `PSMspacemap` be walked the way `radsrvitem.dll` walks it?
//!
//! It is the largest thing this crate does not read: 66712 bytes across the
//! four sheet fixtures, 71% of everything the byte audit leaves over, and the
//! only stream family sitting at a flat 0%.
//!
//! `radsrvitem.dll` names it. `sub_5647A180` is `Segment::Load` -- its own
//! failure string says `Segment::Load()[pMgr = 0x%p]: m_iNext > 0x2000` -- and
//! it opens the member stream by the name `swprintf_s(L"0x%.8x", segment << 13)`,
//! which is why the members are called `0x00000000`, `0x00002000`, `0x00004000`:
//! they are segments 0, 1 and 2 of a 13-bit index space. `Segment::Save`
//! (`sub_5647B550`) writes the same two pieces back, so the header is fixed:
//!
//! ```text
//! u32  magic 'tseg'   ('sseg' is the legacy form, read by a different entry reader)
//! u16  entry count
//! u16  simple-slot capacity      -- 24 * this is allocated, so it is a capacity
//! u16  m_iNext, the next free index, refused above 0x2000
//! u16  free-list length n ; n x u16
//! ```
//!
//! Then the entries, per `sub_5647A900`:
//!
//! ```text
//! u32  head       -- low half is the index, and the reader refuses the entry
//!                    unless bit 17 is set, which is what ends the walk
//! u16  a
//! u16  count
//! count x { u32 value ; u16 tag }
//! ```
//!
//! Two of those are worth saying out loud, because they are what a misread
//! frame could not satisfy at once:
//!
//! * the reader's own gate on bit 17 is the terminator. `Segment::Load` runs
//!   `while (entry)` on this reader, so the walk stops when the head stops
//!   looking like one -- which in these files means the zero fill at the end.
//! * `Segment::Load` then requires `persist_id >> 13 == segment` for every
//!   entry and refuses a second entry on an index it has already filled. So
//!   every index has to be under 0x2000 and no two may collide. A frame that
//!   is off by even two bytes fails that immediately.
//!
//! The persist id is `(segment << 13) | index`, which is how the rest of the
//! document addresses these objects.
//!
//! There is a second entry reader, `sub_5647AB70`, which `Segment::Load`
//! selects on a runtime condition rather than on anything in the file. Its
//! frame is a different, more compact one (10 bytes plus two optional runs).
//! No stream in this corpus reads under it -- the walk derails inside the
//! first few entries every time -- so it is not the form these files are in.

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
/// The bit that makes the entry carry two more members than it states.
const ENTRY_EXTENDED: u32 = 0x0001_0000;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One entry, as `sub_5647A900` transfers it.
struct Entry {
    head: u32,
    index: u16,
    /// The `u16` between the head and the member count. The reader stores it
    /// beside the count and adds two to it when the extended bit is set.
    span: u16,
    members: Vec<(u32, u16)>,
    len: usize,
}

fn read_entry(data: &[u8], at: usize) -> Option<Entry> {
    let head = u32_at(data, at)?;
    if head & ENTRY_LIVE == 0 {
        return None;
    }
    let span = u16_at(data, at + 4)?;
    let count = u16_at(data, at + 6)?;
    let len = 8 + 6 * usize::from(count);
    if at + len > data.len() {
        return None;
    }
    let members = (0..usize::from(count))
        .map(|i| {
            let base = at + 8 + 6 * i;
            (u32_at(data, base).unwrap(), u16_at(data, base + 4).unwrap())
        })
        .collect();
    Some(Entry {
        head,
        index: (head & 0xFFFF) as u16,
        span,
        members,
        len,
    })
}

/// What one segment stream states, and what walking it actually yields.
struct Segment {
    legacy: bool,
    entry_count: u16,
    capacity: u16,
    next_free: u16,
    free_list: Vec<u16>,
    entries_start: usize,
    entries: Vec<Entry>,
    /// Where the walk stopped, and whether everything after it is zero fill.
    ends_at: usize,
    tail_is_zero: bool,
}

fn read_segment(data: &[u8]) -> Option<Segment> {
    let magic = u32_at(data, 0)?;
    if magic != SEGMENT_MAGIC && magic != SEGMENT_MAGIC_LEGACY {
        return None;
    }
    let entry_count = u16_at(data, 4)?;
    let capacity = u16_at(data, 6)?;
    let next_free = u16_at(data, 8)?;
    let free_len = u16_at(data, 10)?;
    let free_list: Vec<u16> = (0..usize::from(free_len))
        .filter_map(|i| u16_at(data, 12 + 2 * i))
        .collect();
    let entries_start = 12 + 2 * free_list.len();

    let mut entries = Vec::new();
    let mut at = entries_start;
    while let Some(entry) = read_entry(data, at) {
        at += entry.len;
        entries.push(entry);
    }
    Some(Segment {
        legacy: magic == SEGMENT_MAGIC_LEGACY,
        entry_count,
        capacity,
        next_free,
        free_list,
        entries_start,
        entries,
        ends_at: at,
        tail_is_zero: data[at..].iter().all(|b| *b == 0),
    })
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

/// The segment number a member stream's name states.
fn segment_of(stream: &str) -> Option<u32> {
    let leaf = stream.rsplit(['/', '\\']).next()?;
    let value = u32::from_str_radix(leaf.strip_prefix("0x")?, 16).ok()?;
    (value % SEGMENT_SPAN == 0).then_some(value >> SEGMENT_SHIFT)
}

fn main() {
    let (mut streams, mut clean) = (0usize, 0usize);
    let (mut entries_total, mut members_total) = (0usize, 0usize);
    let (mut extended, mut span_is_count) = (0usize, 0usize);
    let (mut bytes_total, mut bytes_walked) = (0usize, 0usize);
    let mut tag_census: BTreeMap<u16, usize> = BTreeMap::new();
    let mut head_census: BTreeMap<u16, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        println!("\n=== {fixture} ===");
        for (stream, data) in spacemap_streams(path) {
            streams += 1;
            bytes_total += data.len();
            let Some(segment) = segment_of(&stream) else {
                failures.push(format!("{stream}: name is not a segment address"));
                continue;
            };
            let Some(read) = read_segment(&data) else {
                failures.push(format!("{stream}: does not read as a segment"));
                continue;
            };
            bytes_walked += read.ends_at;

            let mut seen: BTreeSet<u16> = BTreeSet::new();
            let mut collisions = 0usize;
            let mut out_of_range = 0usize;
            for entry in &read.entries {
                if u32::from(entry.index) >= SEGMENT_SPAN {
                    out_of_range += 1;
                }
                if !seen.insert(entry.index) {
                    collisions += 1;
                }
                members_total += entry.members.len();
                for (_, tag) in &entry.members {
                    *tag_census.entry(*tag).or_default() += 1;
                }
                *head_census.entry((entry.head >> 16) as u16).or_default() += 1;
                if entry.head & ENTRY_EXTENDED != 0 {
                    extended += 1;
                }
                // The reader adds two to the span when the extended bit is
                // set, which only makes sense if the span counts members. It
                // does not: this says how often the two agree.
                if usize::from(entry.span) == entry.members.len() {
                    span_is_count += 1;
                }
            }
            entries_total += read.entries.len();

            let ok = read.tail_is_zero && collisions == 0 && out_of_range == 0;
            if ok {
                clean += 1;
            } else {
                failures.push(format!(
                    "{stream}: {} bytes left unwalked, {collisions} index collisions, \
                     {out_of_range} indices over 0x2000",
                    data.len() - read.ends_at
                ));
            }
            println!(
                "  {stream}: segment {segment}, {} bytes{} -- header says {} entries, \
                 capacity {}, next free 0x{:04x}, {} free -- walked {} entries from {} \
                 to {} ({} zero-fill bytes) {}",
                data.len(),
                if read.legacy { " [legacy sseg]" } else { "" },
                read.entry_count,
                read.capacity,
                read.next_free,
                read.free_list.len(),
                read.entries.len(),
                read.entries_start,
                read.ends_at,
                data.len() - read.ends_at,
                if ok { "OK" } else { "<- NOT CLEAN" }
            );
        }
    }

    println!("\n=== summary ===");
    println!("streams: {clean}/{streams} walked to the end with no index collision");
    println!(
        "bytes: {bytes_walked} of {bytes_total} consumed ({:.1}%), the rest zero fill",
        100.0 * bytes_walked as f64 / bytes_total as f64
    );
    println!("entries: {entries_total}, members: {members_total}");
    println!("entries with the extended bit set: {extended}");
    println!("entries whose span equals their member count: {span_is_count}");
    println!("head high halves: {head_census:?}");
    println!("member tags: {tag_census:?}");
    for failure in failures.iter().take(10) {
        println!("  FAILED {failure}");
    }
}
