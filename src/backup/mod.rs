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
//! extract the raw MDF bytes for relational-database parsing.
//!
//! Phase 13+: the long-term goal of this module is to enable a
//! fully-offline pipeline that reconstructs `SmartPlant` "Publish Data" XML
//! (`*_Data.xml` / `*_Meta.xml`) straight from the backup package,
//! without ever requiring a live SQL Server instance. Stage 0 ships only
//! the MTF envelope layer; subsequent stages will add MDF page parsing,
//! schema recovery, and object-graph reconstruction.

pub mod boot_page;
pub mod mdf_page;
pub mod msci;
pub mod mtf;
pub mod syscatalog;
pub mod text_scan;

pub use boot_page::{parse_boot_page, BootPageError, BootPageInfo};
pub use mdf_page::{MdfPageCursor, MdfPageHeader, PageAddress, PageType, PAGE_SIZE};
pub use msci::{parse_msci, MsciConfig, MsciError, MsciFile};
pub use mtf::{
    detect_logical_block_size, MtfBlock, MtfBlockCursor, MtfBlockType, MtfError, MtfHeader,
    MtfStream, MtfStreamCursor, MtfStreamKind,
};
pub use syscatalog::{scan_sysschobjs_rows, SysschobjsRow, SYSSCHOBJS_ROW_MARKER};
pub use text_scan::{find_ascii_run_containing, find_utf16le_run_containing};
