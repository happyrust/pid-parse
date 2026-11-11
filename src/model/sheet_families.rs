//! Single-source-of-truth registry for the Sheet PSM record families
//! (M3 of `docs/plans/2026-07-18-architecture-deepening-master-plan-cn.md`).
//!
//! One [`SheetRecordFamily`] row per family carries everything the
//! per-family "parallel lists" used to duplicate:
//!
//! - `streams/cluster.rs` populates [`crate::model::SheetGeometry`] by
//!   walking [`SHEET_RECORD_FAMILIES`] and calling
//!   [`SheetRecordFamily::decode_into`] instead of one hand-written
//!   decode+convert+assign block a family;
//! - the "is this sheet empty" check derives from
//!   [`SheetRecordFamily::record_count`] instead of a hand-kept
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
//! `SheetRecordSchema` note: `primitive_circle` (`igCircle2d`, PSM
//! `0x0059`) gained its registry row once real-sheet evidence landed —
//! W's `Sheet6` draws its instrument balloons with 17 top-level
//! `igCircle2d` records whose coordinates land in page space.

use super::sheet::SheetGeometry;
use crate::parsers::sheet_records::{
    decode_dependency_objects, decode_igarcs, decode_igboundaries, decode_igbspcurves,
    decode_igcircles, decode_igcomplexstrings, decode_igdimensions, decode_igellipticalarcs,
    decode_iggroups, decode_iglines, decode_iglinestrings, decode_igpoints, decode_igrectangles,
    decode_igsymbols, decode_igtextboxes, decode_smartframes,
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
    /// 14-bit PSM type code. No two rows share one.
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

/// The Sheet PSM record families, in the historical cluster-wiring
/// order (order is cosmetic: every row writes a distinct field and
/// byte-audit sorts ranges before merging).
///
/// Every row reads a type code the native reader has a class for, so one a
/// chain can hold a record of. The row that stood first until 2026-11-11,
/// `GLine2d` (`0x3FE6`), did not: its type code is past the class table,
/// its three corpus "records" were the top two bytes of a page frame's
/// `1/√2` read as a type word by a byte-sliding walk
/// (`docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`),
/// and from the day the families took the record chain it decoded nothing.
pub const SHEET_RECORD_FAMILIES: &[SheetRecordFamily] = &[
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
        name: "igArc2d",
        type_code: 0x0061,
        // No drawing sheet of the corpus carries one; the linked border
        // templates draw their logo and rounded corners with arcs, which
        // is where the emitter's 18-record evidence comes from.
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igarcs",
        model_dto: "DecodedIgArc2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igarcs = decode_igarcs(data).into_iter().map(Into::into).collect();
        },
        record_count: |geometry| geometry.decoded_igarcs.len(),
        decoded_ranges: |data| {
            decode_igarcs(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // A sheet-level circle draws directly — a full circle is a leaf
    // curve nothing else expresses. Sat decode-only until W's Sheet6
    // proved the page transform applies to them (instrument balloons).
    SheetRecordFamily {
        name: "igCircle2d",
        type_code: 0x0059,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igcircles",
        model_dto: "DecodedIgCircle2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igcircles = decode_igcircles(data)
                .into_iter()
                .map(super::DecodedIgCircle2dRecord::from)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igcircles.len(),
        decoded_ranges: |data| {
            decode_igcircles(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    SheetRecordFamily {
        name: "igBoundary2d",
        type_code: 0x0013,
        // Emits since fills landed: a ring whose style resolves to a
        // `JStyleSimpleFill` draws as a closed polyline the fill can
        // attach to (`geometry.rs`, `IgBoundary2dEmitter`). This flag
        // said `false` for the first two days of that behaviour — the
        // policy test now checks the emitters instead of a second copy
        // of this claim.
        emits_geometry: true,
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
        name: "DependencyObject",
        type_code: 0x00FA,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Probed,
        geometry_field: "decoded_dependency_objects",
        model_dto: "DecodedDependencyObjectRecord",
        decode_into: |data, geometry| {
            geometry.decoded_dependency_objects = decode_dependency_objects(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_dependency_objects.len(),
        decoded_ranges: |data| {
            decode_dependency_objects(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // Three rows stood here until 2026-11-11: `JStyleOverride` (`0x0030`),
    // `SubRecord0x0010` and `AttributeFragment` (`0x0010`). No chain of any
    // stream holds a `0x0010` record, and the `0x0030` records are in
    // `StyleCluster`, where `style_link` reads them; what the three rows
    // used to report were fields of other records read as record headers
    // by a byte-sliding walk, and since the families decode on the chain
    // they decoded nothing — except from a record a walk read off the
    // chain after a wrong length, which only they would make something of
    // (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
    // §3, §12, §17.2).

    // A rectangle is the parent of the four `igLine2d` edges it lists; the
    // edges are records of the same stream and emit on their own, so the
    // rectangle decodes (its byte range is claimed, its edges are named)
    // and emits nothing.
    SheetRecordFamily {
        name: "igRectangle2d",
        type_code: 0x0020,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igrectangles",
        model_dto: "DecodedIgRectangle2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igrectangles = decode_igrectangles(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igrectangles.len(),
        decoded_ranges: |data| {
            decode_igrectangles(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // A B-spline is a leaf curve nothing else draws; it emits as the
    // polyline `bspline::sample` makes of it.
    SheetRecordFamily {
        name: "igBspCurve2d",
        type_code: 0x005D,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igbspcurves",
        model_dto: "DecodedIgBspCurve2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igbspcurves = decode_igbspcurves(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igbspcurves.len(),
        decoded_ranges: |data| {
            decode_igbspcurves(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // A driving dimension constrains a parametric body's geometry; it is
    // not a stroke of it, and the file keeps every one on a `Dimension`
    // layer it has switched off. Decoded so it stops being a silent drop
    // and its refusals get counted, emitting nothing (plan D2).
    SheetRecordFamily {
        name: "igDimension",
        type_code: 0x0115,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igdimensions",
        model_dto: "DecodedIgDimensionRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igdimensions = decode_igdimensions(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igdimensions.len(),
        decoded_ranges: |data| {
            decode_igdimensions(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // A run of curves joined end to end. It states no coordinate: its
    // curves are `igLine2d` / `igArc2d` records of the same stream that
    // name it as their owner and carry its layer and style, so they draw
    // what it draws. Decoded so that it stops being a graphic record
    // nothing reads; emits nothing, or its members would draw twice.
    SheetRecordFamily {
        name: "igComplexString2d",
        type_code: 0x0021,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igcomplexstrings",
        model_dto: "DecodedIgComplexString2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igcomplexstrings = decode_igcomplexstrings(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igcomplexstrings.len(),
        decoded_ranges: |data| {
            decode_igcomplexstrings(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // Graphic objects that select and move as one. The corpora's groups
    // list the two `igRectangle2d` of a border, which draw through their
    // own `igLine2d` edges, or the lines, arcs, B-splines, elliptical arcs,
    // boundaries and point of a logo, which draw through their own
    // families; the group emits nothing for the reason a rectangle emits
    // nothing.
    SheetRecordFamily {
        name: "igGroup",
        type_code: 0x007B,
        emits_geometry: false,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_iggroups",
        model_dto: "DecodedIgGroupRecord",
        decode_into: |data, geometry| {
            geometry.decoded_iggroups = decode_iggroups(data).into_iter().map(Into::into).collect();
        },
        record_count: |geometry| geometry.decoded_iggroups.len(),
        decoded_ranges: |data| {
            decode_iggroups(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
    // An elliptical arc is a leaf curve, like a B-spline: nothing lists it
    // and nothing else draws it. It emits as the polyline `bspline::sample`
    // makes of the rational B-spline a cached symbol body draws for the
    // same record.
    SheetRecordFamily {
        name: "igEllipticalArc2d",
        type_code: 0x007E,
        emits_geometry: true,
        trace_class: SheetFamilyTraceClass::Decoded,
        geometry_field: "decoded_igellipticalarcs",
        model_dto: "DecodedIgEllipticalArc2dRecord",
        decode_into: |data, geometry| {
            geometry.decoded_igellipticalarcs = decode_igellipticalarcs(data)
                .into_iter()
                .map(Into::into)
                .collect();
        },
        record_count: |geometry| geometry.decoded_igellipticalarcs.len(),
        decoded_ranges: |data| {
            decode_igellipticalarcs(data)
                .into_iter()
                .map(|r| r.byte_range)
                .collect()
        },
    },
];

/// True when no family has any decoded records on `geometry`
/// (registry-derived replacement for the historical hand-kept `&&`
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
    fn registry_and_census_decoded_set_stay_in_sync() {
        use crate::parsers::undecoded_census::DECODED_TYPE_CODES;
        for family in SHEET_RECORD_FAMILIES {
            assert!(
                DECODED_TYPE_CODES.contains(&family.type_code),
                "family {} (0x{:04X}) is missing from undecoded_census::DECODED_TYPE_CODES",
                family.name,
                family.type_code
            );
        }
        for code in DECODED_TYPE_CODES {
            assert!(
                SHEET_RECORD_FAMILIES.iter().any(|f| f.type_code == code),
                "undecoded_census::DECODED_TYPE_CODES lists 0x{code:04X}, which no registry family decodes"
            );
        }
    }

    #[test]
    fn registry_has_sixteen_rows_with_unique_fields() {
        assert_eq!(SHEET_RECORD_FAMILIES.len(), 16);
        let mut fields: Vec<&str> = SHEET_RECORD_FAMILIES
            .iter()
            .map(|f| f.geometry_field)
            .collect();
        fields.sort_unstable();
        fields.dedup();
        assert_eq!(
            fields.len(),
            16,
            "every registry row must own a distinct SheetGeometry field"
        );
    }

    /// No two rows read one type code, and none reads a type code no chain
    /// holds a record of: the four families that did are retired.
    #[test]
    fn no_type_code_is_shared_and_the_retired_families_have_no_row() {
        let mut codes: Vec<u16> = SHEET_RECORD_FAMILIES.iter().map(|f| f.type_code).collect();
        codes.sort_unstable();
        let duplicates: Vec<u16> = codes
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| w[0])
            .collect();
        assert_eq!(duplicates, Vec::<u16>::new(), "one row a type code");
        for family in SHEET_RECORD_FAMILIES {
            assert!(
                ![0x0010, 0x0030, 0x3FE6].contains(&family.type_code),
                "{} reads 0x{:04X}, a type code no sheet chain holds",
                family.name,
                family.type_code
            );
            assert!(
                ![
                    "JStyleOverride",
                    "SubRecord0x0010",
                    "AttributeFragment",
                    "GLine2d"
                ]
                .contains(&family.name),
                "{} is retired",
                family.name
            );
        }
    }

    /// A family decodes where the chain walk starts a record, and the walk
    /// stops on a type word the native reader has no class for. A row whose
    /// type code the walk stops on is handed no record by any stream: that
    /// was `GLine2d` (`0x3FE6`) from the day the walk took the class table
    /// for its tripwire, and it stood in the registry for one more round.
    #[test]
    fn every_row_reads_a_type_code_a_chain_can_hold() {
        use crate::parsers::sheet_records::psm_type_word_opens_a_record;
        for family in SHEET_RECORD_FAMILIES {
            assert!(
                psm_type_word_opens_a_record(family.type_code),
                "{} reads 0x{:04X}, a type word the chain walk stops on",
                family.name,
                family.type_code
            );
        }
    }

    #[test]
    fn decode_into_and_record_count_agree_on_synthetic_igline_stream() {
        // One canonical igLine2d record (56 bytes), as in the parser unit
        // tests: header 0x0018 + btf 50 + payload. The family is chain-gated,
        // so the record has to sit behind the 8-byte Sheet stream header.
        let mut data = vec![0u8; crate::parsers::sheet_records::SHEET_STREAM_HEADER_LEN];
        data.extend_from_slice(&0x0018u16.to_le_bytes());
        data.extend_from_slice(&50u32.to_le_bytes());
        data.extend_from_slice(&7u32.to_le_bytes()); // oid
        data.extend_from_slice(&100u32.to_le_bytes()); // aux_lo (parent_ref)
        data.extend_from_slice(&12u32.to_le_bytes()); // aux_hi
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
        let start = crate::parsers::sheet_records::SHEET_STREAM_HEADER_LEN;
        assert_eq!((igline_row.record_count)(&geometry), 1);
        assert_eq!((igline_row.decoded_ranges)(&data), vec![start..start + 56]);
        assert!(!sheet_geometry_has_no_family_records(&geometry));
    }

    /// The registry's `emits_geometry` flag is a *claim* about the L6
    /// emitters; this holds it against the emitters' own statement
    /// (`geometry::emitter_families`) instead of a hand-written copy of
    /// the same list. The old form asserted the copy, so when
    /// `IgBoundary2dEmitter` started emitting fills the flag stayed
    /// `false` and the test stayed green — exactly the drift it existed
    /// to catch.
    #[test]
    fn emission_policy_matches_geometry_emitter_no_op_set() {
        let emitters = crate::geometry::emitter_families();

        // Every registry family registers exactly one emitter, and no
        // emitter serves a family the registry does not know — the L6
        // counterpart of the cluster-wiring test.
        let mut registry_names: Vec<&str> = SHEET_RECORD_FAMILIES.iter().map(|f| f.name).collect();
        registry_names.sort_unstable();
        let mut emitter_names: Vec<&str> = emitters.iter().map(|(name, _)| *name).collect();
        emitter_names.sort_unstable();
        assert_eq!(
            registry_names, emitter_names,
            "every sheet record family must register exactly one geometry emitter"
        );

        let mut registry_audit_only: Vec<&str> = SHEET_RECORD_FAMILIES
            .iter()
            .filter(|f| !f.emits_geometry)
            .map(|f| f.name)
            .collect();
        registry_audit_only.sort_unstable();
        let mut no_op_emitters: Vec<&str> = emitters
            .iter()
            .filter(|(_, is_no_op)| *is_no_op)
            .map(|(name, _)| *name)
            .collect();
        no_op_emitters.sort_unstable();
        assert_eq!(
            registry_audit_only, no_op_emitters,
            "emits_geometry must agree with the emitters themselves"
        );
    }
}
