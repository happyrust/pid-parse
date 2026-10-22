//! What shape 3's `A` and `B` really are, measured on the **library** corpus.
//!
//! `igTextBox` shape 3 stores `A` then `B` u16s at `+22` / `+24`, and the
//! drawing side reads them as counts of 8-byte formatting runs
//! (`docs/analysis/2026-08-22-igtextbox-tail-kind-2-and-formatting-runs.md`:
//! `A` selector-1 character styles, then `B` selector-2 restatements of the
//! record's own paragraph style). The symbol library never read runs at all,
//! so the claim has only ever been checked against `.pid` drawings.
//!
//! This probe checks the same invariants on every `0x004D` record in every
//! `.sym` `Sheet*` stream, and asks whether the run style ids resolve in the
//! symbol's own `StyleCluster` — which is what a library that honoured runs
//! would need.
//!
//! ```text
//! cargo run --example probe_sym_text_runs -- [--root DIR]... [--samples N]
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_parse::style_link::{
    DocumentStyleTable, ResolvedTextHeight, PSM_TYPE_CODE_JSTYLE_TEXT_CHAR,
    PSM_TYPE_CODE_JSTYLE_TEXT_PARA,
};
use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const TYPE_TEXTBOX: u16 = 0x004D;

/// Discriminator `igTextBox` derives from the run count.
const SHAPE_AT: usize = 18;
/// One formatting run on disk: `(u16 len, u16 selector, u32 style_id)`.
const RUN_LEN: usize = 8;
/// The record's own paragraph style id, which a selector-2 run restates.
const PARAGRAPH_ID_AT: usize = 14;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let ref_dir = corpus.join("Ref");
    if ref_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ref_dir) {
            let mut found: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| {
                    entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("symbol")
                })
                .map(|entry| entry.path())
                .collect();
            found.sort();
            roots.extend(found);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    roots
}

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

/// One formatting run as stored.
#[derive(Clone, Copy)]
struct Run {
    len: u16,
    selector: u16,
    style_id: u32,
}

impl Run {
    fn read(payload: &[u8], at: usize) -> Option<Self> {
        Some(Self {
            len: u16_le(payload, at)?,
            selector: u16_le(payload, at + 2)?,
            style_id: u32_le(payload, at + 4)?,
        })
    }
}

/// The runs a record stores, and the length its text claims: `(count, runs)`.
///
/// Shape 1 stores none; shape 2 stores its single entry at `+22`, ahead of the
/// count; shape 3 stores `A` selector-1 runs then `B` selector-2 runs after the
/// text.
fn runs_of(payload: &[u8]) -> Option<(u16, Vec<Run>)> {
    let shape = u16_le(payload, SHAPE_AT)?;
    match shape {
        1 => Some((u16_le(payload, 22)?, Vec::new())),
        2 => {
            let count = u16_le(payload, 30)?;
            Some((count, vec![Run::read(payload, 22)?]))
        }
        3 => {
            let a = usize::from(u16_le(payload, 22)?);
            let b = usize::from(u16_le(payload, 24)?);
            let count = u16_le(payload, 26)?;
            let first = 28 + 2 * usize::from(count);
            let mut runs = Vec::with_capacity(a + b);
            for index in 0..a + b {
                runs.push(Run::read(payload, first + RUN_LEN * index)?);
            }
            Some((count, runs))
        }
        _ => None,
    }
}

#[derive(Default)]
struct Tally {
    records: usize,
    by_shape: BTreeMap<u16, usize>,
    runs: usize,
    selectors: BTreeMap<u16, usize>,
    /// How many records each invariant breaks.
    selector_order_broken: usize,
    coverage_broken: usize,
    /// Selector-1 style ids that name a `JStyleTextChar`, a `JStyleTextPara`,
    /// something else, or nothing this document defines.
    sel1_resolution: BTreeMap<&'static str, usize>,
    sel2_resolution: BTreeMap<&'static str, usize>,
    /// Selector-2 runs whose id equals the record's own `+14`.
    sel2_restates_paragraph: usize,
    sel2_seen: usize,
    /// Records with more than one selector-1 run — mixed styling the library
    /// cannot express today.
    mixed_styling: usize,
    /// Text length covered by selector-1 runs, summed.
    covered: usize,
    chars: usize,
    /// Shape 3's two counts, as stored.
    a_hist: BTreeMap<usize, usize>,
    b_hist: BTreeMap<usize, usize>,
    zero_len_runs: usize,
    style_zero_runs: usize,
    zero_len_records: usize,
    /// Records sitting in a storage with no `StyleCluster` at all.
    no_table_records: usize,
    /// Selector-2 runs that name a different paragraph than the record's `+14`.
    sel2_elsewhere: Vec<String>,
    /// Print every record of the files given, not just the tally.
    dump: bool,
    /// What the library draws with (the `+14` paragraph hop) against what the
    /// record's runs state.
    compared: usize,
    base_missing: usize,
    height_drift: usize,
    font_drift: usize,
    colour_drift: usize,
    sel2_height_drift: usize,
    /// Records the library now draws from a run, and how many of those differ
    /// from the paragraph-only read it made before.
    drawn_from_run: usize,
    drawn_compared: usize,
    drawn_height_differs: usize,
    drawn_font_differs: usize,
    drawn_colour_differs: usize,
    /// Height histograms in micrometres: the paragraph route, and what the
    /// library draws.
    para_heights: BTreeMap<u64, usize>,
    drawn_heights: BTreeMap<u64, usize>,
    /// Heights that land on an exact half-point, the text editor's own grid.
    para_half_point: usize,
    drawn_half_point: usize,
    /// Records whose selector-1 runs disagree within themselves: how many
    /// differ in height, typeface, colour, and how many at all.
    spans_multiple: usize,
    span_height_conflict: usize,
    span_font_conflict: usize,
    span_colour_conflict: usize,
    /// One sample per conflicting record, bounded.
    span_samples: Vec<String>,
    samples: Vec<String>,
}

fn main() {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut sample_count = 12usize;
    let mut dump = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                if let Some(dir) = args.next() {
                    roots.push(PathBuf::from(dir));
                }
            }
            "--dump" => dump = true,
            "--samples" => {
                sample_count = args.next().and_then(|n| n.parse().ok()).unwrap_or(12);
            }
            other => roots.push(PathBuf::from(other)),
        }
    }
    if roots.is_empty() {
        roots = library_roots(&corpus_dir());
    }
    if roots.is_empty() {
        println!("no symbol library on this machine; nothing to measure");
        return;
    }

    let mut syms = Vec::new();
    for root in &roots {
        collect_syms(root, &mut syms);
    }
    syms.sort();
    syms.dedup();
    println!("{} .sym files under {} root(s)", syms.len(), roots.len());

    let mut tally = Tally {
        dump,
        ..Tally::default()
    };
    for sym in &syms {
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            continue;
        };
        let name = sym
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // Styles are scoped to the storage a sheet sits in, the same rule the
        // library reader applies.
        let mut tables: BTreeMap<&str, DocumentStyleTable> = BTreeMap::new();
        for (stream_path, raw) in &pkg.streams {
            if stream_path.rsplit('/').next() == Some("StyleCluster") {
                let parent = stream_path.rfind('/').map_or("", |cut| &stream_path[..cut]);
                tables.insert(
                    parent,
                    DocumentStyleTable::from_stylecluster_bytes(&raw.data),
                );
            }
        }
        for (path, raw) in &pkg.streams {
            let Some(stream_name) = path.rsplit('/').next() else {
                continue;
            };
            if !stream_name.starts_with("Sheet") {
                continue;
            }
            let bytes = &raw.data;
            if bytes.len() <= CHAIN_START || u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            let parent = path.rfind('/').map_or("", |cut| &path[..cut]);
            let table = tables.get(parent);
            let mut at = CHAIN_START;
            while at + PSM_ENVELOPE_LEN <= bytes.len() {
                let (Some(raw_type), Some(btf)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
                    break;
                };
                if raw_type == 0 {
                    break;
                }
                let body = at + PSM_ENVELOPE_LEN;
                let Some(end) = body.checked_add(btf as usize) else {
                    break;
                };
                if end > bytes.len() {
                    break;
                }
                if dump {
                    let shape = bytes
                        .get(body + SHAPE_AT..body + SHAPE_AT + 2)
                        .map_or(-1, |b| i32::from(u16::from_le_bytes([b[0], b[1]])));
                    println!(
                        "  REC {name} {path} +{at} type=0x{raw_type:04X} len={btf} shape={shape}"
                    );
                }
                if raw_type & 0x0FFF == TYPE_TEXTBOX {
                    let payload = &bytes[body..end];
                    examine(&mut tally, &name, stream_name, payload, table, sample_count);
                }
                at = end;
            }
        }
    }

    report(&tally);
}

fn examine(
    tally: &mut Tally,
    name: &str,
    stream: &str,
    payload: &[u8],
    table: Option<&DocumentStyleTable>,
    sample_count: usize,
) {
    tally.records += 1;
    let shape = u16_le(payload, SHAPE_AT).unwrap_or(u16::MAX);
    *tally.by_shape.entry(shape).or_default() += 1;
    let Some((count, runs)) = runs_of(payload) else {
        return;
    };
    tally.chars += usize::from(count);
    if table.is_none() {
        tally.no_table_records += 1;
    }
    if shape == 3 {
        let a = usize::from(u16_le(payload, 22).unwrap_or(0));
        let b = usize::from(u16_le(payload, 24).unwrap_or(0));
        *tally.a_hist.entry(a).or_default() += 1;
        *tally.b_hist.entry(b).or_default() += 1;
    }
    let mut has_zero_len = false;
    for run in &runs {
        if run.len == 0 {
            tally.zero_len_runs += 1;
            has_zero_len = true;
        }
        if run.style_id == 0 {
            tally.style_zero_runs += 1;
        }
    }
    tally.zero_len_records += usize::from(has_zero_len);
    if !runs.is_empty() {
        tally.runs += runs.len();
        let selector_one: usize = runs.iter().filter(|run| run.selector == 1).count();
        if selector_one > 1 {
            tally.mixed_styling += 1;
        }
        for run in &runs {
            *tally.selectors.entry(run.selector).or_default() += 1;
        }
        // On disk the entries are written A selector-1s then B selector-2s.
        let ordered = runs
            .iter()
            .enumerate()
            .all(|(index, run)| run.selector == if index < selector_one { 1 } else { 2 });
        if !ordered {
            tally.selector_order_broken += 1;
        }
        let covered: usize = runs
            .iter()
            .filter(|run| run.selector == 1)
            .map(|run| usize::from(run.len))
            .sum();
        tally.covered += covered;
        if covered != usize::from(count) {
            tally.coverage_broken += 1;
        }
        for run in runs.iter().filter(|run| run.selector == 1) {
            let kind = table.map_or("no table", |table| match table.get(run.style_id) {
                Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_CHAR => {
                    "JStyleTextChar"
                }
                Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_PARA => {
                    "JStyleTextPara"
                }
                Some(_) => "other record",
                None => "not defined",
            });
            *tally.sel1_resolution.entry(kind).or_default() += 1;
        }
        let paragraph = u32_le(payload, PARAGRAPH_ID_AT);
        if tally.dump {
            let paragraph_style =
                table.and_then(|table| paragraph.and_then(|id| table.resolve_text_height(id)));
            let runs_shown: Vec<String> = runs
                .iter()
                .map(|run| {
                    let resolved = table.and_then(|table| table.resolve_run_style(run.style_id));
                    format!(
                        "{}x{}#{} -> {} {} mm",
                        run.len,
                        run.selector,
                        run.style_id,
                        resolved
                            .as_ref()
                            .and_then(|style| style.font_name.clone())
                            .unwrap_or_else(|| "-".to_owned()),
                        resolved
                            .as_ref()
                            .map_or(f64::NAN, |style| style.height_m * 1e3)
                    )
                })
                .collect();
            println!(
                "{name} [{stream}] shape={shape} count={count} +14={:?} -> {} {} mm | runs: {}",
                paragraph,
                paragraph_style
                    .as_ref()
                    .and_then(|style| style.font_name.clone())
                    .unwrap_or_else(|| "-".to_owned()),
                paragraph_style
                    .as_ref()
                    .map_or(f64::NAN, |style| style.height_m * 1e3),
                runs_shown.join("  ")
            );
        }
        for run in runs.iter().filter(|run| run.selector == 2) {
            tally.sel2_seen += 1;
            if Some(run.style_id) == paragraph {
                tally.sel2_restates_paragraph += 1;
            } else if tally.sel2_elsewhere.len() < 30 {
                let kind = |id: Option<u32>| match (table, id) {
                    (Some(table), Some(id)) => match table.get(id) {
                        Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_PARA => {
                            "JStyleTextPara".to_owned()
                        }
                        Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_CHAR => {
                            "JStyleTextChar".to_owned()
                        }
                        Some(record) => format!("record 0x{:04X}", record.type_code),
                        None => "not defined".to_owned(),
                    },
                    (Some(_), None) => "absent".to_owned(),
                    (None, _) => "no table".to_owned(),
                };
                tally.sel2_elsewhere.push(format!(
                    "{name} [{stream}] +14={:?} ({}) run={} ({})",
                    paragraph,
                    kind(paragraph),
                    run.style_id,
                    kind(Some(run.style_id))
                ));
            }
            let kind = table.map_or("no table", |table| match table.get(run.style_id) {
                Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_CHAR => {
                    "JStyleTextChar"
                }
                Some(record) if record.type_code == PSM_TYPE_CODE_JSTYLE_TEXT_PARA => {
                    "JStyleTextPara"
                }
                Some(_) => "other record",
                None => "not defined",
            });
            *tally.sel2_resolution.entry(kind).or_default() += 1;
        }

        // What the library draws with **now** — the widest selector-1 run,
        // the paragraph only where there is no run to follow — against the
        // paragraph-only route it read before.
        if let Some(table) = table {
            let paragraph = u32_le(payload, PARAGRAPH_ID_AT);
            let base = paragraph.and_then(|id| table.resolve_text_height(id));
            let widest = widest_run_style(table, &runs, count);
            let drawn = widest.clone().or_else(|| base.clone());
            if widest.is_some() {
                tally.drawn_from_run += 1;
            }
            if let Some(base) = &base {
                if let Some(drawn) = &drawn {
                    tally.drawn_compared += 1;
                    if (drawn.height_m - base.height_m).abs() > 1e-9 {
                        tally.drawn_height_differs += 1;
                    }
                    if drawn.font_name != base.font_name {
                        tally.drawn_font_differs += 1;
                    }
                    if drawn.colour != base.colour {
                        tally.drawn_colour_differs += 1;
                    }
                }
            }
            let bucket = |style: &ResolvedTextHeight| (style.height_m * 1e6).round() as u64;
            let half_point = |style: &ResolvedTextHeight| {
                let points = style.height_m * 1e3 * 72.0 / 25.4;
                ((points * 2.0) - (points * 2.0).round()).abs() < 1e-3
            };
            if let Some(base) = &base {
                *tally.para_heights.entry(bucket(base)).or_default() += 1;
                tally.para_half_point += usize::from(half_point(base));
            }
            if let Some(drawn) = &drawn {
                *tally.drawn_heights.entry(bucket(drawn)).or_default() += 1;
                tally.drawn_half_point += usize::from(half_point(drawn));
            }
            // Would drawing each run on its own change anything on paper?
            // Ink is monochrome here, so a colour-only split does not show.
            let selector_one: Vec<(u16, ResolvedTextHeight)> = runs
                .iter()
                .filter(|run| run.selector == 1)
                .filter_map(|run| {
                    table
                        .resolve_run_style(run.style_id)
                        .map(|style| (run.len, style))
                })
                .collect();
            if selector_one.len() > 1 {
                tally.spans_multiple += 1;
                let heights: BTreeSet<u64> = selector_one
                    .iter()
                    .map(|(_, style)| style.height_m.to_bits())
                    .collect();
                let fonts: BTreeSet<Option<&str>> = selector_one
                    .iter()
                    .map(|(_, style)| style.font_name.as_deref())
                    .collect();
                let colours: BTreeSet<Option<u32>> =
                    selector_one.iter().map(|(_, style)| style.colour).collect();
                if heights.len() > 1 {
                    tally.span_height_conflict += 1;
                }
                if fonts.len() > 1 {
                    tally.span_font_conflict += 1;
                }
                if colours.len() > 1 {
                    tally.span_colour_conflict += 1;
                }
                if (heights.len() > 1 || fonts.len() > 1) && tally.span_samples.len() < 15 {
                    let shown: Vec<String> = selector_one
                        .iter()
                        .map(|(len, style)| {
                            format!(
                                "{len}x {} {:.3}mm",
                                style.font_name.clone().unwrap_or_else(|| "-".to_owned()),
                                style.height_m * 1e3
                            )
                        })
                        .collect();
                    tally
                        .span_samples
                        .push(format!("{name} [{stream}] {}", shown.join(" | ")));
                }
            }

            // Any run disagreeing with the paragraph at all, which is what the
            // drawing side counted before it wired the runs.
            if let Some(base) = &base {
                tally.compared += 1;
                let mut drift = (false, false, false);
                for run in runs.iter().filter(|run| run.selector == 1) {
                    let Some(resolved) = table.resolve_run_style(run.style_id) else {
                        continue;
                    };
                    if (resolved.height_m - base.height_m).abs() > 1e-9 {
                        drift.0 = true;
                    }
                    if resolved.font_name != base.font_name {
                        drift.1 = true;
                    }
                    if resolved.colour != base.colour {
                        drift.2 = true;
                    }
                }
                tally.height_drift += usize::from(drift.0);
                tally.font_drift += usize::from(drift.1);
                tally.colour_drift += usize::from(drift.2);
            }
            if let Some(base) = &base {
                for run in runs.iter().filter(|run| run.selector == 2) {
                    let Some(other) = table.resolve_text_height(run.style_id) else {
                        continue;
                    };
                    if (other.height_m - base.height_m).abs() > 1e-9 {
                        tally.sel2_height_drift += 1;
                    }
                }
            }
        }
    }

    if tally.samples.len() < sample_count && runs.len() > 1 {
        let rendered: Vec<String> = runs
            .iter()
            .map(|run| format!("{}x{}#{}", run.len, run.selector, run.style_id))
            .collect();
        tally.samples.push(format!(
            "{name} [{stream}] shape={shape} count={count} runs=[{}]",
            rendered.join(" ")
        ));
    }
}

/// The lettering the library draws a record with now: the selector-1 run
/// covering the most characters, ties to the one the label starts in, and only
/// where every run resolves and the lengths add up to the character count —
/// the same three conditions `style_link::resolve_text_style` applies.
fn widest_run_style(
    table: &DocumentStyleTable,
    runs: &[Run],
    count: u16,
) -> Option<ResolvedTextHeight> {
    let selector_one: Vec<&Run> = runs.iter().filter(|run| run.selector == 1).collect();
    if selector_one.is_empty()
        || selector_one
            .iter()
            .map(|run| usize::from(run.len))
            .sum::<usize>()
            != usize::from(count)
    {
        return None;
    }
    let resolved: Vec<(usize, ResolvedTextHeight)> = selector_one
        .iter()
        .map(|run| {
            table
                .resolve_run_style(run.style_id)
                .map(|style| (usize::from(run.len), style))
        })
        .collect::<Option<_>>()?;
    let mut coverage: Vec<(usize, ResolvedTextHeight)> = Vec::new();
    for (chars, style) in resolved {
        match coverage
            .iter_mut()
            .find(|(_, seen)| seen.style_id == style.style_id)
        {
            Some((total, _)) => *total += chars,
            None => coverage.push((chars, style)),
        }
    }
    coverage
        .into_iter()
        .fold(
            None::<(usize, ResolvedTextHeight)>,
            |best, entry| match best {
                Some(best) if best.0 >= entry.0 => Some(best),
                _ => Some(entry),
            },
        )
        .map(|(_, style)| style)
}

fn report(tally: &Tally) {
    println!("\n== 0x004D records on the library's Sheet* streams ==");
    println!("  records {}", tally.records);
    for (shape, count) in &tally.by_shape {
        println!("  shape {shape:>3} x{count}");
    }
    println!(
        "  formatting runs {}  selectors {:?}",
        tally.runs, tally.selectors
    );

    println!("\n== the invariants the drawing side states ==");
    println!(
        "  A selector-1 runs written before B selector-2: {} records break it",
        tally.selector_order_broken
    );
    println!(
        "  selector-1 lengths sum to the character count:  {} records break it \
         ({} chars covered of {})",
        tally.coverage_broken, tally.covered, tally.chars
    );
    println!(
        "  selector-2 restates the record's own +14:        {}/{}",
        tally.sel2_restates_paragraph, tally.sel2_seen
    );

    println!("\n== do the run style ids resolve in the symbol's own StyleCluster? ==");
    println!("  selector 1 {:?}", tally.sel1_resolution);
    println!("  selector 2 {:?}", tally.sel2_resolution);

    println!("\n== shape 3's two counts, as stored ==");
    println!("  A {:?}", tally.a_hist);
    println!("  B {:?}", tally.b_hist);

    println!("\n== details ==");
    println!(
        "  zero-length runs {} (in {} records)  runs naming style 0 {}  records with no StyleCluster {}",
        tally.zero_len_runs, tally.zero_len_records, tally.style_zero_runs, tally.no_table_records
    );
    println!(
        "  records whose text carries more than one selector-1 run: {}",
        tally.mixed_styling
    );

    println!("\n== what the library draws with (+14) against the runs ==");
    println!(
        "  compared {} records ({} with no resolvable +14)",
        tally.compared, tally.base_missing
    );
    println!(
        "  runs state a different height {}  font {}  colour {}",
        tally.height_drift, tally.font_drift, tally.colour_drift
    );
    println!(
        "  selector-2 paragraph resolves to a different height: {}",
        tally.sel2_height_drift
    );

    println!(
        "
== what the library draws now: widest run, paragraph as the default =="
    );
    println!(
        "  records lettered by a run {} of {}; compared with the paragraph route {}",
        tally.drawn_from_run, tally.records, tally.drawn_compared
    );
    println!(
        "  drawn height differs {}  font {}  colour {}",
        tally.drawn_height_differs, tally.drawn_font_differs, tally.drawn_colour_differs
    );
    println!(
        "  heights on an exact half-point: drawn {}/{}  paragraph {}/{}",
        tally.drawn_half_point,
        tally.drawn_heights.values().sum::<usize>(),
        tally.para_half_point,
        tally.para_heights.values().sum::<usize>()
    );
    for (label, histogram) in [
        ("paragraph", &tally.para_heights),
        ("drawn", &tally.drawn_heights),
    ] {
        let mut entries: Vec<(u64, usize)> = histogram.iter().map(|(k, v)| (*k, *v)).collect();
        entries.sort_by_key(|(micrometres, _)| *micrometres);
        let shown: Vec<String> = entries
            .iter()
            .map(|(micrometres, count)| format!("{:.3}mm x{count}", *micrometres as f64 / 1e3))
            .collect();
        println!("  {label}: {}", shown.join("  "));
    }

    println!("\n== would drawing each selector-1 run as its own span change the page? ==");
    println!(
        "  records with more than one selector-1 run {}: height differs {}  typeface {}  colour {}",
        tally.spans_multiple,
        tally.span_height_conflict,
        tally.span_font_conflict,
        tally.span_colour_conflict
    );
    for line in &tally.span_samples {
        println!("  {line}");
    }

    println!("\n== selector-2 runs that name a different paragraph than +14 ==");
    for line in &tally.sel2_elsewhere {
        println!("  {line}");
    }

    println!("\n== samples (records with runs) ==");
    for line in &tally.samples {
        println!("  {line}");
    }
}
