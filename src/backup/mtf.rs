//! Microsoft Tape Format (MTF) envelope parser.
//!
//! SmartPlant's `Export.dmp` wraps the raw SQL Server database backup
//! pages in MTF blocks. This module reads those blocks without trying
//! to understand the inner database payload.
//!
//! # Format summary
//!
//! MTF is a block-structured, little-endian container defined by
//! Microsoft in the `[MS-TAPE]` Open Specification. Data is organized
//! in fixed-size **logical blocks** (commonly 1024 or 65536 bytes).
//! Each logical block either starts a new **descriptor block** or
//! continues the payload of the previous descriptor.
//!
//! Every descriptor block begins with a 52-byte **Common Block Header**
//! (a.k.a. DBLK header). The first four bytes of that header are an
//! ASCII tag identifying the descriptor kind — `TAPE`, `SSET`, `VOLB`,
//! `DBDB`, `ESET`, `EOTM`, `SFMB`, etc.
//!
//! ## Why block size is not a header field
//!
//! `[MS-TAPE]` does NOT store the logical block size inside the `TAPE`
//! descriptor itself — the value is a property of the external storage
//! medium (tape drive or file). For BAK-on-disk fixtures we recover
//! it empirically in [`detect_logical_block_size`]: descriptor tags
//! are guaranteed to land on a **512-byte-aligned** grid, so the
//! offset of the *second* descriptor block equals the TAPE block's
//! byte length.
//!
//! Stage 0 exercises:
//!
//! 1. [`MtfHeader::probe`] reads the 52-byte common header plus the
//!    already-understood TAPE-specific fields (Media Family ID etc.).
//! 2. [`detect_logical_block_size`] infers the block stride from the
//!    position of the next descriptor tag.
//! 3. [`MtfBlockType`] enumerates known tags; unknown tags are
//!    preserved verbatim so higher layers can surface them without
//!    abort.
//!
//! Later stages will build on these with [`MtfBlockCursor`] (full walk
//! of every descriptor) and SQL-specific `MQDA` / `DBDB` parsers.
//!
//! # References
//!
//! * `[MS-TAPE]` — Microsoft Tape Format (MTF), Open Specifications.
//! * Mark Rasmussen, "The Anatomy of an MTF Backup File", blog series
//!   at `improve.dk`. Used for cross-checking field offsets against
//!   real SQL Server 2008 R2 backups.

use thiserror::Error;

/// Minimum size of the MTF Common Block Header (DBLK header). Every
/// descriptor block starts with this 52-byte preamble.
pub const COMMON_BLOCK_HEADER_LEN: usize = 52;

/// Stride of the byte grid on which MTF descriptor tags are guaranteed
/// to land. `[MS-TAPE]` 3.2 defines 512 bytes as the minimum aligned
/// physical block size; BAK-on-disk fixtures always respect this, even
/// when the *logical* block size is 1024 or 65536 bytes.
pub const DESCRIPTOR_GRID_STEP: usize = 512;

/// Upper bound (bytes) for the [`detect_logical_block_size`] scan. A
/// single TAPE block never exceeds this in practice — SQL Server 2008
/// R2 uses 1024; SQL Server 2016+ uses 65536 — so 256 KiB gives ample
/// slack without chewing through a 20 MB input if the file is
/// corrupted and no second descriptor is ever found.
const BLOCK_SIZE_SCAN_LIMIT: usize = 256 * 1024;

/// ASCII tag that identifies an MTF descriptor block kind. The four
/// bytes live at offset 0 of every descriptor block and also double as
/// the block's "magic number".
///
/// Unknown tags are captured via [`MtfBlockType::Unknown`] so the
/// descriptor walker can surface them instead of bailing out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtfBlockType {
    /// `TAPE` — Tape Descriptor. Always the first descriptor block.
    Tape,
    /// `SSET` — Start of Set Descriptor (begins a backup set).
    StartOfSet,
    /// `VOLB` — Volume Descriptor.
    Volume,
    /// `DBDB` — Database Backup Descriptor. SQL Server marker for the
    /// MDF/LDF byte stream.
    DatabaseBackup,
    /// `MQDA` — MQDA (SQL Server Metadata Descriptor, version 2+).
    /// Often appears alongside `DBDB` in SQL Server 2005+ backups.
    SqlMetadata,
    /// `ESET` — End of Set Descriptor (closes a backup set).
    EndOfSet,
    /// `EOTM` — End of Tape Marker.
    EndOfTape,
    /// `SFMB` — Soft Filemark Block. Short half-block that sits
    /// between logical segments on tape-style streams.
    SoftFilemark,
    /// Any four-byte tag we do not yet know how to decode. The raw
    /// bytes are preserved so higher layers can log / probe.
    Unknown([u8; 4]),
}

impl MtfBlockType {
    /// Interpret a 4-byte slice (read directly from the start of a
    /// descriptor block) as an [`MtfBlockType`]. Unknown patterns are
    /// returned as [`MtfBlockType::Unknown`] rather than erroring so
    /// the walk can continue past unfamiliar payloads.
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        match &bytes {
            b"TAPE" => Self::Tape,
            b"SSET" => Self::StartOfSet,
            b"VOLB" => Self::Volume,
            b"DBDB" => Self::DatabaseBackup,
            b"MQDA" => Self::SqlMetadata,
            b"ESET" => Self::EndOfSet,
            b"EOTM" => Self::EndOfTape,
            b"SFMB" => Self::SoftFilemark,
            _ => Self::Unknown(bytes),
        }
    }

    /// `true` when this tag is one of the enumerated MTF descriptor
    /// kinds we recognize. Used by the block-size detector to decide
    /// whether a candidate offset is a real descriptor boundary or
    /// just random bytes that happen to be ASCII.
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// Render the four-byte tag as a Rust string for diagnostics. Falls
    /// back to `"????"` if the bytes do not form printable ASCII.
    pub fn tag(&self) -> String {
        let bytes = match self {
            Self::Tape => *b"TAPE",
            Self::StartOfSet => *b"SSET",
            Self::Volume => *b"VOLB",
            Self::DatabaseBackup => *b"DBDB",
            Self::SqlMetadata => *b"MQDA",
            Self::EndOfSet => *b"ESET",
            Self::EndOfTape => *b"EOTM",
            Self::SoftFilemark => *b"SFMB",
            Self::Unknown(b) => *b,
        };
        if bytes.iter().all(|&c| (0x20..=0x7E).contains(&c)) {
            String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| "????".into())
        } else {
            "????".into()
        }
    }
}

/// Errors returned by the MTF layer. Bubble up with `thiserror` so
/// callers can distinguish "truncated input" from "unexpected tag".
#[derive(Debug, Error)]
pub enum MtfError {
    /// Input is shorter than the 52-byte Common Block Header, so we
    /// cannot even identify the first descriptor.
    #[error(
        "MTF input too short: need at least {needed} bytes for the common block header, got {got}"
    )]
    TooShort { needed: usize, got: usize },

    /// First descriptor block is not a `TAPE` descriptor. MTF always
    /// starts with `TAPE`, so anything else means the file is not an
    /// MTF stream (maybe already extracted MDF, maybe a different
    /// backup format).
    #[error("MTF input does not start with a TAPE descriptor (got tag `{got}`)")]
    NotATapeStart { got: String },
}

/// Parsed `TAPE` descriptor header — the very first block of every
/// MTF stream. Decoded fields cover the Common Block Header
/// (offsets 0..52) plus selected fields from the TAPE-specific body
/// that downstream code actually needs.
///
/// The block size is **not** a field on this struct: MTF keeps it
/// outside the descriptor, so use [`detect_logical_block_size`] on
/// the full input when you need the stride.
#[derive(Debug, Clone)]
pub struct MtfHeader {
    /// Always [`MtfBlockType::Tape`] when `probe` succeeds; kept as a
    /// field so callers do not special-case header vs non-header
    /// descriptors when building tables / logs.
    pub block_type: MtfBlockType,

    /// Raw 32-bit attribute bitfield (flags defined in `[MS-TAPE]`).
    /// Surfaced opaquely; individual flags can be cracked open in
    /// later stages if needed.
    pub attributes: u32,

    /// Byte offset (within the descriptor block itself) to the first
    /// event / variable-length field. Used to skip past the fixed
    /// portion of the header without hard-coding every field's size.
    pub offset_to_first_event: u16,

    /// Operating system identifier that wrote the tape. Informational.
    pub os_id: u16,

    /// OS version that wrote the tape. Informational.
    pub os_version: u16,

    /// `DisplayableSize` from the common header — the human-readable
    /// total size of the descriptor's payload in bytes.
    pub displayable_size: u64,

    /// `FormatLogicalAddress` — tape/file address at which this
    /// descriptor lives. Useful for cross-referencing against the
    /// containing medium.
    pub format_logical_address: u64,

    /// Raw bytes of the 52-byte Common Block Header — kept for
    /// debugging / hex dumping. Callers should never parse it by
    /// hand; it is provided so probe tools can echo the exact bytes
    /// when diagnosing format anomalies.
    pub raw_common_header: [u8; COMMON_BLOCK_HEADER_LEN],
}

impl MtfHeader {
    /// Parse the first [`COMMON_BLOCK_HEADER_LEN`] bytes of an MTF
    /// stream as a TAPE descriptor header.
    ///
    /// Errors cover the two "fatal at the start" cases: truncated
    /// input and wrong magic. A valid TAPE header is not enough on
    /// its own to walk the rest of the stream — callers should also
    /// invoke [`detect_logical_block_size`] to discover the block
    /// stride.
    pub fn probe(data: &[u8]) -> Result<Self, MtfError> {
        if data.len() < COMMON_BLOCK_HEADER_LEN {
            return Err(MtfError::TooShort {
                needed: COMMON_BLOCK_HEADER_LEN,
                got: data.len(),
            });
        }

        let block_type = MtfBlockType::from_bytes([data[0], data[1], data[2], data[3]]);
        if block_type != MtfBlockType::Tape {
            return Err(MtfError::NotATapeStart {
                got: block_type.tag(),
            });
        }

        let attributes = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let offset_to_first_event = u16::from_le_bytes([data[8], data[9]]);
        let os_id = u16::from_le_bytes([data[10], data[11]]);
        let os_version = u16::from_le_bytes([data[12], data[13]]);
        let displayable_size = u64::from_le_bytes([
            data[14], data[15], data[16], data[17], data[18], data[19], data[20], data[21],
        ]);
        let format_logical_address = u64::from_le_bytes([
            data[22], data[23], data[24], data[25], data[26], data[27], data[28], data[29],
        ]);

        let mut raw_common_header = [0u8; COMMON_BLOCK_HEADER_LEN];
        raw_common_header.copy_from_slice(&data[..COMMON_BLOCK_HEADER_LEN]);

        Ok(Self {
            block_type,
            attributes,
            offset_to_first_event,
            os_id,
            os_version,
            displayable_size,
            format_logical_address,
            raw_common_header,
        })
    }
}

/// Infer the stride between descriptor blocks by scanning forward
/// from offset `DESCRIPTOR_GRID_STEP` in `DESCRIPTOR_GRID_STEP`-byte
/// increments, looking for the first position that begins with a
/// known MTF tag. That offset equals the size of the initial TAPE
/// descriptor, which for every SQL Server backup we have observed is
/// also the stream's logical block size.
///
/// Returns `None` if no recognizable descriptor is found within
/// [`BLOCK_SIZE_SCAN_LIMIT`] bytes — in which case the file is either
/// truncated, corrupt, or a format we do not yet support.
///
/// # Rationale
///
/// `[MS-TAPE]` does not expose the block size inside the TAPE header
/// itself, because MTF traditionally targeted physical tape drives
/// whose block size was decided by the drive hardware. On disk-based
/// BAK files we recover it empirically from the position of the
/// second descriptor tag.
///
/// ## Why a 512-byte grid?
///
/// `[MS-TAPE]` 3.2 specifies 512 bytes as the minimum physical block
/// alignment. Even "short" descriptors like SFMB, which actually
/// carry less than 512 bytes of payload, are padded out to this grid
/// so subsequent descriptors remain aligned. The scan step matches
/// this guarantee.
pub fn detect_logical_block_size(data: &[u8]) -> Option<u32> {
    find_next_descriptor(data, DESCRIPTOR_GRID_STEP)
        .filter(|offset| *offset <= BLOCK_SIZE_SCAN_LIMIT)
        .map(|offset| offset as u32)
}

/// Locate the next 512-byte-aligned offset at-or-after `start` whose
/// first four bytes form a known MTF tag. Shared by both the
/// block-size detector and the full cursor so both agree on what
/// counts as a descriptor boundary.
///
/// The `start` argument is rounded up to the nearest multiple of
/// [`DESCRIPTOR_GRID_STEP`] before scanning — callers can therefore
/// pass any byte offset without aligning it first.
fn find_next_descriptor(data: &[u8], start: usize) -> Option<usize> {
    let mut offset = align_up(start.max(DESCRIPTOR_GRID_STEP), DESCRIPTOR_GRID_STEP);
    while offset + 4 <= data.len() {
        let tag = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];
        if MtfBlockType::from_bytes(tag).is_known() {
            return Some(offset);
        }
        offset += DESCRIPTOR_GRID_STEP;
    }
    None
}

/// Round `value` up to the nearest multiple of `alignment`. Kept as a
/// free helper so tests can assert on the rounding edge cases
/// (already-aligned values, zero) without a full descriptor fixture.
fn align_up(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

/// Summary of one descriptor block as seen by the cursor. `offset`
/// and `size` are byte ranges into the original MTF buffer; `size`
/// spans from this descriptor's tag up to the next descriptor's tag
/// (or end of file for the final block).
///
/// Stage 0 exposes just enough information for a probe / extraction
/// tool; richer per-kind decoding (`DBDB` stream pointers etc.) lives
/// in higher-level modules.
#[derive(Debug, Clone)]
pub struct MtfBlock {
    /// Decoded ASCII tag that begins the descriptor.
    pub block_type: MtfBlockType,

    /// Byte offset of the descriptor's first byte inside the MTF
    /// buffer the cursor was given. Always a multiple of
    /// [`DESCRIPTOR_GRID_STEP`].
    pub offset: usize,

    /// Distance (in bytes) from this descriptor's tag to the next
    /// descriptor's tag. For the final descriptor it runs to the end
    /// of the input buffer.
    pub size: usize,

    /// Raw 52-byte Common Block Header copy (zero-padded on the right
    /// if the block is shorter than 52 bytes, which can legitimately
    /// happen for end markers).
    pub raw_common_header: [u8; COMMON_BLOCK_HEADER_LEN],
}

/// Iterator that walks an MTF stream descriptor-by-descriptor. Each
/// `.next()` returns one [`MtfBlock`]; the walk stops on the first
/// position where no known tag can be found on the 512-byte grid.
///
/// The cursor is intentionally minimal — it does **not** validate
/// the stream (e.g. "SSET must follow TAPE"), because real SQL Server
/// traces interleave `SFMB` half-blocks between otherwise-standard
/// descriptors and the simpler "scan forward" strategy handles that
/// variability uniformly.
pub struct MtfBlockCursor<'a> {
    data: &'a [u8],
    position: usize,
    /// Turns into `true` once an `EOTM`-class descriptor has been
    /// returned; the next `.next()` call will then yield `None` even
    /// if bytes remain. Guards against chasing false tags in the
    /// post-end padding.
    stopped: bool,
}

impl<'a> MtfBlockCursor<'a> {
    /// Build a new cursor positioned at the start of `data`. The
    /// first `.next()` call will yield the `TAPE` descriptor if the
    /// input is well-formed.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            stopped: false,
        }
    }
}

impl Iterator for MtfBlockCursor<'_> {
    type Item = MtfBlock;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        if self.position + 4 > self.data.len() {
            return None;
        }
        let tag = [
            self.data[self.position],
            self.data[self.position + 1],
            self.data[self.position + 2],
            self.data[self.position + 3],
        ];
        let block_type = MtfBlockType::from_bytes(tag);
        // Unknown tags at the cursor position mean we've desynced
        // (the previous block-size guess was wrong or the stream is
        // corrupt). Stop cleanly instead of emitting garbage.
        if !block_type.is_known() {
            return None;
        }

        let next_offset = find_next_descriptor(self.data, self.position + DESCRIPTOR_GRID_STEP)
            .unwrap_or(self.data.len());
        let size = next_offset - self.position;

        let mut raw_common_header = [0u8; COMMON_BLOCK_HEADER_LEN];
        let copy_len = COMMON_BLOCK_HEADER_LEN.min(size);
        raw_common_header[..copy_len]
            .copy_from_slice(&self.data[self.position..self.position + copy_len]);

        let block = MtfBlock {
            block_type,
            offset: self.position,
            size,
            raw_common_header,
        };

        // EOTM marks end-of-tape — there's no guarantee tags after it
        // are real. Freezing the cursor here keeps output clean.
        if matches!(block_type, MtfBlockType::EndOfTape) {
            self.stopped = true;
        }
        self.position = next_offset;
        Some(block)
    }
}

/// Size in bytes of the fixed prefix of a stream header inside a
/// DBLK. The four-byte tag plus the immediately-following attribute
/// and length fields cover 16 bytes, which is enough to identify the
/// stream kind and classify it. Stage-0 tolerates the fact that the
/// full header layout beyond these 16 bytes is SQL-Server-specific.
pub const STREAM_HEADER_LEN: usize = 16;

/// Alignment in bytes that separates consecutive stream headers
/// inside one DBLK. Real SQL Server 2008 R2 backups observed in this
/// fixture interleave stream bodies so that each new stream tag
/// lands on a 4-byte boundary; callers should therefore scan on that
/// stride. [`MtfStreamCursor`] does this internally.
pub const STREAM_TAG_ALIGNMENT: usize = 4;

/// ASCII tag that identifies a stream embedded in a descriptor
/// block. Streams carry the actual payload (SQL backup pages,
/// configuration info, timestamps, padding) while descriptor blocks
/// are primarily structural.
///
/// Unknown tags are preserved verbatim so higher layers can probe /
/// log without tripping the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtfStreamKind {
    /// `SPAD` — Stream padding, used between real streams to meet
    /// MTF's physical-block alignment requirements.
    Pad,
    /// `MSDA` — Microsoft SQL Server Data. Body holds the compressed
    /// / extent-sparse representation of a database's data file
    /// (.mdf). This is the prize stream for the offline backup path.
    SqlData,
    /// `MSDB` — Microsoft SQL Server Database-file metadata. Rare in
    /// practice; may accompany multi-file databases.
    SqlDatabase,
    /// `MSCI` — Microsoft SQL Configuration Information. Carries the
    /// backup set metadata (database name, recovery model, logical /
    /// physical filenames, page counts).
    SqlConfig,
    /// `MSTR` — ANSI string-table stream (very rare).
    AnsiString,
    /// `TSMP` — Timestamp stream. Carries backup-set start / end
    /// timestamps.
    Timestamp,
    /// `STAN` / `STUN` — generic ANSI / Unicode string streams used by
    /// descriptor-specific variable-length payloads.
    Generic([u8; 4]),
    /// Anything else — the raw bytes are kept so logs can echo them.
    Unknown([u8; 4]),
}

impl MtfStreamKind {
    /// Classify a 4-byte ASCII tag read from a stream header.
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        match &bytes {
            b"SPAD" => Self::Pad,
            b"MSDA" => Self::SqlData,
            b"MSDB" => Self::SqlDatabase,
            b"MSCI" => Self::SqlConfig,
            b"MSTR" => Self::AnsiString,
            b"TSMP" => Self::Timestamp,
            b"STAN" | b"STUN" => Self::Generic(bytes),
            _ => Self::Unknown(bytes),
        }
    }

    /// Render the tag for log / diagnostic output; falls back to
    /// `"????"` for non-printable bytes.
    pub fn tag(&self) -> String {
        let bytes = match self {
            Self::Pad => *b"SPAD",
            Self::SqlData => *b"MSDA",
            Self::SqlDatabase => *b"MSDB",
            Self::SqlConfig => *b"MSCI",
            Self::AnsiString => *b"MSTR",
            Self::Timestamp => *b"TSMP",
            Self::Generic(b) | Self::Unknown(b) => *b,
        };
        if bytes.iter().all(|&c| (0x20..=0x7E).contains(&c)) {
            String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| "????".into())
        } else {
            "????".into()
        }
    }
}

/// One stream header decoded from the start of a stream slot inside a
/// DBLK. Stage-0 treats the header as opaque past the first 16 bytes
/// — SQL Server embeds its own sub-format in that region, and we do
/// not yet decode it.
///
/// `body_offset` / `body_end` give the exact byte range of the
/// payload inside the original MTF buffer. The body end is
/// determined empirically by scanning forward for the next
/// recognized stream tag (or the DBLK boundary, whichever comes
/// first), because the declared body-length word in SQL Server
/// backups does not consistently represent this stream's size.
///
/// Callers extracting a stream's bytes slice the buffer with
/// `&data[body_offset..body_end]`.
#[derive(Debug, Clone)]
pub struct MtfStream {
    pub kind: MtfStreamKind,
    /// Raw attribute word from stream header bytes 4..8. Kept as a
    /// `u32` because SQL Server backups combine the `FSYS_ATTR` and
    /// `MFS_ATTR` pair into a single 32-bit field.
    pub raw_attributes: u32,
    /// Declared length as read from stream header bytes 8..16. Kept
    /// verbatim: its semantics differ by stream kind (SPAD stores
    /// the padding size, MSCI/MSDA store what seems to be a
    /// backup-set-wide value). Callers needing a trustworthy size
    /// should use `body_end - body_offset` instead.
    pub declared_length: u64,
    /// Absolute offset of the stream header's first byte in the
    /// original MTF buffer.
    pub header_offset: usize,
    /// Absolute offset of the stream body's first byte (right after
    /// the 16-byte fixed header prefix).
    pub body_offset: usize,
    /// Absolute offset just past the stream body, located by
    /// scanning ahead for the next stream tag or DBLK boundary.
    pub body_end: usize,
}

impl MtfStream {
    /// Body size in bytes, derived from `body_end - body_offset`.
    pub fn body_len(&self) -> usize {
        self.body_end - self.body_offset
    }
}

/// Iterator that walks the streams embedded in one descriptor block.
/// The caller supplies the `(body_start, body_end)` byte range that
/// the containing DBLK's payload occupies; the cursor hops from one
/// stream header to the next by scanning the 4-byte tag grid for
/// the next recognized stream kind.
///
/// This scan-based strategy handles the SQL-Server-specific stream
/// layout encountered in real backups (where the declared-length
/// field does not describe the body size) without needing a full
/// per-kind decoder in stage 0.
pub struct MtfStreamCursor<'a> {
    data: &'a [u8],
    position: usize,
    end: usize,
}

impl<'a> MtfStreamCursor<'a> {
    /// Build a cursor that walks streams inside `data[start..end]`.
    /// Used by higher layers that first located a DBLK and now want
    /// to iterate its internal payload.
    pub fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        Self {
            data,
            position: start.min(data.len()),
            end: end.min(data.len()),
        }
    }
}

impl Iterator for MtfStreamCursor<'_> {
    type Item = MtfStream;

    fn next(&mut self) -> Option<Self::Item> {
        // Align the starting scan offset: callers can pass any
        // in-block position, but stream tags only land on a 4-byte
        // grid.
        self.position = align_up(self.position, STREAM_TAG_ALIGNMENT);
        let header_offset = self.find_next_stream_tag(self.position)?;
        if header_offset + STREAM_HEADER_LEN > self.end {
            return None;
        }

        let tag = [
            self.data[header_offset],
            self.data[header_offset + 1],
            self.data[header_offset + 2],
            self.data[header_offset + 3],
        ];
        let raw_attributes = u32::from_le_bytes([
            self.data[header_offset + 4],
            self.data[header_offset + 5],
            self.data[header_offset + 6],
            self.data[header_offset + 7],
        ]);
        let declared_length = u64::from_le_bytes([
            self.data[header_offset + 8],
            self.data[header_offset + 9],
            self.data[header_offset + 10],
            self.data[header_offset + 11],
            self.data[header_offset + 12],
            self.data[header_offset + 13],
            self.data[header_offset + 14],
            self.data[header_offset + 15],
        ]);

        let body_offset = header_offset + STREAM_HEADER_LEN;
        // Scan forward on the 4-byte grid for the next recognized
        // stream tag; the gap gives this stream's body end.
        let body_end = self.find_next_stream_tag(body_offset).unwrap_or(self.end);

        let stream = MtfStream {
            kind: MtfStreamKind::from_bytes(tag),
            raw_attributes,
            declared_length,
            header_offset,
            body_offset,
            body_end,
        };
        self.position = body_end;
        Some(stream)
    }
}

impl MtfStreamCursor<'_> {
    /// Scan forward on the 4-byte grid starting at `start` and
    /// return the first offset whose four bytes match one of the
    /// known stream kinds. Returns `None` when the scan reaches the
    /// cursor's upper bound without a hit.
    fn find_next_stream_tag(&self, start: usize) -> Option<usize> {
        let mut offset = align_up(start, STREAM_TAG_ALIGNMENT);
        while offset + 4 <= self.end {
            let tag = [
                self.data[offset],
                self.data[offset + 1],
                self.data[offset + 2],
                self.data[offset + 3],
            ];
            if matches!(
                MtfStreamKind::from_bytes(tag),
                MtfStreamKind::Pad
                    | MtfStreamKind::SqlData
                    | MtfStreamKind::SqlDatabase
                    | MtfStreamKind::SqlConfig
                    | MtfStreamKind::AnsiString
                    | MtfStreamKind::Timestamp
                    | MtfStreamKind::Generic(_)
            ) {
                return Some(offset);
            }
            offset += STREAM_TAG_ALIGNMENT;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal synthetic TAPE descriptor block. Only the
    /// fields the probe actually reads are meaningful; everything
    /// else is zeroed. Keeps tests self-contained (no dependence on a
    /// real `Export.dmp` fixture).
    fn synthetic_common_header() -> Vec<u8> {
        let mut bytes = vec![0u8; COMMON_BLOCK_HEADER_LEN];
        bytes[0..4].copy_from_slice(b"TAPE");
        // attributes = 0x0003_0000 (arbitrary; mirrors real sample)
        bytes[4..8].copy_from_slice(&0x0003_0000u32.to_le_bytes());
        // offset_to_first_event = 0x008C (arbitrary; mirrors real sample)
        bytes[8..10].copy_from_slice(&0x008Cu16.to_le_bytes());
        // os_id = 0x010E (Windows NT family in real samples)
        bytes[10..12].copy_from_slice(&0x010Eu16.to_le_bytes());
        // displayable_size = 0xABCD (arbitrary)
        bytes[14..22].copy_from_slice(&0xABCDu64.to_le_bytes());
        // format_logical_address = 0 (first descriptor)
        bytes[22..30].copy_from_slice(&0u64.to_le_bytes());
        bytes
    }

    /// Extend a synthetic TAPE header with padding + a second known
    /// descriptor at a specified offset. Used to drive
    /// [`detect_logical_block_size`] tests without needing the real
    /// fixture.
    fn synthetic_stream_with_second_tag_at(offset: usize, tag: &[u8; 4]) -> Vec<u8> {
        let mut bytes = synthetic_common_header();
        bytes.resize(offset + 4, 0);
        bytes[offset..offset + 4].copy_from_slice(tag);
        bytes
    }

    #[test]
    fn block_type_known_tags_decode() {
        assert_eq!(MtfBlockType::from_bytes(*b"TAPE"), MtfBlockType::Tape);
        assert_eq!(MtfBlockType::from_bytes(*b"SSET"), MtfBlockType::StartOfSet);
        assert_eq!(
            MtfBlockType::from_bytes(*b"DBDB"),
            MtfBlockType::DatabaseBackup
        );
        assert_eq!(
            MtfBlockType::from_bytes(*b"MQDA"),
            MtfBlockType::SqlMetadata
        );
        assert_eq!(MtfBlockType::from_bytes(*b"ESET"), MtfBlockType::EndOfSet);
        assert_eq!(
            MtfBlockType::from_bytes(*b"SFMB"),
            MtfBlockType::SoftFilemark
        );
    }

    #[test]
    fn block_type_unknown_tag_preserves_raw_bytes() {
        let got = MtfBlockType::from_bytes(*b"ZZZZ");
        assert_eq!(got, MtfBlockType::Unknown(*b"ZZZZ"));
        assert_eq!(got.tag(), "ZZZZ");
        assert!(!got.is_known());
    }

    #[test]
    fn block_type_non_ascii_unknown_renders_question_marks() {
        // Control bytes should not leak into log output as garbage.
        let got = MtfBlockType::from_bytes([0x00, 0x01, 0xFE, 0xFF]);
        assert_eq!(got.tag(), "????");
    }

    #[test]
    fn probe_rejects_short_input() {
        let err = MtfHeader::probe(&[0u8; 10]).unwrap_err();
        match err {
            MtfError::TooShort { needed, got } => {
                assert_eq!(needed, COMMON_BLOCK_HEADER_LEN);
                assert_eq!(got, 10);
            }
            other => panic!("expected TooShort, got {other:?}"),
        }
    }

    #[test]
    fn probe_rejects_non_tape_magic() {
        let mut bytes = synthetic_common_header();
        bytes[0..4].copy_from_slice(b"ESET");
        let err = MtfHeader::probe(&bytes).unwrap_err();
        match err {
            MtfError::NotATapeStart { got } => assert_eq!(got, "ESET"),
            other => panic!("expected NotATapeStart, got {other:?}"),
        }
    }

    #[test]
    fn probe_decodes_synthetic_tape_header() {
        let bytes = synthetic_common_header();
        let hdr = MtfHeader::probe(&bytes).expect("synthetic TAPE header should parse");
        assert_eq!(hdr.block_type, MtfBlockType::Tape);
        assert_eq!(hdr.attributes, 0x0003_0000);
        assert_eq!(hdr.offset_to_first_event, 0x008C);
        assert_eq!(hdr.os_id, 0x010E);
        assert_eq!(hdr.os_version, 0);
        assert_eq!(hdr.displayable_size, 0xABCD);
        assert_eq!(hdr.format_logical_address, 0);
        assert_eq!(&hdr.raw_common_header[0..4], b"TAPE");
    }

    #[test]
    fn detect_logical_block_size_finds_sset_at_1024() {
        // Most common layout: TAPE occupies exactly one 1024-byte
        // block and the next descriptor starts at offset 1024.
        let bytes = synthetic_stream_with_second_tag_at(1024, b"SSET");
        assert_eq!(detect_logical_block_size(&bytes), Some(1024));
    }

    #[test]
    fn detect_logical_block_size_handles_sfmb_half_block() {
        // The real SQL Server 2008 R2 layout observed in our
        // `Export.dmp`: SFMB appears at offset 1024 (a half-block
        // filemark), so that's what the detector should report as the
        // end of the TAPE block.
        let bytes = synthetic_stream_with_second_tag_at(1024, b"SFMB");
        assert_eq!(detect_logical_block_size(&bytes), Some(1024));
    }

    #[test]
    fn detect_logical_block_size_walks_512_byte_grid() {
        // Some older BAKs use 512-byte logical blocks; the detector
        // must therefore report the first recognizable tag it finds,
        // not force-round to 1024.
        let bytes = synthetic_stream_with_second_tag_at(512, b"SSET");
        assert_eq!(detect_logical_block_size(&bytes), Some(512));
    }

    #[test]
    fn detect_logical_block_size_supports_64k_stride() {
        // SQL Server 2016+ routinely uses 64 KiB blocks. We exercise
        // a single 64 KiB stride so regressions that accidentally cap
        // the scan short surface as test failures.
        let bytes = synthetic_stream_with_second_tag_at(65536, b"SSET");
        assert_eq!(detect_logical_block_size(&bytes), Some(65536));
    }

    #[test]
    fn detect_logical_block_size_returns_none_when_no_second_tag() {
        // Plain TAPE + padding — no hint where the next block should
        // be. Detector must surface `None` rather than guess.
        let bytes = vec![0u8; 8192]; // not a TAPE either, but the
                                     // detector only looks for the
                                     // *next* tag, so zero-filled
                                     // input is a valid "no hint"
                                     // fixture.
        assert_eq!(detect_logical_block_size(&bytes), None);
    }

    #[test]
    fn align_up_handles_edges() {
        // Already aligned → identity. Non-aligned → next multiple.
        assert_eq!(align_up(0, 512), 0);
        assert_eq!(align_up(1, 512), 512);
        assert_eq!(align_up(512, 512), 512);
        assert_eq!(align_up(513, 512), 1024);
        // Zero alignment is defensive: we treat it as no-op instead
        // of dividing by zero.
        assert_eq!(align_up(7, 0), 7);
    }

    #[test]
    fn cursor_walks_synthetic_three_block_stream() {
        // TAPE@0 (1024B), SFMB@1024 (512B), SSET@1536 (512B). Mirrors
        // the real Export.dmp shape well enough to exercise the
        // walker's boundary bookkeeping.
        let mut bytes = vec![0u8; 2048];
        bytes[0..4].copy_from_slice(b"TAPE");
        bytes[1024..1028].copy_from_slice(b"SFMB");
        bytes[1536..1540].copy_from_slice(b"SSET");

        let blocks: Vec<MtfBlock> = MtfBlockCursor::new(&bytes).collect();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, MtfBlockType::Tape);
        assert_eq!(blocks[0].offset, 0);
        assert_eq!(blocks[0].size, 1024);
        assert_eq!(blocks[1].block_type, MtfBlockType::SoftFilemark);
        assert_eq!(blocks[1].offset, 1024);
        assert_eq!(blocks[1].size, 512);
        assert_eq!(blocks[2].block_type, MtfBlockType::StartOfSet);
        assert_eq!(blocks[2].offset, 1536);
        // Final block's size fills to end-of-buffer.
        assert_eq!(blocks[2].size, bytes.len() - 1536);
    }

    #[test]
    fn cursor_stops_at_eotm() {
        // Everything after an EOTM descriptor is zero-padding; the
        // cursor must not keep matching tags beyond that point.
        let mut bytes = vec![0u8; 4096];
        bytes[0..4].copy_from_slice(b"TAPE");
        bytes[1024..1028].copy_from_slice(b"EOTM");
        // A rogue "SSET" in what is really just post-EOTM padding —
        // the cursor must ignore it.
        bytes[2048..2052].copy_from_slice(b"SSET");

        let kinds: Vec<MtfBlockType> = MtfBlockCursor::new(&bytes).map(|b| b.block_type).collect();
        assert_eq!(kinds, vec![MtfBlockType::Tape, MtfBlockType::EndOfTape]);
    }

    #[test]
    fn cursor_stops_on_unknown_leading_tag() {
        // Cursor must give up cleanly (not panic) if the bytes at its
        // current position do not form a known tag — happens in
        // practice if an upstream stage slices the buffer wrong.
        let bytes = vec![0x00u8; 2048];
        let blocks: Vec<MtfBlock> = MtfBlockCursor::new(&bytes).collect();
        assert!(blocks.is_empty());
    }

    #[test]
    fn cursor_copies_common_header_into_block() {
        let mut bytes = vec![0u8; 2048];
        bytes[0..4].copy_from_slice(b"TAPE");
        // Put a distinctive byte pattern in the header so we can see
        // it round-trip through the cursor's copy.
        bytes[10] = 0xAB;
        bytes[51] = 0xCD;

        let blocks: Vec<MtfBlock> = MtfBlockCursor::new(&bytes).take(1).collect();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].raw_common_header[0..4], *b"TAPE");
        assert_eq!(blocks[0].raw_common_header[10], 0xAB);
        assert_eq!(blocks[0].raw_common_header[51], 0xCD);
    }

    /// Emit a synthetic stream header at `offset` with the given tag
    /// and declared length, filling the body slot with `fill`.
    /// Helper for the stream-cursor tests below.
    fn emit_stream(buf: &mut Vec<u8>, offset: usize, tag: &[u8; 4], body_len: u64, fill: u8) {
        let total = STREAM_HEADER_LEN + body_len as usize;
        let end = offset + total;
        if buf.len() < end {
            buf.resize(end, 0);
        }
        buf[offset..offset + 4].copy_from_slice(tag);
        // attribute words (SQL Server-combined FSYS/MFS) = 0
        buf[offset + 4..offset + 8].fill(0);
        // declared length (semantics vary, but helpful for readers)
        buf[offset + 8..offset + 16].copy_from_slice(&body_len.to_le_bytes());
        // body payload
        buf[offset + 16..offset + 16 + body_len as usize].fill(fill);
    }

    #[test]
    fn stream_kind_classifies_known_tags() {
        assert_eq!(MtfStreamKind::from_bytes(*b"SPAD"), MtfStreamKind::Pad);
        assert_eq!(MtfStreamKind::from_bytes(*b"MSDA"), MtfStreamKind::SqlData);
        assert_eq!(
            MtfStreamKind::from_bytes(*b"MSDB"),
            MtfStreamKind::SqlDatabase
        );
        assert_eq!(
            MtfStreamKind::from_bytes(*b"MSCI"),
            MtfStreamKind::SqlConfig
        );
        assert_eq!(
            MtfStreamKind::from_bytes(*b"TSMP"),
            MtfStreamKind::Timestamp
        );
    }

    #[test]
    fn stream_cursor_scans_forward_to_find_body_end() {
        // Two synthetic streams back-to-back. The cursor should
        // report each stream with `body_end` pointing at the next
        // stream's header offset (so body_len == declared_length
        // when the synthesized layout is tight).
        let mut bytes = vec![0u8; 512];
        emit_stream(&mut bytes, 0, b"SPAD", 100, 0xAA);
        let spad_end = STREAM_HEADER_LEN + 100; // 116, already 4-aligned
        emit_stream(&mut bytes, spad_end, b"MSCI", 50, 0xBB);

        let streams: Vec<MtfStream> = MtfStreamCursor::new(&bytes, 0, bytes.len()).collect();
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].kind, MtfStreamKind::Pad);
        assert_eq!(streams[0].header_offset, 0);
        assert_eq!(streams[0].body_offset, STREAM_HEADER_LEN);
        assert_eq!(streams[0].body_end, spad_end);
        assert_eq!(streams[0].body_len(), 100);
        assert_eq!(streams[1].kind, MtfStreamKind::SqlConfig);
        assert_eq!(streams[1].header_offset, spad_end);
        assert_eq!(streams[1].body_offset, spad_end + STREAM_HEADER_LEN);
        // Final stream runs to the end of the slice.
        assert_eq!(streams[1].body_end, bytes.len());
    }

    #[test]
    fn stream_cursor_skips_gap_between_headers() {
        // Tag scanner must step over opaque body bytes without
        // mistaking them for new headers — simulate by placing an
        // SPAD followed by 64 bytes of random bytes before MSDA.
        let mut bytes = vec![0u8; 512];
        emit_stream(&mut bytes, 0, b"SPAD", 80, 0x11);
        // Random bytes in the gap — must NOT be recognized as tags.
        for b in bytes.iter_mut().take(96 + 64).skip(96) {
            *b = 0xEE;
        }
        emit_stream(&mut bytes, 160, b"MSDA", 32, 0x22);

        let kinds: Vec<MtfStreamKind> = MtfStreamCursor::new(&bytes, 0, bytes.len())
            .map(|s| s.kind)
            .collect();
        assert_eq!(kinds, vec![MtfStreamKind::Pad, MtfStreamKind::SqlData]);
    }

    #[test]
    fn stream_cursor_returns_none_when_no_stream_tag_present() {
        // Guards against the "over-scan" case: a DBLK whose body
        // section is entirely padding should yield zero streams.
        let bytes = vec![0u8; 128];
        assert!(MtfStreamCursor::new(&bytes, 0, bytes.len())
            .next()
            .is_none());
    }

    #[test]
    fn stream_cursor_honours_end_bound() {
        // Cursor must never read past the DBLK it was given. We
        // place an MSDA header at offset 0 with a huge declared
        // length; restricting the end to 256 should cap the body at
        // that offset.
        let mut bytes = vec![0u8; 512];
        emit_stream(&mut bytes, 0, b"MSDA", 10_000, 0xCC);

        let streams: Vec<MtfStream> = MtfStreamCursor::new(&bytes, 0, 256).collect();
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].kind, MtfStreamKind::SqlData);
        assert_eq!(streams[0].body_end, 256);
    }
}
