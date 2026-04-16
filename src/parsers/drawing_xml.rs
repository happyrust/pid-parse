use crate::error::PidError;
use crate::model::DrawingMeta;

pub fn parse_drawing_xml(xml: &str) -> Result<DrawingMeta, PidError> {
    let tags = crate::parsers::xml_util::collect_simple_tags(xml);
    Ok(DrawingMeta {
        drawing_number: tags.get("DrawingNumber").cloned(),
        document_category: tags.get("DocumentCategory").cloned(),
        template_name: tags.get("Template").cloned(),
        rules_uid: tags.get("RulesUID").cloned(),
        formats_uid: tags.get("FormatsUID").cloned(),
        gapping_uid: tags.get("GappingUID").cloned(),
        symbology_uid: tags.get("SymbologyUID").cloned(),
        default_formats_uid: tags.get("DefaultFormatsUID").cloned(),
        raw_xml: xml.to_string(),
        tags,
    })
}
