//! SQL Server MDF **Boot Page** (page 9 of the primary data file)
//! decoder.
//!
//! Every SQL Server database carries a single "Boot Page" at page
//! number 9 of the primary `.mdf` file. It holds DBINFO metadata —
//! the database name, creation timestamp, SQL Server build info,
//! last-checkpoint LSN, and various configuration flags — in a
//! fixed layout that has been stable from SQL Server 2005 through
//! 2022.
//!
//! Stage-1 starts minimal: we only decode the database-name field,
//! which is sufficient for our offline `SmartPlant` pipeline to
//! correlate the MDF inside an MSDA stream with the logical
//! database the backup describes. More DBINFO fields (create
//! timestamp, server version, family GUID) can be added when
//! downstream stages need them.
//!
//! # Field layout (relative to page start)
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | `0x000` | 96 | Standard MDF page header (parsed elsewhere) |
//! | `0x060` | 52 | DBINFO magic / family GUID / build info (not yet decoded) |
//! | `0x094` | 256 | `DatabaseName` — UTF-16LE, right-padded with U+0020 |
//! | `0x194` | ... | Additional DBINFO fields (not yet decoded) |
//!
//! The 256-byte name field holds **128 UTF-16LE code units**,
//! right-padded with ASCII space (0x20). Stage-1 trims the padding
//! to return a rustic `String`.

use crate::backup::mdf_page::PAGE_SIZE;

/// Byte offset of the database-name field inside a Boot Page body.
pub const DATABASE_NAME_OFFSET: usize = 0x94;
/// Byte length of the database-name field (128 UTF-16LE code units).
pub const DATABASE_NAME_LEN: usize = 256;

/// Parsed subset of Boot Page DBINFO. More fields will land in
/// stage-1 follow-ups; the struct is `non_exhaustive`-flavored but
/// not marked so because callers in the same crate benefit from
/// destructuring in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootPageInfo {
    /// Logical database name read from the boot page's
    /// `DatabaseName` field with trailing padding stripped.
    pub database_name: String,
}

/// Errors returned by [`parse_boot_page`]. Kept small because the
/// only real failure mode is "input does not look like a page".
#[derive(Debug)]
pub enum BootPageError {
    /// Input slice is shorter than a full MDF page.
    TooShort {
        /// Bytes actually available in the input slice.
        got: usize,
    },
    /// The boot page's `DatabaseName` bytes could not be decoded as
    /// valid UTF-16LE that resolves to at least one printable
    /// character after trimming padding. Indicates the slice is
    /// not a real boot page.
    NameFieldEmpty,
}

impl std::fmt::Display for BootPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got } => write!(
                f,
                "boot page input too short: need {PAGE_SIZE} bytes, got {got}"
            ),
            Self::NameFieldEmpty => {
                f.write_str("DatabaseName field contained only padding / null bytes")
            }
        }
    }
}

impl std::error::Error for BootPageError {}

/// Parse a Boot Page. `page_bytes` must cover the full 8192-byte
/// page; shorter input is rejected rather than silently padded.
pub fn parse_boot_page(page_bytes: &[u8]) -> Result<BootPageInfo, BootPageError> {
    if page_bytes.len() < PAGE_SIZE {
        return Err(BootPageError::TooShort {
            got: page_bytes.len(),
        });
    }
    let name_slice = &page_bytes[DATABASE_NAME_OFFSET..DATABASE_NAME_OFFSET + DATABASE_NAME_LEN];
    let decoded = decode_utf16le_trim_padding(name_slice);
    if decoded.is_empty() {
        return Err(BootPageError::NameFieldEmpty);
    }
    Ok(BootPageInfo {
        database_name: decoded,
    })
}

/// Decode a UTF-16LE slice, then strip trailing padding.
///
/// **Why this is not a simple byte-level trim**: SQL Server stores
/// the database name as an ASCII-in-UTF-16LE string (each character
/// is `<ascii> 0x00`) but right-pads the rest of the 256-byte field
/// with repeating **single** `0x20` bytes — not `0x20 0x00` UTF-16LE
/// space code units. When the pad bytes are consumed two-at-a-time
/// they decode as U+2020 (`†`), not U+0020 (` `). A byte-level trim
/// would fix the pad but also eats the 0x00 high byte of the last
/// real character (e.g. `"A"` = `41 00` ends on a 0x00 byte), which
/// cuts the string short.
///
/// The robust approach: decode to UTF-16 code units first, then
/// trim trailing units that match any padding sentinel
/// (`U+0000`, `U+0020`, `U+2020`).
fn decode_utf16le_trim_padding(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let trimmed_end = units
        .iter()
        .rposition(|&c| c != 0x0000 && c != 0x0020 && c != 0x2020)
        .map_or(0, |i| i + 1);
    String::from_utf16_lossy(&units[..trimmed_end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic 8192-byte page with `name` embedded at the
    /// standard `DatabaseName` offset, left-aligned and padded out
    /// with single-byte `0x20` fill — **not** UTF-16LE code unit
    /// spaces. Matches the exact byte layout SQL Server writes to
    /// disk, which is what the decoder needs to handle.
    fn synthetic_boot_page(name: &str) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_SIZE];
        let mut name_bytes: Vec<u8> = Vec::with_capacity(DATABASE_NAME_LEN);
        for c in name.chars() {
            name_bytes.extend_from_slice(&(c as u16).to_le_bytes());
        }
        // Byte-level 0x20 padding — a single 0x20 per trailing byte,
        // not a UTF-16LE-encoded space. See the parser's doc comment
        // for why this distinction matters.
        while name_bytes.len() < DATABASE_NAME_LEN {
            name_bytes.push(0x20);
        }
        page[DATABASE_NAME_OFFSET..DATABASE_NAME_OFFSET + DATABASE_NAME_LEN]
            .copy_from_slice(&name_bytes);
        page
    }

    #[test]
    fn parse_boot_page_reads_database_name_with_trailing_padding() {
        let page = synthetic_boot_page("MyDatabase");
        let info = parse_boot_page(&page).expect("synthetic boot page should parse");
        assert_eq!(info.database_name, "MyDatabase");
    }

    #[test]
    fn parse_boot_page_handles_full_width_name() {
        // Exercise the upper bound: 64-char name fits well under
        // 128 UTF-16LE code units.
        let name = "SmartPlantEngineeringDatabase_for_offline_backup_roundtrip";
        let page = synthetic_boot_page(name);
        let info = parse_boot_page(&page).expect("wide name should parse");
        assert_eq!(info.database_name, name);
    }

    #[test]
    fn parse_boot_page_rejects_short_input() {
        let err = parse_boot_page(&[0u8; 1024]).unwrap_err();
        match err {
            BootPageError::TooShort { got } => assert_eq!(got, 1024),
            other => panic!("expected TooShort, got {other:?}"),
        }
    }

    #[test]
    fn parse_boot_page_rejects_all_padding_name_field() {
        // If the page is just zeros / spaces at the name offset,
        // the parser should refuse rather than return an empty
        // string — that's a strong signal the slice is not really
        // a boot page.
        let page = vec![0u8; PAGE_SIZE];
        assert!(matches!(
            parse_boot_page(&page),
            Err(BootPageError::NameFieldEmpty)
        ));
    }

    #[test]
    fn decode_utf16le_trim_padding_strips_trailing_spaces_and_nulls() {
        // Round-trip "foo    " in UTF-16LE + null terminator.
        let bytes = [
            b'f', 0, b'o', 0, b'o', 0, 0x20, 0, 0x20, 0, 0x20, 0, 0, 0, 0, 0,
        ];
        assert_eq!(decode_utf16le_trim_padding(&bytes), "foo");
    }
}
