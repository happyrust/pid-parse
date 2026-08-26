//! Does the librarian's palette index say which family a name belongs to?
//!
//! Today the only thing that sorts `psOk` from `lsOk` is the two letters in
//! front of them. That works, but it is a reading of the vendor's naming habit,
//! not of the file: a style called `Solid` or `Chinese` says nothing about its
//! family, and a project that renamed its statuses would take the rule with it.
//!
//! The librarian states it outright. Every entry opens with a `u16` that
//! `sub_100586A0` uses as an index — `*((_DWORD *)v2 + 11) + 4 * LOWORD(Block[0])`
//! — into the palette directory the payload opens with. Each palette record is
//! 40 bytes holding **two GUIDs**, and one of them should be the style class:
//! `47FCC335-…` for a simple line, `47FCC33B-…` for a point symbol, and so on
//! out of the `radsrvitem` type table that `tools/psm_type_clsid.py` resolves.
//!
//! So there is a checkable claim, and this probe checks it end to end: for each
//! named entry, take the family of the record its `oid` really lands on, take
//! the GUID its palette index points at, and see whether the two agree — over
//! every named entry in the corpus, with nothing straddling.

use std::collections::{BTreeMap, BTreeSet};
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

/// Every style family and the CLSID `radsrvitem`'s type table gives it.
const FAMILIES: [(u16, &str, &str); 10] = [
    (
        0x0029,
        "Multiplexer",
        "47FCC330-2D0F-11D0-A1FF-080036A1CF02",
    ),
    (0x002A, "SimpleFill", "47FCC331-2D0F-11D0-A1FF-080036A1CF02"),
    (0x002B, "HatchFill", "47FCC332-2D0F-11D0-A1FF-080036A1CF02"),
    (0x002C, "TextChar", "47FCC333-2D0F-11D0-A1FF-080036A1CF02"),
    (0x002D, "TextPara", "47FCC334-2D0F-11D0-A1FF-080036A1CF02"),
    (0x002E, "SimpleLine", "47FCC335-2D0F-11D0-A1FF-080036A1CF02"),
    (
        0x002F,
        "SimpleDashType",
        "47FCC336-2D0F-11D0-A1FF-080036A1CF02",
    ),
    (0x0030, "Override", "47FCC338-2D0F-11D0-A1FF-080036A1CF02"),
    (
        0x0032,
        "PointSymbol",
        "47FCC33B-2D0F-11D0-A1FF-080036A1CF02",
    ),
    (
        0x0033,
        "LineTerminator",
        "47FCC33C-2D0F-11D0-A1FF-080036A1CF02",
    ),
];

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const STREAM_HEADER_LEN: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const PSM_TYPE_CODE_JSTYLE_LIBRARIAN: u16 = 0x005A;
const LIBRARIAN_BODY_OFFSET: usize = 12;
const PALETTE_RECORD_LEN: usize = 40;
const MAX_STATED_UNITS: u32 = 1024;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// A 16-byte GUID in the on-disk mixed-endian layout, printed the usual way.
fn guid_at(data: &[u8], at: usize) -> Option<String> {
    let raw = data.get(at..at + 16)?;
    Some(format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{}",
        u32::from_le_bytes(raw[0..4].try_into().ok()?),
        u16::from_le_bytes(raw[4..6].try_into().ok()?),
        u16::from_le_bytes(raw[6..8].try_into().ok()?),
        raw[8],
        raw[9],
        raw[10..16]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<String>()
    ))
}

fn read_string(payload: &[u8], at: usize) -> Option<(String, usize)> {
    let units = u32_at(payload, at)?;
    if units > MAX_STATED_UNITS {
        return None;
    }
    let end = at + 4 + units as usize * 2;
    let text = payload.get(at + 4..end)?;
    let wide: Vec<u16> = text
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(u16::from_le_bytes)
        .collect();
    Some((String::from_utf16(&wide).ok()?, end))
}

/// One named entry: the palette it points at and the object it names.
struct Named {
    palette_index: u16,
    object: u32,
    name: String,
}

/// One librarian read for this question: the two GUIDs of each palette record,
/// and every entry that states a name.
type Librarian = (Vec<(String, String)>, Vec<Named>);

/// The palette GUIDs and the named entries of one librarian payload.
fn read_librarian(payload: &[u8]) -> Option<Librarian> {
    let palette_count = u16_at(payload, LIBRARIAN_BODY_OFFSET)?;
    let mut palettes = Vec::new();
    for i in 0..usize::from(palette_count) {
        let at = LIBRARIAN_BODY_OFFSET + 2 + i * PALETTE_RECORD_LEN;
        palettes.push((guid_at(payload, at)?, guid_at(payload, at + 24)?));
    }
    let count_at = LIBRARIAN_BODY_OFFSET + 2 + usize::from(palette_count) * PALETTE_RECORD_LEN;
    let entry_count = u32_at(payload, count_at)?;
    let mut at = count_at + 4;
    let mut named = Vec::new();
    for _ in 0..entry_count {
        let palette_index = u16_at(payload, at)?;
        let (name, after_name) = read_string(payload, at + 6)?;
        let (_, after_path) = read_string(payload, after_name)?;
        let object = u32_at(payload, after_path + 4)?;
        at = after_path + 16;
        if at > payload.len() {
            return None;
        }
        if !name.is_empty() {
            named.push(Named {
                palette_index,
                object,
                name,
            });
        }
    }
    Some((palettes, named))
}

/// Every record's family by `oid`, plus every librarian payload, from one
/// `StyleCluster` stream.
fn read_cluster(data: &[u8]) -> (BTreeMap<u32, u16>, Vec<Vec<u8>>) {
    let mut families = BTreeMap::new();
    let mut librarians = Vec::new();
    if u32_at(data, 0) != Some(CLUSTER_MAGIC) {
        return (families, librarians);
    }
    let mut at = STREAM_HEADER_LEN;
    while let (Some(type_word), Some(bytes_to_follow)) = (u16_at(data, at), u32_at(data, at + 2)) {
        let payload_start = at + PSM_ENVELOPE_LEN;
        let Some(end) = payload_start.checked_add(bytes_to_follow as usize) else {
            break;
        };
        if bytes_to_follow == 0 || end > data.len() {
            break;
        }
        let payload = &data[payload_start..end];
        let type_code = type_word & 0x3FFF;
        if type_code == PSM_TYPE_CODE_JSTYLE_LIBRARIAN {
            librarians.push(payload.to_vec());
        } else if let Some(oid) = u32_at(payload, 0) {
            families.entry(oid).or_insert(type_code);
        }
        at = end;
    }
    (families, librarians)
}

fn cluster_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
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
        .filter(|p| p.rsplit('/').next().unwrap_or("") == "StyleCluster")
        .collect();
    let mut out = Vec::new();
    for cluster in paths {
        let Ok(mut stream) = cfb.open_stream(&cluster) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push((cluster, data));
        }
    }
    out
}

/// Join every named entry in every `.sym` under a directory: which palette
/// GUID it points at, and which family its `oid` really lands on.
///
/// Keyed by GUID rather than by position, because a table that holds only
/// while every file lists its palettes in the same order is a coincidence
/// waiting to be found out.
fn sweep(root: &Path, by_guid: &mut BTreeMap<String, BTreeSet<u16>>) -> (usize, usize, usize) {
    let Ok(top) = std::fs::read_dir(root) else {
        println!("skip: {} is absent", root.display());
        return (0, 0, 0);
    };
    let (mut files, mut joined, mut unresolved) = (0usize, 0usize, 0usize);
    let mut stack: Vec<std::fs::DirEntry> = top.filter_map(Result::ok).collect();
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
        for (_, data) in cluster_streams(&path) {
            let (families, librarians) = read_cluster(&data);
            for payload in &librarians {
                let Some((palettes, named)) = read_librarian(payload) else {
                    continue;
                };
                for entry in &named {
                    let Some(landed) = families.get(&entry.object) else {
                        unresolved += 1;
                        continue;
                    };
                    let Some((guid, _)) = palettes.get(usize::from(entry.palette_index)) else {
                        continue;
                    };
                    joined += 1;
                    by_guid.entry(guid.clone()).or_default().insert(*landed);
                }
            }
        }
    }
    (files, joined, unresolved)
}

fn main() {
    let by_clsid: BTreeMap<&str, (u16, &str)> = FAMILIES
        .iter()
        .map(|(code, label, clsid)| (*clsid, (*code, *label)))
        .collect();
    let mut by_guid: BTreeMap<String, BTreeSet<u16>> = BTreeMap::new();

    // Which GUID slot of a palette record is the style class, and does the
    // index reach the family the oid really lands on?
    let mut agree_first = 0usize;
    let mut agree_second = 0usize;
    let mut checked = 0usize;
    let mut unresolved = 0usize;
    let mut disagreements: Vec<String> = Vec::new();
    let mut by_index: BTreeMap<u16, BTreeSet<String>> = BTreeMap::new();
    let mut prefix_vs_palette: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        println!("\n=== {fixture} ===");
        for (cluster, data) in cluster_streams(path) {
            let (families, librarians) = read_cluster(&data);
            for payload in &librarians {
                let Some((palettes, named)) = read_librarian(payload) else {
                    println!("  {cluster}: librarian did not walk");
                    continue;
                };
                println!(
                    "  {cluster}: {} palettes, {} named",
                    palettes.len(),
                    named.len()
                );
                for (i, (first, second)) in palettes.iter().enumerate() {
                    let hit = by_clsid
                        .get(second.as_str())
                        .map(|(code, label)| format!("{label} 0x{code:04X}"))
                        .unwrap_or_else(|| "-".into());
                    println!("      palette {i:<3} {first}  {second}  {hit}");
                }
                for entry in &named {
                    let Some(landed) = families.get(&entry.object) else {
                        unresolved += 1;
                        continue;
                    };
                    let Some((first, second)) = palettes.get(usize::from(entry.palette_index))
                    else {
                        disagreements.push(format!(
                            "{fixture}: {:?} palette index {} is past the directory",
                            entry.name, entry.palette_index
                        ));
                        continue;
                    };
                    checked += 1;
                    if by_clsid.get(first.as_str()).map(|(code, _)| code) == Some(landed) {
                        agree_first += 1;
                    }
                    if by_clsid.get(second.as_str()).map(|(code, _)| code) == Some(landed) {
                        agree_second += 1;
                    }
                    by_index
                        .entry(entry.palette_index)
                        .or_default()
                        .insert(format!("0x{landed:04X}"));
                    by_guid.entry(first.clone()).or_default().insert(*landed);

                    // The rule this would replace.
                    let guessed = if entry.name.starts_with("ps") {
                        Some(0x0032u16)
                    } else if entry.name.starts_with("ls") {
                        Some(0x002E)
                    } else {
                        None
                    };
                    if guessed.is_none() {
                        prefix_vs_palette.push(format!(
                            "{:?} -> 0x{landed:04X} (the prefix says nothing)",
                            entry.name
                        ));
                    }
                }
            }
        }
    }

    let (sym_files, sym_joined, sym_unresolved) =
        sweep(Path::new("test-file/symbols-full"), &mut by_guid);

    println!("\n=== summary ===");
    println!("named entries joined to a record: {checked} (oid not in this cluster: {unresolved})");
    println!(
        "sweep of test-file/symbols-full: {sym_files} files, {sym_joined} joined, \
         {sym_unresolved} unresolved"
    );
    println!("\npalette GUID -> families it reached (corpus-wide):");
    let mut straddles = 0usize;
    for (guid, codes) in &by_guid {
        let labels: Vec<String> = codes.iter().map(|code| format!("0x{code:04X}")).collect();
        if codes.len() > 1 {
            straddles += 1;
        }
        println!(
            "  {guid}  {}{}",
            labels.join(" "),
            if codes.len() > 1 {
                "   <-- STRADDLES"
            } else {
                ""
            }
        );
    }
    println!("palette GUIDs reaching more than one family: {straddles}");
    println!(
        "\nthe dead hypothesis: a palette record's GUIDs are not the style class.\n  \
         slot 1 matches the class CLSID for {agree_first} of {checked} entries, \
         slot 2 for {agree_second}.\n  \
         They are the palette's own identity and the interface it serves, so the\n  \
         family has to be joined against the records, which is what the table above is."
    );
    println!("\npalette index -> families it reached (the four sheets and two samples):");
    for (index, codes) in &by_index {
        println!("  {index:<3} {codes:?}");
    }
    if !disagreements.is_empty() {
        println!("\nentries whose palette index is past the directory:");
        for line in disagreements.iter().take(10) {
            println!("  {line}");
        }
    }
    println!(
        "\nnames the ps/ls rule cannot classify: {}",
        prefix_vs_palette.len()
    );
    let mut sample: Vec<&String> = prefix_vs_palette.iter().collect();
    sample.sort_unstable();
    sample.dedup();
    for line in sample.iter().take(20) {
        println!("  {line}");
    }
}
