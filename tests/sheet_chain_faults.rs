//! A sheet stream whose record chain does not hold is read as far as it
//! holds, and says where it stopped.
//!
//! A `Sheet*` stream is an eight-byte header and then records end to end,
//! each one's length stated by itself. Every record family decodes at the
//! offsets that walk reaches and nowhere else, so a stream whose walk stalls
//! — one length field off by a byte — used to yield nothing at all: the
//! drawing's own ink gone from the page, the border template alone left on
//! it, and not a word said. An independent review of this branch measured
//! exactly that on three damaged copies of `DWG-0201GP06-01.pid`, which is
//! what the first three [`Damage`]s here are.
//!
//! What is held now, on each damaged copy:
//!
//! * the parse result names the stream and the offset the walk stopped at,
//!   as a [`pid_parse::StreamChainFault`] on the document and as a warning of
//!   the normalized geometry;
//! * every record before the damaged one decodes exactly as it does on the
//!   intact file, family by family, and reaches the page model's entities;
//! * nothing after the stop is read. The walk is not picked up again further
//!   on: `examples/probe_can_a_stopped_chain_be_picked_up_again` measures how
//!   often the first place a chain can be picked up is inside a record
//!   (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
//!   §15).
//!
//! The header is held to the chain too — the magic, and the live-record
//! count, which is the only thing that says records are missing from a
//! stream cut off at a record boundary — and the last test holds every
//! stream of every corpus in reach to raising no fault at all.
//!
//! What `pid_plot` says about such a drawing, and what the page's ledger
//! files, is held by `pid-plot/tests/cli_pid_plot.rs` on the same three
//! damaged copies.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_smartframes, parse_psm_header, sheet_record_starts, SheetChain, SHEET_STREAM_MAGIC,
};
use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};
use pid_parse::{
    build_normalized_geometry, ParseOptions, PidDocument, PidPackage, PidParser, StreamChainFault,
    StreamChainStopReason, SHEET_RECORD_FAMILIES,
};

const FIXTURE: &str = "test-file/DWG-0201GP06-01.pid";
const SHEET: &str = "/Sheet6";

/// One way of breaking the fixture's `/Sheet6`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Damage {
    /// The record in the middle of the chain states a length of zero.
    MiddleLengthZero,
    /// The last record states one byte more than the stream has: a file
    /// that lost its last byte.
    LastLengthPlusOne,
    /// The last record states four bytes fewer than it has: four stray
    /// bytes at the stream's end.
    LastLengthMinusFour,
    /// The stream ends where its last record began: cut off at a record
    /// boundary, so the chain walks cleanly to the new end.
    CutAtTheLastRecord,
    /// The header's record count is one too many.
    HeaderCountPlusOne,
    /// The stream's first word is not the chain magic.
    WrongMagic,
}

const ALL: [Damage; 6] = [
    Damage::MiddleLengthZero,
    Damage::LastLengthPlusOne,
    Damage::LastLengthMinusFour,
    Damage::CutAtTheLastRecord,
    Damage::HeaderCountPlusOne,
    Damage::WrongMagic,
];

/// What a damaged copy has to read as.
struct Expected {
    /// Where the walk stops, when it stops before the stream's end.
    stop: Option<usize>,
    /// Bytes from the stop to the stream's end.
    unread: usize,
    /// Records the walk reads.
    records_read: usize,
    /// Offset of the first record the damage touches: every record that
    /// ends at or before it is untouched and has to decode as on the intact
    /// file. The stream's length when the damage touches no record.
    first_touched: usize,
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn put_u32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Apply `damage` to the bytes of an intact sheet stream.
fn damage_stream(data: &mut Vec<u8>, damage: Damage) -> Expected {
    let starts = sheet_record_starts(data);
    assert!(
        starts.len() > 10,
        "the fixture's {SHEET} walks: {} records",
        starts.len()
    );
    let last = starts[starts.len() - 1];
    let len = data.len();
    match damage {
        Damage::MiddleLengthZero => {
            let middle = starts[starts.len() / 2];
            put_u32(data, middle + 2, 0);
            Expected {
                stop: Some(middle),
                unread: len - middle,
                records_read: starts.len() / 2,
                first_touched: middle,
            }
        }
        Damage::LastLengthPlusOne => {
            let stated = u32_at(data, last + 2);
            put_u32(data, last + 2, stated + 1);
            Expected {
                stop: Some(last),
                unread: len - last,
                records_read: starts.len() - 1,
                first_touched: last,
            }
        }
        Damage::LastLengthMinusFour => {
            let stated = u32_at(data, last + 2);
            put_u32(data, last + 2, stated - 4);
            Expected {
                stop: Some(len - 4),
                unread: 4,
                records_read: starts.len(),
                first_touched: last,
            }
        }
        Damage::CutAtTheLastRecord => {
            data.truncate(last);
            Expected {
                stop: None,
                unread: 0,
                records_read: starts.len() - 1,
                first_touched: last,
            }
        }
        Damage::HeaderCountPlusOne => {
            let stated = u32_at(data, 4);
            put_u32(data, 4, stated + 1);
            Expected {
                stop: None,
                unread: 0,
                records_read: starts.len(),
                first_touched: len,
            }
        }
        Damage::WrongMagic => {
            put_u32(data, 0, 0x4B43_4142);
            Expected {
                stop: None,
                unread: 0,
                records_read: starts.len(),
                first_touched: len,
            }
        }
    }
}

/// The intact fixture's `/Sheet6` bytes, the bytes after `damage`, the
/// damaged document parsed, and what it has to read as. The fixture is
/// committed, so a checkout without it fails here rather than skipping.
fn damaged(damage: Damage) -> (Vec<u8>, Vec<u8>, PidDocument, Expected) {
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let intact = package
        .get_stream(SHEET)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {SHEET}"))
        .data
        .clone();
    let mut broken = intact.clone();
    let expected = damage_stream(&mut broken, damage);
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: SHEET.to_string(),
            new_data: broken.clone(),
        }],
        ..WritePlan::default()
    };
    let bytes = PidWriter::write_to_bytes(&package, &plan)
        .unwrap_or_else(|error| panic!("{damage:?}: the damaged copy is written: {error}"));
    let doc = PidParser::new()
        .parse_reader(Cursor::new(bytes))
        .unwrap_or_else(|error| panic!("{damage:?}: the damaged copy parses: {error}"));
    (intact, broken, doc, expected)
}

/// Why a walk that stops at each damage stops there.
fn stop_reason(damage: Damage) -> Option<StreamChainStopReason> {
    match damage {
        Damage::MiddleLengthZero => Some(StreamChainStopReason::ZeroLength),
        Damage::LastLengthPlusOne => Some(StreamChainStopReason::PastEnd),
        Damage::LastLengthMinusFour => Some(StreamChainStopReason::ShortEnvelope),
        Damage::CutAtTheLastRecord | Damage::HeaderCountPlusOne | Damage::WrongMagic => None,
    }
}

/// Each damage is said in a warning that names the stream and, where the
/// walk stopped short, the offset it stopped at.
#[test]
fn a_broken_chain_is_named_in_a_warning_with_the_offset_it_stops_at() {
    for damage in ALL {
        let (_, broken, doc, expected) = damaged(damage);
        let geometry = build_normalized_geometry(&doc);

        // The fault itself, on the document and handed on by the geometry.
        assert_eq!(
            doc.chain_faults.len(),
            1,
            "{damage:?}: one stream is damaged: {:#?}",
            doc.chain_faults
        );
        assert_eq!(geometry.chain_faults, doc.chain_faults, "{damage:?}");
        let fault = &doc.chain_faults[0];
        assert_eq!(fault.stream_path, SHEET, "{damage:?}");
        assert_eq!(fault.stream_len, broken.len(), "{damage:?}");
        assert_eq!(
            fault.stop.as_ref().map(|stop| (stop.offset, stop.reason)),
            expected.stop.zip(stop_reason(damage)),
            "{damage:?}: where the walk stops and why"
        );
        assert_eq!(fault.unread_bytes, expected.unread, "{damage:?}");
        assert_eq!(fault.records_read, expected.records_read, "{damage:?}");
        match damage {
            // Nothing stops these walks: the header is what gives them away.
            Damage::CutAtTheLastRecord | Damage::HeaderCountPlusOne => {
                assert!(fault.count_differs(), "{damage:?}: {fault:#?}");
                assert_eq!(fault.wrong_magic, None, "{damage:?}");
            }
            Damage::WrongMagic => {
                assert_eq!(fault.wrong_magic, Some(0x4B43_4142), "{damage:?}");
                assert!(!fault.count_differs(), "{damage:?}: {fault:#?}");
            }
            Damage::MiddleLengthZero | Damage::LastLengthPlusOne | Damage::LastLengthMinusFour => {
                assert_eq!(fault.wrong_magic, None, "{damage:?}");
            }
        }
        assert_eq!(
            StreamChainFault::of_stream(SHEET, &broken).as_ref(),
            Some(fault),
            "{damage:?}: the fault is the stream's bytes read once more"
        );
        let about_the_sheet: Vec<&String> = geometry
            .warnings
            .iter()
            .filter(|warning| warning.contains(SHEET) && warning.contains("record chain"))
            .collect();
        assert_eq!(
            about_the_sheet.len(),
            1,
            "{damage:?}: one warning about {SHEET}'s record chain, among {:#?}",
            geometry.warnings
        );
        let warning = about_the_sheet[0];
        match expected.stop {
            Some(stop) => {
                assert!(
                    warning.contains(&format!("stops at +{stop} ")),
                    "{damage:?}: the warning names the stop +{stop}: {warning}"
                );
                assert!(
                    warning.contains(&format!("{} byte(s) from it on are not", expected.unread)),
                    "{damage:?}: the warning says {} bytes are unread: {warning}",
                    expected.unread
                );
            }
            None => assert!(
                !warning.contains("stops at"),
                "{damage:?}: the chain runs to the stream's end: {warning}"
            ),
        }
    }
}

/// Every record the damage does not touch decodes as it does on the intact
/// stream, family by family, and reaches the same entities.
#[test]
fn the_records_before_a_break_decode_as_on_the_intact_stream() {
    let intact_doc = PidParser::new()
        .parse_file(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    assert!(
        intact_doc.chain_faults.is_empty(),
        "the fixture itself is whole: {:#?}",
        intact_doc.chain_faults
    );
    let intact_geometry = build_normalized_geometry(&intact_doc);
    assert!(
        !intact_geometry
            .warnings
            .iter()
            .any(|warning| warning.contains("record chain")),
        "the fixture itself raises no chain warning: {:#?}",
        intact_geometry.warnings
    );
    let intact_entities = intact_geometry.entities;

    for damage in ALL {
        let (intact, broken, doc, expected) = damaged(damage);
        // The registry's own re-decode of the two byte strings.
        let mut kept = 0usize;
        for family in SHEET_RECORD_FAMILIES {
            let before: Vec<_> = (family.decoded_ranges)(&intact)
                .into_iter()
                .filter(|range| range.end <= expected.first_touched)
                .collect();
            let after: Vec<_> = (family.decoded_ranges)(&broken)
                .into_iter()
                .filter(|range| range.start < expected.first_touched)
                .collect();
            assert_eq!(
                after, before,
                "{damage:?}: {} decodes the records before +{} as on the intact stream",
                family.name, expected.first_touched
            );
            kept += before.len();
            // Past the first record the damage touches, a family may decode
            // that record and nothing else.
            for range in (family.decoded_ranges)(&broken) {
                assert!(
                    range.start <= expected.first_touched,
                    "{damage:?}: {} decoded +{}..+{}, past the damaged record at +{}",
                    family.name,
                    range.start,
                    range.end,
                    expected.first_touched
                );
            }
        }
        assert!(
            kept > 0,
            "{damage:?}: no record before +{} to hold the prefix to",
            expected.first_touched
        );

        // What the parser kept for the sheet: the lines, record for record.
        let lines = |doc: &PidDocument| {
            doc.sheet_streams
                .iter()
                .find(|sheet| sheet.path == SHEET)
                .and_then(|sheet| sheet.geometry.as_ref())
                .map(|geometry| geometry.decoded_iglines.clone())
                .unwrap_or_default()
        };
        let before: Vec<_> = lines(&intact_doc)
            .into_iter()
            .filter(|line| line.byte_end <= expected.first_touched)
            .collect();
        let after: Vec<_> = lines(&doc)
            .into_iter()
            .filter(|line| line.byte_start < expected.first_touched)
            .collect();
        assert_eq!(
            after, before,
            "{damage:?}: the sheet's igLine2d records before the break"
        );

        // And what a renderer is handed: the decoded entities drawn from
        // those records, in the same order.
        let from_the_prefix = |entities: &[pid_parse::PidGraphicEntity]| {
            entities
                .iter()
                .filter(|entity| {
                    entity.source.stream_path.as_deref() == Some(SHEET)
                        && entity
                            .source
                            .byte_range
                            .as_ref()
                            .is_some_and(|range| range.end <= expected.first_touched)
                        && matches!(entity.confidence, pid_parse::PidGeometryConfidence::Decoded)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let before = from_the_prefix(&intact_entities);
        let broken_entities = build_normalized_geometry(&doc).entities;
        let after = from_the_prefix(&broken_entities);
        assert!(
            !before.is_empty(),
            "{damage:?}: no decoded entity before the break"
        );
        // Nothing decoded comes from past the stop. (The heuristic probes
        // are not chain readers and scan the whole stream as they always
        // did; what they find is not `Decoded`.)
        if let Some(stop) = expected.stop {
            for entity in &broken_entities {
                let from_the_sheet = entity.source.stream_path.as_deref() == Some(SHEET)
                    && matches!(entity.confidence, pid_parse::PidGeometryConfidence::Decoded);
                if let Some(range) = entity.source.byte_range.as_ref().filter(|_| from_the_sheet) {
                    assert!(
                        range.end <= stop,
                        "{damage:?}: {} is decoded from +{}..+{}, past the stop at +{stop}",
                        entity.id,
                        range.start,
                        range.end
                    );
                }
            }
        }
        assert_eq!(
            after.len(),
            before.len(),
            "{damage:?}: decoded entities from the records before +{}",
            expected.first_touched
        );
        // One thing about an entity is the page's and not its record's: the
        // coordinate context, which is read off the sheet's border frame. A
        // break ahead of every frame leaves the page to the template's name
        // (millimetres for the page, no proven transform for the entity);
        // a break that leaves a frame on the chain changes nothing at all.
        let frame_before_the_break = !decode_smartframes(&broken).is_empty();
        for (after, before) in after.iter().zip(&before) {
            let mut placed = after.clone();
            placed.coordinate_context = before.coordinate_context.clone();
            assert_eq!(
                &placed, before,
                "{damage:?}: the same entities, in the same order"
            );
            assert_eq!(
                after.coordinate_context == before.coordinate_context,
                frame_before_the_break,
                "{damage:?}: {} keeps its page transform exactly when a border frame \
                 is read before the break",
                after.id
            );
        }
        // The walk the test expects is the one the bytes give.
        let walked = {
            let mut at = 8usize;
            let mut read = 0usize;
            while let Some(header) = parse_psm_header(&broken, at) {
                let end = header.body_start + header.bytes_to_follow as usize;
                if header.bytes_to_follow == 0 || end > broken.len() {
                    break;
                }
                read += 1;
                at = end;
            }
            (read, at)
        };
        assert_eq!(
            walked,
            (expected.records_read, expected.stop.unwrap_or(broken.len())),
            "{damage:?}: records read and where the walk ends"
        );
    }
}

/// What the walk over one corpus read.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct Tally {
    /// Files parsed: drawings, border templates and symbols.
    documents: usize,
    /// Files whose bytes an earlier file of the walk already had; they are
    /// not read a second time.
    repeats: usize,
    /// Files that are not a compound file a parser reads; they hold no
    /// stream to raise a fault.
    unread: usize,
    /// `Sheet*` streams.
    sheets: usize,
    /// Of those, the ones that are a header and nothing else.
    header_only: usize,
    /// `JSite*/PSMcluster0` symbol caches.
    caches: usize,
    /// Records on those streams' chains.
    records: usize,
    /// Of those, the ones the native reader reads.
    live: usize,
}

impl Tally {
    fn add(&mut self, other: &Self) {
        self.documents += other.documents;
        self.repeats += other.repeats;
        self.unread += other.unread;
        self.sheets += other.sheets;
        self.header_only += other.header_only;
        self.caches += other.caches;
        self.records += other.records;
        self.live += other.live;
    }

    /// Whether this walk read no less than `floor` did.
    fn reaches(&self, floor: &Self) -> bool {
        self.documents >= floor.documents
            && self.sheets >= floor.sheets
            && self.header_only >= floor.header_only
            && self.caches >= floor.caches
            && self.records >= floor.records
            && self.live >= floor.live
    }
}

/// Whether the parser reads the stream at `path` as a record chain: a
/// `Sheet*` stream at any depth (`Some(true)`), or a top-level `JSite*`
/// storage's `PSMcluster0` (`Some(false)`).
fn chain_stream_kind(path: &str) -> Option<bool> {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let leaf = *parts.last()?;
    if leaf.starts_with("Sheet") {
        return Some(true);
    }
    (parts.len() == 2 && parts[0].starts_with("JSite") && leaf == "PSMcluster0").then_some(false)
}

/// Hold one parsed file to raising no fault, and count what it holds.
fn hold_sound(label: &str, package: &PidPackage, tally: &mut Tally) {
    assert!(
        package.parsed.chain_faults.is_empty(),
        "{label}: an intact file raises no chain fault: {:#?}",
        package.parsed.chain_faults
    );
    let geometry = build_normalized_geometry(&package.parsed);
    assert!(geometry.chain_faults.is_empty(), "{label}");
    assert!(
        !geometry
            .warnings
            .iter()
            .any(|warning| warning.contains("record chain")),
        "{label}: no chain warning: {:#?}",
        geometry.warnings
    );
    tally.documents += 1;
    for (path, raw) in &package.streams {
        let Some(is_sheet) = chain_stream_kind(path) else {
            continue;
        };
        // The measurement the header check rests on, stream by stream: the
        // magic, a walk that ends on the stream's last byte, and a header
        // count equal to the records without the native-skip bit.
        let chain = SheetChain::walk(&raw.data);
        if !raw.data.is_empty() {
            assert_eq!(
                chain.magic,
                Some(SHEET_STREAM_MAGIC),
                "{label} {path}: the chain magic"
            );
            assert_eq!(
                chain.stated_records.map(|stated| stated as usize),
                Some(chain.live_records),
                "{label} {path}: the header counts the live records"
            );
        }
        assert_eq!(
            (chain.stop, chain.end),
            (None, raw.data.len()),
            "{label} {path}: the walk ends on the stream's last byte"
        );
        assert!(chain.is_sound(), "{label} {path}");
        if is_sheet {
            tally.sheets += 1;
            if chain.starts.is_empty() {
                tally.header_only += 1;
            }
        } else {
            tally.caches += 1;
        }
        tally.records += chain.starts.len();
        tally.live += chain.live_records;
    }
}

/// Every drawing, template and symbol under `dir`, and every `RefData~*.zip`
/// that may hold more of them. A machine-local symbol library and this
/// crate's own plots are left out.
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
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if ["pid", "igr", "sym"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
        {
            files.push(path);
        } else if extension.eq_ignore_ascii_case("zip") && name.starts_with("RefData~") {
            archives.push(path);
        }
    }
}

/// One walk over every corpus in reach. A file is read once however many
/// corpora carry a copy of it: the same bytes parse to the same streams.
struct Walk {
    /// Every file's bytes read so far, by length and hash.
    seen: HashSet<(usize, u64)>,
    /// What a renderer parses a drawing with. A chain is held to its
    /// framing in the same two places under every profile that decodes the
    /// stream; this one leaves out the string scans and the probes, which
    /// read no chain.
    parser: PidParser,
}

impl Walk {
    fn new() -> Self {
        Self {
            seen: HashSet::new(),
            parser: PidParser::with_options(ParseOptions::geometry()),
        }
    }

    /// Hold one file to raising no fault, unless its bytes were already
    /// read.
    fn hold(&mut self, label: &str, bytes: Vec<u8>, tally: &mut Tally) {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        if !self.seen.insert((bytes.len(), hasher.finish())) {
            tally.repeats += 1;
            return;
        }
        match self.parser.parse_package_from_reader(Cursor::new(bytes)) {
            Ok(package) => hold_sound(label, &package, tally),
            Err(_) => tally.unread += 1,
        }
    }

    /// The drawings, templates and symbols on disk under `root`, and then
    /// the ones inside its reference-data archives.
    fn corpus(&mut self, root: &Path) -> (Tally, Tally) {
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents(root, &mut files, &mut archives);
        let mut on_disk = Tally::default();
        for file in &files {
            match std::fs::read(file) {
                Ok(bytes) => self.hold(&file.display().to_string(), bytes, &mut on_disk),
                Err(_) => on_disk.unread += 1,
            }
        }
        let mut archived = Tally::default();
        #[cfg(feature = "refdata-archive")]
        for archive in &archives {
            for extension in ["igr", "sym"] {
                let Ok(members) = pid_parse::refdata_archive::archive_members(archive, extension)
                else {
                    continue;
                };
                for member in members {
                    let label = member.label().display().to_string();
                    match pid_parse::refdata_archive::read_archive_file(&member) {
                        Ok(bytes) => self.hold(&label, bytes, &mut archived),
                        Err(_) => archived.unread += 1,
                    }
                }
            }
        }
        (on_disk, archived)
    }
}

/// No intact stream raises a fault: every drawing, border template and
/// symbol in reach parses with an empty fault list and no chain warning,
/// and every chain stream in them opens with the magic, walks to its last
/// byte and holds the live-record count its header states.
///
/// The repository's own files are always read, and what they hold has a
/// floor under it — a walk that stopped finding these streams would
/// otherwise pass by reading none. The corpora outside the repository are
/// read where this machine has them: the plant backup and the acceptance
/// sheets under either `pid-test-data`, and the share. Each line printed
/// says what was read.
#[test]
fn no_intact_stream_of_any_corpus_raises_a_chain_fault() {
    let mut walk = Walk::new();
    let (on_disk, archived) = walk.corpus(Path::new("test-file"));
    eprintln!("test-file, on disk: {on_disk:?}");
    eprintln!("test-file, in its RefData archives: {archived:?}");
    assert_eq!(
        (on_disk.unread, archived.unread),
        (0, 0),
        "every file of the repository's parses"
    );
    assert!(
        on_disk.reaches(&REPOSITORY_ON_DISK),
        "the repository's own files hold no less than when this was written: \
         {on_disk:?} against {REPOSITORY_ON_DISK:?}"
    );
    // The archives are held with the files on disk, as one sum: a member
    // whose bytes a file on disk already had is read once, under the file.
    #[cfg(feature = "refdata-archive")]
    {
        let mut whole = on_disk;
        whole.add(&archived);
        let mut floor = REPOSITORY_ON_DISK;
        floor.add(&REPOSITORY_ARCHIVED);
        assert!(
            whole.reaches(&floor),
            "the repository's files and archives hold no less than when this was written: \
             {whole:?} against {floor:?}"
        );
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    for named in ["PID_TEST_DATA_DIR", "PID_PARSE_SELF_CHECK_DIR"] {
        if let Some(dir) = std::env::var_os(named) {
            roots.push(PathBuf::from(dir));
        }
    }
    roots.extend(
        [
            r"D:\work\cad\pid-test-data",
            r"D:\work\plant-code\cad\pid-test-data",
            r"D:\work\cad\PID自研软件验证",
        ]
        .map(PathBuf::from),
    );
    let mut total = on_disk;
    total.add(&archived);
    let mut read: Vec<PathBuf> = Vec::new();
    for root in roots {
        let Ok(canonical) = root.canonicalize() else {
            eprintln!("not on this machine: {}", root.display());
            continue;
        };
        if read.contains(&canonical) {
            continue;
        }
        read.push(canonical);
        let (on_disk, archived) = walk.corpus(&root);
        eprintln!("{}, on disk: {on_disk:?}", root.display());
        eprintln!("{}, in its RefData archives: {archived:?}", root.display());
        total.add(&on_disk);
        total.add(&archived);
    }
    eprintln!("in all: {total:?}");
}

/// What `test-file` held on disk when this was written, the machine-local
/// `symbols-full` left out: seven drawings, five border templates and six
/// symbols. A floor, so that a fixture added later does not have to touch
/// it.
const REPOSITORY_ON_DISK: Tally = Tally {
    documents: 18,
    repeats: 0,
    unread: 0,
    sheets: 32,
    header_only: 6,
    caches: 17,
    records: 11_945,
    live: 11_895,
};

/// What the templates and symbols inside `test-file`'s twelve committed
/// `RefData~*.zip` archives held beyond that, each distinct file once: 637
/// of their 1 242 members, the other 605 being copies.
#[cfg(feature = "refdata-archive")]
const REPOSITORY_ARCHIVED: Tally = Tally {
    documents: 637,
    repeats: 0,
    unread: 0,
    sheets: 1306,
    header_only: 139,
    caches: 41,
    records: 20_569,
    live: 20_165,
};
