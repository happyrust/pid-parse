//! Which connectivity relationships the `.pid` can name, scored on the
//! five reference drawings.
//!
//! N4-4. Reference `<Rel>` rows of the eight publishable definitions are
//! compared with the sheet endpoint pairs the reader already resolves
//! (`source_drawing_id` / `target_drawing_id`). A graphic maps to the
//! published object this exporter already emits: the item a representation
//! names, or `DrawingID.BPT` for a branch point. A relationship is a hit
//! only when that ordered pair is the reference's `UID1`/`UID2`. `PipingConnectors`
//! is counted and not treated as emittable: one end is a pipeline, which
//! the `.pid` does not publish.
//!
//! ```powershell
//! cargo run --example probe_connectivity_relationships
//! ```

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::attribute_rows::decode_attribute_row;
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use pid_parse::publish::load_publish_from_pid;
use pid_parse::PidParser;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const DEFS: [&str; 9] = [
    "PipingEnd1Conn",
    "PipingEnd2Conn",
    "SignalEnd1Conn",
    "SignalEnd2Conn",
    "PipingTapOrFitting",
    "SignalTapOrFitting",
    "EquipmentComponentComposition",
    "PBSItemNotes",
    "PipingConnectors",
];

fn main() {
    let Some(drawings) = corpus() else {
        println!("skip: no drawings in {DEFAULT_CORPUS} or {CORPUS_ENV}");
        return;
    };
    let mut totals: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    for (stem, pid, xml) in &drawings {
        println!("== {stem}");
        report_row_fields(pid);
        let reference = read_reference(xml);
        let drawing = load_publish_from_pid(pid).expect("pid loads");
        let published = published_index(&drawing);
        report_ceiling(&reference, &published);
        let doc = PidParser::new().parse_file(pid).expect("pid parses");
        let graph = doc.object_graph.expect("object graph");
        let mut both = 0usize;
        let mut unresolved = 0usize;
        let mut hits: BTreeMap<&str, usize> = BTreeMap::new();
        let mut extras = 0usize;
        let mut pairs: BTreeMap<String, usize> = BTreeMap::new();
        for rel in &graph.relationships {
            let (Some(source), Some(target)) = (&rel.source_drawing_id, &rel.target_drawing_id)
            else {
                unresolved += 1;
                continue;
            };
            both += 1;
            let left = published.at_drawing(source);
            let right = published.at_drawing(target);
            if left.is_empty() || right.is_empty() {
                continue;
            }
            let mut matched = false;
            for uid1 in &left {
                for uid2 in &right {
                    let key = format!(
                        "{} -> {}",
                        published.class_of(uid1),
                        published.class_of(uid2)
                    );
                    *pairs.entry(key).or_default() += 1;
                    if let Some(def) = reference.find(uid1, uid2) {
                        *hits.entry(def).or_default() += 1;
                        matched = true;
                    }
                }
            }
            if !matched {
                extras += 1;
            }
        }
        println!(
            "  sheet rels={} both_drawing_ids={both} unresolved={unresolved} extras={extras}",
            graph.relationships.len()
        );
        for def in DEFS {
            let hit = hits.get(def).copied().unwrap_or(0);
            let reference_count = reference.count(def);
            let eligible = reference.eligible(def, &published);
            println!(
                "  {def} ref={reference_count} both_ends_published={eligible} sheet_hit={hit}"
            );
            let cell = totals.entry(def).or_default();
            cell.0 += reference_count;
            cell.1 += eligible;
            cell.2 += hit;
        }
        let top = pairs
            .iter()
            .map(|(pair, count)| (count, pair))
            .collect::<Vec<_>>();
        let mut top = top;
        top.sort_by(|a, b| b.0.cmp(a.0).then(a.1.cmp(b.1)));
        for (count, pair) in top.into_iter().take(8) {
            println!("    pair {pair} {count}");
        }
    }
    println!("corpus");
    for (def, (reference, eligible, hit, _)) in &totals {
        println!("  {def} ref={reference} both_ends_published={eligible} sheet_hit={hit}");
    }
}

struct Published {
    /// Drawing id -> published UIDs that graphic stands for.
    by_drawing: HashMap<String, Vec<String>>,
    class_of: HashMap<String, String>,
}

impl Published {
    fn at_drawing(&self, drawing: &str) -> Vec<String> {
        self.by_drawing.get(drawing).cloned().unwrap_or_default()
    }

    fn class_of(&self, uid: &str) -> String {
        self.class_of
            .get(uid)
            .cloned()
            .unwrap_or_else(|| "?".to_string())
    }

    fn contains(&self, uid: &str) -> bool {
        self.class_of.contains_key(uid)
    }
}

fn published_index(drawing: &pid_parse::publish::PidPublishDrawing) -> Published {
    let mut published = Published {
        by_drawing: HashMap::new(),
        class_of: HashMap::new(),
    };
    for item in &drawing.items {
        if let Some(class) = item.class {
            published
                .class_of
                .insert(item.uid.clone(), class.to_string());
        }
    }
    for point in &drawing.process_points {
        published
            .class_of
            .insert(point.uid.clone(), "PIDProcessPoint".to_string());
    }
    for point in &drawing.branch_points {
        published
            .class_of
            .insert(point.uid.clone(), point.class.to_string());
        published
            .by_drawing
            .entry(point.representation_uid.clone())
            .or_default()
            .push(point.uid.clone());
    }
    for representation in &drawing.representations {
        if let Some(item) = &representation.item_uid {
            if published.class_of.contains_key(item) {
                published
                    .by_drawing
                    .entry(representation.uid.clone())
                    .or_default()
                    .push(item.clone());
            }
        }
    }
    published
}

struct Reference {
    /// `(UID1, UID2)` -> `DefUID`, first one wins if duplicated.
    by_ends: HashMap<(String, String), &'static str>,
    counts: BTreeMap<&'static str, usize>,
    rows: Vec<(&'static str, String, String)>,
}

impl Reference {
    fn count(&self, def: &str) -> usize {
        self.counts.get(def).copied().unwrap_or(0)
    }

    fn find(&self, uid1: &str, uid2: &str) -> Option<&'static str> {
        self.by_ends
            .get(&(uid1.to_string(), uid2.to_string()))
            .copied()
    }

    fn eligible(&self, def: &str, published: &Published) -> usize {
        self.rows
            .iter()
            .filter(|(row_def, uid1, uid2)| {
                *row_def == def && published.contains(uid1) && published.contains(uid2)
            })
            .count()
    }
}

fn report_ceiling(reference: &Reference, published: &Published) {
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    for (def, uid1, uid2) in &reference.rows {
        if def == &"PipingConnectors" {
            continue;
        }
        let key = format!(
            "{def} {} -> {}",
            published.class_of(uid1),
            published.class_of(uid2)
        );
        *classes.entry(key).or_default() += 1;
    }
    for (key, count) in classes.iter().filter(|(_, count)| **count >= 5) {
        println!("  ceiling {key} {count}");
    }
}

fn report_row_fields(path: &Path) {
    let file = std::fs::File::open(path).expect("open");
    let mut compound = cfb::CompoundFile::open(file).expect("compound");
    let paths: Vec<String> = compound
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    let mut fields: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut samples = 0usize;
    for stream_path in paths {
        let Ok(mut stream) = compound.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err()
            || data.get(..4) != Some(&0x6C90_F544u32.to_le_bytes())
        {
            continue;
        }
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            if header.native_reader_skips() || header.type_code != 0x0089 {
                continue;
            }
            let Some(payload) = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .and_then(|end| data.get(header.body_start..end))
            else {
                continue;
            };
            let Some(row) = decode_attribute_row(payload) else {
                continue;
            };
            if row.class != "P&IDAttributes" {
                continue;
            }
            let kind = row.text("ModelItemType").unwrap_or("");
            if !matches!(kind, "Nozzle" | "ItemNote" | "Relationship") {
                continue;
            }
            let bucket = fields.entry(kind.to_string()).or_default();
            for field in &row.fields {
                *bucket.entry(field.name.clone()).or_default() += 1;
            }
            if kind == "Relationship" && samples < 2 {
                let text = row
                    .fields
                    .iter()
                    .map(|field| format!("{}={}", field.name, show(&field.value)))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("  relationship row {text}");
                samples += 1;
            }
        }
    }
    for (kind, names) in &fields {
        let text = names
            .iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  rows {kind} {text}");
    }
}

fn show(value: &pid_parse::parsers::attribute_rows::AttributeValue) -> String {
    let text = match value {
        pid_parse::parsers::attribute_rows::AttributeValue::Text(text) => text.clone(),
        pid_parse::parsers::attribute_rows::AttributeValue::Integer(number) => number.to_string(),
        pid_parse::parsers::attribute_rows::AttributeValue::Boolean(flag) => flag.to_string(),
    };
    if text.chars().count() > 48 {
        let mut short: String = text.chars().take(48).collect();
        short.push('…');
        short
    } else {
        text
    }
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

fn read_reference(path: &Path) -> Reference {
    let xml = std::fs::read_to_string(path).expect("reference");
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut rels: Vec<(String, String, String)> = Vec::new();
    let mut depth = 0usize;
    let mut tag = String::new();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                    values.clear();
                } else if depth == 2 {
                    take(&start, &mut values);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    take(&start, &mut values);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    finish(&tag, &mut values, &mut rels);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut reference = Reference {
        by_ends: HashMap::new(),
        counts: BTreeMap::new(),
        rows: Vec::new(),
    };
    for (def, uid1, uid2) in rels {
        let Some(def) = DEFS.iter().copied().find(|candidate| *candidate == def) else {
            continue;
        };
        *reference.counts.entry(def).or_default() += 1;
        reference.rows.push((def, uid1.clone(), uid2.clone()));
        reference.by_ends.entry((uid1, uid2)).or_insert(def);
    }
    reference
}

fn take(start: &BytesStart<'_>, values: &mut BTreeMap<String, String>) {
    let interface = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    for attribute in start.attributes().flatten() {
        let name = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute
            .unescape_value()
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_default();
        values.insert(format!("{interface}.{name}"), value);
    }
}

fn finish(
    tag: &str,
    values: &mut BTreeMap<String, String>,
    rels: &mut Vec<(String, String, String)>,
) {
    if tag != "Rel" {
        return;
    }
    let _uid = values.remove("IObject.UID");
    rels.push((
        values.remove("IRel.DefUID").unwrap_or_default(),
        values.remove("IRel.UID1").unwrap_or_default(),
        values.remove("IRel.UID2").unwrap_or_default(),
    ));
}
