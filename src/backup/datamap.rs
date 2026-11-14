//! `SPPIDDataMap.xml`: the map a plant publishes its P&ID data by.
//!
//! Plan `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md`
//! N9-2 (the enum map) and
//! `docs/plans/2026-11-10-publish-export-completion.md` S6 (the class and
//! property maps). The file is a publish document itself: a `<Container>`
//! of definition blocks keyed by their `IObject` `UID`, and of `<Rel>`
//! blocks that join two of them under a `DefUID`. What is read:
//!
//! * `SPMapClassDef`, a P&ID item type (`Vessel`, `PipeRun`), with the
//!   classes `MapClassToClass` publishes it as;
//! * `SPMapPropertyDef`, an attribute of an item type, with the class
//!   that owns it (`MapClassMapProperties`), the schema properties it is
//!   published as (`MapPropertyToProperty`), the enum or unit list its
//!   values come from (`MapPropertyMapEnumList`) and the conversion the
//!   vendor's adapter runs on it (`ProcessPropertyCriteria`);
//! * `MapEnumToEnum`, the published spelling of an enum, and
//!   `MapEnumMapEnum`, the enum of the list one level up that an enum
//!   sits under;
//! * `SPMapEnumDef`, a codelist entry with its name, description and the
//!   index the plant database stores (`MapEnumNumber`), the list it is an
//!   entry of (`MapEnumListMapEnum`), and the schema enums the vendor's
//!   adapter reads back into it (`EnumToMapEnum`);
//! * the names of the enum lists and of the unit lists;
//! * how many `<Rel>` blocks the file holds under each `DefUID`.
//!
//! The class and property maps are read out and nothing more: no
//! conversion is run and no enum hierarchy is walked here, and no publish
//! stage consults them. The enum map is what `publish::codelist_values`
//! spells a codelist cell of the dump by. The file may be the XML itself or the
//! `SPPIDDataMap.xml` entry inside a zip (dated copies in the same zip
//! are left alone).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// One `SPPIDDataMap.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataMap {
    /// `MapEnumToEnum` `UID1` → publish spelling of `UID2` (`@{GUID}`).
    pub enum_to_enum: HashMap<String, String>,
    /// `SPMapEnumListDef` `IObject` `UID` → `Name`.
    pub enum_list_names: HashMap<String, String>,
    /// `SPMapUoMListDef` `IObject` `UID` → `Name` (`SPMU_1` → `Distance`).
    pub unit_list_names: HashMap<String, String>,
    /// `SPMapClassDef` blocks, in file order.
    pub classes: Vec<MapClass>,
    /// `SPMapPropertyDef` blocks, in file order.
    pub properties: Vec<MapProperty>,
    /// `MapEnumMapEnum`: an enum (`UID2`) → the enum it sits under
    /// (`UID1`), a member of the list one level up (`EquipmentType` under
    /// `EquipmentSubclass` under `EquipmentClass`). The first rel that
    /// names an enum as `UID2` is the one kept.
    pub enum_parents: HashMap<String, String>,
    /// How many `<Rel>` blocks the file holds under each `DefUID`, whether
    /// or not this module reads that kind any further.
    pub rel_counts: BTreeMap<String, usize>,
    /// `SPMapEnumDef` blocks keyed by their `IObject` `UID` (`SP_93_2`).
    pub enums: HashMap<String, MapEnum>,
    /// `MapEnumListMapEnum`: an enum list (`SP_93`) → its enums, in file
    /// order.
    pub enum_list_members: HashMap<String, Vec<String>>,
    /// `MapEnumToEnum` `UID1` → every `UID2` as the file writes it
    /// (`EE68D`, a `{GUID}`), in file order. [`Self::enum_to_enum`] is the
    /// same rels spelled `@{…}` whatever the target, the last one kept.
    pub enum_targets: HashMap<String, Vec<String>>,
    /// `EnumToMapEnum` rels as `(UID1, UID2)`, in file order: a schema
    /// enum and the map enum the vendor's adapter reads it back into.
    pub enum_retrievals: Vec<(String, String)>,
}

/// One `SPMapEnumDef`: an entry of a plant codelist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapEnum {
    /// `IObject` `Name` (`New`).
    pub name: String,
    /// `IObject` `Description`.
    pub description: String,
    /// `IMapEnumDef` `MapEnumNumber`: the codelist index the plant
    /// database stores. `None` when the block states none or not a number.
    pub number: Option<i64>,
}

/// One `SPMapClassDef`: a P&ID item type the map publishes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapClass {
    /// `IObject` `UID` (`SPMC_10`, or a `{GUID}`).
    pub uid: String,
    /// `IObject` `Name`: the item type (`PipeRun`).
    pub name: String,
    /// `IMapObject` `ToolOID` (`SPMC_PipeRun`).
    pub tool_oid: String,
    /// `IMapClassDef` `ProcessClassCriteria`: the adapter class that
    /// publishes the item type (`EFAdapter.PipeRunDataExchanger,True,11`).
    /// Empty when the block states none.
    pub process_criteria: String,
    /// `MapClassToClass` `UID2`s in file order: what the item type is
    /// published as (`PIDPipingConnector`).
    pub publish_classes: Vec<String>,
}

/// One `SPMapPropertyDef`: an attribute of a P&ID item type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapProperty {
    /// `IObject` `UID`.
    pub uid: String,
    /// `IObject` `Name`: the attribute (`Description`), dotted when it is
    /// reached through a relationship (`ProcessDesign.Max.Temperature`).
    pub name: String,
    /// `IMapObject` `ToolOID` (`SPMP_Vessel_Description`). Not a key: two
    /// blocks carry the same one where the map lists an attribute of an
    /// item type twice (`SPMP_Vessel_EquipmentType`), and where two item
    /// types share one (`SPMP_PipeRun_ProcessDesign.Max.Temperature`, on
    /// `PipeRun` and on `DuctRun`).
    pub tool_oid: String,
    /// `UID1` of the first `MapClassMapProperties` rel that names this
    /// property: the [`MapClass`] that owns it. Empty when none does.
    pub class_uid: String,
    /// `MapPropertyToProperty` rels that start at this property, in file
    /// order. Most properties have none: the map lists every attribute of
    /// an item type and publishes some of them.
    pub targets: Vec<MapPropertyTarget>,
    /// `UID2` of the first `MapPropertyMapEnumList` rel that starts at
    /// this property: the enum list ([`DataMap::enum_list_names`]) or the
    /// unit list ([`DataMap::unit_list_names`]) its values come from.
    pub value_list: Option<String>,
    /// `IMapPropertyDef` `ProcessPropertyCriteria`: the conversion the
    /// adapter runs (`EFAdapter.EnumConversions`). `None` when the block
    /// does not carry the attribute, `Some("")` when it carries it empty.
    pub process_criteria: Option<String>,
}

/// One `MapPropertyToProperty` rel: a schema property an attribute is
/// published as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapPropertyTarget {
    /// `UID2`: the schema property (`LongMaterialDescription`).
    pub property: String,
    /// `IMapRel` `End2MapValue`: the class of the object the property is
    /// stated on. Usually one of the owning item type's
    /// [`MapClass::publish_classes`]; a pipe run's process properties name
    /// `PIDProcessPoint` instead.
    pub publish_class: String,
    /// `IMapRel` `EndEdgeDef`: the edge from the item to the object that
    /// carries the property (`ProcessPointCollection_21`). Empty when the
    /// rel states none.
    pub edge: String,
}

/// One way the map publishes a schema property: an item type, its
/// attribute, and the rel between that attribute and the property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSource<'a> {
    /// The item type. `None` when no `MapClassMapProperties` rel owns the
    /// property.
    pub class: Option<&'a MapClass>,
    /// The attribute.
    pub property: &'a MapProperty,
    /// The rel that names the schema property and its class.
    pub target: &'a MapPropertyTarget,
}

impl DataMap {
    /// The map class with this `IObject` `UID`.
    #[must_use]
    pub fn class(&self, uid: &str) -> Option<&MapClass> {
        self.classes.iter().find(|class| class.uid == uid)
    }

    /// Every attribute the map publishes as `property` on an object of
    /// `publish_class`, by the class each `MapPropertyToProperty` rel names
    /// itself ([`MapPropertyTarget::publish_class`]).
    ///
    /// The match is on names alone: it says which attribute of which item
    /// type a value of that property comes from, not how the value is
    /// spelled.
    #[must_use]
    pub fn sources(&self, publish_class: &str, property: &str) -> Vec<MapSource<'_>> {
        let mut sources = Vec::new();
        for candidate in &self.properties {
            for target in &candidate.targets {
                if target.publish_class == publish_class && target.property == property {
                    sources.push(MapSource {
                        class: self.class(&candidate.class_uid),
                        property: candidate,
                        target,
                    });
                }
            }
        }
        sources
    }

    /// The `UID` of the enum of `list` whose `MapEnumNumber` is `number`:
    /// the map enum a codelist cell of that list stands for.
    #[must_use]
    pub fn enum_numbered(&self, list: &str, number: i64) -> Option<&str> {
        self.enum_list_members
            .get(list)?
            .iter()
            .find(|uid| self.enums.get(*uid).and_then(|entry| entry.number) == Some(number))
            .map(String::as_str)
    }

    /// The published spelling of a codelist cell: `@` and the schema enum
    /// the first `MapEnumToEnum` rel sends the cell's map enum to, as the
    /// file writes it (`@{78398AB4-…}`, `@EE425`).
    #[must_use]
    pub fn enum_target(&self, list: &str, number: i64) -> Option<String> {
        let target = self
            .enum_targets
            .get(self.enum_numbered(list, number)?)?
            .first()?;
        Some(format!("@{target}"))
    }

    /// For each map enum, the schema enums an `EnumToMapEnum` rel reads
    /// back into it and no `MapEnumToEnum` rel publishes to, in file order.
    ///
    /// Such a schema enum is a node the vendor's adapter only reads back
    /// from, never writes: `NewConstruction` → `SP_93_2`, beside the
    /// `{78398AB4-…}` that `SP_93_2` is published as.
    #[must_use]
    pub fn enums_retrieved_only(&self) -> HashMap<&str, Vec<&str>> {
        let published: std::collections::HashSet<&str> = self
            .enum_targets
            .values()
            .flatten()
            .map(String::as_str)
            .collect();
        let mut retrieved: HashMap<&str, Vec<&str>> = HashMap::new();
        for (schema, map_enum) in &self.enum_retrievals {
            if !published.contains(schema.as_str()) {
                retrieved
                    .entry(map_enum.as_str())
                    .or_default()
                    .push(schema.as_str());
            }
        }
        retrieved
    }
}

/// Why a data map was not read.
#[derive(Debug)]
pub enum DataMapError {
    /// The file or zip could not be opened.
    Io(std::io::Error),
    /// The zip has no entry whose name is exactly `SPPIDDataMap.xml`.
    MissingEntry,
    /// The XML could not be read as UTF-8 events, or it ends before its
    /// root element closes.
    Xml(String),
}

impl std::fmt::Display for DataMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingEntry => write!(f, "zip has no SPPIDDataMap.xml entry"),
            Self::Xml(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for DataMapError {}

/// Read a data map from an XML file or from the `SPPIDDataMap.xml` entry of a zip.
///
/// # Errors
///
/// Returns [`DataMapError::MissingEntry`] when `path` is a zip without that
/// entry, [`DataMapError::Io`] when the path cannot be read, and
/// [`DataMapError::Xml`] when the events cannot be decoded or the document
/// is cut short: a map is read whole or not at all.
pub fn read_datamap(path: &Path) -> Result<DataMap, DataMapError> {
    let bytes = fs::read(path).map_err(DataMapError::Io)?;
    if bytes.starts_with(b"PK\x03\x04") {
        let entry = sppid_datamap_entry(&bytes)?;
        parse_datamap_xml(&entry)
    } else {
        parse_datamap_xml(&bytes)
    }
}

/// `SP_20_{code}` in the enum map, already spelled `@{GUID}`.
#[must_use]
pub fn fluid_system_guid(map: &DataMap, code: i64) -> Option<&str> {
    map.enum_to_enum
        .get(&format!("SP_20_{code}"))
        .map(String::as_str)
}

/// `SP_19_{code}` in the enum map, but only when its target is a
/// standard `@{GUID}` enum. A project-extension target (`EE_…`) is not
/// published as `FluidCode`; across the seven reference drawings only
/// the standard targets are, and code 20 is the only one observed.
#[must_use]
pub fn fluid_code_guid(map: &DataMap, code: i64) -> Option<&str> {
    map.enum_to_enum
        .get(&format!("SP_19_{code}"))
        .map(String::as_str)
        .filter(|value| is_guid_spelling(value))
}

/// `@{8-4-4-4-12}` hex. The map spells every target `@{…}`, so the
/// braces alone do not tell a standard enum from an `EE_…` extension.
fn is_guid_spelling(value: &str) -> bool {
    let Some(inner) = value.strip_prefix("@{").and_then(|v| v.strip_suffix('}')) else {
        return false;
    };
    let groups: Vec<&str> = inner.split('-').collect();
    groups.iter().map(|g| g.len()).eq([8, 4, 4, 4, 12])
        && groups
            .iter()
            .all(|g| g.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn sppid_datamap_entry(bytes: &[u8]) -> Result<Vec<u8>, DataMapError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|error| DataMapError::Xml(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| DataMapError::Xml(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let raw = entry.name_raw().to_vec();
        let name = crate::refdata_archive::decode_zip_name(&raw);
        let base = name.rsplit(['/', '\\']).next().unwrap_or(&name);
        if base != "SPPIDDataMap.xml" {
            continue;
        }
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).map_err(DataMapError::Io)?;
        return Ok(payload);
    }
    Err(DataMapError::MissingEntry)
}

/// The top-level block of the document that is being read.
enum Block {
    Class(MapClass),
    Property(MapProperty),
    /// An `SPMapEnumDef`, with its `IObject` `UID`.
    Enum(String, MapEnum),
    Rel(MapRel),
    /// A definition this module keeps nothing of.
    Other,
}

/// One `<Rel>` block: its `IRel`, and the `IMapRel` a property rel carries.
#[derive(Default)]
struct MapRel {
    uid1: String,
    uid2: String,
    def_uid: String,
    end2_map_value: String,
    end_edge_def: String,
}

/// The rels that join blocks, kept until every block has been read: a rel
/// may come before the definitions it joins.
#[derive(Default)]
struct Links {
    class_to_class: Vec<(String, String)>,
    class_properties: Vec<(String, String)>,
    property_targets: Vec<(String, MapPropertyTarget)>,
    property_lists: Vec<(String, String)>,
}

impl Block {
    fn open(name: &str) -> Self {
        match name {
            "SPMapClassDef" => Self::Class(MapClass::default()),
            "SPMapPropertyDef" => Self::Property(MapProperty::default()),
            "SPMapEnumDef" => Self::Enum(String::new(), MapEnum::default()),
            "Rel" => Self::Rel(MapRel::default()),
            _ => Self::Other,
        }
    }

    /// Take what one interface element of the block states.
    fn element(&mut self, name: &str, start: &BytesStart<'_>) {
        let text = |key: &str| attr(start, key).unwrap_or_default();
        match (self, name) {
            (Self::Class(class), "IObject") => {
                class.uid = text("UID");
                class.name = text("Name");
            }
            (Self::Class(class), "IMapObject") => class.tool_oid = text("ToolOID"),
            (Self::Class(class), "IMapClassDef") => {
                class.process_criteria = text("ProcessClassCriteria");
            }
            (Self::Property(property), "IObject") => {
                property.uid = text("UID");
                property.name = text("Name");
            }
            (Self::Property(property), "IMapObject") => property.tool_oid = text("ToolOID"),
            (Self::Property(property), "IMapPropertyDef") => {
                property.process_criteria = attr(start, "ProcessPropertyCriteria");
            }
            (Self::Enum(uid, entry), "IObject") => {
                *uid = text("UID");
                entry.name = text("Name");
                entry.description = text("Description");
            }
            (Self::Enum(_, entry), "IMapEnumDef") => {
                entry.number =
                    attr(start, "MapEnumNumber").and_then(|number| number.trim().parse().ok());
            }
            (Self::Rel(rel), "IRel") => {
                rel.uid1 = text("UID1");
                rel.uid2 = text("UID2");
                rel.def_uid = text("DefUID");
            }
            (Self::Rel(rel), "IMapRel") => {
                rel.end2_map_value = text("End2MapValue");
                rel.end_edge_def = text("EndEdgeDef");
            }
            _ => {}
        }
    }

    /// File the finished block: a definition into `map`, a rel into its
    /// count and, when it is one that joins blocks, into `links`.
    fn close(self, map: &mut DataMap, links: &mut Links) {
        match self {
            Self::Class(class) => map.classes.push(class),
            Self::Property(property) => map.properties.push(property),
            Self::Enum(uid, entry) => {
                map.enums.insert(uid, entry);
            }
            Self::Rel(rel) => {
                *map.rel_counts.entry(rel.def_uid.clone()).or_insert(0) += 1;
                match rel.def_uid.as_str() {
                    "MapClassToClass" => links.class_to_class.push((rel.uid1, rel.uid2)),
                    "MapClassMapProperties" => links.class_properties.push((rel.uid1, rel.uid2)),
                    "MapPropertyToProperty" => links.property_targets.push((
                        rel.uid1,
                        MapPropertyTarget {
                            property: rel.uid2,
                            publish_class: rel.end2_map_value,
                            edge: rel.end_edge_def,
                        },
                    )),
                    "MapPropertyMapEnumList" => links.property_lists.push((rel.uid1, rel.uid2)),
                    "MapEnumMapEnum" => {
                        map.enum_parents.entry(rel.uid2).or_insert(rel.uid1);
                    }
                    "MapEnumListMapEnum" => {
                        map.enum_list_members
                            .entry(rel.uid1)
                            .or_default()
                            .push(rel.uid2);
                    }
                    "MapEnumToEnum" => {
                        map.enum_targets.entry(rel.uid1).or_default().push(rel.uid2);
                    }
                    "EnumToMapEnum" => map.enum_retrievals.push((rel.uid1, rel.uid2)),
                    _ => {}
                }
            }
            Self::Other => {}
        }
    }
}

impl Links {
    /// Hang each rel on the block it starts at. A rel whose map class or
    /// map property is not a block of this file stays counted in
    /// [`DataMap::rel_counts`] and is attached to nothing.
    fn attach(self, map: &mut DataMap) {
        let class_at: HashMap<String, usize> = map
            .classes
            .iter()
            .enumerate()
            .map(|(index, class)| (class.uid.clone(), index))
            .collect();
        let property_at: HashMap<String, usize> = map
            .properties
            .iter()
            .enumerate()
            .map(|(index, property)| (property.uid.clone(), index))
            .collect();
        for (class, publish_class) in self.class_to_class {
            if let Some(&index) = class_at.get(&class) {
                map.classes[index].publish_classes.push(publish_class);
            }
        }
        for (class, property) in self.class_properties {
            if let Some(&index) = property_at.get(&property) {
                let owner = &mut map.properties[index].class_uid;
                if owner.is_empty() {
                    *owner = class;
                }
            }
        }
        for (property, target) in self.property_targets {
            if let Some(&index) = property_at.get(&property) {
                map.properties[index].targets.push(target);
            }
        }
        for (property, list) in self.property_lists {
            if let Some(&index) = property_at.get(&property) {
                map.properties[index].value_list.get_or_insert(list);
            }
        }
    }
}

fn parse_datamap_xml(bytes: &[u8]) -> Result<DataMap, DataMapError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut map = DataMap::default();
    let mut links = Links::default();
    // `<Container>` is depth 1, its blocks depth 2, their elements depth 3.
    let mut depth = 0usize;
    let mut block: Option<Block> = None;
    let mut in_list = false;
    let mut in_unit_list = false;
    let mut root_closed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_) | Event::Empty(_)) if root_closed => {
                return Err(DataMapError::Xml(format!(
                    "SPPIDDataMap.xml has an element after its root closes, at byte {}",
                    reader.buffer_position()
                )));
            }
            Ok(Event::Empty(_)) if depth == 0 => root_closed = true,
            Ok(Event::Start(start)) => {
                let qname = start.name();
                let name = local_name(qname.as_ref());
                depth += 1;
                if depth == 2 {
                    block = Some(Block::open(name));
                } else if let Some(block) = block.as_mut() {
                    block.element(name, &start);
                }
                if name == "SPMapEnumListDef" {
                    in_list = true;
                } else if name == "SPMapUoMListDef" {
                    in_unit_list = true;
                } else if name == "IObject" && in_list {
                    remember_list(&mut map.enum_list_names, &start);
                } else if name == "IObject" && in_unit_list {
                    remember_list(&mut map.unit_list_names, &start);
                } else if name == "IRel" {
                    remember_enum(&mut map, &start);
                }
            }
            Ok(Event::Empty(start)) => {
                let qname = start.name();
                let name = local_name(qname.as_ref());
                if depth >= 2 {
                    if let Some(block) = block.as_mut() {
                        block.element(name, &start);
                    }
                }
                if name == "SPMapEnumListDef" || name == "SPMapUoMListDef" {
                    // A list with no child is not a name we can keep.
                } else if name == "IObject" && in_list {
                    remember_list(&mut map.enum_list_names, &start);
                } else if name == "IObject" && in_unit_list {
                    remember_list(&mut map.unit_list_names, &start);
                } else if name == "IRel" {
                    remember_enum(&mut map, &start);
                }
            }
            Ok(Event::End(end)) => {
                let qname = end.name();
                let name = local_name(qname.as_ref());
                if name == "SPMapEnumListDef" {
                    in_list = false;
                } else if name == "SPMapUoMListDef" {
                    in_unit_list = false;
                }
                if depth == 2 {
                    if let Some(block) = block.take() {
                        block.close(&mut map, &mut links);
                    }
                }
                depth = depth.saturating_sub(1);
                root_closed = depth == 0;
            }
            Ok(Event::Eof) if root_closed => break,
            Ok(Event::Eof) if depth == 0 => {
                return Err(DataMapError::Xml(
                    "SPPIDDataMap.xml has no root element".to_string(),
                ));
            }
            Ok(Event::Eof) => {
                return Err(DataMapError::Xml(format!(
                    "SPPIDDataMap.xml ends at byte {} with {depth} element(s) still open; \
                     the file is cut short",
                    reader.buffer_position()
                )));
            }
            Err(error) => return Err(DataMapError::Xml(error.to_string())),
            _ => {}
        }
    }
    links.attach(&mut map);
    Ok(map)
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn remember_list(names: &mut HashMap<String, String>, start: &BytesStart<'_>) {
    let Some(uid) = attr(start, "UID") else {
        return;
    };
    let Some(name) = attr(start, "Name") else {
        return;
    };
    names.insert(uid, name);
}

fn remember_enum(map: &mut DataMap, start: &BytesStart<'_>) {
    if attr(start, "DefUID").as_deref() != Some("MapEnumToEnum") {
        return;
    }
    let Some(uid1) = attr(start, "UID1") else {
        return;
    };
    let Some(uid2) = attr(start, "UID2") else {
        return;
    };
    map.enum_to_enum.insert(uid1, publish_guid(&uid2));
}

fn attr(start: &BytesStart<'_>, key: &str) -> Option<String> {
    start.attributes().flatten().find_map(|attribute| {
        if attribute.key.as_ref() == key {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    })
}

fn publish_guid(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('@') {
        return format!("@{rest}");
    }
    if trimmed.starts_with('{') {
        format!("@{trimmed}")
    } else {
        format!("@{{{trimmed}}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fluid_code_is_published_only_when_codelist_19_maps_it_to_a_standard_guid() {
        let xml = br#"
            <Container>
              <Rel>
                <IRel UID1="SP_19_20" UID2="{663C8387-FD73-449D-9ABF-BB3ECC4037BE}" DefUID="MapEnumToEnum"/>
              </Rel>
              <Rel>
                <IRel UID1="SP_19_10022" UID2="EE_PCDEC_1" DefUID="MapEnumToEnum"/>
              </Rel>
            </Container>
        "#;
        let map = parse_datamap_xml(xml).expect("xml");
        assert_eq!(
            fluid_code_guid(&map, 20),
            Some("@{663C8387-FD73-449D-9ABF-BB3ECC4037BE}")
        );
        assert_eq!(fluid_code_guid(&map, 10022), None);
        assert_eq!(fluid_code_guid(&map, 7), None);
    }

    #[test]
    fn map_enum_and_list_names_are_read() {
        let xml = br#"
            <Container>
              <SPMapEnumListDef>
                <IObject UID="SP_20" Name="Fluid System" Description="Fluid"/>
              </SPMapEnumListDef>
              <Rel>
                <IRel UID1="SP_20_7" UID2="{A729B087-24B7-4418-A185-53CAA90C2286}" DefUID="MapEnumToEnum"/>
              </Rel>
              <Rel>
                <IRel UID1="SP_14_10" UID2="EE68D" DefUID="MapEnumToEnum"/>
              </Rel>
            </Container>
        "#;
        let map = parse_datamap_xml(xml).expect("xml");
        assert_eq!(
            map.enum_list_names.get("SP_20").map(String::as_str),
            Some("Fluid System")
        );
        assert_eq!(
            fluid_system_guid(&map, 7),
            Some("@{A729B087-24B7-4418-A185-53CAA90C2286}")
        );
        assert_eq!(
            map.enum_to_enum.get("SP_14_10").map(String::as_str),
            Some("@{EE68D}")
        );
    }

    // A codelist entry is found by its list and its number, not by its
    // `UID`, and a schema enum is retrieved-only when no rel publishes to it.
    #[test]
    fn codelist_entries_their_targets_and_retrievals_are_read() {
        let xml = br#"
            <Container>
              <Rel><IRel UID1="L" UID2="L_a" DefUID="MapEnumListMapEnum"/></Rel>
              <SPMapEnumDef>
                <IObject UID="L_a" Name="New" Description="Brand new"/>
                <IMapObject ToolOID="SPEN_L_a"/>
                <IMapEnumDef MapEnumNumber="2"/>
              </SPMapEnumDef>
              <SPMapEnumDef>
                <IObject UID="L_b" Name="Old"/>
                <IMapEnumDef MapEnumNumber="x"/>
              </SPMapEnumDef>
              <Rel><IRel UID1="L" UID2="L_b" DefUID="MapEnumListMapEnum"/></Rel>
              <Rel><IRel UID1="L_a" UID2="EE1" DefUID="MapEnumToEnum"/></Rel>
              <Rel><IRel UID1="L_a" UID2="EE2" DefUID="MapEnumToEnum"/></Rel>
              <Rel><IRel UID1="EE1" UID2="L_a" DefUID="EnumToMapEnum"/></Rel>
              <Rel><IRel UID1="Brand" UID2="L_a" DefUID="EnumToMapEnum"/></Rel>
              <Rel><IRel UID1="EE2" UID2="L_b" DefUID="EnumToMapEnum"/></Rel>
            </Container>
        "#;
        let map = parse_datamap_xml(xml).expect("xml");
        assert_eq!(
            map.enums.get("L_a"),
            Some(&MapEnum {
                name: "New".to_string(),
                description: "Brand new".to_string(),
                number: Some(2),
            })
        );
        assert_eq!(map.enums.get("L_b").map(|entry| entry.number), Some(None));
        assert_eq!(map.enum_list_members.get("L").map(Vec::len), Some(2));
        assert_eq!(map.enum_numbered("L", 2), Some("L_a"));
        assert_eq!(map.enum_numbered("L", 3), None);
        assert_eq!(map.enum_numbered("M", 2), None);
        assert_eq!(map.enum_target("L", 2).as_deref(), Some("@EE1"));
        assert_eq!(
            map.enum_to_enum.get("L_a").map(String::as_str),
            Some("@{EE2}")
        );
        assert_eq!(map.enum_retrievals.len(), 3);
        let retrieved = map.enums_retrieved_only();
        assert_eq!(retrieved.get("L_a"), Some(&vec!["Brand"]));
        assert_eq!(retrieved.get("L_b"), None);
    }

    // The class and property maps on a document small enough to read: a rel
    // that comes before the blocks it joins, a property published two ways,
    // one that states no conversion at all, and a rel whose property is not
    // a block of the file.
    #[test]
    fn classes_properties_and_the_rels_between_them_are_read() {
        let xml = br#"
            <Container>
              <Rel>
                <IObject UID="C1.PIDThing"/>
                <IRel UID1="C1" UID2="PIDThing" DefUID="MapClassToClass" IsRequired="True"/>
              </Rel>
              <SPMapClassDef>
                <IObject UID="C1" Name="Thing" Description=""/>
                <IMapObject ToolOID="SPMC_Thing" OnlyInMapFile="False"/>
                <IMapClassDef SelectClassCriteria="" ProcessClassCriteria="EFAdapter.ThingPublisher,False"/>
              </SPMapClassDef>
              <SPMapPropertyDef>
                <IObject UID="P1" Name="Size" Description="Size"/>
                <IMapObject ToolOID="SPMP_Thing_Size"/>
                <IMapPropDef/>
                <IMapPropertyDef SelectPropertyCriteria="" ProcessPropertyCriteria=""/>
              </SPMapPropertyDef>
              <SPMapPropertyDef>
                <IObject UID="P2" Name="Kind"/>
                <IMapObject ToolOID="SPMP_Thing_Kind"/>
                <IMapPropDef/>
                <IMapPropertyDef/>
              </SPMapPropertyDef>
              <SPMapUoMListDef>
                <IObject UID="SPMU_1" Name="Distance"/>
                <IMapObject ToolOID="SPUOM_1"/>
                <IMapEnumListDef/>
                <IMapUoMListDef/>
              </SPMapUoMListDef>
              <Rel>
                <IObject UID="r1"/>
                <IRel UID1="C1" UID2="P1" DefUID="MapClassMapProperties"/>
              </Rel>
              <Rel>
                <IObject UID="r2"/>
                <IRel UID1="C1" UID2="P2" DefUID="MapClassMapProperties"/>
              </Rel>
              <Rel>
                <IObject UID="r3"/>
                <IRel UID1="P1" UID2="ThingSize" DefUID="MapPropertyToProperty"/>
                <IMapRel End2MapValue="PIDThing"/>
              </Rel>
              <Rel>
                <IObject UID="r4"/>
                <IRel UID1="P1" UID2="PointSize" DefUID="MapPropertyToProperty"/>
                <IMapRel End2MapValue="PIDPoint" EndEdgeDef="PointCollection_1"/>
              </Rel>
              <Rel>
                <IObject UID="r5"/>
                <IRel UID1="P1" UID2="SPMU_1" DefUID="MapPropertyMapEnumList"/>
              </Rel>
              <Rel>
                <IObject UID="r6"/>
                <IRel UID1="SP_2_1" UID2="SP_1_7" DefUID="MapEnumMapEnum"/>
              </Rel>
              <Rel>
                <IObject UID="r7"/>
                <IRel UID1="P9" UID2="Lost" DefUID="MapPropertyToProperty"/>
                <IMapRel End2MapValue="PIDThing"/>
              </Rel>
            </Container>
        "#;
        let map = parse_datamap_xml(xml).expect("xml");

        assert_eq!(map.classes.len(), 1);
        let class = &map.classes[0];
        assert_eq!(
            (
                class.uid.as_str(),
                class.name.as_str(),
                class.tool_oid.as_str()
            ),
            ("C1", "Thing", "SPMC_Thing")
        );
        assert_eq!(class.process_criteria, "EFAdapter.ThingPublisher,False");
        assert_eq!(class.publish_classes, ["PIDThing"]);

        assert_eq!(map.properties.len(), 2);
        let size = &map.properties[0];
        assert_eq!(
            (
                size.uid.as_str(),
                size.name.as_str(),
                size.tool_oid.as_str()
            ),
            ("P1", "Size", "SPMP_Thing_Size")
        );
        assert_eq!(size.class_uid, "C1");
        assert_eq!(
            size.targets,
            [
                MapPropertyTarget {
                    property: "ThingSize".to_string(),
                    publish_class: "PIDThing".to_string(),
                    edge: String::new(),
                },
                MapPropertyTarget {
                    property: "PointSize".to_string(),
                    publish_class: "PIDPoint".to_string(),
                    edge: "PointCollection_1".to_string(),
                },
            ]
        );
        assert_eq!(size.value_list.as_deref(), Some("SPMU_1"));
        assert_eq!(size.process_criteria.as_deref(), Some(""));
        let kind = &map.properties[1];
        assert_eq!(kind.class_uid, "C1");
        assert!(kind.targets.is_empty());
        assert_eq!(kind.value_list, None);
        assert_eq!(kind.process_criteria, None);

        assert_eq!(
            map.unit_list_names.get("SPMU_1").map(String::as_str),
            Some("Distance")
        );
        assert!(map.enum_list_names.is_empty());
        assert_eq!(
            map.enum_parents.get("SP_1_7").map(String::as_str),
            Some("SP_2_1")
        );
        let counts: Vec<(&str, usize)> = map
            .rel_counts
            .iter()
            .map(|(def_uid, count)| (def_uid.as_str(), *count))
            .collect();
        assert_eq!(
            counts,
            [
                ("MapClassMapProperties", 2),
                ("MapClassToClass", 1),
                ("MapEnumMapEnum", 1),
                ("MapPropertyMapEnumList", 1),
                ("MapPropertyToProperty", 3),
            ]
        );

        let sources = map.sources("PIDThing", "ThingSize");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].class.map(|class| class.name.as_str()),
            Some("Thing")
        );
        assert_eq!(sources[0].property.name, "Size");
        assert_eq!(map.sources("PIDPoint", "PointSize").len(), 1);
        assert!(map.sources("PIDThing", "PointSize").is_empty());
        // The rel from `P9` is counted and hangs on no property.
        assert!(map.sources("PIDThing", "Lost").is_empty());
    }

    // A map cut short reads as fewer blocks and rels than the plant wrote,
    // and a publish that followed it would leave out what the lost part
    // mapped. Every cut before the root closes, at every byte, is refused;
    // the whole document still reads.
    #[test]
    fn a_map_cut_short_anywhere_before_its_root_closes_is_refused() {
        let xml: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<Container>
  <SPMapEnumListDef>
    <IObject UID="SP_20" Name="Fluid System"/>
  </SPMapEnumListDef>
  <Rel>
    <IRel UID1="SP_20_7" UID2="{A729B087-24B7-4418-A185-53CAA90C2286}" DefUID="MapEnumToEnum"/>
  </Rel>
  <Rel><IRel UID1="C1" UID2="PIDThing" DefUID="MapClassToClass"/></Rel>
</Container>
"#;
        let whole = parse_datamap_xml(xml).expect("the whole map reads");
        assert_eq!(whole.rel_counts.values().sum::<usize>(), 2);
        let root_closed = xml.len() - 1;
        let accepted: Vec<usize> = (0..root_closed)
            .filter(|&cut| parse_datamap_xml(&xml[..cut]).is_ok())
            .collect();
        assert!(
            accepted.is_empty(),
            "{} of {root_closed} cuts read as a map, the first at byte {:?}",
            accepted.len(),
            accepted.first()
        );
        let before_root_end = root_closed - "</Container>".len();
        assert_eq!(
            parse_datamap_xml(&xml[..before_root_end])
                .expect_err("a cut map")
                .to_string(),
            format!(
                "SPPIDDataMap.xml ends at byte {before_root_end} with 1 element(s) still open; \
                 the file is cut short"
            )
        );
    }

    #[test]
    fn the_committed_map_cut_short_is_refused() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file/backup-test/DWG-0202GP06-01_p/RefData~4~804.zip");
        let zip = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let xml = sppid_datamap_entry(&zip).expect("the entry");
        let find = |needle: &[u8], from: usize| {
            from + xml[from..]
                .windows(needle.len())
                .position(|window| window == needle)
                .expect("a needle")
        };
        let middle = xml.len() / 2;
        let rel_closed = find(b"</Rel>", middle) + b"</Rel>".len();
        let in_rel = find(b"UID1=\"", middle) + 3;
        let root_end = xml
            .windows(b"</Container>".len())
            .rposition(|window| window == b"</Container>")
            .expect("the root's end tag");
        for (what, cut) in [
            ("a tenth in", xml.len() / 10),
            ("halfway", middle),
            ("inside an attribute name", in_rel),
            ("between two rels", rel_closed),
            ("before the root's end tag", root_end),
            ("inside the root's end tag", root_end + 5),
            (
                "one byte short of the root closing",
                root_end + "</Container".len(),
            ),
        ] {
            assert!(
                parse_datamap_xml(&xml[..cut]).is_err(),
                "the map cut {what} (byte {cut} of {}) read as a map",
                xml.len()
            );
        }
    }

    /// The data map every backup package at hand carries, out of the
    /// committed package's reference-data archive: the `SPPIDDataMap.xml`
    /// entry of 3 678 496 bytes, SHA-256 `248c6a3b…06794548`.
    fn committed_map() -> DataMap {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file/backup-test/DWG-0202GP06-01_p/RefData~4~804.zip");
        read_datamap(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// `Item.Attribute` for each way `map` publishes `property` on `class`.
    fn spelled(map: &DataMap, class: &str, property: &str) -> Vec<String> {
        let mut found: Vec<String> = map
            .sources(class, property)
            .iter()
            .map(|source| {
                format!(
                    "{}.{}",
                    source.class.map_or("?", |class| class.name.as_str()),
                    source.property.name
                )
            })
            .collect();
        found.sort();
        found
    }

    #[test]
    fn the_committed_map_holds_these_blocks_and_rels() {
        let map = committed_map();
        let counts: Vec<(&str, usize)> = map
            .rel_counts
            .iter()
            .map(|(def_uid, count)| (def_uid.as_str(), *count))
            .collect();
        assert_eq!(
            counts,
            [
                ("ClassToMapClass", 174),
                ("EnumListToMapEnumList", 42),
                ("EnumToMapEnum", 1040),
                ("FromClassForMapRelDef", 1),
                ("MapClassMapProperties", 1818),
                ("MapClassToClass", 29),
                ("MapEnumListMapEnum", 3343),
                ("MapEnumListMapEnumList", 11),
                ("MapEnumListToEnumList", 53),
                ("MapEnumMapEnum", 1573),
                ("MapEnumToEnum", 1921),
                ("MapPropertyMapEnumList", 1022),
                ("MapPropertyToProperty", 694),
                ("MapSoftwareToolSchema", 1),
                ("PackageCollection", 1),
                ("PackageUMLViewDefs", 8),
                ("PropertyToMapProperty", 355),
                ("ToClassForMapRelDef", 1),
            ]
        );
        assert_eq!(map.rel_counts.values().sum::<usize>(), 12_087);

        assert_eq!(map.classes.len(), 31);
        assert_eq!(map.properties.len(), 1_818);
        // Every rel of the five kinds read further found the block it
        // starts at, so what hangs on the blocks adds up to the rel counts.
        assert_eq!(
            map.classes
                .iter()
                .map(|class| class.publish_classes.len())
                .sum::<usize>(),
            29
        );
        assert_eq!(
            map.properties
                .iter()
                .filter(|property| map.class(&property.class_uid).is_some())
                .count(),
            1_818
        );
        assert_eq!(
            map.properties
                .iter()
                .map(|property| property.targets.len())
                .sum::<usize>(),
            694
        );
        assert_eq!(
            map.properties
                .iter()
                .filter(|property| property.value_list.is_some())
                .count(),
            1_022
        );
        assert_eq!(map.enum_parents.len(), 1_573);
        assert_eq!(map.enum_to_enum.len(), 1_921);
        assert_eq!(map.enum_targets.len(), 1_921);
        assert_eq!(map.enums.len(), 3_131);
        assert_eq!(map.enum_list_members.len(), 117);
        assert_eq!(
            map.enum_list_members.values().map(Vec::len).sum::<usize>(),
            3_343
        );
        assert_eq!(map.enum_retrievals.len(), 1_040);
        assert_eq!(map.enum_list_names.len(), 102);
        assert_eq!(map.unit_list_names.len(), 15);
    }

    #[test]
    fn the_committed_map_publishes_these_item_types() {
        let map = committed_map();
        let published: BTreeMap<&str, Vec<&str>> = map
            .classes
            .iter()
            .map(|class| {
                (
                    class.name.as_str(),
                    class.publish_classes.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(published.len(), 31, "item type names are distinct");
        for (item_type, class) in [
            ("Vessel", "PIDProcessVessel"),
            ("Exchanger", "PIDHeatTransferEquipment"),
            ("EquipmentOther", "PIDProcessEquipment"),
            ("Mechanical", "PIDMechanicalEquipment"),
            ("ItemNote", "PIDNote"),
            ("PipeRun", "PIDPipingConnector"),
            ("PipingComp", "PIDPipingComponent"),
            ("OPC", "PIDBranchPoint"),
        ] {
            assert_eq!(published[item_type], [class], "{item_type}");
        }
        assert_eq!(published["Pipeline"], ["PIDPipeline", "IProcessPoint"]);
        let unpublished: Vec<&str> = published
            .iter()
            .filter(|(_, classes)| classes.is_empty())
            .map(|(item_type, _)| *item_type)
            .collect();
        assert_eq!(unpublished, ["ControlSysIO", "Equipment", "Revision"]);
        assert_eq!(
            map.class("SPMC_10")
                .map(|class| class.process_criteria.as_str()),
            Some("EFAdapter.PipeRunDataExchanger,True,11")
        );
    }

    // What `pid_load::published_attribute` takes from this file: where an
    // equipment item's `Description` and `Name` go.
    #[test]
    fn the_committed_map_sends_an_equipment_description_to_its_material_description() {
        let map = committed_map();
        assert_eq!(
            spelled(&map, "PIDProcessVessel", "LongMaterialDescription"),
            ["Vessel.Description"]
        );
        assert_eq!(
            spelled(&map, "PIDHeatTransferEquipment", "LongMaterialDescription"),
            ["Exchanger.Description"]
        );
        assert_eq!(
            spelled(&map, "PIDProcessEquipment", "LongMaterialDescription3"),
            ["EquipmentOther.Description"]
        );
        for (class, item_type) in [
            ("PIDProcessVessel", "Vessel"),
            ("PIDHeatTransferEquipment", "Exchanger"),
            ("PIDProcessEquipment", "EquipmentOther"),
            ("PIDMechanicalEquipment", "Mechanical"),
        ] {
            assert_eq!(
                spelled(&map, class, "Description"),
                [format!("{item_type}.Name")],
                "{class}"
            );
        }
        // The map lists these two attributes and publishes them as nothing.
        for tool_oid in ["SPMP_Mechanical_Description", "SPMP_ItemNote_Description"] {
            let property = map
                .properties
                .iter()
                .find(|property| property.tool_oid == tool_oid)
                .unwrap_or_else(|| panic!("{tool_oid} is not in the map"));
            assert!(property.targets.is_empty(), "{tool_oid}");
        }
    }

    #[test]
    fn the_committed_map_states_a_conversion_and_a_value_list_per_property() {
        let map = committed_map();
        let one = |class: &str, property: &str| {
            let sources = map.sources(class, property);
            assert_eq!(sources.len(), 1, "{class}.{property}");
            sources[0]
        };

        let sequence = one("PIDPipingConnector", "PipingConnectorSeqNo");
        assert_eq!(sequence.property.tool_oid, "SPMP_PipeRun_TagSequenceNo");
        assert_eq!(sequence.property.process_criteria.as_deref(), Some(""));
        assert_eq!(sequence.property.value_list, None);

        // A length with its unit: the list is a unit list, and the block
        // states no conversion.
        let thickness = one("PIDPipingConnector", "TotalInsulThick");
        assert_eq!(thickness.property.tool_oid, "SPMP_PipeRun_InsulThick");
        assert_eq!(thickness.property.process_criteria, None);
        let list = thickness.property.value_list.as_deref().expect("a list");
        assert_eq!(
            map.unit_list_names.get(list).map(String::as_str),
            Some("Thickness")
        );
        let slope = one("PIDPipingConnector", "SlopedPipingAngle");
        assert_eq!(slope.property.tool_oid, "SPMP_PipeRun_Slope");
        let list = slope.property.value_list.as_deref().expect("a list");
        assert_eq!(
            map.unit_list_names.get(list).map(String::as_str),
            Some("Angle")
        );

        // An enum: the list is an enum list and the adapter converts.
        let status = one("PIDPipingConnector", "ConstructionStatus");
        assert_eq!(
            status.property.process_criteria.as_deref(),
            Some("EFAdapter.EnumConversions")
        );
        let list = status.property.value_list.as_deref().expect("a list");
        assert_eq!(list, "SP_93");
        assert_eq!(
            map.enum_list_names.get(list).map(String::as_str),
            Some("ConstructionStatus")
        );

        // The seventeen spellings of the conversion, and the two ways a
        // block states none.
        let mut conversions = BTreeMap::<Option<&str>, usize>::new();
        for property in &map.properties {
            *conversions
                .entry(property.process_criteria.as_deref())
                .or_default() += 1;
        }
        let conversions: Vec<(Option<&str>, usize)> = conversions.into_iter().collect();
        assert_eq!(
            conversions,
            [
                (None, 1_377),
                (Some(""), 110),
                (Some("EFAdapter.BoolToEnumConv"), 2),
                (Some("EFAdapter.CompFlowDirConv"), 2),
                (Some("EFAdapter.EnumConversions"), 215),
                (Some("EFAdapter.EnumConversions,<ComparePID>"), 6),
                (Some("EFAdapter.EnumToBoolConv"), 28),
                (Some("EFAdapter.EnumToStringConv"), 13),
                (Some("EFAdapter.EnumToStringConv,<Description>"), 2),
                (Some("EFAdapter.EnumToStringConv,<Name>"), 9),
                (Some("EFAdapter.NominalDiameterConv"), 7),
                (Some("EFAdapter.PortFlowDirectionConv"), 2),
                (Some("EFAdapter.PpConnNameConv, <ItemTag>"), 1),
                (
                    Some(
                        "EFAdapter.PpConnNameConv, <ItemTag>-<NominalDiameter>-\
                         <PipingMaterialsClass>-<InsulPurpose>-<InsulThick>"
                    ),
                    1
                ),
                (Some("EFAdapter.TagPrefixConv"), 4),
                (Some("EFAdapter.UoMToDoubleConv"), 30),
                (Some("EFAdapter.UoMToEnumConv"), 9),
            ]
        );
    }

    // The codelists `publish::codelist_values` spells a dump cell by: how
    // many entries each holds, how many of them `MapEnumToEnum` publishes,
    // and an entry of each with its name, description and target.
    #[test]
    fn the_committed_map_spells_these_codelists() {
        let map = committed_map();
        let entry = |list: &str, number: i64| {
            let uid = map.enum_numbered(list, number).expect("an entry");
            let entry = &map.enums[uid];
            (
                entry.name.as_str(),
                entry.description.as_str(),
                map.enum_target(list, number),
            )
        };
        let mut sizes = Vec::new();
        for list in [
            "SP_93", "SP_22", "SP_25", "SP_35", "SP_28", "SP_29", "SP_17", "SP_14", "SP_47",
        ] {
            let members = &map.enum_list_members[list];
            let published = members
                .iter()
                .filter(|uid| map.enum_targets.contains_key(*uid))
                .count();
            sizes.push((map.enum_list_names[list].as_str(), members.len(), published));
        }
        assert_eq!(
            sizes,
            [
                ("ConstructionStatus", 14, 14),
                ("EquipmentType", 604, 604),
                ("PipingComponentType", 272, 119),
                ("NozzleType", 19, 19),
                ("InstrumentComponentType", 269, 136),
                ("InstrumentComponentClass", 11, 2),
                ("InstrumentLocation", 18, 1),
                ("LineType", 31, 12),
                ("FlowDirection", 5, 5),
            ]
        );
        let target = |text: &str| Some(text.to_string());
        assert_eq!(
            entry("SP_93", 2),
            (
                "New",
                "New",
                target("@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}")
            )
        );
        assert_eq!(
            entry("SP_22", 100),
            (
                "1 diameter, 1 chamber, elliptical head reactor",
                "1D 1C 2:1 reactor",
                target("@EE7BA")
            )
        );
        assert_eq!(
            entry("SP_25", 18),
            (
                "Gate valve",
                "Gate valve",
                target("@{5F7F9234-BC29-11D6-BDBC-00104BCC2B69}")
            )
        );
        assert_eq!(
            entry("SP_35", 38),
            (
                "Flanged Nozzle",
                "",
                target("@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}")
            )
        );
        assert_eq!(
            entry("SP_28", 101),
            ("Automatic recirculation valve", "", None)
        );
        assert_eq!(entry("SP_28", 103), ("Regulator", "", target("@EE426")));
        assert_eq!(
            entry("SP_29", 1),
            (
                "Control valves and regulators",
                "Control valves and regulators",
                target("@EE3F8")
            )
        );
        assert_eq!(entry("SP_17", 2), ("Field", "", target("@EE31D")));
        assert_eq!(
            entry("SP_14", 10),
            ("Pneumatic binary", "PB", target("@EE68D"))
        );
        assert_eq!(
            entry("SP_47", 2),
            ("End 1 is upstream (Inlet)", "", target("@EE872"))
        );

        // The schema enums the adapter only reads back: 94 of them, onto 83
        // entries, nine of which take more than one. Each construction
        // status the seven reference drawings carry takes exactly one.
        let retrieved = map.enums_retrieved_only();
        assert_eq!(retrieved.len(), 83);
        assert_eq!(retrieved.values().map(Vec::len).sum::<usize>(), 94);
        assert_eq!(
            retrieved.values().filter(|found| found.len() > 1).count(),
            9
        );
        for (number, only) in [
            (1, Some("{B72FE633-954A-40BA-AB15-CA7A63796594}")),
            (2, Some("NewConstruction")),
            (3, Some("ExistingConstruction")),
            (4, None),
            (8, Some("FutureConstruction")),
            (14, None),
        ] {
            let uid = map.enum_numbered("SP_93", number).expect("a status");
            assert_eq!(
                retrieved.get(uid).map(Vec::as_slice),
                only.as_ref().map(std::slice::from_ref),
                "SP_93 {number}"
            );
        }
    }

    // What the file says beyond one attribute to one property of the item's
    // own class.
    #[test]
    fn the_committed_map_has_these_other_shapes() {
        let map = committed_map();
        let item_type = |property: &MapProperty| {
            map.class(&property.class_uid)
                .map_or("?", |class| class.name.as_str())
        };

        // One attribute published as two properties.
        let twice: Vec<&MapProperty> = map
            .properties
            .iter()
            .filter(|property| property.targets.len() > 1)
            .collect();
        assert_eq!(twice.len(), 1);
        assert_eq!(twice[0].tool_oid, "SPMP_PipeRun_ScheduleOrThickness");
        assert_eq!(
            twice[0]
                .targets
                .iter()
                .map(|target| target.property.as_str())
                .collect::<Vec<_>>(),
            ["ScheduleOrThickness", "ScheduleThickness2"]
        );

        // A rel names the class it publishes on. For 637 of the 694 that is
        // a publish class of the owning item type; the other 57 are a pipe
        // run's process properties, stated on the process point.
        let mut elsewhere = BTreeMap::<(&str, &str), usize>::new();
        let mut with_edge = BTreeMap::<(&str, &str, &str), usize>::new();
        for property in &map.properties {
            let own = map
                .class(&property.class_uid)
                .map_or(&[][..], |class| class.publish_classes.as_slice());
            for target in &property.targets {
                if !own.contains(&target.publish_class) {
                    *elsewhere
                        .entry((item_type(property), target.publish_class.as_str()))
                        .or_default() += 1;
                }
                if !target.edge.is_empty() {
                    *with_edge
                        .entry((
                            item_type(property),
                            target.publish_class.as_str(),
                            target.edge.as_str(),
                        ))
                        .or_default() += 1;
                }
            }
        }
        assert_eq!(
            elsewhere.into_iter().collect::<Vec<_>>(),
            [(("PipeRun", "PIDProcessPoint"), 57)]
        );
        assert_eq!(
            with_edge.into_iter().collect::<Vec<_>>(),
            [(
                ("DuctRun", "PIDDuctConnector", "ProcessPointCollection_21"),
                48
            )]
        );

        // A tool id is not a key: ten are carried by two blocks each. Nine
        // are an attribute the map lists twice for one item type, the
        // tenth is on two item types.
        let mut blocks_of = BTreeMap::<&str, Vec<&str>>::new();
        for property in &map.properties {
            blocks_of
                .entry(property.tool_oid.as_str())
                .or_default()
                .push(item_type(property));
        }
        assert_eq!(blocks_of.len(), 1_808);
        let shared: Vec<(&str, Vec<&str>)> = blocks_of
            .into_iter()
            .filter(|(_, item_types)| item_types.len() > 1)
            .collect();
        let listed_twice =
            |tool_oid: &'static str, owner: &'static str| (tool_oid, vec![owner, owner]);
        assert_eq!(
            shared,
            [
                listed_twice("SPMP_DuctNozzle_NozzleType", "DuctNozzle"),
                listed_twice("SPMP_EquipComponent_EquipmentType", "EquipComponent"),
                listed_twice("SPMP_EquipmentOther_EquipmentType", "EquipmentOther"),
                listed_twice("SPMP_Equipment_EquipmentType", "Equipment"),
                listed_twice("SPMP_Exchanger_EquipmentType", "Exchanger"),
                listed_twice("SPMP_Mechanical_EquipmentType", "Mechanical"),
                listed_twice("SPMP_Nozzle_NozzleType", "Nozzle"),
                (
                    "SPMP_PipeRun_ProcessDesign.Max.Temperature",
                    vec!["PipeRun", "DuctRun"]
                ),
                listed_twice("SPMP_PipingComp_PipingCompType", "PipingComp"),
                listed_twice("SPMP_Vessel_EquipmentType", "Vessel"),
            ]
        );

        // An equipment type sits under its subclass, the subclass under its
        // class, and the class under nothing.
        assert_eq!(
            map.enum_parents.get("SP_22_100").map(String::as_str),
            Some("SP_23_11")
        );
        assert_eq!(
            map.enum_parents.get("SP_23_11").map(String::as_str),
            Some("SP_24_2")
        );
        assert_eq!(map.enum_parents.get("SP_24_2"), None);
    }
}
