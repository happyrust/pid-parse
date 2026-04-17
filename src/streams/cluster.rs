use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{ClusterInfo, ClusterKind, ClusterProbeInfo, PidDocument, SheetStream};
use crate::parsers::{cluster_header, dynamic_attr_records, magic};
use std::io::Read;

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
        let path = format!("/{}", name);
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
        let (magic_u32_le, magic_tag, header, attribute_records, probe_summary) =
            if let Ok(mut stream) = cfb.open_stream(&path) {
                let mut data = Vec::new();
                stream.read_to_end(&mut data)?;

                let m = data
                    .get(0..4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let tag = m.and_then(magic::magic_tag);
                let hdr = cluster_header::parse_header(&data);
                let (records, summary) = dynamic_attr_records::parse_attribute_records(&data);

                (m, tag, hdr, records, Some(summary))
            } else {
                (None, None, None, Vec::new(), None)
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
            endpoint_records: Vec::new(),
        });
    }

    Ok(())
}

/// Heuristic: find where the indexed string table starts in PSMcluster0.
/// Scans for a [u32 byte_len (even, 4..512)] followed by valid UTF-16LE text,
/// then backs up 4 bytes to include the preceding u32 index field.
/// Returns (offset, detection_method) for the string table start.
fn find_string_table_start(data: &[u8]) -> (usize, String) {
    // First try to find index=2 entry (reliably u32-aligned) and back-derive entry 1
    for i in 20..data.len().saturating_sub(12) {
        let val = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if val == 2 {
            let blen = u32::from_le_bytes([
                data[i + 4],
                data[i + 5],
                data[i + 6],
                data[i + 7],
            ]) as usize;
            if blen >= 4 && blen < 512 && blen % 2 == 0 && i + 8 + blen <= data.len() {
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
        let stored_blen =
            u32::from_le_bytes([data[blen_pos], data[blen_pos + 1], data[blen_pos + 2], data[blen_pos + 3]])
                as usize;
        if stored_blen != blen {
            continue;
        }
        let first_char = u16::from_le_bytes([data[str_start], data[str_start + 1]]);
        if (0x20..=0x7e).contains(&first_char) {
            let idx_val =
                u32::from_le_bytes([data[idx_pos], data[idx_pos + 1], data[idx_pos + 2], data[idx_pos + 3]]);
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
