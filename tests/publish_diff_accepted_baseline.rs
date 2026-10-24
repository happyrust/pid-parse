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

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");
const BASELINE_REL: &str = "docs/baselines/publish-diff-accepted.json";
const A01_DIR: &str = "test-file/export-test/publish-data/A01";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CORPUS_DEFAULT: &str = r"D:\work\cad\PID自研软件验证";
const PACKAGE_DEFAULT: &str = r"D:\work\cad\pid-test-data";
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
        }
    }

    /// The kind of a printed difference line, or `None` for accounting lines
    /// (`meta ...`, `reference-only:`, `excused ...`) and the pairing note.
    fn of(line: &str) -> Option<Self> {
        if line.starts_with("class ") {
            Some(Kind::Class)
        } else if line.starts_with("extra ") {
            Some(Kind::Extra)
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

/// A scratch directory under the system temp dir, removed on drop. Nothing is
/// ever written inside the repository or next to a fixture.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |delta| delta.as_nanos());
        let path =
            std::env::temp_dir().join(format!("pid-parse-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path)
            .unwrap_or_else(|error| panic!("create {}: {error}", path.display()));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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
        listed, 27,
        "A01 lists 18 emitted + 9 reference-only differences"
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
    // shipped and the run gains one difference it did not have before.
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
        BTreeSet::from([format!("value_different\t{needle}")]),
        "the renamed input contributes exactly one unregistered difference"
    );
    assert!(
        classification.disappeared.is_empty(),
        "every registered difference must still print: {:#?}",
        classification.disappeared
    );
}

#[test]
fn the_ijingchang_run_never_leaves_the_registered_item() {
    let pid = ij_pid();
    let reference = ij_reference();
    if !pid.is_file() || !reference.is_file() {
        eprintln!("skip: I井场 corpus is absent ({})", pid.display());
        return;
    }

    let baseline = baseline();
    let figure = &baseline["figures"]["I井场"];
    let dump = PathBuf::from(PACKAGE_DEFAULT).join("Export.dmp");
    let datamap = PathBuf::from(PACKAGE_DEFAULT).join("RefData~4~804.zip");

    for mode in ["pid-alone", "oracle-exp"] {
        let mut args: Vec<OsString> = vec![pid.clone().into_os_string()];
        args.push("--diff-against".into());
        args.push(reference.clone().into_os_string());
        args.push("--plant".into());
        args.push("P01".into());
        if mode == "oracle-exp" {
            if !dump.is_file() || !datamap.is_file() {
                eprintln!("skip: I井场 {mode} needs the exp dump and the datamap");
                continue;
            }
            args.push("--oracle-exp".into());
            args.push(dump.clone().into_os_string());
            args.push("--datamap".into());
            args.push(datamap.clone().into_os_string());
        }

        let run = run_cli(args);
        let classification = Classification::of(figure, &run);
        let recorded = figure["modes"]
            .get(mode)
            .unwrap_or_else(|| panic!("the baseline records no {mode} mode"));

        // The drawing name is derived from the path, so both outcomes are
        // legitimate. Anything else is a finding.
        match run.code {
            0 => {
                assert_eq!(
                    run.summary(),
                    recorded["clean_summary"].as_str().unwrap_or_default(),
                    "I井场 {mode}: clean-branch summary"
                );
                assert!(
                    classification.registered.is_empty(),
                    "I井场 {mode} exited 0 while printing a difference: {:#?}",
                    classification.registered
                );
                assert!(
                    classification.unregistered.is_empty(),
                    "I井场 {mode} exited 0 while printing an unregistered difference: {:#?}",
                    classification.unregistered
                );
            }
            1 => {
                assert_eq!(
                    run.summary(),
                    recorded["alias_summary"].as_str().unwrap_or_default(),
                    "I井场 {mode}: alias-branch summary"
                );
                assert_eq!(
                    classification.registered.len(),
                    1,
                    "I井场 {mode} must print exactly the one registered item: {:#?}",
                    classification.registered
                );
                classification.assert_complete(&format!("I井场 {mode}"));
            }
            other => panic!("I井场 {mode} exited {other}; the contract allows only 0 or 1"),
        }
    }
}

#[test]
fn the_ijingchang_alias_stem_reproduces_the_registered_item() {
    let pid = ij_pid();
    let reference = ij_reference();
    if !pid.is_file() || !reference.is_file() {
        eprintln!("skip: I井场 corpus is absent ({})", pid.display());
        return;
    }

    let baseline = baseline();
    let figure = &baseline["figures"]["I井场"];

    let scratch = Scratch::new("d3-ij-alias");
    let alias = scratch.path().join("IB031~1.pid");
    std::fs::copy(&pid, &alias).expect("copy the I井场 fixture to the 8.3 alias stem");

    let run = run_cli([
        alias.as_os_str(),
        OsStr::new("--diff-against"),
        reference.as_os_str(),
        OsStr::new("--plant"),
        OsStr::new("P01"),
    ]);

    assert_eq!(
        run.code, 1,
        "the 8.3 alias stem must produce the registered difference"
    );
    assert_eq!(
        run.summary(),
        figure["modes"]["pid-alone"]["alias_summary"]
            .as_str()
            .unwrap_or_default()
    );

    let classification = Classification::of(figure, &run);
    classification.assert_complete("I井场 alias stem");
    assert_eq!(classification.registered.len(), 1);
    let only = classification
        .registered
        .iter()
        .next()
        .expect("one registered item");
    assert!(
        only.contains("IObject.Name: \"IB031~1\""),
        "unexpected registered item: {only}"
    );
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
        "the bound must stay in FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCE, not be restated here"
    );
    let source = exception["bound_source"].as_str().unwrap_or_default();
    assert!(source.contains("src/publish/diff.rs"));
    assert!(source.contains("FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCE"));
}
