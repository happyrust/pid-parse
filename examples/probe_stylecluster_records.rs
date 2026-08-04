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
