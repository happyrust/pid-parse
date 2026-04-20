use crate::error::PidError;
use crate::model::PidDocument;
use crate::package::PidPackage;
use std::path::{Path, PathBuf};

pub struct PidParser {
    options: ParseOptions,
}

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub scan_strings: bool,
    pub parse_xml: bool,
    pub parse_jsite_properties: bool,
    pub keep_unknown_streams: bool,
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
    pub fn new() -> Self {
        Self {
            options: ParseOptions::default(),
        }
    }

    pub fn with_options(options: ParseOptions) -> Self {
        Self { options }
    }

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
    /// Implementation note: v0.5.3 writes the bytes to a unique temp
    /// file, parses it, and removes the temp file. A zero-disk pure
    /// in-memory path is on the Phase 10a roadmap — it requires making
    /// `cfb::reader::parse_pid_package` generic over `Read + Seek`,
    /// which is a bigger refactor than fits in this patch. Consumers can
    /// treat this method as the authoritative "bytes in → package out"
    /// entry-point regardless; the internal scratch file is invisible
    /// on success.
    ///
    /// Errors: any `PidError::Io` from the temp file write, or any
    /// parse error surfaced by [`PidParser::parse_package`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PidError> {
        let scratch = unique_temp_path();
        std::fs::write(&scratch, bytes)?;
        let result = PidParser::new().parse_package(&scratch);
        // Best-effort cleanup; ignore failure (e.g. antivirus lock) — the
        // package was already parsed.
        let _ = std::fs::remove_file(&scratch);
        result
    }
}

/// Private helper: produce a unique temp-file path using PID + nanos.
/// Mirrors the convention used by `tests/writer_validate_cli.rs` so
/// scratch files land in the same place / naming scheme.
fn unique_temp_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("pid-parse-from-bytes-{pid}-{nanos}.pid"))
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
