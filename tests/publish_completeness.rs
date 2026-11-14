//! How much of each reference the publish export states, pinned so that it
//! only rises.
//!
//! `pid_publish_xml --diff-against` is clean when nothing a run wrote is
//! wrong and nothing it left out is unexcused (`tests/publish_from_pid.rs`,
//! `tests/publish_diff_accepted_baseline.rs`). Half of a reference can go
//! unstated under that reading, and a class of attributes moved into the
//! exclusion list reads there exactly like a class written out.
//!
//! This suite holds the other reading. Each case publishes one drawing in
//! one input mode, measures the `_Data.xml` against the drawing's
//! `SmartPlant` reference with `pid_parse::publish::completeness::measure`,
//! and compares the result with a [`Pin`]:
//!
//! * every **denominator** is the reference's own count — all of its
//!   objects, interface elements, values, empty attributes and relations,
//!   nothing excused — and is pinned exactly. Each case also counts the
//!   attributes off the reference's text and requires the denominators to
//!   add up to that count, so no part of the reference sits outside them;
//! * every **numerator** may only rise. A fall fails as a regression. A rise
//!   fails as well, until the pin is raised to it in the same change: the
//!   pin is then always what the export states today, and the next fall is
//!   seen from there, not from a floor left far below;
//! * what a run states **against** its reference — a value stated
//!   differently, an interface element the reference's block lacks — may
//!   only fall.
//!
//! The three DWG-0202GP06-01 cases read committed fixtures only and run on
//! every checkout. The seven acceptance drawings whose plant backup is the
//! reference's own revision need the corpus; without it each case prints the
//! directory it did not find and skips.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use pid_parse::publish::completeness::{measure, Completeness, Share};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

mod common;
use common::Scratch;

const CLI: &str = env!("CARGO_BIN_EXE_pid_publish_xml");

/// What one run states of its reference. A pair is `(stated, reference)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pin {
    container: (usize, usize),
    objects: (usize, usize),
    /// Of the stated objects, the pipelines paired by name.
    paired_by_name: usize,
    interfaces: (usize, usize),
    values: (usize, usize),
    empty_attributes: (usize, usize),
    relations: (usize, usize),
    /// Of the stated relations, those under the reference's own UIDs.
    relations_verbatim: usize,
    /// Values stated differently from the reference. May only fall.
    values_different: usize,
    /// Interface elements the reference's block lacks. May only fall.
    interfaces_not_in_reference: usize,
}

impl Pin {
    fn of(measured: &Completeness) -> Self {
        let pair = |share: Share| (share.stated, share.reference);
        Self {
            container: pair(measured.container),
            objects: pair(measured.objects),
            paired_by_name: measured.objects_paired_by_name,
            interfaces: pair(measured.interfaces),
            values: pair(measured.values),
            empty_attributes: pair(measured.empty_attributes),
            relations: pair(measured.relations),
            relations_verbatim: measured.relations_verbatim,
            values_different: measured.values_different,
            interfaces_not_in_reference: measured.interfaces_not_in_reference,
        }
    }
}

/// A share: the denominator does not move, the numerator only rises.
fn share(moved: &mut Vec<String>, name: &str, now: (usize, usize), pinned: (usize, usize)) {
    if now.1 != pinned.1 {
        moved.push(format!(
            "{name}: the reference carries {}, the pin says {} — a denominator is the \
             reference's own count and nothing is taken out of it",
            now.1, pinned.1
        ));
    }
    rises(moved, name, now.0, pinned.0);
}

/// A count that may only rise, and whose pin follows it up.
fn rises(moved: &mut Vec<String>, name: &str, now: usize, pinned: usize) {
    match now.cmp(&pinned) {
        Ordering::Less => moved.push(format!("{name} fell from {pinned} to {now}")),
        Ordering::Greater => moved.push(format!(
            "{name} rose from {pinned} to {now}: raise the pin to it"
        )),
        Ordering::Equal => {}
    }
}

/// A count that may only fall, and whose pin follows it down.
fn falls(moved: &mut Vec<String>, name: &str, now: usize, pinned: usize) {
    match now.cmp(&pinned) {
        Ordering::Greater => moved.push(format!("{name} rose from {pinned} to {now}")),
        Ordering::Less => moved.push(format!(
            "{name} fell from {pinned} to {now}: lower the pin to it"
        )),
        Ordering::Equal => {}
    }
}

/// The attributes a `_Data.xml` carries, counted off its text: every `="`
/// that opens a value, less the XML declaration's own. Nothing here goes
/// through the reader `measure` uses.
fn attributes_in(xml: &str) -> usize {
    let declaration = xml.find("?>").map_or(0, |end| end + 2);
    xml[declaration..].matches("=\"").count()
}

/// Each object block's interface element names in document order, by
/// `IObject UID`, with the block's class; a `PIDPipeline` is also filed under
/// `name:<IObject Name>`, the key a pipeline pairs by when the UIDs differ.
fn interface_sequences(xml: &str) -> HashMap<String, (String, Vec<String>)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sequences = HashMap::new();
    let mut depth = 0usize;
    /// The class, the element names, the `IObject UID` and `Name`.
    type OpenBlock = (String, Vec<String>, Option<String>, Option<String>);
    let mut block: Option<OpenBlock> = None;
    let element = |start: &BytesStart<'_>, block: &mut Option<OpenBlock>| {
        let Some((_, names, uid, name)) = block.as_mut() else {
            return;
        };
        let interface = start.name().as_ref().to_string();
        if interface == "IObject" {
            for attribute in start.attributes().flatten() {
                let value = attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default();
                match attribute.key.as_ref() {
                    "UID" => *uid = Some(value),
                    "Name" => *name = Some(value),
                    _ => {}
                }
            }
        }
        names.push(interface);
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    let class = start.name().as_ref().to_string();
                    block = Some((class, Vec::new(), None, None));
                } else if depth == 2 {
                    element(&start, &mut block);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) if depth == 2 => element(&start, &mut block),
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth != 1 {
                    continue;
                }
                let Some((class, names, Some(uid), name)) = block.take() else {
                    continue;
                };
                if class == "Rel" {
                    continue;
                }
                if let (true, Some(name)) = (class == "PIDPipeline", name) {
                    sequences.insert(format!("name:{name}"), (class.clone(), names.clone()));
                }
                sequences.insert(uid, (class, names));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    sequences
}

/// Every object the generated document shares with the reference writes the
/// interface elements both carry in the reference's order: what it writes,
/// less what the reference's block lacks, is a subsequence of the
/// reference's sequence. Returns how many objects were compared.
fn hold_interface_order(what: &str, generated_xml: &str, reference_xml: &str) -> usize {
    let generated = interface_sequences(generated_xml);
    let reference = interface_sequences(reference_xml);
    let mut compared = 0;
    let mut out_of_order = Vec::new();
    for (key, (class, names)) in &generated {
        let Some((reference_class, reference_names)) = reference.get(key) else {
            continue;
        };
        // A pipeline is compared once, by name.
        if class != reference_class || class == "PIDPipeline" && !key.starts_with("name:") {
            continue;
        }
        compared += 1;
        let mut rest = reference_names.iter();
        let mut shared = names.iter().filter(|name| reference_names.contains(name));
        if !shared.all(|name| rest.any(|found| found == name)) {
            out_of_order.push(format!(
                "{class} {key}: ours {names:?}, reference {reference_names:?}"
            ));
        }
    }
    out_of_order.sort();
    assert!(
        out_of_order.is_empty(),
        "{what}: {} objects write their interface elements out of the reference's order\n  {}",
        out_of_order.len(),
        out_of_order
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    compared
}

/// Measure `generated_xml` against `reference_xml` and hold the result to
/// `pin`.
fn hold(what: &str, generated_xml: &str, reference_xml: &str, pin: &Pin) {
    let ordered = hold_interface_order(what, generated_xml, reference_xml);
    eprintln!("{what}: {ordered} objects write their interface elements in the reference's order");
    let measured =
        measure(generated_xml, reference_xml).unwrap_or_else(|error| panic!("{what}: {error}"));
    assert_eq!(
        measured.reference_attributes(),
        attributes_in(reference_xml),
        "{what}: the shares' denominators do not add up to the attributes in the reference \
         file, so part of the reference is in none of them"
    );
    let now = Pin::of(&measured);
    let mut moved = Vec::new();
    share(
        &mut moved,
        "container attributes",
        now.container,
        pin.container,
    );
    share(&mut moved, "objects", now.objects, pin.objects);
    share(
        &mut moved,
        "interface elements",
        now.interfaces,
        pin.interfaces,
    );
    share(&mut moved, "non-empty values", now.values, pin.values);
    share(
        &mut moved,
        "empty attributes",
        now.empty_attributes,
        pin.empty_attributes,
    );
    share(&mut moved, "relations", now.relations, pin.relations);
    rises(
        &mut moved,
        "relations under the reference's own UIDs",
        now.relations_verbatim,
        pin.relations_verbatim,
    );
    falls(
        &mut moved,
        "values stated differently",
        now.values_different,
        pin.values_different,
    );
    falls(
        &mut moved,
        "interface elements the reference's block lacks",
        now.interfaces_not_in_reference,
        pin.interfaces_not_in_reference,
    );
    if now.paired_by_name != pin.paired_by_name {
        moved.push(format!(
            "pipelines paired by name moved from {} to {}",
            pin.paired_by_name, now.paired_by_name
        ));
    }
    assert!(
        moved.is_empty(),
        "{what}: the pin no longer holds\n  {}\n\nmeasured now:\n{measured}\n\nas a pin:\n{now:?}\n\n\
         `cargo run --example probe_how_much_of_a_reference_does_a_publish_state -- \
         <ours_Data.xml> <reference_Data.xml>` lists what is missing.",
        moved.join("\n  ")
    );
    eprintln!("{what}:\n{measured}");
}

/// Run `pid_publish_xml <input> --out-dir <scratch> --plant P01 <extra…>`
/// and return the `<name>_Data.xml` it wrote.
fn publish(input: &Path, extra: &[&OsStr], name: &str) -> String {
    let out = Scratch::unmade("completeness");
    let output = Command::new(CLI)
        .arg(input)
        .arg("--out-dir")
        .arg(out.path())
        .args(["--plant", "P01"])
        .args(extra)
        .output()
        .expect("spawn pid_publish_xml");
    assert!(
        output.status.success(),
        "{} exited {:?}\n{}",
        input.display(),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    read(&out.join(format!("{name}_Data.xml")))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

// ---------------------------------------------------------------------
// DWG-0202GP06-01: the `.pid`, its Oracle `exp` backup package and its
// reference are all committed, so these three run wherever the repository
// is checked out. The package is a later revision of the plant than the
// reference (`tests/publish_from_pid.rs`), which is why the dump modes
// stop short of every object and relation.
// ---------------------------------------------------------------------

const DWG_DIR: &str = "test-file/export-test/publish-data/DWG-0202GP06-01";
const DWG_PACKAGE: &str = "test-file/backup-test/DWG-0202GP06-01_p";
const DWG_NAME: &str = "DWG-0202GP06-01";

fn dwg_reference() -> String {
    read(&repo_path(DWG_DIR).join(format!("{DWG_NAME}_Data.xml")))
}

const DWG_ALONE: Pin = Pin {
    container: (15, 15),
    objects: (78, 108),
    paired_by_name: 0,
    interfaces: (509, 747),
    values: (71, 236),
    empty_attributes: (91, 93),
    relations: (81, 133),
    relations_verbatim: 81,
    values_different: 0,
    interfaces_not_in_reference: 0,
};

/// The dump adds the ports, the off-page connectors, the connectivity and
/// the note and nozzle relations. Still missing are one off-page connector
/// and the reference's five pipelines with the six `PipingConnectors`
/// relations to them: the dump's own six pipelines carry the later
/// revision's tags and pair with none of the five by name.
const DWG_WITH_ITS_DUMP: Pin = Pin {
    container: (15, 15),
    objects: (102, 108),
    paired_by_name: 0,
    interfaces: (677, 747),
    values: (99, 236),
    empty_attributes: (91, 93),
    relations: (127, 133),
    relations_verbatim: 127,
    values_different: 0,
    interfaces_not_in_reference: 0,
};

/// The package publishes its own `.pid`, which is the later revision too:
/// two representations are other objects, 37 of the 39 values stated
/// differently are `GraphicOID`s and two are `PipingConnectorSeqNo`s, and
/// the vessel has since been given an `EquipSeqNo`, for which the
/// reference's block has no `INamedEquipment`.
const DWG_PACKAGE_DIRECTORY: Pin = Pin {
    container: (15, 15),
    objects: (100, 108),
    paired_by_name: 0,
    interfaces: (673, 747),
    values: (58, 236),
    empty_attributes: (91, 93),
    relations: (123, 133),
    relations_verbatim: 123,
    values_different: 39,
    interfaces_not_in_reference: 1,
};

#[test]
fn the_dwg_fixture_alone_states_this_much_of_its_reference() {
    let generated = publish(
        &repo_path(DWG_DIR).join(format!("{DWG_NAME}.pid")),
        &[],
        DWG_NAME,
    );
    hold(
        "DWG-0202GP06-01 · .pid alone",
        &generated,
        &dwg_reference(),
        &DWG_ALONE,
    );
}

#[test]
fn the_dwg_fixture_with_its_packages_dump_states_this_much_of_its_reference() {
    let dump = repo_path(DWG_PACKAGE).join("Export.dmp");
    let datamap = repo_path(DWG_PACKAGE).join("RefData~4~804.zip");
    let generated = publish(
        &repo_path(DWG_DIR).join(format!("{DWG_NAME}.pid")),
        &[
            OsStr::new("--oracle-exp"),
            dump.as_os_str(),
            OsStr::new("--datamap"),
            datamap.as_os_str(),
        ],
        DWG_NAME,
    );
    hold(
        "DWG-0202GP06-01 · --oracle-exp",
        &generated,
        &dwg_reference(),
        &DWG_WITH_ITS_DUMP,
    );
}

#[test]
fn the_dwg_package_states_this_much_of_the_reference() {
    let generated = publish(&repo_path(DWG_PACKAGE), &[], DWG_NAME);
    hold(
        "DWG-0202GP06-01 · backup package directory",
        &generated,
        &dwg_reference(),
        &DWG_PACKAGE_DIRECTORY,
    );
}

// ---------------------------------------------------------------------
// The seven acceptance drawings whose `DocUID` is in the 53-sheet plant
// backup: the dump is the reference's own revision, so every object and
// relation is stated in `--oracle-exp` mode and what is left is interface
// elements and values. The corpus is not in the repository.
// ---------------------------------------------------------------------

const ACCEPTANCE_ENV: &str = "PID_ACCEPTANCE_CORPUS";
const ACCEPTANCE_DEFAULT: &str = r"D:\work\cad\pid-test-data\验收数据";
const PACKAGE_ENV: &str = "PID_TEST_DATA_DIR";
const PACKAGE_DEFAULT: &str = r"D:\work\cad\pid-test-data";

fn acceptance_dir() -> PathBuf {
    std::env::var_os(ACCEPTANCE_ENV)
        .map_or_else(|| PathBuf::from(ACCEPTANCE_DEFAULT), PathBuf::from)
}

fn package_dir() -> PathBuf {
    std::env::var_os(PACKAGE_ENV).map_or_else(|| PathBuf::from(PACKAGE_DEFAULT), PathBuf::from)
}

/// One acceptance drawing and what each mode states of its reference.
struct Drawing {
    /// The short name the baseline and the analysis pages use.
    key: &'static str,
    /// The file stem of the `.pid` and of its `_Data.xml`.
    stem: &'static str,
    alone: Pin,
    with_the_dump: Pin,
}

/// Hold `drawing` to its two pins, or say which directory kept a mode from
/// running.
fn hold_acceptance_drawing(drawing: &Drawing) {
    let Drawing { key, stem, .. } = *drawing;
    let corpus = acceptance_dir();
    if !corpus.is_dir() {
        eprintln!(
            "skipping: {key} — the acceptance corpus directory {} is absent (set {ACCEPTANCE_ENV})",
            corpus.display()
        );
        return;
    }
    let pid = corpus.join(format!("{stem}.pid"));
    let reference = corpus.join(format!("{stem}_Data.xml"));
    if !pid.is_file() || !reference.is_file() {
        eprintln!(
            "skipping: {key} — the acceptance corpus directory {} holds no {stem}.pid beside a \
             {stem}_Data.xml (set {ACCEPTANCE_ENV})",
            corpus.display()
        );
        return;
    }
    let reference_xml = read(&reference);
    hold(
        &format!("{key} · .pid alone"),
        &publish(&pid, &[], stem),
        &reference_xml,
        &drawing.alone,
    );

    let package = package_dir();
    let dump = package.join("Export.dmp");
    let datamap = package.join("RefData~4~804.zip");
    if !dump.is_file() || !datamap.is_file() {
        eprintln!(
            "skipping: {key} · --oracle-exp — the backup package directory {} is absent or holds no \
             Export.dmp beside a RefData~4~804.zip (set {PACKAGE_ENV})",
            package.display()
        );
        return;
    }
    let generated = publish(
        &pid,
        &[
            OsStr::new("--oracle-exp"),
            dump.as_os_str(),
            OsStr::new("--datamap"),
            datamap.as_os_str(),
        ],
        stem,
    );
    hold(
        &format!("{key} · --oracle-exp"),
        &generated,
        &reference_xml,
        &drawing.with_the_dump,
    );
}

const I_WELL_SITE: Drawing = Drawing {
    key: "I井场",
    stem: "I井场 注采阀组工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (1735, 3093),
        paired_by_name: 0,
        interfaces: (13274, 22930),
        values: (1530, 6702),
        empty_attributes: (2496, 2802),
        relations: (1971, 4107),
        relations_verbatim: 1971,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (3093, 3093),
        paired_by_name: 79,
        interfaces: (22659, 22930),
        values: (3323, 6702),
        empty_attributes: (2496, 2802),
        relations: (4107, 4107),
        relations_verbatim: 4001,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const JK_WELL_SITE: Drawing = Drawing {
    key: "J K井场",
    stem: "J K井场 注采阀组工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (2605, 4655),
        paired_by_name: 0,
        interfaces: (19937, 34527),
        values: (2296, 10034),
        empty_attributes: (3772, 4241),
        relations: (2955, 6175),
        relations_verbatim: 2955,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (4655, 4655),
        paired_by_name: 123,
        interfaces: (34135, 34527),
        values: (5019, 10034),
        empty_attributes: (3775, 4241),
        relations: (6175, 6175),
        relations_verbatim: 6012,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const LM_WELL_SITE: Drawing = Drawing {
    key: "L M井场",
    stem: "L M井场 注采阀组工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (3068, 5493),
        paired_by_name: 0,
        interfaces: (23535, 40755),
        values: (2720, 11847),
        empty_attributes: (4493, 5029),
        relations: (3474, 7274),
        relations_verbatim: 3474,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (5493, 5493),
        paired_by_name: 141,
        interfaces: (40301, 40755),
        values: (5954, 11847),
        empty_attributes: (4493, 5029),
        relations: (7274, 7274),
        relations_verbatim: 7082,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const LIGHT_HYDROCARBON_SEPARATOR: Drawing = Drawing {
    key: "轻烃分离器I",
    stem: "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (667, 1219),
        paired_by_name: 0,
        interfaces: (5005, 8906),
        values: (578, 2574),
        empty_attributes: (884, 1046),
        relations: (789, 1685),
        relations_verbatim: 789,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (1219, 1219),
        paired_by_name: 31,
        interfaces: (8814, 8906),
        values: (1292, 2574),
        empty_attributes: (884, 1046),
        relations: (1685, 1685),
        relations_verbatim: 1638,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const AIR_COOLER_A: Drawing = Drawing {
    key: "空冷器A",
    stem: "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (695, 1260),
        paired_by_name: 0,
        interfaces: (5301, 9446),
        values: (613, 2733),
        empty_attributes: (1068, 1220),
        relations: (780, 1652),
        relations_verbatim: 780,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (1260, 1260),
        paired_by_name: 47,
        interfaces: (9320, 9446),
        values: (1410, 2733),
        empty_attributes: (1068, 1220),
        relations: (1652, 1652),
        relations_verbatim: 1596,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const AIR_COOLER_B: Drawing = Drawing {
    key: "空冷器B",
    stem: "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (699, 1248),
        paired_by_name: 0,
        interfaces: (5331, 9293),
        values: (615, 2665),
        empty_attributes: (1077, 1184),
        relations: (784, 1636),
        relations_verbatim: 784,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (1248, 1248),
        paired_by_name: 30,
        interfaces: (9189, 9293),
        values: (1381, 2665),
        empty_attributes: (1077, 1184),
        relations: (1636, 1636),
        relations_verbatim: 1603,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

const AIR_COOLER_C: Drawing = Drawing {
    key: "空冷器C",
    stem: "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
    alone: Pin {
        container: (15, 15),
        objects: (690, 1229),
        paired_by_name: 0,
        interfaces: (5245, 9126),
        values: (607, 2616),
        empty_attributes: (1054, 1158),
        relations: (775, 1614),
        relations_verbatim: 775,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
    with_the_dump: Pin {
        container: (15, 15),
        objects: (1229, 1229),
        paired_by_name: 28,
        interfaces: (9027, 9126),
        values: (1355, 2616),
        empty_attributes: (1054, 1158),
        relations: (1614, 1614),
        relations_verbatim: 1582,
        values_different: 0,
        interfaces_not_in_reference: 0,
    },
};

#[test]
fn the_i_well_site_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&I_WELL_SITE);
}

#[test]
fn the_jk_well_site_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&JK_WELL_SITE);
}

#[test]
fn the_lm_well_site_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&LM_WELL_SITE);
}

#[test]
fn the_light_hydrocarbon_separator_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&LIGHT_HYDROCARBON_SEPARATOR);
}

#[test]
fn the_air_cooler_a_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&AIR_COOLER_A);
}

#[test]
fn the_air_cooler_b_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&AIR_COOLER_B);
}

#[test]
fn the_air_cooler_c_states_this_much_of_its_reference() {
    hold_acceptance_drawing(&AIR_COOLER_C);
}
