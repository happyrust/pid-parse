//! Apply [`MetadataUpdates`] to an in-memory [`PidPackage`] by overwriting
//! the targeted streams' raw bytes.
//!
//! Handles three update channels:
//!
//! 1. `/TaggedTxtData/Drawing` XML body (`drawing_xml`)
//! 2. `/TaggedTxtData/General` XML body (`general_xml`)
//! 3. Phase 9l (v0.5.0+): `SummaryInformation` and
//!    `DocumentSummaryInformation` OLE property-set edits via
//!    `summary_updates` — string properties only. See
//!    [`crate::writer::summary_write`] for the key table.
use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::MetadataUpdates;
use crate::writer::summary_write;

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";

/// Overwrite `/TaggedTxtData/Drawing` / `/TaggedTxtData/General` with the
/// provided XML bodies and apply `summary_updates` to the OLE property-set
/// streams. Empty XML bodies are rejected to avoid producing a
/// `quick-xml`-unparseable stream. An empty `summary_updates` map is a
/// free no-op.
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
    summary_write::apply_summary_updates(package, &updates.summary_updates)?;
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
