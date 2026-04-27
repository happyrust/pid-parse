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

/// High-level parse profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseProfile {
    /// Full-fidelity parse. This is the default and preserves all current
    /// parser behavior.
    Full,
    /// Lightweight inventory/triage parse that skips expensive semantic and
    /// derived passes.
    Light,
}

/// Tunables that control how aggressively `PidParser` decodes a `.pid`.
///
/// All fields default to "maximal fidelity" — full XML parse, full
/// `JSite` properties, full unknown-stream retention. Shrink them
/// when a bulk scan only needs a subset of the model:
///
/// - `profile` — high-level full vs light parse profile.
/// - `scan_strings` — per-stream UTF-16 string probes.
/// - `parse_xml` — `SmartPlant`-embedded XML fragments.
/// - `parse_jsite_properties` — `JSite` dynamic property blobs.
/// - `keep_unknown_streams` — retain decoded diagnostics for unknown
///   streams (`PidDocument::unknown_streams` and embedded `JSite` raw-stream
///   summaries). Package-side raw bytes are always retained for writer
///   passthrough.
/// - `max_preview_strings` — cap on the per-stream string preview
///   collected during scan.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// High-level parse profile. [`ParseProfile::Full`] preserves existing
    /// behavior; [`ParseProfile::Light`] skips expensive semantic and derived
    /// passes for inventory-style callers.
    pub profile: ParseProfile,
    /// Enable per-stream UTF-16 / ASCII string probes.
    pub scan_strings: bool,
    /// Enable `SmartPlant`-embedded XML fragment decoding
    /// (`Drawing` / `General` metadata, rules, formats, …).
    pub parse_xml: bool,
    /// Enable decoding of `JSite` dynamic property blobs (can be
    /// expensive on big files with many sites).
    pub parse_jsite_properties: bool,
    /// Retain decoded diagnostics for streams that don't match any
    /// registered decoder. This does not control package-side raw byte
    /// retention; [`crate::writer::PidWriter`] passthrough remains
    /// byte-preserving even when this is `false`.
    pub keep_unknown_streams: bool,
    /// Upper bound on preview strings kept per stream during scans.
    pub max_preview_strings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            profile: ParseProfile::Full,
            scan_strings: true,
            parse_xml: true,
            parse_jsite_properties: true,
            keep_unknown_streams: true,
            max_preview_strings: 64,
        }
    }
}

impl ParseOptions {
    /// Build an explicit light parse profile for bulk inventory / triage.
    ///
    /// This keeps stream inventory and package raw bytes, but disables XML
    /// body parsing and `JSite` property decoding by default. The reader also
    /// skips heavier semantic and derived passes while this profile is active.
    pub fn light() -> Self {
        Self {
            profile: ParseProfile::Light,
            parse_xml: false,
            parse_jsite_properties: false,
            max_preview_strings: 16,
            ..Self::default()
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

    fn build_cfb_bytes_with_unknown_top_level_stream() -> Vec<u8> {
        use std::io::Cursor;
        let mut cfb = ::cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        cfb.create_storage("/TaggedTxtData").unwrap();
        let mut drawing = cfb.create_stream("/TaggedTxtData/Drawing").unwrap();
        drawing.write_all(b"<Drawing />").unwrap();
        drop(drawing);
        let mut mystery = cfb.create_stream("/MysteryTopLevel").unwrap();
        mystery.write_all(b"root-unknown-payload").unwrap();
        drop(mystery);
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

    fn write_temp_pid(bytes: &[u8]) -> std::path::PathBuf {
        let path = unique_temp_path();
        std::fs::write(&path, bytes).unwrap();
        path
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

    #[test]
    fn keep_unknown_streams_false_keeps_package_raw_streams() {
        let bytes = build_cfb_bytes_with_unknown_top_level_stream();
        let path = write_temp_pid(&bytes);
        let parser = PidParser::with_options(ParseOptions {
            keep_unknown_streams: false,
            ..ParseOptions::default()
        });

        let pkg = parser.parse_package(&path).expect("parse");

        assert!(
            pkg.get_stream("/MysteryTopLevel").is_some(),
            "package raw streams must remain available for writer passthrough"
        );
        assert!(
            pkg.parsed.unknown_streams.is_empty(),
            "keep_unknown_streams=false should suppress decoded unknown diagnostics"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn keep_unknown_streams_true_populates_unknown_stream_inventory() {
        let bytes = build_cfb_bytes_with_unknown_top_level_stream();
        let path = write_temp_pid(&bytes);

        let pkg = PidParser::new().parse_package(&path).expect("parse");

        assert!(
            pkg.parsed
                .unknown_streams
                .iter()
                .any(|s| s.path == "/MysteryTopLevel"),
            "default parser should retain top-level unknown stream diagnostics"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn light_profile_keeps_inventory_and_raw_streams_but_skips_derived_passes() {
        let bytes = build_minimal_cfb_bytes();
        let path = write_temp_pid(&bytes);
        let parser = PidParser::with_options(ParseOptions::light());

        let pkg = parser.parse_package(&path).expect("parse");

        assert_eq!(parser.options.profile, ParseProfile::Light);
        assert!(
            pkg.get_stream("/TaggedTxtData/Drawing").is_some(),
            "light package parsing must retain raw streams for callers"
        );
        assert!(
            pkg.parsed
                .streams
                .iter()
                .any(|stream| stream.path == "/TaggedTxtData/Drawing"),
            "light parsing should keep the stream inventory"
        );
        assert!(
            pkg.parsed.drawing_meta.is_none(),
            "light parsing should skip tagged-text XML bodies"
        );
        assert!(
            pkg.parsed.cross_reference.is_none(),
            "light parsing should skip derived cross-reference graph"
        );
        assert!(
            pkg.parsed.layout.is_none(),
            "light parsing should skip derived layout"
        );
        let _ = std::fs::remove_file(&path);
    }
}
