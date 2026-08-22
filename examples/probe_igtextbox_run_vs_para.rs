//! If the character-style run wins over the paragraph default, how much of
//! the drawing changes?
//!
//! t-42 established the on-disk model (`docs/analysis/2026-08-22-igtextbox-
//! tail-kind-2-and-formatting-runs.md`): an `igTextBox` carries `A + B`
//! formatting runs of 8 bytes, `(u16 length, u16 selector, u32 style id)`.
//! Selector 1 names a `JStyleTextChar`; the native sizer refuses a record
//! whose selector-1 lengths do not sum to the character count, so on any
//! record that has runs at all, **every character is covered by one**.
//!
//! The shipped decoder ignores those runs and resolves lettering the other
//! way: `igTextBox +14` names a `JStyleTextPara`, whose `+38` names a
//! `JStyleTextChar`. Both paths end at a character style, and on 153 of 215
//! shape-2 records they end at *different* ones.
//!
//! `Interop.RAD2D.dll` -- Intergraph's own interop assembly -- says which is
//! which. `TextStyle` (the paragraph style) exposes `CharStyle` /
//! `CharStyleName`: that is the `+38` hop, and it is the paragraph's
//! **default** lettering. Per-character lettering lives on `TextBox.Edit`
//! (`TextEdit`), which is range-based: `SetSelect(start, end)` and then
//! `TextSize` / `Color` / `Font` / `Bold` / `SuperScript` over that range,
//! plus `IsPropertyShared(TextPropertyConstants, out bool)` -- an API that
//! only exists because ranges may disagree with the default. So the run is
//! the effective style wherever one covers the text, and the paragraph
//! default only shows through where no run does.
//!
//! This probe measures the consequence, using the library's own resolver for
//! both sides so the comparison is against real decoder behaviour rather than
//! a re-implementation: `resolve_text_height` already accepts either shape --
//! hand it a paragraph id and it hops to `+38`, hand it a character style id
//! and it reads that record directly.
//!
//! ```powershell
//! cargo run --example probe_igtextbox_run_vs_para
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use pid_parse::parsers::sheet_records::{decode_igtextboxes, PSM_ENVELOPE_LEN};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable, ResolvedTextHeight};

/// Payload offset where an `igTextBox` body begins.
const BODY_START: usize = 22;

/// The four fixtures `tests/style_link_ratchet.rs` pins its counts over. Its
/// `155` is the number of distinct `(stream, oid)` entries `text_heights_for_file`
/// builds across exactly these, so this probe keys the same way and reports
/// them as their own aggregate -- otherwise "how many of the 155 change?" has
/// no denominator to be a fraction of.
const RATCHET_CORPUS: &[&str] = &[
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
];

const FIXTURES: &[&str] = &[
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Where the selector-1 runs start and how many there are, per body shape.
///
/// Shape 1 has none. Shape 2 keeps its single run *before* the character
/// count, at the body start. Shape 3 puts `A` of them after the text.
fn selector_one_runs(payload: &[u8], shape: u16) -> Vec<u32> {
    let (at, count) = match shape {
        1 => return Vec::new(),
        2 => (BODY_START, 1usize),
        3 => {
            let Some(a) = u16_at(payload, BODY_START) else {
                return Vec::new();
            };
            let Some(chars) = u16_at(payload, BODY_START + 4) else {
                return Vec::new();
            };
            (BODY_START + 6 + 2 * chars as usize, a as usize)
        }
        _ => return Vec::new(),
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let entry = at + i * 8;
        // Only entries that actually declare selector 1 are character-style
        // runs; anything else is not ours to read.
        if u16_at(payload, entry + 2) != Some(1) {
            continue;
        }
        if let Some(value) = u32_at(payload, entry + 4) {
            out.push(value);
        }
    }
    out
}

/// A height difference below this is float noise, not something anyone sees.
const VISIBLE_HEIGHT_MM: f64 = 1e-4;

/// One typographic point in millimetres.
const POINT_MM: f64 = 25.4 / 72.0;

/// A height rendered as points, when it lands on a half-point -- which is how
/// a text editor states a size, and not how a drafting standard states one.
fn points(height_mm: f64) -> String {
    let pt = height_mm / POINT_MM;
    let half = (pt * 2.0).round() / 2.0;
    if (pt - half).abs() < 0.005 && half > 0.0 {
        format!("{half}pt")
    } else {
        "-".to_owned()
    }
}

/// The three things a switch to run-priority could move on screen.
fn lettering(resolved: &ResolvedTextHeight) -> (u64, Option<u32>, Option<&str>) {
    (
        resolved.height_m.to_bits(),
        resolved.colour,
        resolved.font_name.as_deref(),
    )
}

/// One entry of the index a renderer joins on: what it letters with today,
/// and what the record's own runs say it should letter with.
struct Entry {
    today: ResolvedTextHeight,
    runs: Vec<ResolvedTextHeight>,
    /// The record carries runs, but at least one names a style this document
    /// does not define.
    runs_unresolvable: bool,
}

/// Entries keyed the way `text_heights_for_file` keys them: `(stream, oid)`,
/// and only for records whose lettering resolves today.
type Index = BTreeMap<(String, u32), Entry>;

#[derive(Default)]
struct Tally {
    entries: usize,
    has_run: usize,
    runs_unresolvable: usize,
    no_run_at_all: usize,
    same: usize,
    changes_height: usize,
    changes_height_visibly: usize,
    changes_colour: usize,
    changes_font: usize,
    changes_any: usize,
    lost_alignment: usize,
    mixed_runs: usize,
    height_moves: BTreeMap<String, usize>,
    font_moves: BTreeMap<String, usize>,
}

fn tally(index: &Index) -> Tally {
    let mut t = Tally::default();
    for entry in index.values() {
        t.entries += 1;
        if entry.runs_unresolvable {
            t.runs_unresolvable += 1;
            continue;
        }
        let Some(first) = entry.runs.first() else {
            t.no_run_at_all += 1;
            continue;
        };
        t.has_run += 1;
        // A record whose runs disagree with each other cannot be rendered
        // with one style at all; count it so the flattening stays visible.
        if entry
            .runs
            .windows(2)
            .any(|w| lettering(&w[0]) != lettering(&w[1]))
        {
            t.mixed_runs += 1;
        }
        let today = &entry.today;
        // Alignment and line spacing are paragraph properties. The run path
        // cannot carry them, so a switch that simply swapped resolvers would
        // drop them -- on every run-covered entry, not just the ones whose
        // lettering moves.
        if today.alignment.is_some() && first.alignment.is_none() {
            t.lost_alignment += 1;
        }
        if lettering(today) == lettering(first) {
            t.same += 1;
            continue;
        }
        t.changes_any += 1;
        if today.height_m != first.height_m {
            t.changes_height += 1;
            if (today.height_mm() - first.height_mm()).abs() > VISIBLE_HEIGHT_MM {
                t.changes_height_visibly += 1;
            }
            *t.height_moves
                .entry(format!(
                    "{:.4}mm ({:>5}) -> {:.4}mm ({:>5})",
                    today.height_mm(),
                    points(today.height_mm()),
                    first.height_mm(),
                    points(first.height_mm())
                ))
                .or_default() += 1;
        }
        if today.colour != first.colour {
            t.changes_colour += 1;
        }
        if today.font_name != first.font_name {
            t.changes_font += 1;
            *t.font_moves
                .entry(format!(
                    "{:?} -> {:?}",
                    today.font_name.as_deref().unwrap_or("<none>"),
                    first.font_name.as_deref().unwrap_or("<none>")
                ))
                .or_default() += 1;
        }
    }
    t
}

/// Walk one `.pid`, or say why it could not be walked.
///
/// Returning the error rather than an empty tally is the habit t-42 paid for:
/// a fixture that silently yields nothing looks exactly like a fixture with
/// nothing in it, and the short corpus reads as a complete one.
fn scan(path: &Path) -> Result<Index, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut cfb = cfb::CompoundFile::open(file).map_err(|e| e.to_string())?;
    let sheets: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().is_some_and(|n| n.starts_with("Sheet")))
        .collect();

    let mut index = Index::new();
    for sheet_path in sheets {
        let Some(sheet) = read_stream(&mut cfb, &sheet_path) else {
            continue;
        };
        let style_path = stylecluster_path_for_sheet(&sheet_path);
        let Some(style_bytes) = read_stream(&mut cfb, &style_path) else {
            continue;
        };
        let table = DocumentStyleTable::from_stylecluster_bytes(&style_bytes);

        for record in decode_igtextboxes(&sheet) {
            // Mirror `text_heights_for_file`: only records whose lettering
            // resolves reach the index, and a later record on the same
            // (stream, oid) overwrites an earlier one.
            let Some(today) = table.resolve_text_height(record.index) else {
                continue;
            };
            let payload_start = record.byte_range.start + PSM_ENVELOPE_LEN;
            let payload = sheet.get(payload_start..record.byte_range.end);
            let ids = payload
                .map(|p| selector_one_runs(p, record.text_sub_type))
                .unwrap_or_default();
            let runs: Vec<ResolvedTextHeight> = ids
                .iter()
                .filter_map(|id| table.resolve_text_height(*id))
                .collect();
            index.insert(
                (sheet_path.clone(), record.oid),
                Entry {
                    today,
                    runs_unresolvable: runs.len() != ids.len(),
                    runs,
                },
            );
        }
    }
    Ok(index)
}

fn read_stream<R: std::io::Read + std::io::Seek>(
    cfb: &mut cfb::CompoundFile<R>,
    path: &str,
) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

fn report_one(label: &str, t: &Tally) {
    println!("\n--- {label} ---");
    println!("  index entries, (stream, oid)       : {}", t.entries);
    println!("    of which carry a resolvable run  : {}", t.has_run);
    println!(
        "    run names an undefined style     : {}",
        t.runs_unresolvable
    );
    println!("    no run at all (paragraph rules)  : {}", t.no_run_at_all);
    println!("  identical either way               : {}", t.same);
    println!("  WOULD CHANGE on screen             : {}", t.changes_any);
    println!(
        "    height                           : {} ({} of them visibly)",
        t.changes_height, t.changes_height_visibly
    );
    println!(
        "    colour                           : {}",
        t.changes_colour
    );
    println!("    typeface                         : {}", t.changes_font);
    println!(
        "  would lose paragraph alignment     : {}",
        t.lost_alignment
    );
    println!("  runs disagree within one label     : {}", t.mixed_runs);
}

fn main() {
    let mut unreadable: Vec<(&str, String)> = Vec::new();
    let mut per_fixture: Vec<(&str, Index)> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            unreadable.push((fixture, "not on disk".to_owned()));
            continue;
        }
        match scan(path) {
            Ok(index) => per_fixture.push((fixture, index)),
            Err(why) => unreadable.push((fixture, why)),
        }
    }

    println!("=== igTextBox lettering: the run, or the paragraph default? ===");
    for (fixture, why) in &unreadable {
        println!("  UNREAD  {fixture}: {why}");
    }
    if !unreadable.is_empty() {
        println!(
            "\n  PARTIAL: {} fixture(s) unreadable. Every count below is short by\n\
             whatever they hold -- do not quote them as corpus-wide.",
            unreadable.len()
        );
    }

    let mut ratchet = Index::new();
    let mut everything = Index::new();
    for (fixture, index) in &per_fixture {
        for (key, entry) in index {
            let scoped = (format!("{fixture}!{}", key.0), key.1);
            if RATCHET_CORPUS.contains(fixture) {
                ratchet.insert(
                    scoped.clone(),
                    Entry {
                        today: entry.today.clone(),
                        runs: entry.runs.clone(),
                        runs_unresolvable: entry.runs_unresolvable,
                    },
                );
            }
            everything.insert(
                scoped,
                Entry {
                    today: entry.today.clone(),
                    runs: entry.runs.clone(),
                    runs_unresolvable: entry.runs_unresolvable,
                },
            );
        }
    }

    let ratchet_tally = tally(&ratchet);
    report_one(
        "ratchet corpus (the 155 the style tests pin)",
        &ratchet_tally,
    );
    report_one("all six fixtures", &tally(&everything));
    for (fixture, index) in &per_fixture {
        report_one(fixture, &tally(index));
    }

    println!("\n--- where the height moves (ratchet corpus) ---");
    for (move_, n) in &ratchet_tally.height_moves {
        println!("  {n:>4} x {move_}");
    }
    println!("\n--- where the typeface moves (ratchet corpus) ---");
    for (move_, n) in &ratchet_tally.font_moves {
        println!("  {n:>4} x {move_}");
    }
    let total = ratchet_tally;

    println!("\n=== reading ===");
    println!(
        "Every record that carries a run has every one of its characters covered\n\
         by one (the native sizer refuses otherwise), so on those records the\n\
         paragraph default never reaches the glyphs. The count above is what the\n\
         drawing currently gets wrong, not an upper bound on what could change."
    );
    if total.lost_alignment > 0 || total.mixed_runs > 0 {
        println!(
            "\nTwo things a naive switch would break: alignment and line spacing are\n\
             paragraph properties and must keep coming from the `+14` hop, and\n\
             {} label(s) carry runs that disagree with each other, which no single\n\
             resolved style can represent.",
            total.mixed_runs
        );
    }
}
