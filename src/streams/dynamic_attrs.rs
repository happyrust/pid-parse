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

        let mut strings = if options.scan_strings {
            crate::parsers::string_scan::scan_ascii_strings(&data, 256)
        } else {
            vec![]
        };

        for value in crate::parsers::string_scan::scan_utf16le_strings(&data, 4, 256) {
            if !strings.contains(&value) {
                strings.push(value);
            }
        }

        let relationships = strings
            .iter()
            .filter(|s| s.starts_with("Relationship."))
            .cloned()
            .collect();

        let class_names = strings
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
            .cloned()
            .collect();

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
        });
    }

    Ok(())
}

fn hex_preview(data: &[u8], n: usize) -> String {
    data.iter()
        .take(n)
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
