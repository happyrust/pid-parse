use crate::error::PidError;
use crate::model::GeneralMeta;

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
