//! Published values the plant database stores as they are.
//!
//! Plan `docs/plans/2026-11-10-publish-export-completion.md` S7. For every
//! published item, each property of [`STORED_PROPERTIES`] is read out of the
//! item's own row of the dump: the data map names the attribute of the
//! item's type that the property is published from, and the dump's data
//! dictionary places that attribute in a table and a column
//! ([`super::placement::place_property`]). The row is the one whose `SP_ID`
//! is the item's UID.
//!
//! A value is written as the reference spells it. The map states the
//! conversion the vendor's adapter runs on an attribute
//! ([`crate::backup::MapProperty::process_criteria`]); a value is written
//! only when this stage runs the same one (see [`spell`]), and a cell it
//! cannot spell is counted and left out.
//!
//! A label on the sheet may already have given the item the property; the
//! label is then kept, and a dump that says otherwise is counted. On the
//! seven acceptance drawings, whose dump is their own revision, no label
//! disagrees with the dump. On `DWG-0202GP06-01`, whose package dump is a
//! later revision than the sheet and its reference, two runs' sequence
//! numbers do (`0401` / `0301` on the sheet and in the reference, `01` /
//! `565` in the dump): an item the sheet does not label cannot be checked
//! that way, and is written from the dump. The item's identity
//! ([`IDENTITY_PROPERTIES`]) is not: a dump that any label of the drawing
//! contradicts gives no item its name.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::oracle_exp::{backup_uid_key, nominal_spelling, ExpTables, StoredCell};
use super::pid_load::{PidPublishAttribute, PidPublishDrawing, PidPublishItem, PidValueSource};
use super::placement::{place_property, DumpColumn, Unplaced};
use crate::backup::{DataMap, MapProperty};

/// One published property this stage reads from the dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredProperty {
    /// The interface element that carries it (`IPipingSpecifiedItem`).
    pub interface: &'static str,
    /// The schema property (`PipingMaterialsClass`).
    pub property: &'static str,
}

/// The properties this stage reads, on whichever published class the map
/// publishes them.
pub const STORED_PROPERTIES: &[StoredProperty] = &[
    StoredProperty {
        interface: "IPipingSpecifiedItem",
        property: "PipingMaterialsClass",
    },
    StoredProperty {
        interface: "IInsulatedItem",
        property: "InsulSpec",
    },
    StoredProperty {
        interface: "INamedPipingConnector",
        property: "PipingConnectorSeqNo",
    },
    StoredProperty {
        interface: "IPipeCrossSectionItem",
        property: "NominalDiameter",
    },
    StoredProperty {
        interface: "IConnector",
        property: "RepresentationsAreAllZeroLength",
    },
    StoredProperty {
        interface: "IPIDTypical",
        property: "IsTypical",
    },
    StoredProperty {
        interface: "IInlineComponent",
        property: "IsFlowDirectional",
    },
    // The values the references state on an object the sheet's labels do
    // not name. The map places each in a table the dump holds, under an
    // item type that is the object's own (`Instrument`, `EquipmentOther` /
    // `Mechanical` / `Vessel`) or one the map spells and the dump does not
    // (`InstrumentInline`, read under `Instrument`).
    StoredProperty {
        interface: "INamedInstrument",
        property: "InstrTagSequenceNo",
    },
    StoredProperty {
        interface: "INamedInstrument",
        property: "MeasuredVariable",
    },
    StoredProperty {
        interface: "INamedEquipment",
        property: "EquipPrefix",
    },
    // `EquipmentOther.Description` in `T_ModelItem`. The interface is
    // stated only on the objects that carry the value, and only the
    // classes whose row already lists `ISpecifiedMatlItem`.
    StoredProperty {
        interface: "ISpecifiedMatlItem",
        property: "LongMaterialDescription3",
    },
    // `TrimSpec` in `T_Equipment`, the table the equipment types inherit
    // it from.
    StoredProperty {
        interface: "IEquipment",
        property: "EquipmentTrimSpec",
    },
    // `SP_SetPressure` in `T_InlineComp`. The cell is the number against
    // the unit (`12MPa`); [`space_before_unit`] writes the space the
    // references use (`12 MPa`).
    StoredProperty {
        interface: "IPressureReliefItem",
        property: "ReliefPressure",
    },
];

/// The item's own identity: written only from a dump that no label of the
/// drawing contradicts.
///
/// A tag is what a later revision renumbers. `DWG-0202GP06-01`'s package
/// dump is such a revision: the sheet's labels and its reference give two
/// runs sequence numbers the dump does not, and two instrument functions
/// tags without the `A3-` the dump gives them, and the dump tags a
/// component and the vessel the reference names nothing. A label checks
/// only the item it sits on, so where any label disagrees, no item takes
/// its identity from the dump.
///
/// `Description` is the second half of the same identity, and the map
/// sends it from the same place as `Name`: an equipment item's
/// `IObject.Description` is its item type's `Name` (`Vessel.Name`,
/// `EquipmentOther.Name`, `Mechanical.Name` → `T_PlantItem.Name`) while
/// its `Name` is the item type's `ItemTag`, and an off-page connector's
/// `Description` is `OPC.Description` (`T_ModelItem.Description`). The
/// two therefore stand or fall together: a drawing whose dump renumbers
/// the tags has no `Description` to state either.
pub const IDENTITY_PROPERTIES: &[StoredProperty] = &[
    StoredProperty {
        interface: "IObject",
        property: "Name",
    },
    StoredProperty {
        interface: "IObject",
        property: "Description",
    },
];

/// What [`attach_stored_values`] did to one drawing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredValues {
    /// Values written where no label gave the property.
    pub written: usize,
    /// Labels the dump agrees with.
    pub confirmed_labels: usize,
    /// Labels kept where the dump says otherwise.
    pub disagreeing_labels: usize,
    /// Cells left out because [`spell`] does not spell them.
    pub unspelled: usize,
    /// [`IDENTITY_PROPERTIES`] values no label gave, left out because a
    /// label of the drawing disagrees with the dump.
    pub withheld: usize,
    /// Placed columns this dump does not let the stage read, each with the
    /// reason, sorted.
    pub unread: BTreeSet<String>,
}

/// Spell one stored cell as the reference writes the property, under the
/// conversion the map names: none (`None` or empty) is the value as
/// stored, `EFAdapter.NominalDiameterConv` spells a code the way a port's
/// diameter is spelled ([`nominal_spelling`]: `80 mm`, `1 in`),
/// `EFAdapter.EnumToBoolConv` spells a code of the dump's Boolean list
/// (codelist 73: 1 `False`, 2 `True`), and `EFAdapter.CompFlowDirConv`
/// spells a component's flow direction (the map's `SP_47`) as whether it
/// has one: 1 `Undefined` is `False`, 2 `End 1 is upstream` and 3 `End 1
/// is downstream` are `True`. The references hold no component of 4
/// `Bi-Directional` or 5 `No Flow`, so neither is spelled.
///
/// `None` when the conversion is not one this stage runs, or the cell is
/// not a value it converts.
#[must_use]
pub fn spell(criteria: Option<&str>, cell: &StoredCell) -> Option<String> {
    match criteria.unwrap_or_default() {
        "" => Some(match cell {
            StoredCell::Text(text) => text.clone(),
            StoredCell::Integer(value) => value.to_string(),
            StoredCell::Number(number) => number.to_string(),
        }),
        "EFAdapter.NominalDiameterConv" => match cell {
            StoredCell::Integer(code) => nominal_spelling(*code),
            StoredCell::Text(_) | StoredCell::Number(_) => None,
        },
        "EFAdapter.EnumToBoolConv" => match cell {
            StoredCell::Integer(1) => Some("False".to_string()),
            StoredCell::Integer(2) => Some("True".to_string()),
            _ => None,
        },
        COMPONENT_FLOW_DIRECTION => match cell {
            StoredCell::Integer(1) => Some("False".to_string()),
            StoredCell::Integer(2 | 3) => Some("True".to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// The conversion that publishes `IsFlowDirectional` from the item's
/// `FlowDirection`.
const COMPONENT_FLOW_DIRECTION: &str = "EFAdapter.CompFlowDirConv";

/// `12MPa` as the references write a relief pressure, `12 MPa`.
///
/// The dump stores the number against the unit. Each of the six
/// `ReliefPressure` values on the seven acceptance drawings is `12MPa` in
/// `T_InlineComp.SP_SetPressure` and `12 MPa` in the reference. A text that
/// already has the space, or that is not a number followed by a unit, is
/// returned unchanged.
fn space_before_unit(text: &str) -> String {
    let bytes = text.as_bytes();
    let Some(index) = bytes.iter().position(u8::is_ascii_alphabetic) else {
        return text.to_string();
    };
    if index > 0
        && bytes[..index]
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b'.')
    {
        format!("{} {}", &text[..index], &text[index..])
    } else {
        text.to_string()
    }
}

/// What a row whose cell is NULL is spelled as under `criteria`: a
/// component that states no flow direction is not flow-directional. `None`
/// for every other conversion, which leaves such a row out.
fn spell_null(criteria: Option<&str>) -> Option<&'static str> {
    (criteria == Some(COMPONENT_FLOW_DIRECTION)).then_some("False")
}

/// The attribute of the item type the conversion reads, where it is not
/// the one the map names: the dictionary attributes no `IsFlowDirectional`
/// to a piping component or an instrument, and the adapter derives it from
/// the attribute the map lists beside it, `InlineComp.FlowDirection`, which
/// the dictionary places in `T_InlineComp`, in the item's own row.
fn attribute_read<'a>(criteria: Option<&str>, attribute: &'a str) -> &'a str {
    if criteria == Some(COMPONENT_FLOW_DIRECTION) {
        "InlineComp.FlowDirection"
    } else {
        attribute
    }
}

/// The column one property of one item type is read from, and the
/// conversion the map names for it.
type Source = Option<(DumpColumn, Option<String>)>;

/// Write [`STORED_PROPERTIES`] and [`IDENTITY_PROPERTIES`] on the published
/// items of `drawing` from the dump behind `tables`, keeping each item's
/// attributes ordered by interface and name.
///
/// A column the dump does not let the stage read is reported in
/// [`StoredValues::unread`], and the drawing is published without it.
pub fn attach_stored_values(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
    map: &DataMap,
) -> StoredValues {
    let mut report = StoredValues::default();
    let mut reader = Reader {
        tables,
        map,
        sources: HashMap::new(),
        columns: HashMap::new(),
        rows: HashMap::new(),
    };
    let mut identity = Vec::new();
    for (index, item) in drawing.items.iter_mut().enumerate() {
        for stored in STORED_PROPERTIES {
            if let Some((value, column)) = reader.read(item, stored, &mut report) {
                state(item, stored, value, column, true, &mut report);
            }
        }
        for stored in IDENTITY_PROPERTIES {
            if !states_identity(item, stored) {
                continue;
            }
            if let Some((value, column)) = reader.read(item, stored, &mut report) {
                identity.push((index, stored, value, column));
            }
        }
    }
    // A label of the drawing is what says whether the dump is this
    // revision. The drawing's own block is not one of them: its
    // `IObject.Name` is the file's name (the publish line derives it, and
    // a run given an 8.3 alias reads `IB031~1`), not a tag the sheet
    // letters, so it cannot contradict the register.
    let contradicted = report.disagreeing_labels > 0
        || identity.iter().any(|(index, stored, value, _)| {
            let item = &drawing.items[*index];
            item.class != Some("PIDDrawing")
                && label(item, stored).is_some_and(|label| label != value)
        });
    for (index, stored, value, column) in identity {
        state(
            &mut drawing.items[index],
            stored,
            value,
            column,
            !contradicted,
            &mut report,
        );
    }
    for item in &mut drawing.items {
        item.attributes
            .sort_by(|left, right| (left.interface, left.name).cmp(&(right.interface, right.name)));
    }
    report
}

/// The value a label already gives `item` for `stored`.
fn label<'a>(item: &'a PidPublishItem, stored: &StoredProperty) -> Option<&'a str> {
    item.attributes
        .iter()
        .find(|existing| existing.interface == stored.interface && existing.name == stored.property)
        .map(|existing| existing.value.as_str())
}

/// Whether `item`'s class states `stored` at all.
///
/// A note's `Description` is mapped by the plant's data map
/// (`PIDNote.Description`, from the item type's `Description`), but this
/// line publishes no note description — `pid_load::published_attribute`
/// answers `None` for `PIDNote` — and no reference drawing states one: a
/// note's block carries its uid and nothing else. Reading the column the
/// map sends it from would report a column the dump does not hold for a
/// value nothing asks for, so a note is passed over.
fn states_identity(item: &PidPublishItem, stored: &StoredProperty) -> bool {
    if stored.interface == "IObject" && stored.property == "Description" {
        return item.class != Some("PIDNote");
    }
    true
}

/// Hold `value` against the label that gives `item` the property, or write
/// it where none does and `write` allows.
fn state(
    item: &mut PidPublishItem,
    stored: &StoredProperty,
    value: String,
    column: DumpColumn,
    write: bool,
    report: &mut StoredValues,
) {
    match label(item, stored) {
        Some(label) if label == value => report.confirmed_labels += 1,
        Some(_) => report.disagreeing_labels += 1,
        None if !write => report.withheld += 1,
        None => {
            report.written += 1;
            item.attributes.push(PidPublishAttribute {
                interface: stored.interface,
                name: stored.property,
                value,
                source: PidValueSource::PlacedColumn(column),
            });
        }
    }
}

/// The dump's columns, read once each, and where each property of each
/// item type is placed in them.
struct Reader<'a> {
    tables: &'a ExpTables,
    map: &'a DataMap,
    sources: HashMap<(&'static str, &'static str, String), Source>,
    columns: HashMap<DumpColumn, Option<HashMap<String, StoredCell>>>,
    rows: HashMap<String, HashSet<String>>,
}

impl Reader<'_> {
    /// `stored` of `item` as the reference spells it, and the column it is
    /// read from; `None` when the dump states nothing this stage spells.
    fn read(
        &mut self,
        item: &PidPublishItem,
        stored: &StoredProperty,
        report: &mut StoredValues,
    ) -> Option<(String, DumpColumn)> {
        let class = item.class?;
        let (map, tables) = (self.map, self.tables);
        let (column, criteria) = self
            .sources
            .entry((class, stored.property, item.model_item_type.clone()))
            .or_insert_with(|| {
                source_of(map, tables, class, stored.property, &item.model_item_type)
            })
            .clone()?;
        let key = backup_uid_key(&item.uid);
        if let Some(template) = criteria.as_deref().and_then(name_template) {
            let name = self.compose(item, &key, template, report)?;
            return Some((name, column));
        }
        let value = if let Some(cell) = self.cell(&column, &key, report)? {
            let Some(value) = spell(criteria.as_deref(), &cell) else {
                report.unspelled += 1;
                return None;
            };
            if stored.property == "ReliefPressure" {
                space_before_unit(&value)
            } else {
                value
            }
        } else {
            let null = spell_null(criteria.as_deref())?;
            let held = self
                .rows
                .entry(column.table.clone())
                .or_insert_with(|| rows_of(tables, &column.table));
            if !held.contains(&key) {
                return None;
            }
            null.to_string()
        };
        Some((value, column))
    }

    /// The cell of `column` in the row `key`: `None` when the column is not
    /// read, `Some(None)` when the row holds none.
    fn cell(
        &mut self,
        column: &DumpColumn,
        key: &str,
        report: &mut StoredValues,
    ) -> Option<Option<StoredCell>> {
        let tables = self.tables;
        let cells = self
            .columns
            .entry(column.clone())
            .or_insert_with(|| match tables.stored_column(column) {
                Ok(Some(cells)) => Some(cells),
                Ok(None) => {
                    report
                        .unread
                        .insert(format!("{column}: the dump holds no such column"));
                    None
                }
                Err(error) => {
                    report.unread.insert(format!("{column}: {error}"));
                    None
                }
            })
            .as_ref()?;
        Some(cells.get(key).cloned())
    }

    /// A name composed by `template` out of the item's own attributes, each
    /// placed under the item's type. A part the map lists in an enum list
    /// is that enum's name (`25 mm`, `6"`, `1-1/2"`: the codelist's display
    /// text, not the published `6 in`); any other is spelled under the
    /// conversion the map names for it.
    ///
    /// A run whose `ItemTag` is NULL has no name: none of the 382 such
    /// connectors of the seven acceptance references is named. The two
    /// [`ONLY_EMPTY`] parts are written empty when NULL and leave the name
    /// out when stated. [`EMPTY_WHEN_NULL`] parts are written empty when
    /// NULL and spelled when stated: a reference name keeps the template's
    /// separators where the diameter or the materials class is empty.
    /// A part that is not spelled or not placed leaves the name out
    /// unspelled.
    fn compose(
        &mut self,
        item: &PidPublishItem,
        key: &str,
        template: &str,
        report: &mut StoredValues,
    ) -> Option<String> {
        let mut name = String::new();
        let mut rest = template;
        let mut first = true;
        while let Some(open) = rest.find('<') {
            name.push_str(&rest[..open]);
            let Some(close) = rest[open..].find('>').map(|close| open + close) else {
                report.unspelled += 1;
                return None;
            };
            let attribute = &rest[open + 1..close];
            rest = &rest[close + 1..];
            let Ok(column) = self
                .tables
                .dictionary()
                .place(&item.model_item_type, attribute)
            else {
                report.unspelled += 1;
                return None;
            };
            let property = attribute_property(self.map, &item.model_item_type, attribute);
            match self.cell(&column, key, report)? {
                Some(_) if ONLY_EMPTY.contains(&attribute) => {
                    report.unspelled += 1;
                    return None;
                }
                Some(cell) => {
                    let Some(part) = spell_part(self.map, property, &cell) else {
                        report.unspelled += 1;
                        return None;
                    };
                    name.push_str(&part);
                }
                None if first => return None,
                // The separators around an empty segment are the template's own text.
                None if ONLY_EMPTY.contains(&attribute) || EMPTY_WHEN_NULL.contains(&attribute) => {
                }
                None => {
                    report.unspelled += 1;
                    return None;
                }
            }
            first = false;
        }
        name.push_str(rest);
        Some(name)
    }
}

/// The conversion that composes a run's name out of its attributes, by the
/// template the map writes after it (`EFAdapter.PpConnNameConv,
/// <ItemTag>-<NominalDiameter>-<PipingMaterialsClass>-<InsulPurpose>-<InsulThick>`).
const COMPOSED_NAME: &str = "EFAdapter.PpConnNameConv";

/// The parts of a composed name the references show only empty: every one
/// of the 631 names the seven acceptance references compose ends in `--`.
/// The plant's dump states an insulation purpose on 21 runs (code 10003,
/// which the map names `IC`) and no thickness; no reference shows how a
/// stated one reads in a name.
const ONLY_EMPTY: [&str; 2] = ["InsulPurpose", "InsulThick"];

/// Parts of a composed name written as an empty segment when the cell is
/// NULL. Of the 631 run names the seven acceptance references state, 247
/// have no diameter (`{ItemTag}--{PipingMaterialsClass}--`) and two have
/// neither a diameter nor a materials class (`{ItemTag}----`). A stated
/// cell is spelled, the way a stated diameter is the enum's name (`6"`,
/// `1-1/2"`) rather than the published `6 in`.
const EMPTY_WHEN_NULL: [&str; 2] = ["NominalDiameter", "PipingMaterialsClass"];

/// The template of a [`COMPOSED_NAME`] conversion.
fn name_template(criteria: &str) -> Option<&str> {
    let (conversion, template) = criteria.split_once(',')?;
    (conversion.trim() == COMPOSED_NAME).then(|| template.trim())
}

/// The map's entry for `item_type`'s `attribute`.
fn attribute_property<'a>(
    map: &'a DataMap,
    item_type: &str,
    attribute: &str,
) -> Option<&'a MapProperty> {
    let class = map.classes.iter().find(|class| class.name == item_type)?;
    map.properties
        .iter()
        .find(|property| property.class_uid == class.uid && property.name == attribute)
}

/// One part of a composed name: a code of an enum list is the enum's name,
/// anything else is spelled under the attribute's own conversion.
fn spell_part(map: &DataMap, property: Option<&MapProperty>, cell: &StoredCell) -> Option<String> {
    let list = property.and_then(|property| property.value_list.as_deref());
    if let (Some(list), StoredCell::Integer(code)) = (list, cell) {
        if map.enum_list_members.contains_key(list) {
            let name = &map.enums.get(map.enum_numbered(list, *code)?)?.name;
            return (!name.is_empty()).then(|| name.clone());
        }
    }
    let criteria = property
        .and_then(|property| property.process_criteria.as_deref())
        .filter(|criteria| name_template(criteria).is_none());
    spell(criteria, cell)
}

/// The `SP_ID`s of the rows of `table`, folded like [`backup_uid_key`];
/// empty when the dump does not let the stage read them.
fn rows_of(tables: &ExpTables, table: &str) -> HashSet<String> {
    let id = DumpColumn {
        table: table.to_string(),
        column: "SP_ID".to_string(),
    };
    match tables.stored_column(&id) {
        Ok(Some(cells)) => cells.into_keys().collect(),
        Ok(None) | Err(_) => HashSet::new(),
    }
}

/// The placed column the map publishes `property` of `class` from, for an
/// item of type `item_type`.
///
/// The map may name an item type the dump does not have: it publishes an
/// inline instrument from `InstrumentInline`, which the `.pid` and the dump
/// call `Instrument`. The attribute is then placed under the item's own
/// type.
fn source_of(
    map: &DataMap,
    tables: &ExpTables,
    class: &str,
    property: &str,
    item_type: &str,
) -> Source {
    let placements = place_property(map, tables.dictionary(), class, property);
    let placement = placements
        .iter()
        .find(|placement| placement.item_type() == Some(item_type))
        .or_else(|| {
            placements
                .iter()
                .find(|placement| matches!(placement.column, Err(Unplaced::NoItemType { .. })))
        })?;
    let criteria = placement.source.property.process_criteria.clone();
    let named = placement.source.property.name.as_str();
    let attribute = attribute_read(criteria.as_deref(), named);
    let column = match &placement.column {
        Ok(column) if attribute == named && placement.item_type() == Some(item_type) => {
            column.clone()
        }
        _ => tables.dictionary().place(item_type, attribute).ok()?,
    };
    Some((column, criteria))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::publish::pid_load::load_publish_from_pid;

    fn repo(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    /// `DWG-0202GP06-01.pid` with its package's dump and data map.
    fn dwg() -> (PidPublishDrawing, ExpTables, DataMap) {
        let package = repo("test-file/backup-test/DWG-0202GP06-01_p");
        let drawing = load_publish_from_pid(&repo(
            "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        ))
        .expect("the committed drawing loads");
        let tables = ExpTables::read(&std::fs::read(package.join("Export.dmp")).unwrap())
            .expect("the committed dump reads");
        let map = crate::backup::read_datamap(&package.join("RefData~4~804.zip"))
            .expect("the committed data map reads");
        (drawing, tables, map)
    }

    fn dwg_with_stored_values() -> (PidPublishDrawing, StoredValues) {
        let (mut drawing, tables, map) = dwg();
        let report = attach_stored_values(&mut drawing, &tables, &map);
        (drawing, report)
    }

    fn stated<'a>(
        drawing: &'a PidPublishDrawing,
        uid: &str,
        name: &str,
    ) -> Option<&'a PidPublishAttribute> {
        drawing
            .items
            .iter()
            .find(|item| item.uid == uid)?
            .attributes
            .iter()
            .find(|attribute| attribute.name == name)
    }

    fn placed(table: &str, column: &str) -> PidValueSource {
        PidValueSource::PlacedColumn(DumpColumn {
            table: table.to_string(),
            column: column.to_string(),
        })
    }

    /// The six runs of the fixture are labelled `1.6AR12` on the sheet and
    /// the dump agrees, so they keep the label as their source; the three
    /// nozzles are labelled nothing and take the class from `T_Nozzle`.
    #[test]
    fn a_nozzle_states_its_piping_materials_class_from_its_own_row() {
        let (drawing, report) = dwg_with_stored_values();
        let connector = stated(
            &drawing,
            "4C2F232018CE42E8B7FFCABD553BDBC4",
            "PipingMaterialsClass",
        )
        .expect("the run states it");
        assert_eq!(connector.value, "1.6AR12");
        assert!(matches!(connector.source, PidValueSource::Label { .. }));
        let nozzle = stated(
            &drawing,
            "06F9755FE3FE408D8A1D6A5F3B38F2DF",
            "PipingMaterialsClass",
        )
        .expect("the nozzle states it");
        assert_eq!(nozzle.interface, "IPipingSpecifiedItem");
        assert_eq!(nozzle.value, "1.6AR12");
        assert_eq!(nozzle.source, placed("T_Nozzle", "PipingMaterialsClass"));
        // The package's dump is a later revision than the sheet: four labels
        // of the drawing disagree with it, six cells it holds are not spelled
        // by a conversion this stage runs, and the two identity values of an
        // item no label gives are withheld because of the disagreement. The
        // four properties the map places in a table the dump holds are
        // confirmed by the sheet's own labels where the sheet states them.
        assert_eq!(
            report,
            StoredValues {
                written: 29,
                confirmed_labels: 21,
                disagreeing_labels: 4,
                unspelled: 6,
                withheld: 3,
                ..StoredValues::default()
            }
        );
        for item in &drawing.items {
            assert!(
                item.attributes.windows(2).all(|pair| {
                    (pair[0].interface, pair[0].name) <= (pair[1].interface, pair[1].name)
                }),
                "{} keeps its attributes ordered",
                item.uid
            );
        }
    }

    /// The fixture's sheet is the revision of its reference and the
    /// package's dump a later one: four runs' sequence numbers agree, and
    /// the two the dump renumbered keep what the sheet and the reference
    /// say.
    #[test]
    fn a_sequence_number_the_sheet_labels_otherwise_keeps_the_label() {
        let (drawing, _) = dwg_with_stored_values();
        let sequence = |uid: &str| {
            let attribute = stated(&drawing, uid, "PipingConnectorSeqNo").expect("labelled");
            assert_eq!(attribute.interface, "INamedPipingConnector");
            assert!(matches!(attribute.source, PidValueSource::Label { .. }));
            attribute.value.as_str()
        };
        assert_eq!(sequence("4C2F232018CE42E8B7FFCABD553BDBC4"), "0101");
        assert_eq!(sequence("7042E6B0A1A944E390A08A5A44771C46"), "0401");
        assert_eq!(sequence("C7C3E09265174C91AC008F3CE02E9653"), "0301");
    }

    /// The diameters the sheet does not label: a nozzle's from `T_Nozzle`,
    /// a piping component's from `T_InlineComp`, the table its type
    /// inherits the attribute from.
    #[test]
    fn a_nominal_diameter_is_spelled_from_the_code_in_the_items_row() {
        let (drawing, _) = dwg_with_stored_values();
        let nozzle = stated(
            &drawing,
            "06F9755FE3FE408D8A1D6A5F3B38F2DF",
            "NominalDiameter",
        )
        .expect("the nozzle states it");
        assert_eq!(nozzle.interface, "IPipeCrossSectionItem");
        assert_eq!(nozzle.value, "80 mm");
        assert_eq!(nozzle.source, placed("T_Nozzle", "NominalDiameter"));
        let component = stated(
            &drawing,
            "C5A2865821394E019D7DAA0CAFE0490D",
            "NominalDiameter",
        )
        .expect("the component states it");
        assert_eq!(component.value, "100 mm");
        assert_eq!(component.source, placed("T_InlineComp", "NominalDiameter"));
    }

    /// Every one of the fixture's seven runs is stored `1`: none of them is
    /// drawn as zero-length pieces alone.
    #[test]
    fn every_run_states_whether_its_representations_are_all_zero_length() {
        let (drawing, _) = dwg_with_stored_values();
        let runs: Vec<_> = drawing
            .items
            .iter()
            .filter(|item| item.class == Some("PIDPipingConnector"))
            .collect();
        assert_eq!(runs.len(), 7);
        for run in runs {
            let flag = stated(&drawing, &run.uid, "RepresentationsAreAllZeroLength")
                .unwrap_or_else(|| panic!("{} states it", run.uid));
            assert_eq!(flag.interface, "IConnector");
            assert_eq!(flag.value, "False");
            assert_eq!(flag.source, placed("T_PipeRun", "SP_ConnectorsZeroLength"));
        }
    }

    #[test]
    fn a_label_the_dump_disagrees_with_is_kept_and_a_class_less_item_gets_nothing() {
        let (mut drawing, tables, map) = dwg();
        let label = drawing
            .items
            .iter_mut()
            .find(|item| item.uid == "4C2F232018CE42E8B7FFCABD553BDBC4")
            .and_then(|item| {
                item.attributes
                    .iter_mut()
                    .find(|attribute| attribute.name == "PipingMaterialsClass")
            })
            .unwrap();
        label.value = "B2".to_string();
        let nozzle = drawing
            .items
            .iter_mut()
            .find(|item| item.uid == "06F9755FE3FE408D8A1D6A5F3B38F2DF")
            .unwrap();
        nozzle.class = None;
        let report = attach_stored_values(&mut drawing, &tables, &map);
        // The vessel's trim spec is still written: clearing the nozzle's
        // class does not touch it.
        assert_eq!(report.written, 26);
        assert_eq!(report.disagreeing_labels, 5);
        let run = stated(
            &drawing,
            "4C2F232018CE42E8B7FFCABD553BDBC4",
            "PipingMaterialsClass",
        )
        .unwrap();
        assert_eq!(run.value, "B2");
        assert!(matches!(run.source, PidValueSource::Label { .. }));
        assert!(stated(
            &drawing,
            "06F9755FE3FE408D8A1D6A5F3B38F2DF",
            "PipingMaterialsClass"
        )
        .is_none());
    }

    /// Where each class's `InsulSpec` is read: a run's and a piping
    /// component's from their own tables; an inline instrument's, which the
    /// map publishes from `InstrumentInline`, from `T_Instrument`, the table
    /// of the type the `.pid` and the dump call it.
    #[test]
    fn an_insulation_spec_is_read_from_the_table_of_the_items_own_type() {
        let (_, tables, map) = dwg();
        let column = |class: &str, item_type: &str| {
            source_of(&map, &tables, class, "InsulSpec", item_type).map(|(column, criteria)| {
                assert_eq!(criteria, None, "{class}");
                column.to_string()
            })
        };
        assert_eq!(
            column("PIDPipingConnector", "PipeRun").as_deref(),
            Some("T_PipeRun.InsulationSpec")
        );
        assert_eq!(
            column("PIDPipingComponent", "PipingComp").as_deref(),
            Some("T_PipingComp.InsulationSpec")
        );
        assert_eq!(
            column("PIDInlineInstrument", "Instrument").as_deref(),
            Some("T_Instrument.InsulationSpec")
        );
        assert_eq!(column("PIDPipingConnector", "DuctRun"), None);
    }

    /// A property no label gives is one the diff holds a run that states it
    /// on some object to on every object; a labelled one is held by the
    /// labels' rule instead. The two identity properties are the exception
    /// the other way round: a label gives them where the sheet labels the
    /// item, and this stage writes them where it does not, so they are
    /// excused in a run without the dump and held in a run with it.
    #[test]
    fn the_properties_no_label_gives_are_the_ones_the_diff_holds_a_dump_run_to() {
        let unlabelled: BTreeSet<(&str, &str)> = STORED_PROPERTIES
            .iter()
            .chain(IDENTITY_PROPERTIES)
            .map(|stored| (stored.interface, stored.property))
            .filter(|pair| !crate::publish::catalog::LABELLED_ATTRIBUTES.contains(pair))
            .collect();
        let held: BTreeSet<(&str, &str)> = crate::publish::catalog::DUMP_STORED_ATTRIBUTES
            .into_iter()
            .collect();
        assert_eq!(unlabelled, held);
        // `IObject.Name` is excused in a run without the dump by the
        // vendor's composed-name rule (source 5 of `diff_exclusions`), not
        // by an exclusion class: the comment on
        // `EXCLUDED_ATTRIBUTE_CLASSES` keeps `Name` and `UID` out of every
        // member list so that a differing name stays in the diff.
        for (interface, attribute) in held {
            if (interface, attribute) == ("IObject", "Name") {
                continue;
            }
            assert!(
                crate::publish::catalog::excluded_attribute(interface, attribute).is_some(),
                "{interface}.{attribute} is excused in a run without the dump"
            );
        }
    }

    /// The fourteen items of the fixture the dump flags are the fourteen
    /// its reference flags, each stored `1`, `False`, in `T_ModelItem`, the
    /// table every item type inherits the attribute from.
    #[test]
    fn every_item_states_whether_it_is_a_typical_from_its_model_item_row() {
        let (drawing, _) = dwg_with_stored_values();
        let flags: Vec<_> = drawing
            .items
            .iter()
            .filter_map(|item| stated(&drawing, &item.uid, "IsTypical"))
            .collect();
        assert_eq!(flags.len(), 14);
        for flag in flags {
            assert_eq!(flag.interface, "IPIDTypical");
            assert_eq!(flag.value, "False");
            assert_eq!(flag.source, placed("T_ModelItem", "SP_IsTypical"));
        }
    }

    /// A piping component and an inline instrument both read their flow
    /// direction from their own row of `T_InlineComp`, through the
    /// `InlineComp.FlowDirection` the map lists beside `IsFlowDirectional`.
    #[test]
    fn whether_a_component_is_flow_directional_is_read_from_its_inline_row() {
        let (drawing, tables, map) = dwg();
        for (class, item_type) in [
            ("PIDPipingComponent", "PipingComp"),
            ("PIDInlineInstrument", "Instrument"),
        ] {
            let (column, criteria) =
                source_of(&map, &tables, class, "IsFlowDirectional", item_type)
                    .unwrap_or_else(|| panic!("{class} is placed"));
            assert_eq!(column.to_string(), "T_InlineComp.FlowDirection");
            assert_eq!(criteria.as_deref(), Some(COMPONENT_FLOW_DIRECTION));
        }
        let mut drawing = drawing;
        attach_stored_values(&mut drawing, &tables, &map);
        let flags: Vec<_> = drawing
            .items
            .iter()
            .filter_map(|item| stated(&drawing, &item.uid, "IsFlowDirectional"))
            .collect();
        assert_eq!(flags.len(), 2);
        for flag in flags {
            assert_eq!(flag.interface, "IInlineComponent");
            assert_eq!(flag.value, "False");
            assert_eq!(flag.source, placed("T_InlineComp", "FlowDirection"));
        }
    }

    #[test]
    fn a_flow_direction_is_spelled_as_whether_the_component_has_one() {
        let criteria = Some(COMPONENT_FLOW_DIRECTION);
        let spelled = |code| spell(criteria, &StoredCell::Integer(code));
        assert_eq!(spelled(1).as_deref(), Some("False"));
        assert_eq!(spelled(2).as_deref(), Some("True"));
        assert_eq!(spelled(3).as_deref(), Some("True"));
        assert_eq!(spelled(4), None);
        assert_eq!(spelled(5), None);
        assert_eq!(spell_null(criteria), Some("False"));
        assert_eq!(spell_null(Some("EFAdapter.EnumToBoolConv")), None);
        assert_eq!(spell_null(None), None);
    }

    #[test]
    fn a_value_with_no_conversion_is_written_as_stored() {
        let text = StoredCell::Text("B2".to_string());
        assert_eq!(spell(None, &text).as_deref(), Some("B2"));
        assert_eq!(spell(Some(""), &text).as_deref(), Some("B2"));
        assert_eq!(
            spell(None, &StoredCell::Integer(532)).as_deref(),
            Some("532")
        );
    }

    #[test]
    fn a_nominal_diameter_code_is_spelled_as_a_ports_is() {
        let criteria = Some("EFAdapter.NominalDiameterConv");
        assert_eq!(
            spell(criteria, &StoredCell::Integer(80)).as_deref(),
            Some("80 mm")
        );
        assert_eq!(
            spell(criteria, &StoredCell::Integer(5032)).as_deref(),
            Some("1 in")
        );
        assert_eq!(spell(criteria, &StoredCell::Integer(5001)), None);
        assert_eq!(spell(criteria, &StoredCell::Text("80".to_string())), None);
    }

    #[test]
    fn a_boolean_code_is_spelled_as_the_dumps_boolean_list_names_it() {
        let criteria = Some("EFAdapter.EnumToBoolConv");
        let (_, tables, _) = dwg();
        let booleans = tables.codelist_short_text(73).expect("codelist 73 reads");
        assert_eq!(booleans.len(), 2);
        for (code, text) in booleans {
            assert_eq!(
                spell(criteria, &StoredCell::Integer(code)).as_deref(),
                Some(text.as_str())
            );
        }
        assert_eq!(
            spell(criteria, &StoredCell::Integer(2)).as_deref(),
            Some("True")
        );
        assert_eq!(spell(criteria, &StoredCell::Integer(0)), None);
        assert_eq!(spell(criteria, &StoredCell::Text("True".to_string())), None);
    }

    #[test]
    fn a_conversion_this_stage_does_not_run_writes_nothing() {
        let text = StoredCell::Text("B2".to_string());
        assert_eq!(spell(Some("EFAdapter.EnumConversions"), &text), None);
    }

    fn unescape(value: &str) -> String {
        value
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&apos;", "'")
    }

    fn xml_attr(tag: &str, name: &str) -> Option<String> {
        let key = format!("{name}=\"");
        let start = tag.find(&key)? + key.len();
        let end = tag[start..].find('"')? + start;
        Some(unescape(&tag[start..end]))
    }

    /// `UID` → `IObject.Name` of every named `<PIDPipingConnector>` in a reference.
    fn reference_run_names(xml: &str) -> HashMap<String, String> {
        let mut names = HashMap::new();
        let mut rest = xml;
        while let Some(start) = rest.find("<PIDPipingConnector>") {
            rest = &rest[start..];
            let Some(end) = rest.find("</PIDPipingConnector>") else {
                break;
            };
            let block = &rest[..end];
            rest = &rest[end..];
            let Some(object_at) = block.find("<IObject") else {
                continue;
            };
            let Some(close) = block[object_at..].find("/>") else {
                continue;
            };
            let tag = &block[object_at..object_at + close];
            let (Some(uid), Some(name)) = (xml_attr(tag, "UID"), xml_attr(tag, "Name")) else {
                continue;
            };
            if !name.is_empty() {
                names.insert(uid, name);
            }
        }
        names
    }

    fn run_name(item: &PidPublishItem) -> Option<&str> {
        label(
            item,
            &StoredProperty {
                interface: "IObject",
                property: "Name",
            },
        )
    }

    /// A NULL diameter or materials class is an empty segment of the run's
    /// name. The sheet labels none of the 631 run names the seven acceptance
    /// references state, so this stage supplies every one of them — the 382
    /// whose parts are all stated, and the 249 whose diameter or materials
    /// class is empty. Each equals the reference, and no name is written on
    /// a run the reference does not name.
    #[test]
    fn an_empty_diameter_or_class_is_an_empty_segment_of_the_run_name() {
        let corpus = PathBuf::from(r"D:\work\cad\pid-test-data\验收数据");
        let package = PathBuf::from(r"D:\work\cad\pid-test-data");
        let dump = package.join("Export.dmp");
        let datamap = package.join("RefData~4~804.zip");
        if !corpus.is_dir() || !dump.is_file() || !datamap.is_file() {
            eprintln!(
                "skipping: the acceptance corpus or its Export.dmp and RefData~4~804.zip is absent"
            );
            return;
        }
        let stems = [
            "I井场 注采阀组工艺及自控流程图",
            "J K井场 注采阀组工艺及自控流程图",
            "L M井场 注采阀组工艺及自控流程图",
            "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
            "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
            "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
            "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
        ];
        let tables = ExpTables::read(&std::fs::read(&dump).expect("the plant dump reads"))
            .expect("the plant dump reads");
        let map = crate::backup::read_datamap(&datamap).expect("the plant data map reads");
        let mut added = 0usize;
        let mut wrong = Vec::new();
        let mut extra = Vec::new();
        let mut missing = Vec::new();
        for stem in stems {
            let pid = corpus.join(format!("{stem}.pid"));
            let reference = corpus.join(format!("{stem}_Data.xml"));
            if !pid.is_file() || !reference.is_file() {
                eprintln!(
                    "skipping: {stem} — the acceptance corpus holds no .pid beside a _Data.xml"
                );
                return;
            }
            let mut drawing = load_publish_from_pid(&pid).expect("the drawing loads");
            let before: HashMap<String, String> = drawing
                .items
                .iter()
                .filter(|item| item.class == Some("PIDPipingConnector"))
                .filter_map(|item| run_name(item).map(|name| (item.uid.clone(), name.to_string())))
                .collect();
            let report = attach_stored_values(&mut drawing, &tables, &map);
            assert_eq!(
                report.withheld, 0,
                "{stem}: a label disagrees with the dump, so no name is taken from it"
            );
            let reference_names = reference_run_names(
                &std::fs::read_to_string(&reference).expect("the reference reads"),
            );
            let mut seen = HashSet::new();
            for item in drawing
                .items
                .iter()
                .filter(|item| item.class == Some("PIDPipingConnector"))
            {
                let Some(name) = run_name(item) else {
                    continue;
                };
                seen.insert(item.uid.clone());
                match reference_names.get(&item.uid) {
                    Some(expected) if expected == name => {
                        if !before.contains_key(&item.uid) {
                            added += 1;
                        }
                    }
                    Some(expected) => wrong.push(format!(
                        "{stem} {} wrote {name:?}, reference {expected:?}",
                        item.uid
                    )),
                    None => extra.push(format!(
                        "{stem} {} wrote {name:?}, reference names none",
                        item.uid
                    )),
                }
            }
            for (uid, expected) in &reference_names {
                if seen.contains(uid) {
                    continue;
                }
                if missing.len() < 12 {
                    missing.push(format!("{stem} {uid} reference {expected:?}"));
                } else {
                    missing.push(format!("{stem} {uid}"));
                }
            }
        }
        assert!(
            wrong.is_empty() && extra.is_empty(),
            "wrong: {wrong:?}\nextra: {extra:?}"
        );
        assert!(
            missing.is_empty(),
            "{} reference names still unspelled, first: {missing:?}",
            missing.len()
        );
        assert_eq!(added, 631);
    }

    /// `12MPa` is how the dump stores the six relief pressures; the
    /// references write `12 MPa`. A trim spec is not a number jammed
    /// against a unit, and this function is not what spells it.
    #[test]
    fn a_relief_pressure_gains_the_space_before_its_unit() {
        assert_eq!(space_before_unit("12MPa"), "12 MPa");
        assert_eq!(space_before_unit("12.5kPa"), "12.5 kPa");
        assert_eq!(space_before_unit("12 MPa"), "12 MPa");
        assert_eq!(space_before_unit("B9"), "B9");
    }

    /// Where the three scattered properties sit in the committed package's
    /// dump. `InstrumentInline` is not an item type there, so an inline
    /// instrument's relief pressure is read under `Instrument`.
    #[test]
    fn the_three_scattered_properties_are_placed_in_the_declaring_table() {
        let (_, tables, map) = dwg();
        let column = |class: &str, property: &str, item_type: &str| {
            let (column, criteria) = source_of(&map, &tables, class, property, item_type)
                .unwrap_or_else(|| panic!("{class}.{property} is placed"));
            assert_eq!(criteria, None, "{class}.{property}");
            column.to_string()
        };
        assert_eq!(
            column(
                "PIDProcessEquipment",
                "LongMaterialDescription3",
                "EquipmentOther"
            ),
            "T_ModelItem.Description"
        );
        assert_eq!(
            column("PIDInlineInstrument", "ReliefPressure", "Instrument"),
            "T_InlineComp.SP_SetPressure"
        );
        assert_eq!(
            column("PIDProcessEquipment", "EquipmentTrimSpec", "EquipmentOther"),
            "T_Equipment.TrimSpec"
        );
        assert_eq!(
            column("PIDProcessVessel", "EquipmentTrimSpec", "Vessel"),
            "T_Equipment.TrimSpec"
        );
    }

    /// The fixture's vessel states `1.6AR12`, the text `T_Equipment.TrimSpec`
    /// holds, and the reference states the same text.
    #[test]
    fn the_vessel_states_its_trim_spec_from_its_equipment_row() {
        let (drawing, _) = dwg_with_stored_values();
        let trim = stated(
            &drawing,
            "F1081777313D41ABB14B7B18FA210D83",
            "EquipmentTrimSpec",
        )
        .expect("the vessel states it");
        assert_eq!(trim.interface, "IEquipment");
        assert_eq!(trim.value, "1.6AR12");
        assert_eq!(trim.source, placed("T_Equipment", "TrimSpec"));
    }
}
