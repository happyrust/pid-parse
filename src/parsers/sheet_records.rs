//! Conservative `Sheet*` record-shape inventory.
//!
//! This module is the bridge between broad byte probing and future typed
//! geometry decoders.  It records bounded evidence for marker records,
//! object `field_x` windows, text runs, and coordinate hints without claiming
//! any of them as decoded CAD primitives.

use crate::parsers::sheet_probe::{
    field_x_window_features, field_x_windows, score_sheet_text_window_candidates,
    sheet_text_window_candidates, SheetCoordinateHint, SheetProbeReport,
    SheetTextWindowScoreReason,
};
use std::collections::{BTreeMap, BTreeSet};

type CoordinatePageMetadataGroupKey = (
    SheetCoordinatePageMetadataCandidateKind,
    Option<u16>,
    usize,
    usize,
    usize,
    usize,
    usize,
);

/// One conservative record-shape inventory for a `Sheet*` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRecordShapeInventory {
    /// Candidate records and evidence windows sorted by byte offset.
    pub records: Vec<SheetRecordShapeEvidence>,
    /// Probe-level `0x89` record-type counts copied from [`SheetProbeReport`].
    pub record_type_counts: BTreeMap<String, usize>,
}

/// Investigation summary for possible primitive line record families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPrimitiveLineInvestigationReport {
    /// Marker-range groups sorted by support, then marker type and range length.
    pub groups: Vec<SheetPrimitiveLineShapeGroup>,
}

/// Investigation summary for possible polyline/circle/arc record families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCurvePrimitiveInvestigationReport {
    /// Marker-range groups with enough numeric shape evidence to review.
    pub groups: Vec<SheetCurvePrimitiveShapeGroup>,
}

/// Investigation summary for possible coordinate/page metadata records.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetCoordinatePageMetadataInvestigationReport {
    /// Marker-range groups that contain coordinate-domain or page metadata evidence.
    pub candidates: Vec<SheetCoordinatePageMetadataCandidate>,
    /// Compact top-ranked evidence groups for cross-fixture reporting.
    pub top_evidence: Vec<SheetCoordinatePageMetadataTopEvidence>,
    /// Bounds of i32 coordinate hints surfaced by the existing Sheet probe.
    pub coordinate_hint_bounds: Option<SheetI32CoordinateBounds>,
    /// Bounds of f64 coordinate hints linked through repeated marker evidence.
    pub f64_coordinate_bounds: Option<SheetF64CoordinateBounds>,
    /// Total normalized f64 coordinate-like pairs observed in marker payloads.
    pub normalized_f64_pair_count: usize,
    /// Total scalar values that match inferred page dimensions, when available.
    pub page_dimension_scalar_matches: usize,
}

/// One marker-range candidate for coordinate/page metadata investigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCoordinatePageMetadataCandidate {
    /// Marker type decoded after `0x89`.
    pub marker_type: Option<u16>,
    /// Bounded range length for this marker shape.
    pub range_len: usize,
    /// Number of records with the same marker/range/numeric shape.
    pub support: usize,
    /// Count of plausible i32 coordinate-like pairs inside the example payload.
    pub candidate_i32_pairs: usize,
    /// Count of plausible f64 coordinate-like pairs inside the example payload.
    pub candidate_f64_pairs: usize,
    /// Count of normalized f64 pairs inside the example payload.
    pub normalized_f64_pairs: usize,
    /// Count of scalar values matching inferred page dimensions.
    pub page_dimension_scalar_matches: usize,
    /// Coarse investigation category. This is not decoded page metadata.
    pub candidate_kind: SheetCoordinatePageMetadataCandidateKind,
    /// Example record offset for manual byte review.
    pub example_offset: usize,
    /// Example range start.
    pub example_range_start: usize,
    /// Example range end.
    pub example_range_end: usize,
    /// Hex prefix of the example range for bounded byte-window review.
    pub example_hex_prefix: String,
    /// Human-readable investigation notes; evidence-only.
    pub investigation_notes: Vec<String>,
}

/// Compact summary for the strongest coordinate/page metadata candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCoordinatePageMetadataTopEvidence {
    /// Marker type decoded after `0x89`.
    pub marker_type: Option<u16>,
    /// Bounded range length for this marker shape.
    pub range_len: usize,
    /// Number of records with the same marker/range/numeric shape.
    pub support: usize,
    /// Coarse investigation category.
    pub candidate_kind: SheetCoordinatePageMetadataCandidateKind,
    /// Count of plausible i32 coordinate-like pairs inside the example payload.
    pub candidate_i32_pairs: usize,
    /// Count of plausible f64 coordinate-like pairs inside the example payload.
    pub candidate_f64_pairs: usize,
    /// Count of normalized f64 pairs inside the example payload.
    pub normalized_f64_pairs: usize,
    /// Count of scalar values matching inferred page dimensions.
    pub page_dimension_scalar_matches: usize,
    /// Example record offset for manual byte review.
    pub example_offset: usize,
    /// Hex prefix of the example range for bounded byte-window review.
    pub example_hex_prefix: String,
}

/// Coarse shape category for coordinate/page metadata investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SheetCoordinatePageMetadataCandidateKind {
    /// Scalars matching inferred page dimensions were found.
    PageDimensionScalarLike,
    /// Normalized f64 coordinate pairs were found.
    NormalizedF64CoordinateLike,
    /// i32 coordinate pairs may describe bounds or origin-like values.
    I32CoordinateDomainLike,
    /// Numeric payload exists but is not selective enough to classify.
    MixedNumeric,
    /// Not enough numeric evidence for page metadata review.
    InsufficientEvidence,
}

/// Bounds for i32 coordinate-like evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetI32CoordinateBounds {
    /// Minimum X value.
    pub min_x: i32,
    /// Maximum X value.
    pub max_x: i32,
    /// Minimum Y value.
    pub min_y: i32,
    /// Maximum Y value.
    pub max_y: i32,
    /// Number of coordinate pairs included in the bounds.
    pub count: usize,
}

/// Bounds for f64 coordinate-like evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetF64CoordinateBounds {
    /// Minimum X value.
    pub min_x: f64,
    /// Maximum X value.
    pub max_x: f64,
    /// Minimum Y value.
    pub min_y: f64,
    /// Maximum Y value.
    pub max_y: f64,
    /// Number of coordinate pairs included in the bounds.
    pub count: usize,
}

/// Phase 25-A spatial-analysis report for normalized `(x, y)` f64 pairs.
///
/// The report is **read-only evidence**: it characterises how
/// `normalized_f64_pair` evidence is distributed across a single sheet's
/// normalized `[0, 1]²` coordinate space. It does **not** promote any
/// entity or change `PidPageTransform` state. Downstream consumers may
/// use the cluster ids as topology hints, never as coordinate
/// authority.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSpatialAnalysisReport {
    /// Total normalized f64 pairs that contributed to the analysis.
    pub pair_count: usize,
    /// Grid resolution used for bucketing (N×N). Records the algorithm
    /// parameter so ratchets can reproduce the result.
    pub grid_resolution: usize,
    /// Connected-component clusters discovered after grid bucketing
    /// and 4-neighbour merging. Empty when `pair_count == 0`.
    pub clusters: Vec<SheetSpatialCluster>,
    /// True when the analysis cannot meaningfully separate clusters
    /// (zero pairs, single cluster, or all pairs collapsed into one
    /// grid cell). Used by Phase 25-A Stop-And-Challenge as the
    /// negative-evidence trigger.
    pub uniform_distribution: bool,
}

/// A single spatial cluster inside [`SheetSpatialAnalysisReport`].
///
/// Cluster ids are sheet-local (start at `0` and increment in
/// connected-component discovery order). They are stable across runs
/// for the same input but must not be compared across sheets or
/// fixtures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetSpatialCluster {
    /// Sheet-local cluster id (0-based, deterministic for the same
    /// input).
    pub id: u32,
    /// Number of normalized f64 pairs that fell into this cluster.
    pub pair_count: usize,
    /// Bounding box in normalized `[0, 1]²` space:
    /// `((min_x, min_y), (max_x, max_y))`.
    pub bbox: ((f64, f64), (f64, f64)),
    /// Centroid (arithmetic mean of pair coordinates) in normalized
    /// `[0, 1]²` space.
    pub centroid: (f64, f64),
}

/// One marker-range shape inspected for curve primitive potential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCurvePrimitiveShapeGroup {
    /// Marker type decoded after `0x89`.
    pub marker_type: Option<u16>,
    /// Bounded range length for this marker shape.
    pub range_len: usize,
    /// Number of records with the same marker/range/numeric shape.
    pub support: usize,
    /// Count of plausible i32 coordinate-like pairs inside the example payload.
    pub candidate_i32_pairs: usize,
    /// Count of plausible f64 coordinate-like pairs inside the example payload.
    pub candidate_f64_pairs: usize,
    /// Total plausible numeric pair count inside the example payload.
    pub numeric_pair_count: usize,
    /// Numeric pair density scaled to 1024 bytes for range-size comparison.
    pub numeric_pairs_per_1kb: usize,
    /// True when the range is compact enough for ordered-vertex review.
    pub compact_vertex_chain_candidate: bool,
    /// Coarse investigation category. This is not decoded geometry.
    pub candidate_kind: SheetCurvePrimitiveCandidateKind,
    /// Example record offset for manual byte review.
    pub example_offset: usize,
    /// Example range start.
    pub example_range_start: usize,
    /// Example range end.
    pub example_range_end: usize,
    /// Bounded numeric samples from the example payload.
    pub numeric_samples: Vec<SheetPrimitiveLineNumericSample>,
    /// Numeric sample offsets relative to `example_range_start`.
    pub numeric_sample_relative_offsets: Vec<usize>,
    /// Consecutive byte deltas between numeric sample relative offsets.
    pub numeric_sample_offset_deltas: Vec<usize>,
    /// Hex prefix of the example range for bounded byte-window review.
    pub example_hex_prefix: String,
    /// Best non-overlapping i32 point sequence found in the example payload.
    pub i32_point_sequence: Option<SheetI32PointSequenceCandidate>,
    /// Human-readable investigation notes; evidence-only.
    pub investigation_notes: Vec<String>,
}

/// Non-overlapping i32 point sequence candidate inside a marker payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetI32PointSequenceCandidate {
    /// Absolute byte offset of the first point.
    pub offset: usize,
    /// Byte offset relative to the marker-range start.
    pub relative_offset: usize,
    /// `relative_offset % 4`; non-zero values need subfield split review.
    pub relative_alignment_mod4: usize,
    /// Number of consecutive non-overlapping i32 pairs.
    pub point_count: usize,
    /// Byte stride between points. Currently fixed to 8 for i32 `(x, y)`.
    pub byte_stride: usize,
    /// First few points formatted for investigation output.
    pub sample_points: Vec<String>,
}

/// Coarse shape category for curve primitive investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SheetCurvePrimitiveCandidateKind {
    /// Enough point-like pairs to review as an ordered vertex chain.
    PolylineLike,
    /// Compact f64-heavy shape worth reviewing as center/radius/angle data.
    CircleArcLike,
    /// Numeric payload exists but is not selective enough to classify.
    MixedNumeric,
    /// Not enough numeric evidence for curve primitive review.
    InsufficientNumeric,
}

/// Investigation summary for possible text placement records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetTextPlacementInvestigationReport {
    /// Ranked text/coordinate candidates enriched with nearest `field_x` evidence.
    pub candidates: Vec<SheetTextPlacementCandidate>,
    /// Raw text/coordinate candidate count before quality filtering.
    pub raw_candidate_count: usize,
    /// Number of candidates rejected because the decoded text looked binary-like.
    pub rejected_candidate_count: usize,
}

/// Investigation summary for possible symbol placement records.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSymbolPlacementInvestigationReport {
    /// Ranked object/Sheet candidates that may represent symbol placement.
    pub candidates: Vec<SheetSymbolPlacementCandidate>,
    /// Number of unique document-level `JSite` symbol paths available.
    pub symbol_path_catalog_count: usize,
}

/// Minimal object identity needed to investigate symbol placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSymbolPlacementObject {
    /// Object Dynamic Attributes `field_x`.
    pub field_x: u32,
    /// Object `DrawingID`.
    pub drawing_id: String,
    /// `ModelItemType` from the DA object graph.
    pub item_type: String,
    /// `DrawingItemType`, commonly `Symbol` for renderable instances.
    pub drawing_item_type: Option<String>,
    /// Object-level symbol path, only when a separate evidence pass proved it.
    pub symbol_path: Option<String>,
}

/// One investigation-only candidate for a symbol placement record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSymbolPlacementCandidate {
    /// Investigation score; higher means better review priority.
    pub score: i32,
    /// Object Dynamic Attributes `field_x`.
    pub field_x: u32,
    /// Object `DrawingID`.
    pub drawing_id: String,
    /// `ModelItemType` from the DA object graph.
    pub item_type: String,
    /// `DrawingItemType`, commonly `Symbol` for renderable instances.
    pub drawing_item_type: Option<String>,
    /// Sheet byte offset where this `field_x` was observed.
    pub field_x_offset: Option<usize>,
    /// Candidate insertion byte offset.
    pub position_offset: Option<usize>,
    /// Candidate insertion X coordinate.
    pub x: Option<f64>,
    /// Candidate insertion Y coordinate.
    pub y: Option<f64>,
    /// Coordinate encoding family, when a position candidate exists.
    pub position_encoding: Option<SheetSymbolPlacementPositionEncoding>,
    /// Resolved symbol path. `None` means only a document-level catalog exists.
    pub symbol_path: Option<String>,
    /// Number of document-level `JSite` symbol paths available.
    pub symbol_path_candidate_count: usize,
    /// Hex bytes around the `field_x` evidence window.
    pub field_x_hex: String,
    /// Human-readable investigation reasons; evidence-only.
    pub notes: Vec<String>,
}

/// Coordinate encoding family for a symbol placement position candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSymbolPlacementPositionEncoding {
    /// Adjacent little-endian `i32` pair.
    I32Pair,
    /// Adjacent little-endian `f64` pair.
    F64Pair,
}

/// One investigation-only candidate for a positioned text record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetTextPlacementCandidate {
    /// Investigation score inherited from the text-window scorer.
    pub score: i32,
    /// Decoded text payload.
    pub text: String,
    /// Byte offset where the text starts.
    pub text_offset: usize,
    /// Byte offset where the candidate coordinate starts.
    pub coordinate_offset: usize,
    /// Candidate insertion X coordinate.
    pub x: i32,
    /// Candidate insertion Y coordinate.
    pub y: i32,
    /// Minimum byte distance between text and coordinate ranges.
    pub byte_distance: usize,
    /// Whether text and coordinate fit inside the same probed chunk.
    pub same_chunk: bool,
    /// Nearest object `field_x` value, when a field window exists.
    pub nearest_field_x: Option<u32>,
    /// Nearest object `field_x` byte offset, when a field window exists.
    pub nearest_field_x_offset: Option<usize>,
    /// Signed byte delta from nearest `field_x` to text.
    pub field_x_delta_from_text: Option<isize>,
    /// Signed byte delta from nearest `field_x` to coordinate.
    pub field_x_delta_from_coordinate: Option<isize>,
    /// Hex bytes covering the decoded text run.
    pub text_hex: String,
    /// Hex bytes covering the candidate coordinate pair.
    pub coordinate_hex: String,
    /// Human-readable investigation reasons; evidence-only.
    pub notes: Vec<String>,
}

/// One repeated marker-range shape inspected for primitive-line potential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPrimitiveLineShapeGroup {
    /// Marker type decoded after `0x89`.
    pub marker_type: Option<u16>,
    /// Bounded range length for this marker shape.
    pub range_len: usize,
    /// Number of records with the same marker/range/numeric-field shape.
    pub support: usize,
    /// Count of plausible i32 coordinate-like pairs inside the example range.
    pub candidate_i32_pairs: usize,
    /// Count of plausible f64 coordinate-like pairs inside the example range.
    pub candidate_f64_pairs: usize,
    /// Investigation-only ranking score; higher means better review priority.
    pub investigation_score: i32,
    /// Example record offset for manual byte review.
    pub example_offset: usize,
    /// Example range start.
    pub example_range_start: usize,
    /// Example range end.
    pub example_range_end: usize,
    /// Bounded numeric samples from the example range for manual review.
    pub numeric_samples: Vec<SheetPrimitiveLineNumericSample>,
    /// Numeric sample offsets relative to `example_range_start`.
    pub numeric_sample_relative_offsets: Vec<usize>,
    /// Consecutive byte deltas between numeric sample relative offsets.
    pub numeric_sample_offset_deltas: Vec<usize>,
    /// Hex prefix of the example range for bounded byte-window review.
    pub example_hex_prefix: String,
    /// Numeric sample offsets that exactly match existing coordinate hints.
    pub coordinate_hint_match_offsets: Vec<usize>,
    /// Nearest byte delta from a numeric sample to an existing coordinate hint.
    pub nearest_coordinate_hint_delta: Option<isize>,
    /// Nearest byte delta from the marker offset to an object `field_x` window.
    pub nearest_field_x_delta: Option<isize>,
    /// Short reasons that explain the investigation score.
    pub investigation_notes: Vec<String>,
}

/// One candidate numeric pair inside a marker-range example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPrimitiveLineNumericSample {
    /// Byte offset of the candidate pair inside the Sheet stream.
    pub offset: usize,
    /// Numeric encoding family.
    pub kind: SheetPrimitiveLineNumericSampleKind,
    /// Formatted pair value; evidence-only, not a decoded coordinate.
    pub value: String,
}

/// Numeric encoding family for a [`SheetPrimitiveLineNumericSample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetPrimitiveLineNumericSampleKind {
    /// Adjacent little-endian `i32` pair.
    I32Pair,
    /// Adjacent little-endian `f64` pair.
    F64Pair,
}

/// Conservative category for a [`SheetRecordShapeEvidence`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SheetRecordShapeKind {
    /// A `0x89 <type-le>` marker run.
    Marker,
    /// A bounded window containing a semantic object `field_x`.
    FieldXWindow,
    /// A printable text run surfaced by the Sheet probe.
    TextRun,
    /// A coordinate-like pair surfaced by the Sheet probe.
    CoordinateHint,
}

/// Bounded evidence for one candidate Sheet record shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRecordShapeEvidence {
    /// Evidence category.
    pub kind: SheetRecordShapeKind,
    /// Primary byte offset for this evidence row.
    pub offset: usize,
    /// Inclusive start of the bounded byte range.
    pub range_start: usize,
    /// Exclusive end of the bounded byte range.
    pub range_end: usize,
    /// Marker type decoded after `0x89`, when this row came from a marker.
    pub marker_type: Option<u16>,
    /// Object `field_x` matched in little-endian form, when present.
    pub field_x: Option<u32>,
    /// Endpoint-record start offset when the field hit sits inside an endpoint.
    pub endpoint_record_start: Option<usize>,
    /// Probe chunk start containing the evidence row, when known.
    pub chunk_start: Option<usize>,
    /// Probe chunk end containing the evidence row, when known.
    pub chunk_end: Option<usize>,
    /// Nearby coordinate offset from the existing Sheet probe, when known.
    pub candidate_coordinate_offset: Option<usize>,
    /// Nearby text offset from the existing Sheet probe, when known.
    pub candidate_text_offset: Option<usize>,
    /// f64 coordinate offset from repeated marker evidence, when known.
    pub f64_coordinate_offset: Option<usize>,
    /// f64 marker offset from repeated marker evidence, when known.
    pub f64_marker_offset: Option<usize>,
    /// Short human-readable evidence notes for investigation output.
    pub notes: Vec<String>,
}

/// Build a conservative Sheet record-shape inventory from existing probe data.
///
/// `field_xs` should be the set of semantic object fields worth correlating
/// against the Sheet bytes.  Passing an empty slice still inventories marker,
/// text, and coordinate evidence.
pub fn sheet_record_shape_inventory(
    data: &[u8],
    probe: &SheetProbeReport,
    field_xs: &[u32],
) -> SheetRecordShapeInventory {
    let mut records = Vec::new();
    records.extend(marker_shape_evidence(data, probe));
    records.extend(field_x_shape_evidence(data, probe, field_xs));
    records.extend(text_shape_evidence(probe));
    records.extend(coordinate_shape_evidence(probe));
    records.sort_by_key(|record| (record.offset, record.kind));

    SheetRecordShapeInventory {
        records,
        record_type_counts: probe.record_type_counts.clone(),
    }
}

/// Build an investigation-only report for possible coordinate/page metadata.
///
/// The report collects coordinate-domain and page-dimension evidence without
/// decoding units, bounds, or transforms. It must not be used to mark
/// [`crate::geometry::PidPageTransform`] as available until a typed record is
/// proven.
pub fn coordinate_page_metadata_investigation_report(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
    inferred_page_dimensions_mm: Option<(f64, f64)>,
) -> SheetCoordinatePageMetadataInvestigationReport {
    let coordinate_hint_bounds = i32_coordinate_bounds(data, inventory);
    let f64_coordinate_bounds = f64_coordinate_bounds(data, inventory);
    let mut groups: BTreeMap<CoordinatePageMetadataGroupKey, SheetCoordinatePageMetadataCandidate> =
        BTreeMap::new();

    for record in inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::Marker)
    {
        let range_len = record.range_end.saturating_sub(record.range_start);
        let Some(bytes) = data.get(record.range_start..record.range_end) else {
            continue;
        };
        let numeric_bytes = bytes.get(4..).unwrap_or_default();
        let candidate_i32_pairs = plausible_i32_pair_count(numeric_bytes);
        let candidate_f64_pairs = plausible_f64_pair_count(numeric_bytes);
        let normalized_f64_pairs = normalized_f64_pair_count(numeric_bytes);
        let page_dimension_scalar_matches =
            page_dimension_scalar_match_count(numeric_bytes, inferred_page_dimensions_mm);
        let candidate_kind = classify_coordinate_page_metadata_candidate(
            candidate_i32_pairs,
            candidate_f64_pairs,
            normalized_f64_pairs,
            page_dimension_scalar_matches,
        );
        let example_hex_prefix = hex_prefix(bytes, 96);
        let investigation_notes = coordinate_page_metadata_notes(
            candidate_kind,
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
            normalized_f64_pairs,
            page_dimension_scalar_matches,
        );
        let key = (
            candidate_kind,
            record.marker_type,
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
            normalized_f64_pairs,
            page_dimension_scalar_matches,
        );
        groups
            .entry(key)
            .and_modify(|group| group.support += 1)
            .or_insert_with(|| SheetCoordinatePageMetadataCandidate {
                marker_type: record.marker_type,
                range_len,
                support: 1,
                candidate_i32_pairs,
                candidate_f64_pairs,
                normalized_f64_pairs,
                page_dimension_scalar_matches,
                candidate_kind,
                example_offset: record.offset,
                example_range_start: record.range_start,
                example_range_end: record.range_end,
                example_hex_prefix,
                investigation_notes,
            });
    }

    let mut candidates = groups.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        coordinate_page_metadata_rank(right)
            .cmp(&coordinate_page_metadata_rank(left))
            .then_with(|| right.support.cmp(&left.support))
            .then_with(|| left.marker_type.cmp(&right.marker_type))
            .then_with(|| left.range_len.cmp(&right.range_len))
    });

    let normalized_f64_pair_count = candidates
        .iter()
        .map(|candidate| candidate.normalized_f64_pairs * candidate.support)
        .sum();
    let page_dimension_scalar_matches = candidates
        .iter()
        .map(|candidate| candidate.page_dimension_scalar_matches * candidate.support)
        .sum();
    let top_evidence = candidates
        .iter()
        .take(8)
        .map(|candidate| SheetCoordinatePageMetadataTopEvidence {
            marker_type: candidate.marker_type,
            range_len: candidate.range_len,
            support: candidate.support,
            candidate_kind: candidate.candidate_kind,
            candidate_i32_pairs: candidate.candidate_i32_pairs,
            candidate_f64_pairs: candidate.candidate_f64_pairs,
            normalized_f64_pairs: candidate.normalized_f64_pairs,
            page_dimension_scalar_matches: candidate.page_dimension_scalar_matches,
            example_offset: candidate.example_offset,
            example_hex_prefix: candidate.example_hex_prefix.clone(),
        })
        .collect();

    SheetCoordinatePageMetadataInvestigationReport {
        candidates,
        top_evidence,
        coordinate_hint_bounds,
        f64_coordinate_bounds,
        normalized_f64_pair_count,
        page_dimension_scalar_matches,
    }
}

/// Default grid resolution (N) used by Phase 25-A spatial analysis
/// when a caller has no fixture-specific tuning. Buckets the
/// normalized `[0, 1]²` space into a `20 × 20` grid — the value the
/// Slice A probe validated as producing non-uniform cross-fixture
/// cluster signal.
pub const SPATIAL_ANALYSIS_DEFAULT_GRID_N: usize = 20;

/// Phase 25-A: spatial-distribution analysis of normalized f64 pairs.
///
/// Buckets pairs into an `n_grid × n_grid` grid covering the
/// normalized `[0, 1]²` coordinate space, then performs 4-neighbour
/// connected-component merging on non-empty cells to discover spatial
/// clusters. Returns a [`SheetSpatialAnalysisReport`] documenting
/// cluster count, per-cluster bbox / centroid / pair count, and a
/// `uniform_distribution` flag used by Phase 25-A Stop-And-Challenge.
///
/// The algorithm is deterministic: the same input produces the same
/// cluster ids (0-based, in connected-component discovery order). It
/// is panic-safe for adversarial values — `NaN`, `Infinity`, and
/// out-of-range coordinates are clamped to `[0, 1]` before bucketing.
///
/// `n_grid == 0` is silently promoted to `1` (a single cluster
/// covering the whole sheet). Pairs slices that are empty produce
/// a zero-cluster report with `uniform_distribution == true`.
///
/// This is **investigation-only evidence**. It must not be used to
/// promote any entity to [`crate::geometry::PidPageTransform::Available`]
/// or to mark any record as decoded.
pub fn coordinate_pair_spatial_analysis(
    pairs: &[(f64, f64)],
    n_grid: usize,
) -> SheetSpatialAnalysisReport {
    let grid_resolution = n_grid.max(1);
    if pairs.is_empty() {
        return SheetSpatialAnalysisReport {
            pair_count: 0,
            grid_resolution,
            clusters: Vec::new(),
            uniform_distribution: true,
        };
    }
    let grid = spatial_grid_bucket(pairs, grid_resolution);
    let clusters = spatial_connected_components(&grid, pairs, grid_resolution);
    let uniform_distribution = clusters.len() <= 1;
    SheetSpatialAnalysisReport {
        pair_count: pairs.len(),
        grid_resolution,
        clusters,
        uniform_distribution,
    }
}

fn spatial_clamp_coordinate(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn spatial_grid_bucket(pairs: &[(f64, f64)], n: usize) -> Vec<Vec<Vec<usize>>> {
    let mut grid: Vec<Vec<Vec<usize>>> = (0..n)
        .map(|_| (0..n).map(|_| Vec::new()).collect())
        .collect();
    if n == 0 {
        return grid;
    }
    let nf = n as f64;
    for (idx, (x, y)) in pairs.iter().copied().enumerate() {
        let xc = spatial_clamp_coordinate(x);
        let yc = spatial_clamp_coordinate(y);
        let cx = ((xc * nf) as usize).min(n - 1);
        let cy = ((yc * nf) as usize).min(n - 1);
        grid[cy][cx].push(idx);
    }
    grid
}

fn spatial_connected_components(
    grid: &[Vec<Vec<usize>>],
    pairs: &[(f64, f64)],
    n: usize,
) -> Vec<SheetSpatialCluster> {
    if n == 0 || pairs.is_empty() {
        return Vec::new();
    }
    let mut visited = vec![vec![false; n]; n];
    let mut clusters = Vec::new();

    for sy in 0..n {
        for sx in 0..n {
            if visited[sy][sx] || grid[sy][sx].is_empty() {
                continue;
            }
            let mut stack = vec![(sx, sy)];
            let mut pair_indices = Vec::<usize>::new();
            while let Some((x, y)) = stack.pop() {
                if visited[y][x] || grid[y][x].is_empty() {
                    continue;
                }
                visited[y][x] = true;
                pair_indices.extend_from_slice(&grid[y][x]);
                if x > 0 {
                    stack.push((x - 1, y));
                }
                if x + 1 < n {
                    stack.push((x + 1, y));
                }
                if y > 0 {
                    stack.push((x, y - 1));
                }
                if y + 1 < n {
                    stack.push((x, y + 1));
                }
            }

            let Some((&first_idx, rest)) = pair_indices.split_first() else {
                continue;
            };
            let (fx, fy) = pairs[first_idx];
            let fx = spatial_clamp_coordinate(fx);
            let fy = spatial_clamp_coordinate(fy);
            let mut min_x = fx;
            let mut max_x = fx;
            let mut min_y = fy;
            let mut max_y = fy;
            let mut sum_x = fx;
            let mut sum_y = fy;
            for &idx in rest {
                let (px, py) = pairs[idx];
                let px = spatial_clamp_coordinate(px);
                let py = spatial_clamp_coordinate(py);
                if px < min_x {
                    min_x = px;
                }
                if px > max_x {
                    max_x = px;
                }
                if py < min_y {
                    min_y = py;
                }
                if py > max_y {
                    max_y = py;
                }
                sum_x += px;
                sum_y += py;
            }
            let count = pair_indices.len() as f64;
            let cluster_id = clusters.len() as u32;
            clusters.push(SheetSpatialCluster {
                id: cluster_id,
                pair_count: pair_indices.len(),
                bbox: ((min_x, min_y), (max_x, max_y)),
                centroid: (sum_x / count, sum_y / count),
            });
        }
    }

    clusters
}

/// Build an investigation-only report for potential text placement records.
///
/// The report deliberately does not emit decoded text geometry. It only joins
/// probe text/coordinate candidates with nearby `field_x` evidence so repeated
/// record shapes can be reviewed before promoting [`crate::geometry::PidGraphicKind::Text`].
pub fn text_placement_investigation_report(
    data: &[u8],
    probe: &SheetProbeReport,
    inventory: &SheetRecordShapeInventory,
    radius: usize,
) -> SheetTextPlacementInvestigationReport {
    let text_candidates = sheet_text_window_candidates(
        &probe.text_runs,
        &probe.coordinate_hints,
        &probe.chunks,
        radius,
    );
    let scores = score_sheet_text_window_candidates(&text_candidates);
    let field_records = inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::FieldXWindow)
        .collect::<Vec<_>>();

    let raw_candidate_count = scores.len();
    let rejected_candidate_count = scores
        .iter()
        .filter(|score| !text_score_quality_passed(score.reasons.as_slice()))
        .count();
    let candidates = scores
        .iter()
        .filter(|score| text_score_quality_passed(score.reasons.as_slice()))
        .map(|score| {
            let candidate = &score.candidate;
            let nearest_field = nearest_field_record(
                &field_records,
                candidate.text_offset,
                candidate.coordinate_offset,
            );
            let mut notes = score
                .reasons
                .iter()
                .map(|reason| format!("{reason:?}"))
                .collect::<Vec<_>>();
            if let Some(field) = nearest_field {
                let text_delta = signed_delta(field.offset, candidate.text_offset);
                let coordinate_delta = signed_delta(field.offset, candidate.coordinate_offset);
                if text_delta
                    .unsigned_abs()
                    .min(coordinate_delta.unsigned_abs())
                    <= 128
                {
                    notes.push("near_field_x_window".to_string());
                }
            } else {
                notes.push("no_field_x_window".to_string());
            }
            notes.push("probe_only_no_text_geometry_promotion".to_string());

            SheetTextPlacementCandidate {
                score: score.score,
                text: candidate.text.clone(),
                text_offset: candidate.text_offset,
                coordinate_offset: candidate.coordinate_offset,
                x: candidate.x,
                y: candidate.y,
                byte_distance: candidate.byte_distance,
                same_chunk: candidate.same_chunk,
                nearest_field_x: nearest_field.and_then(|field| field.field_x),
                nearest_field_x_offset: nearest_field.map(|field| field.offset),
                field_x_delta_from_text: nearest_field
                    .map(|field| signed_delta(field.offset, candidate.text_offset)),
                field_x_delta_from_coordinate: nearest_field
                    .map(|field| signed_delta(field.offset, candidate.coordinate_offset)),
                text_hex: hex_range(data, candidate.text_offset, candidate.text_byte_len),
                coordinate_hex: hex_range(data, candidate.coordinate_offset, 8),
                notes,
            }
        })
        .collect();

    SheetTextPlacementInvestigationReport {
        candidates,
        raw_candidate_count,
        rejected_candidate_count,
    }
}

/// Build an investigation-only report for potential symbol placement records.
///
/// This joins DA object identities to Sheet `field_x` evidence and the
/// document-level `JSite` symbol catalog. It intentionally does not promote
/// [`crate::geometry::PidGraphicKind::SymbolInstance`] until a source-backed
/// object-to-symbol-path linkage is proven.
pub fn symbol_placement_investigation_report(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
    objects: &[SheetSymbolPlacementObject],
    symbol_paths: &[String],
) -> SheetSymbolPlacementInvestigationReport {
    let symbol_path_catalog = symbol_paths.iter().collect::<BTreeSet<_>>();
    let symbol_path_catalog_count = symbol_path_catalog.len();
    let mut candidates = objects
        .iter()
        .filter_map(|object| {
            let field_record = field_record_for_field_x(inventory, object.field_x)?;
            let position = symbol_position_candidate(data, field_record);
            let mut score = 0;
            let mut notes = Vec::new();

            if object.drawing_item_type.as_deref() == Some("Symbol") {
                score += 40;
                notes.push("drawing_item_type_symbol".to_string());
            } else {
                notes.push("drawing_item_type_not_symbol".to_string());
            }
            score += 20;
            notes.push("field_x_window_found".to_string());
            if position.is_some() {
                score += 20;
                notes.push("position_candidate_found".to_string());
            } else {
                notes.push("no_position_candidate".to_string());
            }
            let symbol_path = if let Some(symbol_path) = &object.symbol_path {
                score += 30;
                notes.push("object_symbol_path_bound".to_string());
                Some(symbol_path.clone())
            } else if symbol_path_catalog_count == 1 {
                score += 5;
                notes.push("single_symbol_path_catalog_candidate".to_string());
                symbol_path_catalog
                    .iter()
                    .next()
                    .map(|path| (*path).clone())
            } else if symbol_path_catalog_count > 1 {
                notes.push(format!(
                    "symbol_path_catalog_unlinked_count={symbol_path_catalog_count}"
                ));
                None
            } else {
                notes.push("no_symbol_path_catalog".to_string());
                None
            };
            notes.push("probe_only_no_symbol_geometry_promotion".to_string());

            Some(SheetSymbolPlacementCandidate {
                score,
                field_x: object.field_x,
                drawing_id: object.drawing_id.clone(),
                item_type: object.item_type.clone(),
                drawing_item_type: object.drawing_item_type.clone(),
                field_x_offset: Some(field_record.offset),
                position_offset: position.map(|position| position.offset),
                x: position.map(|position| position.x),
                y: position.map(|position| position.y),
                position_encoding: position.map(|position| position.encoding),
                symbol_path,
                symbol_path_candidate_count: symbol_path_catalog_count,
                field_x_hex: hex_range(
                    data,
                    field_record.range_start,
                    field_record
                        .range_end
                        .saturating_sub(field_record.range_start),
                ),
                notes,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|candidate| {
        (
            std::cmp::Reverse(candidate.score),
            candidate.field_x,
            candidate.field_x_offset,
        )
    });

    SheetSymbolPlacementInvestigationReport {
        candidates,
        symbol_path_catalog_count,
    }
}

/// Group marker-range records for polyline/circle/arc reverse engineering.
///
/// This is an investigation report only. It classifies numeric marker payloads
/// for manual review and must not be used to emit decoded curve geometry.
pub fn curve_primitive_investigation_report(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
) -> SheetCurvePrimitiveInvestigationReport {
    let mut groups: BTreeMap<
        (
            SheetCurvePrimitiveCandidateKind,
            Option<u16>,
            usize,
            usize,
            usize,
        ),
        SheetCurvePrimitiveShapeGroup,
    > = BTreeMap::new();

    for record in inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::Marker)
    {
        let range_len = record.range_end.saturating_sub(record.range_start);
        let Some(bytes) = data.get(record.range_start..record.range_end) else {
            continue;
        };
        let numeric_base_offset = record.range_start.saturating_add(4);
        let numeric_bytes = bytes.get(4..).unwrap_or_default();
        let candidate_i32_pairs = plausible_i32_pair_count(numeric_bytes);
        let candidate_f64_pairs = plausible_f64_pair_count(numeric_bytes);
        let i32_point_sequence = best_i32_point_sequence(numeric_bytes, numeric_base_offset);
        let i32_point_sequence_count = i32_point_sequence
            .as_ref()
            .map(|sequence| sequence.point_count)
            .unwrap_or_default();
        let i32_point_sequence_aligned = i32_point_sequence
            .as_ref()
            .is_some_and(|sequence| sequence.offset.saturating_sub(record.range_start) % 4 == 0);
        let numeric_pair_count = candidate_i32_pairs + candidate_f64_pairs;
        let numeric_pairs_per_1kb = numeric_density_per_1kb(numeric_pair_count, range_len);
        let compact_vertex_chain_candidate = compact_vertex_chain_candidate(
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
            numeric_pairs_per_1kb,
            i32_point_sequence_count,
            i32_point_sequence_aligned,
        );
        let candidate_kind = classify_curve_candidate(
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
            numeric_pairs_per_1kb,
            i32_point_sequence_count,
            i32_point_sequence_aligned,
        );
        let numeric_samples = primitive_line_numeric_samples(numeric_bytes, numeric_base_offset, 8);
        let numeric_sample_relative_offsets =
            sample_relative_offsets(&numeric_samples, record.range_start);
        let numeric_sample_offset_deltas = sample_offset_deltas(&numeric_sample_relative_offsets);
        let example_hex_prefix = hex_prefix(bytes, 96);
        let i32_point_sequence = i32_point_sequence.map(|sequence| {
            let relative_offset = sequence.offset.saturating_sub(record.range_start);
            SheetI32PointSequenceCandidate {
                relative_offset,
                relative_alignment_mod4: relative_offset % 4,
                offset: sequence.offset,
                point_count: sequence.point_count,
                byte_stride: sequence.byte_stride,
                sample_points: sequence.sample_points,
            }
        });
        let investigation_notes = curve_candidate_notes(
            candidate_kind,
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
            numeric_pairs_per_1kb,
            compact_vertex_chain_candidate,
            i32_point_sequence.as_ref(),
        );
        let key = (
            candidate_kind,
            record.marker_type,
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
        );
        groups
            .entry(key)
            .and_modify(|group| group.support += 1)
            .or_insert_with(|| SheetCurvePrimitiveShapeGroup {
                marker_type: record.marker_type,
                range_len,
                support: 1,
                candidate_i32_pairs,
                candidate_f64_pairs,
                numeric_pair_count,
                numeric_pairs_per_1kb,
                compact_vertex_chain_candidate,
                candidate_kind,
                example_offset: record.offset,
                example_range_start: record.range_start,
                example_range_end: record.range_end,
                numeric_samples,
                numeric_sample_relative_offsets,
                numeric_sample_offset_deltas,
                example_hex_prefix,
                i32_point_sequence,
                investigation_notes,
            });
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        curve_candidate_rank(right)
            .cmp(&curve_candidate_rank(left))
            .then_with(|| right.support.cmp(&left.support))
            .then_with(|| left.marker_type.cmp(&right.marker_type))
            .then_with(|| left.range_len.cmp(&right.range_len))
    });
    SheetCurvePrimitiveInvestigationReport { groups }
}

/// Group marker-range records for primitive-line reverse engineering.
///
/// This is intentionally an investigation report.  It does not decode lines
/// and must not be used to emit [`crate::geometry::PidGraphicKind::Line`].
pub fn primitive_line_investigation_report(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
) -> SheetPrimitiveLineInvestigationReport {
    let mut groups: BTreeMap<(Option<u16>, usize, usize, usize), SheetPrimitiveLineShapeGroup> =
        BTreeMap::new();
    let coordinate_hint_offsets = evidence_offsets(inventory, SheetRecordShapeKind::CoordinateHint);
    let field_x_offsets = evidence_offsets(inventory, SheetRecordShapeKind::FieldXWindow);

    for record in inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::Marker)
    {
        let range_len = record.range_end.saturating_sub(record.range_start);
        let Some(bytes) = data.get(record.range_start..record.range_end) else {
            continue;
        };
        let numeric_base_offset = record.range_start.saturating_add(4);
        let numeric_bytes = bytes.get(4..).unwrap_or_default();
        let candidate_i32_pairs = plausible_i32_pair_count(numeric_bytes);
        let candidate_f64_pairs = plausible_f64_pair_count(numeric_bytes);
        let numeric_samples = primitive_line_numeric_samples(numeric_bytes, numeric_base_offset, 8);
        let numeric_sample_relative_offsets =
            sample_relative_offsets(&numeric_samples, record.range_start);
        let numeric_sample_offset_deltas = sample_offset_deltas(&numeric_sample_relative_offsets);
        let example_hex_prefix = hex_prefix(bytes, 96);
        let coordinate_hint_match_offsets =
            matching_offsets(&numeric_samples, &coordinate_hint_offsets);
        let nearest_coordinate_hint_delta =
            nearest_sample_delta(&numeric_samples, &coordinate_hint_offsets);
        let nearest_field_x_delta = nearest_delta(record.offset, &field_x_offsets);
        let key = (
            record.marker_type,
            range_len,
            candidate_i32_pairs,
            candidate_f64_pairs,
        );
        groups
            .entry(key)
            .and_modify(|group| group.support += 1)
            .or_insert_with(|| SheetPrimitiveLineShapeGroup {
                marker_type: record.marker_type,
                range_len,
                support: 1,
                candidate_i32_pairs,
                candidate_f64_pairs,
                investigation_score: 0,
                example_offset: record.offset,
                example_range_start: record.range_start,
                example_range_end: record.range_end,
                numeric_samples,
                numeric_sample_relative_offsets,
                numeric_sample_offset_deltas,
                example_hex_prefix,
                coordinate_hint_match_offsets,
                nearest_coordinate_hint_delta,
                nearest_field_x_delta,
                investigation_notes: Vec::new(),
            });
    }

    let mut groups: Vec<_> = groups.into_values().collect();
    for group in &mut groups {
        let (score, notes) = primitive_line_candidate_score(group);
        group.investigation_score = score;
        group.investigation_notes = notes;
    }
    groups.sort_by(|left, right| {
        right
            .investigation_score
            .cmp(&left.investigation_score)
            .then_with(|| right.support.cmp(&left.support))
            .then_with(|| left.marker_type.cmp(&right.marker_type))
            .then_with(|| left.range_len.cmp(&right.range_len))
    });
    SheetPrimitiveLineInvestigationReport { groups }
}

fn classify_coordinate_page_metadata_candidate(
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    normalized_f64_pairs: usize,
    page_dimension_scalar_matches: usize,
) -> SheetCoordinatePageMetadataCandidateKind {
    if page_dimension_scalar_matches >= 2 {
        SheetCoordinatePageMetadataCandidateKind::PageDimensionScalarLike
    } else if normalized_f64_pairs > 0 {
        SheetCoordinatePageMetadataCandidateKind::NormalizedF64CoordinateLike
    } else if candidate_i32_pairs >= 2 {
        SheetCoordinatePageMetadataCandidateKind::I32CoordinateDomainLike
    } else if candidate_i32_pairs + candidate_f64_pairs > 0 {
        SheetCoordinatePageMetadataCandidateKind::MixedNumeric
    } else {
        SheetCoordinatePageMetadataCandidateKind::InsufficientEvidence
    }
}

fn coordinate_page_metadata_notes(
    candidate_kind: SheetCoordinatePageMetadataCandidateKind,
    range_len: usize,
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    normalized_f64_pairs: usize,
    page_dimension_scalar_matches: usize,
) -> Vec<String> {
    let mut notes = vec![format!("candidate_kind={candidate_kind:?}")];
    notes.push(format!("range_len={range_len}"));
    if candidate_i32_pairs > 0 {
        notes.push(format!("i32_pairs={candidate_i32_pairs}"));
    }
    if candidate_f64_pairs > 0 {
        notes.push(format!("f64_pairs={candidate_f64_pairs}"));
    }
    if normalized_f64_pairs > 0 {
        notes.push(format!("normalized_f64_pairs={normalized_f64_pairs}"));
    }
    if page_dimension_scalar_matches > 0 {
        notes.push(format!(
            "page_dimension_scalar_matches={page_dimension_scalar_matches}"
        ));
    }
    if candidate_kind == SheetCoordinatePageMetadataCandidateKind::InsufficientEvidence {
        notes.push("insufficient_numeric_page_metadata_evidence".to_string());
    }
    notes.push("probe_only_no_coordinate_page_metadata_promotion".to_string());
    notes
}

fn coordinate_page_metadata_rank(
    candidate: &SheetCoordinatePageMetadataCandidate,
) -> (u8, usize, usize, usize) {
    let kind_rank = match candidate.candidate_kind {
        SheetCoordinatePageMetadataCandidateKind::PageDimensionScalarLike => 5,
        SheetCoordinatePageMetadataCandidateKind::NormalizedF64CoordinateLike => 4,
        SheetCoordinatePageMetadataCandidateKind::I32CoordinateDomainLike => 3,
        SheetCoordinatePageMetadataCandidateKind::MixedNumeric => 2,
        SheetCoordinatePageMetadataCandidateKind::InsufficientEvidence => 1,
    };
    (
        kind_rank,
        candidate.support,
        candidate.page_dimension_scalar_matches + candidate.normalized_f64_pairs,
        candidate.candidate_i32_pairs + candidate.candidate_f64_pairs,
    )
}

fn i32_coordinate_bounds(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
) -> Option<SheetI32CoordinateBounds> {
    let mut bounds: Option<SheetI32CoordinateBounds> = None;
    for offset in inventory
        .records
        .iter()
        .filter(|record| record.kind == SheetRecordShapeKind::CoordinateHint)
        .map(|record| record.offset)
        .collect::<BTreeSet<_>>()
    {
        let Some((x, y)) = i32_pair_at(data, offset) else {
            continue;
        };
        bounds = Some(match bounds {
            Some(bounds) => SheetI32CoordinateBounds {
                min_x: bounds.min_x.min(x),
                max_x: bounds.max_x.max(x),
                min_y: bounds.min_y.min(y),
                max_y: bounds.max_y.max(y),
                count: bounds.count + 1,
            },
            None => SheetI32CoordinateBounds {
                min_x: x,
                max_x: x,
                min_y: y,
                max_y: y,
                count: 1,
            },
        });
    }
    bounds
}

fn f64_coordinate_bounds(
    data: &[u8],
    inventory: &SheetRecordShapeInventory,
) -> Option<SheetF64CoordinateBounds> {
    let mut bounds: Option<SheetF64CoordinateBounds> = None;
    for offset in inventory
        .records
        .iter()
        .filter_map(|record| record.f64_coordinate_offset)
        .collect::<BTreeSet<_>>()
    {
        let Some((x, y)) = f64_pair_at(data, offset) else {
            continue;
        };
        if !normalized_f64_pair_values(x, y) {
            continue;
        }
        bounds = Some(match bounds {
            Some(bounds) => SheetF64CoordinateBounds {
                min_x: bounds.min_x.min(x),
                max_x: bounds.max_x.max(x),
                min_y: bounds.min_y.min(y),
                max_y: bounds.max_y.max(y),
                count: bounds.count + 1,
            },
            None => SheetF64CoordinateBounds {
                min_x: x,
                max_x: x,
                min_y: y,
                max_y: y,
                count: 1,
            },
        });
    }
    bounds
}

fn primitive_line_candidate_score(group: &SheetPrimitiveLineShapeGroup) -> (i32, Vec<String>) {
    let mut score = 0;
    let mut notes = Vec::new();
    let numeric_pairs = group.candidate_i32_pairs + group.candidate_f64_pairs;

    if group.support > 1 {
        score += 40 + group.support.min(4) as i32 * 5;
        notes.push(format!("repeated_shape_support={}", group.support));
    } else {
        score += 5;
        notes.push("single_example_shape".to_string());
    }

    match group.range_len {
        16..=512 => {
            score += 20;
            notes.push("bounded_compact_range".to_string());
        }
        513..=2048 => {
            score += 5;
            notes.push("bounded_medium_range".to_string());
        }
        0..=15 => {
            score -= 20;
            notes.push("too_short_for_line_payload".to_string());
        }
        _ => {
            score -= 20;
            notes.push("large_range_needs_subrecord_split".to_string());
        }
    }

    match numeric_pairs {
        0 => {
            score -= 50;
            notes.push("no_numeric_coordinate_candidates".to_string());
        }
        1..=4 => {
            score += 25;
            notes.push(format!("selective_numeric_pairs={numeric_pairs}"));
        }
        5..=16 => {
            score += 10;
            notes.push(format!("moderate_numeric_pairs={numeric_pairs}"));
        }
        _ => {
            score -= 10;
            notes.push(format!("many_numeric_pairs={numeric_pairs}"));
        }
    }

    if group.candidate_i32_pairs >= 2 {
        score += 10;
        notes.push("has_i32_start_end_candidates".to_string());
    }
    if group.candidate_f64_pairs > 0 {
        score += 5;
        notes.push("has_f64_pair_candidates".to_string());
    }
    if group.coordinate_hint_match_offsets.is_empty() {
        notes.push("no_coordinate_hint_sample_match".to_string());
    } else {
        score += 15;
        notes.push(format!(
            "coordinate_hint_matches={}",
            group.coordinate_hint_match_offsets.len()
        ));
    }
    if let Some(delta) = group.nearest_coordinate_hint_delta {
        notes.push(format!("nearest_coordinate_hint_delta={delta}"));
    }
    if let Some(delta) = group.nearest_field_x_delta {
        if delta.unsigned_abs() <= 128 {
            score += 5;
        }
        notes.push(format!("nearest_field_x_delta={delta}"));
    }

    (score, notes)
}

fn classify_curve_candidate(
    range_len: usize,
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    numeric_pairs_per_1kb: usize,
    i32_point_sequence_count: usize,
    i32_point_sequence_aligned: bool,
) -> SheetCurvePrimitiveCandidateKind {
    let total_pairs = candidate_i32_pairs + candidate_f64_pairs;
    if compact_vertex_chain_candidate(
        range_len,
        candidate_i32_pairs,
        candidate_f64_pairs,
        numeric_pairs_per_1kb,
        i32_point_sequence_count,
        i32_point_sequence_aligned,
    ) {
        SheetCurvePrimitiveCandidateKind::PolylineLike
    } else if (16..=512).contains(&range_len) && candidate_f64_pairs >= 2 {
        SheetCurvePrimitiveCandidateKind::CircleArcLike
    } else if total_pairs >= 2 {
        SheetCurvePrimitiveCandidateKind::MixedNumeric
    } else {
        SheetCurvePrimitiveCandidateKind::InsufficientNumeric
    }
}

fn compact_vertex_chain_candidate(
    range_len: usize,
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    _numeric_pairs_per_1kb: usize,
    i32_point_sequence_count: usize,
    i32_point_sequence_aligned: bool,
) -> bool {
    let total_pairs = candidate_i32_pairs + candidate_f64_pairs;
    (16..=512).contains(&range_len)
        && (4..=16).contains(&total_pairs)
        && candidate_i32_pairs >= 4
        && candidate_f64_pairs <= candidate_i32_pairs
        && i32_point_sequence_count >= 3
        && i32_point_sequence_aligned
}

fn numeric_density_per_1kb(numeric_pair_count: usize, range_len: usize) -> usize {
    if range_len == 0 {
        return 0;
    }
    numeric_pair_count.saturating_mul(1024) / range_len
}

fn curve_candidate_notes(
    candidate_kind: SheetCurvePrimitiveCandidateKind,
    range_len: usize,
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    numeric_pairs_per_1kb: usize,
    compact_vertex_chain_candidate: bool,
    i32_point_sequence: Option<&SheetI32PointSequenceCandidate>,
) -> Vec<String> {
    let mut notes = vec![format!("candidate_kind={candidate_kind:?}")];
    notes.push(format!("range_len={range_len}"));
    notes.push(format!("numeric_pairs_per_1kb={numeric_pairs_per_1kb}"));
    if candidate_i32_pairs > 0 {
        notes.push(format!("i32_pairs={candidate_i32_pairs}"));
    }
    if candidate_f64_pairs > 0 {
        notes.push(format!("f64_pairs={candidate_f64_pairs}"));
    }
    if compact_vertex_chain_candidate {
        notes.push("compact_vertex_chain_candidate".to_string());
    } else if candidate_i32_pairs + candidate_f64_pairs >= 4 {
        notes.push("mixed_or_large_numeric_payload_needs_subrecord_split".to_string());
    }
    if let Some(sequence) = i32_point_sequence {
        notes.push(format!(
            "i32_point_sequence_points={}",
            sequence.point_count
        ));
        notes.push(format!(
            "i32_point_sequence_relative_offset={}",
            sequence.relative_offset
        ));
        if sequence.relative_alignment_mod4 != 0 {
            notes.push(format!(
                "i32_point_sequence_alignment_mod4={}",
                sequence.relative_alignment_mod4
            ));
            notes.push("unaligned_i32_point_sequence_needs_subfield_split".to_string());
        }
        if sequence.point_count < 3 {
            notes.push("short_i32_point_sequence_needs_more_vertices".to_string());
        }
    } else if candidate_i32_pairs > 0 {
        notes.push("no_non_overlapping_i32_point_sequence".to_string());
    }
    notes.push("probe_only_no_curve_geometry_promotion".to_string());
    notes
}

fn curve_candidate_rank(group: &SheetCurvePrimitiveShapeGroup) -> (u8, usize, usize) {
    let kind_rank = match group.candidate_kind {
        SheetCurvePrimitiveCandidateKind::PolylineLike => 4,
        SheetCurvePrimitiveCandidateKind::CircleArcLike => 3,
        SheetCurvePrimitiveCandidateKind::MixedNumeric => 2,
        SheetCurvePrimitiveCandidateKind::InsufficientNumeric => 1,
    };
    (kind_rank, group.support, group.numeric_pair_count)
}

fn evidence_offsets(
    inventory: &SheetRecordShapeInventory,
    kind: SheetRecordShapeKind,
) -> Vec<usize> {
    inventory
        .records
        .iter()
        .filter(|record| record.kind == kind)
        .map(|record| record.offset)
        .collect()
}

fn matching_offsets(
    samples: &[SheetPrimitiveLineNumericSample],
    evidence_offsets: &[usize],
) -> Vec<usize> {
    let evidence_offsets = evidence_offsets.iter().copied().collect::<BTreeSet<_>>();
    samples
        .iter()
        .filter(|sample| evidence_offsets.contains(&sample.offset))
        .map(|sample| sample.offset)
        .collect()
}

fn nearest_sample_delta(
    samples: &[SheetPrimitiveLineNumericSample],
    evidence_offsets: &[usize],
) -> Option<isize> {
    samples
        .iter()
        .flat_map(|sample| {
            evidence_offsets
                .iter()
                .map(move |offset| signed_delta(sample.offset, *offset))
        })
        .min_by_key(|delta| delta.unsigned_abs())
}

fn nearest_delta(origin: usize, evidence_offsets: &[usize]) -> Option<isize> {
    evidence_offsets
        .iter()
        .map(|offset| signed_delta(origin, *offset))
        .min_by_key(|delta| delta.unsigned_abs())
}

fn nearest_field_record<'a>(
    records: &[&'a SheetRecordShapeEvidence],
    text_offset: usize,
    coordinate_offset: usize,
) -> Option<&'a SheetRecordShapeEvidence> {
    records.iter().copied().min_by_key(|record| {
        record
            .offset
            .abs_diff(text_offset)
            .min(record.offset.abs_diff(coordinate_offset))
    })
}

fn text_score_quality_passed(reasons: &[SheetTextWindowScoreReason]) -> bool {
    reasons
        .iter()
        .any(|reason| matches!(reason, SheetTextWindowScoreReason::TextQualityPassed))
}

fn field_record_for_field_x(
    inventory: &SheetRecordShapeInventory,
    field_x: u32,
) -> Option<&SheetRecordShapeEvidence> {
    inventory
        .records
        .iter()
        .filter(|record| {
            record.kind == SheetRecordShapeKind::FieldXWindow && record.field_x == Some(field_x)
        })
        .min_by_key(|record| {
            (
                record.endpoint_record_start.is_some(),
                record.candidate_coordinate_offset.is_none()
                    && record.f64_coordinate_offset.is_none(),
                record.offset,
            )
        })
}

#[derive(Debug, Clone, Copy)]
struct SymbolPositionCandidate {
    offset: usize,
    x: f64,
    y: f64,
    encoding: SheetSymbolPlacementPositionEncoding,
}

#[derive(Debug, Clone)]
struct I32PointSequenceCandidate {
    offset: usize,
    point_count: usize,
    byte_stride: usize,
    sample_points: Vec<String>,
}

fn symbol_position_candidate(
    data: &[u8],
    record: &SheetRecordShapeEvidence,
) -> Option<SymbolPositionCandidate> {
    record
        .candidate_coordinate_offset
        .and_then(|offset| {
            i32_pair_at(data, offset).map(|(x, y)| SymbolPositionCandidate {
                offset,
                x: f64::from(x),
                y: f64::from(y),
                encoding: SheetSymbolPlacementPositionEncoding::I32Pair,
            })
        })
        .or_else(|| {
            record.f64_coordinate_offset.and_then(|offset| {
                f64_pair_at(data, offset).and_then(|(x, y)| {
                    (plausible_f64_coordinate(x) && plausible_f64_coordinate(y)).then_some(
                        SymbolPositionCandidate {
                            offset,
                            x,
                            y,
                            encoding: SheetSymbolPlacementPositionEncoding::F64Pair,
                        },
                    )
                })
            })
        })
}

fn best_i32_point_sequence(bytes: &[u8], base_offset: usize) -> Option<I32PointSequenceCandidate> {
    let mut best: Option<I32PointSequenceCandidate> = None;
    for alignment in 0usize..8 {
        let mut offset = alignment;
        while offset.saturating_add(8) <= bytes.len() {
            let start = offset;
            let mut sample_points = Vec::new();
            let mut point_count = 0usize;
            while offset.saturating_add(8) <= bytes.len() {
                let Some((x, y)) = i32_pair_at(bytes, offset) else {
                    break;
                };
                if !plausible_i32_coordinate(x) || !plausible_i32_coordinate(y) {
                    break;
                }
                point_count += 1;
                if sample_points.len() < 8 {
                    sample_points.push(format!("({x}, {y})"));
                }
                offset = offset.saturating_add(8);
            }
            if point_count >= 2
                && best
                    .as_ref()
                    .is_none_or(|candidate| point_count > candidate.point_count)
            {
                best = Some(I32PointSequenceCandidate {
                    offset: base_offset.saturating_add(start),
                    point_count,
                    byte_stride: 8,
                    sample_points,
                });
            }
            offset = start.saturating_add(4).max(offset.saturating_add(4));
        }
    }
    best
}

fn i32_pair_at(data: &[u8], offset: usize) -> Option<(i32, i32)> {
    let bytes = data.get(offset..offset.checked_add(8)?)?;
    Some((
        i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    ))
}

fn f64_pair_at(data: &[u8], offset: usize) -> Option<(f64, f64)> {
    let bytes = data.get(offset..offset.checked_add(16)?)?;
    Some((
        f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        f64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
    ))
}

fn signed_delta(origin: usize, target: usize) -> isize {
    if target >= origin {
        target.saturating_sub(origin).min(isize::MAX as usize) as isize
    } else {
        -(origin.saturating_sub(target).min(isize::MAX as usize) as isize)
    }
}

fn sample_relative_offsets(
    samples: &[SheetPrimitiveLineNumericSample],
    base_offset: usize,
) -> Vec<usize> {
    samples
        .iter()
        .map(|sample| sample.offset.saturating_sub(base_offset))
        .collect()
}

fn sample_offset_deltas(relative_offsets: &[usize]) -> Vec<usize> {
    relative_offsets
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .collect()
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    let mut parts = bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>();
    if bytes.len() > limit {
        parts.push("...".to_string());
    }
    parts.join(" ")
}

fn hex_range(data: &[u8], offset: usize, len: usize) -> String {
    data.get(offset..offset.saturating_add(len).min(data.len()))
        .map(|bytes| hex_prefix(bytes, len))
        .unwrap_or_default()
}

fn marker_shape_evidence(data: &[u8], probe: &SheetProbeReport) -> Vec<SheetRecordShapeEvidence> {
    let marker_offsets = marker_offsets(data);
    marker_offsets
        .iter()
        .enumerate()
        .filter_map(|(index, &offset)| {
            let marker_type = marker_type_at(data, offset)?;
            let chunk = chunk_containing_offset(probe, offset);
            let next_marker = marker_offsets.get(index + 1).copied();
            let range_end = next_marker
                .or_else(|| chunk.map(|chunk| chunk.1))
                .unwrap_or_else(|| offset.saturating_add(3).min(data.len()))
                .min(data.len());
            Some(SheetRecordShapeEvidence {
                kind: SheetRecordShapeKind::Marker,
                offset,
                range_start: offset,
                range_end: range_end.max(offset.saturating_add(3).min(data.len())),
                marker_type: Some(marker_type),
                field_x: None,
                endpoint_record_start: None,
                chunk_start: chunk.map(|chunk| chunk.0),
                chunk_end: chunk.map(|chunk| chunk.1),
                candidate_coordinate_offset: None,
                candidate_text_offset: None,
                f64_coordinate_offset: None,
                f64_marker_offset: None,
                notes: vec![format!("marker_type=0x{marker_type:04X}")],
            })
        })
        .collect()
}

fn field_x_shape_evidence(
    data: &[u8],
    probe: &SheetProbeReport,
    field_xs: &[u32],
) -> Vec<SheetRecordShapeEvidence> {
    let windows = field_x_windows(data, field_xs, 64);
    let features = field_x_window_features(data, &windows, &probe.chunks);
    windows
        .iter()
        .zip(features.iter())
        .map(|(window, feature)| {
            let mut notes = Vec::new();
            if let Some(delta) = feature.field_delta_from_chunk {
                notes.push(format!("field_delta_from_chunk={delta}"));
            }
            if let Some(delta) = feature.coordinate_delta_from_chunk {
                notes.push(format!("coordinate_delta_from_chunk={delta}"));
            }
            if let Some(f64_shape) = feature.repeated_f64_pair_shape {
                notes.push(format!(
                    "f64_coordinate_delta_from_field={}",
                    f64_shape.coordinate_delta_from_field
                ));
            }
            SheetRecordShapeEvidence {
                kind: SheetRecordShapeKind::FieldXWindow,
                offset: window.offset,
                range_start: window.window_start,
                range_end: window.window_end,
                marker_type: None,
                field_x: Some(window.field_x),
                endpoint_record_start: window.endpoint_record_start,
                chunk_start: feature.chunk_start,
                chunk_end: feature.chunk_end,
                candidate_coordinate_offset: feature
                    .candidate_position
                    .as_ref()
                    .map(|position| position.offset),
                candidate_text_offset: nearest_text_offset(probe, window.offset, 128),
                f64_coordinate_offset: feature
                    .repeated_f64_pair_shape
                    .map(|shape| shape.coordinate_offset),
                f64_marker_offset: feature
                    .repeated_f64_pair_shape
                    .map(|shape| shape.marker_offset),
                notes,
            }
        })
        .collect()
}

fn text_shape_evidence(probe: &SheetProbeReport) -> Vec<SheetRecordShapeEvidence> {
    probe
        .text_runs
        .iter()
        .map(|text| {
            let range_end = text.offset.saturating_add(text.byte_len);
            let chunk = chunk_containing_range(probe, text.offset, range_end);
            SheetRecordShapeEvidence {
                kind: SheetRecordShapeKind::TextRun,
                offset: text.offset,
                range_start: text.offset,
                range_end,
                marker_type: None,
                field_x: None,
                endpoint_record_start: None,
                chunk_start: chunk.map(|chunk| chunk.0),
                chunk_end: chunk.map(|chunk| chunk.1),
                candidate_coordinate_offset: nearest_coordinate_offset(
                    &probe.coordinate_hints,
                    text.offset,
                    128,
                ),
                candidate_text_offset: Some(text.offset),
                f64_coordinate_offset: None,
                f64_marker_offset: None,
                notes: vec![format!("text_bytes={}", text.byte_len)],
            }
        })
        .collect()
}

fn coordinate_shape_evidence(probe: &SheetProbeReport) -> Vec<SheetRecordShapeEvidence> {
    probe
        .coordinate_hints
        .iter()
        .map(|coordinate| {
            let range_end = coordinate.offset.saturating_add(8);
            let chunk = chunk_containing_range(probe, coordinate.offset, range_end);
            SheetRecordShapeEvidence {
                kind: SheetRecordShapeKind::CoordinateHint,
                offset: coordinate.offset,
                range_start: coordinate.offset,
                range_end,
                marker_type: None,
                field_x: None,
                endpoint_record_start: None,
                chunk_start: chunk.map(|chunk| chunk.0),
                chunk_end: chunk.map(|chunk| chunk.1),
                candidate_coordinate_offset: Some(coordinate.offset),
                candidate_text_offset: nearest_text_offset(probe, coordinate.offset, 128),
                f64_coordinate_offset: None,
                f64_marker_offset: None,
                notes: vec![format!("coordinate=({}, {})", coordinate.x, coordinate.y)],
            }
        })
        .collect()
}

fn marker_offsets(data: &[u8]) -> Vec<usize> {
    (0..data.len().saturating_sub(2))
        .filter(|&offset| data[offset] == 0x89)
        .collect()
}

fn marker_type_at(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *data.get(offset + 1)?,
        *data.get(offset + 2)?,
    ]))
}

fn chunk_containing_offset(probe: &SheetProbeReport, offset: usize) -> Option<(usize, usize)> {
    probe
        .chunks
        .iter()
        .find(|chunk| chunk.start <= offset && offset < chunk.end)
        .map(|chunk| (chunk.start, chunk.end))
}

fn chunk_containing_range(
    probe: &SheetProbeReport,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    probe
        .chunks
        .iter()
        .find(|chunk| chunk.start <= start && end <= chunk.end)
        .map(|chunk| (chunk.start, chunk.end))
}

fn nearest_coordinate_offset(
    coordinates: &[SheetCoordinateHint],
    offset: usize,
    radius: usize,
) -> Option<usize> {
    coordinates
        .iter()
        .filter(|coordinate| coordinate.offset.abs_diff(offset) <= radius)
        .min_by_key(|coordinate| coordinate.offset.abs_diff(offset))
        .map(|coordinate| coordinate.offset)
}

fn nearest_text_offset(probe: &SheetProbeReport, offset: usize, radius: usize) -> Option<usize> {
    probe
        .text_runs
        .iter()
        .filter(|text| text.offset.abs_diff(offset) <= radius)
        .min_by_key(|text| text.offset.abs_diff(offset))
        .map(|text| text.offset)
}

fn plausible_i32_pair_count(bytes: &[u8]) -> usize {
    bytes
        .windows(8)
        .step_by(4)
        .filter(|window| {
            let x = i32::from_le_bytes([window[0], window[1], window[2], window[3]]);
            let y = i32::from_le_bytes([window[4], window[5], window[6], window[7]]);
            plausible_i32_coordinate(x) && plausible_i32_coordinate(y)
        })
        .count()
}

fn plausible_i32_coordinate(value: i32) -> bool {
    let abs = value.unsigned_abs();
    (1_000..=1_000_000).contains(&abs)
}

fn plausible_f64_pair_count(bytes: &[u8]) -> usize {
    bytes
        .windows(16)
        .enumerate()
        .filter(|(relative_offset, _)| relative_offset % 4 == 0)
        .filter(|(_, window)| {
            let x = f64::from_le_bytes([
                window[0], window[1], window[2], window[3], window[4], window[5], window[6],
                window[7],
            ]);
            let y = f64::from_le_bytes([
                window[8], window[9], window[10], window[11], window[12], window[13], window[14],
                window[15],
            ]);
            plausible_f64_coordinate(x) && plausible_f64_coordinate(y)
        })
        .count()
}

fn plausible_f64_coordinate(value: f64) -> bool {
    value.is_finite() && (1.0e-6..=1.0e9).contains(&value.abs())
}

/// Collect every normalized `(x, y)` f64 pair from a byte slice.
///
/// Scans 16-byte windows at 4-byte alignment (matching the Sheet PSM
/// coordinate layout), decoding each window as two little-endian `f64`
/// values. A pair is kept only when both components are finite, inside
/// the normalized `[-1e-9, 1 + 1e-9]` range, and at least one component
/// is non-zero — the same predicate the coordinate/page metadata
/// investigation report uses for its `normalized_f64_pair_count`.
///
/// This is **investigation-only evidence** feeding
/// [`coordinate_pair_spatial_analysis`]. It does not decode records or
/// promote any entity, and the scan is panic-safe and bounded.
pub fn collect_normalized_f64_pairs(bytes: &[u8]) -> Vec<(f64, f64)> {
    let mut pairs = Vec::new();
    for (relative_offset, window) in bytes.windows(16).enumerate() {
        if relative_offset % 4 != 0 {
            continue;
        }
        let x = f64::from_le_bytes([
            window[0], window[1], window[2], window[3], window[4], window[5], window[6], window[7],
        ]);
        let y = f64::from_le_bytes([
            window[8], window[9], window[10], window[11], window[12], window[13], window[14],
            window[15],
        ]);
        if normalized_f64_pair_values(x, y) {
            pairs.push((x, y));
        }
    }
    pairs
}

fn normalized_f64_pair_count(bytes: &[u8]) -> usize {
    collect_normalized_f64_pairs(bytes).len()
}

fn normalized_f64_coordinate(value: f64) -> bool {
    value.is_finite() && (-1.0e-9..=1.0 + 1.0e-9).contains(&value)
}

fn normalized_f64_pair_values(x: f64, y: f64) -> bool {
    normalized_f64_coordinate(x)
        && normalized_f64_coordinate(y)
        && (x.abs() > 1.0e-12 || y.abs() > 1.0e-12)
}

fn page_dimension_scalar_match_count(
    bytes: &[u8],
    inferred_page_dimensions_mm: Option<(f64, f64)>,
) -> usize {
    let Some((width, height)) = inferred_page_dimensions_mm else {
        return 0;
    };
    bytes
        .windows(8)
        .enumerate()
        .filter(|(relative_offset, _)| relative_offset % 4 == 0)
        .filter(|(_, window)| {
            let value = f64::from_le_bytes([
                window[0], window[1], window[2], window[3], window[4], window[5], window[6],
                window[7],
            ]);
            scalar_matches_page_dimension(value, width, height)
        })
        .count()
        + bytes
            .windows(4)
            .step_by(4)
            .filter(|window| {
                let value = i32::from_le_bytes([window[0], window[1], window[2], window[3]]);
                scalar_matches_page_dimension(f64::from(value), width, height)
            })
            .count()
}

fn scalar_matches_page_dimension(value: f64, width: f64, height: f64) -> bool {
    value.is_finite()
        && ((value - width).abs() <= 1.0e-6
            || (value - height).abs() <= 1.0e-6
            || (value - width.round()).abs() <= 1.0e-6
            || (value - height.round()).abs() <= 1.0e-6)
}

fn primitive_line_numeric_samples(
    bytes: &[u8],
    base_offset: usize,
    limit: usize,
) -> Vec<SheetPrimitiveLineNumericSample> {
    let mut samples = Vec::new();
    for (relative_offset, window) in bytes.windows(8).step_by(4).enumerate() {
        if samples.len() >= limit {
            return samples;
        }
        let x = i32::from_le_bytes([window[0], window[1], window[2], window[3]]);
        let y = i32::from_le_bytes([window[4], window[5], window[6], window[7]]);
        if plausible_i32_coordinate(x) && plausible_i32_coordinate(y) {
            samples.push(SheetPrimitiveLineNumericSample {
                offset: base_offset + relative_offset * 4,
                kind: SheetPrimitiveLineNumericSampleKind::I32Pair,
                value: format!("({x}, {y})"),
            });
        }
    }
    for (relative_offset, window) in bytes.windows(16).enumerate() {
        if relative_offset % 4 != 0 {
            continue;
        }
        if samples.len() >= limit {
            return samples;
        }
        let x = f64::from_le_bytes([
            window[0], window[1], window[2], window[3], window[4], window[5], window[6], window[7],
        ]);
        let y = f64::from_le_bytes([
            window[8], window[9], window[10], window[11], window[12], window[13], window[14],
            window[15],
        ]);
        if plausible_f64_coordinate(x) && plausible_f64_coordinate(y) {
            samples.push(SheetPrimitiveLineNumericSample {
                offset: base_offset + relative_offset,
                kind: SheetPrimitiveLineNumericSampleKind::F64Pair,
                value: format!("({x:.6}, {y:.6})"),
            });
        }
    }
    samples
}

// ---------------------------------------------------------------------------
// M1 deepening seam: shared PSM record envelope + per-family decoder trait
//
// RFC: docs/plans/2026-07-16-psm-decoder-deepening-refactor-rfc-cn.md §3.1.
// Every PSM record family on a `Sheet*` stream shares the same 6-byte
// envelope (`u16` type word + `u32` bytes_to_follow) and the same
// walk+advance scan loop. Before this seam landed, both were copy-pasted
// per family (11×). Families implement `PsmRecordDecoder` and keep only
// their genuinely family-specific payload validation; the public
// `decode_*` free functions remain as thin wrappers for API stability.
// ---------------------------------------------------------------------------

/// Byte length of the PSM record **envelope** shared by every record
/// family: `u16` LE type word (14-bit type code + 2 flag bits) followed
/// by a `u32` LE `bytes_to_follow`.
///
/// Not to be confused with [`PSM_RECORD_HEADER_LEN`] (18), which is the
/// `GLine2d`-specific header that additionally carries `oid` + an 8-byte
/// aux prefix after the shared envelope.
pub const PSM_ENVELOPE_LEN: usize = 6;

/// The decoded 6-byte PSM record envelope shared by every PSM record
/// family (see [`parse_psm_header`]).
///
/// Deliberately minimal: `oid` is **not** part of the shared envelope —
/// `igLine2d` and friends carry it in their payload sub-header, while
/// `GLine2d` (PSM `0x3FE6`) carries it inside its 18-byte
/// [`PSM_RECORD_HEADER_LEN`] header. Each family reads its own fields
/// after [`Self::body_start`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsmHeader {
    /// 14-bit PSM type code (`type_word & 0x3FFF`).
    pub type_code: u16,
    /// Top 2 bits of the type word (`type_word >> 14`), the record-level
    /// flag bits documented in `PSMSerializeIn`'s skip-record branch.
    pub type_flags: u16,
    /// Payload byte count following the 6-byte envelope, as stored.
    pub bytes_to_follow: u32,
    /// Absolute offset of the first payload byte (envelope start + 6).
    pub body_start: usize,
}

/// Parse the shared 6-byte PSM record envelope at `offset`.
///
/// Returns `None` when fewer than [`PSM_ENVELOPE_LEN`] bytes remain at
/// `offset` (or `offset` overflows). Performs **no** family validation:
/// callers must check [`PsmHeader::type_code`] / bounds themselves.
/// Panic-free on arbitrary input.
pub fn parse_psm_header(data: &[u8], offset: usize) -> Option<PsmHeader> {
    let body_start = offset.checked_add(PSM_ENVELOPE_LEN)?;
    let header = data.get(offset..body_start)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    Some(PsmHeader {
        type_code: type_word & 0x3FFF,
        type_flags: type_word >> 14,
        bytes_to_follow: u32::from_le_bytes([header[2], header[3], header[4], header[5]]),
        body_start,
    })
}

/// Offset of the first record in a `Sheet*` stream: the 8-byte stream
/// header (`magic` + `record_count`) precedes the chain.
pub const SHEET_STREAM_HEADER_LEN: usize = 8;

/// Every offset a record actually starts at, by walking the stream as the
/// chain it is: `u16` type word, `u32 bytes_to_follow`, payload, repeat.
///
/// This is what tells a record from a byte pattern that merely looks like
/// one. [`PsmRecordDecoder::scan`] tries every offset and slides a byte on
/// rejection, so a header shape occurring *inside* another record's payload
/// is accepted as a record — which is how `0x3FE6 GLine2d` came to have
/// three corpus "records" that are really the top two bytes of an
/// `igSmartFrame2d` page-ratio `f64` (see
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`).
///
/// Returns an empty vector unless the walk consumes the stream exactly. A
/// partial walk means the stream is not the chain this assumes, and half a
/// chain is worse evidence than none: a caller gating on membership would
/// silently drop every record past the stall.
#[must_use]
pub fn sheet_record_starts(data: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut at = SHEET_STREAM_HEADER_LEN;
    while at < data.len() {
        let Some(header) = parse_psm_header(data, at) else {
            return Vec::new();
        };
        let Some(end) = header
            .body_start
            .checked_add(header.bytes_to_follow as usize)
        else {
            return Vec::new();
        };
        if header.bytes_to_follow == 0 || end > data.len() {
            return Vec::new();
        }
        starts.push(at);
        at = end;
    }
    starts
}

/// One PSM record family's decoder (RFC §3.1, L4 seam).
///
/// Implementations are unit structs owning only the family-specific
/// payload validation inside [`Self::decode_at`]; the walk+advance scan
/// loop lives once in the provided [`Self::scan`]. The associated
/// [`Self::Record`] keeps each family's typed parser DTO — this seam
/// deliberately does **not** collapse families into an enum or an
/// untyped map (that would erase the Typed Audit evidence language from
/// `CONTEXT.md`).
///
/// Contract for implementors:
///
/// - [`Self::decode_at`] must be panic-free on arbitrary bytes and
///   return `None` on any validation failure (same bar as every
///   `decode_*_at` free function, enforced by
///   `tests/parser_panic_safety.rs`).
/// - [`Self::advance_of`] returns the accepted record's full on-disk
///   byte length; [`Self::scan`] clamps it to ≥ 1 so a buggy zero can
///   never stall the walk.
/// - Families whose scan must deviate from the shared loop (none today;
///   watch `0x0010` Mode B) may override [`Self::scan`], documenting why.
pub trait PsmRecordDecoder {
    /// Typed decoded-record DTO for this family (e.g.
    /// [`SheetIgLine2dDecoded`]).
    type Record;

    /// The 14-bit PSM type code this decoder accepts.
    fn type_code(&self) -> u16;

    /// Smallest possible full-record byte length for this family
    /// (envelope + minimum payload). [`Self::scan`] uses it to bound
    /// the walk so trailing bytes shorter than one record are skipped
    /// without per-offset decode attempts.
    fn min_record_len(&self) -> usize;

    /// Try to decode one record starting at `offset`. `None` when any
    /// family validation rule fails. Must be panic-free and
    /// bounds-checked.
    fn decode_at(&self, data: &[u8], offset: usize) -> Option<Self::Record>;

    /// Byte count [`Self::scan`] advances after accepting `record` —
    /// the record's full on-disk length (envelope + payload).
    fn advance_of(&self, record: &Self::Record) -> usize;

    /// Shared walk+advance scan loop (previously copy-pasted per
    /// family): try [`Self::decode_at`] at each offset, jump past
    /// accepted records, slide one byte on rejection.
    fn scan(&self, data: &[u8]) -> Vec<Self::Record> {
        let mut out = Vec::new();
        let min_len = self.min_record_len().max(1);
        if data.len() < min_len {
            return out;
        }
        let max_offset = data.len() - min_len;
        let mut off = 0usize;
        while off <= max_offset {
            if let Some(record) = self.decode_at(data, off) {
                let advance = self.advance_of(&record).max(1);
                out.push(record);
                off = off.saturating_add(advance);
                continue;
            }
            off += 1;
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Phase 14 Slices D/F: PSM-encoded geometry decoder family
// ---------------------------------------------------------------------------

/// Byte length of one PSM record header in
/// [`decode_primitive_lines`].
///
/// Layout reverse-engineered from `radsrvitem.dll`
/// `PSMSerializeOut` (0x56491E80) / `PSMSerializeIn` (0x564915E0)
/// in
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`:
///
/// ```text
/// 0..2    uint16_le  type_code         // 14-bit, top 2 bits flags
/// 2..6    uint32_le  bytes_to_follow   // payload length excl. these 6
/// 6..10   uint32_le  oid               // object identifier
/// 10..18  8 bytes    aux               // type-specific aux payload prefix
/// ```
pub const PSM_RECORD_HEADER_LEN: usize = 18;

/// Byte length of one in-memory `GLine2d` payload:
/// 6 × `f64` LE = 48 bytes.
///
/// Layout reverse-engineered from `radsrvitem.dll`
/// `GLine2d::Validate` (`sub_56524C50` @ 0x56524C50). Documented in
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`.
pub const GLINE2D_PAYLOAD_LEN: usize = 48;

/// PSM type code that identifies a standard Intergraph Sigma
/// `igLine2d` record on disk.
///
/// Equals the Intergraph IGDS class tag for `igLine2d` (`0x18 = 24`,
/// looked up by `radsrvitem.dll!sub_56448F70`). The fixture
/// histogram in `examples/probe_psm_type_code_histogram.rs` shows
/// 309 cross-fixture hits for this type code on `Sheet*` streams,
/// far more than the 3 `GLine2d` (PSM `0x3FE6`) records — `igLine2d`
/// is the canonical Sigma 2D line representation, while `GLine2d`
/// (PSM `0x3FE6`) is `SmartPlant`'s extended parametric wrapper.
pub const PSM_TYPE_CODE_IGLINE2D: u16 = 0x0018;

/// Byte length of one PSM `igLine2d` payload (after the 6-byte
/// `(type_code, bytes_to_follow)` header): 12 bytes finishing the
/// shared PSM envelope (`oid` + the 8-byte `aux` pair) + 38 bytes
/// the class itself writes (`sub_type_word` + `index` + four f64-LE
/// `(start.x, start.y, end.x, end.y)`) = **50 bytes total**.
///
/// The envelope split is `radsrvitem.dll`'s, not a guess:
/// `PSMSerializeOut` (`0x56491E80`) writes
/// `type(2) + bytes_to_follow(4) + oid(4) + aux(8)` before handing
/// the stream to the class's `Save`. See
/// `docs/analysis/2026-08-11-remaining-header-is-the-psm-aux-field.md`.
pub const IGLINE2D_PAYLOAD_LEN: usize = 50;

/// PSM type code that identifies a standard Intergraph Sigma
/// `igLineString2d` (polyline) record on disk.
///
/// Equals the IGDS class tag for `igLineString2d` (`0x84 = 132`).
/// The fixture histogram surfaces 131 cross-fixture hits — the
/// canonical polyline representation in `SmartPlant` `Sheet*`
/// streams.
pub const PSM_TYPE_CODE_IGLINESTRING2D: u16 = 0x0084;

/// Minimum byte length of one PSM `igLineString2d` payload: 18
/// bytes sub-header + 4 (`vertex_count`) + 2 (`form` + `scope`) +
/// `vertex_count × 16` (vertices). With `vertex_count >= 2`, the
/// minimum is `24 + 2 * 16 = 56` bytes.
pub const IGLINESTRING2D_MIN_PAYLOAD_LEN: usize = 24 + 2 * 16;

/// Maximum vertex count we'll accept on a decoded `igLineString2d`.
/// Real fixtures have ≤ 10; capping at 10,000 catches both
/// legitimate worst-case usage and obvious noise.
const IGLINESTRING2D_MAX_VERTEX_COUNT: u32 = 10_000;

/// `form` byte upper bound from `GLineString2d::Validate` (memory
/// layout `*(_BYTE *)(a2 + 8)`): `form <= 6`.
const IGLINESTRING2D_FORM_MAX: u8 = 6;

/// PSM type code that identifies a standard Intergraph Sigma
/// `igPoint2d` record on disk (IGDS class tag `0x5E = 94`).
///
/// Cross-fixture histogram surfaces 145 hits on `Sheet*` streams.
pub const PSM_TYPE_CODE_IGPOINT2D: u16 = 0x005E;

/// Byte length of one PSM `igPoint2d` payload: 18-byte sub-header
/// + 16 bytes (`f64 x, f64 y`) = **34 bytes**.
pub const IGPOINT2D_PAYLOAD_LEN: usize = 34;

/// PSM type code that identifies a standard Intergraph Sigma
/// `igTextBox` record on disk (IGDS class tag `0x4D = 77`).
///
/// Cross-fixture histogram surfaces 175 hits — text annotations on
/// `Sheet*` streams.
pub const PSM_TYPE_CODE_IGTEXTBOX: u16 = 0x004D;

/// Fixed byte overhead of a **sub-type 2** `igTextBox` payload: 22 header
/// bytes + a 10-byte body prefix + the 36-byte placement/style tail.
///
/// Long read as *the* overhead of the family, with the text length derived
/// from `bytes_to_follow` on that basis. It only ever described sub-type 2 —
/// the common shape, 215 of the corpus's 260 records — and the other two
/// shapes were refused wholesale because that arithmetic did not describe
/// them. See `igtextbox_body_shape` for the real per-shape layout and
/// `IGTEXTBOX_MIN_PAYLOAD_LEN` for the floor that replaced this as the
/// size gate.
pub const IGTEXTBOX_PAYLOAD_OVERHEAD: usize = 68;

/// Payload offset where an `igTextBox` body begins, after the shared header.
const IGTEXTBOX_BODY_START: usize = 22;

/// Bytes of placement/style that follow every `igTextBox` body: the
/// insertion point, the text direction, and a 4-byte trailer.
const IGTEXTBOX_TAIL_LEN: usize = 36;

/// Smallest `igTextBox` payload over every sub-type: sub-type 1 carrying no
/// text, which is 22 header + 2 body + 36 tail.
///
/// The old gate was 68 — sub-type 2's overhead — and it alone rejected the
/// corpus's empty and single-character sub-type 1 records, whose payloads
/// measure 60 and 62 bytes.
const IGTEXTBOX_MIN_PAYLOAD_LEN: usize = IGTEXTBOX_BODY_START + 2 + IGTEXTBOX_TAIL_LEN;

/// Maximum `text_length` (UTF-16LE chars) accepted. Real
/// `SmartPlant` fixture texts are short labels (e.g. tag names);
/// cap at 1024 chars to reject obvious noise.
const IGTEXTBOX_MAX_TEXT_LENGTH: u16 = 1024;

/// How far `(cos, sin)` may stray from unit length and still be read as a
/// text direction.
///
/// `igTextBox` stores its rotation the way `igSymbol2d` stores its placement:
/// as direction components rather than an angle. The pair is exact in the
/// corpus — every angle is `0`, `90` or `180` degrees, so the components are
/// `0` and `±1` — and this tolerance only absorbs float noise.
///
/// The check earns its place twice over. It is where the rotation comes from,
/// and it is the only test that catches a `text_length` which over-reads: a
/// length past the end of the real label puts these two reads on bytes that
/// are not the direction, and they miss unit length by orders of magnitude.
/// Measured in `docs/analysis/2026-08-13-igtextbox-rotation-is-a-direction-pair.md`.
const IGTEXTBOX_DIRECTION_UNIT_TOLERANCE: f64 = 1e-6;

/// PSM type code that identifies a standard Intergraph Sigma
/// `igSymbol2d` record on disk (IGDS class tag `0xCE = 206`).
///
/// Cross-fixture histogram surfaces 103 hits — `SmartPlant` symbol
/// instantiations (equipment, instruments, valves).
pub const PSM_TYPE_CODE_IGSYMBOL2D: u16 = 0x00CE;

/// Minimum byte length of one PSM `igSymbol2d` payload. Real
/// fixtures show 113-byte (most common) and 121-byte variants.
pub const IGSYMBOL2D_MIN_PAYLOAD_LEN: usize = 113;

/// Maximum byte length we'll accept on a decoded `igSymbol2d`.
const IGSYMBOL2D_MAX_PAYLOAD_LEN: usize = 200;

/// Byte tag that immediately precedes an `igSymbol2d` placement matrix.
///
/// The header between the OID block and the matrix is *not* a fixed
/// length: across the 109 records in the five fixtures it ends at payload
/// offset 33 or 35, tracking the two payload-size families. Reading the
/// matrix from a constant offset therefore lands mid-field and yields
/// denormal noise, which is what the first version of this decoder did.
/// Every record carries this tag exactly once
/// (`examples/probe_igsymbol2d_matrix.rs`), so it is the anchor.
const IGSYMBOL2D_MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];

/// How far into the payload the matrix tag is searched for. Observed
/// positions are 33 and 35; bounding the search stops a byte pair inside
/// the coordinate data from being mistaken for the tag.
const IGSYMBOL2D_TAG_SEARCH_END: usize = 64;

/// Offset of the placement's body style reference within an `igSymbol2d`
/// payload.
///
/// Fixed, unlike the tag-relative `jsite_ref`: the 113 / 115 / 121 / 123
/// byte payload variants all carry it here, and all 109 placements across
/// the five `test-file` fixtures resolve it against the root document's
/// `StyleCluster`. The byte before it reads `0x0B` on 103 of the 109 —
/// close to universal but not a law, so it is not used as a gate.
pub const IGSYMBOL2D_STYLE_REF_OFFSET: usize = 25;

/// PSM type code for `DependencyObject` records.
///
/// The class is `imagdex.dex`'s **Dependency Object**, resolved through the
/// PSM type-code table in `radsrvitem.dll` and the RAD CLSID registry in
/// `jutil.dll` (see `docs/analysis/2026-08-04-psm-type-code-registry.md`).
/// Phase 15 named it `GraphicGroup` / `GraphicPersist` on shape alone; that
/// was a guess and it was wrong. The two OID references its payload carries
/// are the two ends of a dependency, not an object and its graphics.
///
/// Phase 15 probe evidence (`examples/probe_psm_0x00fa_shape.rs` and
/// `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md`)
/// shows these records are standalone variable-size PSM records whose
/// payload starts with `oid`, `parent_ref`, a small kind/count word,
/// and a sub-type discriminator, followed by a reference-like raw tail.
pub const PSM_TYPE_CODE_DEPENDENCY_OBJECT: u16 = 0x00FA;

/// Minimum `0x00FA` payload size observed in current fixtures.
pub const DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN: usize = 44;

/// Maximum `0x00FA` payload size accepted by the conservative decoder.
///
/// Current fixtures top out at 200 bytes. The cap leaves room for
/// variant growth while rejecting obvious wide-scan false positives.
const DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN: usize = 512;

/// PSM type code the `GLine2d` `PrimitiveLine` decoder accepts.
///
/// **Retracted as a corpus identification, 2026-08-10.** The original
/// claim was that every record whose 18-byte PSM header is followed by 48
/// bytes matching the `GLine2d` validation rules carries
/// `type_code == 0x3FE6`. That is a tautology: the candidate set was
/// *defined* by those rules. Two independent tests both say no —
/// `radsrvitem.dll`'s authoritative type→class enumeration
/// (`sub_56448F70`) covers `0x06`..`0x117` and has no `0x3FE6`, and none
/// of the corpus's three former "records" sits on a record boundary. Each
/// sits 160 bytes inside an `igSmartFrame2d`, on the top two bytes of its
/// `1/√2` page-ratio `f64`. See
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.
///
/// The class is real — `radsrvitem.dll` has a `GLine2d::Validate`
/// (`sub_56524C50`) and the six-double parametric layout is its — so the
/// decoder stays, now gated on chain membership by
/// [`decode_primitive_lines`]. It decodes nothing on this corpus, which is
/// the correct answer for a corpus that contains none.
pub const PSM_TYPE_CODE_GLINE2D: u16 = 0x3FE6;

/// Unit-vector tolerance used when accepting a candidate `GLine2d`
/// record. The IDA-decoded `GLine2d::Validate` uses a strict
/// tolerance (`sub_56472D30()` returns ~1e-9 for normalized 2D),
/// but real `SmartPlant` writes can have direction vectors rounded
/// through coordinate transforms; 1e-3 catches every fixture record
/// we have without bringing in obvious false positives.
const GLINE2D_UNIT_VECTOR_TOLERANCE: f64 = 1e-3;

/// Sentinel beyond which an origin / parameter / direction value
/// is treated as garbage (uninitialized memory, out-of-domain).
const GLINE2D_COORDINATE_DOMAIN_LIMIT: f64 = 1e9;

/// PSM type code that identifies a RAD `JStyleOverride` record.
///
/// Phase 16 proved that the earlier Phase 14 `GArc2d` interpretation
/// of `0x0030` was a historical misidentification. The authoritative
/// implementation is RAD `style.dll` `JStyleOverride` (CLSID
/// `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`), documented in
/// `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`.
pub const PSM_TYPE_CODE_JSTYLE_OVERRIDE: u16 = 0x0030;

/// One decoded PSM `GLine2d` `PrimitiveLine` record.
///
/// Phase 14 anti-promotion guarantee: this DTO carries the **raw
/// parametric** geometry (`origin + t * direction` for
/// `t ∈ [param_start, param_end]`), not Cartesian `start`/`end`
/// endpoints, and exposes a [`Self::byte_range`] covering the full
/// PSM record. Producing
/// [`crate::geometry::PidGeometryConfidence::Decoded`] is the
/// responsibility of `geometry.rs`; this module only decodes
/// bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetPrimitiveLineDecoded {
    /// Byte range covering the **entire** PSM record (header +
    /// inner payload + any trailing attribute bytes the writer
    /// included in `bytes_to_follow`).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code from the record header. Always
    /// [`PSM_TYPE_CODE_GLINE2D`] for records this decoder emits.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    /// Documented as the `0x8000` / `0x4000` bits in
    /// `PSMSerializeIn`'s skip-record branch.
    pub type_flags: u16,
    /// `bytes_to_follow` field from the PSM header. The decoder
    /// uses this as the trailing edge of [`Self::byte_range`].
    pub bytes_to_follow: u32,
    /// `oid` field from the PSM header (the object identifier
    /// `SmartPlant` assigns to this geometry).
    pub oid: u32,
    /// Local-space origin: `point(t) = origin + t * direction`.
    pub origin: (f64, f64),
    /// Unit direction vector. `sqrt(direction_x^2 + direction_y^2)`
    /// is guaranteed within 1e-3 of 1.0 at decode time (a private
    /// `GLINE2D_UNIT_VECTOR_TOLERANCE` constant in this module).
    pub direction: (f64, f64),
    /// Parameter range start. `param_start < param_end` is
    /// guaranteed at decode time.
    pub param_start: f64,
    /// Parameter range end.
    pub param_end: f64,
}

impl SheetPrimitiveLineDecoded {
    /// Cartesian endpoint A computed from the parametric form
    /// (`origin + param_start * direction`).
    pub fn endpoint_a(&self) -> (f64, f64) {
        (
            self.origin.0 + self.param_start * self.direction.0,
            self.origin.1 + self.param_start * self.direction.1,
        )
    }

    /// Cartesian endpoint B computed from the parametric form
    /// (`origin + param_end * direction`).
    pub fn endpoint_b(&self) -> (f64, f64) {
        (
            self.origin.0 + self.param_end * self.direction.0,
            self.origin.1 + self.param_end * self.direction.1,
        )
    }
}

/// Decode every PSM-encoded `GLine2d` `PrimitiveLine` record in a
/// `Sheet*` stream's bytes.
///
/// A candidate has to be a **member of the stream's record chain**
/// ([`sheet_record_starts`]) before its payload is even looked at, and
/// then satisfy all of:
///
/// 1. PSM type code (header bytes 0..2 LE, masked to 14 bits) ==
///    [`PSM_TYPE_CODE_GLINE2D`];
/// 2. `bytes_to_follow` (header bytes 2..6 LE) is `>=` 48 and fits
///    within the remaining stream;
/// 3. All six payload `f64`s are finite (rejecting NaN / inf /
///    sub-domain values: `|x| <= 1e9`, a private
///    `GLINE2D_COORDINATE_DOMAIN_LIMIT` in this module);
/// 4. `(direction.x, direction.y)` has unit length within `1e-3`
///    (a private `GLINE2D_UNIT_VECTOR_TOLERANCE`) and is not the
///    zero vector;
/// 5. `param_start < param_end` strictly.
///
/// **Chain membership is the load-bearing rule, and it is why this family
/// no longer decodes anything on the reference corpus.** Rules 1–5 alone
/// are satisfiable by accident: a `[1 0; 0 1]` placement matrix reads as a
/// unit direction plus an ordered parameter pair, and `1/√2` — the ISO
/// A-series page ratio every `igSmartFrame2d` carries — is an `f64` whose
/// top two bytes are literally `E6 3F`. All eleven page frames in the
/// corpus spell the type code that way and three of them used to decode as
/// lines 160 bytes inside another record. See
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.
///
/// Adversarial bytes pass through without panics, and a stream that is not
/// a clean chain yields nothing rather than falling back to a scan.
pub fn decode_primitive_lines(data: &[u8]) -> Vec<SheetPrimitiveLineDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| GLine2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode a single PSM `GLine2d` `PrimitiveLine` starting at
/// `offset` in `data`. Returns `None` when any of the validation
/// rules in [`decode_primitive_lines`] fail. Bounds-checked: passing
/// `offset >= data.len()` or a truncated tail simply returns `None`.
///
/// Thin wrapper over [`GLine2dDecoder::decode_at`].
pub fn decode_primitive_line_at(data: &[u8], offset: usize) -> Option<SheetPrimitiveLineDecoded> {
    GLine2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `SmartPlant` extended
/// `GLine2d` family (PSM type `0x3FE6`).
///
/// The family deviation from the standard IGDS record shape lives
/// here, not in the shared seam: `GLine2d` uses the extended
/// [`PSM_RECORD_HEADER_LEN`] (18-byte) header carrying `oid` at
/// header bytes 6..10 (standard families carry `oid` in their payload
/// sub-header), and its `bytes_to_follow` may exceed the 48-byte
/// geometry payload — the trailing attribute bytes are included in
/// the decoded [`SheetPrimitiveLineDecoded::byte_range`] so
/// [`PsmRecordDecoder::scan`] jumps the whole record.
pub struct GLine2dDecoder;

impl PsmRecordDecoder for GLine2dDecoder {
    type Record = SheetPrimitiveLineDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_GLINE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetPrimitiveLineDecoded> {
        let envelope = parse_psm_header(data, offset)?;
        if envelope.type_code != PSM_TYPE_CODE_GLINE2D {
            return None;
        }
        let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
        let payload_end = header_end.checked_add(GLINE2D_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        let bytes_to_follow_usize = envelope.bytes_to_follow as usize;
        // bytes_to_follow must cover at least the 48-byte payload.
        if bytes_to_follow_usize < GLINE2D_PAYLOAD_LEN {
            return None;
        }
        // The full record (envelope + bytes_to_follow trailer) must fit.
        let record_end = offset.checked_add(PSM_ENVELOPE_LEN + bytes_to_follow_usize)?;
        if record_end > data.len() {
            return None;
        }
        decode_primitive_line_payload(data, offset, &envelope, header_end, payload_end, record_end)
    }

    fn advance_of(&self, record: &SheetPrimitiveLineDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `GLine2d` (the 18-byte
/// extended header's `oid` plus the six-double parametric payload).
fn decode_primitive_line_payload(
    data: &[u8],
    offset: usize,
    envelope: &PsmHeader,
    header_end: usize,
    payload_end: usize,
    record_end: usize,
) -> Option<SheetPrimitiveLineDecoded> {
    let type_code = envelope.type_code;
    let type_flags = envelope.type_flags;
    let bytes_to_follow = envelope.bytes_to_follow;

    let header = data.get(offset..header_end)?;
    let oid = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);

    // 6-double GLine2d payload at offset + 18.
    let payload = data.get(header_end..payload_end)?;
    let mut d = [0f64; 6];
    for (i, slot) in d.iter_mut().enumerate() {
        let chunk = payload.get(i * 8..i * 8 + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !d.iter().all(|x| x.is_finite()) {
        return None;
    }
    if d.iter().any(|x| x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT) {
        return None;
    }
    let dir_x = d[2];
    let dir_y = d[3];
    if dir_x.abs() < 1e-12 && dir_y.abs() < 1e-12 {
        return None;
    }
    let unit_err = ((dir_x * dir_x + dir_y * dir_y).sqrt() - 1.0).abs();
    if unit_err > GLINE2D_UNIT_VECTOR_TOLERANCE {
        return None;
    }
    let param_start = d[4];
    let param_end = d[5];
    if param_start >= param_end {
        return None;
    }

    Some(SheetPrimitiveLineDecoded {
        byte_range: offset..record_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        origin: (d[0], d[1]),
        direction: (dir_x, dir_y),
        param_start,
        param_end,
    })
}

// ---------------------------------------------------------------------------
// Phase 14 Slice J: PSM-encoded igLine2d decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `igLine2d` record — Intergraph Sigma's standard
/// 2D line primitive (PSM type `0x0018`, IGDS class tag `0x18`).
///
/// **Byte layout** (fully revealed via fixture byte dump in
/// `examples/probe_igline2d_shape.rs`; total 56 bytes = 6-byte PSM
/// header + 50-byte payload):
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16 LE   type_code = 0x0018
///   2..5   u32 LE   bytes_to_follow = 50
///
/// Payload (50 bytes):
///   0..3   u32 LE   oid          \ still the shared PSM envelope
///   4..7   u32 LE   aux_lo       | (`PSMSerializeOut` writes these;
///   8..11  u32 LE   aux_hi       / `PSMSerializeIn` discards the aux)
///   12..13 u16 LE   sub_type_word
///   14..17 u32 LE   index / sub_oid
///   18..25 f64 LE   start.x
///   26..33 f64 LE   start.y
///   34..41 f64 LE   end.x
///   42..49 f64 LE   end.y
/// ```
///
/// The crate read `aux_hi` as a `remaining_header` sub-header length
/// and refused every record whose value was not `12`, which silently
/// dropped 88 real lines — among them the whole page border of `A01`.
/// It is not a length: the native reader reads those 8 bytes into a
/// local it never consults. Chain membership, not `aux_hi`, is what
/// tells a record from a coincidence.
///
/// 4-double Cartesian `(start, end)` representation. Compare to
/// the parametric 6-double `GLine2d` (PSM `0x3FE6`) family, which
/// is `SmartPlant`'s extended wrapper of the same geometric concept.
/// `igLine2d` is by far the more common form in real fixtures
/// (309 vs 3 cross-fixture hits).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgLine2dDecoded {
    /// Byte range covering the full PSM record (6-byte header +
    /// 50-byte payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGLINE2D`]
    /// (`0x0018` = decimal 24, matching IGDS class tag for
    /// `igLine2d`).
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header; always 50 for valid
    /// records.
    pub bytes_to_follow: u32,
    /// Object identifier (payload bytes 0..3).
    pub oid: u32,
    /// Low half of the PSM envelope's `aux` pair (payload bytes
    /// 4..7). The crate reads it as the owning `PrimitiveCluster` or
    /// entity; that reading is hypothesis, and `PSMSerializeOut`
    /// sources it from object-record bookkeeping rather than from the
    /// class, so it is not part of the line's geometry.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this line sits on (payload bytes 8..11).
    pub sheet_layer_ref: u32,
    /// High half of the PSM envelope's `aux` pair (payload bytes
    /// 8..11), verbatim. Corpus values are `12` (326 records), `8`
    /// (`A01`'s page border, 80) and `6996` (`DWG-0202/Sheet6615`, 8).
    /// Carried for evidence only — the native reader discards it, so
    /// nothing validates against it.
    pub aux_hi: u32,
    /// Sub-type discriminator (payload bytes 12..13). Seen values
    /// include `0x0010`, `0x0001`, `0x0065`, `0x0032`, `0x0023`,
    /// `0x001F`, `0x002B`; semantics not yet decoded.
    pub sub_type_word: u16,
    /// Index / sub-oid (payload bytes 14..17).
    pub index: u32,
    /// Start point of the line segment.
    pub start: (f64, f64),
    /// End point of the line segment.
    pub end: (f64, f64),
}

impl SheetIgLine2dDecoded {
    /// Length of the line segment (`= ‖end − start‖`).
    pub fn length(&self) -> f64 {
        let dx = self.end.0 - self.start.0;
        let dy = self.end.1 - self.start.1;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Decode every PSM-encoded `igLine2d` record in a `Sheet*`
/// stream's bytes.
///
/// A candidate has to be a **member of the stream's record chain**
/// ([`sheet_record_starts`]) before its payload is even looked at —
/// the same gate `decode_primitive_lines` has used since the
/// `GLine2d` ghost family was retired — and then satisfy all of:
///
/// 1. PSM `type_code` == [`PSM_TYPE_CODE_IGLINE2D`] (`0x0018`);
/// 2. `bytes_to_follow` == 50 (igLine2d records are fixed-size);
/// 3. All four geometry `f64`s are finite and in domain
///    `[-1e9, 1e9]`;
/// 4. `start != end` (line must be non-degenerate; collapse
///    rejected to avoid noise).
///
/// The chain replaced a fourth rule this crate invented, `aux_hi ==
/// 12`, which had no counterpart in the native reader and refused 88
/// real lines. On the corpus the swap is free in both directions:
/// every record the old sliding scan accepted is chain-resident
/// (326, zero off-chain), and dropping the invented rule adds exactly
/// the 88 (`examples/probe_phase40_igline_chain_vs_rule`).
///
/// Panic-free and bounds-checked: adversarial bytes simply fail
/// validation and are skipped, and a stream that is not a clean chain
/// yields nothing rather than falling back to a scan.
pub fn decode_iglines(data: &[u8]) -> Vec<SheetIgLine2dDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| IgLine2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode a single PSM `igLine2d` record starting at
/// `offset` in `data`. Returns `None` when any of the validation
/// rules in [`decode_iglines`] fail. Bounds-checked; tolerant of
/// truncated input.
///
/// Thin wrapper over [`IgLine2dDecoder::decode_at`].
pub fn decode_igline_at(data: &[u8], offset: usize) -> Option<SheetIgLine2dDecoded> {
    IgLine2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igLine2d` family (PSM type
/// `0x0018`) — the M1 pilot migration of the seven-layer template
/// onto the shared decoder seam. Validation rules are documented on
/// [`decode_iglines`].
pub struct IgLine2dDecoder;

impl PsmRecordDecoder for IgLine2dDecoder {
    type Record = SheetIgLine2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGLINE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGLINE2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgLine2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGLINE2D {
            return None;
        }
        if header.bytes_to_follow as usize != IGLINE2D_PAYLOAD_LEN {
            return None;
        }
        let payload_end = header.body_start.checked_add(IGLINE2D_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igline_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgLine2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igLine2d` (everything after
/// the shared PSM envelope). Split out of
/// [`IgLine2dDecoder::decode_at`] so the envelope handling and payload
/// rules read separately.
fn decode_igline_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgLine2dDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;
    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    // `aux_hi` is the PSM envelope's own bookkeeping, read and discarded by
    // `PSMSerializeIn`. Carried through as evidence; never a validation rule.
    let aux_hi = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let index = u32::from_le_bytes([payload[14], payload[15], payload[16], payload[17]]);

    // Parse 4 f64 doubles at offsets 18, 26, 34, 42.
    let mut d = [0f64; 4];
    for (i, slot) in d.iter_mut().enumerate() {
        let pos = 18 + i * 8;
        let chunk = payload.get(pos..pos + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !d.iter().all(|x| x.is_finite()) {
        return None;
    }
    if d.iter().any(|x| x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT) {
        return None;
    }

    let start = (d[0], d[1]);
    let end = (d[2], d[3]);
    // Reject degenerate (zero-length) lines.
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    if dx.abs() < 1e-12 && dy.abs() < 1e-12 {
        return None;
    }

    Some(SheetIgLine2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref: aux_hi,
        aux_hi,
        sub_type_word,
        index,
        start,
        end,
    })
}

// ---------------------------------------------------------------------------
// Phase 14 Slice K: PSM-encoded igLineString2d (polyline) decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `igLineString2d` record — Intergraph Sigma's
/// standard 2D polyline primitive (PSM type `0x0084`, IGDS class
/// tag `0x84`).
///
/// **Byte layout** (fully revealed via fixture byte dump in
/// `examples/probe_iglinestring2d_shape.rs`; total
/// `6 + 24 + vc*16` bytes where `vc = vertex_count`):
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16 LE   type_code = 0x0084
///   2..5   u32 LE   bytes_to_follow = 24 + vc * 16
///
/// Payload (24 + vc * 16 bytes):
///   0..3   u32 LE   oid
///   4..7   u32 LE   parent_ref
///   8..11  u32 LE   remaining_header (variable: 0x08, 0x0C, 0x11)
///   12..13 u16 LE   sub_type_word
///   14..17 u32 LE   index
///   18..21 u32 LE   vertex_count (>= 2)
///   22     u8       form  (0..=6)
///   23     u8       scope (0..=4 or == 6)
///   24..   vc × 16 bytes  (f64 LE x, f64 LE y) per vertex
/// ```
///
/// Each polyline has at least 2 vertices. `form` / `scope` byte
/// upper bounds come from `radsrvitem.dll!sub_56524DD0`
/// (`GLineString2d::Validate`) decompile.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgLineString2dDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_IGLINESTRING2D`] (`0x0084`).
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header (`= 24 + vc*16`).
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this line string sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator at payload bytes 12..13.
    pub sub_type_word: u16,
    /// Index / sub-oid at payload bytes 14..17.
    pub index: u32,
    /// `form` byte (memory-layout `*(_BYTE *)(a2 + 8)`, 0..=6).
    pub form: u8,
    /// `scope` byte (memory-layout `*(_BYTE *)(a2 + 9)`, 0..=4 or
    /// `== 6`).
    pub scope: u8,
    /// Polyline vertices in source order.
    pub vertices: Vec<(f64, f64)>,
}

impl SheetIgLineString2dDecoded {
    /// Number of vertices in this polyline (`= self.vertices.len()`).
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Cumulative polyline length (sum of segment lengths).
    pub fn total_length(&self) -> f64 {
        self.vertices
            .windows(2)
            .map(|w| {
                let dx = w[1].0 - w[0].0;
                let dy = w[1].1 - w[0].1;
                (dx * dx + dy * dy).sqrt()
            })
            .sum()
    }
}

/// Decode every PSM-encoded `igLineString2d` record in a `Sheet*`
/// stream's bytes.
///
/// Walk every byte offset and verify:
///
/// 1. PSM `type_code` == [`PSM_TYPE_CODE_IGLINESTRING2D`];
/// 2. `bytes_to_follow >= IGLINESTRING2D_MIN_PAYLOAD_LEN` (56);
/// 3. `(bytes_to_follow - 24) % 16 == 0` (vertex bytes evenly
///    divisible);
/// 4. `vertex_count = (bytes_to_follow - 24) / 16` and the inline
///    `vertex_count` field at payload offset 18 match;
/// 5. `vertex_count` in `[2, IGLINESTRING2D_MAX_VERTEX_COUNT]`;
/// 6. `form <= 6`;
/// 7. `scope <= 4` or `scope == 6`;
/// 8. All vertex coordinates finite and in domain `[-1e9, 1e9]`;
/// 9. Not all vertices identical (non-degenerate polyline).
///
/// Thin wrapper over [`IgLineString2dDecoder`]'s shared
/// [`PsmRecordDecoder::scan`].
pub fn decode_iglinestrings(data: &[u8]) -> Vec<SheetIgLineString2dDecoded> {
    IgLineString2dDecoder.scan(data)
}

/// Try to decode a single PSM `igLineString2d` record starting at
/// `offset`. Returns `None` when any validation rule in
/// [`decode_iglinestrings`] fails. Bounds-checked and panic-free.
///
/// Thin wrapper over [`IgLineString2dDecoder::decode_at`].
pub fn decode_iglinestring_at(data: &[u8], offset: usize) -> Option<SheetIgLineString2dDecoded> {
    IgLineString2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igLineString2d` family (PSM
/// type `0x0084`). Validation rules are documented on
/// [`decode_iglinestrings`].
pub struct IgLineString2dDecoder;

impl PsmRecordDecoder for IgLineString2dDecoder {
    type Record = SheetIgLineString2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGLINESTRING2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGLINESTRING2D_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgLineString2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGLINESTRING2D {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        if btf < IGLINESTRING2D_MIN_PAYLOAD_LEN {
            return None;
        }
        if !(btf - 24).is_multiple_of(16) {
            return None;
        }
        let computed_vc = ((btf - 24) / 16) as u32;
        if !(2..=IGLINESTRING2D_MAX_VERTEX_COUNT).contains(&computed_vc) {
            return None;
        }
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_iglinestring_payload(data, offset, &header, payload_end, computed_vc)
    }

    fn advance_of(&self, record: &SheetIgLineString2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igLineString2d`
/// (everything after the shared PSM envelope and vertex-count
/// pre-checks).
fn decode_iglinestring_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
    computed_vc: u32,
) -> Option<SheetIgLineString2dDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sheet_layer_ref = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    // remaining_header at +8..11 — variable across records, not validated
    // strictly; rejected only if absurdly large.
    let remaining_header = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    if remaining_header > 0xFFFF {
        return None;
    }
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let index = u32::from_le_bytes([payload[14], payload[15], payload[16], payload[17]]);
    let inline_vc = u32::from_le_bytes([payload[18], payload[19], payload[20], payload[21]]);
    if inline_vc != computed_vc {
        return None;
    }
    let form = payload[22];
    let scope = payload[23];
    if form > IGLINESTRING2D_FORM_MAX {
        return None;
    }
    if scope > 4 && scope != 6 {
        return None;
    }

    // Parse vertices.
    let mut vertices = Vec::with_capacity(computed_vc as usize);
    let mut all_same = true;
    let first_coord = (
        f64::from_le_bytes([
            payload[24],
            payload[25],
            payload[26],
            payload[27],
            payload[28],
            payload[29],
            payload[30],
            payload[31],
        ]),
        f64::from_le_bytes([
            payload[32],
            payload[33],
            payload[34],
            payload[35],
            payload[36],
            payload[37],
            payload[38],
            payload[39],
        ]),
    );
    for i in 0..computed_vc as usize {
        let pos = 24 + i * 16;
        let chunk = payload.get(pos..pos + 16)?;
        let x = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        let y = f64::from_le_bytes([
            chunk[8], chunk[9], chunk[10], chunk[11], chunk[12], chunk[13], chunk[14], chunk[15],
        ]);
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        if x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT || y.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
            return None;
        }
        if i > 0 && ((x - first_coord.0).abs() > 1e-12 || (y - first_coord.1).abs() > 1e-12) {
            all_same = false;
        }
        vertices.push((x, y));
    }
    // Reject degenerate (all-vertex-identical) polyline.
    if all_same {
        return None;
    }

    Some(SheetIgLineString2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        index,
        form,
        scope,
        vertices,
    })
}

// ---------------------------------------------------------------------------
// Phase 14 Slice L: PSM-encoded igPoint2d decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `igPoint2d` record — Intergraph Sigma's
/// standard 2D point primitive (PSM type `0x005E`, IGDS class
/// tag `0x5E`).
///
/// **Byte layout** (revealed via fixture byte dump in
/// `examples/probe_igpoint2d_shape.rs`; total 40 bytes =
/// 6-byte PSM header + 34-byte payload):
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16   type_code = 0x005E
///   2..5   u32   bytes_to_follow = 34
///
/// Payload (34 bytes):
///   0..3   u32   oid
///   4..7   u32   parent_ref
///   8..11  u32   remaining_header (variable: 0x08, 0x12)
///   12..13 u16   sub_type_word
///   14..17 u32   index
///   18..25 f64   x
///   26..33 f64   y
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgPoint2dDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGPOINT2D`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header; always 34.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this point sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// Point `(x, y)`.
    pub point: (f64, f64),
}

/// Decode every PSM-encoded `igPoint2d` record in a `Sheet*`
/// stream's bytes.
///
/// Walk every byte offset and verify:
/// 1. `type_code == 0x005E`
/// 2. `bytes_to_follow == 34`
/// 3. 2 doubles finite + in domain `[-1e9, 1e9]`
///
/// Panic-free and bounds-checked.
///
/// Thin wrapper over [`IgPoint2dDecoder`]'s shared
/// [`PsmRecordDecoder::scan`].
pub fn decode_igpoints(data: &[u8]) -> Vec<SheetIgPoint2dDecoded> {
    IgPoint2dDecoder.scan(data)
}

/// Try to decode a single PSM `igPoint2d` record starting at
/// `offset`. Returns `None` on validation failure.
///
/// Thin wrapper over [`IgPoint2dDecoder::decode_at`].
pub fn decode_igpoint_at(data: &[u8], offset: usize) -> Option<SheetIgPoint2dDecoded> {
    IgPoint2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igPoint2d` family (PSM type
/// `0x005E`). Validation rules are documented on [`decode_igpoints`].
pub struct IgPoint2dDecoder;

impl PsmRecordDecoder for IgPoint2dDecoder {
    type Record = SheetIgPoint2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGPOINT2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGPOINT2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgPoint2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGPOINT2D {
            return None;
        }
        if header.bytes_to_follow as usize != IGPOINT2D_PAYLOAD_LEN {
            return None;
        }
        let payload_end = header.body_start.checked_add(IGPOINT2D_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igpoint_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgPoint2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igPoint2d` (everything
/// after the shared PSM envelope).
fn decode_igpoint_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgPoint2dDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;
    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sheet_layer_ref = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let index = u32::from_le_bytes([payload[14], payload[15], payload[16], payload[17]]);

    let x = f64::from_le_bytes([
        payload[18],
        payload[19],
        payload[20],
        payload[21],
        payload[22],
        payload[23],
        payload[24],
        payload[25],
    ]);
    let y = f64::from_le_bytes([
        payload[26],
        payload[27],
        payload[28],
        payload[29],
        payload[30],
        payload[31],
        payload[32],
        payload[33],
    ]);
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    if x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT || y.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
        return None;
    }

    Some(SheetIgPoint2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        index,
        point: (x, y),
    })
}

// ---------------------------------------------------------------------------
// Phase 14 Slice M: PSM-encoded igTextBox decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `igTextBox` record — Intergraph Sigma's
/// standard text annotation primitive (PSM type `0x004D`, IGDS
/// class tag `0x4D`).
///
/// **Byte layout** (revealed via fixture byte dump in
/// `examples/probe_igtextbox_shape.rs`; total = 6-byte PSM header
/// + a payload of at least `68 + text_length * 2` bytes):
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16   type_code = 0x004D
///   2..5   u32   bytes_to_follow >= 68 + text_length * 2
///
/// Payload (>= 68 + text_length * 2 bytes):
///   0..3    u32   oid
///   4..7    u32   parent_ref
///   8..11   u32   remaining_header
///   12..13  u16   sub_type_word
///   14..17  u32   index
///   18..29  12 bytes  sub-fields (length flags + sub-index)
///   30..31  u16   text_length (UTF-16LE chars) — the record's own
///                 statement, and the one this decoder believes
///   32..    UTF-16LE chars × text_length × 2 bytes
///   then    16 bytes  2 × f64 insertion point
///   then    16 bytes  2 × f64 text direction (cos, sin) — a unit vector,
///                 the rotation this record letters at
///   then    4 bytes   trailer
///   then    0, 32 or 40 further bytes on 33 of the corpus's 224
///                 records — undecoded, and the reason the payload is
///                 bounded below rather than exactly
/// ```
///
/// Text content is decoded from UTF-16LE to a Rust `String`.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgTextBoxDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGTEXTBOX`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this text box sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// Which of the three `igTextBox` shapes this record is, from payload
    /// `+18`. Decides where the text length and the body end sit — see
    /// `igtextbox_body_shape`.
    pub text_sub_type: u16,
    /// Inline text length (UTF-16LE chars).
    pub text_length: u16,
    /// Decoded text content (lossy UTF-16LE → UTF-8 conversion).
    pub text: String,
    /// First trailing f64 triple (presumed `insertion.x`).
    pub trailing_double_1: f64,
    /// Second trailing f64 (`insertion.y`).
    pub trailing_double_2: f64,
    /// Third trailing f64: the **cosine** of the text direction.
    ///
    /// Long read as "often 1.0 — scale or marker", because on its own it only
    /// ever takes `0` or `±1`. It is the first half of the direction pair
    /// whose second half sits 8 bytes further on; together they are a unit
    /// vector on every record whose text length is right. See
    /// [`SheetIgTextBoxDecoded::rotation_rad`].
    pub trailing_double_3: f64,
    /// Text rotation in radians, from the `(cos, sin)` pair after the text.
    ///
    /// `0`, `π/2` and `π` are the only values the corpus holds — a P&ID
    /// letters horizontally, up the side of a vertical pipe run, or upside
    /// down. Renderers want degrees: `rotation_rad.to_degrees()`.
    pub rotation_rad: f64,
    /// The record's formatting runs, in stored order — see [`IgTextBoxRun`].
    /// Empty for shape 1, which is the shape a record without runs takes.
    pub runs: Vec<IgTextBoxRun>,
}

/// One formatting run of an `igTextBox`: `len` characters lettered with the
/// style `style_id` names, loaded into the slot `selector` says.
///
/// On disk it is an 8-byte entry `(u16 len, u16 selector, u32 style_id)`.
/// Selector 1 names the `JStyleTextChar` those characters draw in (313 of the
/// corpus's 320 such runs; the other seven name a `JStyleTextPara` or a
/// `JStyleSimpleLine`). Selector 2 restates the record's own paragraph style,
/// `+14`, on all 13 that exist. The native sizer refuses a record whose
/// selector-1 lengths do not sum to its character count, so where a record
/// has selector-1 runs at all, every character is in one — and the paragraph
/// style's character style is then only the default they override. Which
/// shape a record takes follows from its runs: shape 1 has none, shape 2
/// exactly one selector-1 run, stored ahead of the count, and shape 3 stores
/// `A` selector-1 then `B` selector-2 runs after its text. See
/// `docs/analysis/2026-08-22-igtextbox-tail-kind-2-and-formatting-runs.md`
/// and `docs/analysis/2026-08-22-run-beats-paragraph-default.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgTextBoxRun {
    /// Characters the run covers.
    pub len: u16,
    /// The slot the run loads into: `1` a character style, `2` the paragraph
    /// style.
    pub selector: u16,
    /// Style id the run names, in the record's own document.
    pub style_id: u32,
}

/// Decode every PSM-encoded `igTextBox` record in a `Sheet*` stream.
///
/// Validation — every rule is about the record's own shape; none
/// assumes a payload size measured off one batch of fixtures:
/// 1. `type_code == 0x004D`;
/// 2. `bytes_to_follow >= 68` (the floor: head + tail around an empty
///    text) and the payload fits in the stream;
/// 3. the inline `text_length` at payload offset 30 is `<= 1024`
///    (reject obvious noise) and the payload has room for that text
///    plus the 36-byte trailing block;
/// 4. the 4 doubles after the text are finite + in domain;
/// 5. the last two of them are a unit vector — the text direction. This
///    is also what checks rule 3's stated length: a length that over-runs
///    the label puts this read on the wrong bytes, and they are not a
///    direction.
///
/// Retired in 2026-08-12: rule 3 used to *derive* the length from
/// `bytes_to_follow` assuming a fixed 68-byte overhead and demand the
/// inline length agree. That refused 33 records whose tail is longer
/// than 36 bytes — 20 of them ordinary readable labels. See
/// [`IGTEXTBOX_PAYLOAD_OVERHEAD`].
///
/// Thin wrapper over [`IgTextBoxDecoder`]'s shared
/// [`PsmRecordDecoder::scan`].
pub fn decode_igtextboxes(data: &[u8]) -> Vec<SheetIgTextBoxDecoded> {
    IgTextBoxDecoder.scan(data)
}

/// Try to decode a single PSM `igTextBox` record starting at
/// `offset`. Returns `None` on validation failure.
///
/// Thin wrapper over [`IgTextBoxDecoder::decode_at`].
pub fn decode_igtextbox_at(data: &[u8], offset: usize) -> Option<SheetIgTextBoxDecoded> {
    IgTextBoxDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igTextBox` family (PSM type
/// `0x004D`). Validation rules are documented on
/// [`decode_igtextboxes`].
pub struct IgTextBoxDecoder;

impl PsmRecordDecoder for IgTextBoxDecoder {
    type Record = SheetIgTextBoxDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGTEXTBOX
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGTEXTBOX_PAYLOAD_OVERHEAD
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgTextBoxDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGTEXTBOX {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        // The smallest an `igTextBox` payload can be, over every sub-type:
        // sub-type 1 with no text is 22 header + 2 body + 36 tail. The
        // per-sub-type bound is applied in the payload decoder, which is
        // where the body length is known.
        if btf < IGTEXTBOX_MIN_PAYLOAD_LEN {
            return None;
        }
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igtextbox_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgTextBoxDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Where the text is and how long the body runs, for one of the three
/// `igTextBox` shapes — `(text_length, text_start, body_len)`, all
/// payload-relative.
///
/// Read out of `radsrvitem.dll`'s `igTextBox` Load (`sub_56498C00`) and its
/// size helper (`sub_5646D450`), and checked against every `0x004D` record on
/// the corpus chain: the formula fits all 260, and sub-type 2's redundant
/// dword agrees with its count on all 215. Evidence level: native-reader.
/// See `docs/analysis/2026-08-13-igtextbox-has-three-shapes.md`.
///
/// `None` for an unknown discriminator or a payload too short for the fields
/// the shape names.
fn igtextbox_body_shape(payload: &[u8], text_sub_type: u16) -> Option<(u16, usize, usize)> {
    let u16_at = |at: usize| -> Option<u16> {
        payload
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    match text_sub_type {
        1 => {
            let count = u16_at(22)?;
            Some((count, 24, 2 + 2 * count as usize))
        }
        2 => {
            // The count appears twice: once packed with a `0x10000` marker at
            // +22 and once plain at +30. Requiring them to agree is free here
            // and rejects a shape that only looks like sub-type 2.
            let packed = payload
                .get(22..26)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))?;
            let count = u16_at(30)?;
            if packed != (count as u32 | 0x1_0000) {
                return None;
            }
            Some((count, 32, 10 + 2 * count as usize))
        }
        3 => {
            // Two counts of doubles trail the text; they are part of the body,
            // so the placement tail starts past them.
            let a = u16_at(22)? as usize;
            let b = u16_at(24)? as usize;
            let count = u16_at(26)?;
            Some((count, 28, 6 + 2 * count as usize + 8 * (a + b)))
        }
        _ => None,
    }
}

/// The formatting runs one `igTextBox` body stores, in stored order: none for
/// shape 1, the single entry at the body start for shape 2 (whose `len` and
/// `selector` are the `count | 0x10000` dword [`igtextbox_body_shape`] checks),
/// and the `A + B` entries after the text for shape 3.
///
/// Every entry lies inside the body length [`igtextbox_body_shape`] measured,
/// so on a record that decoded this cannot come up short; an entry that
/// would is left out rather than refusing a record the other rules accepted.
fn igtextbox_runs(payload: &[u8], text_sub_type: u16, text_length: u16) -> Vec<IgTextBoxRun> {
    let entry = |at: usize| -> Option<IgTextBoxRun> {
        let b = payload.get(at..at + 8)?;
        Some(IgTextBoxRun {
            len: u16::from_le_bytes([b[0], b[1]]),
            selector: u16::from_le_bytes([b[2], b[3]]),
            style_id: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        })
    };
    let u16_at = |at: usize| -> usize {
        payload
            .get(at..at + 2)
            .map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]])))
    };
    match text_sub_type {
        2 => entry(IGTEXTBOX_BODY_START).into_iter().collect(),
        3 => {
            let count = u16_at(IGTEXTBOX_BODY_START) + u16_at(IGTEXTBOX_BODY_START + 2);
            let first = IGTEXTBOX_BODY_START + 6 + 2 * usize::from(text_length);
            (0..count).filter_map(|i| entry(first + 8 * i)).collect()
        }
        _ => Vec::new(),
    }
}

/// Family-specific payload validation for `igTextBox` (everything
/// after the shared PSM envelope and the minimum-size pre-check).
fn decode_igtextbox_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgTextBoxDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sheet_layer_ref = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let index = u32::from_le_bytes([payload[14], payload[15], payload[16], payload[17]]);

    // Which of the three shapes this record is, and therefore where its text
    // length lives. Reading `+30` unconditionally only ever worked because
    // sub-type 2 is the common one; on the other two that offset is some
    // other field, which is why they were refused wholesale.
    let text_sub_type = u16::from_le_bytes([payload[18], payload[19]]);
    let (inline_text_length, text_start, body_len) = igtextbox_body_shape(payload, text_sub_type)?;
    if inline_text_length > IGTEXTBOX_MAX_TEXT_LENGTH {
        return None;
    }

    // The payload is 22 header bytes, then the sub-type's body, then the
    // 36-byte placement/style tail. A record may carry more after the tail;
    // requiring room rather than an exact fit is what lets those through.
    let tail_start = IGTEXTBOX_BODY_START.checked_add(body_len)?;
    if tail_start.checked_add(IGTEXTBOX_TAIL_LEN)? > payload.len() {
        return None;
    }
    let text_byte_len = (inline_text_length as usize) * 2;
    if text_start.checked_add(text_byte_len)? > payload.len() {
        return None;
    }
    let mut u16_chars = Vec::with_capacity(inline_text_length as usize);
    for i in 0..inline_text_length as usize {
        let pos = text_start + i * 2;
        u16_chars.push(u16::from_le_bytes([payload[pos], payload[pos + 1]]));
    }
    let text = String::from_utf16_lossy(&u16_chars);

    // Four doubles open the tail: the insertion point, then the direction
    // pair `(cos, sin)` the text is lettered along.
    let mut trailing = [0f64; 4];
    for (i, slot) in trailing.iter_mut().enumerate() {
        let pos = tail_start + i * 8;
        let chunk = payload.get(pos..pos + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !trailing.iter().all(|x| x.is_finite()) {
        return None;
    }
    if trailing
        .iter()
        .any(|x| x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT)
    {
        return None;
    }

    // The direction has to be a direction. This is what makes the stated text
    // length checkable: read the pair off the wrong offset -- which is what a
    // length that over-runs the label produces -- and it misses unit length by
    // orders of magnitude rather than by float noise.
    let (cos, sin) = (trailing[2], trailing[3]);
    if (cos.hypot(sin) - 1.0).abs() > IGTEXTBOX_DIRECTION_UNIT_TOLERANCE {
        return None;
    }
    let rotation_rad = sin.atan2(cos);
    let runs = igtextbox_runs(payload, text_sub_type, inline_text_length);

    Some(SheetIgTextBoxDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        index,
        text_sub_type,
        text_length: inline_text_length,
        text,
        trailing_double_1: trailing[0],
        trailing_double_2: trailing[1],
        trailing_double_3: trailing[2],
        rotation_rad,
        runs,
    })
}

// ---------------------------------------------------------------------------
// Phase 14 Slice N: PSM-encoded igSymbol2d decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `igSymbol2d` record — Intergraph Sigma's
/// standard symbol-instance primitive (PSM type `0x00CE`, IGDS
/// class tag `0xCE`).
///
/// In `SmartPlant` `Sheet*` streams these are the placed instances
/// of equipment / instrument / valve symbols. Fixture byte dump
/// (`examples/probe_igsymbol2d_shape.rs`) shows two common sizes:
/// 113 bytes (most common) and 121 bytes (with an extra 8-byte
/// scalar). Layout extracted conservatively:
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16   type_code = 0x00CE
///   2..5   u32   bytes_to_follow ∈ {113, 115, 121, 123, ...}
///
/// Payload (variable, ≥ 113 bytes):
///   0..3    u32   oid
///   4..7    u32   parent_ref
///   8..11   u32   remaining_header
///   12..13  u16   sub_type_word
///   14..T-5 variable sub-fields (flags, references, sub-IDs)
///   T-4..T-1 u32  jsite_ref — id of the `JSite<id>` storage that
///                 names this instance's `.sym` path (Phase 35-C,
///                 132/132 records across 6 fixtures)
///   T..T+3  4 bytes   `IGSYMBOL2D_MATRIX_TAG`, T is 33 or 35
///   +0..7   f64   transform[0]  ⎫ 2×2 placement matrix, row-major.
///   +8..15  f64   transform[1]  ⎪ Fixtures hold only the eight
///   +16..23 f64   transform[2]  ⎪ axis-aligned placements: entries
///   +24..31 f64   transform[3]  ⎭ are 0 or ±1, `det` is ±1.
///   +32..39 f64   insertion.x   ⎫ translation, in sheet units (0..1)
///   +40..47 f64   insertion.y   ⎭
///   +48..55 f64   uniform scale; 1.0 in 108 of 109 fixture records
///   ...     variable tail (symbol library ref + class ID + flags)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgSymbol2dDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGSYMBOL2D`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference (often the symbol library or page).
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this symbol placement sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Numeric id of the top-level `JSite<id>` storage carrying this
    /// instance's symbol reference, read from the 4 bytes immediately
    /// preceding `IGSYMBOL2D_MATRIX_TAG`. Phase 35-C cross-fixture
    /// probe (`examples/probe_igsymbol2d_jsite_link.rs`): 132/132
    /// records across all 6 fixtures carry a value equal to a same-file
    /// `JSite<id>` id, and the referenced site's `JProperties` names the
    /// `.sym` library path. See
    /// `docs/analysis/2026-07-26-phase35c-igsymbol2d-jsite-link.md`.
    ///
    /// The `PSMspacemap` corroborates this from a second, offset-independent
    /// direction: the site's space-map entry records this symbol as an
    /// incoming reference tagged `181`, and this `jsite_ref` equals that
    /// entry's persist id on all 80 `igSymbol2d` edges across the four sheet
    /// fixtures (`psm_space_map_181_edges_match_igsymbol_jsite_ref`, and
    /// `docs/analysis/2026-08-27-the-spacemap-is-an-incoming-reference-index.md`).
    pub jsite_ref: u32,
    /// Style id the placement names for its body's line work, read as
    /// the u32 at payload `+25` — a fixed offset, unlike the tag-relative
    /// `jsite_ref`, and present in the 113-byte minimum payload.
    ///
    /// Level: corpus + screen. All 109 placements across the five
    /// `test-file` fixtures resolve this id to a `JStyleSimpleLine` in
    /// the **root** document's `StyleCluster`, and the palette that falls
    /// out is `SmartPlant`'s item-class colouring: equipment `#800000`,
    /// piping `#808000`, instruments `#008000`, annotation `#000000`,
    /// electric trace `#0000FF`. A `SmartPlant` screenshot of `DWG-0201`
    /// shows exactly those colours on exactly those placements — including
    /// a vessel whose own `.sym` styles its strokes black, which is what
    /// settles that the placement's style, not the library's, is the one
    /// the body draws with. See
    /// `docs/analysis/2026-08-24-placement-names-the-body-style.md`.
    pub style_ref: u32,
    /// Row-major 2×2 placement matrix, read from just past
    /// `IGSYMBOL2D_MATRIX_TAG`. An un-rotated, un-mirrored symbol reads
    /// `[1.0, 0.0, 0.0, 1.0]`; a negative determinant means the placement
    /// is mirrored, not merely turned.
    pub transform: [f64; 4],
    /// Insertion point in sheet units, the translation column of the same
    /// matrix.
    pub insertion: (f64, f64),
    /// Persist id, **inside the definition cache storage**, of the `JSheet`
    /// that holds this placement's symbol body: the payload's second-to-last
    /// `u32`. The cache is the `LdcSite` storage [`Self::definition_site_ref`]
    /// names; the sheet's tag-183 space-map edge leads to the layer manager
    /// whose layers carry the body, in symbol-local coordinates. 107/107
    /// placements across the four sheet fixtures resolve, and where the body
    /// has circles or arcs they equal the placed `.sym`'s to a nanometre
    /// (`docs/analysis/2026-09-07-placement-tail-names-the-cached-definition.md`).
    pub definition_sheet_ref: u32,
    /// Id of the top-level `LdcSite` storage (`JSite<id>`) that is the
    /// definition cache holding [`Self::definition_sheet_ref`]: the payload's
    /// last `u32`. Two caches exist per drawing -- `PSMroots` names them
    /// `Server Document` (static definitions) and `Imagineer Document`
    /// (parametric instances) -- and this word says which.
    pub definition_site_ref: u32,
}

/// Decode every PSM-encoded `igSymbol2d` record.
///
/// Validation:
/// 1. `type_code == 0x00CE`;
/// 2. `bytes_to_follow ∈ [113, 200]`;
/// 3. `IGSYMBOL2D_MATRIX_TAG` occurs within the first
///    `IGSYMBOL2D_TAG_SEARCH_END` payload bytes;
/// 4. the 6 doubles following it are finite + in domain.
///
/// Thin wrapper over [`IgSymbol2dDecoder`]'s shared
/// [`PsmRecordDecoder::scan`].
pub fn decode_igsymbols(data: &[u8]) -> Vec<SheetIgSymbol2dDecoded> {
    IgSymbol2dDecoder.scan(data)
}

/// Try to decode a single PSM `igSymbol2d` record starting at
/// `offset`. Returns `None` on validation failure.
///
/// Thin wrapper over [`IgSymbol2dDecoder::decode_at`].
pub fn decode_igsymbol_at(data: &[u8], offset: usize) -> Option<SheetIgSymbol2dDecoded> {
    IgSymbol2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igSymbol2d` family (PSM type
/// `0x00CE`). Validation rules are documented on [`decode_igsymbols`].
pub struct IgSymbol2dDecoder;

impl PsmRecordDecoder for IgSymbol2dDecoder {
    type Record = SheetIgSymbol2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGSYMBOL2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGSYMBOL2D_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgSymbol2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGSYMBOL2D {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        if !(IGSYMBOL2D_MIN_PAYLOAD_LEN..=IGSYMBOL2D_MAX_PAYLOAD_LEN).contains(&btf) {
            return None;
        }
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igsymbol_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgSymbol2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igSymbol2d` (everything
/// after the shared PSM envelope and size window pre-check).
fn decode_igsymbol_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgSymbol2dDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sheet_layer_ref = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let style_ref = u32::from_le_bytes([
        payload[IGSYMBOL2D_STYLE_REF_OFFSET],
        payload[IGSYMBOL2D_STYLE_REF_OFFSET + 1],
        payload[IGSYMBOL2D_STYLE_REF_OFFSET + 2],
        payload[IGSYMBOL2D_STYLE_REF_OFFSET + 3],
    ]);

    let search_end = IGSYMBOL2D_TAG_SEARCH_END.min(payload.len());
    let tag_at = payload
        .get(..search_end)?
        .windows(IGSYMBOL2D_MATRIX_TAG.len())
        .position(|w| w == IGSYMBOL2D_MATRIX_TAG)?;
    let matrix_at = tag_at + IGSYMBOL2D_MATRIX_TAG.len();

    // The u32 immediately before the matrix tag is the `JSite<id>`
    // storage id of the placed symbol (Phase 35-C, 132/132 records; the
    // PSMspacemap's tag-181 incoming edge agrees, 80/80).
    let jsite_at = tag_at.checked_sub(4)?;
    let jsite_ref = u32::from_le_bytes([
        payload[jsite_at],
        payload[jsite_at + 1],
        payload[jsite_at + 2],
        payload[jsite_at + 3],
    ]);

    // 4 doubles of placement matrix, then the translation pair.
    let mut doubles = [0f64; 6];
    for (i, slot) in doubles.iter_mut().enumerate() {
        let pos = matrix_at + i * 8;
        let chunk = payload.get(pos..pos + 8)?;
        *slot = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
    }
    if !doubles.iter().all(|x| x.is_finite()) {
        return None;
    }
    if doubles
        .iter()
        .any(|x| x.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT)
    {
        return None;
    }

    // The tail after the six doubles: an `f64 1.0`, a flag word, then
    // `u32 has_membassy`, `u32 0`, an optional `(membassy oid, 0)` pair when
    // that flag is set, and finally the two words that name the body --
    // `(definition JSheet oid, LdcSite id)`. Reading them from the end is
    // what makes the optional pair irrelevant. Payloads shorter than the
    // matrix plus this tail cannot occur under the 113-byte floor above, but
    // a synthetic one reads as "no definition" rather than as garbage.
    let tail_end = payload.len();
    let (definition_sheet_ref, definition_site_ref) = if tail_end >= matrix_at + 48 + 8 {
        (
            u32_le(payload, tail_end - 8).unwrap_or_default(),
            u32_le(payload, tail_end - 4).unwrap_or_default(),
        )
    } else {
        (0, 0)
    };

    Some(SheetIgSymbol2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        jsite_ref,
        style_ref,
        transform: [doubles[0], doubles[1], doubles[2], doubles[3]],
        insertion: (doubles[4], doubles[5]),
        definition_sheet_ref,
        definition_site_ref,
    })
}

/// PSM type code for `JSheet Object` (`docext.dex`): a sheet, which in a
/// definition cache storage is one symbol body.
pub const PSM_TYPE_CODE_JSHEET: u16 = 0x0114;

/// The oid of every `JSheet` record in a record-chain stream, in on-disk
/// order. No payload is read beyond the oid: the sheet's layers are reached
/// through its space-map edge to a `JSheetLayerManager`, not through its
/// own bytes.
pub fn jsheet_oids(data: &[u8]) -> Vec<u32> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| {
            let header = parse_psm_header(data, at)?;
            (header.type_code == PSM_TYPE_CODE_JSHEET)
                .then(|| u32_le(data, header.body_start))
                .flatten()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Phase 15 Slice C: PSM-encoded DependencyObject / GraphicPersist decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `0x00FA` `DependencyObject` record.
///
/// This DTO intentionally exposes only the stable header fields proven
/// by fixture byte dumps. The variable tail is retained as raw bytes
/// because candidate child-OID extraction is still an audit-layer
/// hypothesis, not a stable schema contract.
///
/// The tail does have a known shape: it ends in a self-describing block,
/// `u16 type` + `u16 selector` + `u16 len` followed by exactly `len` bytes,
/// which two unrelated drawings agree on bucket for bucket. See
/// `docs/analysis/2026-08-04-graphicgroup-tail-property-block.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetDependencyObjectDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_DEPENDENCY_OBJECT`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Graphic group object identifier.
    pub oid: u32,
    /// Parent reference. Current fixtures consistently use `6`
    /// (`PID_Page`), so the first decoder version validates it.
    pub parent_ref: u32,
    /// Small kind/count-like word at payload offsets 14..15.
    pub group_kind_word: u16,
    /// Sub-type / version-like discriminator at payload offsets 16..17.
    pub sub_type_word: u16,
    /// Raw variable tail from payload offset 18 onward.
    pub raw_reference_payload: Vec<u8>,
}

/// Decode every conservative PSM `0x00FA` `DependencyObject` record.
///
/// Validation:
/// 1. `type_code == 0x00FA` and type flags are zero;
/// 2. `bytes_to_follow` is even and in `[44, 512]`;
/// 3. payload exists, `oid != 0`, `parent_ref == 6`;
/// 4. payload bytes 8..13 are zero in the current fixture family;
/// 5. `group_kind_word` is a small non-zero discriminator.
pub fn decode_dependency_objects(data: &[u8]) -> Vec<SheetDependencyObjectDecoded> {
    DependencyObjectDecoder.scan(data)
}

/// Try to decode one conservative PSM `0x00FA` `DependencyObject` record at
/// `offset`. Returns `None` on validation failure.
///
/// Thin wrapper over [`DependencyObjectDecoder::decode_at`].
pub fn decode_dependency_object_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetDependencyObjectDecoded> {
    DependencyObjectDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the audit-only `DependencyObject` /
/// `GraphicPersist` family (PSM type `0x00FA`). Validation rules are
/// documented on [`decode_dependency_objects`].
pub struct DependencyObjectDecoder;

impl PsmRecordDecoder for DependencyObjectDecoder {
    type Record = SheetDependencyObjectDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_DEPENDENCY_OBJECT
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetDependencyObjectDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_DEPENDENCY_OBJECT {
            return None;
        }
        if header.type_flags != 0 {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        if !(DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN..=DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN).contains(&btf) {
            return None;
        }
        if !btf.is_multiple_of(2) {
            return None;
        }
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_dependency_object_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetDependencyObjectDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `DependencyObject` (fixture
/// invariants: non-zero `oid`, `parent_ref == 6` `PID_Page`, zeroed
/// bytes 8..13, small non-zero kind word).
fn decode_dependency_object_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetDependencyObjectDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;

    let payload = data.get(header.body_start..payload_end)?;
    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    if oid == 0 {
        return None;
    }
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    if parent_ref != 6 {
        return None;
    }
    if payload.get(8..14)? != [0u8; 6].as_slice() {
        return None;
    }
    let group_kind_word = u16::from_le_bytes([payload[14], payload[15]]);
    if group_kind_word == 0 || group_kind_word > 16 {
        return None;
    }
    let sub_type_word = u16::from_le_bytes([payload[16], payload[17]]);
    let raw_reference_payload = payload.get(18..)?.to_vec();

    Some(SheetDependencyObjectDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        group_kind_word,
        sub_type_word,
        raw_reference_payload,
    })
}

// ---------------------------------------------------------------------------
// Phase 16 Slice D: PSM-encoded `JStyleOverride` decoder (PSM type `0x0030`)
//
// **Re-identification of Phase 14 §6.1 future-slice.**
//
// The PSM type code `0x0030` does **not** map to IGDS `GArc2d`. IDA
// reverse engineering (see `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`)
// proves that:
//
// 1. The PSM lookup table at `radsrvitem.dll!dword_5667B068[48]` resolves
//    `type_code == 0x0030` to CLSID
//    `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`.
// 2. The RAD framework CLSID registry in `JUTIL.dll` (file offset
//    `0x35680`) maps that CLSID to `style.dll` class **"JSL Override
//    Style"** (RAD friendly name).
// 3. The implementing C++ class name is **`JStyleOverride`** (from
//    `style.dll` RTTI), with inheritance chain `JStyleOverride →
//    JStyleR2d → JStyleBase`.
// 4. The on-disk serialization for the SmartPlant fixture set is the
//    **Version 3** path (`style.dll!sub_1000F030`), which writes exactly
//    13 `IOContext::DoIO` calls totalling **64 bytes**, matching the
//    fixture PSM payload exactly.
//
// Authoritative Version-3 byte layout:
//
// ```text
// disk +0..3   u32 (host this+22)
// disk +4..7   u32 (host this+24)
// disk +8..11  u32 (host this+25)
// disk +12..15 u32 (host this+38)
// disk +16..23 f64 (host this+26)
// disk +24..31 f64 (host this+28) — `rotation_angle` candidate
// disk +32..39 f64 (host this+30)
// disk +40..47 f64 (host this+34)
// disk +48..51 u32 (host this+32)
// disk +52..55 u32 (host this+47)
// disk +56..59 u32 (host this+48)
// disk +60..61 u16 (host this+36)
// disk +62..63 u16 (host byte+146)
// ```
//
// Phase 17 removed the historical Phase 14 `PrimitiveArc` compatibility
// decoder. PSM type `0x0030` is now represented solely by this
// `JStyleOverride` decoder on the parser surface.

/// Inner PSM payload length emitted by `JStyleOverride`
/// `IJPersistImp::Save` Version-3 path (`style.dll!sub_1000F030`).
/// Fixture PSM records always have `bytes_to_follow >= 64`; any
/// surplus belongs to the optional attribute / linkage tail
/// (`bytes_to_follow - 64`), which is *not* part of the `JStyleOverride`
/// payload itself.
pub const JSTYLE_OVERRIDE_PAYLOAD_LEN: usize = 64;
/// Minimum `bytes_to_follow` a `JStyleOverride` record can ship with
/// (= the 64-byte Version-3 payload alone). Larger values are valid
/// (fixture commonly ships 128 / 145 / 224 / 384 etc.) — the surplus
/// is the optional attribute tail.
pub const JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW: u32 = 64;
/// Upper bound on `bytes_to_follow` — guards against header-noise
/// hits. Fixture maximum observed is 384.
pub const JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW: u32 = 4096;
/// Coordinate-style f64 fields (`+16..23`, `+24..31`, `+32..39`,
/// `+40..47`) must satisfy `|x| <= JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT`
/// to discard adversarial / out-of-range noise. The fixture-observed
/// maximum across all 4 doubles is ~10 (rotation angle `2π`), but the
/// limit is set generously to avoid rejecting future records that
/// might encode larger magnitudes (e.g. world-space coordinates).
pub const JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT: f64 = 1.0e6;

/// One decoded PSM `0x0030` `JStyleOverride` record (RAD `style.dll`
/// CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`,
/// IDA-confirmed Version-3 IO path
/// `style.dll!sub_1000F030`).
///
/// All 13 fields written by the Save / Load Version-3 path are
/// exposed verbatim (4×u32 prefix + 4×f64 mid + 3×u32 + 2×u16). The
/// surplus attribute tail (`bytes_to_follow > 64`) is retained as
/// `raw_attribute_tail` for audit; its internal layout (plant tag,
/// linkage references, `1.0` markers, etc.) is documented as
/// hypothesis in
/// `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11 but is
/// not part of the stable contract.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetJStyleOverrideDecoded {
    /// Byte range covering the full PSM record (header + payload +
    /// any attribute tail captured by `bytes_to_follow`).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always `0x0030`.
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header. `>= 64`. Values above
    /// 64 indicate an optional attribute tail.
    pub bytes_to_follow: u32,
    /// `oid` from the PSM header.
    pub oid: u32,
    /// Version-3 disk field at payload `+0..3` (host `this+22`).
    pub field_a_u32: u32,
    /// Version-3 disk field at payload `+4..7` (host `this+24`).
    pub field_b_u32: u32,
    /// Version-3 disk field at payload `+8..11` (host `this+25`).
    pub field_c_u32: u32,
    /// Version-3 disk field at payload `+12..15` (host `this+38`).
    pub field_d_u32: u32,
    /// Version-3 disk field at payload `+16..23` (host `this+26`),
    /// f64.
    pub field_1_f64: f64,
    /// Version-3 disk field at payload `+24..31` (host `this+28`),
    /// f64. **Rotation-angle candidate** — cross-fixture probe
    /// shows values cluster around `{0, π/2, 3π/2, 2π}`.
    pub field_2_f64: f64,
    /// Version-3 disk field at payload `+32..39` (host `this+30`),
    /// f64.
    pub field_3_f64: f64,
    /// Version-3 disk field at payload `+40..47` (host `this+34`),
    /// f64.
    pub field_4_f64: f64,
    /// Version-3 disk field at payload `+48..51` (host `this+32`).
    pub field_e_u32: u32,
    /// Version-3 disk field at payload `+52..55` (host `this+47`).
    pub field_f_u32: u32,
    /// Version-3 disk field at payload `+56..59` (host `this+48`).
    pub field_g_u32: u32,
    /// Version-3 disk field at payload `+60..61` (host `this+36`).
    pub field_h_u16: u16,
    /// Version-3 disk field at payload `+62..63` (host byte 146).
    pub field_i_u16: u16,
    /// Raw attribute tail bytes (`bytes_to_follow - 64`). Retained
    /// for audit only; internal layout is hypothesis pending more
    /// fixture coverage.
    pub raw_attribute_tail: Vec<u8>,
}

/// Decode every conservative PSM `0x0030` `JStyleOverride` record in
/// a `Sheet*` stream's bytes (RAD `JStyleOverride`
/// `IJPersistImp::Save` Version-3 path).
///
/// Walks every byte offset; at each offset, runs
/// [`decode_jstyle_override_at`] and accepts any record that passes
/// header + payload validation. Adversarial input is panic-safe via
/// the per-offset `Option` boundary and bounded `Vec` allocations.
pub fn decode_jstyle_overrides(data: &[u8]) -> Vec<SheetJStyleOverrideDecoded> {
    JStyleOverrideDecoder.scan(data)
}

/// [`PsmRecordDecoder`] adapter for the RAD `JStyleOverride` family
/// (PSM type `0x0030`, `style.dll` Version-3 IO — **not** an arc; see
/// the Phase 16 reclassification note on
/// [`PSM_TYPE_CODE_JSTYLE_OVERRIDE`]).
///
/// Like `GLine2d`, this family uses the extended 18-byte
/// [`PSM_RECORD_HEADER_LEN`] header (`oid` at header bytes 6..10) and
/// carries a variable attribute tail inside `bytes_to_follow`; both
/// deviations live on this adapter.
pub struct JStyleOverrideDecoder;

impl PsmRecordDecoder for JStyleOverrideDecoder {
    type Record = SheetJStyleOverrideDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_JSTYLE_OVERRIDE
    }

    fn min_record_len(&self) -> usize {
        PSM_RECORD_HEADER_LEN + JSTYLE_OVERRIDE_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetJStyleOverrideDecoded> {
        let envelope = parse_psm_header(data, offset)?;
        if envelope.type_code != PSM_TYPE_CODE_JSTYLE_OVERRIDE {
            return None;
        }
        if envelope.type_flags != 0 {
            return None;
        }
        if !(JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW..=JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW)
            .contains(&envelope.bytes_to_follow)
        {
            return None;
        }
        let btf_usize = envelope.bytes_to_follow as usize;
        let record_end = offset.checked_add(PSM_ENVELOPE_LEN + btf_usize)?;
        if record_end > data.len() {
            return None;
        }
        let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
        let payload_end = header_end.checked_add(JSTYLE_OVERRIDE_PAYLOAD_LEN)?;
        if payload_end > data.len() {
            return None;
        }
        decode_jstyle_override_payload(data, offset, &envelope, header_end, payload_end, record_end)
    }

    fn advance_of(&self, record: &SheetJStyleOverrideDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Try to decode one PSM `0x0030` `JStyleOverride` record at `offset`.
///
/// Validation (only fields with IDA + cross-fixture probe evidence):
///
/// 1. PSM 14-bit type code == `0x0030` and type flags are zero.
/// 2. `bytes_to_follow` ∈ `[JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW,
///    JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW]`.
/// 3. The record body fits in `data`.
/// 4. All four payload f64 fields are finite and within
///    `|x| <= JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT`.
///
/// This authoritative decoder does **not** apply the historical
/// `axis_a.y ≈ 0` constraint from the removed Phase 14 `PrimitiveArc`
/// compatibility path, which rejected ~51% of real `JStyleOverride`
/// records.
///
/// Thin wrapper over [`JStyleOverrideDecoder::decode_at`].
pub fn decode_jstyle_override_at(data: &[u8], offset: usize) -> Option<SheetJStyleOverrideDecoded> {
    JStyleOverrideDecoder.decode_at(data, offset)
}

/// Family-specific payload extraction for `JStyleOverride` (the
/// 18-byte extended header's `oid`, the 64-byte Version-3 field
/// block, and the raw attribute tail).
fn decode_jstyle_override_payload(
    data: &[u8],
    offset: usize,
    envelope: &PsmHeader,
    header_end: usize,
    payload_end: usize,
    record_end: usize,
) -> Option<SheetJStyleOverrideDecoded> {
    let type_code = envelope.type_code;
    let type_flags = envelope.type_flags;
    let bytes_to_follow = envelope.bytes_to_follow;

    let header = data.get(offset..header_end)?;
    let oid = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);

    let payload = data.get(header_end..payload_end)?;

    let field_a_u32 = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let field_b_u32 = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let field_c_u32 = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let field_d_u32 = u32::from_le_bytes([payload[12], payload[13], payload[14], payload[15]]);

    let field_1_f64 = f64::from_le_bytes([
        payload[16],
        payload[17],
        payload[18],
        payload[19],
        payload[20],
        payload[21],
        payload[22],
        payload[23],
    ]);
    let field_2_f64 = f64::from_le_bytes([
        payload[24],
        payload[25],
        payload[26],
        payload[27],
        payload[28],
        payload[29],
        payload[30],
        payload[31],
    ]);
    let field_3_f64 = f64::from_le_bytes([
        payload[32],
        payload[33],
        payload[34],
        payload[35],
        payload[36],
        payload[37],
        payload[38],
        payload[39],
    ]);
    let field_4_f64 = f64::from_le_bytes([
        payload[40],
        payload[41],
        payload[42],
        payload[43],
        payload[44],
        payload[45],
        payload[46],
        payload[47],
    ]);

    if !field_1_f64.is_finite()
        || !field_2_f64.is_finite()
        || !field_3_f64.is_finite()
        || !field_4_f64.is_finite()
    {
        return None;
    }
    if field_1_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_2_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_3_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
        || field_4_f64.abs() > JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT
    {
        return None;
    }

    let field_e_u32 = u32::from_le_bytes([payload[48], payload[49], payload[50], payload[51]]);
    let field_f_u32 = u32::from_le_bytes([payload[52], payload[53], payload[54], payload[55]]);
    let field_g_u32 = u32::from_le_bytes([payload[56], payload[57], payload[58], payload[59]]);
    let field_h_u16 = u16::from_le_bytes([payload[60], payload[61]]);
    let field_i_u16 = u16::from_le_bytes([payload[62], payload[63]]);

    let raw_attribute_tail = data.get(payload_end..record_end)?.to_vec();

    Some(SheetJStyleOverrideDecoded {
        byte_range: offset..record_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        field_a_u32,
        field_b_u32,
        field_c_u32,
        field_d_u32,
        field_1_f64,
        field_2_f64,
        field_3_f64,
        field_4_f64,
        field_e_u32,
        field_f_u32,
        field_g_u32,
        field_h_u16,
        field_i_u16,
        raw_attribute_tail,
    })
}

// ---------------------------------------------------------------------------
// Phase 18: PSM 0x0010 sub-record family — audit-only decoder
//
// PSM type code 0x0010 is the most prevalent (638 cross-fixture hits)
// yet-unexplained record type after the Phase 14-17 typed decoders. probe
// evidence (`examples/probe_psm_0x0010_shape.rs`) shows highly polymorphic
// payload shapes — sizes range from ~13 bytes to ≥99 bytes, with multiple
// sub-kinds inside (some carry IEEE 754 doubles, others carry leading
// discriminators such as `02 00 01 00`). They are referenced from
// JStyleOverride `+38..41` (`referenced_oid_a`) and `+56..59`
// (`referenced_oid_c`); see
// `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11.
//
// Until IDA reverse engineering confirms the class identity and the
// sub-kind discriminator, this decoder follows the Phase 15 DependencyObject
// audit-only template: stable 6-byte PSM header (`type_word + bytes_to_follow`,
// **not** the 18-byte IGDS header used by Phase 14 typed primitives) +
// raw payload + full provenance. NO sub-kind field naming, NO
// `PidGraphicKind` emission. Sub-kind decoders will land in a future phase.

/// PSM type code for the `0x0010` sub-record family (638 cross-fixture
/// hits, polymorphic payload).
///
/// Validated against four Sheet-bearing fixtures via
/// `examples/probe_psm_0x0010_shape.rs`. See
/// `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md` §6.3
/// for the initial categorization as "embedded sub-records / attribute
/// fragments inside other record types".
pub const PSM_TYPE_CODE_SUB_RECORD_0X0010: u16 = 0x0010;

/// Minimum `bytes_to_follow` accepted by the conservative 0x0010 audit
/// decoder. Cross-fixture probe baseline shows the smallest observed
/// payload is 13 bytes; the threshold is set to 8 to leave headroom
/// while still rejecting trivially-small noise hits.
pub const SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW: u32 = 8;

/// Upper bound on `bytes_to_follow` for 0x0010 records. Probe maximum
/// observed across the four-fixture set is ~99 bytes; the cap leaves
/// room for variant growth while rejecting wide-scan false positives
/// whose `bytes_to_follow` accidentally reads as a giant value.
pub const SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW: u32 = 100_000;

/// One decoded PSM `0x0010` sub-record (audit-only).
///
/// This DTO exposes only the stable 6-byte PSM header and the variable
/// payload as raw bytes. The payload structure is **highly polymorphic**
/// (size 13..99+, multiple sub-kinds with different discriminators), so
/// per-field naming is deferred until IDA reverse engineering confirms
/// the class identity and sub-kind layout. The decoder is intentionally
/// permissive: it admits every byte sequence whose header satisfies
/// `type_code == 0x0010` and `bytes_to_follow` fits within the stream.
/// Some hits may be embedded fragments inside larger parent records
/// rather than standalone 0x0010 records — the audit collection
/// preserves both, leaving sub-kind disambiguation to a future phase.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSubRecord0x0010Decoded {
    /// Byte range covering the full PSM record (6-byte header + payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_SUB_RECORD_0X0010`] (`0x0010`).
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header. Equals `raw_payload.len()`
    /// by construction.
    pub bytes_to_follow: u32,
    /// Raw payload bytes (length = `bytes_to_follow`). Sub-kind
    /// discrimination and per-field decoding are deferred to a
    /// future phase.
    pub raw_payload: Vec<u8>,
    /// Audit-only Phase 19 field: `payload[0..2]` as little-endian
    /// `u16`. `None` when `raw_payload.len() < 2`.
    ///
    /// This is **not** a named sub-kind discriminator — Phase 19 probe
    /// (`examples/probe_psm_0x0010_sub_kind.rs`) showed that while
    /// `leading_word == 0x0002` accounts for ~28% of records and many
    /// size buckets are mono-valued at this position, several large
    /// buckets (size 31, 70, 13, 16, 43) are heterogeneous at +0 and
    /// likely encode coordinates or OIDs in the leading bytes rather
    /// than a sub-kind tag. The field name therefore describes only
    /// the byte position, not any semantic role; promotion to a typed
    /// `sub_kind` field is deferred pending IDA confirmation.
    pub leading_word: Option<u16>,
}

/// Decode every PSM `0x0010` sub-record in a Sheet stream's bytes.
///
/// Walk every offset; at each offset, accept the 6-byte PSM header
/// when `type_code == 0x0010` and `bytes_to_follow` satisfies the
/// `[SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW,
/// SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW]` envelope. After accepting a
/// record, the scanner advances past it so non-overlapping back-to-back
/// records are decoded individually.
///
/// The decoder is **conservative** and panic-free: adversarial bytes
/// either fail validation and are skipped, or never decode at all.
pub fn decode_sub_records_0x0010(data: &[u8]) -> Vec<SheetSubRecord0x0010Decoded> {
    SubRecord0x0010Decoder.scan(data)
}

/// Try to decode a single PSM `0x0010` sub-record starting at `offset`
/// in `data`. Returns `None` when any validation rule from
/// [`decode_sub_records_0x0010`] fails. Bounds-checked: passing
/// `offset >= data.len()` or a truncated tail returns `None`.
///
/// Thin wrapper over [`SubRecord0x0010Decoder::decode_at`].
pub fn decode_sub_record_0x0010_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetSubRecord0x0010Decoded> {
    SubRecord0x0010Decoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the audit-only polymorphic
/// `0x0010` sub-record family. Validation rules are documented on
/// [`decode_sub_records_0x0010`].
pub struct SubRecord0x0010Decoder;

impl PsmRecordDecoder for SubRecord0x0010Decoder {
    type Record = SheetSubRecord0x0010Decoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_SUB_RECORD_0X0010
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN.saturating_add(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW as usize)
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetSubRecord0x0010Decoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_SUB_RECORD_0X0010 {
            return None;
        }
        if !(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
            .contains(&header.bytes_to_follow)
        {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        let raw_payload = data.get(header.body_start..payload_end)?.to_vec();
        let leading_word = raw_payload
            .get(0..2)
            .map(|slot| u16::from_le_bytes([slot[0], slot[1]]));
        Some(SheetSubRecord0x0010Decoded {
            byte_range: offset..payload_end,
            type_code: header.type_code,
            type_flags: header.type_flags,
            bytes_to_follow: header.bytes_to_follow,
            raw_payload,
            leading_word,
        })
    }

    fn advance_of(&self, record: &SheetSubRecord0x0010Decoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

// Phase 26: PSM 0x0010 attribute-fragment decoder (additive, audit-only)
//
// 2026-05-31 analysis proved 0x0010 records carry SmartPlant attribute
// text (instrument tags / line numbers / nominal sizes / drawing refs /
// annotation labels) as length-prefixed UTF-16LE after a fixed
// `marker(4) + aux(8)` prefix. See
// `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`. This decoder
// is strictly additive: the raw Phase 18 `decode_sub_records_0x0010`
// path is unchanged, and no `PidGraphicKind` is emitted.

/// One length-prefixed UTF-16LE string extracted from a PSM `0x0010`
/// attribute fragment (Phase 26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAttributeString {
    /// Offset (within the record payload) of the `u16` length word.
    pub len_offset: usize,
    /// Character count from the `u16` length prefix.
    pub char_count: u16,
    /// Decoded UTF-16LE text. Records whose tail is not clean UTF-16LE
    /// are rejected wholesale, so this is always valid Unicode.
    pub text: String,
}

/// A PSM `0x0010` record decoded as a `SmartPlant` **attribute fragment**
/// (Phase 26).
///
/// Carries engineering attribute text — instrument tags, line numbers,
/// nominal sizes, drawing references, annotation labels — as one or more
/// length-prefixed UTF-16LE strings after a fixed `marker(4) + aux(8)`
/// prefix. This is an **additive, audit-only** view that coexists with
/// the raw [`SheetSubRecord0x0010Decoded`] from Phase 18 (unchanged) and
/// emits no geometry kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetAttributeFragmentDecoded {
    /// Byte range covering the full PSM record (6-byte header + payload).
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_SUB_RECORD_0X0010`] (`0x0010`).
    pub type_code: u16,
    /// `payload[0..4]` as little-endian `u32`. A repeating **type
    /// marker** (frequently `0x00010002`), **not** a unique object id.
    pub marker: u32,
    /// `payload[4..12]` raw. 8-byte header/aux whose per-field semantics
    /// remain pending IDA confirmation; preserved verbatim for audit.
    pub aux: [u8; 8],
    /// Length-prefixed UTF-16LE strings parsed from `payload[12..]`.
    /// Non-empty by construction.
    pub strings: Vec<DecodedAttributeString>,
}

/// Payload offset where the first length-prefixed string begins
/// (`marker(4) + aux(8)`).
pub const ATTRIBUTE_FRAGMENT_STRING_START: usize = 12;

/// Upper bound on a single string's `u16` character count, guarding
/// against a bogus length prefix.
pub const ATTRIBUTE_FRAGMENT_MAX_CHAR_COUNT: u16 = 4096;

/// Decode every PSM `0x0010` **attribute fragment** in a Sheet stream's
/// bytes (Phase 26, additive + audit-only).
///
/// Uses the same header envelope as [`decode_sub_records_0x0010`], then
/// parses `marker(4) + aux(8) + [u16 len + UTF-16LE]*` and keeps a record
/// only when at least one clean, non-empty UTF-16LE string is recovered.
/// Records whose tail is not clean length-prefixed UTF-16LE are skipped
/// (still available via the raw Phase 18 decoder). Panic-free.
pub fn decode_attribute_fragments(data: &[u8]) -> Vec<SheetAttributeFragmentDecoded> {
    AttributeFragmentDecoder.scan(data)
}

/// Try to decode a single PSM `0x0010` attribute fragment at `offset`.
/// Returns `None` when the header fails validation, the payload is too
/// short for `marker(4)+aux(8)+len`, or no clean UTF-16LE string is
/// found. Bounds-checked and panic-free.
///
/// Thin wrapper over [`AttributeFragmentDecoder::decode_at`].
pub fn decode_attribute_fragment_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetAttributeFragmentDecoded> {
    AttributeFragmentDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the Phase 26 attribute-fragment
/// **view** of the `0x0010` family (audit-only, additive): same PSM
/// envelope as [`SubRecord0x0010Decoder`], but the payload must parse
/// as `marker(4) + aux(8) + [u16 len + UTF-16LE]*` with at least one
/// clean non-empty string. Two adapters deliberately share one type
/// code here — the raw Phase 18 collection and this typed view coexist
/// by design.
pub struct AttributeFragmentDecoder;

impl PsmRecordDecoder for AttributeFragmentDecoder {
    type Record = SheetAttributeFragmentDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_SUB_RECORD_0X0010
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN.saturating_add(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW as usize)
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetAttributeFragmentDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_SUB_RECORD_0X0010 {
            return None;
        }
        if !(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
            .contains(&header.bytes_to_follow)
        {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_attribute_fragment_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetAttributeFragmentDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for attribute fragments (the
/// `marker + aux + length-prefixed UTF-16LE strings` grammar).
fn decode_attribute_fragment_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetAttributeFragmentDecoded> {
    let type_code = header.type_code;

    let payload = data.get(header.body_start..payload_end)?;
    if payload.len() < ATTRIBUTE_FRAGMENT_STRING_START + 2 {
        return None;
    }
    let marker = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let mut aux = [0u8; 8];
    aux.copy_from_slice(payload.get(4..12)?);

    let mut strings = Vec::new();
    let mut pos = ATTRIBUTE_FRAGMENT_STRING_START;
    while pos + 2 <= payload.len() {
        let char_count = u16::from_le_bytes([payload[pos], payload[pos + 1]]);
        if char_count == 0 || char_count > ATTRIBUTE_FRAGMENT_MAX_CHAR_COUNT {
            break;
        }
        let text_start = pos + 2;
        let byte_len = (char_count as usize).checked_mul(2)?;
        let text_end = text_start.checked_add(byte_len)?;
        if text_end > payload.len() {
            break;
        }
        let Some(raw) = payload.get(text_start..text_end) else {
            break;
        };
        let Some(text) = decode_attribute_utf16le(raw) else {
            break;
        };
        if text.chars().all(char::is_whitespace) {
            break;
        }
        strings.push(DecodedAttributeString {
            len_offset: pos,
            char_count,
            text,
        });
        pos = text_end;
    }

    if strings.is_empty() {
        return None;
    }
    Some(SheetAttributeFragmentDecoded {
        byte_range: offset..payload_end,
        type_code,
        marker,
        aux,
        strings,
    })
}

/// Decode `bytes` as UTF-16LE, returning `None` on odd length, any
/// unpaired surrogate, or any control character other than space/tab.
fn decode_attribute_utf16le(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut out = String::new();
    for unit in char::decode_utf16(units) {
        let ch = unit.ok()?;
        if ch.is_control() && ch != ' ' && ch != '\t' {
            return None;
        }
        out.push(ch);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Phase 34-D: PSM `igBoundary2d` (type `0x0013`) — typed audit-only decoder
//
// Grammar pinned by `examples/probe_0013_igboundary2d_grammar.rs`
// (2026-07-07) on all 20 cross-fixture records (`DWG-0202` ×5,
// `publish-DWG-0202` ×5, `工艺管道及仪表流程-1` ×10; every hit in primary
// top-level `/Sheet6`, all `bytes_to_follow = 172`):
//
// - the record reuses the 18-byte IGDS prefix (`oid`, `parent_ref`,
//   `remaining_header == 12`, `sub_type_word == 0x0010`, `index`);
// - a 10-byte sub-header follows: `u32 == 1`, `u32 segment_count`,
//   two small bytes (`2`, `1` across all fixtures);
// - `segment_count` groups of `0x67 tag + 4×f64` (33 bytes each)
//   carry per-segment `(start, end)` coordinates; the recurring
//   `0x67` byte the Phase 34-C closeout flagged is a fixed
//   per-segment tag, not an interleaved vertex-array stride;
// - an anchor point (2×f64, inside the segment bbox on 20/20
//   records), a `u8` flag, and `u32 member_count == segment_count`
//   follow;
// - the payload ends with `member_count` 8-byte references
//   (`u32 member_oid + u16 class_word + u16 sub_word`); on 60/60
//   fixture members the `member_oid` resolves to a real `0x0018`
//   `igLine2d` record in the same stream whose `(start, end)`
//   equals `segments[i]` in forward order, and the class word is
//   always `0x00CB`.
//
// Total payload length is exactly `49 + 41 × segment_count`.
//
// **Why audit-only:** the member-reference table proves
// `igBoundary2d` is an *association* record — its segment
// coordinates re-list geometry that the member `igLine2d` records
// already emit as normalized `Line` entities. Emitting the boundary
// as a `PidGraphicKind::Polyline` would double-count those bytes'
// geometry, so this decoder exposes fully-typed fields (closure is
// available via [`SheetIgBoundary2dDecoded::is_closed_loop`]) but
// deliberately produces no normalized geometry entity.

/// PSM type code for the standard Intergraph Sigma `igBoundary2d`
/// record (`0x13 = 19`, the IGDS class tag used directly as the PSM
/// type code, matching the Slice J discovery for `igLine2d`).
///
/// Cross-fixture histogram: 20 top-level hits in 3 fixtures, all in
/// primary `/Sheet6` with canonical `igLine2d` neighbours. See
/// `docs/analysis/2026-06-30-phase34-0013-003d-evidence-closeout.md`.
pub const PSM_TYPE_CODE_IGBOUNDARY2D: u16 = 0x0013;

/// Per-segment tag byte that opens every `igBoundary2d` segment
/// group (`0x67`). Fixture dumps show `segment_count` groups of
/// `tag + 4×f64` starting at payload offset 28.
pub const IGBOUNDARY2D_SEGMENT_TAG: u8 = 0x67;

/// Upper bound accepted for `segment_count`. All fixture records
/// carry 3 segments; the cap leaves headroom for larger closed
/// boundaries while rejecting wide-scan false positives.
pub const IGBOUNDARY2D_MAX_SEGMENT_COUNT: u32 = 64;

/// Fixed payload byte overhead of an `igBoundary2d` record outside
/// its per-segment data: 18-byte IGDS prefix + 10-byte sub-header +
/// 16-byte anchor + 1-byte flag + 4-byte member count = 49 bytes.
/// Each segment adds 33 group bytes + an 8-byte member reference,
/// so `bytes_to_follow == 49 + 41 × segment_count`.
pub const IGBOUNDARY2D_FIXED_PAYLOAD_LEN: usize = 49;

/// Per-segment payload cost: a 33-byte `0x67 + 4×f64` group plus an
/// 8-byte trailer member reference.
pub const IGBOUNDARY2D_PER_SEGMENT_LEN: usize = 41;

/// One `(start, end)` segment decoded from an `igBoundary2d`
/// segment group (`0x67 tag + 4×f64`).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgBoundary2dSegment {
    /// Payload offset of the group's `0x67` tag byte.
    pub tag_offset: usize,
    /// Segment start `(x, y)` in normalized sheet coordinates.
    pub start: (f64, f64),
    /// Segment end `(x, y)` in normalized sheet coordinates.
    pub end: (f64, f64),
}

/// One 8-byte member reference from the `igBoundary2d` trailer:
/// `u32 member_oid + u16 class_word + u16 sub_word`.
///
/// On all 60 fixture members the `member_oid` resolves to a real
/// `0x0018 igLine2d` record in the same Sheet stream whose
/// `(start, end)` equals the same-index segment in forward order;
/// `class_word` is always `0x00CB` and `sub_word` is `13` for the
/// first member and `12` for the rest. The words are exposed
/// verbatim without semantic naming until a native reader confirms
/// their roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetIgBoundary2dMemberRef {
    /// Object identifier of the member record (fixture evidence:
    /// a `0x0018 igLine2d` in the same stream).
    pub member_oid: u32,
    /// Class-like word at member offset +4 (`0x00CB` across all
    /// fixture members).
    pub class_word: u16,
    /// Sub-code word at member offset +6 (`13` / `12` in fixtures).
    pub sub_word: u16,
}

/// One decoded PSM `igBoundary2d` record (type `0x0013`).
///
/// Fully-typed **audit-only** DTO: every payload byte is accounted
/// for (prefix, sub-header, segment groups, anchor, flag, member
/// references), but no normalized geometry entity is emitted
/// because the member references prove the segment coordinates
/// duplicate member `igLine2d` geometry (see module comment).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgBoundary2dDecoded {
    /// Byte range covering the full PSM record (6-byte header +
    /// payload) within the Sheet stream.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always
    /// [`PSM_TYPE_CODE_IGBOUNDARY2D`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word (0 across fixtures).
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header
    /// (`49 + 41 × segment_count` by validation).
    pub bytes_to_follow: u32,
    /// Object identifier from payload offset 0.
    pub oid: u32,
    /// Parent reference from payload offset 4 (varies per record;
    /// not validated).
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this boundary sits on (payload
    /// offset 8, the high half of the PSM envelope's `aux`).
    ///
    /// Grouping a storage's objects by this field reproduces every
    /// layer's own `+12` object tally exactly, and the vendor's
    /// `HGeomGetLayer` / `HGeomPutLayer` pair reaches the same edge
    /// through the graphic's own interface. Never validated: an
    /// `== <n>` rule here admits one layer and refuses the rest. See
    /// `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md`.
    pub sheet_layer_ref: u32,
    /// Sub-type word at payload offset 12. Always `0x0010`.
    pub sub_type_word: u16,
    /// Index-like word at payload offset 14 (21 / 24 in fixtures,
    /// constant per fixture family like the other IGDS decoders).
    pub index: u32,
    /// `u32` at payload offset 22: number of segment groups and
    /// trailer member references.
    pub segment_count: u32,
    /// Two sub-header bytes at payload offsets 26–27 (`[2, 1]`
    /// across all fixture records; exposed verbatim, not
    /// validated).
    pub sub_header_tail: [u8; 2],
    /// Decoded segment groups (`segment_count` entries).
    pub segments: Vec<SheetIgBoundary2dSegment>,
    /// Anchor point following the segment groups; inside the
    /// segment bounding box on 20/20 fixture records.
    pub anchor: (f64, f64),
    /// `u8` flag between the anchor and the member count (`1`
    /// across all fixture records; exposed verbatim, not
    /// validated).
    pub trailer_flag: u8,
    /// Trailer member references (`segment_count` entries).
    pub member_refs: Vec<SheetIgBoundary2dMemberRef>,
}

impl SheetIgBoundary2dDecoded {
    /// `true` when consecutive segments chain end-to-start and the
    /// last segment closes back onto the first start, each within
    /// `tolerance` per axis.
    ///
    /// Fixture records close within a few f64 ulps (exact-bit
    /// equality fails on 19/20 records, `1e-9` passes on 20/20), so
    /// callers should pass a small positive tolerance such as
    /// `1e-9` rather than `0.0`.
    pub fn is_closed_loop(&self, tolerance: f64) -> bool {
        let close = |a: (f64, f64), b: (f64, f64)| {
            (a.0 - b.0).abs() <= tolerance && (a.1 - b.1).abs() <= tolerance
        };
        if self.segments.len() < 2 {
            return false;
        }
        let chained = self
            .segments
            .windows(2)
            .all(|w| close(w[0].end, w[1].start));
        chained
            && close(
                self.segments[self.segments.len() - 1].end,
                self.segments[0].start,
            )
    }
}

/// Decode every PSM `igBoundary2d` record in a Sheet stream's bytes.
///
/// Validation rules (all must hold, otherwise the offset is skipped):
///
/// 1. `type_code == 0x0013`; type flags are zero;
/// 2. `sub_type_word == 0x0010` (payload offset 12);
/// 3. sub-header `u32` at payload offset 18 equals `1`;
/// 4. `segment_count` (payload offset 22) is in
///    `1..=IGBOUNDARY2D_MAX_SEGMENT_COUNT`;
/// 5. `bytes_to_follow == 49 + 41 × segment_count` exactly;
/// 6. every segment group starts with the `0x67` tag byte;
/// 7. all segment and anchor coordinates are finite and within the
///    `[-1e9, 1e9]` domain;
/// 8. trailer `member_count == segment_count`;
/// 9. not all segment vertices are identical (non-degenerate).
///
/// After accepting a record the scanner advances past it. Panic-free
/// and bounds-checked: adversarial bytes simply fail validation.
pub fn decode_igboundaries(data: &[u8]) -> Vec<SheetIgBoundary2dDecoded> {
    IgBoundary2dDecoder.scan(data)
}

/// Try to decode a single PSM `igBoundary2d` record starting at
/// `offset`. Returns `None` when any validation rule in
/// [`decode_igboundaries`] fails. Bounds-checked and panic-free.
///
/// Thin wrapper over [`IgBoundary2dDecoder::decode_at`].
pub fn decode_igboundary_at(data: &[u8], offset: usize) -> Option<SheetIgBoundary2dDecoded> {
    IgBoundary2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the typed audit-only
/// `igBoundary2d` association family (PSM type `0x0013`). Validation
/// rules are documented on [`decode_igboundaries`]; no geometry is
/// emitted for this family (member `igLine2d` records already carry
/// the segments).
pub struct IgBoundary2dDecoder;

impl PsmRecordDecoder for IgBoundary2dDecoder {
    type Record = SheetIgBoundary2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGBOUNDARY2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGBOUNDARY2D_FIXED_PAYLOAD_LEN + IGBOUNDARY2D_PER_SEGMENT_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgBoundary2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGBOUNDARY2D {
            return None;
        }
        if header.type_flags != 0 {
            return None;
        }
        let btf = header.bytes_to_follow as usize;
        let payload_end = header.body_start.checked_add(btf)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igboundary_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgBoundary2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igBoundary2d` (segment
/// grammar, anchor, and member-reference trailer).
fn decode_igboundary_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgBoundary2dDecoded> {
    let type_code = header.type_code;
    let type_flags = header.type_flags;
    let bytes_to_follow = header.bytes_to_follow;
    let btf = bytes_to_follow as usize;

    let payload = data.get(header.body_start..payload_end)?;

    let read_u32 = |pos: usize| -> Option<u32> {
        payload
            .get(pos..pos + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let read_f64 = |pos: usize| -> Option<f64> {
        payload
            .get(pos..pos + 8)
            .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    };

    let oid = read_u32(0)?;
    let parent_ref = read_u32(4)?;
    let sheet_layer_ref = read_u32(8)?;
    let sub_type_word = u16::from_le_bytes([*payload.get(12)?, *payload.get(13)?]);
    if sub_type_word != 0x0010 {
        return None;
    }
    let index = read_u32(14)?;
    if read_u32(18)? != 1 {
        return None;
    }
    let segment_count = read_u32(22)?;
    if !(1..=IGBOUNDARY2D_MAX_SEGMENT_COUNT).contains(&segment_count) {
        return None;
    }
    let n = segment_count as usize;
    let expected_len =
        IGBOUNDARY2D_FIXED_PAYLOAD_LEN.checked_add(IGBOUNDARY2D_PER_SEGMENT_LEN.checked_mul(n)?)?;
    if btf != expected_len {
        return None;
    }
    let sub_header_tail = [*payload.get(26)?, *payload.get(27)?];

    // Segment groups: `0x67 tag + 4×f64` each, starting at +28.
    let mut segments = Vec::with_capacity(n);
    let mut pos = 28usize;
    let mut all_same = true;
    let mut first_vertex: Option<(f64, f64)> = None;
    for _ in 0..n {
        if *payload.get(pos)? != IGBOUNDARY2D_SEGMENT_TAG {
            return None;
        }
        let sx = read_f64(pos + 1)?;
        let sy = read_f64(pos + 9)?;
        let ex = read_f64(pos + 17)?;
        let ey = read_f64(pos + 25)?;
        for v in [sx, sy, ex, ey] {
            if !v.is_finite() || v.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
                return None;
            }
        }
        let reference = *first_vertex.get_or_insert((sx, sy));
        for vertex in [(sx, sy), (ex, ey)] {
            if vertex != reference {
                all_same = false;
            }
        }
        segments.push(SheetIgBoundary2dSegment {
            tag_offset: pos,
            start: (sx, sy),
            end: (ex, ey),
        });
        pos += 33;
    }
    if all_same {
        return None;
    }

    // Anchor + flag + member count.
    let anchor_x = read_f64(pos)?;
    let anchor_y = read_f64(pos + 8)?;
    for v in [anchor_x, anchor_y] {
        if !v.is_finite() || v.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
            return None;
        }
    }
    let trailer_flag = *payload.get(pos + 16)?;
    let member_count = read_u32(pos + 17)?;
    if member_count != segment_count {
        return None;
    }

    // Trailer member references.
    let mut member_refs = Vec::with_capacity(n);
    let member_base = pos + 21;
    for i in 0..n {
        let base = member_base + i * 8;
        let member_oid = read_u32(base)?;
        let class_word = u16::from_le_bytes([*payload.get(base + 4)?, *payload.get(base + 5)?]);
        let sub_word = u16::from_le_bytes([*payload.get(base + 6)?, *payload.get(base + 7)?]);
        member_refs.push(SheetIgBoundary2dMemberRef {
            member_oid,
            class_word,
            sub_word,
        });
    }

    Some(SheetIgBoundary2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        index,
        segment_count,
        sub_header_tail,
        segments,
        anchor: (anchor_x, anchor_y),
        trailer_flag,
        member_refs,
    })
}

// ---------------------------------------------------------------------------
// PSM `igSmartFrame2d` (type `0x003D`) — typed sheet-frame decoder
//
// Named from the native reader rather than inferred. `radsrvitem.dll`
// `sub_564464D0` validates the record and classifies it:
//
//   if ( *(WORD *)a2 != 61 ) return E_INVALIDARG;        // 61 == 0x003D
//   v6 = *(DWORD *)(a2 + 32);
//   if ( (*(DWORD *)(a2 + 20) & 0x8000) == 0 )  "_Empty SmartFrame2d"     igOLENone
//   else if ( !(v6 & 0x40) && !(v6 & 0x20000) ) "_Embedded SmartFrame2d"  igOLEEmbedded
//   else if ( v6 & 0x20000 )                    "_Locally Linked ..."     igOLELinked
//   else                                        externally linked         igOLELinked
//
// It reads the type word at `a2 + 0`, which is where the PSM envelope puts
// it, so `a2` is the record start and the two flag words are at record
// `+20` / `+32` — payload `+14` / `+26`.
//
// The record is an OLE container frame, and a P&ID's sheet border is a
// linked OLE object, which is why the payload carries a page extent without
// being a page transform on its own: it is the framed object's own size.
// Across the six fixtures both flag words take only three distinct values
// each over all twelve records, and the classification agrees with the
// payload — the eleven embedded/linked frames carry ISO A-series pages,
// while the single locally-linked one sits in a nested JSite with a
// degenerate 1e-6 extent.
//
// See `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`.
// ---------------------------------------------------------------------------

/// PSM type code of an `igSmartFrame2d` record (`61`).
pub const PSM_TYPE_CODE_IGSMARTFRAME: u16 = 0x003D;

/// Payload offset of the content flag word (record `+20`).
const IGSMARTFRAME_CONTENT_FLAGS_AT: usize = 14;
/// Payload offset of the link flag word (record `+32`).
const IGSMARTFRAME_LINK_FLAGS_AT: usize = 26;
/// Payload offset of the framed extent's width, in metres.
const IGSMARTFRAME_WIDTH_AT: usize = 76;
/// Payload offset of the framed extent's height, in metres.
const IGSMARTFRAME_HEIGHT_AT: usize = 84;
/// Payload offset of the extent's aspect ratio (`height / width`).
const IGSMARTFRAME_ASPECT_AT: usize = 148;
/// Shortest payload that reaches every field above.
const IGSMARTFRAME_MIN_PAYLOAD: usize = IGSMARTFRAME_ASPECT_AT + 8;

/// `record + 20` bit: the frame holds something.
const IGSMARTFRAME_HAS_CONTENT: u32 = 0x8000;
/// `record + 32` bit: the framed object is linked rather than embedded.
const IGSMARTFRAME_LINKED: u32 = 0x40;
/// `record + 32` bit: the link is local to the drawing.
const IGSMARTFRAME_LOCALLY_LINKED: u32 = 0x2_0000;

/// Widest sheet a page extent can plausibly claim, in metres. ISO A0 is
/// 1.189m; this only has to reject the degenerate and the absurd.
const IGSMARTFRAME_MAX_PAGE_M: f64 = 5.0;
/// Narrowest sheet a page extent can plausibly claim, in metres.
const IGSMARTFRAME_MIN_PAGE_M: f64 = 0.05;

/// Which of the native reader's three states a frame is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSmartFrame2dState {
    /// `igOLENone` — the frame holds nothing.
    Empty,
    /// `igOLEEmbedded` — the framed object is stored in this drawing.
    Embedded,
    /// `igOLELinked` against an external document.
    Linked,
    /// `igOLELinked` against something inside this drawing.
    LocallyLinked,
}

/// One decoded PSM `igSmartFrame2d` record (PSM type `0x003D`).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgSmartFrame2dDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGSMARTFRAME`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM header.
    pub bytes_to_follow: u32,
    /// Object identifier.
    pub oid: u32,
    /// Parent reference.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this smart frame sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Content flag word at payload `+14`, verbatim.
    pub content_flags: u32,
    /// Link flag word at payload `+26`, verbatim.
    pub link_flags: u32,
    /// State the native reader's rules put the frame in.
    pub state: SheetSmartFrame2dState,
    /// Framed extent width in metres (payload `+76`).
    pub extent_width_m: f64,
    /// Framed extent height in metres (payload `+84`).
    pub extent_height_m: f64,
    /// Extent aspect ratio (payload `+148`); `1/sqrt(2)` on ISO A sheets.
    pub aspect_ratio: f64,
}

impl SheetIgSmartFrame2dDecoded {
    /// The extent as a sheet page, or `None` when this frame is not one.
    ///
    /// An empty frame has nothing to size, and a locally-linked frame is not
    /// a page — the one in the fixtures is a nested-site placeholder whose
    /// extent is a micrometre square. What remains is the drawing's border
    /// template, embedded or linked, whose extent is the sheet size.
    #[must_use]
    pub fn page_extent_m(&self) -> Option<(f64, f64)> {
        if !matches!(
            self.state,
            SheetSmartFrame2dState::Embedded | SheetSmartFrame2dState::Linked
        ) {
            return None;
        }
        let plausible = |v: f64| (IGSMARTFRAME_MIN_PAGE_M..=IGSMARTFRAME_MAX_PAGE_M).contains(&v);
        (plausible(self.extent_width_m) && plausible(self.extent_height_m))
            .then_some((self.extent_width_m, self.extent_height_m))
    }
}

/// Decode every PSM `igSmartFrame2d` record in a `Sheet*` stream.
///
/// Thin wrapper over [`IgSmartFrame2dDecoder`]'s shared
/// [`PsmRecordDecoder::scan`].
pub fn decode_smartframes(data: &[u8]) -> Vec<SheetIgSmartFrame2dDecoded> {
    IgSmartFrame2dDecoder.scan(data)
}

/// Try to decode a single PSM `igSmartFrame2d` record at `offset`.
pub fn decode_smartframe_at(data: &[u8], offset: usize) -> Option<SheetIgSmartFrame2dDecoded> {
    IgSmartFrame2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igSmartFrame2d` family.
pub struct IgSmartFrame2dDecoder;

impl PsmRecordDecoder for IgSmartFrame2dDecoder {
    type Record = SheetIgSmartFrame2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGSMARTFRAME
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGSMARTFRAME_MIN_PAYLOAD
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgSmartFrame2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGSMARTFRAME {
            return None;
        }
        let payload_len = usize::try_from(header.bytes_to_follow).ok()?;
        if payload_len < IGSMARTFRAME_MIN_PAYLOAD {
            return None;
        }
        let payload_end = header.body_start.checked_add(payload_len)?;
        let payload = data.get(header.body_start..payload_end)?;

        let u32_at = |at: usize| -> Option<u32> {
            payload
                .get(at..at + 4)
                .and_then(|s| <[u8; 4]>::try_from(s).ok())
                .map(u32::from_le_bytes)
        };
        let f64_at = |at: usize| -> Option<f64> {
            payload
                .get(at..at + 8)
                .and_then(|s| <[u8; 8]>::try_from(s).ok())
                .map(f64::from_le_bytes)
        };

        let content_flags = u32_at(IGSMARTFRAME_CONTENT_FLAGS_AT)?;
        let link_flags = u32_at(IGSMARTFRAME_LINK_FLAGS_AT)?;
        let extent_width_m = f64_at(IGSMARTFRAME_WIDTH_AT)?;
        let extent_height_m = f64_at(IGSMARTFRAME_HEIGHT_AT)?;
        let aspect_ratio = f64_at(IGSMARTFRAME_ASPECT_AT)?;
        // A frame with a non-finite or negative extent is not a frame this
        // reader understands, whatever the flags say.
        if ![extent_width_m, extent_height_m, aspect_ratio]
            .iter()
            .all(|v| v.is_finite())
            || extent_width_m < 0.0
            || extent_height_m < 0.0
        {
            return None;
        }

        let state = if content_flags & IGSMARTFRAME_HAS_CONTENT == 0 {
            SheetSmartFrame2dState::Empty
        } else if link_flags & IGSMARTFRAME_LINKED == 0
            && link_flags & IGSMARTFRAME_LOCALLY_LINKED == 0
        {
            SheetSmartFrame2dState::Embedded
        } else if link_flags & IGSMARTFRAME_LOCALLY_LINKED != 0 {
            SheetSmartFrame2dState::LocallyLinked
        } else {
            SheetSmartFrame2dState::Linked
        };

        Some(SheetIgSmartFrame2dDecoded {
            byte_range: offset..payload_end,
            type_code: header.type_code,
            type_flags: header.type_flags,
            bytes_to_follow: header.bytes_to_follow,
            oid: u32_at(0)?,
            parent_ref: u32_at(4)?,
            sheet_layer_ref: u32_at(8)?,
            content_flags,
            link_flags,
            state,
            extent_width_m,
            extent_height_m,
            aspect_ratio,
        })
    }

    fn advance_of(&self, record: &SheetIgSmartFrame2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

// ---------------------------------------------------------------------------
// 2026-08-27: the symbol-information / expression family
// ---------------------------------------------------------------------------
//
// Four record families that live only in a `JSite<N>/PSMcluster0` — never in
// a `Sheet*` stream — and form one parametric chain:
//
//   JSymbolInformation (0x00BD, symbol.dex) carries named variables
//     ├─ each variable's value is a Double Value Object (0x00C7, exprdex.dll)
//     ├─ those values are grouped by a Variables Object (0x00EA, exprdex.dll)
//     └─ a Standard Relation (0x006F, jengine.dll) feeds one into a JDim
//        (0x0115) through a JBExpression formula
//
// Class names come from the type-code table (`tools/psm_type_clsid.py`), and
// for `0x00BD` independently from the `PSMroots` name `SymbolInformation`.
// Evidence and the byte-level derivation:
// `docs/analysis/2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`.
//
// These are **audit-only** and emit no geometry. They are deliberately not
// registered in `model::sheet_families` and have no `GeometryEmitter`: that
// registry describes `Sheet*` families, and a no-op emitter there would claim
// these can appear on a sheet, which they never do.

/// PSM type code for `JSymbolInformation` (`symbol.dex`) — a symbol's
/// named-variable table. Also the class `PSMroots` calls
/// `SymbolInformation`.
pub const PSM_TYPE_CODE_JSYMBOL_INFORMATION: u16 = 0x00BD;

/// PSM type code for `Double Value Object` (`exprdex.dll`) — one named
/// double in the expression subsystem.
pub const PSM_TYPE_CODE_DOUBLE_VALUE: u16 = 0x00C7;

/// PSM type code for `Variables Object` (`exprdex.dll`) — the group that
/// owns a set of [`PSM_TYPE_CODE_DOUBLE_VALUE`] records.
pub const PSM_TYPE_CODE_VARIABLES: u16 = 0x00EA;

/// PSM type code for `Assoc subsystem Standard Relation implementation`
/// (`jengine.dll`) — binds operands with a `JBExpression` formula.
pub const PSM_TYPE_CODE_STANDARD_RELATION: u16 = 0x006F;

/// Fixed payload length of a [`PSM_TYPE_CODE_DOUBLE_VALUE`] record.
const DOUBLE_VALUE_PAYLOAD_LEN: usize = 24;

/// Payload offset where a [`PSM_TYPE_CODE_VARIABLES`] member list starts.
const VARIABLES_MEMBERS_AT: usize = 17;

/// Payload length of a [`PSM_TYPE_CODE_JSYMBOL_INFORMATION`] head, before
/// any variable table.
const SYMBOL_INFORMATION_HEAD_LEN: usize = 44;

/// `flags` value at payload `+14` that says a variable table follows the
/// head. Every other observed value means the record stops at the head.
const SYMBOL_INFORMATION_HAS_VARIABLES: u16 = 0x0010;

/// `DE264241-E929-11CE-A608-080036C61102` — `JBExpression object`, at
/// payload `+38` of every [`PSM_TYPE_CODE_STANDARD_RELATION`] record.
const JBEXPRESSION_CLSID: [u8; 16] = [
    0x41, 0x42, 0x26, 0xDE, 0x29, 0xE9, 0xCE, 0x11, 0xA6, 0x08, 0x08, 0x00, 0x36, 0xC6, 0x11, 0x02,
];

/// `D97A3FB0-1601-11CE-B7EE-08003601E53B` — `Double Value Object`, at
/// payload `+58`, the relation expression's value type.
const DOUBLE_VALUE_CLSID: [u8; 16] = [
    0xB0, 0x3F, 0x7A, 0xD9, 0x01, 0x16, 0xCE, 0x11, 0xB7, 0xEE, 0x08, 0x00, 0x36, 0x01, 0xE5, 0x3B,
];

/// `0145EEC0-1602-11CE-B7EE-08003601E53B` — the interface each relation
/// operand slot names after its oid.
const RELATION_OPERAND_IID: [u8; 16] = [
    0xC0, 0xEE, 0x45, 0x01, 0x02, 0x16, 0xCE, 0x11, 0xB7, 0xEE, 0x08, 0x00, 0x36, 0x01, 0xE5, 0x3B,
];

/// Payload offset of [`JBEXPRESSION_CLSID`] in a relation record.
const RELATION_EXPRESSION_CLSID_AT: usize = 38;
/// Payload offset of [`DOUBLE_VALUE_CLSID`] in a relation record.
const RELATION_VALUE_CLSID_AT: usize = 58;
/// Payload offset of the operand signature's length word.
const RELATION_SIGNATURE_AT: usize = 74;

fn u16_le(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_le(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_le(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// One decoded `0x00C7` `Double Value Object`: a single named double the
/// expression subsystem persists on its own so other objects can reference
/// it by oid.
#[derive(Debug, Clone, PartialEq)]
pub struct PsmDoubleValueDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Persist id of this value object.
    pub oid: u32,
    /// The owning [`PsmVariablesDecoded`] group.
    pub parent_ref: u32,
    /// The value itself, payload `+12`. The same double appears inline in
    /// the [`PsmSymbolInformationDecoded`] variable that names it.
    pub value: f64,
    /// Trailing pair at payload `+20` / `+22`; constant `1` / `15` across
    /// the corpus, meaning unknown, carried for audit.
    pub trailing: (u16, u16),
}

/// Decode every `0x00C7` `Double Value Object` record in `data`.
///
/// Validation: type code and zero type flags, `bytes_to_follow == 24`,
/// non-zero `oid` and `parent_ref`, zeroed word at payload `+8`, and a
/// finite value.
pub fn decode_double_values(data: &[u8]) -> Vec<PsmDoubleValueDecoded> {
    DoubleValueDecoder.scan(data)
}

/// Try to decode one `0x00C7` record at `offset`. Returns `None` on
/// validation failure.
pub fn decode_double_value_at(data: &[u8], offset: usize) -> Option<PsmDoubleValueDecoded> {
    DoubleValueDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x00C7` `Double Value Object`.
pub struct DoubleValueDecoder;

impl PsmRecordDecoder for DoubleValueDecoder {
    type Record = PsmDoubleValueDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_DOUBLE_VALUE
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + DOUBLE_VALUE_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<PsmDoubleValueDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_DOUBLE_VALUE || header.type_flags != 0 {
            return None;
        }
        if header.bytes_to_follow as usize != DOUBLE_VALUE_PAYLOAD_LEN {
            return None;
        }
        let body = header.body_start;
        let end = body.checked_add(DOUBLE_VALUE_PAYLOAD_LEN)?;
        if end > data.len() {
            return None;
        }
        let oid = u32_le(data, body)?;
        let parent_ref = u32_le(data, body + 4)?;
        if oid == 0 || parent_ref == 0 || u32_le(data, body + 8)? != 0 {
            return None;
        }
        let value = f64_le(data, body + 12)?;
        if !value.is_finite() {
            return None;
        }
        Some(PsmDoubleValueDecoded {
            byte_range: offset..end,
            oid,
            parent_ref,
            value,
            trailing: (u16_le(data, body + 20)?, u16_le(data, body + 22)?),
        })
    }

    fn advance_of(&self, record: &PsmDoubleValueDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// One decoded `0x00EA` `Variables Object`: the group that owns a set of
/// [`PsmDoubleValueDecoded`] records.
#[derive(Debug, Clone, PartialEq)]
pub struct PsmVariablesDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Persist id of this group.
    pub oid: u32,
    /// Persist ids of the member value objects, in on-disk order. Each
    /// member's own `parent_ref` names this record back.
    pub members: Vec<u32>,
    /// Trailing word after the member list; `321` across the corpus,
    /// meaning unknown, carried for audit.
    pub trailing: u32,
}

/// Decode every `0x00EA` `Variables Object` record in `data`.
///
/// Validation: type code and zero type flags, non-zero `oid`, zeroed words
/// at payload `+4` / `+8`, version byte `1` at `+12`, and a member count
/// whose slots plus the trailing word account for `bytes_to_follow`
/// exactly. Each 8-byte slot must be a zero word followed by a non-zero id.
pub fn decode_variables(data: &[u8]) -> Vec<PsmVariablesDecoded> {
    VariablesDecoder.scan(data)
}

/// Try to decode one `0x00EA` record at `offset`. Returns `None` on
/// validation failure.
pub fn decode_variables_at(data: &[u8], offset: usize) -> Option<PsmVariablesDecoded> {
    VariablesDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x00EA` `Variables Object`.
pub struct VariablesDecoder;

impl PsmRecordDecoder for VariablesDecoder {
    type Record = PsmVariablesDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_VARIABLES
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + VARIABLES_MEMBERS_AT + 8 + 4
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<PsmVariablesDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_VARIABLES || header.type_flags != 0 {
            return None;
        }
        let body = header.body_start;
        let btf = header.bytes_to_follow as usize;
        let end = body.checked_add(btf)?;
        if end > data.len() {
            return None;
        }
        let oid = u32_le(data, body)?;
        if oid == 0 || u32_le(data, body + 4)? != 0 || u32_le(data, body + 8)? != 0 {
            return None;
        }
        if *data.get(body + 12)? != 1 {
            return None;
        }
        let count = u32_le(data, body + 13)? as usize;
        // Every byte must be accounted for: head, `count` 8-byte slots, tail.
        if count == 0 || VARIABLES_MEMBERS_AT + count.checked_mul(8)? + 4 != btf {
            return None;
        }
        let mut members = Vec::with_capacity(count);
        for index in 0..count {
            let slot = body + VARIABLES_MEMBERS_AT + index * 8;
            if u32_le(data, slot)? != 0 {
                return None;
            }
            let member = u32_le(data, slot + 4)?;
            if member == 0 {
                return None;
            }
            members.push(member);
        }
        Some(PsmVariablesDecoded {
            byte_range: offset..end,
            oid,
            members,
            trailing: u32_le(data, body + VARIABLES_MEMBERS_AT + count * 8)?,
        })
    }

    fn advance_of(&self, record: &PsmVariablesDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// One named variable inside a [`PsmSymbolInformationDecoded`].
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInformationVariable {
    /// The variable's value, equal to the `value` of the
    /// [`PsmDoubleValueDecoded`] this entry names.
    pub value: f64,
    /// Variable name; only `Left`, `Right`, `Bottom` and `Top` appear in
    /// the corpus.
    pub name: String,
    /// Persist id of the `0x00C7` record holding the same value.
    pub value_ref: u32,
}

/// One decoded `0x00BD` `JSymbolInformation` record.
///
/// Two shapes share a 44-byte head. When `flags` is `0x0010` a variable
/// table follows and [`Self::variables`] is populated; otherwise the record
/// stops at the head (or a 6-byte zero tail) and the vector is empty.
#[derive(Debug, Clone, PartialEq)]
pub struct PsmSymbolInformationDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Persist id of this symbol-information object.
    pub oid: u32,
    /// Parent reference from payload `+4`; zero on some records.
    pub parent_ref: u32,
    /// Discriminator at payload `+12`; `3` on the shape that can carry
    /// variables, `11` on the short variant.
    pub kind: u16,
    /// Flag word at payload `+14` that gates the variable table.
    pub flags: u16,
    /// Two extents at payload `+16` / `+24`. Zero on the stub shape; their
    /// geometric meaning is not settled, so they are carried as read.
    pub extents: (f64, f64),
    /// The named variables, empty unless `flags` gates them in.
    pub variables: Vec<SymbolInformationVariable>,
}

/// Decode every `0x00BD` `JSymbolInformation` record in `data`.
///
/// Validation: type code and zero type flags, non-zero `oid`, zeroed word
/// at payload `+8`, zeroed double at `+32` and the constant `4` at `+40`.
/// When the variable table is gated in, every entry must carry the `1` /
/// `1` lead, a finite value, a non-empty name and a non-zero value
/// reference, and the table must end exactly at `bytes_to_follow`.
pub fn decode_symbol_informations(data: &[u8]) -> Vec<PsmSymbolInformationDecoded> {
    SymbolInformationDecoder.scan(data)
}

/// Try to decode one `0x00BD` record at `offset`. Returns `None` on
/// validation failure.
pub fn decode_symbol_information_at(
    data: &[u8],
    offset: usize,
) -> Option<PsmSymbolInformationDecoded> {
    SymbolInformationDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x00BD` `JSymbolInformation`.
pub struct SymbolInformationDecoder;

impl PsmRecordDecoder for SymbolInformationDecoder {
    type Record = PsmSymbolInformationDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_JSYMBOL_INFORMATION
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + SYMBOL_INFORMATION_HEAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<PsmSymbolInformationDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_JSYMBOL_INFORMATION || header.type_flags != 0 {
            return None;
        }
        let body = header.body_start;
        let btf = header.bytes_to_follow as usize;
        if btf < SYMBOL_INFORMATION_HEAD_LEN {
            return None;
        }
        let end = body.checked_add(btf)?;
        if end > data.len() {
            return None;
        }
        let oid = u32_le(data, body)?;
        if oid == 0 || u32_le(data, body + 8)? != 0 {
            return None;
        }
        if f64_le(data, body + 32)? != 0.0 || u32_le(data, body + 40)? != 4 {
            return None;
        }
        let flags = u16_le(data, body + 14)?;
        let mut variables = Vec::new();
        if flags == SYMBOL_INFORMATION_HAS_VARIABLES {
            let mut at = body + SYMBOL_INFORMATION_HEAD_LEN;
            let count = u32_le(data, at)?;
            at += 4;
            for _ in 0..count {
                if *data.get(at)? != 1 || u32_le(data, at + 1)? != 1 {
                    return None;
                }
                at += 5;
                let value = f64_le(data, at)?;
                if !value.is_finite() {
                    return None;
                }
                at += 8;
                let chars = usize::from(u16_le(data, at)?);
                at += 2;
                let units: Vec<u16> = (0..chars)
                    .map(|index| u16_le(data, at + index * 2))
                    .collect::<Option<_>>()?;
                let name = String::from_utf16(&units).ok()?;
                if name.is_empty() {
                    return None;
                }
                at += chars * 2;
                let value_ref = u32_le(data, at)?;
                if value_ref == 0 {
                    return None;
                }
                at += 4;
                variables.push(SymbolInformationVariable {
                    value,
                    name,
                    value_ref,
                });
            }
            // The table must land exactly on the record's declared end.
            if at != end {
                return None;
            }
        }
        Some(PsmSymbolInformationDecoded {
            byte_range: offset..end,
            oid,
            parent_ref: u32_le(data, body + 4)?,
            kind: u16_le(data, body + 12)?,
            flags,
            extents: (f64_le(data, body + 16)?, f64_le(data, body + 24)?),
            variables,
        })
    }

    fn advance_of(&self, record: &PsmSymbolInformationDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// PSM type code of `symbol.dex` `JFlavorHolder`
/// (`tools/psm_type_clsid.py 0xED` → `0C29A523-1143-11D0-AF3A-080036D72102`).
/// One per `0x00BD` `JSymbolInformation` in every nested storage; see
/// `docs/analysis/2026-09-20-jflavorholder-carries-the-placed-instances-parameters.md`.
pub const PSM_TYPE_CODE_JFLAVOR_HOLDER: u16 = 0x00ED;

/// Payload bytes before a flavour holder's variant word: oid, ten zero
/// bytes, the link word, the count word.
const FLAVOR_HOLDER_HEAD_LEN: usize = 20;
/// Byte length of one `01 01 00 00 00` + `f64` value entry.
const FLAVOR_HOLDER_ENTRY_LEN: usize = 13;
/// The variant word of a holder that carries the placed instance's values.
pub const FLAVOR_HOLDER_INSTANCE: u32 = 1;
/// The variant word of a holder that only names its `JSymbolInformation`.
pub const FLAVOR_HOLDER_TEMPLATE: u32 = 2;

/// One decoded `0x00ED` `JFlavorHolder` record.
///
/// Two variants share the head (oid, ten zero bytes, a link word, a `u16`
/// value count) and the `"Sheets"` name that follows. The **instance**
/// variant (`Imagineer Document`, the storage a placement names) then
/// carries one `f64` per variable of the storage's `JSymbolInformation`,
/// in that record's variable order -- the parameters the placed instance
/// was actually drawn with, where the `JSymbolInformation` copy repeats the
/// library defaults. The **template** variant (`Server Document`) carries no
/// value and names its `JSymbolInformation` by oid instead. Measured on the
/// corpus in
/// `docs/analysis/2026-09-20-jflavorholder-carries-the-placed-instances-parameters.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct PsmFlavorHolderDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Persist id of the holder.
    pub oid: u32,
    /// The word at payload `+14`: an oid on the instance variant (not the
    /// placement's graphic oid; meaning unsettled), zero on the template
    /// variant. Carried as read.
    pub link: u32,
    /// [`FLAVOR_HOLDER_INSTANCE`] or [`FLAVOR_HOLDER_TEMPLATE`].
    pub variant: u32,
    /// Persist id of the `JSymbolInformation` the template variant names
    /// (that record's `parent_ref` is this holder's oid); `None` on the
    /// instance variant.
    pub symbol_information_ref: Option<u32>,
    /// The instance's parameter values, metres, in the `JSymbolInformation`
    /// variable order; empty on the template variant.
    pub values: Vec<f64>,
}

/// Decode every `0x00ED` `JFlavorHolder` record in `data`.
///
/// Validation: type code and zero type flags, non-zero `oid`, ten zero
/// bytes at payload `+4`, the `1` / `1` lead at `+20`, a variant word of `1`
/// or `2`, the length-prefixed UTF-16 names in the order each variant
/// states them (`"Sheets"`; `"SI"` + a non-zero reference + `"Sheets"`),
/// two words, and exactly `count` finite value entries closing the record
/// (`count` is zero on the template variant).
pub fn decode_flavor_holders(data: &[u8]) -> Vec<PsmFlavorHolderDecoded> {
    FlavorHolderDecoder.scan(data)
}

/// [`PsmRecordDecoder`] adapter for `0x00ED` `JFlavorHolder`.
pub struct FlavorHolderDecoder;

impl FlavorHolderDecoder {
    /// A `u32` byte length followed by that many bytes of UTF-16LE, decoded;
    /// the offset just past it.
    fn utf16_at(data: &[u8], at: usize, end: usize) -> Option<(String, usize)> {
        let bytes = usize::try_from(u32_le(data, at)?).ok()?;
        if bytes % 2 != 0 || bytes == 0 {
            return None;
        }
        let start = at.checked_add(4)?;
        let stop = start.checked_add(bytes)?;
        if stop > end {
            return None;
        }
        let units: Vec<u16> = (0..bytes / 2)
            .map(|index| u16_le(data, start + index * 2))
            .collect::<Option<_>>()?;
        Some((String::from_utf16(&units).ok()?, stop))
    }
}

impl PsmRecordDecoder for FlavorHolderDecoder {
    type Record = PsmFlavorHolderDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_JFLAVOR_HOLDER
    }

    fn min_record_len(&self) -> usize {
        // Envelope + head + lead + variant + the shortest name.
        PSM_ENVELOPE_LEN + FLAVOR_HOLDER_HEAD_LEN + 2 + 4 + 4 + 4
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<PsmFlavorHolderDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_JFLAVOR_HOLDER || header.type_flags != 0 {
            return None;
        }
        let body = header.body_start;
        let btf = header.bytes_to_follow as usize;
        let end = body.checked_add(btf)?;
        if end > data.len() || btf < FLAVOR_HOLDER_HEAD_LEN + 2 + 4 {
            return None;
        }
        let oid = u32_le(data, body)?;
        if oid == 0 || data.get(body + 4..body + 14)?.iter().any(|b| *b != 0) {
            return None;
        }
        let link = u32_le(data, body + 14)?;
        let count = usize::from(u16_le(data, body + 18)?);
        if u16_le(data, body + 20)? != 0x0101 {
            return None;
        }
        let variant = u32_le(data, body + 22)?;
        let mut at = body + 26;
        let symbol_information_ref = match variant {
            FLAVOR_HOLDER_INSTANCE => None,
            FLAVOR_HOLDER_TEMPLATE => {
                let (marker, next) = Self::utf16_at(data, at, end)?;
                if marker != "SI" {
                    return None;
                }
                let reference = u32_le(data, next)?;
                if reference == 0 {
                    return None;
                }
                at = next + 4;
                Some(reference)
            }
            _ => return None,
        };
        let (name, next) = Self::utf16_at(data, at, end)?;
        if name != "Sheets" {
            return None;
        }
        // Two words this reader does not interpret, then the values.
        at = next.checked_add(8)?;
        if variant == FLAVOR_HOLDER_TEMPLATE && count != 0 {
            return None;
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            if *data.get(at)? != 1 || u32_le(data, at + 1)? != 1 {
                return None;
            }
            let value = f64_le(data, at + 5)?;
            if !value.is_finite() {
                return None;
            }
            values.push(value);
            at += FLAVOR_HOLDER_ENTRY_LEN;
        }
        if at != end {
            return None;
        }
        Some(PsmFlavorHolderDecoded {
            byte_range: offset..end,
            oid,
            link,
            variant,
            symbol_information_ref,
            values,
        })
    }

    fn advance_of(&self, record: &PsmFlavorHolderDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// One decoded `0x006F` `Assoc subsystem Standard Relation` record: a
/// `JBExpression` formula over operand objects.
#[derive(Debug, Clone, PartialEq)]
pub struct PsmStandardRelationDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Persist id of the relation.
    pub oid: u32,
    /// Operand signature, e.g. `%>i%<i` — `%>` marks the output operand,
    /// `%<` each input, and the letter is a type tag.
    pub signature: String,
    /// Operand persist ids in signature order, so the first is the output.
    /// In the corpus the output is always a `0x0115` `JDim` and the inputs
    /// are `0x00C7` values.
    pub operands: Vec<u32>,
    /// The formula, e.g. `0E$1+0.01`, where `$n` is the n-th input. The
    /// leading `0E` is present on every record and not yet understood.
    pub formula: String,
}

/// Decode every `0x006F` `Standard Relation` record in `data`.
///
/// Validation: type code and zero type flags, non-zero `oid`, the
/// `JBExpression` and `Double Value` CLSIDs at their fixed payload
/// offsets, one operand slot per signature marker with the expected
/// per-slot interface GUID, and a formula that ends the record exactly.
pub fn decode_standard_relations(data: &[u8]) -> Vec<PsmStandardRelationDecoded> {
    StandardRelationDecoder.scan(data)
}

/// Try to decode one `0x006F` record at `offset`. Returns `None` on
/// validation failure.
pub fn decode_standard_relation_at(
    data: &[u8],
    offset: usize,
) -> Option<PsmStandardRelationDecoded> {
    StandardRelationDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x006F` `Standard Relation`.
pub struct StandardRelationDecoder;

impl PsmRecordDecoder for StandardRelationDecoder {
    type Record = PsmStandardRelationDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_STANDARD_RELATION
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + RELATION_SIGNATURE_AT + 4
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<PsmStandardRelationDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_STANDARD_RELATION || header.type_flags != 0 {
            return None;
        }
        let body = header.body_start;
        let btf = header.bytes_to_follow as usize;
        if btf < RELATION_SIGNATURE_AT + 4 {
            return None;
        }
        let end = body.checked_add(btf)?;
        if end > data.len() {
            return None;
        }
        let oid = u32_le(data, body)?;
        if oid == 0 {
            return None;
        }
        let guid_at = |at: usize| data.get(body + at..body + at + 16);
        if guid_at(RELATION_EXPRESSION_CLSID_AT)? != JBEXPRESSION_CLSID
            || guid_at(RELATION_VALUE_CLSID_AT)? != DOUBLE_VALUE_CLSID
        {
            return None;
        }

        let mut at = body + RELATION_SIGNATURE_AT;
        let signature_len = u32_le(data, at)? as usize;
        at += 4;
        let signature_bytes = data.get(at..at.checked_add(signature_len)?)?;
        if !signature_bytes.is_ascii() {
            return None;
        }
        let signature: String = signature_bytes
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| char::from(*byte))
            .collect();
        at += signature_len;

        // `u32 2`, `u16 0x20`, then the operand count.
        if u32_le(data, at)? != 2 || u16_le(data, at + 4)? != 0x0020 {
            return None;
        }
        let count = u32_le(data, at + 6)? as usize;
        if count != signature.matches('%').count() || count == 0 {
            return None;
        }
        at += 10;
        let mut operands = Vec::with_capacity(count);
        for index in 0..count {
            // 8-byte slot marker: `0x0020`, a zero word, then `0x4000` on
            // the first operand and `0x8000` on every later one.
            let marker = if index == 0 { 0x4000u16 } else { 0x8000u16 };
            if u16_le(data, at)? != 0x0020
                || u32_le(data, at + 2)? != 0
                || u16_le(data, at + 6)? != marker
            {
                return None;
            }
            at += 8;
            let operand = u32_le(data, at)?;
            if operand == 0 {
                return None;
            }
            at += 4;
            if data.get(at..at.checked_add(16)?)? != RELATION_OPERAND_IID {
                return None;
            }
            at += 16;
            operands.push(operand);
        }

        let chars = u32_le(data, at)? as usize;
        at += 4;
        if at.checked_add(chars.checked_mul(2)?)? != end {
            return None;
        }
        let units: Vec<u16> = (0..chars)
            .map(|index| u16_le(data, at + index * 2))
            .collect::<Option<_>>()?;
        let formula = String::from_utf16(&units).ok()?.replace('\0', "");
        if formula.is_empty() {
            return None;
        }
        Some(PsmStandardRelationDecoded {
            byte_range: offset..end,
            oid,
            signature,
            operands,
            formula,
        })
    }

    fn advance_of(&self, record: &PsmStandardRelationDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

#[cfg(test)]
mod symbol_information_family_tests {
    use super::*;

    /// Wrap `payload` in the 6-byte PSM envelope for `type_code`.
    fn framed(type_code: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(PSM_ENVELOPE_LEN + payload.len());
        out.extend_from_slice(&type_code.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn double_value_payload(oid: u32, parent: u32, value: f64) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&parent.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&15u16.to_le_bytes());
        payload
    }

    fn variables_payload(oid: u32, members: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.push(1);
        payload.extend_from_slice(&(members.len() as u32).to_le_bytes());
        for member in members {
            payload.extend_from_slice(&0u32.to_le_bytes());
            payload.extend_from_slice(&member.to_le_bytes());
        }
        payload.extend_from_slice(&321u32.to_le_bytes());
        payload
    }

    fn symbol_information_payload(oid: u32, variables: &[(f64, &str, u32)]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&13u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&3u16.to_le_bytes());
        let flags = if variables.is_empty() {
            0
        } else {
            SYMBOL_INFORMATION_HAS_VARIABLES
        };
        payload.extend_from_slice(&flags.to_le_bytes());
        payload.extend_from_slice(&0.1016f64.to_le_bytes());
        payload.extend_from_slice(&0.17145f64.to_le_bytes());
        payload.extend_from_slice(&0.0f64.to_le_bytes());
        payload.extend_from_slice(&4u32.to_le_bytes());
        if !variables.is_empty() {
            payload.extend_from_slice(&(variables.len() as u32).to_le_bytes());
            for (value, name, value_ref) in variables {
                payload.push(1);
                payload.extend_from_slice(&1u32.to_le_bytes());
                payload.extend_from_slice(&value.to_le_bytes());
                let units: Vec<u16> = name.encode_utf16().collect();
                payload.extend_from_slice(&(units.len() as u16).to_le_bytes());
                for unit in units {
                    payload.extend_from_slice(&unit.to_le_bytes());
                }
                payload.extend_from_slice(&value_ref.to_le_bytes());
            }
        }
        payload
    }

    fn relation_payload(oid: u32, operands: &[u32], signature: &str, formula: &str) -> Vec<u8> {
        let mut payload = vec![0u8; RELATION_SIGNATURE_AT];
        payload[0..4].copy_from_slice(&oid.to_le_bytes());
        payload[RELATION_EXPRESSION_CLSID_AT..RELATION_EXPRESSION_CLSID_AT + 16]
            .copy_from_slice(&JBEXPRESSION_CLSID);
        payload[RELATION_VALUE_CLSID_AT..RELATION_VALUE_CLSID_AT + 16]
            .copy_from_slice(&DOUBLE_VALUE_CLSID);
        let mut signature_bytes = signature.as_bytes().to_vec();
        signature_bytes.push(0);
        payload.extend_from_slice(&(signature_bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(&signature_bytes);
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&0x0020u16.to_le_bytes());
        payload.extend_from_slice(&(operands.len() as u32).to_le_bytes());
        for (index, operand) in operands.iter().enumerate() {
            payload.extend_from_slice(&0x0020u16.to_le_bytes());
            payload.extend_from_slice(&0u32.to_le_bytes());
            let marker: u16 = if index == 0 { 0x4000 } else { 0x8000 };
            payload.extend_from_slice(&marker.to_le_bytes());
            payload.extend_from_slice(&operand.to_le_bytes());
            payload.extend_from_slice(&RELATION_OPERAND_IID);
        }
        let mut units: Vec<u16> = formula.encode_utf16().collect();
        units.push(0);
        payload.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for unit in units {
            payload.extend_from_slice(&unit.to_le_bytes());
        }
        payload
    }

    #[test]
    fn double_value_decodes_the_canonical_record() {
        let data = framed(
            PSM_TYPE_CODE_DOUBLE_VALUE,
            &double_value_payload(30, 41, 0.0609),
        );
        let decoded = decode_double_values(&data);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 30);
        assert_eq!(decoded[0].parent_ref, 41);
        assert_eq!(decoded[0].value, 0.0609);
        assert_eq!(decoded[0].trailing, (1, 15));
        assert_eq!(decoded[0].byte_range, 0..data.len());
    }

    #[test]
    fn double_value_rejects_a_wrong_length_or_zeroed_identity() {
        let mut short = double_value_payload(30, 41, 1.0);
        short.truncate(20);
        assert!(decode_double_value_at(&framed(PSM_TYPE_CODE_DOUBLE_VALUE, &short), 0).is_none());
        let zero_oid = double_value_payload(0, 41, 1.0);
        assert!(
            decode_double_value_at(&framed(PSM_TYPE_CODE_DOUBLE_VALUE, &zero_oid), 0).is_none()
        );
        let orphan = double_value_payload(30, 0, 1.0);
        assert!(decode_double_value_at(&framed(PSM_TYPE_CODE_DOUBLE_VALUE, &orphan), 0).is_none());
    }

    #[test]
    fn double_value_rejects_a_non_finite_value() {
        let payload = double_value_payload(30, 41, f64::NAN);
        assert!(decode_double_value_at(&framed(PSM_TYPE_CODE_DOUBLE_VALUE, &payload), 0).is_none());
    }

    #[test]
    fn variables_decodes_its_member_list() {
        let data = framed(
            PSM_TYPE_CODE_VARIABLES,
            &variables_payload(153, &[190, 744, 759]),
        );
        let decoded = decode_variables(&data);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 153);
        assert_eq!(decoded[0].members, vec![190, 744, 759]);
        assert_eq!(decoded[0].trailing, 321);
        assert_eq!(decoded[0].byte_range, 0..data.len());
    }

    #[test]
    fn variables_rejects_a_count_that_does_not_account_for_the_payload() {
        let mut payload = variables_payload(153, &[190, 744]);
        // Claim three members while only two slots follow.
        payload[13..17].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_variables_at(&framed(PSM_TYPE_CODE_VARIABLES, &payload), 0).is_none());
    }

    #[test]
    fn variables_rejects_an_empty_list_or_a_bad_version() {
        assert!(decode_variables_at(
            &framed(PSM_TYPE_CODE_VARIABLES, &variables_payload(1, &[])),
            0
        )
        .is_none());
        let mut payload = variables_payload(153, &[190]);
        payload[12] = 2;
        assert!(decode_variables_at(&framed(PSM_TYPE_CODE_VARIABLES, &payload), 0).is_none());
    }

    #[test]
    fn symbol_information_decodes_its_named_variables() {
        let payload = symbol_information_payload(
            22,
            &[
                (0.0609, "Left", 30),
                (0.0609, "Right", 31),
                (0.0354, "Top", 33),
            ],
        );
        let data = framed(PSM_TYPE_CODE_JSYMBOL_INFORMATION, &payload);
        let decoded = decode_symbol_informations(&data);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 22);
        assert_eq!(decoded[0].flags, SYMBOL_INFORMATION_HAS_VARIABLES);
        assert_eq!(decoded[0].extents, (0.1016, 0.17145));
        let names: Vec<&str> = decoded[0]
            .variables
            .iter()
            .map(|variable| variable.name.as_str())
            .collect();
        assert_eq!(names, vec!["Left", "Right", "Top"]);
        assert_eq!(decoded[0].variables[2].value_ref, 33);
        assert_eq!(decoded[0].byte_range, 0..data.len());
    }

    #[test]
    fn symbol_information_decodes_the_stub_shape_with_no_variables() {
        let data = framed(
            PSM_TYPE_CODE_JSYMBOL_INFORMATION,
            &symbol_information_payload(95, &[]),
        );
        let decoded = decode_symbol_informations(&data);
        assert_eq!(decoded.len(), 1);
        assert!(decoded[0].variables.is_empty());
        assert_eq!(decoded[0].flags, 0);
    }

    #[test]
    fn symbol_information_rejects_a_variable_table_that_overruns_the_record() {
        let mut payload = symbol_information_payload(22, &[(1.0, "Left", 30)]);
        // Claim two variables where only one is present.
        payload[SYMBOL_INFORMATION_HEAD_LEN..SYMBOL_INFORMATION_HEAD_LEN + 4]
            .copy_from_slice(&2u32.to_le_bytes());
        assert!(decode_symbol_information_at(
            &framed(PSM_TYPE_CODE_JSYMBOL_INFORMATION, &payload),
            0
        )
        .is_none());
    }

    #[test]
    fn symbol_information_rejects_a_head_that_fails_its_constants() {
        let mut payload = symbol_information_payload(22, &[]);
        payload[40..44].copy_from_slice(&5u32.to_le_bytes());
        assert!(decode_symbol_information_at(
            &framed(PSM_TYPE_CODE_JSYMBOL_INFORMATION, &payload),
            0
        )
        .is_none());
    }

    #[test]
    fn standard_relation_decodes_signature_operands_and_formula() {
        let payload = relation_payload(44, &[24, 33], "%>i%<i", "0E$1+0.01");
        let data = framed(PSM_TYPE_CODE_STANDARD_RELATION, &payload);
        let decoded = decode_standard_relations(&data);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 44);
        assert_eq!(decoded[0].signature, "%>i%<i");
        assert_eq!(decoded[0].operands, vec![24, 33]);
        assert_eq!(decoded[0].formula, "0E$1+0.01");
        assert_eq!(decoded[0].byte_range, 0..data.len());
    }

    #[test]
    fn standard_relation_rejects_an_operand_count_the_signature_does_not_promise() {
        let payload = relation_payload(44, &[24, 33], "%>i", "0E$1");
        assert!(
            decode_standard_relation_at(&framed(PSM_TYPE_CODE_STANDARD_RELATION, &payload), 0)
                .is_none()
        );
    }

    #[test]
    fn standard_relation_rejects_a_missing_expression_class() {
        let mut payload = relation_payload(44, &[24, 33], "%>i%<i", "0E$1");
        payload[RELATION_EXPRESSION_CLSID_AT] ^= 0xFF;
        assert!(
            decode_standard_relation_at(&framed(PSM_TYPE_CODE_STANDARD_RELATION, &payload), 0)
                .is_none()
        );
    }

    #[test]
    fn standard_relation_rejects_a_formula_that_does_not_end_the_record() {
        let mut payload = relation_payload(44, &[24, 33], "%>i%<i", "0E$1");
        payload.push(0);
        assert!(
            decode_standard_relation_at(&framed(PSM_TYPE_CODE_STANDARD_RELATION, &payload), 0)
                .is_none()
        );
    }

    #[test]
    fn every_family_ignores_a_record_of_another_type() {
        let payload = double_value_payload(30, 41, 1.0);
        let data = framed(PSM_TYPE_CODE_IGLINE2D, &payload);
        assert!(decode_double_values(&data).is_empty());
        assert!(decode_variables(&data).is_empty());
        assert!(decode_symbol_informations(&data).is_empty());
        assert!(decode_standard_relations(&data).is_empty());
    }

    #[test]
    fn every_family_survives_a_truncated_stream() {
        let full = framed(
            PSM_TYPE_CODE_JSYMBOL_INFORMATION,
            &symbol_information_payload(22, &[(1.0, "Left", 30)]),
        );
        for cut in 0..full.len() {
            let _ = decode_symbol_informations(&full[..cut]);
            let _ = decode_double_values(&full[..cut]);
            let _ = decode_variables(&full[..cut]);
            let _ = decode_standard_relations(&full[..cut]);
        }
    }
}

// ---------------------------------------------------------------------------
// 2026-09-07: the nested-site curve family (decoded, held back)
// ---------------------------------------------------------------------------
//
// Two record families the vendor's own graphic predicate says draw, which
// the corpus keeps only inside nested `JSite<N>/PSMcluster0` storages and
// never in a top-level `Sheet*` stream: `igCircle2d` (`0x0059`, 12 records
// across the four fixtures) and `igArc2d` (`0x0061`, 12). Their layouts are
// native-reader grade twice over: Phase 36's byte statistics on the corpus
// and the `imagdex.dex` `IJPersist::DoIO` workers decompiled in
// `docs/analysis/2026-08-31-imagdex-geometry-doio-ida.md` agree byte for
// byte -- the 18-byte sub-header `igLine2d` opens with, then the doubles,
// then one flag byte.
//
// They are decoded so the records stop being invisible, and **held back
// from the drawing**: the storage they sit in has no proven transform to
// the page (`docs/analysis/2026-08-31-jsite-geometry-coverage-gap.md`), and
// a circle drawn in the wrong coordinate space is worse than a circle named
// as missing. They are deliberately not registered in
// `model::sheet_families`, which describes what a `Sheet*` stream can hold;
// the surface is `JSite::nested_geometry`, and `build_normalized_geometry`
// names the held-back counts in its warnings.

/// PSM type code for `igCircle2d` (`JCircle2d`, `imagdex.dex`; IGDS class
/// tag `0x59 = 89`).
pub const PSM_TYPE_CODE_IGCIRCLE2D: u16 = 0x0059;

/// PSM type code for `igArc2d` (`JArc2d`, `imagdex.dex`; IGDS class tag
/// `0x61 = 97`).
pub const PSM_TYPE_CODE_IGARC2D: u16 = 0x0061;

/// Payload of one `igCircle2d`: the 18-byte sub-header, then `center.x`,
/// `center.y`, `radius` (3×f64) and one flag byte.
pub const IGCIRCLE2D_PAYLOAD_LEN: usize = 43;

/// Payload of one `igArc2d`: the 18-byte sub-header, then `center.x`,
/// `center.y`, `radius`, `start_angle`, `end_angle` (5×f64) and one flag
/// byte. The angles are absolute start / end angles, not a sweep
/// (`docs/analysis/2026-07-27-ugeom2d1-curve-readers-ida.md`), and the arc
/// runs **clockwise** from the first to the second
/// (`docs/analysis/2026-09-19-igarc2d-sweeps-clockwise-from-start-to-end.md`).
pub const IGARC2D_PAYLOAD_LEN: usize = 59;

/// Offset of the first geometry double in either payload -- the end of the
/// `oid / parent_ref / sheet_layer_ref / sub_type_word / index` sub-header
/// every fixed-layout geometry record opens with.
const CURVE_GEOMETRY_AT: usize = 18;

/// One decoded `0x0059` `igCircle2d` record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgCircle2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this circle sits on (payload `+8`), in the
    /// layer table of the storage the record lives in.
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); semantics not decoded.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`), the same slot `igLine2d`
    /// carries its style link in.
    pub index: u32,
    /// Centre, in the storage's own coordinates (payload `+18`, `+26`).
    pub center: (f64, f64),
    /// Radius, same units (payload `+34`).
    pub radius: f64,
    /// The trailing byte (payload `+42`); meaning unknown, carried for
    /// audit.
    pub flag: u8,
}

/// One decoded `0x0061` `igArc2d` record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgArc2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this arc sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); semantics not decoded.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`).
    pub index: u32,
    /// Centre, in the storage's own coordinates (payload `+18`, `+26`).
    pub center: (f64, f64),
    /// Radius, same units (payload `+34`).
    pub radius: f64,
    /// Absolute start angle, radians counter-clockwise from +X (payload
    /// `+42`); the arc leaves it **clockwise** towards `end_angle`.
    pub start_angle: f64,
    /// Absolute end angle, radians counter-clockwise from +X (payload `+50`).
    pub end_angle: f64,
    /// The trailing byte (payload `+58`); meaning unknown, carried for
    /// audit. Not the sweep direction: the corpus has it `0` and `1` on
    /// arcs that both run clockwise.
    pub flag: u8,
}

/// The 18-byte sub-header the fixed-layout geometry families share.
struct CurveSubHeader {
    oid: u32,
    parent_ref: u32,
    sheet_layer_ref: u32,
    sub_type_word: u16,
    index: u32,
}

fn curve_sub_header(payload: &[u8]) -> Option<CurveSubHeader> {
    Some(CurveSubHeader {
        oid: u32_le(payload, 0)?,
        parent_ref: u32_le(payload, 4)?,
        sheet_layer_ref: u32_le(payload, 8)?,
        sub_type_word: u16_le(payload, 12)?,
        index: u32_le(payload, 14)?,
    })
}

/// `count` consecutive doubles from payload `+18`, or `None` when any is
/// non-finite or outside the coordinate domain every other geometry family
/// enforces.
fn curve_doubles<const N: usize>(payload: &[u8]) -> Option<[f64; N]> {
    let mut out = [0f64; N];
    for (slot, value) in out.iter_mut().enumerate() {
        let read = f64_le(payload, CURVE_GEOMETRY_AT + slot * 8)?;
        if !read.is_finite() || read.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
            return None;
        }
        *value = read;
    }
    Some(out)
}

/// Decode every `igCircle2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_iglines`]: a candidate has to start where the
/// stream's own chain says a record starts ([`sheet_record_starts`]), then
/// satisfy: type code [`PSM_TYPE_CODE_IGCIRCLE2D`]; `bytes_to_follow ==
/// 43`; three finite in-domain doubles; `radius > 0`.
pub fn decode_igcircles(data: &[u8]) -> Vec<SheetIgCircle2dDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| IgCircle2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igCircle2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igcircle_at(data: &[u8], offset: usize) -> Option<SheetIgCircle2dDecoded> {
    IgCircle2dDecoder.decode_at(data, offset)
}

/// Decode every `igArc2d` record in a record-chain stream.
///
/// Same gate and rules as [`decode_igcircles`], with five doubles and
/// `bytes_to_follow == 59`. No rule is placed on the angles beyond being
/// finite: the native reader stores them as absolute angles and does not
/// normalise them, so neither does this.
pub fn decode_igarcs(data: &[u8]) -> Vec<SheetIgArc2dDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| IgArc2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igArc2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igarc_at(data: &[u8], offset: usize) -> Option<SheetIgArc2dDecoded> {
    IgArc2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x0059` `igCircle2d`.
pub struct IgCircle2dDecoder;

impl PsmRecordDecoder for IgCircle2dDecoder {
    type Record = SheetIgCircle2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGCIRCLE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGCIRCLE2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgCircle2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGCIRCLE2D
            || header.bytes_to_follow as usize != IGCIRCLE2D_PAYLOAD_LEN
        {
            return None;
        }
        let end = header.body_start.checked_add(IGCIRCLE2D_PAYLOAD_LEN)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let [center_x, center_y, radius] = curve_doubles::<3>(payload)?;
        if radius <= 0.0 {
            return None;
        }
        Some(SheetIgCircle2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            center: (center_x, center_y),
            radius,
            flag: *payload.get(IGCIRCLE2D_PAYLOAD_LEN - 1)?,
        })
    }

    fn advance_of(&self, record: &SheetIgCircle2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// [`PsmRecordDecoder`] adapter for `0x0061` `igArc2d`.
pub struct IgArc2dDecoder;

impl PsmRecordDecoder for IgArc2dDecoder {
    type Record = SheetIgArc2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGARC2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGARC2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgArc2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGARC2D
            || header.bytes_to_follow as usize != IGARC2D_PAYLOAD_LEN
        {
            return None;
        }
        let end = header.body_start.checked_add(IGARC2D_PAYLOAD_LEN)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let [center_x, center_y, radius, start_angle, end_angle] = curve_doubles::<5>(payload)?;
        if radius <= 0.0 {
            return None;
        }
        Some(SheetIgArc2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            center: (center_x, center_y),
            radius,
            start_angle,
            end_angle,
            flag: *payload.get(IGARC2D_PAYLOAD_LEN - 1)?,
        })
    }

    fn advance_of(&self, record: &SheetIgArc2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

// ---------------------------------------------------------------------------
// 2026-09-07: igRectangle2d and igBspCurve2d
// ---------------------------------------------------------------------------
//
// The two remaining families of the `imagdex.dex` curve set
// (`docs/analysis/2026-08-31-imagdex-geometry-doio-ida.md` §5 / §5bis), each
// checked field by field against the records the corpus holds -- three
// rectangles and one B-spline, which is too few for statistics and exactly
// enough to read a native layout against
// (`docs/analysis/2026-09-07-rectangle-owns-its-edges-bspline-is-a-leaf.md`).
//
// They are not alike in what they draw. A rectangle is a **parent**: its
// five doubles are `(origin.x, origin.y, width, rotation, height / width)`,
// and its tail lists the oids of the four `igLine2d` records that are its
// edges -- records that sit in the same stream, decode on their own and are
// already emitted. Drawing the rectangle would draw its edges twice, so it
// decodes and emits nothing. A B-spline is a **leaf**: poles, optional
// weights and knots, and nothing else draws it, so it is emitted as the
// polyline `crate::bspline::sample` makes of it.

/// PSM type code for `igRectangle2d` (`JRectangle2d`, `imagdex.dex`; IGDS
/// class tag `0x20 = 32`).
pub const PSM_TYPE_CODE_IGRECTANGLE2D: u16 = 0x0020;

/// PSM type code for `igBspCurve2d` (`JBspCurve2d`, `imagdex.dex`; IGDS class
/// tag `0x5D = 93`).
pub const PSM_TYPE_CODE_IGBSPCURVE2D: u16 = 0x005D;

/// Shortest `igRectangle2d` payload: the 18-byte sub-header, five doubles and
/// the `u32` count of the edge list.
pub const IGRECTANGLE2D_MIN_PAYLOAD_LEN: usize = CURVE_GEOMETRY_AT + 40 + 4;

/// Payload offset of a rectangle's edge count.
const IGRECTANGLE2D_EDGES_AT: usize = CURVE_GEOMETRY_AT + 40;

/// Most edges a rectangle may list. Four on the corpus; the bound only stops
/// a corrupt count from being read as a length.
const IGRECTANGLE2D_MAX_EDGES: usize = 64;

/// Shortest `igBspCurve2d` payload: sub-header, `u32 N`, two poles, the
/// weight flag, `u32 M`, four knots (degree one), the trailing double and
/// four flag bytes.
pub const IGBSPCURVE2D_MIN_PAYLOAD_LEN: usize = CURVE_GEOMETRY_AT + 4 + 32 + 4 + 4 + 32 + 8 + 4;

/// Most poles a B-spline may carry. Five on the corpus; same purpose as the
/// polyline reader's vertex cap.
const IGBSPCURVE2D_MAX_POLES: usize = 4096;

/// One decoded `0x0020` `igRectangle2d` record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgRectangle2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this rectangle sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); semantics not decoded.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`).
    pub index: u32,
    /// The corner the width runs from (payload `+18`, `+26`), in the
    /// stream's own coordinates.
    pub origin: (f64, f64),
    /// Extent along the rectangle's own x axis (payload `+34`).
    pub width: f64,
    /// Angle of that axis, radians (payload `+42`). Zero on every corpus
    /// record, so the reading rests on the native reader's bounding-box
    /// routine rather than on data.
    pub rotation: f64,
    /// Height as a fraction of the width (payload `+50`): the A2 border of
    /// the `A01` export reads `0.594 x 0.707071`, which is `594 x 420 mm`
    /// exactly.
    pub aspect: f64,
    /// Oids of the `igLine2d` records that are this rectangle's edges, in
    /// on-disk order (payload `+62` on, after the `u32` count at `+58`).
    /// Four on every corpus record, and on every one the four lines'
    /// endpoints are the rectangle's corners.
    pub edges: Vec<u32>,
}

impl SheetIgRectangle2dDecoded {
    /// Extent along the rectangle's own y axis.
    pub fn height(&self) -> f64 {
        self.width * self.aspect
    }

    /// The four corners, counter-clockwise from the origin, with the
    /// rotation applied.
    pub fn corners(&self) -> [(f64, f64); 4] {
        let (sin, cos) = self.rotation.sin_cos();
        let (x0, y0) = self.origin;
        let along = |u: f64, v: f64| (x0 + u * cos - v * sin, y0 + u * sin + v * cos);
        let (w, h) = (self.width, self.height());
        [along(0.0, 0.0), along(w, 0.0), along(w, h), along(0.0, h)]
    }
}

/// One decoded `0x005D` `igBspCurve2d` record: a non-uniform B-spline,
/// rational when [`Self::weights`] is non-empty.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgBspCurve2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this curve sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); semantics not decoded.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`).
    pub index: u32,
    /// Control points, in order (payload `+22` on, after `u32 N` at `+18`).
    pub poles: Vec<(f64, f64)>,
    /// One weight per pole when the curve is rational; empty otherwise
    /// (the `u32` flag after the poles was zero).
    pub weights: Vec<f64>,
    /// The knot vector (after `u32 M`). `M - N - 1` is the degree.
    pub knots: Vec<f64>,
    /// The double after the knots; `-1.0` on the corpus, meaning unknown.
    pub trailing: f64,
    /// The four bytes that close the record; `04 01 01 00` on the corpus.
    pub flags: [u8; 4],
}

impl SheetIgBspCurve2dDecoded {
    /// Polynomial degree, `M - N - 1`.
    pub fn degree(&self) -> usize {
        self.knots.len().saturating_sub(self.poles.len() + 1)
    }
}

/// Decode every `igRectangle2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_iglines`]; then: type code
/// [`PSM_TYPE_CODE_IGRECTANGLE2D`]; five finite in-domain doubles;
/// `width > 0` and `aspect > 0`; an edge count of at most
/// [`IGRECTANGLE2D_MAX_EDGES`] whose list accounts for the payload exactly.
pub fn decode_igrectangles(data: &[u8]) -> Vec<SheetIgRectangle2dDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| IgRectangle2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igRectangle2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igrectangle_at(data: &[u8], offset: usize) -> Option<SheetIgRectangle2dDecoded> {
    IgRectangle2dDecoder.decode_at(data, offset)
}

/// Decode every `igBspCurve2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_iglines`]; then: type code
/// [`PSM_TYPE_CODE_IGBSPCURVE2D`]; `2..=4096` finite in-domain poles; when
/// rational, one finite positive weight per pole; at least `N + 2` finite
/// non-decreasing knots; and every counted field accounting for the payload
/// exactly, trailing double and four flag bytes included.
pub fn decode_igbspcurves(data: &[u8]) -> Vec<SheetIgBspCurve2dDecoded> {
    sheet_record_starts(data)
        .into_iter()
        .filter_map(|at| IgBspCurve2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igBspCurve2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igbspcurve_at(data: &[u8], offset: usize) -> Option<SheetIgBspCurve2dDecoded> {
    IgBspCurve2dDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x0020` `igRectangle2d`.
pub struct IgRectangle2dDecoder;

impl PsmRecordDecoder for IgRectangle2dDecoder {
    type Record = SheetIgRectangle2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGRECTANGLE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGRECTANGLE2D_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgRectangle2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        let len = header.bytes_to_follow as usize;
        if header.type_code != PSM_TYPE_CODE_IGRECTANGLE2D || len < IGRECTANGLE2D_MIN_PAYLOAD_LEN {
            return None;
        }
        let end = header.body_start.checked_add(len)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let [x, y, width, rotation, aspect] = curve_doubles::<5>(payload)?;
        if width <= 0.0 || aspect <= 0.0 {
            return None;
        }
        let edge_count = u32_le(payload, IGRECTANGLE2D_EDGES_AT)? as usize;
        if edge_count > IGRECTANGLE2D_MAX_EDGES
            || IGRECTANGLE2D_EDGES_AT + 4 + edge_count * 4 != payload.len()
        {
            return None;
        }
        let edges = (0..edge_count)
            .map(|slot| u32_le(payload, IGRECTANGLE2D_EDGES_AT + 4 + slot * 4))
            .collect::<Option<Vec<u32>>>()?;
        Some(SheetIgRectangle2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            origin: (x, y),
            width,
            rotation,
            aspect,
            edges,
        })
    }

    fn advance_of(&self, record: &SheetIgRectangle2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// The curve an `igBspCurve2d` payload carries after its 18-byte sub-header,
/// read the same way from a drawing's record and from a `.sym` body's.
#[derive(Debug, Clone, PartialEq)]
pub struct BspCurveGeometry {
    /// Control points, in order.
    pub poles: Vec<(f64, f64)>,
    /// One weight per pole when rational; empty otherwise.
    pub weights: Vec<f64>,
    /// The knot vector.
    pub knots: Vec<f64>,
    /// The double after the knots; `-1.0` on the corpus.
    pub trailing: f64,
    /// The four closing bytes.
    pub flags: [u8; 4],
}

/// Read the curve out of a full `igBspCurve2d` payload (sub-header
/// included), or `None` when any counted field fails validation or the
/// fields do not account for the payload exactly. Validation rules are
/// documented on [`decode_igbspcurves`].
pub fn bspcurve_geometry(payload: &[u8]) -> Option<BspCurveGeometry> {
    let in_domain =
        |value: f64| value.is_finite() && value.abs() <= GLINE2D_COORDINATE_DOMAIN_LIMIT;
    let mut at = CURVE_GEOMETRY_AT;
    let pole_count = u32_le(payload, at)? as usize;
    if !(2..=IGBSPCURVE2D_MAX_POLES).contains(&pole_count) {
        return None;
    }
    at += 4;
    let mut poles = Vec::with_capacity(pole_count);
    for _ in 0..pole_count {
        let x = f64_le(payload, at)?;
        let y = f64_le(payload, at + 8)?;
        if !in_domain(x) || !in_domain(y) {
            return None;
        }
        poles.push((x, y));
        at += 16;
    }
    let rational = u32_le(payload, at)? != 0;
    at += 4;
    let mut weights = Vec::new();
    if rational {
        for _ in 0..pole_count {
            let weight = f64_le(payload, at)?;
            if !weight.is_finite() || weight <= 0.0 {
                return None;
            }
            weights.push(weight);
            at += 8;
        }
    }
    let knot_count = u32_le(payload, at)? as usize;
    // Degree at least one, and no more knots than a cap the corpus is
    // nowhere near; the exact-length rule below is the real guard.
    if knot_count < pole_count + 2 || knot_count > pole_count + IGBSPCURVE2D_MAX_POLES {
        return None;
    }
    at += 4;
    let mut knots = Vec::with_capacity(knot_count);
    for _ in 0..knot_count {
        let knot = f64_le(payload, at)?;
        if !knot.is_finite() || knots.last().is_some_and(|last| knot < *last) {
            return None;
        }
        knots.push(knot);
        at += 8;
    }
    let trailing = f64_le(payload, at)?;
    if !trailing.is_finite() {
        return None;
    }
    at += 8;
    let flags: [u8; 4] = payload.get(at..at + 4)?.try_into().ok()?;
    at += 4;
    if at != payload.len() {
        return None;
    }
    Some(BspCurveGeometry {
        poles,
        weights,
        knots,
        trailing,
        flags,
    })
}

/// [`PsmRecordDecoder`] adapter for `0x005D` `igBspCurve2d`.
pub struct IgBspCurve2dDecoder;

impl PsmRecordDecoder for IgBspCurve2dDecoder {
    type Record = SheetIgBspCurve2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGBSPCURVE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGBSPCURVE2D_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgBspCurve2dDecoded> {
        let header = parse_psm_header(data, offset)?;
        let len = header.bytes_to_follow as usize;
        if header.type_code != PSM_TYPE_CODE_IGBSPCURVE2D || len < IGBSPCURVE2D_MIN_PAYLOAD_LEN {
            return None;
        }
        let end = header.body_start.checked_add(len)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let curve = bspcurve_geometry(payload)?;
        Some(SheetIgBspCurve2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            poles: curve.poles,
            weights: curve.weights,
            knots: curve.knots,
            trailing: curve.trailing,
            flags: curve.flags,
        })
    }

    fn advance_of(&self, record: &SheetIgBspCurve2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

#[cfg(test)]
mod rectangle_and_bspline_tests {
    use super::*;

    /// The 8-byte stream header the chain walk skips, then the records.
    fn chain(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; SHEET_STREAM_HEADER_LEN];
        for record in records {
            out.extend_from_slice(record);
        }
        out
    }

    fn sub_header(out: &mut Vec<u8>, layer: u32) {
        out.extend_from_slice(&1909u32.to_le_bytes()); // oid
        out.extend_from_slice(&6615u32.to_le_bytes()); // parent_ref
        out.extend_from_slice(&layer.to_le_bytes()); // sheet_layer_ref
        out.extend_from_slice(&0u16.to_le_bytes()); // sub_type_word
        out.extend_from_slice(&1u32.to_le_bytes()); // index
    }

    /// The `/Sheet6615` rectangle of DWG-0202, byte for byte in shape.
    fn rectangle_record(doubles: [f64; 5], edges: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        sub_header(&mut payload, 6996);
        for value in doubles {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.extend_from_slice(&(edges.len() as u32).to_le_bytes());
        for edge in edges {
            payload.extend_from_slice(&edge.to_le_bytes());
        }
        let mut out = PSM_TYPE_CODE_IGRECTANGLE2D.to_le_bytes().to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// The `arrester breather valve(RD)` B-spline: five poles, no weights,
    /// a clamped cubic knot vector over two spans.
    fn bspline_record(poles: &[(f64, f64)], weights: &[f64], knots: &[f64]) -> Vec<u8> {
        let mut payload = Vec::new();
        sub_header(&mut payload, 2824);
        payload.extend_from_slice(&(poles.len() as u32).to_le_bytes());
        for (x, y) in poles {
            payload.extend_from_slice(&x.to_le_bytes());
            payload.extend_from_slice(&y.to_le_bytes());
        }
        payload.extend_from_slice(&u32::from(!weights.is_empty()).to_le_bytes());
        for weight in weights {
            payload.extend_from_slice(&weight.to_le_bytes());
        }
        payload.extend_from_slice(&(knots.len() as u32).to_le_bytes());
        for knot in knots {
            payload.extend_from_slice(&knot.to_le_bytes());
        }
        payload.extend_from_slice(&(-1.0f64).to_le_bytes());
        payload.extend_from_slice(&[0x04, 0x01, 0x01, 0x00]);
        let mut out = PSM_TYPE_CODE_IGBSPCURVE2D.to_le_bytes().to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    const CORPUS_POLES: [(f64, f64); 5] = [
        (0.005_041, 0.005_076),
        (0.005_379, 0.004_890),
        (0.006_078, 0.004_280),
        (0.005_379, 0.003_669),
        (0.005_041, 0.003_483),
    ];
    const CORPUS_KNOTS: [f64; 9] = [0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];

    #[test]
    fn a_rectangle_reads_its_frame_and_its_four_edges() {
        let record = rectangle_record(
            [0.126_271, 0.092_379, 0.127_478, 0.0, 0.406_863],
            &[4732, 5238, 6099, 6530],
        );
        assert_eq!(record.len(), PSM_ENVELOPE_LEN + 78);
        let decoded = decode_igrectangles(&chain(&[record]));
        assert_eq!(decoded.len(), 1);
        let rectangle = &decoded[0];
        assert_eq!(rectangle.oid, 1909);
        assert_eq!(rectangle.sheet_layer_ref, 6996);
        assert_eq!(rectangle.origin, (0.126_271, 0.092_379));
        assert_eq!(rectangle.width, 0.127_478);
        assert_eq!(rectangle.rotation, 0.0);
        assert!((rectangle.height() - 0.051_866).abs() < 1e-6);
        assert_eq!(rectangle.edges, vec![4732, 5238, 6099, 6530]);
        let corners = rectangle.corners();
        assert!((corners[2].0 - 0.253_749).abs() < 1e-6);
        assert!((corners[2].1 - 0.144_245).abs() < 1e-6);
    }

    #[test]
    fn a_rectangle_whose_edge_list_does_not_fill_the_payload_is_refused() {
        let mut record = rectangle_record([0.0, 0.0, 0.594, 0.0, 0.707_071], &[369, 297, 294, 296]);
        // Claim five edges over a four-edge payload.
        let count_at = PSM_ENVELOPE_LEN + IGRECTANGLE2D_EDGES_AT;
        record[count_at..count_at + 4].copy_from_slice(&5u32.to_le_bytes());
        assert!(decode_igrectangles(&chain(&[record])).is_empty());
        // A zero or negative extent is not a rectangle.
        for doubles in [[0.0, 0.0, 0.0, 0.0, 0.7], [0.0, 0.0, 0.5, 0.0, -0.7]] {
            let record = rectangle_record(doubles, &[1, 2, 3, 4]);
            assert!(
                decode_igrectangles(&chain(&[record])).is_empty(),
                "{doubles:?}"
            );
        }
    }

    #[test]
    fn a_bspline_reads_its_poles_knots_and_degree() {
        let record = bspline_record(&CORPUS_POLES, &[], &CORPUS_KNOTS);
        assert_eq!(record.len(), PSM_ENVELOPE_LEN + 194);
        let decoded = decode_igbspcurves(&chain(&[record]));
        assert_eq!(decoded.len(), 1);
        let curve = &decoded[0];
        assert_eq!(curve.sheet_layer_ref, 2824);
        assert_eq!(curve.poles, CORPUS_POLES.to_vec());
        assert!(curve.weights.is_empty());
        assert_eq!(curve.knots, CORPUS_KNOTS.to_vec());
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.trailing, -1.0);
        assert_eq!(curve.flags, [0x04, 0x01, 0x01, 0x00]);
    }

    #[test]
    fn a_rational_bspline_carries_one_weight_per_pole() {
        let record = bspline_record(
            &[(0.0, 0.0), (0.01, 0.01), (0.02, 0.0)],
            &[1.0, 0.5, 1.0],
            &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        let decoded = decode_igbspcurves(&chain(&[record]));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].weights, vec![1.0, 0.5, 1.0]);
        assert_eq!(decoded[0].degree(), 2);
    }

    #[test]
    fn a_bspline_with_a_bad_count_or_a_falling_knot_is_refused() {
        // Knots out of order.
        let falling = bspline_record(
            &CORPUS_POLES,
            &[],
            &[0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0, 1.0, 1.0],
        );
        assert!(decode_igbspcurves(&chain(&[falling])).is_empty());
        // Too few knots for the poles (degree zero).
        let flat = bspline_record(&CORPUS_POLES, &[], &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert!(decode_igbspcurves(&chain(&[flat])).is_empty());
        // A pole count that overruns the payload.
        let mut overrun = bspline_record(&CORPUS_POLES, &[], &CORPUS_KNOTS);
        let n_at = PSM_ENVELOPE_LEN + CURVE_GEOMETRY_AT;
        overrun[n_at..n_at + 4].copy_from_slice(&50u32.to_le_bytes());
        assert!(decode_igbspcurves(&chain(&[overrun])).is_empty());
    }

    #[test]
    fn neither_family_reads_the_other_and_both_survive_truncation() {
        let rectangle = rectangle_record([0.0, 0.0, 0.5, 0.0, 0.5], &[1, 2, 3, 4]);
        let bspline = bspline_record(&CORPUS_POLES, &[], &CORPUS_KNOTS);
        let stream = chain(&[rectangle, bspline]);
        assert_eq!(decode_igrectangles(&stream).len(), 1);
        assert_eq!(decode_igbspcurves(&stream).len(), 1);
        for cut in 0..stream.len() {
            let _ = decode_igrectangles(&stream[..cut]);
            let _ = decode_igbspcurves(&stream[..cut]);
            let _ = decode_igrectangle_at(&stream[..cut], cut.saturating_sub(1));
            let _ = decode_igbspcurve_at(&stream[..cut], cut.saturating_sub(1));
        }
    }
}

#[cfg(test)]
mod nested_curve_family_tests {
    use super::*;

    /// One record in the 6-byte envelope, then the 18-byte sub-header,
    /// then `doubles`, then the flag byte -- the layout §4 / §5 of the
    /// imagdex analysis reads off the native `DoIO` workers.
    fn curve_record(type_code: u16, layer: u32, doubles: &[f64], flag: u8) -> Vec<u8> {
        let payload_len = CURVE_GEOMETRY_AT + doubles.len() * 8 + 1;
        let mut out = Vec::with_capacity(PSM_ENVELOPE_LEN + payload_len);
        out.extend_from_slice(&type_code.to_le_bytes());
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        out.extend_from_slice(&4321u32.to_le_bytes()); // oid
        out.extend_from_slice(&77u32.to_le_bytes()); // parent_ref
        out.extend_from_slice(&layer.to_le_bytes()); // sheet_layer_ref
        out.extend_from_slice(&0x0010u16.to_le_bytes()); // sub_type_word
        out.extend_from_slice(&9u32.to_le_bytes()); // index
        for value in doubles {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.push(flag);
        out
    }

    /// The 8-byte stream header the chain walk skips, then the records.
    fn chain(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; SHEET_STREAM_HEADER_LEN];
        for record in records {
            out.extend_from_slice(record);
        }
        out
    }

    #[test]
    fn a_circle_reads_its_centre_radius_and_layer() {
        let stream = chain(&[curve_record(
            PSM_TYPE_CODE_IGCIRCLE2D,
            156,
            &[0.25, 0.125, 0.0075],
            1,
        )]);
        let decoded = decode_igcircles(&stream);
        assert_eq!(decoded.len(), 1);
        let circle = &decoded[0];
        assert_eq!(circle.byte_range, SHEET_STREAM_HEADER_LEN..stream.len());
        assert_eq!(circle.oid, 4321);
        assert_eq!(circle.parent_ref, 77);
        assert_eq!(circle.sheet_layer_ref, 156);
        assert_eq!(circle.sub_type_word, 0x0010);
        assert_eq!(circle.index, 9);
        assert_eq!(circle.center, (0.25, 0.125));
        assert_eq!(circle.radius, 0.0075);
        assert_eq!(circle.flag, 1);
    }

    #[test]
    fn an_arc_reads_its_absolute_angles() {
        let stream = chain(&[curve_record(
            PSM_TYPE_CODE_IGARC2D,
            199,
            &[0.5, 0.25, 0.01, 0.0, std::f64::consts::FRAC_PI_2],
            0,
        )]);
        let decoded = decode_igarcs(&stream);
        assert_eq!(decoded.len(), 1);
        let arc = &decoded[0];
        assert_eq!(arc.sheet_layer_ref, 199);
        assert_eq!(arc.center, (0.5, 0.25));
        assert_eq!(arc.radius, 0.01);
        assert_eq!(arc.start_angle, 0.0);
        assert_eq!(arc.end_angle, std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn the_two_families_do_not_read_each_other() {
        let circle = curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &[0.1, 0.2, 0.05], 1);
        let arc = curve_record(PSM_TYPE_CODE_IGARC2D, 8, &[0.1, 0.2, 0.05, 0.0, 1.0], 1);
        let stream = chain(&[circle, arc]);
        assert_eq!(decode_igcircles(&stream).len(), 1);
        assert_eq!(decode_igarcs(&stream).len(), 1);
        // The arc sits second in the chain; a sliding scan would also have
        // found it, so make sure the chain gate is what admitted it.
        assert_eq!(
            decode_igarcs(&stream)[0].byte_range.start,
            SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN + IGCIRCLE2D_PAYLOAD_LEN
        );
    }

    #[test]
    fn a_circle_payload_of_the_wrong_length_is_refused() {
        // Four doubles make a 51-byte payload: not a circle, whatever the
        // type code says.
        let stream = chain(&[curve_record(
            PSM_TYPE_CODE_IGCIRCLE2D,
            8,
            &[0.1, 0.2, 0.05, 0.0],
            1,
        )]);
        assert!(decode_igcircles(&stream).is_empty());
    }

    #[test]
    fn a_non_positive_radius_or_a_non_finite_double_is_refused() {
        for doubles in [[0.1, 0.2, 0.0], [0.1, 0.2, -0.05], [f64::NAN, 0.2, 0.05]] {
            let stream = chain(&[curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &doubles, 1)]);
            assert!(decode_igcircles(&stream).is_empty(), "{doubles:?}");
        }
        let stream = chain(&[curve_record(
            PSM_TYPE_CODE_IGARC2D,
            8,
            &[0.1, 0.2, 0.05, f64::INFINITY, 1.0],
            1,
        )]);
        assert!(decode_igarcs(&stream).is_empty());
    }

    #[test]
    fn a_record_off_the_chain_is_not_a_record() {
        // A valid circle whose stream lacks the 8-byte header: the chain
        // walk stalls at once and nothing is admitted, exactly as for lines.
        let bare = curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &[0.1, 0.2, 0.05], 1);
        assert!(decode_igcircles(&bare).is_empty());
        // The single-record entry point still reads it where it is.
        assert!(decode_igcircle_at(&bare, 0).is_some());
    }

    #[test]
    fn both_families_survive_truncation() {
        let stream = chain(&[
            curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &[0.1, 0.2, 0.05], 1),
            curve_record(PSM_TYPE_CODE_IGARC2D, 8, &[0.1, 0.2, 0.05, 0.0, 1.0], 1),
        ]);
        for cut in 0..stream.len() {
            let _ = decode_igcircles(&stream[..cut]);
            let _ = decode_igarcs(&stream[..cut]);
            let _ = decode_igcircle_at(&stream[..cut], cut.saturating_sub(1));
            let _ = decode_igarc_at(&stream[..cut], cut.saturating_sub(1));
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 32-J2: PSM `igDimension` / `JDim` (type `0x0115`) — the framed record
//
// Byte account:
// `docs/analysis/2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`.
// The 18 corpus records all sit in a symbol definition cache
// (`/JSite<N>/PSMcluster0`) on a layer named `Dimension` that the view
// filter set has switched off — the file says this family does not draw,
// so nothing here emits geometry.
//
// The frame is the native reader's arithmetic, not a corpus fit:
// `radsrvitem.dll!sub_564BB990` takes the *record header* as its base, so
// its `a2+40` is the payload's `+34`, and it reads the closing dword at
// `a2+40 + *(u32*)(a2+36)` — the payload's `34 + main_len` — and only
// under bit `0x100` of the flag word at the payload's `+26`. The corpus
// alone cannot separate `0x100` from `0x200` (every record carrying one
// carries the other); the reader can, and this decoder follows the reader.
//
// What is *not* backed is the inside of the blocks. The block at `+34` is
// sized by `sub_56446B50` from the dimension kind at `+14`, which the
// native reader switches on over eight values — and this corpus only ever
// exercises kind 1. So kind 1 is all this decoder accepts; every other kind
// is refused rather than guessed at, which is why §7 of the analysis splits
// the fields into "may be decoded" and "stays raw" and why the raw bytes of
// the block area travel in the DTO untouched.
//
// The two references this decoder does read were then checked against the
// file's own incoming-reference index
// (`docs/analysis/2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`):
// the measured-geometry slot at `+92` is backed by the space map 40/40 with
// the slot predicting the target class without a counterexample, and the
// closing dword resolves to a `0x0058 JDimGroup` on every record that
// carries one, with the group's member table naming the dimension back on
// all six groups. The `+140` word the 09-14 reading had called the owner
// failed both tests a reference has to pass and is *not* a field here — it
// stays inside `raw_tail`.
// ---------------------------------------------------------------------------

/// PSM type code for `imagdex.dex`'s `JDim Object` — the driving dimension
/// of a parametric symbol definition, `igDimension` in the RAD registry.
///
/// Resolved through `radsrvitem`'s `type code → CLSID` table plus the
/// `jutil` registry (`tools/psm_type_clsid.py`); `imagdex` carries the
/// matching `.?AVJDim@@`, so the identity is not inferred from shape.
pub const PSM_TYPE_CODE_IGDIMENSION: u16 = 0x0115;

/// Bytes of `JDim` payload before the kind-dependent block area: the
/// 18-byte envelope the graphic families share plus 16 bytes of header
/// words ending in `main_len`.
///
/// This is the `34` of the record's frame equation,
/// `payload = 34 + main_len + tail`.
pub const IGDIMENSION_FRAME_PREFIX_LEN: usize = 34;

/// Bit of the `+26` flag word that puts a closing `u32` at
/// `34 + main_len`.
///
/// Read off `sub_564BB990`, which tests exactly this bit before reading
/// the dword. Not `0x0300`: the two bits are inseparable in the corpus and
/// the native reader keeps them apart.
pub const IGDIMENSION_TAIL_WORD_FLAG: u16 = 0x0100;

/// Byte length of the closing word the [`IGDIMENSION_TAIL_WORD_FLAG`] bit
/// announces.
const IGDIMENSION_TAIL_WORD_LEN: usize = 4;

/// The one dimension kind (`+14`) this decoder reads.
///
/// The native reader dispatches over eight kinds, one block reader each,
/// and all 18 corpus records are this one. The other seven get block
/// lengths of 52 / 64 / 80 and a grammar nobody has seen bytes for.
pub const IGDIMENSION_KIND_LINEAR: u16 = 1;

/// Bit of the block's leading `u32` that grows a kind-1 block from 48
/// bytes to 80 (`sub_56446B50`).
///
/// No corpus record sets it, so the 80-byte shape is as unread as the
/// other seven kinds and is refused on the same grounds.
const IGDIMENSION_WIDE_BLOCK_FLAG: u32 = 0x2000;

/// Byte length of a kind-1 block when [`IGDIMENSION_WIDE_BLOCK_FLAG`] is
/// clear, per `sub_56446B50`.
pub const IGDIMENSION_LINEAR_BLOCK_LEN: usize = 48;

/// Payload offset where the reference slots begin, once the kind-1 block
/// is behind us: `34 + 48`.
///
/// This is why the section after the block starts at `+82` — the first
/// probe's puzzlement over "one reading cannot cover 102 / 70 / 82" was
/// the block length tracking the kind.
pub const IGDIMENSION_BLOCK_AREA_START: usize =
    IGDIMENSION_FRAME_PREFIX_LEN + IGDIMENSION_LINEAR_BLOCK_LEN;

/// Payload offset of the dimension value, in metres.
///
/// The one block field with a reading: 18/18 corpus values are whole
/// multiples of an inch (0.15″…4.5″), and the 35.56 mm on `D06`'s two
/// records is the `Parametric Manifold` cache's own 35.59 mm rounded.
const IGDIMENSION_VALUE_OFFSET: usize = 42;

/// Payload offset of the reference to the geometry this dimension
/// measures. Resolves 18/18 to a `0x0018 Line Object` or a
/// `0x005E Point Object` in the same storage, and the space map records
/// the edge on 40/40 of the members it indexes there.
///
/// The `u32` at `+140` is deliberately *not* a slot constant: its value
/// is 48 or 16 in every one of four independent index spaces and resolves
/// to a different class in each, so it is not a reference
/// (`2026-09-15-tag-188-members-land-in-jdim-reference-slots.md` §7).
const IGDIMENSION_MEASURED_SLOT: usize = 92;

/// Bytes one reference slot occupies: the `u32` oid and the marker word
/// behind it.
const IGDIMENSION_SLOT_LEN: usize = 6;

/// Smallest `JDim` payload this decoder will read: the frame prefix, the
/// kind-1 block, the `+82` section header, and a measured-geometry slot
/// that fits inside the main area.
pub const IGDIMENSION_MIN_PAYLOAD_LEN: usize = IGDIMENSION_MEASURED_SLOT + IGDIMENSION_SLOT_LEN;

/// The measured-geometry slot of a [`SheetIgDimensionDecoded`]: the `u32`
/// and the marker word that follows it, both verbatim.
///
/// Deliberately *not* resolved. Whether `oid` names an object of the same
/// storage is the caller's question — a decoder that answered it would
/// need the whole stream and would turn "this file is unusual" into "this
/// record is invalid". The marker is carried rather than validated for
/// the same reason: it pairs perfectly with the target class in the corpus
/// (`0x00CB` ↔ line 31/31, `0x00F0` ↔ point 8/8), but a decoder that
/// enforced the pairing would refuse the first file that names a third
/// class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetIgDimensionRef {
    /// The `u32` in the slot, as stored.
    pub oid: u32,
    /// The `u16` immediately behind it, as stored.
    pub marker: u16,
}

/// One decoded PSM `igDimension` / `JDim` record (type `0x0115`).
///
/// Carries only the fields the two analyses mark as read: the frame, the
/// dimension kind, the value, the measured-geometry slot and the group the
/// closing dword names. The rest of the record — the `+20` remap code, the
/// `+22` word the native reader stores without interpreting, the other
/// sixteen bits of `+26`, the three suspected-tolerance doubles, the
/// `+140` word that is not a reference, and the whole point/axis grammar
/// after `+82` — is either dropped or travels verbatim in
/// [`Self::raw_tail`]. None of it is guessed at.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgDimensionDecoded {
    /// Byte range covering the full PSM record (6-byte envelope +
    /// payload) within the Sheet stream.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_IGDIMENSION`].
    pub type_code: u16,
    /// Top 2 bits of the PSM type word, verbatim and never validated.
    pub type_flags: u16,
    /// `bytes_to_follow` from the PSM envelope, which the frame equation
    /// has to reproduce exactly.
    pub bytes_to_follow: u32,
    /// Object identifier from payload offset 0.
    pub oid: u32,
    /// Parent reference from payload offset 4 — the defining sheet.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this dimension sits on (payload offset 8).
    /// Always a layer named `Dimension` in the corpus, and that layer is
    /// off in every view filter set; carried, never validated.
    pub sheet_layer_ref: u32,
    /// Sub-type word at payload offset 12 (`0` across the corpus).
    /// Carried verbatim: an `== 0` rule here buys nothing and is how the
    /// `igLine2d` decoder once refused 88 real records.
    pub sub_type_word: u16,
    /// Dimension kind at payload offset 14. Always
    /// [`IGDIMENSION_KIND_LINEAR`] — the other seven are refused.
    pub kind: u16,
    /// `main_len` at payload offset 30: the bytes the record's main area
    /// spans, starting at [`IGDIMENSION_FRAME_PREFIX_LEN`].
    pub main_len: u32,
    /// Oid of the `0x0058 JDimGroup` this dimension belongs to: the
    /// closing `u32` at `34 + main_len`, present exactly when the `+26`
    /// flag word has [`IGDIMENSION_TAIL_WORD_FLAG`] set.
    ///
    /// Where the word sits is the native reader's arithmetic; what it
    /// holds is corpus evidence, cross-checked from the group's side —
    /// 13/13 resolve to a `JDimGroup` and the group's member table names
    /// the dimension back on all six groups
    /// (`2026-09-15-tag-188-members-land-in-jdim-reference-slots.md` §6).
    /// Not resolved here, for the reasons on [`SheetIgDimensionRef`].
    pub group_ref: Option<u32>,
    /// The dimension value in metres, payload offset 42.
    pub value_m: f64,
    /// Reference to the geometry this dimension measures (payload
    /// offset 92).
    pub measured: SheetIgDimensionRef,
    /// The block area verbatim: payload `+82 .. 34 + main_len`.
    ///
    /// Everything the reference slots sit in — the section count, the
    /// three point pairs per section, the axis `±1.0`, the `π/2` on the
    /// two 288-byte records — with the section grammar left unread,
    /// because the corpus shows section lengths of 102 / 70 / 82 and no
    /// single reading covers them.
    pub raw_tail: Vec<u8>,
}

/// Decode every PSM `igDimension` record in a Sheet stream's bytes.
///
/// Validation rules (all must hold, otherwise the offset is skipped):
///
/// 1. `type_code == 0x0115`;
/// 2. the dimension kind at payload `+14` is
///    [`IGDIMENSION_KIND_LINEAR`];
/// 3. the frame closes exactly:
///    `bytes_to_follow == 34 + main_len + tail`, where `tail` is 4 when
///    the `+26` flag word has [`IGDIMENSION_TAIL_WORD_FLAG`] set and 0
///    otherwise;
/// 4. the main area reaches at least [`IGDIMENSION_MIN_PAYLOAD_LEN`], so
///    the measured-geometry slot is inside it;
/// 5. the block's leading `u32` has [`IGDIMENSION_WIDE_BLOCK_FLAG`]
///    clear, i.e. the kind-1 block really is 48 bytes;
/// 6. the dimension value is finite and inside the coordinate domain.
///
/// A record that fails any of them is *refused*, not guessed at: the
/// decoder claims no bytes there, which is what lets
/// [`crate::parsers::undecoded_census::refused_record_census`] count it
/// separately from an undecoded type code once this family is registered.
///
/// After accepting a record the scanner advances past it. Panic-free and
/// bounds-checked: adversarial bytes simply fail validation.
pub fn decode_igdimensions(data: &[u8]) -> Vec<SheetIgDimensionDecoded> {
    IgDimensionDecoder.scan(data)
}

/// Try to decode a single PSM `igDimension` record starting at `offset`.
/// Returns `None` when any validation rule in [`decode_igdimensions`]
/// fails. Bounds-checked and panic-free.
///
/// Thin wrapper over [`IgDimensionDecoder::decode_at`].
pub fn decode_igdimension_at(data: &[u8], offset: usize) -> Option<SheetIgDimensionDecoded> {
    IgDimensionDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for the `igDimension` / `JDim` family
/// (PSM type `0x0115`). Validation rules are documented on
/// [`decode_igdimensions`]; no geometry is emitted for this family.
pub struct IgDimensionDecoder;

impl PsmRecordDecoder for IgDimensionDecoder {
    type Record = SheetIgDimensionDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGDIMENSION
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGDIMENSION_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgDimensionDecoded> {
        let header = parse_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGDIMENSION {
            return None;
        }
        let payload_end = header
            .body_start
            .checked_add(header.bytes_to_follow as usize)?;
        if payload_end > data.len() {
            return None;
        }
        decode_igdimension_payload(data, offset, &header, payload_end)
    }

    fn advance_of(&self, record: &SheetIgDimensionDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// Family-specific payload validation for `igDimension`: the frame
/// equation, the dimension kind, the measured-geometry slot and the
/// closing group reference.
fn decode_igdimension_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
) -> Option<SheetIgDimensionDecoded> {
    let payload = data.get(header.body_start..payload_end)?;

    let read_u16 = |pos: usize| -> Option<u16> {
        payload
            .get(pos..pos + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let read_u32 = |pos: usize| -> Option<u32> {
        payload
            .get(pos..pos + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let read_f64 = |pos: usize| -> Option<f64> {
        payload
            .get(pos..pos + 8)
            .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    };

    let kind = read_u16(14)?;
    if kind != IGDIMENSION_KIND_LINEAR {
        return None;
    }

    // The frame, as `sub_564BB990` computes it.
    let flag_word = read_u16(26)?;
    let main_len = read_u32(30)?;
    let tail_len = if flag_word & IGDIMENSION_TAIL_WORD_FLAG == 0 {
        0
    } else {
        IGDIMENSION_TAIL_WORD_LEN
    };
    let main_end = IGDIMENSION_FRAME_PREFIX_LEN.checked_add(main_len as usize)?;
    if main_end.checked_add(tail_len)? != payload.len() {
        return None;
    }
    if main_end < IGDIMENSION_MIN_PAYLOAD_LEN {
        return None;
    }

    // The block is 48 bytes only while this bit is clear; the 80-byte
    // shape has never been seen and is not guessed at.
    if read_u32(IGDIMENSION_FRAME_PREFIX_LEN)? & IGDIMENSION_WIDE_BLOCK_FLAG != 0 {
        return None;
    }

    let value_m = read_f64(IGDIMENSION_VALUE_OFFSET)?;
    if !value_m.is_finite() || value_m.abs() > GLINE2D_COORDINATE_DOMAIN_LIMIT {
        return None;
    }

    let measured = SheetIgDimensionRef {
        oid: read_u32(IGDIMENSION_MEASURED_SLOT)?,
        marker: read_u16(IGDIMENSION_MEASURED_SLOT + 4)?,
    };

    let group_ref = if tail_len == 0 {
        None
    } else {
        Some(read_u32(main_end)?)
    };
    let raw_tail = payload
        .get(IGDIMENSION_BLOCK_AREA_START..main_end)?
        .to_vec();

    Some(SheetIgDimensionDecoded {
        byte_range: offset..payload_end,
        type_code: header.type_code,
        type_flags: header.type_flags,
        bytes_to_follow: header.bytes_to_follow,
        oid: read_u32(0)?,
        parent_ref: read_u32(4)?,
        sheet_layer_ref: read_u32(8)?,
        sub_type_word: read_u16(12)?,
        kind,
        main_len,
        group_ref,
        value_m,
        measured,
        raw_tail,
    })
}

#[cfg(test)]
mod jdim_family_tests {
    use super::*;

    /// `D06.pid` `/JSite145/PSMcluster0` oid 18, all 198 payload bytes as
    /// they sit on disk — record `[1]` of
    /// `cargo run --example probe_jdim_bytes`. Flags `0x0351`,
    /// `main_len` 160, so the frame is `34 + 160 + 4 = 198`; the value is
    /// 35.56 mm, the measured slot names line 56 and the closing word
    /// names `JDimGroup` 28. The `48` at `+140` is the constant that is
    /// not a reference.
    ///
    /// Sixteen bytes to the row, so a row here is a row of that dump.
    #[rustfmt::skip]
    const CANONICAL_JDIM_PAYLOAD: [u8; 198] = [
        // +0    oid 18        parent 15     layer 35      sub_type  kind
        0x12, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        // +16                              +22 = 6       flags 0x0351  main_len 160
        0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x51, 0x03, 0x00, 0x00, 0xA0, 0x00,
        // +32   the kind-1 block opens at +34; the value starts at +42
        0x00, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3F, 0xD8, 0x6D, 0x17, 0x9A, 0xEB, 0x34,
        0xA2, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3F, 0xBF, 0x0E, 0x1C, 0x3C, 0x00, 0x00,
        0x40, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // +80   the block ends at +82; section count 1; measured slot at +92
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x38, 0x00, 0x00, 0x00,
        // +96   marker 0x00CB -- the line class
        0xCB, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA8, 0xE8, 0x48, 0x2E,
        0xFF, 0x21, 0xC5, 0x3F, 0x87, 0x3D, 0xED, 0xF0, 0xD7, 0x64, 0xC1, 0x3F, 0xA8, 0xE8, 0x48, 0x2E,
        // +128                                                      +140 = 48, not a reference
        0xFF, 0x21, 0xC5, 0x3F, 0x87, 0x3D, 0xED, 0xF0, 0xD7, 0x64, 0xC1, 0x3F, 0x30, 0x00, 0x00, 0x00,
        // +144  0x0067 behind it -- the assoc element list code, unread
        0x67, 0x00, 0xA8, 0xE8, 0x48, 0x2E, 0xFF, 0x21, 0xC5, 0x3F, 0x87, 0x3D, 0xED, 0xF0, 0xD7, 0x64,
        0xC1, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xF0, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xDA, 0x6D, 0x17, 0x9A, 0xEB, 0x34,
        // +192  the main area ends at +194, then the closing word
        0xB2, 0x3F, 0x1C, 0x00, 0x00, 0x00,
    ];

    /// Wrap a payload in the 6-byte PSM envelope the type code writes.
    fn record(payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(PSM_ENVELOPE_LEN + payload.len());
        out.extend_from_slice(&PSM_TYPE_CODE_IGDIMENSION.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    /// The 8-byte stream header the chain walk skips, then the records.
    fn chain(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; SHEET_STREAM_HEADER_LEN];
        for bytes in records {
            out.extend_from_slice(bytes);
        }
        out
    }

    fn canonical() -> Vec<u8> {
        record(&CANONICAL_JDIM_PAYLOAD)
    }

    /// The canonical payload with one field rewritten, still wrapped as a
    /// record — the frame is left to close or not on its own.
    fn canonical_with(patch: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut payload = CANONICAL_JDIM_PAYLOAD.to_vec();
        patch(&mut payload);
        record(&payload)
    }

    /// The canonical record resized so its main area spans `main_len`
    /// bytes, with the tail word kept or dropped to match `flag_word`.
    /// The frame still closes, so only the resizing is under test.
    fn canonical_resized(main_len: usize, flag_word: u16) -> Vec<u8> {
        let mut payload = CANONICAL_JDIM_PAYLOAD.to_vec();
        payload[26..28].copy_from_slice(&flag_word.to_le_bytes());
        payload[30..34].copy_from_slice(&(main_len as u32).to_le_bytes());
        payload.resize(IGDIMENSION_FRAME_PREFIX_LEN + main_len, 0);
        if flag_word & IGDIMENSION_TAIL_WORD_FLAG != 0 {
            payload.extend_from_slice(&28u32.to_le_bytes());
        }
        record(&payload)
    }

    #[test]
    fn a_real_jdim_reads_its_frame_value_measured_slot_and_group() {
        let stream = chain(&[canonical()]);
        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 1);
        let jdim = &decoded[0];

        assert_eq!(jdim.byte_range, SHEET_STREAM_HEADER_LEN..stream.len());
        assert_eq!(jdim.type_code, PSM_TYPE_CODE_IGDIMENSION);
        assert_eq!(jdim.type_flags, 0);
        assert_eq!(jdim.bytes_to_follow, 198);
        assert_eq!(jdim.oid, 18);
        assert_eq!(jdim.parent_ref, 15);
        assert_eq!(jdim.sheet_layer_ref, 35);
        assert_eq!(jdim.sub_type_word, 0);
        assert_eq!(jdim.kind, IGDIMENSION_KIND_LINEAR);
        assert_eq!(jdim.main_len, 160);
        assert_eq!(
            jdim.group_ref,
            Some(28),
            "the JDimGroup whose member table lists 18, 19 and 24"
        );

        // 35.56 mm, which is 1.4 inches.
        assert!((jdim.value_m - 0.035_56).abs() < 1e-12, "{}", jdim.value_m);

        assert_eq!(
            jdim.measured,
            SheetIgDimensionRef {
                oid: 56,
                marker: 0x00CB
            },
            "the line this dimension measures"
        );

        // `+82 .. 34 + 160`, verbatim and nothing more — the `48` at
        // `+140` is in here and nowhere else.
        assert_eq!(jdim.raw_tail.len(), 194 - IGDIMENSION_BLOCK_AREA_START);
        assert_eq!(
            jdim.raw_tail.as_slice(),
            &CANONICAL_JDIM_PAYLOAD[IGDIMENSION_BLOCK_AREA_START..194]
        );
        let plus_140 = 140 - IGDIMENSION_BLOCK_AREA_START;
        assert_eq!(&jdim.raw_tail[plus_140..plus_140 + 4], &48u32.to_le_bytes());
    }

    #[test]
    fn a_frame_that_does_not_close_is_not_a_dimension() {
        // `main_len` one byte past what the payload can hold.
        for main_len in [159u32, 161] {
            let stream = chain(&[canonical_with(|payload| {
                payload[30..34].copy_from_slice(&main_len.to_le_bytes());
            })]);
            assert!(
                decode_igdimensions(&stream).is_empty(),
                "main_len {main_len} closes a 198-byte payload"
            );
        }
    }

    #[test]
    fn a_dimension_kind_other_than_linear_is_refused_not_guessed() {
        for kind in [0u16, 2, 3, 4, 5, 6, 7, 8] {
            let stream = chain(&[canonical_with(|payload| {
                payload[14..16].copy_from_slice(&kind.to_le_bytes());
            })]);
            assert!(
                decode_igdimensions(&stream).is_empty(),
                "kind {kind} has a block length nobody has bytes for"
            );
        }
    }

    #[test]
    fn a_wide_block_head_is_refused_because_the_block_is_no_longer_48_bytes() {
        let stream = chain(&[canonical_with(|payload| {
            let head = u32::from_le_bytes([payload[34], payload[35], payload[36], payload[37]]);
            payload[34..38].copy_from_slice(&(head | IGDIMENSION_WIDE_BLOCK_FLAG).to_le_bytes());
        })]);
        assert!(decode_igdimensions(&stream).is_empty());
    }

    #[test]
    fn a_main_area_too_short_for_the_measured_slot_is_refused() {
        // 34 + 63 = 97: one byte short of the slot's last byte. Read the
        // offset directly — a record this short is below the scan's
        // minimum length, so a `scan` that found nothing would prove
        // nothing about the rule.
        let stream = chain(&[canonical_resized(63, 0x0051)]);
        assert!(decode_igdimension_at(&stream, SHEET_STREAM_HEADER_LEN).is_none());
        // 34 + 64 = 98 is exactly enough, so the floor is a floor and not
        // an off-by-one.
        let stream = chain(&[canonical_resized(64, 0x0051)]);
        assert!(decode_igdimension_at(&stream, SHEET_STREAM_HEADER_LEN).is_some());
        assert_eq!(decode_igdimensions(&stream).len(), 1);
    }

    #[test]
    fn a_non_finite_or_out_of_domain_value_is_refused() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e10] {
            let stream = chain(&[canonical_with(|payload| {
                payload[42..50].copy_from_slice(&value.to_le_bytes());
            })]);
            assert!(
                decode_igdimensions(&stream).is_empty(),
                "value {value} is not a length"
            );
        }
    }

    #[test]
    fn the_group_reference_follows_bit_0x0100_and_not_bit_0x0200() {
        // `0x0051` is a corpus flag word with neither bit: no tail, and
        // so no group — the three 194-byte records of `DWG-0201` and
        // `A01` are exactly the ones no `JDimGroup` lists.
        let stream = chain(&[canonical_resized(160, 0x0051)]);
        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].group_ref, None);
        assert_eq!(decoded[0].bytes_to_follow, 194);

        // `0x0200` alone still means no tail — the corpus cannot separate
        // the two bits, `sub_564BB990` can.
        let stream = chain(&[canonical_resized(160, 0x0251)]);
        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].group_ref, None);
    }

    #[test]
    fn a_main_area_that_stops_short_of_plus_140_still_decodes() {
        // 34 + 111 = 145: the measured slot fits, the `+140` word does
        // not. Nothing depends on it, so the record decodes and the raw
        // tail simply ends where the main area does.
        let stream = chain(&[canonical_resized(111, 0x0051)]);
        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].measured.oid, 56);
        assert_eq!(
            decoded[0].raw_tail.len(),
            145 - IGDIMENSION_BLOCK_AREA_START
        );
    }

    #[test]
    fn a_refused_dimension_leaves_its_own_offset_unclaimed() {
        // One readable record, then one of a kind this decoder refuses.
        // The refusal has to show as *no claim starting there*, which is
        // what separates a refused record from an undecoded type code in
        // the census.
        let good = canonical();
        let refused = canonical_with(|payload| {
            payload[14..16].copy_from_slice(&7u16.to_le_bytes());
        });
        let refused_start = SHEET_STREAM_HEADER_LEN + good.len();
        let stream = chain(&[good, refused]);

        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].byte_range.start, SHEET_STREAM_HEADER_LEN);
        assert!(
            !decoded
                .iter()
                .any(|record| record.byte_range.start == refused_start),
            "a refused kind must not be claimed by this family"
        );
        assert!(decode_igdimension_at(&stream, refused_start).is_none());
    }

    #[test]
    fn a_wrong_type_code_is_not_a_dimension() {
        let mut stream = chain(&[canonical()]);
        stream[SHEET_STREAM_HEADER_LEN] = 0x18; // igLine2d instead
        assert!(decode_igdimensions(&stream).is_empty());
    }

    #[test]
    fn two_back_to_back_dimensions_both_decode() {
        let second = canonical_with(|payload| {
            payload[0..4].copy_from_slice(&21u32.to_le_bytes());
        });
        let stream = chain(&[canonical(), second]);
        let decoded = decode_igdimensions(&stream);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 18);
        assert_eq!(decoded[1].oid, 21);
        assert_eq!(decoded[0].byte_range.end, decoded[1].byte_range.start);
    }

    #[test]
    fn the_decoder_survives_truncation_and_noise() {
        let stream = chain(&[canonical()]);
        for cut in 0..stream.len() {
            assert!(
                decode_igdimensions(&stream[..cut]).is_empty(),
                "a truncated record must not decode at cut {cut}"
            );
            let _ = decode_igdimension_at(&stream[..cut], cut.saturating_sub(1));
        }
        assert!(decode_igdimensions(&[]).is_empty());
        assert!(decode_igdimension_at(&stream, stream.len()).is_none());
        assert!(decode_igdimension_at(&stream, usize::MAX).is_none());

        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igdimensions(&noise);
        assert!(decode_igdimensions(&vec![0u8; 4096]).is_empty());
        assert!(decode_igdimensions(&vec![0xFFu8; 4096]).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions};

    #[test]
    fn inventory_collects_marker_field_text_and_coordinate_evidence() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x89, 0xCE, 0x00, 0xAA]);
        data.extend_from_slice(b"TAG-101");
        data.resize(32, 0);
        data.extend_from_slice(&12345_i32.to_le_bytes());
        data.extend_from_slice(&67890_i32.to_le_bytes());
        data.resize(64, 0);
        data.extend_from_slice(&0.125_f64.to_le_bytes());
        data.extend_from_slice(&0.25_f64.to_le_bytes());
        data.extend_from_slice(&[0x5E, 0x00, 0x22, 0x00, 0x00, 0x00]);
        data.extend_from_slice(&630_u32.to_le_bytes());

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[630]);

        assert!(
            inventory.records.iter().any(|record| {
                record.kind == SheetRecordShapeKind::Marker && record.marker_type == Some(0x00CE)
            }),
            "expected marker type evidence"
        );
        assert!(
            inventory.records.iter().any(|record| {
                record.kind == SheetRecordShapeKind::FieldXWindow
                    && record.field_x == Some(630)
                    && record.f64_coordinate_offset.is_some()
            }),
            "expected field_x evidence with f64 coordinate source"
        );
        assert!(
            inventory
                .records
                .iter()
                .any(|record| record.kind == SheetRecordShapeKind::TextRun),
            "expected text-run evidence"
        );
        assert!(
            inventory
                .records
                .iter()
                .any(|record| record.kind == SheetRecordShapeKind::CoordinateHint),
            "expected coordinate-hint evidence"
        );
        assert!(
            inventory
                .records
                .iter()
                .all(|record| record.range_start <= record.offset
                    && record.offset < record.range_end
                    && record.range_end <= data.len()),
            "all evidence ranges should be bounded"
        );
    }

    #[test]
    fn primitive_line_investigation_groups_marker_numeric_shapes() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x89, 0x10, 0x00, 0x00]);
        data.extend_from_slice(&1000_i32.to_le_bytes());
        data.extend_from_slice(&2000_i32.to_le_bytes());
        data.extend_from_slice(&3000_i32.to_le_bytes());
        data.extend_from_slice(&4000_i32.to_le_bytes());
        data.extend_from_slice(&[0x89, 0x10, 0x00, 0x00]);
        data.extend_from_slice(&5000_i32.to_le_bytes());
        data.extend_from_slice(&6000_i32.to_le_bytes());
        data.extend_from_slice(&7000_i32.to_le_bytes());
        data.extend_from_slice(&8000_i32.to_le_bytes());

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[]);
        let report = primitive_line_investigation_report(&data, &inventory);

        assert!(
            report.groups.iter().any(|group| {
                group.marker_type == Some(0x0010)
                    && group.support == 2
                    && group.candidate_i32_pairs >= 2
                    && group.investigation_score > 0
                    && group
                        .investigation_notes
                        .iter()
                        .any(|note| note == "bounded_compact_range")
                    && !group.numeric_samples.is_empty()
            }),
            "expected repeated marker range with plausible coordinate pairs: {:?}",
            report.groups
        );
        assert_eq!(
            report.groups.first().and_then(|group| group.marker_type),
            Some(0x0010),
            "repeated compact numeric shape should be ranked first: {:?}",
            report.groups
        );
        let top_group = report
            .groups
            .first()
            .expect("expected repeated compact numeric group");
        assert_eq!(
            top_group.numeric_sample_relative_offsets,
            vec![4, 8, 12],
            "numeric sample offsets should be range-relative: {top_group:?}"
        );
        assert_eq!(
            top_group.numeric_sample_offset_deltas,
            vec![4, 4],
            "numeric sample offset deltas should expose candidate field spacing: {top_group:?}"
        );
        assert_eq!(
            top_group.coordinate_hint_match_offsets,
            vec![4, 8, 12],
            "numeric samples should record exact matches to existing coordinate hints: {top_group:?}"
        );
        assert_eq!(
            top_group.nearest_coordinate_hint_delta,
            Some(0),
            "exact coordinate hint matches should produce zero nearest delta: {top_group:?}"
        );
        assert!(
            top_group.example_hex_prefix.starts_with("89 10 00 00"),
            "hex prefix should dump the marker range start: {top_group:?}"
        );
    }

    #[test]
    fn curve_primitive_investigation_classifies_vertex_like_marker_payloads() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x89, 0x30, 0x00, 0x00]);
        for (x, y) in [(1000_i32, 2000_i32), (3000, 4000), (5000, 6000)] {
            data.extend_from_slice(&x.to_le_bytes());
            data.extend_from_slice(&y.to_le_bytes());
        }
        data.extend_from_slice(&[0x89, 0x31, 0x00, 0x00]);

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[]);
        let report = curve_primitive_investigation_report(&data, &inventory);

        let top = report
            .groups
            .first()
            .expect("expected curve primitive investigation group");
        assert_eq!(top.marker_type, Some(0x0030));
        assert_eq!(
            top.candidate_kind,
            SheetCurvePrimitiveCandidateKind::PolylineLike
        );
        assert!(top.compact_vertex_chain_candidate);
        assert_eq!(top.numeric_pair_count, 5);
        assert!(top.numeric_pairs_per_1kb > 0);
        assert!(top
            .investigation_notes
            .iter()
            .any(|note| note == "compact_vertex_chain_candidate"));
        let sequence = top
            .i32_point_sequence
            .as_ref()
            .expect("compact vertex candidate should expose non-overlapping i32 points");
        assert_eq!(sequence.point_count, 3);
        assert_eq!(sequence.byte_stride, 8);
        assert_eq!(sequence.relative_alignment_mod4, 0);
        assert_eq!(
            sequence.sample_points,
            vec!["(1000, 2000)", "(3000, 4000)", "(5000, 6000)"]
        );
        assert!(top
            .investigation_notes
            .iter()
            .any(|note| note == "i32_point_sequence_points=3"));
        assert!(top
            .investigation_notes
            .iter()
            .any(|note| note == "probe_only_no_curve_geometry_promotion"));
        assert!(!top.numeric_sample_relative_offsets.is_empty());
        assert!(top.example_hex_prefix.starts_with("89 30 00 00"));
    }

    #[test]
    fn text_placement_investigation_links_text_coordinate_and_field_x_without_promotion() {
        let mut data = vec![0; 8];
        let text_offset = data.len();
        data.extend_from_slice(b"PUMP-101");
        data.resize(32, 0);
        let coordinate_offset = data.len();
        data.extend_from_slice(&1200_i32.to_le_bytes());
        data.extend_from_slice(&(-450_i32).to_le_bytes());
        data.resize(48, 0);
        let field_x_offset = data.len();
        data.extend_from_slice(&630_u32.to_le_bytes());

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[630]);
        let report = text_placement_investigation_report(&data, &probe, &inventory, 64);

        assert!(report.raw_candidate_count >= 1);
        assert_eq!(
            report.rejected_candidate_count,
            report
                .raw_candidate_count
                .saturating_sub(report.candidates.len())
        );
        let candidate = report
            .candidates
            .iter()
            .find(|candidate| candidate.text == "PUMP-101")
            .expect("expected text placement investigation candidate");
        assert_eq!(candidate.text_offset, text_offset);
        assert_eq!(candidate.coordinate_offset, coordinate_offset);
        assert_eq!(candidate.nearest_field_x, Some(630));
        assert_eq!(candidate.nearest_field_x_offset, Some(field_x_offset));
        assert_eq!(
            candidate.field_x_delta_from_coordinate,
            Some(signed_delta(field_x_offset, coordinate_offset))
        );
        assert!(candidate.text_hex.starts_with("50 55 4D 50"));
        assert!(candidate
            .notes
            .iter()
            .any(|note| note == "probe_only_no_text_geometry_promotion"));
    }

    #[test]
    fn coordinate_page_metadata_investigation_reports_domain_without_promotion() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x89, 0x40, 0x00, 0x00]);
        data.extend_from_slice(&0.25_f64.to_le_bytes());
        data.extend_from_slice(&0.5_f64.to_le_bytes());
        data.extend_from_slice(&594.0_f64.to_le_bytes());
        data.extend_from_slice(&420.0_f64.to_le_bytes());
        data.extend_from_slice(&[0x89, 0x41, 0x00, 0x00]);
        data.extend_from_slice(&1200_i32.to_le_bytes());
        data.extend_from_slice(&3400_i32.to_le_bytes());

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[]);
        let report =
            coordinate_page_metadata_investigation_report(&data, &inventory, Some((594.0, 420.0)));

        let top = report
            .candidates
            .first()
            .expect("expected coordinate page metadata investigation candidate");
        assert_eq!(top.marker_type, Some(0x0040));
        assert_eq!(
            top.candidate_kind,
            SheetCoordinatePageMetadataCandidateKind::PageDimensionScalarLike
        );
        assert!(top.normalized_f64_pairs > 0);
        assert!(top.page_dimension_scalar_matches >= 2);
        assert!(top.example_hex_prefix.starts_with("89 40 00 00"));
        assert!(top
            .investigation_notes
            .iter()
            .any(|note| { note == "probe_only_no_coordinate_page_metadata_promotion" }));
        assert!(
            report
                .coordinate_hint_bounds
                .is_some_and(|bounds| bounds.count > 0),
            "i32 probe coordinate bounds should stay evidence-only: {report:?}"
        );
        assert_eq!(
            report
                .f64_coordinate_bounds
                .map(|bounds| bounds.count)
                .unwrap_or_default(),
            0,
            "standalone marker f64 pairs are not object-linked f64 coordinate bounds"
        );
    }

    #[test]
    fn symbol_placement_investigation_links_symbol_object_to_field_x_without_promotion() {
        let mut data = vec![0; 16];
        let coordinate_offset = data.len();
        data.extend_from_slice(&1200_i32.to_le_bytes());
        data.extend_from_slice(&(-450_i32).to_le_bytes());
        data.resize(48, 0);
        let field_x_offset = data.len();
        data.extend_from_slice(&630_u32.to_le_bytes());

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[630]);
        let objects = vec![SheetSymbolPlacementObject {
            field_x: 630,
            drawing_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            item_type: "Equipment".to_string(),
            drawing_item_type: Some("Symbol".to_string()),
            symbol_path: Some(r"\\srv\symbols\Pump.sym".to_string()),
        }];
        let symbol_paths = vec![r"\\srv\symbols\Pump.sym".to_string()];

        let report =
            symbol_placement_investigation_report(&data, &inventory, &objects, &symbol_paths);

        let candidate = report
            .candidates
            .first()
            .expect("expected symbol placement investigation candidate");
        assert_eq!(report.symbol_path_catalog_count, 1);
        assert_eq!(candidate.field_x, 630);
        assert_eq!(candidate.field_x_offset, Some(field_x_offset));
        assert_eq!(candidate.position_offset, Some(coordinate_offset));
        assert_eq!(candidate.x, Some(1200.0));
        assert_eq!(candidate.y, Some(-450.0));
        assert_eq!(
            candidate.position_encoding,
            Some(SheetSymbolPlacementPositionEncoding::I32Pair)
        );
        assert_eq!(
            candidate.symbol_path.as_deref(),
            Some(r"\\srv\symbols\Pump.sym")
        );
        assert!(candidate
            .notes
            .iter()
            .any(|note| note == "object_symbol_path_bound"));
        assert!(candidate
            .notes
            .iter()
            .any(|note| note == "probe_only_no_symbol_geometry_promotion"));
    }

    #[test]
    fn primitive_line_numeric_samples_reject_near_zero_f64_noise() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x89, 0x20, 0x00, 0x00]);
        data.extend_from_slice(&1.0e-12_f64.to_le_bytes());
        data.extend_from_slice(&2.0e-12_f64.to_le_bytes());
        data.extend_from_slice(&0.25_f64.to_le_bytes());
        data.extend_from_slice(&0.5_f64.to_le_bytes());
        data.extend_from_slice(&[0x89, 0x21, 0x00, 0x00]);

        let probe = probe_sheet_stream("Sheet6", "/Sheet6", &data, &SheetProbeOptions::default());
        let inventory = sheet_record_shape_inventory(&data, &probe, &[]);
        let report = primitive_line_investigation_report(&data, &inventory);

        let samples = report
            .groups
            .iter()
            .flat_map(|group| group.numeric_samples.iter())
            .filter(|sample| sample.kind == SheetPrimitiveLineNumericSampleKind::F64Pair)
            .map(|sample| sample.value.as_str())
            .collect::<Vec<_>>();
        assert!(
            samples.contains(&"(0.250000, 0.500000)"),
            "expected finite non-zero f64 coordinate sample: {samples:?}"
        );
        assert!(
            samples.iter().all(|value| *value != "(0.000000, 0.000000)"),
            "near-zero f64 noise should not be sampled: {samples:?}"
        );
        assert!(
            report.groups.iter().any(|group| {
                group.numeric_samples.iter().any(|sample| {
                    sample.offset == 20
                        && sample.kind == SheetPrimitiveLineNumericSampleKind::F64Pair
                })
            }),
            "f64 samples should report byte offsets, not window indices: {:?}",
            report.groups
        );
        assert!(
            report
                .groups
                .iter()
                .any(|group| { group.numeric_sample_relative_offsets.contains(&20) }),
            "f64 sample relative offsets should preserve byte alignment: {:?}",
            report.groups
        );
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice D: PSM GLine2d decoder tests
    // -----------------------------------------------------------------

    /// Bytes a `GLine2d` record declares after the 6-byte envelope when it
    /// carries nothing but its own body: the 12 header bytes the envelope
    /// does not cover (`oid` + aux) plus the 48-byte payload.
    ///
    /// Declaring 48 here instead would make the record claim to end 12 bytes
    /// before its own payload does — harmless to a scan, but it is not a
    /// record a chain walk can step over, so the fixture would no longer
    /// model a real stream.
    const SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW: usize =
        PSM_RECORD_HEADER_LEN - PSM_ENVELOPE_LEN + GLINE2D_PAYLOAD_LEN;

    /// Build a single synthetic PSM `GLine2d` record:
    /// 18-byte header (type=0x3FE6, oid=`oid`) + 6×f64 `GLine2d` payload
    /// (origin, direction, params).
    fn build_synthetic_gline2d_record(
        oid: u32,
        origin: (f64, f64),
        direction: (f64, f64),
        param_start: f64,
        param_end: f64,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN);
        // type_code (14-bit) at bits 0..14 of the LE u16, top 2 bits = 0 flags.
        let type_word: u16 = PSM_TYPE_CODE_GLINE2D;
        out.extend_from_slice(&type_word.to_le_bytes());
        out.extend_from_slice(&(SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW as u32).to_le_bytes());
        // oid
        out.extend_from_slice(&oid.to_le_bytes());
        // 8-byte aux (set to fixed pattern for inspection).
        out.extend_from_slice(&[0u8; 8]);
        // 6 doubles
        for v in [
            origin.0,
            origin.1,
            direction.0,
            direction.1,
            param_start,
            param_end,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Wrap synthetic records into a `Sheet*` stream: the 8-byte stream
    /// header, then the records nose to tail.
    ///
    /// [`decode_primitive_lines`] only considers offsets that are members of
    /// the stream's record chain, so a bare record with no stream header
    /// decodes as nothing. Tests that hand it loose bytes would pass for the
    /// wrong reason — which is the exact failure mode this gate exists to
    /// stop — so they go through here. The walk does not inspect the header
    /// bytes, only their length.
    fn synthetic_sheet_stream(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; SHEET_STREAM_HEADER_LEN];
        for record in records {
            out.extend_from_slice(record);
        }
        out
    }

    #[test]
    fn primitive_line_decodes_canonical_synthetic_record() {
        // Canonical synthetic line: origin at (0,0), unit horizontal
        // direction, params [0, 1.0]. Endpoints A=(0,0) B=(1,0).
        let record = build_synthetic_gline2d_record(42, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        let stream = synthetic_sheet_stream(&[record]);
        let decoded = decode_primitive_lines(&stream);
        assert_eq!(decoded.len(), 1, "expected exactly one decoded line");
        let line = &decoded[0];
        assert_eq!(line.type_code, PSM_TYPE_CODE_GLINE2D);
        assert_eq!(line.type_flags, 0);
        assert_eq!(
            line.bytes_to_follow as usize,
            SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW
        );
        assert_eq!(line.oid, 42);
        assert_eq!(line.origin, (0.0, 0.0));
        assert!((line.direction.0 - 1.0).abs() < 1e-12);
        assert!(line.direction.1.abs() < 1e-12);
        assert_eq!(line.param_start, 0.0);
        assert_eq!(line.param_end, 1.0);
        assert_eq!(line.byte_range.start, SHEET_STREAM_HEADER_LEN);
        // byte_range covers the envelope plus everything it declares.
        assert_eq!(
            line.byte_range.end,
            SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN + SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW
        );
        // Endpoint helpers.
        assert_eq!(line.endpoint_a(), (0.0, 0.0));
        let (bx, by) = line.endpoint_b();
        assert!((bx - 1.0).abs() < 1e-12);
        assert!(by.abs() < 1e-12);
    }

    #[test]
    fn primitive_line_rejects_wrong_type_code() {
        // Same bytes but with a non-GLine2d type code.
        let mut record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        // Overwrite type with 0x1234 (not the GLine2d type).
        record[0] = 0x34;
        record[1] = 0x12;
        let decoded = decode_primitive_lines(&synthetic_sheet_stream(&[record]));
        assert!(decoded.is_empty(), "wrong type_code must be rejected");
    }

    #[test]
    fn primitive_line_rejects_non_unit_direction() {
        // direction = (2.0, 0.0): length 2, not unit.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (2.0, 0.0), 0.0, 1.0);
        assert!(
            decode_primitive_lines(&synthetic_sheet_stream(&[record])).is_empty(),
            "non-unit direction vector must be rejected"
        );
    }

    #[test]
    fn primitive_line_rejects_zero_direction() {
        // direction = (0.0, 0.0): zero vector.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (0.0, 0.0), 0.0, 1.0);
        assert!(
            decode_primitive_lines(&synthetic_sheet_stream(&[record])).is_empty(),
            "zero direction vector must be rejected"
        );
    }

    #[test]
    fn primitive_line_rejects_reversed_param_range() {
        // param_start == param_end.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.0, 1.0);
        assert!(
            decode_primitive_lines(&synthetic_sheet_stream(&[record])).is_empty(),
            "param_start >= param_end must be rejected"
        );
        // param_start > param_end.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.5, 0.5);
        assert!(
            decode_primitive_lines(&synthetic_sheet_stream(&[record])).is_empty(),
            "reversed param range must be rejected"
        );
    }

    #[test]
    fn primitive_line_rejects_nan_coordinate() {
        let mut record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        // Overwrite origin.x with NaN bytes (any non-finite).
        let nan_bytes = f64::NAN.to_le_bytes();
        let origin_off = PSM_RECORD_HEADER_LEN;
        record[origin_off..origin_off + 8].copy_from_slice(&nan_bytes);
        assert!(
            decode_primitive_lines(&synthetic_sheet_stream(&[record])).is_empty(),
            "NaN coordinate must be rejected"
        );
    }

    /// A payload that satisfies every `GLine2d` rule is still not a record
    /// when it sits inside another one.
    ///
    /// This is the corpus failure in miniature: three "records" that passed
    /// all five payload rules while living 160 bytes inside an
    /// `igSmartFrame2d`. Chain membership is what rejects them, so it needs
    /// a test that fails if the gate is ever removed.
    #[test]
    fn primitive_line_inside_another_records_payload_is_not_a_record() {
        let hidden = build_synthetic_gline2d_record(99, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        // One record of some other family whose payload happens to contain a
        // perfectly well-formed GLine2d.
        let mut host = Vec::new();
        host.extend_from_slice(&PSM_TYPE_CODE_IGLINE2D.to_le_bytes());
        host.extend_from_slice(&(hidden.len() as u32).to_le_bytes());
        host.extend_from_slice(&hidden);

        let stream = synthetic_sheet_stream(&[host]);
        assert!(
            decode_primitive_line_at(&stream, SHEET_STREAM_HEADER_LEN + 6).is_some(),
            "the buried bytes do satisfy every payload rule; without that the \
             test would prove nothing"
        );
        assert!(
            decode_primitive_lines(&stream).is_empty(),
            "a payload-shaped match inside another record is not a record"
        );
    }

    #[test]
    fn primitive_line_decoder_is_panic_safe_on_truncated_input() {
        // Build a complete stream, then truncate at various sizes.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        let stream = synthetic_sheet_stream(&[record]);
        for trunc_len in 0..stream.len() {
            // Must not panic, must return empty / no decoded line.
            let decoded = decode_primitive_lines(&stream[..trunc_len]);
            assert!(
                decoded.is_empty(),
                "truncated input of length {trunc_len} must not decode anything"
            );
        }
        // Empty input also fine.
        assert!(decode_primitive_lines(&[]).is_empty());
    }

    #[test]
    fn primitive_line_decoder_is_panic_safe_on_random_noise() {
        // Adversarial deterministic noise: incrementing bytes.
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        // Just running without panic is the test; whatever it decodes
        // is acceptable (must be `Vec`, not panic).
        let _decoded = decode_primitive_lines(&noise);
        // All-zeros: nothing valid.
        assert!(decode_primitive_lines(&vec![0u8; 4096]).is_empty());
        // All-0xFF: nothing valid (type_code = 0x3FFF != GLine2d).
        assert!(decode_primitive_lines(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn primitive_line_decodes_two_back_to_back_records() {
        let data = synthetic_sheet_stream(&[
            build_synthetic_gline2d_record(7, (0.10, 0.10), (1.0, 0.0), 0.0, 0.5),
            build_synthetic_gline2d_record(8, (0.20, 0.30), (0.0, 1.0), 0.0, 0.8),
        ]);
        let decoded = decode_primitive_lines(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 7);
        assert_eq!(decoded[1].oid, 8);
        // The two records must have non-overlapping byte ranges.
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice J: PSM igLine2d decoder tests
    // -----------------------------------------------------------------

    /// Build a synthetic PSM `igLine2d` record (56 bytes total):
    /// 6-byte PSM header + 50-byte payload, with the `aux_hi` most
    /// corpus records carry.
    fn build_synthetic_igline2d_record(
        oid: u32,
        parent_ref: u32,
        sub_type_word: u16,
        index: u32,
        start: (f64, f64),
        end: (f64, f64),
    ) -> Vec<u8> {
        build_synthetic_igline2d_record_with_aux_hi(
            oid,
            parent_ref,
            12,
            sub_type_word,
            index,
            start,
            end,
        )
    }

    /// Same, with `aux_hi` chosen by the caller — the field the decoder
    /// used to demand equal `12`.
    fn build_synthetic_igline2d_record_with_aux_hi(
        oid: u32,
        parent_ref: u32,
        aux_hi: u32,
        sub_type_word: u16,
        index: u32,
        start: (f64, f64),
        end: (f64, f64),
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + IGLINE2D_PAYLOAD_LEN);
        out.extend_from_slice(&PSM_TYPE_CODE_IGLINE2D.to_le_bytes());
        out.extend_from_slice(&(IGLINE2D_PAYLOAD_LEN as u32).to_le_bytes());
        // Payload starts here.
        out.extend_from_slice(&oid.to_le_bytes());
        out.extend_from_slice(&parent_ref.to_le_bytes());
        out.extend_from_slice(&aux_hi.to_le_bytes());
        out.extend_from_slice(&sub_type_word.to_le_bytes());
        out.extend_from_slice(&index.to_le_bytes());
        out.extend_from_slice(&start.0.to_le_bytes());
        out.extend_from_slice(&start.1.to_le_bytes());
        out.extend_from_slice(&end.0.to_le_bytes());
        out.extend_from_slice(&end.1.to_le_bytes());
        out
    }

    #[test]
    fn igline2d_decodes_canonical_horizontal_segment() {
        let record = build_synthetic_igline2d_record(
            177,
            1212,
            0x0010,
            86,
            (0.4719, 0.3897),
            (0.5736, 0.3897),
        );
        let decoded = decode_iglines(&synthetic_sheet_stream(&[record]));
        assert_eq!(decoded.len(), 1);
        let line = &decoded[0];
        assert_eq!(line.type_code, PSM_TYPE_CODE_IGLINE2D);
        assert_eq!(line.bytes_to_follow, 50);
        assert_eq!(line.oid, 177);
        assert_eq!(line.parent_ref, 1212);
        assert_eq!(line.aux_hi, 12);
        assert_eq!(line.sub_type_word, 0x0010);
        assert_eq!(line.index, 86);
        assert!((line.start.0 - 0.4719).abs() < 1e-9);
        assert!((line.start.1 - 0.3897).abs() < 1e-9);
        assert!((line.end.0 - 0.5736).abs() < 1e-9);
        assert!((line.end.1 - 0.3897).abs() < 1e-9);
        assert_eq!(line.byte_range.start, SHEET_STREAM_HEADER_LEN);
        assert_eq!(line.byte_range.end, SHEET_STREAM_HEADER_LEN + 6 + 50);
        assert!((line.length() - 0.1017).abs() < 1e-3);
    }

    #[test]
    fn igline2d_rejects_wrong_type_code() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        record[0] = 0xE6;
        record[1] = 0x3F;
        assert!(decode_igline_at(&record, 0).is_none());
    }

    #[test]
    fn igline2d_rejects_wrong_bytes_to_follow() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        // Overwrite bytes_to_follow with 49 (not exactly 50).
        record[2] = 49;
        assert!(decode_igline_at(&record, 0).is_none());
    }

    /// `aux_hi` (payload `+8`) is PSM envelope bookkeeping the native
    /// reader reads and discards, so no value of it makes a record less
    /// of a record. The decoder demanded `12` for two phases and dropped
    /// 88 real lines — `8` is what `A01`'s page border carries and `6996`
    /// what `DWG-0202/Sheet6615` carries.
    #[test]
    fn igline2d_accepts_every_aux_hi_the_corpus_carries() {
        for aux_hi in [0u32, 8, 12, 6996, u32::MAX] {
            let record = build_synthetic_igline2d_record_with_aux_hi(
                1,
                1,
                aux_hi,
                0x10,
                0,
                (0.0, 0.0),
                (1.0, 0.0),
            );
            let decoded =
                decode_igline_at(&record, 0).unwrap_or_else(|| panic!("aux_hi {aux_hi} refused"));
            assert_eq!(decoded.aux_hi, aux_hi, "carried through verbatim");
            assert_eq!(decoded.start, (0.0, 0.0));
            assert_eq!(decoded.end, (1.0, 0.0));
        }
    }

    #[test]
    fn igline2d_rejects_degenerate_zero_length_line() {
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.5, 0.5), (0.5, 0.5));
        assert!(decode_igline_at(&record, 0).is_none());
    }

    #[test]
    fn igline2d_rejects_nan_coordinate() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 1.0));
        let nan_bytes = f64::NAN.to_le_bytes();
        // start.x is at record offset 6 + 18 = 24.
        record[24..32].copy_from_slice(&nan_bytes);
        assert!(decode_igline_at(&record, 0).is_none());
    }

    #[test]
    fn igline2d_rejects_out_of_domain_coordinate() {
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (1e10, 0.0), (1.0, 1.0));
        assert!(decode_igline_at(&record, 0).is_none());
    }

    /// The chain, not a payload rule, is what tells an `igLine2d` from a
    /// coincidence — the gate `decode_primitive_lines` has carried since
    /// the `GLine2d` ghost family was retired.
    #[test]
    fn igline2d_ignores_a_record_shaped_match_buried_in_another_record() {
        let hidden = build_synthetic_igline2d_record(9, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        let mut host = Vec::new();
        host.extend_from_slice(&PSM_TYPE_CODE_DEPENDENCY_OBJECT.to_le_bytes());
        host.extend_from_slice(&(hidden.len() as u32).to_le_bytes());
        host.extend_from_slice(&hidden);

        let stream = synthetic_sheet_stream(&[host]);
        assert!(
            decode_igline_at(&stream, SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN).is_some(),
            "the buried bytes do satisfy every payload rule; without that the \
             test would prove nothing"
        );
        assert!(
            decode_iglines(&stream).is_empty(),
            "a payload-shaped match inside another record is not a record"
        );
    }

    #[test]
    fn igline2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 1.0));
        for trunc_len in 0..record.len() {
            assert!(
                decode_iglines(&record[..trunc_len]).is_empty(),
                "truncated input length {trunc_len} must not decode"
            );
        }
        assert!(decode_iglines(&[]).is_empty());
    }

    #[test]
    fn igline2d_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_iglines(&noise);
        assert!(decode_iglines(&vec![0u8; 4096]).is_empty());
        assert!(decode_iglines(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn igline2d_decodes_two_back_to_back_records() {
        let data = synthetic_sheet_stream(&[
            build_synthetic_igline2d_record(7, 100, 0x10, 1, (0.1, 0.1), (0.2, 0.1)),
            build_synthetic_igline2d_record(8, 100, 0x65, 2, (0.3, 0.3), (0.3, 0.5)),
        ]);
        let decoded = decode_iglines(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 7);
        assert_eq!(decoded[1].oid, 8);
        assert_eq!(decoded[0].sub_type_word, 0x10);
        assert_eq!(decoded[1].sub_type_word, 0x65);
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
    }

    // -----------------------------------------------------------------
    // M1 deepening seam: shared PSM envelope + PsmRecordDecoder tests
    // -----------------------------------------------------------------

    #[test]
    fn psm_header_parses_canonical_envelope() {
        // type word 0x8018 = type_code 0x0018 with flag bit 0b10.
        let mut data = vec![0u8; 10];
        data[0..2].copy_from_slice(&0x8018u16.to_le_bytes());
        data[2..6].copy_from_slice(&50u32.to_le_bytes());
        let header = parse_psm_header(&data, 0).expect("envelope");
        assert_eq!(header.type_code, 0x0018);
        assert_eq!(header.type_flags, 0b10);
        assert_eq!(header.bytes_to_follow, 50);
        assert_eq!(header.body_start, 6);
    }

    #[test]
    fn psm_header_honors_nonzero_offset() {
        let mut data = vec![0xAAu8; 16];
        data[4..6].copy_from_slice(&PSM_TYPE_CODE_IGLINE2D.to_le_bytes());
        data[6..10].copy_from_slice(&7u32.to_le_bytes());
        let header = parse_psm_header(&data, 4).expect("envelope at offset");
        assert_eq!(header.type_code, PSM_TYPE_CODE_IGLINE2D);
        assert_eq!(header.bytes_to_follow, 7);
        assert_eq!(header.body_start, 10);
    }

    #[test]
    fn psm_header_rejects_truncated_and_out_of_range_offsets() {
        let data = [0u8; 5];
        assert!(parse_psm_header(&data, 0).is_none(), "5 bytes < envelope");
        assert!(parse_psm_header(&data, 4).is_none());
        assert!(parse_psm_header(&data, 5).is_none());
        assert!(parse_psm_header(&data, usize::MAX).is_none(), "no overflow");
        assert!(parse_psm_header(&[], 0).is_none());
    }

    #[test]
    fn igline2d_trait_scan_matches_free_function_on_a_clean_chain() {
        let data = synthetic_sheet_stream(&[
            build_synthetic_igline2d_record(7, 100, 0x10, 1, (0.1, 0.1), (0.2, 0.1)),
            build_synthetic_igline2d_record(8, 100, 0x65, 2, (0.3, 0.3), (0.3, 0.5)),
        ]);

        // The stream header is not a record, so the sliding scan cannot see
        // it; on a chain of nothing but this family the two must still agree.
        let via_trait = IgLine2dDecoder.scan(&data);
        let via_free_fn = decode_iglines(&data);
        assert_eq!(
            via_trait, via_free_fn,
            "on a clean chain of one family the gate changes nothing"
        );
        assert_eq!(via_trait.len(), 2);
        assert_eq!(via_trait[0].oid, 7);
        assert_eq!(via_trait[1].oid, 8);
    }

    #[test]
    fn igline2d_trait_reports_family_metadata() {
        assert_eq!(IgLine2dDecoder.type_code(), PSM_TYPE_CODE_IGLINE2D);
        assert_eq!(
            IgLine2dDecoder.min_record_len(),
            PSM_ENVELOPE_LEN + IGLINE2D_PAYLOAD_LEN
        );
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        let decoded = IgLine2dDecoder.decode_at(&record, 0).expect("canonical");
        assert_eq!(
            IgLine2dDecoder.advance_of(&decoded),
            record.len(),
            "advance must equal the full on-disk record length"
        );
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice K: PSM igLineString2d (polyline) decoder tests
    // -----------------------------------------------------------------

    /// Build a synthetic `igLineString2d` record with the given
    /// vertex list. Total = 6 PSM header + 24 sub-header + vc*16.
    #[allow(clippy::too_many_arguments)]
    fn build_synthetic_iglinestring2d_record(
        oid: u32,
        parent_ref: u32,
        remaining_header: u32,
        sub_type_word: u16,
        index: u32,
        form: u8,
        scope: u8,
        vertices: &[(f64, f64)],
    ) -> Vec<u8> {
        let vc = vertices.len() as u32;
        let payload_len = 24 + vertices.len() * 16;
        let mut out = Vec::with_capacity(6 + payload_len);
        out.extend_from_slice(&PSM_TYPE_CODE_IGLINESTRING2D.to_le_bytes());
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        out.extend_from_slice(&oid.to_le_bytes());
        out.extend_from_slice(&parent_ref.to_le_bytes());
        out.extend_from_slice(&remaining_header.to_le_bytes());
        out.extend_from_slice(&sub_type_word.to_le_bytes());
        out.extend_from_slice(&index.to_le_bytes());
        out.extend_from_slice(&vc.to_le_bytes());
        out.push(form);
        out.push(scope);
        for (x, y) in vertices {
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
        }
        out
    }

    #[test]
    fn iglinestring2d_decodes_canonical_two_vertex_polyline() {
        let record = build_synthetic_iglinestring2d_record(
            494,
            482,
            0x11,
            0x0010,
            1,
            1,
            2,
            &[(0.1, 0.2), (0.3, 0.4)],
        );
        let decoded = decode_iglinestrings(&record);
        assert_eq!(decoded.len(), 1);
        let pl = &decoded[0];
        assert_eq!(pl.type_code, PSM_TYPE_CODE_IGLINESTRING2D);
        assert_eq!(pl.bytes_to_follow, 56);
        assert_eq!(pl.oid, 494);
        assert_eq!(pl.parent_ref, 482);
        assert_eq!(pl.sub_type_word, 0x0010);
        assert_eq!(pl.index, 1);
        assert_eq!(pl.form, 1);
        assert_eq!(pl.scope, 2);
        assert_eq!(pl.vertex_count(), 2);
        assert_eq!(pl.vertices[0], (0.1, 0.2));
        assert_eq!(pl.vertices[1], (0.3, 0.4));
        assert!((pl.total_length() - (0.2_f64.hypot(0.2))).abs() < 1e-9);
    }

    #[test]
    fn iglinestring2d_decodes_three_vertex_polyline() {
        let record = build_synthetic_iglinestring2d_record(
            275,
            417,
            0x08,
            0x0010,
            30,
            1,
            1,
            &[(0.0, 0.0), (0.1, 0.0), (0.1, 0.1)],
        );
        let decoded = decode_iglinestrings(&record);
        assert_eq!(decoded.len(), 1);
        let pl = &decoded[0];
        assert_eq!(pl.vertex_count(), 3);
        assert_eq!(pl.bytes_to_follow, 24 + 3 * 16);
        // Total length: 0.1 + 0.1 = 0.2.
        assert!((pl.total_length() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn iglinestring2d_rejects_wrong_type_code() {
        let mut record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            1,
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        record[0] = 0x18; // make it look like igLine2d
        record[1] = 0x00;
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_rejects_inconsistent_vertex_count() {
        let mut record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            1,
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        // Overwrite inline vc at payload offset 18 (= record offset 24).
        record[24..28].copy_from_slice(&5u32.to_le_bytes());
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_rejects_form_out_of_range() {
        let record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            7,
            1,
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_rejects_scope_out_of_range() {
        let record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            5, // invalid: > 4 and != 6
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_accepts_scope_6() {
        let record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            6, // accepted special case
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        assert_eq!(decode_iglinestrings(&record).len(), 1);
    }

    #[test]
    fn iglinestring2d_rejects_single_vertex_polyline() {
        // Vertex count = 1 is invalid (GLineString2d::Validate
        // requires >= 2).
        let mut record = vec![];
        record.extend_from_slice(&PSM_TYPE_CODE_IGLINESTRING2D.to_le_bytes());
        record.extend_from_slice(&(24u32 + 16).to_le_bytes());
        record.extend_from_slice(&[0u8; 18]);
        record.extend_from_slice(&1u32.to_le_bytes()); // vc=1
        record.push(1); // form
        record.push(1); // scope
        record.extend_from_slice(&0.5f64.to_le_bytes());
        record.extend_from_slice(&0.5f64.to_le_bytes());
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_rejects_degenerate_all_same_vertices() {
        let record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            1,
            &[(0.5, 0.5), (0.5, 0.5)],
        );
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_rejects_nan_vertex() {
        let mut record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            1,
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        // First vertex.x at record offset 6 + 24 = 30.
        record[30..38].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_iglinestrings(&record).is_empty());
    }

    #[test]
    fn iglinestring2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_iglinestring2d_record(
            1,
            1,
            0x11,
            0x10,
            0,
            1,
            1,
            &[(0.0, 0.0), (1.0, 1.0)],
        );
        for trunc_len in 0..record.len() {
            assert!(
                decode_iglinestrings(&record[..trunc_len]).is_empty(),
                "truncated input length {trunc_len} must not decode"
            );
        }
        assert!(decode_iglinestrings(&[]).is_empty());
    }

    #[test]
    fn iglinestring2d_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_iglinestrings(&noise);
        assert!(decode_iglinestrings(&vec![0u8; 4096]).is_empty());
        assert!(decode_iglinestrings(&vec![0xFFu8; 4096]).is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice L: PSM igPoint2d decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_igpoint2d_record(
        oid: u32,
        parent_ref: u32,
        sub_type_word: u16,
        index: u32,
        x: f64,
        y: f64,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + IGPOINT2D_PAYLOAD_LEN);
        out.extend_from_slice(&PSM_TYPE_CODE_IGPOINT2D.to_le_bytes());
        out.extend_from_slice(&(IGPOINT2D_PAYLOAD_LEN as u32).to_le_bytes());
        out.extend_from_slice(&oid.to_le_bytes());
        out.extend_from_slice(&parent_ref.to_le_bytes());
        out.extend_from_slice(&18u32.to_le_bytes()); // remaining_header
        out.extend_from_slice(&sub_type_word.to_le_bytes());
        out.extend_from_slice(&index.to_le_bytes());
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out
    }

    #[test]
    fn igpoint2d_decodes_canonical_point() {
        let record = build_synthetic_igpoint2d_record(130, 6, 0x0010, 13, 0.1737, 0.2199);
        let decoded = decode_igpoints(&record);
        assert_eq!(decoded.len(), 1);
        let p = &decoded[0];
        assert_eq!(p.type_code, PSM_TYPE_CODE_IGPOINT2D);
        assert_eq!(p.bytes_to_follow, 34);
        assert_eq!(p.oid, 130);
        assert_eq!(p.parent_ref, 6);
        assert_eq!(p.sub_type_word, 0x0010);
        assert_eq!(p.index, 13);
        assert!((p.point.0 - 0.1737).abs() < 1e-9);
        assert!((p.point.1 - 0.2199).abs() < 1e-9);
    }

    #[test]
    fn igpoint2d_rejects_wrong_type_code() {
        let mut record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.0, 0.0);
        record[0] = 0x18;
        record[1] = 0x00;
        assert!(decode_igpoints(&record).is_empty());
    }

    #[test]
    fn igpoint2d_rejects_wrong_bytes_to_follow() {
        let mut record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.0, 0.0);
        record[2] = 33;
        assert!(decode_igpoints(&record).is_empty());
    }

    #[test]
    fn igpoint2d_rejects_nan() {
        let mut record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.0, 0.0);
        // x at record offset 6 + 18 = 24
        record[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igpoints(&record).is_empty());
    }

    #[test]
    fn igpoint2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.5, 0.5);
        for trunc_len in 0..record.len() {
            assert!(decode_igpoints(&record[..trunc_len]).is_empty());
        }
        assert!(decode_igpoints(&[]).is_empty());
    }

    #[test]
    fn igpoint2d_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igpoints(&noise);
        assert!(decode_igpoints(&vec![0u8; 4096]).is_empty());
        assert!(decode_igpoints(&vec![0xFFu8; 4096]).is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice M: PSM igTextBox decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_igtextbox_record(text: &str, oid: u32, parent_ref: u32) -> Vec<u8> {
        let u16_chars: Vec<u16> = text.encode_utf16().collect();
        let text_length = u16_chars.len() as u16;
        let payload_len = IGTEXTBOX_PAYLOAD_OVERHEAD + u16_chars.len() * 2;
        let mut out = Vec::with_capacity(6 + payload_len);
        out.extend_from_slice(&PSM_TYPE_CODE_IGTEXTBOX.to_le_bytes());
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        // Payload:
        out.extend_from_slice(&oid.to_le_bytes()); // 0..4 oid
        out.extend_from_slice(&parent_ref.to_le_bytes()); // 4..8 parent
        out.extend_from_slice(&12u32.to_le_bytes()); // 8..12 remaining_header
        out.extend_from_slice(&0x0010u16.to_le_bytes()); // 12..14 sub_type
        out.extend_from_slice(&0u32.to_le_bytes()); // 14..18 index
                                                    // 18..30 is the sub-type 2 body prefix: the shape
                                                    // discriminator, the style-tail tag, the count packed with
                                                    // its 0x10000 marker, and a spare dword.
        out.extend_from_slice(&2u16.to_le_bytes()); // 18..20 text sub-type
        out.extend_from_slice(&0u16.to_le_bytes()); // 20..22 style-tail tag
        out.extend_from_slice(&(text_length as u32 | 0x1_0000).to_le_bytes()); // 22..26
        out.extend_from_slice(&0u32.to_le_bytes()); // 26..30
                                                    // 30..32: inline text_length.
        out.extend_from_slice(&text_length.to_le_bytes());
        // 32..32+text_length*2: UTF-16LE text.
        for c in u16_chars {
            out.extend_from_slice(&c.to_le_bytes());
        }
        // 36 bytes of trailing data (3 f64 + 12 bytes).
        out.extend_from_slice(&0.5f64.to_le_bytes()); // ins.x
        out.extend_from_slice(&0.5f64.to_le_bytes()); // ins.y
        out.extend_from_slice(&1.0f64.to_le_bytes()); // scale
        out.extend_from_slice(&[0u8; 12]); // trailer
        out
    }

    #[test]
    fn igtextbox_decodes_canonical_ascii_text() {
        let record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        let decoded = decode_igtextboxes(&record);
        assert_eq!(decoded.len(), 1);
        let t = &decoded[0];
        assert_eq!(t.type_code, PSM_TYPE_CODE_IGTEXTBOX);
        assert_eq!(t.oid, 100);
        assert_eq!(t.parent_ref, 50);
        assert_eq!(t.text_length, 8);
        assert_eq!(t.text, "PUMP-101");
        assert!((t.trailing_double_1 - 0.5).abs() < 1e-9);
        assert!((t.trailing_double_2 - 0.5).abs() < 1e-9);
        assert!((t.trailing_double_3 - 1.0).abs() < 1e-9);
    }

    /// Rewrite the direction pair that follows the insertion point.
    fn set_text_direction(record: &mut [u8], text_len_chars: usize, cos: f64, sin: f64) {
        let text_end = 6 + 32 + text_len_chars * 2;
        record[text_end + 16..text_end + 24].copy_from_slice(&cos.to_le_bytes());
        record[text_end + 24..text_end + 32].copy_from_slice(&sin.to_le_bytes());
    }

    /// A label lettered up a vertical pipe decodes as a quarter turn.
    ///
    /// The rotation is stored as `(cos, sin)` rather than as an angle — the
    /// same convention `igSymbol2d` uses for its placement matrix — so each
    /// component alone reads as a meaningless `0` or `±1`. Together they are
    /// the angle, and this is the corpus's second most common one: 35 of its
    /// labels run vertically.
    #[test]
    fn igtextbox_decodes_the_direction_the_text_is_lettered_along() {
        for (cos, sin, expect_deg) in [
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 90.0),
            (-1.0, 0.0, 180.0),
            (0.0, -1.0, -90.0),
        ] {
            let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
            set_text_direction(&mut record, 8, cos, sin);

            let decoded = decode_igtextboxes(&record);
            assert_eq!(decoded.len(), 1, "({cos}, {sin}) is a direction");
            assert!(
                (decoded[0].rotation_rad.to_degrees() - expect_deg).abs() < 1e-9,
                "({cos}, {sin}) should letter at {expect_deg} degrees, got {}",
                decoded[0].rotation_rad.to_degrees()
            );
        }
    }

    /// A pair that is not a unit vector is not a direction, and the record is
    /// refused.
    ///
    /// This is the check that keeps the stated text length honest. A length
    /// that over-runs the real label puts these two reads on bytes that are
    /// not the direction at all; on the corpus they then miss unit length by
    /// many orders of magnitude, never by float noise. Every record whose
    /// text carries the binary that follows an over-read fails here.
    #[test]
    fn igtextbox_refuses_a_direction_that_is_not_a_unit_vector() {
        for (cos, sin) in [(0.0, 0.0), (2.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
            let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
            set_text_direction(&mut record, 8, cos, sin);

            assert!(
                decode_igtextboxes(&record).is_empty(),
                "({cos}, {sin}) is not a direction, so the record's shape is wrong"
            );
        }
    }

    /// A record whose tail is longer than 36 bytes still decodes.
    ///
    /// The corpus has 33 of these — tails of 68 or 76 bytes instead of 36,
    /// with the head unchanged and the text at `+32` reading normally. The
    /// old rule derived the text length from `bytes_to_follow` on a fixed
    /// 68-byte overhead and refused them all, because the derived length
    /// disagreed with the length the record states at `+30`. Believing
    /// `+30` and asking only that the payload have *room* for the text and
    /// the trailing block accepts them, and this pins that: a return to
    /// the derived-length rule fails here.
    #[test]
    fn igtextbox_decodes_a_record_with_a_longer_tail() {
        for extra in [32usize, 40] {
            let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
            // Append the bytes and grow `bytes_to_follow` to match, leaving
            // the head and the stated text length untouched.
            let btf = u32::from_le_bytes([record[2], record[3], record[4], record[5]]);
            let grown = btf + extra as u32;
            record[2..6].copy_from_slice(&grown.to_le_bytes());
            record.extend(std::iter::repeat_n(0u8, extra));

            let decoded = decode_igtextboxes(&record);
            assert_eq!(
                decoded.len(),
                1,
                "a {extra}-byte longer tail must not refuse the record"
            );
            assert_eq!(decoded[0].text, "PUMP-101");
            assert_eq!(decoded[0].text_length, 8);
            // The record still spans its whole stated length, so the scan
            // resumes after the extra bytes rather than inside them.
            assert_eq!(decoded[0].byte_range, 0..record.len());
        }
    }

    /// The stated length still has to fit: `+30` is believed, not obeyed
    /// blindly. A length claiming more text than the payload can hold —
    /// with its trailing block — is still refused.
    #[test]
    fn igtextbox_refuses_a_stated_length_the_payload_cannot_hold() {
        // Claim 400 chars (800 bytes) in a payload sized for 8, stated
        // consistently so the count-agreement rule is not what fires.
        let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        set_consistent_count(&mut record, 400);
        assert!(
            decode_igtextboxes(&record).is_empty(),
            "a stated length with no room for text plus the trailing block is noise"
        );

        // The other way a stated length lies: a sub-type 2 record carries its
        // count twice -- packed with a 0x10000 marker at +22 and plain at
        // +30 -- and editing only one of them breaks their agreement.
        let mut record = build_synthetic_igtextbox_record("HELLO", 1, 1);
        record[6 + 30..6 + 32].copy_from_slice(&999u16.to_le_bytes());
        assert!(
            decode_igtextboxes(&record).is_empty(),
            "a count that disagrees with its packed twin is not believed"
        );
    }

    /// Build a sub-type 1 or sub-type 3 record: the two shapes whose body
    /// puts the count somewhere other than `+30`, and which the decoder
    /// refused wholesale until the native layout was read out of
    /// `radsrvitem.dll`.
    fn build_igtextbox_of_sub_type(
        sub_type: u16,
        text: &str,
        extra_doubles: (u16, u16),
    ) -> Vec<u8> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let count = units.len() as u16;
        let (a, b) = extra_doubles;
        let body_len = match sub_type {
            1 => 2 + 2 * units.len(),
            3 => 6 + 2 * units.len() + 8 * (a as usize + b as usize),
            other => panic!("this helper builds sub-types 1 and 3, not {other}"),
        };
        let payload_len = 22 + body_len + 36;

        let mut out = Vec::with_capacity(6 + payload_len);
        out.extend_from_slice(&PSM_TYPE_CODE_IGTEXTBOX.to_le_bytes());
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        out.extend_from_slice(&7u32.to_le_bytes()); // oid
        out.extend_from_slice(&1u32.to_le_bytes()); // parent_ref
        out.extend_from_slice(&12u32.to_le_bytes()); // remaining_header
        out.extend_from_slice(&0x0010u16.to_le_bytes()); // sub_type_word
        out.extend_from_slice(&0u32.to_le_bytes()); // index
        out.extend_from_slice(&sub_type.to_le_bytes()); // +18 shape
        out.extend_from_slice(&0u16.to_le_bytes()); // +20 style-tail tag
        match sub_type {
            1 => out.extend_from_slice(&count.to_le_bytes()), // +22 count
            _ => {
                out.extend_from_slice(&a.to_le_bytes()); // +22
                out.extend_from_slice(&b.to_le_bytes()); // +24
                out.extend_from_slice(&count.to_le_bytes()); // +26
            }
        }
        for unit in &units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        if sub_type == 3 {
            for _ in 0..(a as usize + b as usize) {
                out.extend_from_slice(&0.0f64.to_le_bytes());
            }
        }
        // The 36-byte placement tail: insertion, direction, trailer.
        out.extend_from_slice(&0.25f64.to_le_bytes());
        out.extend_from_slice(&0.75f64.to_le_bytes());
        out.extend_from_slice(&0.0f64.to_le_bytes()); // cos
        out.extend_from_slice(&1.0f64.to_le_bytes()); // sin -> 90 degrees
        out.extend_from_slice(&[0u8; 4]);
        out
    }

    /// The two shapes the decoder used to refuse now decode, text and all.
    ///
    /// 45 of the corpus's 260 `igTextBox` records are sub-type 1 or 3, and 43
    /// of those carry a readable label — lettering that was simply absent
    /// from every drawing. The layout comes from `radsrvitem.dll`'s
    /// `igTextBox` Load; see [`igtextbox_body_shape`].
    #[test]
    fn igtextbox_decodes_the_two_shapes_whose_count_is_not_at_plus_30() {
        for (sub_type, extras) in [(1u16, (0u16, 0u16)), (3, (2, 3))] {
            let record =
                build_igtextbox_of_sub_type(sub_type, "\u{8BBE}\u{5907}\u{4F4D}\u{53F7}", extras);
            let decoded = decode_igtextboxes(&record);

            assert_eq!(decoded.len(), 1, "sub-type {sub_type} must decode");
            let t = &decoded[0];
            assert_eq!(t.text_sub_type, sub_type);
            assert_eq!(t.text, "\u{8BBE}\u{5907}\u{4F4D}\u{53F7}");
            assert_eq!(t.text_length, 4);
            // The placement tail sits past the body, which for sub-type 3
            // includes the trailing doubles -- read it at the wrong offset
            // and the direction is not a unit vector.
            assert!((t.trailing_double_1 - 0.25).abs() < 1e-12);
            assert!((t.trailing_double_2 - 0.75).abs() < 1e-12);
            assert!((t.rotation_rad.to_degrees() - 90.0).abs() < 1e-9);
        }
    }

    /// Each shape hands over the formatting runs it stores, in stored order.
    ///
    /// Shape 2's one run is the dword the decoder already checks against the
    /// count — its low word is the run length, its high word selector `1` —
    /// plus the style id after it. Shape 3's `A` character-style runs and `B`
    /// paragraph runs follow the text; the three below are how a line number
    /// alternates two character styles between its segments and separators.
    #[test]
    fn igtextbox_hands_over_the_runs_each_shape_stores() {
        let mut shape_2 = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        shape_2[6 + 26..6 + 30].copy_from_slice(&0x42u32.to_le_bytes());
        let decoded = decode_igtextboxes(&shape_2);
        assert_eq!(decoded.len(), 1);
        assert_eq!(
            decoded[0].runs,
            vec![IgTextBoxRun {
                len: 8,
                selector: 1,
                style_id: 0x42,
            }]
        );

        let mut shape_3 = build_igtextbox_of_sub_type(3, "250-LNG-", (2, 1));
        let first = 6 + 28 + 2 * 8;
        for (i, (len, selector, style_id)) in [(3u16, 1u16, 0x42u32), (5, 1, 0x46), (1, 2, 0x24)]
            .into_iter()
            .enumerate()
        {
            let at = first + 8 * i;
            shape_3[at..at + 2].copy_from_slice(&len.to_le_bytes());
            shape_3[at + 2..at + 4].copy_from_slice(&selector.to_le_bytes());
            shape_3[at + 4..at + 8].copy_from_slice(&style_id.to_le_bytes());
        }
        let decoded = decode_igtextboxes(&shape_3);
        assert_eq!(decoded.len(), 1);
        assert_eq!(
            decoded[0].runs,
            vec![
                IgTextBoxRun {
                    len: 3,
                    selector: 1,
                    style_id: 0x42,
                },
                IgTextBoxRun {
                    len: 5,
                    selector: 1,
                    style_id: 0x46,
                },
                IgTextBoxRun {
                    len: 1,
                    selector: 2,
                    style_id: 0x24,
                },
            ]
        );

        let shape_1 = build_igtextbox_of_sub_type(1, "PUMP", (0, 0));
        let decoded = decode_igtextboxes(&shape_1);
        assert_eq!(decoded.len(), 1);
        assert!(
            decoded[0].runs.is_empty(),
            "shape 1 is the shape without runs"
        );
    }

    /// An unknown shape is refused rather than guessed at.
    #[test]
    fn igtextbox_refuses_a_sub_type_the_native_reader_does_not_name() {
        let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        record[6 + 18..6 + 20].copy_from_slice(&4u16.to_le_bytes());

        assert!(
            decode_igtextboxes(&record).is_empty(),
            "the native Load names sub-types 1, 2 and 3; a fourth is not a shape we know"
        );
    }

    #[test]
    fn igtextbox_decodes_chinese_unicode_text() {
        let record = build_synthetic_igtextbox_record("流量计", 200, 100);
        let decoded = decode_igtextboxes(&record);
        assert_eq!(decoded.len(), 1);
        let t = &decoded[0];
        assert_eq!(t.text, "流量计");
        assert_eq!(t.text_length, 3);
    }

    #[test]
    fn igtextbox_rejects_wrong_type_code() {
        let mut record = build_synthetic_igtextbox_record("X", 1, 1);
        record[0] = 0x18;
        record[1] = 0x00;
        assert!(decode_igtextboxes(&record).is_empty());
    }

    /// Set a sub-type 2 record's count in both places the format states it,
    /// so a test can exercise a rule other than their agreement.
    fn set_consistent_count(record: &mut [u8], count: u16) {
        record[6 + 22..6 + 26].copy_from_slice(&(count as u32 | 0x1_0000).to_le_bytes());
        record[6 + 30..6 + 32].copy_from_slice(&count.to_le_bytes());
    }

    #[test]
    fn igtextbox_rejects_zero_length_overhead_violation() {
        // bytes_to_follow below the 68-byte floor is rejected.
        let mut record = vec![];
        record.extend_from_slice(&PSM_TYPE_CODE_IGTEXTBOX.to_le_bytes());
        record.extend_from_slice(&50u32.to_le_bytes()); // way less than 68
        record.extend_from_slice(&[0u8; 50]);
        assert!(decode_igtextboxes(&record).is_empty());
    }

    #[test]
    fn igtextbox_rejects_nan_trailing_double() {
        let mut record = build_synthetic_igtextbox_record("ABC", 1, 1);
        let text_end = 6 + 32 + 3 * 2; // record header + payload header + text
        record[text_end..text_end + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igtextboxes(&record).is_empty());
    }

    #[test]
    fn igtextbox_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igtextbox_record("X", 1, 1);
        for trunc_len in 0..record.len() {
            assert!(decode_igtextboxes(&record[..trunc_len]).is_empty());
        }
        assert!(decode_igtextboxes(&[]).is_empty());
    }

    #[test]
    fn igtextbox_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igtextboxes(&noise);
        assert!(decode_igtextboxes(&vec![0u8; 4096]).is_empty());
        assert!(decode_igtextboxes(&vec![0xFFu8; 4096]).is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 14 Slice N: PSM igSymbol2d decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_igsymbol2d_record(
        oid: u32,
        parent_ref: u32,
        jsite_ref: u32,
        transform: [f64; 4],
        insertion: (f64, f64),
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + IGSYMBOL2D_MIN_PAYLOAD_LEN);
        out.extend_from_slice(&PSM_TYPE_CODE_IGSYMBOL2D.to_le_bytes());
        out.extend_from_slice(&(IGSYMBOL2D_MIN_PAYLOAD_LEN as u32).to_le_bytes());
        out.extend_from_slice(&oid.to_le_bytes()); // 0..4
        out.extend_from_slice(&parent_ref.to_le_bytes()); // 4..8
        out.extend_from_slice(&8u32.to_le_bytes()); // 8..12 remaining_header
        out.extend_from_slice(&0x0010u16.to_le_bytes()); // 12..14 sub_type
        out.extend_from_slice(&[0u8; 15]); // 14..29 sub-fields
        out.extend_from_slice(&jsite_ref.to_le_bytes()); // 29..33 JSite<id>
        out.extend_from_slice(&IGSYMBOL2D_MATRIX_TAG); // 33..37
        for v in &transform {
            // 37..69
            out.extend_from_slice(&v.to_le_bytes());
        }
        // 69..85: translation
        out.extend_from_slice(&insertion.0.to_le_bytes());
        out.extend_from_slice(&insertion.1.to_le_bytes());
        // 85..93: uniform scale, then the trailing library ref + flags.
        out.extend_from_slice(&1.0f64.to_le_bytes());
        out.extend_from_slice(&[0u8; 20]); // 93..113
        out
    }

    /// Payload offset of `transform[0]` in a record from
    /// [`build_synthetic_igsymbol2d_record`].
    const SYNTHETIC_IGSYMBOL2D_MATRIX_AT: usize = 33 + 4;

    /// The last two words of the payload name the body: the definition
    /// cache's `JSheet`, then the `LdcSite` storage that cache is. Both
    /// payload shapes the corpus has -- 113 bytes with `has_membassy = 0`,
    /// 121 with the flag set and a `(membassy, 0)` pair in front -- put them
    /// at the end, which is why they are read from the end.
    #[test]
    fn igsymbol2d_tail_names_the_definition_sheet_and_cache() {
        let mut short =
            build_synthetic_igsymbol2d_record(500, 6, 399, [1.0, 0.0, 0.0, 1.0], (0.3, 0.4));
        let len = short.len();
        short[len - 8..len - 4].copy_from_slice(&125u32.to_le_bytes());
        short[len - 4..].copy_from_slice(&145u32.to_le_bytes());
        let decoded = decode_igsymbols(&short);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].definition_sheet_ref, 125);
        assert_eq!(decoded[0].definition_site_ref, 145);

        // The 121-byte shape: flag word set, then `(membassy oid, 0)`, then
        // the same two words.
        let mut long =
            build_synthetic_igsymbol2d_record(500, 6, 399, [1.0, 0.0, 0.0, 1.0], (0.3, 0.4));
        long.truncate(PSM_ENVELOPE_LEN + 93);
        long.extend_from_slice(&0x0100_5001_u32.to_le_bytes()); // flags
        long.extend_from_slice(&1u32.to_le_bytes()); // has_membassy
        long.extend_from_slice(&0u32.to_le_bytes());
        long.extend_from_slice(&142u32.to_le_bytes()); // membassy oid
        long.extend_from_slice(&0u32.to_le_bytes());
        long.extend_from_slice(&93u32.to_le_bytes()); // JSheet
        long.extend_from_slice(&145u32.to_le_bytes()); // LdcSite
        let payload_len = (long.len() - PSM_ENVELOPE_LEN) as u32;
        long[2..6].copy_from_slice(&payload_len.to_le_bytes());
        assert_eq!(payload_len, 121);
        let decoded = decode_igsymbols(&long);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].definition_sheet_ref, 93);
        assert_eq!(decoded[0].definition_site_ref, 145);
    }

    #[test]
    fn igsymbol2d_decodes_canonical_unrotated_symbol() {
        let record = build_synthetic_igsymbol2d_record(
            500,
            6,
            399,
            [1.0, 0.0, 0.0, 1.0], // identity transform
            (0.3, 0.4),
        );
        let decoded = decode_igsymbols(&record);
        assert_eq!(decoded.len(), 1);
        let s = &decoded[0];
        assert_eq!(s.type_code, PSM_TYPE_CODE_IGSYMBOL2D);
        assert_eq!(s.bytes_to_follow, 113);
        assert_eq!(s.oid, 500);
        assert_eq!(s.parent_ref, 6);
        assert_eq!(s.jsite_ref, 399);
        assert_eq!(s.transform, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(s.insertion, (0.3, 0.4));
    }

    #[test]
    fn igsymbol2d_rejects_wrong_type_code() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
        record[0] = 0x18;
        record[1] = 0x00;
        assert!(decode_igsymbols(&record).is_empty());
    }

    #[test]
    fn igsymbol2d_rejects_undersized_bytes_to_follow() {
        let mut record = vec![];
        record.extend_from_slice(&PSM_TYPE_CODE_IGSYMBOL2D.to_le_bytes());
        record.extend_from_slice(&100u32.to_le_bytes());
        record.extend_from_slice(&[0u8; 100]);
        assert!(decode_igsymbols(&record).is_empty());
    }

    #[test]
    fn igsymbol2d_rejects_nan_transform_element() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
        let at = PSM_ENVELOPE_LEN + SYNTHETIC_IGSYMBOL2D_MATRIX_AT;
        record[at..at + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igsymbols(&record).is_empty());
    }

    #[test]
    fn igsymbol2d_rejects_record_without_matrix_tag() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
        let tag_at = PSM_ENVELOPE_LEN + 33;
        record[tag_at..tag_at + IGSYMBOL2D_MATRIX_TAG.len()].copy_from_slice(&[0xFF; 4]);
        assert!(decode_igsymbols(&record).is_empty());
    }

    #[test]
    fn igsymbol2d_follows_the_tag_when_the_header_grows() {
        // The 115- and 123-byte fixture families push the matrix two
        // bytes later; a decoder pinned to one offset misses them. The
        // growth happens before the JSite id, which stays glued to the
        // tag (Phase 35-C: +29 records vs +31 records).
        let base =
            build_synthetic_igsymbol2d_record(7, 6, 204, [0.0, 1.0, -1.0, 0.0], (0.25, 0.75));
        let mut shifted = base.clone();
        let jsite_at = PSM_ENVELOPE_LEN + 29;
        shifted.splice(jsite_at..jsite_at, [0u8, 0u8]);
        let btf = (shifted.len() - PSM_ENVELOPE_LEN) as u32;
        shifted[2..6].copy_from_slice(&btf.to_le_bytes());

        let decoded = decode_igsymbols(&shifted);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].jsite_ref, 204);
        assert_eq!(decoded[0].transform, [0.0, 1.0, -1.0, 0.0]);
        assert_eq!(decoded[0].insertion, (0.25, 0.75));
    }

    #[test]
    fn igsymbol2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
        for trunc_len in 0..record.len() {
            assert!(decode_igsymbols(&record[..trunc_len]).is_empty());
        }
        assert!(decode_igsymbols(&[]).is_empty());
    }

    #[test]
    fn igsymbol2d_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igsymbols(&noise);
        assert!(decode_igsymbols(&vec![0u8; 4096]).is_empty());
        assert!(decode_igsymbols(&vec![0xFFu8; 4096]).is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 15 Slice C: PSM 0x00FA DependencyObject decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_dependency_object_record(
        oid: u32,
        parent_ref: u32,
        group_kind_word: u16,
        sub_type_word: u16,
        raw_reference_payload: &[u8],
    ) -> Vec<u8> {
        let bytes_to_follow = 18 + raw_reference_payload.len();
        assert!(bytes_to_follow >= DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN);
        let mut out = Vec::with_capacity(6 + bytes_to_follow);
        out.extend_from_slice(&PSM_TYPE_CODE_DEPENDENCY_OBJECT.to_le_bytes());
        out.extend_from_slice(&(bytes_to_follow as u32).to_le_bytes());
        out.extend_from_slice(&oid.to_le_bytes());
        out.extend_from_slice(&parent_ref.to_le_bytes());
        out.extend_from_slice(&[0u8; 6]);
        out.extend_from_slice(&group_kind_word.to_le_bytes());
        out.extend_from_slice(&sub_type_word.to_le_bytes());
        out.extend_from_slice(raw_reference_payload);
        out
    }

    #[test]
    fn dependency_object_decodes_canonical_header_and_raw_tail() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);

        let decoded = decode_dependency_objects(&record);
        assert_eq!(decoded.len(), 1);
        let group = &decoded[0];
        assert_eq!(group.byte_range, 0..record.len());
        assert_eq!(group.type_code, PSM_TYPE_CODE_DEPENDENCY_OBJECT);
        assert_eq!(group.type_flags, 0);
        assert_eq!(
            group.bytes_to_follow,
            DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN as u32
        );
        assert_eq!(group.oid, 42);
        assert_eq!(group.parent_ref, 6);
        assert_eq!(group.group_kind_word, 2);
        assert_eq!(group.sub_type_word, 0x01A1);
        assert_eq!(group.raw_reference_payload, raw_tail);
    }

    #[test]
    fn dependency_object_rejects_wrong_type_code() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        record[0] = 0x18;
        record[1] = 0x00;

        assert!(decode_dependency_objects(&record).is_empty());
    }

    #[test]
    fn dependency_object_rejects_nonzero_type_flags() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        let flagged_type = PSM_TYPE_CODE_DEPENDENCY_OBJECT | 0x4000;
        record[0..2].copy_from_slice(&flagged_type.to_le_bytes());

        assert!(decode_dependency_objects(&record).is_empty());
    }

    #[test]
    fn dependency_object_rejects_invalid_size_and_truncation() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        record[2..6].copy_from_slice(&43u32.to_le_bytes());
        assert!(decode_dependency_objects(&record).is_empty());

        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        assert!(decode_dependency_objects(&record[..record.len() - 1]).is_empty());
    }

    #[test]
    fn dependency_object_rejects_invalid_header_fields() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        assert!(
            decode_dependency_objects(&build_synthetic_dependency_object_record(
                0, 6, 2, 0x01A1, &raw_tail
            ))
            .is_empty()
        );
        assert!(
            decode_dependency_objects(&build_synthetic_dependency_object_record(
                42, 7, 2, 0x01A1, &raw_tail
            ))
            .is_empty()
        );
        assert!(
            decode_dependency_objects(&build_synthetic_dependency_object_record(
                42, 6, 0, 0x01A1, &raw_tail
            ))
            .is_empty()
        );
    }

    #[test]
    fn dependency_object_rejects_nonzero_reserved_prefix() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        record[6 + 8] = 1;

        assert!(decode_dependency_objects(&record).is_empty());
    }

    #[test]
    fn dependency_object_decoder_is_panic_safe_on_short_input() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        for trunc_len in 0..record.len() {
            assert!(decode_dependency_objects(&record[..trunc_len]).is_empty());
        }
        assert!(decode_dependency_objects(&[]).is_empty());
    }

    #[test]
    fn dependency_object_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_dependency_objects(&noise);
        assert!(decode_dependency_objects(&vec![0u8; 4096]).is_empty());
        assert!(decode_dependency_objects(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn dependency_object_decodes_two_back_to_back_records() {
        let raw_tail = vec![0x01; DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN - 18];
        let first = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        let second = build_synthetic_dependency_object_record(43, 6, 1, 0x00B8, &raw_tail);
        let mut data = first;
        data.extend_from_slice(&second);

        let decoded = decode_dependency_objects(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 42);
        assert_eq!(decoded[1].oid, 43);
    }

    #[test]
    fn primitive_line_byte_range_covers_full_record_when_attribute_tail_present() {
        // Build a record whose bytes_to_follow covers its 12 uncovered
        // header bytes, the 48-byte payload, and a 200-byte attribute tail.
        const TAIL_LEN: usize = 200;
        const BYTES_TO_FOLLOW: usize = SYNTHETIC_GLINE2D_BYTES_TO_FOLLOW + TAIL_LEN;
        let mut record = Vec::new();
        let type_word: u16 = PSM_TYPE_CODE_GLINE2D;
        record.extend_from_slice(&type_word.to_le_bytes());
        record.extend_from_slice(&(BYTES_TO_FOLLOW as u32).to_le_bytes());
        // oid
        record.extend_from_slice(&123u32.to_le_bytes());
        // 8-byte aux
        record.extend_from_slice(&[0u8; 8]);
        // GLine2d payload
        for v in [0.0f64, 0.0, 1.0, 0.0, 0.0, 1.0] {
            record.extend_from_slice(&v.to_le_bytes());
        }
        record.extend_from_slice(&[0xAB; TAIL_LEN]);

        let decoded = decode_primitive_lines(&synthetic_sheet_stream(&[record]));
        assert_eq!(decoded.len(), 1);
        let line = &decoded[0];
        assert_eq!(line.byte_range.start, SHEET_STREAM_HEADER_LEN);
        // byte_range covers the envelope plus everything it declares, tail
        // included — not just the geometry payload.
        assert_eq!(
            line.byte_range.end,
            SHEET_STREAM_HEADER_LEN + PSM_ENVELOPE_LEN + BYTES_TO_FOLLOW
        );
        assert_eq!(line.bytes_to_follow as usize, BYTES_TO_FOLLOW);
    }

    // -----------------------------------------------------------------
    // Phase 26: PSM 0x0010 attribute-fragment decoder tests
    // -----------------------------------------------------------------

    fn build_attribute_fragment(marker: u32, aux: [u8; 8], strings: &[&str]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&marker.to_le_bytes());
        payload.extend_from_slice(&aux);
        for s in strings {
            let units: Vec<u16> = s.encode_utf16().collect();
            payload.extend_from_slice(&(units.len() as u16).to_le_bytes());
            for u in units {
                payload.extend_from_slice(&u.to_le_bytes());
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    #[test]
    fn attribute_fragment_decodes_single_string() {
        let rec = build_attribute_fragment(
            0x0001_0002,
            [3, 0, 1, 0, 0x44, 0, 0, 0],
            &["ODOIL020150 MM"],
        );
        let decoded = decode_attribute_fragments(&rec);
        assert_eq!(decoded.len(), 1);
        let f = &decoded[0];
        assert_eq!(f.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
        assert_eq!(f.marker, 0x0001_0002);
        assert_eq!(f.aux, [3, 0, 1, 0, 0x44, 0, 0, 0]);
        assert_eq!(f.strings.len(), 1);
        assert_eq!(f.strings[0].char_count, 14);
        assert_eq!(f.strings[0].text, "ODOIL020150 MM");
    }

    #[test]
    fn attribute_fragment_decodes_cjk_string() {
        let rec = build_attribute_fragment(0x0001_0002, [0; 8], &["设计温度"]);
        let decoded = decode_attribute_fragments(&rec);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].strings[0].text, "设计温度");
        assert_eq!(decoded[0].strings[0].char_count, 4);
    }

    #[test]
    fn attribute_fragment_decodes_multiple_strings() {
        let rec = build_attribute_fragment(1, [0; 8], &["A3", "DN80"]);
        let decoded = decode_attribute_fragments(&rec);
        assert_eq!(decoded.len(), 1);
        let texts: Vec<&str> = decoded[0].strings.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["A3", "DN80"]);
    }

    #[test]
    fn attribute_fragment_rejects_control_char_string() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&[0u8; 8]);
        payload.extend_from_slice(&2u16.to_le_bytes());
        // U+0001 U+0002 — control chars, must reject the whole record.
        payload.extend_from_slice(&[0x01, 0x00, 0x02, 0x00]);
        let mut rec = Vec::new();
        rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        rec.extend_from_slice(&payload);
        assert!(decode_attribute_fragments(&rec).is_empty());
    }

    #[test]
    fn attribute_fragment_skips_truncated_length_prefix() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&[0u8; 8]);
        payload.extend_from_slice(&100u16.to_le_bytes());
        payload.extend_from_slice(&[0x41, 0x00]); // only 1 char available, len says 100
        let mut rec = Vec::new();
        rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        rec.extend_from_slice(&payload);
        assert!(decode_attribute_fragments(&rec).is_empty());
    }

    #[test]
    fn attribute_fragment_rejects_wrong_type_code() {
        let mut rec = build_attribute_fragment(1, [0; 8], &["A3"]);
        rec[0] = 0x18; // igLine2d type code
        assert!(decode_attribute_fragments(&rec).is_empty());
    }

    #[test]
    fn attribute_fragment_rejects_too_short_payload() {
        // payload < marker(4)+aux(8)+len(2) = 14
        let payload = vec![0u8; 10];
        let mut rec = Vec::new();
        rec.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        rec.extend_from_slice(&payload);
        assert!(decode_attribute_fragments(&rec).is_empty());
    }

    #[test]
    fn attribute_fragment_at_returns_some_for_valid_offset() {
        let rec = build_attribute_fragment(0x0001_0002, [0; 8], &["DN80"]);
        let f = decode_attribute_fragment_at(&rec, 0).expect("valid offset must decode");
        assert_eq!(f.strings[0].text, "DN80");
        assert_eq!(f.byte_range, 0..rec.len());
    }

    #[test]
    fn attribute_fragment_decoder_is_panic_safe() {
        let rec = build_attribute_fragment(
            0x0001_0002,
            [1, 2, 3, 4, 5, 6, 7, 8],
            &["ODOIL020150 MM", "DN80"],
        );
        for trunc in 0..rec.len() {
            let _ = decode_attribute_fragments(&rec[..trunc]);
        }
        let noise: Vec<u8> = (0..4096u32).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_attribute_fragments(&noise);
        let _ = decode_attribute_fragments(&vec![0xFFu8; 2048]);
    }

    // Phase 18: PSM 0x0010 sub-record family audit-only decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_sub_record_0x0010(type_flags: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + payload.len());
        let type_word = PSM_TYPE_CODE_SUB_RECORD_0X0010 | ((type_flags & 0x3) << 14);
        out.extend_from_slice(&type_word.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn sub_record_0x0010_decodes_canonical_payload() {
        let payload: Vec<u8> = (0..16u8).collect();
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        let decoded = decode_sub_records_0x0010(&record);
        assert_eq!(decoded.len(), 1);
        let rec = &decoded[0];
        assert_eq!(rec.byte_range, 0..(6 + payload.len()));
        assert_eq!(rec.type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
        assert_eq!(rec.type_flags, 0);
        assert_eq!(rec.bytes_to_follow as usize, payload.len());
        assert_eq!(rec.raw_payload, payload);
        // Phase 19: leading_word = u16::from_le_bytes([payload[0],
        // payload[1]]) = LE(0x00, 0x01) = 0x0100.
        assert_eq!(rec.leading_word, Some(0x0100));
    }

    #[test]
    fn sub_record_0x0010_decoded_at_returns_some_for_valid_offset() {
        let payload = vec![0xAAu8; 32];
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        let rec = decode_sub_record_0x0010_at(&record, 0).expect("valid offset must decode");
        assert_eq!(rec.bytes_to_follow, 32);
        assert_eq!(rec.raw_payload.len(), 32);
    }

    #[test]
    fn sub_record_0x0010_rejects_wrong_type_code() {
        let payload = vec![0u8; 16];
        let mut record = build_synthetic_sub_record_0x0010(0, &payload);
        // Flip type_code to 0x0018 (igLine2d).
        record[0] = 0x18;
        record[1] = 0x00;
        assert!(decode_sub_records_0x0010(&record).is_empty());
    }

    #[test]
    fn sub_record_0x0010_rejects_zero_bytes_to_follow() {
        let payload = vec![];
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        // bytes_to_follow = 0 < SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW (8).
        assert!(decode_sub_records_0x0010(&record).is_empty());
    }

    #[test]
    fn sub_record_0x0010_rejects_below_minimum_bytes_to_follow() {
        // bytes_to_follow = 7 < min (8). Must build a 13-byte buffer
        // so the header fits but payload is too small to be accepted.
        let mut record = Vec::with_capacity(6 + 7);
        record.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        record.extend_from_slice(&7u32.to_le_bytes());
        record.extend_from_slice(&[0u8; 7]);
        assert!(decode_sub_records_0x0010(&record).is_empty());
    }

    #[test]
    fn sub_record_0x0010_rejects_oversized_bytes_to_follow() {
        // Encode a fake bytes_to_follow well above the cap; we only
        // need the header (6 bytes) — payload bounds will reject the
        // record anyway because data.len() is small.
        let mut record = Vec::with_capacity(6);
        record.extend_from_slice(&PSM_TYPE_CODE_SUB_RECORD_0X0010.to_le_bytes());
        record.extend_from_slice(&(SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW + 1).to_le_bytes());
        let mut padded = record.clone();
        padded.extend(std::iter::repeat_n(0u8, 16));
        assert!(decode_sub_records_0x0010(&padded).is_empty());
    }

    #[test]
    fn sub_record_0x0010_rejects_truncated_payload() {
        let payload = vec![0u8; 16];
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        // Drop the last 4 payload bytes; bytes_to_follow header still
        // says 16 but only 12 are available.
        let truncated = &record[..record.len() - 4];
        assert!(decode_sub_records_0x0010(truncated).is_empty());
    }

    #[test]
    fn sub_record_0x0010_returns_empty_for_short_or_empty_input() {
        assert!(decode_sub_records_0x0010(&[]).is_empty());
        // 13 bytes = exactly header (6) + min payload (8) - 1.
        assert!(decode_sub_records_0x0010(&[0u8; 13]).is_empty());
    }

    #[test]
    fn sub_record_0x0010_decoder_is_panic_safe_on_short_input() {
        let payload = vec![0xCCu8; 16];
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        for trunc_len in 0..record.len() {
            // Should not panic; truncated inputs return an empty vec.
            let _ = decode_sub_records_0x0010(&record[..trunc_len]);
        }
    }

    #[test]
    fn sub_record_0x0010_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_sub_records_0x0010(&noise);
        assert!(decode_sub_records_0x0010(&vec![0u8; 4096]).is_empty());
        // All-0xFF stream never matches type_code 0x0010 (low 14 bits
        // would be 0x3FFF, not 0x0010).
        assert!(decode_sub_records_0x0010(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn sub_record_0x0010_decodes_two_back_to_back_records() {
        let payload_a = vec![0xAAu8; 16];
        let payload_b = vec![0xBBu8; 24];
        let mut data = build_synthetic_sub_record_0x0010(0, &payload_a);
        data.extend_from_slice(&build_synthetic_sub_record_0x0010(0, &payload_b));
        let decoded = decode_sub_records_0x0010(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].raw_payload, payload_a);
        assert_eq!(decoded[1].raw_payload, payload_b);
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
    }

    #[test]
    fn sub_record_0x0010_preserves_type_flags() {
        let payload = vec![0u8; 16];
        let record = build_synthetic_sub_record_0x0010(0b11, &payload);
        let decoded = decode_sub_records_0x0010(&record);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].type_flags, 0b11);
        assert_eq!(decoded[0].type_code, PSM_TYPE_CODE_SUB_RECORD_0X0010);
    }

    // -----------------------------------------------------------------
    // Phase 19: PSM 0x0010 leading_word audit field
    // -----------------------------------------------------------------

    #[test]
    fn sub_record_0x0010_leading_word_matches_first_two_payload_bytes_le() {
        // Payload starts with the Phase-19-probe-confirmed dominant
        // leading word 0x0002 (28% of cross-fixture records).
        let mut payload = vec![0x02u8, 0x00];
        payload.extend(std::iter::repeat_n(0xAAu8, 14));
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        let decoded = decode_sub_records_0x0010(&record);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].leading_word, Some(0x0002));

        // Same exercise with 0x0003 (the second-most-common leading
        // word per Phase 19 probe, 3.6% of records).
        let mut payload = vec![0x03u8, 0x00];
        payload.extend(std::iter::repeat_n(0xBBu8, 14));
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        let decoded = decode_sub_records_0x0010(&record);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].leading_word, Some(0x0003));

        // Non-trivial byte-order check: payload starts with 0x1C 0x4E
        // → LE u16 = 0x4E1C (one of the size-16 bucket dominant words).
        let mut payload = vec![0x1Cu8, 0x4E];
        payload.extend(std::iter::repeat_n(0xCCu8, 14));
        let record = build_synthetic_sub_record_0x0010(0, &payload);
        let decoded = decode_sub_records_0x0010(&record);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].leading_word, Some(0x4E1C));
    }

    #[test]
    fn sub_record_0x0010_leading_word_is_none_for_sub_two_byte_payload() {
        // The decoder enforces bytes_to_follow >= 8, so the only way
        // to observe `leading_word == None` is to construct the DTO
        // manually with a < 2 byte raw_payload. This tests that the
        // schema honors that contract (and protects against a future
        // refactor that drops the Option<>).
        let dto = SheetSubRecord0x0010Decoded {
            byte_range: 0..6,
            type_code: PSM_TYPE_CODE_SUB_RECORD_0X0010,
            type_flags: 0,
            bytes_to_follow: 0,
            raw_payload: Vec::new(),
            leading_word: None,
        };
        assert_eq!(dto.leading_word, None);
        let dto_one = SheetSubRecord0x0010Decoded {
            leading_word: None,
            raw_payload: vec![0x42],
            bytes_to_follow: 1,
            ..dto.clone()
        };
        assert_eq!(dto_one.leading_word, None);
    }

    // Phase 25-A spatial-distribution analysis tests.

    #[test]
    fn spatial_analysis_empty_pairs_yield_zero_clusters_and_uniform_true() {
        let report = coordinate_pair_spatial_analysis(&[], 20);
        assert_eq!(report.pair_count, 0);
        assert_eq!(report.grid_resolution, 20);
        assert!(report.clusters.is_empty());
        assert!(report.uniform_distribution);
    }

    #[test]
    fn spatial_analysis_single_pair_emits_one_cluster_centered_on_pair() {
        let pairs = [(0.4, 0.6)];
        let report = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(report.pair_count, 1);
        assert_eq!(report.clusters.len(), 1);
        assert!(report.uniform_distribution);
        let cluster = &report.clusters[0];
        assert_eq!(cluster.id, 0);
        assert_eq!(cluster.pair_count, 1);
        assert_eq!(cluster.bbox, ((0.4, 0.6), (0.4, 0.6)));
        assert_eq!(cluster.centroid, (0.4, 0.6));
    }

    #[test]
    fn spatial_analysis_two_pairs_in_same_grid_cell_collapse_to_one_cluster() {
        let pairs = [(0.05, 0.05), (0.06, 0.06)];
        let report = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(report.clusters.len(), 1, "same grid cell ⇒ 1 cluster");
        assert_eq!(report.clusters[0].pair_count, 2);
    }

    #[test]
    fn spatial_analysis_two_pairs_in_4_neighbour_cells_merge_to_one_cluster() {
        // (0.05, 0.05) lives in cell (1, 1); (0.1, 0.05) lives in
        // cell (2, 1). Sharing an edge — should merge.
        let pairs = [(0.05, 0.05), (0.1, 0.05)];
        let report = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(report.clusters.len(), 1);
        assert_eq!(report.clusters[0].pair_count, 2);
    }

    #[test]
    fn spatial_analysis_two_pairs_in_diagonal_cells_remain_two_clusters() {
        // (0.025, 0.025) → cell (0, 0); (0.075, 0.075) → cell (1, 1).
        // 4-neighbour adjacency excludes diagonals → 2 distinct
        // clusters.
        let pairs = [(0.025, 0.025), (0.075, 0.075)];
        let report = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(report.clusters.len(), 2);
        assert!(!report.uniform_distribution);
    }

    #[test]
    fn spatial_analysis_far_apart_pairs_yield_two_clusters_with_correct_bboxes() {
        let pairs = [(0.1, 0.1), (0.1, 0.15), (0.9, 0.9)];
        let report = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(report.clusters.len(), 2);
        let cluster_a = report
            .clusters
            .iter()
            .find(|c| c.pair_count == 2)
            .expect("expected 2-pair cluster near (0.1, 0.1)");
        let cluster_b = report
            .clusters
            .iter()
            .find(|c| c.pair_count == 1)
            .expect("expected 1-pair cluster near (0.9, 0.9)");
        assert_eq!(cluster_a.bbox, ((0.1, 0.1), (0.1, 0.15)));
        assert_eq!(cluster_b.bbox, ((0.9, 0.9), (0.9, 0.9)));
    }

    #[test]
    fn spatial_analysis_n_grid_zero_is_promoted_to_one() {
        let pairs = [(0.1, 0.1), (0.9, 0.9)];
        let report = coordinate_pair_spatial_analysis(&pairs, 0);
        assert_eq!(report.grid_resolution, 1);
        assert_eq!(report.clusters.len(), 1, "n_grid=1 collapses all pairs");
        assert_eq!(report.clusters[0].pair_count, 2);
    }

    #[test]
    fn spatial_analysis_n_grid_one_collapses_all_pairs_to_single_cluster() {
        let pairs = [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)];
        let report = coordinate_pair_spatial_analysis(&pairs, 1);
        assert_eq!(report.clusters.len(), 1);
        assert_eq!(report.clusters[0].pair_count, 3);
        assert!(report.uniform_distribution);
    }

    #[test]
    fn spatial_analysis_clamps_nan_infinity_and_out_of_range_inputs() {
        let pairs = [
            (f64::NAN, 0.5),
            (f64::INFINITY, 0.5),
            (-0.5, 0.5),
            (1.5, 0.5),
        ];
        let report = coordinate_pair_spatial_analysis(&pairs, 4);
        assert_eq!(report.pair_count, 4);
        for cluster in &report.clusters {
            let ((min_x, min_y), (max_x, max_y)) = cluster.bbox;
            assert!(
                (0.0..=1.0).contains(&min_x)
                    && (0.0..=1.0).contains(&min_y)
                    && (0.0..=1.0).contains(&max_x)
                    && (0.0..=1.0).contains(&max_y),
                "clamped bbox should stay in [0, 1]² for cluster {cluster:?}"
            );
        }
    }

    #[test]
    fn spatial_analysis_is_deterministic_across_invocations() {
        let pairs: Vec<(f64, f64)> = (0..50)
            .map(|i| (i as f64 / 100.0, (i as f64 * 1.3) % 1.0))
            .collect();
        let a = coordinate_pair_spatial_analysis(&pairs, 20);
        let b = coordinate_pair_spatial_analysis(&pairs, 20);
        assert_eq!(a, b, "spatial analysis must be deterministic");
    }

    #[test]
    fn spatial_analysis_uniform_distribution_flag_matches_cluster_count() {
        let single = coordinate_pair_spatial_analysis(&[(0.5, 0.5)], 20);
        assert!(single.uniform_distribution);

        let multi = coordinate_pair_spatial_analysis(&[(0.1, 0.1), (0.9, 0.9)], 20);
        assert!(!multi.uniform_distribution);

        let empty = coordinate_pair_spatial_analysis(&[], 20);
        assert!(empty.uniform_distribution);
    }

    // -----------------------------------------------------------------
    // Phase 34-D: PSM igBoundary2d (0x0013) decoder tests
    // -----------------------------------------------------------------

    /// One `(start, end)` synthetic segment for the igBoundary2d
    /// test builder.
    type TestSegment = ((f64, f64), (f64, f64));

    /// Build a synthetic PSM `igBoundary2d` record mirroring the
    /// fixture layout proven by
    /// `examples/probe_0013_igboundary2d_grammar.rs`: 6-byte PSM
    /// header + `49 + 41 × n` payload.
    fn build_synthetic_igboundary2d_record(
        oid: u32,
        parent_ref: u32,
        index: u32,
        segments: &[TestSegment],
        anchor: (f64, f64),
        member_oids: &[u32],
    ) -> Vec<u8> {
        build_synthetic_igboundary2d_record_on_layer(
            oid,
            parent_ref,
            12,
            index,
            segments,
            anchor,
            member_oids,
        )
    }

    /// Same, with the sheet-layer reference at payload `+8` chosen by
    /// the caller — the field the decoder used to gate on.
    #[allow(clippy::too_many_arguments)]
    fn build_synthetic_igboundary2d_record_on_layer(
        oid: u32,
        parent_ref: u32,
        sheet_layer_ref: u32,
        index: u32,
        segments: &[TestSegment],
        anchor: (f64, f64),
        member_oids: &[u32],
    ) -> Vec<u8> {
        let n = segments.len();
        assert_eq!(n, member_oids.len(), "test builder invariant");
        let btf = (IGBOUNDARY2D_FIXED_PAYLOAD_LEN + IGBOUNDARY2D_PER_SEGMENT_LEN * n) as u32;
        let mut out = Vec::with_capacity(6 + btf as usize);
        out.extend_from_slice(&PSM_TYPE_CODE_IGBOUNDARY2D.to_le_bytes());
        out.extend_from_slice(&btf.to_le_bytes());
        // 18-byte IGDS prefix.
        out.extend_from_slice(&oid.to_le_bytes());
        out.extend_from_slice(&parent_ref.to_le_bytes());
        out.extend_from_slice(&sheet_layer_ref.to_le_bytes());
        out.extend_from_slice(&0x0010u16.to_le_bytes());
        out.extend_from_slice(&index.to_le_bytes());
        // 10-byte sub-header: u32 == 1, u32 segment_count, bytes [2, 1].
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(n as u32).to_le_bytes());
        out.push(2);
        out.push(1);
        // Segment groups.
        for (start, end) in segments {
            out.push(IGBOUNDARY2D_SEGMENT_TAG);
            out.extend_from_slice(&start.0.to_le_bytes());
            out.extend_from_slice(&start.1.to_le_bytes());
            out.extend_from_slice(&end.0.to_le_bytes());
            out.extend_from_slice(&end.1.to_le_bytes());
        }
        // Anchor + flag + member count + member refs.
        out.extend_from_slice(&anchor.0.to_le_bytes());
        out.extend_from_slice(&anchor.1.to_le_bytes());
        out.push(1);
        out.extend_from_slice(&(n as u32).to_le_bytes());
        for (i, member_oid) in member_oids.iter().enumerate() {
            out.extend_from_slice(&member_oid.to_le_bytes());
            out.extend_from_slice(&0x00CBu16.to_le_bytes());
            out.extend_from_slice(&(if i == 0 { 13u16 } else { 12u16 }).to_le_bytes());
        }
        out
    }

    /// Closed fixture-shaped triangle: three chained segments.
    fn canonical_igboundary_triangle() -> Vec<u8> {
        build_synthetic_igboundary2d_record(
            81,
            71,
            21,
            &[
                ((0.2025, 0.2202), (0.1993, 0.2210)),
                ((0.1993, 0.2210), (0.1993, 0.2194)),
                ((0.1993, 0.2194), (0.2025, 0.2202)),
            ],
            (0.2001, 0.2204),
            &[70, 83, 82],
        )
    }

    #[test]
    fn igboundary2d_decodes_canonical_closed_triangle() {
        let record = canonical_igboundary_triangle();
        assert_eq!(record.len(), 6 + 172, "3-segment record is 178 bytes");
        let decoded = decode_igboundaries(&record);
        assert_eq!(decoded.len(), 1);
        let b = &decoded[0];
        assert_eq!(b.type_code, PSM_TYPE_CODE_IGBOUNDARY2D);
        assert_eq!(b.bytes_to_follow, 172);
        assert_eq!(b.oid, 81);
        assert_eq!(b.parent_ref, 71);
        assert_eq!(b.sub_type_word, 0x0010);
        assert_eq!(b.index, 21);
        assert_eq!(b.segment_count, 3);
        assert_eq!(b.sub_header_tail, [2, 1]);
        assert_eq!(b.segments.len(), 3);
        assert_eq!(b.segments[0].tag_offset, 28);
        assert_eq!(b.segments[1].tag_offset, 61);
        assert_eq!(b.segments[2].tag_offset, 94);
        assert!((b.segments[0].start.0 - 0.2025).abs() < 1e-12);
        assert!((b.anchor.0 - 0.2001).abs() < 1e-12);
        assert_eq!(b.trailer_flag, 1);
        assert_eq!(
            b.member_refs,
            vec![
                SheetIgBoundary2dMemberRef {
                    member_oid: 70,
                    class_word: 0x00CB,
                    sub_word: 13
                },
                SheetIgBoundary2dMemberRef {
                    member_oid: 83,
                    class_word: 0x00CB,
                    sub_word: 12
                },
                SheetIgBoundary2dMemberRef {
                    member_oid: 82,
                    class_word: 0x00CB,
                    sub_word: 12
                },
            ]
        );
        assert!(b.is_closed_loop(1e-9));
        assert_eq!(b.byte_range, 0..(6 + 172));
    }

    #[test]
    fn igboundary2d_rejects_wrong_type_code() {
        let mut record = canonical_igboundary_triangle();
        record[0] = 0x18; // igLine2d type code instead
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_bytes_to_follow_off_formula() {
        let mut record = canonical_igboundary_triangle();
        record[2] = 171; // 172 -> 171 breaks `49 + 41 × n`
        record.truncate(6 + 171);
        assert!(decode_igboundaries(&record).is_empty());
    }

    /// Payload `+8` names the sheet layer the boundary sits on, so
    /// every value has to survive. The corpus census behind
    /// `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md` found
    /// nine `igBoundary2d` on layers `156` / `199` / `465` that the
    /// old `== 12` rule refused.
    #[test]
    fn igboundary2d_accepts_every_sheet_layer_the_corpus_carries() {
        for sheet_layer_ref in [0u32, 8, 12, 156, 199, 465, u32::MAX] {
            let record = build_synthetic_igboundary2d_record_on_layer(
                81,
                71,
                sheet_layer_ref,
                21,
                &[
                    ((0.2025, 0.2202), (0.1993, 0.2210)),
                    ((0.1993, 0.2210), (0.1993, 0.2194)),
                    ((0.1993, 0.2194), (0.2025, 0.2202)),
                ],
                (0.2004, 0.2202),
                &[91, 92, 93],
            );
            let decoded = decode_igboundary_at(&record, 0)
                .unwrap_or_else(|| panic!("sheet_layer_ref {sheet_layer_ref} refused"));
            assert_eq!(
                decoded.sheet_layer_ref, sheet_layer_ref,
                "carried through verbatim"
            );
        }
    }

    #[test]
    fn igboundary2d_rejects_wrong_sub_type_word() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 12] = 0x11;
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_wrong_sub_header_flag() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 18] = 2; // u32 at +18 must equal 1
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_missing_segment_tag() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 61] = 0x68; // second group tag corrupted
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_member_count_mismatch() {
        let mut record = canonical_igboundary_triangle();
        // member_count u32 lives at payload +144 (pos=127 + 17).
        record[6 + 144] = 4;
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_nan_segment_coordinate() {
        let mut record = canonical_igboundary_triangle();
        let nan = f64::NAN.to_le_bytes();
        record[6 + 29..6 + 37].copy_from_slice(&nan); // first segment start.x
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_out_of_domain_anchor() {
        let mut record = canonical_igboundary_triangle();
        let big = 1e10f64.to_le_bytes();
        record[6 + 127..6 + 135].copy_from_slice(&big); // anchor.x
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_degenerate_all_same_vertices() {
        let p = (0.5, 0.5);
        let record = build_synthetic_igboundary2d_record(
            1,
            1,
            21,
            &[(p, p), (p, p), (p, p)],
            p,
            &[10, 11, 12],
        );
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_rejects_zero_segment_count() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 22..6 + 26].copy_from_slice(&0u32.to_le_bytes());
        assert!(decode_igboundaries(&record).is_empty());
    }

    #[test]
    fn igboundary2d_open_chain_reports_not_closed() {
        let record = build_synthetic_igboundary2d_record(
            1,
            1,
            21,
            &[
                ((0.1, 0.1), (0.2, 0.1)),
                ((0.2, 0.1), (0.2, 0.2)),
                // Last segment does not return to (0.1, 0.1).
                ((0.2, 0.2), (0.3, 0.3)),
            ],
            (0.2, 0.15),
            &[10, 11, 12],
        );
        let decoded = decode_igboundaries(&record);
        assert_eq!(decoded.len(), 1);
        assert!(!decoded[0].is_closed_loop(1e-9));
    }

    #[test]
    fn igboundary2d_decoder_is_panic_safe_on_short_input() {
        let record = canonical_igboundary_triangle();
        for trunc_len in 0..record.len() {
            assert!(
                decode_igboundaries(&record[..trunc_len]).is_empty(),
                "truncated input length {trunc_len} must not decode"
            );
        }
        assert!(decode_igboundaries(&[]).is_empty());
        assert!(decode_igboundary_at(&record, record.len()).is_none());
        assert!(decode_igboundary_at(&record, usize::MAX).is_none());
    }

    #[test]
    fn igboundary2d_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igboundaries(&noise);
        assert!(decode_igboundaries(&vec![0u8; 4096]).is_empty());
        assert!(decode_igboundaries(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn igboundary2d_decodes_two_back_to_back_records() {
        let mut data = canonical_igboundary_triangle();
        data.extend(build_synthetic_igboundary2d_record(
            522,
            526,
            24,
            &[
                ((0.4, 0.4), (0.5, 0.4)),
                ((0.5, 0.4), (0.5, 0.5)),
                ((0.5, 0.5), (0.4, 0.4)),
            ],
            (0.47, 0.43),
            &[521, 524, 523],
        ));
        let decoded = decode_igboundaries(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 81);
        assert_eq!(decoded[1].oid, 522);
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
        assert!(decoded[1].is_closed_loop(1e-9));
    }
}
