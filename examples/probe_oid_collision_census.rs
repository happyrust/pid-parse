//! Does any `(stream, oid)` collision change what the drawing looks like?
//!
//! The three style indexes [`pid_parse::style_link`] publishes —
//! [`pid_parse::style_link::LineStyleIndex`],
//! [`pid_parse::style_link::FillIndex`] and
//! [`pid_parse::style_link::TextHeightIndex`] — are all keyed by
//! `(stream path, graphic oid)`, and `OpenCADStudio` joins its entities on the
//! same pair. That is only sound if the pair names at most one record.
//!
//! It does not always. One case is known: in `工艺管道及仪表流程-1.pid`, two
//! `igTextBox` records on `/Sheet6` share oid 6345, so they collapse into one
//! index entry and the second label silently inherits the first's style. That
//! was found sideways, while counting something else. This probe turns the one
//! case into the whole picture.
//!
//! # The question that decides what to do about it
//!
//! Counting collisions is the easy half and on its own it decides nothing. A
//! collision only matters if the two records **should have drawn differently**
//! — if they name the same style, collapsing them costs the drawing nothing
//! and re-keying three indexes plus the consumer's join would buy nothing.
//!
//! So the primary test here is deliberately **not** "do the two records
//! resolve to equal styles today":
//!
//! > **Do the colliding records name the same style id at all?**
//!
//! That is the stronger question and the more durable one. A style id is what
//! the geometry record stores; whether the resolver reaches it in one hop or
//! two, through a paragraph or through a run, two records naming *the same id*
//! cannot come out different. An answer phrased that way survives the
//! run-versus-paragraph question currently open elsewhere, which an answer
//! phrased in resolved heights and colours would not. The resolved comparison
//! is printed too, as corroboration under today's rule.
//!
//! # What is also worth knowing
//!
//! * Whether the colliding records are **distinct records at all** — two
//!   non-overlapping byte ranges, rather than one record the scanner reached
//!   twice. The byte ranges are printed for every collision.
//! * Whether an oid is reused **across families** (a line and a label sharing
//!   one). Those land in different indexes so nothing collapses, but it says
//!   what the oid is: a per-family counter, or a document-wide identity.
//!
//! ```powershell
//! cargo run --example probe_oid_collision_census
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use pid_parse::parsers::sheet_records::{
    decode_igboundaries, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igtextboxes,
};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Which published index a record feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Index {
    Line,
    Fill,
    Text,
}

impl Index {
    fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Fill => "fill",
            Self::Text => "text",
        }
    }
}

/// One decoded record, reduced to what the collision question needs.
#[derive(Debug, Clone)]
struct Entry {
    /// Which of the three families inside the index it came from.
    family: &'static str,
    /// Where the record sits in the stream, so two entries can be shown to be
    /// two records rather than one reached twice.
    byte_range: std::ops::Range<usize>,
    /// The style id the record stores. **This is the field the verdict turns
    /// on**: equal ids cannot resolve differently under any resolution rule.
    style_index: u32,
    /// Whether this record resolves to anything, and so whether it actually
    /// takes the key in the published index. A record that resolves to
    /// nothing is never inserted and cannot collapse over anyone.
    occupies_key: bool,
    /// The resolved style rendered as a string, under today's rule, for
    /// corroboration only.
    resolved: Option<String>,
    /// Enough of the record's own content to say whether two records sharing
    /// an oid are the same object published twice or two different objects.
    /// That is the question "what is an oid" turns on, and it cannot be
    /// answered from the style id alone.
    detail: String,
}

/// A fixture that fails to open is fatal: this probe exists to state a
/// corpus-wide count, and a fixture that quietly contributes nothing lowers it
/// invisibly.
fn streams(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let file = std::fs::File::open(path)
        .unwrap_or_else(|error| panic!("{} exists but did not open: {error}", path.display()));
    let mut cfb = cfb::CompoundFile::open(file).unwrap_or_else(|error| {
        panic!(
            "{} did not read as a compound file: {error}",
            path.display()
        )
    });
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.insert(name, bytes);
        }
    }
    out
}

fn is_sheet(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .unwrap_or_default()
        .starts_with("Sheet")
}

/// What the published indexes are keyed by, plus the fixture and which index,
/// so one census can hold all six drawings and all three indexes at once.
type Key = (String, Index, String, u32);

/// One key and every record that wants it.
type Collision<'a> = (&'a Key, &'a Vec<Entry>);

fn main() {
    // (fixture, index, stream, oid) -> every record that wants that key.
    let mut wanted: BTreeMap<Key, Vec<Entry>> = BTreeMap::new();
    // (fixture, stream, oid) -> which indexes claim it, for the cross-family
    // question.
    let mut families_per_oid: BTreeMap<(String, String, u32), Vec<Index>> = BTreeMap::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        assert!(path.exists(), "{fixture} is missing");
        let short = fixture.rsplit('/').next().unwrap_or(fixture).to_string();
        let all = streams(path);
        for (name, bytes) in all.iter().filter(|(name, _)| is_sheet(name)) {
            let table = all
                .get(&stylecluster_path_for_sheet(name))
                .map_or_else(DocumentStyleTable::default, |b| {
                    DocumentStyleTable::from_stylecluster_bytes(b)
                });
            // `style_link` skips a sheet whose own StyleCluster is empty, so
            // the census has to skip it too or it counts keys that never
            // reach an index.
            if table.is_empty() {
                continue;
            }

            let mut push = |index: Index, entry: Entry, oid: u32| {
                wanted
                    .entry((short.clone(), index, name.clone(), oid))
                    .or_default()
                    .push(entry);
                let slot = families_per_oid
                    .entry((short.clone(), name.clone(), oid))
                    .or_default();
                if !slot.contains(&index) {
                    slot.push(index);
                }
            };

            for record in decode_iglines(bytes) {
                let resolved = table.resolve_line_style(record.index);
                push(
                    Index::Line,
                    Entry {
                        family: "igLine2d",
                        byte_range: record.byte_range.clone(),
                        style_index: record.index,
                        occupies_key: resolved.is_some(),
                        resolved: resolved.map(|r| format!("{r:?}")),
                        detail: String::new(),
                    },
                    record.oid,
                );
            }
            for record in decode_igpoints(bytes) {
                let resolved = table.resolve_line_style(record.index);
                push(
                    Index::Line,
                    Entry {
                        family: "igPoint2d",
                        byte_range: record.byte_range.clone(),
                        style_index: record.index,
                        occupies_key: resolved.is_some(),
                        resolved: resolved.map(|r| format!("{r:?}")),
                        detail: String::new(),
                    },
                    record.oid,
                );
            }
            for record in decode_iglinestrings(bytes) {
                let resolved = table.resolve_line_style(record.index);
                push(
                    Index::Line,
                    Entry {
                        family: "igLineString2d",
                        byte_range: record.byte_range.clone(),
                        style_index: record.index,
                        occupies_key: resolved.is_some(),
                        resolved: resolved.map(|r| format!("{r:?}")),
                        detail: String::new(),
                    },
                    record.oid,
                );
            }
            for record in decode_igboundaries(bytes) {
                let resolved = table.resolve_fill(record.index);
                push(
                    Index::Fill,
                    Entry {
                        family: "igBoundary2d",
                        byte_range: record.byte_range.clone(),
                        style_index: record.index,
                        occupies_key: resolved.is_some(),
                        resolved: resolved.map(|r| format!("{r:?}")),
                        detail: String::new(),
                    },
                    record.oid,
                );
            }
            for record in decode_igtextboxes(bytes) {
                let resolved = table.resolve_text_height(record.index);
                push(
                    Index::Text,
                    Entry {
                        family: "igTextBox",
                        byte_range: record.byte_range.clone(),
                        style_index: record.index,
                        occupies_key: resolved.is_some(),
                        resolved: resolved.map(|r| format!("{r:?}")),
                        detail: format!(
                            "text {:?} at ({:.4}, {:.4}) rot {:.4}",
                            record.text,
                            record.trailing_double_1,
                            record.trailing_double_2,
                            record.rotation_rad
                        ),
                    },
                    record.oid,
                );
            }
        }
    }

    println!("=== (stream, oid) across the three published indexes ===\n");
    println!(
        "{:>6} {:>10} {:>10} {:>12} {:>14}",
        "index", "records", "keys", "colliding", "in-collision"
    );
    let mut all_collisions: Vec<Collision> = Vec::new();
    for index in [Index::Line, Index::Fill, Index::Text] {
        let of_index: Vec<_> = wanted
            .iter()
            .filter(|((_, i, _, _), _)| *i == index)
            .collect();
        let records: usize = of_index.iter().map(|(_, v)| v.len()).sum();
        let keys = of_index.len();
        let colliding: Vec<_> = of_index
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .copied()
            .collect();
        let in_collision: usize = colliding.iter().map(|(_, v)| v.len()).sum();
        println!(
            "{:>6} {records:>10} {keys:>10} {:>12} {in_collision:>14}",
            index.name(),
            colliding.len()
        );
        all_collisions.extend(colliding);
    }

    // A key held by two *overlapping* byte ranges is not two records sharing
    // an oid. It is one record the scanner decoded twice, starting a few bytes
    // apart -- `PsmRecordDecoder::scan` walks candidate offsets, so a record
    // whose bytes also validate at a shifted start appears twice. That is a
    // decoder question and it has nothing to say about whether an oid is
    // unique. Separating the two is the whole difference between "the key is
    // broken" and "the scanner is loose".
    let overlaps = |entries: &Vec<Entry>| -> bool {
        let mut ranges: Vec<_> = entries.iter().map(|e| e.byte_range.clone()).collect();
        ranges.sort_by_key(|r| r.start);
        ranges.windows(2).any(|w| w[0].end > w[1].start)
    };

    println!("\n=== two records, or one record found twice? ===");
    let (redecodes, genuine): (Vec<_>, Vec<_>) = all_collisions
        .iter()
        .partition(|(_, entries)| overlaps(entries));
    println!(
        "colliding keys whose ranges OVERLAP  (one record, re-decoded): {}",
        redecodes.len()
    );
    println!(
        "colliding keys whose ranges are DISJOINT (really two records): {}",
        genuine.len()
    );

    println!("\n=== does any GENUINE collision change what is drawn? ===");
    println!("(the durable test: records naming ONE style id cannot resolve apart,");
    println!(" under the two-hop rule today or any other rule later)\n");

    let mut same_id = 0usize;
    let mut different_id = 0usize;
    let mut collapses = 0usize;
    for (_, entries) in &genuine {
        let ids: std::collections::BTreeSet<u32> = entries.iter().map(|e| e.style_index).collect();
        if ids.len() == 1 {
            same_id += 1;
        } else {
            different_id += 1;
        }
        // Only records that resolve are inserted, so a key collapses only
        // when two or more of its records actually occupy it.
        if entries.iter().filter(|e| e.occupies_key).count() > 1 {
            collapses += 1;
        }
    }
    println!(
        "genuine colliding keys:                        {}",
        genuine.len()
    );
    println!("  ...where every record names ONE style id:    {same_id}   <- harmless");
    println!("  ...where the records name DIFFERENT ids:     {different_id}   <- would change the drawing");
    println!("  ...where >1 record reaches the index (a real collapse): {collapses}");

    // Same question over the re-decodes, so the claim "they are the same
    // record" is checked rather than asserted: one record read twice must
    // name one style id.
    let redecode_id_disagreements = redecodes
        .iter()
        .filter(|(_, entries)| {
            entries
                .iter()
                .map(|e| e.style_index)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        })
        .count();
    println!(
        "\nre-decodes whose two readings disagree about the style id: {redecode_id_disagreements}"
    );
    println!("(non-zero would mean they are not the same record after all)");

    println!("\n=== every GENUINE collision, in full ===");
    if genuine.is_empty() {
        println!("(none)");
    }
    for ((fixture, index, stream, oid), entries) in &genuine {
        let ids: std::collections::BTreeSet<u32> = entries.iter().map(|e| e.style_index).collect();
        let resolved: std::collections::BTreeSet<&Option<String>> =
            entries.iter().map(|e| &e.resolved).collect();
        println!(
            "\n{fixture}  {stream}  oid {oid}  [{}]  style ids {ids:?}  {}",
            index.name(),
            if ids.len() == 1 {
                "SAME ID -> harmless"
            } else {
                "DIFFERENT IDS -> changes the drawing"
            }
        );
        println!(
            "  resolved styles distinct under today's rule: {}",
            resolved.len()
        );
        for entry in entries.iter() {
            println!(
                "  {:>15}  bytes {:>7}..{:<7}  index {:>4}  in-index {}",
                entry.family,
                entry.byte_range.start,
                entry.byte_range.end,
                entry.style_index,
                entry.occupies_key
            );
            if !entry.detail.is_empty() {
                println!("                   {}", entry.detail);
            }
        }
    }

    // The collapse costs the drawing nothing, but that is a statement about
    // *style*. Whether both records reach the drawing at all is a separate
    // question with a separate answer, and it is the one a reader will ask
    // next -- so measure it here rather than leaving it hanging.
    println!("\n=== do both halves of a genuine collision reach the drawing? ===");
    for ((fixture, _, stream, oid), _) in &genuine {
        let path = FIXTURES
            .iter()
            .find(|f| f.ends_with(fixture.as_str()))
            .expect("the collision came from a fixture in the list");
        let Ok(parsed) = pid_parse::PidParser::new().parse_file(Path::new(path)) else {
            println!("  {fixture}: did not re-parse");
            continue;
        };
        let geometry = pid_parse::build_normalized_geometry(&parsed);
        let hits: Vec<_> = geometry
            .entities
            .iter()
            .filter(|e| {
                e.graphic_oid == Some(*oid)
                    && e.source.stream_path.as_deref() == Some(stream.as_str())
            })
            .collect();
        println!(
            "  {fixture} {stream} oid {oid}: {} normalized entit(ies)",
            hits.len()
        );
        for hit in &hits {
            if let pid_parse::PidGraphicKind::Text {
                insertion, value, ..
            } = &hit.kind
            {
                println!(
                    "    {:?} at ({:.4}, {:.4})  confidence {:?}",
                    value, insertion.x, insertion.y, hit.confidence
                );
            }
        }
    }

    println!("\n=== is an oid reused across families? ===");
    println!("(different indexes, so nothing collapses -- this says what an oid *is*)");
    let cross: Vec<_> = families_per_oid
        .iter()
        .filter(|(_, indexes)| indexes.len() > 1)
        .collect();
    println!("oids claimed by more than one index: {}", cross.len());
    for ((fixture, stream, oid), indexes) in cross.iter().take(10) {
        let names: Vec<&str> = indexes.iter().map(|i| i.name()).collect();
        println!("  {fixture}  {stream}  oid {oid}  -> {names:?}");
    }

    println!("\n=== verdict ===");
    if different_id == 0 {
        println!(
            "No collision in this corpus puts two different style ids on one key,\n\
             so no collapse changes a single drawn thing -- and that holds however\n\
             the resolver is rewired, because it is a statement about what the\n\
             records store rather than about what they resolve to.\n\
             Re-keying three indexes and the consumer's join would buy nothing\n\
             today. Guard the count instead, so a fixture that does collide\n\
             meaningfully turns it red."
        );
    } else {
        println!(
            "{different_id} colliding key(s) carry different style ids: one of those records\n\
             is being drawn with the other's style right now. The key has to\n\
             change, or the second record has to be refused rather than\n\
             silently dropped."
        );
    }
}
