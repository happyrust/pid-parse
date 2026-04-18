//! Apply [`MetadataUpdates`] to an in-memory [`PidPackage`] by overwriting
//! the targeted streams' raw bytes.
//!
//! The first release only handles `/TaggedTxtData/Drawing` and
//! `/TaggedTxtData/General` — i.e. the XML metadata streams parsed by
//! `streams::tagged_text`. `summary_updates` is intentionally a no-op until
//! we commit to a full `SummaryInformation` property-set writer.
use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::MetadataUpdates;

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";

/// Overwrite `/TaggedTxtData/Drawing` and `/TaggedTxtData/General` with the
/// byte content of the provided XML strings. Returns an error if the caller
/// provided an empty XML body (guarded because an empty XML would silently
/// break downstream `quick-xml` parsing).
pub fn apply_metadata_updates(
    package: &mut PidPackage,
    updates: &MetadataUpdates,
) -> Result<(), PidError> {
    if let Some(xml) = updates.drawing_xml.as_ref() {
        validate_xml_not_empty(xml, DRAWING_PATH)?;
        package.replace_stream(DRAWING_PATH, xml.as_bytes().to_vec());
    }
    if let Some(xml) = updates.general_xml.as_ref() {
        validate_xml_not_empty(xml, GENERAL_PATH)?;
        package.replace_stream(GENERAL_PATH, xml.as_bytes().to_vec());
    }
    // `summary_updates` is parked until we ship a dedicated property-set
    // writer. Silently ignored by design; the field is kept so callers can
    // start building plans now without breaking API later.
    Ok(())
}

fn validate_xml_not_empty(xml: &str, context: &str) -> Result<(), PidError> {
    if xml.trim().is_empty() {
        return Err(PidError::ParseFailure {
            context: context.to_string(),
            message: "metadata xml body is empty".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::RawStream;
    use std::collections::BTreeMap;

    fn empty_package() -> PidPackage {
        PidPackage::new(None, BTreeMap::new(), PidDocument::default())
    }

    fn package_with_stream(path: &str, data: &[u8]) -> PidPackage {
        let mut map = BTreeMap::new();
        map.insert(
            path.to_string(),
            RawStream {
                path: path.to_string(),
                data: data.to_vec(),
                modified: false,
            },
        );
        PidPackage::new(None, map, PidDocument::default())
    }

    #[test]
    fn drawing_xml_update_replaces_stream_and_marks_modified() {
        let mut pkg = package_with_stream(DRAWING_PATH, b"<Old/>");
        let updates = MetadataUpdates {
            drawing_xml: Some("<Drawing id=\"1\"/>".to_string()),
            ..Default::default()
        };
        apply_metadata_updates(&mut pkg, &updates).expect("ok");
        let s = pkg.get_stream(DRAWING_PATH).expect("stream");
        assert_eq!(s.data, b"<Drawing id=\"1\"/>");
        assert!(s.modified);
    }

    #[test]
    fn general_xml_update_inserts_when_missing() {
        let mut pkg = empty_package();
        let updates = MetadataUpdates {
            general_xml: Some("<General/>".to_string()),
            ..Default::default()
        };
        apply_metadata_updates(&mut pkg, &updates).expect("ok");
        let s = pkg.get_stream(GENERAL_PATH).expect("stream");
        assert_eq!(s.data, b"<General/>");
    }

    #[test]
    fn empty_xml_is_rejected() {
        let mut pkg = empty_package();
        let updates = MetadataUpdates {
            drawing_xml: Some("   \n".to_string()),
            ..Default::default()
        };
        let err = apply_metadata_updates(&mut pkg, &updates).expect_err("should error");
        match err {
            PidError::ParseFailure { context, .. } => assert_eq!(context, DRAWING_PATH),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn no_updates_is_noop() {
        let mut pkg = package_with_stream(DRAWING_PATH, b"<Keep/>");
        apply_metadata_updates(&mut pkg, &MetadataUpdates::default()).expect("ok");
        let s = pkg.get_stream(DRAWING_PATH).expect("stream");
        assert_eq!(s.data, b"<Keep/>");
        assert!(!s.modified);
    }
}
