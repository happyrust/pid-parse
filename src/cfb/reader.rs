use crate::api::ParseOptions;
use crate::error::PidError;
use crate::model::{PidDocument, StreamEntry};
use std::io::Read;
use std::path::Path;

pub fn parse_pid_file(path: &Path, options: &ParseOptions) -> Result<PidDocument, PidError> {
    let mut cfb = ::cfb::open(path)?;
    let tree = crate::cfb::tree::build_tree(&cfb, "/")?;
    let streams = collect_streams(&mut cfb, options)?;

    let mut doc = PidDocument {
        cfb_tree: tree,
        streams,
        ..PidDocument::default()
    };

    crate::streams::summary::parse_summary_streams(&mut cfb, &mut doc)?;

    if options.parse_xml {
        crate::streams::tagged_text::parse_tagged_text_streams(&mut cfb, &mut doc, options)?;
    }

    if options.parse_jsite_properties {
        crate::streams::jsite::parse_jsites(&mut cfb, &mut doc, options)?;
    }

    crate::streams::cluster::parse_clusters(&mut cfb, &mut doc, options)?;
    crate::streams::dynamic_attrs::parse_dynamic_attrs(&mut cfb, &mut doc, options)?;

    build_object_inventory(&mut doc);

    Ok(doc)
}

fn build_object_inventory(doc: &mut PidDocument) {
    use crate::model::{ObjectInventory, PidItem};
    use std::collections::BTreeMap;

    let da = match doc.dynamic_attributes.as_ref() {
        Some(da) if !da.attribute_records.is_empty() => da,
        _ => return,
    };

    let mut inv = ObjectInventory::default();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for rec in &da.attribute_records {
        if rec.class_name != "P&IDAttributes" {
            continue;
        }

        let mut item_type = None;
        let mut drawing_id = None;
        let mut model_id = None;

        for attr in &rec.attributes {
            match attr.name.as_str() {
                "ModelItemType" => {
                    if let crate::model::AttributeValue::Text(v) = &attr.value {
                        if !v.is_empty() {
                            item_type = Some(v.clone());
                        }
                    }
                }
                "DrawingID" => {
                    if let crate::model::AttributeValue::Text(v) = &attr.value {
                        if !v.is_empty() {
                            drawing_id = Some(v.clone());
                        }
                    }
                }
                "ModelID" => {
                    if let crate::model::AttributeValue::Text(v) = &attr.value {
                        if !v.is_empty() {
                            model_id = Some(v.clone());
                        }
                    }
                }
                "ProjectNumber" => {
                    if inv.project.is_none() {
                        if let crate::model::AttributeValue::Text(v) = &attr.value {
                            if !v.is_empty() {
                                inv.project = Some(v.clone());
                            }
                        }
                    }
                }
                "DrawingNo" => {
                    if inv.drawing_id.is_none() {
                        if let crate::model::AttributeValue::Text(v) = &attr.value {
                            if !v.is_empty() {
                                inv.drawing_id = Some(v.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(ref t) = item_type {
            *counts.entry(t.clone()).or_default() += 1;
            inv.items.push(PidItem {
                item_type: t.clone(),
                drawing_id,
                model_id,
            });
        }
    }

    inv.item_counts = counts;
    if !inv.items.is_empty() {
        doc.object_inventory = Some(inv);
    }
}

fn collect_streams<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    options: &ParseOptions,
) -> Result<Vec<StreamEntry>, PidError> {
    let paths: Vec<_> = cfb
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_path_buf())
        .collect();

    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let mut stream = cfb.open_stream(&p)?;
        let mut data = Vec::new();
        stream.read_to_end(&mut data)?;

        let path_str = p.to_string_lossy().replace('\\', "/");
        let preview_ascii = if options.scan_strings {
            crate::parsers::string_scan::scan_ascii_strings(&data, options.max_preview_strings)
        } else {
            vec![]
        };

        let magic_u32_le = data
            .get(0..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));

        out.push(StreamEntry {
            path: path_str,
            size: data.len() as u64,
            preview_ascii,
            magic_u32_le,
        });
    }

    Ok(out)
}
