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

    pub unknown_streams: Vec<UnknownStream>,
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
            unknown_streams: vec![],
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetStream {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub extracted_texts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnknownStream {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
}
