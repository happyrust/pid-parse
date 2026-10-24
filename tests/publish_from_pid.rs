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
        report["classified_items"]["representations"],
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
    let exp_tables = pid_parse::publish::oracle_exp::ExpTables::read(&bytes).expect("exp tables");
    let tables = exp_tables.port_tables().expect("exp rows");
    pid_parse::publish::oracle_exp::attach_oracle_ports(&mut drawing, &tables, &dump);
    drawing.connectivity_ends = exp_tables
        .connectivity_ends(&drawing.drawing_uid)
        .expect("exp connectivity");
    drawing.exp_relations = exp_tables
        .composition_relations()
        .expect("exp compositions");
    let mut opc_points = exp_tables
        .opc_points(&drawing.drawing_uid)
        .expect("exp opc");
    let representations: HashSet<&str> = drawing
        .representations
        .iter()
        .map(|representation| representation.uid.as_str())
        .collect();
    opc_points.retain(|point| representations.contains(point.representation_uid.as_str()));
    drawing.opc_points = opc_points;
    let datamap_path = PathBuf::from(r"D:\work\cad\pid-test-data\RefData~4~804.zip");
    if !datamap_path.is_file() {
        eprintln!("skip: SPPID data map zip is missing");
        return;
    }
    let datamap = pid_parse::backup::read_datamap(&datamap_path).expect("datamap");
    pid_parse::publish::pipeline::attach_pipelines(
        &mut drawing,
        &exp_tables,
        "P01",
        Some(&datamap),
    )
    .expect("pipelines");
    let generated_xml = write_pid_data_xml(&drawing, "P01");
    let reference_text = std::fs::read_to_string(reference_xml).expect("reference");
    let diff = pid_parse::publish::diff_pid_export(&generated_xml, &reference_text)
        .expect("both documents are well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(
        (diff.objects, diff.values, diff.rels),
        (3_093, 3_346, 4_107)
    );
    // G2 (census `contract-work/g2-evidence/devin/ref-only-inventory.log`):
    // nothing the reference carries is missing from the generated side; the
    // 79 name-paired pipelines, the plant-database attribute classes and the
    // vendor's composed `IObject.Name` values are excused by design and
    // accounted in `diff.exclusions`.
    assert_eq!(diff.objects_not_in_generated, 0);
    assert_eq!(diff.values_not_in_generated, 0);
    assert_eq!(diff.rels_not_in_generated, 0);
    assert_eq!(diff.objects_excused_by_design, 0);
    assert_eq!(diff.objects_paired_by_name, 79);
    assert_eq!(diff.values_excused_by_design, 3_379);
    assert_eq!(diff.rels_excused_by_design, 0);
    assert_eq!(diff.rels_paired_by_name, 106);
    assert_eq!(diff.pipelines_paired_by_name, 79);
    assert_eq!(diff.piping_connectors_paired_by_name, 106);
    assert_eq!(diff.fluid_system_reference_empty, 23);
    assert_eq!(
        diff.fluid_system_reference_empty,
        pid_parse::publish::diff::FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCE,
        "the corpus census is the registered allowance's independent basis"
    );
    assert_eq!(drawing.pipeline_synthesis.fluid_code_missing, 0);
    assert_eq!(drawing.pipeline_synthesis.fluid_system_unmapped, 0);
    let generated = read_document(&generated_xml);
    let rels_of = |def_uid: &str| generated.rels.iter().filter(|rel| rel.3 == def_uid).count();
    assert_eq!(rels_of("PipingConnectors"), 106);
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
    assert_eq!(report["oracle_exp"]["dump_equipment_compositions"], 377);
    assert_eq!(report["oracle_exp"]["equipment_composition_rels"], 7);
    assert_eq!(report["oracle_exp"]["dump_item_notes"], 114);
    assert_eq!(report["oracle_exp"]["item_note_rels"], 6);
    assert_eq!(report["classified_items"]["items_published"], 556);
    assert_eq!(report["classes_total"], 3_093);
    let classes = report["classes"].as_object().expect("classes object");
    assert_eq!(classes.len(), 20);
    assert_eq!(classes["PIDDrawing"]["objects"], 1);
    assert_eq!(classes["PIDRepresentation"]["objects"], 902);
    assert_eq!(classes["PIDPipeline"]["objects"], 79);
    assert_eq!(classes["PIDBranchPoint"]["objects"], 2);
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
        .arg("--datamap")
        .arg(dump.parent().expect("dump dir").join("RefData~4~804.zip"))
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
    assert!(
        stderr.contains("79 PIDPipeline")
            && stderr.contains("106 PipingConnectors")
            && stderr.contains("79 FluidSystem"),
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

/// One stored ZIP member. The backup publisher inflates this the same way
/// it inflates `PlantData~2~711.zip`.
fn stored_zip(name: &str, payload: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }
    let name = name.as_bytes();
    let crc = crc32(payload);
    let size = u32::try_from(payload.len()).expect("member fits in a zip32 size");
    let name_len = u16::try_from(name.len()).expect("name fits in a zip name length");
    let mut out = Vec::new();
    let push16 = |out: &mut Vec<u8>, value: u16| out.extend_from_slice(&value.to_le_bytes());
    let push32 = |out: &mut Vec<u8>, value: u32| out.extend_from_slice(&value.to_le_bytes());
    push32(&mut out, 0x0403_4b50);
    push16(&mut out, 20);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, crc);
    push32(&mut out, size);
    push32(&mut out, size);
    push16(&mut out, name_len);
    push16(&mut out, 0);
    out.extend_from_slice(name);
    out.extend_from_slice(payload);
    let cd_at = u32::try_from(out.len()).expect("zip offset");
    push32(&mut out, 0x0201_4b50);
    push16(&mut out, 20);
    push16(&mut out, 20);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, crc);
    push32(&mut out, size);
    push32(&mut out, size);
    push16(&mut out, name_len);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push32(&mut out, 0);
    push32(&mut out, 0);
    out.extend_from_slice(name);
    let cd_size = u32::try_from(out.len()).expect("zip size") - cd_at;
    push32(&mut out, 0x0605_4b50);
    push16(&mut out, 0);
    push16(&mut out, 0);
    push16(&mut out, 1);
    push16(&mut out, 1);
    push32(&mut out, cd_size);
    push32(&mut out, cd_at);
    push16(&mut out, 0);
    out
}

#[test]
fn a_backup_directory_without_a_dump_publishes_the_pid() {
    let pid = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/D06.pid");
    if !pid.is_file() {
        eprintln!("skip: test-file/D06.pid is missing");
        return;
    }
    let bytes = std::fs::read(&pid).expect("read D06.pid");
    let package = unique_out_dir("mini-package");
    std::fs::create_dir_all(&package).expect("mini package dir");
    std::fs::write(package.join("Manifest.txt"), "BackupType<<|>>2\n").expect("manifest");
    std::fs::write(
        package.join("PlantData~2~1.zip"),
        stored_zip("D06.pid", &bytes),
    )
    .expect("plant zip");
    let out = unique_out_dir("mini-out");
    let missing_out = Command::new(CLI)
        .arg(&package)
        .output()
        .expect("spawn directory without --out-dir");
    assert_eq!(missing_out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_out.stderr).contains("--out-dir"));
    let output = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(&out)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn mini backup");
    assert!(
        output.status.success(),
        "exit {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("_Index.json")).expect("index"))
            .expect("index json");
    assert_eq!(index["export"], "PidOnly");
    assert_eq!(index["pipeline_uids"], "not synthesized");
    assert_eq!(index["pid_entries"], 1);
    assert_eq!(index["drawing_rows"], 0);
    assert_eq!(index["failed"], 0);
    assert_eq!(index["paired"], 0);
    assert!(out.join("D06_Data.xml").is_file());
    assert!(out.join("D06_Meta.xml").is_file());
    assert!(out.join("D06_Coverage.json").is_file());

    // G4: the package gate takes a reference DIRECTORY of <Name>_Data.xml
    // files, scores every written drawing and exits non-zero when one is
    // not clean.
    let gate_out = unique_out_dir("mini-gate");
    let gated = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(&gate_out)
        .arg("--diff-against")
        .arg(&out)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn gated mini backup");
    let stdout = String::from_utf8_lossy(&gated.stdout);
    assert_eq!(gated.status.code(), Some(0), "{stdout}");
    let row = &index["drawings"][0];
    assert!(
        stdout.contains(&format!(
            "D06: clean (objects {}, values {}, rels {})",
            row["objects"], row["values"], row["relationships"]
        )),
        "{stdout}"
    );

    // A reference directory without the drawing's file is reported as
    // unpaired, never as clean.
    let empty = unique_out_dir("mini-gate-empty");
    std::fs::create_dir_all(&empty).expect("empty reference dir");
    let unpaired = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(unique_out_dir("mini-gate-unpaired"))
        .arg("--diff-against")
        .arg(&empty)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn unpaired mini backup");
    let stdout = String::from_utf8_lossy(&unpaired.stdout);
    assert_eq!(unpaired.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("D06: unpaired"), "{stdout}");
    assert!(stdout.contains("no reference"), "{stdout}");

    // One drifting drawing makes the gate exit 1 and names the drawing.
    let drift = unique_out_dir("mini-gate-drift");
    std::fs::create_dir_all(&drift).expect("drift reference dir");
    let reference = std::fs::read_to_string(out.join("D06_Data.xml")).expect("D06 data xml");
    let mutated = reference.replacen("<IObject UID=\"", "<IObject UID=\"ZZZ", 1);
    assert_ne!(mutated, reference);
    std::fs::write(drift.join("D06_Data.xml"), mutated).expect("drifted reference");
    let dirty = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(unique_out_dir("mini-gate-dirty"))
        .arg("--diff-against")
        .arg(&drift)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn dirty mini backup");
    let stdout = String::from_utf8_lossy(&dirty.stdout);
    assert_eq!(dirty.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("D06: dirty"), "{stdout}");
}

#[test]
fn backup_directory_publishes_the_whole_package() {
    let package = PathBuf::from(r"D:\work\cad\pid-test-data");
    if !package.join("Manifest.txt").is_file()
        || !package.join("Export.dmp").is_file()
        || !package.join("PlantData~2~711.zip").is_file()
    {
        eprintln!("skip: D:\\work\\cad\\pid-test-data is not a backup package");
        return;
    }
    let out = unique_out_dir("sqplant");
    let started = std::time::Instant::now();
    let output = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(&out)
        .args(["--plant", "P01"])
        .output()
        .expect("spawn backup directory");
    let elapsed = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?} after {elapsed:?}\n{stderr}",
        output.status.code()
    );
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("_Index.json")).expect("index"))
            .expect("index json");
    assert_eq!(index["pid_entries"], 53, "{index}");
    assert_eq!(index["drawing_rows"], 53);
    assert_eq!(index["paired"], 53);
    assert_eq!(index["failed"], 0);
    assert_eq!(index["shells"], 2);
    assert_eq!(index["objects"], 80_085);
    assert_eq!(index["relationships"], 106_705);
    assert_eq!(index["piping_ports"], 9_557);
    assert_eq!(index["signal_ports"], 22_021);
    assert_eq!(index["connectivity_rels"], 19_371);
    assert_eq!(index["opc_branch_points"], 476);
    assert_eq!(index["equipment_composition_rels"], 372);
    assert_eq!(index["item_note_rels"], 114);
    assert_eq!(index["pipelines"], 2_395);
    assert_eq!(index["piping_connectors"], 3_128);
    assert_eq!(index["pipeline_uids"], "synthesized");
    assert!(
        elapsed.as_secs() < 215,
        "package took {elapsed:?}, the one-drawing loop was 431s"
    );
    let shells = index["drawings"]
        .as_array()
        .expect("drawings")
        .iter()
        .filter(|drawing| drawing["uid_from"] == "T_DRAWING.PATH")
        .count();
    assert_eq!(shells, 2);

    // D1b: `classes` is a full census of the sibling `_Data.xml`'s
    // top-level blocks — every tag the document carries is a `classes`
    // key with that tag's block count, and the sum closes on
    // `classes_total` and the drawing's `_Index.json` `objects`.
    // `dump_equipment_compositions` / `dump_item_notes` are dump-scope
    // constants (377 / 114 on every exp drawing); the `*_rels` pair is
    // per drawing.
    let mut dump_compositions = HashSet::new();
    let mut dump_notes = HashSet::new();
    let mut composition_rels = HashSet::new();
    let mut note_rels = HashSet::new();
    let mut exp_drawings = 0usize;
    for row in index["drawings"].as_array().expect("drawings") {
        let name = row["name"].as_str().expect("drawing name");
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(out.join(format!("{name}_Coverage.json")))
                .expect("coverage json"),
        )
        .expect("coverage json");
        let document = read_document(
            &std::fs::read_to_string(out.join(format!("{name}_Data.xml"))).expect("data xml"),
        );
        let mut tags: BTreeMap<String, usize> = BTreeMap::new();
        for (class, _) in document.objects.values() {
            *tags.entry(class.clone()).or_default() += 1;
        }
        let classes = report["classes"].as_object().expect("classes");
        for (tag, blocks) in &tags {
            assert!(
                classes.contains_key(tag.as_str()),
                "{name}: `{tag}` blocks are in the data xml but not in `classes`"
            );
            assert_eq!(classes[tag.as_str()]["objects"], *blocks, "{name}: {tag}");
        }
        let census: usize = classes
            .values()
            .map(|cell| cell["objects"].as_u64().expect("objects") as usize)
            .sum();
        assert_eq!(
            census,
            tags.values().sum::<usize>(),
            "{name}: `classes` holds a tag the data xml does not carry"
        );
        assert_eq!(report["classes_total"], census, "{name}: classes_total");
        assert_eq!(row["objects"], census, "{name}: index objects");
        if let Some(oracle) = report.get("oracle_exp") {
            exp_drawings += 1;
            dump_compositions.insert(
                oracle["dump_equipment_compositions"]
                    .as_u64()
                    .expect("dump"),
            );
            dump_notes.insert(oracle["dump_item_notes"].as_u64().expect("dump"));
            composition_rels.insert(oracle["equipment_composition_rels"].as_u64().expect("rels"));
            note_rels.insert(oracle["item_note_rels"].as_u64().expect("rels"));
        }
    }
    assert_eq!(exp_drawings, 51);
    assert_eq!(dump_compositions, HashSet::from([377]));
    assert_eq!(dump_notes, HashSet::from([114]));
    assert!(
        composition_rels.len() > 1,
        "per-drawing counts did not vary: {composition_rels:?}"
    );
    assert!(
        note_rels.len() > 1,
        "per-drawing counts did not vary: {note_rels:?}"
    );

    let Some(drawings) = corpus() else {
        eprintln!("skip: I井场 single-file comparison, corpus is missing");
        return;
    };
    let Some((stem, pid, _)) = drawings.iter().find(|(stem, ..)| stem.starts_with("I井场"))
    else {
        eprintln!("skip: I井场 is not in the corpus");
        return;
    };
    let single = unique_out_dir("ijingchang-single");
    let single_run = Command::new(CLI)
        .arg(pid)
        .arg("--out-dir")
        .arg(&single)
        .arg("--oracle-exp")
        .arg(package.join("Export.dmp"))
        .arg("--datamap")
        .arg(package.join("RefData~4~804.zip"))
        .args(["--plant", "P01"])
        .output()
        .expect("spawn single I井场");
    assert!(
        single_run.status.success(),
        "{}",
        String::from_utf8_lossy(&single_run.stderr)
    );
    for suffix in ["_Data.xml", "_Meta.xml"] {
        let packaged = std::fs::read(out.join(format!("{stem}{suffix}"))).expect("package file");
        let alone = std::fs::read(single.join(format!("{stem}{suffix}"))).expect("single file");
        assert_eq!(packaged, alone, "{stem}{suffix}");
    }
    let mut packaged: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join(format!("{stem}_Coverage.json"))).expect("coverage"),
    )
    .expect("coverage json");
    let mut alone: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(single.join(format!("{stem}_Coverage.json"))).expect("coverage"),
    )
    .expect("coverage json");
    packaged.as_object_mut().expect("object").remove("source");
    alone.as_object_mut().expect("object").remove("source");
    assert_eq!(packaged, alone);
}

/// N12 — a `Manifest.txt` directory whose `Export.dmp` is a SQL Server
/// MTF stream: the `MSDA` body is stripped to the MDF image, staged once,
/// and every `T_Drawing` row publishes through the database path into the
/// same `_Index.json` shape. `PlantData` zips are only listed, so the
/// index still reports `.pid` entries and pairs drawings by
/// `T_Drawing.Path`. Soft-skips without the TEST02_p fixture.
#[test]
fn sql_server_mtf_directory_publishes_t_drawing_rows() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/backup-test/TEST02_p");
    if !package.join("Manifest.txt").is_file() || !package.join("Export.dmp").is_file() {
        eprintln!("skip: test-file/backup-test/TEST02_p is not a backup package");
        return;
    }
    let out = unique_out_dir("test02-mtf");
    let output = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(&out)
        .args(["--plant", "TEST02"])
        .output()
        .expect("spawn MTF backup directory");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit {:?}\n{stderr}",
        output.status.code()
    );
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("_Index.json")).expect("index"))
            .expect("index json");
    assert_eq!(index["export"], "SqlServerMtf", "{index}");
    assert_eq!(index["failed"], 0, "{index}");
    assert_eq!(index["drawing_rows"], 1, "{index}");
    assert_eq!(index["paired"], 1, "{index}");
    assert_eq!(index["pipeline_uids"], "database", "{index}");
    assert!(index["objects"].as_u64().expect("objects") > 0, "{index}");
    // Every `.pid` the package shipped pairs by T_Drawing.Path or lands
    // in `unmatched_pid_entries` — none is dropped silently.
    let pid_entries = index["pid_entries"].as_u64().expect("pid_entries");
    let unmatched = index["unmatched_pid_entries"]
        .as_array()
        .expect("unmatched_pid_entries");
    assert_eq!(unmatched.len() as u64 + 1, pid_entries, "{index}");
    let drawing = &index["drawings"].as_array().expect("drawings")[0];
    assert_eq!(drawing["uid_from"], "T_Drawing.SP_ID", "{drawing}");
    let name = drawing["name"].as_str().expect("drawing name");
    let data = std::fs::read_to_string(out.join(format!("{name}_Data.xml"))).expect("data xml");
    assert!(data.contains("<PIDDrawing"), "{name}_Data.xml");
    assert!(out.join(format!("{name}_Meta.xml")).is_file());

    let gate_out = unique_out_dir("test02-mtf-gate");
    let gated = Command::new(CLI)
        .arg(&package)
        .arg("--out-dir")
        .arg(&gate_out)
        .arg("--diff-against")
        .arg(&out)
        .args(["--plant", "TEST02"])
        .output()
        .expect("spawn gated MTF backup directory");
    let stdout = String::from_utf8_lossy(&gated.stdout);
    // G6: the gate resolves a `PipingConnectors` rel's pipeline end in either
    // orientation — the DB line writes `UID1` = pipeline / `UID2` = connector,
    // the Oracle exp line the opposite — so a byte-identical reference is
    // clean. Assert the real criterion: the gated run reproduces the data xml
    // byte for byte, exits 0, prints the exact clean verdict, and prints no
    // `  {name}: …` finding line.
    let first = std::fs::read(out.join(format!("{name}_Data.xml"))).expect("first data xml");
    let second = std::fs::read(gate_out.join(format!("{name}_Data.xml"))).expect("gated data xml");
    assert_eq!(
        first, second,
        "{name}_Data.xml differs between the two runs"
    );
    let verdict = format!("{name}: clean (objects 12, values 29, rels 15)");
    assert!(
        stdout.lines().any(|line| line == verdict),
        "no `{verdict}` verdict:\n{stdout}"
    );
    assert_eq!(gated.status.code(), Some(0), "{stdout}");
    assert!(
        !stdout.lines().any(|line| line.starts_with("  ")),
        "a clean gate prints no finding line:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------
// G5 — the `_Meta.xml` companion gate. The in-repo A01 fixture ships the
// reference pair `<A01>_Data.xml` / `<A01>_Meta.xml`; a run that writes a
// meta document (`--out-dir`) scores it against that sibling.
// ---------------------------------------------------------------------

const A01_DIR: &str = "test-file/export-test/publish-data/A01";

fn a01_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(A01_DIR)
        .join(name)
}

/// Stage `<dir>/A01_Data.xml` (the byte-exact fixture) plus `meta_xml` as
/// `<dir>/A01_Meta.xml`; returns the data path for `--diff-against`.
fn stage_a01_reference(dir: &Path, meta_xml: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("reference dir");
    std::fs::copy(a01_fixture("A01_Data.xml"), dir.join("A01_Data.xml")).expect("copy data xml");
    std::fs::write(dir.join("A01_Meta.xml"), meta_xml).expect("write reference meta");
    dir.join("A01_Data.xml")
}

/// `A01.pid --out-dir <out> --plant TEST02 --diff-against <data xml>`,
/// returning `(status code, stdout + stderr)`.
fn run_meta_gate(reference_data: &Path, out_dir: &Path) -> (Option<i32>, String) {
    let output = Command::new(CLI)
        .arg(a01_fixture("A01.pid"))
        .arg("--out-dir")
        .arg(out_dir)
        .args(["--plant", "TEST02"])
        .arg("--diff-against")
        .arg(reference_data)
        .output()
        .expect("spawn pid_publish_xml meta gate");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code(), combined)
}

#[test]
fn meta_gate_matches_the_a01_reference_when_a_meta_document_is_written() {
    if !a01_fixture("A01.pid").is_file() || !a01_fixture("A01_Meta.xml").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    let out_dir = unique_out_dir("meta-gate");
    let output = Command::new(CLI)
        .arg(a01_fixture("A01.pid"))
        .arg("--out-dir")
        .arg(&out_dir)
        .args(["--plant", "TEST02"])
        .arg("--diff-against")
        .arg(a01_fixture("A01_Data.xml"))
        .output()
        .expect("spawn pid_publish_xml meta gate");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("meta compared:"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("meta accepted differences"),
        "stdout:\n{stdout}"
    );
    assert!(stderr.contains("Meta diff against"), "stderr:\n{stderr}");
    assert!(stderr.contains("matched"), "stderr:\n{stderr}");
    assert!(!stdout.contains("value Container"), "stdout:\n{stdout}");
    // The _Data.xml side still has its registered findings, so exit 1.
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn meta_gate_names_a_drifted_container_attribute() {
    if !a01_fixture("A01.pid").is_file() || !a01_fixture("A01_Meta.xml").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    let reference = unique_out_dir("meta-gate-attr");
    let meta = std::fs::read_to_string(a01_fixture("A01_Meta.xml")).expect("reference meta");
    let drifted = meta.replacen("ToolSignature=\"AAAD\"", "ToolSignature=\"BBBB\"", 1);
    assert_ne!(drifted, meta);
    let data = stage_a01_reference(&reference, &drifted);
    let out_dir = unique_out_dir("meta-gate-attr-out");
    let (code, combined) = run_meta_gate(&data, &out_dir);
    assert_eq!(code, Some(1), "{combined}");
    for needle in ["ToolSignature", "AAAD", "BBBB"] {
        assert!(combined.contains(needle), "{needle} missing:\n{combined}");
    }
    let _ = std::fs::remove_dir_all(&reference);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn meta_gate_names_a_drifted_block_order() {
    if !a01_fixture("A01.pid").is_file() || !a01_fixture("A01_Meta.xml").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    let meta = std::fs::read_to_string(a01_fixture("A01_Meta.xml")).expect("reference meta");
    let dv_open = meta
        .find("<DocumentVersion>")
        .expect("DocumentVersion opens");
    let dv_close = meta
        .find("</DocumentVersion>")
        .expect("DocumentVersion closes")
        + "</DocumentVersion>".len();
    let dr_open = meta
        .find("<DocumentRevision>")
        .expect("DocumentRevision opens");
    let dr_close = meta
        .find("</DocumentRevision>")
        .expect("DocumentRevision closes")
        + "</DocumentRevision>".len();
    assert!(dv_open < dv_close && dv_close < dr_open && dr_open < dr_close);
    let drifted = format!(
        "{}{}{}{}{}",
        &meta[..dv_open],
        &meta[dr_open..dr_close],
        &meta[dv_close..dr_open],
        &meta[dv_open..dv_close],
        &meta[dr_close..],
    );
    let reference = unique_out_dir("meta-gate-order");
    let data = stage_a01_reference(&reference, &drifted);
    let out_dir = unique_out_dir("meta-gate-order-out");
    let (code, combined) = run_meta_gate(&data, &out_dir);
    assert_eq!(code, Some(1), "{combined}");
    for needle in ["block-order", "DocumentVersion", "DocumentRevision"] {
        assert!(combined.contains(needle), "{needle} missing:\n{combined}");
    }
    let _ = std::fs::remove_dir_all(&reference);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn meta_gate_names_a_drifted_def_uid_and_wiring() {
    if !a01_fixture("A01.pid").is_file() || !a01_fixture("A01_Meta.xml").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    let meta = std::fs::read_to_string(a01_fixture("A01_Meta.xml")).expect("reference meta");

    // (a) RevisedDocument relabelled as VersionedDoc.
    let drifted = meta.replacen("DefUID=\"RevisedDocument\"", "DefUID=\"VersionedDoc\"", 1);
    assert_ne!(drifted, meta);
    let reference = unique_out_dir("meta-gate-defuid");
    let data = stage_a01_reference(&reference, &drifted);
    let out_dir = unique_out_dir("meta-gate-defuid-out");
    let (code, combined) = run_meta_gate(&data, &out_dir);
    assert_eq!(code, Some(1), "{combined}");
    for needle in ["VersionedDoc", "RevisedDocument"] {
        assert!(combined.contains(needle), "{needle} missing:\n{combined}");
    }
    let _ = std::fs::remove_dir_all(&reference);
    let _ = std::fs::remove_dir_all(&out_dir);

    // (b) FileComposition's UID1 / UID2 ends swapped.
    let drifted = meta.replacen(
        "UID1=\"AAADC317979198A43C0DECACEB99146F\" UID2=\"3937034585A866941E0CDBAEF2065D34\"",
        "UID1=\"3937034585A866941E0CDBAEF2065D34\" UID2=\"AAADC317979198A43C0DECACEB99146F\"",
        1,
    );
    assert_ne!(drifted, meta);
    let reference = unique_out_dir("meta-gate-wiring");
    let data = stage_a01_reference(&reference, &drifted);
    let out_dir = unique_out_dir("meta-gate-wiring-out");
    let (code, combined) = run_meta_gate(&data, &out_dir);
    assert_eq!(code, Some(1), "{combined}");
    for needle in ["FileComposition", "is not the registered"] {
        assert!(combined.contains(needle), "{needle} missing:\n{combined}");
    }
    let _ = std::fs::remove_dir_all(&reference);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn meta_gate_does_not_run_without_a_generated_meta_document() {
    if !a01_fixture("A01.pid").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    let output = Command::new(CLI)
        .arg(a01_fixture("A01.pid"))
        .args(["--plant", "TEST02"])
        .arg("--diff-against")
        .arg(a01_fixture("A01_Data.xml"))
        .output()
        .expect("spawn pid_publish_xml without a meta write");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("Meta diff against"),
        "no meta was generated, so no meta verdict may print:\n{combined}"
    );
}

#[test]
fn meta_gate_says_it_did_not_run_when_the_reference_has_no_sibling_meta() {
    if !a01_fixture("A01.pid").is_file() {
        eprintln!("skip: {A01_DIR} fixture is missing");
        return;
    }
    // Only the `_Data.xml` is staged: the run writes a meta document, but
    // there is no `<Name>_Meta.xml` beside the reference to score it against.
    let reference = unique_out_dir("meta-gate-no-sibling");
    std::fs::create_dir_all(&reference).expect("reference dir");
    let data = reference.join("A01_Data.xml");
    std::fs::copy(a01_fixture("A01_Data.xml"), &data).expect("copy data xml");
    let out_dir = unique_out_dir("meta-gate-no-sibling-out");
    let (_, combined) = run_meta_gate(&data, &out_dir);
    assert!(
        combined.contains("Meta diff not run: no sibling <Name>_Meta.xml beside"),
        "{combined}"
    );
    assert!(!combined.contains("Meta diff against"), "{combined}");
    let _ = std::fs::remove_dir_all(&reference);
    let _ = std::fs::remove_dir_all(&out_dir);
}
