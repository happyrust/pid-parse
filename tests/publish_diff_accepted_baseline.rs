//! The accepted `--diff-against` differences, verified item by item against
//! `docs/baselines/publish-diff-accepted.json`.
//!
//! The baseline is data, not a suppression list. This suite proves the gate
//! still fails when a registered difference disappears and when an
//! unregistered one appears; nothing here relaxes an exit code.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

// Every scratch directory here is a `Scratch` under the system temp dir,
// removed when it drops. Nothing is ever written inside the repository or
// next to a fixture.
mod common;
use common::Scratch;

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");
const BASELINE_REL: &str = "docs/baselines/publish-diff-accepted.json";
const A01_DIR: &str = "test-file/export-test/publish-data/A01";
const A01_PACKAGE: &str = "test-file/backup-test/TEST02_p";
const A01_MDF: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const A01_DATABASE_FIGURE: &str = "A01 · Export.mdf";
const A01_PACKAGE_FIGURE: &str = "A01 · TEST02_p 目录";
/// The three synthetic residuals the database line leaves on A01.
const A01_DATABASE_RESIDUALS: [&str; 3] = ["connector_uid", "rel_uid", "graphic_oid"];
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CORPUS_DEFAULT: &str = r"D:\work\cad\PID自研软件验证";
const PACKAGE_DEFAULT: &str = r"D:\work\cad\pid-test-data";
const PACKAGE_ENV: &str = "PID_TEST_DATA_DIR";
const ACCEPTANCE_ENV: &str = "PID_ACCEPTANCE_CORPUS";
const ACCEPTANCE_DEFAULT: &str = r"D:\work\cad\pid-test-data\验收数据";
const IJ_STEM: &str = "I井场 注采阀组工艺及自控流程图";

/// The reference object the deleted-object probe removes.
const PROBE_OBJECT_UID: &str = "C57494A1B154442C9DF0F4BA713E88EC";
/// The drawing object whose generated name the renamed-input probe changes.
const DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";

// ---------------------------------------------------------------- baseline --

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn baseline_text() -> String {
    let path = manifest_dir().join(BASELINE_REL);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn baseline() -> Value {
    let path = manifest_dir().join(BASELINE_REL);
    serde_json::from_str(&baseline_text())
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

// ------------------------------------------------------------ line kinds ----

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Class,
    Extra,
    ValueDifferent,
    ValueNotInReference,
    RelNotInReference,
    MissingObject,
    MissingRel,
    InterfaceNotInReference,
    Container,
    Duplicate,
}

impl Kind {
    fn key(self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Extra => "extra",
            Kind::ValueDifferent => "value_different",
            Kind::ValueNotInReference => "value_not_in_reference",
            Kind::RelNotInReference => "rel_not_in_reference",
            Kind::MissingObject => "missing_object",
            Kind::MissingRel => "missing_rel",
            Kind::InterfaceNotInReference => "interface_not_in_reference",
            Kind::Container => "container",
            Kind::Duplicate => "duplicate",
        }
    }

    /// The kind of a printed difference line, or `None` for accounting lines
    /// (`meta ...`, `reference-only:`, `excused ...`, `structure: ...`) and
    /// the pairing note.
    fn of(line: &str) -> Option<Self> {
        if line.starts_with("class ") {
            Some(Kind::Class)
        } else if line.starts_with("extra ") {
            Some(Kind::Extra)
        } else if line.starts_with("interface ") {
            Some(Kind::InterfaceNotInReference)
        } else if line.starts_with("container ") {
            Some(Kind::Container)
        } else if line.starts_with("duplicate ") {
            Some(Kind::Duplicate)
        } else if line.starts_with("missing rel (") {
            Some(Kind::MissingRel)
        } else if line.starts_with("missing ") {
            Some(Kind::MissingObject)
        } else if line.starts_with("value ") {
            if line.contains(" not in reference") {
                Some(Kind::ValueNotInReference)
            } else {
                Some(Kind::ValueDifferent)
            }
        } else if line.starts_with("rel (") {
            Some(Kind::RelNotInReference)
        } else {
            None
        }
    }
}

fn registered_lines(figure: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for section in ["differences", "reference_only_differences"] {
        let map = match figure.get(section).and_then(Value::as_object) {
            Some(map) => map,
            None => continue,
        };
        for (kind, entry) in map {
            let items = match entry.get("items").and_then(Value::as_array) {
                Some(items) => items,
                None => continue,
            };
            for item in items {
                let line = item.get("line").and_then(Value::as_str).unwrap_or_default();
                out.insert(format!("{kind}\t{line}"));
            }
        }
    }
    out
}

fn printed_lines(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .filter_map(|line| Kind::of(line).map(|kind| format!("{}\t{line}", kind.key())))
        .collect()
}

/// The three states the baseline must keep apart.
struct Classification {
    registered: BTreeSet<String>,
    disappeared: BTreeSet<String>,
    unregistered: BTreeSet<String>,
}

impl Classification {
    fn of(figure: &Value, run: &Run) -> Self {
        let registered = registered_lines(figure);
        let printed = printed_lines(&run.stdout);
        Self {
            disappeared: registered.difference(&printed).cloned().collect(),
            unregistered: printed.difference(&registered).cloned().collect(),
            registered: registered.intersection(&printed).cloned().collect(),
        }
    }

    fn assert_complete(&self, what: &str) {
        assert!(
            self.unregistered.is_empty(),
            "{what}: unregistered difference(s): {:#?}",
            self.unregistered
        );
        assert!(
            self.disappeared.is_empty(),
            "{what}: registered difference(s) no longer printed: {:#?}",
            self.disappeared
        );
    }
}

// -------------------------------------------------------------- running -----

struct Run {
    code: i32,
    stdout: String,
}

impl Run {
    fn summary(&self) -> String {
        self.stdout
            .lines()
            .find(|line| line.starts_with("pid export diff: "))
            .unwrap_or_default()
            .to_string()
    }
}

fn run_cli<I, S>(args: I) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(CLI)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("spawn {CLI}: {error}"));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

fn a01_pid() -> PathBuf {
    manifest_dir().join(A01_DIR).join("A01.pid")
}

fn a01_reference() -> PathBuf {
    manifest_dir().join(A01_DIR).join("A01_Data.xml")
}

fn a01_run(pid: &Path, reference: &Path) -> Run {
    run_cli([
        pid.as_os_str(),
        OsStr::new("--diff-against"),
        reference.as_os_str(),
        OsStr::new("--plant"),
        OsStr::new("TEST02"),
    ])
}

fn a01_mdf() -> PathBuf {
    manifest_dir().join(A01_MDF)
}

fn a01_package() -> PathBuf {
    manifest_dir().join(A01_PACKAGE)
}

/// The database publish of A01 against `reference`, or `None` (with a
/// `skipping:` line) when the database or the reference is absent.
fn a01_database_run(reference: &Path) -> Option<Run> {
    let mdf = a01_mdf();
    for path in [&mdf, &reference.to_path_buf()] {
        if !path.is_file() {
            eprintln!("skipping: {} is absent", path.display());
            return None;
        }
    }
    Some(run_cli([
        mdf.as_os_str(),
        OsStr::new("--drawing"),
        OsStr::new(DRAWING_UID),
        OsStr::new("--plant"),
        OsStr::new("TEST02"),
        OsStr::new("--diff-against"),
        reference.as_os_str(),
    ]))
}

fn corpus_dir() -> PathBuf {
    match std::env::var_os(CORPUS_ENV) {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(CORPUS_DEFAULT),
    }
}

fn ij_pid() -> PathBuf {
    corpus_dir().join(format!("{IJ_STEM}.pid"))
}

fn ij_reference() -> PathBuf {
    corpus_dir().join(format!("{IJ_STEM}_Data.xml"))
}

fn package_dir() -> PathBuf {
    std::env::var_os(PACKAGE_ENV).map_or_else(|| PathBuf::from(PACKAGE_DEFAULT), PathBuf::from)
}

fn acceptance_dir() -> PathBuf {
    std::env::var_os(ACCEPTANCE_ENV)
        .map_or_else(|| PathBuf::from(ACCEPTANCE_DEFAULT), PathBuf::from)
}

/// Run one `acceptance_figures` entry in both modes and hold every printed
/// difference line to the mode's registered set.
fn assert_acceptance_figure(key: &str) {
    let baseline = baseline();
    let figure = &baseline["acceptance_figures"]["figures"][key];
    let stem = figure["stem"]
        .as_str()
        .unwrap_or_else(|| panic!("acceptance figure {key} has no stem"));
    let pid = acceptance_dir().join(format!("{stem}.pid"));
    let reference = acceptance_dir().join(format!("{stem}_Data.xml"));
    if !pid.is_file() || !reference.is_file() {
        eprintln!(
            "skipping: {key} acceptance drawing is absent ({})",
            pid.display()
        );
        return;
    }
    let dump = package_dir().join("Export.dmp");
    let datamap = package_dir().join("RefData~4~804.zip");

    for mode in ["pid-alone", "oracle-exp"] {
        let recorded = &figure["modes"][mode];
        let mut args: Vec<OsString> = vec![
            pid.clone().into_os_string(),
            "--diff-against".into(),
            reference.clone().into_os_string(),
            "--plant".into(),
            "P01".into(),
        ];
        if mode == "oracle-exp" {
            if !dump.is_file() || !datamap.is_file() {
                eprintln!("skipping: {key} {mode} needs the exp dump and the datamap");
                continue;
            }
            args.extend([
                "--oracle-exp".into(),
                dump.clone().into_os_string(),
                "--datamap".into(),
                datamap.clone().into_os_string(),
            ]);
        }
        let run = run_cli(args);
        assert_eq!(
            run.summary(),
            recorded["summary"].as_str().unwrap_or_default(),
            "{key} {mode}: summary"
        );
        Classification::of(recorded, &run).assert_complete(&format!("{key} {mode}"));
        if let Some(line) = recorded["informational_line"].as_str() {
            assert!(
                run.stdout.lines().any(|printed| printed == line),
                "{key} {mode}: the pairing line {line:?} was not printed:\n{}",
                run.stdout
            );
        }
        assert_eq!(
            i64::from(run.code),
            recorded["exit_code"].as_i64().expect("exit_code"),
            "{key} {mode}: exit code\n{}",
            run.stdout
        );
    }
}

// -------------------------------------------------------- XML surgery -------

/// `(name, is_close)` for a line that is a single open or close tag, `None`
/// for text, self-closing tags, the XML declaration, and comments.
fn tag_of(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("</") {
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        return if name.is_empty() {
            None
        } else {
            Some((name, true))
        };
    }
    let rest = trimmed.strip_prefix('<')?;
    if trimmed.ends_with("/>") {
        return None;
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some((name, false))
    }
}

/// Remove the whole top-level block that contains `needle`.
///
/// The reference `_Data.xml` is pretty-printed one element per line, so a
/// depth walk over open and close tags finds the enclosing block without
/// pulling in an XML parser.
fn strip_block_containing(text: &str, needle: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let hit = lines
        .iter()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle}"));

    let mut start = hit;
    let mut owed = 0usize;
    loop {
        assert!(start > 0, "no enclosing block for {needle}");
        start -= 1;
        match tag_of(lines[start]) {
            Some((_, true)) => owed += 1,
            Some((_, false)) => {
                if owed == 0 {
                    break;
                }
                owed -= 1;
            }
            None => {}
        }
    }

    let (name, _) = tag_of(lines[start]).expect("the enclosing line is a tag");
    let close = format!("</{name}>");
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == close)
        .map(|offset| start + 1 + offset)
        .unwrap_or_else(|| panic!("no {close} after line {}", start + 1));

    let trailing = if text.ends_with('\n') { "\n" } else { "" };
    let kept: Vec<&str> = lines[..start]
        .iter()
        .chain(lines[end + 1..].iter())
        .copied()
        .collect();
    format!("{}{trailing}", kept.join("\n"))
}

// ----------------------------------------------------------------- tests ----

#[test]
fn the_a01_run_prints_exactly_the_registered_differences() {
    let baseline = baseline();
    let figure = &baseline["figures"]["A01"];
    let run = a01_run(&a01_pid(), &a01_reference());

    assert_eq!(
        run.code,
        figure["exit_code"].as_i64().expect("A01 exit_code") as i32,
        "A01 exits 1 by design: the baseline records the differences, it does not suppress them"
    );
    assert_eq!(
        run.summary(),
        figure["summary"].as_str().unwrap_or_default()
    );
    Classification::of(figure, &run).assert_complete("A01");

    let listed: usize = figure["category_counts"]
        .as_object()
        .expect("A01 category_counts")
        .values()
        .filter_map(Value::as_u64)
        .map(|count| count as usize)
        .sum();
    assert_eq!(
        listed, 28,
        "A01 lists 19 emitted + 9 reference-only differences"
    );
}

#[test]
fn the_a01_entry_pins_the_graphic_oids_and_the_rel_def_uids() {
    let baseline = baseline();
    let figure = &baseline["figures"]["A01"];

    let mut pairs: Vec<(String, String)> = Vec::new();
    for item in figure["differences"]["value_different"]["items"]
        .as_array()
        .expect("A01 value_different items")
    {
        assert_eq!(
            item["interface_attribute"].as_str(),
            Some("IDrawingRepresentation.GraphicOID")
        );
        pairs.push((
            item["ours"].as_str().unwrap_or_default().to_string(),
            item["reference"].as_str().unwrap_or_default().to_string(),
        ));
    }
    pairs.sort();
    assert_eq!(
        pairs,
        vec![
            ("184".to_string(), "24601".to_string()),
            ("417".to_string(), "24615".to_string()),
            ("51".to_string(), "24606".to_string()),
        ]
    );

    let mut def_uids: Vec<String> = figure["differences"]["rel_not_in_reference"]["items"]
        .as_array()
        .expect("A01 rel_not_in_reference items")
        .iter()
        .map(|item| item["def_uid"].as_str().unwrap_or_default().to_string())
        .collect();
    def_uids.sort();
    let expected: Vec<String> = [
        "DrawingItems",
        "DrawingItems",
        "DrawingItems",
        "DwgRepresentationComposition",
        "DwgRepresentationComposition",
        "DwgRepresentationComposition",
        "ProcessPointCollection",
    ]
    .iter()
    .map(|name| (*name).to_string())
    .collect();
    assert_eq!(def_uids, expected);

    // The interface is IDrawingRepresentation, not IDrawingPresentation. The
    // prose note may name the wrong spelling to warn against it; no pinned
    // interface attribute may carry it.
    let mut interfaces: Vec<String> = Vec::new();
    for section in ["differences", "reference_only_differences"] {
        if let Some(map) = figure.get(section).and_then(Value::as_object) {
            for entry in map.values() {
                if let Some(items) = entry.get("items").and_then(Value::as_array) {
                    for item in items {
                        if let Some(name) = item.get("interface_attribute").and_then(Value::as_str)
                        {
                            interfaces.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    assert!(
        !interfaces.is_empty(),
        "the A01 entry must pin interface attributes"
    );
    for name in &interfaces {
        assert!(
            !name.contains("IDrawingPresentation"),
            "the interface is IDrawingRepresentation; found {name}"
        );
    }
}

#[test]
fn a_deleted_object_is_unregistered_and_still_exits_one() {
    let baseline = baseline();
    let figure = &baseline["figures"]["A01"];
    let reference = std::fs::read_to_string(a01_reference()).expect("read A01_Data.xml");
    let stripped = strip_block_containing(&reference, PROBE_OBJECT_UID);
    assert_ne!(stripped, reference, "the probe must remove something");

    let scratch = Scratch::new("d3-deleted-object");
    let mutated = scratch.path().join("A01_Data.xml");
    std::fs::write(&mutated, &stripped).expect("write the mutated reference");

    let run = a01_run(&a01_pid(), &mutated);
    assert_eq!(run.code, 1, "deleting an object must not silence the gate");

    let needle = format!("extra PIDProcessVessel {PROBE_OBJECT_UID}");
    assert!(
        run.stdout.contains(&needle),
        "the run must print {needle:?}"
    );

    let classification = Classification::of(figure, &run);
    assert!(
        classification
            .unregistered
            .contains(&format!("extra\t{needle}")),
        "the deleted object must be unregistered, not absorbed by the baseline: {:#?}",
        classification.unregistered
    );
}

#[test]
fn a_changed_generated_document_is_unregistered_and_still_exits_one() {
    let baseline = baseline();
    let figure = &baseline["figures"]["A01"];

    // Renaming the input changes the generated document and nothing else: the
    // drawing name is derived from the file name, so the reference stays as
    // shipped and the run gains the two differences that name reaches — the
    // drawing's IObject.Name and the container's DocName.
    let scratch = Scratch::new("d3-renamed-input");
    let renamed = scratch.path().join("A01-mutated.pid");
    std::fs::copy(a01_pid(), &renamed).expect("copy A01.pid under a mutated stem");

    let run = a01_run(&renamed, &a01_reference());
    assert_eq!(
        run.code, 1,
        "an unregistered generated change must not silence the gate"
    );

    let needle =
        format!("value PIDDrawing {DRAWING_UID} IObject.Name: \"A01-mutated\" reference \"A01\"");
    assert!(
        run.stdout.contains(&needle),
        "the run must print {needle:?}"
    );

    let classification = Classification::of(figure, &run);
    assert_eq!(
        classification.unregistered,
        BTreeSet::from([
            format!("value_different\t{needle}"),
            "container\tcontainer DocName: \"A01-mutated\" reference \"A01\"".to_string(),
        ]),
        "the renamed input contributes exactly its two unregistered differences"
    );
    assert!(
        classification.disappeared.is_empty(),
        "every registered difference must still print: {:#?}",
        classification.disappeared
    );
}

/// The whole-package publish of `TEST02_p` scored against `reference_dir`,
/// with each `  A01: ` prefix taken off its difference line, or `None` (with
/// a `skipping:` line) when the package is absent.
fn a01_package_run(figure: &Value, reference_dir: &Path) -> Option<(Run, String)> {
    let package = a01_package();
    if !package.join("Manifest.txt").is_file() {
        eprintln!("skipping: {} is absent", package.display());
        return None;
    }
    let prefix = figure["printed_prefix"]
        .as_str()
        .expect("the package figure records its printed_prefix");
    let scratch = Scratch::new("a01-package-gate");
    let run = run_cli([
        package.as_os_str(),
        OsStr::new("--out-dir"),
        scratch.path().as_os_str(),
        OsStr::new("--diff-against"),
        reference_dir.as_os_str(),
        OsStr::new("--plant"),
        OsStr::new("TEST02"),
    ]);
    let verdict = run
        .stdout
        .lines()
        .find(|line| line.starts_with("A01: "))
        .unwrap_or_default()
        .to_string();
    let stdout = run
        .stdout
        .lines()
        .map(|line| line.strip_prefix(prefix).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    Some((
        Run {
            code: run.code,
            stdout,
        },
        verdict,
    ))
}

#[test]
fn the_a01_database_run_prints_exactly_the_registered_differences() {
    let baseline = baseline();
    let figure = &baseline["figures"][A01_DATABASE_FIGURE];
    let Some(run) = a01_database_run(&a01_reference()) else {
        return;
    };
    assert_eq!(
        i64::from(run.code),
        figure["exit_code"].as_i64().expect("exit_code"),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.summary(),
        figure["summary"].as_str().unwrap_or_default()
    );
    Classification::of(figure, &run).assert_complete(A01_DATABASE_FIGURE);
}

#[test]
fn the_a01_package_run_prints_exactly_the_registered_differences() {
    let baseline = baseline();
    let figure = &baseline["figures"][A01_PACKAGE_FIGURE];
    let reference_dir = manifest_dir().join(A01_DIR);
    let Some((run, verdict)) = a01_package_run(figure, &reference_dir) else {
        return;
    };
    assert_eq!(
        i64::from(run.code),
        figure["exit_code"].as_i64().expect("exit_code"),
        "{}",
        run.stdout
    );
    assert_eq!(verdict, figure["verdict"].as_str().unwrap_or_default());
    Classification::of(figure, &run).assert_complete(A01_PACKAGE_FIGURE);
}

#[test]
fn the_a01_database_entries_list_the_three_synthetic_residuals_alone() {
    let baseline = baseline();
    let database = &baseline["figures"][A01_DATABASE_FIGURE];
    let package = &baseline["figures"][A01_PACKAGE_FIGURE];
    assert_eq!(
        registered_lines(database),
        registered_lines(package),
        "the package's one drawing is the database's A01"
    );
    assert_eq!(registered_lines(database).len(), 27);

    for figure in [database, package] {
        let residuals: BTreeSet<&str> = figure["residuals"]
            .as_object()
            .expect("residuals")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(residuals, BTreeSet::from(A01_DATABASE_RESIDUALS));
        let mut seen = BTreeSet::new();
        for section in ["differences", "reference_only_differences"] {
            for (kind, entry) in figure[section].as_object().expect(section) {
                let items = entry["items"].as_array().expect("items");
                assert_eq!(entry["count"].as_u64(), Some(items.len() as u64));
                assert_eq!(
                    figure["category_counts"][kind].as_u64(),
                    Some(items.len() as u64)
                );
                for item in items {
                    let residual = item["residual"].as_str().unwrap_or_default();
                    assert!(
                        A01_DATABASE_RESIDUALS.contains(&residual),
                        "{item}: {residual:?} is not one of the three residuals"
                    );
                    seen.insert(residual);
                    if residual == "graphic_oid" {
                        assert_eq!(
                            item["interface_attribute"].as_str(),
                            Some("IDrawingRepresentation.GraphicOID")
                        );
                    }
                }
            }
        }
        assert_eq!(seen, BTreeSet::from(A01_DATABASE_RESIDUALS));
    }
}

#[test]
fn a_deleted_object_is_unregistered_in_the_database_and_package_runs() {
    let baseline = baseline();
    let reference_path = a01_reference();
    if !reference_path.is_file() {
        eprintln!("skipping: {} is absent", reference_path.display());
        return;
    }
    let reference = std::fs::read_to_string(&reference_path).expect("read A01_Data.xml");
    let stripped = strip_block_containing(&reference, PROBE_OBJECT_UID);
    assert_ne!(stripped, reference, "the probe must remove something");
    let scratch = Scratch::new("a01-database-deleted-object");
    let mutated = scratch.path().join("A01_Data.xml");
    std::fs::write(&mutated, &stripped).expect("write the mutated reference");
    let needle = format!("extra PIDProcessVessel {PROBE_OBJECT_UID}");

    let figure = &baseline["figures"][A01_DATABASE_FIGURE];
    let Some(run) = a01_database_run(&mutated) else {
        return;
    };
    assert_eq!(run.code, 1, "deleting an object must not silence the gate");
    let classification = Classification::of(figure, &run);
    assert!(
        classification
            .unregistered
            .contains(&format!("extra\t{needle}")),
        "{A01_DATABASE_FIGURE}: the deleted object must be unregistered: {:#?}",
        classification.unregistered
    );

    let figure = &baseline["figures"][A01_PACKAGE_FIGURE];
    let Some((run, _)) = a01_package_run(figure, scratch.path()) else {
        return;
    };
    assert_eq!(run.code, 1, "deleting an object must not silence the gate");
    let classification = Classification::of(figure, &run);
    assert!(
        classification
            .unregistered
            .contains(&format!("extra\t{needle}")),
        "{A01_PACKAGE_FIGURE}: the deleted object must be unregistered: {:#?}",
        classification.unregistered
    );
}

/// The I井场 command line for `input` in `mode`, or `None` (with a
/// `skipping:` line) when the mode's dump or datamap is absent.
fn ij_args(input: &Path, reference: &Path, mode: &str) -> Option<Vec<OsString>> {
    let mut args: Vec<OsString> = vec![
        input.into(),
        "--diff-against".into(),
        reference.into(),
        "--plant".into(),
        "P01".into(),
    ];
    if mode == "oracle-exp" {
        let dump = package_dir().join("Export.dmp");
        let datamap = package_dir().join("RefData~4~804.zip");
        if !dump.is_file() || !datamap.is_file() {
            eprintln!("skipping: I井场 {mode} needs the exp dump and the datamap");
            return None;
        }
        args.push("--oracle-exp".into());
        args.push(dump.into_os_string());
        args.push("--datamap".into());
        args.push(datamap.into_os_string());
    }
    Some(args)
}

#[test]
fn the_ijingchang_run_never_leaves_the_registered_item() {
    let pid = ij_pid();
    let reference = ij_reference();
    if !pid.is_file() || !reference.is_file() {
        eprintln!("skipping: I井场 corpus is absent ({})", pid.display());
        return;
    }

    let baseline = baseline();
    let figure = &baseline["figures"]["I井场"];

    // The drawing is named by the file's stored name, so the long name and
    // the 8.3 alias the volume keeps for it are one input, and each has to
    // run clean in each mode. The two registered lines belong to a file
    // that is itself named by the short stem, which the next test runs.
    let mut inputs = vec![("its long name", pid.clone())];
    if let Some(alias) = common::short_path_of(&pid) {
        inputs.push(("its 8.3 alias", alias));
    }

    for (by, input) in &inputs {
        for mode in ["pid-alone", "oracle-exp"] {
            let Some(args) = ij_args(input, &reference, mode) else {
                continue;
            };
            let run = run_cli(args);
            eprintln!(
                "I井场 {mode} by {by} ({}): exit {}, {}",
                input.display(),
                run.code,
                run.summary()
            );
            let classification = Classification::of(figure, &run);
            let recorded = figure["modes"]
                .get(mode)
                .unwrap_or_else(|| panic!("the baseline records no {mode} mode"));

            assert_eq!(
                run.code, 0,
                "I井场 {mode} by {by} must run clean; it printed:\n{}",
                run.stdout
            );
            assert_eq!(
                run.summary(),
                recorded["clean_summary"].as_str().unwrap_or_default(),
                "I井场 {mode} by {by}: summary"
            );
            assert!(
                classification.registered.is_empty(),
                "I井场 {mode} by {by} exited 0 while printing a difference: {:#?}",
                classification.registered
            );
            assert!(
                classification.unregistered.is_empty(),
                "I井场 {mode} by {by} exited 0 while printing an unregistered difference: {:#?}",
                classification.unregistered
            );
        }
    }
}

#[test]
fn the_ijingchang_alias_stem_reproduces_the_registered_item() {
    let pid = ij_pid();
    let reference = ij_reference();
    if !pid.is_file() || !reference.is_file() {
        eprintln!("skipping: I井场 corpus is absent ({})", pid.display());
        return;
    }

    let baseline = baseline();
    let figure = &baseline["figures"]["I井场"];

    let scratch = Scratch::new("d3-ij-alias");
    let alias = scratch.path().join("IB031~1.pid");
    std::fs::copy(&pid, &alias).expect("copy the I井场 fixture to the 8.3 alias stem");

    // This file is stored under the short stem, so the stem is its name in
    // each mode: the dump is read by the drawing's UID, not by its name.
    for mode in ["pid-alone", "oracle-exp"] {
        let Some(args) = ij_args(&alias, &reference, mode) else {
            continue;
        };
        let run = run_cli(args);
        eprintln!(
            "I井场 {mode} by a copy stored as IB031~1.pid: exit {}, {}",
            run.code,
            run.summary()
        );
        let recorded = &figure["modes"][mode];

        assert_eq!(
            run.code, 1,
            "I井场 {mode}: the 8.3 alias stem must produce the registered difference"
        );
        assert_eq!(
            run.summary(),
            recorded["alias_summary"].as_str().unwrap_or_default(),
            "I井场 {mode}: alias-stem summary"
        );

        let classification = Classification::of(figure, &run);
        classification.assert_complete(&format!("I井场 alias stem, {mode}"));
        assert_eq!(classification.registered.len(), 2);
        for needle in [
            "IObject.Name: \"IB031~1\"",
            "container DocName: \"IB031~1\"",
        ] {
            assert!(
                classification
                    .registered
                    .iter()
                    .any(|item| item.contains(needle)),
                "no registered item carries {needle}: {:#?}",
                classification.registered
            );
        }
    }
}

#[test]
fn the_jk_well_site_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("J K井场");
}

#[test]
fn the_lm_well_site_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("L M井场");
}

#[test]
fn the_light_hydrocarbon_separator_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("轻烃分离器I");
}

#[test]
fn the_air_cooler_a_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("空冷器A");
}

#[test]
fn the_air_cooler_b_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("空冷器B");
}

#[test]
fn the_air_cooler_c_prints_exactly_the_registered_differences() {
    assert_acceptance_figure("空冷器C");
}

/// A `.pid` and a dump of another revision are not a pair: the dump has no
/// `T_DRAWING` row for the drawing's own DocUID, so it carries none of that
/// sheet's content. `--oracle-exp` must refuse (exit 2) instead of publishing
/// the `.pid` alone and reporting success, and it must write nothing.
#[test]
fn a_drawing_absent_from_the_dump_is_refused_not_silently_published() {
    let baseline = baseline();
    let section = &baseline["revision_mismatch_figures"];
    let expected_exit = section["expected_exit_code"].as_i64().expect("exit code");
    let figures = section["figures"]
        .as_object()
        .expect("revision_mismatch_figures.figures");
    assert_eq!(figures.len(), 4, "the four mismatched drawings");

    let dump = package_dir().join("Export.dmp");
    let datamap = package_dir().join("RefData~4~804.zip");
    if !dump.is_file() || !datamap.is_file() {
        eprintln!(
            "skipping: the exp dump or the datamap is absent ({})",
            dump.display()
        );
        return;
    }

    for (key, figure) in figures {
        let stem = figure["stem"].as_str().expect("stem");
        let doc_uid = figure["doc_uid"].as_str().expect("doc_uid");
        let pid = acceptance_dir().join(format!("{stem}.pid"));
        let reference = acceptance_dir().join(format!("{stem}_Data.xml"));
        if !pid.is_file() || !reference.is_file() {
            eprintln!(
                "skipping: {key} acceptance drawing is absent ({})",
                pid.display()
            );
            continue;
        }
        let scratch = Scratch::new("revision-mismatch");
        let output = Command::new(CLI)
            .arg(&pid)
            .arg("--out-dir")
            .arg(scratch.path())
            .arg("--diff-against")
            .arg(&reference)
            .args(["--plant", "P01"])
            .arg("--oracle-exp")
            .arg(&dump)
            .arg("--datamap")
            .arg(&datamap)
            .output()
            .expect("spawn pid_publish_xml --oracle-exp");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(i32::try_from(expected_exit).expect("i32")),
            "{key}: a dump of another revision must be refused, not published\n\
             stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stderr.contains("has no T_DRAWING row for DocUID"),
            "{key}: stderr must name the missing row:\n{stderr}"
        );
        assert!(
            stderr.contains(doc_uid),
            "{key}: stderr must carry the drawing's own DocUID {doc_uid}:\n{stderr}"
        );
        let written: Vec<_> = std::fs::read_dir(scratch.path())
            .expect("read the scratch dir")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert!(
            written.is_empty(),
            "{key}: a refused run writes nothing, found {written:?}"
        );
        assert!(
            !stdout.contains("pid export diff:"),
            "{key}: a refused run scores nothing, stdout:\n{stdout}"
        );
    }
}

/// The informational line each acceptance figure records names the bound
/// `FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES` registers for its `DocUID`, so
/// the table and the baseline cannot drift apart.
#[test]
fn every_acceptance_figure_records_the_allowance_its_doc_uid_registers() {
    let baseline = baseline();
    let figures = baseline["acceptance_figures"]["figures"]
        .as_object()
        .expect("acceptance_figures.figures");
    assert_eq!(figures.len(), 6);
    for (key, figure) in figures {
        let doc_uid = figure["doc_uid"].as_str().expect("doc_uid");
        let bound = pid_parse::publish::diff::fluid_system_reference_empty_allowance(doc_uid);
        let line = figure["modes"]["oracle-exp"]["informational_line"]
            .as_str()
            .expect("informational_line");
        assert!(
            line.ends_with(&format!("FluidSystem {bound}（登记上界 {bound}）")),
            "{key}: {line:?} against the registered bound {bound}"
        );
    }
}

#[test]
fn the_package_entry_records_the_registered_allowance() {
    let baseline = baseline();
    let exceptions = baseline["whole_package"]["registered_exceptions"]
        .as_array()
        .expect("whole_package.registered_exceptions");
    assert_eq!(exceptions.len(), 1);
    let exception = &exceptions[0];
    assert_eq!(
        exception["id"].as_str(),
        Some("fluid_system_reference_empty")
    );
    assert_eq!(exception["class_name"].as_str(), Some("FluidSystem"));
    assert_eq!(exception["observed"].as_u64(), Some(23));
    assert!(
        exception.get("bound").is_none(),
        "the bound must stay in FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES, not be restated here"
    );
    let source = exception["bound_source"].as_str().unwrap_or_default();
    assert!(source.contains("src/publish/diff.rs"));
    assert!(source.contains("FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES"));
}
