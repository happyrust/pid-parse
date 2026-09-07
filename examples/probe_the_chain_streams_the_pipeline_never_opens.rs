//! Which record chains does the geometry pipeline never open, and what is in
//! them?
//!
//! `streams/cluster.rs` hands a stream to the decoder registry on one
//! condition: its leaf name starts with `Sheet`. Everything else — the
//! top-level `PSMcluster0`, `StyleCluster`, and every `PSMcluster0` nested
//! inside a `JSite<n>` storage — is read for its header and its string table
//! and then closed. Those streams are not empty: `2026-08-27-aux-hi-is-the-
//! sheet-layer.md` §7 found 12 circles, 12 arcs, 3 rectangles, one BspCurve
//! and nine `igBoundary2d` sitting in them, each one on a sheet layer.
//!
//! Before anything is wired, the size of the omission has to be a number.
//! Five questions:
//!
//! 1. **Which chain streams does the pipeline open, and which does it not?**
//!    A chain stream is one that opens with the chain magic and yields record
//!    starts; the `Sheet` prefix is the whole of the current admission rule.
//! 2. **What do the unopened ones hold?** Full type-code census with the RAD
//!    class name, taken with an empty claimed set so nothing is filtered.
//! 3. **What would today's registry decode from them, unchanged?** The
//!    families are already written; the question is only what they would
//!    claim if handed these bytes. That is the free half of the gap.
//! 4. **What would remain?** Codes with no decoder at all, and records a
//!    wired family walked over and refused. The first needs a new decoder,
//!    the second needs an existing one's rules revisited.
//! 5. **Do the numbers that come out look like the same drawing?** A decoder
//!    aimed at bytes it was never tested on can produce well-formed nonsense.
//!    The coordinate box of what the unopened streams would yield is printed
//!    next to the box of what the same fixture's opened streams already
//!    yield: same drawing, or not.
//!
//! ```powershell
//! cargo run --example probe_the_chain_streams_the_pipeline_never_opens
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::model::{decode_all_families_into, SheetGeometry, SHEET_RECORD_FAMILIES};
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::{
    is_native_graphic_type_code, rad_class_name, refused_record_census, undecoded_type_code_census,
};

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;

/// One chain stream of one fixture, bytes kept whole.
struct Chain {
    fixture: &'static str,
    path: String,
    data: Vec<u8>,
    /// How many record starts the chain walker finds — the denominator every
    /// later count is measured against.
    records: usize,
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

    fn leaf(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

fn fixture_leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Every chain stream of every fixture that exists on this machine.
fn load() -> Vec<Chain> {
    let mut out = Vec::new();
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            continue;
        };
        let stream_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
            .collect();
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
            out.push(Chain {
                fixture,
                path: stream_path,
                data,
                records,
            });
        }
    }
    out
}

/// Every record of a stream by type code, from the chain walk the record
/// decoders themselves use. This is the authoritative denominator.
fn chain_census(data: &[u8]) -> BTreeMap<u16, usize> {
    let mut out: BTreeMap<u16, usize> = BTreeMap::new();
    for at in sheet_record_starts(data) {
        let Some(word) = data.get(at..at + 2) else {
            continue;
        };
        let type_code = u16::from_le_bytes([word[0], word[1]]) & 0x3FFF;
        *out.entry(type_code).or_default() += 1;
    }
    out
}

/// The same inventory as [`chain_census`], but taken with the census walk —
/// the instrument that reports the residue to callers. Run with an empty
/// claimed set it should see every record; where it does not, the residue
/// report is the thing that is wrong, not the stream.
fn census_walk_census(data: &[u8]) -> BTreeMap<u16, usize> {
    let mut out: BTreeMap<u16, usize> = BTreeMap::new();
    for row in undecoded_type_code_census(data, &[]) {
        *out.entry(row.type_code).or_default() += row.count;
    }
    for row in refused_record_census(data, &[]) {
        *out.entry(row.type_code).or_default() += row.count;
    }
    out
}

fn decode(data: &[u8]) -> SheetGeometry {
    let mut geometry = SheetGeometry::default();
    decode_all_families_into(data, &mut geometry);
    geometry
}

fn main() {
    let chains = load();
    if chains.is_empty() {
        println!("no fixture carries a record chain; nothing to measure");
        return;
    }
    inventory(&chains);
    what_is_in_there(&chains);
    what_the_registry_would_claim(&chains);
    what_would_remain(&chains);
    does_it_look_like_the_same_drawing(&chains);
}

/// Question 1: the admission rule, applied and counted.
fn inventory(chains: &[Chain]) {
    println!("=== 1. chain streams, split by the `Sheet` prefix rule ===");
    let mut opened = (0usize, 0usize);
    let mut closed = (0usize, 0usize);
    for chain in chains {
        if chain.is_opened_by_the_pipeline() {
            opened.0 += 1;
            opened.1 += chain.records;
        } else {
            closed.0 += 1;
            closed.1 += chain.records;
        }
    }
    println!(
        "  opened: {} streams, {} records — closed: {} streams, {} records",
        opened.0, opened.1, closed.0, closed.1
    );
    println!("\n  the closed ones, by fixture:");
    let mut by_fixture: BTreeMap<&str, Vec<&Chain>> = BTreeMap::new();
    for chain in chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
        by_fixture
            .entry(fixture_leaf(chain.fixture))
            .or_default()
            .push(chain);
    }
    for (fixture, mut streams) in by_fixture {
        streams.sort_by_key(|chain| std::cmp::Reverse(chain.records));
        let total: usize = streams.iter().map(|chain| chain.records).sum();
        println!("    {fixture}: {} streams, {total} records", streams.len());
        for chain in streams.iter().take(12) {
            println!("       {} x{}", chain.path, chain.records);
        }
        if streams.len() > 12 {
            println!("       … {} more", streams.len() - 12);
        }
    }
}

/// Question 2: the whole inventory of the closed streams, named — and the two
/// walks side by side, because the residue report depends on the second one
/// seeing what the first one sees.
fn what_is_in_there(chains: &[Chain]) {
    println!("\n=== 2. every record the pipeline never sees, by family ===");
    let mut chain_totals: BTreeMap<u16, usize> = BTreeMap::new();
    let mut census_totals: BTreeMap<u16, usize> = BTreeMap::new();
    for chain in chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
        for (code, count) in chain_census(&chain.data) {
            *chain_totals.entry(code).or_default() += count;
        }
        for (code, count) in census_walk_census(&chain.data) {
            *census_totals.entry(code).or_default() += count;
        }
    }
    let wired: Vec<u16> = SHEET_RECORD_FAMILIES.iter().map(|f| f.type_code).collect();
    let mut ranked: Vec<(&u16, &usize)> = chain_totals.iter().collect();
    ranked.sort_by_key(|(code, count)| (std::cmp::Reverse(**count), **code));
    println!("  (graphic families only; the non-graphic tail is summarised below)");
    let mut quiet = 0usize;
    for (code, count) in &ranked {
        if !is_native_graphic_type_code(**code) {
            quiet += **count;
            continue;
        }
        let seen = census_totals.get(code).copied().unwrap_or_default();
        println!(
            "  0x{code:04X} {:>22} x{count:<5} {:<17} census walk sees {seen}{}",
            rad_class_name(**code).unwrap_or("?"),
            if wired.contains(code) {
                "a decoder exists"
            } else {
                "NO DECODER"
            },
            if seen == **count {
                ""
            } else {
                "  <-- MISMATCH"
            }
        );
    }
    println!("  plus {quiet} records of families the native graphic predicate refuses");
}

/// Question 3: hand the closed bytes to the registry as it stands today.
fn what_the_registry_would_claim(chains: &[Chain]) {
    println!("\n=== 3. what today's decoders would claim from the closed streams ===");
    let mut per_family: BTreeMap<&str, usize> = BTreeMap::new();
    let mut per_family_emits: BTreeMap<&str, bool> = BTreeMap::new();
    let mut per_stream: Vec<(String, Vec<(&str, usize)>)> = Vec::new();
    for chain in chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
        let geometry = decode(&chain.data);
        let mut rows = Vec::new();
        for family in SHEET_RECORD_FAMILIES {
            let claimed = (family.record_count)(&geometry);
            if claimed == 0 {
                continue;
            }
            *per_family.entry(family.name).or_default() += claimed;
            per_family_emits.insert(family.name, family.emits_geometry);
            rows.push((family.name, claimed));
        }
        if !rows.is_empty() {
            per_stream.push((
                format!("{}{}", fixture_leaf(chain.fixture), chain.path),
                rows,
            ));
        }
    }
    let mut ranked: Vec<(&&str, &usize)> = per_family.iter().collect();
    ranked.sort_by_key(|(name, count)| (std::cmp::Reverse(**count), **name));
    let drawable: usize = ranked
        .iter()
        .filter(|(name, _)| per_family_emits.get(**name).copied().unwrap_or_default())
        .map(|(_, count)| **count)
        .sum();
    for (name, count) in &ranked {
        println!(
            "  {name:>20}: {count} records{}",
            if per_family_emits.get(**name).copied().unwrap_or_default() {
                ""
            } else {
                "  (audit-only family, emits no geometry)"
            }
        );
    }
    println!(
        "  → {} records would be claimed, {drawable} of them by families that emit geometry",
        ranked.iter().map(|(_, count)| **count).sum::<usize>()
    );
    println!("\n  by stream:");
    for (path, rows) in per_stream.iter().take(20) {
        let text: Vec<String> = rows
            .iter()
            .map(|(name, count)| format!("{name} x{count}"))
            .collect();
        println!("    {path}: {}", text.join(", "));
    }
    if per_stream.len() > 20 {
        println!("    … {} more streams", per_stream.len() - 20);
    }
}

/// Question 4: the residue, split by what each half asks for.
fn what_would_remain(chains: &[Chain]) {
    println!("\n=== 4. what would still be uncovered after wiring, unchanged ===");
    let mut undecoded: BTreeMap<u16, (usize, bool, Option<&'static str>)> = BTreeMap::new();
    let mut refused: BTreeMap<u16, (usize, bool, Option<&'static str>)> = BTreeMap::new();
    for chain in chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
        let claimed: Vec<core::ops::Range<usize>> = SHEET_RECORD_FAMILIES
            .iter()
            .flat_map(|family| (family.decoded_ranges)(&chain.data))
            .collect();
        for row in undecoded_type_code_census(&chain.data, &claimed) {
            let slot =
                undecoded
                    .entry(row.type_code)
                    .or_insert((0, row.is_graphic, row.rad_class_name));
            slot.0 += row.count;
        }
        for row in refused_record_census(&chain.data, &claimed) {
            let slot =
                refused
                    .entry(row.type_code)
                    .or_insert((0, row.is_graphic, row.rad_class_name));
            slot.0 += row.count;
        }
    }
    println!("  no decoder exists (a new family would have to be written):");
    let mut ranked: Vec<CensusRow<'_>> = undecoded.iter().collect();
    ranked.sort_by_key(|(code, (count, _, _))| (std::cmp::Reverse(*count), **code));
    for (code, (count, graphic, name)) in ranked.iter().filter(|(_, (_, g, _))| *g) {
        println!(
            "    0x{code:04X} {:>26} x{count} ({})",
            name.unwrap_or("?"),
            if *graphic { "graphic" } else { "non-graphic" }
        );
    }
    let quiet: usize = ranked
        .iter()
        .filter(|(_, (_, g, _))| !*g)
        .map(|(_, (count, _, _))| *count)
        .sum();
    println!("    plus {quiet} records of non-graphic families");
    println!("  a decoder walked them and refused (existing rules would need revisiting):");
    let mut ranked: Vec<CensusRow<'_>> = refused.iter().collect();
    ranked.sort_by_key(|(code, (count, _, _))| (std::cmp::Reverse(*count), **code));
    for (code, (count, graphic, name)) in ranked {
        println!(
            "    0x{code:04X} {:>26} x{count} ({})",
            name.unwrap_or("?"),
            if *graphic { "graphic" } else { "non-graphic" }
        );
    }
}

/// One census row as ranked above: the type code against
/// `(count, is_graphic, class name)`.
type CensusRow<'a> = (&'a u16, &'a (usize, bool, Option<&'static str>));

/// A coordinate box, or nothing when no record contributed a point.
#[derive(Default)]
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

    fn text(&self) -> String {
        match (self.min, self.max) {
            (Some((x0, y0)), Some((x1, y1))) => format!(
                "{} pts, x {x0:.1}..{x1:.1}, y {y0:.1}..{y1:.1}",
                self.points
            ),
            _ => "no points".to_string(),
        }
    }
}

/// Every coordinate the geometry-emitting families put on the page, from one
/// decoded stream.
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
    extent
}

/// Question 5: the plausibility check. Nothing here is a pass/fail gate — it
/// is the one look at the numbers that catches a decoder producing tidy
/// nonsense on bytes it has never been aimed at.
fn does_it_look_like_the_same_drawing(chains: &[Chain]) {
    println!("\n=== 5. the closed streams' coordinate box against the opened ones' ===");
    let mut per_fixture: BTreeMap<&str, (Box2d, Box2d)> = BTreeMap::new();
    for chain in chains {
        let geometry = decode(&chain.data);
        let extent = box_of(&geometry);
        if extent.points == 0 {
            continue;
        }
        let slot = per_fixture
            .entry(fixture_leaf(chain.fixture))
            .or_insert_with(|| (Box2d::default(), Box2d::default()));
        let target = if chain.is_opened_by_the_pipeline() {
            &mut slot.0
        } else {
            &mut slot.1
        };
        if let (Some((x0, y0)), Some((x1, y1))) = (extent.min, extent.max) {
            let before = target.points;
            target.add(x0, y0);
            target.add(x1, y1);
            target.points = before + extent.points;
        }
    }
    for (fixture, (opened, closed)) in &per_fixture {
        println!("  {fixture}");
        println!("     opened: {}", opened.text());
        println!("     closed: {}", closed.text());
    }
    println!("\n  the closed streams that would newly yield coordinates:");
    for chain in chains.iter().filter(|c| !c.is_opened_by_the_pipeline()) {
        let extent = box_of(&decode(&chain.data));
        if extent.points == 0 {
            continue;
        }
        println!(
            "    {}{} [{}]: {}",
            fixture_leaf(chain.fixture),
            chain.path,
            chain.leaf(),
            extent.text()
        );
    }
}
