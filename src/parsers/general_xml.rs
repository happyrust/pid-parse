//! Decoder for the `TaggedTxtData/General` XML body.
//!
//! Sibling of [`crate::parsers::drawing_xml`] but for the
//! file-scoped metadata — path, file size, and the free-form tag
//! bag. Output feeds [`crate::model::GeneralMeta`].

use crate::error::PidError;
use crate::model::GeneralMeta;

/// Parse the `/TaggedTxtData/General` XML body into a
/// [`GeneralMeta`]. Returns `PidError::Xml` on malformed input.
pub fn parse_general_xml(xml: &str) -> Result<GeneralMeta, PidError> {
    let tags = crate::parsers::xml_util::collect_simple_tags(xml);
    Ok(GeneralMeta {
        file_path: tags
            .get("Location")
            .cloned()
            .or_else(|| tags.get("Path").cloned())
            .or_else(|| tags.get("FilePath").cloned()),
        file_size: tags
            .get("Size")
            .cloned()
            .or_else(|| tags.get("FileSize").cloned()),
        raw_xml: xml.to_string(),
        tags,
    })
}
