//! Census of Sheet PSM records that never reach the drawing, split by
//! `SmartPlant`'s own graphic predicate (Phase 38 S2: silent drops become
//! named warnings).
//!
//! A record misses the drawing two ways, and this module counts both:
//!
//! - **No decoder claims the type code** —
//!   [`undecoded_type_code_census`], the Phase 38 S2 half.
//! - **A decoder exists and refused these bytes** —
//!   [`refused_record_census`], the Phase 40 half. The type code has a
//!   family, that family walked over this record, and one of its validation
//!   rules said no. The record then falls out of both the decoded output and
//!   the undecoded census, because the latter tests the *type code* rather
//!   than the *record*.
//!
//! The second half stayed silent for two phases while the first was being
//! closed, and it is the larger of the two on the corpus: 141 refused
//! records against 5 undecoded ones. Measured in
//! `docs/analysis/2026-08-10-the-silent-bucket-is-refusals-not-unknowns.md`.
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

/// One PSM type code whose family decoder walked this stream and refused
/// some of its records.
///
/// Distinct from [`UndecodedTypeCodeCount`] in what it asks the reader to
/// do: an undecoded type code needs a new decoder, a refused record needs
/// an existing decoder's rules revisited against a shape the corpus
/// actually contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedRecordCount {
    /// PSM 14-bit type code — one this crate does decode.
    pub type_code: u16,
    /// Number of chain-validated records with this code that the family's
    /// own decoder did not claim.
    pub count: usize,
    /// Whether the native graphic predicate accepts this code, i.e. whether
    /// the refusal costs the drawing strokes rather than styling.
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
    unclaimed_counts(data, claimed, |type_code| {
        !DECODED_TYPE_CODES.contains(&type_code)
    })
    .into_iter()
    .map(|(type_code, count)| UndecodedTypeCodeCount {
        type_code,
        count,
        is_graphic: is_native_graphic_type_code(type_code),
        rad_class_name: rad_class_name(type_code),
    })
    .collect()
}

/// Count the chain-validated PSM records in `data` whose type code **has** a
/// typed decoder in this crate and whose bytes that decoder still did not
/// claim — records refused by their own family's validation rules.
///
/// This is the other half of [`undecoded_type_code_census`], over the same
/// walk and the same `claimed` ranges, so a record is counted by exactly one
/// of the two. A refused record is not a mystery type code: the family is
/// wired, it saw these bytes, and a rule said no.
///
/// Naming them is what got them re-measured. 88 of the corpus's first 141 —
/// 21% of every line in the drawings, `A01`'s whole page border among them —
/// were `igLine2d` refused on a rule this crate invented for a PSM envelope
/// field the native reader discards; that rule is gone and they draw. The
/// 53 that remain are `igTextBox` and `igLineString2d`, still unexplained.
///
/// The graphic/non-graphic split is the same native predicate the undecoded
/// census uses, so consumers can apply one warn rule to both.
pub fn refused_record_census(data: &[u8], claimed: &[Range<usize>]) -> Vec<RefusedRecordCount> {
    unclaimed_counts(data, claimed, |type_code| {
        DECODED_TYPE_CODES.contains(&type_code)
    })
    .into_iter()
    .map(|(type_code, count)| RefusedRecordCount {
        type_code,
        count,
        is_graphic: is_native_graphic_type_code(type_code),
        rad_class_name: rad_class_name(type_code),
    })
    .collect()
}

/// Walk the record chain once and count, per type code, the records that no
/// `claimed` range covers and that `keep` selects.
///
/// Both censuses go through here so they cannot disagree about what a record
/// is or which ranges are claimed — the failure that let refused records fall
/// between them in the first place.
fn unclaimed_counts(
    data: &[u8],
    claimed: &[Range<usize>],
    keep: impl Fn(u16) -> bool,
) -> std::collections::BTreeMap<u16, usize> {
    let mut counts: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
    let mut off = 0usize;
    while off + 6 <= data.len() {
        if let Some(end) = psm_record_end(data, off) {
            if end == data.len() || psm_record_end(data, end).is_some() {
                let type_code = u16::from_le_bytes([data[off], data[off + 1]]) & 0x3FFF;
                let claimed_here = claimed.iter().any(|range| range.contains(&off));
                if !claimed_here && keep(type_code) {
                    *counts.entry(type_code).or_insert(0) += 1;
                }
                off = end;
                continue;
            }
        }
        off += 1;
    }
    counts
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
    fn a_refused_record_of_a_decoded_family_is_counted_as_a_refusal() {
        // An igLine2d the igLine2d decoder walked over and did not claim.
        // The undecoded census cannot see it -- it tests the type code, and
        // this type code has a decoder -- so it needs its own count or it is
        // dropped in silence.
        let data = record(0x0018, 50);

        assert!(
            undecoded_type_code_census(&data, &[]).is_empty(),
            "the undecoded census is blind to a refused record by design"
        );
        assert_eq!(
            refused_record_census(&data, &[]),
            vec![RefusedRecordCount {
                type_code: 0x0018,
                count: 1,
                is_graphic: true,
                rad_class_name: Some("Line Object"),
            }]
        );
    }

    #[test]
    fn a_claimed_record_is_neither_undecoded_nor_refused() {
        let data = record(0x0018, 50);
        let claimed: Vec<Range<usize>> = std::iter::once(0..data.len()).collect();

        assert!(refused_record_census(&data, &claimed).is_empty());
        assert!(undecoded_type_code_census(&data, &claimed).is_empty());
    }

    #[test]
    fn every_unclaimed_record_lands_in_exactly_one_census() {
        // The two censuses partition the unclaimed records: one tests "has a
        // decoder", the other "has none". A record that answered neither is
        // what went missing for two phases.
        let mut data = record(0x0018, 50); // decoded family, unclaimed
        data.extend(record(0x0115, 8)); // no decoder, graphic
        data.extend(record(0x0077, 8)); // no decoder, not graphic

        let undecoded: usize = undecoded_type_code_census(&data, &[])
            .iter()
            .map(|entry| entry.count)
            .sum();
        let refused: usize = refused_record_census(&data, &[])
            .iter()
            .map(|entry| entry.count)
            .sum();

        assert_eq!(undecoded, 2);
        assert_eq!(refused, 1);
        assert_eq!(undecoded + refused, 3, "no record is counted twice or lost");
    }

    #[test]
    fn a_refused_style_record_is_counted_but_not_graphic() {
        // A refused JStyleOverride costs the drawing styling, not strokes.
        // Same predicate as the undecoded half, so one warn rule serves both.
        let data = record(0x0030, 64);

        let census = refused_record_census(&data, &[]);

        assert_eq!(census.len(), 1);
        assert!(!census[0].is_graphic);
        assert_eq!(census[0].rad_class_name, Some("JSL Override Style"));
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
