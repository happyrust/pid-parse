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
        format!(
            "so the {lost} attribute row(s), text box(es), dependency object(s) and symbol \
             placement(s) the chain read before the stop are not in the XML"
        ),
    ] {
        assert!(warning.contains(&needle), "`{needle}` in: {warning}");
    }

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
        "the publish reader read the records the chain reached".to_string(),
    ] {
        assert!(
            warnings[0].contains(&needle),
            "`{needle}` in: {}",
            warnings[0]
        );
    }
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
    let sound = copy_with_stream(&scratch.0.join("sound"), "drawing.pid", STYLES, intact);
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
    for needle in [
        format!("the record chain of {STYLES} does not hold"),
        format!("stops at +{middle} of"),
        "the publish reader takes nothing from a stream whose chain stops short".to_string(),
        "the chain read no attribute row, text box, dependency object or symbol placement \
         before the stop, so nothing it read is missing from the XML"
            .to_string(),
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
    }
    assert!(
        !lines[0].contains("are not in the XML"),
        "nothing is said to be missing: {}",
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
    for needle in [
        copy.display().to_string(),
        format!("the record chain of {ATTRIBUTES} does not hold"),
        format!("stops at +{first} of"),
        "the chain read 0 record(s) before the stop".to_string(),
    ] {
        assert!(lines[0].contains(&needle), "`{needle}` in: {}", lines[0]);
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
