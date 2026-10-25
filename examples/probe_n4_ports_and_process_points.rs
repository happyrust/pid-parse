//! Where a `.pid` keeps the points N4 publishes, and whether they rebuild
//! the reference ports and process points.
//!
//! N4-2. `ConstructUIDForPort` (`0x5F5CFE70`) builds a port UID as the
//! parent item's id, a dot, and the point's `*PointNumber`.
//! `ConstructUIDForProcessPoint` (`0x5F5CFD40`) builds a process point as
//! the pipe run's own id plus `.PPT`. Composition relationships use the
//! three-letter prefixes in `docs/analysis/n2a-port-relation-filter.md`.
//! This probe reads those facts back out of the five drawings in
//! `D:\work\cad\PID自研软件验证` and scores them against each
//! `<stem>_Data.xml`. It does not write publish XML.
//!
//! `PID_PARSE_SELF_CHECK_DIR` points it at another directory. With neither
//! that nor the default directory present it prints `skip:` and exits.
//!
//! ```powershell
//! cargo run --example probe_n4_ports_and_process_points
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::attribute_rows::{
    decode_attribute_row, AttributeField, AttributeRow, AttributeValue,
};
use pid_parse::parsers::sheet_records::{
    decode_igsymbol_at, parse_live_psm_header, sheet_record_starts,
};
use pid_parse::parsers::undecoded_census::rad_class_name;
use pid_parse::publish::{load_publish_from_pid, PidPublishDrawing};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
/// `PortFlowDirectionConv` (`0x5F583230`): ordinal, then the published GUID.
const FLOW_GUID: [(i32, &str); 5] = [
    (1, "{23F94152-B87D-11D6-80D4-00A0C92487AF}"),
    (2, "{23F94146-B87D-11D6-80D4-00A0C92487AF}"),
    (3, "{23F9414C-B87D-11D6-80D4-00A0C92487AF}"),
    (4, "{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}"),
    (5, "{23F94152-B87D-11D6-80D4-00A0C92487AF}"),
];
const NEEDLES: [&str; 7] = [
    "PipingPoint",
    "SignalPoint",
    "DuctingPoint",
    "PipingPointNumber",
    "SignalPointNumber",
    "FlowDirection",
    "_ConnectPoint",
];

#[derive(Clone)]
struct Port {
    name: String,
    flow: String,
}

struct Reference {
    piping: HashMap<String, Port>,
    signal: HashMap<String, Port>,
    process: HashSet<String>,
    connectors: HashSet<String>,
    /// `(rel UID, UID1, UID2)` for one `DefUID`.
    ppc: Vec<(String, String, String)>,
    spc: Vec<(String, String, String)>,
    prp: Vec<(String, String, String)>,
}

struct RecordFact {
    type_code: u16,
    parent: u32,
}

struct PointRow {
    class: String,
    stream: u32,
    described: u32,
    fields: Vec<(String, String)>,
}

struct PointGeom {
    index: u32,
    sub_type: u16,
}

struct PlacedSymbol {
    stream: u32,
    oid: u32,
    definition_sheet_ref: u32,
    definition_site_ref: u32,
}

struct Scan {
    streams: usize,
    chain_streams: usize,
    decoded_rows: usize,
    undecoded_rows: usize,
    strings: BTreeMap<String, usize>,
    classes: BTreeMap<String, usize>,
    /// Field name -> count, over rows whose class looks like a point.
    point_fields: BTreeMap<String, usize>,
    /// `(stream, oid)` -> record. Oids repeat across streams.
    records: HashMap<(u32, u32), RecordFact>,
    /// Graphic oid -> `ModelID`, for item rows only. Oids are not unique
    /// across streams; this map is only a hint for the attribute walk.
    models: HashMap<u32, String>,
    /// `(stream, graphic oid)` -> `ModelID`.
    models_at: HashMap<(u32, u32), String>,
    points: Vec<PointRow>,
    /// `(stream, oid)` of a `0x005E` point.
    point_geom: HashMap<(u32, u32), PointGeom>,
    symbols: Vec<PlacedSymbol>,
    stream_site: HashMap<u32, Option<u32>>,
    stream_path: HashMap<u32, String>,
}

fn main() {
    let Some(drawings) = corpus() else {
        println!("skip: no drawings in {DEFAULT_CORPUS} or {CORPUS_ENV}");
        return;
    };
    let mut piping_same = 0usize;
    let mut signal_same = 0usize;
    let mut process_same = 0usize;
    let mut process_extra = 0usize;
    let mut process_missing = 0usize;
    for (stem, pid, xml) in &drawings {
        println!("== {stem}");
        let reference = read_reference(xml);
        let uid2_is_parent = report_reference(&reference);
        let drawing = load_publish_from_pid(pid).expect("the .pid loads");
        let (same, extra, missing) = report_process_points(&drawing, &reference, uid2_is_parent);
        process_same += same;
        process_extra += extra;
        process_missing += missing;
        let scan = scan_pid(pid);
        let (piping, signal) = report_points(&scan, &drawing, &reference, uid2_is_parent);
        piping_same += piping;
        signal_same += signal;
    }
    println!(
        "corpus process same={process_same} extra={process_extra} missing={process_missing} \
         piping_uid_same={piping_same} signal_uid_same={signal_same}"
    );
    println!(
        "VERDICT process EMIT when extra=0 and missing=0 (ConstructUIDForProcessPoint 0x5F5CFD40, PRP)"
    );
    println!(
        "VERDICT ports HOLD: no PipingPointNumber / SignalPointNumber / FlowDirection bytes; \
         _ConnectPoint.index is not the point number, so piping and signal ports are not emitted"
    );
}

fn corpus() -> Option<Vec<(String, PathBuf, PathBuf)>> {
    let root =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = root.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();
    (!drawings.is_empty()).then_some(drawings)
}

fn attributes_of(start: &BytesStart<'_>) -> Vec<(String, String)> {
    start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                attribute.key.as_ref().to_string(),
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn read_reference(path: &Path) -> Reference {
    let xml = std::fs::read_to_string(path).expect("reference xml");
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut reference = Reference {
        piping: HashMap::new(),
        signal: HashMap::new(),
        process: HashSet::new(),
        connectors: HashSet::new(),
        ppc: Vec::new(),
        spc: Vec::new(),
        prp: Vec::new(),
    };
    let mut depth = 0usize;
    let mut tag = String::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    tag = start.name().as_ref().to_string();
                    values.clear();
                } else if depth == 2 {
                    take_interface(&start, &mut values);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    take_interface(&start, &mut values);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    finish_block(&mut reference, &tag, &mut values);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    reference
}

fn take_interface(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = start.name().as_ref().to_string();
    for (name, value) in attributes_of(start) {
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn finish_block(reference: &mut Reference, tag: &str, values: &mut BTreeMap<String, String>) {
    let Some(uid) = values.remove("IObject.UID") else {
        return;
    };
    if tag == "Rel" {
        let uid1 = values.remove("IRel.UID1").unwrap_or_default();
        let uid2 = values.remove("IRel.UID2").unwrap_or_default();
        let def = values.remove("IRel.DefUID").unwrap_or_default();
        let slot = match def.as_str() {
            "PipingPortComposition" => Some(&mut reference.ppc),
            "SignalPortComposition" => Some(&mut reference.spc),
            "ProcessPointCollection" => Some(&mut reference.prp),
            _ => None,
        };
        if let Some(slot) = slot {
            slot.push((uid, uid1, uid2));
        }
        return;
    }
    let name = values.remove("IObject.Name").unwrap_or_default();
    let flow = values
        .remove("IConnection.ConnectionFlowDirection")
        .unwrap_or_default();
    match tag {
        "PIDPipingPort" => {
            reference.piping.insert(uid, Port { name, flow });
        }
        "PIDSignalPort" => {
            reference.signal.insert(uid, Port { name, flow });
        }
        "PIDProcessPoint" => {
            reference.process.insert(uid);
        }
        "PIDPipingConnector" => {
            reference.connectors.insert(uid);
        }
        _ => {}
    }
}

/// How the reference composes a port UID, and which rel end is the parent.
/// Returns whether `UID2` is the parent.
fn report_reference(reference: &Reference) -> bool {
    let (piping_uid2, piping_uid1, piping_neither) = rel_parent(&reference.ppc, &reference.piping);
    let (signal_uid2, signal_uid1, signal_neither) = rel_parent(&reference.spc, &reference.signal);
    let (process_uid2, process_uid1, process_neither) =
        rel_parent_process(&reference.prp, &reference.process);
    println!(
        "  ref piping={} signal={} process={} connectors={} PPC={} SPC={} PRP={}",
        reference.piping.len(),
        reference.signal.len(),
        reference.process.len(),
        reference.connectors.len(),
        reference.ppc.len(),
        reference.spc.len(),
        reference.prp.len(),
    );
    println!(
        "  rel parent is UID2: piping {piping_uid2} (UID1 {piping_uid1}, neither {piping_neither}) \
         signal {signal_uid2} (UID1 {signal_uid1}, neither {signal_neither}) \
         process {process_uid2} (UID1 {process_uid1}, neither {process_neither})"
    );
    let flow = reference
        .piping
        .values()
        .filter(|port| !port.flow.is_empty())
        .count();
    println!("  ref piping ConnectionFlowDirection non-empty={flow}");
    piping_uid2 >= piping_uid1
}

fn rel_parent(
    rels: &[(String, String, String)],
    ports: &HashMap<String, Port>,
) -> (usize, usize, usize) {
    let mut uid2_parent = 0usize;
    let mut uid1_parent = 0usize;
    let mut neither = 0usize;
    for (rel_uid, uid1, uid2) in rels {
        let port_uid = if ports.contains_key(uid1) {
            uid1
        } else if ports.contains_key(uid2) {
            uid2
        } else {
            neither += 1;
            continue;
        };
        let port = &ports[port_uid];
        let suffix = &port.name;
        let built_from_uid2 = format!("{uid2}.{suffix}");
        let built_from_uid1 = format!("{uid1}.{suffix}");
        if port_uid == &built_from_uid2 && rel_uid_matches(rel_uid, uid1, uid2) {
            uid2_parent += 1;
        } else if port_uid == &built_from_uid1 && rel_uid_matches(rel_uid, uid1, uid2) {
            uid1_parent += 1;
        } else {
            neither += 1;
        }
    }
    (uid2_parent, uid1_parent, neither)
}

fn rel_parent_process(
    rels: &[(String, String, String)],
    points: &HashSet<String>,
) -> (usize, usize, usize) {
    let mut uid2_parent = 0usize;
    let mut uid1_parent = 0usize;
    let mut neither = 0usize;
    for (rel_uid, uid1, uid2) in rels {
        let from_uid2 = format!("{uid2}.PPT");
        let from_uid1 = format!("{uid1}.PPT");
        if points.contains(&from_uid2) && rel_uid_matches(rel_uid, uid1, uid2) {
            uid2_parent += 1;
        } else if points.contains(&from_uid1) && rel_uid_matches(rel_uid, uid1, uid2) {
            uid1_parent += 1;
        } else {
            neither += 1;
        }
    }
    (uid2_parent, uid1_parent, neither)
}

fn rel_uid_matches(rel_uid: &str, uid1: &str, uid2: &str) -> bool {
    let Some((prefix, rest)) = rel_uid.split_once('-') else {
        return false;
    };
    prefix.len() == 3 && rest == format!("{uid1}-{uid2}")
}

fn report_process_points(
    drawing: &PidPublishDrawing,
    reference: &Reference,
    uid2_is_parent: bool,
) -> (usize, usize, usize) {
    let ours: HashSet<String> = drawing
        .items
        .iter()
        .filter(|item| item.class == Some("PIDPipingConnector"))
        .map(|item| format!("{}.PPT", item.uid))
        .collect();
    let same = ours.intersection(&reference.process).count();
    let extra = ours.len() - same;
    let missing = reference.process.len() - same;
    let mut rel_same = 0usize;
    let mut rel_extra = 0usize;
    for uid in &ours {
        let parent = uid.trim_end_matches(".PPT");
        let (uid1, uid2) = if uid2_is_parent {
            (uid.as_str(), parent)
        } else {
            (parent, uid.as_str())
        };
        let rel = format!("PRP-{uid1}-{uid2}");
        if reference
            .prp
            .iter()
            .any(|(id, a, b)| id == &rel && a == uid1 && b == uid2)
        {
            rel_same += 1;
        } else {
            rel_extra += 1;
        }
    }
    println!(
        "  process from published connectors: same={same} extra={extra} missing={missing} \
         PRP same={rel_same} extra={rel_extra} (UID2 parent={uid2_is_parent})"
    );
    if extra + missing > 0 {
        let mut listed = 0usize;
        for uid in ours.difference(&reference.process) {
            println!("    extra process {uid}");
            listed += 1;
            if listed == 8 {
                break;
            }
        }
    }
    (same, extra, missing)
}

fn scan_pid(path: &Path) -> Scan {
    let mut scan = Scan {
        streams: 0,
        chain_streams: 0,
        decoded_rows: 0,
        undecoded_rows: 0,
        strings: BTreeMap::new(),
        classes: BTreeMap::new(),
        point_fields: BTreeMap::new(),
        records: HashMap::new(),
        models: HashMap::new(),
        models_at: HashMap::new(),
        points: Vec::new(),
        point_geom: HashMap::new(),
        symbols: Vec::new(),
        stream_site: HashMap::new(),
        stream_path: HashMap::new(),
    };
    let file = std::fs::File::open(path).expect("open pid");
    let mut compound = CompoundFile::open(file).expect("compound file");
    let paths: Vec<String> = compound
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    for stream_path in paths {
        let Ok(mut stream) = compound.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        scan.streams += 1;
        count_strings(&data, &mut scan.strings);
        if data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..]) {
            continue;
        }
        let stream_index = scan.chain_streams as u32;
        scan.chain_streams += 1;
        scan.stream_site.insert(stream_index, site_id(&stream_path));
        scan.stream_path.insert(stream_index, stream_path);
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            if header.native_reader_skips() {
                continue;
            }
            let Some(payload) = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .and_then(|end| data.get(header.body_start..end))
            else {
                continue;
            };
            let Some(oid) = u32_at(payload, 0) else {
                continue;
            };
            let parent = u32_at(payload, 4).unwrap_or(0);
            scan.records.insert(
                (stream_index, oid),
                RecordFact {
                    type_code: header.type_code,
                    parent,
                },
            );
            if header.type_code == 0x005E && payload.len() >= 18 {
                scan.point_geom.insert(
                    (stream_index, oid),
                    PointGeom {
                        index: u32_at(payload, 14).unwrap_or(0),
                        sub_type: u16_at(payload, 12).unwrap_or(0),
                    },
                );
            }
            if header.type_code == 0x00CE {
                if let Some(symbol) = decode_igsymbol_at(&data, at) {
                    scan.symbols.push(PlacedSymbol {
                        stream: stream_index,
                        oid: symbol.oid,
                        definition_sheet_ref: symbol.definition_sheet_ref,
                        definition_site_ref: symbol.definition_site_ref,
                    });
                }
            }
            if header.type_code != FREE_FORM_ATTR_SET {
                continue;
            }
            let Some(row) = decode_attribute_row(payload) else {
                scan.undecoded_rows += 1;
                continue;
            };
            scan.decoded_rows += 1;
            note_row(&mut scan, stream_index, &row);
        }
    }
    scan
}

fn site_id(path: &str) -> Option<u32> {
    path.split(['/', '\\']).find_map(|part| {
        part.strip_prefix("JSite")
            .and_then(|id| id.parse::<u32>().ok())
    })
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn count_strings(data: &[u8], counts: &mut BTreeMap<String, usize>) {
    for needle in NEEDLES {
        let ascii = needle.as_bytes();
        let mut at = 0usize;
        let mut ascii_hits = 0usize;
        while let Some(found) = data[at..]
            .windows(ascii.len())
            .position(|window| window == ascii)
        {
            ascii_hits += 1;
            at += found + ascii.len();
        }
        let wide: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
        at = 0;
        let mut wide_hits = 0usize;
        while let Some(found) = data[at..]
            .windows(wide.len())
            .position(|window| window == wide)
        {
            wide_hits += 1;
            at += found + wide.len();
        }
        if ascii_hits + wide_hits > 0 {
            *counts.entry(needle.to_string()).or_default() += ascii_hits + wide_hits;
        }
    }
}

fn note_row(scan: &mut Scan, stream: u32, row: &AttributeRow) {
    if row.class == "P&IDAttributes" {
        if let Some(model) = row.text("ModelID").filter(|model| model.len() > 1) {
            if matches!(row.text("DrawingItemType"), Some("Symbol" | "Connector")) {
                scan.models.insert(row.described, model.to_string());
                scan.models_at
                    .insert((stream, row.described), model.to_string());
            }
        }
    }
    let interesting = class_is_point(&row.class) || row.fields.iter().any(field_is_point);
    if !interesting {
        return;
    }
    *scan.classes.entry(row.class.clone()).or_default() += 1;
    if class_is_point(&row.class) {
        for field in &row.fields {
            *scan.point_fields.entry(field.name.clone()).or_default() += 1;
        }
    }
    scan.points.push(PointRow {
        class: row.class.clone(),
        stream,
        described: row.described,
        fields: row
            .fields
            .iter()
            .map(|field| (field.name.clone(), show_value(&field.value)))
            .collect(),
    });
}

fn class_is_point(class: &str) -> bool {
    let lower = class.to_ascii_lowercase();
    lower.contains("point") || lower.contains("connect") || lower.contains("flow")
}

fn field_is_point(field: &AttributeField) -> bool {
    if matches!(
        field.name.as_str(),
        "PipingPointNumber"
            | "SignalPointNumber"
            | "SP_DuctingPointNumber"
            | "FlowDirection"
            | "ForeignKey"
            | "SP_PlantItemID"
            | "PipingPointUsage"
    ) || field.name.contains("PointNumber")
        || field.name.contains("ConnectPoint")
    {
        return true;
    }
    let AttributeValue::Text(value) = &field.value else {
        return false;
    };
    field.name == "Type"
        && matches!(
            value.as_str(),
            "PipingPoint" | "SignalPoint" | "DuctingPoint"
        )
}

fn show_value(value: &AttributeValue) -> String {
    let text = match value {
        AttributeValue::Text(text) => text.clone(),
        AttributeValue::Integer(number) => number.to_string(),
        AttributeValue::Boolean(flag) => flag.to_string(),
    };
    if text.chars().count() > 80 {
        let mut short: String = text.chars().take(80).collect();
        short.push('…');
        short
    } else {
        text
    }
}

fn report_points(
    scan: &Scan,
    drawing: &PidPublishDrawing,
    reference: &Reference,
    uid2_is_parent: bool,
) -> (usize, usize) {
    println!(
        "  streams={} chains={} decoded_rows={} undecoded={} point_rows={}",
        scan.streams,
        scan.chain_streams,
        scan.decoded_rows,
        scan.undecoded_rows,
        scan.points.len()
    );
    if !scan.strings.is_empty() {
        let text = scan
            .strings
            .iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  strings {text}");
    }
    if !scan.classes.is_empty() {
        let text = scan
            .classes
            .iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  point classes {text}");
    }
    if !scan.point_fields.is_empty() {
        let text = scan
            .point_fields
            .iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  point fields {text}");
    }
    let published: HashMap<&str, &str> = drawing
        .items
        .iter()
        .filter_map(|item| item.class.map(|class| (item.uid.as_str(), class)))
        .collect();
    let mut built_piping: HashMap<String, (String, String, String)> = HashMap::new();
    let mut built_signal: HashMap<String, (String, String)> = HashMap::new();
    let mut no_parent = 0usize;
    let mut no_number = 0usize;
    let mut samples = 0usize;
    for row in &scan.points {
        let kind = point_kind(row);
        let number = point_number(row);
        let parent = parent_model(scan, row);
        if samples < 6 {
            let fields = row
                .fields
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(" ");
            let fact = scan.records.get(&(row.stream, row.described));
            let type_name = fact
                .and_then(|fact| rad_class_name(fact.type_code))
                .unwrap_or("?");
            println!(
                "    sample class={} described={} type={type_name} parent={} kind={} number={} {fields}",
                row.class,
                row.described,
                parent.as_deref().unwrap_or("-"),
                kind.unwrap_or("-"),
                number.map(|number| number.to_string()).unwrap_or_else(|| "-".into()),
            );
            samples += 1;
        }
        let Some(kind) = kind else {
            continue;
        };
        let Some(number) = number else {
            no_number += 1;
            continue;
        };
        let Some(parent) = parent else {
            no_parent += 1;
            continue;
        };
        if !published.contains_key(parent.as_str()) {
            continue;
        }
        let uid = format!("{parent}.{number}");
        let name = number.to_string();
        if kind == "PipingPoint" {
            let flow = flow_guid(row).unwrap_or_default();
            built_piping.insert(uid, (name, flow, parent));
        } else if kind == "SignalPoint" {
            built_signal.insert(uid, (name, parent));
        }
    }
    let piping_same = built_piping
        .iter()
        .filter(|(uid, (name, _, _))| {
            reference
                .piping
                .get(*uid)
                .is_some_and(|port| port.name == *name)
        })
        .count();
    let signal_same = built_signal
        .iter()
        .filter(|(uid, (name, _))| {
            reference
                .signal
                .get(*uid)
                .is_some_and(|port| port.name == *name)
        })
        .count();
    let piping_flow_same = built_piping
        .iter()
        .filter(|(uid, (_, flow, _))| {
            reference
                .piping
                .get(*uid)
                .is_some_and(|port| flow.is_empty() || port.flow == *flow)
        })
        .count();
    let (ppc_same, spc_same) = score_rels(&built_piping, &built_signal, reference, uid2_is_parent);
    println!(
        "  built piping={} uid+name same={piping_same} flow_ok={piping_flow_same} \
         signal={} uid+name same={signal_same} no_number={no_number} no_parent={no_parent} \
         PPC same={ppc_same} SPC same={spc_same}",
        built_piping.len(),
        built_signal.len(),
    );
    report_index_join(scan, drawing, reference);
    (piping_same, signal_same)
}

fn score_rels(
    piping: &HashMap<String, (String, String, String)>,
    signal: &HashMap<String, (String, String)>,
    reference: &Reference,
    uid2_is_parent: bool,
) -> (usize, usize) {
    let ppc = piping
        .iter()
        .filter(|(port, (_, _, parent))| {
            let (uid1, uid2) = ends(port, parent, uid2_is_parent);
            let rel = format!("PPC-{uid1}-{uid2}");
            reference
                .ppc
                .iter()
                .any(|(id, a, b)| id == &rel && a == uid1 && b == uid2)
        })
        .count();
    let spc = signal
        .iter()
        .filter(|(port, (_, parent))| {
            let (uid1, uid2) = ends(port, parent, uid2_is_parent);
            let rel = format!("SPC-{uid1}-{uid2}");
            reference
                .spc
                .iter()
                .any(|(id, a, b)| id == &rel && a == uid1 && b == uid2)
        })
        .count();
    (ppc, spc)
}

fn ends<'a>(port: &'a str, parent: &'a str, uid2_is_parent: bool) -> (&'a str, &'a str) {
    if uid2_is_parent {
        (port, parent)
    } else {
        (parent, port)
    }
}

fn point_kind(row: &PointRow) -> Option<&str> {
    if matches!(
        row.class.as_str(),
        "PipingPoint" | "SignalPoint" | "DuctingPoint"
    ) {
        return Some(row.class.as_str());
    }
    row.fields.iter().find_map(|(name, value)| {
        (name == "Type"
            && matches!(
                value.as_str(),
                "PipingPoint" | "SignalPoint" | "DuctingPoint"
            ))
        .then_some(value.as_str())
    })
}

fn point_number(row: &PointRow) -> Option<i32> {
    for name in [
        "PipingPointNumber",
        "SignalPointNumber",
        "SP_DuctingPointNumber",
        "PointNumber",
        "Number",
    ] {
        if let Some((_, value)) = row.fields.iter().find(|(field, _)| field == name) {
            if let Ok(number) = value.parse::<i32>() {
                return Some(number);
            }
        }
    }
    None
}

fn flow_guid(row: &PointRow) -> Option<String> {
    let (_, value) = row
        .fields
        .iter()
        .find(|(name, _)| name == "FlowDirection")?;
    if value.is_empty() {
        return None;
    }
    if value.starts_with('{') {
        return Some(value.clone());
    }
    let ordinal = value.parse::<i32>().ok()?;
    FLOW_GUID
        .iter()
        .find(|(candidate, _)| *candidate == ordinal)
        .map(|(_, guid)| (*guid).to_string())
}

fn parent_model(scan: &Scan, row: &PointRow) -> Option<String> {
    for name in ["SP_PlantItemID", "ForeignKey"] {
        if let Some((_, value)) = row.fields.iter().find(|(field, _)| field == name) {
            if value.len() > 8 {
                return Some(value.clone());
            }
        }
    }
    let mut oid = row.described;
    for _ in 0..8 {
        if let Some(model) = scan.models_at.get(&(row.stream, oid)) {
            return Some(model.clone());
        }
        let parent = scan.records.get(&(row.stream, oid))?.parent;
        if parent == 0 || parent == oid {
            break;
        }
        oid = parent;
    }
    None
}

/// Point `index` on the symbol body, joined through the placement's
/// definition sheet. Returns how many built UIDs equal a reference port.
fn report_index_join(scan: &Scan, drawing: &PidPublishDrawing, reference: &Reference) -> usize {
    let published: HashSet<&str> = drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .map(|item| item.uid.as_str())
        .collect();
    let mut index_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut sub_hist: BTreeMap<u16, usize> = BTreeMap::new();
    let mut with_geom = 0usize;
    let mut by_path: BTreeMap<&str, usize> = BTreeMap::new();
    let mut chains_hit_sheet = 0usize;
    for row in scan
        .points
        .iter()
        .filter(|row| row.class == "_ConnectPoint")
    {
        *by_path
            .entry(
                scan.stream_path
                    .get(&row.stream)
                    .map(String::as_str)
                    .unwrap_or("?"),
            )
            .or_default() += 1;
        let Some(geom) = scan.point_geom.get(&(row.stream, row.described)) else {
            continue;
        };
        with_geom += 1;
        *index_hist.entry(geom.index).or_default() += 1;
        *sub_hist.entry(geom.sub_type).or_default() += 1;
        if chain_hits_sheet(scan, row.stream, row.described) {
            chains_hit_sheet += 1;
        }
    }
    let indexes = index_hist
        .iter()
        .map(|(index, count)| format!("{index}:{count}"))
        .collect::<Vec<_>>()
        .join(" ");
    let subs = sub_hist
        .iter()
        .map(|(sub, count)| format!("{sub:#x}:{count}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "  connect points with point-record={} sheet_in_chain={chains_hit_sheet} symbols={}",
        with_geom,
        scan.symbols.len()
    );
    if !indexes.is_empty() {
        println!("  point.index {indexes}");
    }
    if !subs.is_empty() {
        println!("  point.sub_type {subs}");
    }
    let paths = by_path
        .iter()
        .map(|(path, count)| format!("{path}={count}"))
        .collect::<Vec<_>>()
        .join(" ");
    if !paths.is_empty() {
        println!("  connect streams {paths}");
    }
    for row in scan
        .points
        .iter()
        .filter(|row| row.class == "_ConnectPoint")
        .take(4)
    {
        let mut chain = Vec::new();
        let mut at = row.described;
        for _ in 0..6 {
            let fact = scan.records.get(&(row.stream, at));
            let name = fact
                .and_then(|fact| rad_class_name(fact.type_code))
                .unwrap_or("missing");
            chain.push(format!("{at}:{name}"));
            let Some(fact) = fact else {
                break;
            };
            if fact.parent == 0 || fact.parent == at {
                break;
            }
            at = fact.parent;
        }
        let index = scan
            .point_geom
            .get(&(row.stream, row.described))
            .map(|geom| geom.index.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!("    chain index={index} {}", chain.join(" -> "));
    }

    let mut built: HashSet<String> = HashSet::new();
    let mut placements = 0usize;
    for symbol in &scan.symbols {
        let Some(model) = scan.models_at.get(&(symbol.stream, symbol.oid)) else {
            continue;
        };
        if !published.contains(model.as_str()) {
            continue;
        }
        placements += 1;
        for row in scan
            .points
            .iter()
            .filter(|row| row.class == "_ConnectPoint")
        {
            let site = scan.stream_site.get(&row.stream).copied().flatten();
            if site != Some(symbol.definition_site_ref) {
                continue;
            }
            if !chain_contains(scan, row.stream, row.described, symbol.definition_sheet_ref) {
                continue;
            }
            let Some(geom) = scan.point_geom.get(&(row.stream, row.described)) else {
                continue;
            };
            built.insert(format!("{model}.{}", geom.index));
        }
    }
    let ports: HashSet<&str> = reference
        .piping
        .keys()
        .chain(reference.signal.keys())
        .map(String::as_str)
        .collect();
    let same = built
        .iter()
        .filter(|uid| ports.contains(uid.as_str()))
        .count();
    println!(
        "  index join placements={placements} built={} same_as_ref_port={same}",
        built.len()
    );
    same
}

fn chain_hits_sheet(scan: &Scan, stream: u32, oid: u32) -> bool {
    let sheets: HashSet<u32> = scan
        .symbols
        .iter()
        .map(|symbol| symbol.definition_sheet_ref)
        .collect();
    let mut at = oid;
    for _ in 0..8 {
        if sheets.contains(&at) {
            return true;
        }
        let Some(fact) = scan.records.get(&(stream, at)) else {
            return false;
        };
        if fact.parent == 0 || fact.parent == at {
            return false;
        }
        at = fact.parent;
    }
    false
}

fn chain_contains(scan: &Scan, stream: u32, oid: u32, target: u32) -> bool {
    let mut at = oid;
    for _ in 0..8 {
        if at == target {
            return true;
        }
        let Some(fact) = scan.records.get(&(stream, at)) else {
            return false;
        };
        if fact.parent == 0 || fact.parent == at {
            return false;
        }
        at = fact.parent;
    }
    false
}
