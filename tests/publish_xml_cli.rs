//! End-to-end CLI tests for the `pid_publish_xml` binary (A10
//! series). Drives the actual compiled binary via `Command` so the
//! whole stack — argument parsing, MDF read, DTO load,
//! `_Data.xml` + `_Meta.xml` rendering, and file I/O — is exercised
//! the same way an operator would invoke it.
//!
//! All tests that touch the real `Export.mdf` fixture skip
//! cleanly when the file is missing so CI workers without the
//! TEST02 backup do not fail.

use std::process::Command;

mod common;
use common::{normalize_a01_synthetic_slots, A01SyntheticMaskOptions, Scratch};

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_publish_xml")
}

const A01_MDF_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const LEGACY_SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export_v2.sqlite";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";
/// A committed `.pid`, which the `.pid` line publishes with no plant
/// database beside it.
const DWG_PID_PATH: &str = "test-file/DWG-0202GP06-01.pid";

/// A sub-directory of this test's own under the OS temp dir, so two
/// concurrent test runs do not collide. The directory lasts as long as
/// the returned guard: it is removed when the test ends, whether the test
/// passed or an assertion failed. We deliberately avoid `tempfile` to
/// keep the dependency surface small.
fn unique_tmp_dir(label: &str) -> Scratch {
    Scratch::new(&format!("publish-xml-cli-{label}"))
}

/// True when the real MDF fixture is on disk. Returns false (and
/// prints a reason) for skipping behavior on bare CI workers that do
/// not carry the `SmartPlant` backup data.
fn fixture_available() -> bool {
    let p = std::path::Path::new(A01_MDF_PATH);
    let here = p.exists();
    if !here {
        eprintln!("skipping: fixture {A01_MDF_PATH} not found");
    }
    here
}

fn legacy_sqlite_fixture_available() -> bool {
    let p = std::path::Path::new(LEGACY_SQLITE_PATH);
    let here = p.exists();
    if !here {
        eprintln!("skipping: legacy fixture {LEGACY_SQLITE_PATH} not found");
    }
    here
}

fn normalize_a01_delivery_contract(xml: &str) -> String {
    normalize_a01_synthetic_slots(xml, A01SyntheticMaskOptions::ALL)
}

#[test]
fn cli_help_flag_exits_zero_with_usage_text() {
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success(), "--help should exit 0; got {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Usage: pid_publish_xml"),
        "--help should print usage to stdout; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("--meta-out"),
        "--help should advertise the new --meta-out flag; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Legacy compatibility"),
        "--help should explain the MDF-first / SQLite-legacy contract; stdout:\n{stdout}"
    );
    assert!(
        out.stderr.is_empty(),
        "--help is not a diagnostic and must not touch stderr; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn cli_missing_input_argument_exits_two() {
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
        stderr.contains("missing <mdf> argument"),
        "stderr should explain the missing argument; got:\n{stderr}"
    );
    // The usage body follows the diagnostic on stderr — the mirror of
    // `--help`, which writes the same body to stdout. A failure never
    // claims stdout.
    assert!(
        stderr.contains("Usage: pid_publish_xml"),
        "a usage error should carry the usage body on stderr; got:\n{stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "a usage error must leave stdout untouched; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn cli_meta_out_with_stdout_is_rejected_as_argument_error() {
    let out = Command::new(binary_path())
        .args([
            "Export.mdf",
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
            A01_MDF_PATH,
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
        "_Meta.xml DocVersionDate should normalize the MDF date `2026/4/20 ...` to ISO-ish; got:\n{meta_xml}"
    );

    // Determinism — invoking the CLI a second time must produce
    // byte-identical _Meta.xml so downstream consumers can diff
    // releases without spurious churn.
    let out2 = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
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
    assert!(
        out2.status.success(),
        "second invocation should also succeed"
    );
    let meta_xml_2 = std::fs::read_to_string(&meta_path).expect("re-read _Meta.xml");
    assert_eq!(
        meta_xml, meta_xml_2,
        "two invocations must produce byte-identical _Meta.xml"
    );
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
            A01_MDF_PATH,
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
}

// -----------------------------------------------------------------
// A12 — --diff-against semantic diff CLI surface
// -----------------------------------------------------------------

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
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--diff-against"),
        "--help should advertise --diff-against; got:\n{stdout}"
    );
}

#[test]
fn cli_diff_against_real_a01_reference_prints_the_synthetic_residuals_and_exits_one() {
    // The database line scores through `diff_pid_export`, as the `.pid` line
    // and the backup package gate do. What A01 prints against its reference
    // is the three synthetic residuals — the connector this line derives
    // under its own UID, the Rel UIDs it composes, and the GraphicOIDs it
    // numbers itself — each registered line by line in
    // `docs/baselines/publish-diff-accepted.json` and held there by
    // `tests/publish_diff_accepted_baseline.rs`. The run exits 1.
    if !fixture_available() {
        return;
    }
    if !std::path::Path::new(A01_REFERENCE_DATA_XML).exists() {
        eprintln!("skipping: reference fixture {A01_REFERENCE_DATA_XML} not found");
        return;
    }

    let out = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            A01_REFERENCE_DATA_XML,
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stdout.contains(
            "pid export diff: objects 12 (extra 4, wrong class 0), values 18 \
             (different 4, not in reference 0), rels 15 (not in reference 15)"
        ),
        "{stdout}"
    );
    let findings: Vec<&str> = stdout
        .lines()
        .filter(|line| {
            ["extra ", "missing ", "value ", "rel (", "class "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    assert_eq!(findings.len(), 27, "{stdout}");
    for line in &findings {
        let residual = line.contains("185EF98B03E844158E3BD8E82806E6CF-CNX")
            || line.contains("8B6386EF73A497F1A03D67B6A046D596")
            || line.starts_with("rel (")
            || line.contains("IDrawingRepresentation.GraphicOID");
        assert!(
            residual,
            "{line:?} is none of the three synthetic residuals"
        );
    }
    assert!(
        stderr.contains("surfaced findings in database mode"),
        "{stderr}"
    );
    assert!(!stderr.contains("is clean"), "{stderr}");
}

#[test]
fn cli_diff_against_self_generated_is_clean_and_exits_zero() {
    // Generate an _Data.xml from the MDF input, then diff it
    // against itself: the report must be clean and the binary
    // must exit 0.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("self-diff");
    let data_path = dir.join("A01_Data.xml");

    let gen = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
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
            A01_MDF_PATH,
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
        stderr.contains("is clean in database mode ("),
        "stderr should announce the clean verdict; got:\n{stderr}"
    );
}

#[test]
fn cli_diff_against_a_reference_with_one_value_changed_exits_one() {
    // The run's own output is a clean reference (exit 0, above). Change one
    // attribute of one object in a copy and the same run must exit 1 and
    // name that value: the gate scores values, not tag counts.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("mutated-diff");
    let data_path = dir.join("A01_Data.xml");
    let gen = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
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
    let own = std::fs::read_to_string(&data_path).expect("generated data xml");
    let original = format!("<IObject UID=\"{A01_DRAWING_UID}\" Name=\"A01\"");
    assert_eq!(own.matches(&original).count(), 1, "{own}");
    let mutated = own.replace(
        &original,
        &format!("<IObject UID=\"{A01_DRAWING_UID}\" Name=\"A01-changed\""),
    );
    let reference = dir.join("A01_Data_changed.xml");
    std::fs::write(&reference, mutated).expect("write the changed reference");

    let diff = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            reference.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against a changed reference");
    assert_eq!(diff.status.code(), Some(1), "{diff:?}");
    let stdout = String::from_utf8_lossy(&diff.stdout);
    let stderr = String::from_utf8_lossy(&diff.stderr);
    assert!(
        stdout.lines().any(|line| line
            == format!(
                "value PIDDrawing {A01_DRAWING_UID} IObject.Name: \"A01\" reference \"A01-changed\""
            )),
        "{stdout}"
    );
    assert!(
        stderr.contains("surfaced findings in database mode"),
        "{stderr}"
    );
}

/// The database run's own `_Data.xml`, written under `dir`.
fn a01_own_output(dir: &std::path::Path) -> String {
    let data_path = dir.join("A01_Data.xml");
    let gen = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
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
    std::fs::read_to_string(&data_path).expect("generated data xml")
}

/// The database run scored against `reference`, as (exit code, stdout, stderr).
fn a01_diff_against(reference: &std::path::Path) -> (Option<i32>, String, String) {
    let diff = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            reference.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against a changed reference");
    (
        diff.status.code(),
        String::from_utf8_lossy(&diff.stdout).into_owned(),
        String::from_utf8_lossy(&diff.stderr).into_owned(),
    )
}

#[test]
fn cli_diff_against_a_reference_missing_one_interface_exits_one() {
    // Every value of the object still agrees: only the interface element is
    // gone from the reference, so the run writes one the reference lacks.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("interface-diff");
    let own = a01_own_output(dir.path());
    let interface = "      <IDocVersionComposition/>\n";
    assert_eq!(own.matches(interface).count(), 1, "{own}");
    let reference = dir.join("A01_Data_no_interface.xml");
    std::fs::write(&reference, own.replace(interface, "")).expect("write the reference");

    let (code, stdout, stderr) = a01_diff_against(&reference);
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stdout.lines().any(|line| line
            == format!(
                "interface PIDDrawing {A01_DRAWING_UID} IDocVersionComposition not in reference"
            )),
        "{stdout}"
    );
    assert!(!stderr.contains("is clean"), "{stderr}");
}

#[test]
fn cli_diff_against_a_reference_of_another_plant_exits_one() {
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("plant-diff");
    let own = a01_own_output(dir.path());
    assert_eq!(own.matches(r#" Plant="TEST02""#).count(), 1, "{own}");
    let reference = dir.join("A01_Data_other_plant.xml");
    std::fs::write(
        &reference,
        own.replace(r#" Plant="TEST02""#, r#" Plant="P01""#),
    )
    .expect("write the reference");

    let (code, stdout, stderr) = a01_diff_against(&reference);
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stdout
            .lines()
            .any(|line| line == r#"container Plant: "TEST02" reference "P01""#),
        "{stdout}"
    );
    assert!(!stderr.contains("is clean"), "{stderr}");
}

#[test]
fn a_block_written_twice_is_not_clean_against_the_runs_own_output() {
    // The command line hands the gate a reference, never a generated
    // document, so the duplicate is planted on the generated side through
    // the gate the command line calls.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("duplicate-diff");
    let own = a01_own_output(dir.path());
    let start = own.find("   <PIDProcessVessel>").expect("a vessel block");
    let end = own[start..]
        .find("   </PIDProcessVessel>\n")
        .map(|at| start + at + "   </PIDProcessVessel>\n".len())
        .expect("the vessel block closes");
    let mut doubled = own.clone();
    doubled.insert_str(end, &own[start..end]);

    let clean = pid_parse::publish::diff::diff_pid_export(&own, &own).expect("own output parses");
    assert!(clean.is_clean(), "{clean}");
    let diff = pid_parse::publish::diff::diff_pid_export(&doubled, &own).expect("doubled parses");
    assert_eq!(diff.duplicate_uids_in_generated, 1, "{diff}");
    assert!(
        diff.findings
            .iter()
            .any(|f| f == "duplicate C57494A1B154442C9DF0F4BA713E88EC in generated"),
        "{diff}"
    );
    assert!(!diff.is_clean(), "{diff}");
}

#[test]
fn cli_diff_against_a_truncated_reference_is_refused_and_exits_one() {
    // The tag counts scan bytes: a reference that lost its closing tag counts
    // exactly like the run's own output, so it must be refused as not
    // well-formed XML rather than scored clean.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("truncated-diff");
    let data_path = dir.join("A01_Data.xml");
    let gen = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
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
    let full = std::fs::read_to_string(&data_path).expect("generated data xml");
    let cut = &full[..full.rfind("</Container>").expect("closing tag")];
    let reference = dir.join("A01_Data_cut.xml");
    std::fs::write(&reference, cut).expect("write the cut reference");

    let diff = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--diff-against",
            reference.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --diff-against a cut reference");
    assert_eq!(diff.status.code(), Some(1), "{diff:?}");
    let stdout = String::from_utf8_lossy(&diff.stdout);
    let stderr = String::from_utf8_lossy(&diff.stderr);
    for stream in [&stdout, &stderr] {
        assert!(stream.contains("not well-formed XML"), "{stream}");
        assert!(stream.contains("A01_Data_cut.xml"), "{stream}");
        assert!(!stream.contains("is clean"), "{stream}");
    }
}

#[test]
fn cli_help_documents_style_flag_and_choices() {
    // A29b advertises `--style a01|dwg`; --help is the only
    // discoverability surface for the flag, so a regression
    // that drops it from print_usage would silently strand
    // the new option.
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success(), "--help should exit 0; got {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--style a01|dwg"),
        "--help should document the --style flag; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("DWG export shape"),
        "--help should explain the DWG branch; stdout:\n{stdout}"
    );
}

#[test]
fn cli_style_unknown_value_exits_two_with_clear_error() {
    // Defensive: an obvious typo (`--style invalid`) must
    // exit with the usage-error code (2) rather than
    // silently falling back to A01 — silent fallback would
    // make CI runs against the wrong reference appear
    // clean for entirely unrelated reasons.
    let out = Command::new(binary_path())
        .args([
            "irrelevant.sqlite",
            "--drawing",
            "irrelevant",
            "--stdout",
            "--style",
            "klingon",
        ])
        .output()
        .expect("spawn pid_publish_xml --style klingon");
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown --style value should exit 2; got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--style accepts `a01` or `dwg`"),
        "error message should name the legal values; stderr:\n{stderr}"
    );
}

#[test]
fn cli_default_style_matches_a01_reference_delivery_contract() {
    // A01 is the only backup-backed correctness baseline.
    // The raw file still contains A39 evidence-guarded
    // publish-time synthetic values (connector-family UID,
    // Rel IObject UID seeds, and representation GraphicOID
    // numbering) that are not present in the full Rust MDF
    // table inventory or raw byte-form scan, so the delivery contract
    // normalizes only those slots and then demands the entire
    // `_Data.xml` match the bundled SmartPlant reference exactly.
    // The emitted bytes themselves must still be deterministic
    // across repeated CLI runs.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("style-default");
    let data_path = dir.join("A01_Data.xml");
    let out = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml default --style");
    assert!(
        out.status.success(),
        "default-style run should exit 0; got {out:?}"
    );
    let xml = std::fs::read_to_string(&data_path).expect("read generated xml");
    let reference_xml =
        std::fs::read_to_string(A01_REFERENCE_DATA_XML).expect("read A01 reference");
    assert!(
        !xml.is_empty(),
        "default-style output must produce non-empty _Data.xml",
    );
    assert_eq!(
        normalize_a01_delivery_contract(&xml),
        normalize_a01_delivery_contract(&reference_xml),
        "default style must match the bundled A01 publish reference under the delivery contract"
    );
    let out2 = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml default --style second time");
    assert!(
        out2.status.success(),
        "second default-style run should exit 0; got {out2:?}"
    );
    let xml2 = std::fs::read_to_string(&data_path).expect("re-read generated xml");
    assert_eq!(
        xml, xml2,
        "repeated CLI emits for the same A01 input must stay byte-identical"
    );
}

#[test]
fn cli_style_dwg_drops_itemtag_on_pipeline_iobject() {
    // A29b end-to-end test: --style dwg must flip the
    // PIDPipeline / PIDPipingConnector IObject shape to the
    // DWG convention. The TEST02 MDF fixture is an A01
    // plant so the data is itself A01-flavor; the test
    // therefore only asserts that the writer-side IObject
    // shape NO LONGER carries `ItemTag` on the pipeline
    // block (the writer's DWG branch drops ItemTag in favor
    // of Name when the loader has a name, or emits a
    // UID-only IObject otherwise). This pins the writer's
    // style switch independent of the loader's flavor.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("style-dwg");
    let data_path = dir.join("A01_Data.xml");
    let out = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--style",
            "dwg",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --style dwg");
    assert!(
        out.status.success(),
        "--style dwg run should exit 0; got {out:?}"
    );
    let xml = std::fs::read_to_string(&data_path).expect("read generated xml");
    assert!(
        xml.contains("<PIDPipeline>"),
        "DWG-style output must still include PIDPipeline; out:\n{xml}",
    );
    // Pipeline IObject must NOT carry an ItemTag in DWG
    // shape — that is the entire point of the flag.
    let pipeline_block_start = xml
        .find("<PIDPipeline>")
        .expect("PIDPipeline must be present");
    let pipeline_block_end = xml[pipeline_block_start..]
        .find("</PIDPipeline>")
        .expect("PIDPipeline must close")
        + pipeline_block_start;
    let pipeline_block = &xml[pipeline_block_start..pipeline_block_end];
    let iobject_line = pipeline_block
        .lines()
        .find(|l| l.contains("<IObject "))
        .expect("PIDPipeline must contain an IObject");
    assert!(
        !iobject_line.contains("ItemTag="),
        "DWG-style PIDPipeline IObject must drop ItemTag; got:\n{iobject_line}",
    );
}

#[test]
fn cli_style_case_insensitive_accepts_uppercase_dwg() {
    // The CLI accepts `--style DWG` / `--style A01` /
    // `--style Dwg` interchangeably; documenting this
    // spares users from tripping on case typos and
    // matches CLI-convention everywhere else (lower-case
    // canonical, upper-case tolerated).
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("style-case");
    let data_path = dir.join("A01_Data.xml");
    let out = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--style",
            "DWG",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml --style DWG");
    assert!(
        out.status.success(),
        "uppercase --style DWG should exit 0; got {out:?}"
    );
}

#[test]
fn cli_list_drawings_prints_table_and_summary_for_test02_fixture() {
    // A30 — `--list-drawings` reads T_Drawing and prints a
    // tab-aligned table on stdout plus a "Total: N drawing(s)"
    // summary on stderr. Pin both surfaces so a regression
    // (broken SQL, missing column, output to wrong stream)
    // trips immediately.
    if !fixture_available() {
        return;
    }
    let out = Command::new(binary_path())
        .args([A01_MDF_PATH, "--list-drawings"])
        .output()
        .expect("spawn pid_publish_xml --list-drawings");
    assert!(
        out.status.success(),
        "--list-drawings should exit 0; got {out:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("SP_ID"),
        "stdout should carry the column header; got:\n{stdout}"
    );
    assert!(
        stdout.contains(A01_DRAWING_UID),
        "TEST02 fixture's only drawing UID must appear in the listing; got:\n{stdout}"
    );
    assert!(
        stdout.contains("A01.pid"),
        "the A01 drawing's Path should appear in the listing; got:\n{stdout}"
    );
    assert!(
        stderr.contains("Total: 1 drawing"),
        "stderr should carry the summary line; got:\n{stderr}"
    );
}

#[test]
fn cli_list_drawings_rejects_combination_with_drawing_flag() {
    // The two modes are mutually exclusive — combining them
    // would conflate "list" output with rendered XML on the
    // same stdout stream and break shell pipelines.
    let out = Command::new(binary_path())
        .args([
            "irrelevant.sqlite",
            "--list-drawings",
            "--drawing",
            "FAKE_UID",
        ])
        .output()
        .expect("spawn pid_publish_xml --list-drawings --drawing");
    assert_eq!(
        out.status.code(),
        Some(2),
        "combining --list-drawings and --drawing should be a usage error; got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--list-drawings is mutually exclusive with --drawing"),
        "error must name the conflict; got:\n{stderr}"
    );
}

#[test]
fn cli_drawing_not_found_error_includes_list_drawings_hint() {
    // A30 attaches `use --list-drawings to see available drawing
    // UIDs` to the DrawingNotFound error path so an operator who
    // mistypes a UID immediately sees the recovery action.
    if !fixture_available() {
        return;
    }
    let out = Command::new(binary_path())
        .args([A01_MDF_PATH, "--drawing", "DOES_NOT_EXIST", "--stdout"])
        .output()
        .expect("spawn pid_publish_xml --drawing DOES_NOT_EXIST --stdout");
    assert_eq!(
        out.status.code(),
        Some(1),
        "missing drawing should exit 1 (I/O / data error); got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("DOES_NOT_EXIST"),
        "error should name the missing UID; got:\n{stderr}"
    );
    assert!(
        stderr.contains("use `--list-drawings`"),
        "DrawingNotFound error must surface the recovery hint; got:\n{stderr}"
    );
}

#[test]
fn cli_help_documents_list_drawings_flag() {
    // Discoverability: --help is the only place ops learn about
    // --list-drawings without reading the source.
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success(), "--help should exit 0; got {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--list-drawings"),
        "--help should advertise the --list-drawings flag; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Print every T_Drawing row"),
        "--help should describe what --list-drawings does; stdout:\n{stdout}"
    );
}

#[test]
fn cli_legacy_sqlite_list_drawings_smoke_still_works_with_deprecation_warning() {
    // `.sqlite` input remains supported only as a transition / legacy
    // path. This test intentionally avoids any publish-fidelity
    // assertions and only pins the minimal CLI compatibility surface:
    // the binary must still list drawings successfully and emit the
    // deprecation warning.
    if !legacy_sqlite_fixture_available() {
        return;
    }
    let out = Command::new(binary_path())
        .args([LEGACY_SQLITE_PATH, "--list-drawings"])
        .output()
        .expect("spawn pid_publish_xml legacy sqlite --list-drawings");
    assert!(
        out.status.success(),
        "legacy sqlite smoke should still exit 0; got {out:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains(A01_DRAWING_UID),
        "legacy sqlite smoke should still list the A01 drawing UID; got:\n{stdout}"
    );
    assert!(
        stderr.contains("deprecated for publish fidelity"),
        "legacy sqlite path should emit a deprecation warning; got:\n{stderr}"
    );
    assert!(
        stderr.contains("legacy compatibility adapter"),
        "warning should explain the remaining support level; got:\n{stderr}"
    );
}

#[test]
fn cli_diff_against_combined_with_out_writes_xml_and_reports() {
    // --diff-against composes with --out: the XML lands on disk
    // AND the report is printed. A01 carries its registered
    // synthetic residuals, so the combined flow exits 1.
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
            A01_MDF_PATH,
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
        "the combined flow against the A01 reference should exit 1; got {out:?}",
    );
    assert!(
        data_path.exists(),
        "--out should still produce the _Data.xml file when paired with --diff-against",
    );
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        report.contains("pid export diff: objects 12 "),
        "report on stdout should carry the pid export diff summary; got:\n{report}"
    );
}

#[test]
fn cli_pid_given_by_its_8_3_alias_is_published_under_its_long_name() {
    // An 8.3 alias is another path to the same file. The drawing is named
    // by the file's stored name, so the alias publishes what the long name
    // does: `DocName`, the drawing's `IObject.Name`, and the names of the
    // three files `--out-dir` writes.
    if !std::path::Path::new(DWG_PID_PATH).is_file() {
        eprintln!("skipping: fixture {DWG_PID_PATH} not found");
        return;
    }
    let dir = unique_tmp_dir("pid-8-3-alias");
    let long_stem = "a drawing reached by its short name";
    let long = dir.join(format!("{long_stem}.pid"));
    std::fs::copy(DWG_PID_PATH, &long).expect("copy the fixture under a long name");
    let Some(alias) = common::short_path_of(&long) else {
        return;
    };
    eprintln!("8.3 alias under test: {}", alias.display());

    let out = Command::new(binary_path())
        .arg(&alias)
        .arg("--stdout")
        .output()
        .expect("spawn pid_publish_xml <8.3 alias> --stdout");
    assert!(
        out.status.success(),
        "publishing a .pid by its 8.3 alias should exit 0; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let xml = String::from_utf8_lossy(&out.stdout);
    let mut lines = xml.lines().map(str::trim);
    let container = lines
        .by_ref()
        .find(|line| line.starts_with("<Container "))
        .expect("the document opens a <Container>");
    assert!(
        container.contains(&format!(" DocName=\"{long_stem}\"")),
        "DocName should be the file's stored name, not the alias; got:\n{container}"
    );
    let drawing_object = lines
        .by_ref()
        .skip_while(|line| *line != "<PIDDrawing>")
        .nth(1)
        .expect("the document carries a <PIDDrawing>");
    assert!(
        drawing_object.starts_with("<IObject ")
            && drawing_object.contains(&format!(" Name=\"{long_stem}\"")),
        "the drawing's IObject.Name should be the file's stored name; got:\n{drawing_object}"
    );

    let out_dir = dir.join("out");
    let out = Command::new(binary_path())
        .arg(&alias)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("spawn pid_publish_xml <8.3 alias> --out-dir");
    assert!(
        out.status.success(),
        "--out-dir by the 8.3 alias should exit 0; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut written: Vec<String> = std::fs::read_dir(&out_dir)
        .expect("--out-dir was created")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    written.sort();
    assert_eq!(
        written,
        [
            format!("{long_stem}_Coverage.json"),
            format!("{long_stem}_Data.xml"),
            format!("{long_stem}_Meta.xml"),
        ],
        "--out-dir should name its files by the file's stored name"
    );
}

#[test]
fn cli_pid_given_in_another_letter_case_is_published_under_its_stored_name() {
    // Where the file system ignores case, a path typed in another case is
    // one more path to the same file, and names the drawing as little as an
    // 8.3 alias does.
    if !std::path::Path::new(DWG_PID_PATH).is_file() {
        eprintln!("skipping: fixture {DWG_PID_PATH} not found");
        return;
    }
    let dir = unique_tmp_dir("pid-letter-case");
    let stored_stem = "A Drawing Stored In Mixed Case";
    std::fs::copy(DWG_PID_PATH, dir.join(format!("{stored_stem}.pid")))
        .expect("copy the fixture under a mixed-case name");
    let typed = dir.join(format!("{}.PID", stored_stem.to_uppercase()));
    if !typed.is_file() {
        eprintln!(
            "skipping: this file system tells letter case apart, so {} is not the copy",
            typed.display()
        );
        return;
    }

    let out = Command::new(binary_path())
        .arg(&typed)
        .arg("--stdout")
        .output()
        .expect("spawn pid_publish_xml <path in upper case> --stdout");
    assert!(
        out.status.success(),
        "publishing a .pid by a path in another case should exit 0; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let xml = String::from_utf8_lossy(&out.stdout);
    let container = xml
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("<Container "))
        .expect("the document opens a <Container>");
    assert!(
        container.contains(&format!(" DocName=\"{stored_stem}\"")),
        "DocName should be spelled as the file is stored; got:\n{container}"
    );
}

// ---------------------------------------------------------------------
// What the database line writes short: the warnings and `--strict`
// ---------------------------------------------------------------------

/// A plant database of one drawing that the database line cannot write in
/// full: an exchanger, whose type has no writer of its own, and a
/// relationship row with an item at each end. No real database in the
/// repository holds either, so this one is made on the spot.
const SHORTFALLS_SQL: &str = include_str!("fixtures/publish-shortfalls.sql");
const SHORTFALLS_DRAWING_UID: &str = "DWG-SHORT-01";
const PLACEHOLDER_WARNING: &str = "warning: 1 object written as a <PIDItem> placeholder, \
                                   class and attributes left out: Exchanger 1";
const RELATIONSHIP_WARNING: &str = "warning: 1 relationship row defined from the item types of \
                                    its ends alone: Nozzle -> Exchanger as Relationship 1";

/// Build the fixture database as `<dir>/shortfalls.sqlite`.
fn shortfalls_database(dir: &Scratch) -> std::path::PathBuf {
    let path = dir.join("shortfalls.sqlite");
    let conn = rusqlite::Connection::open(&path).expect("create the fixture database");
    conn.execute_batch(SHORTFALLS_SQL)
        .expect("run tests/fixtures/publish-shortfalls.sql");
    path
}

/// The `warning:` lines of a run's stderr, apart from the notice every
/// `.sqlite` input prints.
fn warnings_in(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("warning: "))
        .filter(|line| !line.contains("`.sqlite` input is deprecated"))
        .collect()
}

/// Publish the fixture drawing into `<out>/SHORT-01_Data.xml` and
/// `<out>/SHORT-01_Meta.xml`, with `extra` flags, and return the run.
fn publish_shortfalls_drawing(
    database: &std::path::Path,
    out: &std::path::Path,
    extra: &[&str],
) -> std::process::Output {
    Command::new(binary_path())
        .arg(database)
        .args(["--drawing", SHORTFALLS_DRAWING_UID])
        .args(extra)
        .arg("--out")
        .arg(out.join("SHORT-01_Data.xml"))
        .arg("--meta-out")
        .arg(out.join("SHORT-01_Meta.xml"))
        .output()
        .expect("spawn pid_publish_xml on the shortfalls database")
}

#[test]
fn cli_database_input_warns_of_what_it_wrote_short_and_exits_zero() {
    // The exchanger is written as a placeholder and the relationship row
    // gets its definition from the two item types alone. Both are written,
    // the exit code stays 0, and stderr says one line for each kind.
    let dir = unique_tmp_dir("shortfalls-warned");
    let database = shortfalls_database(&dir);

    let out = publish_shortfalls_drawing(&database, &dir, &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a shortfall without --strict should leave the exit code at 0; stderr:\n{stderr}"
    );
    assert_eq!(
        warnings_in(&stderr),
        [PLACEHOLDER_WARNING, RELATIONSHIP_WARNING],
        "stderr should say each kind of shortfall once; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("--strict"),
        "nothing should speak of --strict when it was not given; stderr:\n{stderr}"
    );
    let xml = std::fs::read_to_string(dir.join("SHORT-01_Data.xml"))
        .expect("--out should have written the _Data.xml");
    assert_eq!(
        xml.matches("<PIDItem>").count(),
        1,
        "the exchanger is the one placeholder the warning counts; xml:\n{xml}"
    );
    assert_eq!(
        xml.matches(r#"DefUID="Relationship""#).count(),
        1,
        "the nozzle-to-exchanger row is the one Rel the warning counts; xml:\n{xml}"
    );
}

#[test]
fn cli_strict_turns_a_shortfall_into_exit_one_and_changes_no_file() {
    // `--strict` changes the exit code and nothing in the delivery: both
    // documents are written, byte for byte what the run without it writes,
    // the warnings are the same, and the last line on stderr says the flag
    // is why the run exits 1.
    let dir = unique_tmp_dir("shortfalls-strict");
    let database = shortfalls_database(&dir);
    let (plain_dir, strict_dir) = (dir.join("plain"), dir.join("strict"));

    let plain = publish_shortfalls_drawing(&database, &plain_dir, &[]);
    assert_eq!(
        plain.status.code(),
        Some(0),
        "the run without --strict should exit 0; stderr:\n{}",
        String::from_utf8_lossy(&plain.stderr)
    );

    let strict = publish_shortfalls_drawing(&database, &strict_dir, &["--strict"]);
    let stderr = String::from_utf8_lossy(&strict.stderr);
    assert_eq!(
        strict.status.code(),
        Some(1),
        "--strict should exit 1 on a shortfall; stderr:\n{stderr}"
    );
    assert_eq!(
        warnings_in(&stderr),
        [PLACEHOLDER_WARNING, RELATIONSHIP_WARNING],
        "--strict should not change the warnings; stderr:\n{stderr}"
    );
    let last = stderr.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("--strict: ") && last.ends_with("exit 1."),
        "the last stderr line should name --strict as the reason for exit 1; stderr:\n{stderr}"
    );
    for name in ["SHORT-01_Data.xml", "SHORT-01_Meta.xml"] {
        let written = std::fs::read(strict_dir.join(name))
            .unwrap_or_else(|e| panic!("--strict should still write {name}: {e}"));
        let expected = std::fs::read(plain_dir.join(name))
            .unwrap_or_else(|e| panic!("the run without --strict should write {name}: {e}"));
        assert!(!written.is_empty(), "{name} should not be empty");
        assert!(
            written == expected,
            "{name} should be the same bytes with and without --strict"
        );
    }
}

#[test]
fn cli_strict_is_decided_after_the_diff_report_not_instead_of_it() {
    // Against the run's own output the --diff-against report is clean. It
    // is still printed, and then --strict exits 1 for the shortfalls.
    let dir = unique_tmp_dir("shortfalls-strict-diff");
    let database = shortfalls_database(&dir);
    let plain = publish_shortfalls_drawing(&database, &dir, &[]);
    assert_eq!(
        plain.status.code(),
        Some(0),
        "the run that writes the reference should exit 0; stderr:\n{}",
        String::from_utf8_lossy(&plain.stderr)
    );

    let out = Command::new(binary_path())
        .arg(&database)
        .args(["--drawing", SHORTFALLS_DRAWING_UID, "--strict"])
        .arg("--diff-against")
        .arg(dir.join("SHORT-01_Data.xml"))
        .output()
        .expect("spawn pid_publish_xml --strict --diff-against");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a clean diff should not undo --strict; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("pid export diff: objects 7 "),
        "the diff report should still be on stdout; got:\n{stdout}"
    );
    assert!(
        stderr.contains("is clean in database mode"),
        "the diff verdict should still be on stderr; got:\n{stderr}"
    );
    let last = stderr.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("--strict: ") && last.ends_with("exit 1."),
        "the last stderr line should name --strict as the reason for exit 1; stderr:\n{stderr}"
    );
}

#[test]
fn cli_strict_on_a_drawing_written_in_full_exits_zero() {
    // A01 has no item without a writer of its own and no relationship row
    // between items, so --strict finds nothing to object to.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("strict-a01");
    let data_path = dir.join("A01_Data.xml");

    let out = Command::new(binary_path())
        .args([
            A01_MDF_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--strict",
            "--out",
        ])
        .arg(&data_path)
        .output()
        .expect("spawn pid_publish_xml --strict on A01");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "A01 is written in full, so --strict should exit 0; stderr:\n{stderr}"
    );
    for phrase in [
        "<PIDItem> placeholder",
        "defined from the item types",
        "--strict",
    ] {
        assert!(
            !stderr.contains(phrase),
            "A01 should print nothing about `{phrase}`; stderr:\n{stderr}"
        );
    }
    assert!(
        data_path.is_file(),
        "--strict should not keep the _Data.xml from being written"
    );
    eprintln!("A01 with --strict: exit 0, stderr:\n{stderr}");
}

#[test]
fn cli_strict_with_a_pid_input_is_an_argument_error() {
    // A `.pid` is read without a plant database, so what --strict gates on
    // cannot arise there. Like --style, the flag is refused rather than
    // accepted and ignored.
    let out = Command::new(binary_path())
        .args(["irrelevant.pid", "--stdout", "--strict"])
        .output()
        .expect("spawn pid_publish_xml <file.pid> --strict");
    assert_eq!(
        out.status.code(),
        Some(2),
        "--strict with a .pid input should exit 2; got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(
            "argument error: a .pid input is read without a plant database: \
             --strict does not apply"
        ),
        "the error should say why --strict does not apply; stderr:\n{stderr}"
    );
}

#[test]
fn cli_strict_with_list_drawings_is_an_argument_error() {
    // --list-drawings writes no drawing, so there is nothing for --strict
    // to gate on.
    let out = Command::new(binary_path())
        .args(["irrelevant.sqlite", "--list-drawings", "--strict"])
        .output()
        .expect("spawn pid_publish_xml --list-drawings --strict");
    assert_eq!(
        out.status.code(),
        Some(2),
        "--strict with --list-drawings should exit 2; got {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("argument error: --list-drawings is mutually exclusive with --strict"),
        "the error should name the two flags; stderr:\n{stderr}"
    );
}

#[test]
fn cli_strict_with_a_backup_directory_is_accepted() {
    // Which kind a package is shows only once it is read, so --strict is
    // not refused for a directory. This one has no dump and no drawing:
    // nothing goes through the database line, and the run is the run
    // without the flag.
    let dir = unique_tmp_dir("strict-package");
    let package = dir.join("package");
    std::fs::create_dir_all(&package).expect("create the package directory");
    std::fs::write(package.join("Manifest.txt"), "").expect("write Manifest.txt");

    let publish = |out_name: &str, extra: &[&str]| {
        let out_dir = dir.join(out_name);
        let out = Command::new(binary_path())
            .arg(&package)
            .arg("--out-dir")
            .arg(&out_dir)
            .args(extra)
            .output()
            .expect("spawn pid_publish_xml <backup dir>");
        // stderr names the output directory; nothing else may differ.
        let stderr =
            String::from_utf8_lossy(&out.stderr).replace(&out_dir.display().to_string(), "<out>");
        (out.status.code(), stderr, out_dir)
    };
    let (plain_code, plain_stderr, plain_out) = publish("plain", &[]);
    let (strict_code, strict_stderr, strict_out) = publish("strict", &["--strict"]);
    assert_eq!(plain_code, Some(0), "stderr:\n{plain_stderr}");
    assert_eq!(
        strict_code,
        Some(0),
        "--strict with a backup directory should be accepted; stderr:\n{strict_stderr}"
    );
    assert_eq!(
        strict_stderr, plain_stderr,
        "with no database drawing --strict should change nothing on stderr"
    );
    let index = |out_dir: &std::path::Path| {
        std::fs::read(out_dir.join("_Index.json")).expect("read _Index.json")
    };
    assert!(
        index(&strict_out) == index(&plain_out),
        "with no database drawing --strict should change nothing in _Index.json"
    );
}

#[test]
fn cli_strict_on_a_sql_server_package_written_in_full_exits_zero() {
    // TEST02_p is a SQL Server package whose one drawing, A01, is written
    // in full: its coverage report says so, and --strict has nothing to
    // object to.
    let package = std::path::Path::new("test-file/backup-test/TEST02_p");
    if !package.join("Manifest.txt").is_file() || !package.join("Export.dmp").is_file() {
        eprintln!("skipping: test-file/backup-test/TEST02_p is not a backup package");
        return;
    }
    let dir = unique_tmp_dir("strict-test02");
    let out_dir = dir.join("out");

    let out = Command::new(binary_path())
        .arg(package)
        .arg("--out-dir")
        .arg(&out_dir)
        .args(["--plant", "TEST02", "--strict"])
        .output()
        .expect("spawn pid_publish_xml TEST02_p --strict");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "TEST02_p is written in full, so --strict should exit 0; stderr:\n{stderr}"
    );
    assert!(
        warnings_in(&stderr).is_empty() && !stderr.contains("--strict"),
        "TEST02_p should print no warning and nothing about --strict; stderr:\n{stderr}"
    );
    let json = |file: &str| -> serde_json::Value {
        let text = std::fs::read_to_string(out_dir.join(file))
            .unwrap_or_else(|error| panic!("{file}: {error}"));
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{file}: {error}"))
    };
    let coverage = json("A01_Coverage.json");
    assert_eq!(
        coverage["shortfalls"],
        serde_json::json!({
            "placeholder_objects": { "total": 0, "by_item_type": {} },
            "relationship_rows_defined_by_item_type": { "total": 0, "rows": [] },
        }),
        "A01_Coverage.json should say that nothing was written short: {coverage}"
    );
    let index = json("_Index.json");
    assert_eq!(index["failed"], 0, "{index}");
    assert_eq!(index["problems"], serde_json::json!([]), "{index}");
}

#[test]
fn cli_help_documents_strict_and_drops_the_unreachable_feature_note() {
    let out = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("spawn pid_publish_xml --help");
    assert!(out.status.success(), "--help should exit 0; got {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("[--strict]"),
        "the synopsis should list --strict; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("\n--strict  "),
        "--help should describe what --strict does; stdout:\n{stdout}"
    );
    // A backup directory takes the flag too.
    assert!(
        stdout.contains("<backup-dir> --out-dir DIR [--plant NAME] [--strict]"),
        "the backup directory synopsis should list --strict; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("--strict            Database input or backup directory: "),
        "--help should say --strict applies to a backup directory; stdout:\n{stdout}"
    );
    // The binary is built with the `backup` feature only, and that includes
    // `backup-mdf`: there is no build of it in which a SQL Server package
    // "still exits 2".
    for unreachable in ["still exits 2", "backup-mdf feature"] {
        assert!(
            !stdout.contains(unreachable),
            "--help should not say `{unreachable}`; stdout:\n{stdout}"
        );
    }
}
