//! Whose page is the geometry inside a nested `JSite` storage drawn on?
//!
//! `probe_the_chain_streams_the_pipeline_never_opens` priced the omission:
//! 51 chain streams and 4298 records the geometry pipeline never opens, of
//! which today's decoders would already claim 400 drawable records. The
//! obvious next move is to widen the admission rule in `streams/cluster.rs`
//! from "leaf name starts with `Sheet`" to "any chain stream". ADR-0003 §5
//! forbids exactly that:
//!
//! > emit document-level `PidGraphicEntity` circles only from top-level /
//! > registered symbol `Sheet*` paths. Nested `/JSite*` and StyleCluster hits
//! > remain typed audit / corpus evidence **until a separate ownership gate
//! > passes**.
//!
//! This probe is the measurement that gate needs. The question it has to
//! settle is not "are there records in there" — that is answered — but **what
//! coordinate space those records are in**, because the corpus already
//! contains a mechanism that would make them page-wrong: an `igSymbol2d`
//! placement carries a `jsite_ref`, a 2×2 matrix and an insertion point, so a
//! site named by a placement holds its graphics in *symbol-local* coordinates
//! and reaching the page requires that transform. Emitting such a site's lines
//! straight into the drawing would stack every symbol's glyph work at the
//! origin.
//!
//! Five questions:
//!
//! 1. **Which storages does the pipeline open today?** The companion probe
//!    printed only the closed half. The admission rule keys on the leaf name,
//!    not the storage, so it can already be reaching inside a `JSite` — and if
//!    it is, the ADR's boundary is not where the code's boundary is.
//! 2. **How many kinds of `JSite` are there?** Cross-tabulate three
//!    independent marks per site: does a placement name it (`jsite_ref`), does
//!    its `JProperties` carry a `.sym` library path, and does it hold a record
//!    chain of its own. A symbol definition should carry the first two and not
//!    the third.
//! 3. **The coordinate-space test.** For every chain-bearing storage, its own
//!    box against the box of the top-level sheets of the same fixture. Local
//!    symbol coordinates sit on the origin at glyph scale; page coordinates
//!    land inside the page and away from it.
//! 4. **The layer test.** A storage that declares its own `Default` /
//!    `Labels` / `DrawingBorder` layer set is a drawing, whatever else it is.
//!    A symbol's glyph library has no reason to carry one.
//! 5. **A verdict per storage**, from the four marks above — so the gate's
//!    output is a list of paths with reasons, not an average.
//! 6. **The duplication test.** Whatever the answer to 1–5, wiring is only
//!    worth doing if it adds strokes. If a nested site's segments are the ones
//!    the top-level sheets already draw, opening it double-counts the drawing
//!    instead of completing it.
//! 7. **What names a chain-bearing site?** If no placement does, something
//!    else must, or the content is unreachable. The storage name carries a
//!    number; ask whether the parent storage holds an object of that oid, and
//!    of what family. That test has a real null: an oid is not a small integer
//!    that turns up everywhere, and the answer is either a record or nothing.
//!
//! ```powershell
//! cargo run --example probe_who_owns_the_nested_jsite_geometry
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::model::{decode_all_families_into, SheetGeometry};
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::PidParser;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
/// `0x0081 JSheetLayer` — the family question 4 reads.
const LAYER: u16 = 0x0081;
/// `0x004A LdcSite` (`lcdef.dll`) — the family question 7 lands on.
const LDC_SITE: u16 = 0x004A;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn fixture_leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

/// The storage a stream lives in: everything up to the last separator, so
/// `/JSite145/PSMcluster0` becomes `/JSite145` and `/Sheet6` becomes `/`.
fn storage_of(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(at) => path[..at].to_string(),
    }
}

/// The `JSite<id>` a path sits under, when it sits under one.
fn jsite_of(path: &str) -> Option<u32> {
    path.split('/')
        .find_map(|part| part.strip_prefix("JSite")?.parse().ok())
}

/// One record-chain stream of one fixture.
struct Chain {
    path: String,
    records: usize,
    geometry: SheetGeometry,
    /// Every `0x0081` record of the stream, as (`oid`, name, object count).
    layers: Vec<(u32, String, u32)>,
    /// Every record of the stream as (`oid`, `type_code`) — the chain walk's
    /// own inventory, so question 7 can ask what an id resolves to.
    objects: Vec<(u32, u16)>,
    /// The payload of every `0x004A LdcSite` record, with its oid.
    site_records: Vec<(Vec<u8>, u32)>,
}

impl Chain {
    /// The pipeline's entire admission rule, quoted from `streams/cluster.rs`.
    fn is_opened_by_the_pipeline(&self) -> bool {
        self.path
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .starts_with("Sheet")
    }
}

/// One symbol placement, reduced to the three fields the gate needs.
struct Placement {
    jsite_ref: u32,
    x: f64,
    y: f64,
}

/// What one fixture contributes.
struct Doc {
    fixture: &'static str,
    chains: Vec<Chain>,
    placements: Vec<Placement>,
    /// Top-level `JSite<id>` → the `.sym` library path its `JProperties`
    /// names, for the sites that name one.
    symbol_paths: BTreeMap<u32, String>,
}

/// Every `u32 n` followed by `n` plausible UTF-16LE characters — the shape the
/// corpus uses for names everywhere else. Same reader as
/// `probe_sheetlayer_edge_lives_on_the_graphic`.
fn first_utf16_name(payload: &[u8]) -> String {
    let mut at = 0usize;
    while at + 4 < payload.len() {
        if let Some(count) = u32_at(payload, at).map(|count| count as usize) {
            if (1..=128).contains(&count) && at + 4 + count * 2 <= payload.len() {
                let units: Vec<u16> = (0..count)
                    .filter_map(|unit| u16_at(payload, at + 4 + unit * 2))
                    .collect();
                if units.len() == count {
                    if let Ok(text) = String::from_utf16(&units) {
                        if text
                            .chars()
                            .all(|c| c.is_alphanumeric() || " _-.()/&#".contains(c))
                        {
                            return text;
                        }
                    }
                }
            }
        }
        at += 1;
    }
    String::new()
}

/// Every `0x0081 JSheetLayer` a chain stream declares, and every record's
/// (`oid`, `type_code`) — one walk, since both read the same chain.
#[allow(clippy::type_complexity)]
fn walk(
    data: &[u8],
) -> (
    Vec<(u32, String, u32)>,
    Vec<(u32, u16)>,
    Vec<(Vec<u8>, u32)>,
) {
    let mut layers = Vec::new();
    let mut objects = Vec::new();
    let mut sites = Vec::new();
    for at in sheet_record_starts(data) {
        let Some(type_code) = u16_at(data, at).map(|word| word & 0x3FFF) else {
            continue;
        };
        let Some(len) = u32_at(data, at + 2).map(|len| len as usize) else {
            continue;
        };
        let Some(payload) = data.get(at + 6..at + 6 + len) else {
            continue;
        };
        let oid = u32_at(payload, 0).unwrap_or_default();
        objects.push((oid, type_code));
        if type_code == LAYER {
            layers.push((
                oid,
                first_utf16_name(payload),
                u32_at(payload, 12).unwrap_or_default(),
            ));
        }
        if type_code == LDC_SITE {
            sites.push((payload.to_vec(), oid));
        }
    }
    (layers, objects, sites)
}

fn load(fixture: &'static str) -> Option<Doc> {
    let path = Path::new(fixture);
    if !path.exists() {
        println!("skip: {fixture} is absent");
        return None;
    }
    let parsed = match PidParser::new().parse_file(fixture) {
        Ok(doc) => doc,
        Err(e) => {
            println!("skip: {fixture} did not parse: {e}");
            return None;
        }
    };
    let symbol_paths: BTreeMap<u32, String> = parsed
        .jsites
        .iter()
        .filter_map(|site| {
            let id: u32 = site.name.strip_prefix("JSite")?.parse().ok()?;
            let library = site
                .symbol_path
                .as_deref()
                .or(site.local_symbol_path.as_deref())?;
            Some((id, library.to_string()))
        })
        .collect();

    let file = std::fs::File::open(path).ok()?;
    let mut cfb = CompoundFile::open(file).ok()?;
    let stream_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();

    let mut chains = Vec::new();
    let mut placements = Vec::new();
    for stream_path in stream_paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        if u32_at(&data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let records = sheet_record_starts(&data).len();
        if records == 0 {
            continue;
        }
        let mut geometry = SheetGeometry::default();
        decode_all_families_into(&data, &mut geometry);
        for symbol in &geometry.decoded_igsymbols {
            placements.push(Placement {
                jsite_ref: symbol.jsite_ref,
                x: symbol.insertion_x,
                y: symbol.insertion_y,
            });
        }
        let (layers, objects, site_records) = walk(&data);
        chains.push(Chain {
            path: stream_path,
            records,
            layers,
            objects,
            site_records,
            geometry,
        });
    }

    Some(Doc {
        fixture,
        chains,
        placements,
        symbol_paths,
    })
}

/// A coordinate box, or nothing when no record contributed a point.
#[derive(Default, Clone)]
struct Box2d {
    min: Option<(f64, f64)>,
    max: Option<(f64, f64)>,
    points: usize,
}

impl Box2d {
    fn add(&mut self, x: f64, y: f64) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        self.points += 1;
        self.min = Some(match self.min {
            None => (x, y),
            Some((mx, my)) => (mx.min(x), my.min(y)),
        });
        self.max = Some(match self.max {
            None => (x, y),
            Some((mx, my)) => (mx.max(x), my.max(y)),
        });
    }

    fn merge(&mut self, other: &Box2d) {
        if let (Some((x0, y0)), Some((x1, y1))) = (other.min, other.max) {
            let before = self.points;
            self.add(x0, y0);
            self.add(x1, y1);
            self.points = before + other.points;
        }
    }

    fn text(&self) -> String {
        match (self.min, self.max) {
            (Some((x0, y0)), Some((x1, y1))) => {
                format!(
                    "{:>4} pts  x {x0:>7.4}..{x1:<7.4} y {y0:>7.4}..{y1:<7.4}",
                    self.points
                )
            }
            _ => "   0 pts  (no coordinates)".to_string(),
        }
    }

    /// The largest side of the box. A glyph is small; a page is not.
    fn span(&self) -> Option<f64> {
        let ((x0, y0), (x1, y1)) = (self.min?, self.max?);
        Some((x1 - x0).max(y1 - y0))
    }

    /// Whether the box straddles the origin — the signature of geometry drawn
    /// around a symbol's own insertion point rather than placed on a page.
    fn holds_origin(&self) -> bool {
        match (self.min, self.max) {
            (Some((x0, y0)), Some((x1, y1))) => x0 <= 0.0 && x1 >= 0.0 && y0 <= 0.0 && y1 >= 0.0,
            _ => false,
        }
    }
}

/// Every coordinate the geometry-emitting families put on the page.
fn box_of(geometry: &SheetGeometry) -> Box2d {
    let mut extent = Box2d::default();
    for record in &geometry.decoded_iglines {
        extent.add(record.start_x, record.start_y);
        extent.add(record.end_x, record.end_y);
    }
    for record in &geometry.decoded_igpoints {
        extent.add(record.x, record.y);
    }
    for record in &geometry.decoded_igtextboxes {
        extent.add(record.trailing_double_1, record.trailing_double_2);
    }
    for record in &geometry.decoded_iglinestrings {
        for (x, y) in record.vertex_xs.iter().zip(&record.vertex_ys) {
            extent.add(*x, *y);
        }
    }
    for record in &geometry.decoded_igsymbols {
        extent.add(record.insertion_x, record.insertion_y);
    }
    extent
}

fn main() {
    let docs: Vec<Doc> = FIXTURES.iter().filter_map(|f| load(f)).collect();
    if docs.is_empty() {
        println!("no fixture available; nothing to measure");
        return;
    }
    what_the_pipeline_opens(&docs);
    the_two_kinds_of_jsite(&docs);
    the_coordinate_space_test(&docs);
    the_layer_test(&docs);
    the_verdict(&docs);
    the_duplication_test(&docs);
    what_names_the_site(&docs);
    the_site_record_anatomy(&docs);
}

/// Question 8: if a site owns a coordinate system, the record that stands for
/// it in the parent is where the site → page transform would live. Print every
/// plausible `f64` in the `0x004A LdcSite` payload next to the site's own box,
/// so a translation that lands the box on the page is visible if it is there.
fn the_site_record_anatomy(docs: &[Doc]) {
    println!("\n=== 8. the LdcSite record's own payload, against the site's box ===");
    for doc in docs {
        for chain in doc.chains.iter().filter(|c| jsite_of(&c.path).is_none()) {
            for (at, oid) in chain.site_records.iter() {
                let Some(site) = doc
                    .chains
                    .iter()
                    .find(|c| jsite_of(&c.path) == Some(*oid) && c.path.ends_with("PSMcluster0"))
                else {
                    continue;
                };
                let extent = box_of(&site.geometry);
                let doubles: Vec<String> = at
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .enumerate()
                    .filter_map(|(index, word)| {
                        let value = f64::from_le_bytes(*word);
                        (value.is_finite() && value != 0.0 && value.abs() < 10.0)
                            .then(|| format!("+{}={value:.4}", index * 8))
                    })
                    .collect();
                println!(
                    "  {}  LdcSite oid {oid}: payload {} bytes",
                    fixture_leaf(doc.fixture),
                    at.len()
                );
                println!("     its site's box:  {}", extent.text());
                println!(
                    "     f64 candidates:  {}",
                    if doubles.is_empty() {
                        "none".to_string()
                    } else {
                        doubles.join(" ")
                    }
                );
            }
        }
    }
}

/// The class the RAD registry gives the families this probe prints.
fn class_of(type_code: u16) -> &'static str {
    match type_code {
        0x0013 => "igBoundary2d",
        0x0018 => "igLine2d",
        0x0020 => "igRectangle2d",
        0x0030 => "JStyleOverride",
        0x0042 => "JSheetLayerManager",
        0x004A => "LdcSite",
        0x004C => "OLE client site",
        0x004D => "igTextBox",
        0x0057 => "Top ViewFilterSet",
        0x0059 => "igCircle2d",
        0x005E => "igPoint2d",
        0x0060 => "Top ViewFilterSet'",
        0x0061 => "igArc2d",
        0x0076 => "SheetView",
        0x007B => "Group",
        0x0081 => "JSheetLayer",
        0x0084 => "igLineString2d",
        0x0088 => "JSheetLayerGroup",
        0x0089 => "DA row",
        0x00CE => "igSymbol2d",
        0x00FA => "DependencyObject",
        0x0114 => "JSheet",
        0x0115 => "igDimension",
        0x3FE6 => "GLine2d",
        _ => "?",
    }
}

/// Question 7: the storage name is `JSite<n>`. Does the parent storage hold an
/// object whose oid is that `n`, and what family is it?
fn what_names_the_site(docs: &[Doc]) {
    println!("\n=== 7. does the parent storage hold an object of the site's own id? ===");
    for doc in docs {
        // Every object of the root storage, by oid — the chain streams whose
        // path has no `JSite` component.
        let mut root: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
        for chain in doc.chains.iter().filter(|c| jsite_of(&c.path).is_none()) {
            for (oid, type_code) in &chain.objects {
                root.entry(*oid).or_default().insert(*type_code);
            }
        }
        let sites: BTreeSet<u32> = doc
            .chains
            .iter()
            .filter_map(|c| jsite_of(&c.path))
            .collect();
        println!(
            "  {}: {} root objects, {} chain-bearing sites",
            fixture_leaf(doc.fixture),
            root.len(),
            sites.len()
        );
        for id in &sites {
            let families: Vec<String> = root
                .get(id)
                .map(|codes| {
                    codes
                        .iter()
                        .map(|code| format!("0x{code:04X} {}", class_of(*code)))
                        .collect()
                })
                .unwrap_or_default();
            println!(
                "     JSite{id} -> root object {id}: {}",
                if families.is_empty() {
                    "absent".to_string()
                } else {
                    families.join(" + ")
                }
            );
        }
        // The null: how often does a *random* id of the same magnitude hit a
        // root object at all? Without this the hits above mean nothing.
        let hits = sites.iter().filter(|id| root.contains_key(id)).count();
        let highest = root.keys().max().copied().unwrap_or(1).max(1);
        println!(
            "     {hits}/{} sites resolve; the root's oid space is 1..{highest}, so a random id \
             would resolve {:.0}% of the time",
            sites.len(),
            100.0 * root.len() as f64 / f64::from(highest)
        );
        // And the other direction: every root object of the family the hits
        // landed on should have a storage, or the correspondence is one-way.
        for family in [0x004Au16, 0x004C] {
            let of_family: BTreeSet<u32> = root
                .iter()
                .filter(|(_, codes)| codes.contains(&family))
                .map(|(oid, _)| *oid)
                .collect();
            if of_family.is_empty() {
                continue;
            }
            let without: Vec<String> = of_family
                .iter()
                .filter(|oid| !sites.contains(oid))
                .map(u32::to_string)
                .collect();
            println!(
                "     root holds {} 0x{family:04X} {} objects; {} have a chain-bearing storage{}",
                of_family.len(),
                class_of(family),
                of_family.len() - without.len(),
                if without.is_empty() {
                    String::new()
                } else {
                    format!(" — no storage for oid {}", without.join(", "))
                }
            );
        }
    }
}

/// Every line segment a stream draws, rounded so two records that state the
/// same segment compare equal without an epsilon sweep. The corpus stores
/// metres; `1e-9` is a nanometre, far below any drafting tolerance and far
/// above f64 noise on numbers of this size.
fn segments_of(geometry: &SheetGeometry) -> BTreeSet<(i64, i64, i64, i64)> {
    let quantise = |value: f64| (value * 1e9).round() as i64;
    let mut out = BTreeSet::new();
    for record in &geometry.decoded_iglines {
        let a = (quantise(record.start_x), quantise(record.start_y));
        let b = (quantise(record.end_x), quantise(record.end_y));
        // Direction is not part of the stroke: order the endpoints so a
        // segment redrawn the other way round still counts as the same one.
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        out.insert((a.0, a.1, b.0, b.1));
    }
    out
}

/// Question 6: does opening these streams add strokes or repeat them?
fn the_duplication_test(docs: &[Doc]) {
    println!("\n=== 6. do the closed streams repeat the opened ones' segments? ===");
    for doc in docs {
        let mut drawn: BTreeSet<(i64, i64, i64, i64)> = BTreeSet::new();
        for chain in doc.chains.iter().filter(|c| c.is_opened_by_the_pipeline()) {
            drawn.extend(segments_of(&chain.geometry));
        }
        println!(
            "  {}: the opened sheets draw {} distinct segments",
            fixture_leaf(doc.fixture),
            drawn.len()
        );
        for chain in doc.chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
            let mine = segments_of(&chain.geometry);
            if mine.is_empty() {
                continue;
            }
            let repeated = mine.intersection(&drawn).count();
            println!(
                "     {:<32} {} segments, {repeated} of them already drawn ({}% new)",
                chain.path,
                mine.len(),
                100 * (mine.len() - repeated) / mine.len()
            );
        }
    }
}

/// Question 1: the admission rule names leaves, so print the storages it
/// admits. If it already reaches inside a `JSite`, the ADR's boundary and the
/// code's boundary are not the same line.
fn what_the_pipeline_opens(docs: &[Doc]) {
    println!("=== 1. the storages the `Sheet` prefix rule admits ===");
    for doc in docs {
        let opened: Vec<&Chain> = doc
            .chains
            .iter()
            .filter(|chain| chain.is_opened_by_the_pipeline())
            .collect();
        let closed: Vec<&Chain> = doc
            .chains
            .iter()
            .filter(|chain| !chain.is_opened_by_the_pipeline())
            .collect();
        println!(
            "  {}: {} opened / {} closed",
            fixture_leaf(doc.fixture),
            opened.len(),
            closed.len()
        );
        for chain in opened {
            println!(
                "     OPEN  {:<34} x{:<5} storage {}{}",
                chain.path,
                chain.records,
                storage_of(&chain.path),
                match jsite_of(&chain.path) {
                    Some(id) => format!("   <-- inside JSite{id}"),
                    None => String::new(),
                }
            );
        }
    }
}

/// Question 2: three independent marks per site. A symbol definition is named
/// by a placement and carries a `.sym`; a nested drawing carries a chain.
fn the_two_kinds_of_jsite(docs: &[Doc]) {
    println!("\n=== 2. every JSite by three marks: named / has .sym / has a chain ===");
    for doc in docs {
        let named: BTreeMap<u32, usize> =
            doc.placements
                .iter()
                .fold(BTreeMap::new(), |mut acc, placement| {
                    *acc.entry(placement.jsite_ref).or_default() += 1;
                    acc
                });
        let mut chained: BTreeMap<u32, usize> = BTreeMap::new();
        for chain in &doc.chains {
            if let Some(id) = jsite_of(&chain.path) {
                *chained.entry(id).or_default() += chain.records;
            }
        }
        let sites: BTreeSet<u32> = named
            .keys()
            .chain(doc.symbol_paths.keys())
            .chain(chained.keys())
            .copied()
            .collect();
        let both = sites
            .iter()
            .filter(|id| named.contains_key(id) && chained.contains_key(id))
            .count();
        println!(
            "  {}: {} sites — {} named by a placement, {} carry a .sym, {} carry a chain, {both} \
             carry both a placement and a chain",
            fixture_leaf(doc.fixture),
            sites.len(),
            named.len(),
            doc.symbol_paths.len(),
            chained.len(),
        );
        for id in &sites {
            let records = chained.get(id).copied().unwrap_or_default();
            if records == 0 {
                continue;
            }
            println!(
                "     JSite{id}: {records} chain records, named by {} placement(s), .sym {}",
                named.get(id).copied().unwrap_or_default(),
                match doc.symbol_paths.get(id) {
                    Some(library) => library.rsplit('\\').next().unwrap_or(library),
                    None => "-",
                }
            );
        }
        let quiet = sites.len() - sites.iter().filter(|id| chained.contains_key(id)).count();
        println!("     … and {quiet} sites with no record chain at all");
    }
}

/// Question 3: local coordinates sit on the origin at glyph scale; page
/// coordinates sit inside the page. Print both boxes side by side.
fn the_coordinate_space_test(docs: &[Doc]) {
    println!("\n=== 3. the coordinate box of every chain stream against its fixture's page ===");
    for doc in docs {
        let mut page = Box2d::default();
        for chain in doc.chains.iter().filter(|c| c.is_opened_by_the_pipeline()) {
            page.merge(&box_of(&chain.geometry));
        }
        println!("  {}", fixture_leaf(doc.fixture));
        println!("     the opened sheets:      {}", page.text());
        for chain in doc.chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
            let extent = box_of(&chain.geometry);
            if extent.points == 0 {
                continue;
            }
            println!(
                "     {:<32} {}{}",
                chain.path,
                extent.text(),
                if extent.holds_origin() {
                    "  origin inside"
                } else {
                    ""
                }
            );
        }
        // Where the placements of this fixture put their symbols, for scale:
        // a site whose own box is the size of one of these gaps is a glyph.
        let mut insertions = Box2d::default();
        for placement in &doc.placements {
            insertions.add(placement.x, placement.y);
        }
        println!(
            "     {} placements insert at:  {}",
            doc.placements.len(),
            insertions.text()
        );
    }
}

/// Question 4: a storage that declares its own layer set is a drawing.
fn the_layer_test(docs: &[Doc]) {
    println!("\n=== 4. the layers each chain stream declares ===");
    for doc in docs {
        for chain in &doc.chains {
            if chain.layers.is_empty() {
                continue;
            }
            let live: Vec<String> = chain
                .layers
                .iter()
                .filter(|(_, _, objects)| *objects > 0)
                .map(|(oid, name, objects)| format!("{name}({oid})={objects}"))
                .collect();
            println!(
                "  {}{}: {} layers, {} of them hold objects{}",
                fixture_leaf(doc.fixture),
                chain.path,
                chain.layers.len(),
                live.len(),
                if live.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", live.join(" "))
                }
            );
        }
    }
}

/// Question 5: one line per closed chain stream that would newly draw, with
/// the reason it should or should not.
fn the_verdict(docs: &[Doc]) {
    println!("\n=== 5. verdict per closed stream that would newly draw ===");
    for doc in docs {
        let named: BTreeSet<u32> = doc
            .placements
            .iter()
            .map(|placement| placement.jsite_ref)
            .collect();
        let mut page = Box2d::default();
        for chain in doc.chains.iter().filter(|c| c.is_opened_by_the_pipeline()) {
            page.merge(&box_of(&chain.geometry));
        }
        for chain in doc.chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
            let extent = box_of(&chain.geometry);
            if extent.points == 0 {
                continue;
            }
            let site = jsite_of(&chain.path);
            let mut reasons: Vec<String> = Vec::new();
            if let Some(id) = site {
                if named.contains(&id) {
                    reasons.push(format!("a placement names JSite{id}: symbol-local"));
                } else {
                    reasons.push(format!("no placement names JSite{id}"));
                }
            }
            if chain.path.ends_with("StyleCluster") {
                reasons.push("style library, not a sheet".to_string());
            }
            if !chain.layers.is_empty() {
                reasons.push(format!("declares {} layers of its own", chain.layers.len()));
            }
            match (extent.span(), page.span()) {
                (Some(mine), Some(theirs)) if theirs > 0.0 => {
                    reasons.push(format!(
                        "span {:.0}% of the opened sheets'",
                        100.0 * mine / theirs
                    ));
                }
                _ => {}
            }
            if extent.holds_origin() {
                reasons.push("box holds the origin".to_string());
            }
            println!(
                "  {}{}: {}",
                fixture_leaf(doc.fixture),
                chain.path,
                reasons.join("; ")
            );
        }
    }
}
