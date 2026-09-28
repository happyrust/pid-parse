//! Which labels letter in more than one style, and how do their runs differ?
//!
//! OCS plan `2026-09-28-pid-import-next-steps.md` R1: before the labels whose
//! runs disagree become MTEXT, measure what the runs actually say. Per label:
//! the paragraph's alignment and line spacing, the rotation, how many
//! distinct letterings its runs reach and which one covers the most
//! characters (the one `style_link` flattens to today). Per run: the
//! characters it covers, its height, typeface and colour, and its height
//! against the widest run's -- a run well under the widest is what a
//! superscript (`m^3`) or a subscript looks like from here.
//!
//! ```powershell
//! cargo run --example probe_run_conflicts
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use pid_parse::style_link::{storage_of_sheet, ResolvedTextHeight};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/D06.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// A run's height against the widest run's below which it reads as a
/// super- or subscript rather than a second body size.
const SMALL_RUN_RATIO: f64 = 0.85;

fn lettering(style: &ResolvedTextHeight) -> (u64, Option<u32>, Option<&str>) {
    (
        style.height_m.to_bits(),
        style.colour,
        style.font_name.as_deref(),
    )
}

fn describe(style: &ResolvedTextHeight) -> String {
    format!(
        "{:.3} mm {} {}",
        style.height_m * 1000.0,
        style.font_name.as_deref().unwrap_or("(no typeface)"),
        style
            .colour
            .map_or_else(|| "(no colour)".to_string(), |c| format!("#{c:06X}"))
    )
}

fn main() {
    let mut total = 0usize;
    let mut per_fixture: BTreeMap<&str, usize> = BTreeMap::new();
    let mut alignments: BTreeMap<String, usize> = BTreeMap::new();
    let mut spacings: BTreeMap<String, usize> = BTreeMap::new();
    let mut rotations: BTreeMap<String, usize> = BTreeMap::new();
    let mut lettering_counts: BTreeMap<usize, usize> = BTreeMap::new();
    let mut run_counts: BTreeMap<usize, usize> = BTreeMap::new();
    let mut with_small_run = 0usize;
    let mut multi_line = 0usize;
    let mut transitions: BTreeMap<String, usize> = BTreeMap::new();
    // `(fixture, stream, oid)` -> every igTextBox record stored under it, so a
    // stream that holds one oid twice is seen for what it is: the geometry
    // emitter draws every record, the style index keeps one per oid.
    let mut by_oid: BTreeMap<(String, String, u32), Vec<String>> = BTreeMap::new();
    // Records whose PSM type word carries a flag bit, per family. The native
    // reader (`PSMSerializeIn`, radsrvitem.dll) skips a record whose type
    // word has `0x8000` set -- `type_flags & 0x2` here -- before it even
    // reads the oid; see docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md.
    let mut flagged: BTreeMap<(&str, u16), usize> = BTreeMap::new();
    let mut flagged_examples: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip {fixture}: not on this machine");
            continue;
        }
        let doc = pid_parse::PidParser::new()
            .parse_file(path)
            .expect("fixture parses");
        let mut in_fixture = 0usize;
        for sheet in &doc.sheet_streams {
            let Some(geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            let mut count_flags = |family: &'static str, flags: u16, describe: String| {
                if flags != 0 {
                    *flagged.entry((family, flags)).or_default() += 1;
                    flagged_examples.push(format!(
                        "{fixture} {} {family} flags {flags:#x}: {describe}",
                        sheet.path
                    ));
                }
            };
            for r in &geometry.decoded_primitive_lines {
                count_flags("primitive line", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_iglines {
                count_flags("igLine2d", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_iglinestrings {
                count_flags("igLineString2d", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_igpoints {
                count_flags("igPoint2d", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_igtextboxes {
                count_flags(
                    "igTextBox",
                    r.type_flags,
                    format!(
                        "oid {} {:?} bytes {}..{}",
                        r.oid, r.text, r.byte_start, r.byte_end
                    ),
                );
            }
            for r in &geometry.decoded_igsymbols {
                // Beside the flagged placement, every other record of the
                // same oid on this sheet: where the live one sits tells
                // whether the ghost overprints it or stands somewhere else.
                let siblings: Vec<String> = geometry
                    .decoded_igsymbols
                    .iter()
                    .filter(|s| s.oid == r.oid && s.byte_start != r.byte_start)
                    .map(|s| {
                        format!(
                            "flags {:#x} at ({:.4}, {:.4}) def sheet {} site {} bytes {}",
                            s.type_flags,
                            s.insertion_x,
                            s.insertion_y,
                            s.definition_sheet_ref,
                            s.definition_site_ref,
                            s.byte_start
                        )
                    })
                    .collect();
                count_flags(
                    "igSymbol2d",
                    r.type_flags,
                    format!(
                        "oid {} at ({:.4}, {:.4}) def sheet {} site {} bytes {} | same oid: [{}]",
                        r.oid,
                        r.insertion_x,
                        r.insertion_y,
                        r.definition_sheet_ref,
                        r.definition_site_ref,
                        r.byte_start,
                        siblings.join("; ")
                    ),
                );
            }
            for r in &geometry.decoded_igboundaries {
                count_flags("igBoundary2d", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_igsmartframes {
                count_flags("igSmartFrame2d", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_dependency_objects {
                count_flags("DependencyObject", r.type_flags, format!("oid {}", r.oid));
            }
            for r in &geometry.decoded_sub_records_0x0010 {
                count_flags("0x0010 sub-record", r.type_flags, String::new());
            }
            for r in &geometry.decoded_jstyle_overrides {
                count_flags("JStyleOverride", r.type_flags, String::new());
            }

            let Some(table) = doc.style_tables.get(&storage_of_sheet(&sheet.path)) else {
                continue;
            };
            for record in &geometry.decoded_igtextboxes {
                by_oid
                    .entry(((*fixture).to_string(), sheet.path.clone(), record.oid))
                    .or_default()
                    .push(format!(
                        "{:?} at ({:.4}, {:.4}) bytes {}..{} layer {} parent {} type_flags {:#x} sub_type {:#06x} shape {} index {}",
                        record.text,
                        record.trailing_double_1,
                        record.trailing_double_2,
                        record.byte_start,
                        record.byte_end,
                        record.sheet_layer_ref,
                        record.parent_ref,
                        record.type_flags,
                        record.sub_type_word,
                        record.text_sub_type,
                        record.index,
                    ));
                let selector_one: Vec<_> =
                    record.runs.iter().filter(|run| run.selector == 1).collect();
                if selector_one.len() < 2 {
                    continue;
                }
                let resolved: Option<Vec<(usize, ResolvedTextHeight)>> = selector_one
                    .iter()
                    .map(|run| {
                        table
                            .resolve_run_style(run.style_id)
                            .map(|style| (usize::from(run.len), style))
                    })
                    .collect();
                let Some(resolved) = resolved else {
                    continue;
                };
                let mut distinct: Vec<_> = resolved.iter().map(|(_, s)| lettering(s)).collect();
                distinct.sort_unstable();
                distinct.dedup();
                if distinct.len() < 2 {
                    continue;
                }

                total += 1;
                in_fixture += 1;
                *lettering_counts.entry(distinct.len()).or_default() += 1;
                *run_counts.entry(resolved.len()).or_default() += 1;
                let (alignment, spacing) = table.paragraph_layout(record.index);
                *alignments.entry(format!("{alignment:?}")).or_default() += 1;
                *spacings.entry(format!("{spacing:?}")).or_default() += 1;
                *rotations
                    .entry(format!("{:.0}°", record.rotation_rad.to_degrees()))
                    .or_default() += 1;
                if record.text.contains(['\r', '\n']) {
                    multi_line += 1;
                }

                // The style covering the most characters, first seen on a tie
                // -- the same choice `style_link::resolve_text_style` makes.
                let mut coverage: Vec<(usize, &ResolvedTextHeight)> = Vec::new();
                for (chars, style) in &resolved {
                    match coverage
                        .iter_mut()
                        .find(|(_, seen)| seen.style_id == style.style_id)
                    {
                        Some((total, _)) => *total += chars,
                        None => coverage.push((*chars, style)),
                    }
                }
                let widest = coverage
                    .iter()
                    .fold(
                        None::<&(usize, &ResolvedTextHeight)>,
                        |best, entry| match best {
                            Some(best) if best.0 >= entry.0 => Some(best),
                            _ => Some(entry),
                        },
                    )
                    .map(|(_, style)| *style)
                    .expect("at least two runs resolved");
                let paragraph = table.resolve_text_height(record.index);

                println!(
                    "\n{fixture} {} oid {} {:?}\n  paragraph {} | alignment {alignment:?} spacing {spacing:?} rotation {:.0}° | {} runs, {} letterings, widest = {}",
                    sheet.path,
                    record.oid,
                    record.text,
                    paragraph
                        .as_ref()
                        .map_or_else(|| "(unresolved)".to_string(), describe),
                    record.rotation_rad.to_degrees(),
                    resolved.len(),
                    distinct.len(),
                    describe(widest),
                );

                let chars: Vec<char> = record.text.chars().collect();
                let mut at = 0usize;
                let mut small = false;
                for (len, style) in &resolved {
                    let slice: String = chars.iter().skip(at).take(*len).collect();
                    at += len;
                    let ratio = style.height_m / widest.height_m;
                    let is_small = ratio < SMALL_RUN_RATIO;
                    small |= is_small;
                    println!(
                        "    run {len:>2} ch {:<16} {}  x{ratio:.2}{}",
                        format!("{:?}", slice.replace('\r', "\\r")),
                        describe(style),
                        if is_small { "  <- small run" } else { "" },
                    );
                    if style.style_id != widest.style_id {
                        *transitions
                            .entry(format!("{} -> {}", describe(widest), describe(style)))
                            .or_default() += 1;
                    }
                }
                if small {
                    with_small_run += 1;
                }
            }
        }
        per_fixture.insert(fixture, in_fixture);
    }

    println!("\n== summary ==");
    println!("labels whose runs reach more than one lettering: {total}");
    for (fixture, count) in &per_fixture {
        println!("  {fixture}: {count}");
    }
    println!("runs per label: {run_counts:?}");
    println!("distinct letterings per label: {lettering_counts:?}");
    println!("paragraph alignment: {alignments:?}");
    println!("paragraph line spacing: {spacings:?}");
    println!("rotation: {rotations:?}");
    println!("labels with a line break: {multi_line}");
    println!("labels with a run under x{SMALL_RUN_RATIO} of the widest: {with_small_run}");
    println!("widest -> other run (per run):");
    for (transition, count) in &transitions {
        println!("  {count:>3}  {transition}");
    }

    let duplicated: Vec<_> = by_oid
        .iter()
        .filter(|(_, records)| records.len() > 1)
        .collect();
    println!(
        "\n== igTextBox oids stored more than once in one stream: {} ==",
        duplicated.len()
    );
    for ((fixture, stream, oid), records) in duplicated {
        println!("{fixture} {stream} oid {oid}:");
        for record in records {
            println!("    {record}");
        }
    }

    println!(
        "\n== decoded records whose PSM type word carries a flag bit: {} ==",
        flagged.values().sum::<usize>()
    );
    for ((family, flags), count) in &flagged {
        println!("  {count:>4}  {family} flags {flags:#x}");
    }
    for example in flagged_examples.iter().take(40) {
        println!("  {example}");
    }
    if flagged_examples.len() > 40 {
        println!("  ... {} more", flagged_examples.len() - 40);
    }
}
