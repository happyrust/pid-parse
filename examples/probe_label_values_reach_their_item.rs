//! Do the label values in a `.pid` reach the item they label?
//!
//! P1 of plan `docs/plans/2026-10-10-publish-from-the-pid-alone.md` found
//! 1 296 published attribute values in label caches (`ItemValue` fields and
//! `<F>name=value</F>` pairs) but could not say whose they are. Every
//! top-level `0x0089 FreeFormAttrSet` row holds one attribute set and names
//! the record it describes at payload `+12`. An item's own `P&IDAttributes`
//! row names its model item (`ModelID`); a label's (`DrawingItemType =
//! LabelPersist`) holds `ModelID = 0`; and the value rows (`_TagSequenceNo`,
//! `_SmartText`, ...) describe `igTextBox` records that no `P&IDAttributes`
//! row describes and that have no edge at all in the top-level space map.
//!
//! The text boxes do point somewhere in their payload. Read on one drawing
//! before this probe was written, and fixed before scoring:
//!
//! * an `igTextBox` (`0x004D`) names, at payload `+4`, the `DependencyObject`
//!   (`0x00FA`) it belongs to (the field the sheet decoder calls
//!   `parent_ref`);
//! * that `DependencyObject` names its two ends at `+16` and `+22`: the
//!   graphic the label hangs on, and an attribute row -- the labelled item's
//!   own `P&IDAttributes` row.
//!
//! So a value is attributed to the `ModelID` of the `P&IDAttributes` row at
//! `+22`, cross-checked with the item whose `P&IDAttributes` row describes
//! the graphic at `+16`. Both ends naming one item, or only one end naming
//! any, attributes the value; two items is "disagree"; a link that is not of
//! the expected family stops the chain ("broken", by link). A value is "right"
//! when the item's published block carries it as any attribute value,
//! "wrong" when the item is published without it, "unpublished" when the
//! item is not in the `_Data.xml`; blank values are not scored.
//!
//! The instrument split is tabulated as published class x the `SymbolAttr`
//! `Type` / `Class` on the item's graphic, without proposing a rule.
//!
//! ```powershell
//! cargo run --example probe_label_values_reach_their_item
//! ```
//!
//! `PID_PARSE_SELF_CHECK_DIR` points the probe at another directory; with
//! neither it nor the default directory present it prints `skip:` and exits.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
const TEXT_BOX: u16 = 0x004D;
const DEPENDENCY: u16 = 0x00FA;
/// Payload offset of the record an attribute row describes.
const ROW_TARGET_OFFSET: usize = 12;
/// Payload offset of the dependency an `igTextBox` belongs to.
const TEXT_PARENT_OFFSET: usize = 4;
/// Payload offsets of a `DependencyObject`'s two ends.
const DEPENDENCY_GRAPHIC_OFFSET: usize = 16;
const DEPENDENCY_ROW_OFFSET: usize = 22;
/// Label property -> the published attributes P1 saw carry its values (for
/// scoring only: a value that differs from one of these on its own item is
/// misattributed or reformatted, not merely unpublished).
const MAPPED: [(&str, &[&str]); 9] = [
    (
        "MeasuredVariableCode",
        &["INamedInstrument.MeasuredVariable"],
    ),
    (
        "TagSequenceNo",
        &[
            "INamedInstrument.InstrTagSequenceNo",
            "INamedPipingConnector.PipingConnectorSeqNo",
            "INamedEquipment.EquipSeqNo",
        ],
    ),
    (
        "PipingMaterialsClass",
        &["IPipingSpecifiedItem.PipingMaterialsClass"],
    ),
    ("InsulationSpec", &["IInsulatedItem.InsulSpec"]),
    ("Note.Body", &["INote.NoteText"]),
    ("TagPrefix", &["INamedEquipment.EquipPrefix"]),
    ("Description", &["IObject.Description"]),
    ("ToFromText", &["IOffpageConnector.ToFromText"]),
    (
        "NominalDiameter",
        &["IPipeCrossSectionItem.NominalDiameter"],
    ),
];
const INSTRUMENT_CLASSES: [&str; 3] = [
    "PIDInstrument",
    "PIDControlSystemFunction",
    "PIDInlineInstrument",
];

// ---------------------------------------------------------------- answer key

/// UID -> (class, every non-empty attribute value as `(interface.attribute, value)`).
type AnswerKey = HashMap<String, (String, Vec<(String, String)>)>;

type Block = Option<(String, Option<String>, Vec<(String, String)>)>;

fn take_interface(start: &BytesStart<'_>, block: &mut Block) {
    let Some((_, uid, values)) = block.as_mut() else {
        return;
    };
    let interface = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    for attribute in start.attributes().flatten() {
        let name = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute
            .unescape_value()
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_default();
        if interface == "IObject" && name == "UID" {
            *uid = Some(value);
        } else if !value.trim().is_empty() {
            values.push((format!("{interface}.{name}"), value));
        }
    }
}

fn read_answer_key(xml: &str) -> AnswerKey {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut key = AnswerKey::new();
    let mut depth = 0usize;
    let mut block: Block = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    let tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                    block = Some((tag, None, Vec::new()));
                } else if depth == 2 {
                    take_interface(&start, &mut block);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    take_interface(&start, &mut block);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some((class, Some(uid), values)) = block.take() {
                        if class != "Rel" {
                            key.insert(uid, (class, values));
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    key
}

// ---------------------------------------------------------------- the .pid

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// A NUL-terminated printable-ASCII name at `at`, and the offset past its NUL.
fn ascii_name(data: &[u8], at: usize, end: usize) -> Option<(String, usize)> {
    let slice = data.get(at..end)?;
    let nul = slice.iter().position(|&byte| byte == 0)?;
    if nul == 0
        || nul > 64
        || !slice[..nul]
            .iter()
            .all(|&byte| (0x20..0x7F).contains(&byte))
    {
        return None;
    }
    Some((
        String::from_utf8_lossy(&slice[..nul]).into_owned(),
        at + nul + 1,
    ))
}

fn text_of(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

/// `(name, value)` of a field framed at `at..end`, with the readings of
/// `probe_publish_from_the_pid_alone` (`0x0000` is a set header).
fn framed_field(data: &[u8], tag: u16, at: usize, end: usize) -> Option<(String, String)> {
    match tag {
        0x8010 => {
            let (name, value_at) = ascii_name(data, at + 5, end)?;
            let (last, body) = data.get(value_at..end)?.split_last()?;
            (*last == 0 && !body.contains(&0)).then(|| (name, text_of(body)))
        }
        0x0003 => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            let value = i32::from_le_bytes(data.get(value_at..end)?.try_into().ok()?);
            Some((name, value.to_string()))
        }
        0x000B => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            let value = u16_at(data, value_at)?;
            (value_at + 2 == end).then(|| (name, if value == 0 { "False" } else { "True" }.into()))
        }
        0x0000 => {
            if *data.get(at + 4)? != 0 {
                return None;
            }
            let (name, next) = ascii_name(data, at + 5, end)?;
            (next == end).then_some((name, String::new()))
        }
        _ => None,
    }
}

/// The one attribute set a row holds: its class and its fields.
fn one_set(payload: &[u8]) -> Option<(String, Vec<(String, String)>)> {
    let mut class = None;
    let mut fields = Vec::new();
    let mut at = 0;
    while at + 6 <= payload.len() {
        let tag = u16_at(payload, at).unwrap_or(u16::MAX);
        let end = at + usize::from(u16_at(payload, at + 2).unwrap_or(0));
        let field = if end >= at + 6 && end <= payload.len() {
            framed_field(payload, tag, at, end)
        } else {
            None
        };
        let Some((name, value)) = field else {
            at += 1;
            continue;
        };
        if tag == 0x0000 {
            class.get_or_insert(name);
        } else if class.is_some() {
            fields.push((name, value));
        }
        at = end;
    }
    Some((class?, fields))
}

struct Row {
    /// The record the row describes (payload `+12`).
    target: u32,
    class: String,
    fields: Vec<(String, String)>,
}

impl Row {
    fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The model UID of an item's own `P&IDAttributes` row (not a label's).
    fn item_model(&self) -> Option<&str> {
        if self.class != "P&IDAttributes" || self.field("DrawingItemType") == Some("LabelPersist") {
            return None;
        }
        self.field("ModelID").filter(|model| model.len() > 1)
    }
}

#[derive(Default)]
struct Records {
    /// oid -> type code of every live record of the root storage's chains.
    families: HashMap<u32, u16>,
    /// oid -> payload of every text box and dependency.
    payloads: HashMap<u32, Vec<u8>>,
    /// oid -> attribute row.
    rows: BTreeMap<u32, Row>,
}

impl Records {
    fn read(path: &Path) -> Self {
        let mut records = Self::default();
        let Ok(file) = std::fs::File::open(path) else {
            return records;
        };
        let Ok(mut cfb) = CompoundFile::open(file) else {
            return records;
        };
        let streams: Vec<PathBuf> = cfb
            .read_root_storage()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for stream_path in streams {
            let Ok(mut stream) = cfb.open_stream(&stream_path) else {
                continue;
            };
            let mut data = Vec::new();
            if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
                continue;
            }
            for at in sheet_record_starts(&data) {
                let Some(header) = parse_live_psm_header(&data, at) else {
                    continue;
                };
                let Some(end) = header
                    .body_start
                    .checked_add(header.bytes_to_follow as usize)
                else {
                    continue;
                };
                let Some(payload) = data.get(header.body_start..end) else {
                    continue;
                };
                let Some(oid) = u32_at(payload, 0) else {
                    continue;
                };
                records.families.insert(oid, header.type_code);
                match header.type_code {
                    FREE_FORM_ATTR_SET => {
                        if let (Some(target), Some((class, fields))) =
                            (u32_at(payload, ROW_TARGET_OFFSET), one_set(payload))
                        {
                            records.rows.insert(
                                oid,
                                Row {
                                    target,
                                    class,
                                    fields,
                                },
                            );
                        }
                    }
                    TEXT_BOX | DEPENDENCY => {
                        records.payloads.insert(oid, payload.to_vec());
                    }
                    _ => {}
                }
            }
        }
        records
    }

    fn link(&self, oid: u32, family: u16, offset: usize) -> Option<u32> {
        if self.families.get(&oid) != Some(&family) {
            return None;
        }
        u32_at(self.payloads.get(&oid)?, offset)
    }
}

/// `<F>name=value</F>` pairs of a label format string.
fn format_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("<F>") {
        let after = &rest[open + 3..];
        let Some(close) = after.find("</F>") else {
            break;
        };
        if let Some((name, value)) = after[..close].split_once('=') {
            pairs.push((name.to_string(), value.to_string()));
        }
        rest = &after[close + 4..];
    }
    pairs
}

/// The `(property, value)` pairs a value row caches.
fn row_values(row: &Row) -> Vec<(String, String)> {
    if row.class == "_SmartText" {
        row.field("FormatString")
            .map(format_pairs)
            .unwrap_or_default()
    } else if row.class.starts_with('_') {
        match (row.field("ItemProperty"), row.field("ItemValue")) {
            (Some(property), Some(value)) => vec![(property.to_string(), value.to_string())],
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

// ---------------------------------------------------------------- the chain

enum Chain {
    /// The labelled item's model UID and which ends named it.
    Item(String, &'static str),
    Disagree,
    /// The link the chain stopped at.
    Broken(&'static str),
}

fn follow(row: &Row, records: &Records, graphic_items: &HashMap<u32, String>) -> Chain {
    let Some(dependency) = records.link(row.target, TEXT_BOX, TEXT_PARENT_OFFSET) else {
        return Chain::Broken("the row's record is not an igTextBox");
    };
    if records.families.get(&dependency) != Some(&DEPENDENCY) {
        return Chain::Broken("the text box's +4 is not a DependencyObject");
    }
    let by_row = records
        .link(dependency, DEPENDENCY, DEPENDENCY_ROW_OFFSET)
        .and_then(|oid| records.rows.get(&oid))
        .and_then(Row::item_model);
    let by_graphic = records
        .link(dependency, DEPENDENCY, DEPENDENCY_GRAPHIC_OFFSET)
        .and_then(|oid| graphic_items.get(&oid))
        .map(String::as_str);
    match (by_row, by_graphic) {
        (Some(row_model), Some(graphic_model)) if row_model == graphic_model => {
            Chain::Item(row_model.to_string(), "+22 and +16 agree")
        }
        (Some(_), Some(_)) => Chain::Disagree,
        (Some(row_model), None) => Chain::Item(row_model.to_string(), "+22 only"),
        (None, Some(graphic_model)) => Chain::Item(graphic_model.to_string(), "+16 only"),
        (None, None) => Chain::Broken("neither end of the dependency is an item"),
    }
}

// ---------------------------------------------------------------- scoring

#[derive(Default, Clone, Copy)]
struct Tally {
    values: usize,
    reached: usize,
    disagree: usize,
    broken: usize,
    right: usize,
    wrong: usize,
    unpublished: usize,
}

impl Tally {
    fn add(&mut self, other: &Self) {
        self.values += other.values;
        self.reached += other.reached;
        self.disagree += other.disagree;
        self.broken += other.broken;
        self.right += other.right;
        self.wrong += other.wrong;
        self.unpublished += other.unpublished;
    }

    fn line(&self) -> String {
        format!(
            "{:>6} {:>7} {:>8} {:>6} {:>6} {:>6} {:>6}",
            self.values,
            self.reached,
            self.disagree,
            self.broken,
            self.right,
            self.wrong,
            self.unpublished
        )
    }
}

const TALLY_HEADER: &str = "values reached disagree broken  right  wrong  unpub";

#[derive(Default)]
struct Totals {
    by_property: BTreeMap<String, Tally>,
    /// (ends that named the item, the item's `ModelItemType`) -> (values, right).
    ways: BTreeMap<(String, String), (usize, usize)>,
    /// broken link -> values.
    broken: BTreeMap<String, usize>,
    /// mapped label property -> (equal to the item's attribute, different, the item has none of them).
    mapped: BTreeMap<String, (usize, usize, usize)>,
    /// `interface.attribute` -> (published non-empty values, given by an attributed label value).
    recall: BTreeMap<String, (usize, usize)>,
    /// published class -> (objects, objects whose every non-empty value a label gives).
    objects: BTreeMap<String, (usize, usize)>,
    /// (published class, `SymbolAttr` Type, `SymbolAttr` Class) -> objects.
    split: BTreeMap<(String, String, String), usize>,
}

fn score_drawing(stem: &str, pid: &Path, data_xml: &Path, totals: &mut Totals) {
    println!("== {stem}");
    let Ok(xml) = std::fs::read_to_string(data_xml) else {
        println!("  skip: {} is not readable UTF-8", data_xml.display());
        return;
    };
    let key = read_answer_key(&xml);
    let records = Records::read(pid);

    // Item graphics: records an item's own `P&IDAttributes` row describes.
    let mut graphic_items: HashMap<u32, String> = HashMap::new();
    let mut model_types: HashMap<String, String> = HashMap::new();
    let mut graphics_of: HashMap<String, Vec<u32>> = HashMap::new();
    let mut symbol_attr: HashMap<u32, (String, String)> = HashMap::new();
    for row in records.rows.values() {
        if let Some(model) = row.item_model() {
            graphic_items.insert(row.target, model.to_string());
            graphics_of
                .entry(model.to_string())
                .or_default()
                .push(row.target);
            model_types.insert(
                model.to_string(),
                row.field("ModelItemType").unwrap_or("?").to_string(),
            );
        }
        if row.class == "SymbolAttr" {
            symbol_attr.insert(
                row.target,
                (
                    row.field("Type").unwrap_or("").to_string(),
                    row.field("Class").unwrap_or("").to_string(),
                ),
            );
        }
    }

    let mut drawing = Tally::default();
    let mut given: HashSet<(String, String)> = HashSet::new();
    let mut value_rows = 0;
    for row in records.rows.values() {
        let values: Vec<(String, String)> = row_values(row)
            .into_iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .collect();
        if values.is_empty() {
            continue;
        }
        value_rows += 1;
        let chain = follow(row, &records, &graphic_items);
        for (property, value) in values {
            let mut one = Tally {
                values: 1,
                ..Tally::default()
            };
            match &chain {
                Chain::Broken(link) => {
                    one.broken = 1;
                    *totals.broken.entry((*link).to_string()).or_insert(0) += 1;
                }
                Chain::Disagree => one.disagree = 1,
                Chain::Item(model, ends) => {
                    one.reached = 1;
                    let right = match key.get(model) {
                        None => {
                            one.unpublished = 1;
                            false
                        }
                        Some((_, published)) => {
                            if let Some((_, attributes)) =
                                MAPPED.iter().find(|(name, _)| *name == property)
                            {
                                let on_item: Vec<&String> = published
                                    .iter()
                                    .filter(|(a, _)| attributes.contains(&a.as_str()))
                                    .map(|(_, v)| v)
                                    .collect();
                                let cell =
                                    totals.mapped.entry(property.clone()).or_insert((0, 0, 0));
                                if on_item.is_empty() {
                                    cell.2 += 1;
                                } else if on_item.iter().any(|v| **v == value) {
                                    cell.0 += 1;
                                } else {
                                    cell.1 += 1;
                                }
                            }
                            let right = published.iter().any(|(_, v)| *v == value);
                            if right {
                                one.right = 1;
                                given.insert((model.clone(), value.clone()));
                            } else {
                                one.wrong = 1;
                            }
                            right
                        }
                    };
                    let kind = model_types.get(model).cloned().unwrap_or_default();
                    let way = totals
                        .ways
                        .entry(((*ends).to_string(), kind))
                        .or_insert((0, 0));
                    way.0 += 1;
                    way.1 += usize::from(right);
                }
            }
            drawing.add(&one);
            totals.by_property.entry(property).or_default().add(&one);
        }
    }
    println!(
        "  {} attribute rows, {} item graphics, {} value rows; values {}: reached {} (right {}, wrong {}, unpublished {}), disagree {}, broken {}",
        records.rows.len(),
        graphic_items.len(),
        value_rows,
        drawing.values,
        drawing.reached,
        drawing.right,
        drawing.wrong,
        drawing.unpublished,
        drawing.disagree,
        drawing.broken
    );

    for (uid, (class, published)) in &key {
        let mut complete = true;
        for (attribute, value) in published {
            if attribute == "IDrawingRepresentation.GraphicOID" {
                continue;
            }
            let is_given = given.contains(&(uid.clone(), value.clone()));
            complete &= is_given;
            let row = totals.recall.entry(attribute.clone()).or_insert((0, 0));
            row.0 += 1;
            row.1 += usize::from(is_given);
        }
        let objects = totals.objects.entry(class.clone()).or_insert((0, 0));
        objects.0 += 1;
        objects.1 += usize::from(complete);
        if INSTRUMENT_CLASSES.contains(&class.as_str()) {
            let attr = graphics_of
                .get(uid)
                .into_iter()
                .flatten()
                .find_map(|graphic| symbol_attr.get(graphic))
                .cloned()
                .unwrap_or_else(|| ("(no SymbolAttr)".to_string(), String::new()));
            *totals
                .split
                .entry((class.clone(), attr.0, attr.1))
                .or_insert(0) += 1;
        }
    }
}

fn print_totals(totals: &Totals, drawings: usize) {
    println!("== all {drawings} drawings");
    let mut all = Tally::default();
    for tally in totals.by_property.values() {
        all.add(tally);
    }
    println!("  {:<32} {TALLY_HEADER}", "label property");
    for (property, tally) in &totals.by_property {
        println!("  {property:<32} {}", tally.line());
    }
    println!("  {:<32} {}", "(all)", all.line());

    println!("  attributed values by the ends that named the item (values, right)");
    for ((ends, kind), (values, right)) in &totals.ways {
        println!("    {values:>6} {right:>6}  {ends:<20} {kind}");
    }
    println!("  mapped label properties on the item reached (equal, different, item has none)");
    for (property, (equal, different, none)) in &totals.mapped {
        println!("    {property:<32} {equal:>6} {different:>6} {none:>6}");
    }
    println!("  broken chains (values)");
    for (link, values) in &totals.broken {
        println!("    {values:>6}  {link}");
    }

    println!("  published values a label gives (attributes with any)");
    let mut recall: Vec<(&String, &(usize, usize))> = totals
        .recall
        .iter()
        .filter(|(_, (_, given))| *given > 0)
        .collect();
    recall.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0.cmp(b.0)));
    let (values, given) = totals
        .recall
        .values()
        .fold((0, 0), |(v, g), (values, given)| (v + values, g + given));
    for (attribute, (values, given)) in recall {
        println!("    {attribute:<52} {given:>6} / {values:<6}");
    }
    println!(
        "    {:<52} {given:>6} / {values:<6}",
        "(all non-empty published values, GraphicOID apart)"
    );

    println!("  instruments: published class x SymbolAttr Type / Class");
    for ((class, kind, attr_class), count) in &totals.split {
        println!("    {count:>5}  {class:<26} {kind:<32} {attr_class}");
    }
}

fn drawings(corpus: &Path) -> Vec<(String, PathBuf, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(corpus) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    out.sort();
    out
}

fn main() {
    let corpus =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let drawings = drawings(&corpus);
    if drawings.is_empty() {
        println!(
            "skip: no <stem>.pid beside a <stem>_Data.xml in {} (set {CORPUS_ENV})",
            corpus.display()
        );
        return;
    }
    println!("corpus: {} ({} drawings)", corpus.display(), drawings.len());
    let mut totals = Totals::default();
    for (stem, pid, data_xml) in &drawings {
        score_drawing(stem, pid, data_xml, &mut totals);
    }
    print_totals(&totals, drawings.len());
}
