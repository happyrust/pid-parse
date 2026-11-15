//! Enumerated values from the codelist cells of an Oracle `exp` dump.
//!
//! Plan `docs/plans/2026-11-10-publish-export-completion.md` S8. The plant
//! database stores an enumerated attribute as the index of an entry of a
//! codelist (`T_PLANTITEM.CONSTRUCTIONSTATUS` 2), and `SPPIDDataMap.xml`
//! holds the entry (`SP_93_2`, `MapEnumNumber` 2) with its name, its
//! description and the schema enum it is published as. An item the `.pid`
//! publishes takes, by the row of the dump that shares its UID:
//!
//! | Class | Value | Spelled from |
//! |---|---|---|
//! | the ten classes that carry `IPBSItem` | `ConstructionStatus2` | `MapEnumToEnum` of `SP_93` `CONSTRUCTIONSTATUS` |
//! | same | `ConstructionStatus` | the one schema enum `EnumToMapEnum` reads back into that entry and no `MapEnumToEnum` publishes to |
//! | equipment | `IEquipment.EqTypeDescription` | name of `SP_22` `T_EQUIPMENT.EQUIPMENTTYPE` |
//! | `PIDPipingComponent` | `PipModelCode` | description of `SP_25` `T_PIPINGCOMP.PIPINGCOMPTYPE` |
//! | same | `CommoditySpecialtyType` | [`SPECIALTY_ITEM`] on `T_PIPINGCOMP.SP_ISSPECIALTYITEM` |
//! | instruments | `IInstrument.InstrumentType3` | `MapEnumToEnum` of `SP_28` `T_INSTRUMENT.INSTRUMENTTYPE` |
//! | same | `IInstrument.ProcFunc` | `MapEnumToEnum` of `SP_29` `T_INSTRUMENT.INSTRUMENTCLASS` |
//! | same | `IInstrumentOcc.InstrumentLocation` | `MapEnumToEnum` of `SP_17` `T_INSTRUMENT.LOCATION` |
//! | `PIDNozzle` | `ProcessEqCompType2` | `MapEnumToEnum` of `SP_35` `T_NOZZLE.NOZZLETYPE` |
//! | same | `ProcEqpCompTypeDescription` | name of that `SP_35` entry |
//! | `PIDPipingConnector` | `PipingConnectorType` | `MapEnumToEnum` of `SP_14` `T_PIPERUN.PIPERUNTYPE` |
//! | same | `IConnector.FlowDirection` | `MapEnumToEnum` of `SP_47` `T_PIPERUN.FLOWDIRECTION` |
//! | `PIDPipingComponent` | `PipingComponentType1`…`3` | [`LEAF_LEVELS`] of the leaf of `SP_27` `PIPINGCOMPCLASS`, `SP_26` `PIPINGCOMPSUBCLASS` and `SP_25` `PIPINGCOMPTYPE` |
//! | equipment | `IEquipment.EqType0`…`5` | same, of `SP_24` `T_EQUIPMENT.CLASS`, `SP_23` `EQUIPMENTSUBCLASS` and `SP_22` `EQUIPMENTTYPE` |
//! | `PIDNozzle` | `ProcessEqCompType1` | same, of `SP_35` `T_NOZZLE.NOZZLETYPE` |
//! | instruments | `IInstrument.InstrumentType` | same, of `SP_29` `INSTRUMENTCLASS` and `SP_28` `INSTRUMENTTYPE` |
//!
//! A NULL cell, an index the list does not hold and an entry with nothing
//! to spell leave the value out. The map sends the nozzle type to
//! `ProcessEqCompType1`; the references carry the mapped enum one level
//! down, as `ProcessEqCompType2`. The drawing takes the two `IDocument`
//! values every reference `PIDDrawing` carries, which the map leaves
//! unmapped ([`DRAWING_CONSTANTS`]).
//!
//! On the seven reference drawings whose backup is their own revision
//! (I井场, J K / L M井场, 轻烃分离器I, 空冷器A/B/C) these rules state each
//! of the 9 218 values the references carry under them, and nothing the
//! references leave out. The levels add 2 392, each equal to the
//! reference's; [`LEAF_LEVELS`] is read off these references and four
//! more, so that says each item's leaf is read right, not that the table
//! holds beyond them.
//!
//! The levels of a hierarchical enumeration are in neither the dump nor the
//! map: the dump keeps the leaf's index and the map publishes the leaf. An
//! item's leaf is what `MapEnumToEnum` publishes the deepest cell of its
//! hierarchy as, and [`LEAF_LEVELS`] holds the levels the reference drawings
//! state for that leaf. An item whose leaf they do not show takes no level,
//! and neither does an instrument of a type the plant added.

use std::collections::HashMap;

use super::oracle_exp::{backup_uid_key, ExpError, ExpTables, ItemTable};
use super::pid_load::{PidPublishAttribute, PidPublishDrawing, PidValueSource};
use crate::backup::DataMap;

/// The classes whose objects carry `IPBSItem` and a construction status.
const PBS_CLASSES: [&str; 10] = [
    "PIDControlSystemFunction",
    "PIDHeatTransferEquipment",
    "PIDInlineInstrument",
    "PIDInstrument",
    "PIDMechanicalEquipment",
    "PIDNozzle",
    "PIDPipingComponent",
    "PIDPipingConnector",
    "PIDProcessEquipment",
    "PIDProcessVessel",
];

const EQUIPMENT_CLASSES: [&str; 4] = [
    "PIDHeatTransferEquipment",
    "PIDMechanicalEquipment",
    "PIDProcessEquipment",
    "PIDProcessVessel",
];

const INSTRUMENT_CLASSES: [&str; 2] = ["PIDInlineInstrument", "PIDInstrument"];

/// `CommoditySpecialtyType` by `T_PIPINGCOMP.SP_ISSPECIALTYITEM`. The map
/// converts `PipingComp.SP_IsSpecialtyItem` with `EFAdapter.BoolToEnumConv`
/// and names no enum for either index, so the two spellings are read off
/// the seven reference drawings: 610 components with 1, 129 with 2, no
/// other index and no other spelling.
pub const SPECIALTY_ITEM: [(i64, &str); 2] = [
    (1, "@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}"),
    (2, "@{5F7F8F68-BC29-11D6-BDBC-00104BCC2B69}"),
];

/// `IDocument` values of every reference `PIDDrawing` (the thirteen at hand
/// agree). The dump's own codelist texts are `Piping Documents` and
/// `P&IDs`, and the map does not map either attribute.
pub const DRAWING_CONSTANTS: [(&str, &str); 2] =
    [("DocCategory", "P&ID Documents"), ("DocType", "P&ID")];

/// The first `INSTRUMENTTYPE` index a plant adds to the vendor's list.
/// An instrument of such a type that the map publishes states neither
/// `InstrumentType3` nor `ProcFunc`; one the map does not publish still
/// states `ProcFunc`. Read off the seven reference drawings, not off a
/// rule of the vendor's: 136 of 136 `InstrumentType3` and 154 of 154
/// `ProcFunc` follow it.
const PLANT_ADDED_INSTRUMENT_TYPE: i64 = 10_000;

/// A hierarchical enumeration: the classes that carry it, and the codelist
/// columns of the item's table, the shallowest first, whose deepest cell
/// the map publishes names the item's leaf.
#[derive(Clone, Copy)]
struct Hierarchy {
    classes: &'static [&'static str],
    interface: &'static str,
    table: ItemTable,
    /// `(column, list)`.
    columns: &'static [(&'static str, &'static str)],
}

const HIERARCHIES: [Hierarchy; 4] = [
    Hierarchy {
        classes: &["PIDPipingComponent"],
        interface: "IPipingComponent",
        table: ItemTable::PipingComp,
        columns: &[
            ("PIPINGCOMPCLASS", "SP_27"),
            ("PIPINGCOMPSUBCLASS", "SP_26"),
            ("PIPINGCOMPTYPE", "SP_25"),
        ],
    },
    Hierarchy {
        classes: &EQUIPMENT_CLASSES,
        interface: "IEquipment",
        table: ItemTable::Equipment,
        columns: &[
            ("CLASS", "SP_24"),
            ("EQUIPMENTSUBCLASS", "SP_23"),
            ("EQUIPMENTTYPE", "SP_22"),
        ],
    },
    Hierarchy {
        classes: &["PIDNozzle"],
        interface: "IEquipmentComponent",
        table: ItemTable::Nozzle,
        columns: &[("NOZZLETYPE", "SP_35")],
    },
    Hierarchy {
        classes: &INSTRUMENT_CLASSES,
        interface: "IInstrument",
        table: ItemTable::Instrument,
        columns: &[("INSTRUMENTCLASS", "SP_29"), ("INSTRUMENTTYPE", "SP_28")],
    },
];

/// The levels of each leaf of a hierarchical enumeration, the root first
/// and the leaf last, by the interface that carries them: what the eleven
/// acceptance references state on the 1 204 objects that carry a level, one
/// chain to a leaf. Read off those references, not off a rule of the
/// vendor's: the dump keeps the leaf's index, the map publishes the leaf,
/// and the schema that hangs the levels above it is not at hand. A value
/// the references show only above a leaf is not a leaf here.
pub const LEAF_LEVELS: [(&str, &[(&str, &str)]); 26] = [
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD9284-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD92C0-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD934A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD9284-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD92CC-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{8C5875BB-2B7A-4FF9-BDCE-A65ABD11D6B4}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD9284-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD92D2-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{5772FBC0-A395-4FF4-9244-DCDF4809D422}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD9284-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD92D2-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{5F7F923A-BC29-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD9284-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD92D2-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD93EC-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD928A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9834-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD928A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD986A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD990C-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD928A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9876-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD9A32-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD928A-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9882-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{7488D8C3-BC43-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD92A2-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9B94-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[(
            "PipingComponentType1",
            "@{81CD92A8-BC07-11D6-BDBC-00104BCC2B69}",
        )],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD92A8-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9A3E-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD9A98-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD92A8-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9A86-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD92A8-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9A86-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType3",
                "@{81CD9B4C-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IPipingComponent",
        &[
            (
                "PipingComponentType1",
                "@{81CD92A8-BC07-11D6-BDBC-00104BCC2B69}",
            ),
            (
                "PipingComponentType2",
                "@{81CD9A8C-BC07-11D6-BDBC-00104BCC2B69}",
            ),
        ],
    ),
    (
        "IEquipment",
        &[("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}")],
    ),
    (
        "IEquipment",
        &[
            ("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}"),
            ("EqType1", "@EE790"),
        ],
    ),
    (
        "IEquipment",
        &[
            ("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}"),
            ("EqType1", "@EE790"),
            ("EqType2", "@EE796"),
            ("EqType3", "@{85654F01-498D-45B9-8524-05FBFC800AAA}"),
        ],
    ),
    (
        "IEquipment",
        &[
            ("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}"),
            ("EqType1", "@EE791"),
            ("EqType2", "@EE7A1"),
            ("EqType3", "@EE7B9"),
            ("EqType4", "@EE7C2"),
            ("EqType5", "@EE7CA"),
        ],
    ),
    (
        "IEquipment",
        &[
            ("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}"),
            ("EqType1", "@EE792"),
            ("EqType2", "@EE79D"),
        ],
    ),
    (
        "IEquipment",
        &[
            ("EqType0", "@{47BF0267-DD41-4E1A-9B41-C4B714C8FF92}"),
            ("EqType1", "@EE793"),
            ("EqType2", "@{40444CCF-1021-4882-B2FF-2000ECAE92F5}"),
            ("EqType3", "@{A371290D-127F-41AD-80E4-0F110C54E848}"),
            ("EqType4", "@{B04624A3-1F93-45CD-A4EB-9F82F2C0A79D}"),
        ],
    ),
    (
        "IEquipment",
        &[("EqType0", "@{B527921C-A88C-4301-A583-AEA03298D33D}")],
    ),
    (
        "IEquipmentComponent",
        &[
            ("ProcessEqCompType1", "@EE6D4"),
            (
                "ProcessEqCompType2",
                "@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}",
            ),
        ],
    ),
    (
        "IInstrument",
        &[
            ("InstrumentType", "@EE3F8.1"),
            ("InstrumentType3", "@EE425"),
        ],
    ),
    (
        "IInstrument",
        &[
            ("InstrumentType", "@EE3F9.2"),
            ("InstrumentType3", "@EE427"),
        ],
    ),
];

/// The codelist cells the rules read: by table and column, the index each
/// `SP_ID` (folded) stores.
type Cells = HashMap<(ItemTable, &'static str), HashMap<String, i64>>;

/// Every column a rule or a hierarchy reads.
const COLUMNS: [(ItemTable, &str); 14] = [
    (ItemTable::PlantItem, "CONSTRUCTIONSTATUS"),
    (ItemTable::Equipment, "CLASS"),
    (ItemTable::Equipment, "EQUIPMENTSUBCLASS"),
    (ItemTable::Equipment, "EQUIPMENTTYPE"),
    (ItemTable::PipingComp, "PIPINGCOMPCLASS"),
    (ItemTable::PipingComp, "PIPINGCOMPSUBCLASS"),
    (ItemTable::PipingComp, "PIPINGCOMPTYPE"),
    (ItemTable::PipingComp, "SP_ISSPECIALTYITEM"),
    (ItemTable::Instrument, "INSTRUMENTCLASS"),
    (ItemTable::Instrument, "INSTRUMENTTYPE"),
    (ItemTable::Instrument, "LOCATION"),
    (ItemTable::Nozzle, "NOZZLETYPE"),
    (ItemTable::PipeRun, "PIPERUNTYPE"),
    (ItemTable::PipeRun, "FLOWDIRECTION"),
];

fn read_cells(tables: &ExpTables) -> Result<Cells, ExpError> {
    let mut cells = Cells::new();
    for (table, column) in COLUMNS {
        cells.insert((table, column), tables.codelist_cells(table, column)?);
    }
    Ok(cells)
}

fn cell(cells: &Cells, table: ItemTable, column: &'static str, key: &str) -> Option<i64> {
    cells.get(&(table, column))?.get(key).copied()
}

/// How a codelist index is spelled.
#[derive(Clone, Copy)]
enum Spelling {
    /// `@` and the schema enum `MapEnumToEnum` publishes the entry as.
    Target,
    /// `@` and the one schema enum the adapter only reads back into the
    /// entry ([`DataMap::enums_retrieved_only`]).
    RetrievedOnly,
    /// The entry's name.
    Name,
    /// The entry's description.
    Description,
    /// [`SPECIALTY_ITEM`].
    SpecialtyItem,
}

impl Spelling {
    fn label(self) -> &'static str {
        match self {
            Self::Target => "MapEnumToEnum",
            Self::RetrievedOnly => "EnumToMapEnum, retrieved only",
            Self::Name => "SPMapEnumDef Name",
            Self::Description => "SPMapEnumDef Description",
            Self::SpecialtyItem => "codelist_values::SPECIALTY_ITEM",
        }
    }
}

/// One value a rule states.
struct Rule {
    interface: &'static str,
    name: &'static str,
    table: ItemTable,
    column: &'static str,
    list: &'static str,
    spelling: Spelling,
}

const fn rule(
    interface: &'static str,
    name: &'static str,
    table: ItemTable,
    column: &'static str,
    list: &'static str,
    spelling: Spelling,
) -> Rule {
    Rule {
        interface,
        name,
        table,
        column,
        list,
        spelling,
    }
}

/// The rules a class states, in the order the values are added.
fn rules_of(class: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    if PBS_CLASSES.contains(&class) {
        rules.push(rule(
            "IPBSItem",
            "ConstructionStatus",
            ItemTable::PlantItem,
            "CONSTRUCTIONSTATUS",
            "SP_93",
            Spelling::RetrievedOnly,
        ));
        rules.push(rule(
            "IPBSItem",
            "ConstructionStatus2",
            ItemTable::PlantItem,
            "CONSTRUCTIONSTATUS",
            "SP_93",
            Spelling::Target,
        ));
    }
    if EQUIPMENT_CLASSES.contains(&class) {
        rules.push(rule(
            "IEquipment",
            "EqTypeDescription",
            ItemTable::Equipment,
            "EQUIPMENTTYPE",
            "SP_22",
            Spelling::Name,
        ));
    }
    if INSTRUMENT_CLASSES.contains(&class) {
        rules.push(rule(
            "IInstrument",
            "InstrumentType3",
            ItemTable::Instrument,
            "INSTRUMENTTYPE",
            "SP_28",
            Spelling::Target,
        ));
        rules.push(rule(
            "IInstrument",
            "ProcFunc",
            ItemTable::Instrument,
            "INSTRUMENTCLASS",
            "SP_29",
            Spelling::Target,
        ));
        rules.push(rule(
            "IInstrumentOcc",
            "InstrumentLocation",
            ItemTable::Instrument,
            "LOCATION",
            "SP_17",
            Spelling::Target,
        ));
    }
    match class {
        "PIDPipingComponent" => {
            rules.push(rule(
                "IPipingComponent",
                "CommoditySpecialtyType",
                ItemTable::PipingComp,
                "SP_ISSPECIALTYITEM",
                "",
                Spelling::SpecialtyItem,
            ));
            rules.push(rule(
                "IPipingComponent",
                "PipModelCode",
                ItemTable::PipingComp,
                "PIPINGCOMPTYPE",
                "SP_25",
                Spelling::Description,
            ));
        }
        "PIDNozzle" => {
            rules.push(rule(
                "IEquipmentComponent",
                "ProcEqpCompTypeDescription",
                ItemTable::Nozzle,
                "NOZZLETYPE",
                "SP_35",
                Spelling::Name,
            ));
            rules.push(rule(
                "IEquipmentComponent",
                "ProcessEqCompType2",
                ItemTable::Nozzle,
                "NOZZLETYPE",
                "SP_35",
                Spelling::Target,
            ));
        }
        "PIDPipingConnector" => {
            rules.push(rule(
                "IConnector",
                "FlowDirection",
                ItemTable::PipeRun,
                "FLOWDIRECTION",
                "SP_47",
                Spelling::Target,
            ));
            rules.push(rule(
                "IPipingConnector",
                "PipingConnectorType",
                ItemTable::PipeRun,
                "PIPERUNTYPE",
                "SP_14",
                Spelling::Target,
            ));
        }
        _ => {}
    }
    rules
}

/// The spelling of `code` under `rule`, or `None` when it has none.
fn spell(
    rule: &Rule,
    code: i64,
    map: Option<&DataMap>,
    retrieved: &HashMap<&str, Vec<&str>>,
) -> Option<String> {
    if let Spelling::SpecialtyItem = rule.spelling {
        return SPECIALTY_ITEM
            .iter()
            .find(|(index, _)| *index == code)
            .map(|(_, spelled)| (*spelled).to_string());
    }
    let map = map?;
    let entry = map.enum_numbered(rule.list, code)?;
    let text = match rule.spelling {
        Spelling::Target => return map.enum_target(rule.list, code),
        Spelling::RetrievedOnly => match retrieved.get(entry).map(Vec::as_slice) {
            Some([only]) => return Some(format!("@{only}")),
            _ => return None,
        },
        Spelling::Name => &map.enums.get(entry)?.name,
        Spelling::Description => &map.enums.get(entry)?.description,
        Spelling::SpecialtyItem => return None,
    };
    Some(text.clone()).filter(|text| !text.is_empty())
}

/// An instrument whose type is one the plant added and the map publishes
/// states neither `InstrumentType3` nor `ProcFunc`
/// ([`PLANT_ADDED_INSTRUMENT_TYPE`]); a plant-added type the map does not
/// publish leaves `ProcFunc` to its class.
fn instrument_rule_holds(rule: &Rule, key: &str, cells: &Cells, map: Option<&DataMap>) -> bool {
    let Some(kind) = cell(cells, ItemTable::Instrument, "INSTRUMENTTYPE", key) else {
        return true;
    };
    if kind < PLANT_ADDED_INSTRUMENT_TYPE {
        return true;
    }
    match rule.name {
        "InstrumentType3" => false,
        "ProcFunc" => map.and_then(|map| map.enum_target("SP_28", kind)).is_none(),
        _ => true,
    }
}

/// The `(attribute, value)` levels of one leaf of [`LEAF_LEVELS`].
type Levels = &'static [(&'static str, &'static str)];

/// The hierarchy of `class`, the column whose cell names the item's leaf
/// and that leaf's [`LEAF_LEVELS`]; `None` when the class carries no
/// hierarchy, no cell names a leaf the references show, or the item is an
/// instrument of a type the plant added, which states no level either
/// ([`PLANT_ADDED_INSTRUMENT_TYPE`]).
fn leaf_levels(
    class: &str,
    key: &str,
    cells: &Cells,
    map: &DataMap,
) -> Option<(Hierarchy, &'static str, Levels)> {
    let hierarchy = HIERARCHIES
        .iter()
        .find(|hierarchy| hierarchy.classes.contains(&class))
        .copied()?;
    if hierarchy.table == ItemTable::Instrument
        && cell(cells, ItemTable::Instrument, "INSTRUMENTTYPE", key)
            .is_some_and(|kind| kind >= PLANT_ADDED_INSTRUMENT_TYPE)
    {
        return None;
    }
    let (column, leaf) = hierarchy.columns.iter().rev().find_map(|&(column, list)| {
        let code = cell(cells, hierarchy.table, column, key)?;
        Some((column, map.enum_target(list, code)?))
    })?;
    let &(_, levels) = LEAF_LEVELS.iter().find(|(interface, levels)| {
        *interface == hierarchy.interface && levels.last().is_some_and(|(_, value)| *value == leaf)
    })?;
    Some((hierarchy, column, levels))
}

/// State the enumerated values of every item the drawing publishes, with
/// the levels of its leaf ([`LEAF_LEVELS`]), and the drawing's
/// [`DRAWING_CONSTANTS`], from the dump's codelist cells.
///
/// Without `datamap` only the values that need no map are stated: the
/// specialty flag and the drawing's constants. A value the item already
/// states is kept. Returns how many values were added.
///
/// # Errors
///
/// Returns [`ExpError::Table`] when a followed column of the dump is not
/// a `NUMBER` column of integers.
pub fn attach_codelist_values(
    drawing: &mut PidPublishDrawing,
    tables: &ExpTables,
    datamap: Option<&DataMap>,
) -> Result<usize, ExpError> {
    let cells = read_cells(tables)?;
    let retrieved = datamap
        .map(DataMap::enums_retrieved_only)
        .unwrap_or_default();
    let mut added = 0;
    for item in &mut drawing.items {
        let Some(class) = item.class else {
            continue;
        };
        let key = backup_uid_key(&item.uid);
        let before = item.attributes.len();
        for rule in rules_of(class) {
            if item
                .attributes
                .iter()
                .any(|stated| stated.interface == rule.interface && stated.name == rule.name)
            {
                continue;
            }
            if rule.table == ItemTable::Instrument
                && !instrument_rule_holds(&rule, &key, &cells, datamap)
            {
                continue;
            }
            let Some(code) = cell(&cells, rule.table, rule.column, &key) else {
                continue;
            };
            let Some(value) = spell(&rule, code, datamap, &retrieved) else {
                continue;
            };
            item.attributes.push(PidPublishAttribute {
                interface: rule.interface,
                name: rule.name,
                value,
                source: PidValueSource::CodelistCell {
                    table: rule.table.name(),
                    column: rule.column,
                    spelling: rule.spelling.label(),
                },
            });
        }
        if let Some((hierarchy, column, levels)) =
            datamap.and_then(|map| leaf_levels(class, &key, &cells, map))
        {
            for &(name, value) in levels {
                if item
                    .attributes
                    .iter()
                    .any(|stated| stated.interface == hierarchy.interface && stated.name == name)
                {
                    continue;
                }
                item.attributes.push(PidPublishAttribute {
                    interface: hierarchy.interface,
                    name,
                    value: value.to_string(),
                    source: PidValueSource::CodelistCell {
                        table: hierarchy.table.name(),
                        column,
                        spelling: "codelist_values::LEAF_LEVELS",
                    },
                });
            }
        }
        if item.attributes.len() > before {
            added += item.attributes.len() - before;
            item.attributes
                .sort_by(|a, b| (a.interface, a.name).cmp(&(b.interface, b.name)));
        }
    }
    for (name, value) in DRAWING_CONSTANTS {
        if drawing
            .drawing_attributes
            .iter()
            .any(|stated| stated.interface == "IDocument" && stated.name == name)
        {
            continue;
        }
        drawing.drawing_attributes.push(PidPublishAttribute {
            interface: "IDocument",
            name,
            value: value.to_string(),
            source: PidValueSource::ClassConstant,
        });
        added += 1;
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use super::*;
    use crate::publish::catalog;

    const CLASSES: [&str; 11] = [
        "PIDControlSystemFunction",
        "PIDHeatTransferEquipment",
        "PIDInlineInstrument",
        "PIDInstrument",
        "PIDMechanicalEquipment",
        "PIDNozzle",
        "PIDPipingComponent",
        "PIDPipingConnector",
        "PIDProcessEquipment",
        "PIDProcessVessel",
        "PIDSignalConnector",
    ];

    // The diff holds a run that states one of these on some object to it on
    // every object, so the list has to be what the rules and the leaf levels
    // state.
    #[test]
    fn the_rules_state_exactly_the_attributes_the_diff_holds_a_dump_run_to() {
        let mut stated: BTreeSet<(&str, &str)> = CLASSES
            .iter()
            .flat_map(|class| rules_of(class))
            .map(|rule| (rule.interface, rule.name))
            .collect();
        stated.extend(DRAWING_CONSTANTS.map(|(name, _)| ("IDocument", name)));
        stated.extend(LEAF_LEVELS.iter().flat_map(|&(interface, levels)| {
            levels.iter().map(move |&(name, _)| (interface, name))
        }));
        let held: BTreeSet<(&str, &str)> = catalog::DUMP_CODELIST_ATTRIBUTES.into_iter().collect();
        assert_eq!(stated, held);
        for (interface, attribute) in held {
            assert!(
                catalog::excluded_attribute(interface, attribute).is_some(),
                "{interface}.{attribute} is excused in a run without the dump"
            );
        }
    }

    // A leaf is looked up by its interface and its value, so no two rows may
    // share them, and a row on no hierarchy's interface would never be read.
    #[test]
    fn each_leaf_has_one_row_on_the_interface_of_a_hierarchy() {
        let mut leaves = BTreeSet::new();
        for (interface, levels) in LEAF_LEVELS {
            assert!(
                HIERARCHIES
                    .iter()
                    .any(|hierarchy| hierarchy.interface == interface),
                "{interface} carries no hierarchy"
            );
            let (_, leaf) = levels.last().expect("a row ends at its leaf");
            assert!(
                leaves.insert((interface, *leaf)),
                "{interface} {leaf} has two rows"
            );
        }
    }

    fn committed(relative: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    // The committed package's dump and map on its own drawing: how many
    // values each attribute takes, and the spelling of one of each.
    #[test]
    fn the_committed_dwg_drawing_takes_these_values() {
        let dump = std::fs::read(committed(
            "test-file/backup-test/DWG-0202GP06-01_p/Export.dmp",
        ))
        .expect("the committed dump");
        let tables = ExpTables::read(&dump).expect("the dump reads");
        let map = crate::backup::read_datamap(&committed(
            "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~804.zip",
        ))
        .expect("the committed map");
        let mut drawing =
            crate::publish::load_publish_from_pid(&committed("test-file/DWG-0202GP06-01.pid"))
                .expect("the drawing loads");
        super::super::oracle_exp::attach_dump_only_components(&mut drawing, &tables)
            .expect("components");
        let added = attach_codelist_values(&mut drawing, &tables, Some(&map)).expect("values");

        let mut by_attribute = BTreeMap::<String, (usize, String)>::new();
        for attribute in drawing
            .items
            .iter()
            .flat_map(|item| &item.attributes)
            .chain(&drawing.drawing_attributes)
            .filter(|attribute| {
                matches!(
                    attribute.source,
                    PidValueSource::CodelistCell { .. } | PidValueSource::ClassConstant
                )
            })
        {
            let entry = by_attribute
                .entry(format!("{}.{}", attribute.interface, attribute.name))
                .or_insert((0, attribute.value.clone()));
            entry.0 += 1;
        }
        let counted: usize = by_attribute.values().map(|(count, _)| count).sum();
        assert_eq!(counted, added);
        let found: Vec<(&str, usize, &str)> = by_attribute
            .iter()
            .map(|(key, (count, first))| (key.as_str(), *count, first.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("IConnector.FlowDirection", 5, "@EE872"),
                ("IDocument.DocCategory", 1, "P&ID Documents"),
                ("IDocument.DocType", 1, "P&ID"),
                ("IEquipment.EqTypeDescription", 1, "Open top tank"),
                (
                    "IEquipmentComponent.ProcEqpCompTypeDescription",
                    3,
                    "Flanged Nozzle"
                ),
                ("IEquipmentComponent.ProcessEqCompType1", 3, "@EE6D4"),
                (
                    "IEquipmentComponent.ProcessEqCompType2",
                    3,
                    "@{B88907F5-D4FC-49D8-BA8E-C1F76F392A52}"
                ),
                ("IPBSItem.ConstructionStatus", 15, "@NewConstruction"),
                (
                    "IPBSItem.ConstructionStatus2",
                    15,
                    "@{78398AB4-9F3D-11D6-BDA7-00104BCC2B69}"
                ),
                (
                    "IPipingComponent.CommoditySpecialtyType",
                    2,
                    "@{5F7F8F6E-BC29-11D6-BDBC-00104BCC2B69}"
                ),
                ("IPipingComponent.PipModelCode", 2, "Conduit gate valve"),
                (
                    "IPipingComponent.PipingComponentType1",
                    1,
                    "@{81CD929C-BC07-11D6-BDBC-00104BCC2B69}"
                ),
                (
                    "IPipingComponent.PipingComponentType2",
                    1,
                    "@{81CD9804-BC07-11D6-BDBC-00104BCC2B69}"
                ),
                (
                    "IPipingComponent.PipingComponentType3",
                    1,
                    "@{81CD9816-BC07-11D6-BDBC-00104BCC2B69}"
                ),
                ("IPipingConnector.PipingConnectorType", 7, "@EE690"),
            ]
        );

        // Without the map only what needs none is stated.
        let mut bare =
            crate::publish::load_publish_from_pid(&committed("test-file/DWG-0202GP06-01.pid"))
                .expect("the drawing loads");
        let without = attach_codelist_values(&mut bare, &tables, None).expect("values");
        let specialty = bare
            .items
            .iter()
            .flat_map(|item| &item.attributes)
            .filter(|attribute| attribute.name == "CommoditySpecialtyType")
            .count();
        assert_eq!(without, specialty + DRAWING_CONSTANTS.len());
    }
}
