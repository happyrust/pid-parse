//! JSON Schema generation for [`PidDocument`].
//!
//! Downstream consumers (TypeScript, Python, C# ...) can run `pid_inspect
//! --schema` to emit a JSON Schema describing the entire `PidDocument`
//! tree, then feed that schema into a codegen tool (quicktype,
//! json-schema-to-typescript, `NJsonSchema` ...).
//!
//! The schema is derived automatically from every `#[derive(JsonSchema)]`
//! model type, so it stays in sync with the Rust source.

use crate::{geometry::NormalizedPidGeometry, model::PidDocument};
use schemars::Schema;

/// Produce the JSON Schema for a `PidDocument`.
///
/// Returns a [`schemars::Schema`] that can be serialized with
/// `serde_json::to_string_pretty` to obtain standard JSON Schema text.
pub fn pid_document_schema() -> Schema {
    schemars::schema_for!(PidDocument)
}

/// Convenience: produce the schema as a pretty-printed JSON string.
///
/// Equivalent to `serde_json::to_string_pretty(&pid_document_schema())`
/// but returns the result already typed as `Result<String, _>`.
pub fn pid_document_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&pid_document_schema())
}

/// Produce the JSON Schema for [`NormalizedPidGeometry`].
///
/// This schema is separate from [`pid_document_schema`] because normalized
/// render geometry is a derived projection rather than a stored field on
/// [`PidDocument`].
pub fn normalized_geometry_schema() -> Schema {
    schemars::schema_for!(NormalizedPidGeometry)
}

/// Convenience: produce the normalized geometry schema as pretty JSON.
pub fn normalized_geometry_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&normalized_geometry_schema())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_serializable_and_well_formed() {
        let schema = pid_document_schema();
        let json = serde_json::to_string(&schema).expect("serializes to JSON");
        // Re-parse to confirm it's syntactically valid JSON.
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("re-parse generated schema");
        let obj = parsed.as_object().expect("root schema is an object");
        // A well-formed JSON Schema must declare the meta-schema and a type.
        assert!(
            obj.contains_key("$schema") || obj.contains_key("$ref") || obj.contains_key("title"),
            "expected schema to carry $schema / $ref / title, got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn schema_pretty_is_non_empty_and_mentions_core_types() {
        let text = pid_document_schema_pretty().expect("pretty JSON");
        assert!(!text.is_empty());
        // All core decoded views should appear by name somewhere in the
        // schema (either as a property or a $defs entry).
        for needle in [
            "PidDocument",
            "SummaryInfo",
            "DrawingMeta",
            "JSite",
            "ClusterInfo",
            "DynamicAttributesBlob",
            "SheetStream",
            "ObjectGraph",
            "CrossReferenceGraph",
        ] {
            assert!(
                text.contains(needle),
                "schema should mention `{}` but did not; output starts with:\n{}",
                needle,
                &text[..text.len().min(500)]
            );
        }
    }

    #[test]
    fn schema_defines_attribute_value_variants() {
        let text = pid_document_schema_pretty().expect("pretty JSON");
        // `AttributeValue` is an untagged enum of Text / Integer / Float /
        // Empty; schemars emits it as a oneOf / anyOf.
        assert!(
            text.contains("AttributeValue"),
            "schema should emit AttributeValue definition"
        );
    }

    #[test]
    fn schema_exposes_psm_segment_owner_candidates() {
        let text = pid_document_schema_pretty().expect("pretty JSON");
        for needle in [
            "PsmSegmentEntry",
            "candidate_owner_cluster_index",
            "candidate_owner_cluster_name",
        ] {
            assert!(
                text.contains(needle),
                "schema should mention `{needle}` but did not; output starts with:\n{}",
                &text[..text.len().min(500)]
            );
        }
    }

    #[test]
    fn schema_exposes_sheet_geometry_dtos() {
        let text = pid_document_schema_pretty().expect("pretty JSON");
        for needle in [
            "SheetGeometry",
            "SheetText",
            "SheetEndpoint",
            "SheetObjectGeometryHint",
            // Phase 14 Slice D-H: PSM-decoded primitive DTOs land in
            // the public schema alongside probe-level DTOs so JSON
            // consumers can deserialize the `decoded_primitive_*`
            // fields without manual schema patching.
            "DecodedPrimitiveLineRecord",
            "DecodedPrimitiveArcRecord",
            "decoded_primitive_lines",
            "decoded_primitive_arcs",
            // Phase 14 Slice H: GArc2d corrected field names
            // (axis_a + axis_ratio + sweep_direction + sweep_*).
            "axis_a_x",
            "axis_ratio",
            "sweep_direction",
            "sweep_start_angle",
            "sweep_end_angle",
            // Phase 14 Slice J: igLine2d (PSM 0x0018) DTO.
            "DecodedIgLine2dRecord",
            "decoded_iglines",
            "parent_ref",
            "sub_type_word",
            "start_x",
            "end_x",
            // Phase 14 Slice K: igLineString2d (PSM 0x0084) polyline DTO.
            "DecodedIgLineString2dRecord",
            "decoded_iglinestrings",
            "vertex_xs",
            "vertex_ys",
        ] {
            assert!(
                text.contains(needle),
                "schema should mention `{needle}` but did not; output starts with:\n{}",
                &text[..text.len().min(500)]
            );
        }
    }

    #[test]
    fn schema_exposes_sheet_record_contract_entries() {
        let text = pid_document_schema_pretty().expect("pretty JSON");
        for needle in [
            "sheet_record_schema",
            "SheetRecordSchema",
            "SheetRecordSchemaEntry",
            "SheetRecordKind",
            "SheetRecordSchemaStatus",
            "SheetRecordProvenanceContract",
            "SheetDecodedGeometryKind",
            "decoded_geometry_kinds",
            "provenance",
            "primitive_line",
            "primitive_polyline",
            "primitive_circle",
            "primitive_arc",
            "symbol_placement",
            "text_placement_style",
            "endpoint_pair",
            "coordinate_page_metadata",
            "unknown",
        ] {
            assert!(
                text.contains(needle),
                "schema should mention `{needle}` but did not; output starts with:\n{}",
                &text[..text.len().min(500)]
            );
        }
    }

    #[test]
    fn normalized_geometry_schema_exposes_graphic_contract() {
        let text = normalized_geometry_schema_pretty().expect("pretty JSON");
        for needle in [
            "NormalizedPidGeometry",
            "PidGraphicEntity",
            "PidGraphicKind",
            "Point",
            "coordinate_context",
            "PidCoordinateContext",
            "PidCoordinateSpace",
            "PidDrawingUnits",
            "PidPageTransform",
            "PidPageBounds",
            "source_sheet",
            "unknown",
            "unavailable",
            "PidGraphicProvenance",
            "PidGeometryConfidence",
            "record_kind",
            "SheetRecordKind",
            "primitive_line",
            "symbol_placement",
            "text_placement_style",
        ] {
            assert!(
                text.contains(needle),
                "geometry schema should mention `{needle}` but did not; output starts with:\n{}",
                &text[..text.len().min(500)]
            );
        }
    }

    #[test]
    fn default_sheet_record_contract_maps_all_decoded_geometry_kinds() {
        use crate::model::{SheetDecodedGeometryKind, SheetRecordKind, SheetRecordSchemaStatus};

        let schema = crate::model::PidDocument::default().sheet_record_schema;
        let mappings: Vec<_> = schema
            .entries
            .iter()
            .flat_map(|entry| {
                entry
                    .decoded_geometry_kinds
                    .iter()
                    .map(move |kind| (*kind, entry.kind, entry.status))
            })
            .collect();

        for (geometry_kind, record_kind) in [
            (
                SheetDecodedGeometryKind::Line,
                SheetRecordKind::PrimitiveLine,
            ),
            (
                SheetDecodedGeometryKind::Polyline,
                SheetRecordKind::PrimitivePolyline,
            ),
            (
                SheetDecodedGeometryKind::Circle,
                SheetRecordKind::PrimitiveCircle,
            ),
            (SheetDecodedGeometryKind::Arc, SheetRecordKind::PrimitiveArc),
            (
                SheetDecodedGeometryKind::Text,
                SheetRecordKind::TextPlacementStyle,
            ),
            (
                SheetDecodedGeometryKind::SymbolInstance,
                SheetRecordKind::SymbolPlacement,
            ),
        ] {
            assert!(
                mappings.iter().any(|(actual_geometry_kind, actual_record_kind, status)| {
                    *actual_geometry_kind == geometry_kind
                        && *actual_record_kind == record_kind
                        && *status == SheetRecordSchemaStatus::Typed
                }),
                "missing typed Sheet record schema entry for {geometry_kind:?} -> {record_kind:?}; mappings={mappings:?}"
            );
        }
    }
}
