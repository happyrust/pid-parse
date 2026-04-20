pub mod diff;
pub mod mermaid;
pub mod report;

use crate::model::{PidDocument, StreamEntry};

/// Names of top-level CFB streams that `pid-parse` already fully or
/// partially decodes. Used by [`unidentified_top_level_streams`] to
/// classify anything else as a decoding target still on the backlog.
///
/// Order is cosmetic — membership is what matters. New decoders should
/// add their stream's root-level name here so [`unidentified_top_level_streams`]
/// stops reporting it as unknown.
pub const KNOWN_TOP_LEVEL_STREAM_NAMES: &[&str] = &[
    "\u{5}SummaryInformation",
    "\u{5}DocumentSummaryInformation",
    "PSMcluster0",
    "StyleCluster",
    "Dynamic Attributes Metadata",
    "Unclustered Dynamic Attributes",
    "PSMroots",
    "PSMclustertable",
    "PSMsegmenttable",
    "DocVersion2",
    "DocVersion3",
    "AppObject",
    "JTaggedTxtStgList",
];

/// Top-level storage-name prefixes whose members are considered
/// "identified" as a group — the storage itself is recognized even if
/// individual stream contents are still being probed.
pub const KNOWN_TOP_LEVEL_STORAGE_PREFIXES: &[&str] = &["Sheet", "TaggedTxtData", "JSite"];

/// Returns every top-level stream (`/Foo`, not `/Foo/Bar`) that does not
/// appear in [`KNOWN_TOP_LEVEL_STREAM_NAMES`] nor whose containing
/// storage starts with one of [`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`].
///
/// The result is the canonical "still unknown to pid-parse" view —
/// call sites (reports / UI / dev tooling) use it to surface "decoding
/// backlog" without duplicating the filter logic.
pub fn unidentified_top_level_streams(doc: &PidDocument) -> Vec<&StreamEntry> {
    doc.streams
        .iter()
        .filter(|s| {
            let path = s.path.trim_start_matches('/');
            !path.contains('/')
                && !KNOWN_TOP_LEVEL_STREAM_NAMES.contains(&path)
                && !KNOWN_TOP_LEVEL_STORAGE_PREFIXES
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PidDocument, StreamEntry};

    fn doc_with_streams(paths: &[&str]) -> PidDocument {
        PidDocument {
            streams: paths
                .iter()
                .map(|p| StreamEntry {
                    path: (*p).to_string(),
                    size: 0,
                    preview_ascii: vec![],
                    magic_u32_le: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn unidentified_empty_for_default_doc() {
        let doc = PidDocument::default();
        assert!(unidentified_top_level_streams(&doc).is_empty());
    }

    #[test]
    fn unidentified_filters_all_known_top_level_names() {
        let mut paths: Vec<&str> = KNOWN_TOP_LEVEL_STREAM_NAMES.to_vec();
        // Add nested paths to prove the filter only looks at top-level.
        paths.push("Sheet1/Payload");
        paths.push("TaggedTxtData/Drawing");
        paths.push("JSite0000/JProperties");
        let doc = doc_with_streams(&paths);
        assert!(
            unidentified_top_level_streams(&doc).is_empty(),
            "all known names + nested paths should filter clean"
        );
    }

    #[test]
    fn unidentified_keeps_unknown_top_level_entries() {
        let doc = doc_with_streams(&[
            "PSMroots",       // known → excluded
            "MysteryStream",  // unknown top-level → included
            "Sheet1/Foo",     // nested → excluded regardless
            "AnotherUnknown", // unknown top-level → included
        ]);
        let leftover: Vec<&str> = unidentified_top_level_streams(&doc)
            .iter()
            .map(|s| s.path.as_str())
            .collect();
        assert_eq!(leftover, vec!["MysteryStream", "AnotherUnknown"]);
    }

    #[test]
    fn unidentified_strips_leading_slash_before_lookup() {
        let doc = doc_with_streams(&["/PSMroots", "/GhostStream"]);
        let leftover: Vec<&str> = unidentified_top_level_streams(&doc)
            .iter()
            .map(|s| s.path.as_str())
            .collect();
        assert_eq!(
            leftover,
            vec!["/GhostStream"],
            "leading slash must not defeat the KNOWN filter"
        );
    }
}
