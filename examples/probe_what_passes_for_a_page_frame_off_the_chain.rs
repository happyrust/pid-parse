//! What does a chain walk that has left the record boundaries take for a
//! record of a family, a page frame above all, and what tells such a "frame"
//! from a real one?
//!
//! A length that is wrong and still inside the stream sends the walk on from
//! an offset where no record starts. It stops on the first word no record
//! opens with — but `0x003D`, the type word of an `igSmartFrame2d`, is also
//! the character `=` in a label's text, and a walk that lands on one reads
//! a "frame" whose payload is whatever follows. This probe changes one
//! length field by every amount from −64 to +64 on every record of every
//! distinct chain stream in reach — `test-file`, both `pid-test-data`, the
//! share, and the `.igr` / `.sym` / `.pid` members of their `RefData~*.zip`
//! and `PlantData~*.zip` — and asks, of every record such a walk reads from
//! where the chain starts none:
//!
//! * whether any of the sixteen registry families decodes it;
//! * when its type word is a frame's, whether it is a frame by each of the
//!   three sets of rules the decoder has had: the ones before 2026-11-11 (a
//!   payload of 156 bytes or more; a finite, non-negative width and height;
//!   a finite ratio), the ratio rule of 2026-11-11 (a stated ratio within a
//!   millionth of height over width — held only where the width is above
//!   zero, and passed by a quotient that overflows, since no number is
//!   greater than a millionth of infinity), and the decoder as it stands.
//!
//! The tallies are kept twice: for all 128 changes, and for the fourteen
//! powers of two (±1, ±2 … ±64) the first run of this probe and
//! `tests/sheet_chain_faults.rs`'s sweep of every family make, so that the
//! numbers of both can be read off one run. They are also kept apart for the
//! repository's own files, which a test sweeps on every run.
//!
//! A wrong length of any size sends the walk to some offset, so the three
//! sets of rules are also tried at every offset off the chain where a walk
//! would read a live record with a frame's type word — what any such walk
//! could come on, not only the ones 64 bytes away or nearer.
//!
//! It also reads every real frame the chains hold, superseded copies
//! included, for what a real one looks like: its payload length, its two
//! flag words, its narrowest side, and whether the ratio it states at
//! payload `+148` is its height at `+84` over its width at `+76`.
//!
//! Read on 2026-11-11 with the fourteen changes: 885 documents, 178 frames
//! on chains (91 of them live, all decoded), payloads of 228, 234, 238 or
//! 246 bytes, the ratio equal to height over width on all 178 to 5e-16. Of
//! 3 337 379 length changes, walks read 49 058 records off the chain; ten
//! were frames by the rules before that day and none by the ratio rule. An
//! independent review then made all 128 changes: 29 120 173 of them,
//! 1 371 448 records off the chain, 102 frames by the old rules and six
//! still by the ratio rule — four of no width, two whose height over width
//! is infinite. Read on 2026-11-12, with the decoder holding a frame to an
//! extent its ratio can be compared with: the same 29 120 173 changes and
//! 1 371 448 records, 200 of them live with a frame's type word, 102 frames
//! by the first rules, six by the ratio rule and none by the decoder; at
//! every offset, 179 places a walk could read such a record, 83 frames by
//! the first rules, six by the ratio rule, none by the decoder. Four of the
//! records read off the chain decoded as another family, `igComplexString2d`,
//! all on one sheet of the plant backup: a type word `0x0021`, a length of
//! 65 536 and a count of 256, 28 bytes into a `DependencyObject`. Read again
//! the same day, with that decoder holding a record to closing as a string
//! closes (four bytes after its last member entry on a sheet, four and a
//! `u32` a member in a symbol cache): the same changes and records, and no
//! family decodes one.
//!
//! Exits non-zero when the decoder takes a record read off the chain for a
//! frame, any family decodes a record read off the chain, a live frame of a
//! chain is refused, or a real frame's ratio is not its height over its
//! width.
//!
//! ```text
//! cargo run --release --example probe_what_passes_for_a_page_frame_off_the_chain
//! ```
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_dependency_object_at, decode_igarc_at, decode_igboundary_at, decode_igbspcurve_at,
    decode_igcircle_at, decode_igcomplexstring_at, decode_igdimension_at,
    decode_igellipticalarc_at, decode_iggroup_at, decode_igline_at, decode_iglinestring_at,
    decode_igpoint_at, decode_igrectangle_at, decode_igsymbol_at, decode_igtextbox_at,
    decode_smartframe_at, parse_psm_header, psm_type_word_opens_a_record, SheetChain,
};
use pid_parse::{ParseOptions, PidParser, SHEET_RECORD_FAMILIES};

/// PSM type code of an `igSmartFrame2d`.
const SMART_FRAME: u16 = 0x003D;

/// One registry family's decoder at an offset: its registry name, and
/// whether it takes the bytes there for a record of its own.
type FamilyAt = (&'static str, fn(&[u8], usize) -> bool);

/// The sixteen registry families, each through the entry point that reads
/// at an offset; `main` holds the names against the registry.
const FAMILIES: [FamilyAt; 16] = [
    ("igLine2d", |data, at| decode_igline_at(data, at).is_some()),
    ("igLineString2d", |data, at| {
        decode_iglinestring_at(data, at).is_some()
    }),
    ("igPoint2d", |data, at| {
        decode_igpoint_at(data, at).is_some()
    }),
    ("igTextBox", |data, at| {
        decode_igtextbox_at(data, at).is_some()
    }),
    ("igSymbol2d", |data, at| {
        decode_igsymbol_at(data, at).is_some()
    }),
    ("igArc2d", |data, at| decode_igarc_at(data, at).is_some()),
    ("igCircle2d", |data, at| {
        decode_igcircle_at(data, at).is_some()
    }),
    ("igBoundary2d", |data, at| {
        decode_igboundary_at(data, at).is_some()
    }),
    ("igSmartFrame2d", |data, at| {
        decode_smartframe_at(data, at).is_some()
    }),
    ("DependencyObject", |data, at| {
        decode_dependency_object_at(data, at).is_some()
    }),
    ("igRectangle2d", |data, at| {
        decode_igrectangle_at(data, at).is_some()
    }),
    ("igBspCurve2d", |data, at| {
        decode_igbspcurve_at(data, at).is_some()
    }),
    ("igDimension", |data, at| {
        decode_igdimension_at(data, at).is_some()
    }),
    ("igComplexString2d", |data, at| {
        decode_igcomplexstring_at(data, at).is_some()
    }),
    ("igGroup", |data, at| decode_iggroup_at(data, at).is_some()),
    ("igEllipticalArc2d", |data, at| {
        decode_igellipticalarc_at(data, at).is_some()
    }),
];

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

fn f64_at(data: &[u8], at: usize) -> f64 {
    data.get(at..at + 8).map_or(f64::NAN, |bytes| {
        f64::from_le_bytes(bytes.try_into().expect("eight bytes"))
    })
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    data.get(at..at + 4).map_or(0, |bytes| {
        u32::from_le_bytes(bytes.try_into().expect("four bytes"))
    })
}

/// A frame by the rules the decoder had before 2026-11-11.
fn frame_by_the_first_rules(payload_len: u32, width: f64, height: f64, ratio: f64) -> bool {
    payload_len >= 156
        && [width, height, ratio].iter().all(|v| v.is_finite())
        && width >= 0.0
        && height >= 0.0
}

/// A frame by the ratio rule as it was written on 2026-11-11: held only
/// where the width is above zero, and passed by an infinite quotient.
fn frame_by_the_ratio_rule(payload_len: u32, width: f64, height: f64, ratio: f64) -> bool {
    if !frame_by_the_first_rules(payload_len, width, height, ratio) {
        return false;
    }
    if width > 0.0 {
        let quotient = height / width;
        if (ratio - quotient).abs() > 1e-6 * quotient.abs() {
            return false;
        }
    }
    true
}

/// What the length changes of a set of streams read.
#[derive(Default, Clone)]
struct Sweep {
    /// Length changes made.
    changes: usize,
    /// Records the walks read from where the chain starts none.
    off_the_chain: usize,
    /// Of those, the ones each registry family decodes.
    decoded: BTreeMap<&'static str, usize>,
    /// Of those, the live ones whose type word is a frame's.
    frame_words: usize,
    /// Of those, frames by the rules before 2026-11-11.
    by_the_first_rules: usize,
    /// Of those, frames by the ratio rule of 2026-11-11.
    by_the_ratio_rule: usize,
    /// Of those, frames by the decoder as it stands.
    by_the_decoder: usize,
}

impl Sweep {
    fn add(&mut self, other: &Self) {
        self.changes += other.changes;
        self.off_the_chain += other.off_the_chain;
        for (family, count) in &other.decoded {
            *self.decoded.entry(family).or_default() += count;
        }
        self.frame_words += other.frame_words;
        self.by_the_first_rules += other.by_the_first_rules;
        self.by_the_ratio_rule += other.by_the_ratio_rule;
        self.by_the_decoder += other.by_the_decoder;
    }

    fn say(&self, label: &str) {
        let decoded = if self.decoded.is_empty() {
            "none".to_string()
        } else {
            self.decoded
                .iter()
                .map(|(family, count)| format!("{family} x{count}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "{label}: length changes {}; records read off the chain {}; decoded by a registry \
             family: {decoded}; with a frame's type word {}, frames by the rules before \
             2026-11-11 {}, by the ratio rule of that day {}, by the decoder now {}",
            self.changes,
            self.off_the_chain,
            self.frame_words,
            self.by_the_first_rules,
            self.by_the_ratio_rule,
            self.by_the_decoder
        );
    }
}

/// What stands at the offsets of a set of streams where a walk could read
/// a live record with a frame's type word and the chain starts none.
#[derive(Default, Clone, Copy)]
struct Anywhere {
    /// Such offsets.
    frame_words: usize,
    /// Of those, frames by the rules before 2026-11-11.
    by_the_first_rules: usize,
    /// Of those, frames by the ratio rule of 2026-11-11.
    by_the_ratio_rule: usize,
    /// Of those, frames by the decoder as it stands.
    by_the_decoder: usize,
}

impl Anywhere {
    fn say(&self, label: &str) {
        println!(
            "{label}: places off a chain where a walk could read a live record with a frame's \
             type word {}; frames by the rules before 2026-11-11 {}, by the ratio rule of that \
             day {}, by the decoder now {}",
            self.frame_words, self.by_the_first_rules, self.by_the_ratio_rule, self.by_the_decoder
        );
    }
}

/// What the walk over the corpora read.
#[derive(Default)]
struct Reading {
    /// Frames on chains, superseded copies included.
    frames: usize,
    /// Of those, the ones the native reader reads.
    live: usize,
    /// Of the live ones, the ones the decoder takes.
    decoded: usize,
    /// Payload length -> frames.
    lengths: BTreeMap<usize, usize>,
    /// (content flag word, link flag word) -> frames.
    flag_words: BTreeMap<(u32, u32), usize>,
    /// Frames whose stated ratio is not their height over their width.
    ratio_differs: usize,
    /// The largest difference among the frames whose ratio agrees.
    ratio_worst: f64,
    /// The narrowest width and the lowest height a frame states, metres.
    narrowest: [f64; 2],
    /// Every change from −64 to +64.
    all: Sweep,
    /// The fourteen powers of two among them.
    fourteen: Sweep,
    /// Every offset a walk could read a frame's type word at.
    anywhere: Anywhere,
    /// What a family decoded off the chain, and each record a frame by the
    /// ratio rule of 2026-11-11, with whether the decoder takes it now.
    lines: Vec<String>,
}

fn read_stream(label: &str, data: &[u8], reading: &mut Reading) {
    let chain = SheetChain::walk(data);
    if !chain.is_sound() || chain.starts.is_empty() {
        return;
    }
    let starts = &chain.starts;
    for &at in starts {
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        if header.type_code != SMART_FRAME {
            continue;
        }
        reading.frames += 1;
        *reading
            .lengths
            .entry(header.bytes_to_follow as usize)
            .or_default() += 1;
        let payload = header.body_start;
        let (width, height, ratio) = (
            f64_at(data, payload + 76),
            f64_at(data, payload + 84),
            f64_at(data, payload + 148),
        );
        *reading
            .flag_words
            .entry((u32_at(data, payload + 14), u32_at(data, payload + 26)))
            .or_default() += 1;
        reading.narrowest = [
            reading.narrowest[0].min(width),
            reading.narrowest[1].min(height),
        ];
        if !header.native_reader_skips() {
            reading.live += 1;
            if decode_smartframe_at(data, at).is_some() {
                reading.decoded += 1;
            } else {
                println!("a live frame the decoder refuses: {label} +{at}");
            }
        }
        let differs = (ratio - height / width).abs();
        if width > 0.0 && differs < 1e-9 {
            reading.ratio_worst = reading.ratio_worst.max(differs);
        } else {
            reading.ratio_differs += 1;
            println!(
                "a frame whose ratio is not its height over its width: {label} +{at}: \
                 {width} by {height}, ratio {ratio}"
            );
        }
    }

    // Every offset off the chain where the bytes spell a live record with a
    // frame's type word: a type word the reader has a class for, a length
    // above zero, a payload the stream holds.
    for at in 8..data.len().saturating_sub(1) {
        if data[at..at + 2] != SMART_FRAME.to_le_bytes() || starts.binary_search(&at).is_ok() {
            continue;
        }
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        if !psm_type_word_opens_a_record(header.type_word())
            || header.bytes_to_follow == 0
            || header.body_start + header.bytes_to_follow as usize > data.len()
        {
            continue;
        }
        reading.anywhere.frame_words += 1;
        let payload = header.body_start;
        let (width, height, ratio) = (
            f64_at(data, payload + 76),
            f64_at(data, payload + 84),
            f64_at(data, payload + 148),
        );
        let len = header.bytes_to_follow;
        if frame_by_the_first_rules(len, width, height, ratio) {
            reading.anywhere.by_the_first_rules += 1;
        }
        if frame_by_the_ratio_rule(len, width, height, ratio) {
            reading.anywhere.by_the_ratio_rule += 1;
        }
        if decode_smartframe_at(data, at).is_some() {
            reading.anywhere.by_the_decoder += 1;
            reading.lines.push(format!(
                "  STILL A FRAME wherever a walk comes on it: {label}: the bytes at +{at}, {len} \
                 of payload, width {width:e}, height {height:e}, ratio {ratio:e}"
            ));
        }
    }

    // Each record's length changed in turn; the walk from where the changed
    // length ends, by the rule `SheetChain::walk` walks by. The bytes from
    // there on are the stream's own, so what a decoder makes of a record
    // the walk reads is read off the unchanged stream.
    for &at in starts {
        let length = u32_at(data, at + 2);
        for delta in -64i64..=64 {
            if delta == 0 {
                continue;
            }
            let Ok(wrong) = u32::try_from(i64::from(length) + delta) else {
                continue;
            };
            let power_of_two = delta.unsigned_abs().is_power_of_two();
            let mut sweep = Sweep {
                changes: 1,
                ..Sweep::default()
            };
            let mut cursor = at + 6 + wrong as usize;
            if wrong == 0 {
                cursor = data.len();
            }
            while cursor < data.len() {
                let Some(header) = parse_psm_header(data, cursor) else {
                    break;
                };
                if !psm_type_word_opens_a_record(header.type_word()) || header.bytes_to_follow == 0
                {
                    break;
                }
                let end = header.body_start + header.bytes_to_follow as usize;
                if end > data.len() {
                    break;
                }
                if starts.binary_search(&cursor).is_err() {
                    sweep.off_the_chain += 1;
                    for (family, decodes) in FAMILIES {
                        if decodes(data, cursor) {
                            *sweep.decoded.entry(family).or_default() += 1;
                            // The tallies count every one; the lines are a
                            // sample of them.
                            if reading.lines.len() < 400 {
                                reading.lines.push(format!(
                                    "  decoded off the chain by {family}: {label}: the record \
                                     at +{at}, {length} -> {wrong} ({delta:+}): the bytes at \
                                     +{cursor}, {} of payload",
                                    header.bytes_to_follow
                                ));
                            }
                        }
                    }
                    if header.type_code == SMART_FRAME && !header.native_reader_skips() {
                        sweep.frame_words += 1;
                        let payload = header.body_start;
                        let (width, height, ratio) = (
                            f64_at(data, payload + 76),
                            f64_at(data, payload + 84),
                            f64_at(data, payload + 148),
                        );
                        let len = header.bytes_to_follow;
                        if frame_by_the_first_rules(len, width, height, ratio) {
                            sweep.by_the_first_rules += 1;
                        }
                        if frame_by_the_ratio_rule(len, width, height, ratio) {
                            sweep.by_the_ratio_rule += 1;
                            let now = decode_smartframe_at(data, cursor).is_some();
                            reading.lines.push(format!(
                                "  a frame by the ratio rule of 2026-11-11, {}: {label}: the \
                                 record at +{at}, {length} -> {wrong} ({delta:+}): the bytes at \
                                 +{cursor}, {len} of payload, width {width:e}, height \
                                 {height:e}, ratio {ratio:e}",
                                if now { "STILL A FRAME" } else { "refused now" }
                            ));
                        }
                        if decode_smartframe_at(data, cursor).is_some() {
                            sweep.by_the_decoder += 1;
                        }
                    }
                }
                cursor = end;
            }
            reading.all.add(&sweep);
            if power_of_two {
                reading.fourteen.add(&sweep);
            }
        }
    }
}

fn main() {
    // The names above are the registry's, all of them.
    let mut probed: Vec<&str> = FAMILIES.iter().map(|(name, _)| *name).collect();
    probed.sort_unstable();
    let mut registered: Vec<&str> = SHEET_RECORD_FAMILIES.iter().map(|f| f.name).collect();
    registered.sort_unstable();
    assert_eq!(probed, registered, "the probe asks every registry family");

    let roots = [
        "test-file",
        r"D:\work\cad\pid-test-data",
        r"D:\work\plant-code\cad\pid-test-data",
        r"D:\work\cad\PID自研软件验证",
    ];
    let parser = PidParser::with_options(ParseOptions::geometry());
    let mut seen_files = HashSet::new();
    let mut seen_streams = HashSet::new();
    let mut reading = Reading {
        narrowest: [f64::INFINITY; 2],
        ..Reading::default()
    };
    let mut documents_read = 0usize;
    let mut hold = |label: &str, bytes: Vec<u8>, reading: &mut Reading| {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        if !seen_files.insert((bytes.len(), hasher.finish())) {
            return;
        }
        let Ok(package) = parser.parse_package_from_reader(Cursor::new(bytes)) else {
            return;
        };
        documents_read += 1;
        let mut paths: Vec<&String> = package.streams.keys().collect();
        paths.sort();
        for path in paths {
            let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
            let leaf = parts.last().copied().unwrap_or_default();
            let is_chain = leaf.starts_with("Sheet")
                || (parts.len() == 2 && parts[0].starts_with("JSite") && leaf == "PSMcluster0");
            if !is_chain {
                continue;
            }
            let data = &package.streams[path].data;
            let mut hasher = DefaultHasher::new();
            data.hash(&mut hasher);
            if !seen_streams.insert((data.len(), hasher.finish())) {
                continue;
            }
            read_stream(&format!("{label} {path}"), data, reading);
        }
    };
    for (index, root) in roots.into_iter().enumerate() {
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents(Path::new(root), &mut files, &mut archives);
        for file in &files {
            if let Ok(bytes) = std::fs::read(file) {
                hold(&file.display().to_string(), bytes, &mut reading);
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
                        hold(&member.label().display().to_string(), bytes, &mut reading);
                    }
                }
            }
        }
        if index == 0 {
            // The repository's own files are read first, so what stands
            // here after them is theirs alone.
            reading
                .all
                .say("the repository's files, every change of -64..=64");
            reading
                .fourteen
                .say("the repository's files, the fourteen powers of two");
            reading.anywhere.say("the repository's files, every offset");
        }
    }

    println!(
        "documents {documents_read}; frames on chains {} ({} live, {} of them decoded)",
        reading.frames, reading.live, reading.decoded
    );
    println!("payload lengths: {:?}", reading.lengths);
    println!(
        "flag words (content / link): {}",
        reading
            .flag_words
            .iter()
            .map(|((content, link), frames)| format!("{content:#x} / {link:#x} x{frames}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "ratio is not height over width: {} (largest difference where it is: {:e}); the \
         narrowest frame is {:e} m wide, the lowest {:e} m high",
        reading.ratio_differs, reading.ratio_worst, reading.narrowest[0], reading.narrowest[1]
    );
    reading.all.say("every corpus, every change of -64..=64");
    reading
        .fourteen
        .say("every corpus, the fourteen powers of two");
    reading.anywhere.say("every corpus, every offset");
    for line in &reading.lines {
        println!("{line}");
    }
    if reading.all.by_the_decoder != 0
        || reading.anywhere.by_the_decoder != 0
        || !reading.all.decoded.is_empty()
        || reading.ratio_differs != 0
        || reading.decoded != reading.live
    {
        std::process::exit(1);
    }
}
