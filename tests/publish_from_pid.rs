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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use pid_parse::publish::{load_publish_from_pid, write_pid_data_xml};
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
    ("I井场 注采阀组工艺及自控流程图", 1428, 1372, 1664),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        289,
        286,
        333,
    ),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        237,
        227,
        272,
    ),
    (
        "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        312,
        309,
        353,
    ),
    (
        "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        584,
        553,
        699,
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
