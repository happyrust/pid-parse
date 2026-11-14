//! How high a degree does an `igBspCurve2d` record of the corpora state, with
//! how many poles and knots — and do the curves the corpora hold decode and
//! sample, to the bit, as they did before a change?
//!
//! A B-spline is evaluated by de Boor's recursion, whose work for one point
//! is the square of the degree, and the degree is not a field of the record:
//! it is `knots − poles − 1`, whatever the two counts say. Until 2026-11-12 a
//! record could state any degree up to 4 095, and one of 4 096 poles and
//! degree 1 024 took 358 seconds to emit in a debug build. The decoder now
//! refuses a degree past `pid_parse::bspline::MAX_DEGREE`, and
//! `pid_parse::bspline::sample` hands such a curve back as its poles. This
//! probe is what that bound was set against, and what shows it moved nothing
//! a file holds.
//!
//! It reads every chain stream — a `Sheet*`, a symbol cache's `PSMcluster0`,
//! any stream that opens with the cluster magic — of every distinct drawing,
//! template and symbol in reach: `test-file`, both `pid-test-data`, the
//! share, and the `.igr` / `.sym` / `.pid` members of their `RefData~*.zip`
//! and `PlantData~*.zip`. Each record the chain starts with type code
//! `0x005D` is read, a superseded copy included.
//!
//! With no argument it prints the census: how many such records there are and
//! where (a drawing's sheet, a template's, a symbol file's, a symbol cache in
//! a drawing), the degrees, the pole and knot counts, the weights of the
//! rational ones, the closing bytes and the double before them, and how the
//! knot vectors are laid out. It exits non-zero when a record of the corpora
//! does not decode.
//!
//! ```text
//! cargo run --example probe_how_high_a_degree_a_bspline_record_states
//! ```
//!
//! `record <file>` writes one line for each thing a change to the decoder or
//! to the evaluation could move, each with a 128-bit sum of the bit patterns
//! involved: every `0x005D` record as decoded and as sampled; every
//! document's whole normalized geometry (the polylines the sheet-level
//! B-splines and elliptical arcs are emitted as among it); the B-splines of a
//! drawing's cached symbol bodies as sampled; and each `.sym` body as the
//! symbol reader gives it, with its B-splines as sampled. `compare <a> <b>`
//! reads two such files and names every line that differs, exiting non-zero
//! when one does. Record on the commit before a change and on the one after,
//! with this file unchanged between the two.
//!
//! A recording is the same from run to run, which it was not until a symbol
//! body's layer table was sorted before it was summed: when this probe was
//! written `SymbolGeometry::sheet_layers` listed the layers of each
//! `PSMcluster0` of the file in the order a hash map yielded the storages.
//! Forty symbols of the committed `RefData~4~681.zip` hold more than one
//! such stream — 38 control valves, each with a symbol cache of its own,
//! and two title-block labels — and between two runs of one build fourteen
//! of them came out the other way round. (A primitive's layer name is
//! resolved against its own storage's table, so the order moved nothing
//! that is drawn.) The library lists them in the storages' own order since
//! #50; the sort stays, so that a recording made on a commit before that
//! compares with one made after.
//!
//! ```text
//! cargo run --example probe_how_high_a_degree_a_bspline_record_states -- record before.txt
//! cargo run --example probe_how_high_a_degree_a_bspline_record_states -- record after.txt
//! cargo run --example probe_how_high_a_degree_a_bspline_record_states -- compare before.txt after.txt
//! ```
//!
//! Read on 2026-11-12, 885 documents: see
//! `docs/analysis/2026-11-12-a-bspline-record-is-held-to-degree-32.md` for the
//! census and for the comparison across the change.
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use pid_parse::bspline::{sample, SEGMENTS_PER_SPAN};
use pid_parse::parsers::sheet_records::{
    bspcurve_geometry, parse_psm_header, SheetChain, PSM_TYPE_CODE_IGBSPCURVE2D, SHEET_STREAM_MAGIC,
};
use pid_parse::symbol_library::{read_symbol_geometry_from_bytes, SymbolPrimitive};
use pid_parse::{build_normalized_geometry, ParseOptions, PidParser};

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

/// Two running sums of the same bytes, 128 bits between them: enough to say
/// two bit patterns are the same without keeping either.
struct Sum(u64, u64);

impl Sum {
    fn new() -> Self {
        Self(0xCBF2_9CE4_8422_2325, 0x9E37_79B9_7F4A_7C15)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
            self.1 = (self.1.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        }
    }

    /// The bit patterns of `values`, so that `0.0` and `-0.0`, and two NaNs
    /// of different payloads, are told apart.
    fn doubles(&mut self, values: impl IntoIterator<Item = f64>) {
        for value in values {
            self.bytes(&value.to_bits().to_le_bytes());
        }
    }

    fn points(&mut self, points: &[(f64, f64)]) {
        self.bytes(&(points.len() as u64).to_le_bytes());
        self.doubles(points.iter().flat_map(|(x, y)| [*x, *y]));
    }

    fn hex(&self) -> String {
        format!("{:016x}{:016x}", self.0, self.1)
    }
}

/// One `0x005D` record a chain starts, as the census keeps it.
struct Curve {
    /// The document, the stream and the offset of the record's envelope.
    place: String,
    /// `drawing`, `template` or `symbol`, by the document's extension.
    document: &'static str,
    /// `sheet` for a `Sheet*` of the document's own storage, `cache` for a
    /// stream under a `JSite*` storage, `other` for the rest.
    stream: &'static str,
    /// Whether the native reader reads the record (its type word does not
    /// carry `0x8000`).
    live: bool,
    degree: usize,
    poles: usize,
    knots: usize,
    /// The smallest and the largest weight, of a rational curve.
    weights: Option<(f64, f64)>,
    flags: [u8; 4],
    trailing: f64,
    /// The smallest step between two knots that is not zero.
    least_knot_step: f64,
    /// Whether the first `degree + 1` knots are one value and the last
    /// `degree + 1` another.
    clamped: bool,
    /// Whether the knots leave no parameter domain (`knots[poles] <=
    /// knots[degree]`): `sample` hands such a curve back as its poles.
    no_domain: bool,
}

/// What the walk over the corpora read.
#[derive(Default)]
struct Reading {
    documents: usize,
    unreadable: Vec<String>,
    chain_streams: usize,
    curves: Vec<Curve>,
    /// Records of type `0x005D` at a chain start that the decoder refuses.
    refused: Vec<String>,
    /// The lines of `record`.
    lines: Vec<String>,
}

fn document_kind(label: &str) -> &'static str {
    let lower = label.to_ascii_lowercase();
    if lower.ends_with(".igr") {
        "template"
    } else if lower.ends_with(".sym") {
        "symbol"
    } else {
        "drawing"
    }
}

fn stream_kind(path: &str) -> &'static str {
    let leaf = path.rsplit('/').next().unwrap_or_default();
    if path.starts_with("/JSite") {
        "cache"
    } else if leaf.starts_with("Sheet") {
        "sheet"
    } else {
        "other"
    }
}

/// The `0x005D` records of one chain stream.
fn read_stream(label: &str, path: &str, data: &[u8], keep_lines: bool, reading: &mut Reading) {
    for at in SheetChain::walk(data).starts {
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        if header.type_code != PSM_TYPE_CODE_IGBSPCURVE2D {
            continue;
        }
        let place = format!("{label} {path} +{at}");
        let end = header.body_start + header.bytes_to_follow as usize;
        let Some(curve) = data.get(header.body_start..end).and_then(bspcurve_geometry) else {
            reading.refused.push(place);
            continue;
        };
        let (poles, knots) = (curve.poles.len(), curve.knots.len());
        let degree = knots.saturating_sub(poles + 1);
        let weights = (!curve.weights.is_empty()).then(|| {
            curve.weights.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(least, most), weight| (least.min(*weight), most.max(*weight)),
            )
        });
        let least_knot_step = curve
            .knots
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .filter(|step| *step > 0.0)
            .fold(f64::INFINITY, f64::min);
        let clamped = degree < knots
            && curve.knots[..=degree].iter().all(|k| *k == curve.knots[0])
            && curve.knots[knots - degree - 1..]
                .iter()
                .all(|k| *k == curve.knots[knots - 1]);
        let no_domain = match (curve.knots.get(poles), curve.knots.get(degree)) {
            (Some(end), Some(start)) => end <= start,
            _ => true,
        };
        if keep_lines {
            let mut decoded = Sum::new();
            decoded.points(&curve.poles);
            decoded.bytes(&(curve.weights.len() as u64).to_le_bytes());
            decoded.doubles(curve.weights.iter().copied());
            decoded.bytes(&(knots as u64).to_le_bytes());
            decoded.doubles(curve.knots.iter().copied());
            decoded.doubles([curve.trailing]);
            decoded.bytes(&curve.flags);
            let points = sample(
                &curve.poles,
                &curve.weights,
                &curve.knots,
                SEGMENTS_PER_SPAN,
            );
            let mut sampled = Sum::new();
            sampled.points(&points);
            reading.lines.push(format!(
                "record\t{place}\tdegree {degree} poles {poles} knots {knots}\tdecoded {}\tsampled {} ({} points)",
                decoded.hex(),
                sampled.hex(),
                points.len()
            ));
        }
        reading.curves.push(Curve {
            place,
            document: document_kind(label),
            stream: stream_kind(path),
            live: !header.native_reader_skips(),
            degree,
            poles,
            knots,
            weights,
            flags: curve.flags,
            trailing: curve.trailing,
            least_knot_step,
            clamped,
            no_domain,
        });
    }
}

/// The B-splines among `primitives`, sampled as a renderer samples them.
fn sampled_bsplines<'a>(primitives: impl Iterator<Item = &'a SymbolPrimitive>) -> (usize, Sum) {
    let mut sum = Sum::new();
    let mut curves = 0usize;
    for primitive in primitives {
        if matches!(primitive, SymbolPrimitive::BSpline { .. }) {
            curves += 1;
            sum.points(&primitive.bspline_points(SEGMENTS_PER_SPAN));
        }
    }
    (curves, sum)
}

/// One document: its `0x005D` records for the census and, for `record`, the
/// lines of everything a change could move.
fn read_document(
    label: &str,
    bytes: &[u8],
    parser: &PidParser,
    seen_streams: &mut HashSet<(usize, u64)>,
    keep_lines: bool,
    reading: &mut Reading,
) {
    let Ok(package) = parser.parse_package_from_reader(Cursor::new(bytes.to_vec())) else {
        reading.unreadable.push(label.to_owned());
        return;
    };
    reading.documents += 1;
    for (path, raw) in &package.streams {
        let data = raw.data.as_slice();
        let opens_a_chain = data
            .get(..4)
            .is_some_and(|word| word == SHEET_STREAM_MAGIC.to_le_bytes());
        if !opens_a_chain {
            continue;
        }
        // A symbol's cached body is the same stream in every drawing that
        // places the symbol: each distinct stream is read once.
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        if !seen_streams.insert((data.len(), hasher.finish())) {
            continue;
        }
        reading.chain_streams += 1;
        read_stream(label, path, data, keep_lines, reading);
    }
    if !keep_lines {
        return;
    }
    // The whole of what the document's records are emitted as, the cached
    // symbol bodies among it. `{:?}` writes a double as the shortest text
    // that reads back to the same bits.
    let geometry = build_normalized_geometry(&package.parsed);
    let mut whole = Sum::new();
    whole.bytes(format!("{geometry:?}").as_bytes());
    let (curves, sampled) = sampled_bsplines(
        geometry
            .symbol_definitions
            .iter()
            .flat_map(|definition| definition.primitives.iter()),
    );
    reading.lines.push(format!(
        "geometry\t{label}\t{} entities, {} cached bodies\t{}\tthe bodies' {curves} B-splines sampled {}",
        geometry.entities.len(),
        geometry.symbol_definitions.len(),
        whole.hex(),
        sampled.hex()
    ));
    if document_kind(label) == "symbol" {
        let mut line = format!("symbol\t{label}\t");
        match read_symbol_geometry_from_bytes(bytes) {
            Ok(mut body) => {
                // The body lists the layers of every `PSMcluster0` the file
                // holds, storage after storage. Until #50 that was the
                // order a hash map yielded the storages, and a symbol with
                // more than one such stream could list the same layers the
                // other way round on another run. Sorted here, so that a
                // line says what the body holds and nothing about the run,
                // or about which side of that change the build is on.
                body.sheet_layers.sort_by(|a, b| {
                    (a.oid, a.layer_number, &a.name).cmp(&(b.oid, b.layer_number, &b.name))
                });
                let mut whole = Sum::new();
                whole.bytes(format!("{body:?}").as_bytes());
                let (curves, sampled) =
                    sampled_bsplines(body.primitives.iter().map(|styled| &styled.primitive));
                let _ = write!(
                    line,
                    "{} primitives\t{}\tits {curves} B-splines sampled {}",
                    body.primitives.len(),
                    whole.hex(),
                    sampled.hex()
                );
            }
            Err(error) => {
                let _ = write!(line, "unreadable: {error}");
            }
        }
        reading.lines.push(line);
    }
}

/// Read every document in reach.
fn read_corpora(keep_lines: bool) -> Reading {
    let roots = [
        "test-file",
        r"D:\work\cad\pid-test-data",
        r"D:\work\plant-code\cad\pid-test-data",
        r"D:\work\cad\PID自研软件验证",
    ];
    let parser = PidParser::with_options(ParseOptions::geometry());
    let mut seen_files = HashSet::new();
    let mut seen_streams = HashSet::new();
    let mut reading = Reading::default();
    let mut hold = |label: &str, bytes: &[u8], reading: &mut Reading| {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        if seen_files.insert((bytes.len(), hasher.finish())) {
            read_document(
                label,
                bytes,
                &parser,
                &mut seen_streams,
                keep_lines,
                reading,
            );
        }
    };
    for root in roots {
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents(Path::new(root), &mut files, &mut archives);
        for file in &files {
            if let Ok(bytes) = std::fs::read(file) {
                hold(&file.display().to_string(), &bytes, &mut reading);
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
                        hold(&member.label().display().to_string(), &bytes, &mut reading);
                    }
                }
            }
        }
    }
    reading
}

/// `value → how many`, most first.
fn tally<T: Ord + Clone>(values: impl Iterator<Item = T>) -> Vec<(T, usize)> {
    let mut counts: BTreeMap<T, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let mut out: Vec<(T, usize)> = counts.into_iter().collect();
    out.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    out
}

fn census(reading: &Reading) {
    let curves = &reading.curves;
    println!(
        "documents {}; chain streams {}; igBspCurve2d records at a chain start {} ({} live, {} superseded); refused {}; unreadable documents {}",
        reading.documents,
        reading.chain_streams,
        curves.len() + reading.refused.len(),
        curves.iter().filter(|curve| curve.live).count(),
        curves.iter().filter(|curve| !curve.live).count(),
        reading.refused.len(),
        reading.unreadable.len()
    );

    println!("\n== where they are");
    println!(
        "{:<9} {:<6} {:>7} {:>10} {:>14} {:>11} {:>11} {:>9}",
        "document",
        "stream",
        "records",
        "superseded",
        "highest degree",
        "most poles",
        "most knots",
        "rational"
    );
    let mut places: BTreeMap<(&str, &str), Vec<&Curve>> = BTreeMap::new();
    for curve in curves {
        places
            .entry((curve.document, curve.stream))
            .or_default()
            .push(curve);
    }
    for ((document, stream), held) in &places {
        println!(
            "{document:<9} {stream:<6} {:>7} {:>10} {:>14} {:>11} {:>11} {:>9}",
            held.len(),
            held.iter().filter(|curve| !curve.live).count(),
            held.iter().map(|curve| curve.degree).max().unwrap_or(0),
            held.iter().map(|curve| curve.poles).max().unwrap_or(0),
            held.iter().map(|curve| curve.knots).max().unwrap_or(0),
            held.iter().filter(|curve| curve.weights.is_some()).count()
        );
    }

    println!("\n== degree (knots - poles - 1)");
    for (degree, records) in tally(curves.iter().map(|curve| curve.degree)) {
        println!("  degree {degree:<3} {records} records");
    }
    println!("\n== poles");
    let mut by_poles = tally(curves.iter().map(|curve| curve.poles));
    by_poles.sort_by_key(|(poles, _)| *poles);
    println!(
        "  {}",
        by_poles
            .iter()
            .map(|(poles, records)| format!("{poles} x{records}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut by_knots = tally(curves.iter().map(|curve| curve.knots));
    by_knots.sort_by_key(|(knots, _)| *knots);
    println!("== knots");
    println!(
        "  {}",
        by_knots
            .iter()
            .map(|(knots, records)| format!("{knots} x{records}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (what, most) in [
        (
            "the highest degree",
            curves.iter().max_by_key(|curve| curve.degree),
        ),
        (
            "the most poles",
            curves.iter().max_by_key(|curve| curve.poles),
        ),
        (
            "the most knots",
            curves.iter().max_by_key(|curve| curve.knots),
        ),
    ] {
        if let Some(curve) = most {
            println!(
                "  {what}: degree {}, {} poles, {} knots: {}",
                curve.degree, curve.poles, curve.knots, curve.place
            );
        }
    }

    println!("\n== weights");
    let rational: Vec<&Curve> = curves
        .iter()
        .filter(|curve| curve.weights.is_some())
        .collect();
    println!("  rational records: {} of {}", rational.len(), curves.len());
    if !rational.is_empty() {
        let least = rational
            .iter()
            .filter_map(|curve| curve.weights)
            .fold(f64::INFINITY, |least, (low, _)| least.min(low));
        let most = rational
            .iter()
            .filter_map(|curve| curve.weights)
            .fold(f64::NEG_INFINITY, |most, (_, high)| most.max(high));
        let widest = rational
            .iter()
            .filter_map(|curve| curve.weights)
            .fold(1.0_f64, |widest, (low, high)| widest.max(high / low));
        let largest_in_one_to_two = rational
            .iter()
            .filter_map(|curve| curve.weights)
            .filter(|(_, high)| (1.0..2.0).contains(high))
            .count();
        println!(
            "  smallest weight {least:e}, largest {most:e}; widest largest/smallest in one record {widest}; records whose largest weight is in [1, 2): {largest_in_one_to_two}"
        );
        for (weights, records) in tally(
            rational
                .iter()
                .filter_map(|curve| curve.weights)
                .map(|(low, high)| format!("{low} .. {high}")),
        ) {
            println!("    {weights}: {records} records");
        }
    }

    println!("\n== the double after the knots, and the four closing bytes");
    for (trailing, records) in tally(curves.iter().map(|curve| format!("{}", curve.trailing))) {
        println!("  trailing {trailing}: {records} records");
    }
    for ((degree, flags), records) in tally(curves.iter().map(|curve| (curve.degree, curve.flags)))
    {
        println!("  degree {degree}, closing bytes {flags:02x?}: {records} records");
    }
    let order_byte = curves
        .iter()
        .filter(|curve| usize::from(curve.flags[0]) == curve.degree + 1)
        .count();
    println!(
        "  records whose first closing byte is degree + 1 (the order): {order_byte} of {}",
        curves.len()
    );

    println!("\n== knot vectors");
    let least_step = curves
        .iter()
        .map(|curve| curve.least_knot_step)
        .fold(f64::INFINITY, f64::min);
    println!(
        "  clamped at both ends: {} of {}; with no parameter domain: {}; smallest step between two distinct knots: {least_step:e}",
        curves.iter().filter(|curve| curve.clamped).count(),
        curves.len(),
        curves.iter().filter(|curve| curve.no_domain).count()
    );

    if !reading.refused.is_empty() {
        println!("\n== refused by the decoder");
        for place in &reading.refused {
            println!("  {place}");
        }
    }
    for label in &reading.unreadable {
        println!("unreadable: {label}");
    }
}

/// The lines of a recording, by the three fields that name what was read.
fn recording(path: &str) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("{path}: {error}");
        std::process::exit(2);
    });
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let key = format!("{}\t{}", fields.next()?, fields.next()?);
            Some((key, fields.next().unwrap_or_default().to_owned()))
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            let reading = read_corpora(false);
            census(&reading);
            if !reading.refused.is_empty() {
                std::process::exit(1);
            }
        }
        [mode, path] if mode == "record" => {
            let reading = read_corpora(true);
            let mut text = reading.lines.join("\n");
            text.push('\n');
            if let Err(error) = std::fs::write(path, text) {
                eprintln!("{path}: {error}");
                std::process::exit(2);
            }
            let count = |kind: &str| {
                reading
                    .lines
                    .iter()
                    .filter(|line| line.starts_with(kind))
                    .count()
            };
            println!(
                "{path}: {} lines: {} records, {} documents' geometry, {} symbol bodies; {} documents read, {} records refused",
                reading.lines.len(),
                count("record\t"),
                count("geometry\t"),
                count("symbol\t"),
                reading.documents,
                reading.refused.len()
            );
        }
        [mode, before, after] if mode == "compare" => {
            let (left, right) = (recording(before), recording(after));
            let mut differ = 0usize;
            for (key, value) in &left {
                match right.get(key) {
                    Some(other) if other == value => {}
                    Some(other) => {
                        differ += 1;
                        println!("DIFFERS  {key}\n    {before}: {value}\n    {after}: {other}");
                    }
                    None => {
                        differ += 1;
                        println!("ONLY IN {before}  {key}\t{value}");
                    }
                }
            }
            for (key, value) in &right {
                if !left.contains_key(key) {
                    differ += 1;
                    println!("ONLY IN {after}  {key}\t{value}");
                }
            }
            let kinds = |lines: &BTreeMap<String, String>, kind: &str| {
                lines.keys().filter(|key| key.starts_with(kind)).count()
            };
            println!(
                "{before}: {} lines ({} records, {} geometries, {} symbol bodies); {after}: {} lines ({} records, {} geometries, {} symbol bodies); {differ} differ",
                left.len(),
                kinds(&left, "record\t"),
                kinds(&left, "geometry\t"),
                kinds(&left, "symbol\t"),
                right.len(),
                kinds(&right, "record\t"),
                kinds(&right, "geometry\t"),
                kinds(&right, "symbol\t"),
            );
            if differ != 0 {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!(
                "usage: probe_how_high_a_degree_a_bspline_record_states [record <file> | compare <before> <after>]"
            );
            std::process::exit(2);
        }
    }
}
