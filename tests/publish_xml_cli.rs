//! End-to-end CLI tests for the `pid_publish_xml` binary (A10
//! series). Drives the actual compiled binary via `Command` so the
//! whole stack — argument parsing, SQLite read, DTO load,
//! `_Data.xml` + `_Meta.xml` rendering, and file I/O — is exercised
//! the same way an operator would invoke it.
//!
//! All tests that touch the real `Export_v2.sqlite` fixture skip
//! cleanly when the file is missing so CI workers without the
//! TEST02 backup do not fail.

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_publish_xml")
}

const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

/// Allocate a unique sub-directory under the OS temp dir so two
/// concurrent test runs do not collide. Tests are responsible for
/// cleaning up; we deliberately avoid `tempfile` to keep the
/// dependency surface small (the workspace already runs other
/// integration tests through bare `std::env::temp_dir()`).
fn unique_tmp_dir(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!(
        "pid-parse-publish-xml-cli-{}-{}-{}",
        std::process::id(),
        label,
        nanos
    ));
    std::fs::create_dir_all(&p).expect("create temp dir");
    p
}

/// True when the real OrcaMDF fixture is on disk. Returns false (and
/// prints a reason) for skipping behavior on bare CI workers that do
/// not carry the SmartPlant backup data.
fn fixture_available() -> bool {
    let p = std::path::Path::new(SQLITE_PATH);
    let here = p.exists();
    if !here {
        eprintln!("skipping: fixture {SQLITE_PATH} not found");
    }
    here
}

#[test]
fn cli_help_flag_exits_zero_with_usage_text() {
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success(), "--help should exit 0; got {out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Usage: pid_publish_xml"),
        "--help should print usage; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("--meta-out"),
        "--help should advertise the new --meta-out flag; stderr:\n{stderr}"
    );
}

#[test]
fn cli_missing_sqlite_argument_exits_two() {
    let out = Command::new(binary_path())
        .output()
        .expect("spawn pid_publish_xml with no args");
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing required argument should exit 2; got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing <sqlite>"),
        "stderr should explain the missing argument; got:\n{stderr}"
    );
}

#[test]
fn cli_meta_out_with_stdout_is_rejected_as_argument_error() {
    let out = Command::new(binary_path())
        .args([
            "Export_v2.sqlite",
            "--drawing",
            "ANY",
            "--stdout",
            "--meta-out",
            "Meta.xml",
        ])
        .output()
        .expect("spawn pid_publish_xml --stdout --meta-out");
    assert_eq!(
        out.status.code(),
        Some(2),
        "--meta-out + --stdout combo should exit 2 (argument error); got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--meta-out requires --out"),
        "stderr should explain the conflict; got:\n{stderr}"
    );
}

#[test]
fn cli_writes_both_data_and_meta_xml_for_real_drawing() {
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("a01");
    let data_path = dir.join("A01_Data.xml");
    let meta_path = dir.join("A01_Meta.xml");

    let out = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--out",
            data_path.to_str().expect("data path utf8"),
            "--meta-out",
            meta_path.to_str().expect("meta path utf8"),
            "--plant",
            "TEST02",
        ])
        .output()
        .expect("spawn pid_publish_xml end-to-end");
    assert!(
        out.status.success(),
        "CLI should succeed; stdout: {} stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // _Data.xml — A01 drawing with the four-object inventory we
    // formalized in A9 (Vessel + Nozzle + PipeRun + PipingPoint).
    let data_xml = std::fs::read_to_string(&data_path).expect("read _Data.xml");
    assert!(
        data_xml.contains("<PIDDrawing>"),
        "_Data.xml must wrap the drawing in <PIDDrawing>; got:\n{data_xml}"
    );
    for tag in ["<PIDProcessVessel>", "<PIDNozzle>", "<PIDPipingPort>"] {
        assert!(
            data_xml.contains(tag),
            "_Data.xml must emit `{tag}` for the A01 drawing; got:\n{data_xml}"
        );
    }
    assert!(
        data_xml.contains(r#"DocUID="D9635C3C898840D1990B7E8BEE1D55DA""#),
        "_Data.xml DocUID must equal the requested drawing UID; got:\n{data_xml}"
    );
    assert!(
        data_xml.contains(r#"Plant="TEST02""#),
        "_Data.xml Plant must reflect --plant; got:\n{data_xml}"
    );

    // _Meta.xml — DocVersioningComponent header + three blocks +
    // three rels in the canonical order.
    let meta_xml = std::fs::read_to_string(&meta_path).expect("read _Meta.xml");
    assert!(
        meta_xml.contains(r#"CompSchema="DocVersioningComponent""#),
        "_Meta.xml must advertise DocVersioningComponent; got:\n{meta_xml}"
    );
    for def_uid in ["VersionedDoc", "RevisedDocument", "FileComposition"] {
        assert!(
            meta_xml.contains(&format!(r#"DefUID="{def_uid}""#)),
            "_Meta.xml must carry the `{def_uid}` Rel; got:\n{meta_xml}"
        );
    }
    assert!(
        meta_xml.contains(r#"Name="A01 Version""#),
        "_Meta.xml DocumentVersion IObject Name should be `<drawing> Version`; got:\n{meta_xml}"
    );
    assert!(
        meta_xml.contains(r#"Name="A01.pid""#),
        "_Meta.xml File IObject Name should be `<drawing>.pid`; got:\n{meta_xml}"
    );
    assert!(
        meta_xml.contains(r#"DocVersionDate="2026/04/20""#),
        "_Meta.xml DocVersionDate should normalize OrcaMDF's raw `2026/4/20 ...` to ISO-ish; got:\n{meta_xml}"
    );

    // Determinism — invoking the CLI a second time must produce
    // byte-identical _Meta.xml so downstream consumers can diff
    // releases without spurious churn.
    let out2 = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--out",
            data_path.to_str().unwrap(),
            "--meta-out",
            meta_path.to_str().unwrap(),
            "--plant",
            "TEST02",
        ])
        .output()
        .expect("spawn pid_publish_xml second time");
    assert!(out2.status.success(), "second invocation should also succeed");
    let meta_xml_2 = std::fs::read_to_string(&meta_path).expect("re-read _Meta.xml");
    assert_eq!(
        meta_xml, meta_xml_2,
        "two invocations must produce byte-identical _Meta.xml"
    );

    // Best-effort cleanup; ignore failures because the temp dir is
    // self-disposing across reboots.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_data_only_invocation_does_not_write_meta_file() {
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("data-only");
    let data_path = dir.join("A01_Data.xml");
    let bogus_meta_path = dir.join("A01_Meta.xml");
    assert!(
        !bogus_meta_path.exists(),
        "test must start with no meta file present"
    );

    let out = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --out only");
    assert!(out.status.success(), "data-only invocation should succeed");
    assert!(data_path.exists(), "_Data.xml must be written");
    assert!(
        !bogus_meta_path.exists(),
        "_Meta.xml must NOT be written when --meta-out is omitted"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// -----------------------------------------------------------------
// A12 — --diff-against semantic diff CLI surface
// -----------------------------------------------------------------

const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";

#[test]
fn cli_no_output_or_diff_flags_is_argument_error() {
    // Pre-A12, the CLI required exactly one of --out / --stdout.
    // Post-A12 the relaxation is that --diff-against also satisfies
    // the requirement; no flags at all must still error cleanly.
    let out = Command::new(binary_path())
        .args(["any.sqlite", "--drawing", "ANY"])
        .output()
        .expect("spawn pid_publish_xml without sinks");
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing all output/diff sinks should exit 2; got {out:?}",
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("at least one of --out FILE / --stdout / --diff-against"),
        "stderr should advertise the new triple-disjunction; got:\n{stderr}"
    );
}

#[test]
fn cli_help_advertises_diff_against_flag() {
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--diff-against"),
        "--help should advertise --diff-against; got:\n{stderr}"
    );
}

#[test]
fn cli_diff_against_real_a01_reference_reports_known_findings_and_exits_one() {
    if !fixture_available() {
        return;
    }
    if !std::path::Path::new(A01_REFERENCE_DATA_XML).exists() {
        eprintln!("skipping: reference fixture {A01_REFERENCE_DATA_XML} not found");
        return;
    }

    let out = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            A01_REFERENCE_DATA_XML,
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against");
    // We intentionally exit 1 today because the writer is known
    // to under-emit PIDProcessPoint and miscount PIDPipingPort /
    // PIDRepresentation against this reference. The test pins this
    // contract so a future writer fix that closes the gap will
    // immediately flag the regression for this assertion to be
    // updated.
    assert_eq!(
        out.status.code(),
        Some(1),
        "A01 reference diff should exit 1 (known unresolved findings); got {out:?}",
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Header echo proves the report rendered.
    assert!(
        stdout.contains("Publish Data XML semantic diff"),
        "stdout should carry the SemanticDiffReport header; got:\n{stdout}"
    );
    // The five guaranteed-MATCH tag varieties on A01.
    for tag in [
        "PIDDrawing",
        "PIDNozzle",
        "PIDPipeline",
        "PIDPipingConnector",
        "PIDProcessVessel",
    ] {
        assert!(
            stdout.contains(tag),
            "report should list {tag}; got:\n{stdout}"
        );
    }
    // The known-MISSING tag (PIDProcessPoint not yet implemented).
    assert!(
        stdout.contains("MISSING"),
        "report should flag at least one MISSING row; got:\n{stdout}"
    );
    assert!(
        stdout.contains("PIDProcessPoint"),
        "PIDProcessPoint is the canonical MISSING tag; got:\n{stdout}"
    );
    // Side echo on stderr summarizes the verdict.
    assert!(
        stderr.contains("surfaced findings"),
        "stderr should summarize the dirty verdict; got:\n{stderr}"
    );
}

#[test]
fn cli_diff_against_self_generated_is_clean_and_exits_zero() {
    // Generate an _Data.xml from the SQLite mirror, then diff it
    // against itself: the report must be clean and the binary
    // must exit 0.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("self-diff");
    let data_path = dir.join("A01_Data.xml");

    let gen = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --out (gen)");
    assert!(gen.status.success(), "initial generation must succeed");

    let diff = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against own output");
    assert_eq!(
        diff.status.code(),
        Some(0),
        "self-diff must exit 0 (clean); got {diff:?}",
    );
    let stderr = String::from_utf8_lossy(&diff.stderr);
    assert!(
        stderr.contains("clean ("),
        "stderr should announce the clean verdict; got:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_diff_against_combined_with_out_writes_xml_and_reports() {
    // --diff-against composes with --out: the XML lands on disk
    // AND the report is printed. Exit 1 because the A01 reference
    // still surfaces unresolved findings.
    if !fixture_available() {
        return;
    }
    if !std::path::Path::new(A01_REFERENCE_DATA_XML).exists() {
        eprintln!("skipping: reference fixture {A01_REFERENCE_DATA_XML} not found");
        return;
    }
    let dir = unique_tmp_dir("diff-and-out");
    let data_path = dir.join("A01_Data.xml");

    let out = Command::new(binary_path())
        .args([
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--out",
            data_path.to_str().unwrap(),
            "--diff-against",
            A01_REFERENCE_DATA_XML,
        ])
        .output()
        .expect("spawn pid_publish_xml --out --diff-against");
    assert_eq!(
        out.status.code(),
        Some(1),
        "combined flow with unresolved findings should exit 1; got {out:?}",
    );
    assert!(
        data_path.exists(),
        "--out should still produce the _Data.xml file when paired with --diff-against",
    );
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        report.contains("PIDProcessPoint"),
        "report on stdout should carry the canonical MISSING tag; got:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
