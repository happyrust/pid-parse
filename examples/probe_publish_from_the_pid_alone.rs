//! How much of a SmartPlant `_Data.xml` can the `.pid` alone give back?
//!
//! The test cases for the publish export are the drawings in
//! `D:\work\cad\PID自研软件验证`: each a `.pid` beside the `_Data.xml` /
//! `_Meta.xml` SmartPlant published from it. Of the five, only `I井场`'s
//! `DocUID` is in the plant backup next to them (`D:\work\cad\pid-test-data`,
//! SQPlant, an Oracle `exp` dump whose rows this crate does not decode yet),
//! so the question that decides the route comes first: with no plant
//! database at all, how much of each reference document is in the `.pid`?
//!
//! The answer key is each `_Data.xml`: every top-level block under
//! `<Container>` is a published object (its element name the class, its
//! `IObject/@UID` the key, every other interface attribute a value) or a
//! `<Rel>` (`IObject/@UID`, `IRel/@UID1`, `@UID2`, `@DefUID`). The `.pid` is
//! read four ways, fixed before any scoring:
//!
//! 1. **Raw** -- the value's bytes (UTF-8, UTF-16LE or GBK) occur in some
//!    stream of the compound file. An upper bound: it says nothing about
//!    which object the bytes belong to. Values shorter than four bytes are
//!    not scanned (they occur by chance) and are counted apart.
//! 2. **Fields** -- every attribute-set field the file frames, in every
//!    stream: `u16 tag, u16 length` (the four header bytes included), then
//!    for tag `0x8010` a kind byte, a NUL-terminated ASCII name and a value
//!    whose only NUL ends the field; for tag `0x0003` a NUL-terminated name
//!    and an `i32`; for tag `0x000B` a NUL-terminated name and a two-byte
//!    boolean; tag `0x0000` with a zero byte and a NUL-terminated name opens
//!    a set (`P&IDAttributes`, `SymbolAttr`, label sets, ...), and the fields
//!    that follow it byte for byte belong to it -- inside such a run any
//!    other tag counts too when a NUL-terminated name opens it. A field
//!    counts only when its length word equals the bytes it frames. A published value
//!    "is in a label cache" when it equals an `ItemValue` field (its set's
//!    `ItemProperty` names the property) or the value of a `<F>name=value</F>`
//!    pair inside any field (the format strings of label smart text), at any
//!    length; it "is in the fields" when it is at least four bytes long and
//!    equals the value of some `0x8010` field.
//! 3. **Named** -- an object is named when a `P&IDAttributes` field
//!    `DrawingNo`, `DrawingID` or `ModelID` holds its UID; the set's
//!    `ModelItemType` / `DrawingItemType` is then shown beside the published
//!    class. A `<Rel>` has both ends named when both UIDs are.
//! 4. **GraphicOID** -- `PidSemanticIndex::load_beside` (the UID join of
//!    2026-10-08 D2): of the published representations, how many the file
//!    joins to a record, and how many of those records are the published
//!    `GraphicOID`. `GraphicOID` is left out of the attribute counts.
//!
//! The `_Meta.xml` beside each `_Data.xml` -- the `Container` header, three
//! document objects and three `<Rel>`s -- is too small to tally, so each of
//! its values is located item by item with the same readings. The attribute
//! totals are listed once by `interface.attribute` and again per published
//! class, the unit an exporter is written in.
//!
//! ```powershell
//! cargo run --example probe_publish_from_the_pid_alone
//! ```
//!
//! `PID_PARSE_SELF_CHECK_DIR` points the probe at another directory; with
//! neither it nor the default directory present it prints `skip:` and exits.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;
use pid_parse::parsers::undecoded_census::rad_class_name;
use pid_parse::{PidParser, PidSemanticIndex};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const STRING_FIELD: u16 = 0x8010;
const INT_FIELD: u16 = 0x0003;
const BOOL_FIELD: u16 = 0x000B;
const SET_HEADER: u16 = 0x0000;
/// `P&IDAttributes` fields whose value is the UID of the object they name.
const UID_FIELDS: [&str; 3] = ["DrawingNo", "DrawingID", "ModelID"];
/// Shorter values are not scanned for raw bytes.
const RAW_MIN_BYTES: usize = 4;
/// Attribute keys listed in the totals need at least this many non-empty
/// values over the corpus.
const LIST_MIN_VALUES: usize = 5;

// ---------------------------------------------------------------- answer key

struct PublishedObject {
    class: String,
    uid: String,
    /// `(interface, attribute, value)` for every attribute but `IObject/@UID`.
    attributes: Vec<(String, String, String)>,
}

struct PublishedRel {
    uid: String,
    uid1: String,
    uid2: String,
    def_uid: String,
}

type Interface = (String, Vec<(String, String)>);

fn unescaped(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    quick_xml::escape::unescape(&text)
        .map_or_else(|_| text.to_string(), std::borrow::Cow::into_owned)
}

fn interface_of(start: &BytesStart<'_>) -> Interface {
    let attributes = start
        .attributes()
        .flatten()
        .map(|attribute| {
            (
                String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
                unescaped(&attribute.value),
            )
        })
        .collect();
    (
        String::from_utf8_lossy(start.name().as_ref()).into_owned(),
        attributes,
    )
}

fn read_answer_key(xml: &str) -> (Vec<PublishedObject>, Vec<PublishedRel>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut objects = Vec::new();
    let mut rels = Vec::new();
    let mut depth = 0usize;
    let mut block: Option<(String, Vec<Interface>)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    let tag = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                    block = Some((tag, Vec::new()));
                } else if depth == 2 {
                    if let Some((_, interfaces)) = block.as_mut() {
                        interfaces.push(interface_of(&start));
                    }
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    if let Some((_, interfaces)) = block.as_mut() {
                        interfaces.push(interface_of(&start));
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    if let Some((tag, interfaces)) = block.take() {
                        finish_block(tag, &interfaces, &mut objects, &mut rels);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (objects, rels)
}

/// The root element's attributes: `<Container DocUID=… DocName=… …>`.
fn container_attributes(xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(start) | Event::Empty(start)) => return interface_of(&start).1,
            Ok(Event::Eof) | Err(_) => return Vec::new(),
            _ => {}
        }
    }
}

fn finish_block(
    tag: String,
    interfaces: &[Interface],
    objects: &mut Vec<PublishedObject>,
    rels: &mut Vec<PublishedRel>,
) {
    let find = |interface: &str, name: &str| {
        interfaces
            .iter()
            .filter(|(element, _)| element == interface)
            .flat_map(|(_, attributes)| attributes.iter())
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let Some(uid) = find("IObject", "UID") else {
        return;
    };
    if tag == "Rel" {
        if let (Some(uid1), Some(uid2), Some(def_uid)) = (
            find("IRel", "UID1"),
            find("IRel", "UID2"),
            find("IRel", "DefUID"),
        ) {
            rels.push(PublishedRel {
                uid,
                uid1,
                uid2,
                def_uid,
            });
        }
        return;
    }
    let mut attributes = Vec::new();
    for (interface, pairs) in interfaces {
        for (key, value) in pairs {
            if interface == "IObject" && key == "UID" {
                continue;
            }
            attributes.push((interface.clone(), key.clone(), value.clone()));
        }
    }
    objects.push(PublishedObject {
        class: tag,
        uid,
        attributes,
    });
}

// ---------------------------------------------------------------- the .pid

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    let mut out = Vec::new();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push((stream_path, data));
        }
    }
    out
}

/// One set of framed fields: the header's class name (`(no header)` for
/// fields that follow none) and its `0x8010` / `0x0003` fields in order.
struct AttributeSet {
    at: usize,
    class: String,
    fields: Vec<(u16, String, String)>,
}

impl AttributeSet {
    fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(_, key, _)| key == name)
            .map(|(_, _, value)| value.as_str())
    }
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

/// The `(name, value)` a field framed at `at..end` carries, if it is one.
/// Inside a run of fields (`in_run`) a tag none of the arms knows is still
/// a field when a NUL-terminated name opens it; its value is kept as hex.
fn framed_field(
    data: &[u8],
    tag: u16,
    at: usize,
    end: usize,
    in_run: bool,
) -> Option<(String, String)> {
    match tag {
        STRING_FIELD => {
            let (name, value_at) = ascii_name(data, at + 5, end)?;
            let (last, body) = data.get(value_at..end)?.split_last()?;
            (*last == 0 && !body.contains(&0)).then(|| (name, text_of(body)))
        }
        INT_FIELD => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            let value = i32::from_le_bytes(data.get(value_at..end)?.try_into().ok()?);
            Some((name, value.to_string()))
        }
        BOOL_FIELD => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            let value = u16_at(data, value_at)?;
            (value_at + 2 == end).then(|| (name, if value == 0 { "False" } else { "True" }.into()))
        }
        SET_HEADER => {
            if *data.get(at + 4)? != 0 {
                return None;
            }
            let (name, next) = ascii_name(data, at + 5, end)?;
            (next == end).then_some((name, String::new()))
        }
        _ if in_run => {
            let (name, value_at) = ascii_name(data, at + 4, end)?;
            let hex: Vec<String> = data
                .get(value_at..end)?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            Some((name, hex.concat()))
        }
        _ => None,
    }
}

fn scan_sets(data: &[u8]) -> Vec<AttributeSet> {
    let mut sets: Vec<AttributeSet> = Vec::new();
    let mut previous_end = None;
    let mut at = 0;
    while at + 6 <= data.len() {
        let tag = u16_at(data, at).unwrap_or(u16::MAX);
        let end = at + usize::from(u16_at(data, at + 2).unwrap_or(0));
        let in_run = previous_end == Some(at);
        let field = if end >= at + 6 && end <= data.len() {
            framed_field(data, tag, at, end, in_run)
        } else {
            None
        };
        let Some((name, value)) = field else {
            at += 1;
            continue;
        };
        if tag == SET_HEADER {
            sets.push(AttributeSet {
                at,
                class: name,
                fields: Vec::new(),
            });
        } else {
            if !in_run || sets.is_empty() {
                sets.push(AttributeSet {
                    at,
                    class: "(no header)".to_string(),
                    fields: Vec::new(),
                });
            }
            if let Some(set) = sets.last_mut() {
                set.fields.push((tag, name, value));
            }
        }
        previous_end = Some(end);
        at = end;
    }
    sets
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

/// The record family a stream offset falls in: `0xNNNN name` inside a
/// record chain, the stream's leaf name otherwise.
fn family_at(stream: &str, data: &[u8], starts: &[usize], at: usize) -> String {
    let index = starts.partition_point(|&start| start <= at);
    if index == 0 {
        return format!("[{}]", stream.rsplit('/').next().unwrap_or(stream));
    }
    let start = starts[index - 1];
    let code = u16_at(data, start).unwrap_or(0) & 0x3FFF;
    format!(
        "0x{code:04X} {}",
        rad_class_name(code).unwrap_or("(unnamed)")
    )
}

/// Every stream's bytes end to end, each followed by four `0xFF` so no
/// needle matches across a stream boundary, with the offsets of every
/// 4-byte prefix: a needle of at least four bytes is looked up by its
/// first four instead of being slid over megabytes.
struct RawIndex {
    bytes: Vec<u8>,
    by_prefix: HashMap<[u8; 4], Vec<u32>>,
}

impl RawIndex {
    fn new(streams: &[(String, Vec<u8>)]) -> Self {
        let mut bytes = Vec::new();
        for (_, data) in streams {
            bytes.extend_from_slice(data);
            bytes.extend_from_slice(&[0xFF; 4]);
        }
        let mut by_prefix: HashMap<[u8; 4], Vec<u32>> = HashMap::new();
        for (at, window) in bytes.windows(4).enumerate() {
            let key = [window[0], window[1], window[2], window[3]];
            by_prefix
                .entry(key)
                .or_default()
                .push(u32::try_from(at).unwrap_or(u32::MAX));
        }
        Self { bytes, by_prefix }
    }

    fn contains(&self, needle: &[u8]) -> bool {
        let Some(key) = needle.get(..4) else {
            return false;
        };
        let key = [key[0], key[1], key[2], key[3]];
        self.by_prefix.get(&key).is_some_and(|offsets| {
            offsets.iter().any(|&at| {
                let at = at as usize;
                self.bytes.get(at..at + needle.len()) == Some(needle)
            })
        })
    }
}

struct PidReading {
    streams: Vec<(String, Vec<u8>)>,
    raw_index: RawIndex,
    raw_seen: RefCell<HashMap<String, bool>>,
    sets: Vec<AttributeSet>,
    /// UID -> (`P&IDAttributes` field naming it, `ModelItemType` or
    /// `DrawingItemType` of that set).
    named: BTreeMap<String, (String, String)>,
    /// `0x8010` field values of at least [`RAW_MIN_BYTES`].
    values: BTreeSet<String>,
    /// Label-cache value -> the properties it is cached for.
    labels: BTreeMap<String, BTreeSet<String>>,
    /// `(storage, family, set class)` -> sets.
    census: BTreeMap<(String, String, String), usize>,
}

fn read_pid(path: &Path) -> PidReading {
    let streams = read_streams(path);
    let mut sets = Vec::new();
    let mut census = BTreeMap::new();
    for (stream, data) in &streams {
        let starts = if u32_at(data, 0) == Some(CHAIN_MAGIC) {
            sheet_record_starts(data)
        } else {
            Vec::new()
        };
        let storage = match stream.rfind('/') {
            Some(0) | None => "/".to_string(),
            Some(at) => stream[..=at].to_string(),
        };
        for set in scan_sets(data) {
            let family = family_at(stream, data, &starts, set.at);
            *census
                .entry((storage.clone(), family, set.class.clone()))
                .or_insert(0) += 1;
            sets.push(set);
        }
    }
    let mut named = BTreeMap::new();
    let mut values = BTreeSet::new();
    let mut labels: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for set in &sets {
        if set.class == "P&IDAttributes" {
            let kind = set
                .field("ModelItemType")
                .filter(|kind| !kind.is_empty())
                .or(set.field("DrawingItemType"))
                .unwrap_or("?");
            for field in UID_FIELDS {
                if let Some(uid) = set.field(field).filter(|uid| !uid.is_empty()) {
                    let kind = if field == "DrawingNo" {
                        "Drawing"
                    } else {
                        kind
                    };
                    named
                        .entry(uid.to_string())
                        .or_insert_with(|| (field.to_string(), kind.to_string()));
                }
            }
        }
        let property = set.field("ItemProperty").unwrap_or("?");
        for (tag, name, value) in &set.fields {
            if *tag != STRING_FIELD {
                continue;
            }
            if value.len() >= RAW_MIN_BYTES {
                values.insert(value.clone());
            }
            if name == "ItemValue" && !value.is_empty() {
                labels
                    .entry(value.clone())
                    .or_default()
                    .insert(property.to_string());
            }
            for (key, pair_value) in format_pairs(value) {
                if !pair_value.is_empty() {
                    labels
                        .entry(pair_value)
                        .or_default()
                        .insert(format!("<F>{key}"));
                }
            }
        }
    }
    let raw_index = RawIndex::new(&streams);
    PidReading {
        streams,
        raw_index,
        raw_seen: RefCell::new(HashMap::new()),
        sets,
        named,
        values,
        labels,
        census,
    }
}

impl PidReading {
    /// Whether `value` (at least [`RAW_MIN_BYTES`] long in UTF-8) occurs in
    /// some stream as UTF-8, UTF-16LE or GBK.
    fn raw(&self, value: &str) -> bool {
        if let Some(&seen) = self.raw_seen.borrow().get(value) {
            return seen;
        }
        let utf16: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let gbk = encoding_rs::GBK.encode(value).0.into_owned();
        let found = [value.as_bytes(), utf16.as_slice(), gbk.as_slice()]
            .iter()
            .any(|needle| self.raw_index.contains(needle));
        self.raw_seen.borrow_mut().insert(value.to_string(), found);
        found
    }

    fn named_kind(&self, uid: &str) -> Option<&(String, String)> {
        self.named.get(uid)
    }

    /// The strongest reading that finds `value`: `named (<field>)`, `label`,
    /// `field`, `raw`; `short` for an unfound value under [`RAW_MIN_BYTES`],
    /// `-` for none. Short values are never looked up as names: some
    /// `ModelID` fields hold `0`.
    fn locate(&self, value: &str) -> String {
        if value.len() >= RAW_MIN_BYTES {
            if let Some((field, _)) = self.named.get(value) {
                return format!("named ({field})");
            }
        }
        if self.labels.contains_key(value) {
            return "label".to_string();
        }
        if value.len() < RAW_MIN_BYTES {
            return "short".to_string();
        }
        if self.values.contains(value) {
            return "field".to_string();
        }
        if self.raw(value) {
            return "raw".to_string();
        }
        "-".to_string()
    }
}

// ---------------------------------------------------------------- scoring

#[derive(Default, Clone, Copy)]
struct ObjectTally {
    objects: usize,
    named: usize,
    raw: usize,
}

#[derive(Default)]
struct AttributeTally {
    values: usize,
    empty: usize,
    in_labels: usize,
    in_fields: usize,
    short: usize,
    raw: usize,
    /// Label-cache property -> published values it carried.
    carried_by: BTreeMap<String, usize>,
}

impl AttributeTally {
    fn add(&mut self, other: &Self) {
        self.values += other.values;
        self.empty += other.empty;
        self.in_labels += other.in_labels;
        self.in_fields += other.in_fields;
        self.short += other.short;
        self.raw += other.raw;
    }
}

#[derive(Default, Clone, Copy)]
struct RelTally {
    rels: usize,
    both_named: usize,
    one_named: usize,
    own_raw: usize,
}

#[derive(Default)]
struct Totals {
    by_class: BTreeMap<String, ObjectTally>,
    kinds: BTreeMap<String, BTreeMap<String, usize>>,
    by_attribute: BTreeMap<String, AttributeTally>,
    /// `(class, interface.attribute)` -> tally.
    by_class_attribute: BTreeMap<(String, String), AttributeTally>,
    by_def: BTreeMap<String, RelTally>,
    census: BTreeMap<(String, String, String), usize>,
    representations: usize,
    uid_joined: usize,
    on_graphic_oid: usize,
}

fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        "-".to_string()
    } else {
        format!("{:.1} %", 100.0 * part as f64 / whole as f64)
    }
}

fn describe_attributes(tally: &AttributeTally) -> String {
    let non_empty = tally.values - tally.empty;
    let scanned = non_empty - tally.short;
    format!(
        "{} (GraphicOID apart): empty {}, non-empty {}: in a label cache {} ({}); \
         of the {scanned} at least {RAW_MIN_BYTES} bytes long, in the fields {} ({}), raw {} ({})",
        tally.values,
        tally.empty,
        non_empty,
        tally.in_labels,
        percent(tally.in_labels, non_empty),
        tally.in_fields,
        percent(tally.in_fields, scanned),
        tally.raw,
        percent(tally.raw, scanned)
    )
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

fn score_drawing(stem: &str, pid: &Path, data_xml: &Path, totals: &mut Totals) {
    println!("== {stem}");
    let Ok(xml) = std::fs::read_to_string(data_xml) else {
        println!("  skip: {} is not readable UTF-8", data_xml.display());
        return;
    };
    let (objects, rels) = read_answer_key(&xml);
    let reading = read_pid(pid);
    let size = std::fs::metadata(pid).map_or(0, |meta| meta.len());
    let header_sets = reading
        .sets
        .iter()
        .filter(|set| set.class != "(no header)")
        .count();
    let field_count: usize = reading.sets.iter().map(|set| set.fields.len()).sum();
    println!(
        "  .pid {size} B, {} streams; {field_count} framed fields in {} sets ({header_sets} with a header); {} UIDs named",
        reading.streams.len(),
        reading.sets.len(),
        reading.named.len()
    );
    for (key, count) in &reading.census {
        *totals.census.entry(key.clone()).or_insert(0) += count;
    }

    let mut by_class: BTreeMap<String, ObjectTally> = BTreeMap::new();
    let mut kinds: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut named_by_field: BTreeMap<String, usize> = BTreeMap::new();
    let mut attributes = AttributeTally::default();
    for object in &objects {
        let tally = by_class.entry(object.class.clone()).or_default();
        tally.objects += 1;
        if let Some((field, kind)) = reading.named_kind(&object.uid) {
            tally.named += 1;
            *named_by_field.entry(field.clone()).or_insert(0) += 1;
            *kinds
                .entry(object.class.clone())
                .or_default()
                .entry(kind.clone())
                .or_insert(0) += 1;
        }
        if reading.raw(&object.uid) {
            tally.raw += 1;
        }
        for (interface, key, value) in &object.attributes {
            if interface == "IDrawingRepresentation" && key == "GraphicOID" {
                continue;
            }
            let mut one = AttributeTally {
                values: 1,
                ..AttributeTally::default()
            };
            let row = totals
                .by_attribute
                .entry(format!("{interface}.{key}"))
                .or_default();
            if value.is_empty() {
                one.empty = 1;
            } else {
                if let Some(properties) = reading.labels.get(value) {
                    one.in_labels = 1;
                    for property in properties {
                        *row.carried_by.entry(property.clone()).or_insert(0) += 1;
                    }
                }
                if value.len() < RAW_MIN_BYTES {
                    one.short = 1;
                } else {
                    one.in_fields = usize::from(reading.values.contains(value));
                    one.raw = usize::from(reading.raw(value));
                }
            }
            row.add(&one);
            attributes.add(&one);
            totals
                .by_class_attribute
                .entry((object.class.clone(), format!("{interface}.{key}")))
                .or_default()
                .add(&one);
        }
    }

    let mut by_def: BTreeMap<String, RelTally> = BTreeMap::new();
    for rel in &rels {
        let tally = by_def.entry(rel.def_uid.clone()).or_default();
        tally.rels += 1;
        let ends = [&rel.uid1, &rel.uid2]
            .iter()
            .filter(|uid| reading.named.contains_key(uid.as_str()))
            .count();
        match ends {
            2 => tally.both_named += 1,
            1 => tally.one_named += 1,
            _ => {}
        }
        if reading.raw(&rel.uid) {
            tally.own_raw += 1;
        }
    }

    let (representations, uid_joined, stale) = match PidParser::new().parse_file(pid) {
        Ok(doc) => PidSemanticIndex::load_beside(pid, &doc).map_or((0, 0, 0), |index| {
            (index.len(), index.uid_joined(), index.stale_graphic_oids())
        }),
        Err(error) => {
            println!("  the .pid does not parse ({error}); GraphicOID not measured");
            (0, 0, 0)
        }
    };
    totals.representations += representations;
    totals.uid_joined += uid_joined;
    totals.on_graphic_oid += uid_joined - stale;

    let object_sum = by_class
        .values()
        .fold(ObjectTally::default(), |sum, t| ObjectTally {
            objects: sum.objects + t.objects,
            named: sum.named + t.named,
            raw: sum.raw + t.raw,
        });
    let rel_sum = by_def
        .values()
        .fold(RelTally::default(), |sum, t| RelTally {
            rels: sum.rels + t.rels,
            both_named: sum.both_named + t.both_named,
            one_named: sum.one_named + t.one_named,
            own_raw: sum.own_raw + t.own_raw,
        });
    let fields: Vec<String> = named_by_field
        .iter()
        .map(|(field, count)| format!("{field} {count}"))
        .collect();
    println!(
        "  objects {}: named {} ({}; {}), UID raw {}",
        object_sum.objects,
        object_sum.named,
        percent(object_sum.named, object_sum.objects),
        fields.join(" / "),
        object_sum.raw
    );
    println!("  attribute values {}", describe_attributes(&attributes));
    println!(
        "  rels {}: both ends named {} ({}), one end {}, own UID raw {}",
        rel_sum.rels,
        rel_sum.both_named,
        percent(rel_sum.both_named, rel_sum.rels),
        rel_sum.one_named,
        rel_sum.own_raw
    );
    println!(
        "  representations {representations}: UID-joined {uid_joined}, on their published GraphicOID {}",
        uid_joined - stale
    );
    score_meta(
        &data_xml.with_file_name(format!("{stem}_Meta.xml")),
        &reading,
    );
    println!(
        "  {:<28} {:>7} {:>6} {:>6}  named as",
        "class", "objects", "named", "raw"
    );
    for (class, tally) in &by_class {
        let seen: Vec<String> = kinds
            .get(class)
            .map(|kinds| {
                kinds
                    .iter()
                    .map(|(kind, n)| format!("{kind} {n}"))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "  {class:<28} {:>7} {:>6} {:>6}  {}",
            tally.objects,
            tally.named,
            tally.raw,
            seen.join(", ")
        );
        let total = totals.by_class.entry(class.clone()).or_default();
        total.objects += tally.objects;
        total.named += tally.named;
        total.raw += tally.raw;
    }
    for (class, seen) in kinds {
        let total = totals.kinds.entry(class).or_default();
        for (kind, n) in seen {
            *total.entry(kind).or_insert(0) += n;
        }
    }
    println!(
        "  {:<34} {:>5} {:>6} {:>5} {:>7}",
        "rel DefUID", "rels", "both", "one", "own raw"
    );
    for (def, tally) in &by_def {
        println!(
            "  {def:<34} {:>5} {:>6} {:>5} {:>7}",
            tally.rels, tally.both_named, tally.one_named, tally.own_raw
        );
        let total = totals.by_def.entry(def.clone()).or_default();
        total.rels += tally.rels;
        total.both_named += tally.both_named;
        total.one_named += tally.one_named;
        total.own_raw += tally.own_raw;
    }
}

fn score_meta(meta_xml: &Path, reading: &PidReading) {
    let Ok(xml) = std::fs::read_to_string(meta_xml) else {
        println!("  _Meta.xml: not readable");
        return;
    };
    let header: Vec<String> = container_attributes(&xml)
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{key}={value} [{}]", reading.locate(&value)))
        .collect();
    println!("  _Meta.xml Container: {}", header.join(", "));
    let (objects, rels) = read_answer_key(&xml);
    for object in &objects {
        let values: Vec<String> = object
            .attributes
            .iter()
            .filter(|(_, _, value)| !value.is_empty())
            .map(|(interface, key, value)| {
                format!("{interface}.{key}={value} [{}]", reading.locate(value))
            })
            .collect();
        println!(
            "  _Meta.xml {} UID [{}]: {}",
            object.class,
            reading.locate(&object.uid),
            values.join(", ")
        );
    }
    for rel in &rels {
        println!(
            "  _Meta.xml Rel {}: own UID [{}], UID1 [{}], UID2 [{}]",
            rel.def_uid,
            reading.locate(&rel.uid),
            reading.locate(&rel.uid1),
            reading.locate(&rel.uid2)
        );
    }
}

fn print_totals(totals: &Totals, drawings: usize) {
    println!("== all {drawings} drawings");
    let objects: usize = totals.by_class.values().map(|t| t.objects).sum();
    let named: usize = totals.by_class.values().map(|t| t.named).sum();
    println!(
        "  objects {objects}: named {named} ({})",
        percent(named, objects)
    );
    println!(
        "  {:<28} {:>7} {:>6} {:>6}  named as",
        "class", "objects", "named", "raw"
    );
    let mut classes: Vec<(&String, &ObjectTally)> = totals.by_class.iter().collect();
    classes.sort_by(|a, b| b.1.objects.cmp(&a.1.objects).then(a.0.cmp(b.0)));
    let class_order: Vec<String> = classes.iter().map(|(class, _)| (*class).clone()).collect();
    for (class, tally) in classes {
        let seen: Vec<String> = totals
            .kinds
            .get(class)
            .map(|kinds| {
                kinds
                    .iter()
                    .map(|(kind, n)| format!("{kind} {n}"))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "  {class:<28} {:>7} {:>6} {:>6}  {}",
            tally.objects,
            tally.named,
            tally.raw,
            seen.join(", ")
        );
    }

    let mut all = AttributeTally::default();
    for tally in totals.by_attribute.values() {
        all.add(tally);
    }
    println!("  attribute values {}", describe_attributes(&all));
    println!(
        "  {:<52} {:>6} {:>6} {:>6} {:>6} {:>6}  label caches carrying them",
        "interface.attribute (non-empty >= 5)", "values", "labels", "short", "fields", "raw"
    );
    let mut rows: Vec<(&String, &AttributeTally)> = totals
        .by_attribute
        .iter()
        .filter(|(_, t)| t.values - t.empty >= LIST_MIN_VALUES)
        .collect();
    rows.sort_by(|a, b| {
        (b.1.values - b.1.empty)
            .cmp(&(a.1.values - a.1.empty))
            .then(a.0.cmp(b.0))
    });
    for (key, tally) in rows {
        let mut carriers: Vec<(&String, &usize)> = tally.carried_by.iter().collect();
        carriers.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let carriers: Vec<String> = carriers
            .iter()
            .take(3)
            .map(|(name, n)| format!("{name} {n}"))
            .collect();
        println!(
            "  {key:<52} {:>6} {:>6} {:>6} {:>6} {:>6}  {}",
            tally.values - tally.empty,
            tally.in_labels,
            tally.short,
            tally.in_fields,
            tally.raw,
            carriers.join(", ")
        );
    }
    println!(
        "  {:<28} {:<52} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "class",
        "interface.attribute (non-empty >= 5)",
        "values",
        "labels",
        "short",
        "fields",
        "raw"
    );
    for class in &class_order {
        let mut rows: Vec<(&String, &AttributeTally)> = totals
            .by_class_attribute
            .iter()
            .filter(|((c, _), t)| c == class && t.values - t.empty >= LIST_MIN_VALUES)
            .map(|((_, key), t)| (key, t))
            .collect();
        rows.sort_by(|a, b| {
            (b.1.values - b.1.empty)
                .cmp(&(a.1.values - a.1.empty))
                .then(a.0.cmp(b.0))
        });
        for (key, tally) in rows {
            println!(
                "  {class:<28} {key:<52} {:>6} {:>6} {:>6} {:>6} {:>6}",
                tally.values - tally.empty,
                tally.in_labels,
                tally.short,
                tally.in_fields,
                tally.raw
            );
        }
    }

    let rels: usize = totals.by_def.values().map(|t| t.rels).sum();
    let both: usize = totals.by_def.values().map(|t| t.both_named).sum();
    let own: usize = totals.by_def.values().map(|t| t.own_raw).sum();
    println!(
        "  rels {rels}: both ends named {both} ({}), own UID raw {own}",
        percent(both, rels)
    );
    println!(
        "  {:<34} {:>5} {:>6} {:>5} {:>7}",
        "rel DefUID", "rels", "both", "one", "own raw"
    );
    let mut defs: Vec<(&String, &RelTally)> = totals.by_def.iter().collect();
    defs.sort_by(|a, b| b.1.rels.cmp(&a.1.rels).then(a.0.cmp(b.0)));
    for (def, tally) in defs {
        println!(
            "  {def:<34} {:>5} {:>6} {:>5} {:>7}",
            tally.rels, tally.both_named, tally.one_named, tally.own_raw
        );
    }
    println!(
        "  representations {}: UID-joined {}, on their published GraphicOID {}",
        totals.representations, totals.uid_joined, totals.on_graphic_oid
    );

    println!("  attribute sets by storage / record family / class (all drawings)");
    for ((storage, family, class), count) in &totals.census {
        println!("    {count:>6}  {storage:<12} {family:<36} {class}");
    }
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
