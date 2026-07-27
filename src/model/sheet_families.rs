//! Single-source-of-truth registry for the Sheet PSM record families
//! (M3 of `docs/plans/2026-07-18-architecture-deepening-master-plan-cn.md`).
//!
//! One [`SheetRecordFamily`] row per family carries everything the
//! per-family "parallel lists" used to duplicate:
//!
//! - `streams/cluster.rs` populates [`crate::model::SheetGeometry`] by
//!   walking [`SHEET_RECORD_FAMILIES`] and calling
//!   [`SheetRecordFamily::decode_into`] instead of 11 hand-written
//!   decode+convert+assign blocks;
//! - the "is this sheet empty" check derives from
//!   [`SheetRecordFamily::record_count`] instead of an 11-conjunct
//!   `&&` chain;
//! - `byte_audit/aggregate.rs` traces decoded byte envelopes from
//!   [`SheetRecordFamily::decoded_ranges`] +
//!   [`SheetRecordFamily::trace_class`];
//! - `schema.rs` derives its per-family needle assertions from
//!   [`SheetRecordFamily::model_dto`] /
//!   [`SheetRecordFamily::geometry_field`];
//! - `tests/sheet_family_wiring.rs` asserts, per fixture sheet, that
//!   the populated [`crate::model::SheetGeometry`] field and a fresh
//!   re-decode of the raw bytes agree — closing the historical
//!   "decoder exists but cluster wiring forgot it" silent gap.
//!
//! **Not** in the registry: the L4 decode logic (lives behind
//! `parsers::sheet_records::PsmRecordDecoder`) and the L6 emission
//! logic (lives behind `geometry.rs`'s `GeometryEmitter` /
//! `EMITTERS`). The registry only owns cross-cutting wiring facts.
//!
//! `SheetRecordSchema` note: the schema contract additionally lists
//! `primitive_circle` (`igCircle2d`, PSM `0x0059`) as a **planned**
//! kind with no registry row — per ADR-0003 it only gains a decoder
//! (and therefore a row here) once the Phase 35-B semantic evidence
//! gate passes.

use super::sheet::SheetGeometry;
use crate::parsers::sheet_records::{
    decode_attribute_fragments, decode_graphic_groups, decode_igboundaries, decode_iglines,
    decode_iglinestrings, decode_igpoints, decode_igsymbols, decode_igtextboxes,
    decode_jstyle_overrides, decode_primitive_lines, decode_smartframes, decode_sub_records_0x0010,
};

/// Byte-audit claim class for a family's decoded byte envelopes.
///
/// Defined here (not in [`crate::byte_audit`]) so the model layer does
/// not depend on the audit layer; `byte_audit/aggregate.rs` maps this
/// onto its own `TraceConfidence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetFamilyTraceClass {
    /// Every payload byte is field-named (typed layout) — claimed as
    /// `Decoded` even when the family is audit-only for geometry
    /// purposes (e.g. `igBoundary2d`).
    Decoded,
    /// Byte envelope is bounded but business semantics remain guarded —
    /// claimed as `Probed`.
    Probed,
}

/// One Sheet PSM record family's cross-cutting wiring facts.
///
/// The function pointers deliberately close over nothing: each row is
/// a plain `const` table entry, testable in isolation and free of
/// hidden state.
pub struct SheetRecordFamily {
    /// Family name as used in analysis docs (`"igLine2d"`, …).
    pub name: &'static str,
    /// 14-bit PSM type code. Note `0x0010` appears twice: the raw
    /// audit collection and the attribute-fragment view are two rows
    /// over the same envelope by design.
    pub type_code: u16,
    /// Whether the family's `GeometryEmitter` produces
    /// `PidGraphicEntity` values (`false` = registered no-op emitter,
    /// audit-only policy).
    pub emits_geometry: bool,
    /// Byte-audit claim class (see [`SheetFamilyTraceClass`]).
    pub trace_class: SheetFamilyTraceClass,
    /// `SheetGeometry` field name carrying this family's records
    /// (schema needle + diagnostics).
    pub geometry_field: &'static str,
    /// Model DTO type name for this family (schema needle).
    pub model_dto: &'static str,
    /// Decode `data` and assign the converted records onto the
    /// family's [`SheetGeometry`] field.
    pub decode_into: fn(&[u8], &mut SheetGeometry),
    /// Number of records currently on the family's [`SheetGeometry`]
    /// field.
    pub record_count: fn(&SheetGeometry) -> usize,
    /// Re-decode `data` and return each accepted record's full byte
    /// range (used by byte-audit tracing and the wiring-consistency
    /// test).
    pub decoded_ranges: fn(&[u8]) -> Vec<core::ops::Range<usize>>,
}

/// The 11 Sheet PSM record families, in the historical cluster-wiring
/// order (order is cosmetic: every row writes a distinct field and
/// byte-audit sorts ranges before merging).
pub const SHEET_RECORD_FAMILIES: &[SheetRecordFamily] = &[
    SheetRecordFamily {
        name: "GLine2d",
        type_code: 0x3FE6,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_primitive_lines",
        model_dto: "DecodedPrimitiveLineRecord",
        decode_into: |data, geometry| {
            geometry.decoded_primitive_lines = decode_primitive_lines(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_primitive_lines.len(),
        decoded_ranges: |data| {
            decode_primitive_lines(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igLine2d",
        type_code: 0x0018,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_iglines",
        model_dto: "DecodedIgLine2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_iglines = decode_iglines(data).into_iter().map(Into::into).collect();
        },
        record_count: |geometry| geometry.decoded_iglines.len(),
        decoded_ranges: |data| {
            decode_iglines(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igLineString2d",
        type_code: 0x0084,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_iglinestrings",
        model_dto: "DecodedIgLineString2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_iglinestrings = decode_iglinestrings(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_iglinestrings.len(),
        decoded_ranges: |data| {
            decode_iglinestrings(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igPoint2d",
        type_code: 0x005E,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igpoints",
        model_dto: "DecodedIgPoint2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igpoints = decode_igpoints(data).into_iter().map(Into::into).collect();
        },
        record_count: |geometry| geometry.decoded_igpoints.len(),
        decoded_ranges: |data| {
            decode_igpoints(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igTextBox",
        type_code: 0x004D,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igtextboxes",
        model_dto: "DecodedIgTextBoxRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igtextboxes = decode_igtextboxes(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igtextboxes.len(),
        decoded_ranges: |data| {
            decode_igtextboxes(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igSymbol2d",
        type_code: 0x00CE,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igsymbols",
        model_dto: "DecodedIgSymbol2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igsymbols =
                decode_igsymbols(data).into_iter().map(Into::into).collect();
        },
        record_count: |geometry| geometry.decoded_igsymbols.len(),
        decoded_ranges: |data| {
            decode_igsymbols(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igBoundary2d",
        type_code: 0x0013,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igboundaries",
        model_dto: "DecodedIgBoundary2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igboundaries = decode_igboundaries(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igboundaries.len(),
        decoded_ranges: |data| {
            decode_igboundaries(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igSmartFrame2d",
        type_code: 0x003D,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igsmartframes",
        model_dto: "DecodedIgSmartFrame2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igsmartframes = decode_smartframes(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igsmartframes.len(),
        decoded_ranges: |data| {
            decode_smartframes(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "GraphicGroup",
        type_code: 0x00FA,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Probed,
        geometry_field: "decoded_graphic_groups",
        model_dto: "DecodedGraphicGroupRecord",
        decode_into: |data, geometry| {
            geometry.decoded_graphic_groups = decode_graphic_groups(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_graphic_groups.len(),
        decoded_ranges: |data| {
            decode_graphic_groups(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "JStyleOverride",
        type_code: 0x0030,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_jstyle_overrides",
        model_dto: "DecodedJStyleOverrideRecord",
        decode_into: |data, geometry| {
            geometry.decoded_jstyle_overrides = decode_jstyle_overrides(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_jstyle_overrides.len(),
        decoded_ranges: |data| {
            decode_jstyle_overrides(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "SubRecord0x0010",
        type_code: 0x0010,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Probed,
        geometry_field: "decoded_sub_records_0x0010",
        model_dto: "DecodedSubRecord0x0010Record",
        decode_into: |data, geometry| {
            geometry.decoded_sub_records_0x0010 = decode_sub_records_0x0010(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_sub_records_0x0010.len(),
        decoded_ranges: |data| {
            decode_sub_records_0x0010(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // Attribute fragments are a second, stricter view over the same
    // `0x0010` envelope: every fragment byte range coincides with a
    // raw `SubRecord0x0010` range, so its `Probed` byte-audit claims
    // merge into the raw row's claims without changing coverage
    // totals — the row exists for wiring/schema completeness.
    SheetRecordFamily {
        name: "AttributeFragment",
        type_code: 0x0010,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Probed,
        geometry_field: "decoded_attribute_fragments",
        model_dto: "DecodedAttributeFragment",
        decode_into: |data, geometry| {
            geometry.decoded_attribute_fragments = decode_attribute_fragments(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_attribute_fragments.len(),
        decoded_ranges: |data| {
            decode_attribute_fragments(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
];

/// True when no family has any decoded records on `geometry`
/// (registry-derived replacement for the historical 11-conjunct `&&`
/// chain in `streams/cluster.rs`).
pub fn sheet_geometry_has_no_family_records(geometry: &SheetGeometry) -> bool {
    SHEET_RECORD_FAMILIES
        .iter()
        .all(|family| (family.record_count)(geometry) == 0)
}

/// Populate every family's `SheetGeometry` field from `data` in one
/// registry walk.
pub fn decode_all_families_into(data: &[u8], geometry: &mut SheetGeometry) {
    for family in SHEET_RECORD_FAMILIES {
        (family.decode_into)(data, geometry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_twelve_rows_with_unique_fields() {
        assert_eq!(SHEET_RECORD_FAMILIES.len(), 12);
        let mut fields: Vec<&str> = SHEET_RECORD_FAMILIES
            .iter()
            .map(|f| f.geometry_field)
            .collect();
        fields.sort_unstable();
        fields.dedup();
        assert_eq!(
            fields.len(),
            12,
            "every registry row must own a distinct SheetGeometry field"
        );
    }

    #[test]
    fn only_the_0x0010_envelope_is_shared_between_rows() {
        let mut codes: Vec<u16> = SHEET_RECORD_FAMILIES.iter().map(|f| f.type_code).collect();
        codes.sort_unstable();
        let duplicates: Vec<u16> = codes
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| w[0])
            .collect();
        assert_eq!(
            duplicates,
            vec![0x0010],
            "0x0010 (raw + attribute-fragment view) is the only sanctioned duplicate"
        );
    }

    #[test]
    fn decode_into_and_record_count_agree_on_synthetic_igline_stream() {
        // One canonical igLine2d record (56 bytes), as in the parser
        // unit tests: header 0x0018 + btf 50 + payload.
        let mut data = Vec::new();
        data.extend_from_slice(&0x0018u16.to_le_bytes());
        data.extend_from_slice(&50u32.to_le_bytes());
        data.extend_from_slice(&7u32.to_le_bytes()); // oid
        data.extend_from_slice(&100u32.to_le_bytes()); // parent_ref
        data.extend_from_slice(&12u32.to_le_bytes()); // remaining_header
        data.extend_from_slice(&0x10u16.to_le_bytes()); // sub_type_word
        data.extend_from_slice(&1u32.to_le_bytes()); // index
        for v in [0.1f64, 0.1, 0.2, 0.1] {
            data.extend_from_slice(&v.to_le_bytes());
        }

        let mut geometry = SheetGeometry::default();
        assert!(sheet_geometry_has_no_family_records(&geometry));

        decode_all_families_into(&data, &mut geometry);

        let igline_row = SHEET_RECORD_FAMILIES
            .iter()
            .find(|f| f.name == "igLine2d")
            .expect("registry row");
        assert_eq!((igline_row.record_count)(&geometry), 1);
        assert_eq!((igline_row.decoded_ranges)(&data), vec![0..56]);
        assert!(!sheet_geometry_has_no_family_records(&geometry));
    }

    #[test]
    fn emission_policy_matches_geometry_emitter_no_op_set() {
        let audit_only: Vec<&str> = SHEET_RECORD_FAMILIES
            .iter()
            .filter(|f| !f.emits_geometry)
            .map(|f| f.name)
            .collect();
        assert_eq!(
            audit_only,
            vec![
                "igBoundary2d",
                "igSmartFrame2d",
                "GraphicGroup",
                "SubRecord0x0010",
                "AttributeFragment"
            ],
            "audit-only set must match the no-op emitters in geometry.rs EMITTERS"
        );
    }
}
