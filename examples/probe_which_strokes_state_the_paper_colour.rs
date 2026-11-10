//! Which strokes of a symbol, a border template or a drawing state a colour
//! the plot does not print as it stands — the word `0x8000_0000`, or white?
//!
//! `style_link` resolves a stroke's colour to a plain `COLORREF`. Two stated
//! colours are lost on the way to the page:
//!
//! - a `JStyleOverride` whose mask says it states a colour and whose colour
//!   word is `0x8000_0000` keeps its base style's colour instead, so the
//!   stroke resolves black. The eleven `验收数据` reference plots paint every
//!   such stroke in white: the origin marks of `Remarks.sym` and
//!   `Item Note & Label.sym` leave no ink there;
//! - a stroke whose colour is white (`0x00FF_FFFF`) resolves white, and the
//!   plot writer keeps a white pen white. The reference plots print every
//!   one of those black: they are the rules of the `.igr` border templates.
//!
//! This reads each file's own `StyleCluster` bytes and, for every geometry
//! record (`igLine2d`, `igPoint2d`, `igLineString2d`, `igArc2d`, `igCircle2d`)
//! of every sheet, says which of the two its style states. It also counts, in
//! every storage, the `JStyleOverride` records that state `0x8000_0000`
//! whether or not a stroke it walks names them: a drawing keeps its symbol
//! bodies in definition storages whose strokes are not `Sheet*` records, and
//! `Item Note & Label.sym` keeps its cross where this reader does not walk.
//! A directory is walked for `.sym`, `.igr` and `.pid` files.
//!
//! ```text
//! cargo run --example probe_which_strokes_state_the_paper_colour -- <file-or-dir> [...]
//! ```
//!
//! Prints one line per file that carries either kind, then the totals.
//! Read-only.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::style_link::{
    storage_of_sheet, StyleHop, OVERRIDE_COLOUR_OFFSET, OVERRIDE_STATES_COLOUR,
};
use pid_parse::PidParser;

/// The colour word the reference plots paint in the paper's colour.
const PAPER_WORD: u32 = 0x8000_0000;
/// White, as a `COLORREF`.
const WHITE: u32 = 0x00FF_FFFF;
/// Bytes of PSM envelope before a style record's payload.
const ENVELOPE: usize = 6;

/// Strokes of one file by what their style states, and the style records
/// that state it whether or not a stroke this reader walks names them.
#[derive(Default, Clone, Copy)]
struct Tally {
    strokes: usize,
    paper_word: usize,
    white: usize,
    /// `JStyleOverride` records stating a colour of `0x8000_0000`.
    paper_overrides: usize,
    /// The widest of those overrides' stated widths, millimetres.
    paper_override_width_mm: f64,
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            collect(&path, out);
        }
    } else if path.extension().is_some_and(|extension| {
        ["sym", "igr", "pid"]
            .iter()
            .any(|wanted| extension.eq_ignore_ascii_case(wanted))
    }) {
        out.push(path.to_path_buf());
    }
}

/// Tally one file, per sheet stream.
fn tally(path: &Path) -> Result<BTreeMap<String, Tally>, String> {
    let doc = PidParser::new()
        .parse_file(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut compound =
        cfb::CompoundFile::open(file).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut clusters: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut out: BTreeMap<String, Tally> = BTreeMap::new();
    // Every storage's style table, whether or not this reader walks a sheet
    // of it: a drawing keeps its symbol bodies in definition storages whose
    // strokes are not `Sheet*` records, and their overrides are here.
    for (storage, table) in &doc.style_tables {
        let cluster = format!("{}/StyleCluster", storage.trim_end_matches('/'));
        let mut bytes = Vec::new();
        if let Ok(mut stream) = compound.open_stream(&cluster) {
            let _ = stream.read_to_end(&mut bytes);
        }
        let entry = out.entry(format!("{cluster} (style records)")).or_default();
        for record in table.records() {
            if record.override_mask.unwrap_or(0) & OVERRIDE_STATES_COLOUR == 0 {
                continue;
            }
            let at = record.byte_range.start + ENVELOPE + OVERRIDE_COLOUR_OFFSET;
            let Some(word) = bytes.get(at..at + 4) else {
                continue;
            };
            if u32::from_le_bytes([word[0], word[1], word[2], word[3]]) == PAPER_WORD {
                entry.paper_overrides += 1;
                entry.paper_override_width_mm = entry
                    .paper_override_width_mm
                    .max(record.override_width_m.unwrap_or(0.0) * 1000.0);
            }
        }
        clusters.insert(storage.clone(), bytes);
    }
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let storage = storage_of_sheet(&sheet.path);
        let Some(table) = doc.style_tables.get(&storage) else {
            continue;
        };
        let Some(bytes) = clusters.get(&storage) else {
            continue;
        };
        let indices = geometry
            .decoded_iglines
            .iter()
            .map(|record| record.index)
            .chain(geometry.decoded_igpoints.iter().map(|record| record.index))
            .chain(
                geometry
                    .decoded_iglinestrings
                    .iter()
                    .map(|record| record.index),
            )
            .chain(geometry.decoded_igarcs.iter().map(|record| record.index))
            .chain(geometry.decoded_igcircles.iter().map(|record| record.index));
        let entry = out.entry(sheet.path.clone()).or_default();
        for index in indices {
            let Some(resolved) = table.resolve_line_style(index) else {
                continue;
            };
            entry.strokes += 1;
            // The word the override itself states, read off its own bytes:
            // the resolved colour has already fallen back to the base's.
            let stated = match resolved.hop {
                StyleHop::ViaOverride { override_id } => table
                    .get(override_id)
                    .filter(|record| {
                        record.override_mask.unwrap_or(0) & OVERRIDE_STATES_COLOUR != 0
                    })
                    .and_then(|record| {
                        let at = record.byte_range.start + ENVELOPE + OVERRIDE_COLOUR_OFFSET;
                        let word = bytes.get(at..at + 4)?;
                        Some(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
                    }),
                StyleHop::Direct => None,
            };
            if stated == Some(PAPER_WORD) {
                entry.paper_word += 1;
            } else if resolved.symbology.colour == WHITE {
                entry.white += 1;
            }
        }
    }
    Ok(out)
}

fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        collect(Path::new(&arg), &mut files);
    }
    if files.is_empty() {
        eprintln!("usage: probe_which_strokes_state_the_paper_colour <file-or-dir> [...]");
        return ExitCode::from(2);
    }
    let mut total = Tally::default();
    let (mut with_paper, mut with_white, mut with_override, mut unread) =
        (0usize, 0usize, 0usize, 0usize);
    for path in &files {
        let sheets = match tally(path) {
            Ok(sheets) => sheets,
            Err(error) => {
                eprintln!("{error}");
                unread += 1;
                continue;
            }
        };
        let mut file = Tally::default();
        for tally in sheets.values() {
            file.strokes += tally.strokes;
            file.paper_word += tally.paper_word;
            file.white += tally.white;
            file.paper_overrides += tally.paper_overrides;
        }
        total.strokes += file.strokes;
        total.paper_word += file.paper_word;
        total.white += file.white;
        total.paper_overrides += file.paper_overrides;
        with_paper += usize::from(file.paper_word > 0);
        with_white += usize::from(file.white > 0);
        with_override += usize::from(file.paper_overrides > 0);
        if file.paper_word == 0 && file.white == 0 && file.paper_overrides == 0 {
            continue;
        }
        println!(
            "{}: {} styled strokes, {} state 0x80000000, {} are white; {} override record(s) state \
             0x80000000",
            path.display(),
            file.strokes,
            file.paper_word,
            file.white,
            file.paper_overrides
        );
        for (sheet, tally) in &sheets {
            if tally.paper_word > 0 || tally.white > 0 {
                println!(
                    "    {sheet:<34} {:>5} strokes, {:>4} state 0x80000000, {:>4} white",
                    tally.strokes, tally.paper_word, tally.white
                );
            }
            if tally.paper_overrides > 0 {
                println!(
                    "    {sheet:<34} {:>5} override(s) state 0x80000000, stating widths up to {:.3} mm",
                    tally.paper_overrides, tally.paper_override_width_mm
                );
            }
        }
    }
    println!(
        "{} file(s) ({} unreadable): {} styled strokes; {} state 0x80000000 in {} file(s); {} are \
         white in {} file(s); {} override record(s) state 0x80000000 in {} file(s)",
        files.len(),
        unread,
        total.strokes,
        total.paper_word,
        with_paper,
        total.white,
        with_white,
        total.paper_overrides,
        with_override
    );
    ExitCode::SUCCESS
}
