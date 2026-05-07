//! CFB → [`crate::model::PidDocument`] / [`crate::package::PidPackage`]
//! entry points.
//!
//! Wraps the [`cfb`] crate with the `.pid`-specific orchestration
//! needed to fan out stream decoding, populate the model, and (in
//! the `parse_pid_package` path) retain every stream's raw bytes for
//! round-trip writing. Every public entry point funnels through here
//! — [`crate::api::PidParser`] is a thin facade over these
//! functions.

use crate::api::{ParseOptions, ParseProfile};
use crate::error::PidError;
use crate::model::{PidDocument, SheetEndpoint, SheetGeometry, SheetStream, StreamEntry};
use crate::package::{PidPackage, RawStream, StorageTimestamps};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// CFB-spec epoch: 1601-01-01 UTC. `cfb` returns this value for storages
/// whose timestamp field was never set by the producer; we treat it as
/// "absent" in [`StorageTimestamps`] so writer round-trips don't accidentally
/// materialize a bogus 1601 date into the output container.
fn cfb_epoch_1601() -> SystemTime {
    // 1601-01-01 UTC is 11_644_473_600 seconds before UNIX_EPOCH (1970-01-01).
    UNIX_EPOCH - std::time::Duration::from_secs(11_644_473_600)
}

/// Thin wrapper over [`parse_pid_package`] that returns only the decoded
/// model, preserving pre-0.3.2 behavior.
pub fn parse_pid_file(path: &Path, options: &ParseOptions) -> Result<PidDocument, PidError> {
    Ok(parse_pid_package(path, options)?.parsed)
}

/// Parse a `.pid` file into a [`PidPackage`], keeping every stream's raw
/// bytes alongside the decoded [`PidDocument`]. This is the input format
/// consumed by [`crate::writer::PidWriter`] for round-trip writes.
pub fn parse_pid_package(path: &Path, options: &ParseOptions) -> Result<PidPackage, PidError> {
    let mut cfb = ::cfb::open(path)?;
    parse_pid_package_from_cfb(&mut cfb, Some(path.to_path_buf()), options)
}

/// Parse a `.pid` compound file from any in-memory or custom reader.
///
/// The returned package has `source_path == None` because the bytes did
/// not come from a stable filesystem path.
pub fn parse_pid_package_from_reader<R: Read + std::io::Seek>(
    reader: R,
    options: &ParseOptions,
) -> Result<PidPackage, PidError> {
    let mut cfb = ::cfb::CompoundFile::open(reader)?;
    parse_pid_package_from_cfb(&mut cfb, None, options)
}

fn parse_pid_package_from_cfb<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    source_path: Option<PathBuf>,
    options: &ParseOptions,
) -> Result<PidPackage, PidError> {
    let light_profile = options.profile == ParseProfile::Light;
    let tree = crate::cfb::tree::build_tree(cfb, "/")?;
    // Capture the root CLSID + all non-root storage CLSIDs before we hand
    // the cfb off to the collectors — `walk()` / `root_entry()` borrow the
    // compound-file, so they must run before the `&mut cfb` borrows below.
    let root_clsid_raw = *cfb.root_entry().clsid();
    let root_clsid = if root_clsid_raw.is_nil() {
        None
    } else {
        Some(root_clsid_raw)
    };
    // Walk entries once to capture non-root CLSIDs, storage timestamps,
    // and non-zero state_bits. We run this pass before handing the cfb
    // off to `collect_streams_and_bytes` because `walk()` borrows `&cfb`
    // and shares scope with the later `&mut cfb`.
    let cfb_epoch = cfb_epoch_1601();
    let mut storage_clsids: BTreeMap<String, ::uuid::Uuid> = BTreeMap::new();
    let mut storage_timestamps: BTreeMap<String, StorageTimestamps> = BTreeMap::new();
    let mut state_bits: BTreeMap<String, u32> = BTreeMap::new();
    for e in cfb.walk() {
        let path_str = e.path().to_string_lossy().replace('\\', "/");
        if e.is_storage() {
            // CLSID: non-root + non-nil only (matches v0.3.7 semantics).
            if e.path() != std::path::Path::new("/") {
                let c = *e.clsid();
                if !c.is_nil() {
                    storage_clsids.insert(path_str.clone(), c);
                }
            }
            // Timestamps: record both created + modified when non-epoch.
            // We include the root here — its timestamps are meaningful for
            // SmartPlant / SPPID host audit.
            let created = Some(e.created()).filter(|&t| t != cfb_epoch);
            let modified = Some(e.modified()).filter(|&t| t != cfb_epoch);
            if created.is_some() || modified.is_some() {
                storage_timestamps
                    .insert(path_str.clone(), StorageTimestamps { created, modified });
            }
        }
        // State bits apply to both storages and streams; record non-zero
        // values. Keep this map sparse — zeros are the CFB default.
        let sb = e.state_bits();
        if sb != 0 {
            state_bits.insert(path_str, sb);
        }
    }
    let (streams, raw_streams) = collect_streams_and_bytes(cfb, options)?;

    let mut doc = PidDocument {
        cfb_tree: tree,
        streams,
        ..PidDocument::default()
    };
    if options.keep_unknown_streams {
        doc.unknown_streams = crate::inspect::unidentified_top_level_streams(&doc)
            .into_iter()
            .map(|stream| crate::model::UnknownStream {
                path: stream.path.clone(),
                size: stream.size,
                magic_u32_le: stream.magic_u32_le,
                magic_tag: stream
                    .magic_u32_le
                    .and_then(crate::parsers::magic::magic_tag),
            })
            .collect();
    }

    crate::streams::summary::parse_summary_streams(cfb, &mut doc)?;

    if options.parse_xml && !light_profile {
        crate::streams::tagged_text::parse_tagged_text_streams(cfb, &mut doc, options)?;
    }

    if options.parse_jsite_properties && !light_profile {
        crate::streams::jsite::parse_jsites(cfb, &mut doc, options)?;
    }

    crate::streams::cluster::parse_clusters(cfb, &mut doc, options)?;
    if !light_profile {
        crate::streams::dynamic_attrs::parse_dynamic_attrs(cfb, &mut doc, options)?;
        crate::streams::psm_tables::parse_psm_tables(cfb, &mut doc, options)?;
        crate::streams::doc_registry::parse_doc_registry(cfb, &mut doc, options)?;
        capture_doc_version2(cfb, &mut doc)?;
        populate_sheet_endpoints(cfb, &mut doc)?;

        build_object_inventory(&mut doc);
        build_object_graph(&mut doc);

        doc.cross_reference = Some(crate::crossref::build_graph(&doc));
        populate_geometry_hints(&raw_streams, &mut doc);
        crate::layout::derive_layout(&mut doc);
    }

    Ok(PidPackage::new(source_path, raw_streams, doc)
        .with_root_clsid(root_clsid)
        .with_storage_clsids(storage_clsids)
        .with_storage_timestamps(storage_timestamps)
        .with_state_bits(state_bits))
}

/// After both `parse_clusters` and `parse_dynamic_attrs` have run, scan each
/// already-discovered Sheet stream for relationship endpoint-pair records.
///
/// This is a two-phase step because the parser needs the set of
/// relationship `field_x` values from the DA trailers to stay strict;
/// running it inline from `parse_clusters` would require either caching
/// sheet bytes or reordering the CFB ingestion pipeline, both more
/// invasive than a simple second read pass on the (already small) sheet
/// streams.
fn populate_sheet_endpoints<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
) -> Result<(), PidError> {
    use std::collections::HashSet;
    let Some(ref da) = doc.dynamic_attributes else {
        return Ok(());
    };
    // Relationship records are identified by class_id=0xF6 in the DA trailer.
    // (See `DaRecordTrailer` doc for the other observed class_id values.)
    let rel_field_xs: HashSet<u32> = da
        .record_trailers
        .iter()
        .filter(|t| t.class_id == 0x0000_00F6)
        .map(|t| t.field_x)
        .collect();
    if rel_field_xs.is_empty() {
        return Ok(());
    }
    for sheet in &mut doc.sheet_streams {
        let mut s = match cfb.open_stream(&sheet.path) {
            Ok(s) => s,
            Err(e) => {
                sheet.endpoint_decode_error = Some(format!(
                    "failed to reopen sheet stream for endpoint records: {e}"
                ));
                continue;
            }
        };
        let mut data = Vec::new();
        if let Err(e) = s.read_to_end(&mut data) {
            sheet.endpoint_decode_error = Some(format!(
                "failed to read sheet stream for endpoint records: {e}"
            ));
            continue;
        }
        sheet.endpoint_records = crate::parsers::sheet_endpoint_records::parse_endpoint_records(
            &sheet.path,
            &data,
            &rel_field_xs,
        );
        sync_sheet_geometry_endpoints(sheet);
    }
    Ok(())
}

fn sync_sheet_geometry_endpoints(sheet: &mut SheetStream) {
    if sheet.endpoint_records.is_empty() {
        if let Some(geometry) = &mut sheet.geometry {
            geometry.endpoints.clear();
        }
        return;
    }

    let geometry = sheet.geometry.get_or_insert_with(SheetGeometry::default);
    geometry.endpoints = sheet
        .endpoint_records
        .iter()
        .map(|record| SheetEndpoint {
            offset: record.offset,
            rel_field_x: record.rel_field_x,
            endpoint_a: record.endpoint_a,
            endpoint_b: record.endpoint_b,
        })
        .collect();
}

fn populate_geometry_hints(raw_streams: &BTreeMap<String, RawStream>, doc: &mut PidDocument) {
    use std::collections::HashSet;

    let Some(ref cross) = doc.cross_reference else {
        return;
    };
    let Some(ref da) = doc.dynamic_attributes else {
        return;
    };

    let identity_index =
        crate::parsers::sheet_probe::sheet_identity_index_from_trailers(&da.record_trailers);
    let object_field_xs: HashSet<u32> = doc
        .object_graph
        .as_ref()
        .map(|graph| {
            graph
                .objects
                .iter()
                .filter_map(|object| object.field_x)
                .collect()
        })
        .unwrap_or_default();

    for sheet in &mut doc.sheet_streams {
        let Some(raw) = raw_streams.get(&sheet.path) else {
            continue;
        };

        let mut field_xs: Vec<_> = cross
            .relationship_endpoint_links
            .iter()
            .filter(|link| link.sheet_path.as_deref() == Some(sheet.path.as_str()))
            .flat_map(|link| [link.source_field_x, link.target_field_x])
            .flatten()
            .collect();
        field_xs.sort_unstable();
        field_xs.dedup();

        if field_xs.is_empty() {
            continue;
        }

        let report = crate::parsers::sheet_probe::probe_sheet_stream(
            &sheet.name,
            &sheet.path,
            &raw.data,
            &Default::default(),
        );
        let windows = crate::parsers::sheet_probe::field_x_windows(&raw.data, &field_xs, 96);
        let features = crate::parsers::sheet_probe::field_x_window_features(
            &raw.data,
            &windows,
            &report.chunks,
        );
        let identities = crate::parsers::sheet_probe::field_x_window_identities(
            &raw.data,
            &windows,
            &identity_index,
        );
        let scores = crate::parsers::sheet_probe::score_field_x_window_features_with_identities(
            &features,
            &object_field_xs,
            &identities,
        );

        let hints = crate::parsers::sheet_probe::populate_object_geometry_hints(&scores, 70);

        if !hints.is_empty() {
            if let Some(geometry) = &mut sheet.geometry {
                geometry.object_geometry_hints = hints;
            }
        }
    }
}

fn capture_doc_version2<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
) -> Result<(), PidError> {
    if let Ok(mut s) = cfb.open_stream("/DocVersion2") {
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;
        if data.len() < 4 {
            return Ok(());
        }
        let magic_u32_le = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let hex_preview = data
            .iter()
            .take(128)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        doc.doc_version2 = Some(crate::model::DocVersion2Raw {
            size: data.len() as u64,
            magic_u32_le,
            hex_preview,
        });
        // Try structured decode (v0.3.8+). The raw field above is always
        // populated for audit / fallback even when the structured decode
        // succeeds, because it's the source of truth for round-trip.
        doc.doc_version2_decoded = crate::parsers::doc_version2::parse_doc_version2(&data);
    }
    Ok(())
}

fn build_object_graph(doc: &mut PidDocument) {
    use crate::model::{AttributeValue, ObjectGraph, PidObject, PidRelationship};
    use std::collections::BTreeMap;

    let da = match doc.dynamic_attributes.as_ref() {
        Some(da) if !da.record_trailers.is_empty() => da,
        _ => return,
    };

    // Index AttributeRecords by the DrawingID text we find inside their
    // attribute list, so we can enrich each trailer with item_type /
    // DrawingItemType / ModelID / extras when they're present.
    let mut attrs_by_drawing_id: BTreeMap<String, Vec<&crate::model::AttributeRecord>> =
        BTreeMap::new();
    let mut project_number_global: Option<String> = None;
    let mut drawing_no_global: Option<String> = None;
    for rec in &da.attribute_records {
        if rec.class_name != "P&IDAttributes" {
            continue;
        }
        let mut did: Option<String> = None;
        for attr in &rec.attributes {
            let AttributeValue::Text(ref v) = attr.value else {
                continue;
            };
            if v.is_empty() {
                continue;
            }
            match attr.name.as_str() {
                "DrawingID" if v.len() == 32 && v.chars().all(|c| c.is_ascii_hexdigit()) => {
                    did = Some(v.clone());
                }
                "ProjectNumber" if project_number_global.is_none() => {
                    project_number_global = Some(v.clone());
                }
                "DrawingNo" if drawing_no_global.is_none() => {
                    drawing_no_global = Some(v.clone());
                }
                _ => {}
            }
        }
        if let Some(d) = did {
            attrs_by_drawing_id.entry(d).or_default().push(rec);
        }
    }

    // Helper to extract enriched fields (item_type / drawing_item_type /
    // model_id / extras) by merging all AttributeRecords that share the
    // same DrawingID. Records sharing a DrawingID are uncommon but the
    // parser sometimes emits more than one; we keep the first non-empty
    // value we see for each field.
    let enrich = |did: &str| -> (
        Option<String>,
        Option<String>,
        Option<String>,
        BTreeMap<String, String>,
    ) {
        let mut item_type: Option<String> = None;
        let mut drawing_item_type: Option<String> = None;
        let mut model_id: Option<String> = None;
        let mut extra: BTreeMap<String, String> = BTreeMap::new();
        let Some(recs) = attrs_by_drawing_id.get(did) else {
            return (item_type, drawing_item_type, model_id, extra);
        };
        for rec in recs {
            for attr in &rec.attributes {
                let value = match &attr.value {
                    AttributeValue::Text(v) if !v.is_empty() => v.clone(),
                    AttributeValue::Integer(n) => n.to_string(),
                    AttributeValue::Float(f) => f.to_string(),
                    _ => continue,
                };
                match attr.name.as_str() {
                    "ModelItemType" => {
                        if item_type.is_none() {
                            item_type = Some(value);
                        }
                    }
                    "DrawingItemType" => {
                        if drawing_item_type.is_none() {
                            drawing_item_type = Some(value);
                        }
                    }
                    "ModelID" => {
                        if model_id.is_none() {
                            model_id = Some(value);
                        }
                    }
                    "ProjectNumber" | "DrawingNo" | "DrawingID" | "Flag" => {}
                    _ => {
                        extra.entry(attr.name.clone()).or_insert(value);
                    }
                }
            }
        }
        (item_type, drawing_item_type, model_id, extra)
    };

    let mut graph = ObjectGraph {
        project_number: project_number_global,
        drawing_no: drawing_no_global,
        ..ObjectGraph::default()
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    // Build field_x / record_id → drawing_id maps straight from trailers.
    let field_x_to_drawing: BTreeMap<u32, String> = da
        .record_trailers
        .iter()
        .filter(|t| t.class_id != 0x0000_00F6)
        .filter_map(|t| t.drawing_id.as_ref().map(|d| (t.field_x, d.clone())))
        .collect();
    let record_id_to_drawing: BTreeMap<u32, String> = da
        .record_trailers
        .iter()
        .filter(|t| t.class_id != 0x0000_00F6)
        .filter_map(|t| t.drawing_id.as_ref().map(|d| (t.record_id, d.clone())))
        .collect();

    // Iterate trailers as the authoritative record list — their ordering
    // matches the raw stream and every trailer has a verified identity
    // (record_id, field_x, class_id).
    let mut seen_objects: std::collections::HashSet<String> = Default::default();
    for t in &da.record_trailers {
        if t.class_id == 0x0000_00F6 {
            // Relationships are identified by `class_id == 0xF6`. The
            // `Relationship.<GUID>` ASCII tag is the canonical identity but
            // a small number of trailers (e.g. "typical" template
            // relationships) don't carry the tag in their record body. We
            // still include them so `graph.relationships` stays in 1:1
            // correspondence with the relationship probe output; the guid
            // is left empty when unknown.
            let guid = t.relationship_guid.clone().unwrap_or_default();
            let model_id = if guid.is_empty() {
                "Relationship".to_string()
            } else {
                format!("Relationship.{guid}")
            };
            *counts.entry("Relationship".to_string()).or_default() += 1;
            graph.relationships.push(PidRelationship {
                model_id,
                guid,
                record_id: Some(t.record_id),
                field_x: Some(t.field_x),
                source_drawing_id: None,
                target_drawing_id: None,
            });
            continue;
        }
        let Some(did) = t.drawing_id.clone() else {
            continue;
        };
        if !seen_objects.insert(did.clone()) {
            continue;
        }
        let (item_type, drawing_item_type, model_id, extra) = enrich(&did);
        let Some(ty) = item_type else { continue };
        *counts.entry(ty.clone()).or_default() += 1;
        graph.by_drawing_id.insert(did.clone(), graph.objects.len());
        graph.objects.push(PidObject {
            drawing_id: did,
            item_type: ty,
            drawing_item_type,
            model_id,
            extra,
            record_id: Some(t.record_id),
            field_x: Some(t.field_x),
        });
    }

    // Resolve relationship endpoints via Sheet endpoint records. Each
    // `SheetEndpointRecord` pairs a relationship's `field_x` with two
    // endpoint `field_x`s, which we then translate to `drawing_id`s. If a
    // Sheet stream is missing or an endpoint has no mapping (e.g. it lives
    // in another drawing), we leave that side as `None`.
    let rel_endpoints: BTreeMap<u32, (u32, u32)> = doc
        .sheet_streams
        .iter()
        .flat_map(|s| s.endpoint_records.iter())
        .map(|r| (r.rel_field_x, (r.endpoint_a, r.endpoint_b)))
        .collect();

    let resolve = |fx: u32| -> Option<String> {
        field_x_to_drawing
            .get(&fx)
            .cloned()
            .or_else(|| record_id_to_drawing.get(&fx).cloned())
    };
    for rel in &mut graph.relationships {
        let Some(fx) = rel.field_x else { continue };
        let Some(&(a, b)) = rel_endpoints.get(&fx) else {
            continue;
        };
        rel.source_drawing_id = resolve(a);
        rel.target_drawing_id = resolve(b);
    }

    graph.counts_by_type = counts;
    if !graph.objects.is_empty() || !graph.relationships.is_empty() {
        doc.object_graph = Some(graph);
    }
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
                    if let crate::model::AttributeValue::Text(v) = &attr.value {
                        if inv.project.is_none() && !v.is_empty() {
                            inv.project = Some(v.clone());
                        }
                    }
                }
                "DrawingNo" => {
                    if let crate::model::AttributeValue::Text(v) = &attr.value {
                        if inv.drawing_id.is_none() && !v.is_empty() {
                            inv.drawing_id = Some(v.clone());
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

/// Single walk of the CFB directory that produces both the flat
/// [`StreamEntry`] index (preview + magic) and the raw-byte map used by
/// the writer layer. Returning both from one pass avoids re-reading every
/// stream just to keep the bytes around.
fn collect_streams_and_bytes<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    options: &ParseOptions,
) -> Result<(Vec<StreamEntry>, BTreeMap<String, RawStream>), PidError> {
    let paths: Vec<_> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .collect();

    let mut entries = Vec::with_capacity(paths.len());
    let mut raw_map: BTreeMap<String, RawStream> = BTreeMap::new();
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

        entries.push(StreamEntry {
            path: path_str.clone(),
            size: data.len() as u64,
            preview_ascii,
            magic_u32_le,
        });
        raw_map.insert(
            path_str.clone(),
            RawStream {
                path: path_str,
                data,
                modified: false,
            },
        );
    }

    Ok((entries, raw_map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        SheetCoordinateHintDto, SheetEndpointRecord, SheetGeometry, SheetStream, SheetText,
    };

    #[test]
    fn sync_sheet_geometry_endpoints_copies_endpoint_records() {
        let mut sheet = SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 0,
            extracted_texts: vec![],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: vec![],
            probe_summary: None,
            geometry: Some(SheetGeometry {
                texts: vec![SheetText {
                    offset: 8,
                    encoding: "utf16_le".into(),
                    text: "PUMP-101".into(),
                    byte_len: 16,
                }],
                endpoints: vec![],
                coordinate_hints: vec![SheetCoordinateHintDto {
                    offset: 32,
                    x: 1200,
                    y: -450,
                }],
                object_geometry_hints: vec![],
            }),
            endpoint_records: vec![SheetEndpointRecord {
                sheet_path: "/Sheet6".into(),
                offset: 0x40,
                rel_field_x: 100,
                endpoint_a: 42,
                endpoint_b: 77,
            }],
            endpoint_decode_error: None,
        };

        sync_sheet_geometry_endpoints(&mut sheet);

        let geometry = sheet.geometry.as_ref().expect("geometry");
        assert_eq!(geometry.texts.len(), 1);
        assert_eq!(geometry.texts[0].text, "PUMP-101");
        assert_eq!(geometry.coordinate_hints.len(), 1);
        assert_eq!(geometry.coordinate_hints[0].x, 1200);
        assert_eq!(geometry.endpoints.len(), 1);
        assert_eq!(geometry.endpoints[0].offset, 0x40);
        assert_eq!(geometry.endpoints[0].rel_field_x, 100);
        assert_eq!(geometry.endpoints[0].endpoint_a, 42);
        assert_eq!(geometry.endpoints[0].endpoint_b, 77);
    }
}
