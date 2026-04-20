use crate::model::{
    ObjectGraph, PidDocument, PidLayoutItem, PidLayoutModel, PidLayoutSegment, PidLayoutText,
    PidLayoutUnplaced, PidObject, PidRelationship,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

const COMPONENT_Y_SPACING: f64 = 320.0;
const LANE_X_SPACING: f64 = 220.0;
const LANE_Y_SPACING: f64 = 110.0;
const TEXT_OFFSET_X: f64 = 46.0;
const TEXT_OFFSET_Y: f64 = 20.0;

pub fn derive_layout(doc: &mut PidDocument) {
    doc.layout = build_layout_model(doc);
}

pub fn build_layout_model(doc: &PidDocument) -> Option<PidLayoutModel> {
    let graph = doc.object_graph.as_ref()?;
    if graph.objects.is_empty() {
        return None;
    }

    let graphic_oid_by_drawing = representation_graphic_oids(graph);
    let primary_ids: BTreeSet<String> = graph
        .objects
        .iter()
        .filter(|object| is_primary_layout_object(object))
        .map(|object| object.drawing_id.clone())
        .collect();
    if primary_ids.is_empty() {
        return None;
    }

    let physical_edges = collect_physical_edges(graph, &primary_ids);
    let adjacency = build_adjacency(&physical_edges);
    let object_lookup: BTreeMap<&str, &PidObject> = graph
        .objects
        .iter()
        .map(|object| (object.drawing_id.as_str(), object))
        .collect();

    let mut visited = BTreeSet::new();
    let mut positions: BTreeMap<String, [f64; 2]> = BTreeMap::new();
    let mut component_index = 0usize;

    for root in sorted_primary_roots(graph, &primary_ids, &adjacency) {
        if !visited.insert(root.clone()) {
            continue;
        }
        let component = collect_component(&root, &adjacency, &primary_ids, &mut visited);
        let placement = layout_component(&component, &adjacency, &object_lookup, component_index);
        positions.extend(placement);
        component_index += 1;
    }

    let mut layout = PidLayoutModel::default();
    let title = doc
        .drawing_meta
        .as_ref()
        .and_then(|meta| meta.drawing_number.clone())
        .or_else(|| doc.summary.as_ref().and_then(|summary| summary.title.clone()))
        .unwrap_or_else(|| "Smart P&ID Import".to_string());
    layout.texts.push(PidLayoutText {
        layout_id: "title".into(),
        drawing_id: None,
        text: title,
        anchor: [0.0, 180.0],
        bounds: Some([-20.0, 170.0, 420.0, 210.0]),
    });

    for object in &graph.objects {
        let label = choose_object_label(object);
        if let Some(anchor) = positions.get(&object.drawing_id) {
            let graphic_oid = graphic_oid_by_drawing.get(&object.drawing_id).copied();
            let (symbol_name, symbol_path) = infer_symbol_identity(object);
            let bounds = Some(bounds_for_item(anchor, object.item_type.as_str(), symbol_name.as_deref()));
            layout.items.push(PidLayoutItem {
                layout_id: format!("item:{}", object.drawing_id),
                drawing_id: Some(object.drawing_id.clone()),
                graphic_oid,
                kind: object.item_type.clone(),
                anchor: *anchor,
                bounds,
                symbol_name,
                symbol_path,
                label: Some(label.clone()),
                model_id: object.model_id.clone(),
            });
            layout.texts.push(PidLayoutText {
                layout_id: format!("text:{}", object.drawing_id),
                drawing_id: Some(object.drawing_id.clone()),
                text: label,
                anchor: [anchor[0] + TEXT_OFFSET_X, anchor[1] + TEXT_OFFSET_Y],
                bounds: Some([
                    anchor[0] + 12.0,
                    anchor[1] - 10.0,
                    anchor[0] + 156.0,
                    anchor[1] + 38.0,
                ]),
            });
        } else {
            layout.unplaced.push(PidLayoutUnplaced {
                drawing_id: Some(object.drawing_id.clone()),
                kind: object.item_type.clone(),
                label,
            });
        }
    }

    for (index, relation) in physical_edges.iter().enumerate() {
        let Some(source_id) = relation.source_drawing_id.as_ref() else {
            continue;
        };
        let Some(target_id) = relation.target_drawing_id.as_ref() else {
            continue;
        };
        let (Some(source), Some(target)) = (positions.get(source_id), positions.get(target_id)) else {
            continue;
        };
        layout.segments.push(PidLayoutSegment {
            layout_id: format!("segment:{}:{index}", relation.guid),
            owner_drawing_id: Some(source_id.clone()),
            graphic_oid: graphic_oid_by_drawing
                .get(source_id)
                .copied()
                .or_else(|| graphic_oid_by_drawing.get(target_id).copied()),
            start: *source,
            end: *target,
            role: relationship_role(relation).to_string(),
        });
    }

    if layout.segments.is_empty() {
        layout
            .warnings
            .push("layout derived without physical relationship segments".into());
    }
    if !layout.unplaced.is_empty() {
        layout.warnings.push(format!(
            "{} object(s) kept in fallback rail",
            layout.unplaced.len()
        ));
    }

    Some(layout)
}

fn representation_graphic_oids(graph: &ObjectGraph) -> BTreeMap<String, u32> {
    let mut representation_oids = BTreeMap::new();
    for object in &graph.objects {
        if object.item_type != "PIDRepresentation" {
            continue;
        }
        let Some(value) = object
            .extra
            .get("IDrawingRepresentation.GraphicOID")
            .or_else(|| object.extra.get("IRepresentation.GraphicOID"))
        else {
            continue;
        };
        let Some(graphic_oid) = parse_u32(value) else {
            continue;
        };
        representation_oids.insert(object.drawing_id.clone(), graphic_oid);
    }

    let mut out = BTreeMap::new();
    for relationship in &graph.relationships {
        if relationship_role(relationship) != "DwgRepresentationComposition" {
            continue;
        }
        let source = relationship.source_drawing_id.as_ref();
        let target = relationship.target_drawing_id.as_ref();
        match (
            source.and_then(|id| representation_oids.get(id).copied()),
            target.and_then(|id| representation_oids.get(id).copied()),
        ) {
            (Some(graphic_oid), _) => {
                if let Some(target_id) = target {
                    out.insert(target_id.clone(), graphic_oid);
                }
            }
            (_, Some(graphic_oid)) => {
                if let Some(source_id) = source {
                    out.insert(source_id.clone(), graphic_oid);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_u32(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    trimmed
        .parse::<u32>()
        .ok()
        .or_else(|| trimmed.strip_prefix("0x").and_then(|hex| u32::from_str_radix(hex, 16).ok()))
}

fn is_primary_layout_object(object: &PidObject) -> bool {
    !matches!(
        object.item_type.as_str(),
        "PIDDrawing" | "PIDRepresentation" | "DocumentVersion" | "DocumentRevision" | "File"
    )
}

fn collect_physical_edges(graph: &ObjectGraph, primary_ids: &BTreeSet<String>) -> Vec<PidRelationship> {
    graph.relationships
        .iter()
        .filter(|relationship| {
            let source = relationship.source_drawing_id.as_ref();
            let target = relationship.target_drawing_id.as_ref();
            let role = relationship_role(relationship);
            let primary_pair = source
                .zip(target)
                .map(|(a, b)| primary_ids.contains(a) && primary_ids.contains(b))
                .unwrap_or(false);
            if !primary_pair {
                return false;
            }
            role == "Relationship"
                || matches!(
                    role,
                    "PipingEnd1Conn"
                        | "PipingEnd2Conn"
                        | "PipingTapOrFitting"
                        | "ProcessPointCollection"
                )
        })
        .cloned()
        .collect()
}

fn build_adjacency(edges: &[PidRelationship]) -> BTreeMap<String, Vec<String>> {
    let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for relationship in edges {
        let (Some(source), Some(target)) = (
            relationship.source_drawing_id.as_ref(),
            relationship.target_drawing_id.as_ref(),
        ) else {
            continue;
        };
        adjacency.entry(source.clone()).or_default().push(target.clone());
        adjacency.entry(target.clone()).or_default().push(source.clone());
    }
    for neighbours in adjacency.values_mut() {
        neighbours.sort();
        neighbours.dedup();
    }
    adjacency
}

fn sorted_primary_roots(
    graph: &ObjectGraph,
    primary_ids: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut roots: Vec<String> = graph
        .objects
        .iter()
        .filter(|object| primary_ids.contains(&object.drawing_id))
        .map(|object| object.drawing_id.clone())
        .collect();
    roots.sort_by(|left, right| {
        let left_object = graph.by_drawing_id.get(left).and_then(|index| graph.objects.get(*index));
        let right_object = graph.by_drawing_id.get(right).and_then(|index| graph.objects.get(*index));
        object_priority(left_object)
            .cmp(&object_priority(right_object))
            .then_with(|| {
                adjacency
                    .get(left)
                    .map(|items| usize::MAX - items.len())
                    .unwrap_or(usize::MAX)
                    .cmp(
                        &adjacency
                            .get(right)
                            .map(|items| usize::MAX - items.len())
                            .unwrap_or(usize::MAX),
                    )
            })
            .then_with(|| left.cmp(right))
    });
    roots
}

fn collect_component(
    root: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    primary_ids: &BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Vec<String> {
    let mut queue = VecDeque::from([root.to_string()]);
    let mut component = Vec::new();
    while let Some(current) = queue.pop_front() {
        component.push(current.clone());
        if let Some(neighbours) = adjacency.get(&current) {
            for neighbour in neighbours {
                if !primary_ids.contains(neighbour) || !visited.insert(neighbour.clone()) {
                    continue;
                }
                queue.push_back(neighbour.clone());
            }
        }
    }
    component
}

fn layout_component(
    component: &[String],
    adjacency: &BTreeMap<String, Vec<String>>,
    object_lookup: &BTreeMap<&str, &PidObject>,
    component_index: usize,
) -> BTreeMap<String, [f64; 2]> {
    let mut out = BTreeMap::new();
    if component.is_empty() {
        return out;
    }

    let root = component
        .iter()
        .min_by(|left, right| {
            object_priority(object_lookup.get(left.as_str()).copied())
                .cmp(&object_priority(object_lookup.get(right.as_str()).copied()))
                .then_with(|| left.cmp(right))
        })
        .cloned()
        .unwrap_or_default();

    let component_set: BTreeSet<&str> = component.iter().map(String::as_str).collect();
    let mut queue = VecDeque::from([(root.clone(), 0usize)]);
    let mut seen = BTreeSet::from([root.clone()]);
    let mut levels: BTreeMap<usize, Vec<String>> = BTreeMap::new();

    while let Some((current, level)) = queue.pop_front() {
        levels.entry(level).or_default().push(current.clone());
        if let Some(neighbours) = adjacency.get(&current) {
            for neighbour in neighbours {
                if !component_set.contains(neighbour.as_str()) || !seen.insert(neighbour.clone()) {
                    continue;
                }
                queue.push_back((neighbour.clone(), level + 1));
            }
        }
    }

    let base_y = -(component_index as f64) * COMPONENT_Y_SPACING;
    for (level, nodes) in levels {
        let mut sorted_nodes = nodes;
        sorted_nodes.sort_by(|left, right| {
            object_priority(object_lookup.get(left.as_str()).copied())
                .cmp(&object_priority(object_lookup.get(right.as_str()).copied()))
                .then_with(|| left.cmp(right))
        });
        let lane_center = (sorted_nodes.len() as f64 - 1.0) / 2.0;
        for (index, drawing_id) in sorted_nodes.iter().enumerate() {
            let x = level as f64 * LANE_X_SPACING;
            let y = base_y + (lane_center - index as f64) * LANE_Y_SPACING;
            out.insert(drawing_id.clone(), [x, y]);
        }
    }
    out
}

fn object_priority(object: Option<&PidObject>) -> usize {
    let Some(object) = object else {
        return usize::MAX;
    };
    match object.item_type.as_str() {
        "PIDPipeline" | "PipeRun" => 0,
        "PIDPipingConnector" | "PIDNozzle" | "Nozzle" | "PIDPipingPort" | "PIDSignalPort" => 1,
        "PIDPipingBranchPoint" | "PIDBranchPoint" | "PIDProcessPoint" => 2,
        "PIDSignalConnector" | "OPC" | "Instrument" | "PIDInstrument" | "PIDControlSystemFunction" => 3,
        "PIDProcessVessel" | "Equipment" | "PIDEquipment" => 4,
        _ => 5,
    }
}

fn relationship_role(relationship: &PidRelationship) -> &str {
    let Some(rest) = relationship.model_id.strip_prefix("Relationship.") else {
        return "Relationship";
    };
    let mut parts = rest.split('.');
    let Some(first) = parts.next() else {
        return "Relationship";
    };
    let Some(second) = parts.next() else {
        return "Relationship";
    };
    if second.len() == 32 && second.chars().all(|ch| ch.is_ascii_hexdigit()) {
        first
    } else {
        "Relationship"
    }
}

fn choose_object_label(object: &PidObject) -> String {
    let keys = [
        "Tag",
        "PipelineName",
        "ItemTag",
        "Name",
        "IObject.Name",
        "DocTitle",
        "Text",
        "DisplayedText",
    ];
    for key in keys {
        if let Some(value) = object.extra.get(key).filter(|value| !value.trim().is_empty()) {
            return value.clone();
        }
    }
    if let Some(model_id) = object.model_id.as_ref().filter(|value| !value.trim().is_empty()) {
        return model_id.clone();
    }
    object.item_type.clone()
}

fn infer_symbol_identity(object: &PidObject) -> (Option<String>, Option<String>) {
    let symbol_path = object
        .extra
        .values()
        .find_map(|value| extract_symbol_path(value));
    let symbol_name = symbol_name_for_type(object.item_type.as_str()).or_else(|| {
        symbol_path.as_ref().and_then(|path| {
            Path::new(path)
                .file_stem()
                .map(|name| name.to_string_lossy().to_string())
        })
    });
    (symbol_name, symbol_path)
}

fn symbol_name_for_type(kind: &str) -> Option<String> {
    let symbol_name = match kind {
        "PIDPipingBranchPoint" | "PIDBranchPoint" => "Branch",
        "PIDPipingConnector" => "Connector",
        "PIDPipeline" | "PipeRun" => "Pipeline",
        "PIDProcessPoint" => "ProcessPoint",
        "PIDPipingPort" => "PipingPort",
        "PIDSignalPort" => "SignalPort",
        "PIDSignalConnector" | "OPC" => "OffPageConnector",
        "PIDNote" | "ItemNote" => "Note",
        "PIDNozzle" | "Nozzle" => "Nozzle",
        "PIDPipingComponent" | "PipingComp" => "PipingComponent",
        "PIDControlSystemFunction" | "Instrument" | "PIDInstrument" => "Instrument",
        "PIDProcessVessel" => "Vessel",
        "Equipment" | "PIDEquipment" => "Equipment",
        _ => return None,
    };
    Some(symbol_name.to_string())
}

fn extract_symbol_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.to_ascii_lowercase().ends_with(".sym") {
        return None;
    }
    Some(trimmed.replace('/', "\\"))
}

fn bounds_for_item(anchor: &[f64; 2], kind: &str, symbol_name: Option<&str>) -> [f64; 4] {
    let semantic = symbol_name.unwrap_or(kind);
    let (half_w, half_h) = match semantic {
        "Pipeline" | "PIDPipeline" | "PipeRun" => (50.0, 8.0),
        "Connector" | "PIDPipingConnector" => (18.0, 18.0),
        "Branch" | "PIDPipingBranchPoint" | "PIDBranchPoint" => (16.0, 16.0),
        "ProcessPoint" | "PIDProcessPoint" => (14.0, 14.0),
        "Note" | "PIDNote" | "ItemNote" => (34.0, 22.0),
        "Nozzle" | "PIDNozzle" | "PipingPort" | "SignalPort" => (18.0, 16.0),
        "OffPageConnector" | "PIDSignalConnector" | "OPC" => (20.0, 18.0),
        "PipingComponent" | "PIDPipingComponent" | "PipingComp" => (18.0, 18.0),
        "Instrument" | "PIDControlSystemFunction" | "PIDInstrument" => (20.0, 20.0),
        "Vessel" | "PIDProcessVessel" => (30.0, 22.0),
        "Equipment" | "PIDEquipment" => (26.0, 18.0),
        _ => (24.0, 16.0),
    };
    [
        anchor[0] - half_w,
        anchor[1] - half_h,
        anchor[0] + half_w,
        anchor[1] + half_h,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ObjectGraph, PidDocument, PidObject, PidRelationship};
    use std::collections::BTreeMap;

    #[test]
    fn build_layout_model_places_connected_objects_and_falls_back_auxiliary_records() {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("DWG-TEST".into()),
            project_number: Some("P-01".into()),
            objects: vec![
                PidObject {
                    drawing_id: "PIPE".into(),
                    item_type: "PIDPipeline".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("LINE-001".into()),
                    extra: BTreeMap::from([("PipelineName".into(), "LINE-001".into())]),
                    record_id: Some(1),
                    field_x: Some(11),
                },
                PidObject {
                    drawing_id: "BRANCH".into(),
                    item_type: "PIDBranchPoint".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("BRAN-1".into()),
                    extra: BTreeMap::new(),
                    record_id: Some(2),
                    field_x: Some(12),
                },
                PidObject {
                    drawing_id: "INST".into(),
                    item_type: "Instrument".into(),
                    drawing_item_type: Some("Symbol".into()),
                    model_id: Some("FIT-001".into()),
                    extra: BTreeMap::from([("Tag".into(), "FIT-001".into())]),
                    record_id: Some(3),
                    field_x: Some(13),
                },
                PidObject {
                    drawing_id: "REP".into(),
                    item_type: "PIDRepresentation".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("REP-1".into()),
                    extra: BTreeMap::from([(
                        "IDrawingRepresentation.GraphicOID".into(),
                        "582".into(),
                    )]),
                    record_id: None,
                    field_x: None,
                },
                PidObject {
                    drawing_id: "DRAW".into(),
                    item_type: "PIDDrawing".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("DWG-TEST".into()),
                    extra: BTreeMap::new(),
                    record_id: None,
                    field_x: None,
                },
            ],
            relationships: vec![
                PidRelationship {
                    model_id: "Relationship.ProcessPointCollection.00000000000000000000000000000001"
                        .into(),
                    guid: "00000000000000000000000000000001".into(),
                    record_id: Some(10),
                    field_x: Some(20),
                    source_drawing_id: Some("BRANCH".into()),
                    target_drawing_id: Some("PIPE".into()),
                },
                PidRelationship {
                    model_id: "Relationship.PipingEnd1Conn.00000000000000000000000000000002"
                        .into(),
                    guid: "00000000000000000000000000000002".into(),
                    record_id: Some(11),
                    field_x: Some(21),
                    source_drawing_id: Some("INST".into()),
                    target_drawing_id: Some("BRANCH".into()),
                },
                PidRelationship {
                    model_id:
                        "Relationship.DwgRepresentationComposition.00000000000000000000000000000003"
                            .into(),
                    guid: "00000000000000000000000000000003".into(),
                    record_id: None,
                    field_x: None,
                    source_drawing_id: Some("REP".into()),
                    target_drawing_id: Some("BRANCH".into()),
                },
            ],
            by_drawing_id: BTreeMap::from([
                ("PIPE".into(), 0),
                ("BRANCH".into(), 1),
                ("INST".into(), 2),
                ("REP".into(), 3),
                ("DRAW".into(), 4),
            ]),
            counts_by_type: BTreeMap::new(),
        });

        let layout = build_layout_model(&doc).expect("layout should be built");
        assert_eq!(layout.items.len(), 3, "only primary objects should be placed");
        assert_eq!(layout.segments.len(), 2, "two physical edges should render as segments");
        assert!(layout.texts.iter().any(|text| text.text == "LINE-001"));
        assert!(layout.texts.iter().any(|text| text.text == "FIT-001"));
        assert!(
            layout
                .items
                .iter()
                .find(|item| item.drawing_id.as_deref() == Some("BRANCH"))
                .and_then(|item| item.graphic_oid)
                == Some(582),
            "representation graphic oid should be transferred onto the represented object"
        );
        assert_eq!(layout.unplaced.len(), 2, "drawing + representation stay in fallback");
    }

    #[test]
    fn derive_layout_stores_layout_on_document() {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: None,
            project_number: None,
            objects: vec![PidObject {
                drawing_id: "PIPE".into(),
                item_type: "PipeRun".into(),
                drawing_item_type: None,
                model_id: Some("PIPE-01".into()),
                extra: BTreeMap::new(),
                record_id: Some(1),
                field_x: Some(1),
            }],
            relationships: vec![],
            by_drawing_id: BTreeMap::from([("PIPE".into(), 0)]),
            counts_by_type: BTreeMap::new(),
        });

        derive_layout(&mut doc);
        assert!(doc.layout.is_some(), "derive_layout should populate doc.layout");
        assert_eq!(doc.layout.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn build_layout_model_classifies_bundle_specific_symbol_kinds() {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("DWG-BUNDLE".into()),
            project_number: None,
            objects: vec![
                PidObject {
                    drawing_id: "NOTE".into(),
                    item_type: "PIDNote".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("NOTE-1".into()),
                    extra: BTreeMap::from([("Text".into(), "Check valve branch".into())]),
                    record_id: Some(1),
                    field_x: Some(1),
                },
                PidObject {
                    drawing_id: "NOZZLE".into(),
                    item_type: "PIDNozzle".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("NZ-1".into()),
                    extra: BTreeMap::new(),
                    record_id: Some(2),
                    field_x: Some(2),
                },
                PidObject {
                    drawing_id: "OFFPAGE".into(),
                    item_type: "PIDSignalConnector".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("OPC-1".into()),
                    extra: BTreeMap::new(),
                    record_id: Some(3),
                    field_x: Some(3),
                },
                PidObject {
                    drawing_id: "VESSEL".into(),
                    item_type: "PIDProcessVessel".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("V-100".into()),
                    extra: BTreeMap::new(),
                    record_id: Some(4),
                    field_x: Some(4),
                },
            ],
            relationships: vec![],
            by_drawing_id: BTreeMap::from([
                ("NOTE".into(), 0),
                ("NOZZLE".into(), 1),
                ("OFFPAGE".into(), 2),
                ("VESSEL".into(), 3),
            ]),
            counts_by_type: BTreeMap::new(),
        });

        let layout = build_layout_model(&doc).expect("bundle-like graph should build layout");
        let mut symbol_names = BTreeMap::new();
        for item in layout.items {
            symbol_names.insert(item.drawing_id.unwrap_or_default(), item.symbol_name);
        }

        assert_eq!(
            symbol_names.get("NOTE").cloned().flatten().as_deref(),
            Some("Note"),
            "bundle PIDNote objects should retain a note semantic"
        );
        assert_eq!(
            symbol_names.get("NOZZLE").cloned().flatten().as_deref(),
            Some("Nozzle"),
            "bundle PIDNozzle objects should retain a nozzle semantic"
        );
        assert_eq!(
            symbol_names.get("OFFPAGE").cloned().flatten().as_deref(),
            Some("OffPageConnector"),
            "bundle signal connectors should retain an off-page semantic"
        );
        assert_eq!(
            symbol_names.get("VESSEL").cloned().flatten().as_deref(),
            Some("Vessel"),
            "bundle PIDProcessVessel objects should retain a vessel semantic"
        );
    }
}
