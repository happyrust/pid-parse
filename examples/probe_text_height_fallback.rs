//! Why a `.pid`'s lettering falls back to a default height instead of the one
//! the drawing states.
//!
//! Phase 39's plan ranks the text-height residue first and blames the unread
//! `version 2` path of `0x002C JStyleTextChar`
//! (`docs/analysis/2026-08-04-jstyletextchar-native-reader-confirmed.md` §5.1).
//! That blame is a hypothesis: the version dispatch was read out of
//! `style.dll`, but no version word has ever been read off a real record, so
//! nobody knows whether this corpus contains a version-2 one at all. Two cheap
//! measurements settle it before any disassembly:
//!
//! * **Where the version word is.** The format guide already fixes families to
//!   a version by native reader — `0x002C` and `0x002D` serialise at version 3,
//!   `0x002E` at version 2 — so a payload column reading 3 on the first two and
//!   2 on the third is the version, located by contrast rather than by guess.
//!   A column that fails the contrast is not it.
//! * **What the fallback bucket is made of.** Every `igTextBox` is resolved the
//!   way `style_link` resolves it, and each miss is attributed to the hop that
//!   failed instead of being counted as one lump.
//!
//! ```powershell
//! cargo run --example probe_text_height_fallback
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::decode_igtextboxes;
use pid_parse::style_link::{
    stylecluster_path_for_sheet, DocumentStyleTable, StyleRecord, PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE,
    PSM_TYPE_CODE_JSTYLE_TEXT_CHAR, PSM_TYPE_CODE_JSTYLE_TEXT_PARA, TEXT_CHAR_HEIGHT_OFFSET,
};

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    // The gongyi fixture, escaped so this source stays pure ASCII.
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

const PSM_ENVELOPE_LEN: usize = 6;
const TEXT_CHAR: u16 = PSM_TYPE_CODE_JSTYLE_TEXT_CHAR;
const TEXT_PARA: u16 = PSM_TYPE_CODE_JSTYLE_TEXT_PARA;
const SIMPLE_LINE: u16 = PSM_TYPE_CODE_JSTYLE_SIMPLE_LINE;

/// Offset `style_link` reads the character height from.
const HEIGHT_OFFSET: usize = TEXT_CHAR_HEIGHT_OFFSET;

/// How far into a payload the version word could plausibly sit. The base block
/// is 26 bytes on `0x002C` and holds the style id at `+14`, so a version
/// written before the class fields is inside this window.
const HEAD_LEN: usize = 14;

/// Families whose serialisation version the native reader has already fixed,
/// which is what makes the contrast test falsifiable.
const KNOWN_VERSIONS: &[(u16, &str, u32)] = &[
    (TEXT_CHAR, "JStyleTextChar", 3),
    (TEXT_PARA, "JStyleTextPara", 3),
    (SIMPLE_LINE, "JStyleSimpleLine", 2),
];

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    let s = data.get(at..at.checked_add(8)?)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// The payload of one style record, sliced back out of its own stream.
fn payload_of<'a>(cluster: &'a [u8], record: &StyleRecord) -> &'a [u8] {
    let start = record.byte_range.start + PSM_ENVELOPE_LEN;
    cluster.get(start..record.byte_range.end).unwrap_or(&[])
}

/// Every `(stream path, bytes)` pair in a `.pid` whose name ends in `name`.
fn streams_named(path: &Path, name: &str) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|stream| {
            stream
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with(name))
        })
        .collect();
    let mut out = Vec::new();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(stream_path.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((stream_path, bytes));
        }
    }
    out
}

/// Distinct values of each of the first [`HEAD_LEN`] payload bytes, per family.
type HeadColumns = BTreeMap<u16, Vec<BTreeSet<u8>>>;

fn collect_head_columns(fixtures: &[PathBuf], columns: &mut HeadColumns, counts: &mut Counts) {
    for fixture in fixtures {
        for (_, cluster) in streams_named(fixture, "StyleCluster") {
            let table = DocumentStyleTable::from_stylecluster_bytes(&cluster);
            for record in table.records() {
                let payload = payload_of(&cluster, record);
                if payload.len() < HEAD_LEN {
                    continue;
                }
                *counts.entry(record.type_code).or_default() += 1;
                let head = columns
                    .entry(record.type_code)
                    .or_insert_with(|| vec![BTreeSet::new(); HEAD_LEN]);
                for (column, byte) in head.iter_mut().zip(&payload[..HEAD_LEN]) {
                    column.insert(*byte);
                }
            }
        }
    }
}

type Counts = BTreeMap<u16, usize>;

/// Report which payload column, if any, reads as the serialisation version.
///
/// A candidate has to be constant within each of the three families whose
/// version is known *and* carry that family's version as its value. One column
/// passing and the rest failing is what makes this a location rather than a
/// coincidence.
fn report_version_column(columns: &HeadColumns, counts: &Counts) -> Option<usize> {
    println!("\n=== A. where the version word sits ===\n");
    for (type_code, name, version) in KNOWN_VERSIONS {
        println!(
            "  0x{type_code:04X} {name:<17} {} record(s), native-reader version {version}",
            counts.get(type_code).copied().unwrap_or_default()
        );
    }

    let mut passing = Vec::new();
    println!("\n  column  0x002C   0x002D   0x002E   verdict");
    for at in 0..HEAD_LEN {
        let seen: Vec<Option<&BTreeSet<u8>>> = KNOWN_VERSIONS
            .iter()
            .map(|(type_code, _, _)| columns.get(type_code).and_then(|head| head.get(at)))
            .collect();
        let cells: Vec<String> = seen
            .iter()
            .map(|column| match column {
                Some(values) if values.len() <= 3 => values
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join("/"),
                Some(values) => format!("{} vals", values.len()),
                None => "-".to_string(),
            })
            .collect();
        let matches_version = seen
            .iter()
            .zip(KNOWN_VERSIONS)
            .all(|(column, (_, _, version))| {
                column.is_some_and(|values| {
                    values.len() == 1 && values.contains(&u8::try_from(*version).unwrap_or(0))
                })
            });
        if matches_version {
            passing.push(at);
        }
        println!(
            "  +{at:<6}{:<9}{:<9}{:<9}{}",
            cells[0],
            cells[1],
            cells[2],
            if matches_version {
                "carries each family's known version"
            } else {
                ""
            }
        );
    }

    match passing.as_slice() {
        [] => {
            println!(
                "\n  No column carries the version. Either it is not written per record,\n  \
                 or it sits past +{HEAD_LEN}."
            );
            None
        }
        [only] => {
            println!("\n  +{only} is the version word: it is the only column that fits.");
            Some(*only)
        }
        many => {
            println!("\n  {many:?} all fit; the contrast does not single one out.");
            None
        }
    }
}

/// Which hop of the two-hop text lookup failed, or the height it produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Resolved,
    /// The `index` names no record in this document's style table.
    ParaUndefined,
    /// The paragraph style's `+38` names no character style.
    ParaNamesNothing,
    /// It names one this document does not define.
    CharUndefined,
    /// The `index` names a family that is not a text style at all.
    NotATextStyle,
    /// The character style is there and the height field is refused.
    HeightRefused,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::ParaUndefined => "index names no record",
            Self::ParaNamesNothing => "paragraph names no character style",
            Self::CharUndefined => "character style undefined here",
            Self::NotATextStyle => "index names a non-text family",
            Self::HeightRefused => "height field refused",
        }
    }
}

/// One record whose height was refused, with the bytes behind the refusal.
struct Refusal {
    char_style_id: u32,
    payload_len: usize,
    raw_height_mm: f64,
    version: Option<u8>,
    sample_text: String,
}

fn classify(
    table: &DocumentStyleTable,
    cluster: &[u8],
    index: u32,
    version_at: Option<usize>,
    text: &str,
) -> (Outcome, Option<Refusal>) {
    let Some(named) = table.get(index) else {
        return (Outcome::ParaUndefined, None);
    };
    let char_style = match named.type_code {
        TEXT_PARA => {
            let Some(reference) = named.text_char_reference else {
                return (Outcome::ParaNamesNothing, None);
            };
            match table.get(reference) {
                Some(record) => record,
                None => return (Outcome::CharUndefined, None),
            }
        }
        TEXT_CHAR => named,
        _ => return (Outcome::NotATextStyle, None),
    };
    if char_style.char_height_m.is_some() {
        return (Outcome::Resolved, None);
    }
    let payload = payload_of(cluster, char_style);
    let refusal = Refusal {
        char_style_id: char_style.style_id,
        payload_len: payload.len(),
        raw_height_mm: f64_at(payload, HEIGHT_OFFSET).unwrap_or(f64::NAN) * 1000.0,
        version: version_at.and_then(|at| payload.get(at).copied()),
        sample_text: text.chars().take(16).collect(),
    };
    (Outcome::HeightRefused, Some(refusal))
}

fn report_fallbacks(fixtures: &[PathBuf], version_at: Option<usize>) {
    println!("\n=== B. what the fallback bucket is made of ===");

    let mut corpus: BTreeMap<Outcome, usize> = BTreeMap::new();
    let mut refusals: Vec<(String, Refusal)> = Vec::new();
    let mut heights: BTreeMap<String, usize> = BTreeMap::new();
    let mut char_versions: BTreeMap<u8, usize> = BTreeMap::new();

    for fixture in fixtures {
        let label = fixture
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut per_file: BTreeMap<Outcome, usize> = BTreeMap::new();
        let mut per_file_heights: BTreeMap<String, usize> = BTreeMap::new();
        let clusters: BTreeMap<String, Vec<u8>> =
            streams_named(fixture, "StyleCluster").into_iter().collect();

        for (sheet_path, sheet) in streams_named(fixture, "Sheet") {
            let Some(cluster) = clusters.get(&stylecluster_path_for_sheet(&sheet_path)) else {
                continue;
            };
            let table = DocumentStyleTable::from_stylecluster_bytes(cluster);
            if table.is_empty() {
                continue;
            }
            if let Some(at) = version_at {
                for record in table.records().iter().filter(|r| r.type_code == TEXT_CHAR) {
                    if let Some(version) = payload_of(cluster, record).get(at) {
                        *char_versions.entry(*version).or_default() += 1;
                    }
                }
            }
            for record in decode_igtextboxes(&sheet) {
                let (outcome, refusal) =
                    classify(&table, cluster, record.index, version_at, &record.text);
                *per_file.entry(outcome).or_default() += 1;
                *corpus.entry(outcome).or_default() += 1;
                if let Some(refusal) = refusal {
                    refusals.push((label.clone(), refusal));
                }
                if let Some(resolved) = table.resolve_text_height(record.index) {
                    let height = format!("{:.3}", resolved.height_mm());
                    *per_file_heights.entry(height.clone()).or_default() += 1;
                    *heights.entry(height).or_default() += 1;
                }
            }
        }

        let total: usize = per_file.values().sum();
        if total == 0 {
            continue;
        }
        println!("\n  -- {label}: {total} text record(s) --");
        for (outcome, count) in &per_file {
            println!("     {count:>4}  {}", outcome.label());
        }
        if !per_file_heights.is_empty() {
            let stated: Vec<String> = per_file_heights
                .iter()
                .map(|(height, count)| format!("{height}x{count}"))
                .collect();
            println!("     heights the drawing states: {}", stated.join("  "));
        }
    }

    let total: usize = corpus.values().sum();
    println!("\n  -- corpus: {total} text record(s) --");
    for (outcome, count) in &corpus {
        println!("     {count:>4}  {}", outcome.label());
    }

    if !char_versions.is_empty() {
        println!("\n  0x002C records by version word:");
        for (version, count) in &char_versions {
            println!("     version {version}: {count} record(s)");
        }
    }

    println!("\n  resolved heights (mm x records):");
    let mut by_height: Vec<(&String, &usize)> = heights.iter().collect();
    by_height.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (height, count) in by_height {
        println!("     {height:>8} x {count}");
    }

    if refusals.is_empty() {
        println!("\n  No text record reaches a character style whose height is refused.");
        return;
    }
    println!("\n  refused heights, one line per text record:");
    for (label, refusal) in &refusals {
        println!(
            "     {label:<28} char style id={:<5} payload={:>3}B version={:<4} +{HEIGHT_OFFSET} reads {:.4}mm  text={:?}",
            refusal.char_style_id,
            refusal.payload_len,
            refusal
                .version
                .map_or_else(|| "?".to_string(), |v| v.to_string()),
            refusal.raw_height_mm,
            refusal.sample_text
        );
    }
}

/// Put a refused character style beside a working one of the same length, from
/// the same document.
///
/// If the refusal were a second record layout — a different serialisation
/// version being the obvious candidate — the two would disagree structurally:
/// different length, or a wholesale different pattern of populated bytes. If
/// they agree everywhere but in the height field itself, the record is the same
/// shape and `0.254 mm` is what the drawing stores, not what a misread produces.
fn report_refused_bytes(fixtures: &[PathBuf]) {
    println!("\n=== C. a refused character style beside a working one ===");

    let mut refused_payloads: Vec<(String, Vec<u8>)> = Vec::new();
    for fixture in fixtures {
        let label = fixture
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (cluster_path, cluster) in streams_named(fixture, "StyleCluster") {
            let table = DocumentStyleTable::from_stylecluster_bytes(&cluster);
            let chars: Vec<&StyleRecord> = table
                .records()
                .iter()
                .filter(|record| record.type_code == TEXT_CHAR)
                .collect();
            let Some(refused) = chars
                .iter()
                .find(|record| record.char_height_m.is_none())
                .copied()
            else {
                continue;
            };
            let refused_bytes = payload_of(&cluster, refused);
            refused_payloads.push((label.clone(), refused_bytes.to_vec()));
            let Some(working) = chars
                .iter()
                .find(|record| {
                    record.char_height_m.is_some()
                        && payload_of(&cluster, record).len() == refused_bytes.len()
                })
                .copied()
            else {
                println!(
                    "\n  {label} {cluster_path}: refused id={} is the only {}-byte record; nothing to compare",
                    refused.style_id,
                    refused_bytes.len()
                );
                continue;
            };
            let working_bytes = payload_of(&cluster, working);

            println!(
                "\n  {label} {cluster_path}, {} bytes each",
                refused_bytes.len()
            );
            println!(
                "    refused id={:<4} height reads {:.4}mm",
                refused.style_id,
                f64_at(refused_bytes, HEIGHT_OFFSET).unwrap_or(f64::NAN) * 1000.0
            );
            println!(
                "    working id={:<4} height reads {:.4}mm",
                working.style_id,
                working.char_height_m.unwrap_or(f64::NAN) * 1000.0
            );
            let differing: Vec<usize> = (0..refused_bytes.len())
                .filter(|at| refused_bytes.get(*at) != working_bytes.get(*at))
                .collect();
            let outside: Vec<usize> = differing
                .iter()
                .copied()
                .filter(|at| !(HEIGHT_OFFSET..HEIGHT_OFFSET + 8).contains(at))
                .collect();
            println!(
                "    {} byte(s) differ; {} of them outside the height field: {outside:?}",
                differing.len(),
                outside.len()
            );
            println!("    refused: {}", hex(refused_bytes));
            println!("    working: {}", hex(working_bytes));
        }
    }

    // Do the refused records from unrelated drawings agree byte for byte once
    // their identity is set aside? If they do, they are one style copied out of
    // shared reference data rather than three drafters making the same mistake.
    let Some((first_label, first)) = refused_payloads.first() else {
        return;
    };
    let identity = |at: usize| (0..2).contains(&at) || (14..18).contains(&at);
    let mut disagreements: Vec<String> = Vec::new();
    for (label, payload) in refused_payloads.iter().skip(1) {
        let differing: Vec<usize> = (0..first.len().max(payload.len()))
            .filter(|at| !identity(*at) && first.get(*at) != payload.get(*at))
            .collect();
        if !differing.is_empty() {
            disagreements.push(format!(
                "  {label} differs from {first_label} at {differing:?}"
            ));
        }
    }
    println!(
        "\n  {} refused record(s) compared against {first_label}'s, identity fields excluded:",
        refused_payloads.len()
    );
    if disagreements.is_empty() {
        println!("    all byte-identical — one style, copied into each drawing");
    } else {
        for line in &disagreements {
            println!("{line}");
        }
    }
}

/// Does the one field that separates a refused record from a working one
/// separate them everywhere?
///
/// Section C singles out payload `+26` — the first class-specific field of
/// `JStyleTextChar`, `this+396` in the native reader's write order. This
/// cross-tabulates it against height plausibility over every `0x002C` record in
/// the corpus, so a coincidence on three records cannot pass for a rule, and
/// asks whether its value names anything else in the same document.
fn report_first_class_field(fixtures: &[PathBuf]) {
    const FIRST_CLASS_FIELD: usize = 26;
    println!("\n=== D. payload +{FIRST_CLASS_FIELD} against height plausibility ===\n");

    let mut table_rows: BTreeMap<(u32, bool), usize> = BTreeMap::new();
    let mut naming: Vec<String> = Vec::new();

    for fixture in fixtures {
        let label = fixture
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (cluster_path, cluster) in streams_named(fixture, "StyleCluster") {
            let table = DocumentStyleTable::from_stylecluster_bytes(&cluster);
            // Every handle in this document, so a reference can be looked up.
            let handles: BTreeMap<u32, (u16, u32)> = table
                .records()
                .iter()
                .filter_map(|record| {
                    let payload = payload_of(&cluster, record);
                    let handle =
                        u32::from(u16::from_le_bytes([*payload.first()?, *payload.get(1)?]));
                    Some((handle, (record.type_code, record.style_id)))
                })
                .collect();

            for record in table.records() {
                if record.type_code != TEXT_CHAR {
                    continue;
                }
                let payload = payload_of(&cluster, record);
                let Some(slice) = payload.get(FIRST_CLASS_FIELD..FIRST_CLASS_FIELD + 4) else {
                    continue;
                };
                let value = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
                *table_rows
                    .entry((value, record.char_height_m.is_some()))
                    .or_default() += 1;
                if value == 0 {
                    continue;
                }
                let names = handles.get(&value).map_or_else(
                    || "names no record in this document".to_string(),
                    |(type_code, style_id)| format!("names 0x{type_code:04X} style id {style_id}"),
                );
                naming.push(format!(
                    "  {label} {cluster_path}: id {} carries 0x{value:04X}, which {names}",
                    record.style_id
                ));
            }
        }
    }

    println!("  +{FIRST_CLASS_FIELD} value   height plausible   records");
    for ((value, plausible), count) in &table_rows {
        println!(
            "  0x{value:08X}   {:<18} {count}",
            if *plausible { "yes" } else { "no" }
        );
    }
    if !naming.is_empty() {
        println!("\n  what a non-zero value names:");
        for line in &naming {
            println!("{line}");
        }
    }
}

/// Is the height these records need stored anywhere else?
///
/// `0.254 mm` is `0.01"`, which no drawing letters at, so either the paragraph
/// style holds a height of its own or the text record carries a scale that
/// turns 0.254 into something legible. Both are cheap to falsify: scan the
/// paragraph payload for an `f64` in the drafting range, and read the text
/// record's own third trailing double, the one the decoder notes as "often 1.0
/// — scale or marker". A field that answers has to separate refused from
/// resolved; a field present on both in the same shape explains nothing.
fn report_alternative_height_sources(fixtures: &[PathBuf]) {
    println!("\n=== E. is the missing height stored anywhere else? ===\n");

    // offset -> (in-range on a refused paragraph, in-range on a resolved one)
    let mut para_windows: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    let mut scales: BTreeMap<(bool, String), usize> = BTreeMap::new();

    for fixture in fixtures {
        let clusters: BTreeMap<String, Vec<u8>> =
            streams_named(fixture, "StyleCluster").into_iter().collect();
        for (sheet_path, sheet) in streams_named(fixture, "Sheet") {
            let Some(cluster) = clusters.get(&stylecluster_path_for_sheet(&sheet_path)) else {
                continue;
            };
            let table = DocumentStyleTable::from_stylecluster_bytes(cluster);
            if table.is_empty() {
                continue;
            }
            for record in decode_igtextboxes(&sheet) {
                let resolved = table.resolve_text_height(record.index).is_some();
                let (outcome, _) = classify(&table, cluster, record.index, None, &record.text);
                if outcome != Outcome::Resolved && outcome != Outcome::HeightRefused {
                    continue;
                }
                *scales
                    .entry((resolved, format!("{:.4}", record.trailing_double_3)))
                    .or_default() += 1;
                let Some(para) = table.get(record.index) else {
                    continue;
                };
                if para.type_code != TEXT_PARA {
                    continue;
                }
                let payload = payload_of(cluster, para);
                for at in 0..payload.len().saturating_sub(7) {
                    let Some(value) = f64_at(payload, at) else {
                        continue;
                    };
                    if !value.is_normal() || !(0.000_5..=0.02).contains(&value) {
                        continue;
                    }
                    let slot = para_windows.entry(at).or_default();
                    if resolved {
                        slot.1 += 1;
                    } else {
                        slot.0 += 1;
                    }
                }
            }
        }
    }

    println!("  text record's third trailing double, by outcome:");
    for ((resolved, value), count) in &scales {
        println!(
            "     {:<9} {value:>10} x {count}",
            if *resolved { "resolved" } else { "refused" }
        );
    }

    println!("\n  f64 windows in the drafting range inside the paragraph style:");
    if para_windows.is_empty() {
        println!("     none at any offset, on either outcome");
    }
    for (at, (refused, resolved)) in &para_windows {
        println!("     +{at:<3} refused {refused}, resolved {resolved}");
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let root = test_file_root();
    let fixtures: Vec<PathBuf> = FIXTURES
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect();
    if fixtures.is_empty() {
        println!("no fixtures under {}; nothing to probe", root.display());
        return;
    }

    let mut columns = HeadColumns::new();
    let mut counts = Counts::new();
    collect_head_columns(&fixtures, &mut columns, &mut counts);
    let version_at = report_version_column(&columns, &counts);
    report_fallbacks(&fixtures, version_at);
    report_refused_bytes(&fixtures);
    report_first_class_field(&fixtures);
    report_alternative_height_sources(&fixtures);
}
