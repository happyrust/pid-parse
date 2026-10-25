//! What does a `JStyleOverride` carry besides the pointer to its base style,
//! and what would the drawing look like if that were applied? (pid-parse plan
//! `2026-10-10-an-override-applies-its-own-width-colour-and-dash.md`, A0.)
//!
//! `DocumentStyleTable::resolve_line_style` follows an override's `+22`
//! reference to its `JStyleSimpleLine` and reads the width, colour and dash
//! from *that* record. The 2026-10-09 comparison of `DWG-0201GP06-01` against
//! a SmartPlant screenshot found the equipment table's fourteen rules drawn
//! dashed where SmartPlant draws them solid -- and every one of them reaches
//! its `Dashed` base through an override, while the one dashed line
//! SmartPlant also dashes (the LIA-LIT signal) names its style outright.
//!
//! This probe lays the override records out field by field so the question
//! can be answered from the file rather than from the screenshot:
//!
//! 1. every `0x0030` record of every `StyleCluster` (root and `JSite*`), its
//!    base record, and the version-3 fields at `+26..+90` -- `+30` broken out
//!    bit by bit, since that is the "which attributes this override states"
//!    mask (`style.dll`'s `JStyleOverride` compose routine, `sub_1002A080`,
//!    tests exactly these bits; see the analysis page);
//! 2. which overrides the drawing's geometry actually reaches (sheet records
//!    and cached symbol bodies alike), and through which records;
//! 3. for each reached override, the line style the base gives today and
//!    the one the override's own fields give: bit0 -> colour at `+34`,
//!    bit1 -> width at `+42`, bit7 -> the dash type whose style id sits at
//!    `+78` (zero meaning none);
//! 4. per fixture, the palette and dash census before and after -- the
//!    numbers the ratchets and OpenCADStudio's `pid_import` are re-pinned to;
//! 5. the dash slot, corroborated from the corpus side: every `u32` slot of
//!    every override, split by whether bit7 is set. `+78` is the only slot
//!    that is ever non-zero on a bit7 override, and when it is, it names a
//!    `0x002F JStyleSimpleDashType` of the same table.
//!
//! The probe changes nothing and asserts nothing; it prints.
//!
//! ```powershell
//! cargo run --example probe_override_applies_its_own_fields
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::style_link::{
    DocumentStyleTable, StyleHop, StyleRecord, PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL,
};
use pid_parse::{PidDocument, PidParser};

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// PSM type code of `JStyleOverride`.
const OVERRIDE: u16 = 0x0030;
/// PSM type code of `JStyleSimpleLine`.
const SIMPLE_LINE: u16 = 0x002E;
/// PSM type code of `JStyleSimpleDashType`.
const DASH_TYPE: u16 = 0x002F;
/// Bytes of PSM envelope (`type_word` + `bytes_to_follow`) ahead of a payload.
const ENVELOPE: usize = 6;

/// The version-3 `JStyleOverride` layout after the 26-byte base block, as
/// `docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md` §1
/// reads it off `style.dll`: four `u32`, four `f64`, three `u32`, two `u16`.
const U32_SLOTS: [usize; 7] = [26, 30, 34, 38, 74, 78, 82];
const F64_SLOTS: [usize; 4] = [42, 50, 58, 66];
const U16_SLOTS: [usize; 2] = [86, 88];

/// `+30` -> `this+96`: the attribute mask.
const MASK_AT: usize = 30;
/// `+34` -> `this+100`: colour, Win32 `COLORREF`.
const COLOUR_AT: usize = 34;
/// `+42` -> `this+104`: width in metres.
const WIDTH_AT: usize = 42;
/// `+78` -> `this+188`: style id of the dash type, or zero.
const DASH_AT: usize = 78;
const BIT_COLOUR: u32 = 0x01;
const BIT_WIDTH: u32 = 0x02;
const BIT_DASH: u32 = 0x80;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f64_at(d: &[u8], at: usize) -> Option<f64> {
    let s = d.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

fn rgb(colour: u32) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        colour & 0xFF,
        (colour >> 8) & 0xFF,
        (colour >> 16) & 0xFF
    )
}

fn mm(width_m: f64) -> String {
    format!("{:.3}mm", width_m * 1000.0)
}

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

/// The payload bytes of a style record, sliced out of its stream by the
/// record's own `byte_range` (which includes the envelope).
fn payload_of<'a>(stream: &'a [u8], record: &StyleRecord) -> Option<&'a [u8]> {
    let start = record.byte_range.start.checked_add(ENVELOPE)?;
    stream.get(start..record.byte_range.end)
}

/// A line style as the drawing would state it: width, colour, dash.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    width_m: f64,
    colour: u32,
    dash_mm: Option<Vec<f64>>,
}

impl Look {
    fn key(&self) -> String {
        format!("{} {}", mm(self.width_m), rgb(self.colour))
    }

    fn dash_key(&self) -> String {
        match &self.dash_mm {
            None => "solid".to_string(),
            Some(segments) => format!("dash {segments:?}"),
        }
    }
}

/// What the base record gives, by the resolver as it stands today.
fn today(table: &DocumentStyleTable, style_id: u32) -> Option<Look> {
    let resolved = table.resolve_line_style(style_id)?;
    Some(Look {
        width_m: resolved.symbology.width_m,
        colour: resolved.symbology.colour,
        dash_mm: resolved.dash.map(|dash| dash.segments_mm()),
    })
}

/// The override's own fields, read raw.
struct OverrideFields {
    mask: u32,
    colour: u32,
    width_m: f64,
    dash_reference: u32,
}

fn override_fields(payload: &[u8]) -> Option<OverrideFields> {
    Some(OverrideFields {
        mask: u32_at(payload, MASK_AT)?,
        colour: u32_at(payload, COLOUR_AT)?,
        width_m: f64_at(payload, WIDTH_AT)?,
        dash_reference: u32_at(payload, DASH_AT)?,
    })
}

/// What applying the mask gives, on top of what the base gives today.
/// Plausibility gates mirror `read_symbology`: a width must be a normal
/// float inside (0, 5 mm]; a colour must have a zero high byte and not be
/// the `-2` "unset" sentinel (`0x80000000`, which the compose routine
/// treats specially, is refused here too and the base colour kept).
fn applied(base: &Look, fields: &OverrideFields, table: &DocumentStyleTable) -> Look {
    let mut out = base.clone();
    if fields.mask & BIT_WIDTH != 0
        && fields.width_m.is_normal()
        && (0.0..=0.005).contains(&fields.width_m)
    {
        out.width_m = fields.width_m;
    }
    if fields.mask & BIT_COLOUR != 0 && fields.colour >> 24 == 0 && fields.colour != 0xFFFF_FFFE {
        out.colour = fields.colour;
    }
    if fields.mask & BIT_DASH != 0 {
        out.dash_mm = if fields.dash_reference == 0 {
            None
        } else {
            table
                .get(fields.dash_reference)
                .filter(|r| r.type_code == DASH_TYPE)
                .and_then(|r| r.dash.map(|d| d.segments_mm()))
        };
    }
    out
}

fn bits(mask: u32) -> String {
    let mut names = Vec::new();
    for bit in 0..32 {
        if mask & (1 << bit) != 0 {
            names.push(format!("bit{bit}"));
        }
    }
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join("+")
    }
}

fn describe_base(table: &DocumentStyleTable, record: &StyleRecord) -> String {
    let Some(base_id) = record.base_reference else {
        return "-> (no reference)".to_string();
    };
    let Some(base) = table.get(base_id) else {
        return format!("-> {base_id} (undefined)");
    };
    let name = base.name.as_deref().unwrap_or("(unnamed)");
    let look = base.symbology.map_or_else(
        || {
            if base.type_code == PSM_TYPE_CODE_JSTYLE_SIMPLE_FILL {
                format!("fill {}", base.fill_colour.map_or("-".to_string(), rgb))
            } else {
                "no line symbology".to_string()
            }
        },
        |s| format!("{} {}", mm(s.width_m), rgb(s.colour)),
    );
    let dash = base
        .dash_reference
        .and_then(|id| table.get(id))
        .filter(|d| d.type_code == DASH_TYPE)
        .and_then(|d| d.dash.map(|p| p.segments_mm()))
        .map_or_else(|| "solid".to_string(), |s| format!("dash {s:?}"));
    format!(
        "-> {base_id} 0x{:04X} {name:?} {look} {dash}",
        base.type_code
    )
}

/// Every reach of an override by a drawn record: `(storage, override id) ->
/// [(family, oid)]`.
type Reaches = BTreeMap<(String, u32), Vec<(&'static str, u32)>>;

fn reaches(doc: &PidDocument) -> Reaches {
    let mut out: Reaches = BTreeMap::new();
    let mut note =
        |storage: &str, table: &DocumentStyleTable, family: &'static str, oid: u32, index: u32| {
            if let Some(record) = table.get(index) {
                if record.type_code == OVERRIDE {
                    out.entry((storage.to_string(), index))
                        .or_default()
                        .push((family, oid));
                }
            }
        };
    for sheet in &doc.sheet_streams {
        let storage = pid_parse::style_link::storage_of_sheet(&sheet.path);
        let Some(table) = doc.style_tables.get(&storage) else {
            continue;
        };
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for r in &geometry.decoded_iglines {
            note(&storage, table, "igLine2d", r.oid, r.index);
        }
        for r in &geometry.decoded_igpoints {
            note(&storage, table, "igPoint2d", r.oid, r.index);
        }
        for r in &geometry.decoded_iglinestrings {
            note(&storage, table, "igLineString2d", r.oid, r.index);
        }
        for r in &geometry.decoded_igsymbols {
            note(&storage, table, "igSymbol2d", r.oid, r.style_ref);
        }
    }
    for site in &doc.jsites {
        let Some(table) = doc.style_tables.get(&site.path) else {
            continue;
        };
        let Some(nested) = site.nested_geometry.as_ref() else {
            continue;
        };
        for r in &nested.lines {
            note(&site.path, table, "body igLine2d", r.oid, r.index);
        }
        for r in &nested.polylines {
            note(&site.path, table, "body igLineString2d", r.oid, r.index);
        }
        for r in &nested.circles {
            note(&site.path, table, "body igCircle2d", r.oid, r.index);
        }
        for r in &nested.arcs {
            note(&site.path, table, "body igArc2d", r.oid, r.index);
        }
        for r in &nested.elliptical_arcs {
            note(&site.path, table, "body igEllipticalArc2d", r.oid, r.index);
        }
        for r in &nested.bsplines {
            note(&site.path, table, "body igBspCurve2d", r.oid, r.index);
        }
    }
    out
}

/// The look a style id gives after the override's fields apply (the base's
/// look when the id is not an override, or its bytes cannot be read).
fn after(table: &DocumentStyleTable, stream: Option<&Vec<u8>>, index: u32, now: &Look) -> Look {
    match (table.get(index), stream) {
        (Some(record), Some(stream)) if record.type_code == OVERRIDE => {
            match payload_of(stream, record).and_then(override_fields) {
                Some(fields) => applied(now, &fields, table),
                None => now.clone(),
            }
        }
        _ => now.clone(),
    }
}

/// Sheet-level census the ratchets count: every drawn record's look, today
/// and after.
fn census(
    doc: &PidDocument,
    streams: &BTreeMap<String, Vec<u8>>,
) -> (
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
    usize,
    usize,
) {
    let mut before: BTreeMap<String, usize> = BTreeMap::new();
    let mut after_map: BTreeMap<String, usize> = BTreeMap::new();
    let mut dashed_before = 0usize;
    let mut dashed_after = 0usize;
    for sheet in &doc.sheet_streams {
        let storage = pid_parse::style_link::storage_of_sheet(&sheet.path);
        let Some(table) = doc.style_tables.get(&storage) else {
            continue;
        };
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let stream = streams.get(&storage);
        let indices = geometry
            .decoded_iglines
            .iter()
            .map(|r| r.index)
            .chain(geometry.decoded_igpoints.iter().map(|r| r.index))
            .chain(geometry.decoded_iglinestrings.iter().map(|r| r.index))
            .chain(geometry.decoded_igsymbols.iter().map(|r| r.style_ref));
        for index in indices {
            let Some(now) = today(table, index) else {
                continue;
            };
            *before.entry(now.key()).or_default() += 1;
            if now.dash_mm.is_some() {
                dashed_before += 1;
            }
            let later = after(table, stream, index, &now);
            *after_map.entry(later.key()).or_default() += 1;
            if later.dash_mm.is_some() {
                dashed_after += 1;
            }
        }
    }
    (before, after_map, dashed_before, dashed_after)
}

/// Cached-body census: strokes of every `JSite*` body whose style changes,
/// grouped by what changes -- OpenCADStudio repaints a body's colour and
/// width in its placement's style, so for a body only the dash matters.
fn body_census(doc: &PidDocument, streams: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for site in &doc.jsites {
        let Some(table) = doc.style_tables.get(&site.path) else {
            continue;
        };
        let Some(nested) = site.nested_geometry.as_ref() else {
            continue;
        };
        let stream = streams.get(&site.path);
        let indices = nested
            .lines
            .iter()
            .map(|r| r.index)
            .chain(nested.polylines.iter().map(|r| r.index))
            .chain(nested.circles.iter().map(|r| r.index))
            .chain(nested.arcs.iter().map(|r| r.index))
            .chain(nested.elliptical_arcs.iter().map(|r| r.index))
            .chain(nested.bsplines.iter().map(|r| r.index));
        for index in indices {
            let Some(now) = today(table, index) else {
                continue;
            };
            let later = after(table, stream, index, &now);
            let key = match (now.dash_mm.is_some(), later.dash_mm.is_some()) {
                (true, false) => "dash -> solid",
                (false, true) => "solid -> dash",
                _ if later != now => "width / colour only",
                _ => "unchanged",
            };
            *out.entry(format!("{} {key}", site.path)).or_default() += 1;
        }
    }
    out
}

fn probe(path: &Path) {
    println!(
        "\n==================== {} ====================",
        path.display()
    );
    let doc = match PidParser::new().parse_file(path) {
        Ok(doc) => doc,
        Err(error) => {
            println!("  parse failed: {error}");
            return;
        }
    };
    let Ok(file) = std::fs::File::open(path) else {
        println!("  cannot open");
        return;
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        println!("  not a compound file");
        return;
    };
    // StyleCluster bytes per storage, keyed the way `style_tables` is.
    let mut streams: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for storage in doc.style_tables.keys() {
        let stream_path = if storage == "/" {
            "/StyleCluster".to_string()
        } else {
            format!("{storage}/StyleCluster")
        };
        if let Some(data) = read_stream(&mut cfb, &stream_path) {
            streams.insert(storage.clone(), data);
        }
    }

    let reached = reaches(&doc);

    // 1. every override, field by field.
    println!("\n-- 1. every 0x0030 JStyleOverride record, version-3 fields --");
    println!(
        "   columns: storage id oid len | +26 +30(mask) +34 +38 | +42 +50 +58 +66 (f64) | +74 +78 +82 | +86 +88 | base"
    );
    let mut total = 0usize;
    let mut mask_census: BTreeMap<(String, u32), usize> = BTreeMap::new();
    let mut slots_by_bit7: BTreeMap<bool, BTreeMap<usize, BTreeSet<u32>>> = BTreeMap::new();
    for (storage, table) in &doc.style_tables {
        let Some(stream) = streams.get(storage) else {
            continue;
        };
        for record in table.records() {
            if record.type_code != OVERRIDE {
                continue;
            }
            total += 1;
            let Some(payload) = payload_of(stream, record) else {
                println!("   {storage} {} (payload out of range)", record.style_id);
                continue;
            };
            let u32s: Vec<String> = U32_SLOTS
                .iter()
                .map(|at| u32_at(payload, *at).map_or("?".to_string(), |v| format!("{v:#x}")))
                .collect();
            let f64s: Vec<String> = F64_SLOTS
                .iter()
                .map(|at| f64_at(payload, *at).map_or("?".to_string(), |v| format!("{v:.6}")))
                .collect();
            let u16s: Vec<String> = U16_SLOTS
                .iter()
                .map(|at| u16_at(payload, *at).map_or("?".to_string(), |v| format!("{v:#x}")))
                .collect();
            let mask = u32_at(payload, MASK_AT).unwrap_or(0);
            let base_family = record.base_reference.and_then(|id| table.get(id)).map_or(
                "none".to_string(),
                |b| {
                    format!(
                        "0x{:04X}{}",
                        b.type_code,
                        if b.type_code == SIMPLE_LINE && b.dash_reference.is_some() {
                            " dashed"
                        } else {
                            ""
                        }
                    )
                },
            );
            *mask_census.entry((base_family.clone(), mask)).or_default() += 1;
            let reached_by = reached
                .get(&(storage.clone(), record.style_id))
                .map_or(0, Vec::len);
            println!(
                "   {storage:<10} {:>4} {:>6} {:>3} | {} {}={} {} {} | {} | {} | {} | {} | reached by {reached_by}",
                record.style_id,
                record.oid,
                payload.len(),
                u32s[0],
                u32s[1],
                bits(mask),
                u32s[2],
                u32s[3],
                f64s.join(" "),
                u32s[4..].join(" "),
                u16s.join(" "),
                describe_base(table, record),
            );
            let has_bit7 = mask & BIT_DASH != 0;
            for at in U32_SLOTS {
                if let Some(v) = u32_at(payload, at) {
                    slots_by_bit7
                        .entry(has_bit7)
                        .or_default()
                        .entry(at)
                        .or_default()
                        .insert(v);
                }
            }
        }
    }
    println!(
        "   {total} override record(s) in {} table(s)",
        doc.style_tables.len()
    );
    println!("\n   mask census (base family, mask) -> count:");
    for ((family, mask), count) in &mask_census {
        println!("     {family:<14} {mask:#010x} {:<24} {count}", bits(*mask));
    }

    // 2. which overrides geometry reaches.
    println!("\n-- 2. overrides reached by drawn records --");
    if reached.is_empty() {
        println!("   none");
    }
    for ((storage, id), users) in &reached {
        let mut by_family: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        for (family, oid) in users {
            by_family.entry(family).or_default().push(*oid);
        }
        let summary: Vec<String> = by_family
            .iter()
            .map(|(family, oids)| format!("{family} x{} {oids:?}", oids.len()))
            .collect();
        println!("   {storage} override {id}: {}", summary.join("; "));
    }

    // 3. before -> after per reached override.
    println!("\n-- 3. per reached override: the base's look today -> with the mask applied --");
    for ((storage, id), users) in &reached {
        let (Some(table), Some(stream)) = (doc.style_tables.get(storage), streams.get(storage))
        else {
            continue;
        };
        let Some(record) = table.get(*id) else {
            continue;
        };
        let Some(now) = today(table, *id) else {
            println!("   {storage} override {id}: resolves to no line style (fill or text base)");
            continue;
        };
        let hop = table
            .resolve_line_style(*id)
            .map(|r| r.hop)
            .unwrap_or(StyleHop::Direct);
        let Some(fields) = payload_of(stream, record).and_then(override_fields) else {
            continue;
        };
        let later = applied(&now, &fields, table);
        let changed = if later == now { "unchanged" } else { "CHANGES" };
        println!(
            "   {storage} override {id} ({hop:?}, {} record(s)): mask {} | today {} {} -> after {} {}  [{changed}]",
            users.len(),
            bits(fields.mask),
            now.key(),
            now.dash_key(),
            later.key(),
            later.dash_key(),
        );
        if fields.mask & BIT_WIDTH != 0 {
            println!("       +42 width  {}", mm(fields.width_m));
        }
        if fields.mask & BIT_COLOUR != 0 {
            println!(
                "       +34 colour {} ({:#x})",
                rgb(fields.colour),
                fields.colour
            );
        }
        if fields.mask & BIT_DASH != 0 {
            let named = if fields.dash_reference == 0 {
                "none (solid)".to_string()
            } else {
                table.get(fields.dash_reference).map_or_else(
                    || "names no style record".to_string(),
                    |r| {
                        format!(
                            "names style {} 0x{:04X} {:?} {}",
                            r.style_id,
                            r.type_code,
                            r.name.as_deref().unwrap_or("(unnamed)"),
                            r.dash
                                .map_or("-".to_string(), |d| format!("dash {:?}", d.segments_mm()))
                        )
                    },
                )
            };
            println!("       +78 dash   {:#x}: {named}", fields.dash_reference);
        }
    }

    // 4. palette and dash census, before and after.
    let (before, after_map, dashed_before, dashed_after) = census(&doc, &streams);
    println!(
        "\n-- 4. sheet-level palette (lines + points + polylines + placements), today -> after --"
    );
    let keys: BTreeSet<&String> = before.keys().chain(after_map.keys()).collect();
    for key in keys {
        let b = before.get(key).copied().unwrap_or(0);
        let a = after_map.get(key).copied().unwrap_or(0);
        let mark = if a == b { "" } else { "  <-- changes" };
        println!("   {key:<22} {b:>4} -> {a:>4}{mark}");
    }
    println!("   dashed sheet records: {dashed_before} -> {dashed_after}");
    println!("\n   cached-body strokes, by storage and what changes:");
    for (key, count) in body_census(&doc, &streams) {
        println!("   {key:<40} {count}");
    }

    // 5. dash slot corroboration.
    println!("\n-- 5. every u32 slot's distinct values, by whether bit7 is set --");
    for (has_bit7, slots) in &slots_by_bit7 {
        for (at, values) in slots {
            let shown: Vec<String> = values.iter().take(8).map(|v| format!("{v:#x}")).collect();
            println!(
                "   bit7={has_bit7:<5} +{at:<3} {} distinct value(s): {}",
                values.len(),
                shown.join(" ")
            );
        }
    }
}

fn main() {
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if path.exists() {
            probe(path);
        } else {
            println!("\n==================== {fixture} ==================== (absent, skipped)");
        }
    }
}
