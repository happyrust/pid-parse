//! End-to-end tests for the `pid_writer_validate` binary.
//!
//! Spawns the freshly-built binary via `env!("CARGO_BIN_EXE_…")` (no
//! third-party `assert_cmd` / `escargot` dependency required) and
//! verifies exit codes + the human / JSON outputs against an in-memory
//! synthetic CFB fixture.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";
const SHEET_PATH: &str = "/PlainSheet/Sheet1";

static FIXTURE_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_temp(name: &str) -> PathBuf {
    let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("pid-validate-cli-{pid}-{n}-{name}.pid"))
}

fn build_fixture(path: &PathBuf) {
    if path.exists() {
        std::fs::remove_file(path).expect("clean fixture");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("ensure tmp parent");
    }
    let mut cfb = ::cfb::create(path).expect("create fixture cfb");
    cfb.create_storage("/TaggedTxtData").unwrap();
    cfb.create_storage("/PlainSheet").unwrap();

    let drawing =
        b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>";
    let mut s = cfb.create_stream(DRAWING_PATH).unwrap();
    s.write_all(drawing).unwrap();
    drop(s);

    let general =
        b"<?xml version=\"1.0\"?><General><FilePath>C:/fixture.pid</FilePath></General>";
    let mut s = cfb.create_stream(GENERAL_PATH).unwrap();
    s.write_all(general).unwrap();
    drop(s);

    let sheet: Vec<u8> = (0u8..16).collect();
    let mut s = cfb.create_stream(SHEET_PATH).unwrap();
    s.write_all(&sheet).unwrap();
    drop(s);

    cfb.flush().unwrap();
}

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_writer_validate")
}

#[test]
fn validate_passes_on_synthetic_fixture_human_format() {
    let src = unique_temp("happy-src");
    let dst = unique_temp("happy-dst");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .output()
        .expect("spawn pid_writer_validate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    assert!(stdout.contains("Result: PASS"), "expected PASS in stdout: {stdout}");
    assert!(
        stdout.contains("0 mismatched"),
        "expected '0 mismatched' summary in stdout: {stdout}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_emits_json_when_flag_set() {
    let src = unique_temp("json-src");
    let dst = unique_temp("json-dst");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .output()
        .expect("spawn pid_writer_validate --json");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit failed: {stdout}");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert_eq!(parsed["mismatched"], serde_json::json!(0));
    let matched = parsed["matched"].as_u64().expect("matched is integer");
    assert_eq!(matched, 3, "fixture has exactly 3 streams");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_help_flag_exits_zero_with_usage() {
    let output = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn --help");
    assert!(output.status.success(), "--help should be exit 0");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage: pid_writer_validate"),
        "expected usage text on stderr: {stderr}"
    );
}

#[test]
fn validate_missing_input_exits_with_parse_error_code() {
    let bogus = unique_temp("nonexistent");
    let output = Command::new(binary_path())
        .arg(&bogus)
        .output()
        .expect("spawn with nonexistent input");
    // Source parse failure → exit 2 per CLI contract.
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for parse failure; got {:?}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn validate_with_edit_drawing_attribute_passes_and_marks_edited() {
    let src = unique_temp("edit-draw-src");
    let dst = unique_temp("edit-draw-dst");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .args(["--edit", "SP_DRAWINGNUMBER=NEW-007"])
        .output()
        .expect("spawn pid_writer_validate --edit");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}; stderr={stderr}; stdout={stdout}",
        output.status.code()
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert_eq!(parsed["mismatched"], serde_json::json!(0));
    assert_eq!(parsed["edited"], serde_json::json!(1), "Drawing stream edited");
    assert_eq!(parsed["matched"], serde_json::json!(2), "General + Sheet untouched");
    let edits = parsed["edits_applied"].as_array().expect("edits array");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["key"], serde_json::json!("SP_DRAWINGNUMBER"));
    assert_eq!(edits[0]["value"], serde_json::json!("NEW-007"));

    // Verify the actual file on disk has the new value.
    let bytes = std::fs::read(&dst).expect("read out");
    assert!(
        // Quick byte search: the new value substring must appear.
        bytes.windows(b"NEW-007".len()).any(|w| w == b"NEW-007"),
        "edited value not found in output bytes"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_with_general_edit_passes() {
    let src = unique_temp("edit-gen-src");
    let dst = unique_temp("edit-gen-dst");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .args(["--general-edit", "FilePath=D:/issued/edited.pid"])
        .output()
        .expect("spawn --general-edit");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit failed: {stdout}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert_eq!(parsed["edited"], serde_json::json!(1));
    assert_eq!(parsed["matched"], serde_json::json!(2));

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_edit_with_unknown_attr_exits_with_edit_error() {
    let src = unique_temp("edit-unknown-src");
    let dst = unique_temp("edit-unknown-dst");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .args(["--edit", "SP_NOSUCH=val"])
        .output()
        .expect("spawn --edit unknown");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown attr should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--edit SP_NOSUCH") || stderr.contains("not found"),
        "stderr should explain the unknown attr; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    // dst likely never created; ignore.
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_edit_argument_malformed_exits_with_argument_error() {
    let src = unique_temp("edit-malformed-src");
    build_fixture(&src);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--edit", "missing-equals-sign"])
        .output()
        .expect("spawn --edit malformed");

    assert_eq!(
        output.status.code(),
        Some(1),
        "malformed --edit should be a usage error (exit 1); got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ATTR=VALUE"),
        "stderr must show ATTR=VALUE hint; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
}
