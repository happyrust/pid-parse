//! What the `/StyleCluster` stream holds, now that its GUIDs have names.
//!
//! Text height and line type are the two remaining render-fidelity gaps, and
//! both were blocked on "the style table is somewhere in `0x005A` and we do not
//! know what it is". The PSM type-code registry names `0x005A` as `style.dll`'s
//! **JSL Style Librarian**, and the RAD CLSID registry names three of the GUIDs
//! the cluster carries in every fixture:
//!
//! * `606FE422-0025-11D0-A1E1-080036A1CF02` -- JSL Text Char Style Type/Workbench
//! * `93ADC030-0CB6-11D0-B29B-08003622D702` -- JSL Dash Style Type/Workbench
//! * `606FE425-0025-11D0-A1E1-080036A1CF02` -- JSL SmartFrame Style Type/Workbench
//!
//! So the carriers are identified and present. This locates them in the stream
//! and reports what surrounds them, which is the step before deciding whether a
//! style record can be read.
//!
//! Phase 35-D showed `igTextBox` carries a stable style id in its trailer
//! (56 for Chinese, 64 for pipe numbers, 21 for general annotation). If the
//! cluster holds a table keyed by those ids, the join is what unblocks height.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

/// The named style workbenches, as `(guid_bytes, label)`.
fn known_styles() -> Vec<([u8; 16], &'static str)> {
    vec![
        (
            guid(0x606FE422, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL Text Char Style",
        ),
        (
            guid(0x93ADC030, 0x0CB6, 0x11D0, "B29B08003622D702"),
            "JSL Dash Style",
        ),
        (
            guid(0x606FE425, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL SmartFrame Style",
        ),
    ]
}

fn guid(d1: u32, d2: u16, d3: u16, rest: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&d1.to_le_bytes());
    out[4..6].copy_from_slice(&d2.to_le_bytes());
    out[6..8].copy_from_slice(&d3.to_le_bytes());
    let tail = (0..8)
        .map(|i| u8::from_str_radix(&rest[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect::<Vec<_>>();
    out[8..16].copy_from_slice(&tail);
    out
}

fn occurrences(data: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || data.len() < needle.len() {
        return Vec::new();
    }
    (0..=data.len() - needle.len())
        .filter(|at| &data[*at..*at + needle.len()] == needle)
        .collect()
}

/// The style ids phase 35-D found in `igTextBox` trailers. If the cluster is
/// keyed by them they should appear as bare words near a style record.
const KNOWN_STYLE_IDS: [u32; 3] = [21, 56, 64];

/// The stream opens with this, ahead of the PSM envelope.
const STYLECLUSTER_MAGIC: u32 = 0x6C90_F544;

/// Every GUID the cluster carries in all six fixtures, from
/// `probe_phase29_stylecluster_prefix`. Names, where known, come from the RAD
/// registry via `tools/clsid_registry.py`; the unnamed ones are not in it.
fn catalogue() -> Vec<([u8; 16], &'static str)> {
    vec![
        // `probe_phase29_stylecluster_prefix` also reported 00000003-... and
        // 00000006-..., but those are GUID-shaped byte runs that straddle two
        // real entries rather than entries themselves, so they are left out.
        (
            guid(0x00000000, 0x0000, 0x0000, "C000000000000046"),
            "IUnknown",
        ),
        (
            guid(0x571A3A00, 0x3D33, 0x11CE, "BA54080036019EE7"),
            "(unnamed 571A3A00)",
        ),
        (
            guid(0x606FE420, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL style type 420",
        ),
        (
            guid(0x606FE421, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL style type 421",
        ),
        (
            guid(0x606FE422, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL Text Char Style",
        ),
        (
            guid(0x606FE423, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL style type 423",
        ),
        (
            guid(0x606FE424, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL style type 424",
        ),
        (
            guid(0x606FE425, 0x0025, 0x11D0, "A1E1080036A1CF02"),
            "JSL SmartFrame Style",
        ),
        (
            guid(0x93ADC030, 0x0CB6, 0x11D0, "B29B08003622D702"),
            "JSL Dash Style",
        ),
        (
            guid(0xEA1ACBD2, 0xE20A, 0x11CE, "95CA080036483902"),
            "(unnamed EA1ACBD2)",
        ),
        (
            guid(0x0391DF90, 0x5D4C, 0x11CE, "BB2E08003601BDA9"),
            "(unnamed 0391DF90)",
        ),
    ]
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let s = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let s = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

/// Read the stream header and the GUID directory that follows it.
///
/// Layout as the fixtures show it:
/// ```text
///   +0   u32  magic 0x6C90F544
///   +4   u32  count (varies per stream)
///   +8   u16  PSM type word; low 14 bits are 0x005A, JSL Style Librarian
///   +10  u32  bytes_to_follow
///   +14  6 bytes
///   +26  u16  directory entry count
///   +28       the directory itself
/// ```
/// Returns where the directory ended, so the body can be looked at separately.
fn read_header_and_directory(data: &[u8]) -> Option<usize> {
    let magic = u32_at(data, 0)?;
    let count = u32_at(data, 4)?;
    let type_word = u16_at(data, 8)?;
    let btf = u32_at(data, 10)?;
    let dir_len = u16_at(data, 26)?;
    println!(
        "    header: magic=0x{magic:08X}{}  count={count}  psm_type=0x{:04X} flags={}  bytes_to_follow={btf}  dir_entries={dir_len}",
        if magic == STYLECLUSTER_MAGIC { "" } else { " (UNEXPECTED)" },
        type_word & 0x3FFF,
        type_word >> 14,
    );
    println!(
        "    envelope covers {}..{} of {} byte(s){}",
        14,
        14 + btf as usize,
        data.len(),
        if 14 + btf as usize == data.len() {
            "  <- exactly the stream"
        } else {
            ""
        }
    );

    // Walk the directory by locating each catalogued GUID and reporting the
    // gap to the next, which is what reveals the entry shape.
    let known = catalogue();
    let mut hits: Vec<(usize, &'static str)> = Vec::new();
    for at in 0..data.len().saturating_sub(16) {
        if let Some((_, label)) = known.iter().find(|(g, _)| &data[at..at + 16] == g) {
            // A GUID cannot start inside the previous one.
            if hits.last().is_some_and(|(prev, _)| at < prev + 16) {
                continue;
            }
            hits.push((at, label));
        }
    }
    println!("    catalogued GUIDs in stream: {}", hits.len());
    let mut last_end = 0usize;
    for (i, (at, label)) in hits.iter().enumerate() {
        let gap = at.saturating_sub(last_end);
        let trailer: Vec<String> = (0..2)
            .filter_map(|k| u32_at(data, at + 16 + k * 4))
            .map(|v| v.to_string())
            .collect();
        println!(
            "      [{i:>2}] @0x{at:04X} gap={gap:<3} {label:<22} next2u32=[{}]",
            trailer.join(", ")
        );
        last_end = at + 16;
        // Stop once the run of directory entries clearly ends.
        if i + 1 < hits.len() && hits[i + 1].0 - last_end > 64 {
            println!(
                "      -- gap of {} bytes here; treating this as the directory end --",
                hits[i + 1].0 - last_end
            );
            return Some(last_end);
        }
    }
    Some(last_end)
}

fn dump(data: &[u8], at: usize, before: usize, after: usize) {
    let start = at.saturating_sub(before);
    let end = (at + after).min(data.len());
    for row in (start..end).step_by(16) {
        let chunk = &data[row..(row + 16).min(end)];
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        let txt: String = chunk
            .iter()
            .map(|b| {
                if (0x20..0x7F).contains(b) {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        let mark = if row <= at && at < row + 16 {
            " <<<"
        } else {
            ""
        };
        println!("      {row:06X}  {:<47}  |{txt}|{mark}", hex.join(" "));
    }
}

/// Enumerate the class GUIDs the body carries, so they can be named.
///
/// A RAD CLSID has its version nibble in `data[7] >> 4` equal to 1 and a
/// recognisable variant, which is enough of a filter to pick real GUIDs out of
/// a stream of records without knowing the record shape yet. Output is meant
/// to be pasted into `tools/clsid_registry.py`.
fn report_body_guids(data: &[u8], from: usize) {
    let mut seen: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut at = from;
    while at + 16 <= data.len() {
        let g = &data[at..at + 16];
        // Version 1 (time-based) GUIDs: the top nibble of the version field.
        let version_ok = (g[7] >> 4) == 1;
        let variant_ok = (g[8] & 0xC0) == 0x80;
        let nonzero = g.iter().any(|b| *b != 0);
        if version_ok && variant_ok && nonzero {
            let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
            let d2 = u16::from_le_bytes([g[4], g[5]]);
            let d3 = u16::from_le_bytes([g[6], g[7]]);
            let tail: String = g[8..16].iter().map(|b| format!("{b:02X}")).collect();
            let text = format!("{d1:08X}-{d2:04X}-{d3:04X}-{}-{}", &tail[..4], &tail[4..]);
            let entry = seen.entry(text).or_insert((0, at));
            entry.0 += 1;
            at += 16;
            continue;
        }
        at += 1;
    }
    println!("    distinct class GUIDs in body: {}", seen.len());
    let mut rows: Vec<(&String, &(usize, usize))> = seen.iter().collect();
    rows.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
    for (text, (count, first)) in rows.iter().take(20) {
        println!("      {count:>4}x  {text}   first@0x{first:04X}");
    }
}

/// Walk the stream as a chain of PSM records.
///
/// The stream is `u32 magic + u32 count` and then records in the same
/// `u16 type_word + u32 bytes_to_follow` envelope the Sheet streams use. The
/// first is the `0x005A` librarian holding the type directories; the style
/// instances have to be in what follows it.
fn walk_records(data: &[u8]) {
    const HEADER: usize = 8;
    const ENVELOPE: usize = 6;

    println!("    record chain:");
    let mut at = HEADER;
    let mut counts: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
    let mut walked = 0usize;
    while at + ENVELOPE <= data.len() {
        let Some(type_word) = u16_at(data, at) else {
            break;
        };
        let Some(btf) = u32_at(data, at + 2) else {
            break;
        };
        let type_code = type_word & 0x3FFF;
        let end = at + ENVELOPE + btf as usize;
        if type_code == 0 || btf == 0 || end > data.len() {
            println!(
                "      stopped at 0x{at:04X}: type=0x{type_code:04X} btf={btf} would end at {end} (stream is {})",
                data.len()
            );
            break;
        }
        let entry = counts.entry(type_code).or_insert((0, at));
        entry.0 += 1;
        if walked < 12 {
            println!(
                "      @0x{at:04X}  type=0x{type_code:04X} flags={}  btf={btf:<6} -> next 0x{end:04X}",
                type_word >> 14
            );
        }
        walked += 1;
        at = end;
    }
    println!(
        "      walked {walked} record(s), reached 0x{at:04X} of 0x{:04X}",
        data.len()
    );
    println!("      type histogram:");
    for (code, (count, first)) in &counts {
        println!("        0x{code:04X}: {count:>4}  first@0x{first:04X}");
    }
}

fn probe(path: &Path) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return;
    };
    let streams: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();

    println!("\n=== {} ===", path.display());
    let style_streams: Vec<&String> = streams
        .iter()
        .filter(|p| p.to_lowercase().contains("style"))
        .collect();
    println!("  style-ish streams: {style_streams:?}");

    for name in &style_streams {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        println!("\n  -- {name} ({} bytes) --", data.len());

        if let Some(dir_end) = read_header_and_directory(&data) {
            println!(
                "    directory ends at 0x{dir_end:04X}; body is {} byte(s)",
                data.len() - dir_end
            );
            println!("    body head:");
            dump(&data, dir_end, 0, 96);
            report_body_guids(&data, dir_end);
        }
        {
            walk_records(&data);
        }

        for (needle, label) in known_styles() {
            let hits = occurrences(&data, &needle);
            if hits.is_empty() {
                println!("    {label:<22} absent");
                continue;
            }
            println!("    {label:<22} {} hit(s) at {:?}", hits.len(), hits);
            // The first one, with enough context to see the record around it.
            dump(&data, hits[0], 48, 96);
        }

        // Are phase 35-D's style ids present as bare words at all?
        let mut id_counts: BTreeMap<u32, usize> = BTreeMap::new();
        for at in (0..data.len().saturating_sub(3)).step_by(2) {
            let word = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
            if KNOWN_STYLE_IDS.contains(&word) {
                *id_counts.entry(word).or_default() += 1;
            }
        }
        println!("    phase 35-D style ids as u32 in this stream: {id_counts:?}");
    }
}

fn main() {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
    ] {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path);
        }
    }
}
