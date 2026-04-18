pub mod api;
pub mod cfb;
pub mod crossref;
pub mod error;
pub mod inspect;
pub mod model;
pub mod package;
pub mod parsers;
pub mod schema;
pub mod streams;
pub mod writer;

pub use api::{ParseOptions, PidParser};
pub use error::PidError;
pub use model::*;
pub use package::{
    diff_packages, PackageDiff, PidPackage, RawStream, StorageClsidDiff, StreamDiff,
};
pub use writer::{MetadataUpdates, PidWriter, SheetChunkPatch, SheetPatch, StreamReplacement, WritePlan};

/// Re-export of [`uuid::Uuid`] for ergonomic access to the root CLSID
/// carried by [`PidPackage::root_clsid`] without forcing consumers to
/// pin their own `uuid` crate version.
pub use uuid::Uuid;
