//! `PublishItemCatalog` — the single source of truth for a `SmartPlant`
//! publish item type (wayfinder M4 · spec #18 · shape decided by #15 ·
//! seed row-set reconciled in #16 /
//! `docs/analysis/2026-10-19-three-publish-tables-seed-catalog.md`).
//!
//! Today the same facts live hand-synced in
//! [`super::sqlite_load`] (`subtables_for_item_type`),
//! [`super::xml_writer`] (`write_business_objects` dispatch +
//! `a01_object_rank`), [`super::diff`] (`supported_pid_tags`), and the
//! `.pid` line (`pid_load::CLASS_OF_ITEM_TYPE` / `branch_point_class`).
//! The catalog models **both** publish lines in one row per item type;
//! those tables become consumers of it, one at a time (A2–A4).
//!
//! Special rows: `"*"` is the generic-object fallback arm (owns
//! `PIDItem`, excluded from the supported-tag derivation), `"$document"`
//! owns the document-level tags (`PIDDrawing` / `PIDRepresentation`),
//! and `"$derived"` owns `.pid`-line tags emitted by enrichment rather
//! than by an item row (exp ports, synthesized `PIDPipeline`).

// Consumers land in A2–A4 (`sqlite_load`, `xml_writer`, `diff`); until
// then the table and its accessors are read only by the consistency
// tests, so dead-code warnings are expected and suppressed.
#![allow(dead_code)]

use super::model::{PublishDrawing, PublishError, PublishObject};
use super::xml_writer;

/// Normalized DB-line writer arm. The real `write_*` functions take
/// heterogeneous arguments (`&PublishDrawing` / `PublishStyle` /
/// `&PublishObject` / a port count); the catalog stores `fn` pointers
/// behind this one signature — thin adapters below do the narrowing.
/// Follows the `SheetRecordFamily` (M3) precedent: `fn` pointers in a
/// `const` table, no macro machinery.
pub(crate) type DbWriterFn =
    fn(&mut String, &PublishObject, &PublishDrawing) -> Result<(), PublishError>;

/// How the `.pid` line decides an item type's published class. The
/// catalogue keeps this as a discriminant rather than an `fn` pointer:
/// the real rule (`pid_load::instrument_class` falling back to
/// `instrument_class_by_library`) reads both the `SymbolAttr` row and
/// the representation's symbol path, which is more context than a row
/// should smuggle into a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PidClassRule {
    /// Every item of this type publishes under one fixed class tag.
    Fixed(&'static str),
    /// Instruments split three ways: `SymbolAttr` first (`Type` /
    /// `Class` text), then the `Instrumentation` library folder —
    /// `PIDInstrument` / `PIDControlSystemFunction` /
    /// `PIDInlineInstrument`.
    InstrumentByEvidence,
}

/// One catalog row: every fact the crate knows about a publish item
/// type, for whichever line(s) produce it. `db_*` fields describe the
/// SQLite/MDF line; `pid_*` fields describe the `.pid` line. A `None`
/// on one side means that line never produces this item type.
#[derive(Debug)]
pub(crate) struct PublishItemRow {
    /// Canonical `ItemTypeName` / `ModelItemType` this row dispatches on.
    pub item_type: &'static str,
    /// Alternate names that share this row's chain and writer
    /// (`Instrument` ↔ `InstrFunction`, `Note` ↔ `ItemNote`).
    pub aliases: &'static [&'static str],
    /// SQLite subtable merge chain (`subtables_for_item_type`), later
    /// rows overwrite earlier on column collision. `None` when the DB
    /// line never sees this item type.
    pub db_subtables: Option<&'static [&'static str]>,
    /// DB-line writer arm. `None` rows with a `db_subtables` chain fall
    /// through to the `"*"` row's generic writer (today: `Exchanger`,
    /// `Mechanical`, pending a dedicated tag — TODO A12+).
    pub db_writer: Option<DbWriterFn>,
    /// Every tag the DB arm emits, including derived children (a
    /// `PipeRun` row emits `PIDPipeline` + `PIDPipingConnector` + two
    /// `PIDPipingPort` + `PIDProcessPoint`).
    pub db_tags: &'static [&'static str],
    /// `.pid`-line class rule; `None` when the line never classes this
    /// item type (either it never appears, or it stays unclassed like
    /// `OPC` does without an exp dump).
    pub pid_class: Option<PidClassRule>,
    /// Every tag the `.pid` line emits for this item type, including
    /// directly-derived children (`.PPT` process points, `.BPT`
    /// branch points off the item's representations).
    pub pid_tags: &'static [&'static str],
    /// `PublishStyle::A01` ordering rank (`Vessel` 0 / `Nozzle` 1 /
    /// `PipeRun` 2 / everything else 9).
    pub a01_rank: u8,
    /// `true` while the row's DB subtable chain is provisional — the
    /// Stage-4 branch-point rows wait on a DWG-mirror fixture to
    /// confirm their table layout.
    pub provisional: bool,
    /// `true` when the row is registered for forward compatibility
    /// only (`PipingPoint` — the loader stopped injecting standalone
    /// rows in A13). Kept in the table so the fact is queryable.
    pub dead: bool,
}

fn run_pipe_run(
    buf: &mut String,
    obj: &PublishObject,
    drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_pipeline(buf, obj, drawing.style)?;
    xml_writer::write_piping_connector(buf, obj, drawing.style)?;
    xml_writer::write_derived_connector_endpoints(buf, obj)
}

fn run_instrument(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_control_system_function(buf, obj)?;
    xml_writer::write_derived_instr_signal_ports(
        buf,
        obj,
        xml_writer::INSTR_DERIVED_SIGNAL_PORT_COUNT,
    )
}

fn run_piping_point(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_piping_port(buf, obj)
}

fn run_note(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_item_note(buf, obj)
}

fn run_piping_comp(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_piping_component(buf, obj)
}

fn run_signal_run(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_signal_connector(buf, obj)
}

fn run_piping_branch_point(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_piping_branch_point(buf, obj)
}

fn run_branch_point(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_pid_branch_point(buf, obj)
}

fn run_generic(
    buf: &mut String,
    obj: &PublishObject,
    _drawing: &PublishDrawing,
) -> Result<(), PublishError> {
    xml_writer::write_generic_object(buf, obj, obj.item_type_name.as_str())
}

/// The catalog table. Rows follow the DB dispatcher's order, then the
/// `.pid`-only item types, then the special `"*"` / `"$document"` /
/// `"$derived"` rows.
pub(crate) static PUBLISH_ITEM_CATALOG: &[PublishItemRow] = &[
    PublishItemRow {
        item_type: "Vessel",
        aliases: &[],
        db_subtables: Some(&[
            "T_PlantItem",
            "T_Equipment",
            "T_ProcessEquipment",
            "T_Vessel",
        ]),
        db_writer: Some(xml_writer::write_process_vessel),
        db_tags: &["PIDProcessVessel"],
        pid_class: Some(PidClassRule::Fixed("PIDProcessVessel")),
        pid_tags: &["PIDProcessVessel"],
        a01_rank: 0,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "Nozzle",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_EquipComponent", "T_Nozzle"]),
        db_writer: Some(xml_writer::write_nozzle),
        db_tags: &["PIDNozzle"],
        pid_class: Some(PidClassRule::Fixed("PIDNozzle")),
        pid_tags: &["PIDNozzle"],
        a01_rank: 1,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "PipeRun",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_Connector", "T_PipeRun", "T_Pipeline"]),
        db_writer: Some(run_pipe_run),
        db_tags: &[
            "PIDPipeline",
            "PIDPipingConnector",
            "PIDPipingPort",
            "PIDProcessPoint",
        ],
        pid_class: Some(PidClassRule::Fixed("PIDPipingConnector")),
        pid_tags: &[
            "PIDPipingConnector",
            "PIDProcessPoint",
            "PIDPipingBranchPoint",
        ],
        a01_rank: 2,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "PipingPoint",
        aliases: &[],
        db_subtables: Some(&["T_PipingPoint"]),
        db_writer: Some(run_piping_point),
        db_tags: &["PIDPipingPort"],
        pid_class: None,
        pid_tags: &[],
        a01_rank: 9,
        provisional: false,
        // The loader stopped injecting standalone PipingPoint rows in
        // A13; both facets stay registered for forward compatibility.
        dead: true,
    },
    PublishItemRow {
        item_type: "Instrument",
        aliases: &["InstrFunction"],
        db_subtables: Some(&["T_PlantItem", "T_Instrument", "T_InstrFunction"]),
        db_writer: Some(run_instrument),
        db_tags: &["PIDControlSystemFunction", "PIDSignalPort"],
        pid_class: Some(PidClassRule::InstrumentByEvidence),
        pid_tags: &[
            "PIDInstrument",
            "PIDControlSystemFunction",
            "PIDInlineInstrument",
        ],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "Note",
        aliases: &["ItemNote"],
        db_subtables: Some(&["T_ItemNote"]),
        db_writer: Some(run_note),
        db_tags: &["PIDNote"],
        // The `.pid` ModelItemType is `ItemNote` — alias lookup.
        pid_class: Some(PidClassRule::Fixed("PIDNote")),
        pid_tags: &["PIDNote"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "PipingComp",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_InlineComp", "T_PipingComp"]),
        db_writer: Some(run_piping_comp),
        db_tags: &["PIDPipingComponent"],
        pid_class: Some(PidClassRule::Fixed("PIDPipingComponent")),
        pid_tags: &["PIDPipingComponent"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "SignalRun",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_Connector", "T_SignalRun"]),
        db_writer: Some(run_signal_run),
        db_tags: &["PIDSignalConnector"],
        pid_class: Some(PidClassRule::Fixed("PIDSignalConnector")),
        pid_tags: &["PIDSignalConnector", "PIDSignalBranchPoint"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "Exchanger",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_Equipment", "T_Exchanger"]),
        // No dedicated DB writer yet — falls through to the `"*"` row.
        db_writer: None,
        db_tags: &[],
        pid_class: Some(PidClassRule::Fixed("PIDHeatTransferEquipment")),
        pid_tags: &["PIDHeatTransferEquipment"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "Mechanical",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem", "T_Equipment", "T_Mechanical"]),
        db_writer: None,
        db_tags: &[],
        pid_class: Some(PidClassRule::Fixed("PIDMechanicalEquipment")),
        pid_tags: &["PIDMechanicalEquipment"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "BranchPoint",
        aliases: &[],
        // Provisional: T_PlantItem carries Name so the writer has
        // IObject input; confirmed when a DWG-mirror fixture lands.
        db_subtables: Some(&["T_PlantItem"]),
        db_writer: Some(run_branch_point),
        db_tags: &["PIDBranchPoint"],
        pid_class: None,
        pid_tags: &[],
        a01_rank: 9,
        provisional: true,
        dead: false,
    },
    PublishItemRow {
        item_type: "PipingBranchPoint",
        aliases: &[],
        db_subtables: Some(&["T_PlantItem"]),
        db_writer: Some(run_piping_branch_point),
        db_tags: &["PIDPipingBranchPoint"],
        pid_class: None,
        pid_tags: &[],
        a01_rank: 9,
        provisional: true,
        dead: false,
    },
    // `.pid`-line-only item types: no DB subtable chain or writer arm.
    PublishItemRow {
        item_type: "EquipmentOther",
        aliases: &[],
        db_subtables: None,
        db_writer: None,
        db_tags: &[],
        pid_class: Some(PidClassRule::Fixed("PIDProcessEquipment")),
        pid_tags: &["PIDProcessEquipment"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "OPC",
        aliases: &[],
        db_subtables: None,
        db_writer: None,
        db_tags: &[],
        // Off-page connectors never carry a class on the item row
        // (`UNCLASSED_ITEM_TYPES`); their `PIDBranchPoint` arrives via
        // exp enrichment (`oracle_exp` opc_points), so the tag is
        // attributed here.
        pid_class: None,
        pid_tags: &["PIDBranchPoint"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "*",
        aliases: &[],
        db_subtables: None,
        db_writer: Some(run_generic),
        // `PIDItem` is real output but a placeholder: excluded from the
        // supported-tag derivation so a reference `PIDItem` still shows
        // up as backlog rather than covered.
        db_tags: &["PIDItem"],
        pid_class: None,
        pid_tags: &[],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "$document",
        aliases: &[],
        db_subtables: None,
        db_writer: None,
        db_tags: &["PIDDrawing", "PIDRepresentation"],
        pid_class: None,
        pid_tags: &["PIDDrawing", "PIDRepresentation"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
    PublishItemRow {
        item_type: "$derived",
        aliases: &[],
        db_subtables: None,
        db_writer: None,
        db_tags: &[],
        pid_class: None,
        // `.pid` line: exp port enrichments and the T_PIPERUN-synthesized
        // pipeline are not attributable to one item row.
        pid_tags: &["PIDPipeline", "PIDPipingPort", "PIDSignalPort"],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
];

/// The whole catalog, as a slice.
pub(crate) fn publish_item_catalog() -> &'static [PublishItemRow] {
    PUBLISH_ITEM_CATALOG
}

/// Look up a row by `ItemTypeName` / `ModelItemType`, following
/// `aliases`.
pub(crate) fn catalog_row(item_type_name: &str) -> Option<&'static PublishItemRow> {
    PUBLISH_ITEM_CATALOG
        .iter()
        .find(|row| row.item_type == item_type_name || row.aliases.contains(&item_type_name))
}

/// The catalog-derived counterpart of `diff::supported_pid_tags`: the
/// union of `db_tags` over every row except the `"*"` fallback, sorted
/// and deduplicated. A2–A4 rewire `supported_pid_tags()` to this; the
/// consistency test pins equality with today's hand-listed set.
pub(crate) fn supported_db_tags() -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = PUBLISH_ITEM_CATALOG
        .iter()
        .filter(|row| row.item_type != "*")
        .flat_map(|row| row.db_tags.iter().copied())
        .collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}

/// The `.pid`-line counterpart: union of `pid_tags` over every row,
/// sorted and deduplicated. Not wired to a consumer yet — the `.pid`
/// coverage report keeps its own bookkeeping until then.
#[cfg(test)]
fn supported_pid_line_tags() -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = PUBLISH_ITEM_CATALOG
        .iter()
        .flat_map(|row| row.pid_tags.iter().copied())
        .collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_addressable_and_unique() {
        let mut names: Vec<&str> = Vec::new();
        for row in PUBLISH_ITEM_CATALOG {
            assert!(!row.item_type.is_empty(), "row with empty item_type");
            names.push(row.item_type);
            names.extend(row.aliases.iter().copied());
        }
        names.sort_unstable();
        let mut dup = names.clone();
        dup.dedup();
        assert_eq!(names, dup, "duplicate item_type / alias in catalog");
    }

    #[test]
    fn every_named_row_carries_at_least_one_line() {
        for row in PUBLISH_ITEM_CATALOG {
            if row.item_type.starts_with(['*', '$']) {
                continue;
            }
            let covered = row.db_subtables.is_some()
                || row.db_writer.is_some()
                || row.pid_class.is_some()
                || !row.pid_tags.is_empty();
            assert!(
                covered,
                "row `{}` is empty on both publish lines",
                row.item_type
            );
        }
    }

    #[test]
    fn writer_rows_declare_their_tags() {
        for row in PUBLISH_ITEM_CATALOG {
            if row.db_writer.is_some() {
                assert!(
                    !row.db_tags.is_empty(),
                    "row `{}` has a DB writer but declares no db_tags",
                    row.item_type
                );
            }
            if let Some(PidClassRule::Fixed(tag)) = row.pid_class {
                assert!(
                    row.pid_tags.contains(&tag),
                    "row `{}` pid_class `{tag}` absent from its pid_tags",
                    row.item_type
                );
            }
        }
    }

    #[test]
    fn derived_db_tags_match_supported_pid_tags() {
        // A4: `diff::supported_pid_tags` now derives from
        // `supported_db_tags`, so comparing against it would be
        // tautological. Pin the historical tag list literally —
        // any coverage-visible change must be a deliberate,
        // ratcheted catalog edit (ADR-0002).
        let derived = supported_db_tags();
        let hand_listed: Vec<&'static str> = vec![
            "PIDBranchPoint",
            "PIDControlSystemFunction",
            "PIDDrawing",
            "PIDNote",
            "PIDNozzle",
            "PIDPipeline",
            "PIDPipingBranchPoint",
            "PIDPipingComponent",
            "PIDPipingConnector",
            "PIDPipingPort",
            "PIDProcessPoint",
            "PIDProcessVessel",
            "PIDRepresentation",
            "PIDSignalConnector",
            "PIDSignalPort",
        ];
        assert_eq!(
            derived, hand_listed,
            "catalog-derived DB tag set diverged from the pinned list"
        );
    }

    #[test]
    fn a01_rank_pins_the_three_named_ranks() {
        let rank = |name: &str| catalog_row(name).map(|row| row.a01_rank);
        assert_eq!(rank("Vessel"), Some(0));
        assert_eq!(rank("Nozzle"), Some(1));
        assert_eq!(rank("PipeRun"), Some(2));
        assert_eq!(rank("Exchanger"), Some(9));
        assert_eq!(rank("anything-unlisted"), None);
    }

    #[test]
    fn alias_lookup_resolves_shared_writer_names() {
        let instrument = catalog_row("InstrFunction").unwrap();
        assert_eq!(instrument.item_type, "Instrument");
        let note = catalog_row("ItemNote").unwrap();
        assert_eq!(note.item_type, "Note");
    }

    #[test]
    fn pid_line_tag_inventory_covers_the_six_unregistered_classes() {
        let tags = supported_pid_line_tags();
        for tag in [
            "PIDProcessEquipment",
            "PIDMechanicalEquipment",
            "PIDHeatTransferEquipment",
            "PIDInstrument",
            "PIDInlineInstrument",
            "PIDSignalBranchPoint",
        ] {
            assert!(
                tags.contains(&tag),
                ".pid-line tag `{tag}` missing from catalog"
            );
        }
    }
}
