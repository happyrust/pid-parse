//! Which `igTextBox` fields could tell a label that hangs from its point from
//! one that stands on it?
//!
//! OCS plan 2026-10-08-pid-afternoon-review-uid-join-and-merge-handoff X2:
//! read as the top of the first line, table cells and title-block captions
//! fall inside their cells, while line numbers, nozzle numbers and slope
//! labels run into the leader or mark just below their point. This dumps,
//! for every decoded text box, the fields that could tell the two apart --
//! shape, tail kind and the bytes past the placement tail, parent, layer,
//! sub-type and index, runs, and the `DependencyObject`s listing it -- as one
//! tab-separated row, for a script that joins it to the measured groups.
//!
//! ```powershell
//! cargo run --quiet --example probe_text_anchor_by_kind > rows.tsv
//! ```
//!
//! Read-only: no parser, schema or model change.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::rad_class_name;
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

const FIXTURES: &[(&str, &str)] = &[
    ("D06", "test-file/D06.pid"),
    ("0201", "test-file/DWG-0201GP06-01.pid"),
    ("0202", "test-file/DWG-0202GP06-01.pid"),
    ("A01", "test-file/export-test/publish-data/A01/A01.pid"),
    (
        "gongyi",
        "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    ),
];

/// Payload offset of the tail-kind tag (record byte 26).
const TAIL_KIND_AT: usize = 20;
const ENVELOPE: usize = 6;

fn stream_bytes(path: &Path, stream: &str) -> Option<Vec<u8>> {
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path).ok()?).ok()?;
    let mut bytes = Vec::new();
    cfb.open_stream(stream).ok()?.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Where the placement tail starts: the insertion point's two doubles,
/// found by value rather than by a shape table.
fn tail_at(payload: &[u8], x: f64, y: f64) -> Option<usize> {
    let mut needle = x.to_le_bytes().to_vec();
    needle.extend_from_slice(&y.to_le_bytes());
    payload.windows(16).rposition(|w| w == needle.as_slice())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A `DependencyObject`'s member oids: `k` entries of `(u32 oid, u16 1)`
/// from the start of its tail.
fn members(tail: &[u8], k: u16) -> Vec<u32> {
    (0..usize::from(k))
        .filter_map(|i| u32_at(tail, i * 6))
        .collect()
}

/// Every record in a stream: oid -> (type code, parent), reading the
/// `(u32 oid, u32 parent)` every Sigma record opens its payload with.
fn records(bytes: &[u8]) -> BTreeMap<u32, (u16, u32)> {
    let mut out = BTreeMap::new();
    for at in sheet_record_starts(bytes) {
        let (Some(word), Some(oid), Some(parent)) = (
            u16_at(bytes, at),
            u32_at(bytes, at + ENVELOPE),
            u32_at(bytes, at + ENVELOPE + 4),
        ) else {
            continue;
        };
        out.entry(oid).or_insert((word & 0x3FFF, parent));
    }
    out
}

/// The payload of the `JStyleTextPara` (`0x002D`) with style id `id`, out of
/// the storage's `StyleCluster` bytes at the range the style table found it.
fn paragraph<'a>(
    styles: &'a [u8],
    table: Option<&pid_parse::style_link::DocumentStyleTable>,
    id: u32,
) -> Option<&'a [u8]> {
    let record = table?.get(id)?;
    if record.type_code != 0x002D {
        return None;
    }
    styles.get(record.byte_range.start + ENVELOPE..record.byte_range.end)
}

fn paragraph_fields(payload: &[u8]) -> String {
    let doubles: Vec<String> = [42, 50, 58, 66, 74, 82]
        .iter()
        .filter_map(|at| payload.get(*at..*at + 8))
        .map(|b| {
            let v = f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
            format!("{v}")
        })
        .collect();
    format!(
        "p12..26={} p26={} p30={} p34={} p35={} p36={} p37={} d={}",
        payload.get(12..26).map_or_else(String::new, hex),
        payload.get(26..30).map_or_else(String::new, hex),
        payload.get(30..34).map_or_else(String::new, hex),
        payload.get(34).copied().unwrap_or(0xFF),
        payload.get(35).copied().unwrap_or(0xFF),
        payload.get(36).copied().unwrap_or(0xFF),
        payload.get(37).copied().unwrap_or(0xFF),
        doubles.join("/")
    )
}

fn class(code: u16) -> String {
    rad_class_name(code).map_or_else(|| format!("0x{code:04X}"), |name| name.replace(' ', "_"))
}

fn main() {
    println!(
        "fixture\tstream\toid\tparent\tparent_class\tgrandparent\tsiblings\th_mm\tleaders\ttail_word\thead\tpara\tlayer\tsub\tindex\tshape\ttail_kind\tbtf\textra_len\textra\truns\tx_mm\ty_mm\trot_deg\tdeps\ttail_style\ttext"
    );
    for (name, fixture) in FIXTURES {
        let path = Path::new(fixture);
        let doc = match PidParser::new().parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                eprintln!("{name}: {error}");
                continue;
            }
        };
        let geometry = build_normalized_geometry(&doc);
        let heights = pid_parse::style_link::text_heights_for_document(&doc);
        let mut points_of: BTreeMap<(String, u32), Vec<(f64, f64)>> = BTreeMap::new();
        for entity in &geometry.entities {
            let (Some(stream), Some(oid)) = (entity.source.stream_path.clone(), entity.graphic_oid)
            else {
                continue;
            };
            let points: Vec<(f64, f64)> = match &entity.kind {
                PidGraphicKind::Line { start, end } => vec![(start.x, start.y), (end.x, end.y)],
                PidGraphicKind::Polyline { points, .. } => {
                    points.iter().map(|p| (p.x, p.y)).collect()
                }
                _ => continue,
            };
            points_of.entry((stream, oid)).or_default().extend(points);
        }
        let mut kind_of: BTreeMap<(String, u32), &'static str> = BTreeMap::new();
        for entity in &geometry.entities {
            let (Some(stream), Some(oid)) = (entity.source.stream_path.clone(), entity.graphic_oid)
            else {
                continue;
            };
            let kind = match &entity.kind {
                PidGraphicKind::Text { .. } => "text",
                PidGraphicKind::SymbolInstance { .. } => "symbol",
                PidGraphicKind::Line { .. } => "line",
                PidGraphicKind::Polyline { .. } => "polyline",
                _ => "other",
            };
            kind_of.entry((stream, oid)).or_insert(kind);
        }
        for sheet in &doc.sheet_streams {
            let Some(sheet_geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            let Some(bytes) = stream_bytes(path, &sheet.path) else {
                continue;
            };
            let styles = stream_bytes(
                path,
                &pid_parse::style_link::stylecluster_path_for_sheet(&sheet.path),
            )
            .unwrap_or_default();
            let table = doc
                .style_tables
                .get(&pid_parse::style_link::storage_of_sheet(&sheet.path));
            let index = records(&bytes);
            let mut children: BTreeMap<u32, BTreeMap<String, usize>> = BTreeMap::new();
            let mut child_oids: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for (oid, (code, parent)) in &index {
                *children
                    .entry(*parent)
                    .or_default()
                    .entry(class(*code))
                    .or_default() += 1;
                child_oids.entry(*parent).or_default().push(*oid);
            }
            let mut listed_in: BTreeMap<u32, Vec<String>> = BTreeMap::new();
            for dependency in &sheet_geometry.decoded_dependency_objects {
                let all = members(
                    &dependency.raw_reference_payload,
                    dependency.group_kind_word,
                );
                for (position, oid) in all.iter().enumerate() {
                    let others: Vec<String> = all
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != position)
                        .map(|(_, other)| {
                            let kind = kind_of
                                .get(&(sheet.path.clone(), *other))
                                .copied()
                                .unwrap_or("?");
                            format!("{other}:{kind}")
                        })
                        .collect();
                    listed_in.entry(*oid).or_default().push(format!(
                        "dep{}/k{}/s{}/at{}[{}]",
                        dependency.oid,
                        dependency.group_kind_word,
                        dependency.sub_type_word,
                        position,
                        others.join(",")
                    ));
                }
            }
            for text in &sheet_geometry.decoded_igtextboxes {
                let payload = bytes
                    .get(text.byte_start + ENVELOPE..text.byte_end)
                    .unwrap_or_default();
                let tail_kind = u16_at(payload, TAIL_KIND_AT).unwrap_or(0);
                let tail = tail_at(payload, text.trailing_double_1, text.trailing_double_2);
                let extra = tail
                    .and_then(|at| payload.get(at + 36..))
                    .unwrap_or_default();
                let tail_word = tail
                    .and_then(|at| payload.get(at + 32..at + 36))
                    .map_or_else(String::new, hex);
                // Whether the dword's one live byte is itself a style id.
                let tail_style = tail
                    .and_then(|at| payload.get(at + 35))
                    .and_then(|byte| table?.get(u32::from(*byte)))
                    .map_or_else(
                        || "-".to_string(),
                        |record| {
                            let body = styles
                                .get(record.byte_range.start + ENVELOPE..record.byte_range.end)
                                .unwrap_or_default();
                            format!(
                                "{} {} {}",
                                class(record.type_code),
                                record.name.as_deref().unwrap_or("?").replace(' ', "_"),
                                hex(&body[..body.len().min(64)])
                            )
                        },
                    );
                let head = payload.get(8..22).map_or_else(String::new, hex);
                let para = u32_at(payload, 14)
                    .and_then(|id| {
                        paragraph(&styles, table, id)
                            .map(|p| format!("para {id} {}", paragraph_fields(p)))
                    })
                    .unwrap_or_else(|| "para ?".to_string());
                let runs: Vec<String> = text
                    .runs
                    .iter()
                    .map(|run| format!("{}x{}:{}", run.selector, run.len, run.style_id))
                    .collect();
                let deps = listed_in
                    .get(&text.oid)
                    .map(|list| list.join(" "))
                    .unwrap_or_default();
                let (parent_class, grandparent) = index
                    .get(&text.parent_ref)
                    .map_or(("-".to_string(), 0), |(code, parent)| {
                        (class(*code), *parent)
                    });
                let siblings: Vec<String> = children
                    .get(&text.parent_ref)
                    .map(|kinds| {
                        kinds
                            .iter()
                            .map(|(kind, n)| format!("{kind}x{n}"))
                            .collect()
                    })
                    .unwrap_or_default();
                let h_mm = heights
                    .get(&(sheet.path.clone(), text.oid))
                    .map_or(0.0, |style| style.height_m * 1000.0);
                // The nearest end of every line-work sibling under a small
                // parent (a label's leader), relative to the insertion point.
                let (ix, iy) = (text.trailing_double_1, text.trailing_double_2);
                let leaders: Vec<String> = if text.parent_ref == 6 {
                    Vec::new()
                } else {
                    child_oids
                        .get(&text.parent_ref)
                        .into_iter()
                        .flatten()
                        .filter_map(|oid| points_of.get(&(sheet.path.clone(), *oid)))
                        .filter(|points| points.len() <= 6)
                        .filter_map(|points| {
                            let (px, py) = points.iter().copied().min_by(|a, b| {
                                let da = (a.0 - ix).hypot(a.1 - iy);
                                let db = (b.0 - ix).hypot(b.1 - iy);
                                da.total_cmp(&db)
                            })?;
                            Some(format!(
                                "({:+.3},{:+.3})",
                                (px - ix) * 1000.0,
                                (py - iy) * 1000.0
                            ))
                        })
                        .collect()
                };
                println!(
                    "{name}\t{}\t{}\t{}\t{parent_class}\t{grandparent}\t{}\t{h_mm:.3}\t{}\t{tail_word}\t{head}\t{para}\t{}\t{}\t{}\t{}\t{tail_kind}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.0}\t{deps}\t{tail_style}\t{}",
                    sheet.path,
                    text.oid,
                    text.parent_ref,
                    siblings.join(","),
                    leaders.join(" "),
                    text.sheet_layer_ref,
                    text.sub_type_word,
                    text.index,
                    text.text_sub_type,
                    text.bytes_to_follow,
                    extra.len(),
                    hex(&extra[..extra.len().min(48)]),
                    runs.join(","),
                    text.trailing_double_1 * 1000.0,
                    text.trailing_double_2 * 1000.0,
                    text.rotation_rad.to_degrees(),
                    text.text.replace('\r', "\u{23CE}").replace('\t', " "),
                );
            }
        }
    }
}
