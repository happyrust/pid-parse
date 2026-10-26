//! `CatalogIndex.xml` — the plant's own `(ItemType, Class, Subclass,
//! Type) → .sym` index shipped inside each `RefData~*.zip` archive of a
//! `SmartPlant` backup.
//!
//! The file is an `XOMContainer` dump: one `CICatalogItems` block header
//! per `ItemType`, followed by one `CICatalogItem` row per catalog entry.
//! Each row repeats its `ItemType`, so blocks and items associate through
//! the attribute rather than through document position. The discriminant
//! tuple `(ItemType, Class, Subclass, Type)` is **not** a unique key — one
//! tuple can name several `.sym` files — so [`CatalogIndexDocument`]
//! exposes a multi-valued map rather than a last-wins table.
//!
//! Both corpus copies observed so far are `UTF-16LE` (`FF FE` BOM). The
//! decoder is agnostic about that: `FF FE` / `FE FF` / `EF BB BF` BOMs are
//! honored, and a `NUL`-byte probe separates a BOM-less `UTF-16LE` payload
//! from `UTF-8` (a pure-ASCII `UTF-16LE` document is also legal `UTF-8`
//! byte-wise — the interleaved `0x00` bytes are the only tell).

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::Path;

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// The encoding [`decode_catalog_index_bytes`] resolved the payload with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogIndexEncoding {
    /// `UTF-16` little-endian — `FF FE` BOM, or a BOM-less payload whose
    /// `NUL` bytes rule out `UTF-8`.
    Utf16Le,
    /// `UTF-16` big-endian — `FE FF` BOM.
    Utf16Be,
    /// `UTF-8` — `EF BB BF` BOM, or a BOM-less payload with no `NUL`
    /// byte.
    Utf8,
}

impl CatalogIndexEncoding {
    /// Stable lowercase tag for reports: `"utf16le"` / `"utf16be"` /
    /// `"utf8"`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf16Le => "utf16le",
            Self::Utf16Be => "utf16be",
            Self::Utf8 => "utf8",
        }
    }
}

/// One `CICatalogItems` block header — just its `ItemType` today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIndexBlock {
    /// The `ItemType` attribute the block carries.
    pub item_type: String,
}

/// One `CICatalogItem` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIndexItem {
    /// The `ItemType` the row belongs to (repeated from its block).
    pub item_type: String,
    /// `Class` discriminant.
    pub class: u32,
    /// `Subclass` discriminant.
    pub subclass: u32,
    /// `Type` discriminant.
    pub type_code: u32,
    /// `AuxString`, or empty when the attribute is absent.
    pub aux_string: String,
    /// The `CatalogItem` path — the `.sym` this row resolves to.
    pub catalog_item: String,
}

/// A parsed `CatalogIndex.xml`: every block header and every item row,
/// plus the encoding the payload was decoded with.
#[derive(Debug)]
pub struct CatalogIndexDocument {
    /// How the payload decoded.
    encoding: CatalogIndexEncoding,
    /// `CICatalogItems` block headers in document order.
    blocks: Vec<CatalogIndexBlock>,
    /// `CICatalogItem` rows in document order.
    items: Vec<CatalogIndexItem>,
}

impl CatalogIndexDocument {
    /// The encoding the payload decoded with.
    #[must_use]
    pub fn encoding(&self) -> CatalogIndexEncoding {
        self.encoding
    }

    /// Every `CICatalogItems` block header, in document order.
    #[must_use]
    pub fn blocks(&self) -> &[CatalogIndexBlock] {
        &self.blocks
    }

    /// Every `CICatalogItem` row, in document order.
    #[must_use]
    pub fn items(&self) -> &[CatalogIndexItem] {
        &self.items
    }

    /// Block-level `ItemType`s, deduplicated and sorted.
    #[must_use]
    pub fn item_types(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .blocks
            .iter()
            .map(|block| block.item_type.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// `ItemType`s that actually carry items, deduplicated and sorted.
    #[must_use]
    pub fn item_bearing_item_types(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .items
            .iter()
            .map(|item| item.item_type.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Block-level `ItemType`s that carry no item, sorted.
    #[must_use]
    pub fn empty_blocks(&self) -> Vec<&str> {
        let bearing: std::collections::BTreeSet<&str> = self
            .items
            .iter()
            .map(|item| item.item_type.as_str())
            .collect();
        let mut names: Vec<&str> = self
            .blocks
            .iter()
            .map(|block| block.item_type.as_str())
            .filter(|name| !bearing.contains(name))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Item count per `ItemType`.
    #[must_use]
    pub fn items_by_item_type(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for item in &self.items {
            *counts.entry(item.item_type.as_str()).or_insert(0) += 1;
        }
        counts
    }

    /// Items whose `CatalogItem` attribute mentions `.sym`.
    #[must_use]
    pub fn sym_reference_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.catalog_item.contains(".sym"))
            .count()
    }

    /// The multi-valued discriminant map: every
    /// `(ItemType, Class, Subclass, Type)` tuple mapped to every
    /// `CatalogItem` string the index gives it, in document order.
    #[must_use]
    pub fn discriminant_map(&self) -> BTreeMap<(String, u32, u32, u32), Vec<&str>> {
        let mut map: BTreeMap<(String, u32, u32, u32), Vec<&str>> = BTreeMap::new();
        for item in &self.items {
            map.entry((
                item.item_type.clone(),
                item.class,
                item.subclass,
                item.type_code,
            ))
            .or_default()
            .push(item.catalog_item.as_str());
        }
        map
    }

    /// Distinct `(ItemType, Class, Subclass, Type)` tuples.
    #[must_use]
    pub fn discriminant_key_count(&self) -> usize {
        self.discriminant_map().len()
    }

    /// Discriminant tuples naming more than one `CatalogItem`.
    #[must_use]
    pub fn discriminant_keys_with_multiple_items(&self) -> usize {
        self.discriminant_map()
            .values()
            .filter(|items| items.len() > 1)
            .count()
    }

    /// The first `CICatalogItem` row, if any.
    #[must_use]
    pub fn first_item(&self) -> Option<&CatalogIndexItem> {
        self.items.first()
    }
}

/// One archive's `CatalogIndex.xml` read end to end.
#[derive(Debug)]
pub struct CatalogIndexReading {
    /// Matched entry name inside the archive (`"CatalogIndex.xml"`, or a
    /// nested path ending in it).
    pub entry_name: String,
    /// Uncompressed size the central directory records for the entry.
    pub uncompressed_bytes: usize,
    /// The decoded, parsed document.
    pub document: CatalogIndexDocument,
}

/// Why a `CatalogIndex.xml` payload or archive was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogIndexError {
    /// The payload is zero bytes.
    Empty,
    /// A `UTF-16` payload ended on an odd byte boundary.
    OddUtf16Length {
        /// Payload byte count (after the BOM, when one was present).
        bytes: usize,
    },
    /// A `UTF-16` payload contains an unpaired surrogate.
    UnpairedSurrogate,
    /// A `UTF-8` payload failed to decode.
    InvalidUtf8 {
        /// Byte offset where decoding stopped being valid.
        position: usize,
    },
    /// `quick-xml` refused the document, or it ended with open tags.
    Xml {
        /// `Reader::buffer_position` at the failure.
        position: u64,
        /// What `quick-xml` reported.
        message: String,
    },
    /// A required attribute is missing, empty, or not a `u32`.
    MalformedAttribute {
        /// Element the attribute belongs to (`CICatalogItems` /
        /// `CICatalogItem`).
        element: &'static str,
        /// Attribute name.
        attribute: &'static str,
        /// Raw value that was rejected (empty when absent).
        value: String,
    },
    /// The archive itself failed to open or read.
    Zip {
        /// Archive path as handed to [`read_catalog_index_from_zip`].
        path: String,
        /// What `std::fs` / the `zip` crate reported.
        message: String,
    },
    /// The archive carries no `CatalogIndex.xml` entry.
    MissingEntry {
        /// Archive path as handed to [`read_catalog_index_from_zip`].
        path: String,
    },
}

impl fmt::Display for CatalogIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty payload"),
            Self::OddUtf16Length { bytes } => {
                write!(f, "UTF-16 payload has odd byte count {bytes}")
            }
            Self::UnpairedSurrogate => {
                write!(f, "UTF-16 payload contains an unpaired surrogate")
            }
            Self::InvalidUtf8 { position } => {
                write!(f, "invalid UTF-8 at byte {position}")
            }
            Self::Xml { position, message } => {
                write!(f, "XML error at byte {position}: {message}")
            }
            Self::MalformedAttribute {
                element,
                attribute,
                value,
            } => write!(
                f,
                "<{element}> attribute `{attribute}` is missing, empty, or not an unsigned integer: `{value}`"
            ),
            Self::Zip { path, message } => write!(f, "{path}: {message}"),
            Self::MissingEntry { path } => {
                write!(f, "{path}: no `CatalogIndex.xml` entry")
            }
        }
    }
}

impl std::error::Error for CatalogIndexError {}

/// Decode a `CatalogIndex.xml` payload into text plus the encoding it
/// used.
///
/// BOM-driven: `FF FE` → `UTF-16LE`, `FE FF` → `UTF-16BE`,
/// `EF BB BF` → `UTF-8`. With no BOM the payload is `UTF-8` only when it
/// decodes as such **and** carries no `0x00` byte — a pure-ASCII
/// `UTF-16LE` document is byte-wise also valid `UTF-8` (the interleaved
/// `NUL`s are legal code points), so the `NUL` probe is the only
/// disambiguator between the two readings.
pub fn decode_catalog_index_bytes(
    raw: &[u8],
) -> Result<(String, CatalogIndexEncoding), CatalogIndexError> {
    if raw.is_empty() {
        return Err(CatalogIndexError::Empty);
    }
    if let Some(rest) = raw.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, false).map(|text| (text, CatalogIndexEncoding::Utf16Le));
    }
    if let Some(rest) = raw.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, true).map(|text| (text, CatalogIndexEncoding::Utf16Be));
    }
    if let Some(rest) = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let text = std::str::from_utf8(rest).map_err(|error| CatalogIndexError::InvalidUtf8 {
            position: error.valid_up_to(),
        })?;
        return Ok((text.to_owned(), CatalogIndexEncoding::Utf8));
    }
    match std::str::from_utf8(raw) {
        Ok(text) if !raw.contains(&0) => Ok((text.to_owned(), CatalogIndexEncoding::Utf8)),
        _ => decode_utf16(raw, false).map(|text| (text, CatalogIndexEncoding::Utf16Le)),
    }
}

/// Decode `UTF-16` code units from `raw` (no BOM). Odd byte counts and
/// unpaired surrogates are refusals, never panics.
fn decode_utf16(raw: &[u8], big_endian: bool) -> Result<String, CatalogIndexError> {
    if !raw.len().is_multiple_of(2) {
        return Err(CatalogIndexError::OddUtf16Length { bytes: raw.len() });
    }
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if big_endian {
                u16::from_be_bytes([pair[0], pair[1]])
            } else {
                u16::from_le_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|_| CatalogIndexError::UnpairedSurrogate)
}

/// Parse `CatalogIndex.xml` bytes (any supported encoding) into a
/// [`CatalogIndexDocument`].
///
/// `CICatalogItems` elements become [`CatalogIndexBlock`]s and
/// `CICatalogItem` elements become [`CatalogIndexItem`]s; every other
/// element is ignored. Required attributes that are missing, empty, or
/// unparsable surface as [`CatalogIndexError::MalformedAttribute`]; a
/// document `quick-xml` cannot lex, or one that ends with unclosed
/// elements, surfaces as [`CatalogIndexError::Xml`].
pub fn parse_catalog_index_document(raw: &[u8]) -> Result<CatalogIndexDocument, CatalogIndexError> {
    let (text, encoding) = decode_catalog_index_bytes(raw)?;
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut blocks = Vec::new();
    let mut items = Vec::new();
    let mut open_tags: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                read_element(&start, reader.buffer_position(), &mut blocks, &mut items)?;
                open_tags.push(start.name().as_ref().to_string());
            }
            Ok(Event::Empty(start)) => {
                read_element(&start, reader.buffer_position(), &mut blocks, &mut items)?;
            }
            Ok(Event::End(_)) => {
                open_tags.pop();
            }
            Ok(Event::Eof) => {
                // quick-xml reaches the end of input without checking
                // that every start tag was closed — a document cut
                // between tags lands here.
                if let Some(tag) = open_tags.pop() {
                    let error = quick_xml::Error::IllFormed(IllFormedError::MissingEndTag(tag));
                    return Err(CatalogIndexError::Xml {
                        position: reader.buffer_position(),
                        message: error.to_string(),
                    });
                }
                break;
            }
            Err(error) => {
                return Err(CatalogIndexError::Xml {
                    position: reader.buffer_position(),
                    message: error.to_string(),
                });
            }
            _ => {}
        }
    }
    Ok(CatalogIndexDocument {
        encoding,
        blocks,
        items,
    })
}

/// Pull `CatalogIndex.xml` out of a `RefData~*.zip` archive and parse it.
///
/// The entry matches when its name is exactly `CatalogIndex.xml` or ends
/// with `/CatalogIndex.xml`. No entry ⇒ [`CatalogIndexError::MissingEntry`];
/// open / decompress failures ⇒ [`CatalogIndexError::Zip`].
pub fn read_catalog_index_from_zip(
    zip_path: &Path,
) -> Result<CatalogIndexReading, CatalogIndexError> {
    let file = std::fs::File::open(zip_path).map_err(|error| zip_fail(zip_path, error))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| zip_fail(zip_path, error))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| zip_fail(zip_path, error))?;
        if entry.is_dir() {
            continue;
        }
        let name = crate::backup::datamap::decode_zip_name(entry.name_raw());
        if name != "CatalogIndex.xml" && !name.ends_with("/CatalogIndex.xml") {
            continue;
        }
        let uncompressed_bytes = entry.size() as usize;
        let mut raw = Vec::new();
        entry
            .read_to_end(&mut raw)
            .map_err(|error| zip_fail(zip_path, error))?;
        let document = parse_catalog_index_document(&raw)?;
        return Ok(CatalogIndexReading {
            entry_name: name,
            uncompressed_bytes,
            document,
        });
    }
    Err(CatalogIndexError::MissingEntry {
        path: zip_path.display().to_string(),
    })
}

fn zip_fail(zip_path: &Path, error: impl fmt::Display) -> CatalogIndexError {
    CatalogIndexError::Zip {
        path: zip_path.display().to_string(),
        message: error.to_string(),
    }
}

/// Dispatch one `Start` / `Empty` element into the block / item vectors.
/// Elements other than `CICatalogItems` / `CICatalogItem` carry no
/// catalog payload and are ignored.
fn read_element(
    start: &BytesStart<'_>,
    position: u64,
    blocks: &mut Vec<CatalogIndexBlock>,
    items: &mut Vec<CatalogIndexItem>,
) -> Result<(), CatalogIndexError> {
    match start.local_name().as_ref() {
        "CICatalogItems" => {
            let item_type = required_attribute(start, "CICatalogItems", "ItemType", position)?;
            blocks.push(CatalogIndexBlock { item_type });
        }
        "CICatalogItem" => {
            let item_type = required_attribute(start, "CICatalogItem", "ItemType", position)?;
            let class = required_u32_attribute(start, "Class", position)?;
            let subclass = required_u32_attribute(start, "Subclass", position)?;
            let type_code = required_u32_attribute(start, "Type", position)?;
            let aux_string = attribute_value(start, "AuxString", position)?.unwrap_or_default();
            let catalog_item = required_attribute(start, "CICatalogItem", "CatalogItem", position)?;
            items.push(CatalogIndexItem {
                item_type,
                class,
                subclass,
                type_code,
                aux_string,
                catalog_item,
            });
        }
        _ => {}
    }
    Ok(())
}

/// The unescaped value of `attribute` on `start`, or `None` when the
/// attribute is absent. Malformed attribute markup and bad escapes are
/// document-level failures and surface as [`CatalogIndexError::Xml`].
fn attribute_value(
    start: &BytesStart<'_>,
    attribute: &'static str,
    position: u64,
) -> Result<Option<String>, CatalogIndexError> {
    match start.try_get_attribute(attribute) {
        Ok(Some(attr)) => attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|value| Some(value.into_owned()))
            .map_err(|error| CatalogIndexError::Xml {
                position,
                message: error.to_string(),
            }),
        Ok(None) => Ok(None),
        Err(error) => Err(CatalogIndexError::Xml {
            position,
            message: error.to_string(),
        }),
    }
}

/// `attribute` must be present and non-empty.
fn required_attribute(
    start: &BytesStart<'_>,
    element: &'static str,
    attribute: &'static str,
    position: u64,
) -> Result<String, CatalogIndexError> {
    match attribute_value(start, attribute, position)? {
        Some(value) if !value.is_empty() => Ok(value),
        other => Err(CatalogIndexError::MalformedAttribute {
            element,
            attribute,
            value: other.unwrap_or_default(),
        }),
    }
}

/// `attribute` must be present and parse as `u32` — the `Class` /
/// `Subclass` / `Type` discriminants of a `CICatalogItem`.
fn required_u32_attribute(
    start: &BytesStart<'_>,
    attribute: &'static str,
    position: u64,
) -> Result<u32, CatalogIndexError> {
    let value = required_attribute(start, "CICatalogItem", attribute, position)?;
    value
        .parse::<u32>()
        .map_err(|_| CatalogIndexError::MalformedAttribute {
            element: "CICatalogItem",
            attribute,
            value,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A small but representative document: two block headers, one item
    /// for `AreaBreak`, and two items sharing `_Gap`'s discriminant tuple
    /// (the multi-valued key the map must keep).
    const SAMPLE_XML: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>",
        "<XOMContainer Domain=\"CatalogIndex\" Version=\"1\">",
        "<XOMObject ClassName=\"CICatalogItems\">",
        "<XOMCore ID=\"ID1\"/>",
        "<CICatalogItems ItemType=\"AreaBreak\"/>",
        "</XOMObject>",
        "<XOMObject ClassName=\"CICatalogItems\">",
        "<XOMCore ID=\"ID2\"/>",
        "<CICatalogItems ItemType=\"_Gap\"/>",
        "</XOMObject>",
        "<XOMObject ClassName=\"CICatalogItem\">",
        "<XOMCore ID=\"ID3\"/>",
        "<CICatalogItem ItemType=\"AreaBreak\" Class=\"0\" Subclass=\"0\" Type=\"64\" ",
        "AuxString=\"\" CatalogItem=\"\\Design\\Area Break.sym\"/>",
        "</XOMObject>",
        "<XOMObject ClassName=\"CICatalogItem\">",
        "<XOMCore ID=\"ID4\"/>",
        "<CICatalogItem ItemType=\"_Gap\" Class=\"0\" Subclass=\"0\" Type=\"0\" ",
        "AuxString=\"\" CatalogItem=\"\\Design\\Gap A.sym\"/>",
        "</XOMObject>",
        "<XOMObject ClassName=\"CICatalogItem\">",
        "<XOMCore ID=\"ID5\"/>",
        "<CICatalogItem ItemType=\"_Gap\" Class=\"0\" Subclass=\"0\" Type=\"0\" ",
        "AuxString=\"\" CatalogItem=\"\\Design\\Gap B.sym\"/>",
        "</XOMObject>",
        "</XOMContainer>"
    );

    fn utf16le_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    fn utf16be_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        bytes
    }

    fn utf8_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    fn utf16le_no_bom(text: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn utf16le_bom_decodes_blocks_items_and_counts() {
        // A non-ASCII `AuxString` proves the UTF-16 path is real, not a
        // lucky ASCII read.
        let xml = SAMPLE_XML.replacen("AuxString=\"\"", "AuxString=\"井\"", 1);
        let document = parse_catalog_index_document(&utf16le_with_bom(&xml)).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf16Le);
        assert_eq!(document.blocks().len(), 2);
        assert_eq!(document.items().len(), 3);
        assert_eq!(document.item_types(), ["AreaBreak", "_Gap"]);
        assert_eq!(document.item_bearing_item_types(), ["AreaBreak", "_Gap"]);
        assert!(document.empty_blocks().is_empty());
        assert_eq!(document.sym_reference_count(), 3);
        assert_eq!(document.items_by_item_type()["AreaBreak"], 1);
        assert_eq!(document.items_by_item_type()["_Gap"], 2);
        let first = document.first_item().unwrap();
        assert_eq!(first.item_type, "AreaBreak");
        assert_eq!((first.class, first.subclass, first.type_code), (0, 0, 64));
        assert_eq!(first.aux_string, "井");
        assert_eq!(first.catalog_item, r"\Design\Area Break.sym");
    }

    #[test]
    fn utf16be_bom_decodes() {
        let document = parse_catalog_index_document(&utf16be_with_bom(SAMPLE_XML)).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf16Be);
        assert_eq!(document.items().len(), 3);
    }

    #[test]
    fn utf8_bom_decodes() {
        let document = parse_catalog_index_document(&utf8_with_bom(SAMPLE_XML)).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf8);
        assert_eq!(document.items().len(), 3);
    }

    #[test]
    fn bomless_utf16le_detected_through_nul_bytes() {
        let document = parse_catalog_index_document(&utf16le_no_bom(SAMPLE_XML)).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf16Le);
        assert_eq!(document.items().len(), 3);
    }

    #[test]
    fn bomless_utf8_decodes() {
        let document = parse_catalog_index_document(SAMPLE_XML.as_bytes()).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf8);
        assert_eq!(document.items().len(), 3);
    }

    #[test]
    fn odd_utf16_length_is_rejected() {
        let mut raw = utf16le_with_bom("<X/>");
        raw.push(0x00);
        assert_eq!(
            decode_catalog_index_bytes(&raw),
            Err(CatalogIndexError::OddUtf16Length { bytes: 9 })
        );
    }

    #[test]
    fn unpaired_surrogate_is_rejected() {
        let raw = [0xFF, 0xFE, 0x00, 0xD8];
        assert_eq!(
            decode_catalog_index_bytes(&raw),
            Err(CatalogIndexError::UnpairedSurrogate)
        );
    }

    #[test]
    fn empty_payload_is_rejected() {
        assert_eq!(
            decode_catalog_index_bytes(&[]),
            Err(CatalogIndexError::Empty)
        );
    }

    #[test]
    fn truncated_element_is_an_xml_error() {
        let raw = SAMPLE_XML.as_bytes();
        let cut = raw[..raw.len() - 15].to_vec();
        assert!(matches!(
            parse_catalog_index_document(&cut),
            Err(CatalogIndexError::Xml { .. })
        ));
    }

    #[test]
    fn unclosed_document_is_an_xml_error() {
        let xml = "<XOMContainer><CICatalogItems ItemType=\"AreaBreak\"/>";
        assert!(matches!(
            parse_catalog_index_document(xml.as_bytes()),
            Err(CatalogIndexError::Xml { .. })
        ));
    }

    #[test]
    fn non_numeric_class_is_a_malformed_attribute() {
        let xml = "<XOMContainer><CICatalogItem ItemType=\"Vessel\" Class=\"x\" \
                   Subclass=\"0\" Type=\"1\" CatalogItem=\"a.sym\"/></XOMContainer>";
        match parse_catalog_index_document(xml.as_bytes()) {
            Err(CatalogIndexError::MalformedAttribute {
                element,
                attribute,
                value,
            }) => {
                assert_eq!(element, "CICatalogItem");
                assert_eq!(attribute, "Class");
                assert_eq!(value, "x");
            }
            other => panic!("expected MalformedAttribute, got {other:?}"),
        }
    }

    #[test]
    fn missing_catalog_item_is_a_malformed_attribute() {
        let xml = "<XOMContainer><CICatalogItem ItemType=\"Vessel\" Class=\"0\" \
                   Subclass=\"0\" Type=\"1\"/></XOMContainer>";
        match parse_catalog_index_document(xml.as_bytes()) {
            Err(CatalogIndexError::MalformedAttribute {
                element,
                attribute,
                value,
            }) => {
                assert_eq!(element, "CICatalogItem");
                assert_eq!(attribute, "CatalogItem");
                assert_eq!(value, "");
            }
            other => panic!("expected MalformedAttribute, got {other:?}"),
        }
    }

    #[test]
    fn block_without_item_type_is_a_malformed_attribute() {
        let xml = "<XOMContainer><CICatalogItems/></XOMContainer>";
        match parse_catalog_index_document(xml.as_bytes()) {
            Err(CatalogIndexError::MalformedAttribute {
                element,
                attribute,
                value,
            }) => {
                assert_eq!(element, "CICatalogItems");
                assert_eq!(attribute, "ItemType");
                assert_eq!(value, "");
            }
            other => panic!("expected MalformedAttribute, got {other:?}"),
        }
    }

    #[test]
    fn unquoted_attribute_is_an_xml_error() {
        let xml = "<XOMContainer><CICatalogItems ItemType=AreaBreak/></XOMContainer>";
        assert!(matches!(
            parse_catalog_index_document(xml.as_bytes()),
            Err(CatalogIndexError::Xml { .. })
        ));
    }

    #[test]
    fn garbage_bytes_never_panic() {
        let ones = vec![0xFF; 64];
        let cycling: Vec<u8> = (0u16..512).map(|value| value as u8).collect();
        let mut state = 0x9E37_79B9u32;
        let xorshift: Vec<u8> = (0..256)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 8) as u8
            })
            .collect();
        for raw in [&ones, &cycling, &xorshift] {
            let _ = parse_catalog_index_document(raw);
            let _ = decode_catalog_index_bytes(raw);
        }
    }

    #[test]
    fn xml_declaration_only_parses_to_an_empty_document() {
        let document = parse_catalog_index_document("<?xml version=\"1.0\"?>".as_bytes()).unwrap();
        assert!(document.blocks().is_empty());
        assert!(document.items().is_empty());
    }

    #[test]
    fn bom_with_no_payload_parses_to_an_empty_document() {
        let document = parse_catalog_index_document(&[0xFF, 0xFE]).unwrap();
        assert_eq!(document.encoding(), CatalogIndexEncoding::Utf16Le);
        assert!(document.items().is_empty());
    }

    #[test]
    fn every_prefix_of_a_valid_document_returns_a_result() {
        let raw = utf16le_with_bom(SAMPLE_XML);
        for end in 0..=raw.len() {
            // Either `Ok` or `Err` passes — panics fail the test run.
            let _ = parse_catalog_index_document(&raw[..end]);
        }
        let raw = SAMPLE_XML.as_bytes().to_vec();
        for end in 0..=raw.len() {
            let _ = parse_catalog_index_document(&raw[..end]);
        }
    }

    #[test]
    fn item_outside_any_block_is_still_recorded() {
        let xml = "<XOMContainer><CICatalogItem ItemType=\"Vessel\" Class=\"1\" \
                   Subclass=\"2\" Type=\"3\" CatalogItem=\"a.sym\"/></XOMContainer>";
        let document = parse_catalog_index_document(xml.as_bytes()).unwrap();
        assert_eq!(document.items().len(), 1);
        assert_eq!(document.item_bearing_item_types(), ["Vessel"]);
        assert!(document.blocks().is_empty());
    }

    #[test]
    fn multi_item_discriminant_keys_are_kept() {
        let document = parse_catalog_index_document(&utf16le_with_bom(SAMPLE_XML)).unwrap();
        let map = document.discriminant_map();
        assert_eq!(map.len(), 2);
        let gap = map[&(String::from("_Gap"), 0, 0, 0)].clone();
        assert_eq!(gap, [r"\Design\Gap A.sym", r"\Design\Gap B.sym"]);
        assert_eq!(document.discriminant_keys_with_multiple_items(), 1);
    }

    const CORPUS_ZIP: &str = r"D:\work\cad\pid-test-data\RefData~4~681.zip";

    fn fixture_zip() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-file/backup-test/TEST02_p/RefData~4~681.zip")
    }

    #[test]
    fn real_refdata_zips_parse_with_expected_counts() {
        // (zip, items, uncompressed bytes, discriminant keys, multi-item keys)
        let cases: [(PathBuf, usize, usize, usize, usize); 2] = [
            (PathBuf::from(CORPUS_ZIP), 581, 481_740, 264, 77),
            (fixture_zip(), 595, 493_202, 265, 79),
        ];
        for (path, items, bytes, keys, multi) in cases {
            if !path.is_file() {
                eprintln!("skip: {} is missing", path.display());
                continue;
            }
            let reading = read_catalog_index_from_zip(&path).unwrap();
            assert_eq!(reading.entry_name, "CatalogIndex.xml");
            assert_eq!(reading.uncompressed_bytes, bytes);
            let document = &reading.document;
            assert_eq!(document.encoding(), CatalogIndexEncoding::Utf16Le);
            assert_eq!(document.blocks().len(), 21);
            assert_eq!(document.item_types().len(), 21);
            assert_eq!(document.item_bearing_item_types().len(), 17);
            assert_eq!(document.items().len(), items);
            assert_eq!(document.sym_reference_count(), items);
            assert_eq!(document.items_by_item_type().values().sum::<usize>(), items);
            assert_eq!(
                document.empty_blocks(),
                ["DuctRun", "DuctingComp", "Room", "RoomComponent"]
            );
            assert_eq!(document.discriminant_key_count(), keys);
            assert_eq!(document.discriminant_keys_with_multiple_items(), multi);
            let first = document.first_item().unwrap();
            assert_eq!(first.item_type, "AreaBreak");
            assert_eq!((first.class, first.subclass, first.type_code), (0, 0, 64));
            assert_eq!(first.aux_string, "");
            assert_eq!(first.catalog_item, r"\Design\Area Break.sym");
        }
    }

    #[test]
    fn real_refdata_zips_cross_check_against_the_publish_catalog() {
        for path in [PathBuf::from(CORPUS_ZIP), fixture_zip()] {
            if !path.is_file() {
                eprintln!("skip: {} is missing", path.display());
                continue;
            }
            let reading = read_catalog_index_from_zip(&path).unwrap();
            let check = crate::publish::catalog::cross_check_catalog_index(&reading.document);
            assert_eq!(
                check.in_both,
                [
                    "EquipmentOther",
                    "Exchanger",
                    "Instrument",
                    "ItemNote",
                    "Mechanical",
                    "Nozzle",
                    "OPC",
                    "PipeRun",
                    "PipingComp",
                    "SignalRun",
                    "Vessel",
                ]
            );
            assert_eq!(
                check.only_in_catalog_index,
                [
                    "AreaBreak",
                    "InstrLoop",
                    "Label",
                    "Package",
                    "System",
                    "_Gap"
                ]
            );
            assert_eq!(
                check.only_in_catalog,
                [
                    "$derived",
                    "$document",
                    "*",
                    "BranchPoint",
                    "PipingBranchPoint",
                    "PipingPoint",
                ]
            );
            assert_eq!(check.matched_through_an_alias, ["ItemNote"]);
            assert_eq!(
                check.catalog_item_types,
                [
                    "$derived",
                    "$document",
                    "*",
                    "BranchPoint",
                    "EquipmentOther",
                    "Exchanger",
                    "Instrument",
                    "Mechanical",
                    "Note",
                    "Nozzle",
                    "OPC",
                    "PipeRun",
                    "PipingBranchPoint",
                    "PipingComp",
                    "PipingPoint",
                    "SignalRun",
                    "Vessel",
                ]
            );
            assert_eq!(check.catalog_aliases, ["InstrFunction", "ItemNote"]);
            assert!(check.empty_difference_sets.is_empty());
        }
    }
}
