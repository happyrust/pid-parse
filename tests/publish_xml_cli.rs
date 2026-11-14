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
