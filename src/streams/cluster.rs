use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{ClusterInfo, ClusterKind, PidDocument, SheetStream};
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
            });
        }
    }

    for s in &doc.streams {
        let leaf = s.path.rsplit('/').next().unwrap_or("");
        if leaf.starts_with("Sheet") {
            doc.sheet_streams.push(SheetStream {
                name: leaf.to_string(),
                path: s.path.clone(),
                size: s.size,
                extracted_texts: s.preview_ascii.clone(),
            });
        }
    }

    Ok(())
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
