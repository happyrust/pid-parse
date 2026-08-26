//! Can the whole librarian payload be walked as the grammar `style.dll` states?
//!
//! `probe_librarian_native_grammar` showed that every name in the librarian is
//! preceded by a `u32` stating its length. That is necessary but not
//! sufficient: a scan that happens to land on the right words is still a scan.
//! The claim worth testing is stronger -- that the payload is a *sequence*, and
//! that reading it the way `sub_100586A0` reads it consumes every byte.
//!
//! `sub_100586A0` (reached from `IJPersistImp::Load`, slot 5 of the vftable at
//! `.rdata:0x100A32FC`, behind the version-1 gate `sub_100585D0`) transfers, in
//! order:
//!
//! ```text
//! u16 palette_count
//! palette_count x { 16 bytes ; u32 ; u32 ; 16 bytes }   // 40 bytes each
//! u32 entry_count
//! entry_count x {
//!     u16 palette_index
//!     u32
//!     u32 name_len_chars ; name_len_chars x u16         // UTF-16LE, no NUL
//!     u32 path_len_chars ; path_len_chars x u16         // UTF-16LE, no NUL
//!     u32 u32 u32 u32                                   // t0 t1 t2 t3
//! }
//! u32 has_source ; [ source object ]
//! u32 has_second ; [ second object ]
//! ```
//!
//! The `u32` we read as the `oid` is `t1`: the native reader discards an entry
//! whose `t1` is `-1`, and only registers a style when `t1` and the name are
//! both non-empty. So the `+8` gap is not a measurement -- it is `u32 path_len`
//! plus `t0`, and it only lands on `t1` while the path is empty.
//!
//! The probe searches for the offset the entry stream starts at rather than
//! assuming one, then reports where the walk ends. A walk that consumes the
//! payload to within a fixed tail, in every fixture, is the sequence being real.

use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const FIXTURES: [&str; 6] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/symbols/Design/Annotation/Graphics/Circle.sym",
    "test-file/symbols/Piping/Valves/Angle/2-Way Angle Globe Valve.sym",
];

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const STREAM_HEADER_LEN: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const PSM_TYPE_CODE_JSTYLE_LIBRARIAN: u16 = 0x005A;

/// Bytes of a palette record, per the four transfers in the first loop.
const PALETTE_RECORD_LEN: usize = 40;

/// Bytes of an entry that are not its two strings.
const ENTRY_FIXED_LEN: usize = 30;

/// Refuse a stated length no real name or path reaches, so a misframed walk
/// stops instead of running off into the next record.
const MAX_STATED_UNITS: u32 = 1024;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_utf16(payload: &[u8], at: usize, units: usize) -> Option<String> {
    let wide: Vec<u16> = (0..units)
        .map(|i| u16_at(payload, at + i * 2))
        .collect::<Option<_>>()?;
    String::from_utf16(&wide).ok()
}

/// A payload read end to end: where the body started, how many palettes and
/// entries it stated, the entries themselves, and where the walk stopped.
type Walk = (usize, u16, u32, Vec<Entry>, usize);

/// What the librarian states about the file its styles came from: the source
/// object's own version, its name, and the path version 2 adds.
type Source = Option<(u32, String, String)>;

/// The source object read: what it stated, whether a second object follows,
/// and where reading it stopped.
type SourceRead = (Source, u32, usize);

/// One entry as the native reader transfers it.
struct Entry {
    palette_index: u16,
    word: u32,
    name: String,
    path: String,
    tail: [u32; 4],
}

/// Walk `entry_count` entries starting at `at`, or `None` if the stream runs
/// out or states a length no name reaches.
fn walk_entries(payload: &[u8], mut at: usize, entry_count: u32) -> Option<(Vec<Entry>, usize)> {
    let mut out = Vec::new();
    for _ in 0..entry_count {
        let palette_index = u16_at(payload, at)?;
        let word = u32_at(payload, at + 2)?;
        let name_units = u32_at(payload, at + 6)?;
        if name_units > MAX_STATED_UNITS {
            return None;
        }
        let name_at = at + 10;
        let name = read_utf16(payload, name_at, name_units as usize)?;
        let path_len_at = name_at + name_units as usize * 2;
        let path_units = u32_at(payload, path_len_at)?;
        if path_units > MAX_STATED_UNITS {
            return None;
        }
        let path_at = path_len_at + 4;
        let path = read_utf16(payload, path_at, path_units as usize)?;
        let tail_at = path_at + path_units as usize * 2;
        let tail = [
            u32_at(payload, tail_at)?,
            u32_at(payload, tail_at + 4)?,
            u32_at(payload, tail_at + 8)?,
            u32_at(payload, tail_at + 12)?,
        ];
        at = tail_at + 16;
        out.push(Entry {
            palette_index,
            word,
            name,
            path,
            tail,
        });
    }
    Some((out, at))
}

/// The header size that makes the walk fit, searched rather than assumed.
///
/// A candidate is scored by how far the walk gets: only an offset that reads a
/// palette count, an entry count, and then every entry without running out of
/// payload survives at all. Ties go to the walk that ends closest to the end.
fn fit_header(payload: &[u8]) -> Option<Walk> {
    let mut best: Option<Walk> = None;
    let mut header = 0usize;
    while header + 6 <= payload.len() && header <= 64 {
        if let Some(fit) = fit_at(payload, header) {
            // An exact fit is the only one that means anything: the entries and
            // the source object together account for every byte. Anything short
            // of that is kept only so the report can show how near it got.
            let exact = read_source_object(payload, fit.4)
                .is_some_and(|(_, _, done_at)| done_at == payload.len());
            let prev_exact = best.as_ref().is_some_and(|prev| {
                read_source_object(payload, prev.4)
                    .is_some_and(|(_, _, done_at)| done_at == payload.len())
            });
            if (exact && !prev_exact)
                || (exact == prev_exact && best.as_ref().is_none_or(|prev| fit.4 > prev.4))
            {
                best = Some(fit);
            }
        }
        header += 2;
    }
    best
}

fn fit_at(payload: &[u8], header: usize) -> Option<Walk> {
    let palettes = u16_at(payload, header)?;
    let count_at = header + 2 + palettes as usize * PALETTE_RECORD_LEN;
    let entry_count = u32_at(payload, count_at)?;
    if entry_count == 0 || entry_count as usize * ENTRY_FIXED_LEN > payload.len() {
        return None;
    }
    let (entries, ends_at) = walk_entries(payload, count_at + 4, entry_count)?;
    Some((header, palettes, entry_count, entries, ends_at))
}

/// The object the librarian persists after its entries: `u32 has_source`, and
/// when that is set, an object stating its own version, a name, and — from
/// version 2 — the file that name was read from. A last `u32` says whether a
/// second object follows.
///
/// Returned as `((version, name, path), has_second, ends_at)` so the caller can
/// see whether reading it this way accounts for the payload exactly.
fn read_source_object(payload: &[u8], mut at: usize) -> Option<SourceRead> {
    let has_source = u32_at(payload, at)?;
    at += 4;
    let source = if has_source == 0 {
        None
    } else {
        let version = u32_at(payload, at)?;
        let name_units = u32_at(payload, at + 4)?;
        if name_units > MAX_STATED_UNITS {
            return None;
        }
        let name = read_utf16(payload, at + 8, name_units as usize)?;
        at = at + 8 + name_units as usize * 2;
        // Version 1 states only a name; the path arrives with version 2.
        let path = if version >= 2 {
            let path_units = u32_at(payload, at)?;
            if path_units > MAX_STATED_UNITS {
                return None;
            }
            let path = read_utf16(payload, at + 4, path_units as usize)?;
            at = at + 4 + path_units as usize * 2;
            path
        } else {
            String::new()
        };
        Some((version, name, path))
    };
    let has_second = u32_at(payload, at)?;
    Some((source, has_second, at + 4))
}

fn librarian_payloads(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let cluster_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().unwrap_or("") == "StyleCluster")
        .collect();

    let mut out = Vec::new();
    for cluster_path in &cluster_paths {
        let Ok(mut stream) = cfb.open_stream(cluster_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CLUSTER_MAGIC) {
            continue;
        }
        let mut at = STREAM_HEADER_LEN;
        while let (Some(type_word), Some(bytes_to_follow)) =
            (u16_at(&data, at), u32_at(&data, at + 2))
        {
            let payload_start = at + PSM_ENVELOPE_LEN;
            let Some(end) = payload_start.checked_add(bytes_to_follow as usize) else {
                break;
            };
            if bytes_to_follow == 0 || end > data.len() {
                break;
            }
            if type_word & 0x3FFF == PSM_TYPE_CODE_JSTYLE_LIBRARIAN {
                out.push((cluster_path.clone(), data[payload_start..end].to_vec()));
            }
            at = end;
        }
    }
    out
}

/// Whether the walk accounted for the payload: with no second object, the
/// source object has to land exactly on the end.
fn is_exact(payload: &[u8], has_second: u32, done_at: usize) -> bool {
    if has_second == 0 {
        done_at == payload.len()
    } else {
        done_at <= payload.len()
    }
}

/// Read every librarian in every `.sym` under a directory and count how many
/// the grammar accounts for. The two hand-picked libraries are a sample; this
/// is the whole shelf.
fn sweep(root: &Path) {
    let Ok(walk) = std::fs::read_dir(root) else {
        println!("skip: {} is absent", root.display());
        return;
    };
    let mut files = 0usize;
    let mut payloads = 0usize;
    let mut exact = 0usize;
    let mut named = 0usize;
    let mut with_path = 0usize;
    let mut sources: Vec<String> = Vec::new();
    let mut versions: Vec<u32> = Vec::new();
    let mut headers: Vec<usize> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    let mut stack: Vec<std::fs::DirEntry> = walk.filter_map(Result::ok).collect();
    while let Some(entry) = stack.pop() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(inner) = std::fs::read_dir(&path) {
                stack.extend(inner.filter_map(Result::ok));
            }
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "sym") {
            continue;
        }
        files += 1;
        for (_, payload) in librarian_payloads(&path) {
            payloads += 1;
            let Some((header, _, _, entries, ends_at)) = fit_header(&payload) else {
                failures.push(format!("{}: no walk fits", path.display()));
                continue;
            };
            headers.push(header);
            named += entries.iter().filter(|e| !e.name.is_empty()).count();
            with_path += entries.iter().filter(|e| !e.path.is_empty()).count();
            match read_source_object(&payload, ends_at) {
                Some((source, has_second, done_at)) if is_exact(&payload, has_second, done_at) => {
                    exact += 1;
                    if let Some((version, name, source_path)) = source {
                        versions.push(version);
                        sources.push(if source_path.is_empty() {
                            name
                        } else {
                            source_path
                        });
                    }
                }
                _ => failures.push(format!("{}: tail unaccounted for", path.display())),
            }
        }
    }

    sources.sort_unstable();
    sources.dedup();
    versions.sort_unstable();
    versions.dedup();
    headers.sort_unstable();
    headers.dedup();
    println!("\n=== sweep of {} ===", root.display());
    println!("{files} .sym files, {payloads} librarian records, {exact} accounted for exactly");
    println!("named entries: {named}, of which carry a path: {with_path}");
    println!("headers seen: {headers:?}");
    println!("source object versions: {versions:?}");
    println!("distinct sources: {sources:?}");
    for failure in failures.iter().take(10) {
        println!("  FAILED {failure}");
    }
}

fn main() {
    let mut payloads_seen = 0usize;
    let mut payloads_walked = 0usize;
    let mut named = 0usize;
    let mut with_path = 0usize;
    let mut headers: Vec<usize> = Vec::new();
    let mut tails: Vec<usize> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        println!("\n=== {fixture} ===");
        for (cluster, payload) in librarian_payloads(path) {
            payloads_seen += 1;
            let Some((header, palettes, entry_count, entries, ends_at)) = fit_header(&payload)
            else {
                println!("  {cluster}: {} bytes -- NO WALK FITS", payload.len());
                continue;
            };
            payloads_walked += 1;
            headers.push(header);
            let names = entries.iter().filter(|e| !e.name.is_empty()).count();
            let paths = entries.iter().filter(|e| !e.path.is_empty()).count();
            named += names;
            with_path += paths;
            println!(
                "  {cluster}: {} bytes; header {header}, {palettes} palettes, \
                 {entry_count} entries, {names} named, {paths} with a path",
                payload.len()
            );
            println!(
                "      tail from {ends_at}: {}",
                payload[ends_at..]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            match read_source_object(&payload, ends_at) {
                Some((source, has_second, done_at)) => {
                    tails.push(payload.len() - done_at);
                    println!(
                        "      source object: {source:?}; has_second {has_second}; \
                         ends at {done_at} of {} ({} unread)",
                        payload.len(),
                        payload.len() - done_at
                    );
                }
                None => println!(
                    "      source object: UNREADABLE from {ends_at} of {}",
                    payload.len()
                ),
            }
            for (i, entry) in entries.iter().enumerate() {
                if entry.name.is_empty() && entry.path.is_empty() {
                    continue;
                }
                println!(
                    "      #{i:<4} pal {:<4} w {:<10} t {:?} {:?}{}",
                    entry.palette_index,
                    entry.word,
                    entry.tail,
                    entry.name,
                    if entry.path.is_empty() {
                        String::new()
                    } else {
                        format!("  <- {:?}", entry.path)
                    }
                );
            }
        }
    }

    headers.sort_unstable();
    headers.dedup();
    tails.sort_unstable();
    tails.dedup();
    println!("\n=== summary ===");
    println!("payloads: {payloads_walked}/{payloads_seen} walked end to end");
    println!("headers seen: {headers:?}");
    println!("bytes left unread after the source object: {tails:?}");
    println!("named entries: {named}, of which carry a path: {with_path}");

    sweep(Path::new("test-file/symbols-full"));
}
