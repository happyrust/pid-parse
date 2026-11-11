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

use std::path::{Path, PathBuf};
use std::process::Command;

use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::publish::pid_load::chain_faults_from_pid;
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
    assert!(chain_faults_from_pid(Path::new(FIXTURE))
        .expect("the fixture reads")
        .is_empty());

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
    ] {
        assert!(warning.contains(&needle), "`{needle}` in: {warning}");
    }

    // The library says the same fault the parser does.
    assert_eq!(
        chain_faults_from_pid(&copy).expect("the copy reads"),
        vec![StreamChainFault::of_stream(SHEET, &broken).expect("the chain does not hold")]
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
