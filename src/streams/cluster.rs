//! Orchestrator for cluster-family streams.
//!
//! Walks every top-level cluster stream (`PSMcluster0`, `StyleCluster`,
//! `Dynamic Attributes Metadata`, sheet clusters, …), runs the
//! cluster-header probe from [`crate::parsers::cluster_header`], and
//! populates [`PidDocument::clusters`] / [`PidDocument::sheet_streams`].
//! DA records are pulled via [`crate::parsers::dynamic_attr_records`].

use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{
    ClusterInfo, ClusterKind, ClusterProbeInfo, DecodedGraphicGroupRecord, DecodedIgLine2dRecord,
    DecodedIgLineString2dRecord, DecodedIgPoint2dRecord, DecodedIgSymbol2dRecord,
    DecodedIgTextBoxRecord, DecodedJStyleOverrideRecord, DecodedPrimitiveLineRecord, PidDocument,
    SheetCoordinateHintDto, SheetGeometry, SheetStream, SheetText,
};
use crate::parsers::{
    cluster_header, dynamic_attr_records, magic,
    sheet_probe::{self, SheetProbeReport, SheetTextEncoding},
    sheet_records::{
        decode_graphic_groups, decode_iglines, decode_iglinestrings, decode_igpoints,
        decode_igsymbols, decode_igtextboxes, decode_jstyle_overrides, decode_primitive_lines,
    },
};
use std::io::Read;

/// Decode every top-level cluster-family stream (`PSMcluster*`,
/// `StyleCluster`, `Sheet*`) into [`PidDocument::clusters`] /
/// [`PidDocument::sheet_streams`].
pub fn parse_clusters<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    options: &ParseOptions,
) -> Result<(), PidError> {
    let names = [
        "PSMcluster0",
        "StyleCluster",
        "Dynamic Attributes Metadata",
        "Unclustered Dynamic Attributes",
    ];

    for name in names {
        let path = format!("/{name}");
        if let Ok(mut s) = cfb.open_stream(&path) {
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;

            let header = cluster_header::parse_header(&data);

            let (string_table, probe_info) = if name == "PSMcluster0" && data.len() > 32 {
                let (table_start, method) = find_string_table_start(&data);
                let (table, end_offset) = cluster_header::parse_string_table(&data, table_start);
                let pi = ClusterProbeInfo {
                    string_table_offset: table_start,
                    detection_method: method,
                    entries_parsed: table.len(),
                    end_offset,
                };
                if table.is_empty() {
                    (None, Some(pi))
                } else {
                    (Some(table), Some(pi))
                }
            } else {
                (None, None)
            };

            doc.clusters.push(ClusterInfo {
                name: name.to_string(),
                path: path.clone(),
                size: data.len() as u64,
                magic_u32_le: data
                    .get(0..4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                extracted_strings: if options.scan_strings {
                    crate::parsers::string_scan::scan_ascii_strings(&data, 128)
                } else {
                    vec![]
                },
                kind: classify_cluster(name),
                header,
                string_table,
                probe_info,
            });
        }
    }

    // Sheet streams share the same magic 0x6C90F544 as clusters: reuse the
    // cluster header parser and the DA attribute-record probe on them.
    let sheet_paths: Vec<(String, String, u64, Vec<String>)> = doc
        .streams
        .iter()
        .filter_map(|s| {
            let leaf = s.path.rsplit('/').next().unwrap_or("");
            if leaf.starts_with("Sheet") {
                Some((
                    leaf.to_string(),
                    s.path.clone(),
                    s.size,
                    s.preview_ascii.clone(),
                ))
            } else {
                None
            }
        })
        .collect();

    for (name, path, size, preview) in sheet_paths {
        let (magic_u32_le, magic_tag, header, attribute_records, probe_summary, geometry) =
            if let Ok(mut stream) = cfb.open_stream(&path) {
                let mut data = Vec::new();
                stream.read_to_end(&mut data)?;

                let m = data
                    .get(0..4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let tag = m.and_then(magic::magic_tag);
                let hdr = cluster_header::parse_header(&data);
                let (records, summary) = dynamic_attr_records::parse_attribute_records(&data);
                let sheet_probe =
                    sheet_probe::probe_sheet_stream(&name, &path, &data, &Default::default());
                let geometry = sheet_geometry_from_probe(&sheet_probe, &data);

                (m, tag, hdr, records, Some(summary), geometry)
            } else {
                (None, None, None, Vec::new(), None, None)
            };

        doc.sheet_streams.push(SheetStream {
            name,
            path,
            size,
            extracted_texts: preview,
            magic_u32_le,
            magic_tag,
            header,
            attribute_records,
            probe_summary,
            geometry,
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });
    }

    Ok(())
}

/// Heuristic: find where the indexed string table starts in `PSMcluster0`.
/// Scans for a [u32 `byte_len` (even, 4..512)] followed by valid UTF-16LE text,
/// then backs up 4 bytes to include the preceding u32 index field.
/// Returns (offset, `detection_method`) for the string table start.
fn find_string_table_start(data: &[u8]) -> (usize, String) {
    // First try to find index=2 entry (reliably u32-aligned) and back-derive entry 1
    for i in 20..data.len().saturating_sub(12) {
        let val = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if val == 2 {
            let blen =
                u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
            if (4..512).contains(&blen) && blen.is_multiple_of(2) && i + 8 + blen <= data.len() {
                let first_char = u16::from_le_bytes([data[i + 8], data[i + 9]]);
                if (0x20..=0x7e).contains(&first_char) {
                    // Walk back to find entry 1: look for a valid byte_len before this
                    if let Some(entry1_start) = find_entry1_before(data, i) {
                        return (entry1_start, "entry2_backtrack".to_string());
                    }
                    return (i, "entry2_direct".to_string());
                }
            }
        }
    }
    (32, "fallback".to_string())
}

/// Given the start of entry 2, walk backwards to find entry 1's index field.
fn find_entry1_before(data: &[u8], entry2_pos: usize) -> Option<usize> {
    // Entry 1 ends right before entry2_pos. The string of entry 1 ends at entry2_pos.
    // Entry format: [u32 index] [u32 byte_len] [UTF-16LE string of byte_len bytes]
    // So byte_len is at (entry2_pos - string_bytes - 4), index is at (entry2_pos - string_bytes - 8).
    // We scan backwards from entry2_pos trying plausible byte_lens.
    for blen in (4..=256).step_by(2) {
        let str_start = entry2_pos.checked_sub(blen)?;
        let blen_pos = str_start.checked_sub(4)?;
        let idx_pos = blen_pos.checked_sub(4)?;
        if idx_pos < 16 {
            continue;
        }
        let stored_blen = u32::from_le_bytes([
            data[blen_pos],
            data[blen_pos + 1],
            data[blen_pos + 2],
            data[blen_pos + 3],
        ]) as usize;
        if stored_blen != blen {
            continue;
        }
        let first_char = u16::from_le_bytes([data[str_start], data[str_start + 1]]);
        if (0x20..=0x7e).contains(&first_char) {
            let idx_val = u32::from_le_bytes([
                data[idx_pos],
                data[idx_pos + 1],
                data[idx_pos + 2],
                data[idx_pos + 3],
            ]);
            if idx_val <= 10 {
                return Some(idx_pos);
            }
            // Sometimes the first entry has a different prefix layout;
            // try including extra prefix bytes
            for extra in 1..=4 {
                let alt_start = idx_pos.checked_sub(extra)?;
                if alt_start >= 16 {
                    return Some(alt_start);
                }
            }
        }
    }
    None
}

fn sheet_geometry_from_probe(report: &SheetProbeReport, raw_data: &[u8]) -> Option<SheetGeometry> {
    let texts: Vec<_> = report
        .text_runs
        .iter()
        .map(|run| SheetText {
            offset: run.offset,
            encoding: sheet_text_encoding_label(&run.encoding).to_string(),
            text: run.text.clone(),
            byte_len: run.byte_len,
        })
        .collect();
    let coordinate_hints: Vec<_> = report
        .coordinate_hints
        .iter()
        .map(|hint| SheetCoordinateHintDto {
            offset: hint.offset,
            x: hint.x,
            y: hint.y,
        })
        .collect();

    // Phase 14/16: walk the raw stream for PSM-encoded `GLine2d`,
    // standard IGDS primitives, GraphicGroup audit records, and
    // authoritative `JStyleOverride` records. All decoders are
    // conservative — they emit zero records when the stream uses
    // a different record shape and never panic.
    let decoded_primitive_lines: Vec<DecodedPrimitiveLineRecord> = decode_primitive_lines(raw_data)
        .into_iter()
        .map(DecodedPrimitiveLineRecord::from)
        .collect();
    let decoded_iglines: Vec<DecodedIgLine2dRecord> = decode_iglines(raw_data)
        .into_iter()
        .map(DecodedIgLine2dRecord::from)
        .collect();
    let decoded_iglinestrings: Vec<DecodedIgLineString2dRecord> = decode_iglinestrings(raw_data)
        .into_iter()
        .map(DecodedIgLineString2dRecord::from)
        .collect();
    let decoded_igpoints: Vec<DecodedIgPoint2dRecord> = decode_igpoints(raw_data)
        .into_iter()
        .map(DecodedIgPoint2dRecord::from)
        .collect();
    let decoded_igtextboxes: Vec<DecodedIgTextBoxRecord> = decode_igtextboxes(raw_data)
        .into_iter()
        .map(DecodedIgTextBoxRecord::from)
        .collect();
    let decoded_igsymbols: Vec<DecodedIgSymbol2dRecord> = decode_igsymbols(raw_data)
        .into_iter()
        .map(DecodedIgSymbol2dRecord::from)
        .collect();
    let decoded_graphic_groups: Vec<DecodedGraphicGroupRecord> = decode_graphic_groups(raw_data)
        .into_iter()
        .map(DecodedGraphicGroupRecord::from)
        .collect();
    let decoded_jstyle_overrides: Vec<DecodedJStyleOverrideRecord> =
        decode_jstyle_overrides(raw_data)
            .into_iter()
            .map(DecodedJStyleOverrideRecord::from)
            .collect();

    if texts.is_empty()
        && coordinate_hints.is_empty()
        && decoded_primitive_lines.is_empty()
        && decoded_iglines.is_empty()
        && decoded_iglinestrings.is_empty()
        && decoded_igpoints.is_empty()
        && decoded_igtextboxes.is_empty()
        && decoded_igsymbols.is_empty()
        && decoded_graphic_groups.is_empty()
        && decoded_jstyle_overrides.is_empty()
    {
        None
    } else {
        Some(SheetGeometry {
            texts,
            endpoints: Vec::new(),
            coordinate_hints,
            object_geometry_hints: Vec::new(),
            decoded_primitive_lines,
            decoded_iglines,
            decoded_iglinestrings,
            decoded_igpoints,
            decoded_igtextboxes,
            decoded_igsymbols,
            decoded_graphic_groups,
            decoded_jstyle_overrides,
        })
    }
}

fn sheet_text_encoding_label(encoding: &SheetTextEncoding) -> &'static str {
    match encoding {
        SheetTextEncoding::Ascii => "ascii",
        SheetTextEncoding::Utf16Le => "utf16_le",
    }
}

fn classify_cluster(name: &str) -> ClusterKind {
    match name {
        "PSMcluster0" => ClusterKind::PsmCluster,
        "StyleCluster" => ClusterKind::StyleCluster,
        "Dynamic Attributes Metadata" => ClusterKind::DynamicAttributesMetadata,
        "Unclustered Dynamic Attributes" => ClusterKind::UnclusteredDynamicAttributes,
        n if n.starts_with("Sheet") => ClusterKind::Sheet,
        _ => ClusterKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::sheet_probe::{
        SheetCoordinateHint, SheetProbeReport, SheetTextEncoding, SheetTextRun,
    };
    use std::collections::BTreeMap;

    #[test]
    fn geometry_from_sheet_probe_normalizes_text_and_coordinate_hints() {
        let report = SheetProbeReport {
            sheet_name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 64,
            candidate_boundaries: Vec::new(),
            chunks: Vec::new(),
            record_type_counts: BTreeMap::new(),
            text_runs: vec![SheetTextRun {
                offset: 8,
                encoding: SheetTextEncoding::Utf16Le,
                text: "PUMP-101".into(),
                byte_len: 16,
            }],
            coordinate_hints: vec![SheetCoordinateHint {
                offset: 32,
                x: 1200,
                y: -450,
            }],
        };

        let geometry = sheet_geometry_from_probe(&report, &[]).expect("geometry evidence");

        assert_eq!(geometry.texts.len(), 1);
        assert_eq!(geometry.texts[0].encoding, "utf16_le");
        assert_eq!(geometry.texts[0].text, "PUMP-101");
        assert_eq!(geometry.coordinate_hints.len(), 1);
        assert_eq!(geometry.coordinate_hints[0].x, 1200);
        assert_eq!(geometry.coordinate_hints[0].y, -450);
        assert!(geometry.endpoints.is_empty());
    }
}
