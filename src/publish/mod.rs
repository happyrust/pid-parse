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
//!
//! The normal path no longer depends on the C# `OrcaMDF` probe. The
//! legacy `SQLite` loader remains for fixture compatibility and as a
//! simple relational adapter behind the MDF reader.
//!
//! ## Outstanding
//!
//! * The DWG-flavor plant MDF (`test-file/backup-test/<plant>_p/
//!   extracted/Export.mdf` for the `DWG-0202GP06-01` fixture)
//!   is the hard prerequisite for: loader canonical-field
//!   enrichment (DWG-only `EqType*` / `ProcessEqCompType*` /
//!   `ConnectionFlowDirection` / insulation+slope fields) and
//!   closing the A24 / A27b tolerated-divergence whitelists.
//!   Until the MDF lands, the DWG-side integration tests
//!   soft-skip through `common::DWG_MDF_MISSING_HINT`.
//!
//! * Stage-4 writer arms for `PIDBranchPoint` (8 interfaces)
//!   and `PIDPipingBranchPoint` (6 interfaces) are implemented
//!   and unit-tested, but the loader-side `item_type_name`
//!   mapping (`"BranchPoint"` / `"PipingBranchPoint"`) and
//!   subtable chain are provisional — they will be confirmed
//!   once the DWG MDF lands and the end-to-end count gates
//!   in `tests/publish_dwg_mirror.rs` fire.

pub mod backup_package;
// wayfinder M4: A1 (`60f1b25`) seeded the table, A2–A4 moved the database
// line onto it — `sqlite_load` resolves subtable chains through
// `catalog_row`, `xml_writer` dispatches through `db_writer` and reads
// `a01_rank`. Both lines read the tag inventory: `diff::supported_pid_tags`
// unions the DB set with `supported_pid_line_tags`, and `pid_load`
// classes an item through the row's `pid_class`.
mod catalog;
pub mod diff;
#[cfg(feature = "backup-mdf")]
pub mod mdf_load;
pub mod model;
pub mod oracle_exp;
pub mod pid_load;
pub mod pid_writer;
pub mod pipeline;
#[cfg(feature = "backup-mdf")]
pub mod sqlite_load;
pub mod xml_writer;

pub use diff::{
    coverage_against_reference, diff_pid_export, diff_publish_xml, diff_rel_defuids,
    parse_attrs_per_interface_per_tag, parse_interfaces_per_tag, parse_pid_tag_counts,
    parse_rel_defuid_counts, parse_rel_details, pid_publish_counts, supported_pid_tags,
    CoverageRow, PidExportDiff, RelDefUidDiff, RelDefUidDiffReport, RelDetail, SemanticDiffReport,
    TagCountDiff, TagDiffStatus, WriterCoverage,
};
#[cfg(feature = "backup-mdf")]
pub use mdf_load::{
    load_drawing_graph_from_mdf, mdf_bytes_from_mtf_dump, open_mdf_as_sqlite,
    open_mtf_dump_as_sqlite,
};
pub use model::{
    CodelistIndex, PublishDrawing, PublishError, PublishObject, PublishRelationship,
    PublishRepresentation, PublishStyle,
};
pub use pid_load::{
    load_publish_from_pid, load_publish_from_reader, PidLoadError, PidPublishAttribute,
    PidPublishDrawing, PidPublishItem, PidPublishRepresentation, PidPublishStats, PidValueSource,
};
pub use pid_writer::{write_pid_coverage_json, write_pid_data_xml, write_pid_meta_xml};
#[cfg(feature = "backup-mdf")]
pub use sqlite_load::{
    attach_pipe_endpoint_connections, load_codelist_index, load_drawing, load_drawing_graph,
    load_objects_by_uids, load_piping_points_for_objects, load_relationships, load_representations,
};
pub use xml_writer::{write_data_xml, write_meta_xml};
