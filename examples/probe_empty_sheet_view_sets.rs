//! Do the clean view sets that govern an empty sheet belong to a body that was
//! deleted, or to a sheet that was never drawn on?
//!
//! `SymbolGeometry::displayed_primitives` refuses a "normal" sheet that draws
//! nothing, because 163 of the library's 868 clean sets govern a sheet with no
//! primitives. That reading only knows what survived. A sheet can come out
//! empty because
//!
//! - the file has no `Sheet<N>` stream for it at all,
//! - the stream is there but holds no record chain,
//! - the chain parses but holds no geometry family,
//! - the chain holds geometry this reader refuses (zero radius, zero length,
//!   coordinates out of range), or
//! - the chain holds geometry the body also keeps on another sheet, which
//!   `read_symbol_geometry` drops across sheets so a body filed twice draws
//!   once.
//!
//! None of those is a deletion. This probe counts them from the bytes and then
//! asks what *would* be evidence of one: a `0x0114 JSheet` object whose stream
//! is gone, a view set naming a sheet no record in the file mentions, and the
//! free lists a storage hands back when an object is removed.
//!
//! ```text
//! cargo run --example probe_empty_sheet_view_sets -- [--root DIR]... [--inventory FILE] [--detail N]
//! ```
//!
//! Read-only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::symbol_library::{read_symbol_geometry, SymbolSheetView};
use pid_parse::PidParser;

const CHAIN_START: usize = 8;
const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const PSM_ENVELOPE_LEN: usize = 6;

/// The PSM type codes `decode_primitive` turns into a `SymbolPrimitive`.
const GEOMETRY_TYPES: [u16; 7] = [0x0018, 0x0059, 0x0061, 0x0084, 0x004D, 0x005D, 0x0013];

/// `JSheet Object` (`docext.dex`).
const J_SHEET: u16 = 0x0114;

const HIDDEN: &[&str] = &[
    "heattrace",
    "hiddenobjects",
    "jacket",
    "label",
    "construction",
    "dimension",
    "deminsion",
];

fn hidden(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    HIDDEN.contains(&normalized.as_str())
}

fn bit_is_set(bitmap: &[u8], n: u16) -> bool {
    bitmap
        .get(usize::from(n / 8))
        .is_some_and(|byte| byte & (1 << (n % 8)) != 0)
}

fn supplementary(view: &SymbolSheetView) -> bool {
    view.layers
        .iter()
        .any(|(number, name)| hidden(name) && bit_is_set(&view.display, *number))
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// The library copies this machine holds: the corpus's reference share first,
/// then the copy checked in under `test-file/`.
fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let ref_dir = corpus.join("Ref");
    if ref_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ref_dir) {
            let mut found: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| {
                    entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("symbol")
                })
                .map(|entry| entry.path())
                .collect();
            found.sort();
            roots.extend(found);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    roots
}

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn stream_leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or("")
}

/// The sheet oid a `Sheet<N>` stream name carries; `None` for any other name.
fn sheet_of_stream(name: &str) -> Option<u32> {
    name.strip_prefix("Sheet")?.parse::<u32>().ok()
}

/// One record of a `Sheet<N>` chain or a `PSMcluster0` body.
struct Record {
    type_code: u16,
    payload: Vec<u8>,
}

/// Walk a cluster-family stream's record chain from [`CHAIN_START`], the same
/// layout `read_sheet_chain` reads. `None` when the walk cannot account for
/// the stream exactly — the reader's own refusal rule.
fn walk_chain(bytes: &[u8]) -> Option<Vec<Record>> {
    if bytes.len() <= CHAIN_START {
        return None;
    }
    let magic = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
    if magic != CLUSTER_MAGIC {
        return None;
    }
    let mut out = Vec::new();
    let mut at = CHAIN_START;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let raw_type = u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?);
        if raw_type == 0 {
            return None;
        }
        let length = u32::from_le_bytes(bytes.get(at + 2..at + 6)?.try_into().ok()?) as usize;
        let body = at + PSM_ENVELOPE_LEN;
        let end = body.checked_add(length)?;
        if end > bytes.len() {
            return None;
        }
        out.push(Record {
            type_code: raw_type & 0x0FFF,
            payload: bytes[body..end].to_vec(),
        });
        at = end;
    }
    (at == bytes.len()).then_some(out)
}

/// What one geometry record decodes to, as the identity
/// `read_symbol_geometry` de-duplicates on: the primitive's own values, with
/// the layer oid, the style index and any trailing bytes left out — a record
/// with a longer payload still decodes to the primitive its first fields
/// state, and the reader treats the two as the same stroke.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
struct Shape {
    code: u16,
    bits: Vec<u64>,
}

/// The identity of the primitive a record decodes to, or `None` where the
/// reader refuses the record (`decode_primitive`'s own checks).
fn shape_of(code: u16, payload: &[u8]) -> Option<Shape> {
    let word = |at: usize| -> Option<u64> {
        payload
            .get(at..at + 8)
            .and_then(|s| <[u8; 8]>::try_from(s).ok())
            .map(u64::from_le_bytes)
    };
    let value = |at: usize| word(at).map(f64::from_bits);
    let in_extent = |value: f64| value.is_finite() && value.abs() <= 10.0;
    let fixed =
        |slots: &[usize]| -> Option<Vec<u64>> { slots.iter().map(|at| word(*at)).collect() };
    let bits = match code {
        0x0018 => {
            if payload.len() < 18 + 32 {
                return None;
            }
            let points = [value(18)?, value(26)?, value(34)?, value(42)?];
            if points.iter().any(|value| !in_extent(*value)) {
                return None;
            }
            if points[0] == points[2] && points[1] == points[3] {
                return None;
            }
            fixed(&[18, 26, 34, 42])?
        }
        0x0059 => {
            if payload.len() < 18 + 24 + 1 {
                return None;
            }
            let radius = value(34)?;
            if !in_extent(value(18)?)
                || !in_extent(value(26)?)
                || !in_extent(radius)
                || radius <= 0.0
            {
                return None;
            }
            fixed(&[18, 26, 34])?
        }
        0x0061 => {
            if payload.len() < 18 + 40 + 1 {
                return None;
            }
            let radius = value(34)?;
            if !in_extent(value(18)?)
                || !in_extent(value(26)?)
                || !in_extent(radius)
                || radius <= 0.0
                || !value(42)?.is_finite()
                || !value(50)?.is_finite()
            {
                return None;
            }
            fixed(&[18, 26, 34, 42, 50])?
        }
        // Text: the run the tail arithmetic proves, with the insertion point
        // that follows it — `SymbolPrimitive::Text`'s own two fields.
        0x004D => {
            let run = [30usize, 22].iter().find_map(|count_at| {
                let count = usize::from(
                    payload
                        .get(*count_at..*count_at + 2)
                        .map(|s| u16::from_le_bytes([s[0], s[1]]))?,
                );
                if count == 0 || count > 1024 {
                    return None;
                }
                let start = count_at + 2;
                let end = start.checked_add(count * 2)?;
                if payload.len() != end.checked_add(36)? {
                    return None;
                }
                let units: Vec<u16> = (0..count)
                    .map(|index| {
                        payload
                            .get(start + index * 2..start + index * 2 + 2)
                            .map(|s| u16::from_le_bytes([s[0], s[1]]))
                    })
                    .collect::<Option<_>>()?;
                let text = String::from_utf16(&units).ok()?;
                if text.contains('\0') {
                    return None;
                }
                let at = (value(end)?, value(end + 8)?);
                if !in_extent(at.0) || !in_extent(at.1) {
                    return None;
                }
                let mut bits: Vec<u64> = text.encode_utf16().map(u64::from).collect();
                bits.push(word(end)?);
                bits.push(word(end + 8)?);
                Some(bits)
            });
            run?
        }
        // Polyline, B-spline and boundary: what they decode to is not
        // reproducible from the payload alone here, so the identity is the
        // bytes past the header slot.
        _ => payload
            .get(18..)
            .unwrap_or_default()
            .chunks(8)
            .map(|chunk| {
                let mut wide = [0u8; 8];
                wide[..chunk.len()].copy_from_slice(chunk);
                u64::from_le_bytes(wide)
            })
            .collect(),
    };
    Some(Shape { code, bits })
}

/// What one `Sheet<N>` stream holds, from the bytes alone.
#[derive(Default)]
struct SheetStream {
    len: usize,
    /// The `u32` the cluster header carries behind the magic — the count the
    /// storage wrote for the records it holds. A stream whose chain is empty
    /// but whose header counts objects is a sheet that was emptied after the
    /// header was written.
    declared: u32,
    records: Option<Vec<Record>>,
    census: BTreeMap<u16, usize>,
    /// Every geometry record's decoded identity; the comparison
    /// `read_symbol_geometry` makes when it drops a sheet's copy of a body
    /// another sheet holds.
    shapes: Vec<Shape>,
    /// Geometry records the reader refuses one by one.
    refused: BTreeMap<u16, usize>,
}

impl SheetStream {
    fn catalog(bytes: &[u8]) -> Self {
        let records = walk_chain(bytes);
        let mut census = BTreeMap::new();
        let mut shapes = Vec::new();
        let mut refused = BTreeMap::new();
        if let Some(records) = &records {
            for record in records {
                *census.entry(record.type_code).or_default() += 1;
                if !GEOMETRY_TYPES.contains(&record.type_code) {
                    continue;
                }
                match shape_of(record.type_code, &record.payload) {
                    Some(shape) => shapes.push(shape),
                    None => *refused.entry(record.type_code).or_default() += 1,
                }
            }
        }
        let declared = bytes
            .get(4..8)
            .and_then(|s| <[u8; 4]>::try_from(s).ok())
            .map_or(0, u32::from_le_bytes);
        Self {
            len: bytes.len(),
            declared,
            records,
            census,
            shapes,
            refused,
        }
    }

    fn chain_ok(&self) -> bool {
        self.records.is_some()
    }

    fn geometry_types(&self) -> String {
        let mut tally: BTreeMap<u16, usize> = BTreeMap::new();
        for shape in &self.shapes {
            *tally.entry(shape.code).or_default() += 1;
        }
        for (code, count) in &self.refused {
            *tally.entry(*code).or_default() += count;
        }
        tally
            .iter()
            .map(|(code, count)| format!("0x{code:04X} x{count}"))
            .collect::<Vec<_>>()
            .join(" + ")
    }

    fn refused_types(&self) -> String {
        self.refused
            .iter()
            .map(|(code, count)| format!("0x{code:04X} x{count}"))
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

/// Why a governed sheet has no primitives in the decoded body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Why {
    NoStream,
    HeaderOnly,
    UnreadableChain,
    NoGeometryFamily,
    /// Every geometry record on the sheet is one the reader refuses (zero
    /// length, zero radius, out-of-range coordinate, a payload too short).
    AllRefused,
    /// Some records decode, but the same shapes another sheet also holds, so
    /// the reader drops this sheet's copy and a body filed twice draws once.
    DuplicateOfAnotherSheet,
    /// Records decode to shapes nothing else holds, yet no primitive landed on
    /// the sheet — a drop this probe cannot reproduce.
    DecodedButAbsent,
    /// No record decodes and none is refused by the checks this probe can
    /// repeat: text, polyline, B-spline and boundary payloads, whose identity
    /// is not readable from the bytes here.
    Opaque,
}

impl Why {
    fn label(self) -> &'static str {
        match self {
            Self::NoStream => "no Sheet<N> stream at all",
            Self::HeaderOnly => "stream present, no record chain",
            Self::UnreadableChain => "stream present, chain does not close",
            Self::NoGeometryFamily => "chain holds no geometry record",
            Self::AllRefused => "geometry records, all refused by the reader",
            Self::DuplicateOfAnotherSheet => "geometry duplicates another sheet's",
            Self::DecodedButAbsent => "geometry decodes, none of it landed",
            Self::Opaque => "geometry families with no readable identity here",
        }
    }
}

/// The `0x0114` oids a file's `PSMcluster0` streams carry, plus the raw
/// record census of those streams.
struct ClusterCensus {
    j_sheet_oids: BTreeSet<u32>,
    census: BTreeMap<u16, usize>,
}

fn cluster_census(package_psms: &[&[u8]]) -> ClusterCensus {
    let mut out = ClusterCensus {
        j_sheet_oids: BTreeSet::new(),
        census: BTreeMap::new(),
    };
    for bytes in package_psms {
        let Some(records) = walk_chain(bytes) else {
            continue;
        };
        for record in records {
            *out.census.entry(record.type_code).or_default() += 1;
            if record.type_code == J_SHEET {
                if let Some(oid) = record.payload.get(0..4) {
                    out.j_sheet_oids
                        .insert(u32::from_le_bytes(oid.try_into().unwrap_or_default()));
                }
            }
        }
    }
    out
}

/// Everything one `.sym` says about a governed-but-empty sheet.
struct Case {
    name: String,
    sheet: u32,
    why: Why,
    stream_len: usize,
    stream_records: usize,
    j_sheet_alive: bool,
    /// The count the cluster header declares against the records it holds.
    declared: u32,
    /// The geometry families the stream does hold, `0xNNNN xN` each.
    stream_geometry: String,
    /// The geometry records this probe's copy of the reader's checks refuses.
    stream_refused: String,
    /// Which other sheets hold the identical geometry shapes.
    duplicate_sheets: Vec<u32>,
    /// How many primitives the decoded body files on some sheet, and which.
    body_sheets: BTreeMap<u32, usize>,
}

fn main() -> ExitCode {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut inventory: Option<String> = None;
    let mut detail = 0usize;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "--root" => {
                at += 1;
                if let Some(dir) = args.get(at) {
                    roots.push(PathBuf::from(dir));
                }
            }
            "--inventory" => {
                at += 1;
                inventory = args.get(at).cloned();
            }
            "--detail" => {
                at += 1;
                detail = args.get(at).and_then(|n| n.parse().ok()).unwrap_or(10);
            }
            other => roots.push(PathBuf::from(other)),
        }
        at += 1;
    }
    if roots.is_empty() {
        roots = library_roots(&corpus_dir());
    }

    let mut files = Vec::new();
    for root in &roots {
        collect_syms(root, &mut files);
    }
    if files.is_empty() {
        eprintln!("no .sym files under {roots:?}");
        return ExitCode::from(2);
    }

    if let Some(needle) = inventory
        .as_ref()
        .and_then(|needle| needle.strip_prefix("dump:"))
    {
        for path in files
            .iter()
            .filter(|path| path.to_string_lossy().contains(needle))
        {
            let Ok(package) = PidParser::new().parse_package(path) else {
                println!("{}: unreadable", path.display());
                continue;
            };
            println!("\n== {}", path.display());
            for (stream_path, raw) in &package.streams {
                let Some(sheet) = sheet_of_stream(stream_leaf(stream_path)) else {
                    continue;
                };
                let stream = SheetStream::catalog(&raw.data);
                println!(
                    "  Sheet{sheet}: {} B, header {} records, chain {:?}",
                    stream.len,
                    stream.declared,
                    stream.census.values().sum::<usize>()
                );
                for shape in &stream.shapes {
                    let values: Vec<String> = shape
                        .bits
                        .iter()
                        .map(|bits| format!("{:.6}", f64::from_bits(*bits)))
                        .collect();
                    println!("     0x{:04X} {}", shape.code, values.join(" "));
                }
                for (code, count) in &stream.refused {
                    println!("     refused 0x{code:04X} x{count}");
                }
            }
        }
        return ExitCode::SUCCESS;
    }

    if let Some(needle) = inventory {
        for path in files
            .iter()
            .filter(|path| path.to_string_lossy().contains(&needle))
        {
            let Ok(package) = PidParser::new().parse_package(path) else {
                println!("{}: unreadable", path.display());
                continue;
            };
            println!("\n== {}", path.display());
            for (stream_path, raw) in &package.streams {
                println!("   {:>9} B  {stream_path}", raw.data.len());
            }
        }
        return ExitCode::SUCCESS;
    }

    let mut read = 0usize;
    let mut cases: Vec<Case> = Vec::new();
    let mut why_tally: BTreeMap<Why, usize> = BTreeMap::new();
    let mut files_with_cases = 0usize;
    let mut files_only_empty = 0usize;
    let mut no_body_names: BTreeSet<String> = BTreeSet::new();
    let mut kept_by_path_order = 0usize;
    let mut j_sheet_alive = 0usize;
    let mut j_sheet_gone = 0usize;
    let mut cluster_records: BTreeMap<u16, usize> = BTreeMap::new();
    let mut streams_absent_from_cluster: BTreeMap<&'static str, usize> = BTreeMap::new();

    for path in &files {
        let Ok(body) = read_symbol_geometry(path) else {
            continue;
        };
        read += 1;
        let Ok(package) = PidParser::new().parse_package(path) else {
            continue;
        };
        let psms: Vec<&[u8]> = package
            .streams
            .iter()
            .filter(|(stream_path, _)| stream_leaf(stream_path) == "PSMcluster0")
            .map(|(_, raw)| raw.data.as_slice())
            .collect();
        if psms.is_empty() {
            *streams_absent_from_cluster
                .entry("no PSMcluster0")
                .or_default() += 1;
            continue;
        }
        let cluster = cluster_census(&psms);
        for (code, count) in &cluster.census {
            *cluster_records.entry(*code).or_default() += count;
        }

        let body_sheets: BTreeMap<u32, usize> = {
            let mut tally: BTreeMap<u32, usize> = BTreeMap::new();
            for styled in &body.primitives {
                *tally.entry(styled.sheet_ref).or_default() += 1;
            }
            tally
        };
        let streams: BTreeMap<u32, SheetStream> = package
            .streams
            .iter()
            .filter_map(|(stream_path, raw)| {
                sheet_of_stream(stream_leaf(stream_path))
                    .map(|sheet| (sheet, SheetStream::catalog(&raw.data)))
            })
            .collect();
        // Identical shapes on two sheets: the reader keeps the first sheet's
        // copy and drops the second's.
        // The reader keeps a shape the first sheet that files it contributes,
        // so a sheet's primitives all vanish when every shape it holds is
        // already filed by another sheet.
        let duplicate_sheets = |shapes: &[Shape], sheet: u32| -> Vec<u32> {
            if shapes.is_empty() {
                return Vec::new();
            }
            streams
                .iter()
                .filter(|(other, stream)| {
                    **other != sheet
                        && !stream.shapes.is_empty()
                        && shapes.iter().all(|shape| stream.shapes.contains(shape))
                })
                .map(|(other, _)| *other)
                .collect()
        };

        let clean: Vec<&SymbolSheetView> = body
            .sheet_views
            .iter()
            .filter(|view| !supplementary(view))
            .collect();
        if clean.is_empty() {
            continue;
        }
        let empty: Vec<&SymbolSheetView> = clean
            .into_iter()
            .filter(|view| !body_sheets.contains_key(&view.sheet_ref))
            .collect();
        if empty.is_empty() {
            continue;
        }
        files_with_cases += 1;
        if body.primitives.is_empty() {
            files_only_empty += 1;
            if let Some(name) = path.file_name() {
                no_body_names.insert(name.to_string_lossy().into_owned());
            }
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for view in empty {
            let sheet = view.sheet_ref;
            let stream = streams.get(&sheet);
            let twins =
                stream.map_or_else(Vec::new, |stream| duplicate_sheets(&stream.shapes, sheet));
            let why = match stream {
                None => Why::NoStream,
                Some(stream) if stream.len <= CHAIN_START => Why::HeaderOnly,
                Some(stream) if !stream.chain_ok() => Why::UnreadableChain,
                Some(stream) if stream.shapes.is_empty() && stream.refused.is_empty() => {
                    Why::NoGeometryFamily
                }
                Some(stream) if stream.shapes.is_empty() => Why::AllRefused,
                Some(_) if !twins.is_empty() => Why::DuplicateOfAnotherSheet,
                Some(stream) if stream.refused.len() == stream.census.len() => Why::Opaque,
                Some(_) => Why::DecodedButAbsent,
            };
            *why_tally.entry(why).or_default() += 1;
            if why == Why::DuplicateOfAnotherSheet {
                // The package's streams are a `BTreeMap`, so the copy that
                // survives is the one whose stream path sorts first.
                let mine = format!("Sheet{sheet}");
                let earliest = twins
                    .iter()
                    .map(|twin| format!("Sheet{twin}"))
                    .min()
                    .unwrap_or_default();
                if earliest < mine {
                    kept_by_path_order += 1;
                }
            }
            let alive = cluster.j_sheet_oids.contains(&sheet);
            if alive {
                j_sheet_alive += 1;
            } else {
                j_sheet_gone += 1;
            }
            cases.push(Case {
                name: name.clone(),
                sheet,
                why,
                stream_len: stream.map_or(0, |s| s.len),
                declared: stream.map_or(0, |s| s.declared),
                stream_records: stream.map_or(0, |s| s.census.values().sum()),
                stream_geometry: stream.map_or_else(String::new, SheetStream::geometry_types),
                stream_refused: stream.map_or_else(String::new, SheetStream::refused_types),
                j_sheet_alive: alive,
                duplicate_sheets: twins,
                body_sheets: body_sheets.clone(),
            });
        }
    }

    println!("{read} library symbols read ({} files)", files.len());
    println!(
        "{} clean view sets govern a sheet with no primitives",
        cases.len()
    );
    println!(
        "  {files_with_cases} files carry at least one, {files_only_empty} of those have no body anywhere ({})",
        no_body_names
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("\nwhy the sheet is empty:");
    for (why, count) in &why_tally {
        println!("  {count:5}  {}", why.label());
    }
    println!("\nof the copies, {kept_by_path_order} keep the twin whose stream path sorts first");
    println!("\nis a 0x0114 JSheet object still there for it?");
    println!("  {j_sheet_alive} alive, {j_sheet_gone} absent from PSMcluster0");
    let over_declared: Vec<&Case> = cases
        .iter()
        .filter(|case| case.declared as usize > case.stream_records)
        .collect();
    let under_declared: Vec<&Case> = cases
        .iter()
        .filter(|case| (case.declared as usize) < case.stream_records)
        .collect();
    println!("\ndoes the cluster header out-count the chain?");
    println!(
        "  {} headers declare more records than the chain holds, {} declare fewer",
        over_declared.len(),
        under_declared.len()
    );
    for case in over_declared.iter().take(5) {
        println!(
            "    {} Sheet{}: header {} vs chain {}",
            case.name, case.sheet, case.declared, case.stream_records
        );
    }
    println!("\nPSMcluster0 record census across those files (top):");
    let mut census: Vec<_> = cluster_records.into_iter().collect();
    census.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (code, count) in census.iter().take(12) {
        println!("  0x{code:04X}  {count}");
    }
    for (note, count) in &streams_absent_from_cluster {
        println!("  {count} files {note}");
    }

    if detail > 0 {
        for (why, heading) in [
            (
                Why::DuplicateOfAnotherSheet,
                "sheets whose body another sheet also holds",
            ),
            (
                Why::AllRefused,
                "sheets whose geometry records are all refused",
            ),
            (
                Why::Opaque,
                "sheets whose geometry is a family this probe cannot key",
            ),
            (
                Why::DecodedButAbsent,
                "sheets whose geometry decodes yet does not land",
            ),
            (Why::HeaderOnly, "sheets whose stream holds no record chain"),
            (Why::NoStream, "sheets with no stream at all"),
        ] {
            println!("\n{heading}:");
            for case in cases.iter().filter(|case| case.why == why).take(detail) {
                let sheets: Vec<String> = case
                    .body_sheets
                    .iter()
                    .map(|(sheet, count)| format!("Sheet{sheet} x{count}"))
                    .collect();
                println!(
                    "  {} Sheet{}: {} B, header says {} records, {} in the chain, [{}]{}; JSheet {}; body on [{}]{}",
                    case.name,
                    case.sheet,
                    case.stream_len,
                    case.declared,
                    case.stream_records,
                    case.stream_geometry,
                    if case.stream_refused.is_empty() {
                        String::new()
                    } else {
                        format!(" (refused: {})", case.stream_refused)
                    },
                    if case.j_sheet_alive {
                        "alive"
                    } else {
                        "gone"
                    },
                    sheets.join(", "),
                    if case.duplicate_sheets.is_empty() {
                        String::new()
                    } else {
                        format!(" twins {:?}", case.duplicate_sheets)
                    }
                );
            }
        }
    }
    ExitCode::SUCCESS
}
