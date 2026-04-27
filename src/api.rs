//! Public parsing entry points.
//!
//! This is the first hop of the reader pipeline: the consumer picks
//! either [`PidParser::parse_file`] (model only) or
//! [`PidParser::parse_package`] (model + preserved raw streams). The
//! decoded output — a [`PidDocument`] or a [`PidPackage`] — is then
//! refined by follow-on passes in [`crate::crossref`],
//! [`crate::layout`], [`crate::import_view`], and written back via
//! [`crate::writer::PidWriter`] when round-tripping.
//!
//! [`ParseOptions`] controls cost and depth (string scans, XML
//! decoding, `JSite` property extraction, unknown-stream retention).
//! The defaults parse everything; shrink them for bulk scans where
//! only a subset of the model is needed.

use crate::error::PidError;
use crate::model::PidDocument;
use crate::package::PidPackage;
use std::path::Path;

/// Front-door parser for `SmartPlant` `.pid` compound files.
///
/// Build one with [`PidParser::new`] (all defaults) or
/// [`PidParser::with_options`] (custom [`ParseOptions`]), then
/// drive it via [`PidParser::parse_file`] for a read-only decode or
/// [`PidParser::parse_package`] when the caller will later round-trip
/// through [`crate::writer::PidWriter`].
pub struct PidParser {
    options: ParseOptions,
}

/// Tunables that control how aggressively `PidParser` decodes a `.pid`.
///
/// All fields default to "maximal fidelity" — full XML parse, full
/// `JSite` properties, full unknown-stream retention. Shrink them
/// when a bulk scan only needs a subset of the model:
///
/// - `scan_strings` — per-stream UTF-16 string probes.
/// - `parse_xml` — `SmartPlant`-embedded XML fragments.
/// - `parse_jsite_properties` — `JSite` dynamic property blobs.
/// - `keep_unknown_streams` — retain unrecognized streams for audit
///   / round-trip.
/// - `max_preview_strings` — cap on the per-stream string preview
///   collected during scan.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Enable per-stream UTF-16 / ASCII string probes.
    pub scan_strings: bool,
    /// Enable `SmartPlant`-embedded XML fragment decoding
    /// (`Drawing` / `General` metadata, rules, formats, …).
    pub parse_xml: bool,
    /// Enable decoding of `JSite` dynamic property blobs (can be
    /// expensive on big files with many sites).
    pub parse_jsite_properties: bool,
    /// Retain streams that don't match any registered decoder, so
    /// [`crate::writer::PidWriter`] can still round-trip them.
    pub keep_unknown_streams: bool,
    /// Upper bound on preview strings kept per stream during scans.
    pub max_preview_strings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            scan_strings: true,
            parse_xml: true,
            parse_jsite_properties: true,
            keep_unknown_streams: true,
            max_preview_strings: 64,
        }
    }
}

impl PidParser {
    /// Build a parser with [`ParseOptions::default`] (maximal fidelity).
    pub fn new() -> Self {
        Self {
            options: ParseOptions::default(),
        }
    }

    /// Build a parser with a custom [`ParseOptions`].
    pub fn with_options(options: ParseOptions) -> Self {
        Self { options }
    }

    /// Parse a `.pid` file on disk into a [`PidDocument`]. Streams are
    /// consumed on the fly; raw bytes are not retained. Use
    /// [`Self::parse_package`] when the caller plans to write the file
    /// back.
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<PidDocument, PidError> {
        crate::cfb::reader::parse_pid_file(path.as_ref(), &self.options)
    }

    /// Parse a `.pid` file into a [`PidPackage`], preserving every stream's
    /// raw bytes alongside the decoded model. Use this when you intend to
    /// modify and write the file back via [`crate::writer::PidWriter`].
    pub fn parse_package<P: AsRef<Path>>(&self, path: P) -> Result<PidPackage, PidError> {
        crate::cfb::reader::parse_pid_package(path.as_ref(), &self.options)
    }
}

impl Default for PidParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PidPackage {
    /// Phase 9o (v0.5.3+): convenience constructor that parses a `.pid`
    /// file into a `PidPackage` using a default [`PidParser`]. Short for
    /// `PidParser::new().parse_package(path)`.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, PidError> {
        PidParser::new().parse_package(path.as_ref())
    }

    /// Phase 9o (v0.5.3+): parse an in-memory byte slice as if it were a
    /// `.pid` file. Useful when the bytes come from HTTP, an archive, or
    /// an embedded resource and you would rather not touch disk.
    ///
    /// Implementation note: v0.11.6+ parses the bytes through an
    /// in-memory `Cursor<Vec<u8>>`, so no scratch file is created and the
    /// returned package has `source_path == None`.
    ///
    /// Errors: any parse error surfaced by the CFB reader.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PidError> {
        let parser = PidParser::new();
        let cursor = std::io::Cursor::new(bytes.to_vec());
        crate::cfb::reader::parse_pid_package_from_reader(cursor, &parser.options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_minimal_cfb_bytes() -> Vec<u8> {
        // Build a tiny CFB in-memory using the same mechanism we write
        // through PidWriter so the two round-trip against each other.
        use std::io::Cursor;
        let mut cfb = ::cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        cfb.create_storage("/TaggedTxtData").unwrap();
        let mut s = cfb.create_stream("/TaggedTxtData/Drawing").unwrap();
        s.write_all(b"<Drawing><DrawingNumber>API-9O</DrawingNumber></Drawing>")
            .unwrap();
        drop(s);
        cfb.flush().unwrap();
        cfb.into_inner().into_inner()
    }

    fn unique_temp_path() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let pid = std::process::id();
        std::env::temp_dir().join(format!("pid-parse-from-bytes-test-{pid}-{nanos}.pid"))
    }

    #[test]
    fn from_bytes_parses_a_minimal_synthetic_pid() {
        let bytes = build_minimal_cfb_bytes();
        let pkg = PidPackage::from_bytes(&bytes).expect("parse");
        assert!(
            pkg.get_stream("/TaggedTxtData/Drawing").is_some(),
            "Drawing stream should survive byte-level parse"
        );
    }

    #[test]
    fn from_bytes_marks_package_as_memory_sourced() {
        let bytes = build_minimal_cfb_bytes();
        let pkg = PidPackage::from_bytes(&bytes).expect("parse");
        assert!(
            pkg.source_path.is_none(),
            "from_bytes should parse directly from memory, not expose a scratch file path"
        );
    }

    #[test]
    fn from_bytes_on_invalid_data_returns_error() {
        let err = PidPackage::from_bytes(b"not a cfb file").expect_err("invalid");
        let msg = format!("{err}");
        assert!(!msg.is_empty(), "error message should be non-empty");
    }

    #[test]
    fn from_path_matches_parse_package_behavior() {
        let bytes = build_minimal_cfb_bytes();
        let path = unique_temp_path();
        std::fs::write(&path, &bytes).unwrap();
        let a = PidPackage::from_path(&path).expect("from_path");
        let b = PidParser::new().parse_package(&path).expect("parse");
        assert_eq!(
            a.streams.keys().collect::<Vec<_>>(),
            b.streams.keys().collect::<Vec<_>>(),
            "from_path and parse_package must produce equivalent stream lists"
        );
        let _ = std::fs::remove_file(&path);
    }
}
