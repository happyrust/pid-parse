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
