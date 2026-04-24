//! End-to-end tests for the `pid_writer_validate` binary.
//!
//! Spawns the freshly-built binary via `env!("CARGO_BIN_EXE_…")` (no
//! third-party `assert_cmd` / `escargot` dependency required) and
//! verifies exit codes + the human / JSON outputs against an in-memory
//! synthetic CFB fixture.

use std::io::{Read as _, Write as _};
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

    let drawing = b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>";
    let mut s = cfb.create_stream(DRAWING_PATH).unwrap();
    s.write_all(drawing).unwrap();
    drop(s);

    let general = b"<?xml version=\"1.0\"?><General><FilePath>C:/fixture.pid</FilePath></General>";
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
    assert!(
        stdout.contains("Result: PASS"),
        "expected PASS in stdout: {stdout}"
    );
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
    assert_eq!(
        parsed["edited"],
        serde_json::json!(1),
        "Drawing stream edited"
    );
    assert_eq!(
        parsed["matched"],
        serde_json::json!(2),
        "General + Sheet untouched"
    );
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
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
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

// --- --apply-plan tests (W3) ---------------------------------------------

fn write_plan(path: &PathBuf, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("ensure plan parent");
    }
    std::fs::write(path, body).expect("write plan.json");
}

fn plan_path(name: &str) -> PathBuf {
    let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("pid-validate-plan-{pid}-{n}-{name}.json"))
}

#[test]
fn validate_apply_plan_passthrough_empty_plan_passes() {
    let src = unique_temp("plan-empty-src");
    let dst = unique_temp("plan-empty-dst");
    let plan = plan_path("empty");
    build_fixture(&src);
    write_plan(&plan, "{}");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .output()
        .expect("spawn --apply-plan (empty)");

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
    assert_eq!(parsed["edited"], serde_json::json!(0));
    assert_eq!(parsed["matched"], serde_json::json!(3));
    assert!(
        parsed.get("plan_applied").is_some(),
        "plan_applied populated"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
    let _ = std::fs::remove_file(&plan);
}

#[test]
fn validate_apply_plan_drawing_metadata_update_rewrites_stream() {
    let src = unique_temp("plan-meta-src");
    let dst = unique_temp("plan-meta-dst");
    let plan = plan_path("meta");
    build_fixture(&src);
    // Replace the Drawing XML body wholesale — plan-level metadata update.
    let new_drawing =
        r#"<?xml version="1.0"?><Drawing><Tag SP_DRAWINGNUMBER="PLAN-777"/></Drawing>"#;
    // Escape for JSON string literal (quotes only; no newlines).
    let escaped = new_drawing.replace('"', "\\\"");
    let body = format!(
        r#"{{"metadata_updates":{{"drawing_xml":"{escaped}","general_xml":null,"summary_updates":{{}}}},"stream_replacements":[],"sheet_patches":[]}}"#
    );
    write_plan(&plan, &body);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .output()
        .expect("spawn --apply-plan meta");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}; stderr={stderr}; stdout={stdout}",
        output.status.code()
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert_eq!(
        parsed["edited"],
        serde_json::json!(1),
        "Drawing stream edited"
    );
    assert_eq!(
        parsed["matched"],
        serde_json::json!(2),
        "General + Sheet untouched"
    );

    // On-disk verification: the new value must appear in bytes.
    let bytes = std::fs::read(&dst).expect("read out");
    assert!(
        bytes.windows(b"PLAN-777".len()).any(|w| w == b"PLAN-777"),
        "expected PLAN-777 in output file"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
    let _ = std::fs::remove_file(&plan);
}

#[test]
fn validate_apply_plan_replaces_sheet_stream_via_base64_payload() {
    let src = unique_temp("plan-repl-src");
    let dst = unique_temp("plan-repl-dst");
    let plan = plan_path("repl");
    build_fixture(&src);
    // base64("ABC") = "QUJD" — payload is the exact ASCII bytes of the string.
    let body = r#"{
        "metadata_updates": {"drawing_xml": null, "general_xml": null, "summary_updates": {}},
        "stream_replacements": [
            {"path": "/PlainSheet/Sheet1", "new_data": "QUJD"}
        ],
        "sheet_patches": []
    }"#;
    write_plan(&plan, body);

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .arg("--json")
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .output()
        .expect("spawn --apply-plan repl");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}; stderr={stderr}; stdout={stdout}",
        output.status.code()
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert_eq!(parsed["edited"], serde_json::json!(1), "Sheet replaced");
    assert_eq!(parsed["matched"], serde_json::json!(2));

    // The Sheet stream in the output file should now contain exactly "ABC".
    let mut cfb = ::cfb::open(&dst).expect("reopen");
    let mut buf = Vec::new();
    cfb.open_stream("/PlainSheet/Sheet1")
        .expect("open Sheet1")
        .read_to_end(&mut buf)
        .unwrap();
    assert_eq!(buf, b"ABC", "Sheet1 should be exactly the replaced payload");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
    let _ = std::fs::remove_file(&plan);
}

#[test]
fn validate_apply_plan_invalid_json_exits_two() {
    let src = unique_temp("plan-bad-src");
    let plan = plan_path("bad");
    build_fixture(&src);
    write_plan(&plan, "this is not json at all");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .output()
        .expect("spawn --apply-plan bad-json");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid plan.json should exit 2 (source/plan load error); got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to load --apply-plan") || stderr.contains("WritePlan JSON"),
        "stderr must mention the plan-load failure; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&plan);
}

#[test]
fn validate_apply_plan_conflicts_with_edit_exits_one() {
    let src = unique_temp("plan-conflict-src");
    let plan = plan_path("conflict");
    build_fixture(&src);
    write_plan(&plan, "{}");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .args(["--edit", "SP_DRAWINGNUMBER=X"])
        .output()
        .expect("spawn --apply-plan+--edit");

    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting flags should be a usage error (exit 1); got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be combined"),
        "stderr must explain the conflict; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&plan);
}

// ---------------------------------------------------------------------
// Phase 9m: --set-summary integration tests.
// ---------------------------------------------------------------------

const SUMMARY_PATH: &str = "/\u{5}SummaryInformation";

/// Build a minimal valid `/\u{5}SummaryInformation` property-set stream
/// carrying a single `VT_LPSTR` `title` property. Mirrors the helper in
/// `writer_roundtrip.rs`; kept local to this file so the integration
/// layer does not take a test-fixture dependency on another test file.
fn minimal_summary_info_bytes(title_ascii: &str) -> Vec<u8> {
    const FMTID_SUMMARY: [u8; 16] = [
        0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3,
        0xD9,
    ];
    const VT_LPSTR: u16 = 0x001E;
    let mut title_bytes = title_ascii.as_bytes().to_vec();
    title_bytes.push(0);
    let mut typed_value = Vec::new();
    typed_value.extend_from_slice(&(VT_LPSTR as u32).to_le_bytes());
    typed_value.extend_from_slice(&(title_bytes.len() as u32).to_le_bytes());
    typed_value.extend_from_slice(&title_bytes);
    while !typed_value.len().is_multiple_of(4) {
        typed_value.push(0);
    }
    let table_size: usize = 8 + 8;
    let prop_offset = table_size as u32;
    let section_size = (table_size + typed_value.len()) as u32;
    let mut section = Vec::new();
    section.extend_from_slice(&section_size.to_le_bytes());
    section.extend_from_slice(&1u32.to_le_bytes());
    section.extend_from_slice(&2u32.to_le_bytes()); // PID_TITLE
    section.extend_from_slice(&prop_offset.to_le_bytes());
    section.extend_from_slice(&typed_value);

    let mut stream = Vec::new();
    stream.extend_from_slice(&0xFFFEu16.to_le_bytes());
    stream.extend_from_slice(&0u16.to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());
    stream.extend_from_slice(&[0u8; 16]);
    stream.extend_from_slice(&1u32.to_le_bytes());
    stream.extend_from_slice(&FMTID_SUMMARY);
    stream.extend_from_slice(&48u32.to_le_bytes());
    stream.extend_from_slice(&section);
    stream
}

/// Same as [`build_fixture`] but additionally inserts a
/// `/\u{5}SummaryInformation` stream with the given ASCII title.
fn build_fixture_with_summary(path: &PathBuf, title: &str) {
    build_fixture(path);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("open rw");
    let mut cfb = ::cfb::CompoundFile::open(file).expect("open cfb");
    let mut s = cfb.create_stream(SUMMARY_PATH).expect("create summary");
    s.write_all(&minimal_summary_info_bytes(title))
        .expect("write summary");
    drop(s);
    cfb.flush().expect("flush");
}

/// Given a path to a round-trip output .pid, open and return the parsed
/// `SummaryInfo.title` (or `None` if the stream is missing).
fn read_title_from_pid(pid_path: &std::path::Path) -> Option<String> {
    let parser = pid_parse::PidParser::new();
    let pkg = parser.parse_package(pid_path).expect("reparse dst");
    pkg.parsed.summary.as_ref().and_then(|s| s.title.clone())
}

#[test]
fn validate_set_summary_single_key_rewrites_title() {
    let src = unique_temp("set-summary-src");
    let dst = unique_temp("set-summary-dst");
    build_fixture_with_summary(&src, "Original Title");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "title=CLI-Rewritten"])
        .output()
        .expect("spawn --set-summary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected exit 0; got {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    assert_eq!(
        read_title_from_pid(&dst).as_deref(),
        Some("CLI-Rewritten"),
        "output .pid's title must reflect the --set-summary edit",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_multiple_keys_accumulate() {
    let src = unique_temp("set-summary-multi-src");
    let dst = unique_temp("set-summary-multi-dst");
    build_fixture_with_summary(&src, "OldTitle");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "title=T1"])
        .args(["--set-summary", "author=Alice"])
        .args(["--set-summary", "subject=Review"])
        .output()
        .expect("spawn --set-summary x3");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected exit 0; got {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    // Title must be "T1" (the source title was "OldTitle" — rewrite
    // happened through our CLI).
    assert_eq!(
        read_title_from_pid(&dst).as_deref(),
        Some("T1"),
        "title should have been replaced to T1",
    );
    // The reader folds author/subject/title into `SummaryInfo.raw` under
    // short symbolic names (e.g. "Author", "Subject", "Title" — see
    // `src/streams/summary.rs::summary_prop_name`). Probe via that
    // back-door so we don't have to re-parse the property-set manually.
    let parser = pid_parse::PidParser::new();
    let pkg = parser.parse_package(&dst).expect("reparse");
    let raw = &pkg.parsed.summary.as_ref().expect("summary").raw;
    assert_eq!(
        raw.get("Author").map(String::as_str),
        Some("Alice"),
        "author should have been appended; raw map: {raw:?}",
    );
    assert_eq!(
        raw.get("Subject").map(String::as_str),
        Some("Review"),
        "subject should have been appended; raw map: {raw:?}",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_conflicts_with_apply_plan_exits_one() {
    let src = unique_temp("set-summary-conflict-src");
    let plan = plan_path("conflict-set-summary");
    build_fixture_with_summary(&src, "OldTitle");
    write_plan(&plan, "{}");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--apply-plan".as_ref(), plan.as_os_str()])
        .args(["--set-summary", "title=X"])
        .output()
        .expect("spawn --apply-plan+--set-summary");

    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting flags should be a usage error (exit 1); got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be combined"),
        "stderr must explain the conflict; got: {stderr}"
    );
    assert!(
        stderr.contains("--set-summary"),
        "conflict message should mention --set-summary; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&plan);
}

#[test]
fn validate_set_summary_unknown_key_exits_two_with_clear_error() {
    let src = unique_temp("set-summary-unknown-src");
    let dst = unique_temp("set-summary-unknown-dst");
    build_fixture_with_summary(&src, "OldTitle");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "not_a_real_key=whatever"])
        .output()
        .expect("spawn --set-summary unknown");

    assert_eq!(
        output.status.code(),
        Some(2),
        "writer-layer error should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown key"),
        "stderr must mention the unknown-key error; got: {stderr}"
    );
    assert!(
        stderr.contains("not_a_real_key"),
        "stderr must quote the offending key; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

// ---------------------------------------------------------------------
// Phase 9n: --delete-summary integration tests.
// ---------------------------------------------------------------------

#[test]
fn validate_delete_summary_removes_target_prop() {
    let src = unique_temp("del-summary-src");
    let dst = unique_temp("del-summary-dst");
    build_fixture_with_summary(&src, "To-Be-Deleted");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--delete-summary", "title"])
        .output()
        .expect("spawn --delete-summary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected exit 0; got {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    // After delete, the reader's title field must be None.
    assert_eq!(
        read_title_from_pid(&dst),
        None,
        "title must be gone after --delete-summary title",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_delete_and_set_summary_combine_legally() {
    // Combine: set a new title + delete author (author is not in the
    // fixture, so it's a silent no-op deletion; we care that the command
    // does not spuriously fail).
    let src = unique_temp("del-set-summary-src");
    let dst = unique_temp("del-set-summary-dst");
    build_fixture_with_summary(&src, "OldTitle");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "title=FreshTitle"])
        .args(["--delete-summary", "author"])
        .output()
        .expect("spawn --set --delete combined");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected exit 0; got {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    assert_eq!(
        read_title_from_pid(&dst).as_deref(),
        Some("FreshTitle"),
        "title should have been replaced",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_delete_summary_conflicts_with_set_summary_on_same_key() {
    let src = unique_temp("del-set-conflict-src");
    let dst = unique_temp("del-set-conflict-dst");
    build_fixture_with_summary(&src, "OldTitle");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "title=X"])
        .args(["--delete-summary", "title"])
        .output()
        .expect("spawn conflict");

    assert_eq!(
        output.status.code(),
        Some(2),
        "writer-layer conflict should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--set-summary and --delete-summary both target key 'title'"),
        "stderr must explain the key conflict; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_delete_summary_unknown_key_exits_two() {
    let src = unique_temp("del-unknown-src");
    let dst = unique_temp("del-unknown-dst");
    build_fixture_with_summary(&src, "Orig");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--delete-summary", "nonexistent_key"])
        .output()
        .expect("spawn unknown delete");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown key on delete should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown key"), "got: {stderr}");
    assert!(stderr.contains("nonexistent_key"), "got: {stderr}");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

// ---------------------------------------------------------------------
// Phase 10i: --set-summary-encoded integration tests.
// ---------------------------------------------------------------------

#[test]
fn validate_set_summary_encoded_ascii_round_trips_with_explicit_codepage() {
    // For ASCII-only strings, CP1252 and UTF-8 byte layouts coincide, so
    // the end-to-end title read-back works even before Phase 10k (reader
    // code-page detection). Full non-ASCII CP1252 / GBK round-trip is
    // already covered by the byte-level writer unit tests
    // (`encode_lpstr_with_cp1252_preserves_western_european_bytes` etc.).
    let src = unique_temp("set-summary-enc-ascii-src");
    let dst = unique_temp("set-summary-enc-ascii-dst");
    build_fixture_with_summary(&src, "OldTitle");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args([
            "--set-summary-encoded",
            "title:windows-1252=Encoded-ASCII-Title",
        ])
        .output()
        .expect("spawn --set-summary-encoded");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected exit 0; got {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    assert_eq!(
        read_title_from_pid(&dst).as_deref(),
        Some("Encoded-ASCII-Title"),
        "CP1252-encoded ASCII title must round-trip through the reader",
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_encoded_rejects_lossy_cp1252_input() {
    // CP1252 cannot represent Chinese; writer must fail fast so the CLI
    // exits 2 rather than silently mojibake'ing the property.
    let src = unique_temp("set-summary-enc-lossy-src");
    let dst = unique_temp("set-summary-enc-lossy-dst");
    build_fixture_with_summary(&src, "Orig");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary-encoded", "title:windows-1252=公司"])
        .output()
        .expect("spawn --set-summary-encoded lossy");

    assert_eq!(
        output.status.code(),
        Some(2),
        "lossy encoding should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot encode"), "got: {stderr}");
    assert!(stderr.contains("windows-1252"), "got: {stderr}");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_encoded_rejects_unknown_encoding_label() {
    let src = unique_temp("set-summary-enc-unknown-src");
    let dst = unique_temp("set-summary-enc-unknown-dst");
    build_fixture_with_summary(&src, "Orig");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary-encoded", "title:Klingon-1=whatever"])
        .output()
        .expect("spawn --set-summary-encoded unknown enc");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown encoding label should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown encoding label"),
        "stderr must explain unknown encoding; got: {stderr}"
    );
    assert!(stderr.contains("Klingon-1"), "offending label: {stderr}");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_encoded_conflicts_with_set_summary_on_same_key() {
    // Phase 10i: two encoding semantics on the same key is ambiguous.
    let src = unique_temp("set-enc-conflict-src");
    let dst = unique_temp("set-enc-conflict-dst");
    build_fixture_with_summary(&src, "Orig");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary", "title=plain"])
        .args(["--set-summary-encoded", "title:UTF-8=encoded"])
        .output()
        .expect("spawn conflict");

    assert_eq!(
        output.status.code(),
        Some(2),
        "lib-layer conflict should exit 2; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--set-summary and --set-summary-encoded both target key 'title'"),
        "stderr must explain the conflict; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[test]
fn validate_set_summary_encoded_usage_error_on_missing_colon() {
    // Syntax guard: missing `:` between KEY and ENCODING should be a
    // usage error (exit 1), not a cryptic "unknown encoding".
    let src = unique_temp("set-enc-syntax-src");
    let dst = unique_temp("set-enc-syntax-dst");
    build_fixture_with_summary(&src, "Orig");

    let output = Command::new(binary_path())
        .arg(&src)
        .args(["--out".as_ref(), dst.as_os_str()])
        .arg("--keep")
        .args(["--set-summary-encoded", "title=plain"])
        .output()
        .expect("spawn bad syntax");

    assert_eq!(
        output.status.code(),
        Some(1),
        "usage/syntax error should exit 1; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("missing `:`"),
        "stderr must cite missing colon; got: {stderr}"
    );

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}
