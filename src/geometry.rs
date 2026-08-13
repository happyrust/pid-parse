//! Normalized drawing geometry projection for decoded `.pid` documents.
//!
//! This module is the contract between low-level `Sheet*` / PSM decoding
//! and renderers such as H7CAD. Coordinate hints are exposed as inferred
//! points because they carry source byte ranges, but they are still not
//! line / text / symbol geometry.

use crate::model::{PidDocument, SheetRecordKind, SheetStream};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SHEET_ENDPOINT_RECORD_LEN: usize = 26;
const UNKNOWN_UNITS_DIAGNOSTIC: &str =
    "Sheet coordinate units are not decoded from coordinate/page metadata records yet";
/// Millimetres in a metre. A border frame states its extent in metres;
/// [`NormalizedPidGeometry::page_dimensions_mm`] reports it in millimetres.
const MM_PER_METRE: f64 = 1000.0;
/// Unit label for coordinates a decoded border frame proves are metres.
const SOURCE_UNIT_METRE: &str = "m";

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
    /// Page dimensions in mm, from the drawing's own `0x003D` border frame
    /// where it has one and from its template name otherwise.
    ///
    /// This is page-size evidence only.  A template name must not by itself
    /// make [`PidPageTransform::Available`] appear on individual entities;
    /// only a decoded border frame carries enough evidence for that.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub page_dimensions_mm: Option<(f64, f64)>,
    /// Non-fatal diagnostics explaining missing or skipped geometry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Phase 38 S2: graphic-class PSM records this crate has **no decoder
    /// for**, named per `(stream, type code)` instead of silently dropped.
    ///
    /// Every entry also produces a prose warning in [`Self::warnings`];
    /// this structured view exists so consumers (`OpenCADStudio`'s
    /// `report_import`) can surface the drop prominently without string
    /// matching. Non-graphic unknown records (constraint families) are
    /// deliberately absent — the native predicate says they draw nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped_graphic_records: Vec<PidDroppedGraphicRecords>,
    /// Phase 40: graphic-class PSM records this crate **does have a decoder
    /// for** and whose bytes that decoder refused, named per `(stream, type
    /// code)` instead of silently dropped.
    ///
    /// The companion to [`Self::dropped_graphic_records`], and on the corpus
    /// the larger of the two by a factor of 28. The two ask for different
    /// work: a dropped record needs a decoder written, a refused one needs an
    /// existing decoder's rules re-measured against a record shape the
    /// drawings actually contain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused_graphic_records: Vec<PidRefusedGraphicRecords>,
}

/// One group of undecoded graphic-class PSM records in one Sheet stream:
/// content `SmartPlant`'s own graphic predicate says should draw, which
/// this crate cannot decode yet and therefore drops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PidDroppedGraphicRecords {
    /// Full CFB path of the Sheet stream the records sit in.
    pub stream_path: String,
    /// PSM 14-bit type code.
    pub type_code: u16,
    /// Class name from the PSM type-code registry, when known
    /// (e.g. `igDimension`, `Graphics Bag`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rad_class_name: Option<String>,
    /// Chain-validated record count outside every claimed byte range.
    pub count: usize,
}

/// One group of graphic-class PSM records in one Sheet stream that their
/// own family's decoder walked over and refused.
///
/// The family is wired and the type code is decoded elsewhere in the same
/// corpus, so this is not a missing decoder: it is a record shape the
/// existing rules do not accept. Refusing is the right default — a decoder
/// that guessed at an unknown shape would draw fiction — but refusing in
/// silence is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PidRefusedGraphicRecords {
    /// Full CFB path of the Sheet stream the records sit in.
    pub stream_path: String,
    /// PSM 14-bit type code — one this crate decodes.
    pub type_code: u16,
    /// Class name from the PSM type-code registry, when known
    /// (e.g. `Line Object`, `Text Object`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rad_class_name: Option<String>,
    /// Chain-validated record count the family's decoder did not claim.
    pub count: usize,
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
    ///
    /// This state requires source-proven transform metadata: page dimensions
    /// alone, scalar hits, or normalized f64 coordinate evidence are not enough.
    /// The decoder must know the source coordinate space, units, transform
    /// direction, and bounded byte provenance before emitting this variant.
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
    /// Annotation record decoded from PSM type `0x0030` (`JStyleOverride`,
    /// RAD `style.dll` CLSID `{47FCC338-...}`). Phase 16 Slice C/F:
    /// `SmartPlant` overloads the RAD `JStyleOverride` class as a tagged
    /// instrument / annotation placement object. The exact field
    /// semantics for `+0..15` remain ambiguous between IDA Version 3
    /// schema (4×u32) and probe v5 cross-fixture evidence (2×f64
    /// `anchor`); the variant carries the probe-derived `anchor`
    /// interpretation but exposes only fields with strong
    /// double-evidence support. Current projections therefore emit this
    /// kind with [`PidGeometryConfidence::Inferred`], even though the
    /// underlying `JStyleOverride` record layout is decoded.
    Annotation {
        /// Inferred anchor point from payload bytes `+0..15`
        /// interpreted as `(f64, f64)`. Cross-fixture probe shows
        /// these values consistently fall in the normalized
        /// coordinate range `[0, 1]`; the IDA Version-3 schema reads
        /// the same bytes as four `u32` fields, so this anchor
        /// remains a probe-derived interpretation pending
        /// disambiguation.
        anchor: PidPoint,
        /// Rotation angle in radians from payload bytes `+24..31`
        /// (IDA Version-3 `field_2_f64`). Cross-fixture observations
        /// cluster around `{0, π/2, 3π/2, 2π}`, consistent with the
        /// orthogonal orientations used by `SmartPlant` instrument
        /// symbols.
        rotation_angle: f64,
        /// Secondary anchor / radius candidate from payload bytes
        /// `+16..23` (IDA Version-3 `field_1_f64`). About 39 % of
        /// cross-fixture records have this value byte-identical to
        /// `anchor.x`, suggesting either a radius (when the
        /// instrument is positioned at `(r, _)`) or a secondary
        /// anchor coordinate.
        secondary_radius: f64,
        /// Human-readable diagnostic explanation, including
        /// `bytes_to_follow` and tail-length information.
        note: String,
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
            Self::Arc { .. } => None,
            Self::Circle { .. } => Some(SheetRecordKind::PrimitiveCircle),
            Self::Text { .. } => Some(SheetRecordKind::TextPlacementStyle),
            Self::SymbolInstance { .. } => Some(SheetRecordKind::SymbolPlacement),
            Self::Annotation { .. } => Some(SheetRecordKind::JStyleOverride),
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
    /// Entity geometry semantics are inferred from bounded source evidence but
    /// are not decoded strongly enough for [`Self::Decoded`].
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

/// The sheet page a drawing states through its own border frame.
///
/// `0x003D igSmartFrame2d` is an OLE container frame, and a P&ID's border
/// template is exactly that: an object linked or embedded into the sheet whose
/// framed extent is the sheet size. The extent is in metres, the origin is
/// `(0, 0)`, and source coordinates already are page coordinates. All seven
/// `ROADMAP-PAGE-TRANSFORM` requirements are discharged against the corpus in
/// `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PageFrame {
    width_m: f64,
    height_m: f64,
}

impl PageFrame {
    /// The page in millimetres, which is the unit a renderer frames on.
    fn dimensions_mm(self) -> (f64, f64) {
        (self.width_m * MM_PER_METRE, self.height_m * MM_PER_METRE)
    }

    /// The identity source-to-page transform this frame proves.
    ///
    /// Source coordinates already are page coordinates -- across the corpus
    /// every drawing's content lands inside `(0, 0)-(w, h)` with a margin on
    /// all four edges -- so the scale is 1 and the origin is the page's own
    /// corner. Converting to millimetres is a unit change applied afterwards,
    /// not part of the transform.
    fn page_transform(self) -> PidPageTransform {
        PidPageTransform::Available {
            origin: PidPoint { x: 0.0, y: 0.0 },
            scale: [1.0, 1.0],
            page_bounds: PidPageBounds {
                min: PidPoint { x: 0.0, y: 0.0 },
                max: PidPoint {
                    x: self.width_m,
                    y: self.height_m,
                },
            },
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }
}

/// The page every border frame in `doc` agrees on.
///
/// A drawing can carry the same frame several times -- `DWG-0201` repeats it
/// six times in `/Sheet6` -- so the frames are deduplicated by extent and a
/// page is taken only when one extent is left. Two frames that disagree are
/// two claims about the page size, and choosing between them would be a
/// guess, so the caller falls back to the template name instead. Repeats are
/// bit-identical across the corpus, which is why exact equality is the test.
fn decoded_page_frame(doc: &PidDocument) -> Option<PageFrame> {
    let mut extents: Vec<(f64, f64)> = Vec::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };
        for frame in &geometry.decoded_igsmartframes {
            let Some(extent) = frame.page_extent_m() else {
                continue;
            };
            if !extents.iter().any(|seen| same_extent(*seen, extent)) {
                extents.push(extent);
            }
        }
    }
    let &[(width_m, height_m)] = extents.as_slice() else {
        return None;
    };
    Some(PageFrame { width_m, height_m })
}

/// Whether two framed extents are the same page, to the bit.
fn same_extent(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits()
}

/// Page dimensions read off the drawing template's name.
///
/// The fallback for a drawing with no border frame to state its own page. A
/// name is not a measurement: it cannot tell a 594.3mm sheet from a 594.0mm
/// one, and a template not named for an ISO size yields nothing. It never
/// promotes [`PidPageTransform`].
fn infer_page_dimensions(doc: &PidDocument) -> Option<(f64, f64)> {
    let template = doc
        .drawing_meta
        .as_ref()
        .and_then(|meta| meta.tags.get("Template"))?;
    let upper = template.to_uppercase();
    if upper.contains("A0") {
        Some((1189.0, 841.0))
    } else if upper.contains("A1") {
        Some((841.0, 594.0))
    } else if upper.contains("A2") {
        Some((594.0, 420.0))
    } else if upper.contains("A3") {
        Some((420.0, 297.0))
    } else if upper.contains("A4") {
        Some((297.0, 210.0))
    } else {
        None
    }
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
    let page_frame = decoded_page_frame(doc);
    let page_dims = page_frame
        .map(PageFrame::dimensions_mm)
        .or_else(|| infer_page_dimensions(doc));
    if page_dims.is_none() {
        warnings.push(
            "coordinate units and page transforms are unavailable; geometry uses raw values"
                .to_string(),
        );
    }
    let sheet_count = doc.sheet_streams.len();
    if sheet_count == 0 {
        warnings.push("no Sheet streams available for geometry decode".to_string());
    } else {
        warnings.push(format!(
            "geometry decode remains partial across {sheet_count} Sheet stream(s); decoded, inferred, and probe-only evidence may coexist"
        ));
    }

    // Phase 38 S2: records SmartPlant's own graphic predicate says should
    // draw, which no decoder here claims, stop being silent. One named
    // warning per (stream, type code); the constraint families the
    // predicate rejects stay quiet by design.
    let mut dropped_graphic_records = Vec::new();
    for sheet in &doc.sheet_streams {
        let Some(sheet_geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for census in &sheet_geometry.undecoded_type_codes {
            if !census.is_graphic {
                continue;
            }
            let class_name = census
                .rad_class_name
                .as_deref()
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            warnings.push(format!(
                "{count} record(s) of undecoded graphic type 0x{code:04X}{class_name} in {path} \
                 have no decoder and are dropped from the drawing",
                count = census.count,
                code = census.type_code,
                path = sheet.path,
            ));
            dropped_graphic_records.push(PidDroppedGraphicRecords {
                stream_path: sheet.path.clone(),
                type_code: census.type_code,
                rad_class_name: census.rad_class_name.clone(),
                count: census.count,
            });
        }
    }

    // Phase 40: the same treatment for the other way a record misses the
    // drawing. S2 closed "no decoder exists"; a record its own decoder
    // refused stayed silent, and on this corpus that is the bigger half.
    let mut refused_graphic_records = Vec::new();
    for sheet in &doc.sheet_streams {
        let Some(sheet_geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for census in &sheet_geometry.refused_records {
            if !census.is_graphic {
                continue;
            }
            let class_name = census
                .rad_class_name
                .as_deref()
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            warnings.push(format!(
                "{count} record(s) of 0x{code:04X}{class_name} in {path} are a shape this \
                 crate's decoder for that type refuses, and are dropped from the drawing",
                count = census.count,
                code = census.type_code,
                path = sheet.path,
            ));
            refused_graphic_records.push(PidRefusedGraphicRecords {
                stream_path: sheet.path.clone(),
                type_code: census.type_code,
                rad_class_name: census.rad_class_name.clone(),
                count: census.count,
            });
        }
    }

    let ctx = EmitContext::from_doc(doc, page_frame);

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

        // M2 seam (RFC §3.2): every decoded PSM record family emits
        // through its registered `GeometryEmitter`. Emitters run **in
        // addition to** the EndpointPair-derived inferred lines above
        // so existing inferred geometry never regresses; consumers
        // should pick the right entity by `confidence` + `record_kind`.
        // Audit-only families are registered as explicit no-op
        // emitters, making the emission policy visible in one table.
        for emitter in EMITTERS {
            emitter.emit(&ctx, sheet, &mut entities);
        }
    }

    let evidence_count = entities.len();
    if evidence_count > 0 {
        let decoded_count = entities
            .iter()
            .filter(|entity| entity.confidence == PidGeometryConfidence::Decoded)
            .count();
        let inferred_count = entities
            .iter()
            .filter(|entity| entity.confidence == PidGeometryConfidence::Inferred)
            .count();
        let probe_only_count = entities
            .iter()
            .filter(|entity| entity.confidence == PidGeometryConfidence::ProbeOnly)
            .count();
        warnings.push(format!(
            "{evidence_count} Sheet evidence item(s) emitted ({decoded_count} decoded, \
             {inferred_count} inferred, {probe_only_count} probe-only); renderers should \
             still gate by kind and confidence"
        ));
        warnings.push(match page_frame {
            Some(frame) => {
                let (width_mm, height_mm) = frame.dimensions_mm();
                format!(
                    "decoded geometry is in metres on the {width_mm:.1} x {height_mm:.1} mm page the drawing's own border frame states; inferred and probe-only evidence keeps unconverted source values"
                )
            }
            None => "Sheet coordinate units and page transforms are unavailable; source coordinates remain unconverted and every entity carries explicit coordinate_context diagnostics"
                .to_string(),
        });
    }

    NormalizedPidGeometry {
        entities,
        page_dimensions_mm: page_dims,
        warnings,
        dropped_graphic_records,
        refused_graphic_records,
    }
}

/// Per-family emission seam (RFC §3.2, L6 counterpart of the parser-side
/// `PsmRecordDecoder`): one adapter per PSM record family turns that
/// family's decoded records on a sheet into zero or more
/// [`PidGraphicEntity`] values.
///
/// Contract:
///
/// - Emitters read only `sheet` and append to `out`; per-family
///   differences (`PidGraphicKind`, `confidence`, evidence notes) live
///   entirely inside each adapter.
/// - **Audit-only families register deliberate no-op emitters** so the
///   "which families emit geometry" policy is visible in the
///   [`EMITTERS`] table rather than implied by absence. Per ADR-0001 /
///   CONTEXT.md, promoting an audit-only family to emission is an
///   evidence-gated decision, never a refactor side effect.
trait GeometryEmitter {
    /// The [`crate::model::sheet_families::SheetRecordFamily::name`]
    /// this emitter serves.
    // Read only by the registry policy test via `emitter_families`.
    #[allow(dead_code)]
    fn family(&self) -> &'static str;

    /// Whether this emitter is a deliberate no-op (audit-only family).
    ///
    /// Declared beside [`GeometryEmitter::emit`] so the claim and the
    /// behaviour sit one code review apart, and exported through
    /// [`emitter_families`] so the registry's `emits_geometry` flag is
    /// tested against the emitters themselves rather than against a
    /// second hand-written copy of the same claim — `igBoundary2d`
    /// drifted exactly that way when fills landed.
    // Read only by the registry policy test via `emitter_families`.
    #[allow(dead_code)]
    fn is_no_op(&self) -> bool {
        false
    }

    /// Append this family's entities for `sheet` onto `out`. `ctx`
    /// carries document-level lookups shared by the whole pass.
    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>);
}

/// Document-level context shared by every [`GeometryEmitter`] during
/// one [`build_normalized_geometry`] pass. Carries cross-stream
/// lookups that an individual `Sheet*` stream cannot resolve on its
/// own.
struct EmitContext<'a> {
    /// `JSite<id>` storage id → symbol-library path (that site's
    /// `JProperties` `.sym` string), used to resolve
    /// `SymbolInstance::symbol_path` from
    /// [`crate::model::DecodedIgSymbol2dRecord::jsite_ref`]
    /// (Phase 35-C).
    jsite_symbol_paths: BTreeMap<u32, &'a str>,
    /// The page the drawing's own border frame states, when it has one.
    /// Decoded coordinates are metres on this page; see
    /// [`decoded_sheet_coordinate_context`].
    page: Option<PageFrame>,
}

impl<'a> EmitContext<'a> {
    /// Build the pass context from the parsed document.
    fn from_doc(doc: &'a PidDocument, page: Option<PageFrame>) -> Self {
        let jsite_symbol_paths = doc
            .jsites
            .iter()
            .filter_map(|site| {
                let id: u32 = site.name.strip_prefix("JSite")?.parse().ok()?;
                let path = site
                    .symbol_path
                    .as_deref()
                    .or(site.local_symbol_path.as_deref())?;
                Some((id, path))
            })
            .collect();
        Self {
            jsite_symbol_paths,
            page,
        }
    }
}

/// Emission registry walked by [`build_normalized_geometry`] for every
/// sheet. Order is stable and part of the golden-snapshot contract
/// (`tests/geometry_golden_snapshot.rs`): emitting families first in
/// their historical order, then the audit-only no-op families.
const EMITTERS: &[&dyn GeometryEmitter] = &[
    // Emitting families (historical order — do not reorder without
    // re-blessing the golden snapshot).
    &GLine2dEmitter,
    &IgSymbol2dEmitter,
    &IgTextBoxEmitter,
    &IgPoint2dEmitter,
    &IgLineString2dEmitter,
    &IgLine2dEmitter,
    &JStyleOverrideEmitter,
    // Emits since fills landed: a boundary ring whose style resolves to
    // a `JStyleSimpleFill` is an area the member lines cannot express.
    // It stays in this position — after the historical emitting block —
    // because the table order is part of the golden-snapshot contract.
    &IgBoundary2dEmitter,
    // Audit-only families: explicit no-op policy.
    &IgSmartFrame2dEmitter,
    &DependencyObjectEmitter,
    &SubRecord0x0010Emitter,
    &AttributeFragmentEmitter,
];

/// `(family name, is_no_op)` for every registered emitter, in table
/// order — one row per [`crate::model::sheet_families::SHEET_RECORD_FAMILIES`]
/// entry.
///
/// This is the emitters' own statement of the emission policy. The
/// registry's `emits_geometry` flag is the *claim*; the emitter is the
/// *behaviour*; `model::sheet_families`' policy test holds the two
/// against each other through this function, so a family cannot start
/// emitting while the registry still calls it audit-only.
// Its only caller is that `#[cfg(test)]` policy test, so the non-test
// build sees it as dead; the precedent for keeping it visible is the
// documented-intent `allow` in `bin/pid_backup_extract.rs`.
#[allow(dead_code)]
pub(crate) fn emitter_families() -> Vec<(&'static str, bool)> {
    EMITTERS
        .iter()
        .map(|emitter| (emitter.family(), emitter.is_no_op()))
        .collect()
}

/// Emits `Decoded` [`PidGraphicKind::Line`] entities for the
/// `SmartPlant` extended `GLine2d` family (PSM `0x3FE6`), converting
/// the parametric `origin + t·direction` form to Cartesian endpoints.
struct GLine2dEmitter;

impl GeometryEmitter for GLine2dEmitter {
    fn family(&self) -> &'static str {
        "GLine2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_primitive_lines.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            let (ax, ay) = record.endpoint_a();
            let (bx, by) = record.endpoint_b();
            out.push(PidGraphicEntity {
                id: format!("{}:primitive-line:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Line {
                    start: PidPoint { x: ax, y: ay },
                    end: PidPoint { x: bx, y: by },
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("primitive-line:{index}")),
                    record_kind: Some(SheetRecordKind::PrimitiveLine),
                    field_x: None,
                    note: Some(format!(
                        "PSM GLine2d record decoded from radsrvitem.dll byte layout (\
                         18-byte header + 6 x f64 payload); oid={} type_code=0x{:04X} \
                         type_flags=0x{:X} bytes_to_follow={} origin=({:.4}, {:.4}) \
                         direction=({:.5}, {:.5}) param=[{:.4}, {:.4}]",
                        record.oid,
                        record.type_code,
                        record.type_flags,
                        record.bytes_to_follow,
                        record.origin_x,
                        record.origin_y,
                        record.direction_x,
                        record.direction_y,
                        record.param_start,
                        record.param_end,
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `Decoded` [`PidGraphicKind::SymbolInstance`] entities for the
/// `igSymbol2d` family (PSM `0x00CE`).
struct IgSymbol2dEmitter;

/// Split an `igSymbol2d` 2×2 placement matrix into an angle and a scale.
///
/// Half the placements in the fixtures are reflections rather than
/// rotations (`det < 0`) — a P&ID mirrors a valve as readily as it turns
/// one. A reflection has no equivalent angle, so it is carried as a
/// negative `y` scale; folding it into the angle would draw the symbol
/// facing the wrong way.
fn decompose_placement(matrix: [f64; 4]) -> (f64, [f64; 2]) {
    let [m00, m01, m10, m11] = matrix;
    let determinant = m00 * m11 - m01 * m10;
    let scale_y = m10.hypot(m11);
    (
        m01.atan2(m00),
        [
            m00.hypot(m01),
            if determinant < 0.0 { -scale_y } else { scale_y },
        ],
    )
}

impl GeometryEmitter for IgSymbol2dEmitter {
    fn family(&self) -> &'static str {
        "igSymbol2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_igsymbols.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            let (rotation, scale) = decompose_placement([
                record.transform_00,
                record.transform_01,
                record.transform_10,
                record.transform_11,
            ]);
            out.push(PidGraphicEntity {
                id: format!("{}:igsymbol2d:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::SymbolInstance {
                    insertion: PidPoint {
                        x: record.insertion_x,
                        y: record.insertion_y,
                    },
                    symbol_path: ctx
                        .jsite_symbol_paths
                        .get(&record.jsite_ref)
                        .map(|path| (*path).to_string()),
                    rotation,
                    scale,
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("igsymbol2d:{index}")),
                    record_kind: Some(SheetRecordKind::SymbolPlacement),
                    field_x: None,
                    note: Some(format!(
                        "PSM igSymbol2d record (Intergraph Sigma standard symbol instance, \
                         type 0x00CE, IGDS class tag 0xCE); oid={} parent_ref={} \
                         sub_type=0x{:04X} jsite_ref={} insertion=({:.4}, {:.4}) \
                         transform=[{:.4}, {:.4}, {:.4}, {:.4}]; byte layout from \
                         fixture dump; jsite_ref -> JSite<id> link from Phase 35-C \
                         cross-fixture probe (132/132)",
                        record.oid,
                        record.parent_ref,
                        record.sub_type_word,
                        record.jsite_ref,
                        record.insertion_x,
                        record.insertion_y,
                        record.transform_00,
                        record.transform_01,
                        record.transform_10,
                        record.transform_11,
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `Decoded` [`PidGraphicKind::Text`] entities for the
/// `igTextBox` family (PSM `0x004D`).
struct IgTextBoxEmitter;

impl GeometryEmitter for IgTextBoxEmitter {
    fn family(&self) -> &'static str {
        "igTextBox"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_igtextboxes.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            out.push(PidGraphicEntity {
                id: format!("{}:igtextbox:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Text {
                    insertion: PidPoint {
                        x: record.trailing_double_1,
                        y: record.trailing_double_2,
                    },
                    value: record.text.clone(),
                    height: 0.0,
                    rotation: record.rotation_rad,
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("igtextbox:{index}")),
                    record_kind: Some(SheetRecordKind::TextPlacementStyle),
                    field_x: None,
                    note: Some(format!(
                        "PSM igTextBox record (Intergraph Sigma standard text annotation, \
                         type 0x004D, IGDS class tag 0x4D); oid={} parent_ref={} \
                         sub_type=0x{:04X} index={} text_length={} text={:?} \
                         insertion=({:.4}, {:.4}) trailing_3={:.4}; byte layout from \
                         fixture dump",
                        record.oid,
                        record.parent_ref,
                        record.sub_type_word,
                        record.index,
                        record.text_length,
                        record.text,
                        record.trailing_double_1,
                        record.trailing_double_2,
                        record.trailing_double_3,
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `Decoded` [`PidGraphicKind::Point`] entities for the
/// `igPoint2d` family (PSM `0x005E`).
struct IgPoint2dEmitter;

impl GeometryEmitter for IgPoint2dEmitter {
    fn family(&self) -> &'static str {
        "igPoint2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_igpoints.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            out.push(PidGraphicEntity {
                id: format!("{}:igpoint2d:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Point {
                    position: PidPoint {
                        x: record.x,
                        y: record.y,
                    },
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("igpoint2d:{index}")),
                    record_kind: Some(SheetRecordKind::CoordinatePageMetadata),
                    field_x: None,
                    note: Some(format!(
                        "PSM igPoint2d record (Intergraph Sigma standard point, \
                         type 0x005E, IGDS class tag 0x5E); oid={} parent_ref={} \
                         sub_type=0x{:04X} index={} position=({:.4}, {:.4}); \
                         byte layout from fixture dump",
                        record.oid,
                        record.parent_ref,
                        record.sub_type_word,
                        record.index,
                        record.x,
                        record.y,
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `Decoded` [`PidGraphicKind::Polyline`] entities for the
/// `igLineString2d` family (PSM `0x0084`).
struct IgLineString2dEmitter;

impl GeometryEmitter for IgLineString2dEmitter {
    fn family(&self) -> &'static str {
        "igLineString2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_iglinestrings.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            if record.vertex_xs.len() != record.vertex_ys.len() {
                continue;
            }
            let points: Vec<PidPoint> = record
                .vertex_xs
                .iter()
                .zip(record.vertex_ys.iter())
                .map(|(x, y)| PidPoint { x: *x, y: *y })
                .collect();
            out.push(PidGraphicEntity {
                id: format!("{}:iglinestring2d:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Polyline {
                    points,
                    closed: false,
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("iglinestring2d:{index}")),
                    record_kind: Some(SheetRecordKind::PrimitivePolyline),
                    field_x: None,
                    note: Some(format!(
                        "PSM igLineString2d record (Intergraph Sigma standard polyline, \
                         type 0x0084, IGDS class tag 0x84); oid={} parent_ref={} \
                         sub_type=0x{:04X} index={} form={} scope={} vc={} \
                         total_length={:.4}; byte layout from fixture dump",
                        record.oid,
                        record.parent_ref,
                        record.sub_type_word,
                        record.index,
                        record.form,
                        record.scope,
                        record.vertex_count(),
                        record.total_length(),
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `Decoded` [`PidGraphicKind::Line`] entities for the standard
/// IGDS `igLine2d` family (PSM `0x0018`).
struct IgLine2dEmitter;

impl GeometryEmitter for IgLine2dEmitter {
    fn family(&self) -> &'static str {
        "igLine2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_iglines.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            out.push(PidGraphicEntity {
                id: format!("{}:igline2d:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Line {
                    start: PidPoint {
                        x: record.start_x,
                        y: record.start_y,
                    },
                    end: PidPoint {
                        x: record.end_x,
                        y: record.end_y,
                    },
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("igline2d:{index}")),
                    record_kind: Some(SheetRecordKind::PrimitiveLine),
                    field_x: None,
                    note: Some(format!(
                        "PSM igLine2d record (Intergraph Sigma standard line, type 0x0018, \
                         IGDS class tag 0x18); oid={} parent_ref={} sub_type=0x{:04X} \
                         index={} start=({:.4}, {:.4}) end=({:.4}, {:.4}) length={:.4}; \
                         byte layout from fixture dump (radsrvitem.dll-adjacent)",
                        record.oid,
                        record.parent_ref,
                        record.sub_type_word,
                        record.index,
                        record.start_x,
                        record.start_y,
                        record.end_x,
                        record.end_y,
                        record.length(),
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// Emits `ProbeOnly` [`PidGraphicKind::Unknown`] entities for the RAD
/// `JStyleOverride` family (PSM `0x0030`).
///
/// This used to emit an `Inferred` annotation anchor. Phase 16 Slice F had
/// recorded a genuine ambiguity: the IDA Version-3 schema calls the payload
/// `4 × u32 + 4 × f64 + 3 × u32 + 2 × u16`, while cross-fixture probe v5
/// evidence found that joining `+0..7` and `+8..15` into two `f64` gives
/// consistently normalized coordinates, and the anchor reading won.
///
/// The native reader settles it the other way. `style.dll`'s version-3
/// serializer makes four separate four-byte reads there, so those are four
/// fields and the normalized coordinates were four `u32` that happen to
/// spell a plausible small double. The record's real doubles are further in.
/// See `docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md`.
///
/// The record is still surfaced, because it is in the file and its byte
/// provenance is sound, but as probe evidence rather than as an annotation
/// with a position no renderer should trust.
struct JStyleOverrideEmitter;

impl GeometryEmitter for JStyleOverrideEmitter {
    fn family(&self) -> &'static str {
        "JStyleOverride"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_jstyle_overrides.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            out.push(PidGraphicEntity {
                id: format!("{}:jstyle-override:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Unknown {
                    note: format!(
                        "PSM 0x0030 JStyleOverride (RAD style.dll, CLSID \
                         {{47FCC338-2D0F-11D0-A1FF-080036A1CF02}}); style record, \
                         not drawable geometry; oid={} bytes_to_follow={} \
                         payload +0..15 = 4 x u32 [{}, {}, {}, {}]",
                        record.oid,
                        record.bytes_to_follow,
                        record.field_a_u32,
                        record.field_b_u32,
                        record.field_c_u32,
                        record.field_d_u32,
                    ),
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("jstyle-override:{index}")),
                    record_kind: Some(SheetRecordKind::JStyleOverride),
                    field_x: None,
                    note: Some(format!(
                        "Layout confirmed against style.dll's own version-3 \
                         serializer: 4 x u32, 4 x f64, 3 x u32, 2 x u16 after a \
                         26-byte base block. The earlier annotation-anchor \
                         reading of payload +0..15 is withdrawn -- the native \
                         reader makes four separate 4-byte reads there. \
                         oid={} bytes_to_follow={} field_1={:.6} field_2={:.6}",
                        record.oid, record.bytes_to_follow, record.field_1_f64, record.field_2_f64,
                    )),
                },
                confidence: PidGeometryConfidence::ProbeOnly,
            });
        }
    }
}

/// Emits `Decoded` closed [`PidGraphicKind::Polyline`] entities for the
/// `igBoundary2d` family (PSM `0x0013`).
///
/// This family was a no-op emitter for a long time, on the correct
/// observation that its segment groups re-list the geometry of the member
/// `igLine2d` records it names (60/60 forward matches cross-fixture), so
/// emitting the ring would draw the outline twice. See
/// `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`.
///
/// What that reasoning missed is the thing the member lines cannot express.
/// Every one of the corpus's 20 boundaries resolves through a
/// `JStyleOverride` to a `JStyleSimpleFill`, and every one closes into a
/// ring at `1e-9` — they are filled areas (all 3-segment arrowheads on the
/// pipelines), and a renderer given only the member lines draws them hollow.
/// See `docs/analysis/2026-08-10-fill-has-a-consumer-after-all.md`.
///
/// So the ring is emitted, and the duplication becomes the renderer's to
/// resolve: it has the boundary's own `graphic_oid` to join against
/// [`crate::style_link::fill_styles_for_file`], and should fill the ring
/// rather than stroke it, because the stroke is already on the sheet.
struct IgBoundary2dEmitter;

impl GeometryEmitter for IgBoundary2dEmitter {
    fn family(&self) -> &'static str {
        "igBoundary2d"
    }

    fn emit(&self, ctx: &EmitContext<'_>, sheet: &SheetStream, out: &mut Vec<PidGraphicEntity>) {
        let Some(geometry) = &sheet.geometry else {
            return;
        };
        for (index, record) in geometry.decoded_igboundaries.iter().enumerate() {
            let Some(byte_range) = source_range(
                record.byte_start,
                record.byte_end.saturating_sub(record.byte_start),
                sheet.size,
            ) else {
                continue;
            };
            // The segments chain end-to-start, so their starts are the ring's
            // vertices and the closing edge is implied by `closed`.
            let points: Vec<PidPoint> = record
                .segments
                .iter()
                .map(|segment| PidPoint {
                    x: segment.start_x,
                    y: segment.start_y,
                })
                .collect();
            if points.len() < 3 {
                continue;
            }
            out.push(PidGraphicEntity {
                id: format!("{}:igboundary2d:{index}", sheet.path),
                drawing_id: None,
                graphic_oid: Some(record.oid),
                kind: PidGraphicKind::Polyline {
                    points,
                    closed: true,
                },
                coordinate_context: decoded_sheet_coordinate_context(&sheet.path, ctx.page),
                source: PidGraphicProvenance {
                    stream_path: Some(sheet.path.clone()),
                    byte_range: Some(byte_range),
                    record_id: Some(format!("igboundary2d:{index}")),
                    record_kind: Some(SheetRecordKind::PrimitivePolyline),
                    field_x: None,
                    note: Some(format!(
                        "PSM igBoundary2d ring (type 0x0013); oid={} segments={} \
                         members={}. The ring re-lists its member igLine2d \
                         segments, so a renderer must fill it rather than stroke \
                         it -- the outline is already on the sheet. Join \
                         style_link::fill_styles_for_file on (stream, oid) for \
                         the fill.",
                        record.oid,
                        record.segments.len(),
                        record.member_refs.len(),
                    )),
                },
                confidence: PidGeometryConfidence::Decoded,
            });
        }
    }
}

/// No-op emitter: `igSmartFrame2d` (PSM `0x003D`) is the sheet's OLE
/// container frame. It contributes the page extent every decoded
/// coordinate is read against (see [`decoded_page_frame`]) rather than
/// drawable geometry of its own, so its emission policy is a deliberate
/// no-op — registered, per the module contract, so the policy is
/// visible in [`EMITTERS`] instead of implied by absence.
struct IgSmartFrame2dEmitter;

impl GeometryEmitter for IgSmartFrame2dEmitter {
    fn family(&self) -> &'static str {
        "igSmartFrame2d"
    }

    fn is_no_op(&self) -> bool {
        true
    }

    fn emit(&self, _ctx: &EmitContext<'_>, _sheet: &SheetStream, _out: &mut Vec<PidGraphicEntity>) {
    }
}

/// No-op emitter: `DependencyObject` (PSM `0x00FA`) is an audit-only
/// grouping record; child-OID extraction remains an audit-layer
/// hypothesis, not render geometry.
struct DependencyObjectEmitter;

impl GeometryEmitter for DependencyObjectEmitter {
    fn family(&self) -> &'static str {
        "DependencyObject"
    }

    fn is_no_op(&self) -> bool {
        true
    }

    fn emit(&self, _ctx: &EmitContext<'_>, _sheet: &SheetStream, _out: &mut Vec<PidGraphicEntity>) {
    }
}

/// No-op emitter: the polymorphic `0x0010` sub-record family is an
/// audit-only typed collection (Phase 18); sub-kind discrimination is
/// deferred pending IDA confirmation.
struct SubRecord0x0010Emitter;

impl GeometryEmitter for SubRecord0x0010Emitter {
    fn family(&self) -> &'static str {
        "SubRecord0x0010"
    }

    fn is_no_op(&self) -> bool {
        true
    }

    fn emit(&self, _ctx: &EmitContext<'_>, _sheet: &SheetStream, _out: &mut Vec<PidGraphicEntity>) {
    }
}

/// No-op emitter: attribute fragments (Phase 26 view of `0x0010`)
/// carry engineering attribute text for audit; they are not placed
/// text geometry (no decoded insertion point).
struct AttributeFragmentEmitter;

impl GeometryEmitter for AttributeFragmentEmitter {
    fn family(&self) -> &'static str {
        "AttributeFragment"
    }

    fn is_no_op(&self) -> bool {
        true
    }

    fn emit(&self, _ctx: &EmitContext<'_>, _sheet: &SheetStream, _out: &mut Vec<PidGraphicEntity>) {
    }
}

fn sheet_source_coordinate_context(sheet_path: &str) -> PidCoordinateContext {
    PidCoordinateContext {
        coordinate_space: PidCoordinateSpace::SourceSheet,
        units: unknown_sheet_units(),
        page_transform: unavailable_sheet_transform(sheet_path),
    }
}

/// Coordinate context for a decoded record's geometry.
///
/// A decoded record carries the drawing's own normalized coordinates, which a
/// border frame proves are metres on the page it states. The inferred
/// coordinate hints and endpoint pairs do not share that space -- they are
/// raw source pairs reaching the +/-900k range -- so they keep
/// [`sheet_source_coordinate_context`] and its explicit unavailable states.
fn decoded_sheet_coordinate_context(
    sheet_path: &str,
    page: Option<PageFrame>,
) -> PidCoordinateContext {
    let Some(page) = page else {
        return sheet_source_coordinate_context(sheet_path);
    };
    PidCoordinateContext {
        coordinate_space: PidCoordinateSpace::SourceSheet,
        units: PidDrawingUnits::Known {
            unit: SOURCE_UNIT_METRE.to_string(),
        },
        page_transform: page.page_transform(),
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
        DecodedJStyleOverrideRecord, SheetCoordinateHintDto, SheetEndpoint, SheetEndpointRecord,
        SheetGeometry, SheetObjectGeometryHint, SheetStream, SheetText,
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
        assert!(geometry.warnings.iter().any(|warning| warning
            .contains("geometry decode remains partial")
            && warning.contains("1 Sheet stream")));
        assert!(geometry
            .warnings
            .iter()
            .all(|warning| !warning.contains("geometry decode not yet implemented")));
    }

    #[test]
    fn jstyle_override_projection_withdraws_the_annotation_anchor() {
        // The payload words below spell two plausible normalized f64 when
        // joined pairwise -- exactly the reading style.dll's own version-3
        // serializer rules out, since it makes four separate 4-byte reads
        // there. The record must surface as probe-only evidence, never as an
        // annotation with a position a renderer would place.
        let anchor_x = 0.25_f64;
        let anchor_y = 0.5_f64;
        let anchor_x_bytes = anchor_x.to_le_bytes();
        let anchor_y_bytes = anchor_y.to_le_bytes();
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 128,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                decoded_jstyle_overrides: vec![DecodedJStyleOverrideRecord {
                    byte_start: 8,
                    byte_end: 78,
                    type_code: 0x0030,
                    type_flags: 0,
                    bytes_to_follow: 64,
                    oid: 42,
                    field_a_u32: u32::from_le_bytes(
                        anchor_x_bytes[0..4].try_into().expect("first anchor word"),
                    ),
                    field_b_u32: u32::from_le_bytes(
                        anchor_x_bytes[4..8].try_into().expect("second anchor word"),
                    ),
                    field_c_u32: u32::from_le_bytes(
                        anchor_y_bytes[0..4].try_into().expect("first anchor word"),
                    ),
                    field_d_u32: u32::from_le_bytes(
                        anchor_y_bytes[4..8].try_into().expect("second anchor word"),
                    ),
                    field_1_f64: anchor_x,
                    field_2_f64: 0.0,
                    field_3_f64: 0.0,
                    field_4_f64: 0.0,
                    field_e_u32: 0,
                    field_f_u32: 0,
                    field_g_u32: 0,
                    field_h_u16: 0,
                    field_i_u16: 0,
                    raw_attribute_tail: Vec::new(),
                }],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);
        assert!(
            !geometry
                .entities
                .iter()
                .any(|entity| matches!(entity.kind, PidGraphicKind::Annotation { .. })),
            "the withdrawn anchor reading must not come back"
        );

        let style_record = geometry
            .entities
            .iter()
            .find(|entity| entity.source.record_kind == Some(SheetRecordKind::JStyleOverride))
            .expect("JStyleOverride should still be surfaced");

        assert_eq!(style_record.confidence, PidGeometryConfidence::ProbeOnly);
        assert!(matches!(style_record.kind, PidGraphicKind::Unknown { .. }));
        assert_eq!(
            style_record.source.byte_range,
            Some(PidByteRange { start: 8, end: 78 })
        );
        assert!(style_record.source.note.as_deref().is_some_and(|note| note
            .contains("annotation-anchor reading")
            && note.contains("withdrawn")));
        assert!(geometry.warnings.iter().any(|warning| {
            warning.contains("1 Sheet evidence item")
                && warning.contains("0 decoded, 0 inferred, 1 probe-only")
        }));
    }

    #[test]
    fn audit_only_families_register_no_op_emitters() {
        // M2 seam policy test: igBoundary2d / DependencyObject / 0x0010 /
        // attribute fragments are registered in EMITTERS as explicit
        // no-ops — decoded records must produce zero entities.
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 4096,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                decoded_igboundaries: vec![crate::model::DecodedIgBoundary2dRecord {
                    byte_start: 16,
                    byte_end: 194,
                    type_code: 0x0013,
                    type_flags: 0,
                    bytes_to_follow: 172,
                    oid: 900,
                    parent_ref: 6,
                    sub_type_word: 0x0010,
                    index: 1,
                    segment_count: 1,
                    sub_header_tail: [2, 1],
                    segments: vec![crate::model::DecodedIgBoundary2dSegment {
                        tag_offset: 28,
                        start_x: 0.1,
                        start_y: 0.1,
                        end_x: 0.4,
                        end_y: 0.1,
                    }],
                    anchor_x: 0.25,
                    anchor_y: 0.1,
                    trailer_flag: 1,
                    member_refs: vec![crate::model::DecodedIgBoundary2dMemberRef {
                        member_oid: 901,
                        class_word: 0x00CB,
                        sub_word: 13,
                    }],
                    closed_loop: false,
                }],
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: vec![crate::model::DecodedDependencyObjectRecord {
                    byte_start: 200,
                    byte_end: 250,
                    type_code: 0x00FA,
                    type_flags: 0,
                    bytes_to_follow: 44,
                    oid: 77,
                    parent_ref: 6,
                    group_kind_word: 2,
                    sub_type_word: 1,
                    raw_reference_payload: vec![0u8; 26],
                }],
                decoded_sub_records_0x0010: vec![crate::model::DecodedSubRecord0x0010Record {
                    byte_start: 260,
                    byte_end: 280,
                    type_code: 0x0010,
                    type_flags: 0,
                    bytes_to_follow: 14,
                    raw_payload: vec![0u8; 14],
                    leading_word: Some(0x0002),
                }],
                decoded_attribute_fragments: vec![crate::model::DecodedAttributeFragment {
                    byte_start: 300,
                    byte_end: 330,
                    type_code: 0x0010,
                    marker: 0x0001_0002,
                    aux: vec![0u8; 8],
                    strings: Vec::new(),
                }],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert!(
            geometry.entities.is_empty(),
            "audit-only families must not emit PidGraphicEntity values, got {:?}",
            geometry
                .entities
                .iter()
                .map(|entity| &entity.id)
                .collect::<Vec<_>>()
        );
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
                decoded_primitive_lines: Vec::new(),
                decoded_iglines: Vec::new(),
                decoded_iglinestrings: Vec::new(),
                decoded_igpoints: Vec::new(),
                decoded_igtextboxes: Vec::new(),
                decoded_igsymbols: Vec::new(),
                decoded_igboundaries: Vec::new(),
                decoded_igsmartframes: Vec::new(),
                decoded_dependency_objects: Vec::new(),
                decoded_jstyle_overrides: Vec::new(),
                decoded_sub_records_0x0010: Vec::new(),
                decoded_attribute_fragments: Vec::new(),
                spatial_analysis: None,
                undecoded_type_codes: vec![],
                refused_records: vec![],
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
    fn default_coordinate_context_keeps_page_transform_unavailable_until_promoted() {
        let context = PidCoordinateContext::default();
        let value = serde_json::to_value(&context).expect("coordinate context JSON");

        assert_eq!(context.coordinate_space, PidCoordinateSpace::Unknown);
        assert!(matches!(
            context.page_transform,
            PidPageTransform::Unavailable { ref diagnostic }
                if diagnostic.contains("metadata is unavailable")
                    && diagnostic.contains("source coordinates are preserved")
        ));
        assert_eq!(value["page_transform"]["state"], "unavailable");
        assert!(
            value["page_transform"]["diagnostic"]
                .as_str()
                .is_some_and(|diagnostic| diagnostic.contains("metadata is unavailable")),
            "default coordinate context should serialize an explicit unavailable transform"
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

        assert_eq!(
            PidGraphicKind::Arc {
                center: PidPoint { x: 2.0, y: 3.0 },
                radius: 4.0,
                start_angle: 0.0,
                end_angle: 1.0,
            }
            .decoded_sheet_record_kind(),
            None,
            "Phase 17 removed the only decoded Sheet arc source; generic Arc remains available but has no current SheetRecordKind"
        );
    }

    #[test]
    fn decoded_geometry_json_exposes_record_id_and_typed_kind() {
        let geometry = NormalizedPidGeometry {
            page_dimensions_mm: None,
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
            dropped_graphic_records: Vec::new(),
            refused_graphic_records: Vec::new(),
        };

        let value = serde_json::to_value(&geometry).expect("geometry JSON");
        let source = &value["entities"][0]["source"];
        assert_eq!(source["record_id"], "sheet.primitive.line:0");
        assert_eq!(source["record_kind"], "primitive_line");
    }

    #[test]
    fn undecoded_graphic_census_entries_become_named_warnings() {
        // Phase 38 S2: a sheet carrying graphic-class records nothing
        // decodes must say so by name; the constraint families the native
        // predicate rejects must stay quiet.
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 512,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                undecoded_type_codes: vec![
                    crate::model::SheetUndecodedTypeCode {
                        type_code: 0x0115,
                        count: 3,
                        is_graphic: true,
                        rad_class_name: Some("igDimension".into()),
                    },
                    crate::model::SheetUndecodedTypeCode {
                        type_code: 0x0077,
                        count: 5,
                        is_graphic: false,
                        rad_class_name: Some("Fix Constraint".into()),
                    },
                ],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.dropped_graphic_records.len(), 1);
        let dropped = &geometry.dropped_graphic_records[0];
        assert_eq!(dropped.type_code, 0x0115);
        assert_eq!(dropped.count, 3);
        assert_eq!(dropped.stream_path, "/Sheet6");
        assert_eq!(dropped.rad_class_name.as_deref(), Some("igDimension"));
        assert!(
            geometry.warnings.iter().any(|warning| {
                warning.contains("0x0115")
                    && warning.contains("igDimension")
                    && warning.contains("3 record(s)")
                    && warning.contains("/Sheet6")
            }),
            "graphic-class drops must be named: {:?}",
            geometry.warnings
        );
        assert!(
            geometry
                .warnings
                .iter()
                .all(|warning| !warning.contains("0x0077")),
            "non-graphic constraint families must stay quiet: {:?}",
            geometry.warnings
        );
    }

    #[test]
    fn refused_graphic_records_become_named_warnings_too() {
        // Phase 40: a record its own family's decoder walked over and
        // refused is as absent from the drawing as one with no decoder at
        // all, and until now only the second kind was ever mentioned. The
        // corpus's 88 refused igLine2d records outnumber every named drop
        // in it by more than an order of magnitude.
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 512,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                refused_records: vec![
                    crate::model::SheetRefusedRecord {
                        type_code: 0x0018,
                        count: 80,
                        is_graphic: true,
                        rad_class_name: Some("Line Object".into()),
                    },
                    crate::model::SheetRefusedRecord {
                        type_code: 0x0030,
                        count: 2,
                        is_graphic: false,
                        rad_class_name: Some("JSL Override Style".into()),
                    },
                ],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        assert_eq!(geometry.refused_graphic_records.len(), 1);
        let refused = &geometry.refused_graphic_records[0];
        assert_eq!(refused.type_code, 0x0018);
        assert_eq!(refused.count, 80);
        assert_eq!(refused.stream_path, "/Sheet6");
        assert_eq!(refused.rad_class_name.as_deref(), Some("Line Object"));
        assert!(
            geometry.warnings.iter().any(|warning| {
                warning.contains("0x0018")
                    && warning.contains("Line Object")
                    && warning.contains("80 record(s)")
                    && warning.contains("refuses")
            }),
            "a refused shape must be named, and named as a refusal: {:?}",
            geometry.warnings
        );
        assert!(
            geometry
                .warnings
                .iter()
                .all(|warning| !warning.contains("0x0030")),
            "a refused style record costs no strokes and stays quiet: {:?}",
            geometry.warnings
        );
    }

    #[test]
    fn a_refusal_reads_differently_from_a_missing_decoder() {
        // The two diagnostics ask for opposite work -- write a decoder vs
        // re-measure one that exists -- so a reader must be able to tell
        // them apart without knowing the type codes.
        let mut doc = PidDocument::default();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 512,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                undecoded_type_codes: vec![crate::model::SheetUndecodedTypeCode {
                    type_code: 0x0020,
                    count: 1,
                    is_graphic: true,
                    rad_class_name: Some("Rectangle Object".into()),
                }],
                refused_records: vec![crate::model::SheetRefusedRecord {
                    type_code: 0x0018,
                    count: 4,
                    is_graphic: true,
                    rad_class_name: Some("Line Object".into()),
                }],
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });

        let geometry = build_normalized_geometry(&doc);

        let no_decoder = geometry
            .warnings
            .iter()
            .find(|warning| warning.contains("0x0020"))
            .expect("the undecoded record is named");
        let refused = geometry
            .warnings
            .iter()
            .find(|warning| warning.contains("0x0018"))
            .expect("the refused record is named");
        assert!(no_decoder.contains("have no decoder"));
        assert!(!no_decoder.contains("refuses"));
        assert!(refused.contains("refuses"));
        assert!(!refused.contains("have no decoder"));
    }

    fn border_frame(
        width_m: f64,
        height_m: f64,
        state: crate::model::SmartFrame2dState,
    ) -> crate::model::DecodedIgSmartFrame2dRecord {
        crate::model::DecodedIgSmartFrame2dRecord {
            byte_start: 0,
            byte_end: 162,
            type_code: 0x003D,
            type_flags: 0,
            bytes_to_follow: 156,
            oid: 42,
            parent_ref: 6,
            content_flags: 0x5c80_8011,
            link_flags: 0x20e9_0040,
            state,
            extent_width_m: width_m,
            extent_height_m: height_m,
            aspect_ratio: height_m / width_m,
        }
    }

    fn doc_with_border_frames(
        frames: Vec<crate::model::DecodedIgSmartFrame2dRecord>,
        template: Option<&str>,
    ) -> PidDocument {
        let mut doc = PidDocument::default();
        if let Some(template) = template {
            let mut meta = crate::model::DrawingMeta::default();
            meta.tags.insert("Template".into(), template.into());
            doc.drawing_meta = Some(meta);
        }
        doc.sheet_streams.push(SheetStream {
            name: "Sheet6".into(),
            path: "/Sheet6".into(),
            size: 512,
            extracted_texts: Vec::new(),
            magic_u32_le: None,
            magic_tag: None,
            header: None,
            attribute_records: Vec::new(),
            probe_summary: None,
            geometry: Some(SheetGeometry {
                decoded_igsmartframes: frames,
                ..SheetGeometry::default()
            }),
            endpoint_records: Vec::new(),
            endpoint_decode_error: None,
        });
        doc
    }

    /// The frame is a measurement and the template name is a name, so the
    /// frame wins where both are present: `XIONGANA2.pid` says "A2", the
    /// frame says 594.3mm, and an A2 is nominally 594.0mm.
    #[test]
    fn a_border_frame_states_the_page_over_the_template_name() {
        let doc = doc_with_border_frames(
            vec![border_frame(
                0.594_305,
                0.420_314,
                crate::model::SmartFrame2dState::Linked,
            )],
            Some("XIONGANA2.pid"),
        );

        let (width_mm, height_mm) = build_normalized_geometry(&doc)
            .page_dimensions_mm
            .expect("a drawing with one border frame states its page");
        assert!(
            (width_mm - 594.305).abs() < 1.0e-6 && (height_mm - 420.314).abs() < 1.0e-6,
            "expected the frame's measured page, got {width_mm} x {height_mm}"
        );
    }

    /// `DWG-0201` repeats its frame six times in `/Sheet6` with a
    /// bit-identical extent. Six copies of one page are still one page.
    #[test]
    fn repeated_border_frames_of_one_extent_are_one_page() {
        let frames = (0..6)
            .map(|_| {
                border_frame(
                    0.594_305,
                    0.420_314,
                    crate::model::SmartFrame2dState::Linked,
                )
            })
            .collect();

        let dimensions = build_normalized_geometry(&doc_with_border_frames(frames, None))
            .page_dimensions_mm
            .expect("six identical frames describe one page");
        assert!((dimensions.0 - 594.305).abs() < 1.0e-6);
    }

    /// Two frames that disagree are two claims about the page. Choosing
    /// between them would be a guess, so the template name decides instead.
    #[test]
    fn border_frames_that_disagree_fall_back_to_the_template_name() {
        let doc = doc_with_border_frames(
            vec![
                border_frame(
                    0.594_305,
                    0.420_314,
                    crate::model::SmartFrame2dState::Linked,
                ),
                border_frame(0.841, 0.594, crate::model::SmartFrame2dState::Linked),
            ],
            Some("XIONGANA2.pid"),
        );

        assert_eq!(
            build_normalized_geometry(&doc).page_dimensions_mm,
            Some((594.0, 420.0)),
            "disagreeing frames must not be picked between"
        );
    }

    /// The nested-site frame `A01` carries is a micrometre square in a
    /// `/JSite*/Sheet*` stream, which is a placeholder rather than a border.
    /// It must not count as a second, disagreeing page.
    #[test]
    fn a_locally_linked_frame_is_not_a_page() {
        let doc = doc_with_border_frames(
            vec![
                border_frame(
                    1.0e-6,
                    1.0e-6,
                    crate::model::SmartFrame2dState::LocallyLinked,
                ),
                border_frame(0.594, 0.420, crate::model::SmartFrame2dState::Embedded),
            ],
            None,
        );

        assert_eq!(
            build_normalized_geometry(&doc).page_dimensions_mm,
            Some((594.0, 420.0))
        );
    }

    /// With a page in hand, a decoded record's coordinates are metres on it.
    #[test]
    fn a_page_frame_makes_decoded_coordinates_metres_on_that_page() {
        let page = PageFrame {
            width_m: 0.594,
            height_m: 0.42,
        };

        let context = decoded_sheet_coordinate_context("/Sheet6", Some(page));

        assert_eq!(
            context.units,
            PidDrawingUnits::Known { unit: "m".into() },
            "a frame states its extent in metres"
        );
        let PidPageTransform::Available {
            origin,
            scale,
            page_bounds,
            matrix,
        } = context.page_transform
        else {
            panic!("a decoded page frame makes the transform available");
        };
        assert_eq!(origin, PidPoint { x: 0.0, y: 0.0 });
        assert_eq!(scale, [1.0, 1.0]);
        assert_eq!(page_bounds.min, PidPoint { x: 0.0, y: 0.0 });
        assert_eq!(page_bounds.max, PidPoint { x: 0.594, y: 0.42 });
        assert_eq!(matrix, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    /// Without a frame nothing is promoted, page-size guess or not.
    #[test]
    fn without_a_border_frame_decoded_coordinates_stay_unconverted() {
        let context = decoded_sheet_coordinate_context("/Sheet6", None);

        assert!(matches!(context.units, PidDrawingUnits::Unknown { .. }));
        assert!(matches!(
            context.page_transform,
            PidPageTransform::Unavailable { .. }
        ));
    }
}
