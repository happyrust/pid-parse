//! End-to-end CLI tests for the `pid_publish_xml` binary (A10
//! series). Drives the actual compiled binary via `Command` so the
//! whole stack — argument parsing, MDF read, DTO load,
//! `_Data.xml` + `_Meta.xml` rendering, and file I/O — is exercised
//! the same way an operator would invoke it.
//!
//! All tests that touch the real `Export.mdf` fixture skip
//! cleanly when the file is missing so CI workers without the
//! TEST02 backup do not fail.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_publish_xml")
}

const SQLITE_PATH: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const A01_REFERENCE_DATA_XML: &str = "test-file/export-test/publish-data/A01/A01_Data.xml";

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

/// True when the real MDF fixture is on disk. Returns false (and
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

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn replace_attr_value(line: &str, attr: &str, replacement: &str) -> String {
    let needle = format!(r#"{attr}=""#);
    let Some(start) = line.find(&needle) else {
        return line.to_string();
    };
    let value_start = start + needle.len();
    let Some(rel_end) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + rel_end;
    format!(
        "{}{}{}",
        &line[..value_start],
        replacement,
        &line[value_end..]
    )
}

fn normalize_a01_delivery_contract(xml: &str) -> String {
    let mut current_block: Option<&'static str> = None;
    let mut uid_aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut port_count = 0usize;
    let mut rep_count = 0usize;
    let mut rel_count = 0usize;
    let mut out = Vec::new();

    for raw in xml.replace("\r\n", "\n").lines() {
        let trimmed = raw.trim_start();
        match trimmed {
            "<PIDDrawing>" => current_block = Some("PIDDrawing"),
            "<PIDProcessVessel>" => current_block = Some("PIDProcessVessel"),
            "<PIDNozzle>" => current_block = Some("PIDNozzle"),
            "<PIDPipeline>" => current_block = Some("PIDPipeline"),
            "<PIDPipingConnector>" => current_block = Some("PIDPipingConnector"),
            "<PIDPipingPort>" => current_block = Some("PIDPipingPort"),
            "<PIDProcessPoint>" => current_block = Some("PIDProcessPoint"),
            "<PIDRepresentation>" => current_block = Some("PIDRepresentation"),
            "<Rel>" => current_block = Some("Rel"),
            _ => {}
        }

        let mut line = raw.to_string();
        if trimmed.starts_with("<IObject ") {
            if let Some(uid) = extract_attr(&line, "UID") {
                let alias = match current_block {
                    Some("PIDDrawing") => Some("@DRAWING@".to_string()),
                    Some("PIDProcessVessel") => Some("@VESSEL@".to_string()),
                    Some("PIDNozzle") => Some("@NOZZLE@".to_string()),
                    Some("PIDPipeline") => Some("@PIPELINE@".to_string()),
                    Some("PIDPipingConnector") => Some("@CONNECTOR@".to_string()),
                    Some("PIDPipingPort") => {
                        port_count += 1;
                        Some(format!("@PORT{port_count}@"))
                    }
                    Some("PIDProcessPoint") => Some("@PROCESS_POINT@".to_string()),
                    Some("PIDRepresentation") => {
                        rep_count += 1;
                        Some(format!("@REP{rep_count}@"))
                    }
                    Some("Rel") => {
                        rel_count += 1;
                        Some(format!("@REL{rel_count}@"))
                    }
                    _ => None,
                };
                if let Some(alias) = alias {
                    if current_block != Some("Rel") {
                        uid_aliases.insert(uid, alias.clone());
                    }
                    line = replace_attr_value(&line, "UID", &alias);
                }
            }
        }

        if trimmed.starts_with("<IDrawingRepresentation ") {
            line = replace_attr_value(&line, "GraphicOID", "@GRAPHIC@");
        }

        if trimmed.starts_with("<IRel ") {
            for attr in ["UID1", "UID2"] {
                if let Some(value) = extract_attr(&line, attr) {
                    if let Some(alias) = uid_aliases.get(&value) {
                        line = replace_attr_value(&line, attr, alias);
                    }
                }
            }
        }

        out.push(line);

        match trimmed {
            "</PIDDrawing>"
            | "</PIDProcessVessel>"
            | "</PIDNozzle>"
            | "</PIDPipeline>"
            | "</PIDPipingConnector>"
            | "</PIDPipingPort>"
            | "</PIDProcessPoint>"
            | "</PIDRepresentation>"
            | "</Rel>" => current_block = None,
            _ => {}
        }
    }

    let mut normalized = out.join("\n");
    normalized.push('\n');
    normalized
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
        stderr.contains("missing <mdf|sqlite>"),
        "stderr should explain the missing argument; got:\n{stderr}"
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
        "_Meta.xml DocVersionDate should normalize the MDF date `2026/4/20 ...` to ISO-ish; got:\n{meta_xml}"
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
fn cli_diff_against_real_a01_reference_is_clean_and_exits_zero() {
    // A14 closes the last semantic diff against the SmartPlant
    // reference A01 export: filtering pure annotation/label
    // representations brings PIDRepresentation from 6 to 4 and the
    // report goes fully clean. This test pins the post-A14
    // milestone — the writer now reproduces the reference fixture's
    // PID tag set exactly.
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
    assert_eq!(
        out.status.code(),
        Some(0),
        "A01 reference diff should exit 0 (clean); got {out:?}",
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stdout.contains("Publish Data XML semantic diff"),
        "stdout should carry the SemanticDiffReport header; got:\n{stdout}"
    );
    // All eight tag varieties on A01 must be MATCH.
    for tag in [
        "PIDDrawing",
        "PIDNozzle",
        "PIDPipeline",
        "PIDPipingConnector",
        "PIDPipingPort",
        "PIDProcessPoint",
        "PIDProcessVessel",
        "PIDRepresentation",
    ] {
        assert!(
            stdout.contains(tag),
            "report should list {tag}; got:\n{stdout}"
        );
    }
    // No actionable rows allowed.
    assert!(
        !stdout.contains("MISSING"),
        "no MISSING rows expected; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("EXTRA"),
        "no EXTRA rows expected; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("DELTA"),
        "no DELTA rows expected; got:\n{stdout}"
    );
    assert!(
        stderr.contains("clean ("),
        "stderr should announce the clean verdict; got:\n{stderr}"
    );
    assert!(
        stderr.contains("8 matching"),
        "summary should report eight matching tag varieties; got:\n{stderr}"
    );
    // A40: the Rel DefUID section must also appear and be
    // clean (every DefUID matches). The summary line on
    // stderr is expected to carry both counts.
    assert!(
        stdout.contains("Publish Data XML Rel DefUID diff"),
        "stdout should carry the A40 Rel diff header; got:\n{stdout}"
    );
    for def_uid in [
        "DrawingItems",
        "DwgRepresentationComposition",
        "EquipmentComponentComposition",
        "PipingConnectors",
        "PipingEnd1Conn",
        "PipingEnd2Conn",
        "PipingPortComposition",
        "ProcessPointCollection",
    ] {
        assert!(
            stdout.contains(def_uid),
            "Rel block should list `{def_uid}`; got:\n{stdout}"
        );
    }
    assert!(
        stderr.contains("matching Rel DefUIDs"),
        "stderr summary should count matching Rel DefUIDs; got:\n{stderr}"
    );
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
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--style a01|dwg"),
        "--help should document the --style flag; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("DWG export shape"),
        "--help should explain the DWG branch; stderr:\n{stderr}"
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
    // The raw file still contains publish-time synthetic
    // values (representation GraphicOID numbering and Rel
    // IObject UID seeds) that are not reconstructable from
    // the publish source alone, so the delivery contract
    // normalizes those unstable slots and then demands the
    // entire `_Data.xml` match the bundled SmartPlant
    // reference exactly. The emitted bytes themselves must
    // still be deterministic across repeated CLI runs.
    if !fixture_available() {
        return;
    }
    let dir = unique_tmp_dir("style-default");
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
        ])
        .output()
        .expect("spawn pid_publish_xml default --style");
    assert!(out.status.success(), "default-style run should exit 0; got {out:?}");
    let xml = std::fs::read_to_string(&data_path).expect("read generated xml");
    let reference_xml = std::fs::read_to_string(A01_REFERENCE_DATA_XML).expect("read A01 reference");
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
            SQLITE_PATH,
            "--drawing",
            A01_DRAWING_UID,
            "--plant",
            "TEST02",
            "--out",
            data_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn pid_publish_xml default --style second time");
    assert!(out2.status.success(), "second default-style run should exit 0; got {out2:?}");
    let xml2 = std::fs::read_to_string(&data_path).expect("re-read generated xml");
    assert_eq!(
        xml, xml2,
        "repeated CLI emits for the same A01 input must stay byte-identical"
    );
    let _ = std::fs::remove_dir_all(&dir);
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
            SQLITE_PATH,
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
    assert!(out.status.success(), "--style dwg run should exit 0; got {out:?}");
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
    let _ = std::fs::remove_dir_all(&dir);
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
            SQLITE_PATH,
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
    let _ = std::fs::remove_dir_all(&dir);
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
        .args([SQLITE_PATH, "--list-drawings"])
        .output()
        .expect("spawn pid_publish_xml --list-drawings");
    assert!(out.status.success(), "--list-drawings should exit 0; got {out:?}");
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
        .args([
            SQLITE_PATH,
            "--drawing",
            "DOES_NOT_EXIST",
            "--stdout",
        ])
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
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--list-drawings"),
        "--help should advertise the --list-drawings flag; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Print every T_Drawing row"),
        "--help should describe what --list-drawings does; stderr:\n{stderr}"
    );
}

#[test]
fn cli_diff_against_combined_with_out_writes_xml_and_reports_clean() {
    // --diff-against composes with --out: the XML lands on disk
    // AND the report is printed. Post-A14 the A01 reference is
    // semantically clean, so the combined flow exits 0.
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
        Some(0),
        "post-A14 the combined flow against A01 reference should exit 0; got {out:?}",
    );
    assert!(
        data_path.exists(),
        "--out should still produce the _Data.xml file when paired with --diff-against",
    );
    let report = String::from_utf8_lossy(&out.stdout);
    // The report content survives even on clean exits — it is the
    // canonical artifact a CI run records for traceability.
    assert!(
        report.contains("Publish Data XML semantic diff"),
        "report on stdout should carry the SemanticDiffReport header; got:\n{report}"
    );
    assert!(
        report.contains("PIDRepresentation"),
        "report should list PIDRepresentation as MATCH; got:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
