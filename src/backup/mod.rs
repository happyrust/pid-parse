//! Parsers for `SmartPlant` / Smart P&ID backup packages (`*.pid` database
//! backups produced by the `SmartPlant` desktop tool).
//!
//! A `SmartPlant` backup package is a folder containing:
//!
//! * `Export.dmp` — either an **Oracle `exp` dump** (the canonical
//!   object / relationship / attribute store on Oracle-backed plants;
//!   parsed by [`crate::publish::oracle_exp`]) or a **Microsoft Tape
//!   Format (MTF)** envelope around SQL Server Master Data Files
//!   (MDF/LDF; see [`mtf`]). [`scan_backup_folder`] sniffs which.
//! * `Manifest.txt` — a plain text `key<<|>>value` listing describing the
//!   plant, the databases, and per-file metadata.
//! * `PlantData~2~*.zip` — per-plant drawing cache (includes the CFB
//!   `.pid` drawing files). Parsed by [`crate::package`] /
//!   [`crate::cfb`].
//! * `RefData~4~*.zip` — reference data: symbol libraries, templates,
//!   report files, material specs, etc.
//! * `PlantConfig.xml` — drawing style configuration.
//!
//! [`scan_backup_folder`] walks that directory once and returns
//! `Manifest.txt`, an `Export.dmp` sniff (MTF header or Oracle `exp`),
//! every `RefData` file, and every `PlantData` file. ZIP members
//! contribute a central-directory index. The scan does not extract an
//! MDF and does not write a database.
//!
//! The [`mtf`] submodule parses the MTF envelope so downstream stages can
//! extract the raw MDF bytes for relational-database parsing.
//!
//! Phase 13+: the long-term goal of this module is to enable a
//! fully-offline pipeline that reconstructs `SmartPlant` "Publish Data" XML
//! (`*_Data.xml` / `*_Meta.xml`) straight from the backup package,
//! without ever requiring a live SQL Server instance. Stage 0 ships only
//! the MTF envelope layer; subsequent stages will add MDF page parsing,
//! schema recovery, and object-graph reconstruction.

// SQL Server internal page / catalog / stream-container parsers are
// only needed on the `backup-mdf` side (extraction probes and the MDF
// loader pipeline). They stay off `backup-exp` so the Oracle `exp`
// consumer surface names exactly the package-level readers it needs.
#[cfg(feature = "backup-mdf")]
pub mod boot_page;
pub mod datamap;
pub mod folder;
pub mod manifest;
#[cfg(feature = "backup-mdf")]
pub mod mdf_page;
#[cfg(feature = "backup-mdf")]
pub mod msci;
pub mod mtf;
pub mod refdata;
#[cfg(feature = "backup-mdf")]
pub mod syscatalog;
pub mod text_scan;
pub mod zip_index;

#[cfg(feature = "backup-mdf")]
pub use boot_page::{parse_boot_page, BootPageError, BootPageInfo};
pub use datamap::{fluid_system_guid, read_datamap, DataMap, DataMapError};
pub use folder::{
    parse_plant_data_filename, scan_backup_folder, BackupFolderScan, ExportDumpFormat,
    ExportDumpScan, PlantDataFile, RefDataFile, ScanProblem, ZipListing,
};
pub use manifest::{
    parse_manifest, parse_manifest_bytes, Manifest, ManifestLine, ManifestTable, FIELD_SEP,
};
#[cfg(feature = "backup-mdf")]
pub use mdf_page::{MdfPageCursor, MdfPageHeader, PageAddress, PageType, PAGE_SIZE};
#[cfg(feature = "backup-mdf")]
pub use msci::{parse_msci, MsciConfig, MsciError, MsciFile};
pub use mtf::{
    detect_logical_block_size, detect_non_mtf_dump_format, MtfBlock, MtfBlockCursor, MtfBlockType,
    MtfError, MtfHeader, MtfStream, MtfStreamCursor, MtfStreamKind,
};
pub use refdata::{
    classify_format, parse_refdata_filename, scan_refdata_dir, RefDataEntry, RefDataFormat,
};
#[cfg(feature = "backup-mdf")]
pub use syscatalog::{scan_sysschobjs_rows, SysschobjsRow, SYSSCHOBJS_ROW_MARKER};
pub use text_scan::{find_ascii_run_containing, find_utf16le_run_containing};
pub use zip_index::{list_zip_entries, list_zip_entries_from_reader, ZipEntry, ZipIndexError};
