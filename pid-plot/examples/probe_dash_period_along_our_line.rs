//! What period does the reference print along a dash this page already draws?
//!
//! Each dash is a body of 1.863 or 2.032 mm, a 0.169 mm mark touching it, then
//! a hole of 1.016 or 1.185 mm. The four sums are the clusters 3.048, 3.217,
//! 3.387 and 3.556, and they alternate on one `Electric` line. The cluster is
//! not a scale. Pieces shorter than 0.5 mm are kept out of the period, and the
//! band around the centreline is 0.15 mm.
//!
//! `cargo run -p pid-plot --release --example probe_dash_period_along_our_line`
//! reads the five sheets below. One drawing name selects that sheet.
//! `PID_PARSE_SELF_CHECK_DIR` is the corpus (default `验收数据` is not
//! assumed; set it).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use pid_parse::PidParser;
use pid_parse::style_link::{StyleHop, line_styles_for_document, storage_of_sheet};
use pid_plot::model::{PaperPrimitive, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

const MIN_LINE_MM: f64 = 12.0;
const LATERAL_MM: f64 = 0.15;
const MERGE_MM: f64 = 0.10;
const SHORT_MM: f64 = 0.50;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

struct Span {
    a: f64,
    b: f64,
}

fn main() {
    let corpus = corpus_dir();
    let requested = std::env::args().nth(1);
    let owned: Vec<String> = requested.as_deref().map_or_else(
        || DRAWINGS.iter().map(|name| (*name).to_string()).collect(),
        |name| vec![name.to_string()],
    );
    for name in &owned {
        let reference_path = corpus.join(format!("{name}.pdf"));
        let Ok(reference_bytes) = std::fs::read(&reference_path) else {
            eprintln!("skipping {name}: the reference PDF is absent");
            continue;
        };
        let pid_path = corpus.join(format!("{name}.pid"));
        let model = lay_out_drawing(&pid_path, &corpus).expect("layout");
        let doc = PidParser::new().parse_file(&pid_path).expect("parse");
        let style_at = style_labels(&doc);
        let reference = read_plot(&reference_bytes).expect("read reference");
        let mut tally: BTreeMap<String, Tally> = BTreeMap::new();
        for primitive in &model.primitives {
            let Some(stroke) = primitive.stroke.as_ref() else {
                continue;
            };
            if stroke.dash_mm.len() != 2 {
                continue;
            }
            let pattern = stroke
                .dash_mm
                .iter()
                .map(|step| format!("{step:.3}"))
                .collect::<Vec<_>>()
                .join("/");
            let style = style_label(primitive, &style_at);
            let key = format!("{pattern} @{:.2} {style}", stroke.width_mm);
            let chains = match &primitive.shape {
                PaperShape::Line { start_mm, end_mm } => vec![vec![*start_mm, *end_mm]],
                PaperShape::Polyline { points_mm, .. } => points_mm
                    .windows(2)
                    .map(|pair| vec![pair[0], pair[1]])
                    .collect(),
                _ => Vec::new(),
            };
            for chain in chains {
                let start = chain[0];
                let end = chain[1];
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let length = (dx * dx + dy * dy).sqrt();
                if length < MIN_LINE_MM {
                    continue;
                }
                let unit = [dx / length, dy / length];
                let mut spans = Vec::new();
                for segment in &reference.segments {
                    if !on_line(start, unit, length, segment.start_mm, segment.end_mm) {
                        continue;
                    }
                    let a = along(start, unit, segment.start_mm);
                    let b = along(start, unit, segment.end_mm);
                    spans.push(Span {
                        a: a.min(b),
                        b: a.max(b),
                    });
                }
                if spans.len() < 2 {
                    continue;
                }
                spans.sort_by(|left, right| left.a.total_cmp(&right.a));
                let sequence: Vec<(f64, f64)> = spans
                    .windows(2)
                    .map(|pair| (pair[0].b - pair[0].a, pair[1].a - pair[0].b))
                    .chain(spans.last().map(|span| (span.b - span.a, 0.0)))
                    .collect();
                let mut shorts = Vec::new();
                let mut bodies = Vec::new();
                let mut holes = Vec::new();
                for pair in sequence.windows(2) {
                    let (body, gap_to_cap) = pair[0];
                    let (cap, hole) = pair[1];
                    if body >= SHORT_MM && cap < SHORT_MM && gap_to_cap.abs() <= 0.05 {
                        bodies.push(body);
                        holes.push(hole);
                    }
                }
                spans.retain(|span| {
                    let piece = span.b - span.a;
                    if piece < SHORT_MM {
                        shorts.push(piece);
                        false
                    } else {
                        true
                    }
                });
                if spans.len() < 2 {
                    continue;
                }
                let mut merged: Vec<Span> = Vec::new();
                for span in spans {
                    if let Some(last) = merged.last_mut()
                        && span.a <= last.b + MERGE_MM
                    {
                        last.b = last.b.max(span.b);
                        continue;
                    }
                    merged.push(span);
                }
                let entry = tally.entry(key.clone()).or_default();
                let mut here = Vec::new();
                let mut ons = Vec::new();
                for pair in merged.windows(2) {
                    let period = pair[1].a - pair[0].a;
                    let gap = pair[1].a - pair[0].b;
                    if !(1.5..8.0).contains(&period) || !(0.3..3.5).contains(&gap) {
                        continue;
                    }
                    here.push(period);
                    ons.push(pair[0].b - pair[0].a);
                    entry.on.push(pair[0].b - pair[0].a);
                }
                if here.len() >= 4 {
                    entry.lines += 1;
                    let mut sorted = here.clone();
                    sorted.sort_by(f64::total_cmp);
                    ons.sort_by(f64::total_cmp);
                    entry
                        .line_medians
                        .push((median(&sorted), median(&ons), length, shorts.len()));
                    entry.periods.extend(here);
                    entry.shorts.extend(shorts);
                    entry.bodies.extend(bodies);
                    entry.holes.extend(holes);
                    let bucket = (median(&sorted) * 20.0).round() as i32;
                    if entry.shown.insert(bucket) {
                        let start_at = sequence
                            .iter()
                            .enumerate()
                            .filter(|(_, (piece, _))| *piece >= SHORT_MM)
                            .nth(2)
                            .map(|(index, _)| index)
                            .unwrap_or(0);
                        let text = sequence
                            .iter()
                            .skip(start_at)
                            .take(8)
                            .map(|(piece, gap)| format!("{piece:.3} g{gap:.3}"))
                            .collect::<Vec<_>>()
                            .join("  ");
                        entry.examples.push((bucket, text));
                    }
                }
            }
        }
        println!("== {name}");
        if tally.is_empty() {
            println!("   no dashed centreline longer than {MIN_LINE_MM:.0} mm");
        }
        for (key, entry) in &tally {
            if entry.periods.len() < 4 {
                continue;
            }
            let mut periods = entry.periods.clone();
            periods.sort_by(f64::total_cmp);
            let mut on = entry.on.clone();
            on.sort_by(f64::total_cmp);
            let mut shorts = entry.shorts.clone();
            shorts.sort_by(f64::total_cmp);
            println!(
                "   {key}  lines {}  periods {}  median period {:.3}  median on {:.3}  short {} median {:.3}",
                entry.lines,
                periods.len(),
                median(&periods),
                median(&on),
                shorts.len(),
                if shorts.is_empty() {
                    0.0
                } else {
                    median(&shorts)
                }
            );
            let medians = entry
                .line_medians
                .iter()
                .map(|(period, on, length, short_count)| {
                    format!("{period:.3}/on{on:.3}/s{short_count}@{length:.0}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            println!("     line medians {medians}");
            let mut buckets: BTreeMap<i32, usize> = BTreeMap::new();
            for period in &periods {
                let bucket = (period * 20.0).round() as i32;
                *buckets.entry(bucket).or_default() += 1;
            }
            for (bucket, count) in buckets {
                if count < 8 {
                    continue;
                }
                println!("     {:>6.2} mm  {count}", bucket as f64 / 20.0);
            }
            for (bucket, text) in &entry.examples {
                println!("     example {:.2}: {text}", *bucket as f64 / 20.0);
            }
            print_buckets("body", &entry.bodies);
            print_buckets("hole", &entry.holes);
        }
        println!();
    }
}

#[derive(Default)]
struct Tally {
    lines: usize,
    periods: Vec<f64>,
    on: Vec<f64>,
    line_medians: Vec<(f64, f64, f64, usize)>,
    shorts: Vec<f64>,
    bodies: Vec<f64>,
    holes: Vec<f64>,
    shown: BTreeSet<i32>,
    examples: Vec<(i32, String)>,
}

fn style_labels(doc: &pid_parse::PidDocument) -> HashMap<(String, usize, usize), Source> {
    let styles = line_styles_for_document(doc);
    let mut out = HashMap::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let table = doc.style_tables.get(&storage_of_sheet(&sheet.path));
        let style_of = |oid: u32| -> String {
            let Some(resolved) = styles.get(&(sheet.path.clone(), oid)) else {
                return format!("oid {oid} unresolved");
            };
            let record = table.and_then(|table| table.get(resolved.style_id));
            let name = record
                .and_then(|record| record.name.clone())
                .unwrap_or_else(|| "unnamed".to_string());
            let dash_id = record.and_then(|record| record.dash_reference);
            let hop = match resolved.hop {
                StyleHop::Direct => "direct",
                StyleHop::ViaOverride { .. } => "override",
            };
            format!(
                "style {} dash {} {hop} {name}",
                resolved.style_id,
                dash_id.map_or_else(|| "-".to_string(), |id| id.to_string())
            )
        };
        for record in &geometry.decoded_iglines {
            out.insert(
                (sheet.path.clone(), record.byte_start, record.byte_end),
                Source {
                    style: style_of(record.oid),
                },
            );
        }
        for record in &geometry.decoded_iglinestrings {
            out.insert(
                (sheet.path.clone(), record.byte_start, record.byte_end),
                Source {
                    style: style_of(record.oid),
                },
            );
        }
    }
    out
}

struct Source {
    style: String,
}

fn source_of<'a>(
    primitive: &PaperPrimitive,
    styles: &'a HashMap<(String, usize, usize), Source>,
) -> Option<&'a Source> {
    let stream = primitive.stream_path.as_ref()?;
    let [start, end] = primitive.byte_range?;
    styles.get(&(stream.clone(), start, end))
}

fn style_label(
    primitive: &PaperPrimitive,
    styles: &HashMap<(String, usize, usize), Source>,
) -> String {
    source_of(primitive, styles)
        .map(|source| source.style.clone())
        .unwrap_or_else(|| "unmatched".to_string())
}

fn print_buckets(label: &str, values: &[f64]) {
    let mut buckets: BTreeMap<i32, usize> = BTreeMap::new();
    for value in values {
        let bucket = (value * 20.0).round() as i32;
        *buckets.entry(bucket).or_default() += 1;
    }
    for (bucket, count) in buckets {
        if count < 8 {
            continue;
        }
        println!("     {label} {:>6.2} mm  {count}", bucket as f64 / 20.0);
    }
}

fn median(values: &[f64]) -> f64 {
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) * 0.5
    } else {
        values[mid]
    }
}

fn along(origin: [f64; 2], unit: [f64; 2], point: [f64; 2]) -> f64 {
    (point[0] - origin[0]) * unit[0] + (point[1] - origin[1]) * unit[1]
}

fn on_line(origin: [f64; 2], unit: [f64; 2], length: f64, start: [f64; 2], end: [f64; 2]) -> bool {
    let run = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
    if !(0.15..8.0).contains(&run) {
        return false;
    }
    let direction = [(end[0] - start[0]) / run, (end[1] - start[1]) / run];
    if (direction[0] * unit[0] + direction[1] * unit[1]).abs() < 0.98 {
        return false;
    }
    let a = along(origin, unit, start);
    let b = along(origin, unit, end);
    if a < -1.0 && b < -1.0 || a > length + 1.0 && b > length + 1.0 {
        return false;
    }
    let lateral = |point: [f64; 2]| {
        let dx = point[0] - origin[0];
        let dy = point[1] - origin[1];
        (-dx * unit[1] + dy * unit[0]).abs()
    };
    lateral(start) <= LATERAL_MM && lateral(end) <= LATERAL_MM
}
