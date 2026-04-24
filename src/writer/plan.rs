//! Declarative write plan consumed by [`crate::writer::PidWriter::write_to`].
//!
//! A [`WritePlan`] describes **what** should change in a [`PidPackage`]
//! before it gets serialized back to CFB. The writer pipeline is:
//!
//! 1. Apply [`MetadataUpdates`] (drawing XML / general XML / future summary)
//! 2. Apply every [`StreamReplacement`] verbatim
//! 3. Apply every [`SheetPatch`] byte-range (experimental, no semantic checks)
//! 4. Write the resulting stream map to a new CFB container
//!
//! Each field is optional / may be empty; a `WritePlan::default()` is a
//! pure passthrough that re-serializes the package unchanged.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Transparent serde adaptor that encodes `Vec<u8>` as a base64 string in
/// JSON (and any other human-readable format) while leaving the Rust-side
/// type untouched. Used on fields that otherwise explode to multi-MB arrays
/// when written out of a JSON plan (e.g. [`StreamReplacement::new_data`]).
mod bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        STANDARD.encode(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(&encoded).map_err(serde::de::Error::custom)
    }
}

/// Top-level write plan. An empty plan (`WritePlan::default()`) is a valid
/// passthrough request.
///
/// Deserialization is tolerant of missing fields: `{}` in JSON is the
/// passthrough plan, `{"metadata_updates": {"drawing_xml": "..."}}` is a
/// metadata-only update. This keeps hand-written plan.json files short.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WritePlan {
    /// Targeted metadata edits. See [`MetadataUpdates`] for supported
    /// streams.
    #[serde(default)]
    pub metadata_updates: MetadataUpdates,
    /// Verbatim stream replacements applied after metadata updates. Paths
    /// are normalized (leading `/`, `/` separator) by the writer.
    #[serde(default)]
    pub stream_replacements: Vec<StreamReplacement>,
    /// Experimental byte-range patches for Sheet streams. No semantic
    /// validation — the caller is responsible for producing a byte-valid
    /// Sheet body.
    #[serde(default)]
    pub sheet_patches: Vec<SheetPatch>,
}

/// Narrow metadata channel for the common "edit drawing number / project"
/// flow without hand-crafting byte replacements. Missing fields default to
/// "untouched" / "empty" so JSON plans can omit them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataUpdates {
    /// New XML body for `/TaggedTxtData/Drawing`. `None` = untouched.
    #[serde(default)]
    pub drawing_xml: Option<String>,
    /// New XML body for `/TaggedTxtData/General`. `None` = untouched.
    #[serde(default)]
    pub general_xml: Option<String>,
    /// OLE property-set edits for `/\x05SummaryInformation` and
    /// `/\x05DocumentSummaryInformation`. Keys are symbolic property names
    /// (`"title"`, `"author"`, `"subject"`, `"keywords"`, `"comments"`,
    /// `"template"`, `"last_author"`, `"rev_number"`, `"app_name"`,
    /// `"category"`, `"manager"`, `"company"`). Values are plain Rust
    /// strings; encoding (`VT_LPSTR` vs `VT_LPWSTR`) is preserved from the
    /// source property or defaulted to `VT_LPWSTR` for newly added ones.
    /// Phase 10g (v0.7.0+): `VT_LPSTR` values are encoded as UTF-8 bytes,
    /// so non-ASCII is accepted. See [`crate::writer::summary_write`] for
    /// the full key table and error semantics. Empty map = free no-op.
    #[serde(default)]
    pub summary_updates: BTreeMap<String, String>,

    /// Phase 9n (v0.5.2+): symbolic-keyed OLE property deletions. Each
    /// entry names a property (same symbolic table as `summary_updates`)
    /// that will be removed from its section before any updates are
    /// applied. Deleting a key that does not currently exist in the
    /// source property-set is a silent no-op — only unknown symbolic keys
    /// return an error. A key appearing in both `summary_deletions` and
    /// `summary_updates` is rejected (ambiguous intent). Empty vec = no-op.
    #[serde(default)]
    pub summary_deletions: Vec<String>,

    /// Phase 10i (v0.8.0+): code-page-aware OLE property updates. Same
    /// symbolic-key table as `summary_updates`; each value carries an
    /// explicit `encoding_rs` label applied to `VT_LPSTR` properties.
    /// `VT_LPWSTR` targets ignore the encoding hint (UTF-16LE is
    /// unambiguous). A key appearing in both `summary_updates` and
    /// `summary_updates_encoded` is rejected (ambiguous intent). See
    /// [`EncodedString`] for the JSON shape. Empty map = no-op.
    #[serde(default)]
    pub summary_updates_encoded: BTreeMap<String, EncodedString>,
}

/// Phase 10i (v0.8.0+): explicit code-page-aware string value used by
/// [`MetadataUpdates::summary_updates_encoded`].
///
/// `encoding` is a label understood by the `encoding_rs` crate
/// (e.g. `"UTF-8"`, `"windows-1252"`, `"GBK"`, `"Shift_JIS"`). At apply
/// time, encoding an unrepresentable character fails fast with
/// [`crate::error::PidError::ParseFailure`] rather than silently
/// producing replacement characters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedString {
    /// Raw Unicode value (what the caller wants the user to see).
    pub value: String,
    /// `encoding_rs` label. Case-insensitive; `"windows-1252"` / `"WINDOWS-1252"`
    /// / `"CP1252"` all work. Invalid labels fail at apply time.
    pub encoding: String,
}

impl EncodedString {
    /// Convenience constructor: `EncodedString::new("X", "windows-1252")`.
    pub fn new(value: impl Into<String>, encoding: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            encoding: encoding.into(),
        }
    }
}

/// Replace (or insert) a single CFB stream with the provided bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamReplacement {
    /// Absolute path inside the CFB (e.g. `/TaggedTxtData/Drawing`).
    pub path: String,
    /// Raw bytes to write. Serialized as a standard base64 string in JSON
    /// plans; round-trips losslessly.
    #[serde(with = "bytes_base64")]
    pub new_data: Vec<u8>,
}

/// Experimental byte-range patch for a Sheet stream. Not semantically
/// validated; callers opt into risk via `experimental = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPatch {
    /// Absolute sheet stream path, e.g. `/Sheet6`.
    pub sheet_path: String,
    /// Ordered list of byte-range edits. Patches may overlap across calls,
    /// but the writer applies them in descending-`start` order within a
    /// single patch to keep earlier offsets stable.
    pub chunk_patches: Vec<SheetChunkPatch>,
    /// Must be `true` in the current release. Present for future
    /// validation hooks.
    pub experimental: bool,
}

/// Half-open byte range `[start, end)` replaced by `replacement`. A
/// `start == end` patch is a pure insertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetChunkPatch {
    /// Inclusive start byte offset inside the target sheet stream.
    pub start: usize,
    /// Exclusive end byte offset inside the target sheet stream.
    pub end: usize,
    /// Patch bytes. Serialized as standard base64 in JSON plans.
    #[serde(with = "bytes_base64")]
    pub replacement: Vec<u8>,
}

impl WritePlan {
    /// Convenience constructor for the metadata-only flow: only the two
    /// `/TaggedTxtData/*` XML streams are touched.
    pub fn metadata_only(drawing_xml: Option<String>, general_xml: Option<String>) -> Self {
        Self {
            metadata_updates: MetadataUpdates {
                drawing_xml,
                general_xml,
                summary_updates: BTreeMap::new(),
                summary_deletions: Vec::new(),
                summary_updates_encoded: BTreeMap::new(),
            },
            ..Self::default()
        }
    }

    /// `true` iff the plan would not change any stream (a passthrough).
    pub fn is_passthrough(&self) -> bool {
        self.metadata_updates.drawing_xml.is_none()
            && self.metadata_updates.general_xml.is_none()
            && self.metadata_updates.summary_updates.is_empty()
            && self.metadata_updates.summary_deletions.is_empty()
            && self.metadata_updates.summary_updates_encoded.is_empty()
            && self.stream_replacements.is_empty()
            && self.sheet_patches.is_empty()
    }

    /// Phase 9o (v0.5.3+): parse a JSON string into a [`WritePlan`].
    /// Errors are wrapped into [`PidError::ParseFailure`] so callers
    /// don't have to deal with `serde_json::Error` directly.
    pub fn from_json(json: &str) -> Result<Self, crate::error::PidError> {
        serde_json::from_str(json).map_err(|e| crate::error::PidError::ParseFailure {
            context: "WritePlan JSON".into(),
            message: e.to_string(),
        })
    }

    /// Phase 9o (v0.5.3+): serialize the plan to a compact JSON string.
    /// Uses the base64-encoded wire format for `Vec<u8>` payloads
    /// (`stream_replacements[*].new_data`,
    /// `sheet_patches[*].chunk_patches[*].replacement`) as of Phase 9k.
    pub fn to_json(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "WritePlan serialization".into(),
            message: e.to_string(),
        })
    }

    /// Phase 9o (v0.5.3+): same as [`to_json`] but with pretty-printed
    /// output (2-space indent, one field per line) — convenient for
    /// hand-authored plan.json files under version control.
    pub fn to_json_pretty(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string_pretty(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "WritePlan serialization".into(),
            message: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plan_is_passthrough() {
        assert!(WritePlan::default().is_passthrough());
    }

    #[test]
    fn metadata_only_sets_only_xml_fields() {
        let plan = WritePlan::metadata_only(Some("<Drawing/>".into()), None);
        assert!(!plan.is_passthrough());
        assert_eq!(
            plan.metadata_updates.drawing_xml.as_deref(),
            Some("<Drawing/>")
        );
        assert!(plan.metadata_updates.general_xml.is_none());
        assert!(plan.stream_replacements.is_empty());
        assert!(plan.sheet_patches.is_empty());
    }

    #[test]
    fn stream_replacement_round_trips_through_json_with_base64_payload() {
        let original = StreamReplacement {
            path: "/TaggedTxtData/Drawing".into(),
            new_data: vec![0x00, 0x01, 0x02, 0xFF, b'A', b'B', b'C'],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        // Base64 of [0x00, 0x01, 0x02, 0xFF, 'A', 'B', 'C'] = "AAEC/0FCQw=="
        assert!(
            json.contains("\"AAEC/0FCQw==\""),
            "expected base64 payload in JSON; got: {json}"
        );
        let decoded: StreamReplacement =
            serde_json::from_str(&json).expect("deserialize base64 back to bytes");
        assert_eq!(decoded.path, original.path);
        assert_eq!(decoded.new_data, original.new_data);
    }

    #[test]
    fn sheet_chunk_patch_round_trips_through_json_with_base64_payload() {
        let original = SheetChunkPatch {
            start: 16,
            end: 20,
            replacement: b"wxyz".to_vec(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        assert!(json.contains("\"d3h5eg==\""), "JSON = {json}");
        let decoded: SheetChunkPatch = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.start, 16);
        assert_eq!(decoded.end, 20);
        assert_eq!(decoded.replacement, b"wxyz".to_vec());
    }

    #[test]
    fn deserialize_rejects_invalid_base64() {
        let json = r#"{"path":"/x","new_data":"!!!not-valid-base64!!!"}"#;
        let err =
            serde_json::from_str::<StreamReplacement>(json).expect_err("must reject bad base64");
        assert!(
            err.to_string().to_lowercase().contains("invalid")
                || err.to_string().to_lowercase().contains("symbol"),
            "unexpected err: {err}"
        );
    }

    // ------------------------------------------------------------------
    // Phase 9o: WritePlan::from_json / to_json / to_json_pretty
    // ------------------------------------------------------------------

    #[test]
    fn plan_json_round_trip_default_is_passthrough() {
        let original = WritePlan::default();
        let json = original.to_json().expect("to_json");
        let restored = WritePlan::from_json(&json).expect("from_json");
        assert!(restored.is_passthrough(), "round-trip default → default");
    }

    #[test]
    fn plan_from_json_rejects_invalid_syntax_with_pid_error() {
        let err = WritePlan::from_json("not json at all").expect_err("reject");
        let msg = format!("{err}");
        assert!(msg.contains("WritePlan JSON"), "context in error: {msg}");
    }

    #[test]
    fn plan_to_json_pretty_contains_newlines_and_indent() {
        let plan = WritePlan::metadata_only(
            Some("<Drawing><DrawingNumber>X</DrawingNumber></Drawing>".into()),
            None,
        );
        let pretty = plan.to_json_pretty().expect("pretty");
        assert!(pretty.contains('\n'), "pretty output should be multi-line");
        assert!(pretty.contains("  \""), "pretty output should be indented");
    }

    #[test]
    fn plan_from_json_empty_object_is_valid_passthrough() {
        let plan = WritePlan::from_json("{}").expect("{} is valid");
        assert!(
            plan.is_passthrough(),
            "all #[serde(default)] fields should yield passthrough"
        );
    }

    // ------------------------------------------------------------------
    // Phase 10i: EncodedString + summary_updates_encoded
    // ------------------------------------------------------------------

    #[test]
    fn encoded_string_serializes_to_object_with_value_and_encoding() {
        let es = EncodedString::new("Ø Pipe", "windows-1252");
        let json = serde_json::to_string(&es).expect("serialize");
        assert!(
            json.contains("\"value\":\"Ø Pipe\""),
            "expected value field: {json}"
        );
        assert!(
            json.contains("\"encoding\":\"windows-1252\""),
            "expected encoding field: {json}"
        );
    }

    #[test]
    fn encoded_string_round_trips_through_json() {
        let es = EncodedString::new("中文 标题", "GBK");
        let json = serde_json::to_string(&es).expect("serialize");
        let back: EncodedString = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, es);
    }

    #[test]
    fn plan_with_summary_updates_encoded_is_not_passthrough() {
        let plan = WritePlan::from_json(
            r#"{"metadata_updates":{"summary_updates_encoded":
               {"title":{"value":"X","encoding":"windows-1252"}}}}"#,
        )
        .expect("parse");
        assert!(!plan.is_passthrough());
        assert_eq!(
            plan.metadata_updates.summary_updates_encoded.get("title"),
            Some(&EncodedString::new("X", "windows-1252"))
        );
    }

    #[test]
    fn plan_omitting_summary_updates_encoded_is_passthrough_compatible() {
        // JSON from v0.7.x consumer that never heard of 10i stays valid
        // and still passes through is_passthrough checks.
        let json = r#"{"metadata_updates":{"drawing_xml":null,"general_xml":null,
                     "summary_updates":{},"summary_deletions":[]}}"#;
        let plan = WritePlan::from_json(json).expect("backward compat");
        assert!(plan.is_passthrough());
        assert!(plan.metadata_updates.summary_updates_encoded.is_empty());
    }
}
