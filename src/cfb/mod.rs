//! Compound File Binary (CFB / OLE2) layer.
//!
//! This module owns the raw container concerns: opening a `.pid`
//! file, walking its storage/stream tree, and collecting each
//! stream's bytes into the in-memory structures consumed by higher
//! layers. It does **not** interpret business semantics — decoding a
//! stream's content is the job of [`crate::parsers`] and
//! [`crate::streams`].
//!
//! - [`reader::parse_pid_file`] — CFB → [`crate::model::PidDocument`]
//!   (decoded model only).
//! - [`reader::parse_pid_package`] — CFB →
//!   [`crate::package::PidPackage`] (model + preserved raw streams,
//!   required for writer round-trips).
//! - [`tree::build_tree`] — reusable helper that materialises the
//!   CFB storage/stream hierarchy as a [`crate::model::StorageNode`].

pub mod reader;
pub mod tree;
