//! Where a published value is stored in an Oracle `exp` dump.
//!
//! A `SmartPlant` `_Data.xml` states its values under `(class, interface,
//! property)`. `SPPIDDataMap.xml` ([`crate::backup::DataMap`]) names the
//! P&ID item type and attribute a property of a class is published from
//! (`PIDPipingConnector` `PipingMaterialsClass` ← `PipeRun`
//! `PipingMaterialsClass`). The dump carries the plant's own data
//! dictionary, which places an item type's attribute in the table and the
//! column that store it (`T_PipeRun.PipingMaterialsClass`): an attribute
//! an item type inherits is placed in the table of the type that declares
//! it (`PipeRun.ItemTag` → `T_PlantItem.ItemTag`), and every such table
//! keys its rows by the item's `SP_ID`.
//!
//! [`DumpDictionary`] is that dictionary, [`DumpDictionary::place`] places
//! one attribute, and [`place_property`] joins the two for a published
//! property: every attribute the map publishes it from, each with its
//! [`DumpColumn`] or the reason the dictionary does not place it.
//!
//! The dictionary is five tables of the dump: `ITEM` (item type → id),
//! `ITEMATTRIBUTIONS` (an item type's attribute → attribution id),
//! `UNIQUEATTS` (attribution id → entity and attribute numbers),
//! `ENTITIES` (entity number → table name) and `ATTRIBUTES` (attribute
//! number → column name). The dump holds each of them twice; the first
//! insert is the one read.

use std::collections::HashMap;
use std::fmt;

use super::oracle_exp::{read_text_cells, ExpError};
use crate::backup::{DataMap, MapSource};

/// The data dictionary an Oracle `exp` dump carries: which table and column
/// store an item type's attribute.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DumpDictionary {
    /// `ITEM.NAME` → `ITEM.ID`.
    items: HashMap<String, String>,
    /// (`ITEMATTRIBUTIONS.ITEMID`, `NAME`) → `ATTRIBUTIONID`; the first row
    /// of a pair is kept.
    attributions: HashMap<(String, String), String>,
    /// `UNIQUEATTS.ID` → (`ENTITY_NUMBER`, `ATTRIBUTE_NUMBER`).
    unique: HashMap<String, (String, String)>,
    /// `ENTITIES.ENTITY_NUMBER` → `ENTITY_NAME`.
    entities: HashMap<String, String>,
    /// `ATTRIBUTES.ATTRIBUTE_NUMBER` → `ATTRIBUTE_NAME`.
    attributes: HashMap<String, String>,
}

/// A table and a column of the dump, spelled as the dictionary spells them
/// (`T_PipeRun`, `PipingMaterialsClass`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DumpColumn {
    /// `ENTITIES.ENTITY_NAME`.
    pub table: String,
    /// `ATTRIBUTES.ATTRIBUTE_NAME`.
    pub column: String,
}

impl DumpColumn {
    /// The table as the dump's `INSERT INTO` names it (`T_PIPERUN`).
    #[must_use]
    pub fn insert_table(&self) -> String {
        self.table.to_ascii_uppercase()
    }

    /// The column as the dump's `INSERT INTO` lists it
    /// (`PIPINGMATERIALSCLASS`).
    #[must_use]
    pub fn insert_column(&self) -> String {
        self.column.to_ascii_uppercase()
    }
}

impl fmt::Display for DumpColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.table, self.column)
    }
}

/// Why the dictionary places no column for an item type's attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unplaced {
    /// `ITEM` has no row of that name (the map's `InstrumentInline`).
    NoItemType {
        /// The item type asked for.
        item_type: String,
    },
    /// `ITEMATTRIBUTIONS` has no row of the item type with that name.
    NoAttribution {
        /// The item type asked for.
        item_type: String,
        /// The attribute asked for.
        attribute: String,
    },
    /// The attribution is not in `UNIQUEATTS`.
    NotUnique {
        /// `ITEMATTRIBUTIONS.ATTRIBUTIONID`.
        attribution: String,
    },
}

impl fmt::Display for Unplaced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoItemType { item_type } => {
                write!(f, "the dictionary has no item type {item_type}")
            }
            Self::NoAttribution {
                item_type,
                attribute,
            } => write!(f, "item type {item_type} has no attribution {attribute}"),
            Self::NotUnique { attribution } => {
                write!(f, "attribution {attribution} is not in UNIQUEATTS")
            }
        }
    }
}

impl DumpDictionary {
    /// Read the dictionary out of a dump.
    ///
    /// A dump with none of the five tables reads as an empty dictionary,
    /// which places nothing.
    ///
    /// # Errors
    ///
    /// Returns [`ExpError::Table`] when the dump has some of the five
    /// tables and not the others, a table's bytes do not frame, or a table
    /// lacks a column the dictionary reads.
    pub fn read(dump: &[u8]) -> Result<Self, ExpError> {
        let tables = [
            read_text_cells(dump, "ITEM", &["NAME", "ID"])?,
            read_text_cells(
                dump,
                "ITEMATTRIBUTIONS",
                &["ITEMID", "NAME", "ATTRIBUTIONID"],
            )?,
            read_text_cells(
                dump,
                "UNIQUEATTS",
                &["ID", "ENTITY_NUMBER", "ATTRIBUTE_NUMBER"],
            )?,
            read_text_cells(dump, "ENTITIES", &["ENTITY_NUMBER", "ENTITY_NAME"])?,
            read_text_cells(dump, "ATTRIBUTES", &["ATTRIBUTE_NUMBER", "ATTRIBUTE_NAME"])?,
        ];
        if tables.iter().all(Option::is_none) {
            return Ok(Self::default());
        }
        let [Some(items), Some(attributions), Some(unique), Some(entities), Some(attributes)] =
            tables
        else {
            return Err(ExpError::Table {
                name: "ITEM".to_string(),
                detail: "the dump carries part of its data dictionary: ITEM, ITEMATTRIBUTIONS, \
                         UNIQUEATTS, ENTITIES and ATTRIBUTES are not all there"
                    .to_string(),
            });
        };
        let pairs = |rows: Vec<Vec<Option<String>>>| -> HashMap<String, String> {
            let mut pairs = HashMap::new();
            for row in rows {
                if let [Some(key), Some(value)] = row.as_slice() {
                    pairs.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            pairs
        };
        let mut by_item = HashMap::new();
        for row in attributions {
            if let [Some(item), Some(name), Some(attribution)] = row.as_slice() {
                by_item
                    .entry((item.clone(), name.clone()))
                    .or_insert_with(|| attribution.clone());
            }
        }
        let mut placed = HashMap::new();
        for row in unique {
            if let [Some(id), Some(entity), Some(attribute)] = row.as_slice() {
                placed
                    .entry(id.clone())
                    .or_insert_with(|| (entity.clone(), attribute.clone()));
            }
        }
        Ok(Self {
            items: pairs(items),
            attributions: by_item,
            unique: placed,
            entities: pairs(entities),
            attributes: pairs(attributes),
        })
    }

    /// Item types in `ITEM`.
    #[must_use]
    pub fn item_type_count(&self) -> usize {
        self.items.len()
    }

    /// Distinct (item type, attribute) pairs in `ITEMATTRIBUTIONS`.
    #[must_use]
    pub fn attribution_count(&self) -> usize {
        self.attributions.len()
    }

    /// The table and column that store `item_type`'s `attribute`, both
    /// named as the map names them (`PipeRun`, `NominalDiameter`).
    ///
    /// # Errors
    ///
    /// Returns the [`Unplaced`] step at which the dictionary stops. An
    /// entity or attribute number with no name in `ENTITIES` /
    /// `ATTRIBUTES` is spelled `?`, as a place that cannot be read.
    pub fn place(&self, item_type: &str, attribute: &str) -> Result<DumpColumn, Unplaced> {
        let item = self
            .items
            .get(item_type)
            .ok_or_else(|| Unplaced::NoItemType {
                item_type: item_type.to_string(),
            })?;
        let attribution = self
            .attributions
            .get(&(item.clone(), attribute.to_string()))
            .ok_or_else(|| Unplaced::NoAttribution {
                item_type: item_type.to_string(),
                attribute: attribute.to_string(),
            })?;
        let (entity, column) = self
            .unique
            .get(attribution)
            .ok_or_else(|| Unplaced::NotUnique {
                attribution: attribution.clone(),
            })?;
        Ok(DumpColumn {
            table: self
                .entities
                .get(entity)
                .map_or_else(|| "?".to_string(), Clone::clone),
            column: self
                .attributes
                .get(column)
                .map_or_else(|| "?".to_string(), Clone::clone),
        })
    }
}

/// One attribute the map publishes a property from, placed in the dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement<'a> {
    /// The item type, its attribute and the map's rel to the property.
    pub source: MapSource<'a>,
    /// Found through the item type ([`DataMap::sources_through_item_type`])
    /// because no rel names the class itself.
    pub through_item_type: bool,
    /// Where the dump stores the attribute.
    pub column: Result<DumpColumn, Unplaced>,
}

impl Placement<'_> {
    /// The item type (`PipeRun`), or `None` when no class owns the
    /// attribute in the map.
    #[must_use]
    pub fn item_type(&self) -> Option<&str> {
        self.source.class.map(|class| class.name.as_str())
    }
}

/// Every attribute the map publishes as `property` on an object of
/// `publish_class`, each placed in the dump.
///
/// The map is read by the class each rel names ([`DataMap::sources`]);
/// when no rel names `publish_class`, by the item types published as it
/// ([`DataMap::sources_through_item_type`]), and the placements say so. An
/// attribute no class owns is placed under item type `?`, which the
/// dictionary does not have.
#[must_use]
pub fn place_property<'a>(
    map: &'a DataMap,
    dictionary: &DumpDictionary,
    publish_class: &str,
    property: &str,
) -> Vec<Placement<'a>> {
    let mut sources = map.sources(publish_class, property);
    let through_item_type = sources.is_empty();
    if through_item_type {
        sources = map.sources_through_item_type(publish_class, property);
    }
    sources
        .into_iter()
        .map(|source| {
            let item_type = source.class.map_or("?", |class| class.name.as_str());
            Placement {
                column: dictionary.place(item_type, &source.property.name),
                source,
                through_item_type,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{MapClass, MapProperty, MapPropertyTarget};

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    /// One insert of national-text columns in the dump's framing.
    fn table(name: &str, columns: &[&str], rows: &[&[Option<&str>]]) -> Vec<u8> {
        let listed = columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = format!("INSERT INTO \"{name}\" ({listed}) VALUES\n").into_bytes();
        out.extend(u16::try_from(columns.len()).unwrap().to_le_bytes());
        for _ in columns {
            out.extend(1u16.to_le_bytes());
            out.extend(4000u16.to_le_bytes());
            out.extend(2000u16.to_le_bytes());
            out.extend(2u16.to_le_bytes());
        }
        out.extend([0, 0, 0, 0]);
        for cells in rows {
            for cell in *cells {
                match cell {
                    None => out.extend(0xFFFEu16.to_le_bytes()),
                    Some(text) => {
                        let bytes = utf16(text);
                        out.extend(u16::try_from(bytes.len()).unwrap().to_le_bytes());
                        out.extend(bytes);
                    }
                }
            }
            out.extend([0, 0]);
        }
        out.extend(0xFFFFu16.to_le_bytes());
        out
    }

    /// `PipeRun` with `PipingMaterialsClass` in `T_PipeRun` and `ItemTag`
    /// in `T_PlantItem`, `Nozzle` with an attribution `UNIQUEATTS` lacks,
    /// and an attribution whose entity has no name.
    fn dictionary_dump() -> Vec<u8> {
        let mut dump = Vec::new();
        dump.extend(table(
            "ITEM",
            &["ID", "NAME"],
            &[
                &[Some("10"), Some("PipeRun")],
                &[Some("20"), Some("Nozzle")],
            ],
        ));
        dump.extend(table(
            "ITEMATTRIBUTIONS",
            &["ITEMID", "ATTRIBUTIONID", "NAME"],
            &[
                &[Some("10"), Some("A1"), Some("PipingMaterialsClass")],
                &[Some("10"), Some("A2"), Some("ItemTag")],
                &[Some("10"), Some("A9"), Some("ItemTag")],
                &[Some("20"), Some("A3"), Some("NominalDiameter")],
                &[Some("20"), Some("A4"), Some("Lost")],
            ],
        ));
        dump.extend(table(
            "UNIQUEATTS",
            &["ID", "ENTITY_NUMBER", "ATTRIBUTE_NUMBER"],
            &[
                &[Some("A1"), Some("1"), Some("100")],
                &[Some("A2"), Some("2"), Some("200")],
                &[Some("A9"), Some("1"), Some("100")],
                &[Some("A4"), Some("7"), Some("100")],
            ],
        ));
        dump.extend(table(
            "ENTITIES",
            &["ENTITY_NUMBER", "ENTITY_NAME"],
            &[
                &[Some("1"), Some("T_PipeRun")],
                &[Some("2"), Some("T_PlantItem")],
            ],
        ));
        dump.extend(table(
            "ATTRIBUTES",
            &["ATTRIBUTE_NUMBER", "ATTRIBUTE_NAME"],
            &[
                &[Some("100"), Some("PipingMaterialsClass")],
                &[Some("200"), Some("ItemTag")],
            ],
        ));
        dump
    }

    fn column(table: &str, column: &str) -> DumpColumn {
        DumpColumn {
            table: table.to_string(),
            column: column.to_string(),
        }
    }

    #[test]
    fn an_attribute_is_placed_in_the_table_that_declares_it() {
        let dictionary = DumpDictionary::read(&dictionary_dump()).expect("reads");
        assert_eq!(dictionary.item_type_count(), 2);
        assert_eq!(dictionary.attribution_count(), 4);
        let placed = dictionary
            .place("PipeRun", "PipingMaterialsClass")
            .expect("placed");
        assert_eq!(placed, column("T_PipeRun", "PipingMaterialsClass"));
        assert_eq!(placed.insert_table(), "T_PIPERUN");
        assert_eq!(placed.insert_column(), "PIPINGMATERIALSCLASS");
        assert_eq!(placed.to_string(), "T_PipeRun.PipingMaterialsClass");
        assert_eq!(
            dictionary.place("PipeRun", "ItemTag"),
            Ok(column("T_PlantItem", "ItemTag")),
            "the first ITEMATTRIBUTIONS row of a pair is the one kept"
        );
    }

    #[test]
    fn the_dictionary_says_where_it_stops() {
        let dictionary = DumpDictionary::read(&dictionary_dump()).expect("reads");
        let cases = [
            (
                "InstrumentInline",
                "InsulationSpec",
                "the dictionary has no item type InstrumentInline",
            ),
            (
                "PipeRun",
                "SlopeSpec",
                "item type PipeRun has no attribution SlopeSpec",
            ),
            (
                "Nozzle",
                "NominalDiameter",
                "attribution A3 is not in UNIQUEATTS",
            ),
        ];
        for (item_type, attribute, said) in cases {
            let error = dictionary.place(item_type, attribute).unwrap_err();
            assert_eq!(error.to_string(), said);
        }
        assert_eq!(
            dictionary.place("Nozzle", "Lost"),
            Ok(column("?", "PipingMaterialsClass")),
            "an entity number with no name is a place that cannot be read"
        );
    }

    #[test]
    fn a_dump_without_a_dictionary_places_nothing_and_half_of_one_is_refused() {
        let empty = DumpDictionary::read(b"").expect("reads");
        assert_eq!(empty, DumpDictionary::default());
        assert!(matches!(
            empty.place("PipeRun", "PipingMaterialsClass"),
            Err(Unplaced::NoItemType { .. })
        ));
        let dump = dictionary_dump();
        let cut = dump
            .windows(b"INSERT INTO \"ENTITIES\"".len())
            .position(|window| window == b"INSERT INTO \"ENTITIES\"")
            .unwrap();
        let error = DumpDictionary::read(&dump[..cut]).unwrap_err();
        assert!(
            error.to_string().contains("part of its data dictionary"),
            "{error}"
        );
    }

    fn map() -> DataMap {
        let target = |property: &str, class: &str| MapPropertyTarget {
            property: property.to_string(),
            publish_class: class.to_string(),
            edge: String::new(),
        };
        DataMap {
            classes: vec![MapClass {
                uid: "SPMC_10".to_string(),
                name: "PipeRun".to_string(),
                publish_classes: vec!["PIDPipingConnector".to_string()],
                ..MapClass::default()
            }],
            properties: vec![
                MapProperty {
                    uid: "P1".to_string(),
                    name: "PipingMaterialsClass".to_string(),
                    class_uid: "SPMC_10".to_string(),
                    targets: vec![target("PipingMaterialsClass", "PIDPipingConnector")],
                    ..MapProperty::default()
                },
                MapProperty {
                    uid: "P2".to_string(),
                    name: "ItemTag".to_string(),
                    class_uid: "SPMC_10".to_string(),
                    targets: vec![target("Name", "PIDProcessPoint")],
                    ..MapProperty::default()
                },
                MapProperty {
                    uid: "P3".to_string(),
                    name: "Orphan".to_string(),
                    targets: vec![target("Orphan", "PIDPipingConnector")],
                    ..MapProperty::default()
                },
            ],
            ..DataMap::default()
        }
    }

    #[test]
    fn a_published_property_is_placed_through_the_map() {
        let dictionary = DumpDictionary::read(&dictionary_dump()).expect("reads");
        let map = map();
        let placed = place_property(
            &map,
            &dictionary,
            "PIDPipingConnector",
            "PipingMaterialsClass",
        );
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].item_type(), Some("PipeRun"));
        assert_eq!(placed[0].source.property.name, "PipingMaterialsClass");
        assert!(!placed[0].through_item_type);
        assert_eq!(
            placed[0].column,
            Ok(column("T_PipeRun", "PipingMaterialsClass"))
        );

        let through = place_property(&map, &dictionary, "PIDPipingConnector", "Name");
        assert_eq!(through.len(), 1, "the rel names PIDProcessPoint");
        assert!(through[0].through_item_type);
        assert_eq!(through[0].column, Ok(column("T_PlantItem", "ItemTag")));

        let orphan = place_property(&map, &dictionary, "PIDPipingConnector", "Orphan");
        assert_eq!(orphan[0].item_type(), None);
        assert_eq!(
            orphan[0].column,
            Err(Unplaced::NoItemType {
                item_type: "?".to_string()
            })
        );
        assert!(place_property(&map, &dictionary, "PIDNozzle", "PipingMaterialsClass").is_empty());
    }

    /// The committed package's dump and data map: the four piping
    /// attributes of a pipe run are columns of `T_PipeRun`, and the map's
    /// `InstrumentInline` is not an item type of the dump.
    #[test]
    fn the_committed_package_places_a_pipe_runs_attributes_in_t_piperun() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file/backup-test/DWG-0202GP06-01_p");
        let dump = std::fs::read(package.join("Export.dmp")).expect("the committed dump");
        let map = crate::backup::read_datamap(&package.join("RefData~4~804.zip"))
            .expect("the committed data map");
        let dictionary = DumpDictionary::read(&dump).expect("the dictionary reads");
        assert_eq!(dictionary.item_type_count(), 80);
        let place = |class: &str, property: &str| -> Vec<String> {
            place_property(&map, &dictionary, class, property)
                .into_iter()
                .map(|placement| match placement.column {
                    Ok(column) => format!("{} <- {column}", placement.source.property.name),
                    Err(unplaced) => format!("{} ({unplaced})", placement.source.property.name),
                })
                .collect()
        };
        assert_eq!(
            place("PIDPipingConnector", "PipingMaterialsClass"),
            ["PipingMaterialsClass <- T_PipeRun.PipingMaterialsClass"]
        );
        assert_eq!(
            place("PIDPipingConnector", "PipingConnectorSeqNo"),
            ["TagSequenceNo <- T_PipeRun.TagSequenceNo"]
        );
        assert_eq!(
            place("PIDPipingConnector", "RepresentationsAreAllZeroLength"),
            ["SP_ConnectorsZeroLength <- T_PipeRun.SP_ConnectorsZeroLength"]
        );
        assert_eq!(
            place("PIDPipingComponent", "InsulSpec"),
            ["InsulationSpec <- T_PipingComp.InsulationSpec"]
        );
        assert_eq!(
            place("PIDNozzle", "NominalDiameter"),
            ["NominalDiameter <- T_Nozzle.NominalDiameter"]
        );
        assert_eq!(
            place("PIDInlineInstrument", "InsulSpec"),
            ["InsulationSpec (the dictionary has no item type InstrumentInline)"]
        );
    }
}
