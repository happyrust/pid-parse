//! Low-level decoders for individual binary / XML fragments.
//!
//! Each submodule is a focused parser for one stream layout or
//! record family (cluster headers, `DocVersion` records, dynamic
//! attribute records, drawing XML, `JProperties`, magic sniffing,
//! PSM table rows, relationship / sheet probes, string scans,
//! tagged text lists, XML helpers). They are deliberately
//! stateless and side-effect-free so they are easy to unit-test.
//!
//! `parsers` is **not** intended as a public API surface for
//! end-users; consumers should go through the orchestrating layer
//! in [`crate::streams`] (or the top-level [`crate::api`] / CFB
//! [`crate::cfb`] entry points) instead. This module is `pub` only
//! so internal crates and tests can reach the primitives directly.

pub mod app_object;
pub mod cluster_header;
pub mod doc_version;
pub mod doc_version2;
pub mod drawing_xml;
pub mod dynamic_attr_records;
pub mod general_xml;
pub mod jproperties;
pub mod magic;
pub mod psm_tables;
pub mod relationship_probe;
pub mod sheet_endpoint_records;
pub mod sheet_probe;
pub mod sheet_records;
pub mod string_scan;
pub mod summary;
pub mod tagged_stg_list;
pub mod xml_util;
