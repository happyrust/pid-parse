//! Which PSM records of the plant backup reach no page — and does any of
//! them carry ink?
//!
//! The census behind
//! `docs/analysis/2026-11-07-every-undrawn-record-of-the-plant-backup-is-inkless.md`.
//! It walks every place a plotted page of the 53-sheet backup draws from and
//! puts each record that draws nothing in one bucket, with the reason:
//!
//! | location | what | read by |
//! |---|---|---|
//! | `sheet` | a drawing's `Sheet*` streams | the family registry (`SHEET_RECORD_FAMILIES`) |
//! | `cache` | a drawing's `/JSite*/PSMcluster0` symbol caches | the eleven nested decoders `streams/jsite.rs` runs |
//! | `sym` | a `.sym` body in a `RefData~*.zip` | `symbol_library::read_symbol_geometry_from_bytes` |
//! | `igr` | a `.igr` template's `Sheet*` streams | the family registry — a template is parsed as a drawing |
//! | `igr-cache` | a template's `/JSite*/PSMcluster0` caches | as `cache` |
//!
//! The `sheet` / `igr` walk is each stream's strict chain
//! (`sheet_record_starts`), which is what `undecoded_census::unclaimed_counts`
//! reads wherever the chain walks; the three buckets it reports —
//! `undecoded`, `refused`, `skipped(0x8000)` — are cross-checked against what
//! the parser stored (`SheetGeometry::{undecoded_type_codes, refused_records,
//! skipped_records}`). The chain-validated greedy walk the census used to
//! make, from byte 0, is replayed beside it: every hit of that walk the chain
//! does not start a record at is listed under `greedy-walk` as a phantom, and
//! every chain record it stepped over as hidden. `cache` is walked on the
//! strict chain only, because that is the gate the nested decoders use; a
//! cache whose chain does not walk would lose every body, so that is counted
//! too.
//!
//! A record that is decoded but still draws nothing is bucketed as well:
//! an audit-only family on a sheet, a cached rectangle (its edges are lines
//! of their own) or dimension, a cached drawable on no body's layer, or on
//! a layer the storage's view filter switches off. A cached `igPoint2d`
//! has no nested decoder at all; its style is resolved here to see whether
//! the line terminator names a glyph that would draw.
//!
//! Run (PowerShell):
//!
//! ```powershell
//! $env:PID_TEST_DATA_DIR='D:\work\plant-code\cad\pid-test-data'
//! cargo run --example probe_which_records_reach_no_page
//! cargo run --example probe_which_records_reach_no_page -- <another .pid dir>
//! ```
//!
//! With no argument the corpus is `$PID_TEST_DATA_DIR\backup-plant-data`
//! and the archives are the `RefData~*.zip` beside it. Read-only; soft-skips
//! when the corpus is absent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_parse::model::{PidDocument, SHEET_RECORD_FAMILIES};
use pid_parse::parsers::sheet_records::{
    decode_igarc_at, decode_igbspcurve_at, decode_igcircle_at, decode_igdimension_at,
    decode_igellipse_at, decode_igellipticalarc_at, decode_igline_at, decode_iglinestring_at,
    decode_igrectangle_at, decode_igsym_boundary_at, decode_igsym_boundary_payload,
    decode_igtextbox_at, parse_psm_header, sheet_record_starts,
};
use pid_parse::parsers::undecoded_census::{
    is_native_graphic_type_code, rad_class_name, DECODED_TYPE_CODES,
};
use pid_parse::style_link::DocumentStyleTable;
use pid_parse::{build_normalized_geometry, PidGraphicKind, PidPackage, PidParser};

/// The type codes `streams/jsite.rs::decode_nested_geometry` has a decoder
/// for — a cached record of any other code is read by nothing.
const NESTED_DECODED: [u16; 11] = [
    0x0059, 0x0061, 0x007E, 0x0063, 0x0018, 0x0084, 0x004D, 0x0020, 0x005D, 0x0115, 0x0013,
];

/// The type codes `symbol_library::decode_primitive` has an arm for.
const SYM_DECODED: [u16; 9] = [
    0x0018, 0x0059, 0x0061, 0x0084, 0x004D, 0x005D, 0x0013, 0x0063, 0x007E,
];

/// A sheet's live `igLine2d` records by oid: `(start, end)`.
type LineEnds = BTreeMap<u32, ((f64, f64), (f64, f64))>;

/// Per symbol key: placements drawn from the library, the drawings, and what
/// the reader stepped over in that body.
type LibraryDrawn = BTreeMap<String, (usize, BTreeSet<String>, BTreeMap<u16, usize>)>;

/// Stream header the cluster magic opens every chain stream with.
const CLUSTER_MAGIC: u32 = 0x6C90_F544;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn class(code: u16) -> &'static str {
    rad_class_name(code).unwrap_or("?")
}

/// The storage a stream lives in (`/` for the root).
fn storage_of(stream: &str) -> &str {
    match stream.rfind('/') {
        Some(0) | None => "/",
        Some(cut) => &stream[..cut],
    }
}

/// `(records, files)` per `(location, code, bucket)`.
#[derive(Default)]
struct Tally {
    rows: BTreeMap<(&'static str, u16, String), (usize, BTreeSet<String>)>,
}

impl Tally {
    fn add(&mut self, location: &'static str, code: u16, bucket: impl Into<String>, file: &str) {
        let row = self
            .rows
            .entry((location, code, bucket.into()))
            .or_default();
        row.0 += 1;
        row.1.insert(file.to_string());
    }
}

/// One record worth printing in full.
struct Detail {
    location: &'static str,
    bucket: String,
    file: String,
    stream: String,
    offset: usize,
    type_word: u16,
    payload: Vec<u8>,
    note: String,
}

impl Detail {
    fn print(&self) {
        let code = self.type_word & 0x3FFF;
        println!(
            "  [{}] {} {} +{} type_word=0x{:04X} (0x{code:04X} {}) btf={} oid={} layer={} sub=0x{:04X}  {}",
            self.location,
            self.file,
            self.stream,
            self.offset,
            self.type_word,
            class(code),
            self.payload.len(),
            u32_at(&self.payload, 0).unwrap_or(0),
            u32_at(&self.payload, 8).unwrap_or(0),
            u16_at(&self.payload, 12).unwrap_or(0),
            self.note,
        );
    }
}

/// Everything the walks collect.
#[derive(Default)]
struct Census {
    tally: Tally,
    details: Vec<Detail>,
    /// `(file, stream, census_says, parser_says)` where the two disagree.
    disagreements: Vec<String>,
    /// Per location: streams walked, records walked.
    walked: BTreeMap<&'static str, (usize, usize)>,
    /// Cache streams whose strict chain does not walk.
    broken_caches: Vec<String>,
    /// igBoundary2d shape, by `(location, outcome, sub-type, tags)`.
    boundaries: BTreeMap<(&'static str, String, u16, String), usize>,
    /// Each `0x8000` record and what became of its oid.
    retired: Vec<String>,
    /// Cached / `.sym` points and what their style draws.
    points: BTreeMap<(&'static str, String), usize>,
    /// Which walk the parser's stored census agrees with, per stream.
    parser_matches: BTreeMap<&'static str, usize>,
    /// Each `0x0021 ComplexString` and what its members are.
    complex: Vec<String>,
    /// Placements of the templates, by body source.
    template_placements: BTreeMap<&'static str, usize>,
    /// `.sym` records carrying the skip bit, with what became of their oid.
    sym_flagged: Vec<String>,
}

/// The type codes a decoder somewhere draws a stroke from.
const STROKE_CODES: [u16; 8] = [
    0x0018, 0x0084, 0x0059, 0x0061, 0x007E, 0x0063, 0x005D, 0x004D,
];

/// Every `0x0021 ComplexString` in a chain stream: its member list (the
/// `u32 oid + 4 x i16 + u16 tag` groups after the `+33` count) and, for each
/// member, the live record of that oid in the same stream.
fn complex_strings(
    census: &mut Census,
    location: &'static str,
    file: &str,
    stream: &str,
    data: &[u8],
    records: &[(usize, usize)],
) {
    let by_oid: BTreeMap<u32, u16> = records
        .iter()
        .filter_map(|&(at, _)| {
            let type_word = u16_at(data, at)?;
            (type_word & 0x8000 == 0).then_some((u32_at(data, at + 6)?, type_word & 0x3FFF))
        })
        .collect();
    for &(at, end) in records {
        if u16_at(data, at).map(|w| w & 0x3FFF) != Some(0x0021) {
            continue;
        }
        let payload = &data[at + 6..end];
        let count = u32_at(payload, 21).unwrap_or(0).min(256) as usize;
        let mut members = Vec::new();
        for slot in 0..count {
            let base = 33 + slot * 14;
            let (Some(oid), Some(tag)) = (u32_at(payload, base), u16_at(payload, base + 12)) else {
                break;
            };
            let kind = by_oid
                .get(&oid)
                .map_or_else(|| "no record".to_string(), |code| format!("0x{code:04X}"));
            members.push(format!("{oid}:{tag:04x}->{kind}"));
        }
        let geometry = members
            .iter()
            .filter(|m| {
                STROKE_CODES
                    .iter()
                    .any(|c| m.ends_with(&format!("0x{c:04X}")))
            })
            .count();
        census.complex.push(format!(
            "[{location}] {file} {stream} +{at}: {count} members, {geometry} of them stroke records: {}",
            members.join(" ")
        ));
    }
}

/// The parser's census walk, record for record: a candidate header is taken
/// when it is end-of-stream or followed by another plausible header.
fn census_walk(data: &[u8]) -> Vec<(usize, usize)> {
    let end_of = |off: usize| -> Option<usize> {
        let header_end = off.checked_add(6)?;
        if header_end > data.len() {
            return None;
        }
        let type_word = u16_at(data, off)?;
        if type_word & 0x3FFF == 0 {
            return None;
        }
        let btf = u32_at(data, off + 2)? as usize;
        if !(8..=100_000).contains(&btf) {
            return None;
        }
        let end = header_end.checked_add(btf)?;
        (end <= data.len()).then_some(end)
    };
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 6 <= data.len() {
        if let Some(end) = end_of(off) {
            if end == data.len() || end_of(end).is_some() {
                out.push((off, end));
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

/// The member tags of an `igBoundary2d`, walked the way the template grammar
/// lays them out (`0x67` / `0x73` 33 bytes, `0x68` a point run, `0x74` 58
/// bytes), stopping at the first tag none of them names.
fn boundary_tags(payload: &[u8]) -> String {
    let count = u32_at(payload, 22).unwrap_or(0).min(64) as usize;
    let mut tags = Vec::new();
    let mut pos = 28usize;
    for _ in 0..count {
        let Some(&tag) = payload.get(pos) else { break };
        tags.push(format!("{tag:02x}"));
        pos += match tag {
            0x67 | 0x73 => 33,
            0x74 => 58,
            0x68 => 7 + 16 * u32_at(payload, pos + 1).unwrap_or(0).min(4096) as usize,
            _ => break,
        };
    }
    let mut unique: Vec<String> = tags.clone();
    unique.sort();
    unique.dedup();
    format!("n={} tags={}", tags.len(), unique.join("/"))
}

/// What the line style a record's `+14` index names says about a point: a
/// terminator whose glyph draws, one whose glyph is blank, no terminator,
/// or no style the table resolves.
fn point_marker(table: Option<&DocumentStyleTable>, index: u32) -> &'static str {
    let Some(table) = table else {
        return "no style table";
    };
    match table.resolve_line_style(index) {
        None => "style unresolved",
        Some(style) => match style.marker {
            None => "no terminator",
            Some(marker) if marker.draws() => "TERMINATOR GLYPH DRAWS",
            Some(_) => "terminator glyph blank",
        },
    }
}

/// Walk one `Sheet*` stream of a drawing or template.
fn walk_sheet(
    census: &mut Census,
    location: &'static str,
    file: &str,
    stream: &str,
    data: &[u8],
    doc: &PidDocument,
) {
    let mut claimed: BTreeMap<usize, (&'static str, bool)> = BTreeMap::new();
    for family in SHEET_RECORD_FAMILIES {
        for range in (family.decoded_ranges)(data) {
            claimed
                .entry(range.start)
                .or_insert((family.name, family.emits_geometry));
        }
    }
    let strict_records = strict_chain(data);
    let strict: BTreeSet<usize> = strict_records.iter().map(|(at, _)| *at).collect();
    let layers = doc.sheet_layers.get(storage_of(stream));
    let greedy = census_walk(data);
    let greedy_starts: BTreeSet<usize> = greedy.iter().map(|(at, _)| *at).collect();
    let kind_of = |off: usize| -> Option<(&'static str, u16)> {
        if claimed.contains_key(&off) {
            return None;
        }
        let type_word = u16_at(data, off)?;
        let code = type_word & 0x3FFF;
        Some((
            if type_word & 0x8000 != 0 {
                "skipped"
            } else if DECODED_TYPE_CODES.contains(&code) {
                "refused"
            } else {
                "undecoded"
            },
            code,
        ))
    };
    let mut by_greedy: BTreeMap<(&'static str, u16), usize> = BTreeMap::new();
    for (off, _) in &greedy {
        if let Some(key) = kind_of(*off) {
            *by_greedy.entry(key).or_default() += 1;
        }
    }
    let mut by_strict: BTreeMap<(&'static str, u16), usize> = BTreeMap::new();
    for (off, _) in &strict_records {
        if let Some(key) = kind_of(*off) {
            *by_strict.entry(key).or_default() += 1;
        }
    }
    // A census hit the strict chain does not start a record at is not a
    // record; a strict-chain record the census walk stepped over is one the
    // census never saw. Both only mean something where the strict chain
    // walks the stream.
    if !strict.is_empty() {
        for &(off, end) in greedy.iter().filter(|(at, _)| !strict.contains(at)) {
            let type_word = u16_at(data, off).unwrap_or(0);
            let inside = strict.range(..=off).next_back().copied();
            let note = match inside {
                Some(start) => format!(
                    "inside the strict-chain record at +{start} (type 0x{:04X}); claims {} bytes to +{end}",
                    u16_at(data, start).unwrap_or(0),
                    end - off - 6
                ),
                None => format!(
                    "inside the 8-byte stream header (magic {:08X}, record count {}); claims {} bytes to +{end}",
                    u32_at(data, 0).unwrap_or(0),
                    u32_at(data, 4).unwrap_or(0),
                    end - off - 6
                ),
            };
            census.tally.add(
                "greedy-walk",
                type_word & 0x3FFF,
                "phantom of the greedy census walk (no record starts here)",
                file,
            );
            census.details.push(Detail {
                location: "greedy-walk",
                bucket: "phantom".into(),
                file: file.to_string(),
                stream: stream.to_string(),
                offset: off,
                type_word,
                payload: data[off + 6..end].to_vec(),
                note,
            });
        }
        for &(off, _) in strict_records
            .iter()
            .filter(|(at, _)| !greedy_starts.contains(at))
        {
            let type_word = u16_at(data, off).unwrap_or(0);
            let fate = match kind_of(off) {
                None => "claimed by a decoder".to_string(),
                Some((kind, _)) => kind.to_string(),
            };
            census.tally.add(
                "greedy-walk",
                type_word & 0x3FFF,
                format!("record the greedy census walk stepped over ({fate})"),
                file,
            );
        }
    }
    let records = if strict.is_empty() {
        greedy.clone()
    } else {
        strict_records.clone()
    };
    complex_strings(census, location, file, stream, data, &records);
    let walked = census.walked.entry(location).or_default();
    walked.0 += 1;
    walked.1 += records.len();
    for &(off, end) in &records {
        let type_word = u16_at(data, off).unwrap_or(0);
        let code = type_word & 0x3FFF;
        let skip = type_word & 0x8000 != 0;
        let payload = &data[off + 6..end];
        if code == 0x0013 {
            let outcome = match claimed.get(&off) {
                Some(_) => "decoded (ring)".to_string(),
                None if skip => "skipped (0x8000)".to_string(),
                None => "refused".to_string(),
            };
            *census
                .boundaries
                .entry((
                    location,
                    outcome,
                    u16_at(payload, 12).unwrap_or(0xFFFF),
                    boundary_tags(payload),
                ))
                .or_default() += 1;
        }
        let bucket: String = match claimed.get(&off) {
            Some((_, true)) => continue,
            Some((name, false)) => format!("audit-only ({name})"),
            None if skip => "skipped (0x8000)".into(),
            None if DECODED_TYPE_CODES.contains(&code) => "refused".into(),
            None => "undecoded".into(),
        };
        if !is_native_graphic_type_code(code) && !bucket.starts_with("undecoded") {
            census.tally.add(location, code, bucket, file);
            continue;
        }
        let layer = u32_at(payload, 8).unwrap_or(0);
        let layer_note = layers
            .and_then(|table| table.iter().find(|l| l.oid == layer))
            .map(|l| format!("layer `{}` displayed={:?}", l.name, l.displayed))
            .unwrap_or_else(|| "layer not in the storage's table".into());
        let chain_note = if strict.is_empty() {
            "strict chain does not walk".to_string()
        } else if strict.contains(&off) {
            "on the strict chain".to_string()
        } else {
            let before = strict.range(..off).next_back().copied();
            format!("NOT a strict-chain record (inside the record at {before:?})")
        };
        if skip {
            census.retired.push(retired_fate(
                file,
                stream,
                off,
                code,
                payload,
                &records,
                data,
                &layer_note,
            ));
        }
        let shape_note = match code {
            0x0020 => {
                let lines: LineEnds = records
                    .iter()
                    .filter_map(|&(at, _)| decode_igline_at(data, at))
                    .map(|line| (line.oid, (line.start, line.end)))
                    .collect();
                rectangle_edges_note(off, payload, &lines)
            }
            0x007B => group_members_note(payload, data, &records),
            _ => String::new(),
        };
        let note = match bucket.as_str() {
            "refused" | "undecoded" => format!(
                "{chain_note}; {layer_note}; {shape_note}; head {}",
                hex(&payload[..payload.len().min(40)])
            ),
            _ => format!("{chain_note}; {layer_note}; {shape_note}"),
        };
        census.tally.add(location, code, bucket.clone(), file);
        census.details.push(Detail {
            location,
            bucket,
            file: file.to_string(),
            stream: stream.to_string(),
            offset: off,
            type_word,
            payload: payload.to_vec(),
            note,
        });
    }
    // The parser's own three censuses for this stream, if it kept them.
    let mut parser: BTreeMap<(&'static str, u16), usize> = BTreeMap::new();
    if let Some(geometry) = doc
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == stream)
        .and_then(|sheet| sheet.geometry.as_ref())
    {
        for row in &geometry.undecoded_type_codes {
            parser.insert(("undecoded", row.type_code), row.count);
        }
        for row in &geometry.refused_records {
            parser.insert(("refused", row.type_code), row.count);
        }
        for row in &geometry.skipped_records {
            parser.insert(("skipped", row.type_code), row.count);
        }
    }
    let matches = if parser == by_strict && parser == by_greedy {
        "both walks"
    } else if parser == by_greedy {
        "the census walk"
    } else if parser == by_strict {
        "the strict chain"
    } else {
        "NEITHER"
    };
    *census.parser_matches.entry(matches).or_default() += 1;
    if matches == "NEITHER" {
        census.disagreements.push(format!(
            "{file} {stream}: census walk {by_greedy:?} strict {by_strict:?} parser {parser:?}"
        ));
    }
}

/// `(start, end)` of every record on the strict chain; empty when the chain
/// does not walk the stream exactly.
fn strict_chain(data: &[u8]) -> Vec<(usize, usize)> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| {
            let header = parse_psm_header(data, at)?;
            Some((at, header.body_start + header.bytes_to_follow as usize))
        })
        .collect()
}

/// Whether a `0x8000` record's oid lives on in an unflagged record of the
/// same type code in the same stream, and on which side of it.
#[allow(clippy::too_many_arguments)]
fn retired_fate(
    file: &str,
    stream: &str,
    off: usize,
    code: u16,
    payload: &[u8],
    records: &[(usize, usize)],
    data: &[u8],
    layer_note: &str,
) -> String {
    let oid = u32_at(payload, 0).unwrap_or(0);
    let mut later = 0usize;
    let mut earlier = 0usize;
    let mut other_code = 0usize;
    for &(at, end) in records {
        let Some(type_word) = u16_at(data, at) else {
            continue;
        };
        if type_word & 0x8000 != 0 || end <= at || u32_at(data, at + 6) != Some(oid) {
            continue;
        }
        if type_word & 0x3FFF != code {
            other_code += 1;
        } else if at > off {
            later += 1;
        } else {
            earlier += 1;
        }
    }
    let verdict = match (earlier, later, other_code) {
        (0, 0, 0) => "no live record of that oid",
        (0, 0, _) => "oid live under another type code",
        (0, _, _) => "live copy later",
        (_, 0, _) => "live copy EARLIER",
        _ => "live copies on both sides",
    };
    format!("{file} {stream} +{off} 0x{code:04X} oid={oid} [{layer_note}]: {verdict}")
}

/// Walk one `/JSite*/PSMcluster0` symbol cache on its strict chain.
fn walk_cache(
    census: &mut Census,
    location: &'static str,
    file: &str,
    stream: &str,
    data: &[u8],
    doc: &PidDocument,
) {
    let site_path = storage_of(stream);
    let site = doc.jsites.iter().find(|site| site.path == site_path);
    let definitions = site
        .and_then(|site| site.nested_geometry.as_ref())
        .map(|nested| nested.definitions.clone())
        .unwrap_or_default();
    let layers = doc.sheet_layers.get(site_path);
    let styles = doc.style_tables.get(site_path);
    let records = strict_chain(data);
    let starts: Vec<usize> = records.iter().map(|(at, _)| *at).collect();
    let walked = census.walked.entry(location).or_default();
    walked.0 += 1;
    walked.1 += starts.len();
    complex_strings(census, location, file, stream, data, &records);
    if starts.is_empty() {
        let greedy = census_walk(data);
        let graphic = greedy
            .iter()
            .filter(|(at, _)| {
                u16_at(data, *at).is_some_and(|w| is_native_graphic_type_code(w & 0x3FFF))
            })
            .count();
        if !greedy.is_empty() {
            census.broken_caches.push(format!(
                "{file} {stream}: {} bytes, census walk sees {} records ({graphic} graphic)",
                data.len(),
                greedy.len()
            ));
        }
        return;
    }
    for at in starts {
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        let end = header.body_start + header.bytes_to_follow as usize;
        let payload = &data[header.body_start..end];
        let code = header.type_code;
        let type_word = u16_at(data, at).unwrap_or(0);
        let layer = u32_at(payload, 8).unwrap_or(0);
        let on_body = definitions
            .iter()
            .any(|definition| definition.layers.binary_search(&layer).is_ok());
        let layer_row = layers.and_then(|table| table.iter().find(|l| l.oid == layer));
        let hidden = layer_row.is_some_and(|l| l.displayed == Some(false));
        let layer_note = layer_row
            .map(|l| format!("layer `{}` displayed={:?}", l.name, l.displayed))
            .unwrap_or_else(|| "layer not in the storage's table".into());

        if header.native_reader_skips() {
            census.retired.push(retired_fate(
                file,
                stream,
                at,
                code,
                payload,
                &records,
                data,
                &layer_note,
            ));
            census.tally.add(location, code, "skipped (0x8000)", file);
            continue;
        }
        let decoded: Option<(&str, bool)> = if decode_igcircle_at(data, at).is_some() {
            Some(("circle", true))
        } else if decode_igarc_at(data, at).is_some() {
            Some(("arc", true))
        } else if decode_igellipticalarc_at(data, at).is_some() {
            Some(("elliptical arc", true))
        } else if decode_igellipse_at(data, at).is_some() {
            Some(("ellipse", true))
        } else if decode_igline_at(data, at).is_some() {
            Some(("line", true))
        } else if let Some(polyline) = decode_iglinestring_at(data, at) {
            Some(("line string", polyline.vertices.len() >= 2))
        } else if let Some(text) = decode_igtextbox_at(data, at) {
            Some(("text", !text.text.is_empty()))
        } else if decode_igrectangle_at(data, at).is_some() {
            Some(("rectangle", false))
        } else if decode_igbspcurve_at(data, at).is_some() {
            Some(("B-spline", true))
        } else if decode_igdimension_at(data, at).is_some() {
            Some(("dimension", false))
        } else if decode_igsym_boundary_at(data, at).is_some() {
            Some(("boundary", true))
        } else {
            None
        };
        if code == 0x0013 {
            let outcome = match decoded {
                Some(_) => "decoded (fill)".to_string(),
                None => "refused".to_string(),
            };
            *census
                .boundaries
                .entry((
                    location,
                    outcome,
                    u16_at(payload, 12).unwrap_or(0xFFFF),
                    boundary_tags(payload),
                ))
                .or_default() += 1;
        }
        let bucket: String = match decoded {
            Some((_, true)) if !on_body => "decoded, on no body's layer".into(),
            Some((_, true)) if hidden => "decoded, on a layer switched off".into(),
            Some((_, true)) => continue,
            Some(("text", false)) => "decoded, empty text".into(),
            Some(("line string", false)) => "decoded, fewer than 2 vertices".into(),
            Some((name, false)) => format!("decoded, not drawn ({name})"),
            None if NESTED_DECODED.contains(&code) => "refused".into(),
            None => "no decoder".into(),
        };
        if code == 0x005E {
            let index = u32_at(payload, 14).unwrap_or(0);
            let marker = point_marker(styles, index);
            let where_ = if !on_body {
                "on no body"
            } else if hidden {
                "layer off"
            } else {
                "on a shown body layer"
            };
            *census
                .points
                .entry((location, format!("{marker} / {where_}")))
                .or_default() += 1;
            let shown_glyph = marker == "TERMINATOR GLYPH DRAWS" && on_body && !hidden;
            if shown_glyph {
                census.details.push(Detail {
                    location,
                    bucket: "point whose glyph would draw".into(),
                    file: file.to_string(),
                    stream: stream.to_string(),
                    offset: at,
                    type_word,
                    payload: payload.to_vec(),
                    note: format!("style index {index}; {layer_note}"),
                });
            }
            census
                .tally
                .add(location, code, format!("{bucket} (point: {marker})"), file);
            continue;
        }
        let graphic = is_native_graphic_type_code(code);
        census.tally.add(location, code, bucket.clone(), file);
        if graphic && (bucket == "refused" || bucket == "no decoder") {
            let extra = match code {
                0x0115 => dimension_note(payload),
                0x0013 => boundary_tags(payload),
                _ => String::new(),
            };
            census.details.push(Detail {
                location,
                bucket,
                file: file.to_string(),
                stream: stream.to_string(),
                offset: at,
                type_word,
                payload: payload.to_vec(),
                note: format!(
                    "{layer_note}; on_body={on_body}; {extra}; head {}",
                    hex(&payload[..payload.len().min(36)])
                ),
            });
        }
    }
}

/// The `JDim` frame fields a refused dimension carries: kind, flag word,
/// `main_len`, whether the frame equation closes, the block's lead dword.
fn dimension_note(payload: &[u8]) -> String {
    let kind = u16_at(payload, 14).unwrap_or(0);
    let flags = u16_at(payload, 26).unwrap_or(0);
    let main_len = u32_at(payload, 30).unwrap_or(0) as usize;
    let tail = if flags & 0x0100 != 0 { 4 } else { 0 };
    let closes = 34 + main_len + tail == payload.len();
    let block = u32_at(payload, 34).unwrap_or(0);
    let value = f64_at(payload, 42).unwrap_or(f64::NAN);
    format!(
        "kind={kind} flags=0x{flags:04X} main_len={main_len} frame_closes={closes} block0=0x{block:08X} value={:.4}mm",
        value * 1000.0
    )
}

/// Census one parsed compound file (a drawing or a template).
fn census_document(
    census: &mut Census,
    sheet_location: &'static str,
    cache_location: &'static str,
    file: &str,
    package: &PidPackage,
) {
    for (stream, raw) in &package.streams {
        let leaf = stream.rsplit('/').next().unwrap_or("");
        if leaf.starts_with("Sheet") {
            walk_sheet(
                census,
                sheet_location,
                file,
                stream,
                &raw.data,
                &package.parsed,
            );
        } else if leaf == "PSMcluster0" && stream.starts_with("/JSite") {
            walk_cache(
                census,
                cache_location,
                file,
                stream,
                &raw.data,
                &package.parsed,
            );
        } else {
            // Every other chain stream — the root cluster with its layers,
            // view sets and space map, the style and attribute clusters. Any
            // graphic record here would be read by nothing.
            for at in sheet_record_starts(&raw.data) {
                let Some(header) = parse_psm_header(&raw.data, at) else {
                    continue;
                };
                if is_native_graphic_type_code(header.type_code) {
                    census.tally.add(
                        "other-stream",
                        header.type_code,
                        format!("graphic code in {stream}, which no geometry reader opens"),
                        file,
                    );
                }
            }
        }
    }
}

/// One `.sym`'s chain walk the way `symbol_library::read_sheet_chain` makes
/// it: from byte 8, `type & 0x0FFF`, a zero type word or an overrun drops
/// the whole sheet, and so does a walk that does not end at the last byte.
struct SymSheet {
    stream: String,
    records: Vec<(usize, u16, Vec<u8>)>,
    dropped: Option<String>,
}

fn sym_sheets(package: &PidPackage) -> Vec<SymSheet> {
    let mut out = Vec::new();
    for (stream, raw) in &package.streams {
        let leaf = stream.rsplit('/').next().unwrap_or("");
        if !leaf.starts_with("Sheet") || raw.data.len() <= 8 {
            continue;
        }
        let bytes = &raw.data;
        let mut sheet = SymSheet {
            stream: stream.clone(),
            records: Vec::new(),
            dropped: None,
        };
        if u32_at(bytes, 0) != Some(CLUSTER_MAGIC) {
            sheet.dropped = Some("no cluster magic".into());
            out.push(sheet);
            continue;
        }
        let mut at = 8usize;
        while at + 6 <= bytes.len() {
            let raw_type = u16_at(bytes, at).unwrap_or(0);
            let length = u32_at(bytes, at + 2).unwrap_or(0) as usize;
            if raw_type == 0 {
                sheet.dropped = Some(format!("zero type word at +{at}"));
                break;
            }
            let end = at + 6 + length;
            if end > bytes.len() {
                sheet.dropped = Some(format!("record at +{at} overruns"));
                break;
            }
            sheet
                .records
                .push((at, raw_type, bytes[at + 6..end].to_vec()));
            at = end;
        }
        if sheet.dropped.is_none() && at != bytes.len() {
            sheet.dropped = Some(format!("{} trailing bytes", bytes.len() - at));
        }
        out.push(sheet);
    }
    out
}

#[cfg(feature = "refdata-archive")]
fn archive_members_of(dir: &Path, extension: &str) -> Vec<(String, Vec<u8>)> {
    use pid_parse::refdata_archive::{archive_members, read_archive_file, refdata_archives};
    let mut out = Vec::new();
    for archive in refdata_archives(dir) {
        let Ok(members) = archive_members(&archive, extension) else {
            continue;
        };
        let zip = archive
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for member in members {
            if let Ok(bytes) = read_archive_file(&member) {
                out.push((format!("{zip}::{}", member.name()), bytes));
            }
        }
    }
    out
}

#[cfg(not(feature = "refdata-archive"))]
fn archive_members_of(_dir: &Path, _extension: &str) -> Vec<(String, Vec<u8>)> {
    println!("(refdata-archive feature off: no RefData~*.zip read)");
    Vec::new()
}

/// Lower-cased file name of a symbol path or member, for joining a
/// placement to the archive member it would read.
fn sym_key(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

/// Census the `.sym` bodies; returns the members by key for the placement
/// join.
fn census_symbols(census: &mut Census, members: &[(String, Vec<u8>)]) -> BTreeMap<String, String> {
    let mut by_key = BTreeMap::new();
    for (label, bytes) in members {
        by_key.insert(sym_key(label), label.clone());
        let Ok(package) = PidPackage::from_bytes(bytes) else {
            census.tally.add("sym", 0, "file does not parse", label);
            continue;
        };
        let Ok(body) = pid_parse::symbol_library::read_symbol_geometry_from_bytes(bytes) else {
            continue;
        };
        let styles: BTreeMap<String, DocumentStyleTable> = package
            .streams
            .iter()
            .filter(|(path, _)| path.rsplit('/').next() == Some("StyleCluster"))
            .map(|(path, raw)| {
                (
                    storage_of(path).to_string(),
                    DocumentStyleTable::from_stylecluster_bytes(&raw.data),
                )
            })
            .collect();
        // The reader opens `Sheet*` streams only. A shape record PSM filed
        // in the symbol's root cluster instead is read by nothing.
        if let Some(root) = package.streams.get("/PSMcluster0") {
            for at in sheet_record_starts(&root.data) {
                let Some(header) = parse_psm_header(&root.data, at) else {
                    continue;
                };
                if STROKE_CODES.contains(&header.type_code) || header.type_code == 0x0013 {
                    census.tally.add(
                        "sym",
                        header.type_code,
                        "in the root /PSMcluster0, which the reader never opens",
                        label,
                    );
                }
            }
        }
        // The reader's own skipped count, per code, is the authority on
        // what it stepped over; the walk below only names the records.
        let mut reader_skipped = body.skipped_records.clone();
        for sheet in sym_sheets(&package) {
            census.walked.entry("sym").or_default().0 += 1;
            if let Some(why) = &sheet.dropped {
                census.tally.add(
                    "sym",
                    0,
                    format!("whole sheet dropped by the reader ({why})"),
                    label,
                );
                census.details.push(Detail {
                    location: "sym",
                    bucket: "sheet dropped".into(),
                    file: label.clone(),
                    stream: sheet.stream.clone(),
                    offset: 0,
                    type_word: 0,
                    payload: Vec::new(),
                    note: why.clone(),
                });
                continue;
            }
            census.walked.entry("sym").or_default().1 += sheet.records.len();
            let line_oids: LineEnds = sheet
                .records
                .iter()
                .filter(|(_, raw_type, payload)| {
                    raw_type & 0x0FFF == 0x0018 && raw_type & 0xC000 == 0 && payload.len() >= 50
                })
                .filter_map(|(_, _, payload)| {
                    Some((
                        u32_at(payload, 0)?,
                        (
                            (f64_at(payload, 18)?, f64_at(payload, 26)?),
                            (f64_at(payload, 34)?, f64_at(payload, 42)?),
                        ),
                    ))
                })
                .collect();
            let table = styles.get(storage_of(&sheet.stream));
            for (at, raw_type, payload) in &sheet.records {
                let code = raw_type & 0x0FFF;
                if raw_type & 0xF000 != 0 {
                    let fate = if SYM_DECODED.contains(&code) {
                        "DRAWN by the library reader"
                    } else {
                        "stepped over"
                    };
                    census.tally.add(
                        "sym",
                        code,
                        format!("type word 0x{raw_type:04X} high bits set, {fate}"),
                        label,
                    );
                    if SYM_DECODED.contains(&code) {
                        let oid = u32_at(payload, 0).unwrap_or(0);
                        let live: Vec<&Vec<u8>> = sheet
                            .records
                            .iter()
                            .filter(|(_, other, body)| {
                                other & 0xF000 == 0
                                    && other & 0x0FFF == code
                                    && u32_at(body, 0) == Some(oid)
                            })
                            .map(|(_, _, body)| body)
                            .collect();
                        let verdict = match live.first() {
                            None => "no live record of that oid".to_string(),
                            Some(body) if *body == payload => "a byte-identical live copy".into(),
                            Some(_) => "a live copy that differs".into(),
                        };
                        census.sym_flagged.push(format!(
                            "{label} {} +{at} 0x{raw_type:04X} oid={oid}: {verdict}",
                            sheet.stream
                        ));
                    }
                }
                let skipped_here = !SYM_DECODED.contains(&code)
                    || (code == 0x0013 && decode_igsym_boundary_payload(payload).is_none());
                if code == 0x0013 {
                    let outcome = if skipped_here {
                        "skipped"
                    } else {
                        "decoded (fill)"
                    };
                    *census
                        .boundaries
                        .entry((
                            "sym",
                            outcome.to_string(),
                            u16_at(payload, 12).unwrap_or(0xFFFF),
                            boundary_tags(payload),
                        ))
                        .or_default() += 1;
                }
                if code == 0x005E {
                    let index = u32_at(payload, 14).unwrap_or(0);
                    let marker = point_marker(table, index);
                    *census
                        .points
                        .entry(("sym", marker.to_string()))
                        .or_default() += 1;
                }
                if !skipped_here {
                    continue;
                }
                if let Some(count) = reader_skipped.get_mut(&code) {
                    *count = count.saturating_sub(1);
                }
                let mut note = String::new();
                if code == 0x0020 {
                    note = rectangle_edges_note(*at, payload, &line_oids);
                } else if code == 0x0115 {
                    note = dimension_note(payload);
                } else if code == 0x0013 {
                    note = boundary_tags(payload);
                }
                let layer = u32_at(payload, 8).unwrap_or(0);
                let layer_name = body
                    .sheet_layers
                    .iter()
                    .find(|l| l.oid == layer)
                    .map_or("?", |l| l.name.as_str());
                census
                    .tally
                    .add("sym", code, "stepped over by the reader", label);
                if is_native_graphic_type_code(code) && code != 0x005E {
                    census.details.push(Detail {
                        location: "sym",
                        bucket: "stepped over".into(),
                        file: label.clone(),
                        stream: sheet.stream.clone(),
                        offset: *at,
                        type_word: *raw_type,
                        payload: payload.clone(),
                        note: format!("layer `{layer_name}`; {note}"),
                    });
                }
            }
        }
        // Whatever the reader refused that the walk thought it decoded (a
        // domain check failing inside an arm it has).
        for (code, count) in reader_skipped {
            for _ in 0..count {
                census
                    .tally
                    .add("sym", code, "refused inside a decoded arm", label);
            }
        }
    }
    by_key
}

/// What a `Group implementation` lists: the `u16` member count at `+16` and
/// the 14-byte member entries from `+28` (the layout `style_link` reads point
/// symbols' groups by), each resolved to the live record of that oid.
fn group_members_note(payload: &[u8], data: &[u8], records: &[(usize, usize)]) -> String {
    let count = usize::from(u16_at(payload, 16).unwrap_or(0)).min(64);
    let members: Vec<String> = (0..count)
        .filter_map(|slot| u32_at(payload, 28 + 14 * slot))
        .map(|oid| {
            let code = records.iter().find_map(|&(at, _)| {
                let type_word = u16_at(data, at)?;
                (type_word & 0x8000 == 0 && u32_at(data, at + 6) == Some(oid))
                    .then_some(type_word & 0x3FFF)
            });
            match code {
                Some(code) => format!("{oid}->0x{code:04X} {}", class(code)),
                None => format!("{oid}->no live record"),
            }
        })
        .collect();
    format!("group of {count}: {}", members.join(", "))
}

/// Whether the four edges a rectangle lists are `igLine2d` records of the
/// same sheet whose ends are the rectangle's corners.
fn rectangle_edges_note(at: usize, payload: &[u8], lines: &LineEnds) -> String {
    let mut record = vec![0x20u8, 0x00];
    record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    record.extend_from_slice(payload);
    let Some(rectangle) = decode_igrectangle_at(&record, 0) else {
        let doubles: Vec<String> = (0..5)
            .map(|i| format!("{:.6}", f64_at(payload, 18 + 8 * i).unwrap_or(f64::NAN)))
            .collect();
        let edges: Vec<u32> = (0..u32_at(payload, 58).unwrap_or(0).min(8) as usize)
            .filter_map(|slot| u32_at(payload, 62 + slot * 4))
            .collect();
        let live = edges.iter().filter(|edge| lines.contains_key(edge)).count();
        return format!(
            "rectangle at +{at} refused by the drawing-side decoder: doubles [{}] edges {edges:?} ({live} live igLine2d of this sheet)",
            doubles.join(", ")
        );
    };
    let corners = rectangle.corners();
    let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
    let mut found = 0;
    let mut on_corners = 0;
    for edge in &rectangle.edges {
        if let Some((start, end)) = lines.get(edge) {
            found += 1;
            if corners.iter().any(|c| near(*c, *start)) && corners.iter().any(|c| near(*c, *end)) {
                on_corners += 1;
            }
        }
    }
    format!(
        "edges {:?}: {found}/{} are live igLine2d of this sheet, {on_corners} end on the corners",
        rectangle.edges,
        rectangle.edges.len()
    )
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
        {
            out.push(path);
        }
    }
}

fn symbol_roots(corpus: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut roots = Vec::new();
    for base in [
        Some(corpus.to_path_buf()),
        corpus.parent().map(Path::to_path_buf),
    ]
    .into_iter()
    .flatten()
    {
        let Ok(entries) = std::fs::read_dir(base.join("Ref")) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && path.file_name().is_some_and(|name| {
                        name.to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("symbol")
                    })
            })
            .collect();
        found.sort();
        roots.extend(found);
    }
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    let mut archives = Vec::new();
    #[cfg(feature = "refdata-archive")]
    for base in [Some(corpus), corpus.parent()].into_iter().flatten() {
        archives.extend(pid_parse::refdata_archive::refdata_archives(base));
    }
    (roots, archives)
}

fn main() {
    let data_dir = std::env::var_os("PID_TEST_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let corpora: Vec<PathBuf> = if args.is_empty() {
        vec![data_dir.join("backup-plant-data")]
    } else {
        args
    };
    let mut pids = Vec::new();
    for corpus in &corpora {
        collect_pids(corpus, &mut pids);
    }
    if pids.is_empty() {
        println!("no .pid under {corpora:?}; soft-skip");
        return;
    }
    let archive_dir = corpora[0]
        .parent()
        .map_or_else(|| data_dir.clone(), Path::to_path_buf);
    println!("corpus: {} drawing(s) under {corpora:?}", pids.len());
    println!("archives: RefData~*.zip in {}", archive_dir.display());

    let mut census = Census::default();
    let (roots, archives) = symbol_roots(&corpora[0]);
    let mut library = pid_parse::symbol_library::SymbolLibrary::with_roots(roots.clone());
    #[cfg(feature = "refdata-archive")]
    {
        library = library.with_archives(archives.clone());
    }
    let _ = &archives;
    // (symbol key) -> (placements drawn from the library, drawings)
    let mut library_drawn: LibraryDrawn = BTreeMap::new();
    let (mut cached, mut from_library, mut missing, mut unnamed) = (0usize, 0usize, 0usize, 0usize);

    for pid in &pids {
        let file = pid
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(package) = PidParser::new().parse_package(pid) else {
            println!("  {file}: does not parse");
            continue;
        };
        census_document(&mut census, "sheet", "cache", &file, &package);

        let geometry = build_normalized_geometry(&package.parsed);
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                symbol_path,
                definition,
                ..
            } = &entity.kind
            else {
                continue;
            };
            if definition
                .and_then(|reference| geometry.symbol_definition(reference))
                .is_some()
            {
                cached += 1;
                continue;
            }
            let Some(path) = symbol_path else {
                unnamed += 1;
                continue;
            };
            match library.resolve(path) {
                Some(body) => {
                    from_library += 1;
                    let row = library_drawn.entry(sym_key(path)).or_default();
                    row.0 += 1;
                    row.1.insert(file.clone());
                    row.2 = body.skipped_records.clone();
                }
                None => missing += 1,
            }
        }
    }

    let syms = archive_members_of(&archive_dir, "sym");
    let igrs = archive_members_of(&archive_dir, "igr");
    println!("archive members: {} .sym, {} .igr", syms.len(), igrs.len());
    let by_key = census_symbols(&mut census, &syms);
    for (label, bytes) in &igrs {
        let Ok(package) = PidPackage::from_bytes(bytes) else {
            census.tally.add("igr", 0, "file does not parse", label);
            continue;
        };
        census_document(&mut census, "igr", "igr-cache", label, &package);
        let geometry = build_normalized_geometry(&package.parsed);
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                symbol_path,
                definition,
                ..
            } = &entity.kind
            else {
                continue;
            };
            let source = if definition
                .and_then(|reference| geometry.symbol_definition(reference))
                .is_some()
            {
                "cached body"
            } else if symbol_path
                .as_deref()
                .is_some_and(|path| library.resolve(path).is_some())
            {
                "library body"
            } else {
                "no body"
            };
            *census.template_placements.entry(source).or_default() += 1;
        }
    }

    println!("\n=== walked ===");
    for (location, (streams, records)) in &census.walked {
        println!("  {location:<10} {streams:>6} streams {records:>8} records");
    }

    println!("\n=== census: every record that draws nothing, by location / code / bucket ===");
    println!(
        "  {:<12} {:<6} {:<26} {:>6} {:>5}  bucket",
        "location", "code", "class", "count", "files"
    );
    for ((location, code, bucket), (count, files)) in &census.tally.rows {
        let graphic = if is_native_graphic_type_code(*code) {
            "*"
        } else {
            " "
        };
        let few: Vec<&String> = files.iter().take(3).collect();
        let more = if files.len() > 3 { ", …" } else { "" };
        println!(
            "  {location:<12} 0x{code:04X}{graphic} {:<26} {count:>6} {:>5}  {bucket}   [{}{more}]",
            class(*code),
            files.len(),
            few.iter()
                .map(|f| f.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    println!("  (* = the native graphic predicate `radsrvitem.dll!sub_56449950` accepts the code)");

    println!("\n=== which walk the parser's stored census agrees with, per sheet stream ===");
    for (walk, streams) in &census.parser_matches {
        println!("  {streams:>5} stream(s): {walk}");
    }
    for line in &census.disagreements {
        println!("  DISAGREE {line}");
    }

    println!("\n=== 0x0021 ComplexString members ===");
    if census.complex.is_empty() {
        println!("  none");
    }
    for line in &census.complex {
        println!("  {line}");
    }

    println!("\n=== caches whose strict chain does not walk ===");
    if census.broken_caches.is_empty() {
        println!("  none");
    }
    for line in &census.broken_caches {
        println!("  {line}");
    }

    println!("\n=== igBoundary2d by location / outcome / sub-type / member tags ===");
    for ((location, outcome, sub, tags), count) in &census.boundaries {
        println!("  {location:<10} {outcome:<28} sub=0x{sub:04X} {tags:<22} {count:>5}");
    }

    println!("\n=== igPoint2d in caches and .sym: what the style's terminator draws ===");
    for ((location, what), count) in &census.points {
        println!("  {location:<10} {what:<50} {count:>6}");
    }

    println!("\n=== 0x8000 records: does the oid live on? (code / layer / verdict) ===");
    let mut fates: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    for line in &census.retired {
        let verdict = line.rsplit(": ").next().unwrap_or("").to_string();
        let code = line
            .split_whitespace()
            .find(|word| word.starts_with("0x"))
            .unwrap_or("?")
            .to_string();
        let layer = line
            .split('[')
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .unwrap_or("?")
            .to_string();
        *fates.entry((code, layer, verdict)).or_default() += 1;
    }
    for ((code, layer, verdict), count) in &fates {
        println!("  {count:>5} x {code} {layer:<48} {verdict}");
    }
    let mut shown = 0;
    for line in census
        .retired
        .iter()
        .filter(|line| !line.ends_with("live copy later"))
    {
        if shown < 25 {
            println!("    {line}");
        }
        shown += 1;
    }
    if shown > 25 {
        println!("    … {} more without a later live copy", shown - 25);
    }

    println!("\n=== .sym records with the skip bit that the library reader draws ===");
    let mut sym_fates: BTreeMap<String, usize> = BTreeMap::new();
    for line in &census.sym_flagged {
        *sym_fates
            .entry(line.rsplit(": ").next().unwrap_or("").to_string())
            .or_default() += 1;
    }
    for (verdict, count) in &sym_fates {
        println!("  {count:>5} x {verdict}");
    }
    for line in census.sym_flagged.iter().take(12) {
        println!("    {line}");
    }

    println!("\n=== placements of the templates ===");
    for (source, count) in &census.template_placements {
        println!("  {count:>5} x {source}");
    }

    println!("\n=== records in full (graphic codes that draw nothing) ===");
    let mut by_bucket: BTreeMap<(&'static str, String, u16), Vec<&Detail>> = BTreeMap::new();
    for detail in &census.details {
        by_bucket
            .entry((
                detail.location,
                detail.bucket.clone(),
                detail.type_word & 0x3FFF,
            ))
            .or_default()
            .push(detail);
    }
    for ((location, bucket, code), details) in &by_bucket {
        println!(
            "\n-- {location} / 0x{code:04X} {} / {bucket}: {} --",
            class(*code),
            details.len()
        );
        // Skipped records are many and alike; their fate is reported above.
        let limit = if bucket.starts_with("skipped") { 3 } else { 60 };
        for detail in details.iter().take(limit) {
            detail.print();
        }
        if details.len() > limit {
            println!("  … {} more", details.len() - limit);
        }
    }

    println!("\n=== placements of the drawings ===");
    println!(
        "  cached body {cached}, library body {from_library}, no body {missing}, no symbol path {unnamed}"
    );
    println!("  library-drawn symbols and what their reader stepped over:");
    for (key, (count, files, skipped)) in &library_drawn {
        let member = by_key
            .get(key)
            .map_or("(not in the archives)", String::as_str);
        let graphic: BTreeMap<String, usize> = skipped
            .iter()
            .filter(|(code, _)| is_native_graphic_type_code(**code))
            .map(|(code, n)| (format!("0x{code:04X}"), *n))
            .collect();
        println!(
            "    {count:>4} x {key:<48} {} drawing(s); graphic codes stepped over {graphic:?}; {member}",
            files.len()
        );
    }
}
