//! End-to-end tests for the `pid_inspect` binary (Phase 10a subset).
//!
//! We only cover the Phase 10a-introduced `--coverage` flag here. The rest
//! of the binary has historically been tested by pushing real fixtures
//! through `parse_real_files` and eyeballing; a future phase may expand
//! this file to full CLI parity.
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_inspect")
}

fn unique_tmp(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!(
        "pid-parse-inspect-cli-{}-{}-{}.pid",
        std::process::id(),
        label,
        nanos
    ));
    p
}

fn unique_tmp_dir(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!(
        "pid-parse-inspect-cli-{}-{}-{}",
        std::process::id(),
        label,
        nanos
    ));
    p
}

/// Produce a single legal 48-byte `DocVersion3` record so the parser's
/// `parse_doc_version3` heuristic accepts it and populates
/// `doc.version_history`. Without a valid record the Phase 10b dynamic
/// classifier would downgrade `DocVersion3` from `FullyDecoded` to
/// `IdentifiedOnly`, which is correct behavior but would prevent this
/// fixture from asserting the `[FULL]` path.
fn legal_doc_version3_record() -> Vec<u8> {
    let mut record = Vec::with_capacity(48);
    // product[16], zero-padded
    record.extend_from_slice(b"TestProduct");
    record.resize(16, 0);
    // version[12], zero-padded
    record.extend_from_slice(b"1.0.0");
    record.resize(16 + 12, 0);
    // operation[4], zero-padded ("SA" = SaveAs)
    record.extend_from_slice(b"SA");
    record.resize(16 + 12 + 4, 0);
    // timestamp[16], zero-padded
    record.extend_from_slice(b"01/01/26 00:00");
    record.resize(48, 0);
    record
}

/// Build a tiny valid CFB fixture with a mixture of coverage classes:
/// a fully-decoded stream (`DocVersion3` with a real record so the
/// dynamic classifier keeps it as `FullyDecoded`), a partially-decoded
/// stream (`PSMsegmenttable`), a storage-prefix stream (`Sheet1/Foo`),
/// and an unknown top-level stream (`MysteryStream`).
fn build_mixed_coverage_fixture(path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create fixture");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");
    cfb.create_storage_all("/Sheet1").expect("storage");

    let mut s = cfb.create_stream("/DocVersion3").unwrap();
    s.write_all(&legal_doc_version3_record()).unwrap();
    drop(s);

    // Use `/PSMcluster0` to hit the `[PART]` bucket. Phase 10b
    // intentionally does NOT wire a dynamic probe for it (cluster
    // streams share complex multi-stream model shapes parked for
    // Phase 10c), so the static `PartiallyDecoded` classification
    // stands regardless of parser-side population. `/PSMsegmenttable`
    // *does* have a dynamic probe and our bytes don't satisfy it, so
    // it would be downgraded to `IdentifiedOnly`; we avoid that
    // fixture-quality footgun here by using the un-probed name.
    let mut s = cfb.create_stream("/PSMcluster0").unwrap();
    s.write_all(b"any-bytes").unwrap();
    drop(s);

    let mut s = cfb.create_stream("/Sheet1/Foo").unwrap();
    s.write_all(b"sheet-bytes").unwrap();
    drop(s);

    let mut s = cfb.create_stream("/MysteryStream").unwrap();
    s.write_all(b"ghost-bytes").unwrap();
    drop(s);

    cfb.flush().unwrap();
}

fn build_sheet_probe_fixture(path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create fixture");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");
    let mut s = cfb.create_stream("/Sheet6").unwrap();
    s.write_all(&[0x11u8; 32]).unwrap();
    s.write_all(&[0x89, 0xCE, 0x00, 0xAA]).unwrap();
    s.write_all(b"ASCII-TAGS").unwrap();
    s.write_all(&[0x00, 0x00]).unwrap();
    for ch in "PUMP-101".encode_utf16() {
        s.write_all(&ch.to_le_bytes()).unwrap();
    }
    s.write_all(&1200i32.to_le_bytes()).unwrap();
    s.write_all(&(-450i32).to_le_bytes()).unwrap();
    s.write_all(&[0x22u8; 32]).unwrap();
    drop(s);
    cfb.flush().unwrap();
}

fn build_controlled_diff_fixture(path: &std::path::Path, sheet_payload: &[u8]) {
    let file = std::fs::File::create(path).expect("create controlled fixture");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");
    let mut s = cfb.create_stream("/Sheet6").unwrap();
    s.write_all(sheet_payload).unwrap();
    drop(s);
    cfb.create_storage_all("/TaggedTxtData").expect("storage");
    let mut s = cfb.create_stream("/TaggedTxtData/Drawing").unwrap();
    s.write_all(b"<Drawing><Template>UNIT_TEST</Template></Drawing>")
        .unwrap();
    drop(s);
    cfb.flush().unwrap();
}

#[test]
fn controlled_diff_dir_reports_stream_level_evidence() {
    let root = unique_tmp_dir("controlled-diff");
    let before_dir = root.join("before");
    let after_dir = root.join("after");
    let metadata_dir = root.join("metadata");
    std::fs::create_dir_all(&before_dir).expect("before dir");
    std::fs::create_dir_all(&after_dir).expect("after dir");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir");

    build_controlled_diff_fixture(&before_dir.join("one-line.pid"), b"before-sheet-bytes");
    build_controlled_diff_fixture(&after_dir.join("one-line.pid"), b"after-sheet-bytes");
    std::fs::write(
        metadata_dir.join("one-line.json"),
        r#"{"case":"one-line","operation":"place_line","expected":{"start":[0,0],"end":[1,0]}}"#,
    )
    .expect("metadata");

    let output = Command::new(binary_path())
        .arg("--controlled-diff-dir")
        .arg(&root)
        .output()
        .expect("spawn pid_inspect --controlled-diff-dir");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("case=one-line operation=place_line"),
        "stdout should include case summary, got:\n{stdout}"
    );
    assert!(
        stdout.contains("first_modified path=/Sheet6"),
        "stdout should include first modified stream, got:\n{stdout}"
    );
    assert!(
        stdout.contains("No geometry promotion was performed."),
        "stdout should state no promotion, got:\n{stdout}"
    );
}

#[test]
fn controlled_diff_dir_json_reports_stream_level_evidence() {
    let root = unique_tmp_dir("controlled-diff-json");
    let before_dir = root.join("before");
    let after_dir = root.join("after");
    let metadata_dir = root.join("metadata");
    std::fs::create_dir_all(&before_dir).expect("before dir");
    std::fs::create_dir_all(&after_dir).expect("after dir");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir");

    build_controlled_diff_fixture(&before_dir.join("one-circle.pid"), b"circle-before");
    build_controlled_diff_fixture(&after_dir.join("one-circle.pid"), b"circle-after");
    std::fs::write(
        metadata_dir.join("one-circle.json"),
        r#"{"case":"one-circle","operation":"place_circle","expected":{"center":[0,0],"radius":10},"notes":"synthetic"}"#,
    )
    .expect("metadata");

    let output = Command::new(binary_path())
        .arg("--controlled-diff-dir")
        .arg(&root)
        .arg("--json")
        .output()
        .expect("spawn pid_inspect --controlled-diff-dir --json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("invalid JSON {err}: {stdout}"));
    assert_eq!(json["promoted_geometry"], false);
    assert_eq!(json["cases"][0]["case"], "one-circle");
    assert_eq!(json["cases"][0]["operation"], "place_circle");
    assert_eq!(json["cases"][0]["notes"], "synthetic");
    assert_eq!(json["cases"][0]["first_modified"]["path"], "/Sheet6");
}

#[test]
fn controlled_diff_dir_json_empty_directory_reports_no_cases() {
    let root = unique_tmp_dir("controlled-diff-empty");

    let output = Command::new(binary_path())
        .arg("--controlled-diff-dir")
        .arg(&root)
        .arg("--json")
        .output()
        .expect("spawn pid_inspect --controlled-diff-dir --json empty");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("invalid JSON {err}: {stdout}"));
    assert_eq!(json["promoted_geometry"], false);
    assert_eq!(
        json["cases"].as_array().map(Vec::len),
        Some(0),
        "empty controlled diff dir should emit empty cases, got:\n{stdout}"
    );
}

#[test]
fn controlled_diff_dir_rejects_metadata_case_mismatch() {
    let root = unique_tmp_dir("controlled-diff-bad-metadata");
    let before_dir = root.join("before");
    let after_dir = root.join("after");
    let metadata_dir = root.join("metadata");
    std::fs::create_dir_all(&before_dir).expect("before dir");
    std::fs::create_dir_all(&after_dir).expect("after dir");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir");

    build_controlled_diff_fixture(&before_dir.join("one-arc.pid"), b"arc-before");
    build_controlled_diff_fixture(&after_dir.join("one-arc.pid"), b"arc-after");
    std::fs::write(
        metadata_dir.join("one-arc.json"),
        r#"{"case":"wrong-name","operation":"place_arc","expected":{"center":[0,0],"radius":10}}"#,
    )
    .expect("metadata");

    let output = Command::new(binary_path())
        .arg("--controlled-diff-dir")
        .arg(&root)
        .output()
        .expect("spawn pid_inspect --controlled-diff-dir bad metadata");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr:\n{stderr}");
    assert!(
        stderr.contains("case field"),
        "stderr should explain metadata case mismatch, got:\n{stderr}"
    );
}

#[test]
fn geometry_json_flag_emits_normalized_probe_entities() {
    let fixture = unique_tmp("geometry-json");
    build_sheet_probe_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--geometry-json")
        .output()
        .expect("spawn pid_inspect --geometry-json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("--geometry-json must emit valid JSON");
    let entities = parsed["entities"]
        .as_array()
        .expect("entities should be an array");
    assert!(
        entities
            .iter()
            .any(|entity| entity["confidence"] == "probe_only"
                && entity["kind"]["kind"] == "unknown"
                && entity["coordinate_context"]["units"]["state"] == "unknown"
                && entity["coordinate_context"]["page_transform"]["state"] == "unavailable"
                && entity["id"]
                    .as_str()
                    .is_some_and(|id| id.contains("/Sheet6:"))),
        "expected Sheet6 probe evidence with explicit coordinate context in normalized geometry JSON; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("--- Summary ---"),
        "--geometry-json should not emit the legacy text report; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}

fn write_byte_audit_baseline(
    fixture: &std::path::Path,
    baseline_path: &std::path::Path,
    mutate: impl FnOnce(&mut pid_parse::byte_audit::ByteAuditReport),
) {
    let parser = pid_parse::PidParser::new();
    let pkg = parser
        .parse_package(fixture)
        .expect("parse fixture package");
    let mut report = pid_parse::byte_audit_report(&pkg);
    mutate(&mut report);
    let json = serde_json::to_string_pretty(&report).expect("serialize baseline");
    std::fs::write(baseline_path, json).expect("write baseline");
}

#[test]
fn coverage_flag_prints_section_and_all_four_buckets() {
    let fixture = unique_tmp("coverage-cli");
    build_mixed_coverage_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--coverage")
        .output()
        .expect("spawn pid_inspect --coverage");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    assert!(
        stdout.contains("--- Coverage ---"),
        "coverage section heading missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[FULL] DocVersion3"),
        "expected fully-decoded entry; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[PART] PSMcluster0"),
        "expected partially-decoded entry; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[ID]   Sheet1"),
        "expected identified-only entry; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[UNK]  MysteryStream"),
        "expected unknown entry; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Fully decoded:     1"),
        "bucket counts missing; stdout:\n{stdout}"
    );
    // When --coverage is used as the sole action-flag, the full legacy
    // report should NOT be emitted.
    assert!(
        !stdout.contains("--- Summary ---"),
        "--coverage alone should suppress the full report; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn coverage_flag_downgrades_docversion3_when_record_is_illegal() {
    // Phase 10b end-to-end: a fixture that ships `/DocVersion3` with
    // unparseable bytes must be classified as `[ID]` and the note
    // must point at the missing model field. This mirrors the
    // `coverage_downgrades_docversion3_when_parser_did_not_populate`
    // lib-unit test at the CLI boundary so we catch any regression
    // between the classifier and the report renderer.
    let fixture = unique_tmp("coverage-downgrade");
    let file = std::fs::File::create(&fixture).expect("create");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");
    let mut s = cfb.create_stream("/DocVersion3").unwrap();
    // Non-printable bytes: the parser rejects the "first byte must be
    // ASCII printable" heuristic, so `doc.version_history` stays None.
    s.write_all(&[0x01u8, 0x02, 0x03, 0x04]).unwrap();
    drop(s);
    cfb.flush().unwrap();

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--coverage")
        .output()
        .expect("spawn pid_inspect --coverage");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout:\n{stdout}");
    assert!(
        stdout.contains("[ID]   DocVersion3"),
        "expected DocVersion3 downgraded to [ID]; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("stream present"),
        "downgrade note should mention the silent-failure reason; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("version_history"),
        "downgrade note should quote the missing field; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn coverage_json_flag_emits_parseable_coverage_report() {
    // Phase 10e: `--coverage --json` must emit a machine-parseable
    // JSON representation of `CoverageReport` — distinct from plain
    // `--json` (which dumps the entire PidDocument). This test
    // spawns the binary, parses stdout with `serde_json`, and asserts
    // the expected shape + values.
    let fixture = unique_tmp("coverage-json");
    build_mixed_coverage_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--coverage")
        .arg("--json")
        .output()
        .expect("spawn pid_inspect --coverage --json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    // The JSON must have the top-level shape `{"entries": [...]}`.
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let entries = parsed
        .get("entries")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("missing/non-array 'entries' in JSON: {parsed}"));
    assert!(
        !entries.is_empty(),
        "CoverageReport.entries must be non-empty for our mixed fixture"
    );

    // At least one entry should have status "FullyDecoded" (DocVersion3
    // with a legal record) and one "Unknown" (MysteryStream).
    let statuses: Vec<String> = entries
        .iter()
        .filter_map(|e| e.get("status").and_then(|s| s.as_str()).map(str::to_string))
        .collect();
    assert!(
        statuses.iter().any(|s| s == "FullyDecoded"),
        "expected a FullyDecoded entry; got statuses: {statuses:?}",
    );
    assert!(
        statuses.iter().any(|s| s == "Unknown"),
        "expected an Unknown entry; got statuses: {statuses:?}",
    );

    // Regression guard: the output must NOT look like the full
    // PidDocument dump. The coverage JSON shape has `entries` at the
    // top level — if `--json` without `--coverage` accidentally
    // hijacks, we'd see `streams` / `summary` / other doc keys.
    assert!(
        parsed.get("streams").is_none(),
        "coverage-only JSON must not include full-doc keys like 'streams'"
    );

    // Phase 10f: every entry produced from a real doc should carry a
    // `stream_size` (JSON number). Spot-check any entry has it.
    assert!(
        entries.iter().any(|e| e
            .get("stream_size")
            .and_then(serde_json::Value::as_u64)
            .is_some()),
        "at least one entry should report stream_size; entries: {entries:?}",
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn byte_audit_flag_prints_text_report() {
    let fixture = unique_tmp("byte-audit-cli");
    build_mixed_coverage_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--byte-audit")
        .output()
        .expect("spawn pid_inspect --byte-audit");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    assert!(
        stdout.contains("--- Byte Audit ---"),
        "byte-audit section heading missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Overall coverage:"),
        "overall coverage summary missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("/DocVersion3") && stdout.contains("parse_doc_version3"),
        "expected traced DocVersion3 stream; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("/MysteryStream") && stdout.contains("unregistered"),
        "expected unregistered mystery stream; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("--- Summary ---"),
        "--byte-audit alone should suppress the full report; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn byte_audit_json_flag_emits_parseable_report() {
    let fixture = unique_tmp("byte-audit-json");
    build_mixed_coverage_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--byte-audit")
        .arg("--json")
        .output()
        .expect("spawn pid_inspect --byte-audit --json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert!(
        parsed.get("per_stream").is_some(),
        "byte-audit JSON should include per_stream; json: {parsed}"
    );
    let unregistered = parsed
        .get("unregistered_paths")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("missing/non-array unregistered_paths: {parsed}"));
    assert!(
        unregistered
            .iter()
            .any(|v| v.as_str() == Some("/MysteryStream")),
        "MysteryStream should be listed as unregistered; json: {parsed}"
    );
    let doc_version = parsed
        .pointer("/per_stream/~1DocVersion3/parser_name")
        .and_then(|v| v.as_str());
    assert_eq!(
        doc_version,
        Some("parse_doc_version3"),
        "DocVersion3 should be traced by parse_doc_version3; json: {parsed}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn byte_audit_baseline_flag_reports_clean_comparison() {
    let fixture = unique_tmp("byte-audit-baseline-clean");
    let baseline = unique_tmp("byte-audit-baseline-clean-json");
    build_mixed_coverage_fixture(&fixture);
    write_byte_audit_baseline(&fixture, &baseline, |_| {});

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--byte-audit")
        .arg("--byte-audit-baseline")
        .arg(&baseline)
        .output()
        .expect("spawn pid_inspect --byte-audit --byte-audit-baseline");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("--- Byte Audit Baseline Comparison ---"),
        "comparison heading missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Regressions: 0"),
        "clean baseline should report zero regressions; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_file(&baseline);
}

#[test]
fn byte_audit_baseline_flag_fails_on_regression() {
    let fixture = unique_tmp("byte-audit-baseline-regression");
    let baseline = unique_tmp("byte-audit-baseline-regression-json");
    build_mixed_coverage_fixture(&fixture);
    write_byte_audit_baseline(&fixture, &baseline, |report| {
        report.overall_coverage_ratio = 1.0;
    });

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--byte-audit")
        .arg("--byte-audit-baseline")
        .arg(&baseline)
        .output()
        .expect("spawn pid_inspect --byte-audit --byte-audit-baseline");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(3),
        "baseline regression should exit 3; stderr: {stderr}; stdout: {stdout}",
    );
    assert!(
        stdout.contains("Regressions: 1"),
        "regression count missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("overall_coverage_decreased"),
        "regression kind missing; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_file(&baseline);
}

#[test]
fn probe_sheet_chunks_prints_report_level_evidence() {
    let fixture = unique_tmp("sheet-chunks-evidence");
    build_sheet_probe_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .arg("--probe-sheet-chunks")
        .arg("Sheet6")
        .output()
        .expect("spawn pid_inspect --probe-sheet-chunks");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code {:?}; stderr: {stderr}; stdout: {stdout}",
        output.status.code()
    );

    assert!(
        stdout.contains("record types: 0x00CE=1"),
        "record type summary missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("text runs:"),
        "text run summary missing; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("coordinate hints:"),
        "coordinate hint summary missing; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn no_flags_still_produces_full_report_including_coverage_section() {
    // Regression guard: Phase 10a embeds the coverage section into
    // `generate_report`, so running `pid_inspect fixture.pid` (no
    // action flags) must show both the classic sections AND the new
    // coverage section.
    let fixture = unique_tmp("coverage-noflag");
    build_mixed_coverage_fixture(&fixture);

    let output = Command::new(binary_path())
        .arg(&fixture)
        .output()
        .expect("spawn pid_inspect");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "exit code {:?}; stdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("--- Coverage ---"),
        "no-flag run must still include the embedded coverage section; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[UNK]  MysteryStream"),
        "coverage bullets rendered; stdout:\n{stdout}"
    );

    let _ = std::fs::remove_file(&fixture);
}
