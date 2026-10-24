//! `_NominalDiameter` label texts against the five reference drawings.
//!
//! N4-1 (plan `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md`).
//! Lists every label form the loader can see, joins it to the item the text
//! box reaches, and scores three publish spellings against that item's
//! reference `NominalDiameter`: `N"` / `a/b"` → `N in`, `N MM` → `N mm`,
//! and a bare number → `N mm`. A form is safe to emit only when every
//! reached label of that form is on a published item and equals the
//! reference. Anything else is counted and not a candidate.
//!
//! ```powershell
//! cargo run --example probe_n4_nominal_diameter
//! ```

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::attribute_rows::{decode_attribute_row, AttributeRow};
use pid_parse::parsers::sheet_records::{parse_live_psm_header, sheet_record_starts};
use quick_xml::events::Event;
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
const TEXT_BOX: u16 = 0x004D;
const DEPENDENCY: u16 = 0x00FA;
const TEXT_PARENT_OFFSET: usize = 4;
const DEPENDENCY_GRAPHIC_OFFSET: usize = 16;
const DEPENDENCY_ROW_OFFSET: usize = 22;

#[derive(Default)]
struct Records {
    families: HashMap<u32, u16>,
    payloads: HashMap<u32, Vec<u8>>,
    rows: BTreeMap<u32, AttributeRow>,
}

impl Records {
    fn read(path: &Path) -> Self {
        let mut records = Self::default();
        let Ok(file) = std::fs::File::open(path) else {
            return records;
        };
        let Ok(mut compound) = cfb::CompoundFile::open(file) else {
            return records;
        };
        let streams: Vec<PathBuf> = compound
            .read_root_storage()
            .filter(|entry| entry.is_stream())
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for stream_path in streams {
            let mut data = Vec::new();
            let Ok(mut stream) = compound.open_stream(&stream_path) else {
                continue;
            };
            if stream.read_to_end(&mut data).is_err() {
                continue;
            }
            if data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..]) {
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
                        records.rows.insert(oid, row);
                    }
                } else if header.type_code == TEXT_BOX || header.type_code == DEPENDENCY {
                    records.payloads.insert(oid, payload.to_vec());
                }
            }
        }
        records
    }

    fn link(&self, oid: u32, family: u16, offset: usize) -> Option<u32> {
        if self.families.get(&oid) != Some(&family) {
            return None;
        }
        let bytes = self.payloads.get(&oid)?.get(offset..offset + 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

fn item_model(row: &AttributeRow) -> Option<&str> {
    if row.class != "P&IDAttributes" {
        return None;
    }
    if !matches!(row.text("DrawingItemType"), Some("Symbol" | "Connector")) {
        return None;
    }
    row.text("ModelID").filter(|model| model.len() > 1)
}

/// `N"` / `a/b"` / `N MM` / a bare number, or a shape this probe does not know.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Form {
    Inches,
    Millimetres,
    Bare,
    Other,
}

fn format_ratio(num: i64, den: i64) -> Option<String> {
    if den <= 0 || num < 0 {
        return None;
    }
    let whole = num / den;
    let mut rem = num % den;
    if rem == 0 {
        return Some(whole.to_string());
    }
    let mut digits = String::new();
    for _ in 0..6 {
        rem *= 10;
        digits.push(char::from_digit((rem / den) as u32, 10)?);
        rem %= den;
        if rem == 0 {
            return Some(format!("{whole}.{digits}"));
        }
    }
    None
}

fn parse_unsigned(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The number in front of `in` or `mm`, from an integer, a fraction or a mixed number.
fn parse_measure(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some((whole, frac)) = text.split_once([' ', '-']) {
        let whole = parse_unsigned(whole)?;
        let (num, den) = frac.split_once('/')?;
        let num = parse_unsigned(num)?;
        let den = parse_unsigned(den)?;
        return format_ratio(whole * den + num, den);
    }
    if let Some((num, den)) = text.split_once('/') {
        return format_ratio(parse_unsigned(num)?, parse_unsigned(den)?);
    }
    parse_unsigned(text).map(|n| n.to_string())
}

fn classify(raw: &str) -> (Form, Option<String>) {
    let text = raw.trim();
    if let Some(body) = text.strip_suffix('"') {
        return (Form::Inches, parse_measure(body).map(|n| format!("{n} in")));
    }
    let upper = text.to_ascii_uppercase();
    if let Some(body) = upper.strip_suffix("MM") {
        let body = body.trim();
        if body.is_empty() {
            return (Form::Other, None);
        }
        return (
            Form::Millimetres,
            parse_measure(body).map(|n| format!("{n} mm")),
        );
    }
    if parse_measure(text).is_some() && !text.contains('/') {
        return (Form::Bare, parse_measure(text).map(|n| format!("{n} mm")));
    }
    (Form::Other, None)
}

struct RefValue {
    class: String,
    interface: String,
    value: String,
}

fn read_reference(xml: &str) -> HashMap<String, RefValue> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = HashMap::new();
    let mut depth = 0usize;
    let mut class = String::new();
    let mut uid = String::new();
    let mut nominal: Option<(String, String)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    class = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                    uid.clear();
                    nominal = None;
                } else if depth == 2 {
                    take_nominal(&start, &mut uid, &mut nominal);
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    take_nominal(&start, &mut uid, &mut nominal);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 && class != "Rel" {
                    if let Some((interface, value)) = nominal.take() {
                        if !uid.is_empty() {
                            out.insert(
                                uid.clone(),
                                RefValue {
                                    class: class.clone(),
                                    interface,
                                    value,
                                },
                            );
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn take_nominal(
    start: &quick_xml::events::BytesStart<'_>,
    uid: &mut String,
    nominal: &mut Option<(String, String)>,
) {
    let interface = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    for attribute in start.attributes().flatten() {
        let name = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute
            .unescape_value()
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_default();
        if interface == "IObject" && name == "UID" {
            *uid = value;
        } else if name == "NominalDiameter" && !value.trim().is_empty() {
            *nominal = Some((interface.clone(), value));
        }
    }
}

#[derive(Default)]
struct Cell {
    labels: usize,
    unparsed: usize,
    no_item: usize,
    disagree: usize,
    no_ref: usize,
    equal: usize,
    differ: usize,
}

fn form_name(form: Form) -> &'static str {
    match form {
        Form::Inches => "inches",
        Form::Millimetres => "N MM",
        Form::Bare => "bare",
        Form::Other => "other",
    }
}

fn main() {
    let corpus = PathBuf::from(DEFAULT_CORPUS);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&corpus)
        .expect("corpus")
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();

    let mut forms: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_form: BTreeMap<Form, Cell> = BTreeMap::new();
    let mut by_class: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut interfaces: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut samples: Vec<String> = Vec::new();
    let mut ref_total = 0usize;
    let mut ref_hit = 0usize;

    for (stem, pid, xml_path) in &drawings {
        let xml = std::fs::read_to_string(xml_path).expect("reference");
        let reference = read_reference(&xml);
        ref_total += reference.len();
        let records = Records::read(pid);
        let mut graphic_items: HashMap<u32, String> = HashMap::new();
        let mut item_types: HashMap<String, String> = HashMap::new();
        for row in records.rows.values() {
            if let Some(model) = item_model(row) {
                graphic_items.insert(row.described, model.to_string());
                item_types.insert(
                    model.to_string(),
                    row.text("ModelItemType").unwrap_or("?").to_string(),
                );
            }
        }
        let mut per_item: BTreeMap<String, Vec<(Form, String)>> = BTreeMap::new();
        let mut drawing_labels = 0usize;
        for row in records.rows.values() {
            if row.text("ItemProperty") != Some("NominalDiameter") {
                continue;
            }
            let Some(raw) = row
                .text("ItemValue")
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            drawing_labels += 1;
            *forms.entry(raw.to_string()).or_insert(0) += 1;
            let (form, published) = classify(raw);
            let cell = by_form.entry(form).or_default();
            cell.labels += 1;
            let Some(published) = published else {
                cell.unparsed += 1;
                continue;
            };
            let Some(dependency) = records.link(row.described, TEXT_BOX, TEXT_PARENT_OFFSET) else {
                cell.no_item += 1;
                continue;
            };
            let by_row = records
                .link(dependency, DEPENDENCY, DEPENDENCY_ROW_OFFSET)
                .and_then(|oid| records.rows.get(&oid))
                .and_then(item_model);
            let by_graphic = records
                .link(dependency, DEPENDENCY, DEPENDENCY_GRAPHIC_OFFSET)
                .and_then(|oid| graphic_items.get(&oid))
                .map(String::as_str);
            let item = match (by_row, by_graphic) {
                (Some(a), Some(b)) if a != b => {
                    cell.disagree += 1;
                    continue;
                }
                (Some(item), _) | (None, Some(item)) => item.to_string(),
                (None, None) => {
                    cell.no_item += 1;
                    continue;
                }
            };
            per_item.entry(item).or_default().push((form, published));
        }
        let mut drawing_equal = 0usize;
        for (item, values) in &per_item {
            let kind = item_types.get(item).cloned().unwrap_or_default();
            let reference_value = reference.get(item);
            let distinct: BTreeMap<&str, Form> = values
                .iter()
                .map(|(form, published)| (published.as_str(), *form))
                .collect();
            if distinct.len() != 1 {
                for (form, _) in values {
                    by_form.entry(*form).or_default().disagree += 1;
                }
                continue;
            }
            let (published, form) = distinct.into_iter().next().expect("one spelling");
            let cell = by_form.entry(form).or_default();
            let Some(reference_value) = reference_value else {
                cell.no_ref += 1;
                by_class.entry(kind.clone()).or_default().0 += 1;
                if samples.len() < 16 {
                    samples.push(format!("no-ref {stem} {kind} {item} label→{published}"));
                }
                continue;
            };
            *interfaces
                .entry((
                    reference_value.class.clone(),
                    reference_value.interface.clone(),
                ))
                .or_insert(0) += 1;
            let class_cell = by_class.entry(reference_value.class.clone()).or_default();
            class_cell.0 += 1;
            if published == reference_value.value {
                cell.equal += 1;
                class_cell.1 += 1;
                drawing_equal += 1;
                ref_hit += 1;
            } else {
                cell.differ += 1;
                if samples.len() < 24 {
                    samples.push(format!(
                        "differ {stem} {} {item} label→{published} ref={}",
                        reference_value.class, reference_value.value
                    ));
                }
            }
        }
        println!(
            "{stem}: labels {drawing_labels}, items {}, equal {drawing_equal}, reference {}",
            per_item.len(),
            reference.len()
        );
    }

    println!("== forms");
    for (raw, count) in &forms {
        let (form, published) = classify(raw);
        println!(
            "  {count:>4}  {raw:?}  {}  {}",
            form_name(form),
            published.as_deref().unwrap_or("-")
        );
    }
    println!("== by form (second pass is the item join; labels is every row)");
    for (form, cell) in &by_form {
        println!(
            "  {:<12} labels {:>4} unparsed {:>3} no_item {:>3} disagree {:>3} no_ref {:>3} equal {:>4} differ {:>3}",
            form_name(*form),
            cell.labels,
            cell.unparsed,
            cell.no_item,
            cell.disagree,
            cell.no_ref,
            cell.equal,
            cell.differ
        );
    }
    println!("== reference interface");
    for ((class, interface), count) in &interfaces {
        println!("  {count:>4}  {class}  {interface}");
    }
    println!("== class equal/seen");
    for (class, (seen, equal)) in &by_class {
        println!("  {class}: seen {seen} equal {equal}");
    }
    println!("reference nominals {ref_total}, label-equal {ref_hit}");
    println!("== samples");
    for sample in &samples {
        println!("  {sample}");
    }
}
