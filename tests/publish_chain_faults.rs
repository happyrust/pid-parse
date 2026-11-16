//! `pid_publish_xml` says when a record chain of the `.pid` it reads does
//! not hold.
//!
//! The publish reader walks every stream of the root storage that opens
//! with the chain magic and takes its records all or nothing
//! (`sheet_record_starts`): a stream whose chain stops before its end gives
//! it no record at all. Everything that stream held — attribute rows, the
//! text of labels, symbol placements — is then missing from the XML, the
//! exit code is 0, and until this test nothing was said. An independent
//! review measured it on a copy of `DWG-0201GP06-01.pid` with one length
//! field of `/Sheet6` set to zero: "92 of 102 label values reach an item,
//! 18 attributes published" became "0 of 102 …, 0 attributes published".
//!
//! What is held: the same copy gets exactly one `warning:` line on stderr
//! that names the file, the stream, the offset the chain stops at and the
//! bytes left unread, and says what the publish reader does with such a
//! stream; the exit code and the delivery are what they were; the intact
//! drawing gets no such line.
//!
//! The line says what the stream cost, not what a stream may cost: how many
//! records of the kinds the publish reader keeps the chain read before the
//! stop. A style table holds none of them, so a `StyleCluster` whose chain
//! stops is said and the XML is the intact drawing's byte for byte. And the
//! line is said from the one read that makes the drawing, so it is there
//! when the damaged stream is the one the drawing's own row is in and the
//! run ends in an error.
//!
//! A backup package is held to the same: publishing a directory reads every
//! `.pid` of its `PlantData` zips through the same reader, and a drawing of
//! it whose chain stops used to lose what that stream held with nothing
//! said at all — the run printed "0 failed" and exited 0. Each such stream
//! now gets the same line, naming the zip and the entry the drawing is.

use std::path::{Path, PathBuf};
use std::process::Command;

use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::publish::pid_load::{read_publish_from_pid, PublishChainFault};
use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};
use pid_parse::{PidParser, StreamChainFault};

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");
const FIXTURE: &str = "test-file/DWG-0201GP06-01.pid";
const SHEET: &str = "/Sheet6";

/// A directory of this test's own under the temporary directory, removed
/// when the guard is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "pid-parse-publish-chain-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A copy of the fixture named `name` under `dir`, its stream at `stream`
/// replaced by `new_data`.
fn copy_with_stream(dir: &Path, name: &str, stream: &str, new_data: Vec<u8>) -> PathBuf {
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: stream.to_string(),
            new_data,
        }],
        ..WritePlan::default()
    };
    std::fs::create_dir_all(dir).expect("the copy's directory");
    let copy = dir.join(name);
    std::fs::write(
        &copy,
        PidWriter::write_to_bytes(&package, &plan).expect("the damaged copy is written"),
    )
    .expect("the damaged copy lands in the scratch directory");
    copy
}

/// The fixture's stream at `stream`, and the same bytes with the length of
/// the record in the middle of its chain set to zero: the damaged bytes,
/// and the offset the chain then stops at.
fn middle_length_zeroed(stream: &str) -> (Vec<u8>, Vec<u8>, usize) {
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let intact = package
        .get_stream(stream)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {stream}"))
        .data
        .clone();
    let starts = sheet_record_starts(&intact);
    let middle = starts[starts.len() / 2];
    let mut broken = intact.clone();
    broken[middle + 2..middle + 6].copy_from_slice(&0u32.to_le_bytes());
    (intact, broken, middle)
}

/// Records of the kinds the publish reader keeps — attribute rows
/// (`0x0089`), text boxes (`0x004D`), dependency objects (`0x00FA`), symbol
/// placements (`0x00CE`) — among the live records of `data`'s chain that
/// start before `stop`.
fn publishable_before(data: &[u8], stop: usize) -> usize {
    sheet_record_starts(data)
        .into_iter()
        .filter(|&at| at < stop)
        .filter(|&at| {
            let word = u16::from_le_bytes([data[at], data[at + 1]]);
            word & 0x8000 == 0 && [0x0089, 0x004D, 0x00FA, 0x00CE].contains(&(word & 0x3FFF))
        })
        .count()
}

/// The live records of `data`'s chain that start before `stop`: the ones
/// the native reader reads, whatever their kind.
fn live_before(data: &[u8], stop: usize) -> usize {
    sheet_record_starts(data)
        .into_iter()
        .filter(|&at| at < stop)
        .filter(|&at| u16::from_le_bytes([data[at], data[at + 1]]) & 0x8000 == 0)
        .count()
}

/// The live-record count a chain stream's header states.
fn stated_live(data: &[u8]) -> usize {
    u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize
}

/// What the publish line says the reader is left without when a chain
/// stops after `live` live records, `lost` of them of a kind the reader
/// keeps, in a stream whose header states `stated`: the two clauses after
/// "the publish reader takes nothing from a stream whose chain stops
/// short".
fn left_without(lost: usize, live: usize, stated: usize) -> [String; 2] {
    let read = match (live, lost) {
        (0, _) => "the chain read no live record before the stop".to_string(),
        (live, 0) => format!(
            "none of the {live} live record(s) the chain read before the stop is an attribute \
             row, a text box, a dependency object or a symbol placement, so the XML is missing \
             nothing the chain read"
        ),
        (live, lost) => format!(
            "{lost} of the {live} live record(s) the chain read before the stop are attribute \
             rows, text boxes, dependency objects or symbol placements, and none of them is in \
             the XML"
        ),
    };
    assert!(
        stated > live,
        "the header states more than a stopped chain read"
    );
    [
        read,
        format!(
            "the other {} live record(s) the header states are not read, and the XML has \
             nothing of them",
            stated - live
        ),
    ]
}

/// The `warning:` lines of a run's stderr.
fn warnings(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect()
}

/// Run the CLI on `pid` into `out_dir`: exit code and stderr.
fn publish(pid: &Path, out_dir: &Path) -> (Option<i32>, String) {
    let output = Command::new(CLI)
        .arg(pid)
        .arg("--out-dir")
        .arg(out_dir)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn pid_publish_xml");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `N of M label values reach an item`, off the CLI's summary line.
fn label_values_reached(stderr: &str) -> (usize, usize) {
    let tail = stderr
        .split("representations; ")
        .nth(1)
        .unwrap_or_else(|| panic!("no summary line in: {stderr}"));
    let mut words = tail.split_whitespace();
    let reached = words.next().and_then(|word| word.parse().ok());
    let of = words.nth(1).and_then(|word| word.parse().ok());
    reached
        .zip(of)
        .unwrap_or_else(|| panic!("no label counts in: {tail}"))
}

#[test]
fn a_chain_that_stops_short_is_said_on_stderr_and_the_exit_code_stays() {
    let scratch = Scratch::new("stop");
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let intact = package
        .get_stream(SHEET)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {SHEET}"))
        .data
        .clone();
    let starts = sheet_record_starts(&intact);
    let middle = starts[starts.len() / 2];
    let mut broken = intact.clone();
    broken[middle + 2..middle + 6].copy_from_slice(&0u32.to_le_bytes());
    let unread = broken.len() - middle;
    let lost = publishable_before(&intact, middle);
    assert!(lost > 0, "{SHEET} holds records the publish reader keeps");
    let [read_clause, unread_clause] =
        left_without(lost, live_before(&intact, middle), stated_live(&intact));
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: SHEET.to_string(),
            new_data: broken.clone(),
        }],
        ..WritePlan::default()
    };
    let copy = scratch.0.join("broken.pid");
    std::fs::write(
        &copy,
        PidWriter::write_to_bytes(&package, &plan).expect("the damaged copy is written"),
    )
    .expect("the damaged copy lands in the scratch directory");

    // The intact drawing: delivered, and not a word about a chain.
    let (code, stderr) = publish(Path::new(FIXTURE), &scratch.0.join("intact"));
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        !stderr.lines().any(|line| line.starts_with("warning:")),
        "the intact drawing raises no warning: {stderr}"
    );
    let (reached, of) = label_values_reached(&stderr);
    assert!(reached > 0 && reached <= of, "{reached} of {of}");
    let read = read_publish_from_pid(Path::new(FIXTURE)).expect("the fixture reads");
    assert!(read.chain_faults.is_empty() && read.drawing.is_ok());

    // The damaged copy: the same exit code, a delivery that has lost what
    // the stream held, and one line that says so.
    let (code, stderr) = publish(&copy, &scratch.0.join("broken"));
    assert_eq!(code, Some(0), "the exit code does not change: {stderr}");
    let (broken_reached, broken_of) = label_values_reached(&stderr);
    assert_eq!(
        (broken_reached, broken_of),
        (0, of),
        "nothing of the stream's labels reaches an item: {stderr}"
    );
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();
    assert_eq!(warnings.len(), 1, "one warning: {stderr}");
    let warning = warnings[0];
    for needle in [
        copy.display().to_string(),
        format!("the record chain of {SHEET} does not hold"),
        format!("stops at +{middle} of {} bytes", broken.len()),
        "a record states a length of zero".to_string(),
        format!("{unread} byte(s) from it on are not"),
        "the publish reader takes nothing from a stream whose chain stops short".to_string(),
        read_clause,
        unread_clause,
    ] {
        assert!(warning.contains(&needle), "`{needle}` in: {warning}");
    }
    // A publisher draws nothing: the line says what the XML is short of
    // and leaves the page out of it.
    assert!(!warning.contains("draws"), "{warning}");

    // The library says the same fault the parser does, from the one read
    // that makes the drawing, with what the stream cost the reader.
    let read = read_publish_from_pid(&copy).expect("the copy reads");
    assert!(read.drawing.is_ok(), "the copy still states its drawing");
    assert_eq!(
        read.chain_faults,
        vec![PublishChainFault {
            fault: StreamChainFault::of_stream(SHEET, &broken).expect("the chain does not hold"),
            publishable_records_read: lost,
        }]
    );
    assert_eq!(
        warning,
        format!(
            "warning: {}: {}",
            copy.display(),
            read.chain_faults[0].describe()
        ),
        "the line is the library's sentence"
    );
}

/// A stream whose header counts one record more than its chain holds is
/// read whole by the publish reader — the chain does not stop — and is
/// still said: the count is the only thing that says records may be gone.
#[test]
fn a_header_that_disagrees_with_its_chain_is_said_and_the_stream_is_still_read() {
    let scratch = Scratch::new("count");
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let mut broken = package
        .get_stream(SHEET)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {SHEET}"))
        .data
        .clone();
    let stated = u32::from_le_bytes([broken[4], broken[5], broken[6], broken[7]]);
    broken[4..8].copy_from_slice(&(stated + 1).to_le_bytes());
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: SHEET.to_string(),
            new_data: broken,
        }],
        ..WritePlan::default()
    };
    let copy = scratch.0.join("count.pid");
    std::fs::write(
        &copy,
        PidWriter::write_to_bytes(&package, &plan).expect("the damaged copy is written"),
    )
    .expect("the damaged copy lands in the scratch directory");

    let (_, intact) = publish(Path::new(FIXTURE), &scratch.0.join("intact"));
    let (code, stderr) = publish(&copy, &scratch.0.join("count"));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(
        label_values_reached(&stderr),
        label_values_reached(&intact),
        "the stream is read as on the intact drawing"
    );
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();
    assert_eq!(warnings.len(), 1, "one warning: {stderr}");
    for needle in [
        format!("the record chain of {SHEET} does not hold"),
        format!(
            "{stated} live record(s) where the header states {}",
            stated + 1
        ),
        format!(
            "the publish reader read the {stated} live record(s) the chain reached; the header \
             states 1 more, which the stream does not hold"
        ),
    ] {
        assert!(
            warnings[0].contains(&needle),
            "`{needle}` in: {}",
            warnings[0]
        );
    }
    assert!(!warnings[0].contains("draws"), "{}", warnings[0]);
}

/// The other way round: a header that counts one record fewer than its
/// chain holds. The chain reaches the stream's end, so nothing is unread
/// and the stream is read as on the intact drawing; the line says how many
/// records the reader read and what the header states, and does not say
/// the header states "more, which the stream does not hold" — it states
/// fewer. Until 2026-11-12 no case held this branch of the sentence.
#[test]
fn a_header_that_counts_fewer_records_than_its_chain_holds_is_said_as_it_is() {
    let scratch = Scratch::new("count-fewer");
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let mut broken = package
        .get_stream(SHEET)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {SHEET}"))
        .data
        .clone();
    let stated = u32::from_le_bytes([broken[4], broken[5], broken[6], broken[7]]);
    assert!(stated > 1, "{SHEET} holds more than one live record");
    assert_eq!(
        live_before(&broken, broken.len()),
        stated as usize,
        "the fixture's header counts the live records its chain holds"
    );
    broken[4..8].copy_from_slice(&(stated - 1).to_le_bytes());
    let copy = copy_with_stream(&scratch.0, "count-fewer.pid", SHEET, broken.clone());

    let (_, intact) = publish(Path::new(FIXTURE), &scratch.0.join("intact"));
    let (code, stderr) = publish(&copy, &scratch.0.join("count-fewer"));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(
        label_values_reached(&stderr),
        label_values_reached(&intact),
        "the stream is read as on the intact drawing"
    );
    let warned = warnings(&stderr);
    assert_eq!(warned.len(), 1, "one warning: {stderr}");
    let said = format!(
        "the publish reader read the {stated} live record(s) the chain reached; the header \
         states {}",
        stated - 1
    );
    for needle in [
        copy.display().to_string(),
        format!("the record chain of {SHEET} does not hold"),
        format!(
            "{stated} live record(s) where the header states {}",
            stated - 1
        ),
        said.clone(),
    ] {
        assert!(warned[0].contains(&needle), "`{needle}` in: {}", warned[0]);
    }
    assert!(
        warned[0].ends_with(&said),
        "the line ends with what the header states: {}",
        warned[0]
    );
    for not_said in ["more, which the stream does not hold", "draws", "stop"] {
        assert!(
            !warned[0].contains(not_said),
            "`{not_said}` in: {}",
            warned[0]
        );
    }

    // The library says the same fault, from the one read that makes the
    // drawing: a chain that reaches the end, with a count that differs.
    let read = read_publish_from_pid(&copy).expect("the copy reads");
    assert!(read.drawing.is_ok(), "the copy still states its drawing");
    assert_eq!(read.chain_faults.len(), 1, "{:?}", read.chain_faults);
    let fault = &read.chain_faults[0];
    assert_eq!(
        fault.fault,
        StreamChainFault::of_stream(SHEET, &broken).expect("the count differs")
    );
    assert!(fault.fault.stop.is_none() && fault.fault.count_differs());
    assert_eq!(
        (fault.fault.live_records_read, fault.fault.stated_records),
        (stated as usize, Some(stated - 1))
    );
    assert_eq!(
        warned[0],
        format!("warning: {}: {}", copy.display(), fault.describe()),
        "the line is the library's sentence"
    );
}

/// A style table holds no record the publish reader keeps. One whose chain
/// stops is still said — the file is damaged — and the line says that
/// nothing the chain read is missing from the XML, which is the intact
/// drawing's byte for byte.
#[test]
fn a_style_table_whose_chain_stops_is_said_and_costs_the_xml_nothing() {
    const STYLES: &str = "/StyleCluster";
    let scratch = Scratch::new("styles");
    let (intact, broken, middle) = middle_length_zeroed(STYLES);
    assert_eq!(
        publishable_before(&intact, intact.len()),
        0,
        "{STYLES} holds no record the publish reader keeps"
    );
    // The two copies carry one file name, so that the drawing name the XML
    // states is the same on both.
    let sound = copy_with_stream(
        &scratch.0.join("sound"),
        "drawing.pid",
        STYLES,
        intact.clone(),
    );
    let damaged = copy_with_stream(&scratch.0.join("damaged"), "drawing.pid", STYLES, broken);

    let (code, stderr) = publish(&sound, &scratch.0.join("sound-out"));
    assert_eq!(code, Some(0), "{stderr}");
    assert!(warnings(&stderr).is_empty(), "{stderr}");
    let summary = label_values_reached(&stderr);

    let (code, stderr) = publish(&damaged, &scratch.0.join("damaged-out"));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(label_values_reached(&stderr), summary, "{stderr}");
    let lines = warnings(&stderr);
    assert_eq!(lines.len(), 1, "one warning: {stderr}");
    let live = live_before(&intact, middle);
    assert!(live > 0, "the table's chain reads styles before the stop");
    let [read_clause, unread_clause] = left_without(0, live, stated_live(&intact));
    for needle in [
        format!("the record chain of {STYLES} does not hold"),
        format!("stops at +{middle} of"),
        "the publish reader takes nothing from a stream whose chain stops short".to_string(),
        read_clause,
        unread_clause,
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
    }
    assert!(
        !lines[0].contains("none of them is in the XML") && !lines[0].contains("draws"),
        "nothing the chain read is said to be missing: {}",
        lines[0]
    );

    let mut compared = 0;
    for entry in std::fs::read_dir(scratch.0.join("sound-out")).expect("the sound delivery") {
        let entry = entry.expect("a delivered file");
        let name = entry.file_name();
        if !name.to_string_lossy().ends_with(".xml") {
            continue;
        }
        let sound = std::fs::read(entry.path()).expect("the sound file");
        let damaged = std::fs::read(scratch.0.join("damaged-out").join(&name))
            .unwrap_or_else(|error| panic!("{name:?} of the damaged copy: {error}"));
        assert!(sound == damaged, "{name:?} is the intact drawing's");
        compared += 1;
    }
    assert_eq!(compared, 2, "the data and the meta XML");
}

/// The stream the drawing's own row is in: with its chain stopped the
/// reader has no `DrawingNo` and the run ends in an error. The warning is
/// said first, from the same read, so the error has its cause beside it.
#[test]
fn a_chain_fault_is_said_before_the_error_it_causes() {
    const ATTRIBUTES: &str = "/Unclustered Dynamic Attributes";
    let scratch = Scratch::new("attributes");
    let package = PidParser::new()
        .parse_package(FIXTURE)
        .unwrap_or_else(|error| panic!("{FIXTURE} should parse: {error}"));
    let intact = package
        .get_stream(ATTRIBUTES)
        .unwrap_or_else(|| panic!("{FIXTURE} has a {ATTRIBUTES}"))
        .data
        .clone();
    // The first record's length set to zero: the chain reads nothing.
    let first = sheet_record_starts(&intact)[0];
    let mut broken = intact.clone();
    broken[first + 2..first + 6].copy_from_slice(&0u32.to_le_bytes());
    let copy = copy_with_stream(&scratch.0, "attributes.pid", ATTRIBUTES, broken);

    let (code, stderr) = publish(&copy, &scratch.0.join("out"));
    assert_eq!(code, Some(1), "the drawing does not load: {stderr}");
    assert!(
        stderr.contains("no P&IDAttributes row carries a DrawingNo"),
        "the error the missing row raises: {stderr}"
    );
    let lines = warnings(&stderr);
    assert_eq!(lines.len(), 1, "one warning: {stderr}");
    // What the line says the reader is left without is what the run then
    // fails for: no live record read, and every one the header states —
    // the drawing's own row among them — unread.
    let stated = stated_live(&intact);
    assert!(stated > 0, "{ATTRIBUTES} states live records");
    let [read_clause, unread_clause] = left_without(0, 0, stated);
    for needle in [
        copy.display().to_string(),
        format!("the record chain of {ATTRIBUTES} does not hold"),
        format!("stops at +{first} of"),
        "the chain read 0 record(s) before the stop".to_string(),
        "the publish reader takes nothing from a stream whose chain stops short".to_string(),
        read_clause,
        unread_clause,
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
    }
    // This stream draws nothing, and the line used to say that nothing the
    // chain read was missing from the XML — in front of the error that the
    // XML could not be made at all.
    for wrong in ["draws", "nothing it read is missing", "missing nothing"] {
        assert!(!lines[0].contains(wrong), "`{wrong}` in: {}", lines[0]);
    }
    let warning_at = stderr.find("warning:").expect("the warning");
    let error_at = stderr.find("error:").expect("the error");
    assert!(
        warning_at < error_at,
        "the cause before the error: {stderr}"
    );

    // The library: the read succeeds, the drawing does not, and the fault
    // that explains it is beside the reason.
    let read = read_publish_from_pid(&copy).expect("the copy reads");
    assert!(
        matches!(
            read.drawing,
            Err(pid_parse::publish::PidLoadError::NoDrawing)
        ),
        "no drawing row was read"
    );
    assert_eq!(read.chain_faults.len(), 1);
    assert_eq!(read.chain_faults[0].fault.stream_path, ATTRIBUTES);
    assert_eq!(read.chain_faults[0].publishable_records_read, 0);
}

/// The same stream stopped in the middle: the chain read attribute rows
/// before the stop, and the reader, which takes a stream's records all or
/// nothing, has none of them. The line states that loss in the chain's own
/// numbers — how many live records it read, how many of them are rows the
/// reader keeps, how many more the header states — and the run fails for
/// exactly it: no row carries a `DrawingNo`.
#[test]
fn a_stopped_attribute_stream_states_the_rows_the_reader_is_left_without() {
    const ATTRIBUTES: &str = "/Unclustered Dynamic Attributes";
    let scratch = Scratch::new("attribute-rows");
    let (intact, broken, middle) = middle_length_zeroed(ATTRIBUTES);
    let live = live_before(&intact, middle);
    let lost = publishable_before(&intact, middle);
    let stated = stated_live(&intact);
    assert!(
        lost > 0 && lost <= live && live < stated,
        "{ATTRIBUTES}: {lost} row(s) the reader keeps among {live} live record(s) before the \
         stop, of {stated} the header states"
    );
    let copy = copy_with_stream(&scratch.0, "rows.pid", ATTRIBUTES, broken.clone());

    let (code, stderr) = publish(&copy, &scratch.0.join("out"));
    assert_eq!(code, Some(1), "the drawing does not load: {stderr}");
    assert!(
        stderr.contains("no P&IDAttributes row carries a DrawingNo"),
        "the error the missing rows raise: {stderr}"
    );
    let lines = warnings(&stderr);
    assert_eq!(lines.len(), 1, "one warning: {stderr}");
    let [read_clause, unread_clause] = left_without(lost, live, stated);
    for needle in [
        copy.display().to_string(),
        format!("the record chain of {ATTRIBUTES} does not hold"),
        format!("stops at +{middle} of {} bytes", broken.len()),
        format!("the stream's header states {stated} live record(s) and the chain read {live}"),
        "the publish reader takes nothing from a stream whose chain stops short".to_string(),
        read_clause,
        unread_clause,
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
    }
    for wrong in ["draws", "nothing it read is missing", "missing nothing"] {
        assert!(!lines[0].contains(wrong), "`{wrong}` in: {}", lines[0]);
    }

    let read = read_publish_from_pid(&copy).expect("the copy reads");
    assert!(
        matches!(
            read.drawing,
            Err(pid_parse::publish::PidLoadError::NoDrawing)
        ),
        "no drawing row was read"
    );
    assert_eq!(
        read.chain_faults,
        vec![PublishChainFault {
            fault: StreamChainFault::of_stream(ATTRIBUTES, &broken)
                .expect("the chain does not hold"),
            publishable_records_read: lost,
        }]
    );
    assert_eq!(
        lines[0],
        format!(
            "warning: {}: {}",
            copy.display(),
            read.chain_faults[0].describe()
        )
    );
}

/// One stored ZIP member, as `tests/publish_from_pid.rs` builds its mini
/// packages: the backup publisher inflates this the way it inflates a
/// `PlantData~*.zip`.
fn stored_zip(name: &str, payload: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }
    let name = name.as_bytes();
    let crc = crc32(payload);
    let size = u32::try_from(payload.len()).expect("member fits in a zip32 size");
    let name_len = u16::try_from(name.len()).expect("name fits in a zip name length");
    let mut out = Vec::new();
    let push16 = |out: &mut Vec<u8>, value: u16| out.extend_from_slice(&value.to_le_bytes());
    let push32 = |out: &mut Vec<u8>, value: u32| out.extend_from_slice(&value.to_le_bytes());
    push32(&mut out, 0x0403_4b50);
    push16(&mut out, 20);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, crc);
    push32(&mut out, size);
    push32(&mut out, size);
    push16(&mut out, name_len);
    push16(&mut out, 0);
    out.extend_from_slice(name);
    out.extend_from_slice(payload);
    let cd_at = u32::try_from(out.len()).expect("zip offset");
    push32(&mut out, 0x0201_4b50);
    push16(&mut out, 20);
    push16(&mut out, 20);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, crc);
    push32(&mut out, size);
    push32(&mut out, size);
    push16(&mut out, name_len);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, 0);
    push32(&mut out, 0);
    out.extend_from_slice(name);
    let cd_size = u32::try_from(out.len()).expect("zip size") - cd_at;
    push32(&mut out, 0x0605_4b50);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 1);
    push16(&mut out, 1);
    push32(&mut out, cd_size);
    push32(&mut out, cd_at);
    push16(&mut out, 0);
    out
}

/// The top-level keys of a JSON object file, sorted.
fn json_keys(path: &Path) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path:?}: {error}")),
    )
    .unwrap_or_else(|error| panic!("{path:?}: {error}"));
    let mut keys: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("{path:?} is an object"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// A backup package of two drawings, one of them with a `/Sheet6` whose
/// chain stops: the run publishes both, exits 0, and says on stderr which
/// drawing lost what — one line, naming the zip and the entry. Neither the
/// index nor a coverage report gains a key for it; the index lists the
/// same sentence among its `problems`, where the sound package's index
/// lists nothing.
#[test]
fn a_package_names_the_drawing_whose_chain_stops() {
    const SOUND: &str = "test-file/D06.pid";
    let scratch = Scratch::new("package");
    let (intact, broken, middle) = middle_length_zeroed(SHEET);
    let lost = publishable_before(&intact, middle);
    let [read_clause, unread_clause] =
        left_without(lost, live_before(&intact, middle), stated_live(&intact));
    let damaged = copy_with_stream(
        &scratch.0.join("copies"),
        "broken.pid",
        SHEET,
        broken.clone(),
    );
    let damaged = std::fs::read(damaged).expect("the damaged copy");

    // Two packages that differ in one drawing's one length field.
    let package = |name: &str, second: &[u8]| {
        let dir = scratch.0.join(name);
        std::fs::create_dir_all(&dir).expect("the package directory");
        std::fs::write(dir.join("Manifest.txt"), "BackupType<<|>>2\n").expect("manifest");
        std::fs::write(
            dir.join("PlantData~2~1.zip"),
            stored_zip("U01/D06.pid", &std::fs::read(SOUND).expect("D06.pid")),
        )
        .expect("the first zip");
        std::fs::write(
            dir.join("PlantData~2~2.zip"),
            stored_zip("U01/DWG-0201.pid", second),
        )
        .expect("the second zip");
        dir
    };
    let sound_package = package("sound", &std::fs::read(FIXTURE).expect("the fixture"));
    let damaged_package = package("damaged", &damaged);

    let (code, stderr) = publish(&sound_package, &scratch.0.join("sound-out"));
    assert_eq!(code, Some(0), "{stderr}");
    assert!(warnings(&stderr).is_empty(), "a sound package: {stderr}");
    assert!(stderr.contains("Published 2 drawings"), "{stderr}");

    let (code, stderr) = publish(&damaged_package, &scratch.0.join("damaged-out"));
    assert_eq!(code, Some(0), "the exit code does not change: {stderr}");
    assert!(
        stderr.contains("Published 2 drawings") && stderr.contains("(0 failed"),
        "{stderr}"
    );
    let lines = warnings(&stderr);
    assert_eq!(lines.len(), 1, "one warning: {stderr}");
    for needle in [
        "PlantData~2~2.zip U01/DWG-0201.pid".to_string(),
        format!("the record chain of {SHEET} does not hold"),
        format!("stops at +{middle} of {} bytes", broken.len()),
        read_clause,
        unread_clause,
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
    }
    assert!(
        !lines[0].contains("D06.pid") && !lines[0].contains("draws"),
        "the sound drawing is not named, and no page is spoken of: {}",
        lines[0]
    );

    // The damaged drawing is delivered, and its delivery is not the sound
    // one's: without the stream's text boxes, dependencies and placements
    // no label value reaches an item, and its items are written without
    // what those would have told (the XML is the longer for it).
    let delivered = |out: &str, file: &str| {
        std::fs::read(scratch.0.join(out).join(file))
            .unwrap_or_else(|error| panic!("{out}/{file}: {error}"))
    };
    assert!(
        delivered("damaged-out", "DWG-0201_Data.xml")
            != delivered("sound-out", "DWG-0201_Data.xml"),
        "the damaged drawing's XML is not the sound drawing's"
    );
    assert!(
        delivered("damaged-out", "D06_Data.xml") == delivered("sound-out", "D06_Data.xml"),
        "the other drawing is untouched"
    );

    // The index and the coverage reports keep their keys. The index lists
    // the drawing and its stream among its problems, in the sentence stderr
    // carries; it still counts the drawing as written, not as failed. The
    // sound package's index lists no problem.
    for file in ["_Index.json", "DWG-0201_Coverage.json", "D06_Coverage.json"] {
        assert_eq!(
            json_keys(&scratch.0.join("damaged-out").join(file)),
            json_keys(&scratch.0.join("sound-out").join(file)),
            "{file} keeps its keys"
        );
    }
    let index = |out: &str| -> serde_json::Value {
        serde_json::from_str(
            &std::fs::read_to_string(scratch.0.join(out).join("_Index.json")).expect("the index"),
        )
        .expect("the index is JSON")
    };
    let problems = |index: &serde_json::Value| -> Vec<String> {
        index["problems"]
            .as_array()
            .expect("the index lists its problems")
            .iter()
            .map(|problem| {
                problem
                    .as_str()
                    .expect("a problem is a sentence")
                    .to_string()
            })
            .collect()
    };
    let (sound_index, damaged_index) = (index("sound-out"), index("damaged-out"));
    assert_eq!(
        problems(&sound_index),
        Vec::<String>::new(),
        "a sound package's index lists no problem"
    );
    let listed = problems(&damaged_index);
    assert_eq!(
        listed,
        vec![lines[0]
            .strip_prefix("warning: ")
            .expect("a warning line")
            .to_string()],
        "the index lists the fault, in the words stderr carries"
    );
    for needle in [
        "PlantData~2~2.zip U01/DWG-0201.pid",
        "the record chain of /Sheet6 does not hold",
    ] {
        assert!(listed[0].contains(needle), "`{needle}` in: {}", listed[0]);
    }
    assert_eq!(
        (&damaged_index["failed"], &damaged_index["pid_entries"]),
        (&serde_json::json!(0), &serde_json::json!(2)),
        "the drawing is written, and the index counts no failure"
    );

    // The library reports the same fault, with the drawing it belongs to.
    let report = pid_parse::publish::backup_package::publish_backup_package(
        &damaged_package,
        &scratch.0.join("library-out"),
        "P01",
        None,
    )
    .expect("the package publishes");
    assert_eq!((report.written, report.failed), (2, 0));
    assert_eq!(report.chain_faults.len(), 1);
    let fault = &report.chain_faults[0];
    assert_eq!(
        (fault.zip_file.as_str(), fault.zip_entry.as_str()),
        ("PlantData~2~2.zip", "U01/DWG-0201.pid")
    );
    assert_eq!(
        fault.fault,
        PublishChainFault {
            fault: StreamChainFault::of_stream(SHEET, &broken).expect("the chain does not hold"),
            publishable_records_read: lost,
        }
    );
    assert_eq!(lines[0], format!("warning: {}", fault.describe()));
    let sound = pid_parse::publish::backup_package::publish_backup_package(
        &sound_package,
        &scratch.0.join("library-sound-out"),
        "P01",
        None,
    )
    .expect("the package publishes");
    assert!(sound.chain_faults.is_empty());
    // The drawings of both packages are read from the `.pid`: none goes
    // through the database line, so none is listed as written short by it.
    assert!(report.shortfalls.is_empty() && sound.shortfalls.is_empty());
    // The documents written from them state each UID once, the damaged
    // drawing's too: no drawing is listed for a UID written twice.
    assert!(report.duplicate_uids.is_empty() && sound.duplicate_uids.is_empty());
}
