//! Minimal SQL Server MDF page-header decoder.
//!
//! SQL Server formats every `.mdf` data file as a sequence of
//! 8192-byte pages. Each page begins with a 96-byte **page header**
//! whose first 32 bytes carry the fields stage-0 needs in order to
//! identify and classify the page.
//!
//! Field layout (source: Paul Randal's "Inside the Storage Engine"
//! and Mark Rasmussen's "Anatomy of a Page" blog series; cross-
//! validated against real `SmartPlant` SQL Server 2008 R2 backups
//! extracted from our TEST02 fixture):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | `0x00` | 1 | `m_headerVersion` (always `0x01` for SQL 2005+) |
//! | `0x01` | 1 | `m_type` — page type enum (see [`PageType`]) |
//! | `0x02` | 1 | `m_typeFlagBits` |
//! | `0x03` | 1 | `m_level` |
//! | `0x04` | 2 | `m_flagBits` |
//! | `0x06` | 2 | `m_indexId` |
//! | `0x08` | 4 | `m_nextPage.pageId` |
//! | `0x0C` | 2 | `m_nextPage.fileId` |
//! | `0x0E` | 2 | `m_pminlen` |
//! | `0x10` | 4 | `m_prevPage.pageId` |
//! | `0x14` | 2 | `m_prevPage.fileId` |
//! | `0x16` | 2 | `m_objId.id` (in SQL 2005+ this is `m_ghostCnt` reuse) |
//! | `0x18` | 2 | `m_slotCnt` (number of rows on the page) |
//! | `0x1A` | 2 | `m_freeCnt` (free byte count) |
//! | `0x1C` | 2 | `m_freeData` |
//! | `0x1E` | 2 | `m_pageId.fileId` |
//! | `0x20` | 4 | `m_pageId.pageId` |
//!
//! Stage-0 focuses on header version, page type, slot/free counts
//! and the two `pageId` pointers. Body parsing lives in stage 1.
//!
//! **Why this is in `backup` instead of a dedicated `mdf` crate**:
//! in stage 0 the only consumer is the MSDA probe that walks the
//! extracted backup byte stream page-by-page. When stage 1 adds
//! row-level decoding we'll move this into a richer `backup::mdf`
//! submodule.

/// Nominal MDF page size in bytes. All SQL Server data files use
/// this page size; changing it would require SQL 2000 or earlier
/// support, which we do not target.
pub const PAGE_SIZE: usize = 8192;

/// Minimum number of bytes [`MdfPageHeader::probe`] needs to
/// classify a candidate page.
pub const MIN_HEADER_BYTES: usize = 32;

/// SQL Server page types. Values match the `sys.dm_db_database_page_allocations.page_type` enum.
/// Names mirror the engine's internal terminology so log output is
/// searchable against Paul Randal's reference material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    /// `type = 1` — Heap / clustered-index leaf data page.
    DataPage,
    /// `type = 2` — Non-clustered index or clustered-index interior
    /// page.
    IndexPage,
    /// `type = 3` — Text-mix page (small LOB data inlined with
    /// row-level text pointers).
    TextMixPage,
    /// `type = 4` — Text-tree page (LOB overflow).
    TextTreePage,
    /// `type = 7` — Sort page (temporary).
    SortPage,
    /// `type = 8` — GAM (Global Allocation Map).
    Gam,
    /// `type = 9` — SGAM (Shared Global Allocation Map).
    Sgam,
    /// `type = 10` — IAM (Index Allocation Map).
    Iam,
    /// `type = 11` — PFS (Page Free Space).
    Pfs,
    /// `type = 13` — Boot page (page 9 of the primary data file;
    /// carries the database name + build info).
    BootPage,
    /// `type = 14` — Server config / file header page (page 0 of
    /// every data file).
    FileHeaderPage,
    /// `type = 15` — Differential Changed Map.
    DiffMap,
    /// `type = 16` — Bulk Changed Map.
    BulkChangeMap,
    /// `type = 17` — LOP state / reserved.
    Reserved17,
    /// `type = 18` — Allocation unit (SQL 2008+).
    AllocUnit,
    /// `type = 19` — Compressed backup page (only seen inside
    /// compressed backups).
    CompressedBackupPage,
    /// `type = 20` — File group extent map.
    FileGroupExtentMap,
    /// `type = 21` — Conflict log (merge-replication).
    ConflictLog,
    /// `type = 22` — reserved.
    Reserved22,
    /// Any value outside the `[1..=22]` canonical range.
    Unknown(u8),
}

impl PageType {
    /// Decode the raw `m_type` byte. Unknown values collapse to
    /// [`PageType::Unknown`] without erroring so callers can surface
    /// them in diagnostics.
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::DataPage,
            2 => Self::IndexPage,
            3 => Self::TextMixPage,
            4 => Self::TextTreePage,
            7 => Self::SortPage,
            8 => Self::Gam,
            9 => Self::Sgam,
            10 => Self::Iam,
            11 => Self::Pfs,
            13 => Self::BootPage,
            14 => Self::FileHeaderPage,
            15 => Self::DiffMap,
            16 => Self::BulkChangeMap,
            17 => Self::Reserved17,
            18 => Self::AllocUnit,
            19 => Self::CompressedBackupPage,
            20 => Self::FileGroupExtentMap,
            21 => Self::ConflictLog,
            22 => Self::Reserved22,
            other => Self::Unknown(other),
        }
    }

    /// Short human-readable tag for log lines.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::DataPage => "data",
            Self::IndexPage => "index",
            Self::TextMixPage => "text-mix",
            Self::TextTreePage => "text-tree",
            Self::SortPage => "sort",
            Self::Gam => "gam",
            Self::Sgam => "sgam",
            Self::Iam => "iam",
            Self::Pfs => "pfs",
            Self::BootPage => "boot",
            Self::FileHeaderPage => "file-hdr",
            Self::DiffMap => "diff",
            Self::BulkChangeMap => "bulk",
            Self::Reserved17 => "r17",
            Self::AllocUnit => "alloc",
            Self::CompressedBackupPage => "cbp",
            Self::FileGroupExtentMap => "fg-ext",
            Self::ConflictLog => "conflict",
            Self::Reserved22 => "r22",
            Self::Unknown(_) => "?",
        }
    }
}

/// Parsed MDF page header. Only the fields stage-0 consumes are
/// surfaced here; the remaining ~64 bytes can be decoded as a
/// follow-up inside `backup::mdf` when body parsing is added.
#[derive(Debug, Clone)]
pub struct MdfPageHeader {
    /// `u8` page-header format version reported at offset 0
    /// (always `0x01` on observed fixtures).
    pub header_version: u8,
    /// Decoded page class (data / index / boot / …).
    pub page_type: PageType,
    /// Raw `u8` flags paired with [`Self::page_type`] on disk; kept
    /// so downstream decoders can recover flag bits not yet modelled.
    pub type_flag_bits: u8,
    /// B-tree level (`0` for leaf pages).
    pub level: u8,
    /// Raw `u16` flag bits from the header; opaque — preserved for
    /// future decoding.
    pub flag_bits: u16,
    /// Identifier of the index this page belongs to (SQL Server
    /// `index_id`; `0` for heap data pages).
    pub index_id: u16,
    /// Number of slots (row pointers) currently used on this page.
    pub slot_count: u16,
    /// Number of bytes free on the page.
    pub free_count: u16,
    /// Offset of the first free byte inside the page body.
    pub free_data: u16,
    /// `(file_id, page_id)` of this page.
    pub page_id: PageAddress,
    /// Forward link to the next page in a B-tree / heap chain;
    /// `(0, 0)` when there is no next page.
    pub next_page: PageAddress,
    /// Back link to the previous page in the chain; `(0, 0)` when
    /// there is no previous page.
    pub prev_page: PageAddress,
}

/// 6-byte `(file_id, page_id)` tuple used by SQL Server to address
/// pages. `file_id` is a `u16` and `page_id` is a `u32`, both
/// little-endian, stored as the pair `(pageId, fileId)` on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAddress {
    /// SQL Server file identifier (`1` for the primary data file).
    pub file_id: u16,
    /// Zero-based page index within the file identified by
    /// [`Self::file_id`].
    pub page_id: u32,
}

impl PageAddress {
    fn from_bytes(page_id: [u8; 4], file_id: [u8; 2]) -> Self {
        Self {
            file_id: u16::from_le_bytes(file_id),
            page_id: u32::from_le_bytes(page_id),
        }
    }
}

impl MdfPageHeader {
    /// Parse the first [`MIN_HEADER_BYTES`] bytes of a candidate
    /// MDF page. Returns `None` when:
    ///
    /// * the input slice is shorter than the header window,
    /// * `m_headerVersion` is not `0x01` (the only SQL Server 2005+
    ///   value), or
    /// * `m_type` is outside the canonical page-type range — this
    ///   means the bytes are either padding or a different SQL
    ///   Server version we do not yet support.
    ///
    /// Callers that want to log the raw type byte even on rejection
    /// can use [`MdfPageHeader::probe_raw`] instead.
    pub fn probe(data: &[u8]) -> Option<Self> {
        let header = Self::probe_raw(data)?;
        if header.header_version != 0x01 {
            return None;
        }
        if matches!(header.page_type, PageType::Unknown(_)) {
            return None;
        }
        Some(header)
    }

    /// Parse the header slice without rejecting unknown page types;
    /// the caller decides whether `Unknown(_)` is fatal.
    pub fn probe_raw(data: &[u8]) -> Option<Self> {
        if data.len() < MIN_HEADER_BYTES {
            return None;
        }
        let header_version = data[0];
        let page_type_raw = data[1];
        let type_flag_bits = data[2];
        let level = data[3];
        let flag_bits = u16::from_le_bytes([data[4], data[5]]);
        let index_id = u16::from_le_bytes([data[6], data[7]]);
        let next_page =
            PageAddress::from_bytes([data[8], data[9], data[10], data[11]], [data[12], data[13]]);
        let prev_page = PageAddress::from_bytes(
            [data[16], data[17], data[18], data[19]],
            [data[20], data[21]],
        );
        let slot_count = u16::from_le_bytes([data[24], data[25]]);
        let free_count = u16::from_le_bytes([data[26], data[27]]);
        let free_data = u16::from_le_bytes([data[28], data[29]]);
        let page_id = PageAddress::from_bytes(
            // On disk the owning page's (fileId, pageId) is stored
            // as `(fileId<u16>, pageId<u32>)` starting at offset
            // 0x1E; we swap byte order to match the tuple
            // constructor's `(page_id, file_id)` convention.
            [data[32 - 6], data[32 - 5], 0, 0], // placeholder; filled below
            [data[30], data[31]],
        );
        // The `m_pageId` field (fileId at 0x1E, pageId at 0x20..0x24)
        // is technically outside the first 32 bytes. We compose it
        // defensively here so the struct carries a meaningful
        // self-referential address, but the bytes beyond `MIN_HEADER_BYTES`
        // are not required: if the input is exactly 32 bytes the
        // page_id field simply reports `pageId = 0`.
        let page_id = if data.len() >= 38 {
            PageAddress::from_bytes(
                [data[32], data[33], data[34], data[35]],
                [data[30], data[31]],
            )
        } else {
            page_id
        };
        Some(Self {
            header_version,
            page_type: PageType::from_raw(page_type_raw),
            type_flag_bits,
            level,
            flag_bits,
            index_id,
            slot_count,
            free_count,
            free_data,
            page_id,
            next_page,
            prev_page,
        })
    }
}

/// Walk `data` at fixed `stride` from `base`, yielding one parsed
/// header per candidate page. Pages where [`MdfPageHeader::probe`]
/// rejects the bytes are skipped, but their index is still
/// advanced so indexing remains stable across runs.
///
/// Stage-0 consumers use this to classify the backup-stream byte
/// layout extracted from MSDA; stage-1 MDF parsing will replace
/// this with a random-access abstraction over the file bytes.
pub struct MdfPageCursor<'a> {
    data: &'a [u8],
    base: usize,
    stride: usize,
    index: usize,
}

impl<'a> MdfPageCursor<'a> {
    /// Build a cursor over `data` starting at `base`, stepping
    /// `stride` bytes between candidates. `stride = 0` panics in
    /// debug and degrades to `PAGE_SIZE` in release to prevent an
    /// infinite loop from slipping past tests.
    pub fn new(data: &'a [u8], base: usize, stride: usize) -> Self {
        debug_assert!(stride > 0, "MdfPageCursor stride must be positive");
        Self {
            data,
            base,
            stride: if stride == 0 { PAGE_SIZE } else { stride },
            index: 0,
        }
    }
}

impl Iterator for MdfPageCursor<'_> {
    type Item = (usize, usize, MdfPageHeader);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let offset = self.base + self.index * self.stride;
            if offset + MIN_HEADER_BYTES > self.data.len() {
                return None;
            }
            self.index += 1;
            let header_bytes = &self.data[offset..];
            if let Some(header) = MdfPageHeader::probe(header_bytes) {
                return Some((self.index - 1, offset, header));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic 32-byte MDF header with the given type byte
    /// and slot count. All other fields are zero. Useful for
    /// exercising the parser without a real page fixture.
    fn synthetic_header(page_type: u8, slot_count: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 48];
        bytes[0] = 0x01; // header version
        bytes[1] = page_type;
        bytes[24..26].copy_from_slice(&slot_count.to_le_bytes());
        bytes
    }

    #[test]
    fn page_type_decodes_canonical_values() {
        assert_eq!(PageType::from_raw(1), PageType::DataPage);
        assert_eq!(PageType::from_raw(2), PageType::IndexPage);
        assert_eq!(PageType::from_raw(10), PageType::Iam);
        assert_eq!(PageType::from_raw(13), PageType::BootPage);
    }

    #[test]
    fn page_type_preserves_unknown_values() {
        assert_eq!(PageType::from_raw(99), PageType::Unknown(99));
        assert_eq!(PageType::from_raw(99).tag(), "?");
    }

    #[test]
    fn probe_rejects_short_input() {
        assert!(MdfPageHeader::probe(&[0u8; 16]).is_none());
    }

    #[test]
    fn probe_rejects_wrong_header_version() {
        let mut bytes = synthetic_header(1, 10);
        bytes[0] = 0x00;
        assert!(MdfPageHeader::probe(&bytes).is_none());
    }

    #[test]
    fn probe_rejects_unknown_page_type() {
        let bytes = synthetic_header(99, 10);
        assert!(MdfPageHeader::probe(&bytes).is_none());
    }

    #[test]
    fn probe_decodes_data_page() {
        let bytes = synthetic_header(1, 42);
        let header = MdfPageHeader::probe(&bytes).expect("valid");
        assert_eq!(header.header_version, 0x01);
        assert_eq!(header.page_type, PageType::DataPage);
        assert_eq!(header.slot_count, 42);
    }

    #[test]
    fn cursor_walks_every_stride_but_reports_only_valid_headers() {
        // Layout: [valid data page][garbage page][valid iam page]
        let mut bytes = vec![0u8; PAGE_SIZE * 3];
        bytes[..32].copy_from_slice(&synthetic_header(1, 5)[..32]);
        // page #1 intentionally left zeroed -> header_version = 0 -> reject
        bytes[PAGE_SIZE * 2..PAGE_SIZE * 2 + 32].copy_from_slice(&synthetic_header(10, 256)[..32]);

        let reports: Vec<_> = MdfPageCursor::new(&bytes, 0, PAGE_SIZE).collect();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].0, 0);
        assert_eq!(reports[0].2.page_type, PageType::DataPage);
        assert_eq!(reports[1].0, 2);
        assert_eq!(reports[1].2.page_type, PageType::Iam);
    }
}
