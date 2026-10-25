//! End-to-end tests for the `pid_inspect` command surface.
//!
//! Since B3 (issue #26) the binary is a thin adapter over
//! [`pid_parse::inspect::commands::InspectCommand`], so the bulk of the
//! suite drives the library directly: build a [`PidPackage`] fixture,
//! `run` the command, assert on [`InspectOutput::render_text`] /
//! [`InspectOutput::json_body`] / [`InspectOutput::exit_code`]. Two
//! spawn tests remain to prove the argument-parser → command → print
//! wiring still works end to end.
use pid_parse::inspect::commands::{ExportBundleOptions, InspectCommand, InspectOutput};
use pid_parse::PidParser;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_inspect")
}

const LEGACY_SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";
const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";

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

/// Parse a fixture `.pid` for commands that take an in-memory package.
fn parse_pkg(path: &std::path::Path) -> pid_parse::PidPackage {
    PidParser::new()
        .parse_package(path)
        .expect("parse fixture package")
}

/// Run a command that must succeed, returning its text rendering.
fn run_ok(cmd: &InspectCommand, pkg: Option<&pid_parse::PidPackage>) -> InspectOutput {
    cmd.run(pkg).expect("inspect command should succeed")
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

fn write_byte_audit_baseline(
    fixture: &std::path::Path,
    baseline_path: &std::path::Path,
    mutate: impl FnOnce(&mut pid_parse::byte_audit::ByteAuditReport),
) {
    let pkg = parse_pkg(fixture);
    let mut report = pid_parse::byte_audit_report(&pkg);
    mutate(&mut report);
    let json = serde_json::to_string_pretty(&report).expect("serialize baseline");
    std::fs::write(baseline_path, json).expect("write baseline");
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

    let out = run_ok(&InspectCommand::ControlledDiffDir { dir: root }, None);
    assert_eq!(out.exit_code(), 0);
    let stdout = out.render_text();
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

    let out = run_ok(&InspectCommand::ControlledDiffDir { dir: root }, None);
    let body = out
        .json_body()
        .expect("json_body")
        .expect("controlled diff always has a JSON body");
    let json: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|err| panic!("invalid JSON {err}: {body}"));
    assert_eq!(json["promoted_geometry"], false);
    assert_eq!(json["cases"][0]["case"], "one-circle");
    assert_eq!(json["cases"][0]["operation"], "place_circle");
    assert_eq!(json["cases"][0]["notes"], "synthetic");
    assert_eq!(json["cases"][0]["first_modified"]["path"], "/Sheet6");
}

#[test]
fn controlled_diff_dir_json_empty_directory_reports_no_cases() {
    let root = unique_tmp_dir("controlled-diff-empty");

    let out = run_ok(&InspectCommand::ControlledDiffDir { dir: root }, None);
    let body = out
        .json_body()
        .expect("json_body")
        .expect("empty dir still emits the evidence report JSON");
    let json: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|err| panic!("invalid JSON {err}: {body}"));
    assert_eq!(json["promoted_geometry"], false);
    assert_eq!(
        json["cases"].as_array().map(Vec::len),
        Some(0),
        "empty controlled diff dir should emit empty cases, got:\n{body}"
    );
}

/// AC6 for `goals/phase14-plan-b-controlled-diff-protocol/plan.md`:
/// run the protocol walkthrough on synthetic CFB fixtures
/// representing two operation classes (line + circle) at once,
/// confirm both cases surface in the JSON evidence report, both
/// `first_modified` slices point at `/Sheet6`, and
/// `promoted_geometry` stays `false` aggregate-wide.
///
/// This test does NOT use a real SmartPlant `.pid` fixture — the
/// `inspect_cli` test suite must stay self-contained per the
/// project test conventions. The synthetic CFB only proves the
/// **protocol's directory + sidecar shape** flows through
/// `pid_inspect --controlled-diff-dir` cleanly; real-byte
/// validation is a separate (operator-driven) workstream covered
/// by the protocol document itself.
#[test]
fn controlled_diff_protocol_synthetic_two_case_walkthrough() {
    let root = unique_tmp_dir("controlled-diff-protocol-walkthrough");
    let before_dir = root.join("before");
    let after_dir = root.join("after");
    let metadata_dir = root.join("metadata");
    std::fs::create_dir_all(&before_dir).expect("before dir");
    std::fs::create_dir_all(&after_dir).expect("after dir");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir");

    // Case 1: place_line. Distinct sheet bytes between before / after.
    build_controlled_diff_fixture(&before_dir.join("line-01.pid"), b"protocol-line-before");
    build_controlled_diff_fixture(&after_dir.join("line-01.pid"), b"protocol-line-AFTER!");
    std::fs::write(
        metadata_dir.join("line-01.json"),
        r#"{"case":"line-01","operation":"place_line","expected":{"start":[0.30,0.40],"end":[0.45,0.40],"coordinate_space":"normalized_page","tool":"PipeRun","template":"A2-W-New.pid","smartplant_version":"12.0 SP1"},"notes":"protocol synthetic"}"#,
    )
    .expect("line-01 metadata");

    // Case 2: place_circle. Different sheet bytes again.
    build_controlled_diff_fixture(
        &before_dir.join("circle-default.pid"),
        b"protocol-circle-before",
    );
    build_controlled_diff_fixture(
        &after_dir.join("circle-default.pid"),
        b"protocol-circle-AFTER!",
    );
    std::fs::write(
        metadata_dir.join("circle-default.json"),
        r#"{"case":"circle-default","operation":"place_circle","expected":{"center":[0.50,0.50],"radius":0.08,"coordinate_space":"normalized_page","tool":"MiscAnnotations.Circle","template":"A2-W-New.pid","smartplant_version":"12.0 SP1"},"notes":"protocol synthetic"}"#,
    )
    .expect("circle-default metadata");

    let out = run_ok(&InspectCommand::ControlledDiffDir { dir: root }, None);
    let body = out
        .json_body()
        .expect("json_body")
        .expect("controlled diff always has a JSON body");
    let json: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|err| panic!("invalid JSON {err}: {body}"));

    assert_eq!(
        json["promoted_geometry"], false,
        "protocol walkthrough must never promote geometry (phase 14 type invariant)"
    );

    let cases = json["cases"]
        .as_array()
        .expect("cases must be a JSON array");
    assert_eq!(
        cases.len(),
        2,
        "expected 2 cases (line + circle), got\n{body}"
    );

    // Cases come back in case-name lex order: circle-default first, then line-01.
    assert_eq!(cases[0]["case"], "circle-default");
    assert_eq!(cases[0]["operation"], "place_circle");
    assert_eq!(cases[0]["first_modified"]["path"], "/Sheet6");
    assert_eq!(cases[0]["modified_sheet_streams"], 1);
    assert_eq!(cases[0]["only_in_before"], 0);
    assert_eq!(cases[0]["only_in_after"], 0);
    // The `expected` payload propagates verbatim through the
    // pipeline — operators rely on this for cross-referencing
    // SmartPlant intent against parsed bytes downstream.
    assert_eq!(
        cases[0]["expected"]["tool"], "MiscAnnotations.Circle",
        "expected payload must round-trip verbatim, got\n{body}"
    );
    assert_eq!(cases[0]["notes"], "protocol synthetic");

    assert_eq!(cases[1]["case"], "line-01");
    assert_eq!(cases[1]["operation"], "place_line");
    assert_eq!(cases[1]["first_modified"]["path"], "/Sheet6");
    assert_eq!(cases[1]["modified_sheet_streams"], 1);
    assert_eq!(
        cases[1]["expected"]["tool"], "PipeRun",
        "line expected payload must round-trip verbatim",
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

    match (InspectCommand::ControlledDiffDir { dir: root }).run(None) {
        Err(e) => {
            assert_eq!(e.exit_code, 2, "stderr: {}", e.message);
            assert!(
                e.message.contains("case field"),
                "stderr should explain metadata case mismatch, got:\n{}",
                e.message
            );
        }
        Ok(_) => panic!("metadata case mismatch must fail with exit 2"),
    }
}

#[test]
fn geometry_json_flag_emits_normalized_probe_entities() {
    let fixture = unique_tmp("geometry-json");
    build_sheet_probe_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    let out = InspectCommand::Geometry
        .run(Some(&pkg))
        .expect("geometry run");
    let body = out
        .json_body()
        .expect("json_body")
        .expect("geometry always has a JSON body");
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("--geometry-json must emit valid JSON");
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
        "expected Sheet6 probe evidence with explicit coordinate context in normalized geometry JSON; json:\n{body}"
    );
    let text = out.render_text();
    assert!(
        !text.contains("--- Summary ---"),
        "--geometry-json should not emit the legacy text report; text:\n{text}"
    );

    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn export_bundle_flag_writes_minimal_bundle_and_raw_bytes_only_when_requested() {
    let fixture = unique_tmp("export-bundle");
    let default_out = unique_tmp_dir("export-bundle-default");
    let raw_out = unique_tmp_dir("export-bundle-raw");
    build_mixed_coverage_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    InspectCommand::ExportBundle(ExportBundleOptions {
        output_dir: default_out.clone(),
        ..ExportBundleOptions::default()
    })
    .run(Some(&pkg))
    .expect("default export bundle run");

    assert!(default_out.join("manifest.json").is_file());
    assert!(default_out.join("raw/streams.json").is_file());
    assert!(default_out.join("decoded/document.json").is_file());
    assert!(default_out.join("audit/confidence_ledger.json").is_file());
    assert!(
        !default_out.join("raw/streams").exists(),
        "default export must not write raw stream byte dumps"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(default_out.join("manifest.json")).unwrap())
            .expect("manifest JSON");
    assert_eq!(manifest["features"]["raw_stream_bytes"], false);
    assert_eq!(manifest["counts"]["streams"], 4);

    InspectCommand::ExportBundle(ExportBundleOptions {
        output_dir: raw_out.clone(),
        include_raw_streams: true,
        ..ExportBundleOptions::default()
    })
    .run(Some(&pkg))
    .expect("raw-streams export bundle run");

    let streams: serde_json::Value =
        serde_json::from_slice(&std::fs::read(raw_out.join("raw/streams.json")).unwrap())
            .expect("streams JSON");
    let mystery = streams["streams"]
        .as_array()
        .expect("streams array")
        .iter()
        .find(|stream| stream["path"] == "/MysteryStream")
        .expect("MysteryStream entry");
    assert_eq!(mystery["raw_bytes_written"], true);
    let escaped = mystery["escaped_filename"]
        .as_str()
        .expect("escaped filename");
    assert_eq!(
        std::fs::read(raw_out.join("raw/streams").join(escaped)).expect("raw stream bytes"),
        b"ghost-bytes"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&default_out);
    let _ = std::fs::remove_dir_all(&raw_out);
}

#[test]
fn export_bundle_publish_flags_write_xml_and_success_status() {
    if !std::path::Path::new(LEGACY_SQLITE_PATH).exists() {
        eprintln!("skipping: fixture {LEGACY_SQLITE_PATH} not found");
        return;
    }
    let fixture = unique_tmp("export-bundle-publish");
    let out_dir = unique_tmp_dir("export-bundle-publish");
    build_mixed_coverage_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    InspectCommand::ExportBundle(ExportBundleOptions {
        output_dir: out_dir.clone(),
        publish_input: Some(PathBuf::from(LEGACY_SQLITE_PATH)),
        publish_drawing: Some(A01_DRAWING_UID.to_string()),
        publish_plant: Some("TEST02".to_string()),
        publish_style: Some("a01".to_string()),
        ..ExportBundleOptions::default()
    })
    .run(Some(&pkg))
    .expect("export-bundle publish run");

    assert!(out_dir.join("publish/data.xml").is_file());
    assert!(out_dir.join("publish/meta.xml").is_file());

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out_dir.join("manifest.json")).unwrap())
            .expect("manifest JSON");
    assert_eq!(manifest["features"]["publish_xml"], true);
    assert_eq!(manifest["inputs"]["publish_mdf"]["kind"], "sqlite");

    let status: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out_dir.join("publish/status.json")).unwrap())
            .expect("status JSON");
    assert_eq!(status["requested_drawing_uid"], A01_DRAWING_UID);
    assert_eq!(status["plant"], "TEST02");
    assert_eq!(status["style"], "a01");
    assert_eq!(status["data_xml_written"], true);
    assert_eq!(status["meta_xml_written"], true);
    assert_eq!(status["status"]["state"], "written");

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn export_bundle_publish_diff_against_writes_diff_artifact() {
    if !std::path::Path::new(A01_MDF_PATH).exists()
        || !std::path::Path::new(A01_REFERENCE_DATA_XML).exists()
    {
        eprintln!("skipping: fixture {A01_MDF_PATH} or {A01_REFERENCE_DATA_XML} not found");
        return;
    }
    let fixture = unique_tmp("export-bundle-publish-diff");
    let out_dir = unique_tmp_dir("export-bundle-publish-diff");
    build_mixed_coverage_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    InspectCommand::ExportBundle(ExportBundleOptions {
        output_dir: out_dir.clone(),
        publish_input: Some(PathBuf::from(A01_MDF_PATH)),
        publish_drawing: Some(A01_DRAWING_UID.to_string()),
        publish_plant: Some("TEST02".to_string()),
        publish_diff_against: Some(PathBuf::from(A01_REFERENCE_DATA_XML)),
        ..ExportBundleOptions::default()
    })
    .run(Some(&pkg))
    .expect("export-bundle publish diff run");

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out_dir.join("manifest.json")).unwrap())
            .expect("manifest JSON");
    assert_eq!(manifest["inputs"]["publish_mdf"]["kind"], "mdf");
    assert!(out_dir.join("publish/publish_diff.json").is_file());
    let diff: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out_dir.join("publish/publish_diff.json")).unwrap())
            .expect("publish diff JSON");
    assert_eq!(diff["reference_path"], A01_REFERENCE_DATA_XML);
    assert!(diff["pid_tags"]["generated_total"].as_u64().is_some());
    assert!(diff["rel_defuids"]["reference_total"].as_u64().is_some());

    let status: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out_dir.join("publish/status.json")).unwrap())
            .expect("status JSON");
    let comparison = status["reference_comparison"]["state"]
        .as_str()
        .expect("comparison state");
    assert!(matches!(comparison, "clean" | "findings"));

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn coverage_flag_prints_section_and_all_four_buckets() {
    let fixture = unique_tmp("coverage-cli");
    build_mixed_coverage_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    let out = run_ok(&InspectCommand::Coverage, Some(&pkg));
    let stdout = out.render_text();

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
    // lib-unit test at the command boundary so we catch any regression
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
    let pkg = parse_pkg(&fixture);

    let stdout = run_ok(&InspectCommand::Coverage, Some(&pkg)).render_text();
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
    // `--json` (which dumps the entire PidDocument). The library call
    // returns the payload; `json_body` gives the same string the CLI
    // printed.
    let fixture = unique_tmp("coverage-json");
    build_mixed_coverage_fixture(&fixture);
    let pkg = parse_pkg(&fixture);

    let out = run_ok(&InspectCommand::Coverage, Some(&pkg));
    let body = out
        .json_body()
        .expect("json_body")
        .expect("coverage always has a JSON body");
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("json body must be valid JSON");
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
    let pkg = parse_pkg(&fixture);

    let stdout = run_ok(&InspectCommand::ByteAudit { baseline: None }, Some(&pkg)).render_text();
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
    let pkg = parse_pkg(&fixture);

    let out = run_ok(&InspectCommand::ByteAudit { baseline: None }, Some(&pkg));
    let body = out
        .json_body()
        .expect("json_body")
        .expect("byte audit always has a JSON body");
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("json body must be valid JSON");
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
    let pkg = parse_pkg(&fixture);

    let out = run_ok(
        &InspectCommand::ByteAudit {
            baseline: Some(baseline.clone()),
        },
        Some(&pkg),
    );
    assert_eq!(out.exit_code(), 0);
    let stdout = out.render_text();
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
    let pkg = parse_pkg(&fixture);

    let out = run_ok(
        &InspectCommand::ByteAudit {
            baseline: Some(baseline.clone()),
        },
        Some(&pkg),
    );
    assert_eq!(
        out.exit_code(),
        3,
        "baseline regression should map to exit 3"
    );
    let stdout = out.render_text();
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
    let pkg = parse_pkg(&fixture);

    let stdout = run_ok(
        &InspectCommand::ProbeSheetChunks {
            target: Some("Sheet6".to_string()),
        },
        Some(&pkg),
    )
    .render_text();
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

// ---------------------------------------------------------------------------
// CLI smoke tests — the only tests still spawning the binary. They cover
// the wiring the in-library calls can't: argv → `InspectCommand` →
// stdout/stderr, including flag-combination validation.
// ---------------------------------------------------------------------------

#[test]
fn cli_smoke_no_flags_still_produces_full_report_including_coverage_section() {
    // Regression guard: a bare `pid_inspect fixture.pid` must show both
    // the classic report sections AND the embedded coverage section.
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

#[test]
fn cli_help_flag_prints_usage_to_stdout_and_exits_zero() {
    // `-h` / `--help` used to be swallowed as an input path and die in
    // the parser; the flag is now answered before any file is opened.
    for flag in ["-h", "--help"] {
        let output = Command::new(binary_path())
            .arg(flag)
            .output()
            .expect("spawn pid_inspect --help");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{flag} should exit 0; got {:?}",
            output.status.code()
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage: pid_inspect"),
            "{flag} should print usage to stdout; stdout:\n{stdout}"
        );
        assert!(
            output.stderr.is_empty(),
            "{flag} is not a diagnostic and must not touch stderr; stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cli_no_args_still_reports_usage_on_stderr() {
    // The other half of the split: a missing input is a failure, so its
    // usage goes to stderr and stdout stays clean.
    let output = Command::new(binary_path())
        .output()
        .expect("spawn pid_inspect with no args");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage: pid_inspect"),
        "a missing input should carry the usage body on stderr; got:\n{stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "a missing input must leave stdout untouched; stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn cli_smoke_export_bundle_publish_requires_drawing_uid() {
    // Flag-combination validation lives in the adapter (the binary
    // exits 2 before any command runs) — this is the one behaviour
    // class an in-library call cannot observe.
    let output = Command::new(binary_path())
        .arg("missing.pid")
        .arg("--export-bundle")
        .arg(unique_tmp_dir("export-bundle-missing-drawing"))
        .arg("--export-bundle-publish")
        .arg(LEGACY_SQLITE_PATH)
        .output()
        .expect("spawn pid_inspect --export-bundle-publish without drawing");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing publish drawing should be argument error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--export-bundle-publish requires --publish-drawing"),
        "stderr should explain missing drawing UID, got:\n{stderr}"
    );
}
