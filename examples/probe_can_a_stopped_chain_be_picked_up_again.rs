//! Whether a record chain that stops can be picked up again further on.
//!
//! A `Sheet*` stream is an eight-byte header and then records end to end,
//! each stating its own length. One length field off by a byte and the walk
//! from record to record stops there: the records before the stop are real,
//! and everything after it is bytes with no framing left to say where a
//! record begins. The tempting repair is to search forward from the stop for
//! the first offset from which a chain walks cleanly to the stream's end,
//! and carry on from it.
//!
//! This probe measures what that search would find if each record of each
//! intact sheet stream in turn were the damaged one. For a record that has
//! another after it, the search starts past the record's own envelope — its
//! length is the thing that cannot be trusted — and the right answer is the
//! start of the next record. Two rules are tried:
//!
//! 1. **The first offset from which a chain walks exactly to the stream's
//!    end.** Counted: the records that hold such an offset *inside
//!    themselves*, ahead of the next record's start, which is where the
//!    search would land.
//! 2. **The same, held to the header as well**: the records read before the
//!    stop, the damaged one and the chain picked up have to add up to the
//!    live-record count the stream's header states. Counted: the records on
//!    which the first offset that satisfies both is still not the next
//!    record's start.
//!
//! A record a search lands inside is a record whose payload would be read as
//! records of its own: geometry no file holds, drawn. On the corpora the
//! second rule still does that on several hundred records, the first of them
//! the page frame that opens `D06.pid`'s `/Sheet6`. So the parser reads a
//! stopped chain as far as it holds and reports the rest unread
//! (`SheetChain::walk`, `StreamChainFault`); it does not look for a place to
//! start again. See
//! `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//! §15.
//!
//! ```text
//! cargo run -q --example probe_can_a_stopped_chain_be_picked_up_again
//! cargo run -q --example probe_can_a_stopped_chain_be_picked_up_again -- <dir or file> ...
//! ```
//!
//! With no argument it reads the committed `test-file` tree (drawings,
//! templates and `.sym` bodies, on disk and inside the `RefData~*.zip`
//! archives; a machine-local `symbols-full` is left out), the shelled-out
//! plant backup and the acceptance corpus where this machine has them. Each
//! distinct sheet stream is measured once. The streams are read straight out
//! of the compound file, not through the parser, so the reading does not
//! rest on the code it is evidence for.
//!
//! Exits non-zero if a sheet stream in reach does not open with the chain
//! magic, does not walk to its last byte, or holds another number of live
//! records than its header states: the measurement is of intact streams, and
//! each of those is also what the parser reports as a fault.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

/// The word every chain stream opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
/// The stream header: the magic and the live-record count.
const HEADER_LEN: usize = 8;
/// A record's envelope: `u16` type word, `u32` bytes to follow.
const ENVELOPE_LEN: usize = 6;
/// The type-word bit of a record the native reader skips.
const NATIVE_SKIP: u8 = 0x80;

/// Every drawing, template and symbol under `dir`, and every `RefData~*.zip`
/// that may hold more of them.
fn collect(dir: &Path, files: &mut Vec<PathBuf>, archives: &mut Vec<PathBuf>) {
    let known = |path: &Path, extensions: &[&str]| {
        path.extension().is_some_and(|extension| {
            extensions
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
    };
    if dir.is_file() {
        files.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library, and this crate's own plots.
            if path
                .file_name()
                .is_some_and(|name| name == "symbols-full" || name == "plot")
            {
                continue;
            }
            collect(&path, files, archives);
        } else if known(&path, &["pid", "igr", "sym"]) {
            files.push(path);
        } else if known(&path, &["zip"])
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("RefData~"))
        {
            archives.push(path);
        }
    }
}

fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("test-file")];
    let data = std::env::var_os("PID_TEST_DATA_DIR").map_or_else(
        || {
            [
                r"D:\work\cad\pid-test-data",
                r"D:\work\plant-code\cad\pid-test-data",
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        },
        |dir| vec![PathBuf::from(dir)],
    );
    if let Some(backup) = data
        .iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|dir| dir.is_dir())
    {
        roots.push(backup);
    }
    let acceptance = std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    );
    if acceptance.is_dir() {
        roots.push(acceptance);
    }
    roots
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The `Sheet*` streams of one compound file, at any depth, with their paths.
fn sheet_streams(bytes: Vec<u8>) -> Option<Vec<(String, Vec<u8>)>> {
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).ok()?;
    let paths: Vec<PathBuf> = file
        .walk()
        .filter(|entry| entry.is_stream() && entry.name().starts_with("Sheet"))
        .map(|entry| entry.path().to_path_buf())
        .collect();
    let mut out = Vec::new();
    for path in paths {
        let mut data = Vec::new();
        if file
            .open_stream(&path)
            .and_then(|mut stream| stream.read_to_end(&mut data))
            .is_ok()
        {
            out.push((path.to_string_lossy().replace('\\', "/"), data));
        }
    }
    Some(out)
}

/// One stream walked as a chain from its header: the record starts, the
/// records without the native-skip bit, and where the walk stopped short of
/// the stream's end, if it did.
fn walk(data: &[u8]) -> (Vec<usize>, u32, Option<usize>) {
    let mut starts = Vec::new();
    let mut live = 0u32;
    let mut at = HEADER_LEN;
    while at < data.len() {
        let Some(length) = u32_at(data, at + 2) else {
            return (starts, live, Some(at));
        };
        let end = at + ENVELOPE_LEN + length as usize;
        if length == 0 || end > data.len() {
            return (starts, live, Some(at));
        }
        starts.push(at);
        if data[at + 1] & NATIVE_SKIP == 0 {
            live += 1;
        }
        at = end;
    }
    (starts, live, None)
}

#[derive(Default)]
struct Reading {
    /// Compound files read.
    documents: usize,
    /// Sheet streams met, repeats included.
    streams: usize,
    /// Distinct sheet streams with two records or more: the ones measured.
    measured: usize,
    /// Streams whose own framing does not hold.
    unsound: Vec<String>,
    /// Records that have another record after them.
    records: usize,
    /// Of those, the ones holding an offset inside themselves from which a
    /// chain walks exactly to the stream's end.
    lands_inside: usize,
    /// Of those, the ones on which the first offset that walks to the end
    /// *and* adds up to the header's live-record count is not the next
    /// record's start.
    lands_inside_with_the_count: usize,
    /// Records for which no offset satisfies both.
    nowhere: usize,
    examples: Vec<String>,
}

impl Reading {
    fn stream(&mut self, seen: &mut HashSet<(usize, u64)>, label: &str, path: &str, data: &[u8]) {
        self.streams += 1;
        if data.is_empty() {
            return;
        }
        let stated = u32_at(data, 4);
        let (starts, live, stop) = walk(data);
        if u32_at(data, 0) != Some(CHAIN_MAGIC) {
            self.unsound.push(format!(
                "{label} {path}: does not open with the chain magic"
            ));
            return;
        }
        if let Some(stop) = stop {
            self.unsound.push(format!(
                "{label} {path}: the chain stops at +{stop} of {} bytes",
                data.len()
            ));
            return;
        }
        if stated != Some(live) {
            self.unsound.push(format!(
                "{label} {path}: the header states {stated:?} live records and the chain holds {live}"
            ));
            return;
        }
        if starts.len() < 2 {
            return;
        }
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        if !seen.insert((data.len(), hasher.finish())) {
            return;
        }
        self.measured += 1;
        let stated = live;
        let n = data.len();

        // `walks[p]`: a chain started at `p` walks exactly to the stream's
        // end. `lives[p]`: the live records on that chain.
        let mut walks = vec![false; n + 1];
        let mut lives = vec![0u32; n + 1];
        walks[n] = true;
        for p in (HEADER_LEN..=n - ENVELOPE_LEN).rev() {
            let Some(length) = u32_at(data, p + 2).filter(|length| *length != 0) else {
                continue;
            };
            let q = p + ENVELOPE_LEN + length as usize;
            if q <= n && walks[q] {
                walks[p] = true;
                lives[p] = lives[q] + u32::from(data[p + 1] & NATIVE_SKIP == 0);
            }
        }

        let mut live_before = 0u32;
        for (k, &start) in starts.iter().enumerate() {
            let this = u32::from(data[start + 1] & NATIVE_SKIP == 0);
            if let Some(&next) = starts.get(k + 1) {
                self.records += 1;
                let search_from = start + ENVELOPE_LEN;
                if (search_from..next).any(|p| walks[p]) {
                    self.lands_inside += 1;
                }
                let fit =
                    (search_from..n).find(|&p| walks[p] && live_before + this + lives[p] == stated);
                match fit {
                    None => self.nowhere += 1,
                    Some(p) if p != next => {
                        self.lands_inside_with_the_count += 1;
                        if self.examples.len() < 8 {
                            let word = u16::from_le_bytes([data[start], data[start + 1]]);
                            self.examples.push(format!(
                                "{label} {path}: record at +{start} (type word 0x{word:04X}) — \
                                 the search lands on +{p}, the next record starts at +{next}"
                            ));
                        }
                    }
                    Some(_) => {}
                }
            }
            live_before += this;
        }
    }

    fn document(&mut self, seen: &mut HashSet<(usize, u64)>, label: &str, bytes: Vec<u8>) {
        let Some(streams) = sheet_streams(bytes) else {
            return;
        };
        self.documents += 1;
        for (path, data) in streams {
            self.stream(seen, label, &path, &data);
        }
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let roots = if args.is_empty() {
        default_roots()
    } else {
        args
    };
    let mut files = Vec::new();
    let mut archives = Vec::new();
    for root in &roots {
        println!("reading {}", root.display());
        collect(root, &mut files, &mut archives);
    }

    let mut reading = Reading::default();
    let mut seen = HashSet::new();
    for file in &files {
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        let label = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        reading.document(&mut seen, &label, bytes);
    }
    #[cfg(feature = "refdata-archive")]
    for archive in &archives {
        use pid_parse::refdata_archive::{archive_members, read_archive_file};
        for extension in ["igr", "sym", "pid"] {
            let Ok(members) = archive_members(archive, extension) else {
                continue;
            };
            for member in members {
                let Ok(bytes) = read_archive_file(&member) else {
                    continue;
                };
                let label = member.label().display().to_string();
                reading.document(&mut seen, &label, bytes);
            }
        }
    }
    #[cfg(not(feature = "refdata-archive"))]
    println!(
        "{} RefData archive(s) not read: built without `refdata-archive`",
        archives.len()
    );

    println!(
        "{} compound file(s), {} sheet stream(s), {} distinct with two records or more",
        reading.documents, reading.streams, reading.measured
    );
    println!(
        "{} record(s) of those streams have another record after them; if each in turn were \
         the one a walk stops on:",
        reading.records
    );
    println!(
        "   an offset inside the record, past its envelope, from which a chain walks exactly \
         to the stream's end: {} record(s)",
        reading.lands_inside
    );
    println!(
        "   held to the header's live-record count as well, the first such offset is not the \
         next record's start: {} record(s); no such offset at all: {}",
        reading.lands_inside_with_the_count, reading.nowhere
    );
    for example in &reading.examples {
        println!("      {example}");
    }
    if !reading.unsound.is_empty() {
        println!(
            "{} sheet stream(s) whose own framing does not hold:",
            reading.unsound.len()
        );
        for line in &reading.unsound {
            println!("   {line}");
        }
        std::process::exit(1);
    }
    println!("every sheet stream read opens with the magic, walks to its last byte and holds the live-record count its header states");
}
