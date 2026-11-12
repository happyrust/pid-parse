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
    /// PSM-decoded `igLine2d` records (PSM type `0x0018`, IGDS
    /// class tag `0x18`) — Intergraph Sigma's standard 2D line
    /// primitive, emitted by
    /// [`crate::parsers::sheet_records::decode_iglines`]: the line
    /// representation of every `SmartPlant` fixture (Phase 14 Slice J).
    /// Each entry is a model-shaped projection of the parser-level DTO
    /// with its provenance; producing
    /// [`crate::geometry::PidGeometryConfidence::Decoded`]
    /// `PidGraphicKind::Line` entities from them is
    /// [`crate::geometry::build_normalized_geometry`]'s part.
    ///
    /// A `decoded_primitive_lines` field stood ahead of this one until
    /// 2026-11-11, for `GLine2d` (`0x3FE6`). No chain holds a record of
    /// that type code — it is not one the native reader has a class for —
    /// and the field was empty on every document from 2026-08-10 on; a
    /// document serialized before or since has no such key.
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
    /// PSM-decoded `igArc2d` records (PSM type `0x0061`, IGDS class
    /// tag `0x61`) — Intergraph Sigma's standard 2D arc primitive,
    /// emitted by [`crate::parsers::sheet_records::decode_igarcs`].
    /// No drawing sheet of the corpus carries one; the linked border
    /// templates letter their logo and rounded corners with them.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igarcs: Vec<DecodedIgArc2dRecord>,
    /// PSM-decoded `igCircle2d` records (PSM type `0x0059`, IGDS class
    /// tag `0x59`) — Intergraph Sigma's standard 2D circle primitive,
    /// emitted by [`crate::parsers::sheet_records::decode_igcircles`].
    /// Promoted from decode-only to emitting once a real sheet carried
    /// them: W's `Sheet6` draws its instrument balloons and sensing-bulb
    /// centre dots with 17 of them at layer-resolved coordinates. The
    /// DTO is the same one `JSiteNestedGeometry` uses for cached-body
    /// circles — the record layout is identical in both storages.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igcircles: Vec<super::DecodedIgCircle2dRecord>,
    /// Fully-typed PSM `igBoundary2d` records (PSM type
    /// `0x0013`, IGDS class tag `0x13`) emitted by
    /// [`crate::parsers::sheet_records::decode_igboundaries`]
    /// (Phase 34-D).
    ///
    /// Every payload byte is field-named (segment groups, anchor,
    /// trailer member references). The family was audit-only at first:
    /// the trailer member references prove the segment coordinates
    /// re-list member `igLine2d` geometry that already emits normalized
    /// `Line` entities, so *stroking* the ring again would double-draw
    /// the outline
    /// (`docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`).
    /// But the ring is the one thing its members cannot say — every ring
    /// on the corpus resolves to a `JStyleSimpleFill` — so it now emits
    /// a closed polyline for renderers to *fill*, never stroke
    /// (`docs/analysis/2026-08-10-fill-has-a-consumer-after-all.md`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igboundaries: Vec<DecodedIgBoundary2dRecord>,
    /// Audit-only PSM `0x00FA` `DependencyObject` / `GraphicPersist` records
    /// emitted by
    /// [`crate::parsers::sheet_records::decode_dependency_objects`].
    ///
    /// These records preserve the stable envelope and raw reference
    /// payload for inspection. They intentionally do not produce
    /// normalized geometry entities and do not expose a stable
    /// `child_oids` field until the variable tail is proven across
    /// size/sub-type buckets.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_dependency_objects: Vec<DecodedDependencyObjectRecord>,
    /// PSM `0x003D` `igSmartFrame2d` records emitted by
    /// [`crate::parsers::sheet_records::decode_smartframes`].
    ///
    /// An OLE container frame. The drawing's border is one of these, linked
    /// or embedded, so an entry whose
    /// [`DecodedIgSmartFrame2dRecord::page_extent_m`] is `Some` gives the
    /// sheet size. It emits no normalized geometry: the extent is metadata
    /// about the page, and the border's own strokes live in the linked
    /// template rather than in this drawing.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igsmartframes: Vec<DecodedIgSmartFrame2dRecord>,
    /// PSM `0x0020` `igRectangle2d` records emitted by
    /// [`crate::parsers::sheet_records::decode_igrectangles`].
    ///
    /// A rectangle is the parent of four `igLine2d` edges it lists by oid;
    /// the edges are records of the same stream, decoded and emitted on
    /// their own, so the rectangle itself emits nothing -- drawing it would
    /// draw its edges twice
    /// (`docs/analysis/2026-09-07-rectangle-owns-its-edges-bspline-is-a-leaf.md`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igrectangles: Vec<DecodedIgRectangle2dRecord>,
    /// PSM `0x005D` `igBspCurve2d` records emitted by
    /// [`crate::parsers::sheet_records::decode_igbspcurves`]: a leaf curve
    /// nothing else draws, emitted as the polyline [`crate::bspline::sample`]
    /// makes of it.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igbspcurves: Vec<DecodedIgBspCurve2dRecord>,
    /// PSM `0x0115` `igDimension` / `JDim` records emitted by
    /// [`crate::parsers::sheet_records::decode_igdimensions`]: the driving
    /// dimensions of a parametric symbol definition. Audit only — the
    /// corpus keeps every one on a `Dimension` layer the file itself has
    /// switched off, so nothing here is drawn
    /// (`docs/analysis/2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igdimensions: Vec<DecodedIgDimensionRecord>,
    /// PSM `0x0021` `ComplexString` records emitted by
    /// [`crate::parsers::sheet_records::decode_igcomplexstrings`]: a run of
    /// curves joined end to end.
    ///
    /// Audit only. A string states no coordinate: it lists its curves by
    /// oid, and each is an `igLine2d` or `igArc2d` of the same stream that
    /// names the string as its owner, sits on its layer and carries its
    /// style id — so the members, decoded and emitted on their own, draw
    /// what the string draws
    /// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
    /// §10.4).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igcomplexstrings: Vec<DecodedIgComplexString2dRecord>,
    /// PSM `0x007B` Group records emitted by
    /// [`crate::parsers::sheet_records::decode_iggroups`]: graphic objects
    /// that select and move as one.
    ///
    /// Audit only: a group states no coordinate, and every member it lists
    /// is a record of the same stream that names the group as its owner.
    /// The corpora's groups are the two `igRectangle2d` of a border, whose
    /// strokes are the `igLine2d` edges those list in turn, or — in the four
    /// `-W` templates of a backup's reference data — the lines, arcs,
    /// B-splines, elliptical arcs, boundaries and point of a logo, each of
    /// which decodes and emits on its own.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_iggroups: Vec<DecodedIgGroupRecord>,
    /// PSM `0x007E` `igEllipticalArc2d` records emitted by
    /// [`crate::parsers::sheet_records::decode_igellipticalarcs`]: a leaf
    /// curve nothing else draws, emitted as the polyline
    /// [`crate::bspline::sample`] makes of the exact rational B-spline
    /// [`crate::bspline::elliptical_arc_directed`] builds from it -- the
    /// curve a cached symbol body draws for the same record.
    ///
    /// Five border templates carry any on their own chain. `XA-A1.igr` has
    /// two half ellipses, which close the counters of an outlined letter
    /// and run counter-clockwise (direction byte 1). `A0-W.igr` to
    /// `A3-W.igr`, inside the committed `RefData~4~682.zip`, have four
    /// each: one ellipse's top and bottom caps and two short pieces of its
    /// left side, the ring of a logo, all clockwise (direction byte 0). No
    /// drawing of the corpora carries one at sheet level, and no page
    /// wears a `-W` template.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decoded_igellipticalarcs: Vec<super::DecodedIgEllipticalArc2dRecord>,
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
    /// Phase 38 S2 census of PSM records **no typed decoder claims**,
    /// emitted by
    /// [`crate::parsers::undecoded_census::undecoded_type_code_census`]
    /// and split by `SmartPlant`'s native graphic predicate.
    ///
    /// Entries whose [`SheetUndecodedTypeCode::is_graphic`] is `true`
    /// are drawable content this crate silently dropped until now
    /// (`igDimension` / `igBalloon` / `igLeader` / `Graphics Bag` and
    /// friends); [`crate::geometry::build_normalized_geometry`] turns
    /// each of them into a named warning. Non-graphic entries (the
    /// `*Relation2d` constraint families) are recorded here but warn
    /// nobody.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub undecoded_type_codes: Vec<SheetUndecodedTypeCode>,
    /// Phase 40 census of PSM records **a typed decoder walked over and
    /// refused**, emitted by
    /// [`crate::parsers::undecoded_census::refused_record_census`].
    ///
    /// The other way a record misses the drawing. [`Self::undecoded_type_codes`]
    /// tests the type code, so it is blind to a record whose family is wired
    /// and whose bytes that family rejected; those fell out of both the
    /// decoded output and the census, unreported. Entries whose
    /// [`SheetRefusedRecord::is_graphic`] is `true` cost the drawing strokes
    /// and [`crate::geometry::build_normalized_geometry`] names each one.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub refused_records: Vec<SheetRefusedRecord>,
    /// Census of PSM records **the native reader skips** — their type word
    /// carries the `0x8000` bit `PSMSerializeIn` tests before reading the
    /// oid — emitted by
    /// [`crate::parsers::undecoded_census::native_skipped_record_census`].
    ///
    /// The third kind, disjoint from the two above. No family decodes such
    /// a record ([`crate::parsers::sheet_records::parse_live_psm_header`]),
    /// so it is not drawn, which is what the native reader does with it; on
    /// the corpus each one is an earlier copy of a live record of the same
    /// oid. Recorded so the count is visible; it warns nobody.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub skipped_records: Vec<SheetSkippedRecord>,
}

/// One PSM type code observed in a Sheet stream with no typed decoder.
/// Model-shaped mirror of
/// [`crate::parsers::undecoded_census::UndecodedTypeCodeCount`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetUndecodedTypeCode {
    /// PSM 14-bit type code.
    pub type_code: u16,
    /// Chain-validated records with this code outside every claimed
    /// byte range of this sheet.
    pub count: usize,
    /// Whether the native graphic predicate
    /// (`radsrvitem.dll!sub_56449950`) accepts this code — `true`
    /// means dropped drawable content.
    pub is_graphic: bool,
    /// Class name from the PSM type-code registry, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rad_class_name: Option<String>,
}

impl From<crate::parsers::undecoded_census::UndecodedTypeCodeCount> for SheetUndecodedTypeCode {
    fn from(d: crate::parsers::undecoded_census::UndecodedTypeCodeCount) -> Self {
        Self {
            type_code: d.type_code,
            count: d.count,
            is_graphic: d.is_graphic,
            rad_class_name: d.rad_class_name.map(str::to_string),
        }
    }
}

/// One PSM type code whose records the native reader skips in a Sheet
/// stream. Model-shaped mirror of
/// [`crate::parsers::undecoded_census::SkippedRecordCount`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetSkippedRecord {
    /// PSM 14-bit type code.
    pub type_code: u16,
    /// Chain-validated records with this code whose type word carries the
    /// native skip bit.
    pub count: usize,
    /// Whether the native graphic predicate
    /// (`radsrvitem.dll!sub_56449950`) accepts this code — `true` means
    /// drawing the record would have added strokes the native reader
    /// does not draw.
    pub is_graphic: bool,
    /// Class name from the PSM type-code registry, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rad_class_name: Option<String>,
}

impl From<crate::parsers::undecoded_census::SkippedRecordCount> for SheetSkippedRecord {
    fn from(d: crate::parsers::undecoded_census::SkippedRecordCount) -> Self {
        Self {
            type_code: d.type_code,
            count: d.count,
            is_graphic: d.is_graphic,
            rad_class_name: d.rad_class_name.map(str::to_string),
        }
    }
}

/// One PSM type code whose decoder refused records in a Sheet stream.
/// Model-shaped mirror of
/// [`crate::parsers::undecoded_census::RefusedRecordCount`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SheetRefusedRecord {
    /// PSM 14-bit type code — one this crate does decode.
    pub type_code: u16,
    /// Chain-validated records with this code that the family's own
    /// decoder did not claim.
    pub count: usize,
    /// Whether the native graphic predicate
    /// (`radsrvitem.dll!sub_56449950`) accepts this code — `true` means
    /// the refusal costs the drawing strokes.
    pub is_graphic: bool,
    /// Class name from the PSM type-code registry, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rad_class_name: Option<String>,
}

impl From<crate::parsers::undecoded_census::RefusedRecordCount> for SheetRefusedRecord {
    fn from(d: crate::parsers::undecoded_census::RefusedRecordCount) -> Self {
        Self {
            type_code: d.type_code,
            count: d.count,
            is_graphic: d.is_graphic,
            rad_class_name: d.rad_class_name.map(str::to_string),
        }
    }
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
/// [`crate::parsers::sheet_records::SheetIgLine2dDecoded`]
/// — PSM type `0x0018` Intergraph Sigma 2D standard line.
///
/// Layout: PSM 6-byte header + 50-byte payload. The payload opens
/// with the rest of the shared PSM envelope — `oid` (u32) and the
/// 8-byte `aux` pair (`parent_ref`, `aux_hi`) — and only then the
/// class's own `sub_type_word` (u16), `index` (u32), and four `f64`
/// LE for `start.x`, `start.y`, `end.x`, `end.y`. See
/// `docs/analysis/2026-08-11-remaining-header-is-the-psm-aux-field.md`
/// for why `aux_hi` is evidence rather than a validation rule.
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
    /// Low half of the PSM envelope's `aux` pair (payload bytes 4..7).
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this line sits on.
    pub sheet_layer_ref: u32,
    /// High half of the PSM envelope's `aux` pair (payload bytes
    /// 8..11), verbatim. `12`, `8` and `6996` all occur on the corpus;
    /// the native reader discards this field, so nothing gates on it.
    pub aux_hi: u32,
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
            sheet_layer_ref: d.sheet_layer_ref,
            aux_hi: d.aux_hi,
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
    /// Oid of the `JSheetLayer` this line string sits on.
    pub sheet_layer_ref: u32,
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
            sheet_layer_ref: d.sheet_layer_ref,
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
    /// Oid of the `JSheetLayer` this point sits on.
    pub sheet_layer_ref: u32,
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
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            x: d.point.0,
            y: d.point.1,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgArc2dDecoded`] — PSM type
/// `0x0061` Intergraph Sigma standard 2D arc.
///
/// The record's parser DTO does not carry `type_code`/`bytes_to_follow`
/// fields (the envelope is not fielded on that family), so neither does
/// this one.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgArc2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this arc sits on.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / style reference, the slot `igLine2d` carries its style
    /// link in.
    pub index: u32,
    /// Centre `x`, sheet units.
    pub center_x: f64,
    /// Centre `y`, sheet units.
    pub center_y: f64,
    /// Radius, sheet units.
    pub radius: f64,
    /// Absolute start angle, radians counter-clockwise from +X; the arc
    /// leaves it **clockwise** towards `end_angle`.
    pub start_angle: f64,
    /// Absolute end angle, radians counter-clockwise from +X.
    pub end_angle: f64,
    /// Trailing flag byte, verbatim.
    pub flag: u8,
}

impl From<crate::parsers::sheet_records::SheetIgArc2dDecoded> for DecodedIgArc2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgArc2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_flags: d.type_flags,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            center_x: d.center.0,
            center_y: d.center.1,
            radius: d.radius,
            start_angle: d.start_angle,
            end_angle: d.end_angle,
            flag: d.flag,
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
    /// Oid of the `JSheetLayer` this text box sits on.
    pub sheet_layer_ref: u32,
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
    /// Which of the three `igTextBox` shapes this record is (payload `+18`).
    #[serde(default)]
    pub text_sub_type: u16,
    /// Third trailing f64: the cosine of the text direction.
    pub trailing_double_3: f64,
    /// Text rotation in radians, from the `(cos, sin)` direction pair that
    /// follows the insertion point. `0`, `π/2` and `π` across the corpus.
    #[serde(default)]
    pub rotation_rad: f64,
    /// The letter height the record states, in sheet units — `0.0` where the
    /// direction pair is the normalized unit vector drawing sheets write;
    /// the height itself where a border template scales its direction by it.
    #[serde(default)]
    pub letter_height_m: f64,
    /// The record's own text-box anchor: the last byte of the 4-byte trailer
    /// that closes the placement tail. `(code >> 4) % 3` names the
    /// horizontal hot point — `0` left, `1` centre, `2` right — and
    /// `(code & 0x0F) % 3` the vertical one — `0` bottom, `1` top,
    /// `2` middle. It overrides the paragraph style's declared alignment.
    #[serde(default)]
    pub anchor_code: u8,
    /// Width of the text box a kind-2 tail carries, in sheet units — `0.0`
    /// when the tail states none.
    ///
    /// The reference plots letter a boxed record from that box's left edge,
    /// whichever hot point `anchor_code` measures the insertion against; see
    /// [`crate::parsers::sheet_records::SheetIgTextBoxDecoded::text_box_width_m`].
    #[serde(default)]
    pub text_box_width_m: f64,
    /// Height of the same box, in sheet units — `0.0` when there is none.
    #[serde(default)]
    pub text_box_height_m: f64,
    /// The record's formatting runs, in stored order. Empty for shape 1.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub runs: Vec<DecodedTextRun>,
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::IgTextBoxRun`] — one formatting run of an
/// `igTextBox`: `len` characters lettered with the style `style_id` names.
/// Selector `1` names a character style, selector `2` restates the record's
/// paragraph style.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedTextRun {
    /// Characters the run covers.
    pub len: u16,
    /// The slot the run loads into: `1` a character style, `2` the paragraph
    /// style.
    pub selector: u16,
    /// Style id the run names, in the record's own document.
    pub style_id: u32,
}

impl From<crate::parsers::sheet_records::IgTextBoxRun> for DecodedTextRun {
    fn from(run: crate::parsers::sheet_records::IgTextBoxRun) -> Self {
        Self {
            len: run.len,
            selector: run.selector,
            style_id: run.style_id,
        }
    }
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
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            text_length: d.text_length,
            text: d.text,
            text_sub_type: d.text_sub_type,
            trailing_double_1: d.trailing_double_1,
            trailing_double_2: d.trailing_double_2,
            trailing_double_3: d.trailing_double_3,
            rotation_rad: d.rotation_rad,
            letter_height_m: d.letter_height_m,
            anchor_code: d.anchor_code,
            text_box_width_m: d.text_box_width_m,
            text_box_height_m: d.text_box_height_m,
            runs: d.runs.into_iter().map(DecodedTextRun::from).collect(),
        }
    }
}

/// Serde default for [`DecodedIgSymbol2dRecord::uniform_scale`]: a record
/// read before the field existed — or one whose scale word is absent —
/// drew its body unresized.
fn default_uniform_scale() -> f64 {
    1.0
}

/// Skip predicate paired with [`default_uniform_scale`]: unit scale is the
/// overwhelming case, so only resized placements earn a field on disk.
fn is_default_uniform_scale(scale: &f64) -> bool {
    *scale == 1.0
}

/// Default for [`DecodedIgSymbol2dRecord::matrix_tag_third_byte`]: the
/// canonical `0xA7`, which is what 12 411 of the 12 417 corpus records
/// carry (every one outside `D04`).
fn default_matrix_tag_third_byte() -> u8 {
    0xA7
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
    /// Oid of the `JSheetLayer` this symbol placement sits on.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Third byte of the matrix tag this record carries: `0xA7` on every
    /// corpus fixture but one, `0x27` on every `igSymbol2d` record in the
    /// plant backup's `D04`. The two differ only in this byte's `0x80`
    /// bit, and every other field at and after the tag reads the same
    /// shape under both.
    #[serde(default = "default_matrix_tag_third_byte")]
    pub matrix_tag_third_byte: u8,
    /// Numeric id of the top-level `JSite<id>` storage carrying this
    /// instance's symbol reference (Phase 35-C: the u32 immediately
    /// before the placement-matrix tag; 132/132 records across all 6
    /// fixtures resolve to a same-file `JSite<id>` whose `JProperties`
    /// names the `.sym` library path).
    #[serde(default)]
    pub jsite_ref: u32,
    /// Style id the placement names for its body's line work (payload
    /// `+25`, fixed). Resolves in the root document's `StyleCluster` on
    /// 109/109 corpus placements, and the colours match a `SmartPlant`
    /// screenshot — including a vessel whose `.sym` says black but draws
    /// in this style's `#800000`. See
    /// `docs/analysis/2026-08-24-placement-names-the-body-style.md`.
    #[serde(default)]
    pub style_ref: u32,
    /// First element of the 2×2 placement matrix, which follows the
    /// record's matrix tag rather than sitting at a fixed offset.
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
    /// Uniform factor the placement resizes its parametric body by — the
    /// f64 after the insertion pair. `1.0` on nearly every record;
    /// parametric symbols carry their own (0.37–5.63 on the corpus). It
    /// multiplies the placement matrix about the insertion point.
    #[serde(
        default = "default_uniform_scale",
        skip_serializing_if = "is_default_uniform_scale"
    )]
    pub uniform_scale: f64,
    /// Oid, inside the definition cache storage, of the `JSheet` holding
    /// this placement's symbol body (the payload's second-to-last `u32`).
    #[serde(default)]
    pub definition_sheet_ref: u32,
    /// Id of the `LdcSite` storage (`JSite<id>`) that is the definition
    /// cache (the payload's last `u32`).
    #[serde(default)]
    pub definition_site_ref: u32,
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
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            matrix_tag_third_byte: d.matrix_tag_third_byte,
            jsite_ref: d.jsite_ref,
            style_ref: d.style_ref,
            transform_00: d.transform[0],
            transform_01: d.transform[1],
            transform_10: d.transform[2],
            transform_11: d.transform[3],
            insertion_x: d.insertion.0,
            insertion_y: d.insertion.1,
            uniform_scale: d.uniform_scale,
            definition_sheet_ref: d.definition_sheet_ref,
            definition_site_ref: d.definition_site_ref,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgRectangle2dDecoded`] -- PSM type
/// `0x0020` `igRectangle2d` (`imagdex.dex`), the parent of four `igLine2d`
/// edges.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgRectangle2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this rectangle sits on.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / style reference.
    pub index: u32,
    /// Corner the width runs from, `x`.
    pub origin_x: f64,
    /// Corner the width runs from, `y`.
    pub origin_y: f64,
    /// Extent along the rectangle's own x axis.
    pub width: f64,
    /// Angle of that axis, radians; zero on the corpus.
    pub rotation: f64,
    /// Height as a fraction of the width.
    pub aspect: f64,
    /// Oids of the `igLine2d` records that are its edges, in on-disk order.
    pub edges: Vec<u32>,
}

impl DecodedIgRectangle2dRecord {
    /// Extent along the rectangle's own y axis.
    pub fn height(&self) -> f64 {
        self.width * self.aspect
    }
}

impl From<crate::parsers::sheet_records::SheetIgRectangle2dDecoded> for DecodedIgRectangle2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgRectangle2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            origin_x: d.origin.0,
            origin_y: d.origin.1,
            width: d.width,
            rotation: d.rotation,
            aspect: d.aspect,
            edges: d.edges,
        }
    }
}

/// One member entry of a [`DecodedIgComplexString2dRecord`] or a
/// [`DecodedIgGroupRecord`], mirroring
/// [`crate::parsers::sheet_records::SheetAssociationMember`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedAssociationMember {
    /// Oid of the member record, in the same stream.
    pub oid: u32,
    /// The member's range box, `[min x, min y, max x, max y]`, in units of
    /// the listing record's `range_scale` over 32768 metres.
    pub range_box: [i16; 4],
    /// The word that closes the entry: `0x8000` on a `ComplexString`'s
    /// `KeyPoint Constraint` members, a small value otherwise.
    pub word: u16,
}

impl DecodedAssociationMember {
    /// The range box in metres, `[min x, min y, max x, max y]`, read in the
    /// unit of the record that lists the member: `range_scale / 32768` m.
    #[must_use]
    pub fn range_box_m(&self, range_scale: f64) -> [f64; 4] {
        self.range_box
            .map(|units| f64::from(units) * range_scale / 32768.0)
    }
}

impl From<crate::parsers::sheet_records::SheetAssociationMember> for DecodedAssociationMember {
    fn from(m: crate::parsers::sheet_records::SheetAssociationMember) -> Self {
        Self {
            oid: m.oid,
            range_box: m.range_box,
            word: m.word,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgComplexString2dDecoded`] -- PSM
/// type `0x0021` `ComplexString`, a run of curves joined end to end.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgComplexString2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this string sits on.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / style reference; every curve member carries the same one.
    pub index: u32,
    /// The scale of the members' range boxes: a box is in units of
    /// `range_scale / 32768` m. `1.0` or `2.0` on the corpora.
    pub range_scale: f64,
    /// The members, in on-disk order: the curves, then the constraints that
    /// join them.
    pub members: Vec<DecodedAssociationMember>,
}

impl DecodedIgComplexString2dRecord {
    /// The range box of one of this string's members, in metres.
    #[must_use]
    pub fn range_box_m(&self, member: &DecodedAssociationMember) -> [f64; 4] {
        member.range_box_m(self.range_scale)
    }

    /// Oids of the members that are curves, in order.
    pub fn curve_oids(&self) -> impl Iterator<Item = u32> + '_ {
        self.members
            .iter()
            .filter(|member| {
                member.word != crate::parsers::sheet_records::IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER
            })
            .map(|member| member.oid)
    }
}

impl From<crate::parsers::sheet_records::SheetIgComplexString2dDecoded>
    for DecodedIgComplexString2dRecord
{
    fn from(d: crate::parsers::sheet_records::SheetIgComplexString2dDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            range_scale: d.range_scale,
            members: d.members.into_iter().map(Into::into).collect(),
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgGroupDecoded`] -- PSM type
/// `0x007B` Group, graphic objects that select and move as one.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgGroupRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` the group sits on; zero on the corpus.
    pub sheet_layer_ref: u32,
    /// The scale of the members' range boxes: a box is in units of
    /// `range_scale / 32768` m. `1.0` or `2.0` on the corpora.
    pub range_scale: f64,
    /// The members, in on-disk order.
    pub members: Vec<DecodedAssociationMember>,
}

impl DecodedIgGroupRecord {
    /// The range box of one of this group's members, in metres.
    #[must_use]
    pub fn range_box_m(&self, member: &DecodedAssociationMember) -> [f64; 4] {
        member.range_box_m(self.range_scale)
    }
}

impl From<crate::parsers::sheet_records::SheetIgGroupDecoded> for DecodedIgGroupRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgGroupDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            range_scale: d.range_scale,
            members: d.members.into_iter().map(Into::into).collect(),
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgBspCurve2dDecoded`] -- PSM type
/// `0x005D` `igBspCurve2d` (`imagdex.dex`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgBspCurve2dRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this curve sits on.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / style reference.
    pub index: u32,
    /// Control point `x` coordinates, in order.
    pub pole_xs: Vec<f64>,
    /// Control point `y` coordinates, in order.
    pub pole_ys: Vec<f64>,
    /// One weight per pole when rational; empty otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub weights: Vec<f64>,
    /// The knot vector; its length minus the pole count minus one is the
    /// degree.
    pub knots: Vec<f64>,
    /// The double after the knots; `-1.0` on the corpus, meaning unknown.
    pub trailing: f64,
    /// The four closing bytes; `04 01 01 00` on the corpus.
    pub flags: [u8; 4],
}

impl DecodedIgBspCurve2dRecord {
    /// The control points as pairs.
    pub fn poles(&self) -> Vec<(f64, f64)> {
        self.pole_xs
            .iter()
            .copied()
            .zip(self.pole_ys.iter().copied())
            .collect()
    }

    /// Polynomial degree, `knots - poles - 1`.
    pub fn degree(&self) -> usize {
        self.knots.len().saturating_sub(self.pole_xs.len() + 1)
    }
}

impl From<crate::parsers::sheet_records::SheetIgBspCurve2dDecoded> for DecodedIgBspCurve2dRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgBspCurve2dDecoded) -> Self {
        let (pole_xs, pole_ys) = d.poles.into_iter().unzip();
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            index: d.index,
            pole_xs,
            pole_ys,
            weights: d.weights,
            knots: d.knots,
            trailing: d.trailing,
            flags: d.flags,
        }
    }
}

/// Stable model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgDimensionDecoded`] -- PSM type
/// `0x0115` `igDimension` / `JDim` (`imagdex.dex`), the driving dimension of
/// a parametric symbol definition.
///
/// Carries only what the two analyses mark as read
/// (`2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`
/// §7, `2026-09-15-tag-188-members-land-in-jdim-reference-slots.md` §10):
/// the frame, the kind, the value, the measured-geometry slot and the
/// group. The block area travels verbatim in [`Self::raw_tail`]; nothing in
/// it is interpreted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgDimensionRecord {
    /// Inclusive byte-range start.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference: the definition sheet this dimension belongs to.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this dimension sits on — a layer named
    /// `Dimension` on every corpus record, switched off by the file.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (`0` across the corpus).
    pub sub_type_word: u16,
    /// Dimension kind at payload `+14`. Only linear (`1`) decodes; the
    /// other seven kinds are refused.
    pub kind: u16,
    /// `main_len` at payload `+30`: the bytes the main area spans after
    /// the 34-byte frame prefix.
    pub main_len: u32,
    /// Oid of the `0x0058 JDimGroup` this dimension belongs to — the
    /// closing word at `34 + main_len`, present when the `+26` flag word
    /// has bit `0x0100` set. Unresolved; storage-local.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub group_ref: Option<u32>,
    /// The dimension value in metres.
    pub value_m: f64,
    /// Storage-local oid of the geometry this dimension measures (an
    /// `igLine2d` or `igPoint2d` of the same storage on the corpus).
    pub measured_oid: u32,
    /// The marker word behind the measured slot, verbatim (`0x00CB` on
    /// lines, `0x00F0` on points across the corpus).
    pub measured_marker: u16,
    /// The block area verbatim, payload `+82 .. 34 + main_len`.
    pub raw_tail: Vec<u8>,
}

impl From<crate::parsers::sheet_records::SheetIgDimensionDecoded> for DecodedIgDimensionRecord {
    fn from(d: crate::parsers::sheet_records::SheetIgDimensionDecoded) -> Self {
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            sub_type_word: d.sub_type_word,
            kind: d.kind,
            main_len: d.main_len,
            group_ref: d.group_ref,
            value_m: d.value_m,
            measured_oid: d.measured.oid,
            measured_marker: d.measured.marker,
            raw_tail: d.raw_tail,
        }
    }
}

/// Audit-only model-shaped DTO mirroring
/// [`crate::parsers::sheet_records::SheetDependencyObjectDecoded`] —
/// PSM type `0x00FA` `DependencyObject` / `GraphicPersist` records.
///
/// Only the stable 18-byte payload prefix and raw variable tail are
/// exposed. Candidate child references remain probe/audit evidence and
/// are not represented as a stable field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DecodedDependencyObjectRecord {
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

impl From<crate::parsers::sheet_records::SheetDependencyObjectDecoded>
    for DecodedDependencyObjectRecord
{
    fn from(d: crate::parsers::sheet_records::SheetDependencyObjectDecoded) -> Self {
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
/// (Phase 34-D): a line member whole, or one of the chords a curved member
/// — a point run, an arc of a circle or an ellipse, a B-spline, a circle —
/// is flattened to. Model-shaped mirror of
/// [`crate::parsers::sheet_records::SheetIgBoundary2dSegment`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgBoundary2dSegment {
    /// Payload offset of the tag byte of the member the segment comes
    /// from; the chords of one curved member share it.
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
    /// Oid of the `JSheetLayer` this boundary sits on (payload
    /// offset 8). Never validated — an `== <n>` rule here admits one
    /// layer and refuses the rest. See
    /// `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md`.
    pub sheet_layer_ref: u32,
    /// Sub-type word at payload offset 12: `0x0010` on a drawing's boundary
    /// over lines, `0x0000` on a linked template's and on a drawing's over
    /// a circle.
    pub sub_type_word: u16,
    /// Index-like word at payload offset 14.
    pub index: u32,
    /// How many members the record states, over all its loops: one segment
    /// a member where every member is a line, several where one is curved.
    pub segment_count: u32,
    /// Sub-header bytes at payload offsets 26–27 (`[2, 1]` across
    /// all fixture records; exposed verbatim).
    pub sub_header_tail: [u8; 2],
    /// Decoded segment groups (`segment_count` entries), every loop's in
    /// turn when the record states more than one.
    pub segments: Vec<DecodedIgBoundary2dSegment>,
    /// How many of [`Self::segments`] each loop holds, in order, on a
    /// record that states more than one loop; empty on a record of one
    /// loop. The first loop is the outer edge of a fill and a loop inside
    /// it is a hole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loop_segment_counts: Vec<u32>,
    /// Anchor X: where the fill was seeded, on a record that names its
    /// members ([`Self::member_refs`]) — a point inside the fill on every
    /// such record of the corpora. A record that names none states no
    /// anchor (the file has `1e30` twice there), and this is then the first
    /// vertex of the ring: a point on the fill's edge, not inside it.
    pub anchor_x: f64,
    /// Anchor Y; see [`Self::anchor_x`].
    pub anchor_y: f64,
    /// `u8` flag between the anchor and the member count (`1` in
    /// fixtures; exposed verbatim).
    pub trailer_flag: u8,
    /// Trailer member references: one to a member on all but one record of
    /// the corpora, none on a record whose loop is its own geometry (a
    /// linked template's logo).
    pub member_refs: Vec<DecodedIgBoundary2dMemberRef>,
    /// `true` when the segments chain end-to-start and close back
    /// onto the first start within `1e-9` per axis (20/20 fixture
    /// records are closed).
    pub closed_loop: bool,
}

impl DecodedIgBoundary2dRecord {
    /// The segments of each loop, in order: one slice for a record of one
    /// loop, one per loop otherwise. Counts that do not add up to the
    /// segments yield what they cover and then the rest as a last loop.
    pub fn loops(&self) -> impl Iterator<Item = &[DecodedIgBoundary2dSegment]> {
        let mut rest = self.segments.as_slice();
        let mut counts = self.loop_segment_counts.iter();
        std::iter::from_fn(move || {
            if rest.is_empty() {
                return None;
            }
            let take = counts
                .next()
                .map_or(rest.len(), |count| (*count as usize).min(rest.len()));
            let (head, tail) = rest.split_at(take);
            rest = tail;
            Some(head)
        })
    }
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
            sheet_layer_ref: d.sheet_layer_ref,
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
            loop_segment_counts: d.loop_segment_counts,
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

/// Which of the native reader's three OLE states an `igSmartFrame2d`
/// record is in. Model-shaped mirror of
/// [`crate::parsers::sheet_records::SheetSmartFrame2dState`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SmartFrame2dState {
    /// `igOLENone` — the frame holds nothing.
    Empty,
    /// `igOLEEmbedded` — the framed object is stored in this drawing.
    Embedded,
    /// `igOLELinked` against an external document.
    Linked,
    /// `igOLELinked` against something inside this drawing.
    LocallyLinked,
}

/// Fully-typed model DTO mirroring
/// [`crate::parsers::sheet_records::SheetIgSmartFrame2dDecoded`] —
/// PSM type `0x003D` `igSmartFrame2d`.
///
/// The record is an OLE container frame, and a P&ID's sheet border is a
/// linked OLE object, so an embedded or linked frame's extent is the sheet
/// size. Field names come from `radsrvitem.dll sub_564464D0` rather than
/// from inference; see
/// `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DecodedIgSmartFrame2dRecord {
    /// Inclusive byte-range start covering the full PSM record.
    pub byte_start: usize,
    /// Exclusive byte-range end.
    pub byte_end: usize,
    /// PSM 14-bit type code. Always `0x003D`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Frame object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this smart frame sits on.
    pub sheet_layer_ref: u32,
    /// Content flag word at payload `+14` (record `+20`), verbatim. Bit
    /// `0x8000` is the reader's has-content test.
    pub content_flags: u32,
    /// Link flag word at payload `+26` (record `+32`), verbatim. Bits
    /// `0x40` and `0x20000` are the reader's linked / locally-linked tests.
    pub link_flags: u32,
    /// State the reader's rules put the frame in.
    pub state: SmartFrame2dState,
    /// Origin of the framed range in metres (payload `+36` / `+44`): see
    /// [`crate::parsers::sheet_records::SheetIgSmartFrame2dDecoded::range_origin_m`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_origin_m: Option<[f64; 2]>,
    /// Framed extent width in metres (payload `+76`).
    pub extent_width_m: f64,
    /// Framed extent height in metres (payload `+84`).
    pub extent_height_m: f64,
    /// Extent aspect ratio (payload `+148`); `1/sqrt(2)` on ISO A sheets.
    pub aspect_ratio: f64,
    /// Where the framed content is placed (payload `+164..+219`): see
    /// [`crate::parsers::sheet_records::SmartFrameContentTransform`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_transform: Option<SmartFrameContentTransformRecord>,
}

/// Model-shaped mirror of
/// [`crate::parsers::sheet_records::SmartFrameContentTransform`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SmartFrameContentTransformRecord {
    /// The 2×2 linear part, in stored order.
    pub linear: [f64; 4],
    /// Translation in metres.
    pub translation_m: [f64; 2],
    /// Uniform scale.
    pub scale: f64,
}

impl SmartFrameContentTransformRecord {
    /// Whether the linear part is the identity, to `1e-9`.
    #[must_use]
    pub fn linear_is_identity(&self) -> bool {
        let [a, b, c, d] = self.linear;
        (a - 1.0).abs() < 1e-9 && b.abs() < 1e-9 && c.abs() < 1e-9 && (d - 1.0).abs() < 1e-9
    }
}

impl From<crate::parsers::sheet_records::SheetIgSmartFrame2dDecoded>
    for DecodedIgSmartFrame2dRecord
{
    fn from(d: crate::parsers::sheet_records::SheetIgSmartFrame2dDecoded) -> Self {
        use crate::parsers::sheet_records::SheetSmartFrame2dState as S;
        Self {
            byte_start: d.byte_range.start,
            byte_end: d.byte_range.end,
            type_code: d.type_code,
            type_flags: d.type_flags,
            bytes_to_follow: d.bytes_to_follow,
            oid: d.oid,
            parent_ref: d.parent_ref,
            sheet_layer_ref: d.sheet_layer_ref,
            content_flags: d.content_flags,
            link_flags: d.link_flags,
            state: match d.state {
                S::Empty => SmartFrame2dState::Empty,
                S::Embedded => SmartFrame2dState::Embedded,
                S::Linked => SmartFrame2dState::Linked,
                S::LocallyLinked => SmartFrame2dState::LocallyLinked,
            },
            range_origin_m: d.range_origin_m,
            extent_width_m: d.extent_width_m,
            extent_height_m: d.extent_height_m,
            aspect_ratio: d.aspect_ratio,
            content_transform: d
                .content_transform
                .map(|t| SmartFrameContentTransformRecord {
                    linear: t.linear,
                    translation_m: t.translation_m,
                    scale: t.scale,
                }),
        }
    }
}

impl DecodedIgSmartFrame2dRecord {
    /// The extent as a sheet page, or `None` when this frame is not one.
    ///
    /// Mirrors
    /// [`crate::parsers::sheet_records::SheetIgSmartFrame2dDecoded::page_extent_m`]:
    /// an empty frame has nothing to size, and a locally-linked frame is a
    /// nested-site placeholder rather than a border.
    #[must_use]
    pub fn page_extent_m(&self) -> Option<(f64, f64)> {
        if !matches!(
            self.state,
            SmartFrame2dState::Embedded | SmartFrame2dState::Linked
        ) {
            return None;
        }
        let plausible = |v: f64| (0.05..=5.0).contains(&v);
        (plausible(self.extent_width_m) && plausible(self.extent_height_m))
            .then_some((self.extent_width_m, self.extent_height_m))
    }
}

/// Why a chain stream's record walk stopped before the stream's end.
/// Model-shaped mirror of [`crate::parsers::sheet_records::SheetChainStop`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamChainStopReason {
    /// The stream is shorter than the eight-byte header a chain stream
    /// opens with.
    ShortHeader,
    /// Fewer than six bytes are left at the stop: too few for a record's
    /// envelope.
    ShortEnvelope,
    /// The record at the stop states a length of zero.
    ZeroLength,
    /// The record at the stop states more bytes than the stream has left.
    PastEnd,
    /// The word at the stop is not one a record opens with
    /// ([`crate::parsers::sheet_records::psm_type_word_opens_a_record`]):
    /// the walk has left the record boundaries by here.
    NoSuchClass,
}

/// Where a chain stream's record walk stopped, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamChainStop {
    /// Offset of the first byte the walk did not read as a record.
    pub offset: usize,
    /// Why it stopped there.
    pub reason: StreamChainStopReason,
    /// The length the record at the stop states, when the stop is a record
    /// stating a length the stream does not hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_len: Option<u32>,
    /// The type word at the stop, when the stop is a word no record opens
    /// with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_word: Option<u16>,
    /// Offset of the last record the walk read before the stop, when it
    /// read one. The stop is where that record's stated length ends, so
    /// when the stop is not a record's start this record's length is the
    /// first thing in doubt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_record_offset: Option<usize>,
}

/// A chain stream — a `Sheet*` stream, a symbol cache's `PSMcluster0`, or a
/// style table (`StyleCluster`) — whose own framing does not hold, as
/// [`crate::parsers::sheet_records::SheetChain`] read it.
///
/// Three things can be wrong, alone or together: the stream does not open
/// with the chain magic; the walk from record to record stops before the
/// stream's end; the walk reaches the end and holds a different number of
/// live records than the header states (a stream cut off at a record
/// boundary reads like that, and nothing else about it does).
///
/// The records the chain read before a stop are decoded as on any stream,
/// and no record family decodes anything from the stop on: what the stream
/// draws is then what the file holds less what the stop hid, and this is
/// where a reader finds out. [`crate::build_normalized_geometry`] says each
/// fault in a warning, and keeps the list in
/// [`crate::NormalizedPidGeometry::chain_faults`].
///
/// What a fault says is what the chain read, which is the file's records
/// only as far as the lengths it followed were right. A stop on a word no
/// record opens with ([`StreamChainStopReason::NoSuchClass`]) is the walk
/// finding that out: the last record it read
/// ([`StreamChainStop::last_record_offset`]) states a length that ends
/// where no record starts — on the corpora's sheets that record is the one
/// whose length is wrong for 98.3% of single-length changes, and one
/// record later for the rest. A fault with no stop and a count that
/// differs places nothing: the walk reached the stream's end, records are
/// missing or were read from where none starts, and
/// [`Self::unread_bytes`] being zero does not mean no byte was lost.
///
/// One record a stop can hide decides more than its own ink: the sheet's
/// border frame, which states the page and where the drawing sits on it.
/// A drawing re-saved several times keeps its live frame after the
/// superseded copies — `DWG-0201GP06-01.pid` as the last record of the
/// stream — so a chain that stops short of it is projected as a drawing
/// with no frame is: the page from the template's name, if it names a
/// size, and no frame placement.
///
/// The heuristic probes — text runs, coordinate pairs, endpoint records —
/// are not chain readers and still scan the whole stream; what they find is
/// `ProbeOnly` and inferred evidence, as on any stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamChainFault {
    /// Path of the stream.
    pub stream_path: String,
    /// The stream's length in bytes.
    pub stream_len: usize,
    /// The stream's first word, when it is not the magic a chain stream
    /// opens with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrong_magic: Option<u32>,
    /// Where the record walk stopped, when it stopped before the stream's
    /// end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StreamChainStop>,
    /// Bytes from the stop to the stream's end, which the walk did not
    /// reach and no record family decodes; zero when the walk reached the
    /// end.
    pub unread_bytes: usize,
    /// Records the chain read. They are decoded as on any stream.
    pub records_read: usize,
    /// Of those, the ones the native reader reads: no `0x8000` in the type
    /// word.
    pub live_records_read: usize,
    /// The live-record count the stream's header states, when the stream is
    /// long enough to have a header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stated_records: Option<u32>,
}

impl StreamChainFault {
    /// What is wrong with the chain stream at `stream_path`, or `None` when
    /// it is sound ([`crate::parsers::sheet_records::SheetChain::is_sound`]).
    #[must_use]
    pub fn of(
        stream_path: &str,
        chain: &crate::parsers::sheet_records::SheetChain,
    ) -> Option<Self> {
        use crate::parsers::sheet_records::SheetChainStop;
        if chain.is_sound() {
            return None;
        }
        let stop = chain.stop.map(|stop| {
            let (reason, record_len, type_word) = match stop {
                SheetChainStop::ShortHeader => (StreamChainStopReason::ShortHeader, None, None),
                SheetChainStop::ShortEnvelope => (StreamChainStopReason::ShortEnvelope, None, None),
                SheetChainStop::ZeroLength => (StreamChainStopReason::ZeroLength, Some(0), None),
                SheetChainStop::PastEnd { stated } => {
                    (StreamChainStopReason::PastEnd, Some(stated), None)
                }
                SheetChainStop::NoSuchClass { type_word } => {
                    (StreamChainStopReason::NoSuchClass, None, Some(type_word))
                }
            };
            StreamChainStop {
                offset: chain.end,
                reason,
                record_len,
                type_word,
                last_record_offset: chain.starts.last().copied(),
            }
        });
        Some(Self {
            stream_path: stream_path.to_string(),
            stream_len: chain.len,
            wrong_magic: chain.wrong_magic(),
            stop,
            unread_bytes: chain.unread_bytes(),
            records_read: chain.starts.len(),
            live_records_read: chain.live_records,
            stated_records: chain.stated_records,
        })
    }

    /// [`Self::of`], straight from a stream's bytes.
    #[must_use]
    pub fn of_stream(stream_path: &str, data: &[u8]) -> Option<Self> {
        Self::of(
            stream_path,
            &crate::parsers::sheet_records::SheetChain::walk(data),
        )
    }

    /// Whether the header's live-record count differs from what the walk
    /// read. On a walk that stopped short it usually does, and the stop is
    /// the reason; on one that reached the stream's end it is the fault.
    #[must_use]
    pub fn count_differs(&self) -> bool {
        self.stated_records
            .is_some_and(|stated| usize::try_from(stated).ok() != Some(self.live_records_read))
    }

    /// The fault in one sentence, naming the stream and, when the walk
    /// stopped short, the offset it stopped at and the bytes left unread:
    /// [`Self::describe_framing`], and what it means for a stream that
    /// draws.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{}; what the stream draws may be incomplete",
            self.describe_framing()
        )
    }

    /// What is wrong with the stream's framing, and nothing about what
    /// follows from it: the stream, and, when the walk stopped short, the
    /// offset it stopped at, why, the records read before it and the bytes
    /// left unread; the header's count against the chain's.
    ///
    /// [`Self::describe`] goes on to say that what the stream draws may be
    /// incomplete, which is the consequence for a sheet. A reader of
    /// something else says its own after this sentence: the publish line
    /// reads attribute rows and labels, from streams some of which draw
    /// nothing, and says what its XML is short of
    /// (`PublishChainFault::describe` in `publish::pid_load`).
    #[must_use]
    pub fn describe_framing(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(found) = self.wrong_magic {
            parts.push(format!(
                "the stream opens with 0x{found:08X}, not the chain magic 0x{:08X}",
                crate::parsers::sheet_records::SHEET_STREAM_MAGIC
            ));
        }
        match &self.stop {
            Some(stop) => {
                let why = match stop.reason {
                    StreamChainStopReason::ShortHeader => {
                        "the stream is shorter than the 8-byte header a chain opens with"
                            .to_string()
                    }
                    StreamChainStopReason::ShortEnvelope => format!(
                        "{} byte(s) are left, fewer than the 6 of a record's envelope",
                        self.unread_bytes
                    ),
                    StreamChainStopReason::ZeroLength => {
                        "a record states a length of zero".to_string()
                    }
                    StreamChainStopReason::PastEnd => format!(
                        "a record states {} byte(s) of payload and {} are left",
                        stop.record_len.unwrap_or_default(),
                        self.unread_bytes.saturating_sub(6)
                    ),
                    StreamChainStopReason::NoSuchClass => format!(
                        "the type word 0x{:04X} is not one a record opens with",
                        stop.type_word.unwrap_or_default()
                    ),
                };
                // The record whose stated length ends at the stop. When
                // the stop is not a record's start, its length is where to
                // look first.
                let last = stop
                    .last_record_offset
                    .map(|at| format!(", the last of them at +{at}"))
                    .unwrap_or_default();
                parts.push(format!(
                    "it stops at +{} of {} bytes, where {why}: the chain read {} record(s) \
                     before the stop{last}, and the {} byte(s) from it on are not read",
                    stop.offset, self.stream_len, self.records_read, self.unread_bytes
                ));
                if let Some(stated) = self.stated_records {
                    parts.push(format!(
                        "the stream's header states {stated} live record(s) and the chain read \
                         {} before the stop",
                        self.live_records_read
                    ));
                }
            }
            None => {
                if let Some(stated) = self.stated_records.filter(|_| self.count_differs()) {
                    parts.push(format!(
                        "it runs to the stream's end with {} live record(s) where the header \
                         states {stated}, and nothing in it says where they differ",
                        self.live_records_read
                    ));
                }
            }
        }
        format!(
            "the record chain of {} does not hold: {}",
            self.stream_path,
            parts.join("; ")
        )
    }
}

/// A live `igSmartFrame2d` record of one of a document's `Sheet*` streams
/// that the frame decoder refuses
/// ([`crate::parsers::sheet_records::refused_smartframes`]).
///
/// A frame record is taken for a frame only when the ratio it states is
/// the height over the width it states and the two are lengths: that is
/// what tells a frame from the bytes a walk reads off a broken chain. A
/// drawing's own border frame that fails the rule — a ratio written wrong,
/// a side of zero — states no page and places nothing, and the drawing is
/// laid out as a sheet with no frame is.
#[derive(Debug, Clone, PartialEq)]
pub struct RefusedSmartFrame {
    /// Path of the stream the record is in.
    pub stream_path: String,
    /// The record, and which part of the frame rule it fails.
    pub refused: crate::parsers::sheet_records::SheetSmartFrameRefused,
}

impl RefusedSmartFrame {
    /// Whether the record could have stated the sheet's page: one whose
    /// flag words read as an embedded or a linked frame, or cannot be read.
    /// An empty frame and one linked inside the drawing state no page when
    /// they decode.
    #[must_use]
    pub fn may_state_a_page(&self) -> bool {
        use crate::parsers::sheet_records::SheetSmartFrame2dState as State;
        !matches!(
            self.refused.state,
            Some(State::Empty | State::LocallyLinked)
        )
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
