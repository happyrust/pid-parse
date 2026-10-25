//! `PublishItemCatalog` — the single source of truth for a `SmartPlant`
//! publish item type (wayfinder M4 · spec #18 · shape decided by #15 ·
//! seed row-set reconciled in #16 /
//! `docs/analysis/2026-10-19-three-publish-tables-seed-catalog.md`).
//!
//! The catalog models **both** publish lines in one row per item type.
//! The database line reads it as of A2–A4: `super::sqlite_load` resolves
//! its subtable chains through [`catalog_row`], `super::xml_writer`
//! dispatches `write_business_objects` through [`PublishItemRow::db_writer`]
//! and reads [`PublishItemRow::a01_rank`] (A1 seeded the table in
//! `60f1b25`, A2 `7c79749`, A3 `4735be9`, A4 `6881c18`).
//!
//! Both lines also share one tag inventory: `super::diff::supported_pid_tags`
//! is the union of [`supported_db_tags`] and [`supported_pid_line_tags`],
//! so a reference carrying a class only the `.pid` line emits is not
//! reported as writer backlog. `super::pid_load` classes an item through
//! [`PublishItemRow::pid_class`] — the hand-written
//! `CLASS_OF_ITEM_TYPE` table it used to keep is gone. The one rule still
//! decided outside the catalog is which of a pipe run's `pid_tags`
//! becomes its `.BPT` branch point (`pid_load::branch_point_class`).
//!
//! Special rows: `"*"` is the generic-object fallback arm (owns
//! `PIDItem`, excluded from the supported-tag derivation), `"$document"`
//! owns the document-level tags (`PIDDrawing` / `PIDRepresentation`),
//! and `"$derived"` owns `.pid`-line tags emitted by enrichment rather
//! than by an item row (exp ports, synthesized `PIDPipeline`).

use super::catalog_index;
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

/// Where a `.pid`-line tag's objects come from, on the row that owns it.
///
/// `super::pid_writer`'s `_Coverage.json` tallies the tags whose count
/// comes from a drawing collection rather than from an item row; a role
/// names that collection, so the report never spells a tag its own
/// catalog does not declare. `Class` / `Document` / `Enrichment` tags are
/// tallied elsewhere (item rows, the drawing itself, the exp enrichment)
/// and are listed here only to keep the inventory complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PidTagRole {
    /// The item's own published class (`pid_class` for a `Fixed` row).
    Class,
    /// One `.PPT` process point per published pipe run.
    ProcessPoint,
    /// A `.BPT` branch point off the item's representations.
    BranchPoint,
    /// A port row the exp dump supplies for a parent this line publishes.
    Port,
    /// Document level: the drawing and its representations.
    Document,
    /// Enrichment the file cannot state (a synthesized `PIDPipeline`, the
    /// exp-side `PIDBranchPoint` an `OPC` turns into).
    Enrichment,
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
    /// branch points off the item's representations), each with the
    /// [`PidTagRole`] naming where its objects come from.
    pub pid_tags: &'static [(&'static str, PidTagRole)],
    /// `PublishStyle::A01` ordering rank (`Vessel` 0 / `Nozzle` 1 /
    /// `PipeRun` 2 / everything else 9).
    pub a01_rank: u8,
    /// `true` while the row's DB subtable chain is provisional — the
    /// Stage-4 branch-point rows wait on a DWG-mirror fixture to
    /// confirm their table layout.
    #[allow(dead_code)] // bookkeeping only: no arm branches on it yet.
    pub provisional: bool,
    /// `true` when the row is registered for forward compatibility
    /// only (`PipingPoint` — the loader stopped injecting standalone
    /// rows in A13). Kept in the table so the fact is queryable.
    #[allow(dead_code)] // bookkeeping only: no arm branches on it yet.
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
        pid_tags: &[("PIDProcessVessel", PidTagRole::Class)],
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
        pid_tags: &[("PIDNozzle", PidTagRole::Class)],
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
            ("PIDPipingConnector", PidTagRole::Class),
            ("PIDProcessPoint", PidTagRole::ProcessPoint),
            ("PIDPipingBranchPoint", PidTagRole::BranchPoint),
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
            ("PIDInstrument", PidTagRole::Class),
            ("PIDControlSystemFunction", PidTagRole::Class),
            ("PIDInlineInstrument", PidTagRole::Class),
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
        pid_tags: &[("PIDNote", PidTagRole::Class)],
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
        pid_tags: &[("PIDPipingComponent", PidTagRole::Class)],
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
        pid_tags: &[
            ("PIDSignalConnector", PidTagRole::Class),
            ("PIDSignalBranchPoint", PidTagRole::BranchPoint),
        ],
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
        pid_tags: &[("PIDHeatTransferEquipment", PidTagRole::Class)],
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
        pid_tags: &[("PIDMechanicalEquipment", PidTagRole::Class)],
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
        pid_tags: &[("PIDProcessEquipment", PidTagRole::Class)],
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
        pid_tags: &[("PIDBranchPoint", PidTagRole::Enrichment)],
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
        pid_tags: &[
            ("PIDDrawing", PidTagRole::Document),
            ("PIDRepresentation", PidTagRole::Document),
        ],
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
        pid_tags: &[
            ("PIDPipeline", PidTagRole::Enrichment),
            ("PIDPipingPort", PidTagRole::Port),
            ("PIDSignalPort", PidTagRole::Port),
        ],
        a01_rank: 9,
        provisional: false,
        dead: false,
    },
];

/// The whole catalog, as a slice.
#[allow(dead_code)] // No caller yet: the queryable form of the table above.
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

/// The catalog-derived source of `diff::supported_pid_tags`: the union of
/// `db_tags` over every row except the `"*"` fallback, sorted and
/// deduplicated. A4 (`6881c18`) made `diff` read this; the consistency
/// test pins equality with the hand-listed set it replaced.
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

/// The `.pid`-line counterpart of [`supported_db_tags`]: the union of
/// `pid_tags` over every row, sorted and deduplicated.
/// `super::diff::supported_pid_tags` merges it with the DB set, so a
/// reference carrying a class only this line emits (`PIDInstrument`,
/// `PIDInlineInstrument`, the three equipment classes `EquipmentOther` /
/// `Exchanger` / `Mechanical` split into, `PIDSignalBranchPoint`) is
/// classified as covered rather than as backlog.
pub(crate) fn supported_pid_line_tags() -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = pid_line_tag_roles().iter().map(|(tag, _)| *tag).collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}

/// Every `.pid`-line tag with the [`PidTagRole`] its row gives it, one
/// entry per tag (first registration wins) and sorted by tag. This is the
/// inventory `super::pid_writer` tallies: matching on the role is how the
/// `_Coverage.json` report decides which drawing collection a class's
/// count comes from, instead of naming the tags itself.
pub(crate) fn pid_line_tag_roles() -> &'static [(&'static str, PidTagRole)] {
    static ROLES: std::sync::LazyLock<Vec<(&'static str, PidTagRole)>> =
        std::sync::LazyLock::new(|| {
            let mut tags: Vec<(&'static str, PidTagRole)> = Vec::new();
            for (tag, role) in PUBLISH_ITEM_CATALOG.iter().flat_map(|row| row.pid_tags) {
                if !tags.iter().any(|(seen, _)| seen == tag) {
                    tags.push((*tag, *role));
                }
            }
            tags.sort_unstable_by_key(|(tag, _)| *tag);
            tags
        });
    ROLES.as_slice()
}

/// The tag's `.pid`-line objects come from the exp dump (`Port`) or from
/// enrichment the file cannot state (`Enrichment`), and no row gives the tag a
/// role the file states. Returns the owning row's `item_type` and the role.
///
/// `None` for a tag the catalog gives a file-stateable role
/// (`Class` / `ProcessPoint` / `BranchPoint` / `Document`), for a tag no row
/// declares, and for a tag no row gives a `pid_tags` entry at all.
pub(crate) fn pid_file_cannot_state(tag: &str) -> Option<(&'static str, PidTagRole)> {
    let mut found = None;
    for row in PUBLISH_ITEM_CATALOG {
        for (declared, role) in row.pid_tags {
            if *declared != tag {
                continue;
            }
            match role {
                PidTagRole::Port | PidTagRole::Enrichment => {
                    found = found.or(Some((row.item_type, *role)));
                }
                _ => return None,
            }
        }
    }
    found
}

/// One class of attributes the vendor reference carries and this line cannot
/// state, with the reason it is not published.
///
/// The membership is **measured**, not guessed: it is the union of the
/// reference-only `(interface, attribute)` keys of the five corpus vendor
/// references (`A` / `B` / `CD` / `lowA` / `I井场`) plus the A01 test-file
/// reference — the G2 census `contract-work/g2-evidence/devin/ref-only-inventory.log`.
/// The rule that admits a member: the plant database is the attribute's only
/// source — the `.pid` file's own attribute map (`super::pid_load`) and the exp
/// tables (`super::oracle_exp`) state it for no object of that interface. The
/// ghost-probe tests in `diff.rs` keep the list honest: an attribute this line
/// never publishes and this table does not name stays a finding.
pub(crate) struct ExcludedAttributeClass {
    /// Short name of the class, printed in the accounting.
    pub class: &'static str,
    /// Why the plant database, and not this line, supplies it.
    pub reason: &'static str,
    /// The `(interface, attributes)` pairs this class covers, measured.
    pub members: &'static [(&'static str, &'static [&'static str])],
}

/// The attribute classes the plant database supplies and the `.pid` publish
/// line therefore never states: the measured union behind
/// [`ExcludedAttributeClass`]. `IObject.Name` and `IObject.UID` are never
/// members — this line states both, so they stay in the diff.
pub(crate) static EXCLUDED_ATTRIBUTE_CLASSES: &[ExcludedAttributeClass] = &[
    ExcludedAttributeClass {
        class: "plant breakdown structure",
        reason: "PBS item columns the plant breakdown structure supplies: the .pid file \
                 states the item's tag and geometry, never its breakdown-structure status.",
        members: &[("IPBSItem", &["ConstructionStatus", "ConstructionStatus2"])],
    },
    ExcludedAttributeClass {
        class: "piping specification",
        reason: "piping specification columns the piping spec database supplies: a .pid file \
                 states the run and the component's tag, not the resolved spec row.",
        members: &[
            ("IInsulatedItem", &["InsulSpec"]),
            ("IPipeCrossSectionItem", &["NominalDiameter"]),
            (
                "IPipingComponent",
                &[
                    "CommoditySpecialtyType",
                    "PipModelCode",
                    "PipingComponentType1",
                    "PipingComponentType2",
                    "PipingComponentType3",
                ],
            ),
            ("IPipingConnector", &["PipingConnectorType"]),
            ("IPipingSpecifiedItem", &["PipingMaterialsClass"]),
            ("ISpecifiedMatlItem", &["LongMaterialDescription3"]),
        ],
    },
    ExcludedAttributeClass {
        class: "instrument index",
        reason: "instrument index columns the instrumentation database supplies: a .pid file \
                 states the symbol and its loop, not the index row.",
        members: &[
            (
                "IInstrument",
                &["InstrumentType", "InstrumentType3", "ProcFunc"],
            ),
            ("IInstrumentOcc", &["InstrumentLocation"]),
            (
                "INamedInstrument",
                &["InstrTagSequenceNo", "MeasuredVariable"],
            ),
        ],
    },
    ExcludedAttributeClass {
        class: "equipment register",
        reason: "equipment register columns the equipment database supplies: a .pid file \
                 states the symbol, not the register row.",
        members: &[
            (
                "IEquipment",
                &[
                    "EqType0",
                    "EqType1",
                    "EqType2",
                    "EqType3",
                    "EqType4",
                    "EqType5",
                    "EqTypeDescription",
                    "EquipmentTrimSpec",
                ],
            ),
            (
                "IEquipmentComponent",
                &[
                    "ProcEqpCompTypeDescription",
                    "ProcessEqCompType1",
                    "ProcessEqCompType2",
                ],
            ),
            ("INamedEquipment", &["EquipPrefix"]),
        ],
    },
    ExcludedAttributeClass {
        class: "connector and port model",
        reason: "connector and port model columns the plant database supplies: a .pid file \
                 states the run's ends, not the resolved connector row.",
        members: &[
            (
                "IConnector",
                &["FlowDirection", "RepresentationsAreAllZeroLength"],
            ),
            ("IInlineComponent", &["IsFlowDirectional"]),
            ("INamedPipingConnector", &["PipingConnectorSeqNo"]),
            ("IPressureReliefItem", &["ReliefPressure"]),
        ],
    },
    ExcludedAttributeClass {
        class: "document metadata",
        reason: "document metadata columns the document register supplies: this line states \
                 the drawing's name and uid, not the register's category/subtype/title.",
        members: &[("IDocument", &["DocCategory", "DocTitle", "DocType"])],
    },
    ExcludedAttributeClass {
        class: "typical and status flags",
        reason: "the typical flag the plant database supplies: a .pid file states the \
                 typical's geometry, not the register flag.",
        members: &[("IPIDTypical", &["IsTypical"])],
    },
    ExcludedAttributeClass {
        class: "item identity columns",
        reason: "the item's own identity columns the plant database supplies: a differing \
                 IObject.Name or IObject.UID stays in the diff.",
        members: &[("IObject", &["Description", "ItemTag"])],
    },
];

/// The exclusion class covering `interface`.`attribute`, if any.
///
/// `None` when no [`EXCLUDED_ATTRIBUTE_CLASSES`] member names the pair —
/// such a key stays a finding.
pub(crate) fn excluded_attribute(
    interface: &str,
    attribute: &str,
) -> Option<&'static ExcludedAttributeClass> {
    EXCLUDED_ATTRIBUTE_CLASSES.iter().find(|class| {
        class
            .members
            .iter()
            .any(|(iface, attributes)| *iface == interface && attributes.contains(&attribute))
    })
}

/// The three-set reconciliation between a parsed `CatalogIndex.xml`
/// ([`catalog_index::CatalogIndexDocument`]) and `PUBLISH_ITEM_CATALOG`.
///
/// The index side names an item type the way the plant spells it; the
/// catalog side answers with the row that dispatches that name — either
/// the row's own `item_type` or one of its `aliases` (`Note` answers for
/// `ItemNote`, `Instrument` for `InstrFunction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIndexCrossCheck {
    /// Index-side `ItemType`s some catalog row claims — the **index
    /// spelling** (`ItemNote`, not the row's `Note`), sorted.
    pub in_both: Vec<String>,
    /// Index-side `ItemType`s no catalog row claims, sorted.
    pub only_in_catalog_index: Vec<String>,
    /// `item_type` of the catalog rows the index never names, sorted.
    ///
    /// `"*"` / `"$document"` / `"$derived"` are dispatch sentinels, not
    /// item types, so they always land here against a real index — that
    /// is expected, not a defect.
    pub only_in_catalog: Vec<String>,
    /// Members of `in_both` the catalog knows only through an `aliases`
    /// entry, sorted (index spelling, e.g. `ItemNote`).
    pub matched_through_an_alias: Vec<String>,
    /// Every catalog row's `item_type`, sorted — the comparison base the
    /// index names were checked against.
    pub catalog_item_types: Vec<String>,
    /// Every alias declared by the catalog, sorted.
    pub catalog_aliases: Vec<String>,
    /// Names of the difference sets that came back empty —
    /// `"only_in_catalog_index"` and/or `"only_in_catalog"`. Registering
    /// an empty set explicitly keeps "empty because the two sides agree"
    /// from looking like "never computed". Empty when both differ.
    pub empty_difference_sets: Vec<&'static str>,
}

/// Reconcile `document`'s item-bearing `ItemType`s against
/// `PUBLISH_ITEM_CATALOG`: which index names the catalog can dispatch
/// ([`CatalogIndexCrossCheck::in_both`]), which it cannot
/// (`only_in_catalog_index`), and which catalog rows the index never
/// exercises (`only_in_catalog`).
///
/// A row claims an index name when the name equals its `item_type` or one
/// of its `aliases`; a row enters `only_in_catalog` only when **no**
/// index name reaches it through either route.
#[must_use]
pub fn cross_check_catalog_index(
    document: &catalog_index::CatalogIndexDocument,
) -> CatalogIndexCrossCheck {
    let index_names = document.item_bearing_item_types();
    let index_set: std::collections::BTreeSet<&str> = index_names.iter().copied().collect();
    let mut in_both = Vec::new();
    let mut only_in_catalog_index = Vec::new();
    let mut matched_through_an_alias = Vec::new();
    for &name in &index_names {
        match PUBLISH_ITEM_CATALOG
            .iter()
            .find(|row| row.item_type == name || row.aliases.contains(&name))
        {
            Some(row) => {
                in_both.push(name.to_owned());
                if row.item_type != name {
                    matched_through_an_alias.push(name.to_owned());
                }
            }
            None => only_in_catalog_index.push(name.to_owned()),
        }
    }
    let mut only_in_catalog: Vec<String> = PUBLISH_ITEM_CATALOG
        .iter()
        .filter(|row| {
            !index_set.contains(row.item_type)
                && !row.aliases.iter().any(|alias| index_set.contains(alias))
        })
        .map(|row| row.item_type.to_owned())
        .collect();
    let mut catalog_item_types: Vec<String> = PUBLISH_ITEM_CATALOG
        .iter()
        .map(|row| row.item_type.to_owned())
        .collect();
    let mut catalog_aliases: Vec<String> = PUBLISH_ITEM_CATALOG
        .iter()
        .flat_map(|row| row.aliases.iter().copied())
        .map(str::to_owned)
        .collect();
    only_in_catalog.sort_unstable();
    catalog_item_types.sort_unstable();
    catalog_aliases.sort_unstable();
    let mut empty_difference_sets = Vec::new();
    if only_in_catalog_index.is_empty() {
        empty_difference_sets.push("only_in_catalog_index");
    }
    if only_in_catalog.is_empty() {
        empty_difference_sets.push("only_in_catalog");
    }
    CatalogIndexCrossCheck {
        in_both,
        only_in_catalog_index,
        only_in_catalog,
        matched_through_an_alias,
        catalog_item_types,
        catalog_aliases,
        empty_difference_sets,
    }
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
                    row.pid_tags
                        .iter()
                        .any(|(declared, role)| *declared == tag && *role == PidTagRole::Class),
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
    fn derived_pid_line_tags_match_the_pinned_list() {
        // Same ratchet as the DB list above: what the `.pid` line can
        // emit may only change through a deliberate catalog edit.
        let derived = supported_pid_line_tags();
        let hand_listed: Vec<&'static str> = vec![
            "PIDBranchPoint",
            "PIDControlSystemFunction",
            "PIDDrawing",
            "PIDHeatTransferEquipment",
            "PIDInlineInstrument",
            "PIDInstrument",
            "PIDMechanicalEquipment",
            "PIDNote",
            "PIDNozzle",
            "PIDPipeline",
            "PIDPipingBranchPoint",
            "PIDPipingComponent",
            "PIDPipingConnector",
            "PIDPipingPort",
            "PIDProcessEquipment",
            "PIDProcessPoint",
            "PIDProcessVessel",
            "PIDRepresentation",
            "PIDSignalBranchPoint",
            "PIDSignalConnector",
            "PIDSignalPort",
        ];
        assert_eq!(
            derived, hand_listed,
            "catalog-derived `.pid` tag set diverged from the pinned list"
        );
    }

    #[test]
    fn the_coverage_tally_is_the_five_derived_tags() {
        // `_Coverage.json` inserts one `classes` row per role below;
        // pinning the list keeps the report's derived section a catalog
        // decision rather than a writer one.
        let tally: Vec<&'static str> = pid_line_tag_roles()
            .iter()
            .filter(|(_, role)| {
                matches!(
                    role,
                    PidTagRole::ProcessPoint | PidTagRole::BranchPoint | PidTagRole::Port
                )
            })
            .map(|(tag, _)| *tag)
            .collect();
        assert_eq!(
            tally,
            vec![
                "PIDPipingBranchPoint",
                "PIDPipingPort",
                "PIDProcessPoint",
                "PIDSignalBranchPoint",
                "PIDSignalPort",
            ]
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

    #[test]
    fn catalog_index_cross_check_splits_the_three_sets() {
        // `ItemNote` reaches the `Note` row through its alias;
        // `ExtraItem` stays index-only; `Vessel` is a direct hit.
        let xml = concat!(
            "<?xml version=\"1.0\"?>",
            "<XOMContainer>",
            "<CICatalogItem ItemType=\"Vessel\" Class=\"0\" Subclass=\"0\" Type=\"1\" ",
            "CatalogItem=\"a.sym\"/>",
            "<CICatalogItem ItemType=\"ItemNote\" Class=\"0\" Subclass=\"0\" Type=\"2\" ",
            "CatalogItem=\"b.sym\"/>",
            "<CICatalogItem ItemType=\"ExtraItem\" Class=\"0\" Subclass=\"0\" Type=\"3\" ",
            "CatalogItem=\"c.sym\"/>",
            "</XOMContainer>",
        );
        let document = super::catalog_index::parse_catalog_index_document(xml.as_bytes()).unwrap();
        let check = cross_check_catalog_index(&document);
        assert_eq!(check.in_both, ["ItemNote", "Vessel"]);
        assert_eq!(check.matched_through_an_alias, ["ItemNote"]);
        assert_eq!(check.only_in_catalog_index, ["ExtraItem"]);
        assert_eq!(check.only_in_catalog.len(), 15);
        for sentinel in ["*", "$document", "$derived"] {
            assert!(
                check.only_in_catalog.iter().any(|row| row == sentinel),
                "sentinel `{sentinel}` missing from only_in_catalog"
            );
        }
        for claimed in ["Note", "Vessel"] {
            assert!(
                !check.only_in_catalog.iter().any(|row| row == claimed),
                "`{claimed}` reached the index and must not be a difference"
            );
        }
        assert!(check.empty_difference_sets.is_empty());
    }

    #[test]
    fn catalog_index_cross_check_registers_empty_difference_sets() {
        // An index naming every catalog `item_type` leaves both
        // difference sets empty — and must say so explicitly.
        let names = [
            "*",
            "$document",
            "$derived",
            "BranchPoint",
            "EquipmentOther",
            "Exchanger",
            "Instrument",
            "Mechanical",
            "Note",
            "Nozzle",
            "OPC",
            "PipeRun",
            "PipingBranchPoint",
            "PipingComp",
            "PipingPoint",
            "SignalRun",
            "Vessel",
        ];
        let mut xml = String::from("<?xml version=\"1.0\"?><XOMContainer>");
        for (index, name) in names.iter().enumerate() {
            xml.push_str(&format!(
                "<CICatalogItem ItemType=\"{name}\" Class=\"0\" Subclass=\"0\" ",
            ));
            xml.push_str(&format!("Type=\"{index}\" CatalogItem=\"x{index}.sym\"/>",));
        }
        xml.push_str("</XOMContainer>");
        let document = super::catalog_index::parse_catalog_index_document(xml.as_bytes()).unwrap();
        let check = cross_check_catalog_index(&document);
        assert!(check.only_in_catalog_index.is_empty());
        assert!(check.only_in_catalog.is_empty());
        assert_eq!(
            check.empty_difference_sets,
            ["only_in_catalog_index", "only_in_catalog"]
        );
    }
}
