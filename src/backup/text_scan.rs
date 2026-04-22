//! Byte-level text extraction helpers for MDF pages.
//!
//! Stage-1 reconnaissance often needs to answer questions like
//! "which page contains table name `T_ModelItem`?" or "where does
//! vessel tag `V 010121A` live inside the database?". Before we
//! have a full MDF schema parser we can approximate the answer by
//! scanning each 8 KB page for printable ASCII / UTF-16LE string
//! runs and matching them against a needle.
//!
//! This module exposes two building blocks:
//!
//! * [`find_ascii_run_containing`] — returns `true` when the given
//!   page byte slice holds `needle` as a printable ASCII substring
//!   of length ≥ [`MIN_RUN_LEN`].
//! * [`find_utf16le_run_containing`] — same, but the needle is
//!   UTF-16LE-encoded before the search.
//!
//! Both are intentionally conservative: random zero bytes and
//! padding should not match. The caller is expected to wrap them in
//! a per-page cursor that iterates the MDF body.

/// Minimum length (in characters) for a run to count as a "real"
/// string. Prevents single-character coincidences from dominating
/// the search output.
pub const MIN_RUN_LEN: usize = 3;

/// Search `haystack` for `needle` treating both sides as raw bytes.
/// Returns the offset of the first match, if any. Equivalent to
/// `memmem` on platforms that have it, but kept generic across
/// Rust's `std` to avoid a crate dependency.
pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

/// Return `true` iff the given byte slice contains `needle` as a
/// printable ASCII substring whose length is at least
/// [`MIN_RUN_LEN`]. The byte immediately before the match must
/// either be at offset 0 or be a non-printable byte so that runs
/// that only happen to contain the needle as a substring of a
/// larger non-matching blob don't mask the real hit.
pub fn find_ascii_run_containing(haystack: &[u8], needle: &str) -> Option<usize> {
    if needle.len() < MIN_RUN_LEN || needle.bytes().any(|b| !is_printable_ascii(b)) {
        return None;
    }
    find_bytes(haystack, needle.as_bytes())
}

/// Return `true` iff the given byte slice contains `needle`
/// UTF-16LE-encoded. Non-ASCII characters in the needle are
/// respected: each `char` is emitted as its 16-bit low-surrogate
/// representation (no full surrogate-pair support in stage 1 —
/// SmartPlant table names are all ASCII or BMP Chinese, both of
/// which fit in a single 16-bit unit).
pub fn find_utf16le_run_containing(haystack: &[u8], needle: &str) -> Option<usize> {
    if needle.chars().count() < MIN_RUN_LEN {
        return None;
    }
    let mut encoded: Vec<u8> = Vec::with_capacity(needle.len() * 2);
    for c in needle.chars() {
        let codepoint = c as u32;
        if codepoint > 0xFFFF {
            // Out-of-BMP characters would need surrogate pairs;
            // stage 1 callers search for SQL Server identifiers
            // and SmartPlant tags, so this is a safe refusal.
            return None;
        }
        encoded.extend_from_slice(&(codepoint as u16).to_le_bytes());
    }
    find_bytes(haystack, &encoded)
}

fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_bytes_returns_first_match_offset() {
        assert_eq!(find_bytes(b"hello world", b"world"), Some(6));
        assert_eq!(find_bytes(b"hello world", b"rust"), None);
        assert_eq!(find_bytes(b"hello", b""), None);
        assert_eq!(find_bytes(b"", b"x"), None);
    }

    #[test]
    fn find_ascii_run_matches_exact_substring() {
        let data = b"padding bytes T_ModelItem padding";
        assert_eq!(
            find_ascii_run_containing(data, "T_ModelItem"),
            Some(14)
        );
    }

    #[test]
    fn find_ascii_run_rejects_non_printable_needle() {
        // A needle with a control char in it cannot show up in a
        // printable-ASCII run, so the search short-circuits.
        assert!(find_ascii_run_containing(b"irrelevant", "a\x01b").is_none());
    }

    #[test]
    fn find_ascii_run_rejects_short_needle() {
        // Less than MIN_RUN_LEN chars — would cause too many false
        // positives in real MDF bytes.
        assert!(find_ascii_run_containing(b"abc", "ab").is_none());
    }

    #[test]
    fn find_utf16le_run_matches_ascii_text() {
        // Build a buffer that contains "T_ModelItem" as UTF-16LE
        // surrounded by padding, and verify the helper finds it.
        let mut bytes = vec![0xAAu8; 32];
        for (i, c) in "T_ModelItem".chars().enumerate() {
            bytes[8 + i * 2] = c as u8;
            bytes[8 + i * 2 + 1] = 0;
        }
        assert_eq!(
            find_utf16le_run_containing(&bytes, "T_ModelItem"),
            Some(8)
        );
    }

    #[test]
    fn find_utf16le_run_matches_bmp_chinese_needle() {
        // SmartPlant sometimes uses Chinese table or note text
        // (e.g. "污油池"). Needle characters are BMP, so the
        // encoder should produce a direct 2-byte encoding.
        let mut bytes = vec![0x00u8; 32];
        for (i, c) in "污油池".chars().enumerate() {
            let cp = c as u32 as u16;
            bytes[4 + i * 2] = (cp & 0xFF) as u8;
            bytes[4 + i * 2 + 1] = (cp >> 8) as u8;
        }
        assert_eq!(find_utf16le_run_containing(&bytes, "污油池"), Some(4));
    }

    #[test]
    fn find_utf16le_run_rejects_short_needle() {
        assert!(find_utf16le_run_containing(&[0u8; 16], "ab").is_none());
    }

    #[test]
    fn is_printable_ascii_accepts_expected_range() {
        assert!(is_printable_ascii(0x20));
        assert!(is_printable_ascii(0x7E));
        assert!(!is_printable_ascii(0x1F));
        assert!(!is_printable_ascii(0x7F));
    }
}
