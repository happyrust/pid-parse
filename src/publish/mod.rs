//! Publish Data XML generation — offline SmartPlant pipeline terminal stage.
//!
//! Stage-1 pipeline reads the MDF extracted from a SmartPlant backup
//! via:
//!
//! 1. Rust: [`crate::backup::mtf`] + [`crate::bin::pid_backup_extract`]
//!    — strips the SQL Server backup stream header and writes a
//!    reconstructable `.mdf` file.
//! 2. Rust: [`mdf_load`] + vendored `oxidized-mdf` — reads the
//!    publish-relevant SmartPlant SQL tables directly from MDF.
//! 3. Rust: *this module* — loads the relevant rows into the publish
//!    DTO and emits a SmartPlant-compatible Publish Data XML document
//!    (`<DrawingName>_Data.xml` and `<DrawingName>_Meta.xml`).
//!
//! ## Submodules
//!
//! * [`mdf_load`] — MDF → publish table adapter.
//! * [`sqlite_load`] — in-memory/legacy SQLite → object graph DTO.
//! * [`model`] — object-graph DTO shared by the loader and the
//!   writer. Includes the [`model::PublishStyle`] selector, which
//!   is an **explicit** input per drawing — the writer never
//!   auto-detects the A01 / DWG SmartPlant flavor.
//! * [`xml_writer`] — DTO → Publish Data XML byte stream. Emits
//!   both the `_Data.xml` and `_Meta.xml` documents and is guarded
//!   end-to-end by the fixture parity gates in `tests/publish_*.rs`
//!   (tag / interface / attribute / Rel-DefUID for `_Data.xml` and
//!   canonical-shape parity for `_Meta.xml`).
//!
//! The normal path no longer depends on the C# OrcaMDF probe. The
//! legacy SQLite loader remains for fixture compatibility and as a
//! simple relational adapter behind the MDF reader.
//!
//! ## Stage-1 outstanding
//!
//! * The DWG-flavor plant mirror (`test-file/backup-test/<plant>_p/
//!   extracted/Export_v2.sqlite` for the `DWG-0202GP06-01` fixture)
//!   is the hard prerequisite for: loader canonical-field
//!   enrichment (DWG-only `EqType*` / `ProcessEqCompType*` /
//!   `ConnectionFlowDirection` / insulation+slope fields) and
//!   closing the A24 / A27b tolerated-divergence whitelists.
//!   Until the mirror lands, the DWG-side integration tests
//!   soft-skip through `common::DWG_SQLITE_MISSING_HINT`.
//!
//! * Stage-4 writer arms for `PIDBranchPoint` (8 interfaces)
//!   and `PIDPipingBranchPoint` (6 interfaces) are implemented
//!   and unit-tested, but the loader-side `item_type_name`
//!   mapping (`"BranchPoint"` / `"PipingBranchPoint"`) and
//!   subtable chain are provisional — they will be confirmed
//!   once the DWG mirror lands and the end-to-end count gates
//!   in `tests/publish_dwg_mirror.rs` fire.

pub mod diff;
pub mod mdf_load;
pub mod model;
pub mod sqlite_load;
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
pub use mdf_load::{load_drawing_graph_from_mdf, open_mdf_as_sqlite};
pub use sqlite_load::{
    attach_pipe_endpoint_connections, load_codelist_index, load_drawing, load_drawing_graph,
    load_objects_by_uids, load_piping_points_for_objects, load_relationships,
    load_representations,
};
pub use xml_writer::{write_data_xml, write_meta_xml};
