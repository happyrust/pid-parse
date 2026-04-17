//! Utilities for decoding 4-byte magic numbers into human-readable tags.
//!
//! PID streams often start with a 4-byte little-endian magic number. When
//! interpreted as ASCII, the bytes sometimes spell a readable label (e.g.
//! `0x746F6F72` -> "toor" = "root" reversed, used by `PSMroots`).
//!
//! This module exposes helpers used by the probe / reporting layer to surface
//! these tags without committing to a semantic interpretation.

/// Convert a little-endian u32 magic number into a 4-character ASCII tag.
///
/// Returns `None` if any byte is outside the printable ASCII range
/// (0x20..=0x7e), to avoid emitting control characters in reports.
///
/// Byte order: the lowest byte of the u32 is the first character (the byte
/// as it was read from the stream), matching how `u32::from_le_bytes` parses
/// the input stream.
pub fn magic_tag(magic: u32) -> Option<String> {
    let bytes = magic.to_le_bytes();
    if bytes.iter().all(|&b| (0x20..=0x7e).contains(&b)) {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        None
    }
}

/// Known magic tags we have identified in `.pid` files. Used for reporting.
///
/// Byte order note: values below are u32 as read by `u32::from_le_bytes`;
/// `magic_tag` renders them back in on-disk byte order, so the printable
/// strings ("root", "Smar", "clst", "stab", ...) read left-to-right as they
/// appear in the file.
pub fn describe_magic(magic: u32) -> &'static str {
    match magic {
        0x6C90F544 => "SP cluster (shared header)",
        0x67657374 => "PSMspacemap segment table",
        0x746F6F72 => "PSMroots root table",
        0x74736C63 => "PSMclustertable index",
        0x62617473 => "PSMsegmenttable index",
        0x72616D53 => "DocVersion (SmartPlant)",
        0x6D783F3C => "XML declaration (<?xm)",
        0x53454C4F => "OLE storage block",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_ascii_converts() {
        // Values are magic_u32_le read from real .pid streams; the ASCII
        // rendering matches the on-disk byte order (since to_le_bytes puts
        // the lowest byte first, i.e. the first byte of the stream).
        assert_eq!(magic_tag(0x67657374).as_deref(), Some("tseg"));
        assert_eq!(magic_tag(0x746F6F72).as_deref(), Some("root"));
        assert_eq!(magic_tag(0x6D783F3C).as_deref(), Some("<?xm"));
        assert_eq!(magic_tag(0x53454C4F).as_deref(), Some("OLES"));
        assert_eq!(magic_tag(0x72616D53).as_deref(), Some("Smar"));
        assert_eq!(magic_tag(0x74736C63).as_deref(), Some("clst"));
        assert_eq!(magic_tag(0x62617473).as_deref(), Some("stab"));
    }

    #[test]
    fn non_printable_returns_none() {
        assert_eq!(magic_tag(0x6C90F544), None);
        assert_eq!(magic_tag(0x00000005), None);
    }

    #[test]
    fn known_tags_described() {
        assert!(describe_magic(0x6C90F544).contains("cluster"));
        assert!(describe_magic(0x746F6F72).contains("root"));
        assert!(describe_magic(0x72616D53).contains("SmartPlant"));
        assert_eq!(describe_magic(0xDEADBEEF), "");
    }
}
