//! Normalized drawing geometry projection for decoded `.pid` documents.
//!
//! This module is the contract between low-level `Sheet*` / PSM decoding
//! and renderers such as H7CAD. Coordinate hints are exposed as inferred
//! points because they carry source byte ranges, but they are still not
//! line / text / symbol geometry.

use crate::model::{PidDocument, SheetRecordKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SHEET_ENDPOINT_RECORD_LEN: usize = 26;
const UNKNOWN_UNITS_DIAGNOSTIC: &str =
    "Sheet coordinate units are not decoded from coordinate/page metadata records yet";

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
    /// Coordinate-space, unit, and page-transform interpretation for
    /// coordinates carried by [`Self::kind`].
    #[serde(default)]
    pub coordinate_context: PidCoordinateContext,
    /// Where this entity came from inside the `.pid` file.
    pub source: PidGraphicProvenance,
    /// How strongly the parser understands the entity payload.
    pub confidence: PidGeometryConfidence,
}

/// Coordinate interpretation attached to a normalized graphic entity.
///
/// This keeps source/model coordinates separate from any future renderer
/// viewport conversion.  When the parser cannot decode units or page
/// transform records, the unavailable states are explicit instead of
/// silently treating raw values as pixels or page-space coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PidCoordinateContext {
    /// Coordinate space of numeric points stored in the entity payload.
    pub coordinate_space: PidCoordinateSpace,
    /// Drawing units for numeric coordinates, or an explicit unknown state.
    pub units: PidDrawingUnits,
    /// Page/model transform metadata, or an explicit unavailable state.
    pub page_transform: PidPageTransform,
}

impl Default for PidCoordinateContext {
    fn default() -> Self {
        Self {
            coordinate_space: PidCoordinateSpace::Unknown,
            units: PidDrawingUnits::Unknown {
                diagnostic: UNKNOWN_UNITS_DIAGNOSTIC.to_string(),
            },
            page_transform: PidPageTransform::Unavailable {
                diagnostic: "Sheet page transform metadata is unavailable; source coordinates are preserved without viewport conversion".to_string(),
            },
        }
    }
}

/// Coordinate space represented by a normalized geometry payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PidCoordinateSpace {
    /// Raw coordinate-like values as stored in a Sheet stream.
    SourceSheet,
    /// `SmartPlant` model/drawing coordinates after source semantics are known.
    Model,
    /// Page-space coordinates after applying a decoded page transform.
    Page,
    /// Renderer viewport coordinates; normalized geometry should not emit
    /// this until an explicit renderer conversion has occurred.
    Viewport,
    /// Coordinate interpretation is not decoded for this evidence item.
    Unknown,
}

/// Drawing units attached to normalized geometry coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PidDrawingUnits {
    /// Units were decoded from source metadata.
    Known {
        /// Unit label, for example `"mm"` or `"in"`.
        unit: String,
    },
    /// Units are currently unavailable and must not be guessed.
    Unknown {
        /// Diagnostic explaining why units are unavailable.
        diagnostic: String,
    },
}

/// Page transform metadata for Sheet-derived geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PidPageTransform {
    /// Source-to-page transform was decoded.
    Available {
        /// Transform origin in source/model coordinates.
        origin: PidPoint,
        /// X/Y scale factors from source/model coordinates to page space.
        scale: [f64; 2],
        /// Page bounds after applying the transform.
        page_bounds: PidPageBounds,
        /// 2D affine transform matrix `[m11, m12, m21, m22, dx, dy]`.
        matrix: [f64; 6],
    },
    /// Transform metadata is unavailable and must not be fabricated.
    Unavailable {
        /// Diagnostic explaining why page transform metadata is unavailable.
        diagnostic: String,
    },
}

/// Axis-aligned page bounds for a decoded page transform.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PidPageBounds {
    /// Minimum page-space corner.
    pub min: PidPoint,
    /// Maximum page-space corner.
    pub max: PidPoint,
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

impl PidGraphicKind {
    /// Sheet record kind required when this payload is emitted with decoded
    /// confidence.
    ///
    /// Callers must still check [`PidGraphicEntity::confidence`]. An inferred
    /// line may be backed by endpoint-pair provenance even though decoded
    /// primitive lines require [`SheetRecordKind::PrimitiveLine`].
    pub fn decoded_sheet_record_kind(&self) -> Option<SheetRecordKind> {
        match self {
            Self::Line { .. } => Some(SheetRecordKind::PrimitiveLine),
            Self::Polyline { .. } => Some(SheetRecordKind::PrimitivePolyline),
            Self::Arc { .. } => Some(SheetRecordKind::PrimitiveArc),
            Self::Circle { .. } => Some(SheetRecordKind::PrimitiveCircle),
            Self::Text { .. } => Some(SheetRecordKind::TextPlacementStyle),
            Self::SymbolInstance { .. } => Some(SheetRecordKind::SymbolPlacement),
            Self::Point { .. } | Self::Unknown { .. } => None,
        }
    }
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
    /// Public Sheet record kind that owns [`Self::record_id`], when known.
    ///
    /// Decoded renderable geometry must use a typed kind from
    /// [`crate::model::SheetRecordSchema`]; probe-only evidence may explicitly
    /// use [`SheetRecordKind::Unknown`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub record_kind: Option<SheetRecordKind>,
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

struct ResolvedObjectPosition {
    offset: usize,
    x: f64,
    y: f64,
    byte_len: usize,
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
        let object_positions: BTreeMap<u32, ResolvedObjectPosition> = sheet
            .geometry
            .as_ref()
            .map(|geometry| {
                geometry
                    .object_geometry_hints
                    .iter()
                    .filter_map(|hint| {
                        hint.position
                            .as_ref()
                            .map(|pos| ResolvedObjectPosition {
                                offset: pos.offset,
                                x: f64::from(pos.x),
                                y: f64::from(pos.y),
                                byte_len: 8,
                            })
                            .or_else(|| {
                                hint.f64_position
                                    .as_ref()
                                    .map(|f64_pos| ResolvedObjectPosition {
                                        offset: f64_pos.offset,
                                        x: f64_pos.x,
                                        y: f64_pos.y,
                                        byte_len: 16,
                                    })
                            })
                            .map(|resolved| (hint.field_x, resolved))
                    })
                    .collect()
            })
            .unwrap_or_default();

        if let Some(geometry) = &sheet.geometry {
            for (index, text) in geometry.texts.iter().enumerate() {
                entities.push(PidGraphicEntity {
                    id: format!("{}:text-probe:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind: PidGraphicKind::Unknown {
                        note: format!("sheet text probe: {}", text.text),
                    },
                    coordinate_context: undecoded_sheet_coordinate_context(&sheet.path),
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: source_range(text.offset, text.byte_len, sheet.size),
                        record_id: Some(format!("text-probe:{index}")),
                        record_kind: Some(SheetRecordKind::Unknown),
                        field_x: None,
                        note: Some("text position is not decoded yet".to_string()),
                    },
                    confidence: PidGeometryConfidence::ProbeOnly,
                });
            }

            for (index, hint) in geometry.coordinate_hints.iter().enumerate() {
                let byte_range = source_range(hint.offset, 8, sheet.size);
                let (kind, confidence, note) = if byte_range.is_some() {
                    (
                        PidGraphicKind::Point {
                            position: PidPoint {
                                x: f64::from(hint.x),
                                y: f64::from(hint.y),
                            },
                        },
                        PidGeometryConfidence::Inferred,
                        "coordinate pair promoted as an inferred point; surrounding record semantics are not decoded yet".to_string(),
                    )
                } else {
                    (
                        PidGraphicKind::Unknown {
                            note: format!(
                                "out-of-bounds coordinate hint: x={} y={} at offset {}",
                                hint.x, hint.y, hint.offset
                            ),
                        },
                        PidGeometryConfidence::ProbeOnly,
                        "coordinate pair is not promoted because its byte range is outside the Sheet stream".to_string(),
                    )
                };
                entities.push(PidGraphicEntity {
                    id: format!("{}:coordinate-hint:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind,
                    coordinate_context: sheet_source_coordinate_context(&sheet.path),
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range,
                        record_id: Some(format!("coordinate-hint:{index}")),
                        record_kind: Some(SheetRecordKind::Unknown),
                        field_x: None,
                        note: Some(note),
                    },
                    confidence,
                });
            }

            for (index, hint) in geometry.object_geometry_hints.iter().enumerate() {
                if let Some(ref pos) = hint.position {
                    let byte_range = source_range(pos.offset, 8, sheet.size);
                    let (kind, confidence, note) = if byte_range.is_some() {
                        (
                            PidGraphicKind::Point {
                                position: PidPoint {
                                    x: f64::from(pos.x),
                                    y: f64::from(pos.y),
                                },
                            },
                            PidGeometryConfidence::Inferred,
                            hint.note.clone(),
                        )
                    } else {
                        (
                            PidGraphicKind::Unknown {
                                note: format!(
                                    "out-of-bounds object geometry hint: field_x={} x={} y={} at offset {}",
                                    hint.field_x, pos.x, pos.y, pos.offset
                                ),
                            },
                            PidGeometryConfidence::ProbeOnly,
                            Some(
                                "object geometry hint is not promoted because its coordinate byte range is outside the Sheet stream".to_string(),
                            ),
                        )
                    };
                    entities.push(PidGraphicEntity {
                        id: format!("{}:geometry-hint:{index}", sheet.path),
                        drawing_id: None,
                        graphic_oid: hint.graphic_oid,
                        kind,
                        coordinate_context: sheet_source_coordinate_context(&sheet.path),
                        source: PidGraphicProvenance {
                            stream_path: Some(sheet.path.clone()),
                            byte_range,
                            record_id: Some(format!("geometry-hint:{index}")),
                            record_kind: Some(SheetRecordKind::Unknown),
                            field_x: Some(hint.field_x),
                            note,
                        },
                        confidence,
                    });
                } else if let Some(ref f64_pos) = hint.f64_position {
                    let byte_range = source_range(f64_pos.offset, 16, sheet.size);
                    let (kind, confidence, note) = if byte_range.is_some() {
                        (
                            PidGraphicKind::Point {
                                position: PidPoint {
                                    x: f64_pos.x,
                                    y: f64_pos.y,
                                },
                            },
                            PidGeometryConfidence::Inferred,
                            hint.note.clone(),
                        )
                    } else {
                        (
                            PidGraphicKind::Unknown {
                                note: format!(
                                    "out-of-bounds f64 geometry hint: field_x={} x={:.6} y={:.6} at offset {}",
                                    hint.field_x, f64_pos.x, f64_pos.y, f64_pos.offset
                                ),
                            },
                            PidGeometryConfidence::ProbeOnly,
                            Some(
                                "f64 geometry hint is not promoted because its byte range is outside the Sheet stream".to_string(),
                            ),
                        )
                    };
                    entities.push(PidGraphicEntity {
                        id: format!("{}:geometry-hint:{index}", sheet.path),
                        drawing_id: None,
                        graphic_oid: hint.graphic_oid,
                        kind,
                        coordinate_context: sheet_source_coordinate_context(&sheet.path),
                        source: PidGraphicProvenance {
                            stream_path: Some(sheet.path.clone()),
                            byte_range,
                            record_id: Some(format!("geometry-hint:{index}")),
                            record_kind: Some(SheetRecordKind::Unknown),
                            field_x: Some(hint.field_x),
                            note,
                        },
                        confidence,
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
                    coordinate_context: undecoded_sheet_coordinate_context(&sheet.path),
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: None,
                        record_id: Some(format!("text-probe:{index}")),
                        record_kind: Some(SheetRecordKind::Unknown),
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
            let endpoint_range = source_range(offset, SHEET_ENDPOINT_RECORD_LEN, sheet.size);
            if let (Some(start), Some(end), Some(byte_range), Some(start_range), Some(end_range)) = (
                object_positions.get(&endpoint_a),
                object_positions.get(&endpoint_b),
                endpoint_range,
                object_positions
                    .get(&endpoint_a)
                    .and_then(|pos| source_range(pos.offset, pos.byte_len, sheet.size)),
                object_positions
                    .get(&endpoint_b)
                    .and_then(|pos| source_range(pos.offset, pos.byte_len, sheet.size)),
            ) {
                entities.push(PidGraphicEntity {
                    id: format!("{}:endpoint-line:{index}", sheet.path),
                    drawing_id: None,
                    graphic_oid: None,
                    kind: PidGraphicKind::Line {
                        start: PidPoint {
                            x: start.x,
                            y: start.y,
                        },
                        end: PidPoint {
                            x: end.x,
                            y: end.y,
                        },
                    },
                    coordinate_context: sheet_source_coordinate_context(&sheet.path),
                    source: PidGraphicProvenance {
                        stream_path: Some(sheet.path.clone()),
                        byte_range: Some(byte_range),
                        record_id: Some(format!("endpoint-line:{index}")),
                        record_kind: Some(SheetRecordKind::EndpointPair),
                        field_x: Some(rel_field_x),
                        note: Some(format!(
                            "endpoint pair promoted to inferred line; endpoint_a_field_x={endpoint_a} position_range={}..{}; endpoint_b_field_x={endpoint_b} position_range={}..{}",
                            start_range.start, start_range.end, end_range.start, end_range.end
                        )),
                    },
                    confidence: PidGeometryConfidence::Inferred,
                });
                continue;
            }

            entities.push(PidGraphicEntity {
                id: format!("{}:endpoint-probe:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: None,
                kind: PidGraphicKind::Unknown {
                    note: format!(
                        "sheet endpoint probe: rel_field_x={rel_field_x} endpoints {endpoint_a} -> {endpoint_b}"
                    ),
                },
                coordinate_context: undecoded_sheet_coordinate_context(&sheet.path),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: endpoint_range,
                    record_id: Some(format!("endpoint-probe:{index}")),
                    record_kind: Some(SheetRecordKind::EndpointPair),
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
        warnings.push(
            "Sheet coordinate units and page transforms are unavailable; source coordinates remain unconverted and every entity carries explicit coordinate_context diagnostics"
                .to_string(),
        );
    }

    NormalizedPidGeometry { entities, warnings }
}

fn sheet_source_coordinate_context(sheet_path: &str) -> PidCoordinateContext {
    PidCoordinateContext {
        coordinate_space: PidCoordinateSpace::SourceSheet,
        units: unknown_sheet_units(),
        page_transform: unavailable_sheet_transform(sheet_path),
    }
}

fn undecoded_sheet_coordinate_context(sheet_path: &str) -> PidCoordinateContext {
    PidCoordinateContext {
        coordinate_space: PidCoordinateSpace::Unknown,
        units: unknown_sheet_units(),
        page_transform: unavailable_sheet_transform(sheet_path),
    }
}

fn unknown_sheet_units() -> PidDrawingUnits {
    PidDrawingUnits::Unknown {
        diagnostic: UNKNOWN_UNITS_DIAGNOSTIC.to_string(),
    }
}

fn unavailable_sheet_transform(sheet_path: &str) -> PidPageTransform {
    PidPageTransform::Unavailable {
        diagnostic: format!(
            "Sheet page transform metadata is not decoded for {sheet_path}; source coordinates are preserved without viewport conversion"
        ),
    }
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
            geometry.entities[0].source.record_kind,
            Some(SheetRecordKind::Unknown)
        );
        assert_eq!(
            geometry.entities[1].source.record_kind,
            Some(SheetRecordKind::EndpointPair)
        );
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
    fn endpoint_pair_with_promoted_endpoint_positions_becomes_inferred_line() {
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
                texts: Vec::new(),
                endpoints: vec![SheetEndpoint {
                    offset: 80,
                    rel_field_x: 200,
                    endpoint_a: 201,
                    endpoint_b: 202,
                }],
                coordinate_hints: Vec::new(),
                object_geometry_hints: vec![
                    SheetObjectGeometryHint {
                        offset: 120,
                        field_x: 201,
                        position: Some(SheetCoordinateHintDto {
                            offset: 128,
                            x: 100,
                            y: 200,
                        }),
                        f64_position: None,
                        graphic_oid: None,
                        note: Some("score=80 identity=graphic_nearby stable_shape".into()),
                    },
                    SheetObjectGeometryHint {
                        offset: 140,
                        field_x: 202,
                        position: Some(SheetCoordinateHintDto {
                            offset: 148,
                            x: 300,
                            y: 400,
                        }),
                        f64_position: None,
                        graphic_oid: None,
                        note: Some("score=80 identity=graphic_nearby stable_shape".into()),
                    },
                ],
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.entities.len(), 3);
        let line = geometry
            .entities
            .iter()
            .find(|entity| entity.id == "/Sheet6:endpoint-line:0")
            .expect("endpoint pair should become an inferred line");
        assert_eq!(line.confidence, PidGeometryConfidence::Inferred);
        assert_eq!(line.source.record_kind, Some(SheetRecordKind::EndpointPair));
        assert_eq!(
            line.source.byte_range,
            Some(PidByteRange {
                start: 80,
                end: 106
            })
        );
        assert!(line
            .source
            .note
            .as_deref()
            .is_some_and(|note| note.contains("endpoint_a_field_x=201")
                && note.contains("endpoint_b_field_x=202")));
        assert!(matches!(
            line.kind,
            PidGraphicKind::Line {
                start: PidPoint { x: 100.0, y: 200.0 },
                end: PidPoint { x: 300.0, y: 400.0 },
            }
        ));
        assert!(!geometry
            .entities
            .iter()
            .any(|entity| entity.id == "/Sheet6:endpoint-probe:0"));
    }

    #[test]
    fn inferred_endpoint_line_json_exposes_confidence_and_endpoint_provenance() {
        let entity = PidGraphicEntity {
            id: "/Sheet6:endpoint-line:0".into(),
            drawing_id: None,
            graphic_oid: None,
            kind: PidGraphicKind::Line {
                start: PidPoint { x: 1.0, y: 2.0 },
                end: PidPoint { x: 3.0, y: 4.0 },
            },
            coordinate_context: sheet_source_coordinate_context("/Sheet6"),
            source: PidGraphicProvenance {
                stream_path: Some("/Sheet6".into()),
                byte_range: Some(PidByteRange {
                    start: 80,
                    end: 106,
                }),
                record_id: Some("endpoint-line:0".into()),
                record_kind: Some(SheetRecordKind::EndpointPair),
                field_x: Some(200),
                note: Some("endpoint pair promoted to inferred line".into()),
            },
            confidence: PidGeometryConfidence::Inferred,
        };

        let value = serde_json::to_value(&entity).expect("entity JSON");

        assert_eq!(value["kind"]["kind"], "line");
        assert_eq!(value["confidence"], "inferred");
        assert_eq!(value["source"]["record_kind"], "endpoint_pair");
        assert_eq!(value["source"]["field_x"], 200);
        assert_eq!(value["source"]["byte_range"]["start"], 80);
        assert_eq!(
            value["coordinate_context"]["coordinate_space"],
            "source_sheet"
        );
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
                    f64_position: None,
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
    fn sheet_entities_declare_coordinate_units_and_transform_state() {
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
                    encoding: "ascii".into(),
                    text: "TAG".into(),
                    byte_len: 3,
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
        assert!(geometry.warnings.iter().any(|warning| {
            warning.contains("coordinate units and page transforms are unavailable")
        }));
        for entity in &geometry.entities {
            assert_eq!(
                entity.coordinate_context.units,
                PidDrawingUnits::Unknown {
                    diagnostic:
                        "Sheet coordinate units are not decoded from coordinate/page metadata records yet"
                            .into()
                }
            );
            assert!(matches!(
                entity.coordinate_context.page_transform,
                PidPageTransform::Unavailable { ref diagnostic }
                    if diagnostic.contains("source coordinates are preserved without viewport conversion")
            ));
        }

        let point_entity = geometry
            .entities
            .iter()
            .find(|entity| matches!(entity.kind, PidGraphicKind::Point { .. }))
            .expect("coordinate hint should produce a point");
        assert_eq!(
            point_entity.coordinate_context.coordinate_space,
            PidCoordinateSpace::SourceSheet
        );
        assert!(matches!(
            point_entity.kind,
            PidGraphicKind::Point {
                position: PidPoint {
                    x: 1200.0,
                    y: -450.0
                }
            }
        ));
    }

    #[test]
    fn inferred_entities_require_bounded_sheet_provenance() {
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 43,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                texts: Vec::new(),
                endpoints: Vec::new(),
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

        assert_eq!(geometry.entities.len(), 1);
        let entity = &geometry.entities[0];
        assert_eq!(entity.source.byte_range, None);
        assert_eq!(entity.confidence, PidGeometryConfidence::ProbeOnly);
        assert!(matches!(entity.kind, PidGraphicKind::Unknown { .. }));
        assert!(
            geometry
                .entities
                .iter()
                .filter(|entity| entity.confidence == PidGeometryConfidence::Inferred)
                .all(|entity| entity.source.byte_range.is_some()),
            "inferred entities must have bounded byte provenance"
        );
    }

    #[test]
    fn available_page_transform_json_exposes_bounds_and_matrix() {
        let context = PidCoordinateContext {
            coordinate_space: PidCoordinateSpace::Model,
            units: PidDrawingUnits::Known { unit: "mm".into() },
            page_transform: PidPageTransform::Available {
                origin: PidPoint { x: 10.0, y: 20.0 },
                scale: [2.0, 2.0],
                page_bounds: PidPageBounds {
                    min: PidPoint { x: 0.0, y: 0.0 },
                    max: PidPoint { x: 100.0, y: 200.0 },
                },
                matrix: [2.0, 0.0, 0.0, 2.0, -20.0, -40.0],
            },
        };

        let value = serde_json::to_value(&context).expect("coordinate context JSON");

        assert_eq!(value["coordinate_space"], "model");
        assert_eq!(value["units"]["state"], "known");
        assert_eq!(value["units"]["unit"], "mm");
        assert_eq!(value["page_transform"]["state"], "available");
        assert_eq!(value["page_transform"]["origin"]["x"], 10.0);
        assert_eq!(value["page_transform"]["scale"][0], 2.0);
        assert_eq!(value["page_transform"]["page_bounds"]["max"]["y"], 200.0);
        assert_eq!(value["page_transform"]["matrix"][4], -20.0);
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
            coordinate_context: PidCoordinateContext::default(),
            source: PidGraphicProvenance {
                stream_path: Some("/Sheet6".into()),
                byte_range: Some(PidByteRange { start: 10, end: 30 }),
                record_id: Some("rec-1".into()),
                record_kind: Some(SheetRecordKind::PrimitiveLine),
                field_x: Some(7),
                note: None,
            },
            confidence: PidGeometryConfidence::Decoded,
        };

        assert_eq!(entity.graphic_oid, Some(42));
        assert_eq!(entity.confidence, PidGeometryConfidence::Decoded);
    }

    #[test]
    fn decoded_renderable_kinds_map_to_typed_sheet_record_kinds() {
        let cases = [
            (
                PidGraphicKind::Line {
                    start: PidPoint { x: 0.0, y: 0.0 },
                    end: PidPoint { x: 1.0, y: 1.0 },
                },
                SheetRecordKind::PrimitiveLine,
            ),
            (
                PidGraphicKind::Polyline {
                    points: vec![PidPoint { x: 0.0, y: 0.0 }, PidPoint { x: 1.0, y: 1.0 }],
                    closed: false,
                },
                SheetRecordKind::PrimitivePolyline,
            ),
            (
                PidGraphicKind::Circle {
                    center: PidPoint { x: 2.0, y: 3.0 },
                    radius: 4.0,
                },
                SheetRecordKind::PrimitiveCircle,
            ),
            (
                PidGraphicKind::Arc {
                    center: PidPoint { x: 2.0, y: 3.0 },
                    radius: 4.0,
                    start_angle: 0.0,
                    end_angle: 1.0,
                },
                SheetRecordKind::PrimitiveArc,
            ),
            (
                PidGraphicKind::Text {
                    insertion: PidPoint { x: 5.0, y: 6.0 },
                    value: "TAG".into(),
                    height: 2.5,
                    rotation: 0.0,
                },
                SheetRecordKind::TextPlacementStyle,
            ),
            (
                PidGraphicKind::SymbolInstance {
                    insertion: PidPoint { x: 7.0, y: 8.0 },
                    symbol_path: Some("Piping/Valve".into()),
                    rotation: 0.0,
                    scale: [1.0, 1.0],
                },
                SheetRecordKind::SymbolPlacement,
            ),
        ];

        for (kind, expected_record_kind) in cases {
            assert_eq!(
                kind.decoded_sheet_record_kind(),
                Some(expected_record_kind),
                "decoded geometry kind must map to the public Sheet schema contract"
            );
            let entity = PidGraphicEntity {
                id: "decoded".into(),
                drawing_id: None,
                graphic_oid: None,
                coordinate_context: PidCoordinateContext::default(),
                source: PidGraphicProvenance {
                    stream_path: Some("/Sheet6".into()),
                    byte_range: Some(PidByteRange { start: 10, end: 30 }),
                    record_id: Some("sheet-record".into()),
                    record_kind: kind.decoded_sheet_record_kind(),
                    field_x: None,
                    note: None,
                },
                kind,
                confidence: PidGeometryConfidence::Decoded,
            };
            assert_eq!(entity.source.record_kind, Some(expected_record_kind));
            assert_ne!(entity.source.record_kind, Some(SheetRecordKind::Unknown));
        }
    }

    #[test]
    fn decoded_geometry_json_exposes_record_id_and_typed_kind() {
        let geometry = NormalizedPidGeometry {
            entities: vec![PidGraphicEntity {
                id: "sheet6:line:0".into(),
                drawing_id: None,
                graphic_oid: None,
                kind: PidGraphicKind::Line {
                    start: PidPoint { x: 1.0, y: 2.0 },
                    end: PidPoint { x: 3.0, y: 4.0 },
                },
                coordinate_context: PidCoordinateContext::default(),
                source: PidGraphicProvenance {
                    stream_path: Some("/Sheet6".into()),
                    byte_range: Some(PidByteRange {
                        start: 100,
                        end: 124,
                    }),
                    record_id: Some("sheet.primitive.line:0".into()),
                    record_kind: Some(SheetRecordKind::PrimitiveLine),
                    field_x: None,
                    note: None,
                },
                confidence: PidGeometryConfidence::Decoded,
            }],
            warnings: Vec::new(),
        };

        let value = serde_json::to_value(&geometry).expect("geometry JSON");
        let source = &value["entities"][0]["source"];
        assert_eq!(source["record_id"], "sheet.primitive.line:0");
        assert_eq!(source["record_kind"], "primitive_line");
    }
}
