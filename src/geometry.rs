//! Normalized drawing geometry projection for decoded `.pid` documents.
//!
//! This module is the contract between low-level `Sheet*` / PSM decoding
//! and renderers such as H7CAD. Coordinate hints are exposed as inferred
//! points because they carry source byte ranges, but they are still not
//! line / text / symbol geometry.

use crate::model::PidDocument;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const SHEET_ENDPOINT_RECORD_LEN: usize = 26;

/// Visualization-ready geometry extracted from a [`PidDocument`].
///
/// Unlike [`crate::model::PidLayoutModel`], this type is reserved for
/// source-backed `SmartPlant` geometry.  Topology-derived fallback drawings
/// should continue to use `PidLayoutModel` until a corresponding
/// [`PidGraphicEntity`] can point at byte / record provenance.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedPidGeometry {
    /// Source-backed graphic entities in drawing order where known.
    pub entities: Vec<PidGraphicEntity>,
    /// Non-fatal diagnostics explaining missing or skipped geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl NormalizedPidGeometry {
    /// True when no source-backed entities were produced.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// One source-backed graphical entity from a `SmartPlant` drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PidGraphicEntity {
    /// Stable renderer-facing identifier local to this geometry projection.
    pub id: String,
    /// Optional `DrawingID` of the semantic object that owns this graphic.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub drawing_id: Option<String>,
    /// Optional `GraphicOID` surfaced by `SmartPlant` representation records.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    /// Concrete geometry payload.
    pub kind: PidGraphicKind,
    /// Where this entity came from inside the `.pid` file.
    pub source: PidGraphicProvenance,
    /// How strongly the parser understands the entity payload.
    pub confidence: PidGeometryConfidence,
}

/// Geometry payload for a [`PidGraphicEntity`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PidGraphicKind {
    /// Straight segment between two model-space points.
    Line {
        /// Segment start point.
        start: PidPoint,
        /// Segment end point.
        end: PidPoint,
    },
    /// Ordered vertex chain, optionally closed.
    Polyline {
        /// Vertices in source order.
        points: Vec<PidPoint>,
        /// Whether the last point connects back to the first.
        closed: bool,
    },
    /// Circular arc in model space.
    Arc {
        /// Arc centre point.
        center: PidPoint,
        /// Radius in source drawing units.
        radius: f64,
        /// Start angle in radians.
        start_angle: f64,
        /// End angle in radians.
        end_angle: f64,
    },
    /// Full circle in model space.
    Circle {
        /// Circle centre point.
        center: PidPoint,
        /// Radius in source drawing units.
        radius: f64,
    },
    /// Coordinate pair whose surrounding record semantics are still inferred.
    Point {
        /// Point position in source drawing units.
        position: PidPoint,
    },
    /// Text-like annotation.
    Text {
        /// Text insertion point.
        insertion: PidPoint,
        /// Text payload as decoded by the source parser.
        value: String,
        /// Text height in source drawing units.
        height: f64,
        /// Rotation in radians.
        rotation: f64,
    },
    /// Instance of a reusable `SmartPlant` symbol.
    SymbolInstance {
        /// Symbol insertion point.
        insertion: PidPoint,
        /// Symbol-library path when the `JSite` layer exposed it.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        symbol_path: Option<String>,
        /// Rotation in radians.
        rotation: f64,
        /// X/Y scale factors.
        scale: [f64; 2],
    },
    /// Evidence was found, but the shape is not decoded enough to render.
    Unknown {
        /// Human-readable explanation for diagnostics.
        note: String,
    },
}

/// Two-dimensional point in source drawing units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PidPoint {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// Source location for a [`PidGraphicEntity`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PidGraphicProvenance {
    /// CFB stream path, such as `/Sheet6`, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stream_path: Option<String>,
    /// Byte range inside [`Self::stream_path`] when the parser can name it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub byte_range: Option<PidByteRange>,
    /// Parser-level record identifier, if the source structure has one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub record_id: Option<String>,
    /// Dynamic Attributes / Sheet `field_x` value associated with the entity.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub field_x: Option<u32>,
    /// Additional provenance note for diagnostics.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

/// Half-open byte range `[start, end)` inside a source stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PidByteRange {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
}

/// Parser confidence for a normalized geometry entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PidGeometryConfidence {
    /// Record layout and geometry semantics are decoded.
    Decoded,
    /// Entity is derived by cross-record inference but has strong provenance.
    Inferred,
    /// Entity is probe evidence only and should not be rendered by default.
    ProbeOnly,
}

/// Build the normalized source-backed geometry projection for `doc`.
///
/// Current behavior is intentionally conservative: Sheet coordinate pairs
/// become source-backed inferred points, while undecoded text and endpoint
/// evidence remains `ProbeOnly` until `Sheet*` and PSM record layouts can
/// name real render geometry with stronger provenance.
pub fn build_normalized_geometry(doc: &PidDocument) -> NormalizedPidGeometry {
    let mut warnings = Vec::new();
    let mut entities = Vec::new();
    let sheet_count = doc.sheet_streams.len();
    if sheet_count == 0 {
        warnings.push("no Sheet streams available for geometry decode".to_string());
    } else {
        warnings.push(format!(
            "geometry decode not yet implemented; {sheet_count} Sheet stream(s) available as probe input"
        ));
    }

    for sheet in &doc.sheet_streams {
        if let Some(geometry) = &sheet.geometry {
            for (index, text) in geometry.texts.iter().enumerate() {
                entities.push(PidGraphicEntity {
                    id: format!("{}:text-probe:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind: PidGraphicKind::Unknown {
                        note: format!("sheet text probe: {}", text.text),
                    },
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: source_range(text.offset, text.byte_len, sheet.size),
                        record_id: Some(format!("text-probe:{index}")),
                        field_x: None,
                        note: Some("text position is not decoded yet".to_string()),
                    },
                    confidence: PidGeometryConfidence::ProbeOnly,
                });
            }

            for (index, hint) in geometry.coordinate_hints.iter().enumerate() {
                entities.push(PidGraphicEntity {
                    id: format!("{}:coordinate-hint:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind: PidGraphicKind::Point {
                        position: PidPoint {
                            x: f64::from(hint.x),
                            y: f64::from(hint.y),
                        },
                    },
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: source_range(hint.offset, 8, sheet.size),
                        record_id: Some(format!("coordinate-hint:{index}")),
                        field_x: None,
                        note: Some(
                            "coordinate pair promoted as an inferred point; surrounding record semantics are not decoded yet"
                                .to_string(),
                        ),
                    },
                    confidence: PidGeometryConfidence::Inferred,
                });
            }

            for (index, hint) in geometry.object_geometry_hints.iter().enumerate() {
                if let Some(ref pos) = hint.position {
                    entities.push(PidGraphicEntity {
                        id: format!("{}:geometry-hint:{index}", sheet.path),
                        drawing_id: None,
                        graphic_oid: hint.graphic_oid,
                        kind: PidGraphicKind::Point {
                            position: PidPoint {
                                x: f64::from(pos.x),
                                y: f64::from(pos.y),
                            },
                        },
                        source: PidGraphicProvenance {
                            stream_path: Some(sheet.path.clone()),
                            byte_range: source_range(hint.offset, 8, sheet.size),
                            record_id: Some(format!("geometry-hint:{index}")),
                            field_x: Some(hint.field_x),
                            note: hint.note.clone(),
                        },
                        confidence: PidGeometryConfidence::Inferred,
                    });
                }
            }
        } else {
            for (index, text) in sheet.extracted_texts.iter().enumerate() {
                entities.push(PidGraphicEntity {
                    id: format!("{}:text-probe:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind: PidGraphicKind::Unknown {
                        note: format!("sheet text probe: {text}"),
                    },
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: None,
                        record_id: Some(format!("text-probe:{index}")),
                        field_x: None,
                        note: Some("text position is not decoded yet".to_string()),
                    },
                    confidence: PidGeometryConfidence::ProbeOnly,
                });
            }
        }

        let endpoint_records: Vec<_> = sheet
            .geometry
            .as_ref()
            .filter(|geometry| !geometry.endpoints.is_empty())
            .map_or_else(
                || {
                    sheet
                        .endpoint_records
                        .iter()
                        .map(|endpoint| {
                            (
                                endpoint.offset,
                                endpoint.rel_field_x,
                                endpoint.endpoint_a,
                                endpoint.endpoint_b,
                            )
                        })
                        .collect()
                },
                |geometry| {
                    geometry
                        .endpoints
                        .iter()
                        .map(|endpoint| {
                            (
                                endpoint.offset,
                                endpoint.rel_field_x,
                                endpoint.endpoint_a,
                                endpoint.endpoint_b,
                            )
                        })
                        .collect()
                },
            );

        for (index, (offset, rel_field_x, endpoint_a, endpoint_b)) in
            endpoint_records.into_iter().enumerate()
        {
            entities.push(PidGraphicEntity {
                id: format!("{}:endpoint-probe:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: None,
                kind: PidGraphicKind::Unknown {
                    note: format!(
                        "sheet endpoint probe: rel_field_x={rel_field_x} endpoints {endpoint_a} -> {endpoint_b}"
                    ),
                },
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: source_range(offset, SHEET_ENDPOINT_RECORD_LEN, sheet.size),
                    record_id: Some(format!("endpoint-probe:{index}")),
                    field_x: Some(rel_field_x),
                    note: Some("endpoint positions are not decoded yet".to_string()),
                },
                confidence: PidGeometryConfidence::ProbeOnly,
            });
        }
    }

    let probe_count = entities.len();
    if probe_count > 0 {
        warnings.push(format!(
            "{probe_count} Sheet evidence item(s) emitted; renderers should still gate by kind and confidence"
        ));
    }

    NormalizedPidGeometry { entities, warnings }
}

fn source_range(start: usize, len: usize, stream_size: u64) -> Option<PidByteRange> {
    if len == 0 {
        return None;
    }
    let end = start.checked_add(len)?;
    if u64::try_from(end).ok()? > stream_size {
        return None;
    }
    Some(PidByteRange { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        SheetCoordinateHintDto, SheetEndpoint, SheetEndpointRecord, SheetGeometry,
        SheetObjectGeometryHint, SheetStream, SheetText,
    };

    #[test]
    fn normalized_geometry_reports_empty_sheet_inputs() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 16,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: None,
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert!(geometry.is_empty());
        assert_eq!(geometry.warnings.len(), 1);
        assert!(geometry.warnings[0].contains("1 Sheet stream"));
    }

    #[test]
    fn sheet_probe_evidence_becomes_probe_only_unknown_entities() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 128,
            extracted_texts: vec!["PUMP-101".into()],
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: None,
            endpoint_records: vec![SheetEndpointRecord {
                sheet_path: "/Sheet6".into(),
                offset: 40,
                rel_field_x: 100,
                endpoint_a: 42,
                endpoint_b: 77,
            }],
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.entities.len(), 2);
        assert!(geometry
            .warnings
            .iter()
            .any(|warning| warning.contains("2 Sheet evidence item")));
        assert!(geometry.entities.iter().all(|entity| {
            matches!(entity.kind, PidGraphicKind::Unknown { .. })
                && entity.confidence == PidGeometryConfidence::ProbeOnly
        }));
        assert_eq!(geometry.entities[1].source.field_x, Some(100));
        assert_eq!(
            geometry.entities[1].source.byte_range,
            Some(PidByteRange { start: 40, end: 66 })
        );
    }

    #[test]
    fn sheet_geometry_evidence_preserves_text_coordinate_and_endpoint_offsets() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 256,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                texts: vec![SheetText {
                    offset: 12,
                    encoding: "utf16_le".into(),
                    text: "PUMP-101".into(),
                    byte_len: 16,
                }],
                endpoints: vec![SheetEndpoint {
                    offset: 80,
                    rel_field_x: 200,
                    endpoint_a: 201,
                    endpoint_b: 202,
                }],
                coordinate_hints: vec![SheetCoordinateHintDto {
                    offset: 40,
                    x: 1200,
                    y: -450,
                }],
                object_geometry_hints: Vec::new(),
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.entities.len(), 3);
        assert_eq!(
            geometry.entities[0].source.byte_range,
            Some(PidByteRange { start: 12, end: 28 })
        );
        assert_eq!(
            geometry.entities[1].source.byte_range,
            Some(PidByteRange { start: 40, end: 48 })
        );
        assert_eq!(
            geometry.entities[1].confidence,
            PidGeometryConfidence::Inferred
        );
        assert!(matches!(
            geometry.entities[1].kind,
            PidGraphicKind::Point {
                position: PidPoint {
                    x: 1200.0,
                    y: -450.0
                }
            }
        ));
        assert_eq!(
            geometry.entities[2].source.byte_range,
            Some(PidByteRange {
                start: 80,
                end: 106
            })
        );
        assert_eq!(geometry.entities[2].source.field_x, Some(200));
    }

    #[test]
    fn coordinate_hints_and_probe_evidence_never_use_decoded_confidence() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 256,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                texts: vec![SheetText {
                    offset: 8,
                    encoding: "ascii".into(),
                    text: "TAG".into(),
                    byte_len: 3,
                }],
                endpoints: vec![SheetEndpoint {
                    offset: 64,
                    rel_field_x: 700,
                    endpoint_a: 701,
                    endpoint_b: 702,
                }],
                coordinate_hints: vec![SheetCoordinateHintDto {
                    offset: 24,
                    x: 1200,
                    y: -450,
                }],
                object_geometry_hints: vec![SheetObjectGeometryHint {
                    offset: 88,
                    field_x: 703,
                    position: Some(SheetCoordinateHintDto {
                        offset: 96,
                        x: 2400,
                        y: -900,
                    }),
                    graphic_oid: Some(17),
                    note: Some(
                        "score=80;identity=graphic_nearby;stable_shape=field_delta:10,coordinate_delta:20,support:3"
                            .into(),
                    ),
                }],
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.entities.len(), 4);
        assert!(
            geometry
                .entities
                .iter()
                .all(|entity| entity.confidence != PidGeometryConfidence::Decoded),
            "coordinate hints and probe records must not become Decoded without typed semantics"
        );
        let inferred_points = geometry
            .entities
            .iter()
            .filter(|entity| {
                entity.confidence == PidGeometryConfidence::Inferred
                    && matches!(entity.kind, PidGraphicKind::Point { .. })
            })
            .count();
        let probe_unknowns = geometry
            .entities
            .iter()
            .filter(|entity| {
                entity.confidence == PidGeometryConfidence::ProbeOnly
                    && matches!(entity.kind, PidGraphicKind::Unknown { .. })
            })
            .count();
        assert_eq!(inferred_points, 2);
        assert_eq!(probe_unknowns, 2);
        assert!(geometry.entities.iter().all(|entity| {
            !matches!(
                entity.kind,
                PidGraphicKind::Line { .. }
                    | PidGraphicKind::Polyline { .. }
                    | PidGraphicKind::Arc { .. }
                    | PidGraphicKind::Circle { .. }
                    | PidGraphicKind::Text { .. }
                    | PidGraphicKind::SymbolInstance { .. }
            )
        }));
    }

    #[test]
    fn truncated_probe_ranges_are_not_claimed() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 30,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                texts: vec![SheetText {
                    offset: 28,
                    encoding: "ascii".into(),
                    text: "TOO-LONG".into(),
                    byte_len: 8,
                }],
                endpoints: vec![SheetEndpoint {
                    offset: 12,
                    rel_field_x: 200,
                    endpoint_a: 201,
                    endpoint_b: 202,
                }],
                coordinate_hints: vec![SheetCoordinateHintDto {
                    offset: 24,
                    x: 1200,
                    y: -450,
                }],
                object_geometry_hints: Vec::new(),
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.entities.len(), 3);
        assert!(
            geometry
                .entities
                .iter()
                .all(|entity| entity.source.byte_range.is_none()),
            "truncated evidence must remain visible but should not claim out-of-bounds byte ranges"
        );
    }

    #[test]
    fn graphic_entity_carries_provenance_and_confidence() {
        let entity = PidGraphicEntity {
            id: "sheet6:line:0".into(),
            drawing_id: Some("DID".into()),
            graphic_oid: Some(42),
            kind: PidGraphicKind::Line {
                start: PidPoint { x: 1.0, y: 2.0 },
                end: PidPoint { x: 3.0, y: 4.0 },
            },
            source: PidGraphicProvenance {
                stream_path: Some("/Sheet6".into()),
                byte_range: Some(PidByteRange { start: 10, end: 30 }),
                record_id: Some("rec-1".into()),
                field_x: Some(7),
                note: None,
            },
            confidence: PidGeometryConfidence::Decoded,
        };

        assert_eq!(entity.graphic_oid, Some(42));
        assert_eq!(entity.confidence, PidGeometryConfidence::Decoded);
    }
}
