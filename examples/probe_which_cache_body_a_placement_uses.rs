//! Which body inside the symbol-definition cache does a placement use?
//!
//! `probe_nested_site_curves_are_symbol_bodies` settled what the nested
//! `LdcSite` storages hold: the drawing's embedded copies of the symbol
//! bodies it places, in symbol-local coordinates, one layer group per body.
//! What it did not settle is the edge from a placement to its body. The
//! `igSymbol2d` record names a `JSite<id>` whose `JProperties` carry the
//! `.sym` path, and the root `PSMcluster0` holds one `0x00EC JFlavorManager`
//! per distinct symbol (D06 6, 0201 17, 0202 11, the process drawing 7 --
//! exactly the distinct-symbol counts) plus `0x0003 OLE membassy` proxies
//! whose payload `+4` names a placement. Somewhere along
//! `igSymbol2d -> JSite / JFlavorManager / membassy -> cache` a number has to
//! say which layer group of which cache storage is this symbol's body.
//!
//! The probe lays both sides out and looks for that number:
//!
//! 1. **Cache side.** For every `LdcSite` storage: its layer managers and
//!    their layers, the geometry each layer holds, and -- ground truth -- the
//!    `.sym` whose circles and arcs the layer reproduces. Plus every
//!    `0x0114 JSheet` / `0x0076 SheetView` / `0x0088 JSheetLayerGroup` /
//!    `0x00BD JSymbolInformation` object, since one of those is the likely
//!    handle a definition is referred to by.
//! 2. **Root side.** Every `0x00EC`, `0x0003`, `0x004F` record and every
//!    `igSymbol2d`, with the `.sym` name each placement resolves to through
//!    its `jsite_ref`.
//! 3. **The search.** Every `u32` at every byte offset of every root-side
//!    record that equals a cache-side handle (manager, sheet, view, group,
//!    symbol-information, or layer oid) is printed with what it hits. A real
//!    edge shows up as one offset that hits, for every placement whose body
//!    the curve match already identified, exactly that body's group.
//!
//! ```powershell
//! cargo run --quiet --example probe_which_cache_body_a_placement_uses
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_layers::{decode_sheet_layer_managers, decode_sheet_layers};
use pid_parse::parsers::sheet_records::{
    decode_igarcs, decode_igcircles, decode_igsymbols, sheet_record_starts,
};
use pid_parse::symbol_library::{SymbolLibrary, SymbolPrimitive};
use pid_parse::PidParser;

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];
const LIBRARY: &str = "test-file/symbols-full";
const CHAIN_MAGIC: u32 = 0x6C90_F544;

const LDC_SITE: u16 = 0x004A;
const SITE_RELATION: u16 = 0x004F;
const MEMBASSY: u16 = 0x0003;
const FLAVOR_MANAGER: u16 = 0x00EC;
const IGSYMBOL2D: u16 = 0x00CE;
const JSHEET: u16 = 0x0114;
const SHEET_VIEW: u16 = 0x0076;
const LAYER_GROUP: u16 = 0x0088;
const SYMBOL_INFORMATION: u16 = 0x00BD;
const LAYER: u16 = 0x0081;
const LAYER_MANAGER: u16 = 0x0042;

/// Geometry families whose payload `+8` is the layer they sit on.
const ON_A_LAYER: [u16; 11] = [
    0x0018, 0x0084, 0x005E, 0x004D, 0x0059, 0x0061, 0x0013, 0x0020, 0x005D, 0x0115, 0x003D,
];

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One record of a chain stream: `(oid, type_code, payload)`.
struct Rec {
    oid: u32,
    type_code: u16,
    payload: Vec<u8>,
}

fn walk(data: &[u8]) -> Vec<Rec> {
    let mut out = Vec::new();
    for at in sheet_record_starts(data) {
        let Some(type_code) = u16_at(data, at).map(|w| w & 0x3FFF) else {
            continue;
        };
        let Some(len) = u32_at(data, at + 2).map(|n| n as usize) else {
            continue;
        };
        let Some(payload) = data.get(at + 6..at + 6 + len) else {
            continue;
        };
        out.push(Rec {
            oid: u32_at(payload, 0).unwrap_or_default(),
            type_code,
            payload: payload.to_vec(),
        });
    }
    out
}

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    (u32_at(&data, 0) == Some(CHAIN_MAGIC)).then_some(data)
}

fn symbol_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn class_of(type_code: u16) -> &'static str {
    match type_code {
        MEMBASSY => "membassy",
        LDC_SITE => "LdcSite",
        SITE_RELATION => "Site-LdcSite relation",
        FLAVOR_MANAGER => "JFlavorManager",
        IGSYMBOL2D => "igSymbol2d",
        JSHEET => "JSheet",
        SHEET_VIEW => "SheetView",
        LAYER_GROUP => "JSheetLayerGroup",
        SYMBOL_INFORMATION => "JSymbolInformation",
        LAYER => "JSheetLayer",
        LAYER_MANAGER => "JSheetLayerManager",
        _ => "?",
    }
}

/// A curve reduced to comparable numbers, from either side.
#[derive(Clone, Copy)]
enum Curve {
    Circle((f64, f64), f64),
    Arc((f64, f64), f64, f64, f64),
}

impl Curve {
    fn same_as(&self, other: &Curve) -> bool {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-9;
        match (self, other) {
            (Curve::Circle(c, r), Curve::Circle(d, s)) => {
                close(c.0, d.0) && close(c.1, d.1) && close(*r, *s)
            }
            (Curve::Arc(c, r, a0, a1), Curve::Arc(d, s, b0, b1)) => {
                close(c.0, d.0)
                    && close(c.1, d.1)
                    && close(*r, *s)
                    && close(*a0, *b0)
                    && close(*a1, *b1)
            }
            _ => false,
        }
    }
}

/// What one cache-side handle is: its class and, for a layer, its name.
struct Handle {
    class: &'static str,
    label: String,
}

fn main() {
    for fixture in FIXTURES {
        if !Path::new(fixture).exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let Ok(doc) = PidParser::new().parse_file(fixture) else {
            println!("skip: {fixture} did not parse");
            continue;
        };
        let Ok(file) = std::fs::File::open(fixture) else {
            continue;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            continue;
        };
        println!(
            "\n==================== {} ====================",
            fixture.rsplit('/').next().unwrap_or(fixture)
        );

        // ---- root side -------------------------------------------------
        let root = read_stream(&mut cfb, "/PSMcluster0")
            .map(|d| walk(&d))
            .unwrap_or_default();
        let sheet = read_stream(&mut cfb, "/Sheet6").unwrap_or_default();
        let placements = decode_igsymbols(&sheet);
        let sheet_records = walk(&sheet);

        // JSite<id> -> .sym name, for the definition sites.
        let site_symbol: BTreeMap<u32, String> = doc
            .jsites
            .iter()
            .filter_map(|site| {
                let id: u32 = site.name.strip_prefix("JSite")?.parse().ok()?;
                let library = site
                    .symbol_path
                    .as_deref()
                    .or(site.local_symbol_path.as_deref())?;
                Some((id, symbol_name(library).to_string()))
            })
            .collect();

        // ---- cache side ------------------------------------------------
        let cache_ids: Vec<u32> = root
            .iter()
            .filter(|r| r.type_code == LDC_SITE)
            .map(|r| r.oid)
            .collect();
        let mut library = SymbolLibrary::new(LIBRARY);
        // .sym name -> its curves, for every symbol this drawing defines.
        let mut bodies: BTreeMap<String, Vec<Curve>> = BTreeMap::new();
        for site in &doc.jsites {
            let Some(path) = site
                .symbol_path
                .as_deref()
                .or(site.local_symbol_path.as_deref())
            else {
                continue;
            };
            if let Some(body) = library.resolve(path) {
                let curves = body
                    .primitives
                    .iter()
                    .filter_map(|p| match &p.primitive {
                        SymbolPrimitive::Circle { center, radius } => {
                            Some(Curve::Circle(*center, *radius))
                        }
                        SymbolPrimitive::Arc {
                            center,
                            radius,
                            start_angle,
                            end_angle,
                        } => Some(Curve::Arc(*center, *radius, *start_angle, *end_angle)),
                        _ => None,
                    })
                    .collect();
                bodies.insert(symbol_name(path).to_string(), curves);
            }
        }

        // Every cache-side handle, keyed by (cache id, oid).
        let mut handles: BTreeMap<(u32, u32), Handle> = BTreeMap::new();
        // Layer oid -> the .sym its curves reproduce (ground truth).
        let mut layer_body: BTreeMap<(u32, u32), BTreeSet<String>> = BTreeMap::new();

        for cache in &cache_ids {
            let path = format!("/JSite{cache}/PSMcluster0");
            let Some(data) = read_stream(&mut cfb, &path) else {
                println!("  {path}: no chain stream");
                continue;
            };
            let records = walk(&data);
            let layers = decode_sheet_layers(&data);
            let managers = decode_sheet_layer_managers(&data);
            let circles = decode_igcircles(&data);
            let arcs = decode_igarcs(&data);
            println!(
                "\n--- cache {path}: {} records, {} layer managers, {} layers, {} circles, {} arcs",
                records.len(),
                managers.len(),
                layers.len(),
                circles.len(),
                arcs.len()
            );
            // Which layer each geometry record sits on.
            let mut on_layer: BTreeMap<u32, BTreeMap<u16, usize>> = BTreeMap::new();
            for r in &records {
                if ON_A_LAYER.contains(&r.type_code) {
                    if let Some(layer) = u32_at(&r.payload, 8) {
                        *on_layer
                            .entry(layer)
                            .or_default()
                            .entry(r.type_code)
                            .or_default() += 1;
                    }
                }
            }
            // Ground truth per layer from the curves.
            for (layer, curve) in circles
                .iter()
                .map(|c| (c.sheet_layer_ref, Curve::Circle(c.center, c.radius)))
                .chain(arcs.iter().map(|a| {
                    (
                        a.sheet_layer_ref,
                        Curve::Arc(a.center, a.radius, a.start_angle, a.end_angle),
                    )
                }))
            {
                for (name, body) in &bodies {
                    if body.iter().any(|b| curve.same_as(b)) {
                        layer_body
                            .entry((*cache, layer))
                            .or_default()
                            .insert(name.clone());
                    }
                }
            }
            for manager in &managers {
                handles.insert(
                    (*cache, manager.oid),
                    Handle {
                        class: "JSheetLayerManager",
                        label: String::new(),
                    },
                );
            }
            // The layer managers and what each governs (layer.manager_oid comes
            // from the document's reconciled table).
            let table = doc
                .sheet_layers
                .get(&format!("/JSite{cache}"))
                .cloned()
                .unwrap_or_default();
            let mut by_manager: BTreeMap<Option<u32>, Vec<String>> = BTreeMap::new();
            for layer in &layers {
                let manager = table
                    .iter()
                    .find(|l| l.oid == layer.oid)
                    .and_then(|l| l.manager_oid);
                let geometry: Vec<String> = on_layer
                    .get(&layer.oid)
                    .map(|m| m.iter().map(|(t, n)| format!("0x{t:04X}x{n}")).collect())
                    .unwrap_or_default();
                let truth = layer_body
                    .get(&(*cache, layer.oid))
                    .map(|names| {
                        names
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join("|")
                    })
                    .unwrap_or_default();
                handles.insert(
                    (*cache, layer.oid),
                    Handle {
                        class: "JSheetLayer",
                        label: format!(
                            "{}{}",
                            layer.name,
                            if truth.is_empty() {
                                String::new()
                            } else {
                                format!(" = {truth}")
                            }
                        ),
                    },
                );
                if layer.object_count > 0 {
                    by_manager.entry(manager).or_default().push(format!(
                        "{}({})={} [{}]{}",
                        layer.name,
                        layer.oid,
                        layer.object_count,
                        geometry.join(" "),
                        if truth.is_empty() {
                            String::new()
                        } else {
                            format!(" <= {truth}")
                        }
                    ));
                }
            }
            for (manager, layers) in &by_manager {
                println!(
                    "  manager {}: {}",
                    manager.map_or("?".to_string(), |m| m.to_string()),
                    layers.join("  ")
                );
            }
            // The other handle-shaped objects in the cache.
            for r in &records {
                let class = match r.type_code {
                    JSHEET | SHEET_VIEW | LAYER_GROUP | SYMBOL_INFORMATION => class_of(r.type_code),
                    _ => continue,
                };
                let words: Vec<String> = (4..r.payload.len().saturating_sub(3))
                    .step_by(4)
                    .filter_map(|at| u32_at(&r.payload, at))
                    .filter(|w| *w != 0 && *w < 100_000)
                    .map(|w| w.to_string())
                    .collect();
                println!(
                    "  {class} oid {} len {}: small words {}",
                    r.oid,
                    r.payload.len(),
                    words.join(",")
                );
                handles.insert(
                    (*cache, r.oid),
                    Handle {
                        class,
                        label: String::new(),
                    },
                );
            }
        }

        // ---- root side listing ----------------------------------------
        println!("\n--- root PSMcluster0: definition-side objects");
        let mut placements_by_oid: BTreeMap<u32, (u32, String)> = BTreeMap::new();
        for p in &placements {
            let name = site_symbol
                .get(&p.jsite_ref)
                .cloned()
                .unwrap_or_else(|| format!("JSite{}?", p.jsite_ref));
            placements_by_oid.insert(p.oid, (p.jsite_ref, name));
        }
        let mut root_candidates: Vec<&Rec> = root
            .iter()
            .filter(|r| {
                matches!(
                    r.type_code,
                    FLAVOR_MANAGER | MEMBASSY | SITE_RELATION | LDC_SITE
                )
            })
            .collect();
        root_candidates.sort_by_key(|r| (r.type_code, r.oid));
        for r in &root_candidates {
            let words: Vec<String> = (4..r.payload.len().saturating_sub(3))
                .step_by(4)
                .filter_map(|at| u32_at(&r.payload, at).map(|w| (at, w)))
                .filter(|(_, w)| *w != 0 && *w < 100_000)
                .map(|(at, w)| {
                    let mut note = String::new();
                    if let Some((jsite, name)) = placements_by_oid.get(&w) {
                        note = format!("=placement({name},JSite{jsite})");
                    } else if let Some(name) = site_symbol.get(&w) {
                        note = format!("=JSite({name})");
                    } else if cache_ids.contains(&w) {
                        note = "=LdcSite".to_string();
                    }
                    format!("+{at}:{w}{note}")
                })
                .collect();
            println!(
                "  {:<22} oid {:<6} len {:<4} {}",
                class_of(r.type_code),
                r.oid,
                r.payload.len(),
                words.join(" ")
            );
        }

        // ---- the search -------------------------------------------------
        println!("\n--- root-side words that equal a cache-side handle");
        let mut hits_by_offset: BTreeMap<(u16, usize, &str), usize> = BTreeMap::new();
        let candidates: Vec<&Rec> = root_candidates
            .iter()
            .copied()
            .chain(sheet_records.iter().filter(|r| r.type_code == IGSYMBOL2D))
            .collect();
        for r in &candidates {
            let who = match r.type_code {
                IGSYMBOL2D => placements_by_oid
                    .get(&r.oid)
                    .map(|(_, name)| format!("igSymbol2d {} ({name})", r.oid))
                    .unwrap_or_else(|| format!("igSymbol2d {}", r.oid)),
                t => format!("{} {}", class_of(t), r.oid),
            };
            let mut lines = Vec::new();
            for at in 0..r.payload.len().saturating_sub(3) {
                let Some(w) = u32_at(&r.payload, at) else {
                    continue;
                };
                if w == 0 {
                    continue;
                }
                for cache in &cache_ids {
                    if let Some(handle) = handles.get(&(*cache, w)) {
                        // Layers without geometry are noise for this question.
                        if handle.class == "JSheetLayer" && !handle.label.contains(" = ") {
                            continue;
                        }
                        lines.push(format!(
                            "+{at}={w} -> JSite{cache} {} {}",
                            handle.class, handle.label
                        ));
                        *hits_by_offset
                            .entry((r.type_code, at, handle.class))
                            .or_default() += 1;
                    }
                }
            }
            if !lines.is_empty() {
                println!("  {who}: {}", lines.join(" ; "));
            }
        }
        println!("  hit histogram (type, offset, class) -> count:");
        for ((t, at, class), n) in &hits_by_offset {
            if *n >= 2 {
                println!("     {} +{at} {class}: {n}", class_of(*t));
            }
        }

        // ---- the igSymbol2d tail, laid out relative to the matrix -------
        // The payload is variable: the matrix tag sits at 33 or 35 and the six
        // doubles follow it, so a field behind them moves with the tag. Print
        // every placement's tail as u32 words from the end of the doubles, so
        // a fixed tail-relative offset shows as a column.
        println!("\n--- igSymbol2d tails (words after the six matrix doubles), with what each word is in the caches");
        for r in sheet_records.iter().filter(|r| r.type_code == IGSYMBOL2D) {
            let Some(tag_at) = r
                .payload
                .windows(4)
                .take(64)
                .position(|w| w == [0x02, 0x00, 0xA7, 0x50])
            else {
                continue;
            };
            let tail_at = tag_at + 4 + 48;
            let name = placements_by_oid
                .get(&r.oid)
                .map(|(jsite, name)| format!("{name} JSite{jsite}"))
                .unwrap_or_default();
            let words: Vec<String> = (tail_at..r.payload.len().saturating_sub(3))
                .step_by(4)
                .filter_map(|at| u32_at(&r.payload, at).map(|w| (at, w)))
                .map(|(at, w)| {
                    let mut hits: Vec<String> = Vec::new();
                    for cache in &cache_ids {
                        if let Some(handle) = handles.get(&(*cache, w)) {
                            if handle.class == "JSheet" || handle.class == "JSheetLayerManager" {
                                hits.push(format!("JSite{cache}.{}", handle.class));
                            }
                        }
                    }
                    format!(
                        "t+{}:{w}{}",
                        at - tail_at,
                        if hits.is_empty() {
                            String::new()
                        } else {
                            format!("[{}]", hits.join("|"))
                        }
                    )
                })
                .collect();
            println!(
                "  oid {:<6} len {:<4} tag@{tag_at} {:<44} {}",
                r.oid,
                r.payload.len(),
                name,
                words.join(" ")
            );
        }

        // ---- inside the cache: how a JSheet reaches its layer manager ----
        // The tail names a JSheet; the body's layers name a manager. The
        // cache's own space map records who references whom, so print the
        // incoming edges of every JSheet and every manager side by side.
        println!("\n--- inside each cache: space-map edges of the JSheets and the layer managers");
        for cache in &cache_ids {
            let key = format!("/JSite{cache}");
            // The map is split into 0x2000-wide segments, one stream each,
            // keyed by the segment's base id; an entry's document-wide id is
            // the base plus its index.
            let prefix = format!("{key}/PSMspacemap/");
            let mut edges: BTreeMap<u32, Vec<(u32, u16)>> = BTreeMap::new();
            for (stream, map) in &doc.psm_space_maps {
                let Some(base_hex) = stream.strip_prefix(&prefix) else {
                    continue;
                };
                let base = u32::from_str_radix(base_hex.trim_start_matches("0x"), 16).unwrap_or(0);
                for entry in &map.entries {
                    edges.insert(
                        base + u32::from(entry.index),
                        entry
                            .live_members()
                            .iter()
                            .map(|m| (m.value, m.tag))
                            .collect(),
                    );
                }
            }
            if edges.is_empty() {
                println!("  {key}: no space map parsed");
                continue;
            }
            let path = format!("/JSite{cache}/PSMcluster0");
            let Some(data) = read_stream(&mut cfb, &path) else {
                continue;
            };
            let records = walk(&data);
            let type_of: BTreeMap<u32, u16> =
                records.iter().map(|r| (r.oid, r.type_code)).collect();
            let describe = |oid: u32| -> String {
                let members: Vec<String> = edges
                    .get(&oid)
                    .map(|members| {
                        members
                            .iter()
                            .map(|(value, tag)| {
                                format!(
                                    "{}:{}(t{})",
                                    value,
                                    type_of.get(value).map(|t| class_of(*t)).unwrap_or("?"),
                                    tag
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                format!("<- [{}]", members.join(" "))
            };
            println!("  {key}");
            for r in records.iter().filter(|r| r.type_code == JSHEET) {
                println!("     JSheet {:<6} {}", r.oid, describe(r.oid));
            }
            for manager in decode_sheet_layer_managers(&data) {
                println!("     manager {:<5} {}", manager.oid, describe(manager.oid));
            }
        }
    }
}
