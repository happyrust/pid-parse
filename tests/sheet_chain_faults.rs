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
    decode_smartframes, parse_psm_header, sheet_record_starts, AttributeFragmentDecoder,
    JStyleOverrideDecoder, PsmRecordDecoder, SheetChain, SubRecord0x0010Decoder,
    SHEET_STREAM_MAGIC,
};
use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};
use pid_parse::{
    build_normalized_geometry, ParseOptions, PidDocument, PidPackage, PidParser, StreamChainFault,
    StreamChainStop, StreamChainStopReason, SHEET_RECORD_FAMILIES,
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

/// One record of a committed stream made to state a length that is wrong
/// and still inside the stream.
struct LengthChange {
    fixture: &'static str,
    stream: &'static str,
    /// Index of the record on the stream's chain.
    record: usize,
    /// What is added to the length it states.
    delta: i64,
    /// The word the walk then finds at the first offset past the record.
    stops_on: u16,
    /// Whether the stream read as a sound chain — no stop, the header's
    /// count met — for as long as the walk took any word for a type word.
    was_silent: bool,
}

/// The two examples of the second independent review, and then every
/// single-length change of the repository's own drawings and templates
/// that used to read as a sound chain, with four of the 69 of their symbol
/// caches. Each of the silent ones went through a "record" of type word
/// zero that ended where a record of the file starts.
const LENGTH_CHANGES: [LengthChange; 13] = [
    LengthChange {
        fixture: "test-file/DWG-0202GP06-01.pid",
        stream: "/Sheet6",
        record: 130,
        delta: -1,
        stops_on: 0x5E00,
        was_silent: false,
    },
    LengthChange {
        fixture: "test-file/DWG-0201GP06-01.pid",
        stream: "/Sheet6",
        record: 0,
        delta: 8,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/CPE-SQ-A0.igr",
        stream: "/Sheet6",
        record: 26,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/CPE-SQ-A0.igr",
        stream: "/Sheet6",
        record: 144,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/CPE-SQ-A1.igr",
        stream: "/Sheet6",
        record: 26,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/CPE-SQ-A1.igr",
        stream: "/Sheet6",
        record: 144,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/CPE-SQ-A2.igr",
        stream: "/Sheet6",
        record: 27,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/XA-A1.igr",
        stream: "/Sheet6",
        record: 30,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/Ref/Template Files/XA-A1.igr",
        stream: "/Sheet6",
        record: 42,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/D06.pid",
        stream: "/JSite145/PSMcluster0",
        record: 10,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/D06.pid",
        stream: "/JSite145/PSMcluster0",
        record: 38,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/D06.pid",
        stream: "/JSite151/PSMcluster0",
        record: 16,
        delta: 16,
        stops_on: 0,
        was_silent: true,
    },
    LengthChange {
        fixture: "test-file/DWG-0201GP06-01.pid",
        stream: "/JSite329/PSMcluster0",
        record: 39,
        delta: 4,
        stops_on: 0,
        was_silent: true,
    },
];

/// The walk as it was before it held a type word to anything: records
/// read, of them live, and where it ended.
fn walk_taking_any_type_word(data: &[u8]) -> (usize, usize, usize) {
    let mut at = 8usize;
    let (mut read, mut live) = (0usize, 0usize);
    while let Some(header) = parse_psm_header(data, at) {
        let end = header.body_start + header.bytes_to_follow as usize;
        if header.bytes_to_follow == 0 || end > data.len() {
            break;
        }
        read += 1;
        if !header.native_reader_skips() {
            live += 1;
        }
        at = end;
    }
    (read, live, at)
}

/// A length that is wrong and still inside the stream sends the walk on
/// from an offset where no record starts. It stops on the word it finds
/// there, so the fault names the first byte past the record whose length
/// is wrong, the record itself, and every byte from there on as unread —
/// where it used to read on through whatever lay there, and, on twelve of
/// these thirteen, to the stream's end with nothing said.
#[test]
fn a_length_that_is_wrong_and_in_bounds_stops_the_walk_past_its_record() {
    for change in &LENGTH_CHANGES {
        let LengthChange {
            fixture,
            stream,
            record,
            delta,
            ..
        } = *change;
        let label = format!("{fixture} {stream} record {record} {delta:+}");
        let package = PidParser::new()
            .parse_package(fixture)
            .unwrap_or_else(|error| panic!("{fixture} should parse: {error}"));
        let intact = package
            .get_stream(stream)
            .unwrap_or_else(|| panic!("{fixture} has a {stream}"))
            .data
            .clone();
        let starts = sheet_record_starts(&intact);
        let at = starts[record];
        let stated = u32_at(&intact, at + 2);
        let wrong = u32::try_from(i64::from(stated) + delta).expect("a length");
        let mut broken = intact.clone();
        put_u32(&mut broken, at + 2, wrong);
        let stop = at + 6 + wrong as usize;
        assert!(
            stop < broken.len() && !starts.contains(&stop),
            "{label}: the wrong length ends inside the stream, where no record starts"
        );

        // What this used to read as.
        let (read, live, end) = walk_taking_any_type_word(&broken);
        if change.was_silent {
            assert_eq!(
                (end, live),
                (broken.len(), u32_at(&broken, 4) as usize),
                "{label}: taking any word for a type word, the walk reaches the stream's end \
                 with the header's count"
            );
        } else {
            assert!(
                end != stop && read > record + 1,
                "{label}: taking any word for a type word, the walk reads on past +{stop}"
            );
        }

        // What it reads as now.
        let fault = StreamChainFault::of_stream(stream, &broken)
            .unwrap_or_else(|| panic!("{label}: the chain does not hold"));
        assert_eq!(
            fault.stop,
            Some(StreamChainStop {
                offset: stop,
                reason: StreamChainStopReason::NoSuchClass,
                record_len: None,
                type_word: Some(change.stops_on),
                last_record_offset: Some(at),
            }),
            "{label}"
        );
        assert_eq!(
            (fault.records_read, fault.unread_bytes),
            (record + 1, broken.len() - stop),
            "{label}: the chain reads up to the changed record and nothing past it"
        );
        let said = fault.describe();
        for needle in [
            format!("stops at +{stop} "),
            format!("0x{:04X} is not one a record opens with", change.stops_on),
            format!("the last of them at +{at}"),
            format!("{} byte(s) from it on are not", broken.len() - stop),
        ] {
            assert!(said.contains(&needle), "{label}: `{needle}` in: {said}");
        }

        // The same through a parse of the whole document.
        let plan = WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: stream.to_string(),
                new_data: broken.clone(),
            }],
            ..WritePlan::default()
        };
        let bytes = PidWriter::write_to_bytes(&package, &plan)
            .unwrap_or_else(|error| panic!("{label}: the changed copy is written: {error}"));
        let doc = PidParser::new()
            .parse_reader(Cursor::new(bytes))
            .unwrap_or_else(|error| panic!("{label}: the changed copy parses: {error}"));
        assert_eq!(doc.chain_faults, vec![fault], "{label}");
        let geometry = build_normalized_geometry(&doc);
        assert_eq!(
            geometry
                .warnings
                .iter()
                .filter(|warning| warning.contains("record chain"))
                .collect::<Vec<_>>(),
            vec![&said],
            "{label}: one warning, the fault's own sentence"
        );
        // No decoded entity comes from past the stop, and every record
        // before the changed one decodes as on the intact stream.
        for entity in &geometry.entities {
            let decoded_here = entity.source.stream_path.as_deref() == Some(stream)
                && matches!(entity.confidence, pid_parse::PidGeometryConfidence::Decoded);
            if let Some(range) = entity.source.byte_range.as_ref().filter(|_| decoded_here) {
                assert!(
                    range.end <= stop,
                    "{label}: {} is decoded from +{}..+{}, past the stop at +{stop}",
                    entity.id,
                    range.start,
                    range.end
                );
            }
        }
        for family in SHEET_RECORD_FAMILIES {
            let before: Vec<_> = (family.decoded_ranges)(&intact)
                .into_iter()
                .filter(|range| range.end <= at)
                .collect();
            let after: Vec<_> = (family.decoded_ranges)(&broken)
                .into_iter()
                .filter(|range| range.start < at)
                .collect();
            assert_eq!(
                after, before,
                "{label}: {} decodes the records before +{at} as on the intact stream",
                family.name
            );
            for range in (family.decoded_ranges)(&broken) {
                assert!(
                    range.end <= stop,
                    "{label}: {} decodes +{}..+{}, past the stop at +{stop}",
                    family.name,
                    range.start,
                    range.end
                );
            }
        }
    }
}

/// A style table is a chain stream too: `StyleCluster`, the document's own
/// and each symbol cache's. `style_link` reads it record by record and
/// stops at the first one that does not frame, so a broken chain leaves the
/// styles after the stop unread — pens, fills and fonts the sheet's records
/// name and then do not find — and it used to say nothing. The fault is
/// raised for it as for a sheet: on the document, in the geometry's
/// warnings, and through those in the export ledger and on `pid_plot`'s
/// stderr.
#[test]
fn a_style_cluster_whose_chain_stops_is_reported_like_a_sheets() {
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let intact_doc = &package.parsed;
    assert!(intact_doc.chain_faults.is_empty());
    let cached = package
        .streams
        .keys()
        .filter(|path| path.starts_with("/JSite") && path.ends_with("/StyleCluster"))
        .min()
        .unwrap_or_else(|| panic!("{FIXTURE} has a symbol cache with a style table"))
        .clone();
    for stream in ["/StyleCluster", cached.as_str()] {
        let intact = package
            .get_stream(stream)
            .unwrap_or_else(|| panic!("{FIXTURE} has a {stream}"))
            .data
            .clone();
        let starts = sheet_record_starts(&intact);
        assert!(starts.len() > 4, "{stream} walks {} records", starts.len());
        let middle = starts[starts.len() / 2];
        let mut broken = intact.clone();
        put_u32(&mut broken, middle + 2, 0);
        let plan = WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: stream.to_string(),
                new_data: broken.clone(),
            }],
            ..WritePlan::default()
        };
        let bytes = PidWriter::write_to_bytes(&package, &plan)
            .unwrap_or_else(|error| panic!("{stream}: the damaged copy is written: {error}"));
        let doc = PidParser::new()
            .parse_reader(Cursor::new(bytes))
            .unwrap_or_else(|error| panic!("{stream}: the damaged copy parses: {error}"));

        let fault = StreamChainFault::of_stream(stream, &broken)
            .unwrap_or_else(|| panic!("{stream}: the chain does not hold"));
        assert_eq!(
            fault.stop.as_ref().map(|stop| (stop.offset, stop.reason)),
            Some((middle, StreamChainStopReason::ZeroLength)),
            "{stream}"
        );
        assert_eq!(doc.chain_faults, vec![fault.clone()], "{stream}");
        let geometry = build_normalized_geometry(&doc);
        assert_eq!(
            geometry
                .warnings
                .iter()
                .filter(|warning| warning.contains("record chain"))
                .collect::<Vec<_>>(),
            vec![&fault.describe()],
            "{stream}: one warning, the fault's own sentence"
        );
        // The table is what the chain read before the stop: it lost styles,
        // and no other storage's table moved.
        let storage = stream
            .strip_suffix("/StyleCluster")
            .filter(|storage| !storage.is_empty())
            .unwrap_or("/");
        for (key, table) in &intact_doc.style_tables {
            let read = &doc.style_tables[key];
            if key == storage {
                assert_ne!(
                    read, table,
                    "{stream}: the table of {key} is short of styles"
                );
            } else {
                assert_eq!(read, table, "{stream}: the table of {key}");
            }
        }
        // The sheets themselves are read whole.
        for sheet in &intact_doc.sheet_streams {
            let read = doc
                .sheet_streams
                .iter()
                .find(|read| read.path == sheet.path)
                .unwrap_or_else(|| panic!("{stream}: {} is still a sheet", sheet.path));
            for family in SHEET_RECORD_FAMILIES {
                let count = |sheet: &pid_parse::SheetStream| {
                    sheet
                        .geometry
                        .as_ref()
                        .map_or(0, |geometry| (family.record_count)(geometry))
                };
                assert_eq!(
                    count(read),
                    count(sheet),
                    "{stream}: {} records of {}",
                    family.name,
                    sheet.path
                );
            }
        }
    }
}

/// A symbol file read as a library body. The body's reader takes a sheet
/// whole or not at all, so a sheet whose chain stops gives it nothing — a
/// body short of that sheet's strokes, and until now nothing that said so.
/// The body carries the faults the parse of the file found.
#[test]
fn a_symbol_files_broken_sheet_is_named_on_the_body_it_leaves_short() {
    use pid_parse::symbol_library::read_symbol_geometry_from_bytes;

    let mut files = Vec::new();
    documents(Path::new("test-file"), &mut files, &mut Vec::new());
    let mut held = 0usize;
    for file in files.iter().filter(|file| {
        file.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
    }) {
        let label = file.display().to_string();
        let bytes = std::fs::read(file).unwrap_or_else(|error| panic!("{label}: {error}"));
        let intact = read_symbol_geometry_from_bytes(&bytes)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert!(
            intact.chain_faults.is_empty(),
            "{label}: {:#?}",
            intact.chain_faults
        );
        let package = PidParser::new()
            .parse_package_from_reader(Cursor::new(bytes))
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        // The sheet that holds the most records.
        let Some((stream, data)) = package
            .streams
            .iter()
            .filter(|(path, _)| chain_stream_kind(path) == Some(true))
            .map(|(path, raw)| (path.clone(), raw.data.clone()))
            .max_by_key(|(_, data)| sheet_record_starts(data).len())
        else {
            continue;
        };
        let starts = sheet_record_starts(&data);
        if starts.len() < 3 {
            continue;
        }
        let middle = starts[starts.len() / 2];
        let mut broken = data.clone();
        put_u32(&mut broken, middle + 2, 0);
        let plan = WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: stream.clone(),
                new_data: broken.clone(),
            }],
            ..WritePlan::default()
        };
        let copy = PidWriter::write_to_bytes(&package, &plan)
            .unwrap_or_else(|error| panic!("{label}: the damaged copy is written: {error}"));
        let body = read_symbol_geometry_from_bytes(&copy)
            .unwrap_or_else(|error| panic!("{label}: the damaged copy reads: {error}"));
        assert_eq!(
            body.chain_faults,
            vec![StreamChainFault::of_stream(&stream, &broken).expect("the chain does not hold")],
            "{label} {stream}"
        );
        assert!(
            body.primitives.len() <= intact.primitives.len(),
            "{label} {stream}: {} primitives against {}",
            body.primitives.len(),
            intact.primitives.len()
        );
        held += 1;
    }
    assert!(
        held >= 3,
        "{held} committed symbol file(s) hold a sheet of three records"
    );
}

/// The fourteen changes made to each record's length field.
const LENGTH_DELTAS: [i64; 14] = [1, -1, 2, -2, 4, -4, 8, -8, 16, -16, 32, -32, 64, -64];

/// What every single-length change of a set of streams reads as.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct LengthSweep {
    /// Distinct streams that hold a record.
    streams: usize,
    /// Records on their chains.
    records: usize,
    /// Changes made: fourteen a record, less those that would state a
    /// length below zero.
    changes: usize,
    /// The walk stops on the changed record: it states no byte, or more
    /// than the stream has.
    stops_on_the_record: usize,
    /// The walk stops at the first offset past the changed record, with
    /// nothing read from where no record starts.
    stops_past_the_record: usize,
    /// The walk reads on from where no record starts, and stops later.
    stops_later: usize,
    /// The walk reaches the stream's end and the header's count gives it
    /// away: a fault that names no place.
    count_differs: usize,
    /// Records the two rows above read from where no record starts.
    records_off_the_chain: usize,
    /// Of those, the ones a record family decodes: something in the parse
    /// result that the file does not hold.
    decoded_off_the_chain: usize,
    /// Of those, the ones the matchers of the three retired families
    /// (`JStyleOverride`, `SubRecord0x0010`, `AttributeFragment`) accept:
    /// what the parse result held of them until those families left the
    /// registry.
    retired_matchers_accept: usize,
    /// The stream reads as a sound chain: the changed record ends where a
    /// later record starts, and every record between is one the native
    /// reader skips. Nothing about the framing can tell.
    swallows_skipped_records: usize,
}

/// Make each of [`LENGTH_DELTAS`] to each record of `data` in turn and
/// count what the walk reads.
///
/// A walk reads a record's own bytes to find the next and nothing before
/// them, so each change is walked on the stream from the changed record on
/// — behind a header counting the live records from there on — rather than
/// from the top; the first records of every stream are walked whole as
/// well and held to the same reading.
fn sweep_lengths(label: &str, data: &[u8], sweep: &mut LengthSweep) {
    let whole = SheetChain::walk(data);
    assert!(whole.is_sound(), "{label}: an intact stream");
    let starts = &whole.starts;
    if starts.is_empty() {
        return;
    }
    sweep.streams += 1;
    sweep.records += starts.len();
    let stated = whole.stated_records.expect("a header");
    let mut live_before = 0u32;
    for (index, &at) in starts.iter().enumerate() {
        let length = u32_at(data, at + 2);
        let shift = at - 8;
        let mut tail = Vec::with_capacity(data.len() - shift);
        tail.extend_from_slice(&SHEET_STREAM_MAGIC.to_le_bytes());
        tail.extend_from_slice(&(stated - live_before).to_le_bytes());
        tail.extend_from_slice(&data[at..]);
        for delta in LENGTH_DELTAS {
            let Ok(wrong) = u32::try_from(i64::from(length) + delta) else {
                continue;
            };
            put_u32(&mut tail, 10, wrong);
            let chain = SheetChain::walk(&tail);
            if index < 24 {
                let mut copy = data.to_vec();
                put_u32(&mut copy, at + 2, wrong);
                let full = SheetChain::walk(&copy);
                assert_eq!(
                    (full.stop, full.end, full.starts.len(), full.count_differs()),
                    (
                        chain.stop,
                        chain.end + shift,
                        index + chain.starts.len(),
                        chain.count_differs()
                    ),
                    "{label}: record {index} at +{at}, {length} -> {wrong}: the stream walked \
                     whole and walked from the record"
                );
            }
            sweep.changes += 1;
            let off_the_chain = chain
                .starts
                .iter()
                .skip(1)
                .filter(|&&read| starts.binary_search(&(read + shift)).is_err())
                .count();
            if off_the_chain > 0 {
                let mut copy = data.to_vec();
                put_u32(&mut copy, at + 2, wrong);
                let off: Vec<usize> = chain
                    .starts
                    .iter()
                    .skip(1)
                    .map(|read| read + shift)
                    .filter(|read| starts.binary_search(read).is_err())
                    .collect();
                for family in SHEET_RECORD_FAMILIES {
                    for range in (family.decoded_ranges)(&copy) {
                        if off.contains(&range.start) {
                            sweep.decoded_off_the_chain += 1;
                        }
                    }
                }
                let retired = JStyleOverrideDecoder
                    .scan(&copy)
                    .into_iter()
                    .map(|record| record.byte_range.start)
                    .chain(
                        SubRecord0x0010Decoder
                            .scan(&copy)
                            .into_iter()
                            .map(|record| record.byte_range.start),
                    )
                    .chain(
                        AttributeFragmentDecoder
                            .scan(&copy)
                            .into_iter()
                            .map(|record| record.byte_range.start),
                    );
                sweep.retired_matchers_accept +=
                    retired.filter(|start| off.contains(start)).count();
            }
            let past_the_record = 8 + 6 + wrong as usize;
            match chain.stop {
                Some(_) if chain.end == 8 => sweep.stops_on_the_record += 1,
                Some(_) if chain.end == past_the_record => sweep.stops_past_the_record += 1,
                Some(_) => {
                    sweep.stops_later += 1;
                    sweep.records_off_the_chain += off_the_chain;
                }
                None if chain.count_differs() => {
                    sweep.count_differs += 1;
                    sweep.records_off_the_chain += off_the_chain;
                }
                None => {
                    let lands = past_the_record + shift;
                    assert!(
                        off_the_chain == 0
                            && (lands == data.len() || starts.binary_search(&lands).is_ok()),
                        "{label}: record {index} at +{at}, {length} -> {wrong}, reads as a sound \
                         chain through {off_the_chain} record(s) the file does not hold"
                    );
                    sweep.swallows_skipped_records += 1;
                }
            }
        }
        if !parse_psm_header(data, at).is_some_and(|header| header.native_reader_skips()) {
            live_before += 1;
        }
    }
}

/// The three kinds of chain stream the sweep keeps apart.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct LengthSweeps {
    /// `Sheet*` streams of drawings and border templates.
    sheets: LengthSweep,
    /// `JSite*/PSMcluster0` symbol caches.
    caches: LengthSweep,
    /// `Sheet*` streams of symbol files.
    symbol_sheets: LengthSweep,
}

/// Sweep every chain stream of one file, each distinct stream once.
fn sweep_file(
    label: &str,
    bytes: Vec<u8>,
    seen: &mut HashSet<(usize, u64)>,
    sweeps: &mut LengthSweeps,
) {
    let is_symbol = label.to_ascii_lowercase().ends_with(".sym");
    let Ok(package) = PidParser::with_options(ParseOptions::geometry())
        .parse_package_from_reader(Cursor::new(bytes))
    else {
        return;
    };
    let mut paths: Vec<&String> = package.streams.keys().collect();
    paths.sort();
    for path in paths {
        let Some(is_sheet) = chain_stream_kind(path) else {
            continue;
        };
        let data = &package.streams[path].data;
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        if !seen.insert((data.len(), hasher.finish())) {
            continue;
        }
        let sweep = match (is_sheet, is_symbol) {
            (true, false) => &mut sweeps.sheets,
            (true, true) => &mut sweeps.symbol_sheets,
            (false, _) => &mut sweeps.caches,
        };
        sweep_lengths(&format!("{label} {path}"), data, sweep);
    }
}

/// Every record of every chain stream the repository commits, with its
/// length field changed by ±1, ±2 … ±64: what each change reads as.
///
/// None reads as a sound chain through a record the file does not hold —
/// 79 did on the files on disk before the walk held type words to the
/// native reader's class table, ten of them on sheets
/// ([`LENGTH_CHANGES`]). Nearly all stop at the first offset past the
/// changed record, which is where the fault then points. What is left,
/// pinned so that a change to the rule shows here:
///
/// * the walk reads one "record" more before it stops, because the bytes
///   it lands on spell a type code the reader has a class for;
/// * the walk lands back on the chain and reaches the stream's end, and
///   the header's count is all that says so;
/// * in symbol files, the changed record ends exactly where a later record
///   starts and every record it swallowed is a superseded copy the native
///   reader skips: that reads as a sound chain, and no rule about framing
///   can say otherwise.
///
/// No record family decodes one of the records read off the chain
/// (`decoded_off_the_chain` is zero in every row). The matchers of the
/// three families retired on 2026-11-11 accept some of them
/// (`retired_matchers_accept`): a sub-type word `0x0010` or a block length
/// `30 00` is as good a type word to the walk as any, and those families
/// were the only ones that made a record of what followed. With their rows
/// in the registry that number was `decoded_off_the_chain`.
#[test]
fn every_single_length_change_of_a_committed_stream_is_accounted_for() {
    let mut files = Vec::new();
    let mut archives = Vec::new();
    documents(Path::new("test-file"), &mut files, &mut archives);
    let mut seen = HashSet::new();
    let mut on_disk = LengthSweeps::default();
    for file in &files {
        let bytes = std::fs::read(file).unwrap_or_else(|error| panic!("{file:?}: {error}"));
        sweep_file(&file.display().to_string(), bytes, &mut seen, &mut on_disk);
    }
    eprintln!("length sweep, test-file on disk: {on_disk:#?}");
    assert_eq!(on_disk, LENGTHS_ON_DISK);

    #[cfg(feature = "refdata-archive")]
    {
        let mut archived = LengthSweeps::default();
        for archive in &archives {
            for extension in ["igr", "sym"] {
                let Ok(members) = pid_parse::refdata_archive::archive_members(archive, extension)
                else {
                    continue;
                };
                for member in members {
                    let label = member.label().display().to_string();
                    if let Ok(bytes) = pid_parse::refdata_archive::read_archive_file(&member) {
                        sweep_file(&label, bytes, &mut seen, &mut archived);
                    }
                }
            }
        }
        eprintln!("length sweep, test-file's RefData archives: {archived:#?}");
        assert_eq!(archived, LENGTHS_ARCHIVED);
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

/// What every single-length change of the chain streams `test-file` holds on
/// disk reads as. Exact: a stream added, or a rule changed, moves these.
const LENGTHS_ON_DISK: LengthSweeps = LengthSweeps {
    sheets: LengthSweep {
        streams: 15,
        records: 8261,
        changes: 109_090,
        stops_on_the_record: 176,
        stops_past_the_record: 108_744,
        stops_later: 168,
        count_differs: 2,
        records_off_the_chain: 175,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 62,
        swallows_skipped_records: 0,
    },
    caches: LengthSweep {
        streams: 16,
        records: 2948,
        changes: 38_725,
        stops_on_the_record: 141,
        stops_past_the_record: 38_097,
        stops_later: 354,
        count_differs: 133,
        records_off_the_chain: 360,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 0,
        swallows_skipped_records: 0,
    },
    symbol_sheets: LengthSweep {
        streams: 9,
        records: 99,
        changes: 1303,
        stops_on_the_record: 70,
        stops_past_the_record: 1233,
        stops_later: 0,
        count_differs: 0,
        records_off_the_chain: 0,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 0,
        swallows_skipped_records: 0,
    },
};

/// The same for the templates and symbols of `test-file`'s `RefData~*.zip`
/// archives, the streams a file on disk already had left out.
#[cfg(feature = "refdata-archive")]
const LENGTHS_ARCHIVED: LengthSweeps = LengthSweeps {
    sheets: LengthSweep {
        streams: 9,
        records: 910,
        changes: 12_120,
        stops_on_the_record: 107,
        stops_past_the_record: 12_012,
        stops_later: 0,
        count_differs: 1,
        records_off_the_chain: 0,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 0,
        swallows_skipped_records: 0,
    },
    caches: LengthSweep {
        streams: 40,
        records: 1444,
        changes: 19_010,
        stops_on_the_record: 382,
        stops_past_the_record: 18_255,
        stops_later: 369,
        count_differs: 4,
        records_off_the_chain: 374,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 0,
        swallows_skipped_records: 0,
    },
    symbol_sheets: LengthSweep {
        streams: 1003,
        records: 16_290,
        changes: 211_607,
        stops_on_the_record: 7795,
        stops_past_the_record: 203_183,
        stops_later: 79,
        count_differs: 536,
        records_off_the_chain: 79,
        decoded_off_the_chain: 0,
        retired_matchers_accept: 0,
        swallows_skipped_records: 14,
    },
};
