//! What decides the scale a record states for the range boxes of its
//! children?
//!
//! Three kinds of record keep a list of children with a range box for each:
//! a sheet's own object (`0x0114 JSheet`, in the storage's `PSMcluster0`), a
//! `0x0021 ComplexString` and a `0x007B` Group. Each entry is 14 bytes — an
//! oid, four `i16` and a word — and each record states one `f64` ahead of
//! its entries, the scale: a box is in `scale / 32768` m. The corpora state
//! `1.0` and `2.0` and nothing else, and until this probe the scale was read
//! as stored with no account of what sets it
//! (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §16: "这一条为什么是 2.0、比例按什么规则取，没有查明").
//!
//! This reads every such record of every distinct drawing, template and
//! symbol in reach — `test-file`, both `pid-test-data`, the share, and the
//! `.igr` / `.sym` / `.pid` members of their `RefData~*.zip` and
//! `PlantData~*.zip`, each distinct sheet stream once — and sets the scale
//! beside what could decide it: the kind of record, the document, the paper
//! the sheet states, the scale of the sheet the record sits on, its layer,
//! its owner, and seven measures of where its children lie. It prints
//!
//! 1. every `ComplexString` and Group, one line each;
//! 2. the sheets, tallied by kind of document, paper and scale;
//! 3. the records that share a document, a sheet, an owner and a layer and
//!    state different scales — which is what rules out every determinant
//!    that is a property of the place a record sits in;
//! 4. for each measure, the largest value among the records at `1.0` and
//!    the smallest among those at `2.0`: a measure decides the scale only
//!    if the first is below the second — and then (4b) the same for the
//!    family the one measure that passes belongs to, the far corner's
//!    p-norms from 1 to 12 and weighted sums of its two coordinates, with
//!    whether the gap between the two scales holds a metre;
//! 5. the rule that comes out, held to every record: the scale is `1.0`
//!    while the far corner of the box that holds the children is less than
//!    a metre from the origin, and `2.0` from there to two metres.
//!
//! It exits non-zero when a record does not state the scale that rule
//! gives, or states a scale other than `1.0` and `2.0`. An argument is taken
//! for part of a document's path, and the records of the documents it names
//! are listed one by one.
//!
//! Read on 2026-11-12: 885 documents, 1 293 records (1 273 sheets, 8
//! strings, 12 groups), 1 233 at `1.0` and 60 at `2.0`. The far corner's
//! distance separates them with nothing in between: at most 998.0 mm under
//! `1.0`, at least 1 015.9 mm under `2.0`. None of the other six measures
//! does, and no property of the place does: `A1-W.igr` keeps its border
//! group at `2.0` and its logo group at `1.0` on one sheet, under one
//! owner, on no layer, and one sheet on A1 paper states `1.0` where
//! thirty-nine state `2.0`.
//!
//! The distance is not the only measure that could, though. It is the far
//! corner's 2-norm, and the p-norms separate the two scales for every p
//! from 1.63 to the 12.00 tried; so do the weighted sums `x + k*y` for k
//! from 0.01 to 0.50. What leaves the distance is the line: a metre is the
//! one round length between 998.0 and 1 015.9 mm, and `2^15` of the
//! record's own units, and the gap between the two scales holds a metre
//! only for p from 1.99 to 2.10 (and for `x + k*y` at k = 0.29 and 0.30,
//! a weight nothing recommends). So the rule of section 5 is a pair — the
//! straight-line distance, and a line at one metre — that fits every
//! record at both ends; neither half is a reading by itself.
//!
//! ```text
//! cargo run --example probe_what_decides_the_scale_of_a_range_box
//! cargo run --example probe_what_decides_the_scale_of_a_range_box -- 验收数据 A1-W.igr
//! ```
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_display_order::find_sheet_child_table;
use pid_parse::parsers::sheet_records::{
    decode_igcomplexstring_at, decode_iggroup_at, parse_live_psm_header, SheetChain,
    PSM_TYPE_CODE_IGCOMPLEXSTRING2D, PSM_TYPE_CODE_IGGROUP,
};
use pid_parse::{ParseOptions, PidParser};

/// The box an entry states for a child with no extent.
const NO_EXTENT: [i16; 4] = [32767, 32767, -32768, -32768];

/// What the sheets are tallied by: kind of document, storage, paper, scale.
type SheetKey = (String, String, String, String);
/// A tally of sheets: how many, and the least and the most distance to the
/// far corner among them.
type SheetTally = (usize, f64, f64);
/// Everything a place could give a record: the document, the storage, the
/// sheet stream, the kind of record, its owner and its layer.
type Place<'a> = (String, String, String, &'a str, u32, u32);

/// Every drawing, template and symbol under `dir`, and every archive of a
/// backup package that may hold more of them.
fn documents(dir: &Path, files: &mut Vec<PathBuf>, archives: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "symbols-full" || name == "plot")
            {
                continue;
            }
            documents(&path, files, archives);
            continue;
        }
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if ["pid", "igr", "sym"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
        {
            files.push(path);
        } else if extension.eq_ignore_ascii_case("zip")
            && (name.starts_with("RefData~") || name.starts_with("PlantData~"))
        {
            archives.push(path);
        }
    }
}

/// One record that lists children with a range box each.
struct Container {
    /// The file, or the archive and its member.
    document: String,
    /// `drawing`, `template` or `symbol`, by the file's extension.
    document_kind: &'static str,
    /// The storage the sheet stream sits in: empty for the document's own,
    /// `/JSite<N>` for a symbol cache.
    storage: String,
    /// The sheet stream, `Sheet<N>`.
    stream: String,
    /// `sheet`, `string` or `group`.
    what: &'static str,
    oid: u32,
    /// The record that owns it; the sheet, for every string and group.
    parent: u32,
    /// The layer it sits on; zero for a group and a sheet.
    layer: u32,
    members: usize,
    /// The scale it states.
    scale: f64,
    /// The box that holds every child with an extent, in the record's units.
    units: Option<[i32; 4]>,
    /// The paper the sheet states: width, height, size code.
    paper: Option<(f64, f64, u32)>,
    /// The scale of the sheet the record sits on (its own, for a sheet).
    sheet_scale: Option<f64>,
}

impl Container {
    /// The box in metres.
    fn metres(&self) -> Option<[f64; 4]> {
        self.units
            .map(|units| units.map(|unit| f64::from(unit) * self.scale / 32768.0))
    }
    /// The far corner: the larger magnitude on each axis.
    fn far_corner(&self) -> Option<(f64, f64)> {
        self.metres()
            .map(|b| (b[0].abs().max(b[2].abs()), b[1].abs().max(b[3].abs())))
    }
    /// The largest magnitude of any coordinate.
    fn reach(&self) -> Option<f64> {
        self.far_corner().map(|(x, y)| x.max(y))
    }
    /// The distance from the origin to the far corner.
    fn far(&self) -> Option<f64> {
        self.far_corner().map(|(x, y)| x.hypot(y))
    }
    /// The far corner's two coordinates added.
    fn sum(&self) -> Option<f64> {
        self.far_corner().map(|(x, y)| x + y)
    }
    /// The distance from the origin to the middle of the box.
    fn centre(&self) -> Option<f64> {
        self.metres()
            .map(|b| ((b[0] + b[2]) / 2.0).hypot((b[1] + b[3]) / 2.0))
    }
    /// The distance from the origin to the near corner: the smaller
    /// magnitude on each axis.
    fn near(&self) -> Option<f64> {
        self.metres()
            .map(|b| b[0].abs().min(b[2].abs()).hypot(b[1].abs().min(b[3].abs())))
    }
    /// The longer side of the box.
    fn span(&self) -> Option<f64> {
        self.metres().map(|b| (b[2] - b[0]).max(b[3] - b[1]))
    }
    /// The diagonal of the box.
    fn diagonal(&self) -> Option<f64> {
        self.metres().map(|b| (b[2] - b[0]).hypot(b[3] - b[1]))
    }
    /// The distance from the origin to the far corner, in the record's own
    /// units.
    fn far_units(&self) -> Option<f64> {
        self.units.map(|u| {
            let x = f64::from(u[0].abs().max(u[2].abs()));
            let y = f64::from(u[1].abs().max(u[3].abs()));
            x.hypot(y)
        })
    }
    fn paper_label(&self) -> String {
        self.paper.map_or_else(
            || "no paper".to_owned(),
            |(width, height, code)| {
                format!("{:.0}x{:.0} ({code})", width * 1000.0, height * 1000.0)
            },
        )
    }
    fn place(&self) -> String {
        format!("{} {}/{}", self.document, self.storage, self.stream)
    }
}

/// The box that holds every box of `boxes` that states an extent.
fn union(boxes: impl Iterator<Item = [i16; 4]>) -> Option<[i32; 4]> {
    let mut all: Option<[i32; 4]> = None;
    for range in boxes {
        if range == NO_EXTENT || range[0] > range[2] || range[1] > range[3] {
            continue;
        }
        let range = range.map(i32::from);
        all = Some(match all {
            None => range,
            Some(held) => [
                held[0].min(range[0]),
                held[1].min(range[1]),
                held[2].max(range[2]),
                held[3].max(range[3]),
            ],
        });
    }
    all
}

fn word(payload: &[u8], at: usize) -> u32 {
    payload.get(at..at + 4).map_or(0, |bytes| {
        u32::from_le_bytes(bytes.try_into().expect("four bytes"))
    })
}

/// The records of one document, on the sheet streams no document read
/// before it held: a drawing caches the sheet of the template it wears, and
/// a package keeps a template beside the drawing it was saved from, so the
/// same stream turns up under several names.
fn read_document(
    label: &str,
    package: &pid_parse::PidPackage,
    seen_streams: &mut HashSet<(usize, u64)>,
    out: &mut Vec<Container>,
) {
    let lower = label.to_ascii_lowercase();
    let document_kind = if lower.ends_with(".igr") {
        "template"
    } else if lower.ends_with(".sym") {
        "symbol"
    } else {
        "drawing"
    };
    let mut paths: Vec<&String> = package.streams.keys().collect();
    paths.sort();
    for path in paths {
        let Some(cut) = path.rfind('/') else {
            continue;
        };
        let (storage, leaf) = (&path[..cut], &path[cut + 1..]);
        let Some(sheet_oid) = leaf
            .strip_prefix("Sheet")
            .and_then(|number| number.parse::<u32>().ok())
        else {
            continue;
        };
        // The document's own sheets and those of its symbol caches.
        let cached = storage.starts_with("/JSite") && !storage[1..].contains('/');
        if !(storage.is_empty() || cached) {
            continue;
        }
        let data = &package.streams[path].data;
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        if !seen_streams.insert((data.len(), hasher.finish())) {
            continue;
        }
        let mut live_oids: HashSet<u32> = HashSet::new();
        let mut associations: Vec<Container> = Vec::new();
        for &at in &SheetChain::walk(data).starts {
            let Some(header) = parse_live_psm_header(data, at) else {
                continue;
            };
            let end = header.body_start + header.bytes_to_follow as usize;
            let Some(payload) = data.get(header.body_start..end) else {
                continue;
            };
            live_oids.insert(word(payload, 0));
            let listed = if header.type_code == PSM_TYPE_CODE_IGCOMPLEXSTRING2D {
                decode_igcomplexstring_at(data, at).map(|string| {
                    (
                        "string",
                        string.oid,
                        string.parent_ref,
                        string.sheet_layer_ref,
                        string.range_scale,
                        string.members,
                    )
                })
            } else if header.type_code == PSM_TYPE_CODE_IGGROUP {
                decode_iggroup_at(data, at).map(|group| {
                    (
                        "group",
                        group.oid,
                        group.parent_ref,
                        group.sheet_layer_ref,
                        group.range_scale,
                        group.members,
                    )
                })
            } else {
                None
            };
            if let Some((what, oid, parent, layer, scale, members)) = listed {
                associations.push(Container {
                    document: label.to_owned(),
                    document_kind,
                    storage: storage.to_owned(),
                    stream: leaf.to_owned(),
                    what,
                    oid,
                    parent,
                    layer,
                    members: members.len(),
                    scale,
                    units: union(members.iter().map(|member| member.range_box)),
                    paper: None,
                    sheet_scale: None,
                });
            }
        }
        // The sheet's own table: the one whose every child is a record of
        // this stream.
        let table = package
            .streams
            .get(&format!("{storage}/PSMcluster0"))
            .and_then(|cluster| {
                find_sheet_child_table(&cluster.data, sheet_oid, |table| {
                    table
                        .children
                        .iter()
                        .all(|child| live_oids.contains(&child.oid))
                })
            });
        let sheet_scale = table.as_ref().map(|table| table.range_scale);
        let paper = table.as_ref().and_then(|table| {
            table
                .paper
                .map(|paper| (paper.width_m, paper.height_m, paper.size_code))
        });
        if let Some(table) = table {
            out.push(Container {
                document: label.to_owned(),
                document_kind,
                storage: storage.to_owned(),
                stream: leaf.to_owned(),
                what: "sheet",
                oid: sheet_oid,
                parent: 0,
                layer: 0,
                members: table.children.len(),
                scale: table.range_scale,
                units: union(table.children.iter().map(|child| child.range)),
                paper,
                sheet_scale,
            });
        }
        for mut association in associations {
            association.sheet_scale = sheet_scale;
            association.paper = paper;
            out.push(association);
        }
    }
}

fn millimetres(metres: Option<f64>) -> String {
    metres.map_or_else(|| "-".to_owned(), |m| format!("{:.1}", m * 1000.0))
}

fn main() {
    let roots = [
        "test-file",
        r"D:\work\cad\pid-test-data",
        r"D:\work\plant-code\cad\pid-test-data",
        r"D:\work\cad\PID自研软件验证",
    ];
    let parser = PidParser::with_options(ParseOptions::geometry());
    let mut seen = HashSet::new();
    let mut seen_streams = HashSet::new();
    let mut containers: Vec<Container> = Vec::new();
    let mut documents_read = 0usize;
    let mut hold = |label: &str, bytes: Vec<u8>| {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        if !seen.insert((bytes.len(), hasher.finish())) {
            return;
        }
        let Ok(package) = parser.parse_package_from_reader(Cursor::new(bytes)) else {
            return;
        };
        documents_read += 1;
        read_document(label, &package, &mut seen_streams, &mut containers);
    };
    for root in roots {
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents(Path::new(root), &mut files, &mut archives);
        for file in &files {
            if let Ok(bytes) = std::fs::read(file) {
                hold(&file.display().to_string(), bytes);
            }
        }
        for archive in &archives {
            for extension in ["igr", "sym", "pid"] {
                let Ok(members) = pid_parse::refdata_archive::archive_members(archive, extension)
                else {
                    continue;
                };
                for member in members {
                    if let Ok(bytes) = pid_parse::refdata_archive::read_archive_file(&member) {
                        hold(&member.label().display().to_string(), bytes);
                    }
                }
            }
        }
    }

    let count = |what: &str| containers.iter().filter(|c| c.what == what).count();
    println!(
        "documents {documents_read}; records {}: {} sheets, {} strings, {} groups",
        containers.len(),
        count("sheet"),
        count("string"),
        count("group")
    );
    let mut scales: BTreeMap<String, usize> = BTreeMap::new();
    for c in &containers {
        *scales.entry(format!("{}", c.scale)).or_default() += 1;
    }
    println!("scales stated: {scales:?}");

    println!("\n== 1. every ComplexString and Group");
    println!(
        "{:<6} {:>5} {:>5} {:>6} {:>5} {:>3}  {:<36} {:>7} {:>7}  {:<5} {:<14} place",
        "kind", "scale", "oid", "parent", "layer", "n", "box, mm", "reach", "far", "sheet", "paper"
    );
    for c in containers.iter().filter(|c| c.what != "sheet") {
        println!(
            "{:<6} {:>5} {:>5} {:>6} {:>5} {:>3}  {:<36} {:>7} {:>7}  {:<5} {:<14} {}",
            c.what,
            c.scale,
            c.oid,
            c.parent,
            c.layer,
            c.members,
            c.metres().map_or_else(
                || "-".to_owned(),
                |b| format!(
                    "{:.1},{:.1} .. {:.1},{:.1}",
                    b[0] * 1000.0,
                    b[1] * 1000.0,
                    b[2] * 1000.0,
                    b[3] * 1000.0
                )
            ),
            millimetres(c.reach()),
            millimetres(c.far()),
            c.sheet_scale
                .map_or_else(|| "-".to_owned(), |scale| format!("{scale}")),
            c.paper_label(),
            c.place(),
        );
    }

    println!("\n== 2. the sheets, by kind of document, storage, paper and scale");
    let mut groups: BTreeMap<SheetKey, SheetTally> = BTreeMap::new();
    for c in containers.iter().filter(|c| c.what == "sheet") {
        let key = (
            c.document_kind.to_owned(),
            if c.storage.is_empty() { "own" } else { "cache" }.to_owned(),
            c.paper_label(),
            format!("{}", c.scale),
        );
        let entry = groups.entry(key).or_insert((0, f64::INFINITY, 0.0));
        entry.0 += 1;
        if let Some(far) = c.far() {
            entry.1 = entry.1.min(far);
            entry.2 = entry.2.max(far);
        }
    }
    for ((kind, storage, paper, scale), (sheets, least, most)) in &groups {
        println!(
            "{kind:<9} {storage:<6} {paper:<16} scale {scale:<4} x{sheets:<5} far corner {:.1} .. {:.1} mm",
            least * 1000.0,
            most * 1000.0
        );
    }

    println!("\n== 3. one place, two scales");
    // Records that share everything a place could give them — the document,
    // the sheet, the owner, the layer and the kind — and differ in scale.
    let mut places: BTreeMap<Place<'_>, Vec<&Container>> = BTreeMap::new();
    for c in containers.iter().filter(|c| c.what != "sheet") {
        places
            .entry((
                c.document.clone(),
                c.storage.clone(),
                c.stream.clone(),
                c.what,
                c.parent,
                c.layer,
            ))
            .or_default()
            .push(c);
    }
    let mut split_places = 0usize;
    for ((document, storage, stream, what, parent, layer), held) in &places {
        let distinct: HashSet<u64> = held.iter().map(|c| c.scale.to_bits()).collect();
        if distinct.len() < 2 {
            continue;
        }
        split_places += 1;
        println!("{document} {storage}/{stream}: {what}s of owner {parent} on layer {layer}");
        for c in held {
            println!(
                "    oid {:<5} scale {:<3} far corner {:>7} mm (sheet {}, paper {})",
                c.oid,
                c.scale,
                millimetres(c.far()),
                c.sheet_scale
                    .map_or_else(|| "-".to_owned(), |scale| format!("{scale}")),
                c.paper_label()
            );
        }
    }
    println!("places that hold records of two scales: {split_places}");
    let unlike_sheet = containers
        .iter()
        .filter(|c| c.what != "sheet" && c.sheet_scale.is_some_and(|sheet| sheet != c.scale))
        .count();
    println!("strings and groups whose scale is not their sheet's: {unlike_sheet}");
    // A paper under two scales.
    let mut by_paper: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for c in containers.iter().filter(|c| c.what == "sheet") {
        *by_paper
            .entry(c.paper_label())
            .or_default()
            .entry(format!("{}", c.scale))
            .or_default() += 1;
    }
    for (paper, by_scale) in &by_paper {
        if by_scale.len() > 1 {
            println!("sheets on {paper} paper state {by_scale:?}");
        }
    }

    println!("\n== 4. does a measure of where the children lie separate the scales?");
    type Measure = fn(&Container) -> Option<f64>;
    let measures: [(&str, Measure); 7] = [
        ("largest coordinate", Container::reach),
        ("far corner, distance from origin", Container::far),
        ("far corner, x + y", Container::sum),
        ("middle of the box, from origin", Container::centre),
        ("near corner, distance from origin", Container::near),
        ("longer side of the box", Container::span),
        ("diagonal of the box", Container::diagonal),
    ];
    for (name, measure) in measures {
        let most_at_one = containers
            .iter()
            .filter(|c| c.scale == 1.0)
            .filter_map(measure)
            .fold(f64::NEG_INFINITY, f64::max);
        let least_at_two = containers
            .iter()
            .filter(|c| c.scale == 2.0)
            .filter_map(measure)
            .fold(f64::INFINITY, f64::min);
        println!(
            "{name:<34} largest at 1.0: {:>8.1} mm   smallest at 2.0: {:>8.1} mm   {}",
            most_at_one * 1000.0,
            least_at_two * 1000.0,
            if most_at_one < least_at_two {
                "separates"
            } else {
                "does not"
            }
        );
    }

    // The seven above are not the only measures there are. The far corner's
    // distance is its 2-norm; its largest coordinate is the limit of the
    // p-norms as p grows, and x + y is the 1-norm. So the family between
    // them is tried, and the weighted sums beside it: which of them
    // separate the two scales, and for which of those the gap between the
    // two holds a metre — the one round length the scale could turn on.
    println!("\n== 4b. which other measures of the far corner separate the scales?");
    let corners = |scale: f64| -> Vec<(f64, f64)> {
        containers
            .iter()
            .filter(|c| c.scale == scale)
            .filter_map(Container::far_corner)
            .collect()
    };
    let (at_one, at_two) = (corners(1.0), corners(2.0));
    // The largest value under 1.0 and the smallest under 2.0, in metres.
    let gap = |measure: &dyn Fn(f64, f64) -> f64| -> (f64, f64) {
        (
            at_one
                .iter()
                .map(|&(x, y)| measure(x, y))
                .fold(f64::NEG_INFINITY, f64::max),
            at_two
                .iter()
                .map(|&(x, y)| measure(x, y))
                .fold(f64::INFINITY, f64::min),
        )
    };
    // The runs of consecutive hundredths that `holds` is true of, as text.
    let runs = |hundredths: std::ops::RangeInclusive<u32>, holds: &dyn Fn(f64) -> bool| -> String {
        let mut found: Vec<(u32, u32)> = Vec::new();
        for step in hundredths {
            if holds(f64::from(step) / 100.0) {
                match found.last_mut() {
                    Some((_, last)) if *last + 1 == step => *last = step,
                    _ => found.push((step, step)),
                }
            }
        }
        if found.is_empty() {
            return "none".to_owned();
        }
        found
            .iter()
            .map(|&(first, last)| {
                format!(
                    "{:.2} to {:.2}",
                    f64::from(first) / 100.0,
                    f64::from(last) / 100.0
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let p_norm = |p: f64| move |x: f64, y: f64| (x.powf(p) + y.powf(p)).powf(1.0 / p);
    let separates = |(most_at_one, least_at_two): (f64, f64)| most_at_one < least_at_two;
    let holds_a_metre =
        |(most_at_one, least_at_two): (f64, f64)| most_at_one < 1.0 && 1.0 <= least_at_two;
    println!(
        "p-norms of the far corner, (x^p + y^p)^(1/p), p from 1.00 to 12.00 by hundredths: separate for p from {}",
        runs(100..=1200, &|p| separates(gap(&p_norm(p))))
    );
    println!(
        "  of those, the ones whose gap holds 1 000 mm: p from {}",
        runs(100..=1200, &|p| holds_a_metre(gap(&p_norm(p))))
    );
    for p in [1.0, 1.5, 1.8, 2.0, 2.2, 3.0, 8.0, 12.0] {
        let (most_at_one, least_at_two) = gap(&p_norm(p));
        println!(
            "  p = {p:<4} largest at 1.0: {:>8.1} mm   smallest at 2.0: {:>8.1} mm   {}",
            most_at_one * 1000.0,
            least_at_two * 1000.0,
            if !separates((most_at_one, least_at_two)) {
                "does not"
            } else if holds_a_metre((most_at_one, least_at_two)) {
                "separates, and the gap holds a metre"
            } else {
                "separates; a metre is not in the gap"
            }
        );
    }
    println!(
        "weighted sums x + k*y, k from 0.00 to 2.00 by hundredths: separate for k from {}; the gap holds 1 000 mm for k from {}",
        runs(0..=200, &|k| separates(gap(&|x, y| x + k * y))),
        runs(0..=200, &|k| holds_a_metre(gap(&|x, y| x + k * y)))
    );
    println!(
        "weighted sums k*x + y, k from 0.00 to 2.00 by hundredths: separate for k from {}; the gap holds 1 000 mm for k from {}",
        runs(0..=200, &|k| separates(gap(&|x, y| k * x + y))),
        runs(0..=200, &|k| holds_a_metre(gap(&|x, y| k * x + y)))
    );

    println!("\n== 5. the rule: 1.0 under a metre to the far corner, 2.0 from one to two");
    let mut broken = 0usize;
    let mut held = 0usize;
    let mut no_extent = 0usize;
    for c in &containers {
        let Some(far) = c.far() else {
            // Nothing with an extent: nothing to decide a scale by.
            no_extent += 1;
            if c.scale != 1.0 {
                broken += 1;
                println!("  NO EXTENT AND NOT 1.0: scale {} {}", c.scale, c.place());
            }
            continue;
        };
        let called_for = if far < 1.0 {
            1.0
        } else if far < 2.0 {
            2.0
        } else {
            4.0
        };
        if c.scale == called_for {
            held += 1;
        } else {
            broken += 1;
            println!(
                "  NOT THE RULE'S: {} oid {} states {} with its far corner at {:.1} mm: {}",
                c.what,
                c.oid,
                c.scale,
                far * 1000.0,
                c.place()
            );
        }
    }
    println!("records that state the rule's scale: {held}; that do not: {broken}; with no child that has an extent (all at 1.0): {no_extent}");
    let far_units = |scale: f64, most: bool| {
        containers
            .iter()
            .filter(|c| c.scale == scale)
            .filter_map(Container::far_units)
            .fold(if most { 0.0 } else { f64::INFINITY }, |held, units| {
                if most {
                    held.max(units)
                } else {
                    held.min(units)
                }
            })
    };
    println!(
        "in the records' own units, the far corner is at most {:.0} of 32768 under 1.0, and from {:.0} to {:.0} under 2.0",
        far_units(1.0, true),
        far_units(2.0, false),
        far_units(2.0, true)
    );
    println!("the nearest to the metre on either side:");
    let mut near: Vec<&Container> = containers
        .iter()
        .filter(|c| c.far().is_some_and(|far| (0.9..1.1).contains(&far)))
        .collect();
    near.sort_by(|a, b| a.far().partial_cmp(&b.far()).expect("finite"));
    let mut printed: HashSet<(String, u64)> = HashSet::new();
    for c in near {
        let far = c.far().unwrap_or_default();
        // One line for each distance a kind of record is seen at.
        if !printed.insert((
            format!("{} {} {}", c.what, c.document_kind, c.paper_label()),
            (far * 1e4).round() as u64,
        )) {
            continue;
        }
        let (x, y) = c.far_corner().unwrap_or_default();
        println!(
            "  {:>7.1} mm ({:.1}, {:.1})  scale {:<3} {:<6} {:<9} paper {:<14} {}",
            far * 1000.0,
            x * 1000.0,
            y * 1000.0,
            c.scale,
            c.what,
            c.document_kind,
            c.paper_label(),
            c.place()
        );
    }

    // Any argument is part of a document's path: its records are listed.
    for wanted in std::env::args().skip(1) {
        println!("\n== the records of the documents named {wanted:?}");
        for c in containers.iter().filter(|c| c.document.contains(&wanted)) {
            let (x, y) = c.far_corner().unwrap_or_default();
            println!(
                "  {:<6} oid {:<5} scale {:<3} far corner ({:.1}, {:.1}) = {:>7} mm, {} children, paper {}  {}",
                c.what,
                c.oid,
                c.scale,
                x * 1000.0,
                y * 1000.0,
                millimetres(c.far()),
                c.members,
                c.paper_label(),
                c.place()
            );
        }
    }

    let other_scales = containers
        .iter()
        .filter(|c| c.scale != 1.0 && c.scale != 2.0)
        .count();
    if broken != 0 || other_scales != 0 {
        std::process::exit(1);
    }
}
