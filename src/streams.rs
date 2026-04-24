//! Stream-family orchestrators that populate a [`crate::model::PidDocument`].
//!
//! Whereas [`crate::parsers`] contains the low-level decoders, the
//! submodules here are the "one stream family at a time" drivers
//! invoked by the CFB reader: cluster scanning, document registry,
//! dynamic attributes, `JSite` extraction, PSM root / cluster /
//! segment tables, summary / document-summary info, and tagged-text
//! storage. Each submodule owns a small, well-named `parse_*`
//! function that reads its bytes and writes the decoded structure
//! back into the document in-place.
//!
//! New stream decoders land here; add the matching low-level record
//! parser under [`crate::parsers`] and register the top-level stream
//! name in [`crate::inspect::KNOWN_TOP_LEVEL_STREAM_NAMES`].

pub mod cluster;
pub mod doc_registry;
pub mod dynamic_attrs;
pub mod jsite;
pub mod psm_tables;
pub mod summary;
pub mod tagged_text;
