//! What does a symbol file keep its body in, and what decides which of its
//! sheets is drawn?
//!
//! A `.sym` holds `Sheet*` streams, a `PSMcluster0` with its layer table
//! and view filter sets, and a `StyleCluster`. The symbol reader
//! (`pid_parse::symbol_library`) draws the sheets the view sets make the
//! body's normal views and, where none of them shows a stroke, the sheet
//! that keeps the most. This probe reads every symbol file in reach — on
//! disk and in the `RefData~*.zip` archives beside them — for what that
//! choice rests on:
//!
//! * whether each of those streams is a record chain that holds — the
//!   magic, a walk to the last byte, the header's count of live records —
//!   the root `PSMcluster0` included, which the cluster walker enters 171
//!   bytes in and the symbol reader reads as a plain chain;
//! * whether the layer table and the view sets read off the chain's own
//!   record starts are the ones the cluster walker reads;
//! * how many sheets a file has, how many of them are normal views, and
//!   which of the reader's ways the body is picked by;
//! * the records the native reader skips (`0x8000` in the type word) on a
//!   symbol's sheets, counted directly from the stream bytes (since
//!   2026-11-14 the symbol reader also skips `0x8000` into
//!   `SymbolGeometry::retired_records`, matching the drawing reader);
//! * whether any of those streams holds no byte, and whether a file has no
//!   `PSMcluster0` in its root: the two things the chain walk has no word
//!   for and the symbol reader takes for a stream read short (since
//!   2026-11-13) — and with them, whether the body of any file lists a
//!   fault, was read short or is not drawn.
//!
//! Read on 2026-11-12 over `test-file` on a machine where
//! `test-file/symbols-full` is the full reference library: 1 845 symbol
//! files (624 on disk, 1 221 in twelve archives), none unread. Every one of
//! 3 697 sheet streams (4 of them in nested storages), 1 845 root and 122
//! nested `PSMcluster0` and 1 967 `StyleCluster` is a chain that holds; the layer tables and the view sets
//! read off the chain are the walker's on every file. A file has one sheet
//! (3), two (1 832) or three (10); no normal view (567), one (1 201) or two
//! (77). The body is one normal view on 961 files and several on 14, the
//! sheet that keeps the most strokes on 729, the only sheet on 126, the
//! whole body on 6, and 9 files hold nothing to draw; 314 sheets are a
//! header and nothing else. 1 106 records on symbol sheets are superseded
//! copies, 660 of them of a type the symbol reader draws.
//!
//! So a symbol file's choice of sheet rests on two streams — the sheet and
//! the root cluster — and on a count of strokes for nearly half the files,
//! which is what `SymbolGeometry::displayed` has to be careful with when
//! either is read short (`tests/sym_file_chain_faults.rs`).
//!
//! Read again on 2026-11-13 for the last of those, over every root this
//! machine has: `test-file` (1 845 symbol files, as above), both
//! `pid-test-data` (1 932 each: 1 288 on disk, 644 in six archives) and the
//! share (644) — 6 353 files, a good many of them the same file kept in
//! several places. No chain stream of a symbol file holds no byte, every
//! file keeps a `PSMcluster0` in its root, and no body lists a fault, was
//! read short or is not drawn — so reading an emptied stream and a missing
//! root cluster as damage costs no sound file.
//!
//! Run: `cargo run --example probe_what_decides_the_sheet_a_symbol_file_draws
//! [-- <root> … [--verbose]]`. Exits non-zero when a stream is not a chain
//! that holds, holds no byte, a table read off the chain is not the
//! walker's, a file has no root cluster, or a body is not a sound file's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_layers::{decode_sheet_layers, decode_sheet_layers_on_chain};
use pid_parse::parsers::sheet_records::{parse_psm_header, SheetChain};
use pid_parse::parsers::view_filter_sets::{
    decode_view_filter_sets, decode_view_filter_sets_on_chain,
};
use pid_parse::symbol_library::read_symbol_geometry_from_bytes;
use pid_parse::PidPackage;

/// The type codes the symbol reader draws a record of.
const DRAWN_TYPES: [u16; 9] = [0x18, 0x59, 0x61, 0x84, 0x4D, 0x5D, 0x13, 0x63, 0x7E];

/// The layer names a symbol keeps off the sheet, as the symbol reader spells
/// them: lower case, no white space.
const SUPPLEMENTARY: [&str; 7] = [
    "heattrace",
    "hiddenobjects",
    "jacket",
    "label",
    "construction",
    "dimension",
    "deminsion",
];

/// Whether a view set displays a supplementary layer, which makes its sheet
/// an alternate view and not a normal one — the reader's own rule, which it
/// keeps to itself.
fn shows_supplementary(display: &[u8], layers: &[(u16, String)]) -> bool {
    layers.iter().any(|(number, name)| {
        let name: String = name
            .chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect();
        SUPPLEMENTARY.contains(&name.as_str())
            && display
                .get(usize::from(number / 8))
                .is_some_and(|byte| byte & (1 << (number % 8)) != 0)
    })
}

/// The `.sym` files and the `RefData~*.zip` archives under `dir`.
fn symbol_files(dir: &Path, files: &mut Vec<PathBuf>, archives: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            symbol_files(&path, files, archives);
            continue;
        }
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("sym") {
            files.push(path);
        } else if extension.eq_ignore_ascii_case("zip") && name.starts_with("RefData~") {
            archives.push(path);
        }
    }
}

/// What the files of one root read as.
#[derive(Default)]
struct Census {
    files: usize,
    unread: usize,
    /// A stream's leaf name with its digits taken out, `<nested>/` in front
    /// of one inside a storage -> (streams, chains that hold, that do not).
    streams: BTreeMap<String, [usize; 3]>,
    tables_not_the_walkers: usize,
    nested_sheets: usize,
    header_only_sheets: usize,
    sheets_per_file: BTreeMap<usize, usize>,
    normal_per_file: BTreeMap<usize, usize>,
    /// How the body is picked -> files.
    pick: BTreeMap<&'static str, usize>,
    superseded: usize,
    superseded_of_a_drawn_type: usize,
    /// Chain streams that hold no byte: not the eight of a header.
    streams_with_no_byte: usize,
    /// Files with no `PSMcluster0` in their root.
    no_root_cluster: usize,
    /// Files whose body lists a fault, was read short or is not drawn.
    bodies_not_sound: usize,
}

impl Census {
    /// The streams whose chain does not hold.
    fn chains_that_do_not_hold(&self) -> usize {
        self.streams.values().map(|counts| counts[2]).sum()
    }
}

fn hold(label: &str, bytes: &[u8], census: &mut Census, verbose: bool) {
    let (Ok(package), Ok(body)) = (
        PidPackage::from_bytes(bytes),
        read_symbol_geometry_from_bytes(bytes),
    ) else {
        census.unread += 1;
        return;
    };
    census.files += 1;
    let mut sheets = 0usize;
    for (path, raw) in &package.streams {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let leaf = parts.last().copied().unwrap_or_default();
        let is_sheet = leaf.starts_with("Sheet");
        if !is_sheet && leaf != "PSMcluster0" && leaf != "StyleCluster" {
            continue;
        }
        let nested = parts.len() > 1;
        let data = &raw.data;
        let chain = SheetChain::walk(data);
        let key = format!(
            "{}{}",
            if nested { "<nested>/" } else { "/" },
            leaf.chars()
                .filter(|c| !c.is_ascii_digit())
                .collect::<String>()
        );
        if data.is_empty() {
            census.streams_with_no_byte += 1;
            println!("  a chain stream with no byte: {label} {path}");
        }
        let entry = census.streams.entry(key).or_default();
        entry[0] += 1;
        if chain.is_sound() {
            entry[1] += 1;
        } else {
            entry[2] += 1;
            println!(
                "  a chain that does not hold: {label} {path}: stop {:?}, end {} of {}, \
                 header {:?}, live {}",
                chain.stop, chain.end, chain.len, chain.stated_records, chain.live_records
            );
        }
        if is_sheet {
            sheets += 1;
            census.nested_sheets += usize::from(nested);
            census.header_only_sheets += usize::from(data.len() == 8);
            for &at in &chain.starts {
                let Some(header) = parse_psm_header(data, at) else {
                    continue;
                };
                if header.native_reader_skips() {
                    census.superseded += 1;
                    if DRAWN_TYPES.contains(&header.type_code) {
                        census.superseded_of_a_drawn_type += 1;
                        if verbose {
                            println!(
                                "  a superseded record of a drawn type: {label} {path} +{at} \
                                 ({:#06x})",
                                header.type_code
                            );
                        }
                    }
                }
            }
        }
        if leaf == "PSMcluster0"
            && (decode_sheet_layers_on_chain(data) != decode_sheet_layers(data)
                || decode_view_filter_sets_on_chain(data) != decode_view_filter_sets(data))
        {
            census.tables_not_the_walkers += 1;
            println!("  tables off the chain are not the walker's: {label} {path}");
        }
    }
    *census.sheets_per_file.entry(sheets).or_default() += 1;
    if !package.streams.contains_key("/PSMcluster0") {
        census.no_root_cluster += 1;
        println!("  no PSMcluster0 in the root: {label}");
    }
    if !body.chain_faults.is_empty() || !body.short.is_empty() || body.displayed().is_err() {
        census.bodies_not_sound += 1;
        println!(
            "  a body that is not a sound file's: {label}: {} fault(s), short {:?}, {:?}",
            body.chain_faults.len(),
            body.short,
            body.displayed().err()
        );
    }

    // The pick, read off the body.
    let displayed = body.displayed_primitives();
    let drawn_from: BTreeSet<u32> = displayed.iter().map(|p| p.sheet_ref).collect();
    let present: BTreeSet<u32> = body
        .primitives
        .iter()
        .map(|p| p.sheet_ref)
        .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
        .collect();
    let normal: BTreeSet<u32> = body
        .sheet_views
        .iter()
        .filter(|view| !shows_supplementary(&view.display, &view.layers))
        .map(|view| view.sheet_ref)
        .collect();
    *census.normal_per_file.entry(normal.len()).or_default() += 1;
    let pick = if body.primitives.is_empty() {
        "nothing to draw"
    } else if displayed.len() == body.primitives.len() && drawn_from.len() > 1 {
        "the whole body"
    } else if !drawn_from.is_empty() && drawn_from.iter().all(|sheet| normal.contains(sheet)) {
        if drawn_from.len() > 1 {
            "several normal views"
        } else {
            "one normal view"
        }
    } else if drawn_from.len() == 1 && present.len() == 1 {
        "the only sheet"
    } else if drawn_from.len() == 1 {
        "the sheet that keeps the most strokes"
    } else {
        "another way"
    };
    *census.pick.entry(pick).or_default() += 1;
    if verbose && (pick == "another way" || pick == "the whole body") {
        println!(
            "  {pick}: {label}: sheets {present:?}, normal {normal:?}, drawn from \
             {drawn_from:?}, {} of {} primitive(s)",
            displayed.len(),
            body.primitives.len()
        );
    }
}

/// The `.sym` members of one archive, each read.
#[cfg(feature = "refdata-archive")]
fn hold_archive(archive: &Path, census: &mut Census, verbose: bool) {
    let Ok(members) = pid_parse::refdata_archive::archive_members(archive, "sym") else {
        return;
    };
    for member in members {
        if let Ok(bytes) = pid_parse::refdata_archive::read_archive_file(&member) {
            hold(
                &member.label().display().to_string(),
                &bytes,
                census,
                verbose,
            );
        }
    }
}

/// No archive is read in a build without the archive reader.
#[cfg(not(feature = "refdata-archive"))]
fn hold_archive(_archive: &Path, _census: &mut Census, _verbose: bool) {}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "--verbose");
    let mut roots: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .collect();
    if roots.is_empty() {
        roots.push("test-file");
    }
    let mut wrong = 0usize;
    for root in roots {
        let mut files = Vec::new();
        let mut archives = Vec::new();
        symbol_files(Path::new(root), &mut files, &mut archives);
        let mut census = Census::default();
        for file in &files {
            if let Ok(bytes) = std::fs::read(file) {
                hold(&file.display().to_string(), &bytes, &mut census, verbose);
            }
        }
        let on_disk = census.files;
        for archive in &archives {
            hold_archive(archive, &mut census, verbose);
        }
        println!(
            "== {root}: {} symbol files ({on_disk} on disk, {} in {} archive(s)), {} unread",
            census.files,
            census.files - on_disk,
            archives.len(),
            census.unread
        );
        println!("streams: how many, chains that hold, chains that do not");
        for (name, counts) in &census.streams {
            println!(
                "  {name:24} {:6} {:6} {:6}",
                counts[0], counts[1], counts[2]
            );
        }
        println!(
            "PSMcluster0 whose tables off the chain are not the walker's: {}",
            census.tables_not_the_walkers
        );
        println!(
            "sheets in nested storages: {}; sheets that are a header and nothing else: {}",
            census.nested_sheets, census.header_only_sheets
        );
        println!("sheets a file: {:?}", census.sheets_per_file);
        println!("normal views a file: {:?}", census.normal_per_file);
        println!("the body is: {:?}", census.pick);
        println!(
            "superseded records on sheets: {}, of a type the symbol reader draws: {}",
            census.superseded, census.superseded_of_a_drawn_type
        );
        println!(
            "chain streams with no byte: {}; files with no PSMcluster0 in the root: {}; bodies that \
             list a fault, were read short or are not drawn: {}",
            census.streams_with_no_byte, census.no_root_cluster, census.bodies_not_sound
        );
        wrong += census.chains_that_do_not_hold()
            + census.tables_not_the_walkers
            + census.streams_with_no_byte
            + census.no_root_cluster
            + census.bodies_not_sound;
    }
    if wrong != 0 {
        eprintln!("{wrong} stream(s) or file(s) are not as the symbol reader takes them");
        std::process::exit(1);
    }
}
