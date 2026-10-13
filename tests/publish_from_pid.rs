//! The `.pid`-only publish export scored against the `SmartPlant` reference
//! (plan `docs/plans/2026-10-10-publish-from-the-pid-alone.md` P3 / P4).
//!
//! For each drawing of `D:\work\cad\PID自研软件验证` (or
//! `PID_PARSE_SELF_CHECK_DIR`) the generated `_Data.xml` is compared with the
//! reference `<stem>_Data.xml` block by block: an emitted object must exist in
//! the reference under the same UID and class, every attribute value it
//! carries must equal the reference's, and every emitted `<Rel>` must exist
//! with the same UID, ends and `DefUID`. Wrong is zero; what is emitted is
//! pinned per drawing so coverage only moves on purpose. Soft-skips when the
//! corpus is absent.
//!
//! The delivery (P5): `pid_publish_xml <file.pid> --out-dir DIR` writes
//! `<DocName>_Data.xml`, `<DocName>_Meta.xml` and `<DocName>_Coverage.json`
//! into `DIR`, and refuses the flag beside `--out` or for a database input.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use pid_parse::publish::{
    load_publish_from_pid, write_pid_coverage_json, write_pid_data_xml, write_pid_meta_xml,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";

#[derive(Default)]
struct Document {
    /// UID -> (class, `interface.attribute` -> value).
    objects: HashMap<String, (String, BTreeMap<String, String>)>,
    /// `(rel UID, UID1, UID2, DefUID)`.
    rels: HashSet<(String, String, String, String)>,
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
                attribute
                    .unescape_value()
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

/// An interface element: its name and its attributes.
type Interface = (String, Vec<(String, String)>);
/// A top-level block being read: its tag and its interfaces.
type Block = Option<(String, Vec<Interface>)>;

fn read_document(xml: &str) -> Document {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut document = Document::default();
    let mut depth = 0usize;
    let mut block: Block = None;
    let add = |start: &BytesStart<'_>, block: &mut Block| {
        if let Some((_, interfaces)) = block.as_mut() {
            let interface = String::from_utf8_lossy(start.name().as_ref()).into_owned();
            interfaces.push((interface, attributes_of(start)));
        }
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    let tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                    block = Some((tag, Vec::new()));
                } else if depth == 2 {
                    add(&start, &mut block);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    add(&start, &mut block);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some((tag, interfaces)) = block.take() {
                        finish(&mut document, tag, interfaces);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    document
}

fn finish(document: &mut Document, tag: String, interfaces: Vec<Interface>) {
    let mut values = BTreeMap::new();
    for (interface, pairs) in &interfaces {
        for (name, value) in pairs {
            values.insert(format!("{interface}.{name}"), value.clone());
        }
    }
    let Some(uid) = values.remove("IObject.UID") else {
        return;
    };
    if tag == "Rel" {
        let get = |key: &str| values.get(key).cloned().unwrap_or_default();
        document
            .rels
            .insert((uid, get("IRel.UID1"), get("IRel.UID2"), get("IRel.DefUID")));
    } else {
        values.retain(|_, value| !value.is_empty());
        document.objects.insert(uid, (tag, values));
    }
}

/// What one drawing's export emits and how it compares with the reference.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Score {
    objects: usize,
    objects_not_in_reference: usize,
    objects_of_another_class: usize,
    values: usize,
    values_different: usize,
    values_not_in_reference: usize,
    rels: usize,
    rels_not_in_reference: usize,
}

fn score(generated: &Document, reference: &Document, listing: &mut Vec<String>) -> Score {
    let mut score = Score::default();
    let mut uids: Vec<&String> = generated.objects.keys().collect();
    uids.sort();
    for uid in uids {
        let (class, values) = &generated.objects[uid];
        score.objects += 1;
        let Some((reference_class, reference_values)) = reference.objects.get(uid) else {
            score.objects_not_in_reference += 1;
            listing.push(format!("extra {class} {uid}"));
            continue;
        };
        if class != reference_class {
            score.objects_of_another_class += 1;
            listing.push(format!("class {class} {uid}: reference {reference_class}"));
        }
        for (key, value) in values {
            score.values += 1;
            match reference_values.get(key) {
                Some(reference_value) if reference_value == value => {}
                Some(reference_value) => {
                    score.values_different += 1;
                    listing.push(format!(
                        "value {class} {uid} {key}: {value:?} reference {reference_value:?}"
                    ));
                }
                None => {
                    score.values_not_in_reference += 1;
                    listing.push(format!(
                        "value {class} {uid} {key}: {value:?} not in reference"
                    ));
                }
            }
        }
    }
    for rel in &generated.rels {
        score.rels += 1;
        if !reference.rels.contains(rel) {
            score.rels_not_in_reference += 1;
            listing.push(format!("rel {rel:?}"));
        }
    }
    score
}

fn corpus() -> Option<Vec<(String, PathBuf, PathBuf)>> {
    let corpus =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&corpus)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();
    (!drawings.is_empty()).then_some(drawings)
}

/// What each drawing's export emits: `(stem, objects, values, rels)`. Every
/// one of them matches the reference; a change here is a coverage change.
const EXPECTED: [(&str, usize, usize, usize); 5] = [
    ("I井场 注采阀组工艺及自控流程图", 1735, 1530, 1971),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        353,
        326,
        397,
    ),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        283,
        256,
        318,
    ),
    (
        "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        374,
        347,
        415,
    ),
    (
        "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        713,
        636,
        828,
    ),
];

/// Per class: (emitted, in the reference, emitted with a name, named in the reference).
type Coverage = BTreeMap<String, (usize, usize, usize, usize)>;

fn export_and_score(
    pid: &Path,
    reference_xml: &Path,
    listing: &mut Vec<String>,
    coverage: &mut Coverage,
) -> Score {
    let drawing = load_publish_from_pid(pid).expect("the .pid loads");
    let generated = read_document(&write_pid_data_xml(&drawing, "P01"));
    let reference = read_document(&std::fs::read_to_string(reference_xml).expect("reference"));
    for (class, values) in generated.objects.values() {
        let cell = coverage.entry(class.clone()).or_default();
        cell.0 += 1;
        cell.2 += usize::from(values.contains_key("IObject.Name"));
    }
    for (class, values) in reference.objects.values() {
        let cell = coverage.entry(class.clone()).or_default();
        cell.1 += 1;
        cell.3 += usize::from(values.contains_key("IObject.Name"));
    }
    score(&generated, &reference, listing)
}

#[test]
fn the_pid_alone_export_matches_the_reference_wherever_it_says_anything() {
    let Some(drawings) = corpus() else {
        eprintln!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    };
    let mut wrong = Vec::new();
    let mut moved = Vec::new();
    let mut coverage = Coverage::new();
    for (stem, pid, reference_xml) in &drawings {
        let mut listing = Vec::new();
        let score = export_and_score(pid, reference_xml, &mut listing, &mut coverage);
        eprintln!("{stem}: {score:?}");
        if let Some((_, objects, values, rels)) =
            EXPECTED.iter().find(|(expected, ..)| expected == stem)
        {
            if (score.objects, score.values, score.rels) != (*objects, *values, *rels) {
                moved.push(format!(
                    "{stem}: emitted ({}, {}, {}), pinned ({objects}, {values}, {rels})",
                    score.objects, score.values, score.rels
                ));
            }
        }
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for line in &listing {
            let kind: Vec<&str> = line.split_whitespace().take(2).collect();
            *kinds.entry(kind.join(" ")).or_insert(0) += 1;
        }
        eprintln!("  {kinds:?}");
        for line in listing.iter().take(6) {
            eprintln!("  {line}");
        }
        if score.objects_not_in_reference
            + score.objects_of_another_class
            + score.values_different
            + score.values_not_in_reference
            + score.rels_not_in_reference
            > 0
        {
            wrong.push(stem.clone());
        }
    }
    eprintln!("class: emitted / reference, named emitted / named in reference");
    for (class, (emitted, reference, named, reference_named)) in &coverage {
        eprintln!("  {class:<28} {emitted:>5} / {reference:<5} {named:>5} / {reference_named:<5}");
    }
    assert!(wrong.is_empty(), "wrong output on {wrong:?}");
    if drawings.len() == EXPECTED.len() {
        assert!(moved.is_empty(), "coverage moved: {moved:#?}");
    }
}

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");

/// A directory of this process's own under the system temp dir.
fn unique_out_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("pid-parse-{label}-{}-{nanos}", std::process::id()))
}

#[test]
fn the_cli_delivers_a_drawings_three_files_into_out_dir() {
    let Some(drawings) = corpus() else {
        eprintln!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    };
    let (stem, pid, _) = &drawings[0];
    let out_dir = unique_out_dir("out-dir");
    let output = Command::new(CLI)
        .arg(pid)
        .arg("--out-dir")
        .arg(&out_dir)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn pid_publish_xml --out-dir");
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let drawing = load_publish_from_pid(pid).expect("the .pid loads");
    let read = |suffix: &str| {
        std::fs::read_to_string(out_dir.join(format!("{stem}{suffix}")))
            .unwrap_or_else(|e| panic!("{stem}{suffix} in {}: {e}", out_dir.display()))
    };
    assert_eq!(read("_Data.xml"), write_pid_data_xml(&drawing, "P01"));
    assert_eq!(
        read("_Meta.xml"),
        write_pid_meta_xml(&drawing, "P01").expect("meta renders")
    );
    let report: serde_json::Value =
        serde_json::from_str(&read("_Coverage.json")).expect("the report is JSON");
    assert_eq!(report["drawing"]["uid"], drawing.drawing_uid.as_str());
    assert_eq!(report["drawing"]["name"], stem.as_str());
    assert_eq!(
        report["objects"]["representations"],
        drawing.representations.len()
    );
    assert_eq!(report["files"]["data"], format!("{stem}_Data.xml"));
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn diff_against_a_pid_exits_zero_when_nothing_emitted_is_wrong() {
    let Some(drawings) = corpus() else {
        eprintln!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    };
    for (stem, pid, reference) in &drawings {
        let output = Command::new(CLI)
            .arg(pid)
            .args(["--diff-against"])
            .arg(reference)
            .args(["--plant", "P01"])
            .output()
            .expect("spawn pid_publish_xml --diff-against");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{stem} exit {:?}\n{stderr}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            stderr.contains("is clean in .pid mode"),
            "{stem} stderr:\n{stderr}"
        );
    }
}

#[test]
fn ijingchang_oracle_exp_matches_the_pinned_reference() {
    let dump = PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
    let Some(drawings) = corpus() else {
        eprintln!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    };
    let Some((_, pid, reference_xml)) =
        drawings.iter().find(|(stem, ..)| stem.starts_with("I井场"))
    else {
        eprintln!("skip: I井场 is not in the corpus");
        return;
    };
    if !dump.is_file() {
        eprintln!("skip: oracle exp dump is missing");
        return;
    }
    let mut drawing = load_publish_from_pid(pid).expect("the .pid loads");
    let bytes = std::fs::read(&dump).expect("dump");
    let tables = pid_parse::publish::oracle_exp::read_port_tables(&bytes).expect("exp rows");
    pid_parse::publish::oracle_exp::attach_oracle_ports(&mut drawing, &tables, &dump);
    drawing.connectivity_ends =
        pid_parse::publish::oracle_exp::connectivity_ends_for_drawing(&bytes, &drawing.drawing_uid)
            .expect("exp connectivity");
    drawing.exp_relations =
        pid_parse::publish::oracle_exp::composition_relations(&bytes).expect("exp compositions");
    let mut opc_points =
        pid_parse::publish::oracle_exp::opc_points_for_drawing(&bytes, &drawing.drawing_uid)
            .expect("exp opc");
    let representations: HashSet<&str> = drawing
        .representations
        .iter()
        .map(|representation| representation.uid.as_str())
        .collect();
    opc_points.retain(|point| representations.contains(point.representation_uid.as_str()));
    drawing.opc_points = opc_points;
    let generated = read_document(&write_pid_data_xml(&drawing, "P01"));
    let reference = read_document(&std::fs::read_to_string(reference_xml).expect("reference"));
    let mut listing = Vec::new();
    let score = score(&generated, &reference, &mut listing);
    assert!(
        listing.is_empty(),
        "{} findings, first: {:?}",
        listing.len(),
        listing.iter().take(8).collect::<Vec<_>>()
    );
    // N8: 7 nozzle compositions, 6 item notes, and the 2 OPC branch
    // points (with their DrawingItems and the 2 connectivity rows).
    assert_eq!(
        (score.objects, score.values, score.rels),
        (3_014, 3_188, 4_001)
    );
    let rels_of = |def_uid: &str| generated.rels.iter().filter(|rel| rel.3 == def_uid).count();
    assert_eq!(
        [
            rels_of("PipingTapOrFitting"),
            rels_of("PipingEnd1Conn"),
            rels_of("PipingEnd2Conn"),
            rels_of("SignalEnd1Conn"),
            rels_of("SignalEnd2Conn"),
            rels_of("SignalTapOrFitting"),
        ],
        [369, 129, 143, 38, 46, 13]
    );
    assert_eq!(
        pid_parse::publish::pid_writer::connectivity_rel_counts(&drawing),
        (738, 0)
    );
    assert_eq!(
        pid_parse::publish::pid_writer::composition_rel_counts(&drawing),
        (377, 7, 114, 6)
    );
    assert_eq!(rels_of("EquipmentComponentComposition"), 7);
    assert_eq!(rels_of("PBSItemNotes"), 6);
    let piping = generated
        .objects
        .values()
        .filter(|(class, _)| class == "PIDPipingPort")
        .count();
    let signal = generated
        .objects
        .values()
        .filter(|(class, _)| class == "PIDSignalPort")
        .count();
    assert_eq!((piping, signal), (362, 915));
    let piping_rels = generated
        .rels
        .iter()
        .filter(|rel| rel.3 == "PipingPortComposition")
        .count();
    let signal_rels = generated
        .rels
        .iter()
        .filter(|rel| rel.3 == "SignalPortComposition")
        .count();
    assert_eq!((piping_rels, signal_rels), (362, 915));
    let report: serde_json::Value =
        serde_json::from_str(&write_pid_coverage_json(&drawing, pid)).expect("coverage json");
    assert_eq!(report["oracle_exp"]["piping_ports"], 362);
    assert_eq!(report["oracle_exp"]["signal_ports"], 915);
    assert_eq!(report["oracle_exp"]["nominal_diameter_unspelled"], 0);
    assert_eq!(report["oracle_exp"]["connectivity_ends"], 738);
    assert_eq!(report["oracle_exp"]["connectivity_rels"], 738);
    assert_eq!(report["oracle_exp"]["connectivity_ends_left_out"], 0);
    assert_eq!(report["oracle_exp"]["opc_branch_points"], 2);
    assert_eq!(report["oracle_exp"]["equipment_compositions"], 377);
    assert_eq!(report["oracle_exp"]["equipment_composition_rels"], 7);
    assert_eq!(report["oracle_exp"]["item_notes"], 114);
    assert_eq!(report["oracle_exp"]["item_note_rels"], 6);
    assert_eq!(report["relationships"]["EquipmentComponentComposition"], 7);
    assert_eq!(report["relationships"]["PBSItemNotes"], 6);
    assert_eq!(report["relationships"]["PipingTapOrFitting"], 369);
    assert!(report["left_out_by_design"][0]
        .as_str()
        .unwrap_or("")
        .contains("Oracle exp"));
}

#[test]
fn diff_against_oracle_exp_exits_zero_for_ijingchang() {
    let dump = PathBuf::from(r"D:\work\cad\pid-test-data\Export.dmp");
    let Some(drawings) = corpus() else {
        eprintln!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    };
    let Some((stem, pid, reference)) = drawings.iter().find(|(stem, ..)| stem.starts_with("I井场"))
    else {
        eprintln!("skip: I井场 is not in the corpus");
        return;
    };
    if !dump.is_file() {
        eprintln!("skip: oracle exp dump is missing");
        return;
    }
    let output = Command::new(CLI)
        .arg(pid)
        .arg("--diff-against")
        .arg(reference)
        .arg("--oracle-exp")
        .arg(&dump)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn pid_publish_xml --oracle-exp");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{stem} exit {:?}\n{stderr}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("is clean in oracle-exp mode"),
        "{stem} stderr:\n{stderr}"
    );
}

#[test]
fn out_dir_is_refused_beside_out_and_for_a_database_input() {
    let out_dir = unique_out_dir("refused");
    let beside_out = Command::new(CLI)
        .args(["drawing.pid", "--out-dir"])
        .arg(&out_dir)
        .args(["--out", "drawing_Data.xml"])
        .output()
        .expect("spawn pid_publish_xml --out-dir --out");
    assert_eq!(beside_out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&beside_out.stderr).contains("mutually exclusive"));

    let database = Command::new(CLI)
        .args(["Export.mdf", "--drawing", "X", "--out-dir"])
        .arg(&out_dir)
        .output()
        .expect("spawn pid_publish_xml <mdf> --out-dir");
    assert_eq!(database.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&database.stderr).contains(".pid input only"));
    let exp_on_database = Command::new(CLI)
        .args([
            "Export.mdf",
            "--drawing",
            "X",
            "--out",
            "x.xml",
            "--oracle-exp",
            "Export.dmp",
        ])
        .output()
        .expect("spawn pid_publish_xml <mdf> --oracle-exp");
    assert_eq!(exp_on_database.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&exp_on_database.stderr)
        .contains("--oracle-exp applies to a .pid input only"));
    assert!(!out_dir.exists(), "a refused invocation writes nothing");
}
