//! Parsers for `SmartPlant` / Smart P&ID backup packages (`*.pid` database
//! backups produced by the `SmartPlant` desktop tool).
//!
//! A `SmartPlant` backup package is a folder containing:
//!
//! * `Export.dmp` — a **Microsoft Tape Format (MTF)** envelope around one
//!   or more SQL Server Master Data Files (MDF/LDF). This is the canonical
//!   object / relationship / attribute store. See [`mtf`].
//! * `Manifest.txt` — a plain text `key<<|>>value` listing describing the
//!   plant, the databases, and per-file metadata.
//! * `PlantData~2~*.zip` — per-plant drawing cache (includes the CFB
//!   `.pid` drawing files). Parsed by [`crate::package`] /
//!   [`crate::cfb`].
//! * `RefData~4~*.zip` — reference data: symbol libraries, templates,
//!   report files, material specs, etc.
//! * `PlantConfig.xml` — drawing style configuration.
//!
//! The [`mtf`] submodule parses the MTF envelope so downstream stages can
//! extract the raw MDF bytes for relational-database parsing. The [`store`]
//! submodule turns a whole backup -- the `<Plant>_p.zip` or its directory --
//! into one SQLite file, the Backup Store (ADR-0004).
//!
//! Phase 13+: the long-term goal of this module is to enable a
//! fully-offline pipeline that reconstructs `SmartPlant` "Publish Data" XML
//! (`*_Data.xml` / `*_Meta.xml`) straight from the backup package,
//! without ever requiring a live SQL Server instance. Stage 0 ships only
//! the MTF envelope layer; subsequent stages will add MDF page parsing,
//! schema recovery, and object-graph reconstruction.

pub mod boot_page;
pub mod manifest;
pub mod mdf_page;
pub mod msci;
pub mod mtf;
pub mod oracle_exp;
pub mod refdata;
pub mod store;
pub mod syscatalog;
pub mod text_scan;
pub mod zip_index;

pub use boot_page::{parse_boot_page, BootPageError, BootPageInfo};
pub use manifest::{
    parse_manifest, parse_manifest_bytes, Manifest, ManifestLine, ManifestTable, FIELD_SEP,
};
pub use mdf_page::{MdfPageCursor, MdfPageHeader, PageAddress, PageType, PAGE_SIZE};
pub use msci::{parse_msci, MsciConfig, MsciError, MsciFile};
pub use mtf::{
    detect_backup_stream_header_len, detect_logical_block_size, detect_non_mtf_dump_format,
    locate_sql_server_streams, mdf_bytes_of_dump, MtfBlock, MtfBlockCursor, MtfBlockType, MtfError,
    MtfHeader, MtfStream, MtfStreamCursor, MtfStreamKind, SqlServerDumpError, SqlServerStreams,
};
pub use oracle_exp::{is_exp_dump, scan_create_tables, ExpColumn, ExpDdlError, ExpTable};
pub use refdata::{
    classify_format, parse_refdata_filename, scan_refdata_dir, RefDataEntry, RefDataFormat,
};
pub use store::{
    build_backup_store, build_backup_store_in_memory, BackupInput, BackupInputKind,
    BackupStoreError, StoreOptions, StoreSummary,
};
pub use syscatalog::{scan_sysschobjs_rows, SysschobjsRow, SYSSCHOBJS_ROW_MARKER};
pub use text_scan::{find_ascii_run_containing, find_utf16le_run_containing};
pub use zip_index::{
    decode_zip_entry_name, list_zip_entries, list_zip_entries_from_reader, ZipEntry, ZipIndexError,
    ZipNameEncoding,
};
