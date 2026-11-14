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
//! that way, and is written from the dump.

use std::collections::{BTreeSet, HashMap};

use super::oracle_exp::{backup_uid_key, nominal_spelling, ExpTables, StoredCell};
use super::pid_load::{PidPublishAttribute, PidPublishDrawing, PidValueSource};
use super::placement::{place_property, DumpColumn, Unplaced};
use crate::backup::DataMap;

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
    /// Placed columns this dump does not let the stage read, each with the
    /// reason, sorted.
    pub unread: BTreeSet<String>,
}

/// Spell one stored cell as the reference writes the property, under the
/// conversion the map names: none (`None` or empty) is the value as
/// stored, and `EFAdapter.NominalDiameterConv` spells a code the way a
/// port's diameter is spelled ([`nominal_spelling`]: `80 mm`, `1 in`).
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
        _ => None,
    }
}

/// The column one property of one item type is read from, and the
/// conversion the map names for it.
type Source = Option<(DumpColumn, Option<String>)>;

/// Write [`STORED_PROPERTIES`] on the published items of `drawing` from
/// the dump behind `tables`, keeping each item's attributes ordered by
/// interface and name.
///
/// A column the dump does not let the stage read is reported in
/// [`StoredValues::unread`], and the drawing is published without it.
pub fn attach_stored_values(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
    map: &DataMap,
) -> StoredValues {
    let mut report = StoredValues::default();
    let mut sources: HashMap<(&'static str, &'static str, String), Source> = HashMap::new();
    let mut columns: HashMap<DumpColumn, Option<HashMap<String, StoredCell>>> = HashMap::new();
    for item in &mut drawing.items {
        let Some(class) = item.class else {
            continue;
        };
        let key = backup_uid_key(&item.uid);
        for stored in STORED_PROPERTIES {
            let source = sources
                .entry((class, stored.property, item.model_item_type.clone()))
                .or_insert_with(|| {
                    source_of(map, tables, class, stored.property, &item.model_item_type)
                });
            let Some((column, criteria)) = source else {
                continue;
            };
            let cells = columns.entry(column.clone()).or_insert_with(|| {
                match tables.stored_column(column) {
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
                }
            });
            let Some(cell) = cells.as_ref().and_then(|cells| cells.get(&key)) else {
                continue;
            };
            let Some(value) = spell(criteria.as_deref(), cell) else {
                report.unspelled += 1;
                continue;
            };
            match item.attributes.iter().find(|existing| {
                existing.interface == stored.interface && existing.name == stored.property
            }) {
                Some(existing) if existing.value == value => report.confirmed_labels += 1,
                Some(_) => report.disagreeing_labels += 1,
                None => {
                    report.written += 1;
                    item.attributes.push(PidPublishAttribute {
                        interface: stored.interface,
                        name: stored.property,
                        value,
                        source: PidValueSource::PlacedColumn(column.clone()),
                    });
                }
            }
        }
        item.attributes
            .sort_by(|left, right| (left.interface, left.name).cmp(&(right.interface, right.name)));
    }
    report
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
    if let Some(placement) = placements
        .iter()
        .find(|placement| placement.item_type() == Some(item_type))
    {
        let column = placement.column.clone().ok()?;
        return Some((column, placement.source.property.process_criteria.clone()));
    }
    let placement = placements
        .iter()
        .find(|placement| matches!(placement.column, Err(Unplaced::NoItemType { .. })))?;
    let column = tables
        .dictionary()
        .place(item_type, &placement.source.property.name)
        .ok()?;
    Some((column, placement.source.property.process_criteria.clone()))
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
        assert_eq!(
            report,
            StoredValues {
                written: 5,
                confirmed_labels: 17,
                disagreeing_labels: 2,
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
        assert_eq!(report.written, 3);
        assert_eq!(report.disagreeing_labels, 3);
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
    fn a_conversion_this_stage_does_not_run_writes_nothing() {
        let text = StoredCell::Text("B2".to_string());
        assert_eq!(spell(Some("EFAdapter.EnumConversions"), &text), None);
    }
}
