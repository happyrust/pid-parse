//! File-level scanner for the `RefData~SCHEMA~ID(.zip)?` payloads
//! that ship inside a `SmartPlant` plant backup folder.
//!
//! A real plant backup folder (`TEST02_p/`) carries up to a dozen
//! `RefData~4~*` files alongside `Manifest.txt`, `Export.dmp`, and
//! `PlantData~2~*.zip`. Despite the uniform name they are *not*
//! all ZIP archives — empirical magic-byte sniffing of the bundled
//! fixture shows at least four different on-disk formats:
//!
//! | First 4 bytes | Format | Examples in `TEST02_p/` |
//! |---|---|---|
//! | `50 4B 03 04` | ZIP archive | `RefData~4~681.zip`, `RefData~4~703` (no extension) |
//! | `D0 CF 11 E0` | OLE / CFB compound file | `RefData~4~683` |
//! | `3C ..` (`<`) | XML | `RefData~4~709` (`<ProjectInsulationSpecifications>`) |
//! | other printable ASCII | `SmartPlant`-private text (e.g. Rules CSV) | `RefData~4~680` (`"Begin Rules",120,…`) |
//!
//! This module ships **only the scan layer**: it walks a directory,
//! filters `RefData~SCHEMA~ID(.zip)?` filenames, parses out the
//! schema and id integers, and classifies each file by its first
//! four magic bytes. Cracking the contents of each ZIP / CFB / XML
//! payload is left to follow-up modules so this PR stays focused
//! and dependency-free.
//!
//! # Tolerance
//!
//! Every public entry point is **panic-safe** by construction: I/O
//! errors propagate via [`std::io::Result`], unparseable filenames
//! and short magic prefixes are silently dropped, and
//! [`classify_format`] accepts any `&[u8]` length (including 0).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One scanned `RefData~SCHEMA~ID(.zip)?` file under a plant
/// backup folder, including everything we can learn without
/// opening the contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefDataEntry {
    /// Schema group from the second `~`-separated segment of the
    /// filename — `4` for every observed plant backup so far.
    /// Preserved verbatim so future callers can group entries by
    /// schema without re-parsing the path.
    pub schema: u32,
    /// Numeric id from the third `~`-separated segment. Unique
    /// inside one plant backup; used as the `BTreeMap` key when a
    /// caller wants stable ordering.
    pub id: u32,
    /// File name as it appears on disk, including the optional
    /// `.zip` suffix. Useful for log messages and for round-tripping
    /// to the original path inside an archive.
    pub file_name: String,
    /// Absolute or relative path to the file (whatever was passed
    /// into [`scan_refdata_dir`]). Ready to feed back to
    /// `std::fs::File::open`.
    pub full_path: PathBuf,
    /// File size in bytes from the directory entry — handy for
    /// rough heuristics ("RefData~4~685.zip is 316 B and probably
    /// empty") without re-reading the file.
    pub size: u64,
    /// Format derived from the leading 4 magic bytes. See
    /// [`classify_format`] for the exact mapping.
    pub format: RefDataFormat,
}

/// Coarse format classification of one `RefData~*` file, derived
/// from its first four magic bytes.
///
/// The variants intentionally bundle "I know the container" cases
/// (`Zip`, `Cfb`, `Xml`) and a generic "looks textual" bucket
/// (`AsciiText`) so the scan layer can stay free of payload-format
/// dependencies. Truly opaque content keeps its raw 4-byte magic
/// in [`RefDataFormat::Unknown`] for downstream debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefDataFormat {
    /// `50 4B 03 04` — local file header of a ZIP archive. Holds
    /// regardless of the `.zip` filename suffix; some `RefData~4~*`
    /// files are zip archives without the extension.
    Zip,
    /// `D0 CF 11 E0` — OLE / Compound File Binary header. Used by
    /// `SmartPlant` to ship templates / report files in the same
    /// container family as `.pid` drawings.
    Cfb,
    /// `3C ..` (`<`) — ASCII-leading XML document. `SmartPlant`
    /// project XML payloads (e.g. `<ProjectInsulationSpecifications>`)
    /// land here.
    Xml,
    /// Printable ASCII (`0x20..=0x7E`) that does not start with `<`.
    /// `SmartPlant`-private CSV-like text formats (e.g. the Rules
    /// file `"Begin Rules",120,…`) fall in this bucket.
    AsciiText,
    /// Anything else; the raw 4-byte magic is preserved verbatim
    /// for diagnostics. Files shorter than 4 bytes pad the missing
    /// trailing positions with `0x00`.
    Unknown([u8; 4]),
}

/// Classify the first 4 bytes of a `RefData~*` file into a
/// [`RefDataFormat`].
///
/// Slices shorter than 4 bytes are zero-padded on the right
/// before comparison so the function is panic-safe for any
/// input — including empty slices, which classify as
/// `Unknown([0;4])`.
pub fn classify_format(bytes: &[u8]) -> RefDataFormat {
    let mut magic = [0u8; 4];
    let take = bytes.len().min(4);
    magic[..take].copy_from_slice(&bytes[..take]);

    const ZIP: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
    const CFB: [u8; 4] = [0xD0, 0xCF, 0x11, 0xE0];

    if magic == ZIP {
        RefDataFormat::Zip
    } else if magic == CFB {
        RefDataFormat::Cfb
    } else if magic[0] == b'<' {
        RefDataFormat::Xml
    } else if (0x20..=0x7E).contains(&magic[0]) {
        RefDataFormat::AsciiText
    } else {
        RefDataFormat::Unknown(magic)
    }
}

/// Parse `RefData~SCHEMA~ID(.zip)?` into `(schema, id)`.
///
/// Returns `None` for any other shape (wrong prefix, non-numeric
/// segments, extra segments, empty segments). Used internally by
/// [`scan_refdata_dir`] to filter directory entries.
pub fn parse_refdata_filename(name: &str) -> Option<(u32, u32)> {
    let stem = name.strip_suffix(".zip").unwrap_or(name);
    let rest = stem.strip_prefix("RefData~")?;
    let mut parts = rest.split('~');
    let schema = parts.next()?.parse::<u32>().ok()?;
    let id = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((schema, id))
}

/// Scan `dir` for every `RefData~SCHEMA~ID(.zip)?` file and
/// return a sorted vector of [`RefDataEntry`].
///
/// The scan is one level deep — subdirectories are ignored — and
/// non-matching filenames are silently skipped. Each matching
/// entry is opened just long enough to read its first 4 bytes for
/// [`classify_format`]; that read failure on any single file
/// short-circuits the whole scan with the underlying
/// [`std::io::Error`] so callers get a clear, immediate diagnostic
/// instead of a half-populated vector.
///
/// The returned vector is sorted by `(schema, id)` ascending so
/// downstream maps and assertions stay deterministic across
/// platforms regardless of `read_dir`'s native ordering.
pub fn scan_refdata_dir(dir: &Path) -> io::Result<Vec<RefDataEntry>> {
    let mut entries: Vec<RefDataEntry> = Vec::new();
    for dirent in fs::read_dir(dir)? {
        let dirent = dirent?;
        let metadata = dirent.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let file_name_os = dirent.file_name();
        let Some(file_name) = file_name_os.to_str() else {
            continue;
        };
        let Some((schema, id)) = parse_refdata_filename(file_name) else {
            continue;
        };
        let full_path = dirent.path();
        let format = classify_format(&read_magic(&full_path)?);
        entries.push(RefDataEntry {
            schema,
            id,
            file_name: file_name.to_string(),
            full_path,
            size: metadata.len(),
            format,
        });
    }
    entries.sort_by_key(|e| (e.schema, e.id));
    Ok(entries)
}

/// Read up to 4 bytes from the head of `path`. Files shorter than
/// 4 bytes return whatever they actually have — [`classify_format`]
/// pads the rest.
fn read_magic(path: &Path) -> io::Result<Vec<u8>> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buf = [0u8; 4];
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(buf[..filled].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_zip_local_file_header_magic() {
        assert_eq!(
            classify_format(&[0x50, 0x4B, 0x03, 0x04, 0x14]),
            RefDataFormat::Zip,
        );
    }

    #[test]
    fn classify_cfb_ole_compound_file_magic() {
        assert_eq!(
            classify_format(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]),
            RefDataFormat::Cfb,
        );
    }

    #[test]
    fn classify_xml_starts_with_open_angle() {
        // Mirrors the real `RefData~4~709` head:
        // `<ProjectInsulationSpecifications>...`.
        assert_eq!(classify_format(b"<ProjectInsu"), RefDataFormat::Xml);
        assert_eq!(classify_format(b"<?xml versi"), RefDataFormat::Xml);
    }

    #[test]
    fn classify_ascii_text_for_smartplant_rules_csv() {
        // Real `RefData~4~680` head: `"Begin Rules",120,...`
        assert_eq!(
            classify_format(b"\"Begin Rules\""),
            RefDataFormat::AsciiText
        );
    }

    #[test]
    fn classify_unknown_preserves_raw_magic_bytes() {
        let result = classify_format(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(result, RefDataFormat::Unknown([0x01, 0x02, 0x03, 0x04]));
    }

    #[test]
    fn classify_short_input_pads_with_zero() {
        // Three-byte slice: padded right to `[0x01, 0x02, 0x03, 0x00]`.
        assert_eq!(
            classify_format(&[0x01, 0x02, 0x03]),
            RefDataFormat::Unknown([0x01, 0x02, 0x03, 0x00]),
        );
        // Empty slice: `[0; 4]`.
        assert_eq!(
            classify_format(&[]),
            RefDataFormat::Unknown([0x00, 0x00, 0x00, 0x00]),
        );
    }

    #[test]
    fn parse_filename_with_zip_suffix() {
        assert_eq!(parse_refdata_filename("RefData~4~681.zip"), Some((4, 681)));
        assert_eq!(parse_refdata_filename("RefData~4~685.zip"), Some((4, 685)));
    }

    #[test]
    fn parse_filename_without_extension() {
        assert_eq!(parse_refdata_filename("RefData~4~680"), Some((4, 680)));
        assert_eq!(parse_refdata_filename("RefData~4~709"), Some((4, 709)));
    }

    #[test]
    fn parse_filename_rejects_alien_names() {
        assert_eq!(parse_refdata_filename("Manifest.txt"), None);
        assert_eq!(parse_refdata_filename("PlantData~2~711.zip"), None);
        assert_eq!(parse_refdata_filename("RefData"), None);
        assert_eq!(parse_refdata_filename("RefData~4"), None);
        assert_eq!(parse_refdata_filename("RefData~~680"), None);
        assert_eq!(parse_refdata_filename("RefData~4~"), None);
        assert_eq!(parse_refdata_filename("RefData~4~680~extra"), None);
        assert_eq!(parse_refdata_filename("RefData~four~680"), None);
        assert_eq!(parse_refdata_filename("RefData~4~abc"), None);
    }

    #[test]
    fn scan_synthetic_directory_classifies_each_file() {
        // Build a temp directory with one of every magic flavor
        // and assert the scan returns them sorted by (schema, id)
        // with the right format.
        let tmp = tempdir();
        write_file(
            &tmp,
            "RefData~4~681.zip",
            &[0x50, 0x4B, 0x03, 0x04, 0x14, 0x00],
        );
        write_file(
            &tmp,
            "RefData~4~683",
            &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        );
        write_file(&tmp, "RefData~4~709", b"<ProjectInsu");
        write_file(&tmp, "RefData~4~680", b"\"Begin Rules\",120");
        write_file(&tmp, "Manifest.txt", b"BackupType<<|>>2\n");
        write_file(&tmp, "PlantData~2~711.zip", &[0x50, 0x4B, 0x03, 0x04]);

        let entries = scan_refdata_dir(&tmp).expect("scan synthetic dir");
        assert_eq!(entries.len(), 4, "non-RefData files must be skipped");

        let ids: Vec<u32> = entries.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            vec![680, 681, 683, 709],
            "entries must be sorted by (schema, id) ascending",
        );

        let by_id: std::collections::BTreeMap<u32, &RefDataEntry> =
            entries.iter().map(|e| (e.id, e)).collect();
        assert_eq!(by_id[&681].format, RefDataFormat::Zip);
        assert_eq!(by_id[&683].format, RefDataFormat::Cfb);
        assert_eq!(by_id[&709].format, RefDataFormat::Xml);
        assert_eq!(by_id[&680].format, RefDataFormat::AsciiText);

        cleanup(&tmp);
    }

    #[test]
    fn scan_short_file_classifies_as_unknown_padded() {
        let tmp = tempdir();
        write_file(&tmp, "RefData~4~999", &[0xFF, 0xFE]);

        let entries = scan_refdata_dir(&tmp).expect("scan");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].format,
            RefDataFormat::Unknown([0xFF, 0xFE, 0x00, 0x00]),
        );
        assert_eq!(entries[0].size, 2);

        cleanup(&tmp);
    }

    #[test]
    fn scan_empty_directory_returns_empty_vector() {
        let tmp = tempdir();
        let entries = scan_refdata_dir(&tmp).expect("scan empty");
        assert!(entries.is_empty());
        cleanup(&tmp);
    }

    // --- Lightweight tempdir helpers ---
    //
    // We deliberately avoid a `tempfile` dev-dependency for one
    // module-local helper. The scan layer is panic-safe and
    // single-threaded; a process-id + counter directory under
    // `std::env::temp_dir()` is plenty for the unit tests.

    fn tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "pid_parse_refdata_test_{}_{}",
            std::process::id(),
            n,
        ));
        std::fs::create_dir_all(&path).expect("create tempdir");
        path
    }

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).expect("write fixture file");
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
