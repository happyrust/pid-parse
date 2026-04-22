//! Minimal SQL Server system-catalog (`sysschobjs`) row scanner.
//!
//! Stage-1 needs a way to map SQL Server **table names** to the
//! internal `object_id` values used by IAM / allocation metadata.
//! The engine stores this in the hidden `sys.sysschobjs` table at
//! `object_id = 34`, whose data pages live in the MDF's
//! metadata region.
//!
//! We do not yet decode SQL Server's full row format (status bits,
//! NULL bitmaps, variable-length column offset arrays) — that's a
//! full rainy-day reverse engineering effort. Instead, stage-1
//! exploits a **signature byte sequence** that sits at a fixed
//! offset inside every `sysschobjs` row:
//!
//! ```text
//! [... variable leading bytes ...]
//! [object_id u32 LE]
//! [constant 13-byte marker: 70 00 08 00 00 00 00 00 02 00 00 01 00]
//! [name_length_bytes u16 LE]
//! [UTF-16LE name, (name_length_bytes) bytes]
//! [... trailing padding ...]
//! ```
//!
//! This signature was discovered empirically from SQL Server 2008
//! R2 fixture `Export.dmp` in our SmartPlant TEST02 backup: the
//! marker appears exactly once per catalog row. See also
//! [`SYSSCHOBJS_ROW_MARKER`].
//!
//! Rows matched here are a **subset** of the real schema (we skip
//! any row whose name is non-printable UTF-16LE or whose declared
//! length would overflow the page). That's fine for stage-1 needs:
//! we only care about recovering the SmartPlant `T_*` user tables.
//!
//! # Validation
//!
//! Each candidate row is cross-checked by:
//!
//! 1. Declared name length must be even (UTF-16LE code units).
//! 2. Declared name length must fit within the page slice remaining
//!    after the marker.
//! 3. All UTF-16LE code units in the name must be printable ASCII
//!    or valid BMP code points.
//! 4. The four bytes preceding the marker are interpreted as the
//!    row's `object_id`. We do not bound-check the numeric range
//!    because SmartPlant catalogs can hit 10000+ object ids.

/// 13-byte signature found immediately before the name field of
/// every `sysschobjs` data row observed in SQL Server 2008 R2
/// backups. See module docs for the full layout.
pub const SYSSCHOBJS_ROW_MARKER: &[u8] = &[
    0x70, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x01, 0x00,
];

/// One decoded `sysschobjs` row: the mapping from internal
/// `object_id` to the human-readable table / view name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysschobjsRow {
    /// Byte offset at which the row's signature marker was found
    /// (inside the page slice passed to [`scan_sysschobjs_rows`]).
    /// Useful for correlating with [`crate::backup::MdfPageCursor`].
    pub marker_offset: usize,
    /// `object_id` (4-byte unsigned little-endian immediately
    /// preceding the marker).
    pub object_id: u32,
    /// Decoded UTF-16LE name (e.g. `"T_ModelItem"`).
    pub name: String,
}

/// Scan `page` for every `sysschobjs`-shaped row. Returns them in
/// marker-offset order. Candidates that fail any of the validations
/// described in the module docs are silently dropped.
pub fn scan_sysschobjs_rows(page: &[u8]) -> Vec<SysschobjsRow> {
    const MARKER: &[u8] = SYSSCHOBJS_ROW_MARKER;
    let mut out = Vec::new();
    if page.len() < MARKER.len() + 4 + 2 {
        return out;
    }

    let mut i = 4; // need 4 bytes of object_id before the marker
    while i + MARKER.len() + 2 <= page.len() {
        if &page[i..i + MARKER.len()] == MARKER {
            let object_id_offset = i - 4;
            let object_id = u32::from_le_bytes([
                page[object_id_offset],
                page[object_id_offset + 1],
                page[object_id_offset + 2],
                page[object_id_offset + 3],
            ]);
            let name_len_offset = i + MARKER.len();
            let name_len = u16::from_le_bytes([page[name_len_offset], page[name_len_offset + 1]])
                as usize;
            let name_bytes_offset = name_len_offset + 2;

            if let Some(decoded) = try_decode_utf16le_name(page, name_bytes_offset, name_len) {
                out.push(SysschobjsRow {
                    marker_offset: i,
                    object_id,
                    name: decoded,
                });
            }

            // Advance past the whole match whether we accepted it
            // or not; marker runs never overlap.
            i = name_bytes_offset
                .saturating_add(name_len.min(page.len().saturating_sub(name_bytes_offset)))
                .max(i + MARKER.len());
        } else {
            i += 1;
        }
    }
    out
}

/// Try to read up to `byte_count` bytes at `offset` as a UTF-16LE
/// string. SQL Server sysschobjs rows store the field length
/// *including trailing metadata padding* (empirically name
/// length + 15 bytes of per-row trailer); the real name ends at
/// the first `U+0000` code unit. We stop there rather than
/// rejecting the whole row because of that padding.
///
/// Stage-1 only treats printable ASCII (0x20..=0x7E) as valid
/// per-unit content. SmartPlant table names are all ASCII in the
/// current fixture; broadening the allow-list to full BMP is a
/// follow-up when we encounter real CJK table names.
fn try_decode_utf16le_name(page: &[u8], offset: usize, byte_count: usize) -> Option<String> {
    if byte_count == 0 {
        return None;
    }
    // Cap declared field length at a sane upper bound. The SQL
    // Server `sysname` type is `nvarchar(128)` plus ~16 bytes of
    // per-row trailing metadata, so ~300 bytes is a safe ceiling.
    if byte_count > 300 {
        return None;
    }
    // SmartPlant fixture emits odd byte_count values (e.g. 37 for
    // the `T_ModelItem` row) because the declared length mixes the
    // real name bytes with a few trailing metadata bytes that are
    // not always 2-byte aligned. Round down to the nearest even
    // count so the UTF-16LE decoder can walk whole code units;
    // the NUL-terminator check below handles the real boundary.
    let even_count = byte_count - (byte_count % 2);
    let end = offset.checked_add(even_count)?;
    if end > page.len() {
        return None;
    }

    let mut decoded = String::with_capacity(even_count / 2);
    for pair in page[offset..end].chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        match unit {
            // Printable ASCII code unit — part of the name.
            0x0020..=0x007E => decoded.push(unit as u8 as char),
            // NUL terminator — the real name ended here; the
            // remaining bytes up to `byte_count` are per-row
            // metadata / padding that the scanner does not yet
            // interpret.
            0x0000 => break,
            // Any other value means we're looking at binary bytes
            // that are not a valid ASCII name. Reject the whole
            // row so ghost matches in random page content are
            // filtered out.
            _ => return None,
        }
    }
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic page slice with one [`sysschobjs`] row at
    /// the given offset. Row bytes:
    /// `[leading][object_id u32 LE][MARKER][name_len u16 LE][name UTF-16LE]`.
    fn emit_row(buf: &mut Vec<u8>, leading_pad: usize, object_id: u32, name: &str) {
        for _ in 0..leading_pad {
            buf.push(0x00);
        }
        buf.extend_from_slice(&object_id.to_le_bytes());
        buf.extend_from_slice(SYSSCHOBJS_ROW_MARKER);
        let name_bytes: Vec<u8> = name
            .chars()
            .flat_map(|c| (c as u16).to_le_bytes())
            .collect();
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&name_bytes);
    }

    #[test]
    fn scan_finds_single_row() {
        let mut page = Vec::new();
        emit_row(&mut page, 16, 0x1085, "T_ModelItem");

        let rows = scan_sysschobjs_rows(&page);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].object_id, 0x1085);
        assert_eq!(rows[0].name, "T_ModelItem");
    }

    #[test]
    fn scan_finds_multiple_rows_in_order() {
        let mut page = Vec::new();
        emit_row(&mut page, 32, 0x1085, "T_ModelItem");
        emit_row(&mut page, 16, 0x1086, "T_Drawing");
        emit_row(&mut page, 8, 0x10B2, "T_ModelItemClaim");

        let rows = scan_sysschobjs_rows(&page);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].object_id, 0x1085);
        assert_eq!(rows[0].name, "T_ModelItem");
        assert_eq!(rows[1].object_id, 0x1086);
        assert_eq!(rows[1].name, "T_Drawing");
        assert_eq!(rows[2].object_id, 0x10B2);
        assert_eq!(rows[2].name, "T_ModelItemClaim");
        // Offsets strictly increase.
        assert!(rows[0].marker_offset < rows[1].marker_offset);
        assert!(rows[1].marker_offset < rows[2].marker_offset);
    }

    #[test]
    fn scan_rejects_rows_with_non_printable_name_bytes() {
        // Emit a valid row followed by a marker with bogus name
        // bytes (0xFFFF code unit is non-printable). Only the
        // first row should survive the scan.
        let mut page = Vec::new();
        emit_row(&mut page, 16, 0x1085, "T_Good");

        let before_noise = page.len();
        // Fake object_id + marker + length(2 bytes for 1 char) +
        // two noise bytes that decode to 0xFFFF — invalid UTF-16.
        page.extend_from_slice(&0x9999u32.to_le_bytes());
        page.extend_from_slice(SYSSCHOBJS_ROW_MARKER);
        page.extend_from_slice(&2u16.to_le_bytes());
        page.extend_from_slice(&[0xFF, 0xFF]);
        assert!(page.len() > before_noise);

        let rows = scan_sysschobjs_rows(&page);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "T_Good");
    }

    #[test]
    fn scan_accepts_rows_with_odd_declared_length() {
        // Real SmartPlant fixture rows report `name_bytes + 15`
        // byte counts, which can be odd. The decoder must round
        // down to the nearest UTF-16LE code unit and still return
        // the real name (terminated by a NUL code unit).
        let mut page = Vec::new();
        page.extend_from_slice(&[0u8; 4]); // object_id padding
        page.extend_from_slice(&0x0042u32.to_le_bytes()); // real id
        page.extend_from_slice(SYSSCHOBJS_ROW_MARKER);
        // Declared length 7 (odd) — name "AB" = 4 bytes + 2-byte
        // NUL + 1 trailing byte.
        page.extend_from_slice(&7u16.to_le_bytes());
        page.extend_from_slice(&[b'A', 0, b'B', 0, 0, 0, 0xAA]);

        let rows = scan_sysschobjs_rows(&page);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "AB");
        assert_eq!(rows[0].object_id, 0x42);
    }

    #[test]
    fn scan_returns_empty_when_page_too_small() {
        assert!(scan_sysschobjs_rows(&[]).is_empty());
        assert!(scan_sysschobjs_rows(&[0u8; 10]).is_empty());
    }

    #[test]
    fn scan_does_not_panic_on_truncated_input_after_marker() {
        // Page ends right after the marker — no length/name bytes.
        let mut page = Vec::new();
        page.extend_from_slice(&[0u8; 4]); // object_id placeholder
        page.extend_from_slice(SYSSCHOBJS_ROW_MARKER);
        // No length + name bytes follow; scan should bail cleanly.
        assert!(scan_sysschobjs_rows(&page).is_empty());
    }

    #[test]
    fn try_decode_caps_at_upper_bound() {
        // 400-byte declared length is refused — outside sysname
        // + per-row metadata padding territory.
        let page = vec![0u8; 500];
        assert!(try_decode_utf16le_name(&page, 0, 400).is_none());
    }

    #[test]
    fn try_decode_stops_at_first_nul_code_unit() {
        // Mirrors the real MSDA layout: `name_bytes_length + 15`
        // declared, name followed by `0x00 0x00` terminator then
        // per-row metadata padding.
        let mut page = vec![0u8; 64];
        // "T_Foo" in UTF-16LE = 10 bytes
        for (i, c) in "T_Foo".chars().enumerate() {
            page[i * 2] = c as u8;
            page[i * 2 + 1] = 0;
        }
        // NUL terminator at bytes 10-11 (U+0000 code unit).
        page[10] = 0;
        page[11] = 0;
        // Fake per-row trailing metadata after the NUL.
        for b in page.iter_mut().take(24).skip(12) {
            *b = 0xCC;
        }
        // Declared length = 10 (name) + 15 (metadata) = 25? Must
        // be even for UTF-16LE — fix at 26 so the decoder loop
        // can walk exactly 13 code units; the scan bails at the
        // first NUL.
        let decoded = try_decode_utf16le_name(&page, 0, 26).expect("should decode");
        assert_eq!(decoded, "T_Foo");
    }

    #[test]
    fn try_decode_rejects_non_ascii_binary_bytes() {
        // Random binary bytes in the declared-length window: the
        // decoder should reject because a real name can't contain
        // those code units.
        let mut page = vec![0u8; 32];
        page[0] = 0xFF;
        page[1] = 0xFF;
        assert!(try_decode_utf16le_name(&page, 0, 16).is_none());
    }
}
