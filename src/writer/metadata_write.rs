//! Apply [`MetadataUpdates`] to an in-memory [`PidPackage`] by overwriting
//! the targeted streams' raw bytes.
//!
//! Handles four update channels:
//!
//! 1. `/TaggedTxtData/Drawing` XML body (`drawing_xml`)
//! 2. `/TaggedTxtData/General` XML body (`general_xml`)
//! 3. Phase 9l (v0.5.0+): `SummaryInformation` /
//!    `DocumentSummaryInformation` property-set edits via
//!    `summary_updates` — string properties only.
//! 4. Phase 9n (v0.5.2+): property deletions via `summary_deletions`
//!    (same symbolic key table; silent no-op on keys not present in
//!    the source property-set).
//!
//! Ordering within one call is deterministic:
//!
//! - Drawing XML write
//! - General XML write
//! - summary deletions (remove props before re-inserting any)
//! - summary updates
//!
//! A key appearing in both `summary_deletions` and `summary_updates` is
//! rejected up-front with a clear error — the intent is ambiguous (do
//! we keep the new value, or remove the prop?).
use crate::error::PidError;
use crate::package::PidPackage;
use crate::writer::plan::MetadataUpdates;
use crate::writer::summary_write;

const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";

/// Overwrite `/TaggedTxtData/Drawing` / `/TaggedTxtData/General` with the
/// provided XML bodies and apply `summary_updates` / `summary_deletions`
/// to the OLE property-set streams. Empty XML bodies are rejected to
/// avoid producing a `quick-xml`-unparseable stream. Empty
/// summary_updates / summary_deletions are free no-ops.
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

    // Phase 9n: reject ambiguous intent before any side effect.
    for del_key in &updates.summary_deletions {
        if updates.summary_updates.contains_key(del_key) {
            return Err(PidError::ParseFailure {
                context: "summary writer".into(),
                message: format!(
                    "summary_updates and summary_deletions both target key \
                     '{del_key}'; at most one must be specified per key"
                ),
            });
        }
    }
    summary_write::apply_summary_deletions(package, &updates.summary_deletions)?;
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

    #[test]
    fn summary_updates_and_deletions_on_same_key_return_error() {
        // Phase 9n: conflict check runs before any side effect and surfaces
        // the offending key so the caller can self-correct.
        let mut pkg = empty_package();
        let mut summary_updates = std::collections::BTreeMap::new();
        summary_updates.insert("title".to_string(), "new".to_string());
        let updates = MetadataUpdates {
            summary_updates,
            summary_deletions: vec!["title".to_string()],
            ..Default::default()
        };
        let err = apply_metadata_updates(&mut pkg, &updates).expect_err("conflict");
        let msg = format!("{err}");
        assert!(
            msg.contains("summary_updates and summary_deletions both target key 'title'"),
            "got: {msg}"
        );
    }
}
