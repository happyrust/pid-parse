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
use crate::model::{
    EmbeddedSymbolDefinition, LayerDisplayOverride, PidDocument, SheetLayer, ViewFilterSet,
    ViewFilterSetLayer,
};
use crate::parsers::cluster_header::decode_psm_cluster0_body_records;
use crate::parsers::psm_tables;
use crate::parsers::sheet_layers::{
    decode_sheet_layer_managers, decode_sheet_layers, PSM_TYPE_CODE_JSHEET_LAYER,
    PSM_TYPE_CODE_JSHEET_LAYER_MANAGER,
};
use crate::parsers::view_filter_sets::{decode_view_filter_sets, ViewFilterSetDecoded};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

/// PSM type code for `JSheet Object`, the sheet a view filter set belongs
/// to and a layer manager registers.
const PSM_TYPE_CODE_JSHEET: u16 = 0x0114;

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
    parse_sheet_layers(cfb, doc);
    link_embedded_definitions(doc);
    Ok(())
}

/// Group each definition cache's records into symbol bodies.
///
/// A body is a `JSheet` of the cache storage; the sheet's space-map entry
/// carries a tag-183 member naming its `JSheetLayerManager`, and the layers
/// that manager governs (already reconciled on [`SheetLayer::manager_oid`])
/// are the layers the body's records sit on. Runs after the space maps and
/// the layer tables are read, and fills
/// [`crate::model::JSiteNestedGeometry::definitions`] on every site that
/// holds nested geometry. A sheet whose entry names no known manager -- the
/// storage's own base sheet does not -- is left out rather than guessed at.
fn link_embedded_definitions(doc: &mut PidDocument) {
    // (storage, sheet oid) -> the manager oids its tag-183 members name.
    let mut sheet_managers: BTreeMap<(String, u32), Vec<u32>> = BTreeMap::new();
    for (map_path, map) in &doc.psm_space_maps {
        let Some(segment) = space_map_segment(map_path) else {
            continue;
        };
        let storage = space_map_storage_path(map_path);
        let Some(layers) = doc.sheet_layers.get(&storage) else {
            continue;
        };
        let managers: BTreeSet<u32> = layers
            .iter()
            .filter_map(|layer| layer.manager_oid)
            .collect();
        for entry in &map.entries {
            let target = (segment << 13) | u32::from(entry.index);
            let named: Vec<u32> = entry
                .live_members()
                .iter()
                .filter(|member| member.tag == 183 && managers.contains(&member.value))
                .map(|member| member.value)
                .collect();
            if !named.is_empty() {
                sheet_managers
                    .entry((storage.clone(), target))
                    .or_default()
                    .extend(named);
            }
        }
    }

    for site in &mut doc.jsites {
        let Some(nested) = site.nested_geometry.as_mut() else {
            continue;
        };
        let Some(layers) = doc.sheet_layers.get(&site.path) else {
            continue;
        };
        let mut definitions = Vec::new();
        for &sheet_oid in &nested.sheets {
            let Some(managers) = sheet_managers.get(&(site.path.clone(), sheet_oid)) else {
                continue;
            };
            // One manager per sheet is the shape every cache on the corpus
            // has; two would mean the reading is wrong, so refuse it.
            let [manager_oid] = managers.as_slice() else {
                continue;
            };
            let mut governed: Vec<u32> = layers
                .iter()
                .filter(|layer| layer.manager_oid == Some(*manager_oid))
                .map(|layer| layer.oid)
                .collect();
            governed.sort_unstable();
            definitions.push(EmbeddedSymbolDefinition {
                sheet_oid,
                manager_oid: *manager_oid,
                layers: governed,
            });
        }
        nested.definitions = definitions;
    }
}

fn storage_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parent = normalized.rsplit_once('/').map_or("", |(parent, _)| parent);
    if parent.is_empty() {
        "/".to_string()
    } else {
        parent.to_string()
    }
}

fn space_map_storage_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let marker = "/PSMspacemap/";
    normalized.find(marker).map_or_else(
        || storage_path(&normalized),
        |at| {
            let prefix = &normalized[..at];
            if prefix.is_empty() {
                "/".to_string()
            } else {
                prefix.to_string()
            }
        },
    )
}

fn space_map_segment(path: &str) -> Option<u32> {
    let leaf = path.rsplit(['/', '\\']).next()?;
    u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
        .ok()
        .map(|address| address >> 13)
}

/// Decode every storage's `JSheetLayer` table and reconcile tag-183 manager
/// registrations from that storage's `PSMspacemap` index.
fn parse_sheet_layers<R: Read + std::io::Seek>(
    cfb: &mut ::cfb::CompoundFile<R>,
    doc: &mut PidDocument,
) {
    let paths: Vec<String> = cfb
        .walk()
        .filter(::cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .filter(|path| path.rsplit('/').next() == Some("PSMcluster0"))
        .collect();

    let mut families: BTreeMap<String, BTreeMap<u32, u16>> = BTreeMap::new();
    let mut decoded_by_storage: BTreeMap<
        String,
        Vec<(String, crate::parsers::sheet_layers::SheetLayerDecoded)>,
    > = BTreeMap::new();
    let mut sets_by_storage: BTreeMap<String, Vec<(String, ViewFilterSetDecoded)>> =
        BTreeMap::new();
    for path in paths {
        let Ok(mut stream) = cfb.open_stream(&path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let storage = storage_path(&path);
        let by_oid = families.entry(storage.clone()).or_default();
        for manager in decode_sheet_layer_managers(&data) {
            by_oid.insert(manager.oid, PSM_TYPE_CODE_JSHEET_LAYER_MANAGER);
        }
        let layers = decode_sheet_layers(&data);
        for layer in &layers {
            by_oid.insert(layer.oid, PSM_TYPE_CODE_JSHEET_LAYER);
        }
        // The sheets are only identities here: what a set belongs to and
        // what a manager registers.
        for record in decode_psm_cluster0_body_records(&data) {
            if record.type_code == PSM_TYPE_CODE_JSHEET {
                if let Some(oid) = record.raw_payload.get(0..4) {
                    by_oid.insert(
                        u32::from_le_bytes(oid.try_into().unwrap_or([0; 4])),
                        PSM_TYPE_CODE_JSHEET,
                    );
                }
            }
        }
        decoded_by_storage
            .entry(storage.clone())
            .or_default()
            .extend(layers.into_iter().map(|layer| (path.clone(), layer)));
        sets_by_storage.entry(storage).or_default().extend(
            decode_view_filter_sets(&data)
                .into_iter()
                .map(|set| (path.clone(), set)),
        );
    }

    // Tag 183, "this manager registers me", read off the space map twice
    // over: onto every layer (its manager) and onto every sheet (the
    // manager serving it). The second is what ties a view filter set, which
    // names its sheet, to the layer objects it governs.
    let mut registrations: BTreeMap<(String, u32), Vec<u32>> = BTreeMap::new();
    let mut sheet_managers: BTreeMap<(String, u32), BTreeSet<u32>> = BTreeMap::new();
    for (map_path, map) in &doc.psm_space_maps {
        let Some(segment) = space_map_segment(map_path) else {
            continue;
        };
        let storage = space_map_storage_path(map_path);
        let Some(by_oid) = families.get(&storage) else {
            continue;
        };
        for entry in &map.entries {
            let target = (segment << 13) | u32::from(entry.index);
            let target_family = by_oid.get(&target).copied();
            if !matches!(
                target_family,
                Some(PSM_TYPE_CODE_JSHEET_LAYER | PSM_TYPE_CODE_JSHEET)
            ) {
                continue;
            }
            for member in entry
                .live_members()
                .iter()
                .filter(|member| member.tag == 183)
            {
                if by_oid.get(&member.value) != Some(&PSM_TYPE_CODE_JSHEET_LAYER_MANAGER) {
                    continue;
                }
                if target_family == Some(PSM_TYPE_CODE_JSHEET_LAYER) {
                    registrations
                        .entry((storage.clone(), target))
                        .or_default()
                        .push(member.value);
                } else {
                    sheet_managers
                        .entry((storage.clone(), target))
                        .or_default()
                        .insert(member.value);
                }
            }
        }
    }

    for (storage, decoded) in decoded_by_storage {
        let mut layers: Vec<SheetLayer> = decoded
            .into_iter()
            .map(|(stream_path, layer)| {
                let registrations = registrations.get(&(storage.clone(), layer.oid));
                let managers: BTreeSet<u32> =
                    registrations.into_iter().flatten().copied().collect();
                SheetLayer {
                    storage_path: storage.clone(),
                    stream_path,
                    oid: layer.oid,
                    parent_ref: layer.parent_ref,
                    name: layer.name,
                    secondary_name: layer.secondary_name,
                    object_count: layer.object_count,
                    layer_number: layer.layer_number,
                    manager_oid: (managers.len() == 1)
                        .then(|| managers.iter().next().copied())
                        .flatten(),
                    manager_registration_count: u32::try_from(registrations.map_or(0, Vec::len))
                        .unwrap_or(u32::MAX),
                    displayed: None,
                    locatable: None,
                    view_filter_set_oid: None,
                }
            })
            .collect();
        layers.sort_by_key(|layer| layer.oid);

        // Resolve each set's `(name, number)` entries to the layer objects
        // registered with the manager(s) of the set's sheet, and write the
        // set's answer onto them.
        let sets = sets_by_storage.remove(&storage).unwrap_or_default();
        let mut resolved_sets = Vec::with_capacity(sets.len());
        for (stream_path, set) in sets {
            let managers = sheet_managers
                .get(&(storage.clone(), set.sheet_ref))
                .cloned()
                .unwrap_or_default();
            let entries = set
                .layers
                .iter()
                .map(|entry| {
                    let displayed = set.is_displayed(entry.layer_number);
                    let locatable = set.is_locatable(entry.layer_number);
                    let candidates: Vec<usize> = layers
                        .iter()
                        .enumerate()
                        .filter(|(_, layer)| {
                            layer.name == entry.name
                                && layer.layer_number == u32::from(entry.layer_number)
                                && layer.manager_oid.is_some_and(|m| managers.contains(&m))
                        })
                        .map(|(index, _)| index)
                        .collect();
                    let layer_oid = match candidates.as_slice() {
                        [index] => {
                            let layer = &mut layers[*index];
                            layer.displayed = displayed;
                            layer.locatable = locatable;
                            layer.view_filter_set_oid = Some(set.oid);
                            Some(layer.oid)
                        }
                        _ => None,
                    };
                    ViewFilterSetLayer {
                        name: entry.name.clone(),
                        layer_number: entry.layer_number,
                        displayed,
                        locatable,
                        layer_oid,
                    }
                })
                .collect();
            resolved_sets.push(ViewFilterSet {
                storage_path: storage.clone(),
                stream_path,
                oid: set.oid,
                sheet_ref: set.sheet_ref,
                active_layer_number: set.active_layer_number,
                layers: entries,
                overrides: set
                    .overrides
                    .iter()
                    .map(|o| LayerDisplayOverride {
                        layer_number: o.layer_number,
                        kind: o.kind,
                        colour: o.colour,
                        line_width: o.line_width,
                        trailing_word: o.trailing_word,
                    })
                    .collect(),
            });
        }
        if !resolved_sets.is_empty() {
            resolved_sets.sort_by_key(|set| set.oid);
            doc.view_filter_sets.insert(storage.clone(), resolved_sets);
        }
        doc.sheet_layers.insert(storage, layers);
    }
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
