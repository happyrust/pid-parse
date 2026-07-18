//! Sheet-level decoded DTO surface: `SheetGeometry`, the per-family
//! `Decoded*Record` mirrors of the parser-layer `Sheet*Decoded` DTOs
//! (with their `From` bridges), and the sheet text / endpoint /
//! coordinate-hint DTOs.
//!
//! Split out of the root model module (M2-PR14, RFC Phase 4) so each
//! new PSM record family lands its model DTO in one focused submodule.
//! Everything is re-exported from [`crate::model`], so public paths
//! are unchanged.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stable Sheet-level DTO that groups normalized text, endpoint and
/// coordinate evidence without claiming full CAD geometry decode.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SheetGeometry {
    /// Text runs normalized from the Sheet probe layer.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub texts: Vec<SheetText>,
    /// Endpoint records normalized from the Sheet endpoint parser.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub endpoints: Vec<SheetEndpoint>,
    /// Coordinate-like pairs retained as hints until record semantics
    /// are proven across more fixtures.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub coordinate_hints: Vec<SheetCoordinateHintDto>,
    /// Future object-to-geometry mapping evidence. Empty until a Sheet
    /// probe can prove that an object `field_x` owns source-backed
    /// geometry coordinates.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub object_geometry_hints: Vec<SheetObjectGeometryHint>,
    /// PSM-decoded `GLine2d` primitive line records emitted by
    /// [`crate::parsers::sheet_records::decode_primitive_lines`]
    /// for this sheet's raw stream bytes. Each entry is a stable
    /// model-shaped projection of the parser-level
    /// `SheetPrimitiveLineDecoded` DTO with full provenance
    /// (`byte_range` + PSM `type_code` + parametric geometry).
    /// Producing
    /// [`crate::geometry::PidGeometryConfidence::Decoded`]
    /// `PidGraphicKind::Line` entities is the responsibility of
    /// [`crate::geometry::build_normalized_geometry`], which
    /// converts these records on-demand.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_primitive_lines: Vec<DecodedPrimitiveLineRecord>,
    /// PSM-decoded `igLine2d` records (PSM type `0x0018`, IGDS
    /// class tag `0x18`) — Intergraph Sigma's standard 2D line
    /// primitive, emitted by
    /// [`crate::parsers::sheet_records::decode_iglines`]. By far
    /// the most common line representation in real `SmartPlant`
    /// fixtures (Phase 14 Slice J).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_iglines: Vec<DecodedIgLine2dRecord>,
    /// PSM-decoded `igLineString2d` records (PSM type `0x0084`,
    /// IGDS class tag `0x84`) — Intergraph Sigma's standard 2D
    /// polyline primitive, emitted by
    /// [`crate::parsers::sheet_records::decode_iglinestrings`]
    /// (Phase 14 Slice K).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_iglinestrings: Vec<DecodedIgLineString2dRecord>,
    /// PSM-decoded `igPoint2d` records (PSM type `0x005E`, IGDS
    /// class tag `0x5E`) — Intergraph Sigma's standard 2D point
    /// primitive, emitted by
    /// [`crate::parsers::sheet_records::decode_igpoints`]
    /// (Phase 14 Slice L).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igpoints: Vec<DecodedIgPoint2dRecord>,
    /// PSM-decoded `igTextBox` records (PSM type `0x004D`, IGDS
    /// class tag `0x4D`) — Intergraph Sigma's standard text
    /// annotation primitive, emitted by
    /// [`crate::parsers::sheet_records::decode_igtextboxes`]
    /// (Phase 14 Slice M).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igtextboxes: Vec<DecodedIgTextBoxRecord>,
    /// PSM-decoded `igSymbol2d` records (PSM type `0x00CE`, IGDS
    /// class tag `0xCE`) — Intergraph Sigma's standard symbol
    /// instance primitive (`SmartPlant` equipment / instrument /
    /// valve placements), emitted by
    /// [`crate::parsers::sheet_records::decode_igsymbols`]
    /// (Phase 14 Slice N).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igsymbols: Vec<DecodedIgSymbol2dRecord>,
    /// Fully-typed **audit-only** PSM `igBoundary2d` records (PSM type
    /// `0x0013`, IGDS class tag `0x13`) emitted by
    /// [`crate::parsers::sheet_records::decode_igboundaries`]
    /// (Phase 34-D).
    ///
    /// Every payload byte is field-named (segment groups, anchor,
    /// trailer member references), but these records intentionally do
    /// not produce normalized geometry entities: the trailer member
    /// references prove the segment coordinates duplicate member
    /// `igLine2d` geometry that already emits normalized `Line`
    /// entities, so emitting the boundary as a polyline would
    /// double-count geometry. See
    /// `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igboundaries: Vec<DecodedIgBoundary2dRecord>,
    /// Audit-only PSM `0x00FA` `GraphicGroup` / `GraphicPersist` records
    /// emitted by
    /// [`crate::parsers::sheet_records::decode_graphic_groups`].
    ///
    /// These records preserve the stable envelope and raw reference
    /// payload for inspection. They intentionally do not produce
    /// normalized geometry entities and do not expose a stable
    /// `child_oids` field until the variable tail is proven across
    /// size/sub-type buckets.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_graphic_groups: Vec<DecodedGraphicGroupRecord>,
    /// PSM-decoded `JStyleOverride` records (PSM type `0x0030`,
    /// RAD `style.dll` CLSID `{47FCC338-...}`) emitted by
    /// [`crate::parsers::sheet_records::decode_jstyle_overrides`].
    /// Authoritative PSM `0x0030` collection. Phase 17 removed the
    /// historical Phase 14 `PrimitiveArc` compatibility field; new
    /// consumers should use this field for `0x0030` records. See
    /// `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_jstyle_overrides: Vec<DecodedJStyleOverrideRecord>,
    /// Audit-only PSM `0x0010` sub-record records emitted by
    /// [`crate::parsers::sheet_records::decode_sub_records_0x0010`]
    /// (Phase 18). The 6-byte PSM header is exposed verbatim and the
    /// variable payload is preserved as raw bytes; no sub-kind
    /// discrimination is attempted because IDA reverse engineering
    /// has not yet pinned the real class identity. Some entries may
    /// be embedded fragments inside larger parent records rather than
    /// standalone 0x0010 records. These records intentionally do not
    /// produce normalized geometry entities.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_sub_records_0x0010: Vec<DecodedSubRecord0x0010Record>,
    /// Phase 26 additive, audit-only attribute-fragment view of PSM
    /// `0x0010` records, emitted by
    /// [`crate::parsers::sheet_records::decode_attribute_fragments`].
    /// Decodes the `marker(4) + aux(8) + [u16 len + UTF-16LE]*`
    /// structure to extract `SmartPlant` attribute text (instrument tags,
    /// line numbers, nominal sizes, drawing references, annotation
    /// labels). Coexists with [`Self::decoded_sub_records_0x0010`] (raw,
    /// unchanged) and produces no normalized geometry entity. See
    /// `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_attribute_fragments: Vec<DecodedAttributeFragment>,
    /// Phase 25-A read-only spatial-distribution analysis of this
    /// sheet's normalized `(x, y)` f64 pairs, emitted by
    /// [`crate::parsers::sheet_records::coordinate_pair_spatial_analysis`].
    /// `None` when the sheet carries no normalized f64 pairs.
    ///
    /// This is **investigation-only evidence**: cluster ids are
    /// sheet-local topology hints, never coordinate authority. It does
    /// not promote any entity and does not change
    /// [`crate::geometry::PidPageTransform`] state.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub spatial_analysis: Option<DecodedSpatialAnalysis>,
}

/// Stable, model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetSpatialAnalysisReport`]
/// (Phase 25-A read-only spatial distribution analysis).
///
/// Read-only evidence: it records how a sheet's normalized `(x, y)`
/// f64 pairs cluster in `[0, 1]²` space. It carries no coordinate
/// authority and never promotes an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct DecodedSpatialAnalysis {
    /// Total normalized f64 pairs that contributed to the analysis.
    pub pair_count: usize,
    /// Grid resolution (N×N) used for bucketing.
    pub grid_resolution: usize,
    /// True when the analysis cannot separate clusters (zero pairs,
    /// a single cluster, or all pairs in one grid cell).
    pub uniform_distribution: bool,
    /// Connected-component clusters, in deterministic discovery order.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub clusters: Vec<DecodedSpatialCluster>,
}

/// Stable, model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetSpatialCluster`].
///
/// Cluster ids are sheet-local (0-based, deterministic) and must not
/// be compared across sheets or fixtures. The bounding box and
/// centroid live in normalized `[0, 1]²` space.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct DecodedSpatialCluster {
    /// Sheet-local cluster id (0-based, deterministic for the same input).
    pub id: u32,
    /// Number of normalized f64 pairs that fell into this cluster.
    pub pair_count: usize,
    /// Bounding-box minimum X in normalized `[0, 1]` space.
    pub min_x: f64,
    /// Bounding-box minimum Y in normalized `[0, 1]` space.
    pub min_y: f64,
    /// Bounding-box maximum X in normalized `[0, 1]` space.
    pub max_x: f64,
    /// Bounding-box maximum Y in normalized `[0, 1]` space.
    pub max_y: f64,
    /// Centroid X (arithmetic mean) in normalized `[0, 1]` space.
    pub centroid_x: f64,
    /// Centroid Y (arithmetic mean) in normalized `[0, 1]` space.
    pub centroid_y: f64,
}

impl From<crate::parsers::sheet_records::SheetSpatialCluster> for DecodedSpatialCluster {
    fn from(cluster: crate::parsers::sheet_records::SheetSpatialCluster) -> Self {
        let ((min_x, min_y), (max_x, max_y)) = cluster.bbox;
        let (centroid_x, centroid_y) = cluster.centroid;
        Self {
            id: cluster.id,
            pair_count: cluster.pair_count,
            min_x,
            min_y,
            max_x,
            max_y,
            centroid_x,
            centroid_y,
        }
    }
}

impl From<crate::parsers::sheet_records::SheetSpatialAnalysisReport> for DecodedSpatialAnalysis {
    fn from(report: crate::parsers::sheet_records::SheetSpatialAnalysisReport) -> Self {
        Self {
            pair_count: report.pair_count,
            grid_resolution: report.grid_resolution,
            uniform_distribution: report.uniform_distribution,
            clusters: report
                .clusters
                .into_iter()
                .map(DecodedSpatialCluster::from)
                .collect(),
        }
    }
}

/// Stable, model-shaped DTO that mirrors
/// [`crate::parsers::sheet_records::SheetPrimitiveLineDecoded`].
///
/// The parser-level DTO uses [`std::ops::Range<usize>`] for byte
/// ranges, but the model surface keeps `byte_start` / `byte_end`
/// as explicit `usize`s to stay aligned with the rest of the
/// stable `Sheet*` DTO family (and to keep `JsonSchema` output
/// compact). Conversion is via
/// [`From<crate::parsers::sheet_records::SheetPrimitiveLineDecoded>`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedPrimitiveLineRecord {
    /// Inclusive byte-range start covering the **full** PSM record
    /// (header + 48-byte `GLine2d` payload + any trailing attribute
    /// bytes captured by `bytes_to_follow`).
    pub byte_start: usize,
    /// Exclusive byte-range end (`byte_start + 6 + bytes_to_follow`).
    pub byte_end: usize,
    /// PSM 14-bit type code from the record header. Always
    /// `0x3FE6` (the `GLine2d` PSM type code) for records this
    /// decoder emits.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` field from the PSM header.
    pub bytes_to_follow: u32,
    /// `oid` field from the PSM header (`SmartPlant`'s object identifier).
    pub oid: u32,
    /// Local-space origin `x`. `point(t) = origin + t * direction`.
    pub origin_x: f64,
    /// Local-space origin `y`.
    pub origin_y: f64,
    /// Unit direction vector `x` component.
    pub direction_x: f64,
    /// Unit direction vector `y` component.
    pub direction_y: f64,
    /// Parameter range start `t`. `param_start < param_end` is
    /// guaranteed at decode time.
    pub param_start: f64,
    /// Parameter range end `t`.
    pub param_end: f64,
}

impl From<crate::parsers::sheet_records::SheetPrimitiveLineDecoded> for DecodedPrimitiveLineRecord {
    fn from(d: crate::parsers::sheet_records::SheetPrimitiveLineDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            origin_x: d.origin.0,
            origin_y: d.origin.1,
            direction_x: d.direction.0,
            direction_y: d.direction.1,
            param_start: d.param_start,
            param_end: d.param_end,
        }
    }
}

impl DecodedPrimitiveLineRecord {
    /// Cartesian endpoint A computed from the parametric form
    /// (`origin + param_start * direction`).
    pub fn endpoint_a(&self) -> (f64, f64) {
        (
            self.origin_x + self.param_start * self.direction_x,
            self.origin_y + self.param_start * self.direction_y,
        )
    }

    /// Cartesian endpoint B computed from the parametric form
    /// (`origin + param_end * direction`).
    pub fn endpoint_b(&self) -> (f64, f64) {
        (
            self.origin_x + self.param_end * self.direction_x,
            self.origin_y + self.param_end * self.direction_y,
        )
    }
}

/// Stable, model-shaped DTO that mirrors
/// [`crate::parsers::sheet_records::SheetIgLine2dDecoded`]
/// — PSM type `0x0018` Intergraph Sigma 2D standard line.
///
/// Layout: PSM 6-byte header + 50-byte payload. Payload fields are
/// `oid` (u32), `parent_ref` (u32), `remaining_header` (u32 = 12,
/// validated), `sub_type_word` (u16), `index` (u32), then four
/// `f64` LE for `start.x`, `start.y`, `end.x`, `end.y`. See
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
/// section "igLine2d 字节布局已揭示" for the full layout and
/// fixture-verified evidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgLine2dRecord {
    /// Inclusive byte-range start covering the full PSM record.
    pub byte_start: usize,
    /// Exclusive byte-range end (`= byte_start + 6 + 50 = byte_start + 56`).
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0018` for records this
    /// decoder emits.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header; always 50 for valid
    /// records.
    pub bytes_to_follow: u32,
    /// Object identifier (payload bytes 0..3).
    pub oid: u32,
    /// Parent reference (payload bytes 4..7).
    pub parent_ref: u32,
    /// Sub-type discriminator (payload bytes 12..13).
    pub sub_type_word: u16,
    /// Index / sub-oid (payload bytes 14..17).
    pub index: u32,
    /// Start point `x` (payload offset 18).
    pub start_x: f64,
    /// Start point `y` (payload offset 26).
    pub start_y: f64,
    /// End point `x` (payload offset 34).
    pub end_x: f64,
    /// End point `y` (payload offset 42).
    pub end_y: f64,
}

impl From<crate::parsers::sheet_records::SheetIgLine2dDecoded> for DecodedIgLine2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgLine2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            start_x: d.start.0,
            start_y: d.start.1,
            end_x: d.end.0,
            end_y: d.end.1,
        }
    }
}

impl DecodedIgLine2dRecord {
    /// Length of the line segment.
    pub fn length(&self) -> f64 {
        let dx = self.end_x - self.start_x;
        let dy = self.end_y - self.start_y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Stable, model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgLineString2dDecoded`]
/// — PSM type `0x0084` polyline. See
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
/// for the full byte layout.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgLineString2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0084`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// `form` byte from the payload (0..=6).
    pub form: u8,
    /// `scope` byte from the payload (0..=4 or `== 6`).
    pub scope: u8,
    /// Polyline vertex `x` coordinates in source order.
    pub vertex_xs: Vec<f64>,
    /// Polyline vertex `y` coordinates in source order.
    pub vertex_ys: Vec<f64>,
}

impl From<crate::parsers::sheet_records::SheetIgLineString2dDecoded>
    for DecodedIgLineString2dRecord
{
    fn from(d: crate::parsers::sheet_records::SheetIgLineString2dDecoded) -> Self {
        let (vertex_xs, vertex_ys) = d.vertices.into_iter().unzip();
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            form: d.form,
            scope: d.scope,
            vertex_xs,
            vertex_ys,
        }
    }
}

impl DecodedIgLineString2dRecord {
    /// Number of vertices in this polyline.
    pub fn vertex_count(&self) -> usize {
        self.vertex_xs.len()
    }

    /// Cumulative polyline length.
    pub fn total_length(&self) -> f64 {
        let mut total = 0.0;
        for i in 1..self.vertex_xs.len() {
            let dx = self.vertex_xs[i] - self.vertex_xs[i - 1];
            let dy = self.vertex_ys[i] - self.vertex_ys[i - 1];
            total += (dx * dx + dy * dy).sqrt();
        }
        total
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgPoint2dDecoded`] —
/// PSM type `0x005E` Intergraph Sigma standard 2D point.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgPoint2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x005E`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header (always 34).
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// Point `x` coordinate.
    pub x: f64,
    /// Point `y` coordinate.
    pub y: f64,
}

impl From<crate::parsers::sheet_records::SheetIgPoint2dDecoded> for DecodedIgPoint2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgPoint2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            x: d.point.0,
            y: d.point.1,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgTextBoxDecoded`] —
/// PSM type `0x004D` Intergraph Sigma standard text annotation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgTextBoxRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x004D`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// Inline text length (UTF-16LE chars).
    pub text_length: u16,
    /// Decoded text (lossy UTF-16LE → UTF-8).
    pub text: String,
    /// First trailing f64 (insertion.x).
    pub trailing_double_1: f64,
    /// Second trailing f64 (insertion.y).
    pub trailing_double_2: f64,
    /// Third trailing f64 (often `1.0`, possibly scale).
    pub trailing_double_3: f64,
}

impl From<crate::parsers::sheet_records::SheetIgTextBoxDecoded> for DecodedIgTextBoxRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgTextBoxDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            text_length: d.text_length,
            text: d.text,
            trailing_double_1: d.trailing_double_1,
            trailing_double_2: d.trailing_double_2,
            trailing_double_3: d.trailing_double_3,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgSymbol2dDecoded`] —
/// PSM type `0x00CE` `SmartPlant` symbol instance.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgSymbol2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x00CE`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// First element of the 2×2 transform matrix at payload
    /// offsets 40..47 (often `cos(rotation) * scale_x`).
    pub transform_00: f64,
    /// Second element of the transform matrix.
    pub transform_01: f64,
    /// Third element of the transform matrix.
    pub transform_10: f64,
    /// Fourth element of the transform matrix.
    pub transform_11: f64,
    /// Insertion point `x`.
    pub insertion_x: f64,
    /// Insertion point `y`.
    pub insertion_y: f64,
}

impl From<crate::parsers::sheet_records::SheetIgSymbol2dDecoded> for DecodedIgSymbol2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgSymbol2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            transform_00: d.transform[0],
            transform_01: d.transform[1],
            transform_10: d.transform[2],
            transform_11: d.transform[3],
            insertion_x: d.insertion.0,
            insertion_y: d.insertion.1,
        }
    }
}

/// Audit-only model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetGraphicGroupDecoded`] —
/// PSM type `0x00FA` `GraphicGroup` / `GraphicPersist` records.
///
/// Only the stable 18-byte payload prefix and raw variable tail are
/// exposed. Candidate child references remain probe/audit evidence and
/// are not represented as a stable field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedGraphicGroupRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x00FA`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Graphic group object identifier.
    pub oid: u32,
    /// Parent reference. Current fixtures consistently use `6`.
    pub parent_ref: u32,
    /// Small kind/count-like word at payload offsets 14..15.
    pub group_kind_word: u16,
    /// Sub-type / version-like discriminator at payload offsets 16..17.
    pub sub_type_word: u16,
    /// Raw variable tail from payload offset 18 onward.
    pub raw_reference_payload: Vec<u8>,
}

impl From<crate::parsers::sheet_records::SheetGraphicGroupDecoded> for DecodedGraphicGroupRecord {
    fn from(d: crate::parsers::sheet_records::SheetGraphicGroupDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            group_kind_word: d.group_kind_word,
            sub_type_word: d.sub_type_word,
            raw_reference_payload: d.raw_reference_payload,
        }
    }
}

/// One `(start, end)` segment of a decoded `igBoundary2d` record
/// (Phase 34-D). Model-shaped mirror of
/// [`crate::parsers::sheet_records::SheetIgBoundary2dSegment`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgBoundary2dSegment {
    /// Payload offset of the segment group's `0x67` tag byte.
    pub tag_offset: usize,
    /// Segment start X in normalized sheet coordinates.
    pub start_x: f64,
    /// Segment start Y in normalized sheet coordinates.
    pub start_y: f64,
    /// Segment end X in normalized sheet coordinates.
    pub end_x: f64,
    /// Segment end Y in normalized sheet coordinates.
    pub end_y: f64,
}

/// One 8-byte trailer member reference of a decoded `igBoundary2d`
/// record (Phase 34-D). Model-shaped mirror of
/// [`crate::parsers::sheet_records::SheetIgBoundary2dMemberRef`].
///
/// Fixture evidence (60/60 members): `member_oid` resolves to a real
/// `0x0018 igLine2d` record in the same Sheet stream whose
/// `(start, end)` equals the same-index segment in forward order;
/// `class_word` is always `0x00CB`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedIgBoundary2dMemberRef {
    /// Object identifier of the referenced member record.
    pub member_oid: u32,
    /// Class-like word at member offset +4 (`0x00CB` in fixtures).
    pub class_word: u16,
    /// Sub-code word at member offset +6 (`13` / `12` in fixtures).
    pub sub_word: u16,
}

/// Fully-typed **audit-only** model DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgBoundary2dDecoded`] —
/// PSM type `0x0013` `igBoundary2d` (Phase 34-D).
///
/// The record is an *association*: its segment coordinates re-list
/// geometry owned by the member `igLine2d` records named in
/// [`Self::member_refs`], so no normalized geometry entity is emitted
/// (that would double-count the member lines). `closed_loop` is
/// computed at decode time with a `1e-9` per-axis tolerance.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgBoundary2dRecord {
    /// Inclusive byte-range start covering the full PSM record.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0013`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header
    /// (`49 + 41 × segment_count` by validation).
    pub bytes_to_follow: u32,
    /// Boundary object identifier.
    pub oid: u32,
    /// Parent reference (varies per record; not validated).
    pub parent_ref: u32,
    /// Sub-type word at payload offset 12. Always `0x0010`.
    pub sub_type_word: u16,
    /// Index-like word at payload offset 14.
    pub index: u32,
    /// Number of segment groups / trailer member references.
    pub segment_count: u32,
    /// Sub-header bytes at payload offsets 26–27 (`[2, 1]` across
    /// all fixture records; exposed verbatim).
    pub sub_header_tail: [u8; 2],
    /// Decoded segment groups (`segment_count` entries).
    pub segments: Vec<DecodedIgBoundary2dSegment>,
    /// Anchor X following the segment groups (inside the segment
    /// bounding box on all fixture records).
    pub anchor_x: f64,
    /// Anchor Y following the segment groups.
    pub anchor_y: f64,
    /// `u8` flag between the anchor and the member count (`1` in
    /// fixtures; exposed verbatim).
    pub trailer_flag: u8,
    /// Trailer member references (`segment_count` entries).
    pub member_refs: Vec<DecodedIgBoundary2dMemberRef>,
    /// `true` when the segments chain end-to-start and close back
    /// onto the first start within `1e-9` per axis (20/20 fixture
    /// records are closed).
    pub closed_loop: bool,
}

impl From<crate::parsers::sheet_records::SheetIgBoundary2dDecoded> for DecodedIgBoundary2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgBoundary2dDecoded) -> Self {
        let closed_loop = d.is_closed_loop(1e-9);
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            segment_count: d.segment_count,
            sub_header_tail: d.sub_header_tail,
            segments: d
                .segments
                .into_iter()
                .map(|s| DecodedIgBoundary2dSegment {
                    tag_offset: s.tag_offset,
                    start_x: s.start.0,
                    start_y: s.start.1,
                    end_x: s.end.0,
                    end_y: s.end.1,
                })
                .collect(),
            anchor_x: d.anchor.0,
            anchor_y: d.anchor.1,
            trailer_flag: d.trailer_flag,
            member_refs: d
                .member_refs
                .into_iter()
                .map(|m| DecodedIgBoundary2dMemberRef {
                    member_oid: m.member_oid,
                    class_word: m.class_word,
                    sub_word: m.sub_word,
                })
                .collect(),
            closed_loop,
        }
    }
}

/// Audit-only model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetSubRecord0x0010Decoded`] —
/// PSM type `0x0010` sub-record family (Phase 18 audit collection).
///
/// Only the stable 6-byte PSM header (`type_code`, `type_flags`,
/// `bytes_to_follow`) and the variable payload are exposed; the
/// payload's sub-kind discriminator and per-field semantics are
/// deferred until IDA reverse engineering confirms the class identity.
/// **Unlike Phase 14 typed primitives**, this DTO has no `oid` field
/// because `0x0010` records use the 6-byte PSM header convention
/// (no fixed `oid` slot), mirroring Phase 15
/// [`DecodedGraphicGroupRecord`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedSubRecord0x0010Record {
    /// Inclusive byte-range start covering the full PSM record
    /// (6-byte header + payload).
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0010`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header. Equals
    /// `raw_payload.len()` by construction.
    pub bytes_to_follow: u32,
    /// Raw payload bytes (length = `bytes_to_follow`). Sub-kind
    /// discrimination and per-field decoding are deferred to a
    /// future phase.
    pub raw_payload: Vec<u8>,
    /// Audit-only Phase 19 field: `payload[0..2]` as little-endian
    /// `u16`; `None` when `raw_payload.len() < 2`.
    ///
    /// Mirrors
    /// [`crate::parsers::sheet_records::SheetSubRecord0x0010Decoded::leading_word`].
    /// This is **not** a named sub-kind discriminator; the field name
    /// describes only the byte position. Promotion to a typed
    /// `sub_kind` field is deferred pending IDA confirmation.
    #[serde(default)]
    pub leading_word: Option<u16>,
}

impl From<crate::parsers::sheet_records::SheetSubRecord0x0010Decoded>
    for DecodedSubRecord0x0010Record
{
    fn from(d: crate::parsers::sheet_records::SheetSubRecord0x0010Decoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            raw_payload: d.raw_payload,
            leading_word: d.leading_word,
        }
    }
}

/// One length-prefixed UTF-16LE string from a PSM `0x0010` attribute
/// fragment (Phase 26). Model-shaped mirror of
/// [`crate::parsers::sheet_records::DecodedAttributeString`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedAttributeStringRecord {
    /// Offset of the `u16` length word within the record payload.
    pub len_offset: usize,
    /// Character count from the `u16` length prefix.
    pub char_count: u16,
    /// Decoded UTF-16LE text.
    pub text: String,
}

/// Stable, model-shaped DTO that mirrors
/// [`crate::parsers::sheet_records::SheetAttributeFragmentDecoded`] —
/// the Phase 26 additive, audit-only attribute-fragment view of PSM
/// `0x0010` records.
///
/// Carries `SmartPlant` attribute text as length-prefixed UTF-16LE strings
/// after a fixed `marker(4) + aux(8)` prefix. Coexists with
/// [`DecodedSubRecord0x0010Record`] (raw) and emits no geometry kind.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedAttributeFragment {
    /// Inclusive byte-range start (6-byte header + payload).
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0010`.
    pub type_code: u16,
    /// `payload[0..4]` little-endian `u32` — a repeating type marker
    /// (often `0x00010002`), not a unique object id.
    pub marker: u32,
    /// `payload[4..12]` raw aux bytes (length 8); per-field semantics
    /// pending IDA confirmation.
    pub aux: Vec<u8>,
    /// Length-prefixed UTF-16LE strings parsed from `payload[12..]`.
    pub strings: Vec<DecodedAttributeStringRecord>,
}

impl From<crate::parsers::sheet_records::SheetAttributeFragmentDecoded>
    for DecodedAttributeFragment
{
    fn from(d: crate::parsers::sheet_records::SheetAttributeFragmentDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            marker: d.marker,
            aux: d.aux.to_vec(),
            strings: d
                .strings
                .into_iter()
                .map(|s| DecodedAttributeStringRecord {
                    len_offset: s.len_offset,
                    char_count: s.char_count,
                    text: s.text,
                })
                .collect(),
        }
    }
}

/// Stable, model-shaped DTO that mirrors
/// [`crate::parsers::sheet_records::SheetJStyleOverrideDecoded`] —
/// PSM type `0x0030` (RAD `JStyleOverride` class, `style.dll` CLSID
/// `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`) Version-3 IO record.
///
/// Phase 16 Slice D DTO with field names that match the
/// authoritative `style.dll!sub_1000F030` IO sequence.
/// See `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` for
/// the byte-level evidence chain.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedJStyleOverrideRecord {
    /// Inclusive byte-range start covering the full PSM record.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x0030`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header (`>= 64`).
    pub bytes_to_follow: u32,
    /// `oid` from the PSM header.
    pub oid: u32,
    /// Version-3 disk field at payload `+0..3`.
    pub field_a_u32: u32,
    /// Version-3 disk field at payload `+4..7`.
    pub field_b_u32: u32,
    /// Version-3 disk field at payload `+8..11`.
    pub field_c_u32: u32,
    /// Version-3 disk field at payload `+12..15`.
    pub field_d_u32: u32,
    /// Version-3 disk field at payload `+16..23` (f64).
    pub field_1_f64: f64,
    /// Version-3 disk field at payload `+24..31` (f64).
    /// Cross-fixture probe shows values cluster around
    /// `{0, π/2, 3π/2, 2π}` — rotation-angle candidate.
    pub field_2_f64: f64,
    /// Version-3 disk field at payload `+32..39` (f64).
    pub field_3_f64: f64,
    /// Version-3 disk field at payload `+40..47` (f64).
    pub field_4_f64: f64,
    /// Version-3 disk field at payload `+48..51`.
    pub field_e_u32: u32,
    /// Version-3 disk field at payload `+52..55`.
    pub field_f_u32: u32,
    /// Version-3 disk field at payload `+56..59`.
    pub field_g_u32: u32,
    /// Version-3 disk field at payload `+60..61`.
    pub field_h_u16: u16,
    /// Version-3 disk field at payload `+62..63`.
    pub field_i_u16: u16,
    /// Optional attribute / linkage tail (`bytes_to_follow - 64`
    /// bytes). Audit-only; internal layout is hypothesis.
    pub raw_attribute_tail: Vec<u8>,
}

impl From<crate::parsers::sheet_records::SheetJStyleOverrideDecoded>
    for DecodedJStyleOverrideRecord
{
    fn from(d: crate::parsers::sheet_records::SheetJStyleOverrideDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            field_a_u32: d.field_a_u32,
            field_b_u32: d.field_b_u32,
            field_c_u32: d.field_c_u32,
            field_d_u32: d.field_d_u32,
            field_1_f64: d.field_1_f64,
            field_2_f64: d.field_2_f64,
            field_3_f64: d.field_3_f64,
            field_4_f64: d.field_4_f64,
            field_e_u32: d.field_e_u32,
            field_f_u32: d.field_f_u32,
            field_g_u32: d.field_g_u32,
            field_h_u16: d.field_h_u16,
            field_i_u16: d.field_i_u16,
            raw_attribute_tail: d.raw_attribute_tail,
        }
    }
}

/// Stable text run DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetText {
    /// Byte offset where the text run begins inside the Sheet stream.
    pub offset: usize,
    /// Encoding family (`"ascii"` or `"utf16_le"`).
    pub encoding: String,
    /// Decoded printable text.
    pub text: String,
    /// Number of bytes consumed by the run in the source Sheet stream.
    pub byte_len: usize,
}

/// Stable endpoint DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetEndpoint {
    /// Byte offset in the Sheet stream where this endpoint record starts.
    pub offset: usize,
    /// Relationship's `field_x` value.
    pub rel_field_x: u32,
    /// Source endpoint `field_x`.
    pub endpoint_a: u32,
    /// Target endpoint `field_x`.
    pub endpoint_b: u32,
}

/// Stable coordinate-hint DTO for Sheet streams.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetCoordinateHintDto {
    /// Byte offset of the first coordinate-like value.
    pub offset: usize,
    /// First coordinate-like value.
    pub x: i32,
    /// Second coordinate-like value.
    pub y: i32,
}

/// Coordinate pair from a repeated-record f64 shape inside a Sheet stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SheetF64CoordinateHintDto {
    /// Byte offset of the first `f64` in the pair.
    pub offset: usize,
    /// First `f64` coordinate value.
    pub x: f64,
    /// Second `f64` coordinate value.
    pub y: f64,
}

/// Candidate mapping from an object `field_x` to source-backed Sheet
/// geometry evidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SheetObjectGeometryHint {
    /// Byte offset where this candidate mapping starts inside the Sheet stream.
    pub offset: usize,
    /// Object Dynamic Attributes `field_x` this mapping appears to describe.
    pub field_x: u32,
    /// Optional coordinate associated with the object, when the probe can
    /// prove it came from the same source record.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub position: Option<SheetCoordinateHintDto>,
    /// Optional f64 coordinate pair from a repeated record shape, used as
    /// fallback when [`Self::position`] is absent.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub f64_position: Option<SheetF64CoordinateHintDto>,
    /// Optional GraphicOID-like value surfaced near this mapping.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub graphic_oid: Option<u32>,
    /// Short diagnostic note describing why this is still a hint.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}
