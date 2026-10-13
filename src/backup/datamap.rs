//! `SPPIDDataMap.xml`: the enum map a backup uses when it publishes.
//!
//! Plan `docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md`
//! N9-2. Only two things are read. A `Rel` whose `DefUID` is
//! `MapEnumToEnum` is the triple (`UID1`, `UID2`). An
//! `SPMapEnumListDef` contributes its `IObject` `UID` and `Name`.
//! The file may be the XML itself or the `SPPIDDataMap.xml` entry
//! inside a zip (dated copies in the same zip are left alone).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

/// The enum map and the list names from one `SPPIDDataMap.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataMap {
    /// `MapEnumToEnum` `UID1` → publish spelling of `UID2` (`@{GUID}`).
    pub enum_to_enum: HashMap<String, String>,
    /// `SPMapEnumListDef` `IObject` `UID` → `Name`.
    pub enum_list_names: HashMap<String, String>,
}

/// Why a data map was not read.
#[derive(Debug)]
pub enum DataMapError {
    /// The file or zip could not be opened.
    Io(std::io::Error),
    /// The zip has no entry whose name is exactly `SPPIDDataMap.xml`.
    MissingEntry,
    /// The XML could not be read as UTF-8 events.
    Xml(String),
}

impl std::fmt::Display for DataMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingEntry => write!(f, "zip has no SPPIDDataMap.xml entry"),
            Self::Xml(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for DataMapError {}

/// Read a data map from an XML file or from the `SPPIDDataMap.xml` entry of a zip.
///
/// # Errors
///
/// Returns [`DataMapError::MissingEntry`] when `path` is a zip without that
/// entry, [`DataMapError::Io`] when the path cannot be read, and
/// [`DataMapError::Xml`] when the events cannot be decoded.
pub fn read_datamap(path: &Path) -> Result<DataMap, DataMapError> {
    let bytes = fs::read(path).map_err(DataMapError::Io)?;
    if bytes.starts_with(b"PK\x03\x04") {
        let entry = sppid_datamap_entry(&bytes)?;
        parse_datamap_xml(&entry)
    } else {
        parse_datamap_xml(&bytes)
    }
}

/// `SP_20_{code}` in the enum map, already spelled `@{GUID}`.
#[must_use]
pub fn fluid_system_guid(map: &DataMap, code: i64) -> Option<&str> {
    map.enum_to_enum
        .get(&format!("SP_20_{code}"))
        .map(String::as_str)
}

fn sppid_datamap_entry(bytes: &[u8]) -> Result<Vec<u8>, DataMapError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|error| DataMapError::Xml(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| DataMapError::Xml(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let raw = entry.name_raw().to_vec();
        let name = decode_zip_name(&raw);
        let base = name.rsplit(['/', '\\']).next().unwrap_or(&name);
        if base != "SPPIDDataMap.xml" {
            continue;
        }
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).map_err(DataMapError::Io)?;
        return Ok(payload);
    }
    Err(DataMapError::MissingEntry)
}

fn parse_datamap_xml(bytes: &[u8]) -> Result<DataMap, DataMapError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut map = DataMap::default();
    let mut in_list = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let qname = start.name();
                let name = local_name(qname.as_ref());
                if name == b"SPMapEnumListDef" {
                    in_list = true;
                } else if name == b"IObject" && in_list {
                    remember_list(&mut map, &start);
                } else if name == b"IRel" {
                    remember_enum(&mut map, &start);
                }
            }
            Ok(Event::Empty(start)) => {
                let qname = start.name();
                let name = local_name(qname.as_ref());
                if name == b"SPMapEnumListDef" {
                    // A list with no child is not a name we can keep.
                } else if name == b"IObject" && in_list {
                    remember_list(&mut map, &start);
                } else if name == b"IRel" {
                    remember_enum(&mut map, &start);
                }
            }
            Ok(Event::End(end)) => {
                if local_name(end.name().as_ref()) == b"SPMapEnumListDef" {
                    in_list = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(DataMapError::Xml(error.to_string())),
            _ => {}
        }
    }
    Ok(map)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn remember_list(map: &mut DataMap, start: &quick_xml::events::BytesStart<'_>) {
    let Some(uid) = attr(start, b"UID") else {
        return;
    };
    let Some(name) = attr(start, b"Name") else {
        return;
    };
    map.enum_list_names.insert(uid, name);
}

fn remember_enum(map: &mut DataMap, start: &quick_xml::events::BytesStart<'_>) {
    if attr(start, b"DefUID").as_deref() != Some("MapEnumToEnum") {
        return;
    }
    let Some(uid1) = attr(start, b"UID1") else {
        return;
    };
    let Some(uid2) = attr(start, b"UID2") else {
        return;
    };
    map.enum_to_enum.insert(uid1, publish_guid(&uid2));
}

fn attr(start: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    start.attributes().flatten().find_map(|attribute| {
        if attribute.key.as_ref() == key {
            attribute
                .unescape_value()
                .ok()
                .map(|value| value.into_owned())
        } else {
            None
        }
    })
}

fn decode_zip_name(raw: &[u8]) -> String {
    if let Ok(utf8) = std::str::from_utf8(raw) {
        return utf8.to_string();
    }
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(raw);
    if had_errors {
        raw.iter().map(|byte| format!("{byte:02x}")).collect()
    } else {
        decoded.into_owned()
    }
}

fn publish_guid(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('@') {
        return format!("@{rest}");
    }
    if trimmed.starts_with('{') {
        format!("@{trimmed}")
    } else {
        format!("@{{{trimmed}}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_enum_and_list_names_are_read() {
        let xml = br#"
            <Container>
              <SPMapEnumListDef>
                <IObject UID="SP_20" Name="Fluid System" Description="Fluid"/>
              </SPMapEnumListDef>
              <Rel>
                <IRel UID1="SP_20_7" UID2="{A729B087-24B7-4418-A185-53CAA90C2286}" DefUID="MapEnumToEnum"/>
              </Rel>
              <Rel>
                <IRel UID1="SP_14_10" UID2="EE68D" DefUID="MapEnumToEnum"/>
              </Rel>
            </Container>
        "#;
        let map = parse_datamap_xml(xml).expect("xml");
        assert_eq!(
            map.enum_list_names.get("SP_20").map(String::as_str),
            Some("Fluid System")
        );
        assert_eq!(
            fluid_system_guid(&map, 7),
            Some("@{A729B087-24B7-4418-A185-53CAA90C2286}")
        );
        assert_eq!(
            map.enum_to_enum.get("SP_14_10").map(String::as_str),
            Some("@{EE68D}")
        );
    }
}
