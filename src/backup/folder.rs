//! One-pass inventory of a `SmartPlant` backup folder.
//!
//! A backup package is a directory, not a single file. Callers used to
//! open [`super::manifest`], [`super::mtf`], [`super::refdata`] and
//! [`super::zip_index`] separately. [`scan_backup_folder`] walks the
//! directory once and returns those four inventories together:
//!
//! * `Manifest.txt` — parsed with [`super::parse_manifest_bytes`].
//! * `Export.dmp` — the first 4 KiB only. A `TAPE` header becomes
//!   [`ExportDumpFormat::SqlServerMtf`]. An Oracle `exp` header becomes
//!   [`ExportDumpFormat::OracleExp`]. Anything else is recorded, not fatal.
//! * `RefData~SCHEMA~ID` — the existing filename and magic-byte scan,
//!   plus a central-directory listing when the file is a ZIP.
//! * `PlantData~SCHEMA~ID` — the same filename shape and listing.
//!
//! The scan does not extract an MDF, decompress ZIP payloads, or write
//! a database. A missing member is an empty slot. A member that is
//! present but unreadable is listed in [`BackupFolderScan::problems`]
//! and the other members are still returned. Only a failure to read
//! the directory itself is an [`std::io::Error`].

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::manifest::{parse_manifest_bytes, Manifest};
use super::mtf::{detect_non_mtf_dump_format, MtfHeader};
use super::refdata::{classify_format, scan_refdata_dir, RefDataEntry, RefDataFormat};
use super::zip_index::{list_zip_entries, ZipEntry};

const MANIFEST_FILE_NAME: &str = "Manifest.txt";
const EXPORT_FILE_NAME: &str = "Export.dmp";

/// How many leading bytes of `Export.dmp` the sniff reads.
///
/// [`MtfHeader::probe`] needs the 52-byte common block header.
/// [`detect_non_mtf_dump_format`] needs the Oracle `EXPORT:V` line,
/// which sits in the first few dozen bytes. The logical-block-size
/// scan walks up to 256 KiB and is intentionally not part of this
/// inventory.
const EXPORT_SNIFF_LEN: usize = 4096;

/// Inventory of one backup directory.
#[derive(Debug, Clone)]
pub struct BackupFolderScan {
    /// Directory that was walked. One level only; nested folders are ignored.
    pub path: PathBuf,
    /// Parsed `Manifest.txt`, when the file was present and readable.
    pub manifest: Option<Manifest>,
    /// `Export.dmp` sniff, when the file was present and its prefix was readable.
    pub export: Option<ExportDumpScan>,
    /// `RefData~SCHEMA~ID` files, sorted by `(schema, id)`.
    pub refdata: Vec<RefDataFile>,
    /// `PlantData~SCHEMA~ID` files, sorted by `(schema, id)`.
    pub plant_data: Vec<PlantDataFile>,
    /// Members that were present but could not be read. Other fields
    /// still hold whatever succeeded.
    pub problems: Vec<ScanProblem>,
}

/// One `RefData` file plus, when it is a ZIP, its central directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefDataFile {
    /// Filename, magic-byte format, and size from [`scan_refdata_dir`].
    pub entry: RefDataEntry,
    /// Central-directory listing for a ZIP. Other formats stay
    /// [`ZipListing::NotAZip`].
    pub zip: ZipListing,
}

/// One `PlantData` file found directly inside the backup folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantDataFile {
    /// Schema group from `PlantData~SCHEMA~ID`. Observed backups use `2`.
    pub schema: u32,
    /// Numeric id from the third `~` segment.
    pub id: u32,
    /// File name as it appears on disk, including a `.zip` suffix when present.
    pub file_name: String,
    /// Path passed back to [`list_zip_entries`] for ZIP members.
    pub full_path: PathBuf,
    /// Length in bytes from the directory entry.
    pub size: u64,
    /// Format from the first four magic bytes. See [`classify_format`].
    pub format: RefDataFormat,
    /// Central-directory listing for a ZIP. Other formats stay
    /// [`ZipListing::NotAZip`].
    pub zip: ZipListing,
}

/// Result of opening a member whose magic bytes say ZIP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipListing {
    /// The leading bytes are not a ZIP local-file header.
    NotAZip,
    /// Central-directory metadata. Payload bytes were not decompressed.
    Listed(Vec<ZipEntry>),
    /// The file looks like a ZIP, but the central directory could not be read.
    /// [`BackupFolderScan::problems`] carries the reason.
    Unreadable,
}

/// Why one present member was left out of the typed fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProblem {
    /// File or directory the failure belongs to.
    pub path: PathBuf,
    /// [`std::io::Error`] or ZIP error text.
    pub detail: String,
}

/// `Export.dmp` location and the format of its prefix.
#[derive(Debug, Clone)]
pub struct ExportDumpScan {
    /// Path of `Export.dmp`.
    pub path: PathBuf,
    /// Full file length from the directory entry. The sniff reads at
    /// most 4 KiB; callers that need the MDF still go through
    /// `pid_backup_extract`.
    pub size: u64,
    /// Classification of the leading bytes.
    pub format: ExportDumpFormat,
}

/// What the leading bytes of `Export.dmp` are.
#[derive(Debug, Clone)]
pub enum ExportDumpFormat {
    /// SQL Server MTF stream. The value is the `TAPE` header.
    /// Block size is not recovered here.
    SqlServerMtf(MtfHeader),
    /// Oracle conventional `exp` dump. `version_line` is the
    /// `EXPORT:V…` text with the framing bytes removed.
    /// Row decoding stays in `publish::oracle_exp`.
    OracleExp {
        /// ASCII version line, for example `EXPORT:V12.02.00`.
        version_line: String,
    },
    /// Present, and neither an MTF `TAPE` header nor an Oracle `exp` dump.
    Unrecognized {
        /// [`MtfHeader::probe`] error text.
        detail: String,
    },
}

/// Parse `PlantData~SCHEMA~ID(.zip)?` into `(schema, id)`.
///
/// Returns `None` for any other shape, including `RefData` names and
/// `Manifest.txt`.
pub fn parse_plant_data_filename(name: &str) -> Option<(u32, u32)> {
    let stem = name.strip_suffix(".zip").unwrap_or(name);
    let rest = stem.strip_prefix("PlantData~")?;
    let mut parts = rest.split('~');
    let schema = parts.next()?.parse::<u32>().ok()?;
    let id = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((schema, id))
}

/// Walk `dir` once and inventory `Manifest.txt`, `Export.dmp`,
/// `RefData` and `PlantData`.
///
/// # Errors
///
/// Returns an error when `dir` cannot be read. Per-file failures are
/// collected on [`BackupFolderScan::problems`] instead.
pub fn scan_backup_folder(dir: &Path) -> io::Result<BackupFolderScan> {
    let mut manifest_path = None;
    let mut export_hit = None;
    let mut plant_hits = Vec::new();

    for dirent in fs::read_dir(dir)? {
        let dirent = dirent?;
        let metadata = dirent.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let file_name = dirent.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name == MANIFEST_FILE_NAME {
            manifest_path = Some(dirent.path());
        } else if name == EXPORT_FILE_NAME {
            export_hit = Some((dirent.path(), metadata.len()));
        } else if let Some((schema, id)) = parse_plant_data_filename(name) {
            plant_hits.push(PlantHit {
                schema,
                id,
                file_name: name.to_string(),
                full_path: dirent.path(),
                size: metadata.len(),
            });
        }
    }
    plant_hits.sort_by_key(|hit| (hit.schema, hit.id));

    let mut problems = Vec::new();
    let manifest = read_manifest(manifest_path, &mut problems);
    let export = read_export(export_hit, &mut problems);
    let refdata = read_refdata(dir, &mut problems);
    let plant_data = read_plant_data(plant_hits, &mut problems);

    Ok(BackupFolderScan {
        path: dir.to_path_buf(),
        manifest,
        export,
        refdata,
        plant_data,
        problems,
    })
}

struct PlantHit {
    schema: u32,
    id: u32,
    file_name: String,
    full_path: PathBuf,
    size: u64,
}

fn read_manifest(path: Option<PathBuf>, problems: &mut Vec<ScanProblem>) -> Option<Manifest> {
    let path = path?;
    match fs::read(&path) {
        Ok(bytes) => Some(parse_manifest_bytes(&bytes)),
        Err(err) => {
            problems.push(ScanProblem {
                path,
                detail: format!("Manifest.txt: {err}"),
            });
            None
        }
    }
}

fn read_export(
    hit: Option<(PathBuf, u64)>,
    problems: &mut Vec<ScanProblem>,
) -> Option<ExportDumpScan> {
    let (path, size) = hit?;
    match read_prefix(&path, EXPORT_SNIFF_LEN) {
        Ok(prefix) => Some(ExportDumpScan {
            path,
            size,
            format: classify_export(&prefix),
        }),
        Err(err) => {
            problems.push(ScanProblem {
                path,
                detail: format!("Export.dmp: {err}"),
            });
            None
        }
    }
}

fn classify_export(prefix: &[u8]) -> ExportDumpFormat {
    if detect_non_mtf_dump_format(prefix).is_some() {
        return ExportDumpFormat::OracleExp {
            version_line: oracle_version_line(prefix),
        };
    }
    match MtfHeader::probe(prefix) {
        Ok(header) => ExportDumpFormat::SqlServerMtf(header),
        Err(err) => ExportDumpFormat::Unrecognized {
            detail: err.to_string(),
        },
    }
}

fn oracle_version_line(data: &[u8]) -> String {
    let after = data.get(3..).unwrap_or(b"");
    let end = after
        .iter()
        .position(|&byte| byte == b'\n' || byte == b'\r')
        .unwrap_or(after.len().min(40));
    let bytes = after.get(..end).unwrap_or(b"");
    String::from_utf8_lossy(bytes).into_owned()
}

fn read_refdata(dir: &Path, problems: &mut Vec<ScanProblem>) -> Vec<RefDataFile> {
    let entries = match scan_refdata_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            problems.push(ScanProblem {
                path: dir.to_path_buf(),
                detail: format!("RefData scan failed: {err}"),
            });
            return Vec::new();
        }
    };
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        let zip = list_if_zip(&entry.full_path, entry.format, problems);
        files.push(RefDataFile { entry, zip });
    }
    files
}

fn read_plant_data(hits: Vec<PlantHit>, problems: &mut Vec<ScanProblem>) -> Vec<PlantDataFile> {
    let mut files = Vec::with_capacity(hits.len());
    for hit in hits {
        let format = match read_prefix(&hit.full_path, 4) {
            Ok(magic) => classify_format(&magic),
            Err(err) => {
                problems.push(ScanProblem {
                    path: hit.full_path,
                    detail: format!("{}: {err}", hit.file_name),
                });
                continue;
            }
        };
        let zip = list_if_zip(&hit.full_path, format, problems);
        files.push(PlantDataFile {
            schema: hit.schema,
            id: hit.id,
            file_name: hit.file_name,
            full_path: hit.full_path,
            size: hit.size,
            format,
            zip,
        });
    }
    files
}

fn list_if_zip(path: &Path, format: RefDataFormat, problems: &mut Vec<ScanProblem>) -> ZipListing {
    if format != RefDataFormat::Zip {
        return ZipListing::NotAZip;
    }
    match list_zip_entries(path) {
        Ok(entries) => ZipListing::Listed(entries),
        Err(err) => {
            problems.push(ScanProblem {
                path: path.to_path_buf(),
                detail: err.to_string(),
            });
            ZipListing::Unreadable
        }
    }
}

fn read_prefix(path: &Path, max_len: usize) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; max_len];
    let mut filled = 0usize;
    while filled < max_len {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::MtfBlockType;

    /// One stored `hello.txt`. The local header starts with `PK\x03\x04`,
    /// which is the magic `classify_format` treats as a ZIP. An archive
    /// that is only an end-of-central-directory record starts with
    /// `PK\x05\x06` and is not classified as a ZIP.
    fn stored_hello_zip() -> Vec<u8> {
        let payload = b"Hi!";
        let name = b"hello.txt";
        let crc = 0xDEAD_BEEFu32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        buf.extend_from_slice(&20u16.to_le_bytes());
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        buf.extend_from_slice(&crc.to_le_bytes());
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
        buf.extend_from_slice(&crc.to_le_bytes());
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
    fn parse_plant_data_filename_accepts_zip_and_bare_names() {
        assert_eq!(
            parse_plant_data_filename("PlantData~2~711.zip"),
            Some((2, 711))
        );
        assert_eq!(parse_plant_data_filename("PlantData~2~711"), Some((2, 711)));
    }

    #[test]
    fn parse_plant_data_filename_rejects_other_backup_members() {
        assert_eq!(parse_plant_data_filename("RefData~4~681.zip"), None);
        assert_eq!(parse_plant_data_filename("Manifest.txt"), None);
        assert_eq!(parse_plant_data_filename("Export.dmp"), None);
        assert_eq!(parse_plant_data_filename("PlantData~2"), None);
    }

    #[test]
    fn empty_directory_has_no_members_and_no_problems() {
        let tmp = TempDir::new();
        let scan = scan_backup_folder(&tmp.path).expect("scan empty dir");
        assert!(scan.manifest.is_none());
        assert!(scan.export.is_none());
        assert!(scan.refdata.is_empty());
        assert!(scan.plant_data.is_empty());
        assert!(scan.problems.is_empty());
    }

    #[test]
    fn missing_directory_is_an_io_error() {
        let tmp = TempDir::new();
        let missing = tmp.path.join("no-such-folder");
        assert!(scan_backup_folder(&missing).is_err());
    }

    #[test]
    fn one_pass_reads_manifest_mtf_refdata_and_plant_data() {
        let tmp = TempDir::new();
        fs::write(
            tmp.path.join("Manifest.txt"),
            b"Name<<|>>TEST02\nVersion<<|>>7.02\n",
        )
        .expect("write manifest");
        let mut export = b"TAPE".to_vec();
        export.resize(52, 0);
        fs::write(tmp.path.join("Export.dmp"), &export).expect("write export");
        fs::write(tmp.path.join("RefData~4~680"), b"\"Begin Rules\"").expect("write rules");
        let hello = stored_hello_zip();
        fs::write(tmp.path.join("RefData~4~681.zip"), &hello).expect("write ref zip");
        fs::write(tmp.path.join("PlantData~2~711.zip"), &hello).expect("write plant zip");
        fs::write(tmp.path.join("PlantData~2~712"), b"<Plant").expect("write plant xml");
        fs::write(tmp.path.join("notes.txt"), b"ignore me").expect("write extra");
        fs::create_dir(tmp.path.join("nested")).expect("nested dir");
        fs::write(
            tmp.path.join("nested").join("Manifest.txt"),
            b"Name<<|>>NESTED",
        )
        .expect("nested manifest");

        let scan = scan_backup_folder(&tmp.path).expect("scan");
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);

        let manifest = scan.manifest.expect("manifest");
        assert_eq!(manifest.first_field("Name"), Some("TEST02"));
        assert_eq!(manifest.first_field("Version"), Some("7.02"));
        assert_eq!(manifest.lines.len(), 2);

        let export = scan.export.expect("export");
        assert_eq!(export.size, 52);
        match export.format {
            ExportDumpFormat::SqlServerMtf(header) => {
                assert_eq!(header.block_type, MtfBlockType::Tape);
            }
            other => panic!("expected MTF, got {other:?}"),
        }

        assert_eq!(scan.refdata.len(), 2);
        assert_eq!(scan.refdata[0].entry.id, 680);
        assert_eq!(scan.refdata[0].entry.format, RefDataFormat::AsciiText);
        assert_eq!(scan.refdata[0].zip, ZipListing::NotAZip);
        assert_eq!(scan.refdata[1].entry.id, 681);
        match &scan.refdata[1].zip {
            ZipListing::Listed(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].name, "hello.txt");
                assert_eq!(entries[0].size, 3);
            }
            other => panic!("expected a listed zip, got {other:?}"),
        }

        assert_eq!(scan.plant_data.len(), 2);
        assert_eq!(scan.plant_data[0].id, 711);
        assert_eq!(scan.plant_data[0].format, RefDataFormat::Zip);
        match &scan.plant_data[0].zip {
            ZipListing::Listed(entries) => assert_eq!(entries[0].name, "hello.txt"),
            other => panic!("expected a listed zip, got {other:?}"),
        }
        assert_eq!(scan.plant_data[1].id, 712);
        assert_eq!(scan.plant_data[1].format, RefDataFormat::Xml);
        assert_eq!(scan.plant_data[1].zip, ZipListing::NotAZip);
    }

    #[test]
    fn oracle_export_is_classified_without_dropping_the_manifest() {
        let tmp = TempDir::new();
        fs::write(tmp.path.join("Manifest.txt"), b"Name<<|>>DWG\n").expect("manifest");
        fs::write(
            tmp.path.join("Export.dmp"),
            b"\x03\x03iEXPORT:V12.02.00\nrest-of-dump",
        )
        .expect("export");

        let scan = scan_backup_folder(&tmp.path).expect("scan");
        assert_eq!(
            scan.manifest.expect("manifest").first_field("Name"),
            Some("DWG")
        );
        match scan.export.expect("export").format {
            ExportDumpFormat::OracleExp { version_line } => {
                assert_eq!(version_line, "EXPORT:V12.02.00");
            }
            other => panic!("expected Oracle exp, got {other:?}"),
        }
        assert!(scan.problems.is_empty());
    }

    #[test]
    fn unrecognized_export_and_broken_zip_stay_on_the_scan() {
        let tmp = TempDir::new();
        let mut export = b"XXXX".to_vec();
        export.resize(52, 0);
        fs::write(tmp.path.join("Export.dmp"), export).expect("export");
        fs::write(tmp.path.join("RefData~4~681.zip"), b"PK\x03\x04garbage").expect("bad zip");
        fs::write(tmp.path.join("PlantData~2~1"), b"plain").expect("plant text");

        let scan = scan_backup_folder(&tmp.path).expect("scan");
        match scan.export.expect("export").format {
            ExportDumpFormat::Unrecognized { detail } => {
                assert!(detail.contains("TAPE"), "{detail}");
            }
            other => panic!("expected unrecognized, got {other:?}"),
        }
        assert_eq!(scan.refdata.len(), 1);
        assert_eq!(scan.refdata[0].zip, ZipListing::Unreadable);
        assert_eq!(scan.plant_data.len(), 1);
        assert_eq!(scan.plant_data[0].zip, ZipListing::NotAZip);
        assert_eq!(scan.problems.len(), 1);
        assert!(scan.problems[0].detail.contains("zip") || scan.problems[0].detail.contains("Zip"));
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "pid_parse_backup_folder_{}_{}",
                std::process::id(),
                n
            ));
            fs::create_dir_all(&path).expect("create tempdir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
