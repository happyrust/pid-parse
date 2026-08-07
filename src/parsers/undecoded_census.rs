//! Census of Sheet PSM records that no typed decoder claims, split by
//! `SmartPlant`'s own graphic predicate (Phase 38 S2: silent drops become
//! named warnings).
//!
//! `radsrvitem.dll!sub_56449950` is the native "is this type code a graphic
//! element" predicate (see
//! `docs/analysis/2026-08-04-annotation-families-risk.md` §2). A record that
//! passes it is content the vendor draws, so dropping one without a word is
//! the failure mode that document names: an `igDimension` / `igBalloon` /
//! `igLeader` annotation class silently vanishing from a rendered drawing.
//! A record that fails it (the `*Relation2d` constraint families) carries no
//! strokes, and warning about it would be noise.
//!
//! This module deliberately produces **warnings, not decoders**: none of
//! these families has a fixture to validate a decoder against, and parser
//! promotion without fixtures is forbidden (format guide §8.3). The first
//! real drawing that trips one of these warnings delivers that fixture.
//!
//! The record walk is the chain-validated greedy walk proven by
//! `examples/probe_psm_type_code_histogram`: a candidate 6-byte PSM header
//! (`u16` type word + `u32 bytes_to_follow`) is accepted only when the
//! position it advances to is end-of-stream or itself a valid header. That
//! keeps one coincidental header inside another record's payload from
//! swallowing the rest of the stream.

use core::ops::Range;

/// PSM type codes accepted by the native graphic predicate
/// `radsrvitem.dll!sub_56449950` (the "these draw" set).
///
/// Source: `docs/analysis/2026-08-04-annotation-families-risk.md` §2, read
/// out of the predicate's own constant set in IDA. The decimal entries are
/// `277 / 279 / 280` (`igDimension` / `igBalloon` / `igLeader`).
pub const NATIVE_GRAPHIC_TYPE_CODES: [u16; 20] = [
    0x0013, 0x0018, 0x0020, 0x0021, 0x003D, 0x004D, 0x0059, 0x005D, 0x005E, 0x0061, 0x0063, 0x007B,
    0x007E, 0x0084, 0x00CE, 0x00FA, 0x00FF, 0x0115, 0x0117, 0x0118,
];

/// PSM type codes a typed decoder in this crate already covers.
///
/// A record with one of these codes is never "unknown": it is either inside
/// a claimed byte range, or it was rejected by that family's validation
/// rules — a different diagnostic than the one this census reports.
/// `model::sheet_families` asserts this list stays in sync with the family
/// registry.
pub const DECODED_TYPE_CODES: [u16; 11] = [
    0x3FE6, 0x0018, 0x0084, 0x005E, 0x004D, 0x00CE, 0x0013, 0x003D, 0x00FA, 0x0030, 0x0010,
];

/// True when `SmartPlant`'s native graphic predicate accepts `type_code`,
/// i.e. records of this code are drawable content.
pub fn is_native_graphic_type_code(type_code: u16) -> bool {
    NATIVE_GRAPHIC_TYPE_CODES.contains(&type_code)
}

/// Class name for a PSM type code, from the resolved PSM `type code → CLSID`
/// registry (`docs/analysis/2026-08-04-psm-type-code-registry.md` §3).
///
/// The three annotation families use their `ig*` class-table names
/// (`radsrvitem.dll!sub_56448F70`), which are better evidenced than the
/// same-GUID-family RAD names that registry flags as low-confidence.
pub fn rad_class_name(type_code: u16) -> Option<&'static str> {
    Some(match type_code {
        0x0006 => "OnElement Constraint",
        0x0007 => "JSheetSetup Object",
        0x0013 => "Boundary2d Object",
        0x0018 => "Line Object",
        0x0020 => "Rectangle Object",
        0x0021 => "ComplexString Object",
        0x0030 => "JSL Override Style",
        0x003D => "SmartFrame2d Object",
        0x004D => "Text Object",
        0x0059 => "Circle Object",
        0x005A => "JSL Style Librarian",
        0x005D => "BspCurve Object",
        0x005E => "Point Object",
        0x0061 => "Arc Object",
        0x0063 => "Ellipse Object",
        0x0077 => "Fix Constraint",
        0x007B => "Group implementation",
        0x007E => "Elliptical Arc Object",
        0x0084 => "LineString Object",
        0x0085 => "Vertical Constraint",
        0x00CE => "JSymbol",
        0x00FA => "Dependency Object",
        0x00FF => "Graphics Bag",
        0x0115 => "igDimension",
        0x0117 => "igBalloon",
        0x0118 => "igLeader",
        _ => return None,
    })
}

/// One PSM type code observed in a Sheet stream with no typed decoder,
/// with its hit count and its native-predicate class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndecodedTypeCodeCount {
    /// PSM 14-bit type code.
    pub type_code: u16,
    /// Number of chain-validated records with this code outside every
    /// claimed byte range.
    pub count: usize,
    /// Whether the native graphic predicate accepts this code — the
    /// warn/stay-quiet split.
    pub is_graphic: bool,
    /// Class name from the PSM type-code registry, when known.
    pub rad_class_name: Option<&'static str>,
}

/// Smallest `bytes_to_follow` a census candidate may carry. Matches the
/// histogram probe's floor; anything smaller is header noise.
const CENSUS_MIN_BYTES_TO_FOLLOW: usize = 8;

/// Largest `bytes_to_follow` a census candidate may carry. Matches the
/// histogram probe's cap; a coincidental header with a giant length would
/// otherwise swallow the stream.
const CENSUS_MAX_BYTES_TO_FOLLOW: usize = 100_000;

/// If a plausible PSM record starts at `bytes[off]`, its end offset.
fn psm_record_end(bytes: &[u8], off: usize) -> Option<usize> {
    let header_end = off.checked_add(6)?;
    if header_end > bytes.len() {
        return None;
    }
    let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
    if type_word & 0x3FFF == 0 {
        return None;
    }
    let btf = u32::from_le_bytes([
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
    ]) as usize;
    if !(CENSUS_MIN_BYTES_TO_FOLLOW..=CENSUS_MAX_BYTES_TO_FOLLOW).contains(&btf) {
        return None;
    }
    let end = header_end.checked_add(btf)?;
    (end <= bytes.len()).then_some(end)
}

/// Count the chain-validated PSM records in `data` whose type code has no
/// typed decoder and whose start offset no typed decoder claimed.
///
/// `claimed` is the union of every family's decoded byte ranges for this
/// same stream (the caller collects them from the family registry). Records
/// inside a claimed range are the typed decoders' business; records of a
/// [`DECODED_TYPE_CODES`] code outside every claimed range are
/// validation-rejected shapes of a known family, which is a different
/// diagnostic — both are skipped here.
///
/// The result is sorted by type code and carries the graphic/non-graphic
/// split; consumers warn on the graphic entries and stay quiet about the
/// rest.
pub fn undecoded_type_code_census(
    data: &[u8],
    claimed: &[Range<usize>],
) -> Vec<UndecodedTypeCodeCount> {
    let mut counts: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    let mut off = 0usize;
    while off + 6 <= data.len() {
        if let Some(end) = psm_record_end(data, off) {
            if end == data.len() || psm_record_end(data, end).is_some() {
                let type_code = u16::from_le_bytes([data[off], data[off + 1]]) & 0x3FFF;
                let claimed_here = claimed.iter().any(|range| range.contains(&off));
                if !claimed_here && !DECODED_TYPE_CODES.contains(&type_code) {
                    *counts.entry(type_code).or_insert(0) += 1;
                }
                off = end;
                continue;
            }
        }
        off += 1;
    }
    counts
        .into_iter()
        .map(|(type_code, count)| UndecodedTypeCodeCount {
            type_code,
            count,
            is_graphic: is_native_graphic_type_code(type_code),
            rad_class_name: rad_class_name(type_code),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One synthetic PSM record: 6-byte header + zeroed payload.
    fn record(type_code: u16, payload_len: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&type_code.to_le_bytes());
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, payload_len as usize));
        bytes
    }

    #[test]
    fn an_igdimension_record_is_counted_as_a_graphic_drop() {
        let data = record(0x0115, 8);

        let census = undecoded_type_code_census(&data, &[]);

        assert_eq!(
            census,
            vec![UndecodedTypeCodeCount {
                type_code: 0x0115,
                count: 1,
                is_graphic: true,
                rad_class_name: Some("igDimension"),
            }]
        );
    }

    #[test]
    fn a_constraint_record_is_counted_but_not_graphic() {
        let data = record(0x0077, 8);

        let census = undecoded_type_code_census(&data, &[]);

        assert_eq!(census.len(), 1);
        assert!(!census[0].is_graphic);
        assert_eq!(census[0].rad_class_name, Some("Fix Constraint"));
    }

    #[test]
    fn claimed_ranges_and_known_family_codes_are_skipped() {
        let mut data = record(0x0115, 8);
        let second_start = data.len();
        data.extend(record(0x0018, 8)); // known family code, unclaimed
        data.extend(record(0x0117, 8));

        let claimed: Vec<Range<usize>> = std::iter::once(0..14).collect();
        let census = undecoded_type_code_census(&data, &claimed);

        // 0x0115 is claimed, 0x0018 has a typed decoder, 0x0117 remains.
        assert_eq!(census.len(), 1);
        assert_eq!(census[0].type_code, 0x0117);
        assert_eq!(census[0].count, 1);
        assert!(second_start > 0);
    }

    #[test]
    fn a_header_without_a_valid_continuation_is_not_counted() {
        // Plausible header whose bytes_to_follow lands mid-buffer on garbage
        // that is not a header and not end-of-stream.
        let mut data = record(0x0115, 8);
        data.extend_from_slice(&[0xAA; 5]); // trailing garbage, no header

        let census = undecoded_type_code_census(&data, &[]);

        assert!(census.is_empty());
    }

    #[test]
    fn the_predicate_set_matches_the_documented_native_set() {
        assert_eq!(NATIVE_GRAPHIC_TYPE_CODES.len(), 20);
        for code in [0x0115, 0x0117, 0x0118, 0x00FF, 0x0020] {
            assert!(is_native_graphic_type_code(code));
        }
        for code in [0x0006, 0x0077, 0x0085, 0x0030, 0x0010, 0x005A] {
            assert!(!is_native_graphic_type_code(code));
        }
    }
}
