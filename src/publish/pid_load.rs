//! Publish data read from the `.pid` alone, with no plant database.
//!
//! Plan `docs/plans/2026-10-10-publish-from-the-pid-alone.md` P2. Only what
//! the drawing file states is kept, and every value says where it came from
//! (`docs/analysis/2026-10-10-publish-from-the-pid-alone.md`,
//! `docs/analysis/2026-10-10-a-label-value-reaches-its-item-through-its-text-box.md`):
//!
//! * an item's own `P&IDAttributes` row ([`crate::parsers::attribute_rows`])
//!   names the drawing (`DrawingNo`), the representation (`DrawingID`), the
//!   model item (`ModelID`) and its type (`ModelItemType`); the record the row
//!   describes is the representation's `GraphicOID`;
//! * a label value row describes an `igTextBox`, whose `+4` is the
//!   `DependencyObject` it belongs to, whose `+22` is the labelled item's own
//!   `P&IDAttributes` row and whose `+16` is the graphic the label hangs on;
//! * an instrument's class comes from the `SymbolAttr` row on its graphic,
//!   or, when that row does not name a class, from the folder its symbol is
//!   filed under in the library (`Instrumentation\<family>\...`,
//!   `instrument_class_by_library`).
//!
//! Item names are composed by the two rules the user accepted on 2026-10-10
//! (instruments `{MeasuredVariable}-{InstrTagSequenceNo}`, equipment
//! `{EquipPrefix}-{EquipSeqNo}`) and are marked as such.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::parsers::attribute_rows::{decode_attribute_row, AttributeRow};
use crate::parsers::jproperties::parse_jproperties;
use crate::parsers::sheet_records::{
    decode_igsymbol_at, parse_live_psm_header, sheet_record_starts,
};
use crate::streams::jsite::extract_unc_or_path;

/// The header magic every record-chain stream opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
const TEXT_BOX: u16 = 0x004D;
const DEPENDENCY: u16 = 0x00FA;
const POINT: u16 = 0x005E;
const SYMBOL: u16 = 0x00CE;
/// Payload offset of the dependency an `igTextBox` belongs to.
const TEXT_PARENT_OFFSET: usize = 4;
/// Payload offset of the graphic a `DependencyObject` hangs on.
const DEPENDENCY_GRAPHIC_OFFSET: usize = 16;
/// Payload offset of the attribute row at a `DependencyObject`'s other end.
const DEPENDENCY_ROW_OFFSET: usize = 22;

/// `ModelItemType` -> published class, one to one over the corpus's 6 131
/// published objects; `Instrument` is split by [`instrument_class`].
const CLASS_OF_ITEM_TYPE: [(&str, &str); 11] = [
    ("Vessel", "PIDProcessVessel"),
    ("EquipmentOther", "PIDProcessEquipment"),
    ("Mechanical", "PIDMechanicalEquipment"),
    ("Exchanger", "PIDHeatTransferEquipment"),
    ("Nozzle", "PIDNozzle"),
    ("PipingComp", "PIDPipingComponent"),
    ("PipeRun", "PIDPipingConnector"),
    ("SignalRun", "PIDSignalConnector"),
    ("ItemNote", "PIDNote"),
    ("OPC", "PIDBranchPoint"),
    ("Instrument", ""),
];
/// Item types left without a class: the `ModelID` on an off-page
/// connector's row is not always the item `SmartPlant` publishes for it (23
/// of the 53 the corpus's files name are not in the reference).
const UNCLASSED_ITEM_TYPES: [&str; 1] = ["OPC"];
const INSTRUMENT_CLASSES: [&str; 3] = [
    "PIDInstrument",
    "PIDControlSystemFunction",
    "PIDInlineInstrument",
];
const EQUIPMENT_CLASSES: [&str; 4] = [
    "PIDProcessVessel",
    "PIDProcessEquipment",
    "PIDMechanicalEquipment",
    "PIDHeatTransferEquipment",
];

/// Where a published value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidValueSource {
    /// A label value row (its oid) that reaches the item through its text
    /// box's dependency.
    Label {
        /// Oid of the `0x0089` value row.
        row: u32,
    },
    /// Composed from the item's other values by an accepted naming rule.
    NameRule(&'static str),
}

/// One published attribute value of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidPublishAttribute {
    /// The publish interface, e.g. `INamedInstrument`.
    pub interface: &'static str,
    /// The attribute, e.g. `MeasuredVariable`.
    pub name: &'static str,
    /// The value.
    pub value: String,
    /// Where it comes from.
    pub source: PidValueSource,
}

/// One model item named by a `P&IDAttributes` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidPublishItem {
    /// The model item UID (`ModelID`).
    pub uid: String,
    /// `ModelItemType`, e.g. `Instrument`.
    pub model_item_type: String,
    /// The published class, or `None` when the file does not decide it.
    pub class: Option<&'static str>,
    /// Attribute values with their sources, ordered by interface and name.
    pub attributes: Vec<PidPublishAttribute>,
}

/// One representation of an item on the drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidPublishRepresentation {
    /// The representation UID (`DrawingID`).
    pub uid: String,
    /// The model item it represents (`ModelID`), or `None` for a branch
    /// point drawn on a run: `SmartPlant` relates that representation to a
    /// branch-point object of its own, not to the run the row names.
    pub item_uid: Option<String>,
    /// The record the item's `P&IDAttributes` row describes.
    pub graphic_oid: u32,
}

/// Counts that say how much of the file was read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PidPublishStats {
    /// Live `0x0089` records in the root storage's record chains.
    pub attribute_records: usize,
    /// Of those, the ones whose set header did not frame.
    pub undecoded_records: usize,
    /// Non-blank label values.
    pub label_values: usize,
    /// Label values whose chain reaches an item.
    pub label_values_reached: usize,
    /// Label values kept as a published attribute.
    pub label_values_published: usize,
    /// Attributes left out because two labels on one item disagree.
    pub conflicting_attributes: usize,
    /// Instruments whose class comes from their symbol's library folder,
    /// their graphics having no `SymbolAttr` row.
    pub instruments_classed_by_library: usize,
}

/// A drawing's publish data as the `.pid` states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidPublishDrawing {
    /// The drawing UID (`DrawingNo`).
    pub drawing_uid: String,
    /// The drawing name: the file name without `.pid`.
    pub drawing_name: String,
    /// Model items, ordered by UID.
    pub items: Vec<PidPublishItem>,
    /// Representations, ordered by UID.
    pub representations: Vec<PidPublishRepresentation>,
    /// How much of the file was read.
    pub stats: PidPublishStats,
}

/// Why a `.pid` gave no publish data.
#[derive(Debug)]
pub enum PidLoadError {
    /// The file could not be read as a compound file.
    Io(std::io::Error),
    /// No `P&IDAttributes` row carries a `DrawingNo`.
    NoDrawing,
}

impl fmt::Display for PidLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::NoDrawing => write!(f, "no P&IDAttributes row carries a DrawingNo"),
        }
    }
}

impl std::error::Error for PidLoadError {}

impl From<std::io::Error> for PidLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// The live records of the root storage's record chains that the loader
/// reads: attribute rows, the payloads of text boxes and dependencies, and
/// the library path of every symbol placement.
#[derive(Default)]
struct Records {
    families: HashMap<u32, u16>,
    payloads: HashMap<u32, Vec<u8>>,
    rows: BTreeMap<u32, AttributeRow>,
    undecoded: usize,
    /// Symbol placement oid -> the `JSite<id>` its symbol reference names.
    symbol_sites: HashMap<u32, u32>,
    /// Symbol placement oid -> the `.sym` library path that site's
    /// `JProperties` states.
    library_paths: HashMap<u32, String>,
}

impl Records {
    fn read(path: &Path) -> Result<Self, PidLoadError> {
        let mut compound = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
        let mut streams: Vec<PathBuf> = Vec::new();
        let mut sites: HashMap<u32, PathBuf> = HashMap::new();
        for entry in compound.read_root_storage() {
            if entry.is_stream() {
                streams.push(entry.path().to_path_buf());
            } else if let Some(id) = entry
                .name()
                .strip_prefix("JSite")
                .and_then(|id| id.parse::<u32>().ok())
            {
                sites.insert(id, entry.path().join("JProperties"));
            }
        }
        let mut records = Self::default();
        for stream_path in streams {
            let mut data = Vec::new();
            compound.open_stream(&stream_path)?.read_to_end(&mut data)?;
            if data.get(0..4) == Some(&CHAIN_MAGIC.to_le_bytes()[..]) {
                records.add_chain(&data);
            }
        }
        let referenced: BTreeSet<u32> = records.symbol_sites.values().copied().collect();
        let mut site_paths: HashMap<u32, String> = HashMap::new();
        for site in referenced {
            let Some(properties) = sites.get(&site) else {
                continue;
            };
            let Ok(mut stream) = compound.open_stream(properties) else {
                continue;
            };
            let mut data = Vec::new();
            if stream.read_to_end(&mut data).is_err() {
                continue;
            }
            if let Some(symbol) = parse_jproperties(&data)
                .strings
                .iter()
                .find(|value| value.ends_with(".sym"))
            {
                site_paths.insert(site, extract_unc_or_path(symbol));
            }
        }
        records.library_paths = records
            .symbol_sites
            .iter()
            .filter_map(|(oid, site)| Some((*oid, site_paths.get(site)?.clone())))
            .collect();
        Ok(records)
    }

    fn add_chain(&mut self, data: &[u8]) {
        for at in sheet_record_starts(data) {
            let Some(header) = parse_live_psm_header(data, at) else {
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
            self.families.insert(oid, header.type_code);
            match header.type_code {
                FREE_FORM_ATTR_SET => match decode_attribute_row(payload) {
                    Some(row) => {
                        self.rows.insert(oid, row);
                    }
                    None => self.undecoded += 1,
                },
                TEXT_BOX | DEPENDENCY => {
                    self.payloads.insert(oid, payload.to_vec());
                }
                SYMBOL => {
                    if let Some(symbol) = decode_igsymbol_at(data, at) {
                        self.symbol_sites.insert(oid, symbol.jsite_ref);
                    }
                }
                _ => {}
            }
        }
    }

    /// The oid at `offset` in the payload of `oid`, when `oid` is a `family`.
    fn link(&self, oid: u32, family: u16, offset: usize) -> Option<u32> {
        if self.families.get(&oid) != Some(&family) {
            return None;
        }
        let bytes = self.payloads.get(&oid)?.get(offset..offset + 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

/// The model item an item's own `P&IDAttributes` row names (not a label's,
/// not the drawing's, not a relationship's).
fn item_model(row: &AttributeRow) -> Option<&str> {
    if row.class != "P&IDAttributes" {
        return None;
    }
    if !matches!(row.text("DrawingItemType"), Some("Symbol" | "Connector")) {
        return None;
    }
    row.text("ModelID").filter(|model| model.len() > 1)
}

/// An instrument's class from the `SymbolAttr` row on its graphic.
fn instrument_class(symbol_attr: Option<&AttributeRow>) -> Option<&'static str> {
    let row = symbol_attr?;
    if row.text("Class") == Some("Instrument In-Line Component") {
        return Some("PIDInlineInstrument");
    }
    match row.text("Type") {
        Some("System functions") => Some("PIDControlSystemFunction"),
        Some("Off-line instruments") => Some("PIDInstrument"),
        _ => None,
    }
}

/// An instrument's class from the folder its symbol is filed under: the
/// library segment right after `Instrumentation`. Over the corpus's 287
/// instruments the folder and the published class never disagree
/// (`In-Line` 66 and `Relief Devices` 6 in-line, `Off-Line` 116 and
/// `Actuators` 11 instruments, `System Functions` 88 control system
/// functions; `docs/analysis/2026-10-10-an-instruments-library-folder-states-its-class.md`).
/// Used when no graphic's `SymbolAttr` names a class: no row, or a row whose
/// `Type` / `Class` do not (one empty row on this corpus).
fn instrument_class_by_library(path: &str) -> Option<&'static str> {
    let segments: Vec<&str> = path
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect();
    let (_, directories) = segments.split_last()?;
    let at = directories
        .iter()
        .position(|directory| directory.eq_ignore_ascii_case("Instrumentation"))?;
    match directories.get(at + 1)?.to_ascii_lowercase().as_str() {
        "in-line" | "relief devices" => Some("PIDInlineInstrument"),
        "off-line" | "actuators" => Some("PIDInstrument"),
        "system functions" => Some("PIDControlSystemFunction"),
        _ => None,
    }
}

/// The published attribute a label property fills on an item of `class`.
fn published_attribute(property: &str, class: &str) -> Option<(&'static str, &'static str)> {
    let instrument = INSTRUMENT_CLASSES.contains(&class);
    let equipment = EQUIPMENT_CLASSES.contains(&class);
    match property {
        "MeasuredVariableCode" if instrument => Some(("INamedInstrument", "MeasuredVariable")),
        "TagSequenceNo" if instrument => Some(("INamedInstrument", "InstrTagSequenceNo")),
        "TagSequenceNo" if class == "PIDPipingConnector" => {
            Some(("INamedPipingConnector", "PipingConnectorSeqNo"))
        }
        "TagSequenceNo" if equipment => Some(("INamedEquipment", "EquipSeqNo")),
        "TagPrefix" if equipment => Some(("INamedEquipment", "EquipPrefix")),
        "PipingMaterialsClass" => Some(("IPipingSpecifiedItem", "PipingMaterialsClass")),
        "InsulationSpec" => Some(("IInsulatedItem", "InsulSpec")),
        "Description" => Some(("IObject", "Description")),
        "ToFromText" => Some(("IOffpageConnector", "ToFromText")),
        "Note.Body" => Some(("INote", "NoteText")),
        _ => None,
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

/// The non-blank `(property, value)` pairs a label value row caches.
fn label_values(row: &AttributeRow) -> Vec<(String, String)> {
    let pairs = if row.class == "_SmartText" {
        row.text("FormatString")
            .map(format_pairs)
            .unwrap_or_default()
    } else if row.class.starts_with('_') {
        match (row.text("ItemProperty"), row.text("ItemValue")) {
            (Some(property), Some(value)) => vec![(property.to_string(), value.to_string())],
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    pairs
        .into_iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .collect()
}

/// The item a label value row reaches: text box `+4` -> dependency, whose
/// `+22` row and `+16` graphic must not name two different items.
fn labelled_item<'a>(
    row: &AttributeRow,
    records: &'a Records,
    graphic_items: &'a HashMap<u32, String>,
) -> Option<&'a str> {
    let dependency = records.link(row.described, TEXT_BOX, TEXT_PARENT_OFFSET)?;
    let by_row = records
        .link(dependency, DEPENDENCY, DEPENDENCY_ROW_OFFSET)
        .and_then(|oid| records.rows.get(&oid))
        .and_then(item_model);
    let by_graphic = records
        .link(dependency, DEPENDENCY, DEPENDENCY_GRAPHIC_OFFSET)
        .and_then(|oid| graphic_items.get(&oid))
        .map(String::as_str);
    match (by_row, by_graphic) {
        (Some(row_item), Some(graphic_item)) if row_item != graphic_item => None,
        (Some(item), _) | (None, Some(item)) => Some(item),
        (None, None) => None,
    }
}

/// Read a drawing's publish data from its `.pid`.
pub fn load_publish_from_pid(path: &Path) -> Result<PidPublishDrawing, PidLoadError> {
    let records = Records::read(path)?;
    let mut stats = PidPublishStats {
        attribute_records: records.rows.len() + records.undecoded,
        undecoded_records: records.undecoded,
        ..PidPublishStats::default()
    };

    // `DrawingNo` -> the `P&IDAttributes` rows that carry it.
    let mut drawing_numbers: BTreeMap<&str, usize> = BTreeMap::new();
    let mut symbol_attr: HashMap<u32, &AttributeRow> = HashMap::new();
    let mut graphic_items: HashMap<u32, String> = HashMap::new();
    let mut item_types: BTreeMap<String, String> = BTreeMap::new();
    let mut item_graphics: HashMap<String, Vec<u32>> = HashMap::new();
    let mut representations = BTreeMap::new();
    for row in records.rows.values() {
        if row.class == "SymbolAttr" {
            symbol_attr.insert(row.described, row);
        }
        if row.class == "P&IDAttributes" {
            if let Some(uid) = row.text("DrawingNo").filter(|uid| uid.len() > 1) {
                *drawing_numbers.entry(uid).or_insert(0) += 1;
            }
        }
        let Some(model) = item_model(row) else {
            continue;
        };
        let item_type = row.text("ModelItemType").unwrap_or_default();
        if !CLASS_OF_ITEM_TYPE
            .iter()
            .any(|(kind, _)| *kind == item_type)
        {
            continue;
        }
        graphic_items.insert(row.described, model.to_string());
        item_types.insert(model.to_string(), item_type.to_string());
        item_graphics
            .entry(model.to_string())
            .or_default()
            .push(row.described);
        // A `Symbol` row on a `DependencyObject` is a run's piece that
        // `SmartPlant` does not publish (518 / 518 over the corpus; every
        // other representation is published).
        let symbol_on = |family| {
            row.text("DrawingItemType") == Some("Symbol")
                && records.families.get(&row.described) == Some(&family)
        };
        let unpublished = symbol_on(DEPENDENCY);
        let branch_point = symbol_on(POINT);
        if let Some(uid) = row
            .text("DrawingID")
            .filter(|uid| uid.len() > 1 && !unpublished)
        {
            representations.insert(
                uid.to_string(),
                PidPublishRepresentation {
                    uid: uid.to_string(),
                    item_uid: (!branch_point).then(|| model.to_string()),
                    graphic_oid: row.described,
                },
            );
        }
    }
    let drawing_uid = drawing_numbers
        .into_iter()
        .max_by_key(|(_, rows)| *rows)
        .map(|(uid, _)| uid.to_string())
        .ok_or(PidLoadError::NoDrawing)?;

    let mut classes: BTreeMap<&str, Option<&'static str>> = BTreeMap::new();
    for (uid, item_type) in &item_types {
        let class = match CLASS_OF_ITEM_TYPE
            .iter()
            .find(|(kind, _)| *kind == item_type.as_str())
        {
            _ if UNCLASSED_ITEM_TYPES.contains(&item_type.as_str()) => None,
            Some((_, "")) => {
                let graphics = item_graphics.get(uid).into_iter().flatten();
                let by_symbol_attr = graphics
                    .clone()
                    .find_map(|graphic| instrument_class(symbol_attr.get(graphic).copied()));
                by_symbol_attr.or_else(|| {
                    let by_library = graphics.clone().find_map(|graphic| {
                        records
                            .library_paths
                            .get(graphic)
                            .and_then(|path| instrument_class_by_library(path))
                    });
                    stats.instruments_classed_by_library += usize::from(by_library.is_some());
                    by_library
                })
            }
            Some((_, class)) => Some(*class),
            None => None,
        };
        classes.insert(uid.as_str(), class);
    }

    // (item, interface, attribute) -> (values, first row giving each).
    let mut found: BTreeMap<(&str, &'static str, &'static str), BTreeMap<String, u32>> =
        BTreeMap::new();
    for (oid, row) in &records.rows {
        let values = label_values(row);
        if values.is_empty() {
            continue;
        }
        stats.label_values += values.len();
        let Some(item) = labelled_item(row, &records, &graphic_items) else {
            continue;
        };
        stats.label_values_reached += values.len();
        let Some((item, Some(class))) = classes.get_key_value(item) else {
            continue;
        };
        for (property, value) in values {
            if let Some((interface, name)) = published_attribute(&property, class) {
                found
                    .entry((*item, interface, name))
                    .or_default()
                    .entry(value)
                    .or_insert(*oid);
            }
        }
    }

    let mut attributes: BTreeMap<&str, Vec<PidPublishAttribute>> = BTreeMap::new();
    for ((item, interface, name), values) in found {
        if values.len() > 1 {
            stats.conflicting_attributes += 1;
            continue;
        }
        if let Some((value, row)) = values.into_iter().next() {
            attributes
                .entry(item)
                .or_default()
                .push(PidPublishAttribute {
                    interface,
                    name,
                    value,
                    source: PidValueSource::Label { row },
                });
        }
    }

    let items = item_types
        .iter()
        .map(|(uid, item_type)| {
            let class = classes.get(uid.as_str()).copied().flatten();
            let mut attributes = attributes.remove(uid.as_str()).unwrap_or_default();
            stats.label_values_published += attributes.len();
            if let Some(name) = class.and_then(|class| composed_name(class, &attributes)) {
                attributes.push(name);
            }
            attributes.sort_by(|a, b| (a.interface, a.name).cmp(&(b.interface, b.name)));
            PidPublishItem {
                uid: uid.clone(),
                model_item_type: item_type.clone(),
                class,
                attributes,
            }
        })
        .collect();

    let drawing_name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(PidPublishDrawing {
        drawing_uid,
        drawing_name,
        items,
        representations: representations.into_values().collect(),
        stats,
    })
}

/// `IObject.Name` by the accepted rules, when both parts are present.
fn composed_name(class: &str, attributes: &[PidPublishAttribute]) -> Option<PidPublishAttribute> {
    let value = |interface: &str, name: &str| {
        attributes
            .iter()
            .find(|attribute| attribute.interface == interface && attribute.name == name)
            .map(|attribute| attribute.value.as_str())
    };
    let (rule, first, second) = if INSTRUMENT_CLASSES.contains(&class) {
        (
            "{MeasuredVariable}-{InstrTagSequenceNo}",
            value("INamedInstrument", "MeasuredVariable")?,
            value("INamedInstrument", "InstrTagSequenceNo")?,
        )
    } else if EQUIPMENT_CLASSES.contains(&class) {
        (
            "{EquipPrefix}-{EquipSeqNo}",
            value("INamedEquipment", "EquipPrefix")?,
            value("INamedEquipment", "EquipSeqNo")?,
        )
    } else {
        return None;
    };
    Some(PidPublishAttribute {
        interface: "IObject",
        name: "Name",
        value: format!("{first}-{second}"),
        source: PidValueSource::NameRule(rule),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(class: &str, fields: &[(&str, &str)]) -> AttributeRow {
        AttributeRow {
            oid: 0,
            described: 0,
            class: class.to_string(),
            fields: fields
                .iter()
                .map(
                    |(name, value)| crate::parsers::attribute_rows::AttributeField {
                        name: (*name).to_string(),
                        value: crate::parsers::attribute_rows::AttributeValue::Text(
                            (*value).to_string(),
                        ),
                    },
                )
                .collect(),
        }
    }

    #[test]
    fn an_item_row_is_a_symbol_or_connector_with_a_model() {
        let symbol = row(
            "P&IDAttributes",
            &[("DrawingItemType", "Symbol"), ("ModelID", "ABC")],
        );
        assert_eq!(item_model(&symbol), Some("ABC"));
        let label = row(
            "P&IDAttributes",
            &[("DrawingItemType", "LabelPersist"), ("ModelID", "0")],
        );
        assert_eq!(item_model(&label), None);
        let relationship = row(
            "P&IDAttributes",
            &[("DrawingItemType", ""), ("ModelID", "ABC")],
        );
        assert_eq!(item_model(&relationship), None);
    }

    #[test]
    fn an_instrument_class_follows_its_symbol_attributes() {
        let csf = row(
            "SymbolAttr",
            &[
                ("Class", "Instrument off-line component"),
                ("Type", "System functions"),
            ],
        );
        let inline = row(
            "SymbolAttr",
            &[
                ("Class", "Instrument In-Line Component"),
                ("Type", "Relief Devices"),
            ],
        );
        let off_line = row(
            "SymbolAttr",
            &[
                ("Class", "Instrument off-line component"),
                ("Type", "Off-line instruments"),
            ],
        );
        assert_eq!(
            instrument_class(Some(&csf)),
            Some("PIDControlSystemFunction")
        );
        assert_eq!(instrument_class(Some(&inline)), Some("PIDInlineInstrument"));
        assert_eq!(instrument_class(Some(&off_line)), Some("PIDInstrument"));
        assert_eq!(instrument_class(None), None);
    }

    #[test]
    fn an_instruments_library_folder_states_its_class() {
        let library = r"\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation";
        assert_eq!(
            instrument_class_by_library(&format!(
                r"{library}\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym"
            )),
            Some("PIDInlineInstrument")
        );
        assert_eq!(
            instrument_class_by_library(&format!(
                r"{library}\Relief Devices\Safety Valve Type 2.sym"
            )),
            Some("PIDInlineInstrument")
        );
        assert_eq!(
            instrument_class_by_library(&format!(r"{library}\Actuators\Diaphragm Actuator.sym")),
            Some("PIDInstrument")
        );
        assert_eq!(
            instrument_class_by_library(&format!(
                r"{library}\Off-Line\Without Implied Components\Pressure\PG-Pressure Gauge.sym"
            )),
            Some("PIDInstrument")
        );
        assert_eq!(
            instrument_class_by_library(&format!(
                r"{library}\System Functions\D C S\DCS Func Access in Prime Loc.sym"
            )),
            Some("PIDControlSystemFunction")
        );
        assert_eq!(
            instrument_class_by_library("C:/Symbols/INSTRUMENTATION/IN-LINE/x.sym"),
            Some("PIDInlineInstrument"),
            "segments compare case-insensitively, either separator"
        );
        assert_eq!(
            instrument_class_by_library(&format!(r"{library}\Unknown Family\x.sym")),
            None
        );
        assert_eq!(
            instrument_class_by_library(r"\\MM-128\Symbols\Piping\Valves\Gate Valve.sym"),
            None,
            "not under Instrumentation"
        );
        assert_eq!(
            instrument_class_by_library(r"\\MM-128\Symbols\Instrumentation\In-Line.sym"),
            None,
            "the family is a folder, not the file"
        );
        assert_eq!(instrument_class_by_library(""), None);
    }

    #[test]
    fn a_smart_text_caches_every_non_blank_pair() {
        let text = row(
            "_SmartText",
            &[(
                "FormatString",
                "<F>MeasuredVariableCode=TT</F><F>SubFunctionArea= </F><F>TagSequenceNo=0112101A</F>",
            )],
        );
        assert_eq!(
            label_values(&text),
            vec![
                ("MeasuredVariableCode".to_string(), "TT".to_string()),
                ("TagSequenceNo".to_string(), "0112101A".to_string()),
            ]
        );
        let property = row(
            "_TagSequenceNo",
            &[("ItemProperty", "TagSequenceNo"), ("ItemValue", "0112101A")],
        );
        assert_eq!(label_values(&property).len(), 1);
    }

    #[test]
    fn a_tag_sequence_number_lands_where_the_class_publishes_it() {
        assert_eq!(
            published_attribute("TagSequenceNo", "PIDInstrument"),
            Some(("INamedInstrument", "InstrTagSequenceNo"))
        );
        assert_eq!(
            published_attribute("TagSequenceNo", "PIDProcessVessel"),
            Some(("INamedEquipment", "EquipSeqNo"))
        );
        assert_eq!(
            published_attribute("TagSequenceNo", "PIDPipingComponent"),
            None
        );
        assert_eq!(
            published_attribute("NominalDiameter", "PIDPipingConnector"),
            None
        );
    }

    #[test]
    fn names_compose_only_for_instruments_and_equipment_with_both_parts() {
        let part = |interface, name, value: &str| PidPublishAttribute {
            interface,
            name,
            value: value.to_string(),
            source: PidValueSource::Label { row: 1 },
        };
        let instrument = [
            part("INamedInstrument", "MeasuredVariable", "TT"),
            part("INamedInstrument", "InstrTagSequenceNo", "0112101A"),
        ];
        assert_eq!(
            composed_name("PIDInstrument", &instrument).map(|name| name.value),
            Some("TT-0112101A".to_string())
        );
        let vessel = [
            part("INamedEquipment", "EquipPrefix", "V"),
            part("INamedEquipment", "EquipSeqNo", "01121A"),
        ];
        assert_eq!(
            composed_name("PIDProcessVessel", &vessel).map(|name| name.value),
            Some("V-01121A".to_string())
        );
        assert_eq!(composed_name("PIDProcessVessel", &vessel[..1]), None);
        assert_eq!(composed_name("PIDPipingComponent", &vessel), None);
    }
}
