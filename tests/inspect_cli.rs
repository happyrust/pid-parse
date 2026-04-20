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

/// Build a tiny valid CFB fixture with a mixture of coverage classes:
/// a fully-decoded stream (`DocVersion3`), a partially-decoded stream
/// (`PSMsegmenttable`), a storage-prefix stream (`Sheet1/Foo`), and an
/// unknown top-level stream (`MysteryStream`).
fn build_mixed_coverage_fixture(path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create fixture");
    let mut cfb = ::cfb::CompoundFile::create(file).expect("cfb create");
    cfb.create_storage_all("/Sheet1").expect("storage");

    let mut s = cfb.create_stream("/DocVersion3").unwrap();
    // Any non-empty bytes — the reader's heuristics will accept or skip;
    // we care only about the top-level name being present.
    s.write_all(b"DocVersion3-bytes").unwrap();
    drop(s);

    let mut s = cfb.create_stream("/PSMsegmenttable").unwrap();
    s.write_all(b"psm-seg-bytes").unwrap();
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
        stdout.contains("[PART] PSMsegmenttable"),
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
