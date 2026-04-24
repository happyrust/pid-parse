//! Decoder for the `TaggedTxtData/Drawing` XML body.
//!
//! Turns the raw XML string (as fetched by [`crate::streams::tagged_text`])
//! into a typed [`DrawingMeta`], including the `SmartPlant`
//! `SP_`-prefixed attribute fallbacks for drawing number / UID
//! fields. Writer-side patches go through
//! [`crate::writer::metadata_helpers`] which produces XML this
//! parser can round-trip.

use crate::error::PidError;
use crate::model::DrawingMeta;

pub fn parse_drawing_xml(xml: &str) -> Result<DrawingMeta, PidError> {
    let tags = crate::parsers::xml_util::collect_simple_tags(xml);

    // Helper: try plain key first, then SP_-prefixed key (Smart P&ID convention).
    let get = |key: &str| -> Option<String> {
        tags.get(key)
            .or_else(|| tags.get(&format!("SP_{key}")))
            .cloned()
    };

    Ok(DrawingMeta {
        drawing_number: get("DrawingNumber"),
        document_category: get("DocumentCategory"),
        template_name: get("Template"),
        rules_uid: get("RulesUID"),
        formats_uid: get("FormatsUID"),
        gapping_uid: get("GappingUID"),
        symbology_uid: get("SymbologyUID"),
        default_formats_uid: get("DefaultFormatsUID"),
        raw_xml: xml.to_string(),
        tags,
    })
}
