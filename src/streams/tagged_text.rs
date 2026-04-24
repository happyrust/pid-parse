//! Orchestrator for `TaggedTxtData/*` + `JTaggedTxtStgList`.
//!
//! Walks every `TaggedTxtData/<Name>` stream, feeds the body to the
//! matching parser (`drawing_xml` for `Drawing`, `general_xml` for
//! `General`, raw preservation for the rest), and populates the
//! corresponding slots on [`PidDocument`] plus the
//! [`PidDocument::tagged_storages`] index.

use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::PidDocument;
use std::io::Read;

pub fn parse_tagged_text_streams<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    _options: &ParseOptions,
) -> Result<(), PidError> {
    if let Ok(mut s) = cfb.open_stream("/TaggedTxtData/Drawing") {
        let mut xml = String::new();
        s.read_to_string(&mut xml)?;
        doc.drawing_meta = Some(crate::parsers::drawing_xml::parse_drawing_xml(&xml)?);
    }

    if let Ok(mut s) = cfb.open_stream("/TaggedTxtData/General") {
        let mut xml = String::new();
        s.read_to_string(&mut xml)?;
        doc.general_meta = Some(crate::parsers::general_xml::parse_general_xml(&xml)?);
    }

    Ok(())
}
