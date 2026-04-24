//! Orchestrator for the `PSMroots` / `PSMclustertable` /
//! `PSMsegmenttable` streams.
//!
//! Reads the three PSM index streams (when present), dispatches each
//! to [`crate::parsers::psm_tables`], and attaches the decoded tables
//! to the document ([`PidDocument::psm_roots`],
//! [`PidDocument::psm_cluster_table`],
//! [`PidDocument::psm_segment_table`]).

use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::PidDocument;
use crate::parsers::psm_tables;
use std::io::Read;

/// Parse the `PSMroots`, `PSMclustertable`, `PSMsegmenttable` streams if
/// present and attach the decoded tables to the document.
pub fn parse_psm_tables<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    _options: &ParseOptions,
) -> Result<(), PidError> {
    if let Some(data) = open_optional(cfb, "/PSMroots")? {
        if let Some(r) = psm_tables::parse_psm_roots(&data) {
            doc.psm_roots = Some(r);
        }
    }
    if let Some(data) = open_optional(cfb, "/PSMclustertable")? {
        if let Some(t) = psm_tables::parse_psm_cluster_table(&data) {
            doc.psm_cluster_table = Some(t);
        }
    }
    if let Some(data) = open_optional(cfb, "/PSMsegmenttable")? {
        if let Some(mut t) = psm_tables::parse_psm_segment_table(&data) {
            // Phase 11b-probe: backfill owner-cluster hints on each segment
            // probe using the cluster table parsed just above (guaranteed
            // to be the same fixture). Conservative fallback — hints are
            // only filled when lengths agree.
            psm_tables::apply_segment_owner_hints(&mut t, doc.psm_cluster_table.as_ref());
            doc.psm_segment_table = Some(t);
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
