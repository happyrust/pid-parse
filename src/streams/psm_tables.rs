//! Orchestrator for the `PSMroots` / `PSMclustertable` /
//! `PSMsegmenttable` streams and the `PSMspacemap` storages.
//!
//! Reads the three PSM index streams (when present), dispatches each
//! to [`crate::parsers::psm_tables`], and attaches the decoded tables
//! to the document ([`PidDocument::psm_roots`],
//! [`PidDocument::psm_cluster_table`],
//! [`PidDocument::psm_segment_table`]).
//!
//! The space maps are found by walking rather than by name: the storage
//! appears at the top level and again under each `JSite` registry, and its
//! members are named for the address their segment starts at.

use crate::config::ParseOptions;
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
    parse_space_maps(cfb, doc);
    Ok(())
}

/// Decode every `PSMspacemap` member the document carries.
///
/// A member that does not decode is skipped rather than failing the parse:
/// the byte audit already reports an undecoded one as leftover under its own
/// path, so swallowing it here loses nothing and keeps one odd segment from
/// taking the whole document down.
fn parse_space_maps<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
) {
    let paths: Vec<String> = cfb
        .walk()
        .filter(::cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .filter(|path| psm_tables::is_space_map_member(path))
        .collect();
    for path in paths {
        let Ok(mut stream) = cfb.open_stream(&path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        if let Some(map) = psm_tables::parse_psm_space_map(&data) {
            doc.psm_space_maps.insert(path, map);
        }
    }
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
