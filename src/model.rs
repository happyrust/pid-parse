use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidDocument {
    pub cfb_tree: StorageNode,
    pub streams: Vec<StreamEntry>,

    pub summary: Option<SummaryInfo>,
    pub drawing_meta: Option<DrawingMeta>,
    pub general_meta: Option<GeneralMeta>,

    pub jsites: Vec<JSite>,
    pub clusters: Vec<ClusterInfo>,

    pub dynamic_attributes: Option<DynamicAttributesBlob>,
    pub sheet_streams: Vec<SheetStream>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_roots: Option<PsmRoots>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_cluster_table: Option<PsmClusterTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_segment_table: Option<PsmSegmentTable>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_history: Option<VersionHistory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_object_registry: Option<AppObjectRegistry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagged_storages: Option<TaggedTextStorageList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_version2: Option<DocVersion2Raw>,

    /// Structured decoding of `/DocVersion2` (v0.3.8+). Present only when
    /// the stream matches the known layout (magic `0x0001_0034` + N ×
    /// 9-byte records); `doc_version2` raw is always populated in
    /// parallel for audit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_version2_decoded: Option<DocVersion2>,

    pub unknown_streams: Vec<UnknownStream>,

    /// P&ID object inventory derived from Dynamic Attributes records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_inventory: Option<ObjectInventory>,

    /// Structured P&ID object graph (objects + relationships), derived from
    /// the same `P&IDAttributes` records as `object_inventory` but indexed
    /// for cross-stream lookup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_graph: Option<ObjectGraph>,

    /// Cross-reference graph that stitches decoded data together
    /// (PSM declarations ↔ actual clusters, JSite ↔ symbols, DA class ↔ records,
    /// PSMroots ↔ cfb tree). Derived from `PidDocument` in a second pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_reference: Option<CrossReferenceGraph>,

    /// Readable whole-drawing layout derived from semantic graph topology,
    /// representation hints, and known symbol categories. This is a
    /// visualization-oriented layout model, not a byte-for-byte SmartPlant
    /// geometry decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<PidLayoutModel>,
}

/// Summary inventory of P&ID objects in the drawing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ObjectInventory {
    pub drawing_id: Option<String>,
    pub project: Option<String>,
    pub item_counts: BTreeMap<String, usize>,
    pub items: Vec<PidItem>,
}

/// A single identifiable P&ID item (instrument, pipe, equipment, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidItem {
    pub item_type: String,
    pub drawing_id: Option<String>,
    pub model_id: Option<String>,
}

impl Default for PidDocument {
    fn default() -> Self {
        Self {
            cfb_tree: StorageNode {
                name: "Root Entry".to_string(),
                path: "/".to_string(),
                kind: EntryKind::Root,
                children: vec![],
            },
            streams: vec![],
            summary: None,
            drawing_meta: None,
            general_meta: None,
            jsites: vec![],
            clusters: vec![],
            dynamic_attributes: None,
            sheet_streams: vec![],
            psm_roots: None,
            psm_cluster_table: None,
            psm_segment_table: None,
            version_history: None,
            app_object_registry: None,
            tagged_storages: None,
            doc_version2: None,
            doc_version2_decoded: None,
            unknown_streams: vec![],
            object_inventory: None,
            object_graph: None,
            cross_reference: None,
            layout: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StorageNode {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub children: Vec<StorageNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub enum EntryKind {
    Root,
    Storage,
    Stream,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamEntry {
    pub path: String,
    pub size: u64,
    pub preview_ascii: Vec<String>,
    pub magic_u32_le: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SummaryInfo {
    pub creating_application: Option<String>,
    pub template: Option<String>,
    pub title: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub raw: BTreeMap<String, String>,
    /// Phase 10j (v0.9.0+): `/\x05DocumentSummaryInformation` section 2
    /// user-defined property dictionary. Keys are the dictionary names
    /// (e.g. `"SP_ProjectID"`), values are typed property values. An
    /// empty map means either the stream has no section 2 or the section
    /// has no named entries. Always `#[serde(default)]`-compatible so
    /// v0.7.x / v0.8.x JSON input deserializes cleanly.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub user_properties: BTreeMap<String, SummaryPropertyValue>,
}

/// Phase 10j (v0.9.0+): typed property value from the user-defined
/// dictionary in DocumentSummaryInformation section 2.
///
/// Covers the VT codes SmartPlant practically emits in section 2;
/// unknown VTs fall through to [`SummaryPropertyValue::Raw`] so the
/// round-trip remains safe (writer passes them through verbatim).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SummaryPropertyValue {
    /// VT_LPSTR (0x001E) — single-byte string. Writer encodes by
    /// `MetadataUpdates.summary_user_updates_encoded` or UTF-8 default.
    Lpstr(String),
    /// VT_LPWSTR (0x001F) — UTF-16LE string.
    Lpwstr(String),
    /// VT_I4 (0x0003).
    I4(i32),
    /// VT_BOOL (0x000B) — property-set representation is u16 0x0000 /
    /// 0xFFFF.
    Bool(bool),
    /// VT_FILETIME (0x0040) — raw 64-bit 100ns-since-1601 value.
    Filetime(u64),
    /// Any other VT the parser recognizes structurally but does not
    /// model explicitly; writer passes bytes through verbatim.
    /// Serialized as a plain JSON array of ints. If the size becomes
    /// a JSON-bloat concern in practice, a future Phase can swap to a
    /// base64 adaptor under a new wire version.
    Raw { vt: u16, bytes: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DrawingMeta {
    pub drawing_number: Option<String>,
    pub document_category: Option<String>,
    pub template_name: Option<String>,
    pub rules_uid: Option<String>,
    pub formats_uid: Option<String>,
    pub gapping_uid: Option<String>,
    pub symbology_uid: Option<String>,
    pub default_formats_uid: Option<String>,
    pub raw_xml: String,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct GeneralMeta {
    pub file_path: Option<String>,
    pub file_size: Option<String>,
    pub raw_xml: String,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct JSite {
    pub name: String,
    pub path: String,
    pub symbol_name: Option<String>,
    pub symbol_path: Option<String>,
    pub local_symbol_path: Option<String>,
    pub has_ole_stream: bool,
    pub ole_links: Vec<String>,
    pub properties: JProperties,
    pub raw_streams: Vec<EmbeddedStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct JProperties {
    pub strings: Vec<String>,
    pub key_values: BTreeMap<String, String>,
    pub guids: Vec<String>,
    pub raw_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddedStream {
    pub name: String,
    pub size: u64,
    pub preview_ascii: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
    pub extracted_strings: Vec<String>,
    pub kind: ClusterKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ClusterHeader>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_table: Option<Vec<IndexedString>>,
    /// Probe metadata for string-table detection heuristic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_info: Option<ClusterProbeInfo>,
}

/// Common header shared by all streams with magic 0x6C90F544.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterHeader {
    pub magic: u32,
    pub record_count: u32,
    pub stream_type: u16,
    pub body_len: u32,
    pub flags: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexedString {
    pub index: u32,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ClusterKind {
    PsmCluster,
    StyleCluster,
    DynamicAttributesMetadata,
    Sheet,
    UnclusteredDynamicAttributes,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicAttributesBlob {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
    pub strings: Vec<String>,
    pub relationships: Vec<String>,
    pub class_names: Vec<String>,
    pub raw_preview_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ClusterHeader>,
    /// Structured attribute records parsed from the binary stream.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub attribute_records: Vec<AttributeRecord>,
    /// Probe summary: heuristic scan metadata (offsets, chunk counts, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_summary: Option<ProbeSummary>,
    /// Byte-level probe output for every `Relationship.<GUID>` record found
    /// in this stream. See `RelationshipProbe` for scope and caveats.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub relationship_probes: Vec<RelationshipProbe>,
    /// Decoded per-record trailer (31-byte footer ending in `0x14 0x00 0x00`)
    /// for every P&IDAttributes record. Exposes `record_id`, `field_x`, and
    /// `class_id` so downstream code can cross-reference records against
    /// Sheet-level endpoint pair records.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub record_trailers: Vec<DaRecordTrailer>,
}

/// Footer of a single P&IDAttributes record inside
/// `/Unclustered Dynamic Attributes`. Bytes layout (verified against two
/// real-world samples) :
///
/// ```text
/// +0   u16   marker = 0x0089
/// +2   u32   size         (record-local length counter)
/// +6   u32   record_id    (e.g. 0x00006009, 0x00006086, monotonic-ish)
/// +10  [u8;8] padding (all zero in observed samples)
/// +18  u32   field_x      (monotonic +2 across relationships, index into
///                          the Sheet endpoint-pair table)
/// +22  u16   separator = 0xFFFF
/// +24  u32   class_id     (0xF6 = Relationship, 0xEA = Drawing, 0x109 =
///                          Symbol/Nozzle, 0x10D/0x10B/… = other class)
/// +28  [u8;3] tail = 0x14 0x00 0x00
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DaRecordTrailer {
    /// Stream offset of the record's first byte (usually `P&IDAttributes`).
    pub record_start: usize,
    /// Stream offset of the `0x89 0x00` trailer marker.
    pub trailer_offset: usize,
    /// `size` u32 from the trailer.
    pub size: u32,
    /// `record_id` u32 from the trailer — the record's canonical identifier.
    pub record_id: u32,
    /// `field_x` u32 from the trailer — indexes into the Sheet
    /// endpoint-pair table for relationships; appears to be a hash-like
    /// local sequence number for other record classes.
    pub field_x: u32,
    /// `class_id` u32 from the trailer — see struct doc for known values.
    pub class_id: u32,
    /// 32-character hex `DrawingID` of the record, resolved by scanning
    /// backwards from the trailer for the `DrawingID\0<32hex>` sequence.
    /// `None` for relationships (which don't carry a DrawingID) and for
    /// records where the marker couldn't be located.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    /// `Relationship.<GUID>` tag captured by scanning backwards from the
    /// trailer. Only populated when `class_id == 0x000000F6`. The hex GUID
    /// (without the leading `Relationship.` prefix) is extracted from the
    /// stream as-is.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relationship_guid: Option<String>,
}

/// A single attribute class record from Unclustered Dynamic Attributes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributeRecord {
    pub class_name: String,
    pub attributes: Vec<AttributeField>,
    /// Confidence level: "heuristic" for probe-derived, "decoded" for verified.
    #[serde(default = "default_confidence")]
    pub confidence: String,
}

fn default_confidence() -> String {
    "heuristic".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributeField {
    pub name: String,
    pub value: AttributeValue,
    /// Audit trail: when the heuristic value decoder strips a leading
    /// prefix byte (see `dynamic_attr_records::strip_value_prefix`), the
    /// pre-strip string is recorded here so callers can detect and
    /// override the heuristic. `None` means no stripping occurred.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Empty,
}

/// Probe metadata for PSMcluster0 string-table heuristic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterProbeInfo {
    /// Byte offset where the string table was detected.
    pub string_table_offset: usize,
    /// Method used to locate the start: "entry2_backtrack" or "fallback".
    pub detection_method: String,
    /// Number of entries parsed.
    pub entries_parsed: usize,
    /// Byte offset where parsing ended.
    pub end_offset: usize,
}

/// Probe summary for heuristic scanning of binary streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProbeSummary {
    /// Byte offset where body scanning began.
    pub body_start_offset: usize,
    /// Number of 0x89 markers found.
    pub marker_count: usize,
    /// Total records extracted (heuristic).
    pub records_extracted: usize,
    /// Byte coverage: how many bytes were interpreted vs total stream size.
    pub bytes_scanned: usize,
}

/// Per-record probe output for a single `Relationship.<GUID>` occurrence
/// inside `Unclustered Dynamic Attributes`. This deliberately does **not**
/// report source/target endpoints — experiments across the full CFB
/// (all streams, both raw and Windows GUID byte layouts) show the
/// Relationship GUID only occurs once, in ASCII, so the endpoints are not
/// stored adjacent to this record. The probe instead records byte-level
/// evidence that later Sheet-level decoders can correlate.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipProbe {
    /// 32-char hex relationship identifier, matching `Relationship.<GUID>`.
    pub guid: String,
    /// Offset of the `Relationship.` ASCII tag inside the stream.
    pub ascii_offset: usize,
    /// Inclusive start of the byte window the probe scanned.
    pub window_start: usize,
    /// Exclusive end of the byte window the probe scanned.
    pub window_end: usize,
    /// Other 32-char hex GUIDs the probe found inside the window
    /// (`offset`, `guid`). Typically includes the enclosing record's
    /// `DrawingNo` value, NOT endpoints.
    pub nearby_ascii_guids: Vec<(usize, String)>,
    /// Short `u16` tokens following the record separator. Labelled by
    /// their position relative to the `0x89 0x00` marker so callers can
    /// reason about record identity without interpreting the value.
    pub trailing_tokens: Vec<RelationshipTrailingToken>,
}

/// A single `u16` token extracted from the Relationship trailing bytes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipTrailingToken {
    pub offset: usize,
    /// Human label indicating *where* the token lives, e.g.
    /// `"after_marker+6"`.
    pub label: String,
    pub value: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SheetStream {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub extracted_texts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_u32_le: Option<u32>,
    /// Four-character ASCII rendering of `magic_u32_le` (e.g. "DF90", "tseg").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ClusterHeader>,
    /// Structured attribute records extracted from the sheet stream (heuristic).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub attribute_records: Vec<AttributeRecord>,
    /// Probe summary: heuristic scan metadata (body_start_offset, marker_count, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_summary: Option<ProbeSummary>,
    /// Endpoint-pair records decoded from the sheet. Each entry maps a
    /// Relationship's `field_x` to the `(endpoint_a, endpoint_b)` field_x
    /// pair of the two objects it connects.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub endpoint_records: Vec<SheetEndpointRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnknownStream {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
    /// Four-character ASCII rendering of `magic_u32_le` (e.g. "toor", "tseg").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_tag: Option<String>,
}

// -----------------------------------------------------------------------
// Phase 10a: SPPID parse coverage inventory
//
// Types below support the `inspect::coverage` module introduced in
// v0.6.0 (Phase 10a Task 1 of the SPPID full-parse roadmap). They carry
// the current "how much of a SPPID file has been decoded" state as a
// structured report, replacing the earlier binary "known vs unidentified"
// view.
// -----------------------------------------------------------------------

/// Decoding state of a single top-level stream or storage, as classified
/// by [`crate::inspect::coverage::coverage_report`].
///
/// The states are ordered from "most complete" to "least complete", which
/// matches the order downstream renderers use to print coverage buckets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum ParseCoverageStatus {
    /// The stream has a dedicated parser AND a structured decoded model;
    /// bytes are interpreted end-to-end and the result lands on a typed
    /// field of [`PidDocument`]. Example: `DocVersion3`, `PSMroots`.
    FullyDecoded,
    /// The stream has a parser but the decoded model covers only part of
    /// the bytes (e.g. header known, record fields partially named).
    /// Example: `PSMclustertable`, `PSMsegmenttable`.
    PartiallyDecoded,
    /// The stream (or storage prefix) is recognized as "this is SPPID
    /// data of kind X" but no decoder beyond that claim exists yet.
    /// Example: `Sheet1` storage prefix; raw bytes preserved for writer
    /// passthrough but not interpreted.
    IdentifiedOnly,
    /// The top-level name is not in any known list. Usually either a
    /// sample-specific artifact or an upstream schema addition we have
    /// not caught up with.
    Unknown,
}

/// Kind of node a [`CoverageEntry`] refers to — distinguishes a bare
/// top-level stream from a top-level storage (i.e. a "directory" whose
/// members carry most of the data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CoverageNodeKind {
    TopLevelStream,
    TopLevelStorage,
}

/// One row in the coverage inventory table.
///
/// `parser` and `document_field` are human-facing strings (not typed
/// references) because the coverage report is a diagnostic artifact;
/// tying it to `fn` pointers or field selectors would force coupling to
/// every parser's internal layout.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoverageEntry {
    /// Top-level name (e.g. `"DocVersion3"` or `"Sheet1"`), already
    /// stripped of leading `/` and of anything after the first `/`.
    pub name: String,
    pub kind: CoverageNodeKind,
    pub status: ParseCoverageStatus,
    /// Name of the parser responsible for decoding this node, if any.
    /// `None` for `Unknown` entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<String>,
    /// Human-readable pointer to the [`PidDocument`] field that holds
    /// the decoded result (e.g. `"version_history"`, `"psm_cluster_table"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_field: Option<String>,
    /// Short free-form note, typically used to explain why an entry is
    /// `PartiallyDecoded` (e.g. `"header known; per-record fields audit-only"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Phase 10f (v0.6.5+): total size in bytes of the stream(s) this
    /// entry covers. For a `TopLevelStream` it's the stream's own size;
    /// for a `TopLevelStorage` it's the sum of every member stream's
    /// size. `None` when the coverage entry was produced without a
    /// backing `StreamEntry` lookup (shouldn't happen in normal parser
    /// runs but is tolerated so diagnostic code paths never panic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_size: Option<u64>,
}

/// Full SPPID coverage inventory for a single [`PidDocument`]. Entries
/// are sorted ascending by `name` so diffs against previous runs are
/// reviewable.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct CoverageReport {
    pub entries: Vec<CoverageEntry>,
}

impl CoverageReport {
    /// Count entries in each status bucket, in declaration order of
    /// [`ParseCoverageStatus`]. Useful for rendering summary lines.
    pub fn status_counts(&self) -> [usize; 4] {
        let mut counts = [0usize; 4];
        for entry in &self.entries {
            let idx = match entry.status {
                ParseCoverageStatus::FullyDecoded => 0,
                ParseCoverageStatus::PartiallyDecoded => 1,
                ParseCoverageStatus::IdentifiedOnly => 2,
                ParseCoverageStatus::Unknown => 3,
            };
            counts[idx] += 1;
        }
        counts
    }

    /// Phase 10f (v0.6.5+): sum of `stream_size` per status bucket, in
    /// the same declaration order as [`Self::status_counts`]. Entries
    /// with `stream_size == None` contribute zero. Useful for weighting
    /// the coverage report by bytes (e.g. "how many bytes are still
    /// Unknown?") rather than just counting streams.
    pub fn total_bytes_by_status(&self) -> [u64; 4] {
        let mut totals = [0u64; 4];
        for entry in &self.entries {
            if let Some(size) = entry.stream_size {
                let idx = match entry.status {
                    ParseCoverageStatus::FullyDecoded => 0,
                    ParseCoverageStatus::PartiallyDecoded => 1,
                    ParseCoverageStatus::IdentifiedOnly => 2,
                    ParseCoverageStatus::Unknown => 3,
                };
                totals[idx] = totals[idx].saturating_add(size);
            }
        }
        totals
    }

    /// Phase 10e (v0.6.4+): serialize to a compact JSON string.
    /// Errors are wrapped into [`crate::error::PidError::ParseFailure`]
    /// so callers can stay on the project's uniform error surface
    /// without pulling in `serde_json::Error`.
    pub fn to_json(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "coverage report JSON".into(),
            message: e.to_string(),
        })
    }

    /// Phase 10e (v0.6.4+): pretty-printed variant of [`Self::to_json`]
    /// (2-space indent, one field per line). Convenient for hand-
    /// reviewable CI artifacts.
    pub fn to_json_pretty(&self) -> Result<String, crate::error::PidError> {
        serde_json::to_string_pretty(self).map_err(|e| crate::error::PidError::ParseFailure {
            context: "coverage report JSON".into(),
            message: e.to_string(),
        })
    }

    /// Phase 10e (v0.6.4+): parse a JSON string back into a
    /// [`CoverageReport`]. The JSON shape must match what
    /// [`Self::to_json`] / [`Self::to_json_pretty`] produce; missing
    /// fields or unknown variants return `PidError::ParseFailure`.
    pub fn from_json(json: &str) -> Result<Self, crate::error::PidError> {
        serde_json::from_str(json).map_err(|e| crate::error::PidError::ParseFailure {
            context: "coverage report JSON".into(),
            message: e.to_string(),
        })
    }
}

/// Decoded `PSMroots` stream: list of root-level named entries.
///
/// Byte layout (observed): `[u32 magic='root']` followed by N records of
/// `[u32 id][u32 char_count][UTF-16LE name]`. There is no explicit count;
/// parsing runs until the stream is exhausted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmRoots {
    pub size: u64,
    pub entries: Vec<PsmRootEntry>,
    /// Bytes that could not be interpreted as `[id][char_count][utf16]` records.
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmRootEntry {
    /// Opaque 32-bit identifier (type tag). Seen values: 0x018C, 0x0149, 0x0019,
    /// 0x0014, 0x4000, 0x2000, 0x0001, ...
    pub id: u32,
    /// Offset inside the stream where this record starts (for debugging).
    pub offset: usize,
    /// Decoded UTF-16LE name.
    pub name: String,
}

/// Decoded `PSMclustertable` stream: canonical list of cluster stream names.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmClusterTable {
    pub size: u64,
    pub count: u32,
    pub entries: Vec<PsmClusterEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmClusterEntry {
    /// Decoded UTF-16LE cluster name, e.g. "PSMcluster0", "Sheet6".
    pub name: String,
    /// Offset inside the stream where the UTF-16LE name begins.
    pub name_offset: usize,
}

/// Decoded `PSMsegmenttable` stream. In sampled file it is a fixed 12 bytes:
/// `[magic 'stab'][u32 count=4][4 × 0x01]`. Schema is likely a per-segment
/// flag array; we expose the raw payload until semantics are confirmed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmSegmentTable {
    pub size: u64,
    pub count: u32,
    pub flags: Vec<u8>,
}

/// Decoded `DocVersion3` stream: fixed-size (48 bytes per record) version
/// log entries that record a document's save history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VersionHistory {
    pub size: u64,
    pub records: Vec<VersionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VersionRecord {
    /// Null-terminated ASCII product identifier, e.g. "SmartPlantPID.a".
    pub product: String,
    /// Null-terminated ASCII version string, e.g. "090000.0144".
    pub version: String,
    /// Operation code: observed values "SA" (save-as / create) and
    /// "SV" (save / modify). Use [`Self::operation_label`] for a
    /// human-readable form that mirrors
    /// [`crate::parsers::doc_version2::op_type_label`].
    pub operation: String,
    /// Null-terminated ASCII timestamp, e.g. "12/29/25 10:45". Use
    /// [`Self::parsed_timestamp`] to destructure it.
    pub timestamp: String,
}

impl VersionRecord {
    /// True iff `operation == "SA"`, mirroring DocVersion2 op_type
    /// `0x82` (SaveAs / create) observed in Phase 9f sample analysis.
    pub fn is_save_as(&self) -> bool {
        self.operation == "SA"
    }

    /// True iff `operation == "SV"`, mirroring DocVersion2 op_type
    /// `0x81` (Save / modify).
    pub fn is_save(&self) -> bool {
        self.operation == "SV"
    }

    /// True iff `operation` is one of the codes SmartPlant is known to
    /// produce. When `false`, the raw string is still available via
    /// [`Self::operation`] for diagnostic logging.
    pub fn is_recognized_operation(&self) -> bool {
        self.is_save_as() || self.is_save()
    }

    /// Human label for [`Self::operation`]. Returns `"SaveAs"` for
    /// `"SA"`, `"Save"` for `"SV"`, and `"unknown"` otherwise. The
    /// unknown case is intentionally a flat string rather than a full
    /// enum variant so callers stay decoupled from future code
    /// additions — pair with [`Self::operation`] when the raw value
    /// matters.
    pub fn operation_label(&self) -> &'static str {
        if self.is_save_as() {
            "SaveAs"
        } else if self.is_save() {
            "Save"
        } else {
            "unknown"
        }
    }

    /// Decompose [`Self::timestamp`] (format `MM/DD/YY HH:MM`) into
    /// `(month, day, year, hour, minute)`. Returns `None` if the raw
    /// string does not match the observed shape exactly — we do not
    /// interpret a two-digit year (e.g. `"26"`), so the caller picks
    /// the century convention.
    pub fn parsed_timestamp(&self) -> Option<(u32, u32, u32, u32, u32)> {
        let (date_part, time_part) = self.timestamp.split_once(' ')?;
        let date: Vec<&str> = date_part.split('/').collect();
        let time: Vec<&str> = time_part.split(':').collect();
        if date.len() != 3 || time.len() != 2 {
            return None;
        }
        let month = date[0].parse::<u32>().ok()?;
        let day = date[1].parse::<u32>().ok()?;
        let year = date[2].parse::<u32>().ok()?;
        let hour = time[0].parse::<u32>().ok()?;
        let minute = time[1].parse::<u32>().ok()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour >= 24 || minute >= 60 {
            return None;
        }
        Some((month, day, year, hour, minute))
    }
}

#[cfg(test)]
mod version_record_tests {
    use super::VersionRecord;

    fn record(op: &str, ts: &str) -> VersionRecord {
        VersionRecord {
            product: "SmartPlantPID.a".into(),
            version: "090000.0144".into(),
            operation: op.into(),
            timestamp: ts.into(),
        }
    }

    #[test]
    fn version_record_is_save_as_matches_sa_literal() {
        let r = record("SA", "12/29/25 10:45");
        assert!(r.is_save_as());
        assert!(!r.is_save());
        assert!(r.is_recognized_operation());
        assert_eq!(r.operation_label(), "SaveAs");
    }

    #[test]
    fn version_record_is_save_matches_sv_literal() {
        let r = record("SV", "12/30/25 09:12");
        assert!(r.is_save());
        assert!(!r.is_save_as());
        assert!(r.is_recognized_operation());
        assert_eq!(r.operation_label(), "Save");
    }

    #[test]
    fn version_record_operation_label_echoes_unknown_to_flat_string() {
        let r = record("XY", "01/01/26 00:00");
        assert!(!r.is_recognized_operation());
        assert_eq!(r.operation_label(), "unknown");
    }

    #[test]
    fn version_record_parsed_timestamp_happy_path() {
        let r = record("SA", "12/29/25 10:45");
        assert_eq!(r.parsed_timestamp(), Some((12, 29, 25, 10, 45)));
    }

    #[test]
    fn version_record_parsed_timestamp_returns_none_for_malformed() {
        // Wrong separator
        assert_eq!(record("SA", "12-29-25 10:45").parsed_timestamp(), None);
        // Missing time part
        assert_eq!(record("SA", "12/29/25").parsed_timestamp(), None);
        // Out-of-range month
        assert_eq!(record("SA", "13/29/25 10:45").parsed_timestamp(), None);
        // Non-numeric
        assert_eq!(record("SA", "ab/cd/ef gh:ij").parsed_timestamp(), None);
        // Empty
        assert_eq!(record("SA", "").parsed_timestamp(), None);
    }
}

/// Decoded `AppObject` stream: registry of external COM / DLL plugins the
/// source application linked to this drawing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AppObjectRegistry {
    pub size: u64,
    /// `u32` at offset 0; observed value `5` on the sampled file (likely
    /// entry count or registry version).
    pub leading_u32: u32,
    pub entries: Vec<AppObjectEntry>,
    /// Any bytes that could not be attributed to a full entry (e.g. trailing
    /// class-id-only record).
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AppObjectEntry {
    /// Offset in stream where this record begins (for debugging).
    pub offset: usize,
    /// 16-byte COM class identifier rendered in GUID form.
    pub clsid: String,
    /// UTF-16LE file path, typically a DLL location.
    pub path: String,
}

/// Decoded `JTaggedTxtStgList`: small index mapping a storage list name
/// (e.g. "TaggedTxtStorages") to the actual storage directory name
/// (e.g. "TaggedTxtData").
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaggedTextStorageList {
    pub size: u64,
    pub list_name: String,
    pub entries: Vec<TaggedTextStorageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaggedTextStorageEntry {
    /// Storage directory name (e.g. "TaggedTxtData").
    pub storage_name: String,
}

/// Raw preservation of the `DocVersion2` stream. The 48-byte binary payload
/// is not yet structurally decoded; we record its magic and hex preview so
/// that downstream tooling can round-trip or inspect without losing data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocVersion2Raw {
    pub size: u64,
    pub magic_u32_le: u32,
    /// Lowercase hex dump of the full payload (up to 128 bytes).
    pub hex_preview: String,
}

/// Structured decoding of the `/DocVersion2` stream (v0.3.8+).
///
/// `/DocVersion2` is a compact per-save version log, matching `/DocVersion3`
/// one-to-one (a SaveAs + N Saves). Format:
///
/// - 12-byte header: `u32 LE magic = 0x0001_0034` + 8 reserved bytes
/// - N × 9-byte records: `op_type | fixed=[0,0,9] | separator | u32 LE version`
///
/// See `src/parsers/doc_version2.rs` for the full analysis and tests.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocVersion2 {
    /// First u32 LE of the stream (always `0x0001_0034` for decoded files).
    pub magic_u32_le: u32,
    /// `true` when the 8 reserved header bytes are all zero (observed on
    /// every real sample). `false` surfaces a potential layout surprise
    /// without rejecting the record set.
    pub reserved_all_zero: bool,
    /// Per-save records in stream order.
    pub records: Vec<DocVersion2Record>,
}

/// One record inside a [`DocVersion2`] log. The `op_type` byte (0x82 =
/// SaveAs, 0x81 = Save) and `version` (u32 LE) are the semantic fields;
/// the other bytes are carried through so round-trippers can still
/// reproduce the original byte stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocVersion2Record {
    pub op_type: u8,
    pub fixed: [u8; 3],
    pub separator: u8,
    pub version: u32,
}

/// Structured P&ID object graph — the core deliverable that ties together
/// the `P&IDAttributes` DA records into a queryable view of the drawing's
/// modeled objects and their relationships.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ObjectGraph {
    /// Drawing-level identifier (`DrawingNo` from P&IDAttributes).
    pub drawing_no: Option<String>,
    /// Project number that owns this drawing.
    pub project_number: Option<String>,
    /// Non-relationship domain objects (equipment, pipe, nozzle, label, …).
    pub objects: Vec<PidObject>,
    /// Relationship records (`ModelItemType == "Relationship"`).
    pub relationships: Vec<PidRelationship>,
    /// `drawing_id -> index in objects` for O(log N) lookup.
    pub by_drawing_id: BTreeMap<String, usize>,
    /// `ModelItemType -> counts`. Matches `ObjectInventory.item_counts` but
    /// split across objects vs relationships.
    pub counts_by_type: BTreeMap<String, usize>,
}

/// Aggregate health view over [`ObjectGraph::relationships`]: how many
/// have both / one / zero of their endpoints resolved to a known
/// `drawing_id`. Useful as a one-line summary in reports and as a CI
/// invariant for fixture drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct EndpointResolutionStats {
    pub total: usize,
    /// Both `source_drawing_id` and `target_drawing_id` are `Some`.
    pub fully_resolved: usize,
    /// Exactly one of source/target is `Some`.
    pub partially_resolved: usize,
    /// Neither endpoint resolved (both `None`).
    pub unresolved: usize,
}

impl ObjectGraph {
    /// O(log N) lookup: returns the [`PidObject`] with `drawing_id`, or
    /// `None` if no such object lives in this graph.
    pub fn object_by_drawing_id(&self, drawing_id: &str) -> Option<&PidObject> {
        self.by_drawing_id
            .get(drawing_id)
            .and_then(|idx| self.objects.get(*idx))
    }

    /// Every relationship whose `source_drawing_id` or
    /// `target_drawing_id` matches `drawing_id`. O(R) — callers that
    /// hammer this in a hot loop should build a reverse index of their
    /// own.
    pub fn relationships_touching(&self, drawing_id: &str) -> Vec<&PidRelationship> {
        self.relationships
            .iter()
            .filter(|r| {
                r.source_drawing_id.as_deref() == Some(drawing_id)
                    || r.target_drawing_id.as_deref() == Some(drawing_id)
            })
            .collect()
    }

    /// Distinct [`PidObject`]s that share at least one resolved
    /// relationship endpoint with `drawing_id`. Self-loops and
    /// unresolved (`None`) endpoints are silently skipped. Output is
    /// sorted by `drawing_id` for deterministic iteration.
    pub fn neighbors_of(&self, drawing_id: &str) -> Vec<&PidObject> {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        let mut out: Vec<&PidObject> = Vec::new();
        for rel in self.relationships_touching(drawing_id) {
            for endpoint in [
                rel.source_drawing_id.as_deref(),
                rel.target_drawing_id.as_deref(),
            ] {
                let Some(other) = endpoint else { continue };
                if other == drawing_id {
                    continue; // self-loop
                }
                if !seen.insert(other) {
                    continue;
                }
                if let Some(obj) = self.object_by_drawing_id(other) {
                    out.push(obj);
                }
            }
        }
        out
    }

    /// Linear scan: every object whose `item_type` exactly equals
    /// `item_type` (case-sensitive). Returned in source order.
    /// O(N) — for hot loops with few item_types, callers can build a
    /// `BTreeMap<&str, Vec<&PidObject>>` themselves once.
    pub fn find_objects_by_item_type(&self, item_type: &str) -> Vec<&PidObject> {
        self.objects
            .iter()
            .filter(|o| o.item_type == item_type)
            .collect()
    }

    /// Linear scan: every object whose `extra` BTreeMap contains
    /// `key`=`value` (both case-sensitive, exact match). Returned in
    /// source order.
    pub fn find_objects_by_extra(&self, key: &str, value: &str) -> Vec<&PidObject> {
        self.objects
            .iter()
            .filter(|o| o.extra.get(key).map(String::as_str) == Some(value))
            .collect()
    }

    /// Shortest-path BFS in the relationship graph from `from_id` to
    /// `to_id`. Returns `Some(path)` where `path[0] == from_id`,
    /// `path.last() == to_id`, and adjacent entries share a resolved
    /// relationship endpoint. Returns `None` if no path exists or
    /// either endpoint is unknown to this graph.
    ///
    /// `from_id == to_id` returns `Some(vec![from_id])` (zero-hop
    /// path). Cycles are safe; each `drawing_id` is enqueued at most
    /// once. Time O(V+E), space O(V).
    pub fn shortest_path<'a>(&'a self, from_id: &'a str, to_id: &'a str) -> Option<Vec<&'a str>> {
        // Both endpoints must be known objects.
        let from_key = self.by_drawing_id.get_key_value(from_id)?.0.as_str();
        let to_key = self.by_drawing_id.get_key_value(to_id)?.0.as_str();
        if from_key == to_key {
            return Some(vec![from_key]);
        }

        // BFS with predecessor map.
        let mut predecessor: std::collections::BTreeMap<&'a str, &'a str> =
            std::collections::BTreeMap::new();
        let mut frontier: std::collections::VecDeque<&'a str> = std::collections::VecDeque::new();
        let mut visited: std::collections::BTreeSet<&'a str> = std::collections::BTreeSet::new();
        frontier.push_back(from_key);
        visited.insert(from_key);

        let mut found = false;
        while let Some(current) = frontier.pop_front() {
            if current == to_key {
                found = true;
                break;
            }
            for neighbor in self.neighbors_of(current) {
                let nid = self
                    .by_drawing_id
                    .get_key_value(neighbor.drawing_id.as_str())
                    .map(|(k, _)| k.as_str())?;
                if visited.insert(nid) {
                    predecessor.insert(nid, current);
                    frontier.push_back(nid);
                }
            }
        }

        if !found {
            return None;
        }

        // Reconstruct: walk predecessor map backward from to_key.
        let mut path: Vec<&'a str> = Vec::new();
        let mut cursor: &'a str = to_key;
        path.push(cursor);
        while cursor != from_key {
            cursor = *predecessor.get(cursor)?;
            path.push(cursor);
        }
        path.reverse();
        Some(path)
    }

    /// BFS-walk: every object reachable from `drawing_id` within
    /// `depth` hops via resolved relationship endpoints. The starting
    /// object itself is **not** included in the result. Self-loops and
    /// unresolved (`None`) endpoints are silently skipped (matches
    /// [`Self::neighbors_of`] semantics).
    ///
    /// `depth=0` → empty `Vec` (no hops taken).
    /// `depth=1` → identical contents to [`Self::neighbors_of`]
    /// (level-by-level vs. single-level happen to match here).
    /// `depth=N` → all objects 1..=N hops away, distinct, in BFS
    /// visitation order (level-by-level; within a level by visitation
    /// order from the predecessor frontier).
    ///
    /// Cycles are safe: each `drawing_id` is visited at most once.
    pub fn neighbors_within(&self, drawing_id: &str, depth: usize) -> Vec<&PidObject> {
        if depth == 0 {
            return Vec::new();
        }
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        seen.insert(drawing_id.to_string());
        let mut out: Vec<&PidObject> = Vec::new();
        let mut frontier: Vec<String> = vec![drawing_id.to_string()];
        for _hop in 0..depth {
            let mut next_frontier: Vec<String> = Vec::new();
            for current in &frontier {
                for neighbor in self.neighbors_of(current) {
                    let nid = neighbor.drawing_id.clone();
                    if seen.insert(nid.clone()) {
                        out.push(neighbor);
                        next_frontier.push(nid);
                    }
                }
            }
            if next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }
        out
    }

    /// All `drawing_id`s that start with `prefix`, in ascending sorted
    /// order. Empty `prefix` returns every id (≡ all object
    /// drawing_ids). Case-sensitive — `drawing_id`s are uppercase
    /// 32-hex by SmartPlant convention.
    ///
    /// `BTreeMap::range`-backed: O(log N + K) where K is the result
    /// length; faster than scanning `objects` linearly when only a
    /// short prefix is given.
    pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str> {
        if prefix.is_empty() {
            return self.by_drawing_id.keys().map(|s| s.as_str()).collect();
        }
        self.by_drawing_id
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Compute [`EndpointResolutionStats`] over `relationships`. Pure
    /// derived view; cheap (O(R)) and lossless.
    pub fn endpoint_resolution_stats(&self) -> EndpointResolutionStats {
        let mut stats = EndpointResolutionStats {
            total: self.relationships.len(),
            ..Default::default()
        };
        for rel in &self.relationships {
            match (
                rel.source_drawing_id.is_some(),
                rel.target_drawing_id.is_some(),
            ) {
                (true, true) => stats.fully_resolved += 1,
                (true, false) | (false, true) => stats.partially_resolved += 1,
                (false, false) => stats.unresolved += 1,
            }
        }
        stats
    }
}

/// A single modeled object on the drawing. Mostly sourced from a single
/// `P&IDAttributes` DA record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidObject {
    /// 32-character hex unique identifier (e.g. "D8FAB6ED48684E799CDFF0396E213773").
    pub drawing_id: String,
    /// `ModelItemType`: "PipeRun", "Nozzle", "Instrument", "Drawing", …
    pub item_type: String,
    /// `DrawingItemType`: "Symbol", "LabelPersist", "ItemNote", …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawing_item_type: Option<String>,
    /// `ModelID` if present (rare in our samples).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Any remaining non-core attributes (including heuristically-decoded
    /// name=value extras).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub extra: BTreeMap<String, String>,
    /// Per-record trailer fields from the DA stream. Populated when the
    /// trailer for this object's record could be located and linked via
    /// `DrawingID` lookback. `None` indicates the trailer was not
    /// successfully matched.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub record_id: Option<u32>,
    /// `field_x` from the DA trailer — for relationships this is the index
    /// into the Sheet endpoint-pair table; here it is used to resolve
    /// endpoint references back to the owning object.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub field_x: Option<u32>,
}

/// A Relationship record from `P&IDAttributes` (`ModelItemType="Relationship"`).
/// Endpoints are resolved via Sheet-level endpoint-pair records that index
/// into the DA `field_x` space.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidRelationship {
    /// `ModelID`, e.g. "Relationship.C5CF946710BF4EBDB02808EBD6879B62".
    pub model_id: String,
    /// The 32-character hex GUID portion of `model_id`, for cross-referencing.
    pub guid: String,
    /// Canonical DA record id for this relationship.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub record_id: Option<u32>,
    /// `field_x` from the DA trailer — the key used to locate this
    /// relationship's endpoint pair in Sheet streams.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub field_x: Option<u32>,
    /// `drawing_id` of the source endpoint, if it could be resolved through
    /// the Sheet endpoint-pair record and the DA `field_x → drawing_id`
    /// mapping.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_drawing_id: Option<String>,
    /// `drawing_id` of the target endpoint, if it could be resolved.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_drawing_id: Option<String>,
}

/// A single endpoint-pair record parsed out of a Sheet stream. Each
/// `Relationship` in the DA stream is expected to have exactly one such
/// record in one of the `/Sheet*` streams; its `endpoint_a` / `endpoint_b`
/// fields are `field_x` values that reference back to the object records.
///
/// Record signature (74-byte span starting at `rel_field_x`):
///
/// ```text
///   +0  u32  rel_field_x       ← matches relationship's DA trailer field_x
///   +4  u32  0x00000006        ← constant discriminator
///   +8  [u8;8] zero padding
///   +16 u16  type = 0x0002     ← endpoint-record marker
///   +18 u32  endpoint_a        ← source field_x
///   +22 u16  0x0001
///   +24 u32  endpoint_b        ← target field_x
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SheetEndpointRecord {
    /// Sheet stream path (e.g. `/Sheet6`).
    pub sheet_path: String,
    /// Byte offset in the Sheet stream where this record starts.
    pub offset: usize,
    /// Relationship's `field_x` value.
    pub rel_field_x: u32,
    /// Source endpoint `field_x` (references the source object's DA record).
    pub endpoint_a: u32,
    /// Target endpoint `field_x` (references the target object's DA record).
    pub endpoint_b: u32,
}

// ---- Readable layout model ---------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PidLayoutModel {
    pub items: Vec<PidLayoutItem>,
    pub segments: Vec<PidLayoutSegment>,
    pub texts: Vec<PidLayoutText>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unplaced: Vec<PidLayoutUnplaced>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutItem {
    pub layout_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    pub kind: String,
    pub anchor: [f64; 2],
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bounds: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutSegment {
    pub layout_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub owner_drawing_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutText {
    pub layout_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    pub text: String,
    pub anchor: [f64; 2],
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bounds: Option<[f64; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutUnplaced {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    pub kind: String,
    pub label: String,
}

// ---- Cross-reference graph ---------------------------------------------------

/// Stitches already-decoded pieces of the document into a small relational
/// view. Pure derivation — requires no extra I/O.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct CrossReferenceGraph {
    /// PSM-declared clusters vs. clusters actually present in the file.
    pub cluster_coverage: ClusterCoverage,
    /// JSite instances grouped by the symbol they reference.
    pub symbol_usage: Vec<SymbolUsage>,
    /// One summary per attribute class found in Unclustered Dynamic Attributes.
    pub attribute_classes: Vec<AttributeClassSummary>,
    /// Each PSMroots entry correlated with its existence in the CFB tree.
    pub root_presence: Vec<RootPresence>,
}

/// Comparison between `PSMclustertable` (declared) and the cluster / sheet
/// streams actually parsed from the file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ClusterCoverage {
    /// Names declared by `PSMclustertable`.
    pub declared: Vec<String>,
    /// Names found on-disk (cluster streams + sheet streams).
    pub found: Vec<String>,
    /// Names present in both sets.
    pub matched: Vec<String>,
    /// Declared but not found on-disk (data-integrity warning).
    pub declared_missing: Vec<String>,
    /// Found on-disk but not declared (typically only when PSM is absent).
    pub found_extra: Vec<String>,
}

/// Symbol → JSite reverse index. One entry per unique `symbol_path`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SymbolUsage {
    /// Absolute symbol path (e.g. `\\server\share\symbols\Valve.sym`).
    pub symbol_path: String,
    /// Basename of the symbol (e.g. `Valve`). `None` when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    /// JSite storage names that reference this symbol (sorted, unique).
    pub jsite_names: Vec<String>,
    /// Number of references. Always equal to `jsite_names.len()`.
    pub usage_count: usize,
}

/// Per-class aggregation of Dynamic Attributes records.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributeClassSummary {
    pub class_name: String,
    pub record_count: usize,
    /// Distinct `DrawingID` / `DrawingNo` values encountered (sorted, unique).
    pub drawing_ids: Vec<String>,
    /// Distinct `ModelID` values encountered (sorted, unique, capped at 32).
    pub model_ids: Vec<String>,
    /// Distinct attribute names encountered under this class (sorted, unique).
    pub unique_attribute_names: Vec<String>,
}

/// Describes whether a name published in `PSMroots` actually maps to a
/// storage or stream in the CFB tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RootPresence {
    pub name: String,
    pub id: u32,
    pub found_as_storage: bool,
    pub found_as_stream: bool,
}

#[cfg(test)]
mod object_graph_impl_tests {
    use super::*;

    /// Tiny synthetic graph with three objects and three relationships:
    ///   A — B  (fully resolved)
    ///   A — ?  (partially resolved; target unknown)
    ///   ? — C  (partially resolved; source unknown)
    ///
    /// `D` is a 4th object that has no relationships, used to verify
    /// `neighbors_of("D")` returns empty.
    fn sample_graph() -> ObjectGraph {
        fn obj(id: &str, item_type: &str, fx: u32) -> PidObject {
            PidObject {
                drawing_id: id.into(),
                item_type: item_type.into(),
                drawing_item_type: None,
                model_id: None,
                extra: BTreeMap::new(),
                record_id: None,
                field_x: Some(fx),
            }
        }
        fn rel(guid: &str, src: Option<&str>, dst: Option<&str>) -> PidRelationship {
            PidRelationship {
                model_id: format!("Relationship.{guid}"),
                guid: guid.into(),
                record_id: None,
                field_x: None,
                source_drawing_id: src.map(String::from),
                target_drawing_id: dst.map(String::from),
            }
        }
        let objects = vec![
            obj("A", "Equipment", 1),
            obj("B", "PipeRun", 2),
            obj("C", "Instrument", 3),
            obj("D", "PipeRun", 4),
        ];
        let mut by_drawing_id = BTreeMap::new();
        for (i, o) in objects.iter().enumerate() {
            by_drawing_id.insert(o.drawing_id.clone(), i);
        }
        ObjectGraph {
            drawing_no: None,
            project_number: None,
            objects,
            relationships: vec![
                rel("R1", Some("A"), Some("B")),
                rel("R2", Some("A"), None),
                rel("R3", None, Some("C")),
            ],
            by_drawing_id,
            counts_by_type: BTreeMap::new(),
        }
    }

    #[test]
    fn object_by_drawing_id_returns_existing_and_none_for_unknown() {
        let g = sample_graph();
        assert_eq!(
            g.object_by_drawing_id("A").map(|o| o.item_type.as_str()),
            Some("Equipment")
        );
        assert_eq!(
            g.object_by_drawing_id("B").map(|o| o.item_type.as_str()),
            Some("PipeRun")
        );
        assert!(g.object_by_drawing_id("Z").is_none());
        assert!(g.object_by_drawing_id("").is_none());
    }

    #[test]
    fn relationships_touching_filters_by_either_endpoint() {
        let g = sample_graph();
        // A appears as source in R1 and R2 → 2 hits.
        let touch_a: Vec<&str> = g
            .relationships_touching("A")
            .iter()
            .map(|r| r.guid.as_str())
            .collect();
        assert_eq!(touch_a, vec!["R1", "R2"]);
        // B appears as target in R1 only.
        let touch_b: Vec<&str> = g
            .relationships_touching("B")
            .iter()
            .map(|r| r.guid.as_str())
            .collect();
        assert_eq!(touch_b, vec!["R1"]);
        // C appears as target in R3 only.
        let touch_c: Vec<&str> = g
            .relationships_touching("C")
            .iter()
            .map(|r| r.guid.as_str())
            .collect();
        assert_eq!(touch_c, vec!["R3"]);
        // D is islanded.
        assert!(g.relationships_touching("D").is_empty());
    }

    #[test]
    fn neighbors_of_dedupes_resolves_and_skips_self_loops() {
        let g = sample_graph();
        // A's neighbors: B (via R1; R2's target is None, skipped).
        let na: Vec<&str> = g
            .neighbors_of("A")
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(na, vec!["B"]);
        // C's neighbors: nothing (R3's source is None).
        assert!(g.neighbors_of("C").is_empty());
        // D's neighbors: nothing.
        assert!(g.neighbors_of("D").is_empty());
    }

    #[test]
    fn neighbors_of_skips_self_loop() {
        let mut g = sample_graph();
        g.relationships.push(PidRelationship {
            model_id: "Relationship.SELF".into(),
            guid: "SELF".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("D".into()),
            target_drawing_id: Some("D".into()),
        });
        // D ↔ D — neighbors_of("D") still empty (we filter self-loops).
        assert!(g.neighbors_of("D").is_empty());
    }

    #[test]
    fn endpoint_resolution_stats_counts_three_buckets() {
        let g = sample_graph();
        let s = g.endpoint_resolution_stats();
        assert_eq!(s.total, 3);
        assert_eq!(s.fully_resolved, 1, "R1 only");
        assert_eq!(s.partially_resolved, 2, "R2 and R3");
        assert_eq!(s.unresolved, 0);
        // Sums must round-trip.
        assert_eq!(
            s.fully_resolved + s.partially_resolved + s.unresolved,
            s.total
        );
    }

    #[test]
    fn shortest_path_returns_zero_hop_for_same_endpoint() {
        let g = sample_graph();
        let path = g.shortest_path("A", "A");
        assert_eq!(path, Some(vec!["A"]));
    }

    #[test]
    fn shortest_path_finds_direct_neighbor() {
        let g = sample_graph();
        // A↔B in sample_graph.
        let path = g.shortest_path("A", "B");
        assert_eq!(path, Some(vec!["A", "B"]));
    }

    #[test]
    fn shortest_path_finds_multi_hop() {
        let mut g = sample_graph();
        // Add B↔C↔E chain extending from sample.
        g.relationships.push(PidRelationship {
            model_id: "Relationship.BC".into(),
            guid: "BC".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("B".into()),
            target_drawing_id: Some("C".into()),
        });
        // A→B→C: A connects to B (R1), B connects to C (BC).
        let path = g.shortest_path("A", "C");
        assert_eq!(path, Some(vec!["A", "B", "C"]));
    }

    #[test]
    fn shortest_path_returns_none_when_unreachable() {
        let g = sample_graph();
        // D is islanded in sample_graph (no relationships touching D).
        assert_eq!(g.shortest_path("A", "D"), None);
    }

    #[test]
    fn shortest_path_returns_none_for_unknown_endpoint() {
        let g = sample_graph();
        assert_eq!(g.shortest_path("A", "ZZZZ"), None);
        assert_eq!(g.shortest_path("ZZZZ", "A"), None);
    }

    #[test]
    fn neighbors_within_zero_returns_empty() {
        let g = sample_graph();
        assert!(g.neighbors_within("A", 0).is_empty());
    }

    #[test]
    fn neighbors_within_one_equals_neighbors_of() {
        let g = sample_graph();
        let one_hop: Vec<&str> = g
            .neighbors_within("A", 1)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        let neighbors: Vec<&str> = g
            .neighbors_of("A")
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(one_hop, neighbors);
    }

    #[test]
    fn neighbors_within_two_walks_two_hops() {
        // Build A↔B↔C↔E chain (D islanded).
        // sample_graph has A↔B, A↔?(unresolved), ?↔C — so reachable
        // from A at depth 2 stays just B (C's source is None so it's
        // not actually neighbor of B). Expand the graph.
        let mut g = sample_graph();
        g.relationships.push(PidRelationship {
            model_id: "Relationship.BC".into(),
            guid: "BC".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("B".into()),
            target_drawing_id: Some("C".into()),
        });
        g.relationships.push(PidRelationship {
            model_id: "Relationship.CD".into(),
            guid: "CD".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("C".into()),
            target_drawing_id: Some("D".into()),
        });
        // Now A→B (1), B→C (2), C→D (3).
        let one: Vec<&str> = g
            .neighbors_within("A", 1)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(one, vec!["B"], "depth=1 reaches only direct neighbor");

        let two: Vec<&str> = g
            .neighbors_within("A", 2)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(
            two,
            vec!["B", "C"],
            "depth=2 reaches B (1 hop) and C (2 hops)"
        );

        let three: Vec<&str> = g
            .neighbors_within("A", 3)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(three, vec!["B", "C", "D"]);
    }

    #[test]
    fn neighbors_within_skips_unreachable() {
        let g = sample_graph();
        // D has no relationships → unreachable from A even at huge depth.
        let many: Vec<&str> = g
            .neighbors_within("A", 100)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert!(
            !many.contains(&"D"),
            "D is islanded; should not appear at any depth"
        );
    }

    #[test]
    fn neighbors_within_handles_cycle_without_infinite_loop() {
        let mut g = sample_graph();
        // Add a cycle: A↔B↔C↔A
        g.relationships.push(PidRelationship {
            model_id: "Relationship.BC".into(),
            guid: "BC".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("B".into()),
            target_drawing_id: Some("C".into()),
        });
        g.relationships.push(PidRelationship {
            model_id: "Relationship.CA".into(),
            guid: "CA".into(),
            record_id: None,
            field_x: None,
            source_drawing_id: Some("C".into()),
            target_drawing_id: Some("A".into()),
        });
        // depth=10 must terminate (no infinite loop) and report each
        // distinct object at most once.
        let result: Vec<&str> = g
            .neighbors_within("A", 10)
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(result, vec!["B", "C"]);
        // No duplicates.
        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 2);
    }

    #[test]
    fn find_by_item_type_returns_matches_in_source_order() {
        let g = sample_graph();
        // sample has A=Equipment, B=PipeRun, C=Instrument, D=PipeRun.
        let pipe_runs: Vec<&str> = g
            .find_objects_by_item_type("PipeRun")
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(pipe_runs, vec!["B", "D"]);
        let equip: Vec<&str> = g
            .find_objects_by_item_type("Equipment")
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(equip, vec!["A"]);
    }

    #[test]
    fn find_by_item_type_returns_empty_for_unknown_type() {
        let g = sample_graph();
        assert!(g.find_objects_by_item_type("NoSuchType").is_empty());
        assert!(g.find_objects_by_item_type("").is_empty());
    }

    #[test]
    fn find_by_extra_returns_matching_value() {
        let mut g = sample_graph();
        g.objects[0].extra.insert("Tag".into(), "FIT-001".into());
        g.objects[1].extra.insert("Tag".into(), "FIT-002".into());
        g.objects[2].extra.insert("Tag".into(), "FIT-001".into());

        let hits: Vec<&str> = g
            .find_objects_by_extra("Tag", "FIT-001")
            .iter()
            .map(|o| o.drawing_id.as_str())
            .collect();
        assert_eq!(hits, vec!["A", "C"]);
    }

    #[test]
    fn find_by_extra_returns_empty_when_key_or_value_missing() {
        let mut g = sample_graph();
        g.objects[0].extra.insert("Tag".into(), "FIT-001".into());
        // Key absent → empty.
        assert!(g.find_objects_by_extra("NoSuchKey", "x").is_empty());
        // Key present but value mismatch → empty.
        assert!(g.find_objects_by_extra("Tag", "OTHER-VALUE").is_empty());
        // No object has the key at all.
        assert!(g.find_objects_by_extra("UnusedKey", "FIT-001").is_empty());
    }

    #[test]
    fn find_by_prefix_returns_sorted_matches() {
        let g = sample_graph();
        // sample has A B C D — query "B" → only B; query "" → all 4.
        assert_eq!(g.find_drawing_ids_by_prefix("B"), vec!["B"]);
        assert_eq!(
            g.find_drawing_ids_by_prefix(""),
            vec!["A", "B", "C", "D"],
            "empty prefix returns all sorted"
        );
    }

    #[test]
    fn find_by_prefix_returns_empty_when_no_match() {
        let g = sample_graph();
        assert!(g.find_drawing_ids_by_prefix("Z").is_empty());
        assert!(g.find_drawing_ids_by_prefix("AAA").is_empty());
    }

    #[test]
    fn find_by_prefix_returns_multiple_when_unique_share_prefix() {
        // Add two ids sharing prefix "X".
        let mut g = sample_graph();
        g.objects.push(PidObject {
            drawing_id: "X1".into(),
            item_type: "Equipment".into(),
            drawing_item_type: None,
            model_id: None,
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        });
        g.objects.push(PidObject {
            drawing_id: "X2".into(),
            item_type: "PipeRun".into(),
            drawing_item_type: None,
            model_id: None,
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        });
        g.by_drawing_id.insert("X1".into(), g.objects.len() - 2);
        g.by_drawing_id.insert("X2".into(), g.objects.len() - 1);

        assert_eq!(g.find_drawing_ids_by_prefix("X"), vec!["X1", "X2"]);
    }

    #[test]
    fn find_by_long_prefix_acts_as_exact_match() {
        let g = sample_graph();
        // "A" is a full id in the sample; longer prefix finds none.
        assert_eq!(g.find_drawing_ids_by_prefix("A"), vec!["A"]);
        assert!(g.find_drawing_ids_by_prefix("ABCDEF").is_empty());
    }

    #[test]
    fn endpoint_resolution_stats_handles_empty_graph() {
        let g = ObjectGraph::default();
        let s = g.endpoint_resolution_stats();
        assert_eq!(s, EndpointResolutionStats::default());
    }
}
