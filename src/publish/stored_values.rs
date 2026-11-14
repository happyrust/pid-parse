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
//! A label on the sheet may already have given the item the property. The
//! dump is the database the reference was published from, so its value is
//! the one written; the labels that said otherwise are counted.

use std::collections::{BTreeSet, HashMap};

use super::oracle_exp::{backup_uid_key, ExpTables, StoredCell};
use super::pid_load::{PidPublishAttribute, PidPublishDrawing, PidValueSource};
use super::placement::{place_property, DumpColumn};
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
pub const STORED_PROPERTIES: &[StoredProperty] = &[StoredProperty {
    interface: "IPipingSpecifiedItem",
    property: "PipingMaterialsClass",
}];

/// What [`attach_stored_values`] did to one drawing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredValues {
    /// Values written.
    pub written: usize,
    /// Of those, values that replace a different value a label gave.
    pub replaced_labels: usize,
    /// Cells left out because [`spell`] does not spell them.
    pub unspelled: usize,
    /// Placed columns this dump does not let the stage read, each with the
    /// reason, sorted.
    pub unread: BTreeSet<String>,
}

/// Spell one stored cell as the reference writes the property, under the
/// conversion the map names (`None` and empty: the value as stored).
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
            let attribute = PidPublishAttribute {
                interface: stored.interface,
                name: stored.property,
                value,
                source: PidValueSource::PlacedColumn(column.clone()),
            };
            report.written += 1;
            match item.attributes.iter_mut().find(|existing| {
                existing.interface == attribute.interface && existing.name == attribute.name
            }) {
                Some(existing) => {
                    if existing.value != attribute.value {
                        report.replaced_labels += 1;
                        *existing = attribute;
                    }
                }
                None => item.attributes.push(attribute),
            }
        }
        item.attributes
            .sort_by(|left, right| (left.interface, left.name).cmp(&(right.interface, right.name)));
    }
    report
}

/// The placed column the map publishes `property` of `class` from, for an
/// item of type `item_type`.
fn source_of(
    map: &DataMap,
    tables: &ExpTables,
    class: &str,
    property: &str,
    item_type: &str,
) -> Source {
    let placement = place_property(map, tables.dictionary(), class, property)
        .into_iter()
        .find(|placement| placement.item_type() == Some(item_type))?;
    let column = placement.column.ok()?;
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
                written: 9,
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

    #[test]
    fn the_dumps_value_replaces_a_label_that_says_otherwise_and_a_class_less_item_gets_none() {
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
        assert_eq!(report.written, 8);
        assert_eq!(report.replaced_labels, 1);
        let run = stated(
            &drawing,
            "4C2F232018CE42E8B7FFCABD553BDBC4",
            "PipingMaterialsClass",
        )
        .unwrap();
        assert_eq!(run.value, "1.6AR12");
        assert_eq!(run.source, placed("T_PipeRun", "PipingMaterialsClass"));
        assert!(stated(
            &drawing,
            "06F9755FE3FE408D8A1D6A5F3B38F2DF",
            "PipingMaterialsClass"
        )
        .is_none());
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
    fn a_conversion_this_stage_does_not_run_writes_nothing() {
        let text = StoredCell::Text("B2".to_string());
        assert_eq!(spell(Some("EFAdapter.EnumConversions"), &text), None);
    }
}
