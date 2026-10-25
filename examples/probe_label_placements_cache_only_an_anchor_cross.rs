//! Which placements draw nothing but a hairline cross from the drawing's own
//! definition cache, and what does the file say about them? (OpenCADStudio
//! plan `2026-10-10-a-label-placement-keeps-its-anchor-cross-off-the-sheet.md`,
//! L0.)
//!
//! The 2026-10-09 comparison of `DWG-0201GP06-01` against a SmartPlant
//! screenshot found four `+` marks OpenCADStudio draws that the screenshot
//! does not show. They are the cached bodies of the sheet's label placements
//! -- `Design\Annotation\Labels\Item Note & Label.sym` and this plant's
//! `Design\Drawing Description.sym` -- each a pair of 0.01 mm lines, 2.54 mm
//! across, on the body's displayed `Default` layer. This probe lays out what
//! the file says about every placement so the rule can be read off the
//! corpus rather than off one screenshot:
//!
//! 1. every `igSymbol2d` placement of every fixture: its library path and
//!    whether a `Labels` / `Labels - …` folder (or the one label filed
//!    outside them) names it a label; the cached body it draws, how many of
//!    the body's strokes the file displays, the box those strokes span, the
//!    widest of them, and whether the displayed strokes are nothing but
//!    hairline lines (narrower than ISO's thinnest 0.13 mm pen);
//! 2. for each label placement, the strokes themselves and their style
//!    chain inside the cache storage -- the `JStyleOverride` the stroke
//!    names, its `+30` mask, the colour word at `+34`, the width at `+42`,
//!    and the base `JStyleSimpleLine`;
//! 3. a census over every `StyleCluster` of every fixture (root and
//!    `JSite*`): the overrides whose `+34` is the by-layer sentinel
//!    `0x80000000`, and the overrides and simple lines stating a hairline
//!    width -- to see whether the crosses' style is shared with anything
//!    else in the corpus;
//! 4. per fixture, the two counts the OpenCADStudio rule rests on, and the
//!    two ways it could miss: label placements whose body is more than the
//!    cross, and non-label placements whose body is only a cross.
//!
//! The probe changes nothing and asserts nothing; it prints.
//!
//! ```powershell
//! cargo run --example probe_label_placements_cache_only_an_anchor_cross
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::decode_iglines;
use pid_parse::style_link::DocumentStyleTable;
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{
    build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser, PidSymbolDefinition,
};

const FIXTURES: [&str; 6] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// PSM type code of `JStyleOverride`.
const OVERRIDE: u16 = 0x0030;
/// PSM type code of `JStyleSimpleLine`.
const SIMPLE_LINE: u16 = 0x002E;
/// Bytes of PSM envelope (`type_word` + `bytes_to_follow`) ahead of a payload.
const ENVELOPE: usize = 6;
/// `JStyleOverride` version-3 slots (`docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md`).
const MASK_AT: usize = 30;
const COLOUR_AT: usize = 34;
const WIDTH_AT: usize = 42;
const DASH_AT: usize = 78;
/// The colour word that means "take the layer's colour".
const BY_LAYER: u32 = 0x8000_0000;
/// Narrower than any ISO 128 pen (the thinnest is 0.13 mm): a hairline.
const UNDER_ISO_WIDTH_MM: f64 = 0.1;
/// The one label symbol of this plant filed outside a `Labels` folder.
const LABEL_SYMBOLS_FILED_ELSEWHERE: [&str; 1] = ["Drawing Description.sym"];

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

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

/// Whether a library path names a label symbol: a directory segment called
/// `Labels` or starting `Labels - `, or a file name on the filed-elsewhere
/// list. Directory segments only -- a symbol whose file name happens to
/// contain the word is not a label.
fn is_label_symbol(path: &str) -> bool {
    let segments: Vec<&str> = path.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    let Some((file, dirs)) = segments.split_last() else {
        return false;
    };
    dirs.iter()
        .any(|dir| *dir == "Labels" || dir.starts_with("Labels - "))
        || LABEL_SYMBOLS_FILED_ELSEWHERE
            .iter()
            .any(|name| file.eq_ignore_ascii_case(name))
}

fn leaf(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The box the displayed strokes span, in symbol-local metres.
fn visible_box(body: &PidSymbolDefinition) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    let mut add = |x: f64, y: f64| {
        b = Some(match b {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    };
    for primitive in body.visible_primitives() {
        match primitive {
            SymbolPrimitive::Line { start, end } => {
                add(start.0, start.1);
                add(end.0, end.1);
            }
            SymbolPrimitive::Circle { center, radius }
            | SymbolPrimitive::Arc { center, radius, .. } => {
                add(center.0 - radius, center.1 - radius);
                add(center.0 + radius, center.1 + radius);
            }
            SymbolPrimitive::Polyline { vertices, .. } => {
                for (x, y) in vertices {
                    add(*x, *y);
                }
            }
            SymbolPrimitive::BSpline { poles, .. } => {
                for (x, y) in poles {
                    add(*x, *y);
                }
            }
            SymbolPrimitive::Text { at, .. } => add(at.0, at.1),
            SymbolPrimitive::Boundary { members, .. } => {
                for member in members {
                    match member {
                        SymbolPrimitive::Circle { center, radius }
                        | SymbolPrimitive::Arc { center, radius, .. } => {
                            add(center.0 - radius, center.1 - radius);
                            add(center.0 + radius, center.1 + radius);
                        }
                        SymbolPrimitive::Line { start, end } => {
                            add(start.0, start.1);
                            add(end.0, end.1);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    b
}

/// Whether the body's displayed strokes are nothing but hairline lines.
fn is_anchor_cross_only(body: &PidSymbolDefinition) -> bool {
    let mut strokes = body.visible_strokes().peekable();
    strokes.peek().is_some()
        && strokes.all(|(primitive, style)| {
            matches!(primitive, SymbolPrimitive::Line { .. })
                && style.is_some_and(|style| style.width_mm < UNDER_ISO_WIDTH_MM)
        })
}

/// One `StyleCluster`, with the bytes kept so the raw override words can be
/// read back through each record's byte range.
struct Styles {
    table: DocumentStyleTable,
    bytes: Vec<u8>,
}

impl Styles {
    fn load(cfb: &mut CompoundFile<std::fs::File>, storage: &str) -> Option<Self> {
        let path = if storage == "/" {
            "/StyleCluster".to_string()
        } else {
            format!("{storage}/StyleCluster")
        };
        let bytes = read_stream(cfb, &path)?;
        Some(Self {
            table: DocumentStyleTable::from_stylecluster_bytes(&bytes),
            bytes,
        })
    }

    /// The raw version-3 words of an override: `(mask, colour, width_m, dash)`.
    fn override_words(&self, style_id: u32) -> Option<(u32, u32, f64, u32)> {
        let record = self.table.get(style_id)?;
        if record.type_code != OVERRIDE {
            return None;
        }
        let payload = self
            .bytes
            .get(record.byte_range.start + ENVELOPE..record.byte_range.end)?;
        Some((
            u32_at(payload, MASK_AT)?,
            u32_at(payload, COLOUR_AT)?,
            f64_at(payload, WIDTH_AT)?,
            u32_at(payload, DASH_AT).unwrap_or(0),
        ))
    }

    fn describe_chain(&self, style_id: u32) -> String {
        let Some(record) = self.table.get(style_id) else {
            return format!("style {style_id}: not in the table");
        };
        match record.type_code {
            OVERRIDE => {
                let (mask, colour, width_m, dash) =
                    self.override_words(style_id).unwrap_or_default();
                let base = record
                    .base_reference
                    .and_then(|id| self.table.get(id))
                    .map(|base| {
                        format!(
                            "base {} {:?} {}",
                            base.style_id,
                            base.name.as_deref().unwrap_or("-"),
                            base.symbology
                                .as_ref()
                                .map(|s| format!("{:.3}mm {:06X}", s.width_mm(), s.colour))
                                .unwrap_or_else(|| "no symbology".into())
                        )
                    })
                    .unwrap_or_else(|| "no base".into());
                format!(
                    "override {style_id}: mask 0x{mask:X} +34 {colour:08X}{} +42 {:.5} m +78 {dash}; {base}",
                    if colour == BY_LAYER { " (by layer)" } else { "" },
                    width_m
                )
            }
            SIMPLE_LINE => format!(
                "simple line {style_id} {:?} {}",
                record.name.as_deref().unwrap_or("-"),
                record
                    .symbology
                    .as_ref()
                    .map(|s| format!("{:.3}mm {:06X}", s.width_mm(), s.colour))
                    .unwrap_or_else(|| "no symbology".into())
            ),
            other => format!("style {style_id}: type 0x{other:04X}"),
        }
    }
}

fn main() {
    let mut corpus_by_layer = 0usize;
    let mut corpus_overrides = 0usize;
    for fixture in FIXTURES {
        if !Path::new(fixture).exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let Ok(parsed) = PidParser::with_options(ParseOptions::geometry()).parse_file(fixture)
        else {
            println!("skip: {fixture} did not parse");
            continue;
        };
        let Ok(file) = std::fs::File::open(fixture) else {
            continue;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            continue;
        };
        let geometry = build_normalized_geometry(&parsed);
        println!("\n==================== {fixture} ====================");

        // ---- 1. every placement ------------------------------------------
        println!("--- placements: oid | symbol | label? | body | displayed/total | box mm | widest | cross-only?");
        let mut styles: BTreeMap<String, Option<Styles>> = BTreeMap::new();
        let mut label_placements = 0usize;
        let mut cross_only = 0usize;
        let mut label_with_more = Vec::new();
        let mut cross_without_label = Vec::new();
        let mut label_details: Vec<String> = Vec::new();
        for entity in &geometry.entities {
            let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                definition,
                ..
            } = &entity.kind
            else {
                continue;
            };
            let path = symbol_path.as_deref().unwrap_or("");
            let label = is_label_symbol(path);
            let body = definition.and_then(|reference| geometry.symbol_definition(reference));
            let oid = entity
                .graphic_oid
                .map(|o| o.to_string())
                .unwrap_or_else(|| "?".into());
            let Some(body) = body else {
                println!(
                    "  {oid:<6} {:<36} {:<6} no cached body",
                    leaf(path),
                    if label { "LABEL" } else { "" }
                );
                continue;
            };
            let displayed = body.visible_strokes().count();
            let widest = body
                .visible_strokes()
                .filter_map(|(_, style)| style.map(|s| s.width_mm))
                .fold(0.0_f64, f64::max);
            let (w, h) = visible_box(body)
                .map(|(x0, y0, x1, y1)| ((x1 - x0) * 1000.0, (y1 - y0) * 1000.0))
                .unwrap_or((0.0, 0.0));
            let only_cross = is_anchor_cross_only(body);
            println!(
                "  {oid:<6} {:<36} {:<6} JSite{}/{:<5} {:>2}/{:<2} {:>6.2}x{:<6.2} {:.3}mm {}",
                leaf(path),
                if label { "LABEL" } else { "" },
                body.reference.site,
                body.reference.sheet,
                displayed,
                body.primitives.len(),
                w,
                h,
                widest,
                if only_cross { "CROSS-ONLY" } else { "" }
            );
            if label {
                label_placements += 1;
            }
            if only_cross {
                cross_only += 1;
            }
            if label && !only_cross {
                label_with_more.push(format!("{oid} {}", leaf(path)));
            }
            if only_cross && !label {
                cross_without_label.push(format!("{oid} {}", leaf(path)));
            }

            // ---- 2. the label bodies' strokes and style chains ----------
            if label {
                let storage = format!("/JSite{}", body.reference.site);
                let cluster = read_stream(&mut cfb, &format!("{storage}/PSMcluster0"));
                let lines = cluster.as_deref().map(decode_iglines).unwrap_or_default();
                let table = styles
                    .entry(storage.clone())
                    .or_insert_with(|| Styles::load(&mut cfb, &storage));
                let mut detail = format!(
                    "  oid {oid} {} at ({:.4}, {:.4}) m, body {storage} sheet {}: layers [{}]",
                    leaf(path),
                    insertion.x,
                    insertion.y,
                    body.reference.sheet,
                    body.sheet_layers
                        .iter()
                        .map(|l| format!(
                            "{}:{}={}",
                            l.oid,
                            l.name.as_deref().unwrap_or("?"),
                            l.displayed.map_or("?".to_string(), |d| d.to_string())
                        ))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                for (index, primitive) in body.primitives.iter().enumerate() {
                    let layer = body.primitive_layers.get(index).copied();
                    let shown = layer.is_none_or(|layer| body.layer_is_displayed(layer));
                    match primitive {
                        SymbolPrimitive::Line { start, end } => {
                            let record = lines.iter().find(|l| {
                                (l.start.0 - start.0).abs() < 1e-9
                                    && (l.start.1 - start.1).abs() < 1e-9
                                    && (l.end.0 - end.0).abs() < 1e-9
                                    && (l.end.1 - end.1).abs() < 1e-9
                                    && Some(l.sheet_layer_ref) == layer
                            });
                            let chain = match (record, table.as_ref()) {
                                (Some(record), Some(table)) => {
                                    format!(
                                        "style {} -> {}",
                                        record.index,
                                        table.describe_chain(record.index)
                                    )
                                }
                                (Some(record), None) => {
                                    format!("style {} (no StyleCluster)", record.index)
                                }
                                (None, _) => "record not matched".to_string(),
                            };
                            detail.push_str(&format!(
                                "\n     {} line ({:.5},{:.5})-({:.5},{:.5}) {:.2} mm on layer {:?}: {chain}",
                                if shown { "shown " } else { "hidden" },
                                start.0,
                                start.1,
                                end.0,
                                end.1,
                                ((end.0 - start.0).hypot(end.1 - start.1)) * 1000.0,
                                layer
                            ));
                        }
                        SymbolPrimitive::Text { text, .. } => detail.push_str(&format!(
                            "\n     {} text {:?} on layer {:?}",
                            if shown { "shown " } else { "hidden" },
                            text,
                            layer
                        )),
                        other => detail.push_str(&format!(
                            "\n     {} {:?} on layer {:?}",
                            if shown { "shown " } else { "hidden" },
                            std::mem::discriminant(other),
                            layer
                        )),
                    }
                }
                label_details.push(detail);
            }
        }
        if !label_details.is_empty() {
            println!("--- label placements, stroke by stroke:");
            for detail in &label_details {
                println!("{detail}");
            }
        }

        // ---- 3. census of every StyleCluster -------------------------------
        println!("--- style census (every storage):");
        let storages: Vec<String> = {
            let mut found = vec!["/".to_string()];
            found.extend(
                cfb.walk()
                    .filter(|entry| entry.is_storage() && entry.name().starts_with("JSite"))
                    .map(|entry| entry.path().to_string_lossy().replace('\\', "/")),
            );
            found.sort();
            found.dedup();
            found
        };
        let mut fixture_overrides = 0usize;
        for storage in &storages {
            let Some(styles) = Styles::load(&mut cfb, storage) else {
                continue;
            };
            for record in styles.table.records() {
                match record.type_code {
                    OVERRIDE => {
                        fixture_overrides += 1;
                        let Some((mask, colour, width_m, dash)) =
                            styles.override_words(record.style_id)
                        else {
                            continue;
                        };
                        let by_layer = colour == BY_LAYER;
                        let hairline = width_m > 0.0 && width_m * 1000.0 < UNDER_ISO_WIDTH_MM;
                        if by_layer {
                            corpus_by_layer += 1;
                        }
                        if by_layer || hairline {
                            println!(
                                "  {storage:<11} override {:<3} mask 0x{mask:<3X} +34 {colour:08X}{:<11} +42 {:.5} m{:<10} +78 {dash} base {:?}",
                                record.style_id,
                                if by_layer { " (by layer)" } else { "" },
                                width_m,
                                if hairline { " (hairline)" } else { "" },
                                record.base_reference
                            );
                        }
                    }
                    SIMPLE_LINE => {
                        if let Some(symbology) = &record.symbology {
                            if symbology.width_mm() < UNDER_ISO_WIDTH_MM {
                                println!(
                                    "  {storage:<11} simple line {:<3} {:?} {:.3}mm {:06X} (under ISO width)",
                                    record.style_id,
                                    record.name.as_deref().unwrap_or("-"),
                                    symbology.width_mm(),
                                    symbology.colour
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        corpus_overrides += fixture_overrides;

        // ---- 4. the rule's two counts and two misses ------------------------
        println!(
            "--- summary: {} placements, {label_placements} named labels by their path, {cross_only} whose displayed body is only hairline lines; \
             labels with more than a cross: {} {:?}; cross-only bodies outside labels: {} {:?}; overrides in {} storages: {fixture_overrides}",
            geometry
                .entities
                .iter()
                .filter(|e| matches!(e.kind, PidGraphicKind::SymbolInstance { .. }))
                .count(),
            label_with_more.len(),
            label_with_more,
            cross_without_label.len(),
            cross_without_label,
            storages.len()
        );
    }
    println!(
        "\n==================== corpus ====================\n  overrides {corpus_overrides}, of which stating the by-layer colour 0x80000000: {corpus_by_layer}"
    );
}
