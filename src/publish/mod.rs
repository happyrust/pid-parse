//! Publish Data XML generation — offline SmartPlant pipeline terminal stage.
//!
//! Stage-1 pipeline produces a SQLite mirror of a SmartPlant backup
//! database via:
//!
//! 1. Rust: [`crate::backup::mtf`] + [`crate::bin::pid_backup_extract`]
//!    — strips the SQL Server backup stream header and writes a
//!    reconstructable `.mdf` file.
//! 2. C#:  `tools/orca-mdf-probe` (forked OrcaMDF + `SqlFloat` /
//!    `SqlReal` additions) — mirrors every user table into a SQLite
//!    database.
//! 3. Rust: *this module* — loads the relevant rows out of SQLite
//!    and emits a SmartPlant-compatible Publish Data XML document
//!    (`<DrawingName>_Data.xml` and `<DrawingName>_Meta.xml`).
//!
//! ## Submodules
//!
//! * [`sqlite_load`] — SQLite → in-memory object graph DTO.
//! * [`model`] — object-graph DTO shared by the loader and the
//!   writer.
//! * [`xml_writer`] — DTO → Publish Data XML byte stream
//!   (lands in a later commit).
//!
//! Everything here depends on the SQLite file being produced by the
//! companion C# probe; the module is pure reader / writer glue and
//! does not touch MDF bytes directly.

pub mod diff;
pub mod model;
pub mod sqlite_load;
pub mod xml_writer;

pub use diff::{
    coverage_against_reference, diff_publish_xml, parse_attrs_per_interface_per_tag,
    parse_interfaces_per_tag, parse_pid_tag_counts, parse_rel_defuid_counts,
    supported_pid_tags, CoverageRow, SemanticDiffReport, TagCountDiff, TagDiffStatus,
    WriterCoverage,
};
pub use model::{
    CodelistIndex, PublishDrawing, PublishError, PublishObject, PublishRelationship,
    PublishRepresentation, PublishStyle,
};
pub use sqlite_load::{
    load_codelist_index, load_drawing, load_drawing_graph, load_objects_by_uids,
    load_piping_points_for_objects, load_relationships, load_representations,
};
pub use xml_writer::{write_data_xml, write_meta_xml};
