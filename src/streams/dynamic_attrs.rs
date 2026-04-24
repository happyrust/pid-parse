use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{DynamicAttributesBlob, PidDocument};
use std::io::Read;

pub fn parse_dynamic_attrs<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
    options: &ParseOptions,
) -> Result<(), PidError> {
    let path = "/Unclustered Dynamic Attributes";
    if let Ok(mut s) = cfb.open_stream(path) {
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;

        let mut seen = std::collections::HashSet::new();
        let mut strings = Vec::new();

        if options.scan_strings {
            for s in crate::parsers::string_scan::scan_ascii_strings(&data, 256) {
                if seen.insert(s.clone()) {
                    strings.push(s);
                }
            }
        }

        for s in crate::parsers::string_scan::scan_utf16le_strings(&data, 4, 256) {
            if seen.insert(s.clone()) {
                strings.push(s);
            }
        }

        let relationships: Vec<String> = strings
            .iter()
            .filter(|s| s.starts_with("Relationship."))
            .cloned()
            .collect();

        let mut class_seen = std::collections::HashSet::new();
        let class_names: Vec<String> = strings
            .iter()
            .filter(|s| {
                matches!(
                    s.as_str(),
                    "Instrument"
                        | "PipingComp"
                        | "PipeRun"
                        | "SignalRun"
                        | "Connector"
                        | "Valves"
                        | "Nozzle"
                        | "ItemNote"
                        | "OPC"
                )
            })
            .filter(|s| class_seen.insert((*s).clone()))
            .cloned()
            .collect();

        let header = crate::parsers::cluster_header::parse_header(&data);
        let (attribute_records, probe_summary) =
            crate::parsers::dynamic_attr_records::parse_attribute_records(&data);
        let relationship_probes = crate::parsers::relationship_probe::probe_relationships(&data);
        let record_trailers = crate::parsers::dynamic_attr_records::extract_record_trailers(&data);

        doc.dynamic_attributes = Some(DynamicAttributesBlob {
            path: path.to_string(),
            size: data.len() as u64,
            magic_u32_le: data
                .get(0..4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            strings,
            relationships,
            class_names,
            raw_preview_hex: hex_preview(&data, 128),
            header,
            attribute_records,
            probe_summary: Some(probe_summary),
            relationship_probes,
            record_trailers,
        });
    }

    Ok(())
}

fn hex_preview(data: &[u8], n: usize) -> String {
    data.iter()
        .take(n)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
