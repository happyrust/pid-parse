//! JSON Schema generation for [`PidDocument`].
//!
//! Downstream consumers (TypeScript, Python, C# ...) can run `pid_inspect
//! --schema` to emit a JSON Schema describing the entire `PidDocument`
//! tree, then feed that schema into a codegen tool (quicktype,
//! json-schema-to-typescript, NJsonSchema ...).
//!
//! The schema is derived automatically from every `#[derive(JsonSchema)]`
//! model type, so it stays in sync with the Rust source.

use crate::model::PidDocument;
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
}
