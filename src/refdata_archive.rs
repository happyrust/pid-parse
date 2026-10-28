//! The reference data a backup package ships inside its `RefData~*.zip`
//! archives: the `.igr` border templates and the `.sym` symbol bodies.
//!
//! A plant backup keeps its reference data in a handful of `RefData~SCHEMA~ID`
//! files beside `Export.dmp`, most of them ZIPs. Two of them are the data a
//! drawing needs to be *drawn*:
//!
//! * `RefData~4~682.zip` — the `.igr` border templates, one per sheet size
//!   (`A0-W.igr`, `CPE-SQ-A2.igr`, `CPECCHBA2.igr`, …), flat at the archive
//!   root, each with a `*-New.pid` twin this reader ignores.
//! * `RefData~4~681.zip` — the symbol library, as the same tree a share holds
//!   (`Design/…`, `Piping/…`, `Equipment/…`, `Instrumentation/…`,
//!   `Assemblies/…`, `Toolbars/…`), which is what makes an archive a drop-in
//!   for a symbol root rather than a directory of unrelated files.
//!
//! [`crate::backup::zip_index`] stops at the central directory, because the
//! publish pipeline only needs to know what a backup holds. Drawing a page
//! needs the bytes, so this module inflates them — and it is a separate
//! feature (`refdata-archive`) from the publish side, so a consumer that only
//! draws (`pid-plot`) reads a backup's reference data without linking the
//! plant-database pipeline.
//!
//! # Names are not what they look like
//!
//! Two things about these entry names cost a round each, so both are handled
//! here rather than at every call site:
//!
//! * The name carries a **UTF-8 BOM** — `\u{feff}CPECCHBA2.igr`, every entry
//!   in the archive, directory entries included. A `Path::file_name` keeps it,
//!   so an exact-name lookup and a `ends_with(\"CPECCHBA2.igr\")` both fail
//!   against the untouched name. [`member_name`] strips it.
//! * Names are stored with **either separator**; members are joined in
//!   [`ArchiveFile::label`] with the platform separator after normalization to
//!   `/`, so the label behaves like a path everywhere the plot side reports
//!   one.
//!
//! Reading a member never goes through `ZipArchive::by_name`: that matches the
//! `zip` crate's own decoding of the stored name, which refuses the GBK names
//! `SmartPlant` writes. Entries are walked by index and matched against
//! [`member_name`], the same way [`crate::backup::zip_index`] lists them.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Failures from the reference-data archive reader.
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// The archive could not be opened, or a member's bytes could not be read.
    #[error("io error reading an archive: {0}")]
    Io(#[from] io::Error),
    /// `zip` refused the archive — invalid central directory, unsupported
    /// compression method, truncated file, …
    #[error("zip archive parse error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// The archive holds no member of that name.
    #[error("no member named {name} in {archive}")]
    NoSuchMember {
        /// The archive that was searched.
        archive: PathBuf,
        /// The member name that was asked for, normalized.
        name: String,
    },
}

/// One file inside a `RefData~*.zip`, with the archive it came from.
///
/// Carrying the archive beside the name is what lets a member stand in for a
/// file everywhere the plot side names one: [`ArchiveFile::label`] is the pair
/// as a path, and its last component is the member's file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    archive: PathBuf,
    name: String,
    size: u64,
}

impl ArchiveFile {
    /// The `RefData~*.zip` this member lives in.
    #[must_use]
    pub fn archive(&self) -> &Path {
        &self.archive
    }

    /// The member's path inside the archive: `/`-separated, BOM-free, with no
    /// leading or trailing separator.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The member's uncompressed size in bytes, from the central directory.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The archive and the member as one path: `<…/RefData~4~682.zip>\A2-W.igr`
    /// (or `…/RefData~4~682.zip/A2-W.igr`), whose file name is the member's.
    ///
    /// This is the name to report and to match on — `Path::ends_with` and
    /// `Path::file_stem` both read the member's own name from it — so nothing
    /// downstream has to know whether a template came from a directory or from
    /// an archive.
    #[must_use]
    pub fn label(&self) -> PathBuf {
        self.archive.join(&self.name)
    }
}

/// The `RefData~*.zip` archives directly inside `dir`, path-sorted.
///
/// Only names that begin with `RefData` and end in `.zip` count. A backup
/// package keeps them in its own folder beside `Manifest.txt`, so the caller
/// passes the package directory (and, for a drawing that sits one level down,
/// its parent). A directory that is absent or unreadable yields nothing rather
/// than an error: no archives is a normal corpus.
#[must_use]
pub fn refdata_archives(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_refdata_archive(path))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Whether a file name is one of the backup's reference-data archives.
fn is_refdata_archive(path: &Path) -> bool {
    let named_refdata = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().starts_with("refdata"));
    let zipped = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    named_refdata && zipped
}

/// Every member of `archive` whose extension is `extension`, name-sorted.
///
/// `extension` is asked for without its dot — `igr` or `sym` — and compared
/// case-insensitively. Only the central directory is read — no member is
/// inflated — so this is the cheap way to ask what an archive holds.
///
/// # Errors
///
/// Returns [`ArchiveError`] when the archive cannot be opened or is not a
/// readable ZIP.
pub fn archive_members(archive: &Path, extension: &str) -> Result<Vec<ArchiveFile>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut out: Vec<ArchiveFile> = Vec::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = member_name(&decode_zip_name(entry.name_raw()));
        if member_extension(&name).is_some_and(|found| found.eq_ignore_ascii_case(extension)) {
            out.push(ArchiveFile {
                archive: archive.to_path_buf(),
                name,
                size: entry.size(),
            });
        }
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(out)
}

/// The bytes of `name` inside `archive`.
///
/// # Errors
///
/// Returns [`ArchiveError::NoSuchMember`] when the archive holds no such
/// member, or [`ArchiveError`] when the archive itself cannot be read.
pub fn read_archive_member(archive: &Path, name: &str) -> Result<Vec<u8>, ArchiveError> {
    let wanted = member_name(name);
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry.is_dir() || member_name(&decode_zip_name(entry.name_raw())) != wanted {
            continue;
        }
        // The buffer grows with the bytes that are actually read rather than
        // with `entry.size()`: a central directory is input like any other,
        // and a record claiming gigabytes must not become an allocation.
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        return Ok(bytes);
    }
    Err(ArchiveError::NoSuchMember {
        archive: archive.to_path_buf(),
        name: wanted,
    })
}

/// [`read_archive_member`] for a member [`archive_members`] already named.
///
/// # Errors
///
/// As [`read_archive_member`].
pub fn read_archive_file(file: &ArchiveFile) -> Result<Vec<u8>, ArchiveError> {
    read_archive_member(&file.archive, &file.name)
}

/// The member name as this module reports it: no BOM, `/`-separated, no
/// trailing separator.
fn member_name(raw: &str) -> String {
    let trimmed = raw.trim_start_matches('\u{feff}');
    let slashed = trimmed.replace('\\', "/");
    let without_dirs = slashed.trim_end_matches('/');
    let mut out = String::with_capacity(without_dirs.len());
    for component in without_dirs.split('/') {
        if component.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(component);
    }
    out
}

/// A member name's extension, without its dot.
fn member_extension(name: &str) -> Option<&str> {
    let leaf = name.rsplit('/').next()?;
    leaf.rsplit_once('.').map(|(_, extension)| extension)
}

/// The name bytes of a ZIP entry as text.
///
/// A UTF-8 name is one; otherwise the bytes are GBK, which is what
/// `SmartPlant` writes for the Chinese paths in these archives. A name that is
/// neither comes back as hex so it stays visible and unique rather than being
/// replaced into a collision.
pub(crate) fn decode_zip_name(raw: &[u8]) -> String {
    if let Ok(utf8) = std::str::from_utf8(raw) {
        return utf8.to_string();
    }
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(raw);
    if had_errors {
        raw.iter().map(|byte| format!("{byte:02x}")).collect()
    } else {
        decoded.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A ZIP holding one stored (uncompressed) member per pair.
    ///
    /// Built here so the tests do not depend on a backup being installed; the
    /// names are written as given, which is how the BOM case is stated.
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options: zip::write::FileOptions<'_, '_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, bytes) in entries {
                writer.start_file(*name, options).expect("start member");
                writer.write_all(bytes).expect("member bytes");
            }
            writer.finish().expect("finish archive");
        }
        buffer.into_inner()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pid-parse-refdata-{name}"));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn a_bom_and_either_separator_leave_the_same_member_name() {
        // Every entry in the wild archive carries the BOM, directories
        // included — `\u{feff}A0-W.igr` and `RefData~4~682.zip` is what a
        // `Path::file_name` would otherwise keep as a different name.
        assert_eq!(member_name("\u{feff}CPECCHBA2.igr"), "CPECCHBA2.igr");
        assert_eq!(
            member_name("\u{feff}Piping\\Valves\\Gate Valve.sym"),
            "Piping/Valves/Gate Valve.sym"
        );
        // A directory entry and a nested member reduce to one spelling each.
        assert_eq!(member_name("Assemblies/Equipment/"), "Assemblies/Equipment");
        assert_eq!(
            member_name("Assemblies//Equipment/"),
            "Assemblies/Equipment"
        );
        // A name that is nothing but the BOM has no name — it can never match
        // a request, which is the correct outcome for a nameless entry.
        assert_eq!(member_name("\u{feff}"), "");
    }

    #[test]
    fn only_refdata_zips_are_archives() {
        assert!(is_refdata_archive(Path::new("RefData~4~682.zip")));
        assert!(is_refdata_archive(Path::new("refdata~1~2.ZIP")));
        // The drawing cache is a ZIP beside them and is not reference data;
        // a `RefData` file that is not a ZIP is a database, not an archive.
        assert!(!is_refdata_archive(Path::new("PlantData~2~711.zip")));
        assert!(!is_refdata_archive(Path::new("RefData~4~680")));
        assert!(!is_refdata_archive(Path::new("RefData~4~680.zip.bak")));
    }

    #[test]
    fn a_directory_lists_its_refdata_archives_in_name_order() {
        let dir = scratch("list");
        for name in [
            "PlantData~2~711.zip",
            "RefData~4~682.zip",
            "RefData~4~681.zip",
            "RefData~4~680",
        ] {
            fs::write(dir.join(name), b"x").expect("placeholder");
        }
        let found = refdata_archives(&dir);
        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names, ["RefData~4~681.zip", "RefData~4~682.zip"]);
        // An absent directory is an empty list, not an error: no archives is
        // the normal case for a project corpus.
        assert!(refdata_archives(&dir.join("absent")).is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn members_are_listed_by_extension_and_read_back_byte_for_byte() {
        let dir = scratch("members");
        let path = dir.join("RefData~4~682.zip");
        let bytes = archive(&[
            ("\u{feff}CPECCHBA2.igr", b"template"),
            ("\u{feff}CPECCHBA2.pid", b"the twin"),
            ("Ref\\Symbols\\Gate Valve.sym", b"symbol"),
        ]);
        fs::write(&path, &bytes).expect("archive");

        let igr = archive_members(&path, "igr").expect("list igr");
        assert_eq!(igr.len(), 1);
        // The BOM is gone, so the label's last component is the name a link
        // record and a `Path::ends_with` both spell.
        assert_eq!(igr[0].name(), "CPECCHBA2.igr");
        assert_eq!(igr[0].size(), 8);
        assert!(igr[0].label().ends_with("CPECCHBA2.igr"));
        assert_eq!(igr[0].label().parent(), Some(path.as_path()));

        // A member is read by the name this module reports, BOM or not.
        assert_eq!(read_archive_file(&igr[0]).expect("read"), b"template");
        assert_eq!(
            read_archive_member(&path, "\u{feff}CPECCHBA2.igr").expect("read with BOM"),
            b"template"
        );
        // The separator in a stored name is not part of the name.
        assert_eq!(
            read_archive_member(&path, "Ref/Symbols/Gate Valve.sym").expect("read"),
            b"symbol"
        );
        assert_eq!(
            archive_members(&path, "SYM")
                .expect("extension is case-insensitive")
                .len(),
            1
        );
        assert!(archive_members(&path, "sym").expect("list sym")[0]
            .name()
            .starts_with("Ref/"));

        // A name nothing carries, an archive that is not a ZIP, and a missing
        // file are the three failures — all errors, none a panic, and each one
        // names what it was looking at when it happened.
        let absent = read_archive_member(&path, "absent.igr").expect_err("no such member");
        assert!(matches!(absent, ArchiveError::NoSuchMember { .. }));
        let message = absent.to_string();
        assert!(message.contains("absent.igr"), "{message}");
        assert!(message.contains("RefData~4~682.zip"), "{message}");
        let bad = dir.join("RefData~4~680.zip");
        fs::write(&bad, b"not a zip at all").expect("bad archive");
        assert!(matches!(
            archive_members(&bad, "igr"),
            Err(ArchiveError::Zip(_))
        ));
        assert!(matches!(
            archive_members(&dir.join("RefData~4~683.zip"), "igr"),
            Err(ArchiveError::Io(_))
        ));
        fs::remove_dir_all(&dir).ok();
    }
}
