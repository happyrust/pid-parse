//! Publish Data XML generation — offline `SmartPlant` pipeline terminal stage.
//!
//! The pipeline reads a `SmartPlant` Plant Backup through a Backup
//! Store ([`crate::backup::store`], plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`):
//!
//! 1. Rust: [`crate::backup::store`] — the `<Plant>_p.zip` (or its
//!    directory, or an `Export.mdf` on its own) becomes a Backup
//!    Store: the Database Dump's tables typed, row by row, each row
//!    with the page and slot it came from. `pid_backup_store` writes
//!    one to a file; publish builds one in memory when it is handed
//!    the backup itself.
//! 2. Rust: [`store_load`] — copies the publish-relevant tables out
//!    of the store into an in-memory `SQLite` connection of TEXT
//!    tables, printed as the MDF adapter printed them (P7).
//! 3. Rust: *this module* — loads the relevant rows into the publish
//!    DTO and emits a SmartPlant-compatible Publish Data XML document
//!    (`<DrawingName>_Data.xml` and `<DrawingName>_Meta.xml`).
//!
//! ## Submodules
//!
//! * [`store_load`] — Backup Store → publish table adapter, and the
//!   classification of what publish is handed ([`PublishInput`]).
//! * [`sqlite_load`] — in-memory/legacy `SQLite` → object graph DTO.
//! * [`model`] — object-graph DTO shared by the loader and the
//!   writer. Includes the [`model::PublishStyle`] selector, which
//!   is an **explicit** input per drawing — the writer never
//!   auto-detects the A01 / DWG `SmartPlant` flavor.
//! * [`xml_writer`] — DTO → Publish Data XML byte stream. Emits
//!   both the `_Data.xml` and `_Meta.xml` documents and is guarded
//!   end-to-end by the fixture parity gates in `tests/publish_*.rs`
//!   (tag / interface / attribute / Rel-DefUID for `_Data.xml` and
//!   canonical-shape parity for `_Meta.xml`).
//!
//! The normal path no longer depends on the C# `OrcaMDF` probe. The
//! legacy `SQLite` loader remains for fixture compatibility and as a
//! simple relational adapter behind the store.
//!
//! ## Stage-1 outstanding
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

pub mod diff;
pub mod model;
pub mod sqlite_load;
pub mod store_load;
pub mod xml_writer;

pub use diff::{
    coverage_against_reference, diff_publish_xml, diff_rel_defuids,
    parse_attrs_per_interface_per_tag, parse_interfaces_per_tag, parse_pid_tag_counts,
    parse_rel_defuid_counts, parse_rel_details, supported_pid_tags, CoverageRow, RelDefUidDiff,
    RelDefUidDiffReport, RelDetail, SemanticDiffReport, TagCountDiff, TagDiffStatus,
    WriterCoverage,
};
pub use model::{
    CodelistIndex, PublishDrawing, PublishError, PublishObject, PublishRelationship,
    PublishRepresentation, PublishStyle,
};
pub use sqlite_load::{
    attach_pipe_endpoint_connections, load_codelist_index, load_drawing, load_drawing_graph,
    load_objects_by_uids, load_piping_points_for_objects, load_relationships, load_representations,
};
pub use store_load::{
    classify_publish_input, copy_publish_tables, load_drawing_graph_from_mdf, open_mdf_as_sqlite,
    open_publish_input, PublishInput, PUBLISH_TABLES,
};
pub use xml_writer::{write_data_xml, write_meta_xml};
