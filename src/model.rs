//! Canonical decoded model for a `SmartPlant` `.pid` file.
//!
//! This module is intentionally large: it is the single source of
//! truth for every shape the reader produces and the writer consumes.
//! The root type — [`PidDocument`] — aggregates the CFB tree, the
//! stream inventory, high-level metadata (summary, drawing, general),
//! business-level decodes (clusters, `JSites`, dynamic attributes,
//! sheets, PSM tables, object graph, object inventory), optional
//! derived views (cross-reference, layout), and the `DocVersion` /
//! `AppObject` / tagged-text auxiliary blocks.
//!
//! The structs here derive [`serde::Serialize`] / [`serde::Deserialize`]
//! and [`schemars::JsonSchema`], so they double as the JSON-schema
//! surface emitted by [`crate::schema`]. Any field add/rename is a
//! schema change — keep deprecations explicit.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fully decoded view of a `SmartPlant` `.pid` file.
///
/// Produced by [`crate::api::PidParser::parse_file`] (or, with raw
/// bytes retained, by [`crate::api::PidParser::parse_package`]). Most
/// fields are populated eagerly by the reader; [`object_graph`],
/// [`cross_reference`], [`layout`] and [`object_inventory`] are
/// filled in by follow-on passes ([`crate::crossref::build_graph`],
/// [`crate::layout::derive_layout`], the reader's own inventory pass)
/// and are [`Option`]al so partially-decoded documents remain usable.
///
/// [`object_graph`]: Self::object_graph
/// [`cross_reference`]: Self::cross_reference
/// [`layout`]: Self::layout
/// [`object_inventory`]: Self::object_inventory
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidDocument {
    /// CFB storage / stream hierarchy as visited by the reader. Every
    /// stream entry in [`Self::streams`] is reachable through this
    /// tree — use one or the other depending on whether you want a
    /// path view ([`Self::streams`]) or a hierarchical view.
    pub cfb_tree: StorageNode,
    /// Flat inventory of every CFB stream the reader walked, with
    /// size / preview / magic metadata. Companion view of
    /// [`Self::cfb_tree`]; paths use the usual `/A/B` form.
    pub streams: Vec<StreamEntry>,

    /// Decoded OLE `SummaryInformation` (title, template, timestamps,
    /// Phase 10j user-defined property dictionary). `None` means the
    /// stream was absent or too short to parse.
    pub summary: Option<SummaryInfo>,
    /// Decoded `TaggedTxtData/Drawing` XML — drawing number,
    /// `SmartPlant` template / rules / formats / symbology / gapping
    /// UIDs, plus every tag seen in the body.
    pub drawing_meta: Option<DrawingMeta>,
    /// Decoded `TaggedTxtData/General` XML — file path / file size
    /// plus the rest of the tag bag.
    pub general_meta: Option<GeneralMeta>,

    /// All `JSite` storages found at the top level of the compound
    /// file. Each entry retains its symbol-path hint and decoded
    /// `JProperties` if `ParseOptions::parse_jsite_properties`
    /// stayed on.
    pub jsites: Vec<JSite>,
    /// Raw `PSMcluster0` / `StyleCluster` / etc. clusters as
    /// observed on disk. Higher-level views live in
    /// [`Self::object_graph`] / [`Self::cross_reference`].
    pub clusters: Vec<ClusterInfo>,

    /// `Dynamic Attributes Metadata` + `Unclustered Dynamic
    /// Attributes` pair, if present. Phase 8 onwards these feed
    /// [`Self::object_inventory`] and [`Self::object_graph`].
    pub dynamic_attributes: Option<DynamicAttributesBlob>,
    /// `Sheet*` storages with their embedded probe data. Order
    /// matches CFB traversal; sheet byte-range patches from the
    /// writer operate against these.
    pub sheet_streams: Vec<SheetStream>,

    /// Optional `/PSMroots` decode — the top-level list of named
    /// style / drawing property collections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_roots: Option<PsmRoots>,
    /// Optional `/PSMclustertable` decode — index from cluster
    /// IDs to cluster metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_cluster_table: Option<PsmClusterTable>,
    /// Optional `/PSMsegmenttable` decode — index from segment
    /// IDs to segment metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psm_segment_table: Option<PsmSegmentTable>,

    /// Optional `/DocVersion3` history. Mutually exclusive-ish with
    /// [`Self::doc_version2`]: which one is populated depends on
    /// which version of `SmartPlant` wrote the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_history: Option<VersionHistory>,
    /// Optional `/AppObject` registry — application-level object
    /// inventory carried alongside the main P&ID payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_object_registry: Option<AppObjectRegistry>,
    /// Optional `/JTaggedTxtStgList` + `/TaggedTxtData/*` decode —
    /// a general-purpose named-text storage system used by the
    /// `Drawing` / `General` metadata XMLs and a few others.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagged_storages: Option<TaggedTextStorageList>,

    /// Raw `/DocVersion2` bytes plus a magic/length summary,
    /// retained independently of the structured decode for audit
    /// and unknown-layout investigations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_version2: Option<DocVersion2Raw>,

    /// Structured decoding of `/DocVersion2` (v0.3.8+). Present only when
    /// the stream matches the known layout (magic `0x0001_0034` + N ×
    /// 9-byte records); `doc_version2` raw is always populated in
    /// parallel for audit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_version2_decoded: Option<DocVersion2>,

    /// Every top-level stream whose name didn't match any registered
    /// decoder (see [`crate::inspect::KNOWN_TOP_LEVEL_STREAM_NAMES`]).
    /// This is a decoded diagnostic inventory controlled by
    /// [`crate::api::ParseOptions::keep_unknown_streams`]; package-side raw
    /// bytes are retained separately for writer passthrough.
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
    /// (PSM declarations ↔ actual clusters, `JSite` ↔ symbols, DA class ↔ records,
    /// `PSMroots` ↔ cfb tree). Derived from `PidDocument` in a second pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_reference: Option<CrossReferenceGraph>,

    /// Readable whole-drawing layout derived from semantic graph topology,
    /// representation hints, and known symbol categories. This is a
    /// visualization-oriented layout model, not a byte-for-byte `SmartPlant`
    /// geometry decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<PidLayoutModel>,
}

/// Summary inventory of P&ID objects in the drawing.
///
/// Derived from the Dynamic Attributes records during the reader
/// pipeline; see [`Self::items`] for the flat list and
/// [`Self::item_counts`] for a `ModelItemType → count` breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ObjectInventory {
    /// Drawing-level identifier parsed out of the `DrawingNo`
    /// attribute (32-hex-char GUID form).
    pub drawing_id: Option<String>,
    /// `ProjectNumber` attribute — owning project number.
    pub project: Option<String>,
    /// Count of each `ModelItemType` seen in the DA stream (e.g.
    /// `"PipeRun" -> 12`). Mirrors
    /// [`ObjectGraph::counts_by_type`] but aggregated across
    /// non-relationship and relationship items.
    pub item_counts: BTreeMap<String, usize>,
    /// Flat list of decoded items (one entry per DA object record).
    pub items: Vec<PidItem>,
}

/// A single identifiable P&ID item (instrument, pipe, equipment, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PidItem {
    /// `ModelItemType` as it appears in the DA record (e.g.
    /// `"PipeRun"`, `"Instrument"`, `"Nozzle"`).
    pub item_type: String,
    /// 32-hex-char drawing-scoped identifier. `None` when the DA
    /// record did not carry a `DrawingID` attribute.
    pub drawing_id: Option<String>,
    /// `ModelID` attribute — rare in our samples but preserved
    /// when present.
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

/// One node in the CFB directory tree as walked by the reader. Hosts
/// either a storage (a "directory", which owns [`Self::children`]) or a
/// stream (a leaf, `children` empty). Used alongside the flat
/// [`StreamEntry`] inventory via [`PidDocument::cfb_tree`] /
/// [`PidDocument::streams`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StorageNode {
    /// Last path segment — the local entry name as stored in the
    /// compound file directory (e.g. `"JSite0001"`).
    pub name: String,
    /// Full `/`-joined path from the root, matching
    /// [`StreamEntry::path`] for stream nodes.
    pub path: String,
    /// Root / storage / stream discriminator.
    pub kind: EntryKind,
    /// Child entries when [`Self::kind`] is
    /// [`EntryKind::Root`] / [`EntryKind::Storage`]; always empty for
    /// [`EntryKind::Stream`].
    pub children: Vec<StorageNode>,
}

/// Discriminator for [`StorageNode::kind`] — mirrors the three CFB
/// directory-entry classes `SmartPlant` uses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub enum EntryKind {
    /// Compound-file root entry at path `/`. Exactly one per document.
    Root,
    /// Intermediate storage ("directory"): no payload bytes, only
    /// children.
    Storage,
    /// Leaf stream: carries bytes and shows up in
    /// [`PidDocument::streams`].
    Stream,
}

/// Flat inventory view of one CFB stream — companion to
/// [`StorageNode`], emitted in [`PidDocument::streams`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StreamEntry {
    /// Full `/`-joined CFB path (e.g. `"/JSite0001/JProperties"`).
    pub path: String,
    /// Byte length reported by the CFB directory entry.
    pub size: u64,
    /// ASCII-ish tokens extracted from the body for quick eyeballing;
    /// never decodes binary data, always a best-effort preview.
    pub preview_ascii: Vec<String>,
    /// First 4 little-endian bytes of the stream interpreted as `u32`,
    /// or `None` when the stream is shorter than 4 bytes. Callers use
    /// this as the primary dispatch key when classifying streams.
    pub magic_u32_le: Option<u32>,
}

/// Decoded OLE `/\x05SummaryInformation` (section 1) plus
/// `/\x05DocumentSummaryInformation` section 2's user-defined
/// property dictionary.
///
/// The five first-class fields cover the properties that show up in
/// every `SmartPlant` save (`PIDPROPID_TITLE`, `PIDPROPID_TEMPLATE`,
/// etc.); the catch-all [`Self::raw`] map preserves the string form
/// of every other property so nothing is silently dropped.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SummaryInfo {
    /// `PID_APPNAME` (PROPID 18) — the `SmartPlant` build that wrote
    /// the file (e.g. `"SPPID 11.0"`).
    pub creating_application: Option<String>,
    /// `PID_TEMPLATE` (PROPID 7) — the template drawing name.
    pub template: Option<String>,
    /// `PID_TITLE` (PROPID 2) — the free-form drawing title.
    pub title: Option<String>,
    /// `PID_CREATE_DTM` (PROPID 12) — ISO-8601 rendering of the
    /// creation `VT_FILETIME`.
    pub created_time: Option<String>,
    /// `PID_LASTSAVE_DTM` (PROPID 13) — ISO-8601 rendering of the
    /// last-modified `VT_FILETIME`.
    pub modified_time: Option<String>,
    /// Every other property rendered to a string key / string value.
    /// The key format is either the symbolic name from the property
    /// set dictionary or `"PROPID_{n}"` when the name is unknown.
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
/// dictionary in `DocumentSummaryInformation` section 2.
///
/// Covers the VT codes `SmartPlant` practically emits in section 2;
/// unknown VTs fall through to [`SummaryPropertyValue::Raw`] so the
/// round-trip remains safe (writer passes them through verbatim).
///
/// # Wire format
///
/// Adjacently-tagged JSON (`{"kind": "…", "value": …}`). The previous
/// internally-tagged attribute (`tag = "kind"` without `content`)
/// could never serialize the newtype variants (`Lpstr` / `Lpwstr` /
/// `I4` / `Bool` / `Filetime`) because internally-tagged enums in
/// serde only support struct / unit variants — `serde_json::to_string`
/// would reject them with _"cannot serialize tagged newtype variant"_.
/// The `content = "value"` fixup lands `PidDocument::to_json` /
/// `cargo run --example parse_walkthrough` for any `.pid` whose
/// `DocumentSummaryInformation` carries user-defined properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SummaryPropertyValue {
    /// `VT_LPSTR` (0x001E) — single-byte string. Writer encodes by
    /// `MetadataUpdates.summary_user_updates_encoded` or UTF-8 default.
    Lpstr(String),
    /// `VT_LPWSTR` (0x001F) — UTF-16LE string.
    Lpwstr(String),
    /// `VT_I4` (0x0003).
    I4(i32),
    /// `VT_BOOL` (0x000B) — property-set representation is u16 0x0000 /
    /// 0xFFFF.
    Bool(bool),
    /// `VT_FILETIME` (0x0040) — raw 64-bit 100ns-since-1601 value.
    Filetime(u64),
    /// Any other VT the parser recognizes structurally but does not
    /// model explicitly; writer passes bytes through verbatim.
    /// Serialized as a plain JSON array of ints. If the size becomes
    /// a JSON-bloat concern in practice, a future Phase can swap to a
    /// base64 adaptor under a new wire version.
    Raw {
        /// MS-OLEPS VT code verbatim from the property header
        /// (e.g. `0x0002` for `VT_I2`).
        vt: u16,
        /// Payload bytes carried through unchanged so the writer can
        /// reproduce the property byte-for-byte.
        bytes: Vec<u8>,
    },
}

/// Decoded `TaggedTxtData/Drawing` XML — the headline drawing
/// metadata `SmartPlant` writes alongside every `.pid`.
///
/// Every `Option` is `None` when the corresponding XML element /
/// attribute wasn't present; the un-typed tag bag in [`Self::tags`]
/// preserves everything else so a consumer can recover any field we
/// haven't yet promoted to a named slot.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DrawingMeta {
    /// `<DrawingNumber>` / `SP_DRAWINGNUMBER` attribute.
    pub drawing_number: Option<String>,
    /// Free-form document category (e.g. `"Piping Documents"`).
    pub document_category: Option<String>,
    /// `SmartPlant` template file name the drawing was started from.
    pub template_name: Option<String>,
    /// UID of the rules set applied to the drawing.
    pub rules_uid: Option<String>,
    /// UID of the numbering / formatting rules set.
    pub formats_uid: Option<String>,
    /// UID of the line-gapping style set.
    pub gapping_uid: Option<String>,
    /// UID of the symbology style set.
    pub symbology_uid: Option<String>,
    /// UID of the fallback formats set when the primary set is absent.
    pub default_formats_uid: Option<String>,
    /// The original XML body, retained verbatim so
    /// [`crate::writer::PidWriter`] can reproduce bytes untouched when
    /// no metadata patch asks for a rewrite.
    pub raw_xml: String,
    /// Every tag pair the parser observed, keyed by local element
    /// name; the five UID-valued entries above are copies of
    /// corresponding keys here.
    pub tags: BTreeMap<String, String>,
}

/// Decoded `TaggedTxtData/General` XML — the file-scoped metadata
/// `SmartPlant` writes next to [`DrawingMeta`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct GeneralMeta {
    /// `<FilePath>` element — the `.pid` path inside the `SmartPlant`
    /// project tree.
    pub file_path: Option<String>,
    /// `<FileSize>` element rendered as-is (the XML ships it as a
    /// decimal string).
    pub file_size: Option<String>,
    /// Original XML body; see [`DrawingMeta::raw_xml`] for the
    /// round-trip rationale.
    pub raw_xml: String,
    /// Every tag pair the parser observed, keyed by local element
    /// name.
    pub tags: BTreeMap<String, String>,
}

/// One `JSite` top-level storage from the compound file — the
/// `SmartPlant` container that groups a symbol instance with its
/// property blob, OLE links, and embedded raw streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct JSite {
    /// Name of the `JSite*` storage as seen in the CFB tree.
    pub name: String,
    /// Full CFB path of the storage (e.g. `/JSite123`).
    pub path: String,
    /// Local symbol name decoded from the `JSite` header when present.
    pub symbol_name: Option<String>,
    /// `SmartPlant` symbol-library path (`SymbolPath`), typically
    /// something like `Piping\Valves\GateValve`.
    pub symbol_path: Option<String>,
    /// Workstation-relative symbol path — the on-disk variant that
    /// can differ from [`Self::symbol_path`] when the symbol library
    /// is remapped.
    pub local_symbol_path: Option<String>,
    /// Whether the `JSite` storage carries a `\x01Ole` sub-stream
    /// (i.e. the symbol embeds an OLE object).
    pub has_ole_stream: bool,
    /// Any `\x01CompObj` / `\x03ObjInfo` links pulled out of the
    /// OLE sub-streams, in the order the reader observed them.
    pub ole_links: Vec<String>,
    /// Decoded `JProperties` blob — the `SmartPlant` dynamic
    /// property payload carried by this site.
    pub properties: JProperties,
    /// Every other embedded stream found inside the storage, sized
    /// and previewed for audit.
    pub raw_streams: Vec<EmbeddedStream>,
}

/// Flattened `JProperties` payload — strings, key/value pairs, GUID
/// references, and the raw-blob length, all extracted from a `JSite`
/// property stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct JProperties {
    /// Every UTF-8 / UTF-16 string recovered from the property blob.
    pub strings: Vec<String>,
    /// String pairs the parser could resolve as `key=value` entries.
    pub key_values: BTreeMap<String, String>,
    /// 32-hex-char GUIDs observed in the blob.
    pub guids: Vec<String>,
    /// Byte length of the original blob, retained so consumers can
    /// compare against expected sizes without re-reading the stream.
    pub raw_len: usize,
}

/// One embedded raw stream inside a `JSite` (or similar) storage —
/// just enough metadata for an inspect report; full bytes stay on
/// the [`crate::package::PidPackage`] side when round-tripping.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddedStream {
    /// Local stream name (last path segment).
    pub name: String,
    /// Size in bytes as reported by the CFB directory.
    pub size: u64,
    /// ASCII-ish preview tokens extracted from the stream body —
    /// handy for spotting stringly-typed payloads in a glance.
    pub preview_ascii: Vec<String>,
}

/// One cluster-family stream as it lives on disk — raw metadata plus,
/// when the reader recognized the layout, a structured [`ClusterHeader`]
/// and string table. Both PSM clusters and style clusters are modelled
/// through this common shape; [`Self::kind`] resolves the subtype.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterInfo {
    /// Local CFB name (last path segment), e.g. `"PSMcluster0"`.
    pub name: String,
    /// Full `/`-joined CFB path the stream was read from.
    pub path: String,
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// Leading `u32` LE of the stream — `0x6C90F544` for the shared
    /// cluster magic; `None` when the stream is shorter than 4 bytes.
    pub magic_u32_le: Option<u32>,
    /// ASCII-ish strings the reader lifted from the payload for quick
    /// diagnostics; ordering matches on-disk occurrence.
    pub extracted_strings: Vec<String>,
    /// Subtype classification derived from the stream name / content.
    pub kind: ClusterKind,
    /// Decoded [`ClusterHeader`] when the stream matched the standard
    /// `0x6C90F544` layout; `None` on unrecognized headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ClusterHeader>,
    /// Sequenced string table recovered by the string-table probe.
    /// `None` when the probe could not locate a table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_table: Option<Vec<IndexedString>>,
    /// Probe metadata for string-table detection heuristic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_info: Option<ClusterProbeInfo>,
}

/// Common header shared by all streams with magic `0x6C90F544`
/// (`PSMcluster*`, `StyleCluster`, `Dynamic Attributes Metadata`, and
/// `Sheet*`). Layout was reverse-engineered across several fixtures; see
/// `src/parsers/cluster_header.rs` for the byte-level reference.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterHeader {
    /// Fixed `u32` LE signature — always `0x6C90F544` for decoded headers.
    pub magic: u32,
    /// Number of records advertised by the header; may disagree with
    /// the count of records the probe actually decodes (drift signal).
    pub record_count: u32,
    /// Discriminator the writer uses to pick a stream subtype; in
    /// practice tracks [`ClusterKind`] but not 1:1 for every file.
    pub stream_type: u16,
    /// Declared body byte length (header + payload). Not always exact
    /// on older fixtures, so downstream code cross-checks with
    /// [`ClusterInfo::size`].
    pub body_len: u32,
    /// Opaque 16-bit flag field — values have not been fully decoded;
    /// callers round-trip them verbatim.
    pub flags: u16,
}

/// One entry `(index, value)` of a cluster string table.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexedString {
    /// Zero-based position in the table (matches on-disk order).
    pub index: u32,
    /// Decoded string value; encoding follows the owning cluster's
    /// conventions (UTF-16LE for PSM clusters, UTF-8 elsewhere).
    pub value: String,
}

/// Cluster-stream subtype discriminator used by [`ClusterInfo::kind`].
/// Matches the well-known top-level stream families the reader knows
/// how to probe; unknown / new families fall through to
/// [`Self::Unknown`] without silently dropping bytes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ClusterKind {
    /// `/PSMcluster*` — the main property-set cluster family.
    PsmCluster,
    /// `/StyleCluster` — drawing-style cluster.
    StyleCluster,
    /// `/Dynamic Attributes Metadata` — DA schema definition stream.
    DynamicAttributesMetadata,
    /// `/Sheet*` — per-sheet payload stream (endpoint-pair records live
    /// here; detail lives on [`SheetStream`]).
    Sheet,
    /// `/Unclustered Dynamic Attributes` — the payload side of the DA
    /// metadata stream; carries the actual attribute records.
    UnclusteredDynamicAttributes,
    /// Stream with cluster magic but unclassified name.
    Unknown,
}

/// Decoded `/Unclustered Dynamic Attributes` stream — the payload that
/// pairs with `/Dynamic Attributes Metadata` to form the DA system.
/// Carries the raw string/relationship bag plus, when the probe
/// succeeded, structured attribute / trailer / relationship records.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicAttributesBlob {
    /// Full `/`-joined CFB path of the stream.
    pub path: String,
    /// Stream size in bytes.
    pub size: u64,
    /// Leading `u32` LE of the stream (cluster magic when present).
    pub magic_u32_le: Option<u32>,
    /// Every UTF-8 / UTF-16 run the string-scan probe recovered;
    /// order matches on-disk occurrence.
    pub strings: Vec<String>,
    /// `Relationship.<GUID>` tags discovered inside the payload.
    pub relationships: Vec<String>,
    /// Attribute class names identified inside the payload
    /// (e.g. `"P&IDAttributes"`, `"DrawingAttributes"`).
    pub class_names: Vec<String>,
    /// Short hex preview of the first bytes of the stream — aids
    /// quick visual inspection in diagnostic reports.
    pub raw_preview_hex: String,
    /// Decoded cluster header when the stream starts with the standard
    /// `0x6C90F544` layout.
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
    /// `None` for relationships (which don't carry a `DrawingID`) and for
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
    /// Class name verbatim from the DA payload
    /// (e.g. `"P&IDAttributes"`, `"DrawingAttributes"`).
    pub class_name: String,
    /// Decoded `(name, value)` fields for this record, in on-disk order.
    pub attributes: Vec<AttributeField>,
    /// Confidence level: "heuristic" for probe-derived, "decoded" for verified.
    #[serde(default = "default_confidence")]
    pub confidence: String,
}

fn default_confidence() -> String {
    "heuristic".to_string()
}

/// A single `(name, value)` attribute field inside an
/// [`AttributeRecord`]. Mirrors one key-value pair from the DA payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributeField {
    /// Attribute name verbatim from the DA record
    /// (e.g. `"ModelItemType"`, `"DrawingID"`).
    pub name: String,
    /// Decoded attribute value; see [`AttributeValue`] for the
    /// supported variants.
    pub value: AttributeValue,
    /// Audit trail: when the heuristic value decoder strips a leading
    /// prefix byte (see `dynamic_attr_records::strip_value_prefix`), the
    /// pre-strip string is recorded here so callers can detect and
    /// override the heuristic. `None` means no stripping occurred.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw_value: Option<String>,
}

/// Decoded value of an [`AttributeField`]. `#[serde(untagged)]` so JSON
/// carries the raw scalar (no `"kind": "…"` wrapping) — matches the way
/// the DA payload itself is stringly-typed on disk.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AttributeValue {
    /// Free-form string value (the common case).
    Text(String),
    /// Signed 64-bit integer when the decoder could resolve the bytes
    /// as an integer scalar.
    Integer(i64),
    /// 64-bit floating-point value.
    Float(f64),
    /// Attribute present in the record but without a resolvable value.
    Empty,
}

/// Probe metadata for `PSMcluster0` string-table heuristic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterProbeInfo {
    /// Byte offset where the string table was detected.
    pub string_table_offset: usize,
    /// Method used to locate the start: "`entry2_backtrack`" or "fallback".
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
    /// Absolute byte offset of the `u16` inside the DA stream.
    pub offset: usize,
    /// Human label indicating *where* the token lives, e.g.
    /// `"after_marker+6"`.
    pub label: String,
    /// Raw little-endian `u16` value read at [`Self::offset`].
    pub value: u16,
}

/// One `/Sheet*` stream — the reader's eager decode of the sheet-level
/// payload that hosts endpoint-pair records and writer byte-range patch
/// targets.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SheetStream {
    /// Local CFB name, e.g. `"Sheet6"`.
    pub name: String,
    /// Full `/`-joined CFB path (matches [`StreamEntry::path`]).
    pub path: String,
    /// Stream size in bytes reported by the CFB directory.
    pub size: u64,
    /// Text runs the string-scan probe lifted out of the payload, in
    /// on-disk order. Useful for dump-style reports.
    pub extracted_texts: Vec<String>,
    /// Leading `u32` LE of the stream (e.g. cluster magic or a 4-char
    /// sheet-specific tag); `None` when the stream is too short.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_u32_le: Option<u32>,
    /// Four-character ASCII rendering of `magic_u32_le` (e.g. "DF90", "tseg").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic_tag: Option<String>,
    /// Decoded [`ClusterHeader`] when the stream opens with the
    /// shared cluster layout; `None` otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ClusterHeader>,
    /// Structured attribute records extracted from the sheet stream (heuristic).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub attribute_records: Vec<AttributeRecord>,
    /// Probe summary: heuristic scan metadata (`body_start_offset`, `marker_count`, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_summary: Option<ProbeSummary>,
    /// Stable DTO surface for normalized Sheet evidence. Populated
    /// incrementally as Sheet text, endpoint and coordinate probes
    /// graduate into contract-backed views.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<SheetGeometry>,
    /// Endpoint-pair records decoded from the sheet. Each entry maps a
    /// Relationship's `field_x` to the `(endpoint_a, endpoint_b)` `field_x`
    /// pair of the two objects it connects.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub endpoint_records: Vec<SheetEndpointRecord>,
    /// Soft failure captured while trying to extract endpoint-pair records
    /// from this sheet. `None` means extraction either succeeded or had no
    /// relationship fields to look for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_decode_error: Option<String>,
}

/// Stable Sheet-level DTO that groups normalized text, endpoint and
/// coordinate evidence without claiming full CAD geometry decode.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SheetGeometry {
    /// Text runs normalized from the Sheet probe layer.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub texts: Vec<SheetText>,
    /// Endpoint records normalized from the Sheet endpoint parser.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub endpoints: Vec<SheetEndpoint>,
    /// Coordinate-like pairs retained as hints until record semantics
    /// are proven across more fixtures.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub coordinate_hints: Vec<SheetCoordinateHintDto>,
    /// Future object-to-geometry mapping evidence. Empty until a Sheet
    /// probe can prove that an object `field_x` owns source-backed
    /// geometry coordinates.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub object_geometry_hints: Vec<SheetObjectGeometryHint>,
}

/// Stable text run DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetText {
    /// Byte offset where the text run begins inside the Sheet stream.
    pub offset: usize,
    /// Encoding family (`"ascii"` or `"utf16_le"`).
    pub encoding: String,
    /// Decoded printable text.
    pub text: String,
    /// Number of bytes consumed by the run in the source Sheet stream.
    pub byte_len: usize,
}

/// Stable endpoint DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetEndpoint {
    /// Byte offset in the Sheet stream where this endpoint record starts.
    pub offset: usize,
    /// Relationship's `field_x` value.
    pub rel_field_x: u32,
    /// Source endpoint `field_x`.
    pub endpoint_a: u32,
    /// Target endpoint `field_x`.
    pub endpoint_b: u32,
}

/// Stable coordinate-hint DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetCoordinateHintDto {
    /// Byte offset of the first coordinate-like value.
    pub offset: usize,
    /// First coordinate-like value.
    pub x: i32,
    /// Second coordinate-like value.
    pub y: i32,
}

/// Candidate mapping from an object `field_x` to source-backed Sheet
/// geometry evidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetObjectGeometryHint {
    /// Byte offset where this candidate mapping starts inside the Sheet stream.
    pub offset: usize,
    /// Object Dynamic Attributes `field_x` this mapping appears to describe.
    pub field_x: u32,
    /// Optional coordinate associated with the object, when the probe can
    /// prove it came from the same source record.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub position: Option<SheetCoordinateHintDto>,
    /// Optional GraphicOID-like value surfaced near this mapping.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    /// Short diagnostic note describing why this is still a hint.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

/// Top-level stream the reader encountered but does not (yet)
/// interpret. This stores classification metadata only; bytes are
/// preserved via the package-side raw store.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnknownStream {
    /// Full `/`-joined CFB path of the stream.
    pub path: String,
    /// Stream size in bytes reported by the CFB directory.
    pub size: u64,
    /// Leading `u32` LE of the stream; `None` when shorter than 4
    /// bytes. First tool callers reach for when classifying unknowns.
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
    /// Entry is a leaf stream directly under the CFB root.
    TopLevelStream,
    /// Entry is a storage (directory) directly under the CFB root;
    /// inner streams contribute to the aggregated `stream_size`.
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
    /// Whether the entry is a bare stream or a storage aggregating
    /// inner streams; see [`CoverageNodeKind`].
    pub kind: CoverageNodeKind,
    /// How thoroughly the reader currently decodes this node — used
    /// to bucket the coverage report.
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
    /// One row per top-level CFB node, sorted ascending by
    /// [`CoverageEntry::name`].
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
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// Parsed `(id, offset, name)` records in on-disk order.
    pub entries: Vec<PsmRootEntry>,
    /// Bytes that could not be interpreted as `[id][char_count][utf16]` records.
    pub trailing_bytes: usize,
}

/// One decoded record from a [`PsmRoots`] stream.
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
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// Declared number of records in the table header; cross-check
    /// against `entries.len()` for drift.
    pub count: u32,
    /// Parsed entries in on-disk order.
    pub entries: Vec<PsmClusterEntry>,
    /// Conservative decoded view for each cluster record, derived from
    /// observed stable prefix slots across real fixtures. This is additive:
    /// callers that only need the legacy name/probe view can keep using
    /// [`PsmClusterTable::entries`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decoded_records: Vec<PsmClusterRecordDecoded>,
    /// Bytes after the last record that could not be attributed to any entry.
    #[serde(default)]
    pub trailing_bytes: usize,
}

/// Conservative decoded candidate fields for one `PSMclustertable` record.
///
/// Field names intentionally keep the `candidate_` prefix where `SmartPlant`
/// semantics are not fully proven yet. They are stable byte-layout evidence,
/// not final business meaning.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PsmClusterRecordDecoded {
    /// Zero-based order in the on-disk table.
    pub index: usize,
    /// Decoded cluster name copied from the legacy [`PsmClusterEntry`] view.
    pub name: String,
    /// Offset inside the `PSMclustertable` stream where this record starts.
    pub record_offset: usize,
    /// Total byte length of this record.
    pub record_len: usize,
    /// Number of bytes before the UTF-16LE name run.
    pub prefix_len: usize,
    /// Candidate UTF-16LE name byte length including the terminating NUL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_bytes_with_nul: Option<u32>,
    /// Candidate cluster ordinal observed in stable prefix slots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_ordinal: Option<u16>,
    /// Candidate marker that splits sheet (`0`) from non-sheet (`1`) rows
    /// in the sampled fixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_non_sheet_marker: Option<u8>,
    /// Candidate trailing payload index present on sampled non-sheet rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_non_sheet_payload_index: Option<u32>,
    /// Confidence string for this decoded view. Phase 11a starts at
    /// `"medium"` until field semantics are proven across more fixtures.
    pub confidence: String,
    /// Source byte ranges for the decoded candidate fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_ranges: Vec<DecodedFieldRange>,
    /// Prefix bytes not covered by the conservative candidate layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_prefix_bytes: Vec<u8>,
}

/// Byte range for a decoded candidate field inside a raw stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedFieldRange {
    /// Field name as exposed by the decoded candidate view.
    pub field_name: String,
    /// Start offset inside the source stream, inclusive.
    pub start: usize,
    /// End offset inside the source stream, exclusive.
    pub end: usize,
}

/// One declared cluster entry inside a [`PsmClusterTable`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmClusterEntry {
    /// Decoded UTF-16LE cluster name, e.g. "`PSMcluster0`", "Sheet6".
    pub name: String,
    /// Offset inside the stream where the UTF-16LE name begins.
    pub name_offset: usize,
    /// Offset inside the stream where this record starts (including prefix).
    #[serde(default)]
    pub record_offset: usize,
    /// Total byte length of this record (prefix + name bytes).
    #[serde(default)]
    pub record_len: usize,
    /// Raw bytes between record start and the name. Contains per-record
    /// header fields whose semantics are not yet fully understood.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix_bytes: Vec<u8>,
    /// Phase 11a-probe: byte-level probe summary for this record. Purely
    /// derived from `prefix_bytes` + the record slice and carries no
    /// semantic claims (e.g. does not name `cluster_id` / `flags`).
    /// Present so reverse-engineering against a second fixture can hint
    /// at field layouts without forcing premature decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<PsmClusterRecordProbe>,
}

/// Phase 11a-probe — byte-level summary of a single `PSMclustertable` record.
/// All fields are computed purely from the raw bytes; none of them claim
/// semantic meaning. Use for visual inspection and fixture-drift detection;
/// do not consume from `layout` / `import_view`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct PsmClusterRecordProbe {
    /// First 4 bytes of `prefix_bytes` interpreted as little-endian u32.
    /// `None` when the prefix is shorter than 4 bytes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub first_u32_le: Option<u32>,
    /// Last 4 bytes of the full record (prefix + name run + optional null
    /// terminator) interpreted as little-endian u32. `None` when the
    /// record is shorter than 4 bytes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_u32_le: Option<u32>,
    /// Space-separated uppercase hex of `prefix_bytes` (empty when prefix
    /// is empty).
    pub prefix_hex: String,
    /// Space-separated uppercase hex of the last up-to-8 bytes of the
    /// record (shorter than 8 when the record itself is shorter).
    pub trailer_hex: String,
    /// Number of Unicode scalar values in `name` (== 1 byte-per-char for
    /// ASCII cluster names, but kept as char count for clarity).
    pub name_char_count: usize,
}

/// Decoded `PSMsegmenttable` stream. In sampled file it is a fixed 12 bytes:
/// `[magic 'stab'][u32 count=4][4 × 0x01]`. Schema is likely a per-segment
/// flag array; we expose the raw payload until semantics are confirmed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmSegmentTable {
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// `u32` after the 4-byte magic; interpreted as segment count on
    /// known fixtures but still treated as opaque by the reader.
    pub count: u32,
    /// Legacy flat flags array, kept for backward compatibility.
    pub flags: Vec<u8>,
    /// Per-segment structured entries (index + offset + flag).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<PsmSegmentEntry>,
    /// Bytes after the last flag that could not be attributed.
    #[serde(default)]
    pub trailing_bytes: usize,
}

/// A single segment entry from `PSMsegmenttable`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PsmSegmentEntry {
    /// Zero-based index within the table.
    pub index: usize,
    /// Byte offset within the stream (relative to stream start).
    pub offset: usize,
    /// The raw flag byte for this segment.
    pub flag: u8,
    /// Candidate positional link into `PSMclustertable`, populated only
    /// when segment and cluster entry counts match exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_owner_cluster_index: Option<usize>,
    /// Candidate owner cluster name from `PSMclustertable`, populated
    /// alongside [`Self::candidate_owner_cluster_index`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_owner_cluster_name: Option<String>,
    /// Phase 11b-probe: byte-level probe summary for this segment. Purely
    /// derived from the raw stream and carries no semantic claims (e.g.
    /// does not name `kind` / `role`). Present so reverse-engineering
    /// against a second fixture can hint at field layouts without forcing
    /// premature decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<PsmSegmentRecordProbe>,
}

/// Phase 11b-probe — byte-level summary of a single `PSMsegmenttable` entry.
/// All fields are computed purely from the raw stream + surrounding bytes;
/// none of them claim semantic meaning. Use for visual inspection and
/// fixture-drift detection; do not consume from `layout` / `import_view`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct PsmSegmentRecordProbe {
    /// Two-digit uppercase hex of the flag byte (e.g. `"01"`).
    pub flag_hex: String,
    /// Space-separated uppercase hex of the `±3`-byte window around this
    /// segment's flag inside the raw stream (1..=7 tokens depending on
    /// proximity to the stream boundaries).
    pub neighbor_window_hex: String,
    /// Absolute byte offset inside the `PSMsegmenttable` stream where the
    /// flag byte lives (equal to `PsmSegmentEntry.offset`, kept for
    /// self-contained probe consumers).
    pub stream_offset: usize,
    /// **Hint**, not a claim — populated only when the number of segment
    /// entries equals the number of `PSMclustertable` entries (so a 1:1
    /// positional mapping is the most natural guess). `None` whenever the
    /// two counts disagree, or the cluster table is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_cluster_hint: Option<String>,
}

/// Decoded `DocVersion3` stream: fixed-size (48 bytes per record) version
/// log entries that record a document's save history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VersionHistory {
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// One record per observed save, in stream order
    /// (oldest first).
    pub records: Vec<VersionRecord>,
    /// Fixed record size in bytes (always 48 for known samples).
    #[serde(default = "default_record_size")]
    pub record_size: usize,
    /// Bytes after the last complete record that could not be interpreted.
    #[serde(default)]
    pub trailing_bytes: usize,
}

fn default_record_size() -> usize {
    48
}

/// One decoded entry from a [`VersionHistory`] stream — a single
/// save / save-as event preserved by `SmartPlant`.
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
    /// Byte offset of this record within the stream.
    #[serde(default)]
    pub offset: usize,
}

impl VersionRecord {
    /// True iff `operation == "SA"`, mirroring `DocVersion2` `op_type`
    /// `0x82` (`SaveAs` / create) observed in Phase 9f sample analysis.
    pub fn is_save_as(&self) -> bool {
        self.operation == "SA"
    }

    /// True iff `operation == "SV"`, mirroring `DocVersion2` `op_type`
    /// `0x81` (Save / modify).
    pub fn is_save(&self) -> bool {
        self.operation == "SV"
    }

    /// True iff `operation` is one of the codes `SmartPlant` is known to
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
            offset: 0,
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
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// `u32` at offset 0; observed value `5` on the sampled file (likely
    /// entry count or registry version).
    pub leading_u32: u32,
    /// Decoded `(clsid, path)` entries in on-disk order.
    pub entries: Vec<AppObjectEntry>,
    /// Any bytes that could not be attributed to a full entry (e.g. trailing
    /// class-id-only record).
    pub trailing_bytes: usize,
}

/// One decoded `(clsid, path)` plugin entry inside an
/// [`AppObjectRegistry`].
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
/// (e.g. "`TaggedTxtStorages`") to the actual storage directory name
/// (e.g. "`TaggedTxtData`").
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaggedTextStorageList {
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// Storage-list name carried by the stream header
    /// (typically `"TaggedTxtStorages"`).
    pub list_name: String,
    /// One entry per referenced storage directory, in on-disk order.
    pub entries: Vec<TaggedTextStorageEntry>,
}

/// One decoded entry inside a [`TaggedTextStorageList`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaggedTextStorageEntry {
    /// Storage directory name (e.g. "`TaggedTxtData`").
    pub storage_name: String,
}

/// Raw preservation of the `DocVersion2` stream. The 48-byte binary payload
/// is not yet structurally decoded; we record its magic and hex preview so
/// that downstream tooling can round-trip or inspect without losing data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocVersion2Raw {
    /// Stream size in bytes as reported by the CFB directory.
    pub size: u64,
    /// Leading `u32` LE — `0x0001_0034` on every real sample; exposed
    /// so fixture drift can be spotted without re-reading the bytes.
    pub magic_u32_le: u32,
    /// Lowercase hex dump of the full payload (up to 128 bytes).
    pub hex_preview: String,
}

/// Structured decoding of the `/DocVersion2` stream (v0.3.8+).
///
/// `/DocVersion2` is a compact per-save version log, matching `/DocVersion3`
/// one-to-one (a `SaveAs` + N Saves). Format:
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
/// `SaveAs`, 0x81 = Save) and `version` (u32 LE) are the semantic fields;
/// the other bytes are carried through so round-trippers can still
/// reproduce the original byte stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocVersion2Record {
    /// Operation type byte — `0x82` (`SaveAs`) or `0x81` (Save) on
    /// known fixtures. See [`crate::parsers::doc_version2::op_type_label`]
    /// for the human-readable mapping.
    pub op_type: u8,
    /// Fixed `[0x00, 0x00, 0x09]` padding observed on every sample;
    /// carried through so round-trip bytes stay identical.
    pub fixed: [u8; 3],
    /// Record separator byte between the `fixed` padding and the
    /// version `u32`; value always `0x00` on decoded files.
    pub separator: u8,
    /// `u32` LE carrying the `SmartPlant` version number that produced
    /// this save entry (monotonic across the save history).
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
    /// Total relationships considered (i.e. `graph.relationships.len()`).
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
    /// O(N) — for hot loops with few `item_types`, callers can build a
    /// `BTreeMap<&str, Vec<&PidObject>>` themselves once.
    pub fn find_objects_by_item_type(&self, item_type: &str) -> Vec<&PidObject> {
        self.objects
            .iter()
            .filter(|o| o.item_type == item_type)
            .collect()
    }

    /// Linear scan: every object whose `extra` `BTreeMap` contains
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
    /// `drawing_ids`). Case-sensitive — `drawing_id`s are uppercase
    /// 32-hex by `SmartPlant` convention.
    ///
    /// `BTreeMap::range`-backed: O(log N + K) where K is the result
    /// length; faster than scanning `objects` linearly when only a
    /// short prefix is given.
    pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str> {
        if prefix.is_empty() {
            return self
                .by_drawing_id
                .keys()
                .map(std::string::String::as_str)
                .collect();
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
    /// `ModelItemType`: "`PipeRun`", "Nozzle", "Instrument", "Drawing", …
    pub item_type: String,
    /// `DrawingItemType`: "Symbol", "`LabelPersist`", "`ItemNote`", …
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
///   +8  [u8;6] zero padding
///   +14 u16  type = 0x0002     ← endpoint-record marker
///   +16 u32  endpoint_a        ← source field_x
///   +20 u16  0x0001
///   +22 u32  endpoint_b        ← target field_x
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

/// Heuristic visualization-oriented layout derived by
/// [`crate::layout::derive_layout`] from an [`ObjectGraph`]. Coordinates
/// are not `SmartPlant`'s own CAD geometry; they are a topology-driven
/// reconstruction suitable for overviews and diffs. Populated into
/// [`PidDocument::layout`] by the reader's post-pass.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PidLayoutModel {
    /// Placed object icons — one entry per primary drawing object that
    /// the layout routine was able to position.
    pub items: Vec<PidLayoutItem>,
    /// Pipeline / connection segments connecting the placed items.
    pub segments: Vec<PidLayoutSegment>,
    /// Free-standing text annotations (labels that aren't attached to
    /// a single [`PidLayoutItem`]).
    pub texts: Vec<PidLayoutText>,
    /// Objects present in the graph but skipped by the layout — kept
    /// so consumers can surface "not drawn" lists without losing data.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unplaced: Vec<PidLayoutUnplaced>,
    /// Human-readable warnings raised during layout synthesis (e.g.
    /// dropped relationships, failed placements). Non-fatal; the
    /// layout is still returned.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

/// One placed item inside a [`PidLayoutModel`] — typically an
/// equipment / instrument / nozzle symbol.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutItem {
    /// Layout-local identifier (e.g. `"item:<drawing_id>"`); stable
    /// across reruns for a given source drawing.
    pub layout_id: String,
    /// Back-pointer to the source [`PidObject::drawing_id`] when the
    /// layout could attribute the item to a concrete object.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    /// Representation `graphic_oid` from the cross-ref graph when
    /// available; carries the `SmartPlant` geometry handle.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    /// `ModelItemType` copied from the source object — drives icon /
    /// color choice on the rendering side.
    pub kind: String,
    /// Laid-out anchor point `[x, y]` in model units.
    pub anchor: [f64; 2],
    /// Axis-aligned bounding box `[x_min, y_min, x_max, y_max]` when
    /// the symbol hint yielded a size; `None` for zero-area items.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bounds: Option<[f64; 4]>,
    /// Representative symbol name (e.g. `"GateValve"`) when the
    /// [`JSite`] layer supplied one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol_name: Option<String>,
    /// Full symbol-library path, mirroring the `JSite` hint.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol_path: Option<String>,
    /// Attached display label when a nearby text record was promoted
    /// to this item (tag, equipment number, …).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    /// Copy of the source object's `ModelID` when present.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_id: Option<String>,
}

/// One placed connection segment inside a [`PidLayoutModel`] — typically a
/// pipe run or a signal line joining two [`PidLayoutItem`] endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutSegment {
    /// Layout-local identifier (e.g. `"seg:<rel_guid>"`).
    pub layout_id: String,
    /// `drawing_id` of the relationship that owns the segment when
    /// the layout could attribute it to a concrete [`PidRelationship`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub owner_drawing_id: Option<String>,
    /// Representation `graphic_oid` for the segment when available.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    /// Start point `[x, y]` in model units.
    pub start: [f64; 2],
    /// End point `[x, y]` in model units.
    pub end: [f64; 2],
    /// Free-form role tag (e.g. `"pipe"`, `"signal"`, `"reference"`)
    /// used by the renderer to pick a stroke style.
    pub role: String,
}

/// Free-standing text annotation inside a [`PidLayoutModel`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutText {
    /// Layout-local identifier (e.g. `"text:<drawing_id>"`).
    pub layout_id: String,
    /// Source object `drawing_id` when the text originates from one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    /// Rendered text contents verbatim.
    pub text: String,
    /// Anchor point `[x, y]` for the text baseline.
    pub anchor: [f64; 2],
    /// Axis-aligned bounding box `[x_min, y_min, x_max, y_max]` when
    /// the text extent could be estimated.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bounds: Option<[f64; 4]>,
}

/// Object that the layout chose not to place. Kept alongside
/// [`PidLayoutModel::items`] so consumers can show an "orphans" list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PidLayoutUnplaced {
    /// Source object's `drawing_id` when available.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    /// `ModelItemType` of the unplaced object.
    pub kind: String,
    /// Best display label the layout could lift (tag, equipment
    /// number, or fallback to the `drawing_id`).
    pub label: String,
}

// ---- Cross-reference graph ---------------------------------------------------

/// Stitches already-decoded pieces of the document into a small relational
/// view. Pure derivation — requires no extra I/O.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct CrossReferenceGraph {
    /// PSM-declared clusters vs. clusters actually present in the file.
    pub cluster_coverage: ClusterCoverage,
    /// `JSite` instances grouped by the symbol they reference.
    pub symbol_usage: Vec<SymbolUsage>,
    /// One summary per attribute class found in Unclustered Dynamic Attributes.
    pub attribute_classes: Vec<AttributeClassSummary>,
    /// Each `PSMroots` entry correlated with its existence in the CFB tree.
    pub root_presence: Vec<RootPresence>,
    /// Phase 3 Step 2: per-relationship provenance link between
    /// `ObjectGraph.relationships` and their matching `SheetEndpointRecord`.
    /// Empty when `object_graph` is absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship_endpoint_links: Vec<RelationshipEndpointLink>,
    /// Aggregate health of [`Self::relationship_endpoint_links`]: how many
    /// relationships linked to a sheet endpoint record, how many had no
    /// `field_x` to query with, and how many queried but missed.
    #[serde(default)]
    pub relationship_endpoint_coverage: EndpointLinkCoverage,
    /// Phase 3 Step 2: per-object provenance link between `ObjectGraph.objects`
    /// and their backing `AttributeRecord` inside `DynamicAttributesBlob`.
    /// Empty when either side is absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_sources: Vec<ObjectSourceRef>,
    /// Aggregate health of [`Self::object_sources`]: how many `PidObject`s
    /// resolved back to a DA record by `DrawingID`, and how many did not.
    #[serde(default)]
    pub object_source_coverage: ObjectSourceCoverage,
    /// Phase 3 Step 3: per-relationship provenance chain diagnostic —
    /// counts how many relationships clear each hop of the chain
    /// (`cluster → sheet → endpoint_record → DA record → object`).
    #[serde(default)]
    pub provenance_chain_coverage: ProvenanceChainCoverage,
    /// Sample list of the first broken chains (capped at 10) to aid
    /// debugging without dumping the full relationship vector.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_chain_breaks: Vec<ProvenanceChainBreak>,
    /// Phase 3 Step 4: per-`SheetStream` aggregation of the Step 1–3
    /// provenance signals (1:1 with `doc.sheet_streams`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheet_provenance: Vec<SheetProvenanceRef>,
    /// Aggregate health of [`Self::sheet_provenance`].
    #[serde(default)]
    pub sheet_provenance_coverage: SheetProvenanceCoverage,
}

/// Comparison between `PSMclustertable` (declared) and the cluster / sheet
/// streams actually parsed from the file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ClusterCoverage {
    /// Names declared by `PSMclustertable`.
    pub declared: Vec<String>,
    /// Provenance-preserving declared entries in original table order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_entries: Vec<DeclaredClusterRef>,
    /// Names found on-disk (cluster streams + sheet streams).
    pub found: Vec<String>,
    /// Provenance-preserving found entries, including source kind and path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub found_entries: Vec<FoundClusterRef>,
    /// Names present in both sets.
    pub matched: Vec<String>,
    /// Entry-level mapping between a declared item and its resolved found entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matches_detailed: Vec<ClusterCoverageMatch>,
    /// Declared but not found on-disk (data-integrity warning).
    pub declared_missing: Vec<String>,
    /// Found on-disk but not declared (typically only when PSM is absent).
    pub found_extra: Vec<String>,
}

/// Which on-disk stream family produced a [`FoundClusterRef`] — tells
/// the crossref pass whether it came from the cluster side or the
/// sheet side of the CFB tree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ClusterCoverageSourceKind {
    /// Entry was lifted from a `PSMcluster*` stream.
    PsmCluster,
    /// Entry was lifted from a `/Sheet*` stream.
    SheetStream,
}

/// Provenance-preserving view of a single `PSMclustertable` entry —
/// keeps the declared name alongside its byte offsets so crossref
/// diagnostics can re-locate the record in the raw stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DeclaredClusterRef {
    /// Declared cluster name (matches [`FoundClusterRef::name`] on a
    /// resolved match).
    pub name: String,
    /// Byte offset of the full record inside the declaring
    /// `PSMclustertable` stream.
    pub record_offset: usize,
    /// Byte offset of the UTF-16LE name run inside the same stream.
    pub name_offset: usize,
    /// Total byte length of the record (prefix + name).
    pub record_len: usize,
}

/// Provenance-preserving view of a cluster actually observed on disk —
/// mirrors [`DeclaredClusterRef`] for the "found" side of the coverage
/// comparison.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FoundClusterRef {
    /// Local stream / storage name (e.g. `"PSMcluster0"`,
    /// `"Sheet1"`).
    pub name: String,
    /// Which stream family the entry was observed under.
    pub source_kind: ClusterCoverageSourceKind,
    /// Full `/`-joined CFB path of the source stream.
    pub path: String,
}

/// One resolved pairing between a [`DeclaredClusterRef`] and the
/// [`FoundClusterRef`] that satisfies it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ClusterCoverageMatch {
    /// Shared cluster name.
    pub name: String,
    /// Index into `ClusterCoverage.declared_entries` of the
    /// declaration side.
    pub declared_index: usize,
    /// Index into `ClusterCoverage.found_entries` of the on-disk side.
    pub found_index: usize,
}

/// Phase 3 Step 2 — provenance link between a [`PidRelationship`] and its
/// backing [`SheetEndpointRecord`]. Generated by the crossref pass and
/// guaranteed 1:1 with `ObjectGraph.relationships` (in source order).
///
/// Each relationship is in one of three states:
/// - `rel_field_x = None` → parser never attached a DA trailer `field_x` to
///   this relationship. `sheet_*` all `None`, `missing_sheet_record = false`.
/// - `rel_field_x = Some(x)` but no sheet endpoint record matches → link
///   carries `missing_sheet_record = true` so callers can surface a
///   fixture-drift warning.
/// - `rel_field_x = Some(x)` and a sheet endpoint record matches → sheet
///   provenance (path + offset) and per-endpoint `field_x` values are
///   populated.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RelationshipEndpointLink {
    /// 32-character hex GUID of the relationship (`PidRelationship.guid`).
    pub relationship_guid: String,
    /// DA record id if the trailer was decoded; else `None`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relationship_record_id: Option<u32>,
    /// `field_x` attached to the relationship's DA trailer.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rel_field_x: Option<u32>,
    /// Source endpoint's `field_x` from the Sheet endpoint record (when linked).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_field_x: Option<u32>,
    /// Target endpoint's `field_x` from the Sheet endpoint record (when linked).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_field_x: Option<u32>,
    /// `drawing_id` of the source endpoint if already resolved on the
    /// `PidRelationship` itself.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_drawing_id: Option<String>,
    /// `drawing_id` of the target endpoint if already resolved.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_drawing_id: Option<String>,
    /// Sheet stream that holds the matching endpoint record (`/Sheet6`, …).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sheet_path: Option<String>,
    /// Byte offset inside that sheet stream where the record starts.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sheet_offset: Option<usize>,
    /// `true` when the relationship carried a `rel_field_x` but no sheet
    /// endpoint record matched it; useful as a fixture-drift guard.
    #[serde(default)]
    pub missing_sheet_record: bool,
}

/// Aggregate health view for [`RelationshipEndpointLink`] — mirrors the
/// `summary + detail` rhythm used for cluster/symbol/attribute provenance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct EndpointLinkCoverage {
    /// Total relationships inspected (= `object_graph.relationships.len()`).
    pub total: usize,
    /// Relationships whose `rel_field_x` resolved to a sheet endpoint record.
    pub linked: usize,
    /// Relationships with no `rel_field_x`; sheet lookup was skipped.
    pub missing_field_x: usize,
    /// Relationships with a `rel_field_x` that did not match any sheet
    /// endpoint record (fixture-drift warning).
    pub missing_sheet_record: usize,
    /// Relationships whose `PidRelationship` already resolved both
    /// source/target `drawing_id`.
    pub fully_resolved: usize,
    /// Relationships where exactly one of source/target `drawing_id` was
    /// already resolved.
    pub partially_resolved: usize,
}

/// Phase 3 Step 2 — provenance link between a [`PidObject`] and the DA
/// `attribute_records` entry (plus trailer, when available) that produced
/// it. Generated by the crossref pass and guaranteed to preserve the
/// `ObjectGraph.objects` source order for 1:1 alignment: callers can
/// index into `object_sources` by the same position as `objects`.
///
/// When the parser already cross-referenced a DA `attribute_records`
/// entry to the object's `drawing_id`, `attribute_record_index` points at
/// that entry and `class_name` / `confidence` mirror its raw values. If
/// no matching attribute record can be found, the link carries
/// `attribute_record_index = None` and `missing_da_record = true` so
/// callers can flag it as fixture drift without losing the object.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ObjectSourceRef {
    /// 32-character hex `drawing_id` of the parent `PidObject`.
    pub drawing_id: String,
    /// DA attribute-record `class_name` when linked, else `None`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub class_name: Option<String>,
    /// Index into `doc.dynamic_attributes.attribute_records` when linked.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attribute_record_index: Option<usize>,
    /// Confidence string reported by the record parser (e.g. `"heuristic"`,
    /// `"decoded"`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub confidence: Option<String>,
    /// `true` when the linked `PidObject` carries a numeric DA `record_id`
    /// (i.e. a trailer was successfully matched). A stronger signal than
    /// an attribute-only match; required before attempting to cross-ref
    /// this record to sheet endpoints.
    #[serde(default)]
    pub has_trailer_record_id: bool,
    /// `true` when no DA `attribute_records` entry matched the object's
    /// `drawing_id`. Mutually exclusive with a populated
    /// `attribute_record_index`.
    #[serde(default)]
    pub missing_da_record: bool,
}

/// Aggregate health view for [`ObjectSourceRef`] — mirrors the
/// `summary + detail` rhythm used for other provenance records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct ObjectSourceCoverage {
    /// Total objects inspected (= `object_graph.objects.len()`).
    pub total_objects: usize,
    /// Objects whose `drawing_id` resolved to a DA attribute record.
    pub linked: usize,
    /// Objects with no matching DA attribute record (fixture-drift warning).
    pub missing_da_record: usize,
    /// Linked objects whose matching `PidObject` also surfaced a DA
    /// `record_id` (trailer was successfully aligned to the record).
    pub with_trailer_record_id: usize,
}

/// Phase 3 Step 3 — end-to-end "cluster → sheet → endpoint → DA record →
/// `PidObject`" provenance chain diagnostic. Each field counts how many
/// relationships passed a given hop; `fully_traced` is the subset that
/// passed all 4 hops. Computed strictly from the other `CrossReferenceGraph`
/// sections, so no extra parser state is required.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct ProvenanceChainCoverage {
    /// Total relationships inspected (= `object_graph.relationships.len()`).
    pub total_relationships: usize,
    /// Relationships that exposed a `rel_field_x` (trailer was decoded).
    pub has_field_x: usize,
    /// Relationships linked to a [`SheetEndpointRecord`] by `rel_field_x`.
    pub sheet_linked: usize,
    /// Relationships whose `source_drawing_id` resolved to a linked
    /// [`ObjectSourceRef`] (i.e. a DA record was matched).
    pub source_object_linked: usize,
    /// Relationships whose `target_drawing_id` resolved to a linked
    /// [`ObjectSourceRef`].
    pub target_object_linked: usize,
    /// Relationships that passed every hop: `has_field_x && sheet_linked &&
    /// source_object_linked && target_object_linked`.
    pub fully_traced: usize,
}

/// Stage at which a given relationship's provenance chain first broke.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ProvenanceChainStage {
    /// Relationship has no `rel_field_x` — the trailer was never decoded,
    /// so the sheet lookup cannot even start.
    MissingFieldX,
    /// `rel_field_x` present but no matching [`SheetEndpointRecord`].
    MissingSheetRecord,
    /// `source_drawing_id` absent or not in [`ObjectSourceRef`] index.
    SourceObjectUnlinked,
    /// `target_drawing_id` absent or not in [`ObjectSourceRef`] index.
    TargetObjectUnlinked,
}

/// A single broken-chain sample — used to surface debug material in reports
/// without dumping the entire relationship list. `reason` is a short
/// human-readable hint; do not parse it programmatically.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProvenanceChainBreak {
    /// 32-character hex GUID of the relationship whose chain broke.
    pub relationship_guid: String,
    /// Hop at which the chain first failed.
    pub stage: ProvenanceChainStage,
    /// Short human-readable explanation (diagnostic only — don't
    /// parse).
    pub reason: String,
}

/// Phase 3 Step 4 — per-`SheetStream` aggregation of the provenance signals
/// collected by Step 1–3. One entry per `doc.sheet_streams[i]`, in source
/// order. Flags:
/// - `declared_in_psm = true` iff the sheet's storage path matched a
///   `PSMclustertable` declared entry (see `ClusterCoverage.matches_detailed`).
/// - `matched_declared_index` is the `declared_entries` index of the hit,
///   or `None` if `declared_in_psm == false`.
/// - `linked_relationship_count` counts how many
///   [`RelationshipEndpointLink`]s point at this sheet path.
/// - `fully_traced_relationship_count` counts the subset whose source and
///   target drawing ids both resolved to a linked [`ObjectSourceRef`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetProvenanceRef {
    /// CFB path of the sheet stream (e.g. `"/Sheet6"`).
    pub sheet_path: String,
    /// Number of [`SheetEndpointRecord`]s decoded from this sheet.
    pub endpoint_record_count: usize,
    /// `true` when the sheet was declared in `PSMclustertable` (and
    /// therefore matched in [`ClusterCoverage::matches_detailed`]).
    pub declared_in_psm: bool,
    /// Index into `ClusterCoverage.declared_entries` when
    /// [`Self::declared_in_psm`] is `true`; `None` otherwise.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub matched_declared_index: Option<usize>,
    /// Relationships whose endpoint record points at this sheet.
    pub linked_relationship_count: usize,
    /// Subset of [`Self::linked_relationship_count`] whose source and
    /// target both cleared the full provenance chain.
    pub fully_traced_relationship_count: usize,
}

/// Aggregate health view for [`SheetProvenanceRef`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct SheetProvenanceCoverage {
    /// Total sheets inspected (= `doc.sheet_streams.len()`).
    pub total_sheets: usize,
    /// Sheets declared in `PSMclustertable`.
    pub declared_sheets: usize,
    /// Sheets present on disk but not declared in `PSMclustertable`.
    pub orphan_sheets: usize,
    /// Sheets that carry at least one `SheetEndpointRecord`.
    pub sheets_with_endpoint_records: usize,
    /// Sheets declared in `PSMclustertable` but carrying no endpoint
    /// records (empty-shell warning).
    pub empty_declared_sheets: usize,
}

/// Symbol → `JSite` reverse index. One entry per unique `symbol_path`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SymbolUsage {
    /// Absolute symbol path (e.g. `\\server\share\symbols\Valve.sym`).
    pub symbol_path: String,
    /// Basename of the symbol (e.g. `Valve`). `None` when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    /// `JSite` storage names that reference this symbol (sorted, unique).
    pub jsite_names: Vec<String>,
    /// Number of references. Always equal to `jsite_names.len()`.
    pub usage_count: usize,
    /// Provenance-preserving per-JSite references that contributed to this usage entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<SymbolReference>,
}

/// One per-[`JSite`] reference contributing to a [`SymbolUsage`] —
/// preserves the `JSite` name/path and a couple of flags so the
/// crossref pass stays provenance-complete.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SymbolReference {
    /// Local `JSite` storage name (matches
    /// [`JSite::name`]).
    pub jsite_name: String,
    /// Full CFB path of the `JSite` storage.
    pub jsite_path: String,
    /// Workstation-relative symbol path mirrored from
    /// [`JSite::local_symbol_path`], kept for drift detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_symbol_path: Option<String>,
    /// `true` iff the referenced `JSite` embeds an `\x01Ole` stream.
    pub has_ole_stream: bool,
}

/// Per-class aggregation of Dynamic Attributes records.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributeClassSummary {
    /// Attribute class this entry summarises
    /// (e.g. `"P&IDAttributes"`).
    pub class_name: String,
    /// Number of records observed for the class.
    pub record_count: usize,
    /// Distinct `DrawingID` / `DrawingNo` values encountered (sorted, unique).
    pub drawing_ids: Vec<String>,
    /// Distinct `ModelID` values encountered (sorted, unique, capped at 32).
    pub model_ids: Vec<String>,
    /// Distinct attribute names encountered under this class (sorted, unique).
    pub unique_attribute_names: Vec<String>,
    /// Provenance-preserving references to the contributing attribute records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<AttributeClassRecordRef>,
}

/// Provenance-preserving reference to a single [`AttributeRecord`]
/// contributing to an [`AttributeClassSummary`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AttributeClassRecordRef {
    /// Class name mirrored from the source record.
    pub class_name: String,
    /// Number of fields in the source record.
    pub attribute_count: usize,
    /// Confidence string from the source record
    /// (`"heuristic"` / `"decoded"`).
    pub confidence: String,
    /// `DrawingID` / `DrawingNo` values surfaced from the source
    /// record's attributes (sorted, unique).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawing_ids: Vec<String>,
    /// `ModelID` values surfaced from the source record's attributes
    /// (sorted, unique).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<String>,
}

/// Describes whether a name published in `PSMroots` actually maps to a
/// storage or stream in the CFB tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RootPresence {
    /// Name as it appears in `PSMroots`.
    pub name: String,
    /// 32-bit identifier mirrored from the [`PsmRootEntry::id`].
    pub id: u32,
    /// `true` when the name resolves to a CFB storage (directory).
    pub found_as_storage: bool,
    /// `true` when the name resolves to a CFB stream (leaf).
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

#[cfg(test)]
mod summary_property_value_serde_tests {
    use super::SummaryPropertyValue;

    // Regression: `#[serde(tag = "kind")]` without a `content` key was an
    // internally-tagged enum, which serde refuses to apply to newtype
    // variants over primitives. Every round-trip below used to fail with
    // "cannot serialize tagged newtype variant" before the adjacently-
    // tagged fix landed. The string literals here pin the resulting wire
    // shape so downstream consumers can rely on it.

    fn roundtrip(value: &SummaryPropertyValue, expected_json: &str) {
        let encoded = serde_json::to_string(value).expect("serialize");
        assert_eq!(encoded, expected_json, "wire shape");
        let decoded: SummaryPropertyValue = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(&decoded, value, "round-trip identity");
    }

    #[test]
    fn lpstr_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::Lpstr("hello".into()),
            r#"{"kind":"lpstr","value":"hello"}"#,
        );
    }

    #[test]
    fn lpwstr_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::Lpwstr("PROJ-001".into()),
            r#"{"kind":"lpwstr","value":"PROJ-001"}"#,
        );
    }

    #[test]
    fn i4_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::I4(-42),
            r#"{"kind":"i4","value":-42}"#,
        );
    }

    #[test]
    fn bool_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::Bool(true),
            r#"{"kind":"bool","value":true}"#,
        );
    }

    #[test]
    fn filetime_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::Filetime(132_000_000_000_000_000),
            r#"{"kind":"filetime","value":132000000000000000}"#,
        );
    }

    #[test]
    fn raw_roundtrips() {
        roundtrip(
            &SummaryPropertyValue::Raw {
                vt: 0x0015,
                bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
            },
            r#"{"kind":"raw","value":{"vt":21,"bytes":[222,173,190,239]}}"#,
        );
    }
}
