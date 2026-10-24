//! Which representations are branch points, and does the run's
//! `ModelItemType` decide piping versus signal?
//!
//! Plan `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md`
//! N4-3. Read-only, on the five drawings of `D:\work\cad\PID自研软件验证`
//! (or `PID_PARSE_SELF_CHECK_DIR`).
//!
//! The candidate rule, written before scoring: a `P&IDAttributes` row whose
//! `DrawingItemType` is `Symbol` and whose described record is an
//! `igPoint2d` (`0x005E`) is a branch-point representation. Its published
//! UID is `DrawingID` & `.` & `BPT` (`ConstructUIDForBranchPoint`,
//! `0x5F5CFC10`). `ModelItemType = PipeRun` publishes
//! `PIDPipingBranchPoint`; `SignalRun` publishes `PIDSignalBranchPoint`;
//! any other type is not emitted. `DrawingItems` is
//! `DRI-{DrawingID}-{DrawingID.BPT}` (representation -> branch point).
//! `PIDBranchPoint` (OPC) is not this rule.
//!
//! Nothing is written. `cargo run --example probe_branch_points_from_representations`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::attribute_rows::{decode_attribute_row, AttributeRow};
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use pid_parse::publish::load_publish_from_pid;
use quick_xml::events::Event;
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
const POINT: u16 = 0x005E;

fn corpus() -> Vec<(String, PathBuf, PathBuf)> {
    let corpus =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&corpus)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();
    drawings
}

struct FileRecords {
    families: HashMap<u32, u16>,
    rows: BTreeMap<u32, AttributeRow>,
}

fn read_records(path: &Path) -> FileRecords {
    let mut records = FileRecords {
        families: HashMap::new(),
        rows: BTreeMap::new(),
    };
    let Ok(file) = std::fs::File::open(path) else {
        return records;
    };
    let Ok(mut compound) = cfb::CompoundFile::open(file) else {
        return records;
    };
    let streams: Vec<PathBuf> = compound
        .read_root_storage()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .collect();
    for stream_path in streams {
        let mut data = Vec::new();
        let Ok(mut stream) = compound.open_stream(&stream_path) else {
            continue;
        };
        if stream.read_to_end(&mut data).is_err()
            || data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..])
        {
            continue;
        }
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            let Some(payload) = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .and_then(|end| data.get(header.body_start..end))
            else {
                continue;
            };
            let Some(oid) = payload
                .get(0..4)
                .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            else {
                continue;
            };
            records.families.insert(oid, header.type_code);
            if header.type_code == FREE_FORM_ATTR_SET {
                if let Some(row) = decode_attribute_row(payload) {
                    records.rows.insert(row.oid, row);
                }
            }
        }
    }
    records
}

struct RefObject {
    uid: String,
    name: String,
    interfaces: BTreeSet<String>,
}

struct Reference {
    objects: BTreeMap<String, Vec<RefObject>>,
    /// `DrawingItems` whose `UID2` ends in `.BPT`: rel UID, UID1, UID2.
    branch_drawing_items: Vec<(String, String, String)>,
}

fn reference(xml: &str) -> Reference {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut objects: BTreeMap<String, Vec<RefObject>> = BTreeMap::new();
    let mut branch_drawing_items = Vec::new();
    let mut depth = 0usize;
    let mut block: Option<String> = None;
    let mut current: Option<RefObject> = None;
    let mut rel_uid = String::new();
    let mut rel_uid1 = String::new();
    let mut rel_uid2 = String::new();
    let mut rel_def = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    block = Some(String::from_utf8_lossy(start.name().as_ref()).into_owned());
                    current = None;
                    rel_uid.clear();
                    rel_uid1.clear();
                    rel_uid2.clear();
                    rel_def.clear();
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                let attr = |key: &[u8]| {
                    start.attributes().flatten().find_map(|a| {
                        (a.key.as_ref() == key)
                            .then(|| String::from_utf8_lossy(&a.value).into_owned())
                    })
                };
                if depth == 2 {
                    if let Some(class) = block.as_deref() {
                        if class == "PIDPipingBranchPoint" || class == "PIDSignalBranchPoint" {
                            if name == "IObject" {
                                current = Some(RefObject {
                                    uid: attr(b"UID").unwrap_or_default(),
                                    name: attr(b"Name").unwrap_or_default(),
                                    interfaces: BTreeSet::new(),
                                });
                            } else if let Some(object) = current.as_mut() {
                                object.interfaces.insert(name);
                            }
                        } else if class == "Rel" {
                            if name == "IObject" {
                                rel_uid = attr(b"UID").unwrap_or_default();
                            } else if name == "IRel" {
                                rel_uid1 = attr(b"UID1").unwrap_or_default();
                                rel_uid2 = attr(b"UID2").unwrap_or_default();
                                rel_def = attr(b"DefUID").unwrap_or_default();
                            }
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                if depth == 2 {
                    if let Some(class) = block.clone() {
                        if let Some(object) = current.take() {
                            objects.entry(class.clone()).or_default().push(object);
                        }
                        if class == "Rel" && rel_def == "DrawingItems" && rel_uid2.ends_with(".BPT")
                        {
                            branch_drawing_items.push((
                                rel_uid.clone(),
                                rel_uid1.clone(),
                                rel_uid2.clone(),
                            ));
                        }
                    }
                    block = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Reference {
        objects,
        branch_drawing_items,
    }
}

/// `PipeRun` -> piping branch point, `SignalRun` -> signal branch point.
fn class_of(model_item_type: &str) -> Option<&'static str> {
    match model_item_type {
        "PipeRun" => Some("PIDPipingBranchPoint"),
        "SignalRun" => Some("PIDSignalBranchPoint"),
        _ => None,
    }
}

struct Candidate {
    drawing_id: String,
    model_item_type: String,
    model_id: String,
}

fn main() {
    let drawings = corpus();
    if drawings.is_empty() {
        println!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    }
    let mut total_right = 0usize;
    let mut total_wrong = 0usize;
    let mut total_undecided = 0usize;
    let mut total_missing = 0usize;
    let mut total_extra = 0usize;
    let mut total_dri_right = 0usize;
    let mut total_dri_wrong = 0usize;
    let mut total_dri_missing = 0usize;

    for (stem, pid, xml_path) in &drawings {
        let records = read_records(pid);
        let mut candidates = Vec::new();
        let mut field_names: BTreeMap<String, usize> = BTreeMap::new();
        for row in records.rows.values() {
            if row.class != "P&IDAttributes" || row.text("DrawingItemType") != Some("Symbol") {
                continue;
            }
            let family = records.families.get(&row.described).copied().unwrap_or(0);
            if family != POINT {
                continue;
            }
            let Some(drawing_id) = row.text("DrawingID").filter(|uid| uid.len() > 1) else {
                continue;
            };
            for field in &row.fields {
                *field_names.entry(field.name.clone()).or_insert(0) += 1;
            }
            candidates.push(Candidate {
                drawing_id: drawing_id.to_string(),
                model_item_type: row.text("ModelItemType").unwrap_or("").to_string(),
                model_id: row.text("ModelID").unwrap_or("").to_string(),
            });
        }
        let loaded = load_publish_from_pid(pid).ok();
        let loader_none = loaded.as_ref().map(|drawing| {
            drawing
                .representations
                .iter()
                .filter(|representation| representation.item_uid.is_none())
                .map(|representation| representation.uid.clone())
                .collect::<BTreeSet<_>>()
        });
        let reference = reference(&std::fs::read_to_string(xml_path).unwrap_or_default());
        let mut by_uid: HashMap<String, &str> = HashMap::new();
        for (class, objects) in &reference.objects {
            for object in objects {
                by_uid.insert(object.uid.clone(), class.as_str());
            }
        }
        let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
        let mut right = 0usize;
        let mut wrong = 0usize;
        let mut undecided_hit = 0usize;
        let mut extra = 0usize;
        let mut predicted: BTreeSet<String> = BTreeSet::new();
        let mut dri_expected: BTreeSet<(String, String, String)> = BTreeSet::new();
        let mut wrong_lines = Vec::new();
        for candidate in &candidates {
            *by_type
                .entry(candidate.model_item_type.clone())
                .or_insert(0) += 1;
            let uid = format!("{}.BPT", candidate.drawing_id);
            let found = by_uid.get(&uid).copied();
            match class_of(&candidate.model_item_type) {
                Some(class) => {
                    predicted.insert(uid.clone());
                    let rel = format!("DRI-{}-{}", candidate.drawing_id, uid);
                    dri_expected.insert((rel, candidate.drawing_id.clone(), uid.clone()));
                    match found {
                        Some(got) if got == class => right += 1,
                        Some(got) => {
                            wrong += 1;
                            wrong_lines.push(format!(
                                "  wrong {uid} rule={class} reference={got} run={}",
                                candidate.model_item_type
                            ));
                        }
                        None => {
                            extra += 1;
                            wrong_lines.push(format!(
                                "  extra {uid} rule={class} run={} model={}",
                                candidate.model_item_type, candidate.model_id
                            ));
                        }
                    }
                }
                None => {
                    if found.is_some() {
                        undecided_hit += 1;
                        wrong_lines.push(format!(
                            "  undecided but reference has {uid} type={} model={}",
                            candidate.model_item_type, candidate.model_id
                        ));
                    } else {
                        extra += 1;
                    }
                }
            }
        }
        let mut missing = 0usize;
        for (uid, class) in &by_uid {
            if !predicted.contains(uid) {
                missing += 1;
                if wrong_lines.len() < 12 {
                    wrong_lines.push(format!("  missing {uid} reference={class}"));
                }
            }
        }
        let dri_ref: BTreeSet<(String, String, String)> =
            reference.branch_drawing_items.iter().cloned().collect();
        let dri_right = dri_expected.intersection(&dri_ref).count();
        let dri_wrong = dri_expected.difference(&dri_ref).count();
        let dri_missing = dri_ref.difference(&dri_expected).count();
        let loader_ids = loader_none.unwrap_or_default();
        let candidate_ids: BTreeSet<String> = candidates
            .iter()
            .map(|candidate| candidate.drawing_id.clone())
            .collect();
        let names_filled = reference
            .objects
            .values()
            .flatten()
            .filter(|object| !object.name.is_empty())
            .count();
        println!("\n==== {stem}");
        println!(
            "  point-symbols {}  loader item_uid=None {}  same set {}",
            candidates.len(),
            loader_ids.len(),
            loader_ids == candidate_ids
        );
        println!("  ModelItemType {by_type:?}");
        println!("  fields {field_names:?}");
        println!(
            "  rule right {right}  wrong {wrong}  extra {extra}  undecided-but-in-reference {undecided_hit}  missing {missing}"
        );
        println!(
            "  reference piping {} signal {}  names filled {names_filled}",
            reference
                .objects
                .get("PIDPipingBranchPoint")
                .map_or(0, Vec::len),
            reference
                .objects
                .get("PIDSignalBranchPoint")
                .map_or(0, Vec::len),
        );
        if let Some(sample) = reference
            .objects
            .get("PIDPipingBranchPoint")
            .and_then(|v| v.first())
        {
            println!("  piping interfaces {:?}", sample.interfaces);
        }
        if let Some(sample) = reference
            .objects
            .get("PIDSignalBranchPoint")
            .and_then(|v| v.first())
        {
            println!("  signal interfaces {:?}", sample.interfaces);
        }
        println!(
            "  DrawingItems .BPT right {dri_right}  not in reference {dri_wrong}  reference not predicted {dri_missing}  reference rows {}",
            reference.branch_drawing_items.len()
        );
        for line in wrong_lines.iter().take(8) {
            println!("{line}");
        }
        total_right += right;
        total_wrong += wrong;
        total_undecided += undecided_hit;
        total_missing += missing;
        total_extra += extra;
        total_dri_right += dri_right;
        total_dri_wrong += dri_wrong;
        total_dri_missing += dri_missing;
    }
    println!(
        "\nTOTAL right {total_right} wrong {total_wrong} extra {total_extra} undecided-but-in-reference {total_undecided} missing {total_missing}"
    );
    println!(
        "TOTAL DrawingItems right {total_dri_right} not-in-reference {total_dri_wrong} reference-not-predicted {total_dri_missing}"
    );
}
