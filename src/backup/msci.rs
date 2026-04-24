//! SQL Server **Microsoft SQL Configuration Information** stream
//! parser.
//!
//! The `MSCI` stream inside the MTF `VOLB` DBLK is a thin wrapper
//! around a SQL-Server-specific record list describing the backup
//! set's database files. SQL Server does not publish the exact
//! layout; this module takes a best-effort, signature-scanning
//! approach that reliably recovers the information we actually need
//! for the offline backup pipeline:
//!
//! * `filegroup_name` — the logical filegroup (e.g. `"PRIMARY"`).
//! * per-file records of [`MsciFile`] with:
//!   * `logical_name` — e.g. `"SP3DTrain_RDB_SCHEMA_dat"`.
//!   * `physical_path` — e.g. `"C:\\Program Files\\Microsoft SQL
//!     Server\\...SP3DTrain_RDB_SCHEMAdat.mdf"`.
//!   * `record_offset` — byte offset of the `SFIN` magic inside the
//!     input buffer, useful for correlating with a parent MSDA.
//!
//! Stage-0 does not attempt to decode the per-file numeric metadata
//! (page size, file size, backup extent counts) because those fields
//! have not yet been cross-validated against multiple fixtures. The
//! raw bytes between `SFIN` magics remain accessible via
//! [`MsciConfig::records`] for future stages to mine.
//!
//! # Signatures observed
//!
//! The MSCI body we have is 3304 bytes long and contains, in order:
//!
//! | Offset (rel) | Magic | Meaning |
//! |--------------|-------|---------|
//! | `0x028`      | `MQCI` | Outer SQL Config wrapper |
//! | `0x03C`      | `SCIN` | Backup set header (date, db name, page counts) |
//! | `0x200`      | `SFGI` | SmartFile Group Info (filegroup name follows) |
//! | `0x2F0`      | `SFIN` | SmartFile Info #1 (MDF) |
//! | `0x7EC`      | `SFIN` | SmartFile Info #2 (LDF) |
//!
//! Each `SFIN` record header begins with the magic, a 32-bit record
//! length field, and a variable-size body. Following the numeric
//! body are two UTF-16LE strings: the logical file name (prefixed
//! by a single 'H' marker character) and the physical file path
//! (also 'H'-prefixed). Stage-0's scanner simply walks forward from
//! each `SFIN` magic picking up the next two UTF-16LE ASCII-printable
//! runs, which matches the observed layout across both records.

use std::fmt;

/// Magic marker introducing a SmartFile Group Info descriptor.
pub const SFGI_MAGIC: [u8; 4] = *b"SFGI";
/// Magic marker introducing a SmartFile Info (per-file) descriptor.
pub const SFIN_MAGIC: [u8; 4] = *b"SFIN";
/// Magic marker introducing the SQL Configuration Information
/// header. Stage-0 does not currently decode this section beyond
/// recording its presence.
pub const SCIN_MAGIC: [u8; 4] = *b"SCIN";

/// Minimum number of printable UTF-16LE characters for a run to be
/// treated as a "real" string rather than incidental noise in the
/// opaque per-file record bytes. Picked to skip the single-character
/// 'H' marker SQL Server places before every string field.
pub const MIN_UTF16_RUN_CHARS: usize = 4;

/// Errors returned by [`parse_msci`]. Mostly cosmetic in stage 0
/// since the parser degrades gracefully on missing markers.
#[derive(Debug)]
pub enum MsciError {
    /// The input does not contain a single `SFIN` record — either
    /// the stream is not MSCI, or the backup contains zero files
    /// (which we have never observed and consider a bug).
    NoFileRecords,
}

impl fmt::Display for MsciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFileRecords => {
                f.write_str("MSCI stream body does not contain any SFIN records")
            }
        }
    }
}

impl std::error::Error for MsciError {}

/// One physical file inside the backup set. Maps 1:1 with a row in
/// SQL Server's `sys.backup_filegroup` view for the same backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsciFile {
    /// Offset of the `SFIN` magic inside the MSCI body slice.
    pub record_offset: usize,
    /// Logical database-file name (e.g. `SP3DTrain_RDB_SCHEMA_dat`).
    pub logical_name: String,
    /// Physical file system path as recorded at backup time.
    pub physical_path: String,
}

/// Parsed MSCI stream body. Currently only carries the filegroup
/// name and per-file records; richer numeric metadata can be added
/// without breaking the field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsciConfig {
    /// Filegroup name read from after the `SFGI` marker, if present.
    pub filegroup_name: Option<String>,
    /// Per-file records, in the order they appear in the stream.
    pub files: Vec<MsciFile>,
    /// Raw byte offsets of every `SFIN` magic within the input
    /// buffer. Exposed for diagnostic tooling.
    pub records: Vec<usize>,
}

/// Parse an MSCI stream body. The input is the byte slice returned
/// by `MtfStream::body_offset..body_end` for a stream of kind
/// [`crate::backup::mtf::MtfStreamKind::SqlConfig`].
pub fn parse_msci(body: &[u8]) -> Result<MsciConfig, MsciError> {
    let sfgi_offsets = find_magic_offsets(body, &SFGI_MAGIC);
    let sfin_offsets = find_magic_offsets(body, &SFIN_MAGIC);

    if sfin_offsets.is_empty() {
        return Err(MsciError::NoFileRecords);
    }

    let filegroup_name = sfgi_offsets
        .first()
        .and_then(|start| find_next_utf16_ascii_run(body, *start + 4, MIN_UTF16_RUN_CHARS))
        .map(|raw| strip_h_marker(&raw).to_string());

    let mut files = Vec::with_capacity(sfin_offsets.len());
    for (idx, sfin_start) in sfin_offsets.iter().copied().enumerate() {
        // Record runs until the next SFIN, or end-of-body for the
        // last record. This bound keeps the run scanner from
        // bleeding file #1's path into file #2.
        let record_end = sfin_offsets
            .get(idx + 1)
            .copied()
            .unwrap_or(body.len());
        let combined =
            find_next_utf16_ascii_run(&body[..record_end], sfin_start + 4, MIN_UTF16_RUN_CHARS)
                .unwrap_or_default();
        let (logical_name, physical_path) = split_name_and_path(&combined);
        files.push(MsciFile {
            record_offset: sfin_start,
            logical_name,
            physical_path,
        });
    }

    Ok(MsciConfig {
        filegroup_name,
        files,
        records: sfin_offsets,
    })
}

/// Strip the `H` marker character SQL Server prefixes in front of
/// every UTF-16LE string field inside MSCI records. Safe no-op when
/// the input does not begin with `H` (e.g. when the parser already
/// stripped it, or when a future fixture omits the marker).
fn strip_h_marker(s: &str) -> &str {
    s.strip_prefix('H').unwrap_or(s)
}

/// Split a combined "logical name + physical path" string (which MSCI
/// emits with no inter-field separator) into its two components.
/// Uses Windows absolute-path sentinels `C:\\` (or any `X:\\`) and
/// UNC prefix `\\\\` to locate the split point. If neither sentinel
/// is found the entire run is returned as the logical name with an
/// empty path.
///
/// The leading `H` marker byte SQL Server emits before each string
/// field is also stripped before returning.
fn split_name_and_path(combined: &str) -> (String, String) {
    let trimmed = strip_h_marker(combined);
    let path_anchor = find_windows_path_start(trimmed);
    match path_anchor {
        Some(idx) => {
            let (name_part, path_part) = trimmed.split_at(idx);
            // The path_part itself may start with a 'H' marker on
            // some SQL Server versions — strip it defensively.
            (name_part.to_string(), strip_h_marker(path_part).to_string())
        }
        None => (trimmed.to_string(), String::new()),
    }
}

/// Find the byte index of the first Windows-style absolute path
/// start (`X:\\` or `\\\\`) inside `s`, or `None` if no such anchor
/// exists. Used to locate the logical-name / physical-path boundary
/// in MSCI records that otherwise have no field separator.
fn find_windows_path_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    // UNC prefix `\\\\` — pick the earliest hit among supported
    // shapes.
    let unc = bytes
        .windows(2)
        .position(|w| w == b"\\\\");
    // Drive-letter prefix `X:\\` — only valid when the char before
    // the colon is an ASCII letter.
    let drive = (0..bytes.len().saturating_sub(2)).find(|&i| {
        bytes[i].is_ascii_alphabetic()
            && bytes.get(i + 1) == Some(&b':')
            && bytes.get(i + 2) == Some(&b'\\')
    });
    match (unc, drive) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Return every occurrence of a 4-byte magic in `haystack`. Output
/// is sorted ascending. O(n · 4) but good enough for a 3-4 KB
/// config stream.
fn find_magic_offsets(haystack: &[u8], magic: &[u8; 4]) -> Vec<usize> {
    if haystack.len() < 4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..haystack.len() - 3 {
        if &haystack[i..i + 4] == magic {
            out.push(i);
        }
    }
    out
}

/// Scan forward from `start` looking for a run of printable ASCII
/// UTF-16LE characters (i.e. every other byte is zero, the other
/// byte is in `0x20..=0x7E`). Returns the decoded run (without the
/// trailing terminator / padding) once the run's length in
/// characters is at least `min_chars`.
fn find_next_utf16_ascii_run(data: &[u8], start: usize, min_chars: usize) -> Option<String> {
    let mut i = start;
    while i + 1 < data.len() {
        // Align to even offsets since UTF-16LE pairs start there in
        // SQL Server's MSCI records.
        if !i.is_multiple_of(2) {
            i += 1;
            continue;
        }
        if is_printable_ascii(data[i]) && data[i + 1] == 0 {
            // Found the start of a candidate run; measure it.
            let mut end = i;
            let mut chars = 0usize;
            let mut s = String::new();
            while end + 1 < data.len()
                && is_printable_ascii(data[end])
                && data[end + 1] == 0
            {
                s.push(data[end] as char);
                end += 2;
                chars += 1;
            }
            if chars >= min_chars {
                return Some(s);
            }
            i = end + 2; // skip past the too-short run's terminator
        } else {
            i += 1;
        }
    }
    None
}

fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic fixture that mimics the real MSCI layout
    /// as best as we can: the logical name and physical path are
    /// emitted back-to-back (no separator) with just a leading 'H'
    /// marker character.
    fn synthetic_msci() -> Vec<u8> {
        let mut out = Vec::new();
        // Pre-padding.
        out.extend_from_slice(&[0u8; 16]);
        // SFGI marker + filegroup name "PRIMARY" ('H' prefixed).
        out.extend_from_slice(b"SFGI");
        out.extend_from_slice(&encode_utf16_with_h_prefix("PRIMARY"));
        // Pad to look like the real layout has filler bytes
        // between SFGI and the first SFIN.
        out.extend_from_slice(&[0u8; 8]);
        // SFIN #1: "DATA_FILE" followed immediately by "C:\\DB\\data.mdf".
        let sfin1_offset = out.len();
        out.extend_from_slice(b"SFIN");
        out.extend_from_slice(&[0xFC, 0x04, 0, 0]); // record length
        out.extend_from_slice(&[0u8; 16]); // opaque numeric fields
        out.extend_from_slice(&encode_utf16_with_h_prefix("DATA_FILE"));
        out.extend_from_slice(&encode_utf16_no_prefix("C:\\DB\\data.mdf"));
        out.extend_from_slice(&[0u8; 16]); // trailing pad
        // SFIN #2: "LOG_FILE" followed immediately by path.
        let sfin2_offset = out.len();
        out.extend_from_slice(b"SFIN");
        out.extend_from_slice(&[0xFC, 0x04, 0, 0]);
        out.extend_from_slice(&[0u8; 16]);
        out.extend_from_slice(&encode_utf16_with_h_prefix("LOG_FILE"));
        out.extend_from_slice(&encode_utf16_no_prefix("C:\\DB\\log.ldf"));
        out.extend_from_slice(&[0u8; 16]);

        // Sanity: make sure the two offsets are where we expect.
        assert!(sfin1_offset < sfin2_offset);
        out
    }

    fn encode_utf16_no_prefix(s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 * s.len());
        for c in s.chars() {
            out.extend_from_slice(&(c as u16).to_le_bytes());
        }
        out
    }

    fn encode_utf16_with_h_prefix(s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 * (s.len() + 1));
        out.push(b'H');
        out.push(0);
        for c in s.chars() {
            out.extend_from_slice(&(c as u16).to_le_bytes());
        }
        out
    }

    #[test]
    fn parse_msci_recovers_filegroup_name_and_two_files() {
        let body = synthetic_msci();
        let parsed = parse_msci(&body).expect("synthetic MSCI should parse");

        assert_eq!(
            parsed.filegroup_name.as_deref(),
            Some("PRIMARY"),
            "leading 'H' marker must be stripped from the filegroup name"
        );
        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.files[0].logical_name, "DATA_FILE");
        assert_eq!(parsed.files[0].physical_path, "C:\\DB\\data.mdf");
        assert_eq!(parsed.files[1].logical_name, "LOG_FILE");
        assert_eq!(parsed.files[1].physical_path, "C:\\DB\\log.ldf");
    }

    #[test]
    fn parse_msci_errors_when_no_sfin_records_present() {
        // Pure filler bytes with no SFIN marker — downstream stages
        // should see a clean error rather than an empty result,
        // because the backup definitely contains at least one file.
        let body = vec![0u8; 256];
        assert!(matches!(parse_msci(&body), Err(MsciError::NoFileRecords)));
    }

    #[test]
    fn parse_msci_handles_unc_path_prefix() {
        // UNC paths (`\\server\share\...`) should split the same way
        // as drive-letter paths. Stage-0 must treat both as anchors.
        let mut body = Vec::new();
        body.extend_from_slice(&[0u8; 8]);
        body.extend_from_slice(b"SFIN");
        body.extend_from_slice(&[0u8; 20]);
        body.extend_from_slice(&encode_utf16_with_h_prefix("SHARE_DATA"));
        body.extend_from_slice(&encode_utf16_no_prefix(r"\\fileserver\backups\db.mdf"));
        body.extend_from_slice(&[0u8; 16]);
        let parsed = parse_msci(&body).expect("UNC-path MSCI should parse");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].logical_name, "SHARE_DATA");
        assert_eq!(
            parsed.files[0].physical_path,
            r"\\fileserver\backups\db.mdf"
        );
    }

    #[test]
    fn find_next_utf16_ascii_run_returns_string_of_sufficient_length() {
        let mut buf = vec![0u8; 32];
        // Prefix noise, then "HELLO" in UTF-16LE starting at 8.
        for (i, c) in "HELLO".chars().enumerate() {
            buf[8 + i * 2] = c as u8;
            buf[8 + i * 2 + 1] = 0;
        }
        assert_eq!(
            find_next_utf16_ascii_run(&buf, 0, 3).as_deref(),
            Some("HELLO")
        );
    }

    #[test]
    fn find_next_utf16_ascii_run_skips_runs_shorter_than_min_chars() {
        // Short "HI" run then longer "WORLD" run further on; with
        // min_chars = 4 we must skip "HI" and return "WORLD".
        let mut buf = vec![0u8; 32];
        buf[0] = b'H';
        buf[2] = b'I';
        for (i, c) in "WORLD".chars().enumerate() {
            buf[10 + i * 2] = c as u8;
            buf[10 + i * 2 + 1] = 0;
        }
        assert_eq!(
            find_next_utf16_ascii_run(&buf, 0, 4).as_deref(),
            Some("WORLD")
        );
    }

    #[test]
    fn split_name_and_path_splits_on_drive_letter() {
        assert_eq!(
            split_name_and_path("HFOOBARC:\\x.mdf"),
            ("FOOBAR".to_string(), "C:\\x.mdf".to_string())
        );
    }

    #[test]
    fn split_name_and_path_splits_on_unc_prefix() {
        assert_eq!(
            split_name_and_path("HFOOBAR\\\\srv\\x.mdf"),
            ("FOOBAR".to_string(), "\\\\srv\\x.mdf".to_string())
        );
    }

    #[test]
    fn split_name_and_path_returns_empty_path_when_no_anchor() {
        assert_eq!(
            split_name_and_path("HFOOBAR"),
            ("FOOBAR".to_string(), String::new())
        );
    }

    #[test]
    fn strip_h_marker_removes_leading_h_only() {
        assert_eq!(strip_h_marker("Hfoo"), "foo");
        assert_eq!(strip_h_marker("Bar"), "Bar");
        assert_eq!(strip_h_marker(""), "");
    }

    #[test]
    fn is_printable_ascii_accepts_expected_range() {
        assert!(is_printable_ascii(0x20));
        assert!(is_printable_ascii(0x41));
        assert!(is_printable_ascii(0x7E));
        assert!(!is_printable_ascii(0x1F));
        assert!(!is_printable_ascii(0x7F));
    }
}
