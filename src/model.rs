use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Summary inventory of P&ID objects in the drawing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectInventory {
    pub drawing_id: Option<String>,
    pub project: Option<String>,
    pub item_counts: BTreeMap<String, usize>,
    pub items: Vec<PidItem>,
}

/// A single identifiable P&ID item (instrument, pipe, equipment, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            unknown_streams: vec![],
            object_inventory: None,
            object_graph: None,
            cross_reference: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNode {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub children: Vec<StorageNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntryKind {
    Root,
    Storage,
    Stream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    pub path: String,
    pub size: u64,
    pub preview_ascii: Vec<String>,
    pub magic_u32_le: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SummaryInfo {
    pub creating_application: Option<String>,
    pub template: Option<String>,
    pub title: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub raw: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralMeta {
    pub file_path: Option<String>,
    pub file_size: Option<String>,
    pub raw_xml: String,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JProperties {
    pub strings: Vec<String>,
    pub key_values: BTreeMap<String, String>,
    pub guids: Vec<String>,
    pub raw_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedStream {
    pub name: String,
    pub size: u64,
    pub preview_ascii: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHeader {
    pub magic: u32,
    pub record_count: u32,
    pub stream_type: u16,
    pub body_len: u32,
    pub flags: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedString {
    pub index: u32,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterKind {
    PsmCluster,
    StyleCluster,
    DynamicAttributesMetadata,
    Sheet,
    UnclusteredDynamicAttributes,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Empty,
}

/// Probe metadata for PSMcluster0 string-table heuristic.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipTrailingToken {
    pub offset: usize,
    /// Human label indicating *where* the token lives, e.g.
    /// `"after_marker+6"`.
    pub label: String,
    pub value: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnknownStream {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
    /// Four-character ASCII rendering of `magic_u32_le` (e.g. "toor", "tseg").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_tag: Option<String>,
}

/// Decoded `PSMroots` stream: list of root-level named entries.
///
/// Byte layout (observed): `[u32 magic='root']` followed by N records of
/// `[u32 id][u32 char_count][UTF-16LE name]`. There is no explicit count;
/// parsing runs until the stream is exhausted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsmRoots {
    pub size: u64,
    pub entries: Vec<PsmRootEntry>,
    /// Bytes that could not be interpreted as `[id][char_count][utf16]` records.
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsmClusterTable {
    pub size: u64,
    pub count: u32,
    pub entries: Vec<PsmClusterEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsmClusterEntry {
    /// Decoded UTF-16LE cluster name, e.g. "PSMcluster0", "Sheet6".
    pub name: String,
    /// Offset inside the stream where the UTF-16LE name begins.
    pub name_offset: usize,
}

/// Decoded `PSMsegmenttable` stream. In sampled file it is a fixed 12 bytes:
/// `[magic 'stab'][u32 count=4][4 × 0x01]`. Schema is likely a per-segment
/// flag array; we expose the raw payload until semantics are confirmed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsmSegmentTable {
    pub size: u64,
    pub count: u32,
    pub flags: Vec<u8>,
}

/// Decoded `DocVersion3` stream: fixed-size (48 bytes per record) version
/// log entries that record a document's save history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionHistory {
    pub size: u64,
    pub records: Vec<VersionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    /// Null-terminated ASCII product identifier, e.g. "SmartPlantPID.a".
    pub product: String,
    /// Null-terminated ASCII version string, e.g. "090000.0144".
    pub version: String,
    /// Operation code: observed values "SA" (save-as / create) and
    /// "SV" (save / modify).
    pub operation: String,
    /// Null-terminated ASCII timestamp, e.g. "12/29/25 10:45".
    pub timestamp: String,
}

/// Decoded `AppObject` stream: registry of external COM / DLL plugins the
/// source application linked to this drawing.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedTextStorageList {
    pub size: u64,
    pub list_name: String,
    pub entries: Vec<TaggedTextStorageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedTextStorageEntry {
    /// Storage directory name (e.g. "TaggedTxtData").
    pub storage_name: String,
}

/// Raw preservation of the `DocVersion2` stream. The 48-byte binary payload
/// is not yet structurally decoded; we record its magic and hex preview so
/// that downstream tooling can round-trip or inspect without losing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocVersion2Raw {
    pub size: u64,
    pub magic_u32_le: u32,
    /// Lowercase hex dump of the full payload (up to 128 bytes).
    pub hex_preview: String,
}

/// Structured P&ID object graph — the core deliverable that ties together
/// the `P&IDAttributes` DA records into a queryable view of the drawing's
/// modeled objects and their relationships.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

/// A single modeled object on the drawing. Mostly sourced from a single
/// `P&IDAttributes` DA record.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ---- Cross-reference graph ---------------------------------------------------

/// Stitches already-decoded pieces of the document into a small relational
/// view. Pure derivation — requires no extra I/O.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootPresence {
    pub name: String,
    pub id: u32,
    pub found_as_storage: bool,
    pub found_as_stream: bool,
}
