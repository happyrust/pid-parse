//! Decode the small document-registry streams: `DocVersion3` (version
//! history), `AppObject` (COM plugin registry), `JTaggedTxtStgList`
//! (tagged-text storage map).
//!
//! Each parser is tolerant of missing streams and format mismatches.

use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::PidDocument;
use crate::parsers::{app_object, doc_version, tagged_stg_list};
use std::io::Read;

pub fn parse_doc_registry<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    _options: &ParseOptions,
) -> Result<(), PidError> {
    if let Some(data) = open_optional(cfb, "/DocVersion3")? {
        if let Some(v) = doc_version::parse_doc_version3(&data) {
            doc.version_history = Some(v);
        }
    }
    if let Some(data) = open_optional(cfb, "/AppObject")? {
        if let Some(r) = app_object::parse_app_object(&data) {
            doc.app_object_registry = Some(r);
        }
    }
    if let Some(data) = open_optional(cfb, "/JTaggedTxtStgList")? {
        if let Some(t) = tagged_stg_list::parse_tagged_stg_list(&data) {
            doc.tagged_storages = Some(t);
        }
    }
    Ok(())
}

fn open_optional<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    path: &str,
) -> Result<Option<Vec<u8>>, PidError> {
    match cfb.open_stream(path) {
        Ok(mut s) => {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            Ok(Some(data))
        }
        Err(_) => Ok(None),
    }
}
