//! The display order of the committed drawings and templates, read off the
//! files themselves.
//!
//! `parsers::sheet_display_order` has its own unit tests on bytes built by
//! hand. These hold it to real files, and run in CI: every one is committed
//! under `test-file`, so a missing file fails rather than skips.
//!
//! For each of the seven fixture drawings and the five border templates
//! beside them:
//!
//! - every top-level sheet stream that holds a live record of the sheet's
//!   own has a display order, and a stream that has none holds no live
//!   record at all;
//! - the sheet's list is the sheet's: it names exactly the live records of
//!   the stream whose parent is the sheet, and every live record of the
//!   stream takes a rank from it — none is left to be painted after the
//!   rest;
//! - how many objects the sheet lists, how many records they rank, where
//!   the list stands the border frame, and the order itself — a digest of
//!   the records' offsets in the order the sheet draws them — are what
//!   [`PINNED`] says. The seven drawings list 1 014 objects between them.
//!
//! And for the one fixture whose frame embeds a sheet, `A01.pid`: the
//! embedded sheet `/JSite204/Sheet6` has an order of its own, read from its
//! storage's cluster, and stands where the drawing lists the frame that
//! names that storage — first.
//!
//! The readings are taken here from the stream bytes with the chain walk
//! and the envelope reader, not through the functions under test: what is
//! compared with the module's answer is counted on its own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_display_order::{DisplayRank, SheetDisplayOrder};
use pid_parse::parsers::sheet_records::{parse_live_psm_header, SheetChain};
use pid_parse::PidParser;

/// One committed file and what its `/Sheet6` states.
struct Pinned {
    /// Path under `test-file`.
    file: &'static str,
    /// Objects the sheet's `JSheet` record lists.
    children: usize,
    /// Live records of the stream, every one of which takes a rank.
    records: usize,
    /// Where the list stands the first page frame; `None` for a template,
    /// which is what a frame draws and holds none.
    frame_place: Option<u32>,
    /// FNV-1a over the offsets of the stream's live records in the order
    /// the sheet draws them.
    order: u64,
}

const PINNED: [Pinned; 12] = [
    Pinned {
        file: "D06.pid",
        children: 26,
        records: 48,
        frame_place: Some(0),
        order: 0x1ad2_9313_97ea_1349,
    },
    Pinned {
        file: "DWG-0201GP06-01.pid",
        children: 160,
        records: 354,
        frame_place: Some(91),
        order: 0x1b05_5e9d_5ac3_e4ab,
    },
    Pinned {
        file: "DWG-0202GP06-01.pid",
        children: 109,
        records: 274,
        frame_place: Some(0),
        order: 0xc5c7_b624_1171_af0e,
    },
    Pinned {
        file: "export-test/publish-data/A01/A01.pid",
        children: 10,
        records: 19,
        frame_place: Some(0),
        order: 0x6b4c_24e1_5043_4d6b,
    },
    Pinned {
        file: "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        children: 109,
        records: 274,
        frame_place: Some(0),
        order: 0x6ddb_c618_67b0_07e7,
    },
    Pinned {
        file: "工艺管道及仪表流程-1.pid",
        children: 149,
        records: 535,
        frame_place: Some(0),
        order: 0x1bc4_6c6f_6937_501b,
    },
    Pinned {
        file: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        children: 451,
        records: 973,
        frame_place: Some(0),
        order: 0x1a0f_9833_ddda_9098,
    },
    Pinned {
        file: "Ref/Template Files/CPE-SQ-A0.igr",
        children: 174,
        records: 174,
        frame_place: None,
        order: 0xf022_5c87_ff2a_31c4,
    },
    Pinned {
        file: "Ref/Template Files/CPE-SQ-A1.igr",
        children: 174,
        records: 174,
        frame_place: None,
        order: 0xf022_5c87_ff2a_31c4,
    },
    Pinned {
        file: "Ref/Template Files/CPE-SQ-A2.igr",
        children: 174,
        records: 174,
        frame_place: None,
        order: 0xacd6_e5c6_2a08_e402,
    },
    Pinned {
        file: "Ref/Template Files/CPECCHB-SQ-A2.igr",
        children: 71,
        records: 87,
        frame_place: None,
        order: 0x6f51_63fc_4ea1_a5a9,
    },
    Pinned {
        file: "Ref/Template Files/XA-A1.igr",
        children: 5024,
        records: 5024,
        frame_place: None,
        order: 0xc2d7_adb2_a115_a670,
    },
];

/// Objects the seven fixture drawings list between them.
const DRAWING_CHILDREN: usize = 1014;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

/// A live record of a sheet stream as the chain walk and the envelope
/// reader give it: where it starts, its oid and its owner.
struct Record {
    at: usize,
    parent: u32,
}

/// The stream's live records that state an oid and an owner, in stream
/// order.
fn live_records(data: &[u8]) -> Vec<Record> {
    let word = |at: usize| -> Option<u32> {
        let bytes = data.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    SheetChain::walk(data)
        .starts
        .into_iter()
        .filter_map(|at| {
            let header = parse_live_psm_header(data, at)?;
            if header.bytes_to_follow < 8 {
                return None;
            }
            word(header.body_start)?;
            Some(Record {
                at,
                parent: word(header.body_start + 4)?,
            })
        })
        .collect()
}

/// FNV-1a over the offsets `order` ranks, in the order it draws them.
fn digest(order: &SheetDisplayOrder) -> u64 {
    let mut drawn: Vec<(DisplayRank, usize)> =
        order.ranks.iter().map(|(at, rank)| (*rank, *at)).collect();
    drawn.sort_unstable();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (_, at) in drawn {
        for byte in (at as u64).to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// What one file's `/Sheet6` reads as, in [`PINNED`]'s own spelling.
fn reading(file: &str) -> (String, usize) {
    let path = fixtures().join(file);
    let package = PidParser::new()
        .parse_package(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let doc = &package.parsed;
    // Every top-level sheet stream: one that holds a record of the sheet's
    // own has an order, and one without an order holds no live record.
    let mut sheets: BTreeMap<String, u32> = BTreeMap::new();
    for stream in package.streams.keys() {
        if let Some(oid) = stream
            .strip_prefix("/Sheet")
            .and_then(|number| number.parse::<u32>().ok())
        {
            sheets.insert(stream.clone(), oid);
        }
    }
    assert!(sheets.contains_key("/Sheet6"), "{file}: no /Sheet6");
    for (stream, oid) in &sheets {
        let data = &package.streams[stream].data;
        let records = live_records(data);
        let owned = records
            .iter()
            .filter(|record| record.parent == *oid)
            .count();
        match doc.sheet_display_order.get(stream) {
            None => assert_eq!(
                records.len(),
                0,
                "{file}: {stream} holds {} live record(s) and states no display order",
                records.len()
            ),
            Some(order) => {
                assert_eq!(
                    order.children, owned,
                    "{file}: {stream} lists {} object(s) and holds {owned} live record(s) of \
                     the sheet's own",
                    order.children
                );
                let unranked: Vec<usize> = records
                    .iter()
                    .map(|record| record.at)
                    .filter(|at| !order.ranks.contains_key(at))
                    .collect();
                assert!(
                    unranked.is_empty(),
                    "{file}: {stream}: {} of {} live record(s) take no rank; the first at +{}",
                    unranked.len(),
                    records.len(),
                    unranked[0]
                );
                assert_eq!(
                    order.ranks.len(),
                    records.len(),
                    "{file}: {stream} ranks something that is not a live record"
                );
            }
        }
    }
    let order = doc
        .sheet_display_order
        .get("/Sheet6")
        .unwrap_or_else(|| panic!("{file}: /Sheet6 states no display order"));
    let frame_place = doc
        .sheet_streams
        .iter()
        .find(|sheet| sheet.path == "/Sheet6")
        .and_then(|sheet| sheet.geometry.as_ref())
        .and_then(|geometry| {
            geometry
                .decoded_igsmartframes
                .iter()
                .filter(|frame| frame.page_extent_m().is_some())
                .filter_map(|frame| order.ranks.get(&frame.byte_start))
                .map(|rank| rank.place)
                .min()
        });
    (
        format!(
            "children: {}, records: {}, frame_place: {frame_place:?}, order: {:#018x}",
            order.children,
            order.ranks.len(),
            digest(order)
        ),
        order.children,
    )
}

#[test]
fn every_committed_sheet_lists_its_own_records_in_the_pinned_order() {
    let mut differences = Vec::new();
    let mut drawing_children = 0usize;
    for pinned in &PINNED {
        let (found, children) = reading(pinned.file);
        if pinned.file.ends_with(".pid") {
            drawing_children += children;
        }
        let stated = format!(
            "children: {}, records: {}, frame_place: {:?}, order: {:#018x}",
            pinned.children, pinned.records, pinned.frame_place, pinned.order
        );
        if found != stated {
            differences.push(format!(
                "  {}\n    reads  {found}\n    pinned {stated}",
                pinned.file
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "the display order of {} committed file(s) is not what is pinned:\n{}",
        differences.len(),
        differences.join("\n")
    );
    assert_eq!(
        drawing_children, DRAWING_CHILDREN,
        "the seven fixture drawings list {drawing_children} object(s) between them"
    );
}

#[test]
fn every_committed_drawing_and_template_is_pinned() {
    // A file added under `test-file` is read here or the walk says so.
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .flatten()
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                // `symbols-full` is this machine's own, not the repository's.
                if entry.file_name().is_some_and(|name| name == "symbols-full") {
                    continue;
                }
                walk(&entry, out);
            } else if entry.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("pid") || extension.eq_ignore_ascii_case("igr")
            }) {
                out.push(entry);
            }
        }
    }
    let root = fixtures();
    let mut found = Vec::new();
    walk(&root, &mut found);
    let mut names: Vec<String> = found
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .expect("under test-file")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    names.sort();
    let mut pinned: Vec<String> = PINNED
        .iter()
        .map(|pinned| pinned.file.to_string())
        .collect();
    pinned.sort();
    assert_eq!(
        names, pinned,
        "the drawings and templates under test-file are not the ones PINNED lists"
    );
}

#[test]
fn a01s_embedded_sheet_is_ordered_and_stands_where_its_frame_is_listed() {
    let path = fixtures().join("export-test/publish-data/A01/A01.pid");
    let package = PidParser::new()
        .parse_package(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let doc = &package.parsed;
    // The storage's sheets: `Sheet6` holds the frame's drawing, `Sheet12` a
    // point and no list, the others nothing.
    let embedded: Vec<&String> = doc.embedded_sheet_display_order.keys().collect();
    assert_eq!(
        embedded,
        ["/JSite204/Sheet6"],
        "the embedded sheets with a list"
    );
    let sheet = &doc.embedded_sheet_display_order["/JSite204/Sheet6"];
    let data = &package.streams["/JSite204/Sheet6"].data;
    let records = live_records(data);
    let owned = records.iter().filter(|record| record.parent == 6).count();
    assert_eq!(
        (records.len(), owned),
        (107, 97),
        "live records, and the sheet's own"
    );
    assert_eq!(sheet.order.children, 97);
    assert_eq!(
        sheet.order.ranks.len(),
        107,
        "every live record takes a rank"
    );
    assert!(records
        .iter()
        .all(|record| sheet.order.ranks.contains_key(&record.at)));
    // The frame that names storage 204 is the drawing's first object.
    let frame = sheet.frame.as_ref().expect("a frame names the storage");
    assert_eq!(frame.stream_path, "/Sheet6");
    assert_eq!(frame.rank, DisplayRank { place: 0, inner: 0 });
    // It is not one of the drawing's own sheets, whose paper is the page's.
    assert!(!doc.sheet_display_order.contains_key("/JSite204/Sheet6"));
    // No other fixture drawing embeds a sheet.
    for pinned in PINNED.iter().filter(|pinned| pinned.file.ends_with(".pid")) {
        if pinned.file.ends_with("A01.pid") {
            continue;
        }
        let path = fixtures().join(pinned.file);
        let doc = PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            doc.embedded_sheet_display_order.is_empty(),
            "{}: embeds {:?}",
            pinned.file,
            doc.embedded_sheet_display_order.keys().collect::<Vec<_>>()
        );
    }
}
