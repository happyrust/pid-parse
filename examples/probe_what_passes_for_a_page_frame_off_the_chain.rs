//! What does a chain walk that has left the record boundaries take for a
//! page frame, and what tells such a "frame" from a real one?
//!
//! A length that is wrong and still inside the stream sends the walk on from
//! an offset where no record starts. It stops on the first word no record
//! opens with — but `0x003D`, the type word of an `igSmartFrame2d`, is also
//! the character `=` in a label's text, and a walk that lands on one reads
//! a "frame" whose payload is whatever follows. This probe changes one
//! length field by ±1, ±2 … ±64 on every record of every distinct chain
//! stream in reach — `test-file`, both `pid-test-data`, the share, and the
//! `.igr` / `.sym` / `.pid` members of their `RefData~*.zip` and
//! `PlantData~*.zip` — and asks, of every record such a walk reads from
//! where the chain starts none, whether it is a frame by the rules the
//! decoder had until 2026-11-11 (a payload of 156 bytes or more; a finite,
//! non-negative width and height; a finite ratio) and whether the decoder
//! takes it now.
//!
//! It also reads every real frame the chains hold, superseded copies
//! included, for what a real one looks like: its payload length, its two
//! flag words, and whether the ratio it states at payload `+148` is its
//! height at `+84` over its width at `+76`.
//!
//! Read on 2026-11-11: 885 documents, 178 frames on chains (91 of them
//! live, all decoded), payloads of 228, 234, 238 or 246 bytes, the ratio
//! equal to height over width on all 178 to 5e-16. Of 3 337 379 length
//! changes, walks read 49 058 records off the chain; ten of them were
//! frames by the old rules — payloads of 65 536, 262 144 and 327 680 bytes,
//! doubles that are denormals or 1e284 — and none is a frame now, because
//! none states the ratio of its own extent. All ten are outside the
//! repository (plant-backup sheets and the symbol caches of two acceptance
//! sheets), which is why `tests/sheet_chain_faults.rs`, which sweeps the
//! committed streams on every run, counted none.
//!
//! Exits non-zero when the decoder takes a record read off the chain for a
//! frame, refuses a live frame of a chain, or a real frame's ratio is not
//! its height over its width.
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
    decode_smartframe_at, parse_psm_header, psm_type_word_opens_a_record, SheetChain,
};
use pid_parse::{ParseOptions, PidParser};

/// The fourteen changes made to each record's length field.
const DELTAS: [i64; 14] = [1, -1, 2, -2, 4, -4, 8, -8, 16, -16, 32, -32, 64, -64];

/// PSM type code of an `igSmartFrame2d`.
const SMART_FRAME: u16 = 0x003D;

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
    /// Length changes made.
    changes: usize,
    /// Records the walks read from where the chain starts none.
    off_the_chain: usize,
    /// Of those, the ones the rules before 2026-11-11 took for a frame,
    /// each with whether the decoder takes it now.
    frames_by_the_old_rules: Vec<(String, bool)>,
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

    // Each record's length changed in turn; the walk from where the changed
    // length ends, by the rule `SheetChain::walk` walks by.
    for &at in starts {
        let length = u32_at(data, at + 2);
        for delta in DELTAS {
            let Ok(wrong) = u32::try_from(i64::from(length) + delta) else {
                continue;
            };
            reading.changes += 1;
            let mut cursor = at + 6 + wrong as usize;
            if wrong == 0 || cursor > data.len() {
                continue;
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
                    reading.off_the_chain += 1;
                    if header.type_code == SMART_FRAME && !header.native_reader_skips() {
                        let payload = header.body_start;
                        let (width, height, ratio) = (
                            f64_at(data, payload + 76),
                            f64_at(data, payload + 84),
                            f64_at(data, payload + 148),
                        );
                        let by_the_old_rules = header.bytes_to_follow >= 156
                            && [width, height, ratio].iter().all(|v| v.is_finite())
                            && width >= 0.0
                            && height >= 0.0;
                        if by_the_old_rules {
                            reading.frames_by_the_old_rules.push((
                                format!(
                                    "{label}: the record at +{at}, {length} -> {wrong}: a \
                                     \"frame\" at +{cursor} of {} bytes, width {width:e}, \
                                     height {height:e}, ratio {ratio:e}, flag words {:#x} / {:#x}",
                                    header.bytes_to_follow,
                                    u32_at(data, payload + 14),
                                    u32_at(data, payload + 26),
                                ),
                                decode_smartframe_at(data, cursor).is_some(),
                            ));
                        }
                    }
                }
                cursor = end;
            }
        }
    }
}

fn main() {
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
    for root in roots {
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
        "ratio is not height over width: {} (largest difference where it is: {:e})",
        reading.ratio_differs, reading.ratio_worst
    );
    let still = reading
        .frames_by_the_old_rules
        .iter()
        .filter(|(_, now)| *now)
        .count();
    println!(
        "length changes {}; records read off the chain {}; frames by the rules before \
         2026-11-11: {}; of them frames now: {still}",
        reading.changes,
        reading.off_the_chain,
        reading.frames_by_the_old_rules.len()
    );
    for (line, now) in &reading.frames_by_the_old_rules {
        println!(
            "  {} {line}",
            if *now {
                "STILL A FRAME"
            } else {
                "refused now  "
            }
        );
    }
    if still != 0 || reading.ratio_differs != 0 || reading.decoded != reading.live {
        std::process::exit(1);
    }
}
