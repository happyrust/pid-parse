//! Publish Data XML generation — offline `SmartPlant` pipeline terminal stage.
//!
//! Every terminal stage here emits the same pair of documents,
//! `<DrawingName>_Data.xml` and `<DrawingName>_Meta.xml`, from one of
//! three inputs:
//!
//! * **`SQL Server` line** — [`crate::backup::mtf`] (or the
//!   `pid_backup_extract` CLI) strips the backup stream header down to a
//!   reconstructable `.mdf`; [`mdf_load`] + vendored `oxidized-mdf` read
//!   the publish-relevant `SmartPlant` SQL tables; [`sqlite_load`] builds
//!   the DTO and [`xml_writer`] writes the XML. Entry:
//!   `pid_publish_xml <mdf> --drawing UID`.
//! * **`.pid` line** — [`pid_load`] reads the parsed drawing,
//!   [`oracle_exp`] attaches the rows only a database knows (ports,
//!   connectivity, nozzle compositions, item notes), [`pipeline`]
//!   synthesizes the `PIDPipeline` rows a dump does not carry, and
//!   [`pid_writer`] writes the XML beside a `_Coverage.json`. Entry:
//!   `pid_publish_xml <file.pid> --out-dir DIR [--oracle-exp dump]
//!   [--diff-against ref]`.
//! * **Backup directory** — [`backup_package`] walks a whole backup
//!   (`PlantData` zip + `Export.dmp`, either an Oracle `exp` dump or a
//!   `SQL Server` MTF dump), reads the tables once and publishes every
//!   drawing into one directory with an `_Index.json`. Entry:
//!   `pid_publish_xml <backup dir> --out-dir DIR`.
//!
//! ## Submodules
//!
//! * `catalog` — `PublishItemCatalog`, one row per publish item type for
//!   both lines; the database line reads it (A2–A4).
//! * [`catalog_index`] — the plant's own `CatalogIndex.xml` from
//!   `RefData~*.zip`: `(ItemType, Class, Subclass, Type) → .sym`, and the
//!   three-set cross-check against `catalog`'s rows.
//! * [`model`] — object-graph DTO shared by the loader and the
//!   writer. Includes the [`model::PublishStyle`] selector, which
//!   is an **explicit** input per drawing — the writer never
//!   auto-detects the A01 / DWG `SmartPlant` flavor.
//! * [`mdf_load`] — MDF → publish table adapter.
//! * [`sqlite_load`] — in-memory/legacy `SQLite` → object graph DTO.
//! * [`oracle_exp`] — Oracle `exp` dump → port rows, connectivity, nozzle
//!   compositions and item notes.
//! * [`pipeline`] — synthesized `PIDPipeline` rows and their
//!   `PipingConnectors` from `T_PIPERUN` + `SPPIDDataMap.xml`.
//! * [`codelist_values`] — enumerated values from the dump's codelist
//!   cells, spelled by `SPPIDDataMap.xml`.
//! * [`unit_values`] — values with a unit (`50 mm`, `3 m`) from the dump's
//!   SI columns, spelled as `SmartPlant` spells a double.
//! * [`placement`] — where a published property is stored in an Oracle
//!   `exp` dump: the data map's attribute placed in a table and a column
//!   by the dump's own data dictionary.
//! * [`stored_values`] — the published properties the dump stores as they
//!   are, read through [`placement`] onto the items a `.pid` publishes.
//! * [`pid_load`] — `.pid` → `PidPublishDrawing`, with a per-value
//!   provenance record (`PidValueSource`).
//! * [`pid_writer`] — that DTO → `_Data.xml` / `_Meta.xml` / `_Coverage.json`.
//! * [`backup_package`] — one backup directory → every drawing, dump read once.
//! * [`xml_writer`] — DTO → Publish Data XML byte stream. Emits
//!   both the `_Data.xml` and `_Meta.xml` documents and is guarded
//!   end-to-end by the fixture parity gates in `tests/publish_*.rs`
//!   (tag / interface / attribute / Rel-DefUID for `_Data.xml` and
//!   canonical-shape parity for `_Meta.xml`).
//! * [`diff`] — emitted-vs-reference scoring (`--diff-against`) and the
//!   coverage report behind `_Coverage.json`.
//! * [`completeness`] — how much of a reference `_Data.xml` a generated one
//!   states, counted against the whole reference with nothing excused.
//!
//! The normal path no longer depends on the C# `OrcaMDF` probe. The
//! legacy `SQLite` loader remains for fixture compatibility and as a
//! simple relational adapter behind the MDF reader.
//!
//! ## Outstanding
//!
//! The `DWG-0202GP06-01` fixture publishes through its Oracle `exp` dump
//! (`--oracle-exp`, and its whole package); what the `.pid` + exp line
//! still leaves out is in `docs/plans/2026-11-10-publish-export-completion.md`:
//!
//! * what [`stored_values`] holds back of an item's identity
//!   (`IObject.Name` and `IObject.Description`, S7). A drawing takes it
//!   from the dump only where no label of the sheet disagrees with the
//!   dump: this fixture's package dump is a later revision, four labels
//!   disagree with it, and its own run leaves out the three such values
//!   the dump would have given. A pipe run's name is composed by the
//!   map's template, and one the template cannot fill as the references
//!   show it is left out and counted: 249 of the 631 run names the seven
//!   acceptance references state;
//! * the hierarchical levels of an enumeration whose leaf no reference
//!   drawing shows: the map holds no levels, and
//!   [`codelist_values::LEAF_LEVELS`] holds those the references state
//!   (S8); and the interfaces an equipment type decides;
//! * three scattered values, `LongMaterialDescription3`, `ReliefPressure`
//!   and `EquipmentTrimSpec`, and the interface stated only beside the
//!   first of them, `ISpecifiedMatlItem`;
//! * rules waiting on reference data: pipelines from `T_PIPELINE` (S10),
//!   off-page connectors in a drawing's stockpile (S12), and objects other
//!   than piping components the database draws on a sheet its `.pid` does
//!   not (S17).
//!
//! The database line (`.mdf`, `.sqlite`, a `SQL Server` backup package)
//! never refuses a drawing; where it cannot write something in full it
//! writes less:
//!
//! * an item whose type has no writer of its own (`Exchanger`,
//!   `Mechanical`, `EquipmentOther`, `OPC`, and any type `catalog` does
//!   not list) is written as a `<PIDItem>` that holds its UID and nothing
//!   else;
//! * a `T_Relationship` row with an item at either end is written as a
//!   `<Rel>` whose definition is decided from the item types of the two
//!   ends alone.
//!
//! [`database_shortfalls`] counts both. The command line prints one
//! warning for each kind and, with `--strict`, exits 1; a `SQL Server`
//! package states them under `shortfalls` in the drawing's
//! `_Coverage.json` and among the `problems` of `_Index.json`. A table
//! the database does not hold and the loader can do without is skipped:
//! `-v` names it, and neither that account nor `--strict` counts it.
//!
//! That line has been checked on A01 alone. No DWG `Export.mdf` is in the
//! repository, and no test runs that line on a DWG drawing: the thirteen
//! DWG gates publish the committed `DWG-0202GP06-01` package through the
//! `.pid` + exp line (`common::generate_dwg_data_xml`). Its
//! `item_type_name` mapping for `PIDBranchPoint` / `PIDPipingBranchPoint`
//! stays provisional.
//!
//! What the review of 2026-11-16 found and left as it was (the limits of
//! the vendored `.mdf` reader, what an interrupted run leaves on disk, the
//! decisions still open) is in
//! `docs/plans/2026-11-16-publish-review-followup.md`, section 9.

pub mod backup_package;
// wayfinder M4: A1 (`60f1b25`) seeded the table, A2–A4 moved the database
// line onto it — `sqlite_load` resolves subtable chains through
// `catalog_row`, `xml_writer` dispatches through `db_writer` and reads
// `a01_rank`. Both lines read the tag inventory: `diff::supported_pid_tags`
// unions the DB set with `supported_pid_line_tags`, and `pid_load`
// classes an item through the row's `pid_class`.
mod catalog;
pub mod catalog_index;
pub mod codelist_values;
pub mod completeness;
pub mod diff;
#[cfg(feature = "backup-mdf")]
pub mod mdf_load;
pub mod model;
pub mod oracle_exp;
pub mod pid_load;
pub mod pid_writer;
pub mod pipeline;
pub mod placement;
#[cfg(feature = "backup-mdf")]
pub mod sqlite_load;
pub mod stored_values;
pub mod unit_values;
pub mod xml_writer;

pub use catalog::{cross_check_catalog_index, CatalogIndexCrossCheck};
pub use diff::{
    check_publish_document, coverage_against_reference, diff_pid_export, diff_publish_xml,
    diff_rel_defuids, parse_attrs_per_interface_per_tag, parse_interfaces_per_tag,
    parse_pid_tag_counts, parse_rel_defuid_counts, parse_rel_details, pid_publish_census,
    pid_publish_counts, supported_pid_tags, CoverageRow, MalformedPublishDocument, PidExportDiff,
    PidPublishCensus, PublishDocumentSide, RelDefUidDiff, RelDefUidDiffReport, RelDetail,
    SemanticDiffReport, TagCountDiff, TagDiffStatus, WriterCoverage,
};
#[cfg(feature = "backup-mdf")]
pub use mdf_load::{
    load_drawing_graph_from_mdf, mdf_bytes_from_mtf_dump, open_mdf_as_sqlite,
    open_mtf_dump_as_sqlite,
};
pub use model::{
    CodelistIndex, DatabaseShortfalls, PublishDrawing, PublishError, PublishObject,
    PublishRelationship, PublishRepresentation, PublishStyle,
};
pub use pid_load::{
    load_publish_from_pid, load_publish_from_reader, PidLoadError, PidPublishAttribute,
    PidPublishDrawing, PidPublishItem, PidPublishRepresentation, PidPublishStats, PidValueSource,
};
pub use pid_load::{
    read_publish_from_pid, read_publish_from_reader, PidPublishRead, PublishChainFault,
};
pub use pid_writer::{write_pid_coverage_json, write_pid_data_xml, write_pid_meta_xml};
#[cfg(feature = "backup-mdf")]
pub use sqlite_load::{
    attach_pipe_endpoint_connections, load_codelist_index, load_drawing, load_drawing_graph,
    load_objects_by_uids, load_piping_points_for_objects, load_relationships, load_representations,
};
pub use xml_writer::{database_shortfalls, write_data_xml, write_meta_xml};
