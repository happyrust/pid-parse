//! Does anything in a `.pid` reference a fill style?
//!
//! Phase 39 dropped gap E — decoding `0x002A JStyleSimpleFill` and
//! `0x002B JStyleHatchFill` — on the grounds that the 36 fill style records
//! in the corpus have no geometry referencing them, so decoding them would
//! produce nothing to draw. Before that gets written up as a negative note it
//! has to be checked, because `style_link`'s own documentation says the
//! opposite in passing: "an override referencing a `JStyleSimpleFill` is a
//! well-formed record describing a filled object, and 4 of the corpus's 48
//! overrides are exactly that."
//!
//! Both cannot be right. This resolves every geometry record's `index` the way
//! `style_link` does — directly, and one hop through a `JStyleOverride` — and
//! reports which ones land on a fill. A geometry record reaching a fill is a
//! consumer; a fill style nothing reaches is decoration in the style table.
//!
//! ```powershell
//! cargo run --example probe_fill_style_consumers
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igboundaries, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igtextboxes,
    PSM_TYPE_CODE_JSTYLE_OVERRIDE,
};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable};

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    // The gongyi fixture, escaped so this source stays pure ASCII.
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

const SIMPLE_FILL: u16 = 0x002A;
const HATCH_FILL: u16 = 0x002B;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn streams_named(path: &Path, leaf_prefix: &str) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with(leaf_prefix))
        })
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name, bytes));
        }
    }
    out
}

/// Where a geometry record's style id ends up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Landing {
    /// The id names a fill outright.
    FillDirect,
    /// The id names an override whose base object is a fill.
    FillViaOverride,
    /// Anything else — a line style, a text style, or nothing at all.
    NotAFill,
}

fn landing(table: &DocumentStyleTable, style_id: u32) -> Landing {
    let is_fill = |type_code: u16| type_code == SIMPLE_FILL || type_code == HATCH_FILL;
    let Some(record) = table.get(style_id) else {
        return Landing::NotAFill;
    };
    if is_fill(record.type_code) {
        return Landing::FillDirect;
    }
    if record.type_code == PSM_TYPE_CODE_JSTYLE_OVERRIDE {
        if let Some(target) = record.base_reference.and_then(|id| table.get(id)) {
            if is_fill(target.type_code) {
                return Landing::FillViaOverride;
            }
        }
    }
    Landing::NotAFill
}

fn main() {
    let root = test_file_root();

    let mut fill_records = 0usize;
    let mut overrides_naming_a_fill = 0usize;
    let mut reached: BTreeMap<(&'static str, Landing), usize> = BTreeMap::new();
    let mut consumers: Vec<String> = Vec::new();
    let mut fills_reached: BTreeSet<u32> = BTreeSet::new();
    let mut fill_inventory: Vec<String> = Vec::new();
    let mut rings: Vec<String> = Vec::new();
    let mut boundary_total = 0usize;
    let mut closed_rings = 0usize;

    for name in FIXTURES {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let clusters: BTreeMap<String, Vec<u8>> =
            streams_named(&path, "StyleCluster").into_iter().collect();

        // How many fill styles the file defines, and how many overrides point
        // at one. The override count is the claim `style_link` makes.
        for (cluster_path, cluster) in &clusters {
            let table = DocumentStyleTable::from_stylecluster_bytes(cluster);
            let mut simple = 0usize;
            let mut hatch = 0usize;
            for record in table.records() {
                match record.type_code {
                    SIMPLE_FILL => simple += 1,
                    HATCH_FILL => hatch += 1,
                    _ => {}
                }
                if record.type_code == SIMPLE_FILL || record.type_code == HATCH_FILL {
                    fill_records += 1;
                }
                let target = record
                    .base_reference
                    .filter(|_| record.type_code == PSM_TYPE_CODE_JSTYLE_OVERRIDE)
                    .and_then(|id| table.get(id));
                if let Some(target) = target {
                    if target.type_code == SIMPLE_FILL || target.type_code == HATCH_FILL {
                        overrides_naming_a_fill += 1;
                        fill_inventory.push(format!(
                            "    {name} {cluster_path}: override id {} -> 0x{:04X} id {} \
                             ({} byte payload)",
                            record.style_id,
                            target.type_code,
                            target.style_id,
                            target.byte_range.len().saturating_sub(6)
                        ));
                    }
                }
            }
            if simple + hatch > 0 {
                fill_inventory.push(format!(
                    "    {name} {cluster_path}: {simple} x 0x002A, {hatch} x 0x002B"
                ));
            }
        }

        // The consumer test: every geometry record's own style id.
        for (sheet_path, sheet) in streams_named(&path, "Sheet") {
            let Some(cluster) = clusters.get(&stylecluster_path_for_sheet(&sheet_path)) else {
                continue;
            };
            let table = DocumentStyleTable::from_stylecluster_bytes(cluster);
            if table.is_empty() {
                continue;
            }
            let mut ids: Vec<(&'static str, u32, u32)> = Vec::new();
            ids.extend(
                decode_iglines(&sheet)
                    .iter()
                    .map(|r| ("igLine2d", r.oid, r.index)),
            );
            ids.extend(
                decode_iglinestrings(&sheet)
                    .iter()
                    .map(|r| ("igLineString2d", r.oid, r.index)),
            );
            ids.extend(
                decode_igpoints(&sheet)
                    .iter()
                    .map(|r| ("igPoint2d", r.oid, r.index)),
            );
            ids.extend(
                decode_igtextboxes(&sheet)
                    .iter()
                    .map(|r| ("igTextBox", r.oid, r.index)),
            );
            // The one family that could bound a filled area, and the reason
            // "no filled-area geometry exists" is worth testing rather than
            // assuming: a boundary is what a fill would fill.
            let boundaries = decode_igboundaries(&sheet);
            ids.extend(boundaries.iter().map(|r| ("igBoundary2d", r.oid, r.index)));

            // A fill needs somewhere to go. The decoder ships an
            // `is_closed_loop` helper whose doc claims 20/20 close at 1e-9;
            // that claim is the prerequisite for drawing an area, so it is
            // re-measured here rather than taken on trust.
            for record in &boundaries {
                boundary_total += 1;
                let closed = record.is_closed_loop(1e-9);
                if closed {
                    closed_rings += 1;
                }
                let xs = record.segments.iter().map(|s| s.start.0);
                let ys = record.segments.iter().map(|s| s.start.1);
                let (min_x, max_x) = xs
                    .clone()
                    .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
                let (min_y, max_y) =
                    ys.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
                rings.push(format!(
                    "  {name} oid={oid:<6} {segments} segment(s)  closed={closed}  \
                     bbox {:.1}x{:.1}mm at ({:.1}, {:.1})",
                    (max_x - min_x) * 1000.0,
                    (max_y - min_y) * 1000.0,
                    min_x * 1000.0,
                    min_y * 1000.0,
                    oid = record.oid,
                    segments = record.segments.len(),
                ));
            }

            for (family, oid, index) in ids {
                let landed = landing(&table, index);
                *reached.entry((family, landed)).or_default() += 1;
                if landed != Landing::NotAFill {
                    fills_reached.insert(index);
                    consumers.push(format!(
                        "  {name} {sheet_path} {family} oid={oid} index={index} -> {landed:?}"
                    ));
                }
            }
        }
    }

    println!("=== fill styles defined ===\n");
    println!("  {fill_records} record(s) of 0x002A / 0x002B across the corpus");
    println!("  {overrides_naming_a_fill} JStyleOverride record(s) name one as their base object");
    for line in &fill_inventory {
        println!("{line}");
    }

    println!("\n=== what geometry references, by family ===\n");
    for ((family, landed), count) in &reached {
        println!("  {family:<16} {landed:?}: {count}");
    }

    println!("\n=== geometry that reaches a fill ===\n");
    if consumers.is_empty() {
        println!("  none — no geometry record's index lands on a fill, directly or via override");
    } else {
        for line in &consumers {
            println!("{line}");
        }
        println!("\n  {} distinct style id(s) reached", fills_reached.len());
    }

    println!("\n=== can those boundaries be filled? ===\n");
    for line in &rings {
        println!("{line}");
    }
    println!(
        "\n  {closed_rings} of {boundary_total} boundary record(s) close into a ring at 1e-9."
    );
}
