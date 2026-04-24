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
//!   writer.
//! * [`xml_writer`] — DTO → Publish Data XML byte stream
//!   (lands in a later commit).
//!
//! The normal path no longer depends on the C# OrcaMDF probe. The
//! legacy SQLite loader remains for fixture compatibility and as a
//! simple relational adapter behind the MDF reader.

pub mod diff;
pub mod mdf_load;
pub mod model;
pub mod sqlite_load;
pub mod xml_writer;

pub use diff::{
    coverage_against_reference, diff_publish_xml, parse_attrs_per_interface_per_tag,
    parse_interfaces_per_tag, parse_pid_tag_counts, parse_rel_defuid_counts, parse_rel_details,
    supported_pid_tags, CoverageRow, RelDetail, SemanticDiffReport, TagCountDiff, TagDiffStatus,
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
