//! Which class do the instruments without a `SymbolAttr` row get, if their
//! symbol's library path is allowed to say?
//!
//! Plan `docs/plans/2026-10-10-publish-from-the-pid-alone.md`, Q2 (b) small
//! ticket opened 2026-10-10 after P5: 68 of the corpus's 287 instruments
//! (`ModelItemType = Instrument`) carry no `SymbolAttr` row on their graphic,
//! so `pid_load` cannot split them into `PIDInstrument` /
//! `PIDControlSystemFunction` / `PIDInlineInstrument` and leaves them out.
//! The candidate the 10-10 analysis named is the placement's symbol library
//! path. This probe measures it, read-only, on the five drawings of
//! `D:\work\cad\PID自研软件验证` (or `PID_PARSE_SELF_CHECK_DIR`):
//!
//! * every instrument item (its `P&IDAttributes` `Symbol` rows, `ModelID`),
//!   whether any of its graphics has a `SymbolAttr` row, the library path of
//!   each graphic that is an `igSymbol2d` placement, and the class the
//!   reference `_Data.xml` publishes it under;
//! * per distinct library path: how many instruments, with / without
//!   `SymbolAttr`, and the reference classes -- a path is usable as a rule
//!   only when it names one class;
//! * the folder chain of the path (every directory segment) tabulated
//!   against the reference class, to see which segment decides;
//! * a dry run of the candidate rule (see [`class_by_path`]) over all 287:
//!   right / wrong / undecided, for the 68 and for the 219 separately.
//!
//! Nothing is written. `cargo run --example probe_instruments_without_symbol_attr`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::attribute_rows::{decode_attribute_row, AttributeRow};
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};
use quick_xml::events::Event;
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;

/// The candidate rule, written before scoring: a directory segment of the
/// library path (case-insensitive) decides the class.
fn class_by_path(path: &str) -> Option<&'static str> {
    let segments: Vec<String> = path
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let (_, dirs) = segments.split_last()?;
    let has = |needle: &str| dirs.iter().any(|dir| dir == needle);
    if has("system functions") {
        Some("PIDControlSystemFunction")
    } else if has("in-line instruments") || has("inline instruments") || has("in-line") {
        Some("PIDInlineInstrument")
    } else if has("off-line instruments") || has("offline instruments") {
        Some("PIDInstrument")
    } else {
        None
    }
}

fn corpus() -> Vec<(String, PathBuf, PathBuf)> {
    let corpus =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&corpus)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();
    drawings
}

/// `IObject/@UID` -> class of every top-level block of a `_Data.xml`.
fn reference_classes(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut classes = HashMap::new();
    let mut depth = 0usize;
    let mut block: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    block = Some(String::from_utf8_lossy(start.name().as_ref()).into_owned());
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 && start.name().as_ref() == b"IObject" {
                    if let (Some(class), Some(uid)) = (
                        block.as_ref(),
                        start.attributes().flatten().find_map(|a| {
                            (a.key.as_ref() == b"UID")
                                .then(|| String::from_utf8_lossy(&a.value).into_owned())
                        }),
                    ) {
                        classes.insert(uid, class.clone());
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    block = None;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    classes
}

/// Every decoded `0x0089` row of the root storage's record chains, by oid.
fn attribute_rows(path: &Path) -> BTreeMap<u32, AttributeRow> {
    let mut rows = BTreeMap::new();
    let Ok(file) = std::fs::File::open(path) else {
        return rows;
    };
    let Ok(mut compound) = cfb::CompoundFile::open(file) else {
        return rows;
    };
    let streams: Vec<PathBuf> = compound
        .read_root_storage()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .collect();
    for stream_path in streams {
        let mut data = Vec::new();
        let Ok(mut stream) = compound.open_stream(&stream_path) else {
            continue;
        };
        if stream.read_to_end(&mut data).is_err()
            || data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..])
        {
            continue;
        }
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            if header.type_code != FREE_FORM_ATTR_SET {
                continue;
            }
            let Some(payload) = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .and_then(|end| data.get(header.body_start..end))
            else {
                continue;
            };
            if let Some(row) = decode_attribute_row(payload) {
                rows.insert(row.oid, row);
            }
        }
    }
    rows
}

struct Instrument {
    uid: String,
    graphics: Vec<u32>,
    symbol_attr: Option<(String, String)>,
    paths: Vec<String>,
    reference_class: Option<String>,
}

fn main() {
    let drawings = corpus();
    if drawings.is_empty() {
        println!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    }
    // path -> (with SymbolAttr, without) -> reference class -> count
    let mut by_path: BTreeMap<String, BTreeMap<bool, BTreeMap<String, usize>>> = BTreeMap::new();
    // directory segment -> reference class -> count (instruments without SymbolAttr)
    let mut by_segment: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut verdict: BTreeMap<(bool, &str), usize> = BTreeMap::new();
    let mut wrong_lines = Vec::new();
    let mut undecided_lines = Vec::new();
    let mut total = 0usize;

    for (stem, pid, reference) in &drawings {
        let Ok(parsed) = PidParser::with_options(ParseOptions::geometry()).parse_file(pid) else {
            println!("skip: {stem} did not parse");
            continue;
        };
        let geometry = build_normalized_geometry(&parsed);
        let mut path_of: HashMap<u32, String> = HashMap::new();
        for entity in &geometry.entities {
            if let (
                PidGraphicKind::SymbolInstance {
                    symbol_path: Some(path),
                    ..
                },
                Some(oid),
            ) = (&entity.kind, entity.graphic_oid)
            {
                path_of.insert(oid, path.clone());
            }
        }
        let classes = reference_classes(&std::fs::read_to_string(reference).unwrap_or_default());
        let rows = attribute_rows(pid);
        let mut symbol_attr: HashMap<u32, (String, String)> = HashMap::new();
        for row in rows.values() {
            if row.class == "SymbolAttr" {
                symbol_attr.insert(
                    row.described,
                    (
                        row.text("Type").unwrap_or_default().to_string(),
                        row.text("Class").unwrap_or_default().to_string(),
                    ),
                );
            }
        }
        let mut instruments: BTreeMap<String, Instrument> = BTreeMap::new();
        for row in rows.values() {
            if row.class != "P&IDAttributes"
                || !matches!(row.text("DrawingItemType"), Some("Symbol" | "Connector"))
                || row.text("ModelItemType") != Some("Instrument")
            {
                continue;
            }
            let Some(uid) = row.text("ModelID").filter(|m| m.len() > 1) else {
                continue;
            };
            let instrument = instruments
                .entry(uid.to_string())
                .or_insert_with(|| Instrument {
                    uid: uid.to_string(),
                    graphics: Vec::new(),
                    symbol_attr: None,
                    paths: Vec::new(),
                    reference_class: classes.get(uid).cloned(),
                });
            instrument.graphics.push(row.described);
            if let Some(attr) = symbol_attr.get(&row.described) {
                instrument.symbol_attr = Some(attr.clone());
            }
            if let Some(path) = path_of.get(&row.described) {
                if !instrument.paths.contains(path) {
                    instrument.paths.push(path.clone());
                }
            }
        }

        let without: Vec<&Instrument> = instruments
            .values()
            .filter(|i| i.symbol_attr.is_none())
            .collect();
        println!(
            "\n==== {stem}: {} instruments, {} without SymbolAttr, {} of those with a library path, {} graphics without one",
            instruments.len(),
            without.len(),
            without.iter().filter(|i| !i.paths.is_empty()).count(),
            without.iter().filter(|i| i.paths.is_empty()).count(),
        );
        for instrument in without {
            println!(
                "  {} ref={:<26} graphics={:?} path={}",
                instrument.uid,
                instrument
                    .reference_class
                    .as_deref()
                    .unwrap_or("(not published)"),
                instrument.graphics,
                if instrument.paths.is_empty() {
                    "-".to_string()
                } else {
                    instrument.paths.join(" | ")
                }
            );
        }

        for instrument in instruments.values() {
            total += 1;
            let reference = instrument
                .reference_class
                .clone()
                .unwrap_or_else(|| "(not published)".to_string());
            let with = instrument.symbol_attr.is_some();
            let key = if instrument.paths.is_empty() {
                "(no igSymbol2d path)".to_string()
            } else {
                instrument.paths.join(" | ")
            };
            *by_path
                .entry(key)
                .or_default()
                .entry(with)
                .or_default()
                .entry(reference.clone())
                .or_insert(0) += 1;
            if !with {
                let mut seen = BTreeSet::new();
                for path in &instrument.paths {
                    let segments: Vec<&str> =
                        path.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
                    if let Some((_, dirs)) = segments.split_last() {
                        for dir in dirs {
                            if seen.insert(dir.to_ascii_lowercase()) {
                                *by_segment
                                    .entry(dir.to_ascii_lowercase())
                                    .or_default()
                                    .entry(reference.clone())
                                    .or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
            // Dry run of the rule.
            let decided: BTreeSet<&str> = instrument
                .paths
                .iter()
                .filter_map(|p| class_by_path(p))
                .collect();
            let outcome = match decided.iter().next() {
                _ if decided.len() > 1 => "wrong",
                Some(class) if Some(*class) == instrument.reference_class.as_deref() => "right",
                Some(_) => "wrong",
                None => "undecided",
            };
            *verdict.entry((with, outcome)).or_insert(0) += 1;
            if outcome == "wrong" {
                wrong_lines.push(format!(
                    "  {stem}: {} ref={reference} rule={decided:?} paths={:?}",
                    instrument.uid, instrument.paths
                ));
            } else if outcome == "undecided" && !with {
                undecided_lines.push(format!(
                    "  {stem}: {} ref={reference} paths={:?}",
                    instrument.uid, instrument.paths
                ));
            }
        }
    }

    println!("\n==== per library path (all {total} instruments): with SymbolAttr / without -> reference class");
    for (path, by_with) in &by_path {
        let fmt = |m: Option<&BTreeMap<String, usize>>| {
            m.map(|m| {
                m.iter()
                    .map(|(c, n)| format!("{c} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
        };
        println!(
            "  {path}\n      with: {}\n      without: {}",
            fmt(by_with.get(&true)),
            fmt(by_with.get(&false))
        );
    }

    println!("\n==== per directory segment (instruments without SymbolAttr) -> reference class");
    for (segment, classes) in &by_segment {
        let line = classes
            .iter()
            .map(|(c, n)| format!("{c} {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {segment:<40} {line}");
    }

    println!(
        "\n==== dry run of class_by_path over all {total}: (has SymbolAttr, outcome) -> count"
    );
    for ((with, outcome), n) in &verdict {
        println!(
            "  {:<18} {outcome:<10} {n}",
            if *with { "with SymbolAttr" } else { "without" }
        );
    }
    if !wrong_lines.is_empty() {
        println!("  wrong:");
        for line in &wrong_lines {
            println!("{line}");
        }
    }
    if !undecided_lines.is_empty() {
        println!("  undecided (without SymbolAttr):");
        for line in &undecided_lines {
            println!("{line}");
        }
    }
}
