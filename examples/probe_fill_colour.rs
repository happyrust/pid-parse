//! Where does `0x002A JStyleSimpleFill` keep its colour on disk?
//!
//! `dlls/style.dll` settles the field list: the version-2 `JStyleSimpleFill`
//! worker (`sub_1001D610`) reads a base block and then exactly three `u32`
//! fields, into object offsets +104, +112, +120. The first, +104, is the same
//! object slot `JStyleSimpleLine` stores its `COLORREF` in, and it is the one
//! the `IJStyleSolidFillImp` interface exposes through a single get/put colour
//! pair — so it is the solid fill's colour. See
//! `docs/analysis/2026-08-10-fill-colour-is-002a-plus-30.md`.
//!
//! The DLL's in-memory framing is not the `.pid`'s, so the byte offset is a
//! corpus question. This dumps every `0x002A` payload and, at every 4-byte
//! offset, reads a Win32 `COLORREF` (`[R, G, B, 0]`) — the same encoding and
//! test that pinned the line colour. The decisive column is "in line palette":
//! whether that offset's colour is one this same file already uses for a line,
//! since a P&ID draws its fills in the same palette as its strokes.
//!
//! ```powershell
//! cargo run --example probe_fill_colour
//! ```

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::PSM_TYPE_CODE_JSTYLE_OVERRIDE;
use pid_parse::style_link::{
    DocumentStyleTable, PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL, PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE,
    SIMPLE_LINE_COLOUR_OFFSET,
};

/// The offset `style.dll` + the corpus place the fill colour at: the first
/// class field after the 30-byte base block.
const FILL_COLOUR_OFFSET: usize = 30;

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
];

const PSM_ENVELOPE_LEN: usize = 6;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn clusters(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with("StyleCluster"))
        })
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name, bytes));
        }
    }
    out
}

fn colorref_rgb(word: u32) -> [u8; 3] {
    [
        (word & 0xFF) as u8,
        ((word >> 8) & 0xFF) as u8,
        ((word >> 16) & 0xFF) as u8,
    ]
}

fn hexs(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let s = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn main() {
    let root = test_file_root();

    for name in FIXTURES {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        for (cluster_name, cluster) in clusters(&path) {
            let table = DocumentStyleTable::from_stylecluster_bytes(&cluster);

            // The palette this file draws lines in: every 0x002E colour.
            let mut line_palette: BTreeSet<u32> = BTreeSet::new();
            for record in table.records() {
                if record.type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE {
                    let payload =
                        &cluster[record.byte_range.start + PSM_ENVELOPE_LEN..record.byte_range.end];
                    if let Some(word) = u32_at(payload, SIMPLE_LINE_COLOUR_OFFSET) {
                        line_palette.insert(word & 0x00FF_FFFF);
                    }
                }
            }

            // Which fill ids a JStyleOverride names as its base object — these
            // are the ones a boundary actually reaches, so their colour is what
            // ends up on the sheet.
            let mut referenced: BTreeSet<u32> = BTreeSet::new();
            for record in table.records() {
                if record.type_code == PSM_TYPE_CODE_JSTYLE_OVERRIDE {
                    if let Some(base) = record.base_reference {
                        if let Some(target) = table.get(base) {
                            if target.type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL {
                                referenced.insert(target.style_id);
                            }
                        }
                    }
                }
            }

            let fills: Vec<_> = table
                .records()
                .iter()
                .filter(|r| r.type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL)
                .collect();
            if fills.is_empty() {
                continue;
            }

            println!("\n=== {name}  {cluster_name} ===");
            println!(
                "  line palette ({}): {}",
                line_palette.len(),
                line_palette
                    .iter()
                    .map(|c| hexs(colorref_rgb(*c)))
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            // Decode the colour at +30 for every fill. `-2` (0xFFFFFFFE) is the
            // "no colour set" sentinel the DLL writes into the same object slot
            // (IJStyleSimpleFillImp::put, obj+104); anything with a non-zero
            // high byte is not a plain RGB COLORREF and is reported as unset.
            for record in &fills {
                let payload =
                    &cluster[record.byte_range.start + PSM_ENVELOPE_LEN..record.byte_range.end];
                let word = u32_at(payload, FILL_COLOUR_OFFSET).unwrap_or(0);
                let high = (word >> 24) & 0xFF;
                let marker = if referenced.contains(&record.style_id) {
                    " <- referenced by a boundary"
                } else {
                    ""
                };
                let reading = if word == 0xFFFF_FFFE {
                    "unset (-2, draw by layer)".to_string()
                } else if high != 0 {
                    format!("unset (0x{word:08X}, high byte set)")
                } else {
                    let rgb = colorref_rgb(word);
                    let known = line_palette.contains(&(word & 0x00FF_FFFF));
                    format!(
                        "{} {}",
                        hexs(rgb),
                        if known {
                            "[in line palette]"
                        } else {
                            "[not a line colour]"
                        }
                    )
                };
                println!(
                    "  0x002A id {:<3} +30 = 0x{word:08X}  {reading}{marker}",
                    record.style_id
                );
            }
        }

        // End to end through the public API a renderer calls: every boundary
        // record's resolved fill, with the colour it now carries.
        if let Ok(index) = pid_parse::style_link::fill_styles_for_file(&path) {
            let coloured = index.values().filter(|fill| fill.colour.is_some()).count();
            println!(
                "  fill_styles_for_file: {} boundary fill(s), {coloured} with a stated colour",
                index.len()
            );
            for ((stream, oid), fill) in &index {
                let colour = match fill.rgb() {
                    Some(rgb) => hexs(rgb),
                    None => "layer default".to_string(),
                };
                println!("    {stream} oid={oid} -> id {} {colour}", fill.style_id);
            }
        }
    }
}
