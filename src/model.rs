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

    pub unknown_streams: Vec<UnknownStream>,

    /// P&ID object inventory derived from Dynamic Attributes records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_inventory: Option<ObjectInventory>,
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
            unknown_streams: vec![],
            object_inventory: None,
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
