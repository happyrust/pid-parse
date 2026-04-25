//! Central-directory-only enumeration of entries inside a ZIP
//! archive — the ZIP-payload step of the plant-restore pipeline.
//!
//! `SmartPlant` plant backups ship most reference data and per-plant
//! drawing caches as ZIP archives:
//!
//! * `RefData~SCHEMA~ID(.zip)?` files classified as
//!   [`crate::backup::RefDataFormat::Zip`] by
//!   [`crate::backup::classify_format`] — Symbol catalogues,
//!   templates, etc.
//! * `PlantData~2~*.zip` — drawing cache including the CFB `.pid`
//!   files.
//!
//! This module ships **only the entry index**: open the file, parse
//! the central directory, and return one [`ZipEntry`] per stored
//! item. Decompressing payload bytes is left to dedicated pipelines
//! (the `cfb` crate handles `.pid`; future steps will crack symbol
//! catalogues). Limiting ourselves to metadata keeps the dependency
//! footprint small — the `zip` crate is enabled with
//! `default-features = false`, dropping aes-crypto / bzip2 / xz /
//! lzma / zstd. Reading the central directory and per-entry
//! metadata works in this minimal mode.
//!
//! # Tolerance
//!
//! All public entry points propagate failure via [`ZipIndexError`]
//! (`io::Error` and `zip::result::ZipError` sources). Nothing
//! panics on a malformed archive — invalid central directories,
//! truncated files, and unknown compression methods all surface as
//! a `Zip` error variant the caller can decide to skip or abort
//! on.

use std::fs;
use std::io;
use std::path::Path;

use thiserror::Error;

/// One entry from a ZIP archive's central directory, captured as
/// pure metadata (no payload bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Full path inside the archive (e.g. `"Assemblies/Equipment/"`
    /// or `"CatalogIndex.xml"`). Forward-slash separated, including
    /// any trailing slash on directory entries.
    pub name: String,
    /// Uncompressed size in bytes from the central directory record.
    pub size: u64,
    /// Stored size after compression, in bytes. Equal to `size` for
    /// `Stored` (uncompressed) entries.
    pub compressed_size: u64,
    /// `true` for directory entries — zero-length payload whose
    /// name ends with `/`.
    pub is_dir: bool,
    /// CRC-32 of the uncompressed contents from the central
    /// directory.
    pub crc32: u32,
}

/// Failures from the ZIP entry index layer.
#[derive(Debug, Error)]
pub enum ZipIndexError {
    /// Underlying I/O error opening or reading the archive file.
    #[error("io error opening or reading zip archive: {0}")]
    Io(#[from] io::Error),
    /// `zip` crate refused the archive — invalid central directory,
    /// unsupported version, truncated payload, etc.
    #[error("zip archive parse error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// List every entry's metadata from a ZIP archive on disk.
///
/// Convenience wrapper around [`list_zip_entries_from_reader`]: opens
/// `path` for reading and forwards to the reader-flavored API.
pub fn list_zip_entries(path: &Path) -> Result<Vec<ZipEntry>, ZipIndexError> {
    let file = fs::File::open(path)?;
    list_zip_entries_from_reader(file)
}

/// List every entry's metadata from a ZIP archive provided as a
/// `Read + Seek` source.
///
/// Lets unit tests feed an in-memory [`std::io::Cursor`] without
/// touching the filesystem. Keeps the same metadata-only contract
/// as [`list_zip_entries`] — payload bytes are never decompressed.
pub fn list_zip_entries_from_reader<R: io::Read + io::Seek>(
    reader: R,
) -> Result<Vec<ZipEntry>, ZipIndexError> {
    let mut archive = zip::ZipArchive::new(reader)?;
    let mut out = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        out.push(ZipEntry {
            name: entry.name().to_string(),
            size: entry.size(),
            compressed_size: entry.compressed_size(),
            is_dir: entry.is_dir(),
            crc32: entry.crc32(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Minimal valid ZIP file: only an end-of-central-directory
    /// record (`PK\x05\x06`) with all zero counts. 22 bytes.
    /// Verified with `zip -j /tmp/empty.zip /dev/null && head -c
    /// 22 /tmp/empty.zip` style tooling — the EOCD layout is
    /// fixed by ZIP spec section 4.3.16.
    const EMPTY_ZIP: &[u8] = &[
        0x50, 0x4B, 0x05, 0x06, // signature `PK\x05\x06`
        0x00, 0x00, // disk number
        0x00, 0x00, // disk where CD starts
        0x00, 0x00, // CD records on this disk
        0x00, 0x00, // total CD records
        0x00, 0x00, 0x00, 0x00, // CD size
        0x00, 0x00, 0x00, 0x00, // CD offset
        0x00, 0x00, // comment length
    ];

    #[test]
    fn list_empty_archive_returns_empty_vector() {
        let entries =
            list_zip_entries_from_reader(Cursor::new(EMPTY_ZIP)).expect("read empty archive");
        assert!(entries.is_empty());
    }

    /// Sentinel CRC-32 value baked into the synthetic central
    /// directory below. The metadata-only `list_zip_entries*`
    /// surface never validates CRC against payload bytes, so any
    /// stable u32 round-trips through `entry.crc32()` and lets us
    /// assert the field plumbing without depending on a hashing
    /// crate.
    const FAKE_CRC: u32 = 0xDEAD_BEEF;

    /// Minimal ZIP with one stored (uncompressed) file
    /// `hello.txt` containing the bytes `Hi!` (3 bytes). The CRC
    /// header field uses [`FAKE_CRC`] — `zip::ZipArchive::new`
    /// only reads central-directory metadata and does not verify
    /// CRC at this stage.
    fn single_stored_entry_zip() -> Vec<u8> {
        // Layout (per APPNOTE 4.3.7 / 4.3.12):
        //   1. Local file header for `hello.txt`
        //   2. Stored payload bytes `Hi!`
        //   3. Central directory record for `hello.txt`
        //   4. End of central directory record
        let payload = b"Hi!";
        let name = b"hello.txt";

        let mut buf: Vec<u8> = Vec::new();

        buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        buf.extend_from_slice(&20u16.to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        buf.extend_from_slice(&FAKE_CRC.to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(name);

        let local_header_offset = 0u32;
        buf.extend_from_slice(payload);

        let cd_offset = buf.len() as u32;
        buf.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        buf.extend_from_slice(&20u16.to_le_bytes());
        buf.extend_from_slice(&20u16.to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        buf.extend_from_slice(&FAKE_CRC.to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        buf.extend_from_slice(&local_header_offset.to_le_bytes());
        buf.extend_from_slice(name);

        let cd_size = buf.len() as u32 - cd_offset;
        buf.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&cd_size.to_le_bytes());
        buf.extend_from_slice(&cd_offset.to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf
    }

    #[test]
    fn list_single_file_archive_captures_metadata() {
        let archive = single_stored_entry_zip();
        let entries =
            list_zip_entries_from_reader(Cursor::new(archive)).expect("read single-entry archive");
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.name, "hello.txt");
        assert_eq!(e.size, 3);
        assert_eq!(e.compressed_size, 3);
        assert!(!e.is_dir);
        assert_eq!(e.crc32, FAKE_CRC);
    }

    #[test]
    fn list_truncated_archive_reports_zip_error() {
        // Cut the EOCD signature — `zip::ZipArchive::new` should
        // fail and we must surface that as `ZipIndexError::Zip`,
        // not panic.
        let mut bad = EMPTY_ZIP.to_vec();
        bad.truncate(10);
        let result = list_zip_entries_from_reader(Cursor::new(bad));
        assert!(matches!(result, Err(ZipIndexError::Zip(_))));
    }

    #[test]
    fn list_empty_byte_slice_reports_zip_error() {
        let result = list_zip_entries_from_reader(Cursor::new(Vec::<u8>::new()));
        assert!(matches!(result, Err(ZipIndexError::Zip(_))));
    }

    #[test]
    fn list_zip_entries_from_path_returns_io_error_for_missing_file() {
        let result = list_zip_entries(Path::new(
            "non-existent-zip-fixture-for-error-path-coverage.zip",
        ));
        assert!(matches!(result, Err(ZipIndexError::Io(_))));
    }
}
