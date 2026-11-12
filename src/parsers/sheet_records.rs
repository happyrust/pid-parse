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
/// The `oid` and the eight-byte `aux` pair that follow are the first
/// twelve bytes of the payload this length is counted from.
pub const PSM_ENVELOPE_LEN: usize = 6;

/// The decoded 6-byte PSM record envelope shared by every PSM record
/// family (see [`parse_psm_header`]).
///
/// Deliberately minimal: `oid` is **not** part of the shared envelope —
/// every family carries it in its payload sub-header and reads its own
/// fields after [`Self::body_start`].
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

/// The bit of [`PsmHeader::type_flags`] the native reader tests before it
/// reads anything else of a record: the type word's `0x8000`, which lands
/// in the upper of the two flag bits.
///
/// `PSMSerializeIn` (`radsrvitem.dll` `0x564915E0`) reads the type word
/// and `bytes_to_follow`, and when `type_word & 0x8000` is set it seeks
/// past the payload and returns without reading the oid — the record is
/// not an object to it (`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`,
/// the `PSMSerializeIn` section). On the corpus every record so flagged is an earlier
/// copy of a live record: same oid, same insertion, same definition, the
/// live one always later in the stream — or, for DWG-0202's orphan
/// `/Sheet6615`, a whole storage the drawing no longer uses. 42 of them
/// across the five fixtures (27 `igSymbol2d`, 8 `igLine2d`, 5
/// `igSmartFrame2d`, 1 `igTextBox`, 1 `igRectangle2d`;
/// `examples/probe_run_conflicts.rs` and
/// `tests/render_gap_census.rs`). Nothing sets the other bit (`0x4000`)
/// anywhere in the corpus.
pub const PSM_TYPE_FLAG_NATIVE_SKIP: u16 = 0b10;

/// The other bit of [`PsmHeader::type_flags`]: the type word's `0x4000`.
/// No record of any chain stream in the corpora sets it, and a chain walk
/// does not read past a word that does ([`psm_type_word_opens_a_record`]).
pub const PSM_TYPE_FLAG_UNSEEN: u16 = 0b01;

/// The last type code the native reader has a class for.
///
/// `radsrvitem.dll` keeps one CLSID per type code in a table the code
/// indexes (`dword_5667B068`, 20 bytes a slot;
/// `docs/analysis/2026-08-04-psm-type-code-registry.md`). It has 281
/// slots, `0x0000` to `0x0118`. What says the table ends there is what
/// follows slot `0x0118` in the file: four zero bytes, three doubles
/// (`1e-6`, `1e-10`, `0.001`) and then pointers into the image — not a
/// GUID. (The slots are not in CLSID order, which this comment said until
/// 2026-11-11: they ascend from `0x0000` through `0x0088` and not after,
/// 78 slots sorting below the one before them.) 230 of the slots name a
/// class of the RAD registry in `jutil.dll`, 50 hold a CLSID that
/// registry does not list, and one, [`PSM_TYPE_CODE_EMPTY_SLOT`], holds
/// none.
///
/// The table was read from one build of the reader, `radsrvitem.dll`
/// 9.0.0.143. A later build that has a class past `0x0118` writes records
/// this walk stops on: a drawing of it would be read as far as the first
/// such record and reported with a
/// [`SheetChainStop::NoSuchClass`] fault — loudly, and short of the rest
/// of the stream. No record of the corpora states a type code the table
/// lacks.
pub const PSM_TYPE_CODE_LAST: u16 = 0x0118;

/// The one slot of the native reader's class table that holds no CLSID.
pub const PSM_TYPE_CODE_EMPTY_SLOT: u16 = 0x00F8;

/// Whether `type_word` can open a record of a chain stream: the tripwire
/// [`SheetChain::walk`] stops on.
///
/// A record's length is the only thing that says where the next one
/// starts, so a length that is wrong and still inside the stream sends the
/// walk on from an offset where no record starts, reading whatever bytes
/// lie there as an envelope. Nothing about a length says it is wrong; the
/// type word beside it can. A word is refused when
///
/// * its type code is past [`PSM_TYPE_CODE_LAST`] or is
///   [`PSM_TYPE_CODE_EMPTY_SLOT`]: the native reader has no class to make
///   of it, so no file it wrote or can read holds such a record;
/// * its `0x4000` bit is set ([`PSM_TYPE_FLAG_UNSEEN`]). `PSMSerializeIn`
///   holds the word of every record it reads against the 14-bit type code
///   of the object the record's oid names, and reports "Stream offset
///   mismatch while reading serial data" when they differ
///   (`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`): a
///   record it reads cannot carry the bit, and it names the failure this
///   rule is for;
/// * its type code is zero. The class table does not rule that out — slot
///   `0` is `JFeatureControlFrame Object` — but zero is the commonest word
///   a payload holds, and no record states it.
///
/// None of the three occurs once on the 499 289 records of the 3 984
/// distinct streams that open with the chain magic in the corpora (sheets,
/// symbol caches, `StyleCluster`, the `PSMcluster` and attribute streams,
/// the sheets of symbol files). Changing one length field by ±1, ±2 … ±64
/// on every record of the corpora's sheets and symbol caches, 353 changes
/// used to read as a sound chain: 351 through a "record" of type word
/// `0x0000`, the other two through `0x4000` and `0xF000`.
///
/// The rule is the class table's and not the list of codes the corpora
/// happen to carry (18 on sheets, 49 in symbol caches). That list refuses
/// more — of 2 106 074 length changes on the corpora's sheets it leaves
/// 1 156 reading a record off the chain, where this rule leaves 35 074 and
/// no rule 620 049 — and it would stop a sound stream at the first record
/// of a class these corpora lack: `igBalloon`, `igLeader` and `Graphics
/// Bag` pass the native graphic predicate and no corpus chain holds one. A
/// stop is loud either way; one on a sound drawing costs the rest of its
/// sheet. `tests/sheet_chain_faults.rs` makes the same changes on the
/// committed streams on every run and pins what each reads as.
#[must_use]
pub fn psm_type_word_opens_a_record(type_word: u16) -> bool {
    let code = type_word & 0x3FFF;
    type_word & 0x4000 == 0
        && code != 0
        && code <= PSM_TYPE_CODE_LAST
        && code != PSM_TYPE_CODE_EMPTY_SLOT
}

impl PsmHeader {
    /// Whether the native reader skips this record without reading it —
    /// [`PSM_TYPE_FLAG_NATIVE_SKIP`] is set in its type word.
    #[must_use]
    pub fn native_reader_skips(&self) -> bool {
        self.type_flags & PSM_TYPE_FLAG_NATIVE_SKIP != 0
    }

    /// The type word as stored: the 14-bit code under its two flag bits.
    #[must_use]
    pub fn type_word(&self) -> u16 {
        (self.type_flags << 14) | (self.type_code & 0x3FFF)
    }
}

/// Parse the shared 6-byte PSM record envelope at `offset`.
///
/// Returns `None` when fewer than [`PSM_ENVELOPE_LEN`] bytes remain at
/// `offset` (or `offset` overflows). Performs **no** family validation:
/// callers must check [`PsmHeader::type_code`] / bounds themselves.
/// Panic-free on arbitrary input.
///
/// Reads flagged records too — this is the envelope as stored, and the
/// census and the probes need to see a record the native reader skips.
/// A family decoder goes through [`parse_live_psm_header`] instead.
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

/// [`parse_psm_header`] for a family decoder: `None` as well when the
/// native reader would skip the record ([`PsmHeader::native_reader_skips`]).
///
/// Every `decode_at` starts here, so a superseded copy of a record is not
/// decoded by any family, not emitted as geometry, and not indexed by
/// `style_link` — the drawing it yields is the one the native reader
/// draws. Such a record is counted apart from the refusals by
/// [`crate::parsers::undecoded_census::native_skipped_record_census`]: it
/// is not a shape a decoder failed on, it is a record the file itself has
/// retired.
pub fn parse_live_psm_header(data: &[u8], offset: usize) -> Option<PsmHeader> {
    parse_psm_header(data, offset).filter(|header| !header.native_reader_skips())
}

/// Offset of the first record in a `Sheet*` stream: the 8-byte stream
/// header (`magic` + `record_count`) precedes the chain.
pub const SHEET_STREAM_HEADER_LEN: usize = 8;

/// The word every chain stream opens with: the cluster magic.
pub const SHEET_STREAM_MAGIC: u32 = crate::parsers::cluster_header::CLUSTER_MAGIC;

/// Why a chain walk stopped before the end of its stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetChainStop {
    /// The stream is shorter than the eight-byte header a chain stream opens
    /// with.
    ShortHeader,
    /// Fewer than six bytes are left at the stop: too few for a record's
    /// envelope.
    ShortEnvelope,
    /// The record at the stop states that no byte follows its envelope. A
    /// record holds at least its own oid, and six bytes on from here is the
    /// middle of whatever this one really held.
    ZeroLength,
    /// The record at the stop states more bytes than the stream has left.
    PastEnd {
        /// The `bytes_to_follow` it states.
        stated: u32,
    },
    /// The word at the stop is not one a record opens with
    /// ([`psm_type_word_opens_a_record`]): the walk has left the record
    /// boundaries by here, and the length of the last record it read is
    /// the first thing in doubt.
    NoSuchClass {
        /// The type word found there.
        type_word: u16,
    },
}

/// A chain stream — a `Sheet*` stream or a symbol cache's `PSMcluster0` —
/// read by its own framing: the eight-byte header (`magic`, live-record
/// count) and then records end to end, `u16` type word, `u32
/// bytes_to_follow`, payload, repeat.
///
/// The walk is what tells a record from a byte pattern that merely looks
/// like one, and it is where every family decodes
/// ([`PsmRecordDecoder::scan`]). A walk that tries every offset and slides a
/// byte on rejection accepts a header shape occurring *inside* another
/// record's payload as a record — which is how `0x3FE6 GLine2d` came to have
/// three corpus "records" that are really the top two bytes of an
/// `igSmartFrame2d` page-ratio `f64` (see
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`),
/// and how three whole families came to decode nothing but such patterns
/// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3).
///
/// A stream whose framing does not hold is read as far as it holds and no
/// further. The records the chain read before a stop decode as on any
/// stream; what follows the stop is not searched for a place to pick the
/// chain up again.
///
/// "The records the chain read", not "the records the file holds": each
/// one's offset came from the lengths before it, and a length can be wrong
/// without running past the stream. The walk then goes on from an offset
/// where no record starts. It stops there when the word it finds is not
/// one a record opens with ([`psm_type_word_opens_a_record`]) — on 98.3%
/// of the length changes measured on the corpora's sheets that is the very
/// next offset, so the stop names the first byte past the record whose
/// length is wrong — and otherwise where the bytes it took for records run
/// out: on the other 1.7% it reads one such record, rarely two, before it
/// stops, and on 450 of 2 106 074 it rejoins the chain further on and
/// reaches the stream's end, where only the header's count says something
/// is wrong and nothing says where. What it read on the way is not in the
/// file as a record, and the families that could decode one are the ones
/// whose type code it happened to state.
///
/// `examples/probe_can_a_stopped_chain_be_picked_up_again`
/// measures what such a search would find if each record of each intact
/// sheet stream in turn were the one the walk stops on. Of the 173 982
/// records of the corpora's sheet streams that have another after them,
/// 21 214 hold an offset inside themselves from which a chain walks exactly
/// to the stream's end; held to the header's live-record count as well, the
/// search still lands inside the record, short of the next record's start,
/// on 743. A payload read as records of its own is geometry no file holds
/// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §15). Instead the stop is stated — [`Self::stop`], [`Self::unread_bytes`]
/// — for the caller to report, which is what
/// [`crate::model::StreamChainFault`] is for.
///
/// The header is held to the chain as well: on every chain stream of the
/// corpora — the 1 684 sheet streams and 171 symbol caches of their 856
/// distinct files, 259 821 records, `tests/sheet_chain_faults.rs` — the
/// first word is the magic and the second is the number of records the
/// native reader reads (those without `0x8000` in the type word). A stream
/// cut off at a record boundary walks cleanly to its new end, and only that
/// count says records are missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetChain {
    /// The stream's length in bytes.
    pub len: usize,
    /// The stream's first word, when it has four bytes.
    pub magic: Option<u32>,
    /// The live-record count the header states, when the stream has a
    /// header.
    pub stated_records: Option<u32>,
    /// Offset of every record the walk read, in stream order.
    pub starts: Vec<usize>,
    /// How many of them the native reader reads: the ones whose type word
    /// does not carry [`PSM_TYPE_FLAG_NATIVE_SKIP`].
    pub live_records: usize,
    /// The first offset the walk did not read as a record: [`Self::len`]
    /// when it read the stream to its end.
    pub end: usize,
    /// Why the walk stopped at [`Self::end`], when that is short of the
    /// stream's end.
    pub stop: Option<SheetChainStop>,
}

impl SheetChain {
    /// Walk `data` as a chain stream.
    ///
    /// Panic-free on arbitrary input. An empty stream reads as an empty
    /// chain with nothing wrong with it: there is no byte it failed to read.
    #[must_use]
    pub fn walk(data: &[u8]) -> Self {
        let word = |at: usize| {
            data.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let mut chain = Self {
            len: data.len(),
            magic: word(0),
            stated_records: word(4),
            starts: Vec::new(),
            live_records: 0,
            end: data.len().min(SHEET_STREAM_HEADER_LEN),
            stop: None,
        };
        if data.is_empty() {
            return chain;
        }
        if data.len() < SHEET_STREAM_HEADER_LEN {
            chain.end = 0;
            chain.stop = Some(SheetChainStop::ShortHeader);
            return chain;
        }
        let mut at = SHEET_STREAM_HEADER_LEN;
        while at < data.len() {
            let Some(header) = parse_psm_header(data, at) else {
                chain.stop = Some(SheetChainStop::ShortEnvelope);
                break;
            };
            if !psm_type_word_opens_a_record(header.type_word()) {
                chain.stop = Some(SheetChainStop::NoSuchClass {
                    type_word: header.type_word(),
                });
                break;
            }
            if header.bytes_to_follow == 0 {
                chain.stop = Some(SheetChainStop::ZeroLength);
                break;
            }
            let end = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .filter(|end| *end <= data.len());
            let Some(end) = end else {
                chain.stop = Some(SheetChainStop::PastEnd {
                    stated: header.bytes_to_follow,
                });
                break;
            };
            chain.starts.push(at);
            if !header.native_reader_skips() {
                chain.live_records += 1;
            }
            at = end;
        }
        chain.end = at;
        chain
    }

    /// Bytes from where the walk stopped to the stream's end: read as
    /// nothing. Zero when the walk read the stream to its end.
    #[must_use]
    pub fn unread_bytes(&self) -> usize {
        self.len.saturating_sub(self.end)
    }

    /// Whether the stream's first word is not the chain magic. An empty
    /// stream has no first word to be wrong.
    #[must_use]
    pub fn wrong_magic(&self) -> Option<u32> {
        match self.magic {
            Some(SHEET_STREAM_MAGIC) => None,
            Some(found) => Some(found),
            // One to three bytes: a header cut short, which `stop` says.
            None => None,
        }
    }

    /// Whether the chain runs to the stream's end and holds a different
    /// number of live records than the header states.
    #[must_use]
    pub fn count_differs(&self) -> bool {
        self.stop.is_none()
            && self
                .stated_records
                .is_some_and(|stated| usize::try_from(stated).ok() != Some(self.live_records))
    }

    /// Whether the stream is what it says it is: the magic, a chain that
    /// runs to the last byte, and as many live records as the header
    /// states. An empty stream is sound.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.stop.is_none() && self.wrong_magic().is_none() && !self.count_differs()
    }

    /// Whether the walk read the stream short of its records: it stopped
    /// before the stream's end, or it reached the end with fewer live
    /// records than the header states.
    ///
    /// The second is what a stream cut off at a record boundary reads as —
    /// the walk ends cleanly on the new last byte, and only the count says
    /// records are gone. It is also what a header that counts one too many
    /// reads as, and nothing in the stream tells the two apart; a reader
    /// that must not take a short stream for a whole one takes both for
    /// short.
    ///
    /// Not short, though [`Self::is_sound`] is false: a first word that is
    /// not the magic over a chain that reaches the end, and a header that
    /// counts fewer live records than the chain holds. Neither left a
    /// record unread.
    #[must_use]
    pub fn is_short(&self) -> bool {
        self.stop.is_some()
            || self.stated_records.is_some_and(|stated| {
                usize::try_from(stated).is_ok_and(|stated| self.live_records < stated)
            })
    }
}

/// Every offset a record starts at in a chain stream that walks to its last
/// byte, and nothing for one that does not.
///
/// This is the all-or-nothing reading, for a caller that tests membership
/// of the chain or that has no way to say a stream was cut short: the
/// publish line's row reader, the semantic join, the probes. The record
/// families read [`SheetChain::walk`] instead — what the chain read before
/// a stop still decodes — and what stopped the walk is reported beside it.
/// Neither reading holds the header to the chain; [`SheetChain::is_sound`]
/// does.
#[must_use]
pub fn sheet_record_starts(data: &[u8]) -> Vec<usize> {
    let chain = SheetChain::walk(data);
    if chain.stop.is_some() {
        return Vec::new();
    }
    chain.starts
}

/// `records` — one record or a run of them, end to end — behind the 8-byte
/// header a chain stream opens with, so that a test hands a family the
/// stream it would be handed in a file.
///
/// The header is the one a file writes: the magic, and the number of records
/// among `records` the native reader reads. Bytes that are not a run of
/// whole records get a count of what the walk reads before it stops.
#[cfg(test)]
pub(crate) fn chain_stream(records: &[u8]) -> Vec<u8> {
    let mut stream = vec![0u8; SHEET_STREAM_HEADER_LEN];
    stream.extend_from_slice(records);
    let live = u32::try_from(SheetChain::walk(&stream).live_records).unwrap_or(u32::MAX);
    stream[..4].copy_from_slice(&SHEET_STREAM_MAGIC.to_le_bytes());
    stream[4..8].copy_from_slice(&live.to_le_bytes());
    stream
}

/// One PSM record family's decoder (RFC §3.1, L4 seam).
///
/// Implementations are unit structs owning only the family-specific
/// payload validation inside [`Self::decode_at`]; the walk lives once in
/// the provided [`Self::scan`], and it is the stream's own record chain.
/// The associated [`Self::Record`] keeps each family's typed parser DTO —
/// this seam deliberately does **not** collapse families into an enum or
/// an untyped map (that would erase the Typed Audit evidence language from
/// `CONTEXT.md`).
///
/// Contract for implementors:
///
/// - [`Self::decode_at`] must be panic-free on arbitrary bytes and
///   return `None` on any validation failure (same bar as every
///   `decode_*_at` free function, enforced by
///   `tests/parser_panic_safety.rs`). It judges the bytes at one offset
///   and nothing about where that offset came from; whether a record
///   starts there is [`Self::scan`]'s question, not the payload's.
/// - [`Self::advance_of`] returns the accepted record's full on-disk
///   byte length — the step the chain takes over it.
/// - No family overrides [`Self::scan`]: a record is a member of the
///   chain or it is not a record, whatever its family.
pub trait PsmRecordDecoder {
    /// Typed decoded-record DTO for this family (e.g.
    /// [`SheetIgLine2dDecoded`]).
    type Record;

    /// The 14-bit PSM type code this decoder accepts.
    fn type_code(&self) -> u16;

    /// Smallest possible full-record byte length for this family
    /// (envelope + minimum payload).
    fn min_record_len(&self) -> usize;

    /// Try to decode one record starting at `offset`. `None` when any
    /// family validation rule fails. Must be panic-free and
    /// bounds-checked.
    fn decode_at(&self, data: &[u8], offset: usize) -> Option<Self::Record>;

    /// The record's full on-disk length (envelope + payload): how far the
    /// chain steps over it.
    fn advance_of(&self, record: &Self::Record) -> usize;

    /// Every record of this family in a chain stream — a `Sheet*` stream
    /// or a symbol cache's `PSMcluster0`: [`Self::decode_at`] at each
    /// offset the stream's own chain starts a record
    /// ([`SheetChain::walk`]), in on-disk order.
    ///
    /// A record is a member of the chain before its payload is even looked
    /// at. This used to try every offset and slide a byte on rejection,
    /// which reads any run of bytes that satisfies a family's rules as a
    /// record of it wherever the run sits — inside a text box, inside a
    /// dependency object's member list — and then steps over whatever that
    /// "record" claims, real records included. Measured over 118 sheet
    /// streams and 147 symbol caches
    /// (`examples/probe_which_decoded_records_start_off_the_chain.rs`),
    /// the chain costs no family a record: 191 256 decodes started on it,
    /// and what started off it was thirteen `igSmartFrame2d` and every one
    /// of the 92 392 "records" of three families — `0x0010` sub-records,
    /// attribute fragments, `JStyleOverride` — which had none on it, since
    /// no sheet chain carries either type code.
    ///
    /// A stream whose chain stops before its end yields the records before
    /// the stop and never falls back to a scan for the rest: the stop is
    /// the caller's to report ([`SheetChain::stop`]), not to paper over.
    fn scan(&self, data: &[u8]) -> Vec<Self::Record> {
        SheetChain::walk(data)
            .starts
            .into_iter()
            .filter_map(|at| self.decode_at(data, at))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Phase 14 Slices D/F: PSM-encoded geometry decoder family
// ---------------------------------------------------------------------------

// The matchers of four retired families stood in this file until 2026-11-11:
// `GLine2d` (`0x3FE6`) below this banner, `JStyleOverride` as a sheet family
// (`0x0030`), `SubRecord0x0010` and `AttributeFragment` (`0x0010`) after the
// dependency object. No chain of any stream holds a record any of them reads,
// nothing in the library called them, and an entry point that reads untrusted
// bytes and matches nothing a file holds is not one to export. They live on
// the test side now, `tests/support/retired_matchers.rs`, for the tests and
// probes that say what their hits were.

/// PSM type code that identifies a standard Intergraph Sigma
/// `igLine2d` record on disk.
///
/// Equals the Intergraph IGDS class tag for `igLine2d` (`0x18 = 24`,
/// looked up by `radsrvitem.dll!sub_56448F70`). The fixture
/// histogram in `examples/probe_psm_type_code_histogram.rs` shows
/// 309 cross-fixture hits for this type code on `Sheet*` streams:
/// `igLine2d` is the Sigma 2D line record, and the only line record a
/// sheet holds. (Phase 14 set a "`GLine2d`, PSM `0x3FE6`" family beside
/// it as `SmartPlant`'s parametric wrapper of the same line; no such type
/// code exists — it is past [`PSM_TYPE_CODE_LAST`] — and what that family
/// matched was the top two bytes of a page frame's aspect ratio,
/// `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`.)
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
pub(crate) const IGTEXTBOX_BODY_START: usize = 22;

/// Bytes of placement/style that follow every `igTextBox` body: the
/// insertion point, the text direction, and a 4-byte trailer.
pub(crate) const IGTEXTBOX_TAIL_LEN: usize = 36;

/// Payload offset of the tail-kind selector: `1` for the placement tail
/// alone, `2` for the tail that then carries a text box.
pub(crate) const IGTEXTBOX_TAIL_KIND_AT: usize = 20;

/// The tail-kind selector of the placement tail alone.
pub(crate) const IGTEXTBOX_TAIL_KIND_PLACEMENT: u16 = 1;

/// The tail-kind selector whose tail carries the text box.
pub(crate) const IGTEXTBOX_TAIL_KIND_BOX: u16 = 2;

/// Bytes a kind-2 tail occupies before the optional fields its own flag
/// word gates — the placement tail plus the box and three counters.
pub(crate) const IGTEXTBOX_TAIL_KIND2_LEN: usize = 68;

/// Offset of the text box's width inside a kind-2 tail, in sheet units.
const IGTEXTBOX_BOX_WIDTH_AT: usize = 40;

/// Offset of the text box's height inside a kind-2 tail, in sheet units.
const IGTEXTBOX_BOX_HEIGHT_AT: usize = 48;

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
pub(crate) const IGTEXTBOX_DIRECTION_UNIT_TOLERANCE: f64 = 1e-6;

/// The band a non-unit direction pair's magnitude must land in to read as a
/// letter height rather than as a mis-stated length: half a millimetre to a
/// quarter metre of lettering. Every height-scaled direction seen is
/// 0.003–0.0045 m; the edges only have to exclude what an f64 misread
/// produces.
pub(crate) const IGTEXTBOX_LETTER_HEIGHT_M: std::ops::RangeInclusive<f64> = 0.0005..=0.25;

/// The letter height a text direction pair states, or `None` when the pair is
/// not a direction at all: `Some(0.0)` for the unit vector every drawing sheet
/// writes, `Some(h)` for a pair scaled to a letter height in
/// `IGTEXTBOX_LETTER_HEIGHT_M`.
///
/// The `None` is what refuses a text length that over-reads: it moves the
/// tail, and the two `f64`s read where the pair should be are then denormal,
/// astronomical or zero. Both `igTextBox` readers apply it, this module's and
/// `symbol_library::decode_text`.
pub(crate) fn igtextbox_letter_height(cos: f64, sin: f64) -> Option<f64> {
    let direction = cos.hypot(sin);
    if (direction - 1.0).abs() <= IGTEXTBOX_DIRECTION_UNIT_TOLERANCE {
        Some(0.0)
    } else if IGTEXTBOX_LETTER_HEIGHT_M.contains(&direction) {
        Some(direction)
    } else {
        None
    }
}

/// PSM type code that identifies a standard Intergraph Sigma
/// `igSymbol2d` record on disk (IGDS class tag `0xCE = 206`).
///
/// Cross-fixture histogram surfaces 103 hits — `SmartPlant` symbol
/// instantiations (equipment, instruments, valves).
pub const PSM_TYPE_CODE_IGSYMBOL2D: u16 = 0x00CE;

/// Minimum byte length of one PSM `igSymbol2d` payload: what the shortest
/// readable record can hold, which is the matrix tag as early as it can sit
/// (a 4-byte `JSite` id in front of it) plus [`IGSYMBOL2D_MATRIX_AND_TAIL_LEN`.
///
/// This is a bound on the record, not a corpus histogram. The known shapes
/// run 109 (the two title-block templates' placements) through 123 bytes, and
/// the tail's optional members make the length vary without touching a byte
/// the decoder reads. What a record must actually hold is checked per record
/// against the matrix the tag anchors — see `IGSYMBOL2D_MATRIX_AND_TAIL_LEN`.
pub const IGSYMBOL2D_MIN_PAYLOAD_LEN: usize = 8 + IGSYMBOL2D_MATRIX_AND_TAIL_LEN;

/// What an `igSymbol2d` payload must hold from its matrix tag on: the 2×2
/// placement matrix (32 bytes), the insertion pair (16), the uniform scale
/// (8), the flag word (4) and the two `u32`s that name the body (8).
///
/// `matrix_at + this` is the decoder's real size gate. Both corpus edges sit
/// on it exactly: the templates' 109-byte record has `matrix_at = 37`
/// (37 + 72 = 109), and one byte less cannot carry what the reader looks at.
const IGSYMBOL2D_MATRIX_AND_TAIL_LEN: usize = 72;

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
///
/// This is the canonical third byte, `0xA7`; see
/// [`IGSYMBOL2D_MATRIX_TAG_VARIANTS`] for the one other value a writer
/// is known to use there.
const IGSYMBOL2D_MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];

/// The matrix tag's third byte, `0x27`, carried by every `igSymbol2d`
/// record in the plant backup's `D04` (`test/U01/D04.pid` inside
/// `PlantData~2~711.zip`) — 6 of 6 records, 5 live plus one the native
/// reader retires (type flags carry [`PSM_TYPE_FLAG_NATIVE_SKIP`]). The
/// retired record shares the live record's oid, header, 2×2 matrix,
/// insertion pair and uniform scale byte-for-byte (bytes 0–95 of the
/// payload are identical) and its tail still resolves to the same
/// `(definition_sheet_ref, definition_site_ref)` pair — but the tail is 4
/// bytes shorter overall and the bytes in between differ, so it is an
/// earlier serialization of the same placement, not a byte-for-byte
/// duplicate. Every field at and after the tag otherwise reads exactly the
/// same shape as the `0xA7` records, which is what makes this a
/// serializer-session discriminator rather than a different record
/// layout: D04 is the only fixture in a corpus of 81 `.pid` files (12 411
/// `0xA7` records across 56 other fixtures, 0 mixed with `0x27`) to carry
/// this value, and it carries it on every one of its own placements, never
/// `0xA7`. The binaries that write this stream are not reachable for
/// disassembly from this workstation, so the exact producer semantics are
/// not proven; see
/// `docs/analysis/2026-11-07-igsymbol2d-matrix-tag-third-byte-is-a-session-flag.md`
/// for the full byte-statistics evidence and that gap.
const IGSYMBOL2D_MATRIX_TAG_ALT: [u8; 4] = [0x02, 0x00, 0x27, 0x50];

/// Both matrix-tag shapes [`decode_igsymbol_payload`] accepts, searched in
/// this order. The byte that varies is the tag's third: `0xA7` (`1010
/// 0111`) vs `0x27` (`0010 0111`) — bit 7 (`0x80`) of that byte, and
/// nothing else about the record's shape changes with it. No disassembly
/// of the writer ties that bit to a named flag; the corpus-wide split (one
/// value per file, never mixed within a file) is what is proven, not what
/// the bit is called. See [`IGSYMBOL2D_MATRIX_TAG_ALT`]'s doc for the
/// evidence and its gap.
const IGSYMBOL2D_MATRIX_TAG_VARIANTS: [[u8; 4]; 2] =
    [IGSYMBOL2D_MATRIX_TAG, IGSYMBOL2D_MATRIX_TAG_ALT];

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

/// Minimum `0x00FA` payload size: a dependency object that lists no member,
/// which is the part that does not grow with the count
/// (`DEPENDENCY_OBJECT_FIXED_LEN`).
///
/// This was 44 — one member — for as long as every record in the fixtures
/// had one. `A井场`'s `/Sheet6 +251219` has none and is 36 bytes of the
/// same grammar; at 44 it was the one record of the 53-sheet plant backup a
/// family refused.
pub const DEPENDENCY_OBJECT_MIN_PAYLOAD_LEN: usize = DEPENDENCY_OBJECT_FIXED_LEN;

/// The part of a `0x00FA` payload that does not grow with its member count:
/// the 16-byte head (oid, parent, six zero bytes, the count) and the shortest
/// trailing property block, 20 bytes.
const DEPENDENCY_OBJECT_FIXED_LEN: usize = 36;

/// What each member adds to a `0x00FA` payload: its `(u32 oid, u16 1)` entry
/// and its `u16` flag.
const DEPENDENCY_OBJECT_BYTES_PER_MEMBER: usize = 8;

/// Maximum `0x00FA` payload size accepted by the conservative decoder.
///
/// Current fixtures top out at 200 bytes. The cap leaves room for
/// variant growth while rejecting obvious wide-scan false positives.
const DEPENDENCY_OBJECT_MAX_PAYLOAD_LEN: usize = 512;

/// Sentinel beyond which a coordinate, a length or a parameter is treated
/// as garbage (uninitialized memory, a value read from where none is
/// stored): every family that reads doubles holds them to it.
const COORDINATE_DOMAIN_LIMIT: f64 = 1e9;

/// PSM type code of a RAD `JStyleOverride` record.
///
/// Phase 16 proved that the earlier Phase 14 `GArc2d` interpretation of
/// `0x0030` was a historical misidentification: the type-code table of
/// `radsrvitem.dll` resolves the code to CLSID
/// `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`, which the RAD registry in
/// `JUTIL.dll` names "JSL Override Style" and `style.dll` implements as
/// `JStyleOverride` (`docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`).
///
/// The class's records are in `StyleCluster`, where
/// [`crate::style_link`] reads them by their own layout. No `Sheet*` chain
/// holds one: the sheet family this crate had for the code until
/// 2026-11-11 matched the length word of the block a `DependencyObject`
/// ends with (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3.2, §12), and its matcher is kept on the test side
/// (`tests/support/retired_matchers.rs`).
pub const PSM_TYPE_CODE_JSTYLE_OVERRIDE: u16 = 0x0030;

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
/// 4-double Cartesian `(start, end)` representation: the one line record
/// a sheet holds (309 across the four fixtures).
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
/// ([`SheetChain::walk`]) before its payload is even looked at, and then
/// satisfy all of:
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
/// validation and are skipped. A stream whose chain stops before its end
/// yields the records before the stop and is never scanned for the rest.
pub fn decode_iglines(data: &[u8]) -> Vec<SheetIgLine2dDecoded> {
    SheetChain::walk(data)
        .starts
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
        let header = parse_live_psm_header(data, offset)?;
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
    if d.iter().any(|x| x.abs() > COORDINATE_DOMAIN_LIMIT) {
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
/// At every record the stream's chain starts, verify:
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
/// 8. All vertex coordinates finite and in domain `[-1e9, 1e9]`.
///
/// A record whose vertices all coincide is still decoded — real drawings
/// carry them as anchors a symbol or label sits on, and the native output
/// strokes nothing for a zero-length run. Emitting the point, not the
/// polyline, is the emitter's call (`IgLineString2dEmitter`).
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
        let header = parse_live_psm_header(data, offset)?;
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
        if x.abs() > COORDINATE_DOMAIN_LIMIT || y.abs() > COORDINATE_DOMAIN_LIMIT {
            return None;
        }
        vertices.push((x, y));
    }
    // An all-vertices-identical record is still decoded: the reference
    // corpus carries 36 of them, `vc=2` anchors a symbol or label sits on
    // (`examples/probe_real5_linestring_refusals.rs`). It decodes so the
    // record stops counting against coverage; the emitter is what decides
    // it has no stroke to draw.

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
/// At every record the stream's chain starts, verify:
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
        let header = parse_live_psm_header(data, offset)?;
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
    if x.abs() > COORDINATE_DOMAIN_LIMIT || y.abs() > COORDINATE_DOMAIN_LIMIT {
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
    /// The letter height the record states, in sheet units — `0.0` where the
    /// direction pair is the normalized unit vector every drawing sheet
    /// writes. The linked border templates scale their direction pair by the
    /// letter height instead, so it lands here: the size their static title
    /// text letters at when no style join answers.
    pub letter_height_m: f64,
    /// Width of the text box the record carries, in sheet units — `0.0` for
    /// a record whose tail states no box (kind 1), which is every record
    /// that is not a title-block field.
    ///
    /// The box sits inside the kind-2 tail at tail `+40`, past the 36 bytes
    /// the kind-1 tail stops at and the flag word at `+36`. The reference
    /// plots letter a boxed record from the box's left edge, wherever inside
    /// the box `anchor_code` puts the insertion point — measured on 58
    /// title-block records across the five drawing fixtures, whose centred
    /// anchor lands them half a box width left of the stored insertion, to
    /// within 0.05 mm. See
    /// `docs/analysis/2026-10-19-igtextbox-box-is-where-the-text-starts.md`.
    pub text_box_width_m: f64,
    /// Height of the same box, in sheet units — `0.0` when there is none.
    ///
    /// Unused by the plot writer: the reference letters a boxed record on the
    /// letter height's own anchor line, not on this box's, so the vertical
    /// half stays with [`SheetIgTextBoxDecoded::anchor_code`].
    pub text_box_height_m: f64,
    /// The record's own text-box anchor: the last byte of the 4-byte trailer
    /// that closes the 36-byte placement tail (payload `+35` of the tail).
    ///
    /// `(code >> 4) % 3` names the horizontal hot point the insertion sits
    /// at — `0` left, `1` centre, `2` right; the `+3`-shifted high nibbles
    /// (`0x38`/`0x48`/`0x58`) mark labels attached to a parent symbol and
    /// read through the same classes. The low nibble is a pair of flags, not
    /// an index: `0` is the baseline, `1` and `3` the ascender line, `2` the
    /// body's top, and any value with `0x4` or `0x8` set the middle — see
    /// `docs/analysis/2026-10-19-a-pair-of-flags-not-an-index.md` and
    /// `docs/analysis/2026-10-19-top-anchored-labels-sit-on-the-ascender-line.md`.
    /// It is the per-record justification and it overrides whatever the
    /// paragraph style declares: `L=0.8MPa ` and `1/2"` share a `Center`
    /// paragraph but letter left-anchored and centred respectively.
    /// Evidence: ~1,500 corpus records' insertion points against the
    /// reference PDFs' glyph boxes.
    pub anchor_code: u8,
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
/// 2. `bytes_to_follow >= 60` (the floor: shape 1's header, count and
///    tail around an empty text) and the payload fits in the stream;
/// 3. the shape discriminator at payload `+18` is one of the three
///    `igtextbox_body_shape` knows, which says where the text length sits
///    (`+22`, `+30` or `+26`) and how long the body runs; the length is
///    `<= 1024` (reject obvious noise) and the payload has room for the
///    body plus the 36-byte placement tail;
/// 4. the 4 doubles opening the tail — the insertion point, then the
///    direction pair — are finite + in domain;
/// 5. the direction pair is a direction: the unit vector, or a pair scaled
///    to a letter height (`igtextbox_letter_height`). This is also what
///    checks rule 3's stated length: a length that over-runs the label puts
///    this read on the wrong bytes, and they are not a direction.
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
        PSM_ENVELOPE_LEN + IGTEXTBOX_MIN_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgTextBoxDecoded> {
        let header = parse_live_psm_header(data, offset)?;
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
///
/// Shared with the symbol library (`symbol_library::decode_text`): a `.sym`
/// body stores `0x004D` records with this same payload layout, so both readers
/// ask this one table rather than keeping two rules — the library's separate
/// count-offset guess could not express shape 3 at all.
pub(crate) fn igtextbox_body_shape(
    payload: &[u8],
    text_sub_type: u16,
) -> Option<(u16, usize, usize)> {
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
            // `A` selector-1 then `B` selector-2 formatting runs trail the
            // text, each an 8-byte `(len, selector, style_id)` entry that
            // [`igtextbox_runs`] walks. They are part of the body, so the
            // placement tail starts past them.
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
///
/// Shared with the symbol library (`symbol_library::text_runs`): a `.sym`
/// stores the same payload, so the library asks this reader rather than
/// walking the entries a second time — the two-readers split that let the
/// library miss shape 3's runs in the first place.
pub(crate) fn igtextbox_runs(
    payload: &[u8],
    text_sub_type: u16,
    text_length: u16,
) -> Vec<IgTextBoxRun> {
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
    if trailing.iter().any(|x| x.abs() > COORDINATE_DOMAIN_LIMIT) {
        return None;
    }

    // The direction has to be a direction. This is what makes the stated text
    // length checkable: read the pair off the wrong offset -- which is what a
    // length that over-runs the label produces -- and it misses unit length by
    // orders of magnitude rather than by float noise.
    //
    // Two populations letter legitimately. Sheets on the drawing corpus
    // normalize the pair to the unit vector `(cos, sin)`. The linked border
    // templates (`*.igr` the drawing's `igSmartFrame2d` names) do not: they
    // store the direction scaled to the letter's height, so `hypot` lands on
    // the text height in metres -- 3.0 to 4.5 mm across `CPE-SQ-A0/A1`,
    // `0.003` up the left-margin column where a unit `sin` would sit. A
    // mis-stated length still misses both populations by orders of magnitude:
    // UTF-16 halves read as `f64` are denormal or astronomical, never
    // millimetres.
    let (cos, sin) = (trailing[2], trailing[3]);
    let letter_height_m = igtextbox_letter_height(cos, sin)?;
    let rotation_rad = sin.atan2(cos);
    let runs = igtextbox_runs(payload, text_sub_type, inline_text_length);
    // The 4-byte trailer closes the tail; its last byte is the anchor.
    let anchor_code = payload[tail_start + IGTEXTBOX_TAIL_LEN - 1];
    let (text_box_width_m, text_box_height_m) = igtextbox_box(payload, tail_start);

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
        letter_height_m,
        text_box_width_m,
        text_box_height_m,
        anchor_code,
        runs,
    })
}

/// The text box a kind-2 `igTextBox` tail carries, as `(width, height)` in
/// sheet units — `(0.0, 0.0)` for any other tail, or for a field that is not
/// a size at all.
///
/// The box is what the reference plots letter against: the record's insertion
/// point is the box's centre and the string prints from its left edge. Only a
/// usable size is reported — a non-finite, negative or absurd pair reads as
/// "no box", which leaves the record placed by its insertion point as before.
fn igtextbox_box(payload: &[u8], tail_start: usize) -> (f64, f64) {
    if payload.get(IGTEXTBOX_TAIL_KIND_AT..IGTEXTBOX_TAIL_KIND_AT + 2)
        != Some(&IGTEXTBOX_TAIL_KIND_BOX.to_le_bytes())
    {
        return (0.0, 0.0);
    }
    let end = tail_start.saturating_add(IGTEXTBOX_TAIL_KIND2_LEN);
    if end > payload.len() {
        return (0.0, 0.0);
    }
    let read = |at: usize| -> Option<f64> {
        let bytes = payload.get(at..at + 8)?;
        Some(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    };
    let usable = |value: f64| value.is_finite() && value > 0.0 && value <= COORDINATE_DOMAIN_LIMIT;
    let (Some(width), Some(height)) = (
        read(tail_start + IGTEXTBOX_BOX_WIDTH_AT),
        read(tail_start + IGTEXTBOX_BOX_HEIGHT_AT),
    ) else {
        return (0.0, 0.0);
    };
    if usable(width) && usable(height) {
        (width, height)
    } else {
        (0.0, 0.0)
    }
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
///   +48..55 f64   uniform scale — the parametric body's resize factor
///   ...     variable tail (symbol library ref + class ID + flags)
/// ```
///
/// The scale double reads as `1.0` on nearly every placement, but
/// parametric bodies carry the factor `SmartPlant` resized them by:
/// `I井场 注采阀组` alone files 16 placements at `0.37`–`5.63` (a metering
/// pump at `0.3705`, a wellhead Christmas tree at `1.1818`, skid-mounted
/// equipment at `5.6252`). The reference plots draw those bodies at the
/// factored size — the scale multiplies the 2×2 matrix about the
/// insertion point — so a reader that takes only the matrix draws a
/// parametric symbol at its template size, visibly off from the plot.
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
    /// Third byte of the matrix tag this record actually carries: `0xA7`
    /// (canonical, `IGSYMBOL2D_MATRIX_TAG`) on every corpus fixture but
    /// one, `0x27` (`IGSYMBOL2D_MATRIX_TAG_ALT`) on every `igSymbol2d`
    /// record in the plant backup's `D04`. The two differ only in this
    /// byte's `0x80` bit; every other field at and after the tag reads the
    /// same shape under both, which is why this is carried as data rather
    /// than used to pick a different layout.
    pub matrix_tag_third_byte: u8,
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
    /// Uniform factor the placement resizes its parametric body by — the
    /// f64 that follows the insertion pair. `1.0` on nearly every record;
    /// parametric symbols carry their own (0.37–5.63 on the corpus). The
    /// factor multiplies the placement matrix about the insertion point:
    /// `page = insertion + scale · matrix · body`. Non-finite or
    /// non-positive reads fall back to `1.0` rather than costing the
    /// placement.
    pub uniform_scale: f64,
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
/// 2. `bytes_to_follow ∈ [IGSYMBOL2D_MIN_PAYLOAD_LEN, 200]`;
/// 3. one of `IGSYMBOL2D_MATRIX_TAG_VARIANTS` occurs within the first
///    `IGSYMBOL2D_TAG_SEARCH_END` payload bytes;
/// 4. the payload holds everything from that tag on
///    (`matrix_at + IGSYMBOL2D_MATRIX_AND_TAIL_LEN`);
/// 5. the 6 doubles following it are finite + in domain.
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
        let header = parse_live_psm_header(data, offset)?;
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
    let search_window = payload.get(..search_end)?;
    let (tag_at, matrix_tag_third_byte) =
        IGSYMBOL2D_MATRIX_TAG_VARIANTS.iter().find_map(|tag| {
            search_window
                .windows(tag.len())
                .position(|w| w == tag)
                .map(|pos| (pos, tag[2]))
        })?;
    let matrix_at = tag_at + IGSYMBOL2D_MATRIX_TAG.len();
    // The gate the family's real shapes sit on: the matrix and the tail that
    // names the body have to fit, or the scale would read over the tail.
    if payload.len() < matrix_at.checked_add(IGSYMBOL2D_MATRIX_AND_TAIL_LEN)? {
        return None;
    }

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
    if doubles.iter().any(|x| x.abs() > COORDINATE_DOMAIN_LIMIT) {
        return None;
    }

    // The seventh double is the uniform scale the parametric body is
    // resized by. A non-positive or non-finite read would break the
    // placement matrix rather than size the body, so it falls back to
    // `1.0` -- drawn at template size, still better than dropped.
    let uniform_scale = payload
        .get(matrix_at + 48..matrix_at + 56)
        .and_then(|chunk| <[u8; 8]>::try_from(chunk).ok())
        .map(f64::from_le_bytes)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);

    // The tail after the six doubles: the `f64` uniform scale, a flag
    // word, then
    // `u32 has_membassy`, `u32 0`, an optional `(membassy oid, 0)` pair when
    // that flag is set, and finally the two words that name the body --
    // `(definition JSheet oid, LdcSite id)`. Reading them from the end is
    // what makes the optional pair irrelevant, and it is why the size gate
    // above is stated from `matrix_at` rather than as one payload length: the
    // 109- to 123-byte shapes all end on these two words.
    let tail_end = payload.len();
    let (definition_sheet_ref, definition_site_ref) = (
        u32_le(payload, tail_end - 8).unwrap_or_default(),
        u32_le(payload, tail_end - 4).unwrap_or_default(),
    );

    Some(SheetIgSymbol2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word,
        matrix_tag_third_byte,
        jsite_ref,
        style_ref,
        transform: [doubles[0], doubles[1], doubles[2], doubles[3]],
        insertion: (doubles[4], doubles[5]),
        uniform_scale,
        definition_sheet_ref,
        definition_site_ref,
    })
}

/// PSM type code for `JSheet Object` (`docext.dex`): a sheet, which in a
/// definition cache storage is one symbol body.
pub const PSM_TYPE_CODE_JSHEET: u16 = 0x0114;

/// The oid of every live `JSheet` record in a record-chain stream, in
/// on-disk order. No payload is read beyond the oid: the sheet's layers are
/// reached through its space-map edge to a `JSheetLayerManager`, not
/// through its own bytes.
///
/// A `JSheet` the native reader skips ([`PsmHeader::native_reader_skips`])
/// is not a live sheet, so it is left out: the header goes through
/// [`parse_live_psm_header`], the judgement every family decoder starts
/// from (OCS plan `2026-09-28-pid-import-next-steps.md` P-D12, applied here
/// by `2026-09-29-pid-import-next-round.md` P-D21). The walk stays
/// [`SheetChain::walk`], unfiltered: it has to see every record, a
/// skipped one included, to reach the next.
pub fn jsheet_oids(data: &[u8]) -> Vec<u32> {
    SheetChain::walk(data)
        .starts
        .into_iter()
        .filter_map(|at| {
            let header = parse_live_psm_header(data, at)?;
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
    /// The group's member count, at payload offsets 14..15: the tail lists
    /// that many `(u32 oid, u16 1)` members, then as many `u16` flags, then a
    /// property block. Named for the reading it had before the byte account
    /// closed (`docs/analysis/2026-09-24-the-last-five-refusals.md`).
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
/// 2. `bytes_to_follow` is even and in `[36, 512]`;
/// 3. payload exists, `oid != 0`, `parent_ref == 6`;
/// 4. payload bytes 8..13 are zero in the current fixture family;
/// 5. the payload has room for the members `group_kind_word` counts:
///    `36 + 8·k <= bytes_to_follow`, zero members included.
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
        let header = parse_live_psm_header(data, offset)?;
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
    // The member count: a group lists each member as `(u32 oid, u16 1)`, then
    // one `u16` flag per member, then a property block of at least 20 bytes,
    // so a record is at least `36 + 8·k` bytes long. The bound used to be
    // `k <= 16`, the largest count one batch of fixtures showed; DWG-0201's
    // 22-member group follows the same layout and was refused for it. See
    // `docs/analysis/2026-09-24-the-last-five-refusals.md`.
    //
    // The count may be zero: the record is then its head, the six bytes that
    // precede every closing block, and the block — 36 bytes, which is what
    // `36 + 8·k` gives. A count of zero used to be refused outright, on no
    // evidence but that no fixture had one.
    let group_kind_word = u16::from_le_bytes([payload[14], payload[15]]);
    if DEPENDENCY_OBJECT_FIXED_LEN
        + DEPENDENCY_OBJECT_BYTES_PER_MEMBER * usize::from(group_kind_word)
        > payload.len()
    {
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

/// Member tag of a point-run `igBoundary2d` member in the
/// linked-template (`0x0000` sub-type) layout: `tag + u32 count +
/// 2-byte tail + count × 16-byte (x, y)` pairs. The run re-lists a
/// closed outline, so consecutive pairs contribute segments.
pub const IGBOUNDARY2D_POINTRUN_TAG: u8 = 0x68;

/// Member tag of an arc `igBoundary2d` member in the linked-template
/// (`0x0000` sub-type) layout: `tag + 7×f64` carrying
/// `(start_angle, end_angle, centre_x, centre_y, radius, 0, 1)` plus a
/// trailing byte that is the arc's direction (0 = clockwise, 1 =
/// counter-clockwise), read the way `igArc2d`'s is. 58 bytes in total.
pub const IGBOUNDARY2D_ARC_TAG: u8 = 0x74;

/// Fixed trailer length of the linked-template (`0x0000` sub-type)
/// `igBoundary2d` variant: two identical 8-byte words followed by
/// `u32 1` and a `u8` flag — 21 bytes.
pub const IGBOUNDARY2D_TEMPLATE_TRAILER_LEN: usize = 21;

/// Upper bound accepted for one `0x68` point run's vertex count.
pub const IGBOUNDARY2D_MAX_POINTRUN_COUNT: u32 = 4096;

/// Radians swept per emitted chord when an `0x74` arc member is
/// flattened into boundary segments (≈14°; ~0.01mm sag at the logo's
/// 3mm radii).
const IGBOUNDARY2D_ARC_CHORD_STEP: f64 = 0.25;

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
    /// Sub-type word at payload offset 12: `0x0010` on drawing-side
    /// records, `0x0000` on linked-template (`.igr`) records.
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
    /// Decoded segment groups (`segment_count` entries), every loop's in
    /// turn when the record states more than one.
    pub segments: Vec<SheetIgBoundary2dSegment>,
    /// How many of [`Self::segments`] each loop holds, in order, on a
    /// record that states more than one loop (the `u32` at payload `+18`);
    /// empty on a record of one loop, which is every record of the corpora
    /// but two. The first loop is the outer edge of a fill and a loop
    /// inside it is a hole: see [`decode_igboundaries`].
    pub loop_segment_counts: Vec<u32>,
    /// Anchor point following the segment groups: where the fill was
    /// seeded. It lies inside the loop on every corpus record that carries
    /// member references, and between the loops on the two that state two.
    /// A linked template's records carry `(1e30, 1e30)` and no reference.
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
    ///
    /// A record of several loops is closed when each of them is.
    pub fn is_closed_loop(&self, tolerance: f64) -> bool {
        let close = |a: (f64, f64), b: (f64, f64)| {
            (a.0 - b.0).abs() <= tolerance && (a.1 - b.1).abs() <= tolerance
        };
        let mut loops = 0usize;
        for segments in self.loops() {
            loops += 1;
            let chained = segments.len() >= 2
                && segments.windows(2).all(|w| close(w[0].end, w[1].start))
                && close(segments[segments.len() - 1].end, segments[0].start);
            if !chained {
                return false;
            }
        }
        loops > 0
    }

    /// The segments of each loop, in order: one slice for a record of one
    /// loop, one per loop otherwise.
    ///
    /// A record whose [`Self::loop_segment_counts`] do not add up to its
    /// segments yields what they cover and then the rest as a last loop,
    /// so no segment is ever left out of a walk.
    pub fn loops(&self) -> impl Iterator<Item = &[SheetIgBoundary2dSegment]> {
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

/// Decode every PSM `igBoundary2d` record in a Sheet stream's bytes.
///
/// Validation rules (all must hold, otherwise the offset is skipped):
///
/// 1. `type_code == 0x0013`; type flags are zero;
/// 2. `sub_type_word == 0x0010` (payload offset 12), or `0x0000` — the
///    linked-template grammar first, then this one;
/// 3. the loop count — the `u32` at payload offset 18 — equals `1`; or,
///    under sub-type `0x0000`, it is two or more, each loop a member
///    count, two bytes and members of the linked-template vocabulary, and
///    the anchor, flag, reference count and references after the last loop
///    close the payload exactly (a fill with holes; rules 4–6 and 8 are
///    this rule's own for such a record);
/// 4. `segment_count` (payload offset 22) is in
///    `1..=IGBOUNDARY2D_MAX_SEGMENT_COUNT`;
/// 5. every member group starts with the `0x67` line tag (33 bytes) or
///    the `0x73` circle tag ([`IGBOUNDARY2D_CIRCLE_MEMBER_LEN`] bytes);
/// 6. the anchor, flag, member count and member references close the
///    payload exactly — `bytes_to_follow == 49 + 41 × segment_count`
///    when every member is a line;
/// 7. all member and anchor coordinates are finite and within the
///    `[-1e9, 1e9]` domain, and a circle's radius is positive;
/// 8. trailer `member_count == segment_count`;
/// 9. not all segment vertices are identical (non-degenerate).
///
/// Read at each record the stream's chain starts. Panic-free and
/// bounds-checked: adversarial bytes simply fail validation.
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
        // Smallest accepted record is a linked-template variant with a
        // single 33-byte line member: 6-byte envelope + 28-byte prefix
        // + member + 21-byte trailer = 88 bytes. The strict variant's
        // own minimum (envelope + 49 + 41 = 96) stays above that, so
        // the scan bound follows the smaller grammar.
        PSM_ENVELOPE_LEN + 28 + 33 + IGBOUNDARY2D_TEMPLATE_TRAILER_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgBoundary2dDecoded> {
        let header = parse_live_psm_header(data, offset)?;
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
    // A linked template writes sub-type `0x0000` with its own trailer. So
    // does a drawing's boundary over a circle -- `EF井场`'s solid dot oid
    // 52844 -- whose trailer is this grammar's anchor and member
    // references, so a refusal from the template grammar falls through.
    if sub_type_word == 0x0000 {
        if let Some(record) = decode_igboundary_template_payload(
            data,
            offset,
            header,
            payload_end,
            oid,
            parent_ref,
            sheet_layer_ref,
        ) {
            return Some(record);
        }
        // More than one loop: a fill with a hole, which neither the
        // template grammar nor the strict one below reads.
        if read_u32(18)? != 1 {
            return decode_igboundary_loops_payload(
                data,
                offset,
                header,
                payload_end,
                oid,
                parent_ref,
                sheet_layer_ref,
            );
        }
    } else if sub_type_word != 0x0010 {
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
    let sub_header_tail = [*payload.get(26)?, *payload.get(27)?];

    // Member groups from +28: a `0x67` line is `tag + 4×f64`; a `0x73`
    // circle is `tag + (centre_x, centre_y, radius, 0, 1) + u8`, the
    // layout of the template grammar's `0x74` arc without the two angles,
    // and is flattened to the chords a fill ring needs.
    let mut segments = Vec::with_capacity(n);
    let mut pos = 28usize;
    let mut all_same = true;
    let mut first_vertex: Option<(f64, f64)> = None;
    let in_domain = |v: f64| v.is_finite() && v.abs() <= COORDINATE_DOMAIN_LIMIT;
    for _ in 0..n {
        let mut chords: Vec<((f64, f64), (f64, f64))> = Vec::new();
        let member_len = match *payload.get(pos)? {
            IGBOUNDARY2D_SEGMENT_TAG => {
                let sx = read_f64(pos + 1)?;
                let sy = read_f64(pos + 9)?;
                let ex = read_f64(pos + 17)?;
                let ey = read_f64(pos + 25)?;
                if ![sx, sy, ex, ey].into_iter().all(in_domain) {
                    return None;
                }
                chords.push(((sx, sy), (ex, ey)));
                33
            }
            IGSYMBOUNDARY_CIRCLE_TAG => {
                let cx = read_f64(pos + 1)?;
                let cy = read_f64(pos + 9)?;
                let radius = read_f64(pos + 17)?;
                if ![cx, cy, radius].into_iter().all(in_domain) || radius <= 0.0 {
                    return None;
                }
                // A whole turn, in the direction the closing byte names --
                // `0` clockwise, as for an arc member.
                let sweep = if *payload.get(pos + IGBOUNDARY2D_CIRCLE_MEMBER_LEN - 1)? == 0 {
                    -std::f64::consts::TAU
                } else {
                    std::f64::consts::TAU
                };
                let steps = (std::f64::consts::TAU / IGBOUNDARY2D_ARC_CHORD_STEP).ceil() as usize;
                let at = |step: usize| {
                    let angle = sweep * step as f64 / steps as f64;
                    (cx + radius * angle.cos(), cy + radius * angle.sin())
                };
                chords.extend((0..steps).map(|step| (at(step), at(step + 1))));
                IGBOUNDARY2D_CIRCLE_MEMBER_LEN
            }
            _ => return None,
        };
        for (start, end) in chords {
            let reference = *first_vertex.get_or_insert(start);
            if start != reference || end != reference {
                all_same = false;
            }
            segments.push(SheetIgBoundary2dSegment {
                tag_offset: pos,
                start,
                end,
            });
        }
        pos += member_len;
    }
    if all_same {
        return None;
    }

    // Anchor + flag + member count.
    let anchor_x = read_f64(pos)?;
    let anchor_y = read_f64(pos + 8)?;
    for v in [anchor_x, anchor_y] {
        if !v.is_finite() || v.abs() > COORDINATE_DOMAIN_LIMIT {
            return None;
        }
    }
    let trailer_flag = *payload.get(pos + 16)?;
    let member_count = read_u32(pos + 17)?;
    if member_count != segment_count {
        return None;
    }

    // Trailer member references, which close the payload exactly.
    let mut member_refs = Vec::with_capacity(n);
    let member_base = pos + 21;
    if member_base.checked_add(n.checked_mul(8)?)? != btf {
        return None;
    }
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
        loop_segment_counts: Vec::new(),
        anchor: (anchor_x, anchor_y),
        trailer_flag,
        member_refs,
    })
}

/// The signed sweep of one `0x74` boundary arc member, in radians.
///
/// The two angles alone name two arcs; the member's trailing byte says
/// which one the ring runs along — `0` clockwise from the start angle,
/// anything else counter-clockwise — the same byte, read the same way, as
/// `igArc2d`'s
/// (`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`).
/// Taking `end − start` as it stands is only right for an arc that does not
/// cross the angle origin: seven members of the CCHB A2 title block's logo
/// do, and drew as 219–334° near-circles where the reference plot prints
/// their 26° and 92° arcs. Angles stored a whole turn apart stay a whole
/// turn.
fn igboundary_arc_sweep(start: f64, end: f64, direction: u8) -> f64 {
    use std::f64::consts::TAU;
    let whole_turn = (end - start).abs() >= TAU - 1e-9;
    match (direction == 0, whole_turn) {
        (true, true) => -TAU,
        (false, true) => TAU,
        (true, false) => -(start - end).rem_euclid(TAU),
        (false, false) => (end - start).rem_euclid(TAU),
    }
}

/// Family-specific payload validation for the linked-template
/// `igBoundary2d` variant (sub-type word `0x0000` rather than the
/// drawing-side `0x0010`). Linked `.igr` border templates letter their
/// logo and rounded corners with this member vocabulary:
///
/// * `0x67` line — `tag + 4×f64` (start, end), 33 bytes;
/// * `0x68` point run — `tag + u32 count + 2-byte tail +
///   count × 16-byte (x, y)`, closed onto its first point;
/// * `0x74` arc — `tag + 7×f64`
///   `(start_angle, end_angle, centre_x, centre_y, radius, 0, 1)` plus a
///   direction byte ([`igboundary_arc_sweep`]); flattened to chords of at
///   most [`IGBOUNDARY2D_ARC_CHORD_STEP`] radians.
///
/// Members chain end-to-start and the record ends with a fixed
/// 21-byte trailer (two identical 8-byte words, `u32 1`, `u8` flag)
/// instead of the strict grammar's anchor + member references. The
/// segment list flattens every member to chords so the shared emitter
/// sees the same closed ring either way; `member_refs` is empty and
/// `anchor` borrows the first vertex for lack of a trailer anchor.
fn decode_igboundary_template_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
    oid: u32,
    parent_ref: u32,
    sheet_layer_ref: u32,
) -> Option<SheetIgBoundary2dDecoded> {
    let payload = data.get(header.body_start..payload_end)?;

    let read_u32 = |pos: usize| -> Option<u32> {
        payload
            .get(pos..pos + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };

    let index = read_u32(14)?;
    if read_u32(18)? != 1 {
        return None;
    }
    let member_count = read_u32(22)?;
    if !(1..=IGBOUNDARY2D_MAX_SEGMENT_COUNT).contains(&member_count) {
        return None;
    }
    let sub_header_tail = [*payload.get(26)?, *payload.get(27)?];
    let members_end = payload
        .len()
        .checked_sub(IGBOUNDARY2D_TEMPLATE_TRAILER_LEN)?;
    if members_end < 28 {
        return None;
    }

    let mut segments = Vec::new();
    let pos = igboundary_template_members(payload, 28, member_count, members_end, &mut segments)?;
    if pos != members_end || segments.is_empty() || igboundary_segments_coincide(&segments) {
        return None;
    }
    // The 21-byte trailer carries two identical 8-byte words followed
    // by `u32 1` and a flag byte on every corpus record; the marker is
    // the only part worth holding a rule to.
    if read_u32(members_end + 16)? != 1 {
        return None;
    }
    let anchor = segments.first().map_or((0.0, 0.0), |s| s.start);
    let trailer_flag = *payload.last()?;

    Some(SheetIgBoundary2dDecoded {
        byte_range: offset..payload_end,
        type_code: header.type_code,
        type_flags: header.type_flags,
        bytes_to_follow: header.bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word: 0x0000,
        index,
        segment_count: member_count,
        sub_header_tail,
        segments,
        loop_segment_counts: Vec::new(),
        anchor,
        trailer_flag,
        member_refs: Vec::new(),
    })
}

/// Read `member_count` members of the linked-template vocabulary — `0x67`
/// line, `0x68` point run, `0x74` arc — from `payload[pos..]`, none
/// starting at or past `members_end`, flattening each to chords appended to
/// `segments`. Returns the offset after the last member.
///
/// One loop of a boundary is one such run of members; see
/// [`decode_igboundary_template_payload`] for the layouts.
fn igboundary_template_members(
    payload: &[u8],
    mut pos: usize,
    member_count: u32,
    members_end: usize,
    segments: &mut Vec<SheetIgBoundary2dSegment>,
) -> Option<usize> {
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
    let check = |v: f64| -> Option<f64> {
        (v.is_finite() && v.abs() <= COORDINATE_DOMAIN_LIMIT).then_some(v)
    };
    let mut push_segment = |tag_offset: usize, start: (f64, f64), end: (f64, f64)| {
        segments.push(SheetIgBoundary2dSegment {
            tag_offset,
            start,
            end,
        });
    };

    for _ in 0..member_count {
        if pos >= members_end {
            return None;
        }
        match *payload.get(pos)? {
            IGBOUNDARY2D_SEGMENT_TAG => {
                let sx = check(read_f64(pos + 1)?)?;
                let sy = check(read_f64(pos + 9)?)?;
                let ex = check(read_f64(pos + 17)?)?;
                let ey = check(read_f64(pos + 25)?)?;
                push_segment(pos, (sx, sy), (ex, ey));
                pos += 33;
            }
            IGBOUNDARY2D_POINTRUN_TAG => {
                let count = read_u32(pos + 1)?;
                if !(2..=IGBOUNDARY2D_MAX_POINTRUN_COUNT).contains(&count) {
                    return None;
                }
                let count = count as usize;
                let run_end = pos.checked_add(7)?.checked_add(count.checked_mul(16)?)?;
                if run_end > members_end {
                    return None;
                }
                let mut prev = (check(read_f64(pos + 7)?)?, check(read_f64(pos + 15)?)?);
                for i in 1..count {
                    let next = (
                        check(read_f64(pos + 7 + i * 16)?)?,
                        check(read_f64(pos + 15 + i * 16)?)?,
                    );
                    push_segment(pos, prev, next);
                    prev = next;
                }
                pos = run_end;
            }
            IGBOUNDARY2D_ARC_TAG => {
                let member_end = pos.checked_add(58)?;
                if member_end > members_end {
                    return None;
                }
                let start_ang = check(read_f64(pos + 1)?)?;
                let end_ang = check(read_f64(pos + 9)?)?;
                let cx = check(read_f64(pos + 17)?)?;
                let cy = check(read_f64(pos + 25)?)?;
                let radius = check(read_f64(pos + 33)?)?;
                if !radius.is_finite()
                    || radius <= 0.0
                    || !start_ang.is_finite()
                    || !end_ang.is_finite()
                {
                    return None;
                }
                let sweep = igboundary_arc_sweep(start_ang, end_ang, *payload.get(pos + 57)?);
                let chords = ((sweep.abs() / IGBOUNDARY2D_ARC_CHORD_STEP).ceil() as usize).max(1);
                let mut prev = (cx + radius * start_ang.cos(), cy + radius * start_ang.sin());
                for i in 1..=chords {
                    let angle = start_ang + sweep * (i as f64 / chords as f64);
                    let next = (cx + radius * angle.cos(), cy + radius * angle.sin());
                    push_segment(pos, prev, next);
                    prev = next;
                }
                pos = member_end;
            }
            _ => return None,
        }
    }
    Some(pos)
}

/// Whether every vertex of `segments` is the first one: a boundary that
/// encloses nothing.
fn igboundary_segments_coincide(segments: &[SheetIgBoundary2dSegment]) -> bool {
    segments.first().is_none_or(|first| {
        segments
            .iter()
            .all(|segment| segment.start == first.start && segment.end == first.start)
    })
}

/// Bytes one loop of a multi-loop boundary cannot be shorter than: its
/// `u32` member count, the two bytes after it, and one `0x67` line.
const IGBOUNDARY2D_MIN_LOOP_LEN: usize = 6 + 33;

/// Family-specific payload validation for an `igBoundary2d` that states
/// more than one loop: a fill with a hole in it.
///
/// The `u32` at payload `+18`, which every other record of the family
/// carries as `1`, is the loop count. Each loop is what a one-loop record
/// holds from `+22`: a `u32` member count, the two bytes `[2, 1]`, and that
/// many members of the linked-template vocabulary
/// ([`igboundary_template_members`]). After the last loop comes the
/// drawing-side trailer — the anchor, a flag byte, a `u32` reference count
/// and that many 8-byte member references — and it closes the payload
/// exactly.
///
/// Read this way the trailer closes the payload of all 2 158 boundaries on
/// the sheet chains of the 86 documents the count was taken on — the plant
/// backup, the acceptance sheets, the share, the fixture drawings and the
/// two template directories — whichever grammar decodes them today (a
/// linked template's 21-byte trailer is this one with no reference, its
/// anchor the pair `1e30`). Two of them state two loops, both in
/// `XA-A1.igr`: an outer loop and a loop wholly inside it wound the other
/// way, with the anchor between the two
/// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §10.6).
///
/// That is not every boundary in reach. The templates inside the committed
/// `RefData~4~682.zip` keep 41 more on their sheet chains, and 20 of them
/// — five in the logo of each of `A0-W.igr` to `A3-W.igr` — this grammar
/// refuses: where a loop's first member tag `0x67` stands they carry
/// `0x02` or `0x03`. No page wears those templates, so the refusal reaches
/// no export; it is an open gap all the same (same document, §17.6).
///
/// The reference count is not held to the member count. One of the two
/// names 217 lines for its 216 members; the odd one is a line 32 µm long
/// that its outer loop steps over.
fn decode_igboundary_loops_payload(
    data: &[u8],
    offset: usize,
    header: &PsmHeader,
    payload_end: usize,
    oid: u32,
    parent_ref: u32,
    sheet_layer_ref: u32,
) -> Option<SheetIgBoundary2dDecoded> {
    let payload = data.get(header.body_start..payload_end)?;
    let index = u32_le(payload, 14)?;
    let loop_count = usize::try_from(u32_le(payload, 18)?).ok()?;
    // The count is bounded by the payload that has to hold the loops, not
    // by a constant: a letter's outline runs to 162 members here.
    if loop_count < 2 || loop_count > payload.len() / IGBOUNDARY2D_MIN_LOOP_LEN {
        return None;
    }
    let members_end = payload
        .len()
        .checked_sub(IGBOUNDARY2D_TEMPLATE_TRAILER_LEN)?;

    let mut segments = Vec::new();
    let mut loop_segment_counts = Vec::with_capacity(loop_count);
    let mut member_total = 0u32;
    let mut sub_header_tail = [0u8; 2];
    let mut pos = 22usize;
    for loop_index in 0..loop_count {
        let member_count = u32_le(payload, pos)?;
        if member_count == 0 {
            return None;
        }
        if loop_index == 0 {
            sub_header_tail = [*payload.get(pos + 4)?, *payload.get(pos + 5)?];
        }
        let before = segments.len();
        pos = igboundary_template_members(
            payload,
            pos.checked_add(6)?,
            member_count,
            members_end,
            &mut segments,
        )?;
        loop_segment_counts.push(u32::try_from(segments.len() - before).ok()?);
        member_total = member_total.checked_add(member_count)?;
    }
    if igboundary_segments_coincide(&segments) {
        return None;
    }

    // Anchor, flag, reference count, references: to the payload's last byte.
    let anchor = (f64_le(payload, pos)?, f64_le(payload, pos.checked_add(8)?)?);
    for value in [anchor.0, anchor.1] {
        if !value.is_finite() || value.abs() > COORDINATE_DOMAIN_LIMIT {
            return None;
        }
    }
    let trailer_flag = *payload.get(pos.checked_add(16)?)?;
    let reference_count = usize::try_from(u32_le(payload, pos.checked_add(17)?)?).ok()?;
    let references_at = pos.checked_add(IGBOUNDARY2D_TEMPLATE_TRAILER_LEN)?;
    if references_at.checked_add(reference_count.checked_mul(8)?)? != payload.len() {
        return None;
    }
    let member_refs = payload
        .get(references_at..)?
        .as_chunks::<8>()
        .0
        .iter()
        .map(|entry| SheetIgBoundary2dMemberRef {
            member_oid: u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]),
            class_word: u16::from_le_bytes([entry[4], entry[5]]),
            sub_word: u16::from_le_bytes([entry[6], entry[7]]),
        })
        .collect();

    Some(SheetIgBoundary2dDecoded {
        byte_range: offset..payload_end,
        type_code: header.type_code,
        type_flags: header.type_flags,
        bytes_to_follow: header.bytes_to_follow,
        oid,
        parent_ref,
        sheet_layer_ref,
        sub_type_word: 0x0000,
        index,
        segment_count: member_total,
        sub_header_tail,
        segments,
        loop_segment_counts,
        anchor,
        trailer_flag,
        member_refs,
    })
}

// ---------------------------------------------------------------------------
// PSM `igBoundary2d` (type `0x0013`), symbol-cache variant — loose member
// read for `.sym` libraries and the drawing's `JSite` definition cache.
//
// The strict decoder above answers the drawing-side question (`0x0010`
// sub-type, `0x67` line members re-listing `igLine2d` strokes). The same
// family inside a `.sym` or a `JSite` storage carries a different member
// vocabulary: `Wellhead Christmas Tree 02.sym` files 22 records whose
// sub-type word is `0x0000` and whose segment groups open with `0x73`
// followed by `(centre_x, centre_y, radius, unused)` — the flange-dot
// circles `SmartPlant`'s plot fills solid. Each carries a fill-family
// style index, which is what marks them as fills rather than outlines.
//
// This read shares the 18-byte prefix, the `u32 == 1` marker at `+18`
// and the segment count at `+22` with the strict one, maps each segment
// group by its tag byte, and leaves the trailer unread — the strict
// grammar's `0x67`-only rule rejects every corpus member here, and its
// exact `49 + 41n` length arithmetic does not hold for this variant.

/// Segment tag of a circular `igBoundary2d` member (`.sym`/cache
/// layout): the group's four words are `centre_x, centre_y, radius,
/// unused`.
pub const IGSYMBOUNDARY_CIRCLE_TAG: u8 = 0x73;

/// Bytes one `0x73` circle member takes in a drawing's `igBoundary2d`:
/// the tag, `(centre_x, centre_y, radius, 0, 1)` and a closing direction
/// byte — the template grammar's 58-byte `0x74` arc without its two
/// angles. `EF井场`'s oid 52844 is one: a single circle member filling
/// the `igCircle2d` its member reference names.
pub const IGBOUNDARY2D_CIRCLE_MEMBER_LEN: usize = 1 + 5 * 8 + 1;

/// One member shape of an `igBoundary2d` record, keyed by the segment
/// group's tag byte rather than resolved through the member-reference
/// trailer.
#[derive(Debug, Clone, PartialEq)]
pub enum IgBoundaryMember {
    /// Segment tag [`IGBOUNDARY2D_SEGMENT_TAG`] (`0x67`): the group's
    /// words are `(start_x, start_y, end_x, end_y)` — the layout
    /// drawing-side records carry.
    Line {
        /// Member start.
        start: (f64, f64),
        /// Member end.
        end: (f64, f64),
    },
    /// Segment tag [`IGSYMBOUNDARY_CIRCLE_TAG`] (`0x73`): the group's
    /// words are `(centre_x, centre_y, radius, unused)` — the layout
    /// `.sym` libraries and `JSite` caches carry.
    Circle {
        /// Member centre.
        center: (f64, f64),
        /// Member radius.
        radius: f64,
    },
}

/// One `igBoundary2d` record read the loose symbol-cache way: prefix,
/// sub-header marker and segment groups are shared with
/// [`SheetIgBoundary2dDecoded`]; the sub-type word, tag vocabulary and
/// trailer are not held to its rules.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedIgSymBoundaryRecord {
    /// Object identifier from payload offset 0.
    pub oid: u32,
    /// Oid of the `JSheetLayer` this boundary sits on (payload offset
    /// 8) — the same slot every other graphic family carries it in.
    pub sheet_layer_ref: u32,
    /// Style index at payload offset 14 — names a fill-family style
    /// where the record fills, resolvable through
    /// [`crate::style_link::DocumentStyleTable::resolve_fill`].
    pub index: u32,
    /// The members the region is filled over, in record order.
    pub members: Vec<IgBoundaryMember>,
}

/// Decode the payload of one `igBoundary2d` record the symbol-cache
/// way. `None` when the prefix, marker, count or geometry reads fail.
/// Groups whose tag names no known member shape are dropped; a record
/// left with no member is not a fill anyone can draw.
pub fn decode_igsym_boundary_payload(payload: &[u8]) -> Option<DecodedIgSymBoundaryRecord> {
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
    let sheet_layer_ref = read_u32(8)?;
    let index = read_u32(14)?;
    // Two member layouts share this record type. The single-word-at-a-time
    // layout every corpus record used until the plug valves surfaced: count
    // at +22, members contiguous at 33 bytes each. The separator-joined
    // layout those carry instead: count at +18 (+22 stays 1), and each
    // 35-byte member block (`04 01` prefix + tag + four f64 words) joined
    // to the next by 13 bytes of an `f64` and `01 01 00 00 00` — a 48-byte
    // stride, re-checked against the `04 01` prefix so the count cannot
    // invent members. A `+18 != 1` guard drops the plug valves' outer
    // discs, the fills their PDFs draw.
    let (count, stride, gated) = if read_u32(18)? > 1 {
        (read_u32(18)? as usize, 48usize, true)
    } else {
        (read_u32(22)? as usize, 33usize, false)
    };
    if !(1..=IGBOUNDARY2D_MAX_SEGMENT_COUNT as usize).contains(&count) {
        return None;
    }
    if payload.len()
        < 26usize
            .checked_add(stride.checked_mul(count.checked_sub(1)?)?)?
            .checked_add(35)?
    {
        return None;
    }
    let mut members = Vec::with_capacity(count);
    for i in 0..count {
        let at = 28 + i * stride;
        // Separator-joined member blocks open with their own `04 01`
        // prefix two bytes ahead of the tag; a record that loses it is not
        // this layout.
        if gated && payload.get(at - 2..at) != Some(&[0x04, 0x01][..]) {
            return None;
        }
        let v = [
            read_f64(at + 1)?,
            read_f64(at + 9)?,
            read_f64(at + 17)?,
            read_f64(at + 25)?,
        ];
        if v.iter()
            .any(|v| !v.is_finite() || v.abs() > COORDINATE_DOMAIN_LIMIT)
        {
            return None;
        }
        let member = match payload[at] {
            IGBOUNDARY2D_SEGMENT_TAG => IgBoundaryMember::Line {
                start: (v[0], v[1]),
                end: (v[2], v[3]),
            },
            IGSYMBOUNDARY_CIRCLE_TAG => {
                if v[2] <= 0.0 {
                    return None;
                }
                IgBoundaryMember::Circle {
                    center: (v[0], v[1]),
                    radius: v[2],
                }
            }
            _ => continue,
        };
        members.push(member);
    }
    (!members.is_empty()).then_some(DecodedIgSymBoundaryRecord {
        oid,
        sheet_layer_ref,
        index,
        members,
    })
}

/// Try to decode a single `igBoundary2d` record starting at `offset`,
/// the symbol-cache way. `None` when the envelope or
/// [`decode_igsym_boundary_payload`] fails.
pub fn decode_igsym_boundary_at(data: &[u8], offset: usize) -> Option<DecodedIgSymBoundaryRecord> {
    let header = parse_live_psm_header(data, offset)?;
    if header.type_code != PSM_TYPE_CODE_IGBOUNDARY2D {
        return None;
    }
    let end = header
        .body_start
        .checked_add(header.bytes_to_follow as usize)?;
    let payload = data.get(header.body_start..end)?;
    decode_igsym_boundary_payload(payload)
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
/// Payload offset of the framed range's origin, two `f64` in metres. The
/// record stores the same pair again at `+116` / `+124`.
const IGSMARTFRAME_RANGE_ORIGIN_AT: usize = 36;
/// Payload offset of the framed extent's width, in metres.
const IGSMARTFRAME_WIDTH_AT: usize = 76;
/// Payload offset of the framed extent's height, in metres.
const IGSMARTFRAME_HEIGHT_AT: usize = 84;
/// Payload offset of the extent's aspect ratio (`height / width`).
const IGSMARTFRAME_ASPECT_AT: usize = 148;
/// Shortest payload that reaches every field above.
const IGSMARTFRAME_MIN_PAYLOAD: usize = IGSMARTFRAME_ASPECT_AT + 8;
/// How far the ratio at [`IGSMARTFRAME_ASPECT_AT`] may lie from the height
/// at [`IGSMARTFRAME_HEIGHT_AT`] over the width at [`IGSMARTFRAME_WIDTH_AT`],
/// as a fraction of that quotient.
///
/// The record states its extent twice, and on all 178 frames of the
/// corpora's chains — 885 documents, the superseded copies included — the
/// two statements agree to 5e-16
/// (`examples/probe_what_passes_for_a_page_frame_off_the_chain.rs`). One part
/// in a million leaves room for a writer that rounds and none for three
/// doubles that merely happen to lie at these offsets. The bound is
/// relative because such doubles are as often 1e-212 as 1e284: an absolute
/// one in a million would pass a height of 4e-212 over a width of 0.6
/// against a "ratio" of 7e-138, which one of the ten is.
const IGSMARTFRAME_ASPECT_TOLERANCE: f64 = 1e-6;
/// Payload offset of the framed content's placement: seven `f64` — a 2×2
/// linear part, a translation in metres, and a uniform scale — laid out as
/// RAD's 2D transform (`imagdex.dex` / `radsrvitem.dll` `sub_5644FD00`,
/// `sub_5649DE50`, composed by `sub_5644E580`).
const IGSMARTFRAME_CONTENT_TRANSFORM_AT: usize = 164;

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

/// Whether a frame's extent and the ratio it states hold each other: the
/// ratio lies within [`IGSMARTFRAME_ASPECT_TOLERANCE`] of the height over
/// the width.
///
/// That can be asked only of an extent that has a ratio. The width and the
/// height are lengths in metres: above zero, and normal numbers — a
/// denormal is what a count or a flag word reads as when the eight bytes
/// are not a double, and is below 2.3e-308 m besides. Their quotient is a
/// normal number too: one that overflows, or is rounded away to nothing,
/// is no number a stated ratio can be near. The comparison is written so
/// that whatever cannot be compared fails it.
///
/// Until 2026-11-12 a width of zero skipped the comparison and a quotient
/// of infinity passed it (nothing is further from infinity than a millionth
/// of infinity), and so did a height of zero under a stated ratio of zero.
/// Six of the 1 371 448 records that walks read off the corpora's chains,
/// with one length changed by every amount from −64 to +64, were frames
/// that way: four of no width and no height, two a count wide under a
/// height of 3.7e62 m. The smallest frame the chains hold is a micrometre
/// square; none states a side of zero
/// (`examples/probe_what_passes_for_a_page_frame_off_the_chain.rs`).
///
/// `None` for an extent a frame states; otherwise the part of the rule it
/// fails, for a caller that has to say why a record is refused.
fn smart_frame_extent_refusal(
    width_m: f64,
    height_m: f64,
    aspect_ratio: f64,
) -> Option<SmartFrameRefusal> {
    let is_length = |metres: f64| metres.is_normal() && metres > 0.0;
    if !is_length(width_m) || !is_length(height_m) {
        return Some(SmartFrameRefusal::NotLengths { width_m, height_m });
    }
    let stated_by_extent = height_m / width_m;
    if !stated_by_extent.is_normal() {
        return Some(SmartFrameRefusal::NoQuotient { width_m, height_m });
    }
    let agrees = aspect_ratio.is_finite()
        && (aspect_ratio - stated_by_extent).abs()
            <= IGSMARTFRAME_ASPECT_TOLERANCE * stated_by_extent;
    (!agrees).then_some(SmartFrameRefusal::RatioNotTheExtents {
        width_m,
        height_m,
        aspect_ratio,
    })
}

/// Why a live `igSmartFrame2d` record is not a frame the decoder takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmartFrameRefusal {
    /// The payload is shorter than the 156 bytes that reach the ratio a
    /// frame states.
    ShortPayload {
        /// The payload's length in bytes.
        payload_len: usize,
    },
    /// The width or the height the record states is not a length: not
    /// finite, zero or below, or a denormal.
    NotLengths {
        /// The width at payload `+76`.
        width_m: f64,
        /// The height at payload `+84`.
        height_m: f64,
    },
    /// The height over the width is no number a ratio can be held to: the
    /// quotient overflows, or is rounded away to nothing.
    NoQuotient {
        /// The width at payload `+76`.
        width_m: f64,
        /// The height at payload `+84`.
        height_m: f64,
    },
    /// The ratio the record states is not its height over its width, to a
    /// millionth of that quotient.
    RatioNotTheExtents {
        /// The width at payload `+76`.
        width_m: f64,
        /// The height at payload `+84`.
        height_m: f64,
        /// The ratio at payload `+148`.
        aspect_ratio: f64,
    },
}

impl SmartFrameRefusal {
    /// The reason as the clause of a sentence about the record: `it states
    /// a width of 0 m and a height of 0.42 m, and a frame's sides are
    /// lengths above zero`.
    #[must_use]
    pub fn describe(&self) -> String {
        match *self {
            Self::ShortPayload { payload_len } => format!(
                "its payload is {payload_len} byte(s), fewer than the \
                 {IGSMARTFRAME_MIN_PAYLOAD} that reach the ratio a frame states"
            ),
            Self::NotLengths { width_m, height_m } => format!(
                "it states a width of {} m and a height of {} m, and a frame's sides are \
                 lengths above zero",
                written_short(width_m),
                written_short(height_m)
            ),
            Self::NoQuotient { width_m, height_m } => format!(
                "its height over its width, {} m over {} m, is no number a ratio can be held \
                 to",
                written_short(height_m),
                written_short(width_m)
            ),
            Self::RatioNotTheExtents {
                width_m,
                height_m,
                aspect_ratio,
            } => format!(
                "it states a ratio of {aspect_ratio:.9} where its height over its width, \
                 {height_m:.6} m over {width_m:.6} m, is {:.9}",
                height_m / width_m
            ),
        }
    }
}

/// A double as a sentence writes it: the shortest decimal that reads back
/// the same, with an exponent where the plain digits would run to dozens —
/// what lies in a refused frame is as often 3.8e-313 as 0.42.
fn written_short(value: f64) -> String {
    if value == 0.0 || (1e-4..1e6).contains(&value.abs()) {
        format!("{value}")
    } else {
        format!("{value:e}")
    }
}

/// A live `igSmartFrame2d` record that [`decode_smartframes`] does not take
/// for a frame, and why.
///
/// The rule that refuses it is what tells a frame from the bytes a walk
/// reads off a broken chain, and a real frame can fail it: a ratio written
/// wrong, a side of zero. A drawing whose own border frame is one of these
/// is laid out as a sheet with no frame, which a caller that lays out a
/// page says ([`refused_smartframes`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSmartFrameRefused {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// Object identifier, when the payload reaches one.
    pub oid: Option<u32>,
    /// The state the record's two flag words put it in, when the payload
    /// reaches them: an embedded or a linked frame is one whose extent
    /// would have been a page.
    pub state: Option<SheetSmartFrame2dState>,
    /// Which part of the frame rule the record fails.
    pub why: SmartFrameRefusal,
}

/// Every live `igSmartFrame2d` record of a chain stream that the frame
/// decoder refuses, with why: the records [`decode_smartframes`] leaves
/// out. Empty on every chain stream of the corpora, whose 91 live frames
/// all decode (`tests/sheet_chain_faults.rs`).
#[must_use]
pub fn refused_smartframes(data: &[u8]) -> Vec<SheetSmartFrameRefused> {
    SheetChain::walk(data)
        .starts
        .iter()
        .filter_map(|&at| smartframe_refusal_at(data, at))
        .collect()
}

/// Why the live `igSmartFrame2d` record at `offset` is not a frame.
///
/// `None` where the record is one — [`decode_smartframe_at`] takes it — and
/// where no live record of that type starts whose payload the stream
/// holds. Panic-free on arbitrary input.
#[must_use]
pub fn smartframe_refusal_at(data: &[u8], offset: usize) -> Option<SheetSmartFrameRefused> {
    read_smartframe_at(data, offset)?.err()
}

/// The state the native reader's rules put a frame with these two flag
/// words in.
fn smart_frame_state(content_flags: u32, link_flags: u32) -> SheetSmartFrame2dState {
    if content_flags & IGSMARTFRAME_HAS_CONTENT == 0 {
        SheetSmartFrame2dState::Empty
    } else if link_flags & IGSMARTFRAME_LINKED == 0 && link_flags & IGSMARTFRAME_LOCALLY_LINKED == 0
    {
        SheetSmartFrame2dState::Embedded
    } else if link_flags & IGSMARTFRAME_LOCALLY_LINKED != 0 {
        SheetSmartFrame2dState::LocallyLinked
    } else {
        SheetSmartFrame2dState::Linked
    }
}

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
    /// Origin of the framed range in metres (payload `+36` / `+44`, stored
    /// again at `+116` / `+124`); `None` when either value is not finite.
    /// Zero on most frames; where it is not, the content sits at
    /// `translation − origin` from the frame's corner.
    pub range_origin_m: Option<[f64; 2]>,
    /// Framed extent width in metres (payload `+76`).
    pub extent_width_m: f64,
    /// Framed extent height in metres (payload `+84`).
    pub extent_height_m: f64,
    /// Extent aspect ratio (payload `+148`): the height over the width,
    /// which the decoder holds it to; `1/sqrt(2)` on an ISO A sheet, give
    /// or take the frame's margins.
    pub aspect_ratio: f64,
    /// Where the framed content is placed in the frame (payload `+164`),
    /// when the record is long enough to carry it.
    pub content_transform: Option<SmartFrameContentTransform>,
}

/// The placement an `igSmartFrame2d` puts its framed content at: RAD's 2D
/// transform, whose scale is kept apart from the linear part and multiplied
/// on its own when two transforms compose (`sub_5644E580`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmartFrameContentTransform {
    /// The 2×2 linear part, payload `+164..+195`, in stored order. The
    /// identity on every frame in the corpus.
    pub linear: [f64; 4],
    /// Translation in metres, payload `+196` / `+204`.
    pub translation_m: [f64; 2],
    /// Uniform scale, payload `+212`.
    pub scale: f64,
}

impl SmartFrameContentTransform {
    /// Whether the linear part is the identity, to `1e-9`.
    #[must_use]
    pub fn linear_is_identity(&self) -> bool {
        let [a, b, c, d] = self.linear;
        (a - 1.0).abs() < 1e-9 && b.abs() < 1e-9 && c.abs() < 1e-9 && (d - 1.0).abs() < 1e-9
    }
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
        read_smartframe_at(data, offset)?.ok()
    }

    fn advance_of(&self, record: &SheetIgSmartFrame2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// The live `igSmartFrame2d` record at `offset`, read once for both
/// answers: the frame, or why the record is not one. `None` where no live
/// record of the type starts whose payload the stream holds.
fn read_smartframe_at(
    data: &[u8],
    offset: usize,
) -> Option<Result<SheetIgSmartFrame2dDecoded, SheetSmartFrameRefused>> {
    let header = parse_live_psm_header(data, offset)?;
    if header.type_code != PSM_TYPE_CODE_IGSMARTFRAME {
        return None;
    }
    let payload_len = usize::try_from(header.bytes_to_follow).ok()?;
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
    let refused = |why: SmartFrameRefusal| SheetSmartFrameRefused {
        byte_range: offset..payload_end,
        oid: u32_at(0),
        state: u32_at(IGSMARTFRAME_CONTENT_FLAGS_AT)
            .zip(u32_at(IGSMARTFRAME_LINK_FLAGS_AT))
            .map(|(content, link)| smart_frame_state(content, link)),
        why,
    };
    if payload_len < IGSMARTFRAME_MIN_PAYLOAD {
        return Some(Err(refused(SmartFrameRefusal::ShortPayload {
            payload_len,
        })));
    }

    let content_flags = u32_at(IGSMARTFRAME_CONTENT_FLAGS_AT)?;
    let link_flags = u32_at(IGSMARTFRAME_LINK_FLAGS_AT)?;
    let extent_width_m = f64_at(IGSMARTFRAME_WIDTH_AT)?;
    let extent_height_m = f64_at(IGSMARTFRAME_HEIGHT_AT)?;
    let aspect_ratio = f64_at(IGSMARTFRAME_ASPECT_AT)?;
    let range_origin_m = [
        f64_at(IGSMARTFRAME_RANGE_ORIGIN_AT)?,
        f64_at(IGSMARTFRAME_RANGE_ORIGIN_AT + 8)?,
    ];
    let range_origin_m = range_origin_m
        .iter()
        .all(|v| v.is_finite())
        .then_some(range_origin_m);
    // The record states its extent twice over, and a frame's two
    // statements agree. The three doubles of a run of bytes that is not
    // a frame do not: this is what tells the 102 "frames" a walk read
    // off the corpora's chains after a wrong length from the 178 the
    // chains hold.
    if let Some(why) = smart_frame_extent_refusal(extent_width_m, extent_height_m, aspect_ratio) {
        return Some(Err(refused(why)));
    }

    let content_transform = (|| {
        let at = IGSMARTFRAME_CONTENT_TRANSFORM_AT;
        let values: Vec<f64> = (0..7)
            .map(|index| f64_at(at + 8 * index))
            .collect::<Option<_>>()?;
        values
            .iter()
            .all(|v| v.is_finite())
            .then(|| SmartFrameContentTransform {
                linear: [values[0], values[1], values[2], values[3]],
                translation_m: [values[4], values[5]],
                scale: values[6],
            })
    })();

    let state = smart_frame_state(content_flags, link_flags);

    Some(Ok(SheetIgSmartFrame2dDecoded {
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
        range_origin_m,
        extent_width_m,
        extent_height_m,
        aspect_ratio,
        content_transform,
    }))
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
        let header = parse_live_psm_header(data, offset)?;
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
        let header = parse_live_psm_header(data, offset)?;
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
        // Checked throughout: on a 32-bit `usize` (wasm32) a count near
        // `u32::MAX / 8` would wrap the sum back under `btf`.
        let closed_len = VARIABLES_MEMBERS_AT
            .checked_add(count.checked_mul(8)?)?
            .checked_add(4)?;
        if count == 0 || closed_len != btf {
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
        let header = parse_live_psm_header(data, offset)?;
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
        let header = parse_live_psm_header(data, offset)?;
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
        let mut values = Vec::with_capacity(crate::parsers::bounded_capacity(
            count,
            end.saturating_sub(at),
            FLAVOR_HOLDER_ENTRY_LEN,
        ));
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
        let header = parse_live_psm_header(data, offset)?;
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
        let decoded = decode_double_values(&chain_stream(&data));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 30);
        assert_eq!(decoded[0].parent_ref, 41);
        assert_eq!(decoded[0].value, 0.0609);
        assert_eq!(decoded[0].trailing, (1, 15));
        assert_eq!(
            decoded[0].byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + data.len()
        );
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
        let decoded = decode_variables(&chain_stream(&data));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 153);
        assert_eq!(decoded[0].members, vec![190, 744, 759]);
        assert_eq!(decoded[0].trailing, 321);
        assert_eq!(
            decoded[0].byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + data.len()
        );
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
        let decoded = decode_symbol_informations(&chain_stream(&data));
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
        assert_eq!(
            decoded[0].byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + data.len()
        );
    }

    #[test]
    fn symbol_information_decodes_the_stub_shape_with_no_variables() {
        let data = framed(
            PSM_TYPE_CODE_JSYMBOL_INFORMATION,
            &symbol_information_payload(95, &[]),
        );
        let decoded = decode_symbol_informations(&chain_stream(&data));
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
        let decoded = decode_standard_relations(&chain_stream(&data));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].oid, 44);
        assert_eq!(decoded[0].signature, "%>i%<i");
        assert_eq!(decoded[0].operands, vec![24, 33]);
        assert_eq!(decoded[0].formula, "0E$1+0.01");
        assert_eq!(
            decoded[0].byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + data.len()
        );
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
        assert!(decode_double_value_at(&data, 0).is_none());
        assert!(decode_variables_at(&data, 0).is_none());
        assert!(decode_symbol_information_at(&data, 0).is_none());
        assert!(decode_standard_relation_at(&data, 0).is_none());
    }

    #[test]
    fn every_family_survives_a_truncated_stream() {
        let full = framed(
            PSM_TYPE_CODE_JSYMBOL_INFORMATION,
            &symbol_information_payload(22, &[(1.0, "Left", 30)]),
        );
        for cut in 0..full.len() {
            assert!(decode_symbol_information_at(&full[..cut], 0).is_none());
            assert!(decode_double_value_at(&full[..cut], 0).is_none());
            assert!(decode_variables_at(&full[..cut], 0).is_none());
            assert!(decode_standard_relation_at(&full[..cut], 0).is_none());
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
// the corpus long kept only inside nested `JSite<N>/PSMcluster0` storages:
// `igCircle2d` (`0x0059`, 12 records across the first four fixtures) and
// `igArc2d` (`0x0061`, 12). Their layouts are native-reader grade twice
// over: Phase 36's byte statistics on the corpus and the `imagdex.dex`
// `IJPersist::DoIO` workers decompiled in
// `docs/analysis/2026-08-31-imagdex-geometry-doio-ida.md` agree byte for
// byte -- the 18-byte sub-header `igLine2d` opens with, then the doubles,
// then one flag byte.
//
// The nested-site copies stay **held back from the drawing**: that storage
// has no proven transform to the page
// (`docs/analysis/2026-08-31-jsite-geometry-coverage-gap.md`), and a circle
// drawn in the wrong coordinate space is worse than a circle named as
// missing. Their surface is `JSite::nested_geometry`.
//
// Top-level `Sheet*` copies are different: a sheet stream's transform is
// proven, so once W's `Sheet6` carried 17 `igCircle2d` records (the
// instrument balloons and sensing-bulb dots, verified against the
// reference plot) the family gained a `model::sheet_families` row and an
// `IgCircle2dEmitter` — the same record, read the same way, now draws.

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
/// (`docs/analysis/2026-07-27-ugeom2d1-curve-readers-ida.md`); the flag
/// byte says which of the two arcs between them the record means -- `0`
/// runs clockwise from the start angle, `1` counter-clockwise
/// (`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`).
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
    /// `+42`). The arc leaves it in the direction [`Self::flag`] names.
    pub start_angle: f64,
    /// Absolute end angle, radians counter-clockwise from +X (payload `+50`).
    pub end_angle: f64,
    /// The trailing byte (payload `+58`): the arc's traversal direction,
    /// `0` clockwise and `1` counter-clockwise -- the same byte, read the
    /// same way, as `igEllipticalArc2d`'s
    /// (`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`).
    /// Measured on the two records of `FT-Ultrasonic Flowmeter.sym` whose
    /// own angles are identical (`90° -> 270°`) and which differ only here:
    /// the wave's two halves bulge opposite ways, and the reference plot
    /// draws the S only when each record is read in its own direction.
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
        if !read.is_finite() || read.abs() > COORDINATE_DOMAIN_LIMIT {
            return None;
        }
        *value = read;
    }
    Some(out)
}

/// Decode every `igCircle2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_iglines`]: a candidate has to start where the
/// stream's own chain says a record starts ([`SheetChain::walk`]), then
/// satisfy: type code [`PSM_TYPE_CODE_IGCIRCLE2D`]; `bytes_to_follow ==
/// 43`; three finite in-domain doubles; `radius > 0`.
pub fn decode_igcircles(data: &[u8]) -> Vec<SheetIgCircle2dDecoded> {
    SheetChain::walk(data)
        .starts
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
    SheetChain::walk(data)
        .starts
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
        let header = parse_live_psm_header(data, offset)?;
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
        let header = parse_live_psm_header(data, offset)?;
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
// 2026-09-30: igEllipticalArc2d (cached bodies only)
// ---------------------------------------------------------------------------
//
// The corpus's first four `0x007E` records were all in A01, inside nested
// `JSite<N>/PSMcluster0` definition caches: `/JSite121` oids 489 / 490 are
// the two 2:1 heads of the vessel `V 010121A` (sheet 481, placed by oid
// 184), `/JSite39` oids 114 / 115 the heads of a body no placement names
// (sheet 96). Like the circles and arcs above they are read for the cached
// bodies only, and deliberately not registered in `model::sheet_families`
// (OCS plan `2026-09-30-a-cached-body-draws-its-elliptical-arcs`, E-D4).
//
// 2026-11-09: the library also writes `0x007E` into `.sym` bodies, not
// only drawing caches. `2to1Parametric V Drum.sym`, `Diaphragm
// Actuator.sym` and `PCV-Self-Actuated Regulator Valve.sym` each carry the
// dome/head arc the drawings using them already cache -- the same record,
// read the same way, from the symbol body instead of the drawing's own
// cache. `symbol_library::decode_primitive`'s `TYPE_ELLIPTICALARC` arm
// reads it via [`elliptical_arc_geometry`], the geometry function this
// family's `decode_at` also calls, so the two paths can never disagree on
// the curve. See `examples/probe_sym_elliptical_arcs.rs`.
//
// The payload is the shared 18-byte sub-header, seven doubles and one flag
// byte. Unlike `igArc2d`, the two angles come first:
//
// | payload | field |
// |---|---|
// | `+18` | sweep start angle |
// | `+26` | sweep end angle |
// | `+34`, `+42` | centre x, y |
// | `+50`, `+58` | major semi-axis vector x, y |
// | `+66` | minor / major ratio |
// | `+74` | flag byte |
//
// The angles are parameters of `P(t) = C + cos t · major + sin t · minor`,
// where `minor` is the major semi-axis turned +90° and scaled by the ratio.
// What is kept from `igArc2d` is its direction byte: `+74` is `0` for
// clockwise and `1` for counter-clockwise traversal from the start angle
// to the end angle, the same convention and the same trailing-byte
// position (relative to the record's own geometry fields) as `igArc2d`'s
// `+58` (`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`).
// All four A01 cached heads and the three `.sym` domes draw correctly only
// when their own `flag` is honoured -- folding it to the minor sweep
// mirrors the diaphragm actuator's dome, the same failure mode the arc note
// describes for `FT-Ultrasonic`, `Local Vent` and `Remarks`.

/// PSM type code for `igEllipticalArc2d` ("Elliptical Arc Object",
/// `imagdex.dex`; IGDS class tag `0x7E = 126`).
pub const PSM_TYPE_CODE_IGELLIPTICALARC2D: u16 = 0x007E;

/// Payload of one `igEllipticalArc2d`: the 18-byte sub-header, then
/// `sweep_start`, `sweep_end`, `center.x`, `center.y`, `major_axis.x`,
/// `major_axis.y`, `ratio` (7×f64) and one flag byte. Note the order: the
/// angles come *before* the centre, where `igArc2d` stores them after its
/// radius.
pub const IGELLIPTICALARC2D_PAYLOAD_LEN: usize = 75;

/// One decoded `0x007E` `igEllipticalArc2d` record.
///
/// The ellipse is `P(t) = center + cos t · major_axis + sin t · minor`,
/// with `minor = ratio · (−major_axis.y, major_axis.x)` -- the major
/// semi-axis turned +90°. The arc runs from `t = sweep_start` to
/// `t = sweep_end` in the direction [`Self::flag`] names -- clockwise
/// (decreasing `t`) when `flag == 0`, counter-clockwise otherwise; the
/// same per-record direction byte [`SheetIgArc2dDecoded`] carries, not a
/// fixed convention. Lengths are in the storage's own units (metres in
/// every nested cache).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgEllipticalArc2dDecoded {
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
    /// Parameter the arc starts at, radians from the major semi-axis
    /// towards the minor one (payload `+18`). The arc leaves it
    /// **clockwise** -- decreasing `t` -- towards [`Self::sweep_end`].
    pub sweep_start: f64,
    /// Parameter the arc ends at, same measure (payload `+26`).
    pub sweep_end: f64,
    /// Centre, in the storage's own coordinates (payload `+34`, `+42`).
    pub center: (f64, f64),
    /// Major semi-axis vector, same units (payload `+50`, `+58`); never
    /// zero.
    pub major_axis: (f64, f64),
    /// Ratio of the second semi-axis (`minor` in [`Self`]'s formula) to
    /// `major_axis`'s own length (payload `+66`), finite and `> 0`. Not
    /// always `<= 1`: the nozzle-label symbols' records reach `3.0` --
    /// see the `2026-11-09` note above [`PSM_TYPE_CODE_IGELLIPSE2D`].
    pub ratio: f64,
    /// The trailing byte (payload `+74`): the arc's traversal direction,
    /// `0` clockwise and `1` counter-clockwise -- the same byte, read the
    /// same way, as [`SheetIgArc2dDecoded::flag`]
    /// (`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`).
    /// Measured on the diaphragm actuator's dome (`oid 429` /
    /// `Diaphragm Actuator.sym` and its two copies), which mirrors unless
    /// read in its own direction.
    pub flag: u8,
}

/// Decode every `igEllipticalArc2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_igarcs`]: a candidate has to start where the
/// stream's own chain says a record starts ([`SheetChain::walk`]), then
/// satisfy: type code [`PSM_TYPE_CODE_IGELLIPTICALARC2D`];
/// `bytes_to_follow == 75`; seven finite in-domain doubles; a non-zero
/// major semi-axis; a finite `ratio > 0` (not capped at `1` -- see the
/// `2026-11-09` note above [`PSM_TYPE_CODE_IGELLIPSE2D`]). As for
/// `igArc2d`, the angles only have to be finite: they are not normalised.
pub fn decode_igellipticalarcs(data: &[u8]) -> Vec<SheetIgEllipticalArc2dDecoded> {
    SheetChain::walk(data)
        .starts
        .into_iter()
        .filter_map(|at| IgEllipticalArc2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igEllipticalArc2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igellipticalarc_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetIgEllipticalArc2dDecoded> {
    IgEllipticalArc2dDecoder.decode_at(data, offset)
}

/// Centre, major semi-axis vector, minor/major ratio and the two sweep
/// parameters (`sweep_start`, `sweep_end`) of an `igEllipticalArc2d`
/// payload.
pub(crate) type EllipticalArcGeometry = ((f64, f64), (f64, f64), f64, f64, f64);

/// The centre, major semi-axis, ratio and sweep of an
/// `igEllipticalArc2d` payload, held to the same rules
/// [`decode_igellipticalarcs`] applies. Shared with the `.sym` reader
/// (`src/symbol_library.rs`), whose payloads are the same bytes — a `.sym`
/// body's dome or vessel head is this same record, not a drawing-only one.
pub(crate) fn elliptical_arc_geometry(payload: &[u8]) -> Option<EllipticalArcGeometry> {
    if payload.len() < IGELLIPTICALARC2D_PAYLOAD_LEN {
        return None;
    }
    let [sweep_start, sweep_end, center_x, center_y, major_x, major_y, ratio] =
        curve_doubles::<7>(payload)?;
    // Every double is finite and inside the domain here, so `hypot`
    // is finite too and `<= 0` means a zero-length major axis. `ratio` is
    // not capped at `1` for the same reason `ellipse_geometry` drops the
    // cap: the nozzle-label symbols' two `0x007E` records per sheet carry
    // `ratio` up to `3.0`, and the arc this traces is exact for any
    // finite positive ratio regardless of which semi-axis is longer. See
    // the `2026-11-09` note above `PSM_TYPE_CODE_IGELLIPSE2D`.
    if major_x.hypot(major_y) <= 0.0 || ratio <= 0.0 {
        return None;
    }
    Some((
        (center_x, center_y),
        (major_x, major_y),
        ratio,
        sweep_start,
        sweep_end,
    ))
}

/// [`PsmRecordDecoder`] adapter for `0x007E` `igEllipticalArc2d`.
pub struct IgEllipticalArc2dDecoder;

impl PsmRecordDecoder for IgEllipticalArc2dDecoder {
    type Record = SheetIgEllipticalArc2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGELLIPTICALARC2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGELLIPTICALARC2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgEllipticalArc2dDecoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGELLIPTICALARC2D
            || header.bytes_to_follow as usize != IGELLIPTICALARC2D_PAYLOAD_LEN
        {
            return None;
        }
        let end = header
            .body_start
            .checked_add(IGELLIPTICALARC2D_PAYLOAD_LEN)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let (center, major_axis, ratio, sweep_start, sweep_end) = elliptical_arc_geometry(payload)?;
        Some(SheetIgEllipticalArc2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            sweep_start,
            sweep_end,
            center,
            major_axis,
            ratio,
            flag: *payload.get(IGELLIPTICALARC2D_PAYLOAD_LEN - 1)?,
        })
    }

    fn advance_of(&self, record: &SheetIgEllipticalArc2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

// ---------------------------------------------------------------------------
// 2026-11-06: igEllipse2d (closed ellipse)
// ---------------------------------------------------------------------------
//
// `0x0063` is `igEllipse2d` ("Ellipse Object", `imagdex.dex`, CLSID
// `A3494010-D3E1-11CD-8AEA-08003601B44A`). It shares `igArc2d`'s 59-byte
// payload length, so the type code alone tells them apart. The layout was
// read off `Air Cooler Type 1.sym`'s two fan ovals and checked against the
// reference plot of the three A1 air-cooler sheets, which print both:
//
// | payload | field |
// |---|---|
// | `+18`, `+26` | centre x, y |
// | `+34`, `+42` | major semi-axis vector x, y |
// | `+50` | minor / major ratio |
// | `+58` | flag byte |
//
// The ellipse is `P(t) = C + cos t · major + sin t · minor`, `minor` the
// major semi-axis turned +90° and scaled by the ratio, as for
// `igEllipticalArc2d`; a closed ellipse has no sweep to read.
//
// 2026-11-09: the library's `Nozzle NPD.sym` / `Nozzle Rating.sym` (the
// nozzle size/rating call-out labels, `Equipment Components\Labels -
// Equipment Components\`) write 10 `0x0063` records apiece with `ratio`
// in `1.92..=3.00` -- the field at `+34`/`+42` is the *shorter* semi-axis
// there, not the longer one, and `ratio` scales it up rather than down.
// The parametric curve `P(t) = C + cos t · major + sin t · ratio ·
// rot90(major)` is an exact ellipse for *any* finite positive `ratio`,
// not only `ratio <= 1`: which semi-axis the record calls `major_axis` is
// a labelling choice, not a constraint the formula needs. A full-corpus
// census of every live `0x0063`/`0x007E` record in the `.pid`/`.sym`
// fixtures and every `RefData~*.zip` (`examples/probe_ellipse_ratio_census.rs`)
// found `ratio > 1` nowhere else: 144 hits, all inside these two symbols,
// topping out at `3.0000`. The upper bound is dropped rather than raised
// to a specific ceiling -- `ratio > 0` is what the geometry actually
// requires -- but the two-sided finite/positive check stays, so a NaN,
// infinite or non-positive ratio is still refused.

/// PSM type code for `igEllipse2d` ("Ellipse Object", `imagdex.dex`; IGDS
/// class tag `0x63 = 99`).
pub const PSM_TYPE_CODE_IGELLIPSE2D: u16 = 0x0063;

/// Payload of one `igEllipse2d`: the 18-byte sub-header, then `center.x`,
/// `center.y`, `major_axis.x`, `major_axis.y`, `ratio` (5×f64) and one flag
/// byte -- the same length as an `igArc2d` payload.
pub const IGELLIPSE2D_PAYLOAD_LEN: usize = 59;

/// One decoded `0x0063` `igEllipse2d` record: a closed ellipse
/// `P(t) = center + cos t · major_axis + sin t · minor`, with
/// `minor = ratio · (−major_axis.y, major_axis.x)`. Lengths are in the
/// storage's own units.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgEllipse2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this ellipse sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); semantics not decoded.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`).
    pub index: u32,
    /// Centre, in the storage's own coordinates (payload `+18`, `+26`).
    pub center: (f64, f64),
    /// Major semi-axis vector, same units (payload `+34`, `+42`); never
    /// zero.
    pub major_axis: (f64, f64),
    /// Ratio of the second semi-axis (`minor` in [`Self`]'s formula) to
    /// `major_axis`'s own length (payload `+50`), finite and `> 0`. Despite
    /// the name, this is not always `<= 1`: the nozzle-label symbols write
    /// it as high as `3.0`, which makes `major_axis` the record's *shorter*
    /// semi-axis -- see the `2026-11-09` note above [`PSM_TYPE_CODE_IGELLIPSE2D`].
    pub ratio: f64,
    /// The trailing byte (payload `+58`); meaning unknown, carried for
    /// audit.
    pub flag: u8,
}

/// Decode every `igEllipse2d` record in a record-chain stream.
///
/// Chain-gated like [`decode_igarcs`], then: type code
/// [`PSM_TYPE_CODE_IGELLIPSE2D`]; `bytes_to_follow == 59`; five finite
/// in-domain doubles; a non-zero major semi-axis; a finite `ratio > 0`
/// (not capped at `1` -- see the `2026-11-09` note above
/// [`PSM_TYPE_CODE_IGELLIPSE2D`]).
pub fn decode_igellipses(data: &[u8]) -> Vec<SheetIgEllipse2dDecoded> {
    SheetChain::walk(data)
        .starts
        .into_iter()
        .filter_map(|at| IgEllipse2dDecoder.decode_at(data, at))
        .collect()
}

/// Try to decode one `igEllipse2d` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igellipse_at(data: &[u8], offset: usize) -> Option<SheetIgEllipse2dDecoded> {
    IgEllipse2dDecoder.decode_at(data, offset)
}

/// Centre, major semi-axis vector and minor/major ratio of a closed ellipse.
pub(crate) type EllipseGeometry = ((f64, f64), (f64, f64), f64);

/// The centre, major semi-axis and ratio of an `igEllipse2d` payload, held
/// to the same rules [`decode_igellipses`] applies. Shared with the `.sym`
/// reader, whose payloads are the same bytes.
pub(crate) fn ellipse_geometry(payload: &[u8]) -> Option<EllipseGeometry> {
    if payload.len() < IGELLIPSE2D_PAYLOAD_LEN {
        return None;
    }
    let [center_x, center_y, major_x, major_y, ratio] = curve_doubles::<5>(payload)?;
    // `ratio` is not capped at `1`: the nozzle-label symbols write it up
    // to `3.0`, naming the record's shorter semi-axis as `major_axis` --
    // see the `2026-11-09` note above `PSM_TYPE_CODE_IGELLIPSE2D`. The
    // curve this traces is an exact ellipse for any finite positive
    // ratio, so only non-positive and non-finite values are refused
    // (`curve_doubles` already guarantees `ratio` is finite here).
    if major_x.hypot(major_y) <= 0.0 || ratio <= 0.0 {
        return None;
    }
    Some(((center_x, center_y), (major_x, major_y), ratio))
}

/// [`PsmRecordDecoder`] adapter for `0x0063` `igEllipse2d`.
pub struct IgEllipse2dDecoder;

impl PsmRecordDecoder for IgEllipse2dDecoder {
    type Record = SheetIgEllipse2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGELLIPSE2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGELLIPSE2D_PAYLOAD_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgEllipse2dDecoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGELLIPSE2D
            || header.bytes_to_follow as usize != IGELLIPSE2D_PAYLOAD_LEN
        {
            return None;
        }
        let end = header.body_start.checked_add(IGELLIPSE2D_PAYLOAD_LEN)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let (center, major_axis, ratio) = ellipse_geometry(payload)?;
        Some(SheetIgEllipse2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            center,
            major_axis,
            ratio,
            flag: *payload.get(IGELLIPSE2D_PAYLOAD_LEN - 1)?,
        })
    }

    fn advance_of(&self, record: &SheetIgEllipse2dDecoded) -> usize {
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
/// `IGRECTANGLE2D_MAX_EDGES` whose list accounts for the payload exactly.
pub fn decode_igrectangles(data: &[u8]) -> Vec<SheetIgRectangle2dDecoded> {
    SheetChain::walk(data)
        .starts
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
    SheetChain::walk(data)
        .starts
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
        let header = parse_live_psm_header(data, offset)?;
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
    let in_domain = |value: f64| value.is_finite() && value.abs() <= COORDINATE_DOMAIN_LIMIT;
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
        let header = parse_live_psm_header(data, offset)?;
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

// ---------------------------------------------------------------------------
// 2026-11-10: igComplexString2d and igGroup — the two association records
// ---------------------------------------------------------------------------
//
// Two graphic classes a sheet files curves and rectangles under, neither of
// which carries a coordinate of its own
// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
// §2.1, §2.4):
//
// * `0x0021` **ComplexString** — a run of curves joined end to end. Its
//   payload is a style id, a member count, the scale of the members' range
//   boxes and one 14-byte entry per member: the member's oid, its range
//   box, and a word that is `0x8000` on the `0x0019 KeyPoint Constraint`
//   records that pin one curve's end to the next one's start and `0x0000` /
//   `0x0005` on the curves. After the entries the record closes, in one of
//   two ways: `00 02 00 00` on a sheet, `00 03 00 00` and one `u32` a
//   member in a symbol cache. Every curve member of the eight sheet records
//   the corpora hold is a live `igLine2d` or `igArc2d` of the same stream
//   whose owner is the string's oid and whose layer and style id are the
//   string's — so the members, which decode and emit on their own, already
//   draw what the string draws, with the pen the string would draw it with.
// * `0x007B` **Group** — a count, the same scale and the same 14-byte
//   entries. The corpora hold twelve on their sheets. Eight list the two
//   `igRectangle2d` of a border (outer frame and inner frame), whose edges
//   are `igLine2d` records of their own. Four — one in each of `A0-W.igr`
//   to `A3-W.igr`, templates a backup package keeps in its
//   `RefData~4~682.zip` — list the 37 records of a logo: sixteen lines,
//   seven arcs, four B-splines, four elliptical arcs, five boundaries and a
//   point. In both kinds every member is a live record of the same stream
//   whose owner is the group's oid.
//
// The double after the count is the unit of the range boxes: a box is in
// `scale / 32768` m. It is `1.0` on fourteen of the twenty records and `2.0`
// on six — `CPECCHB-SQ-A1.igr`'s outer frame, and the groups of the A0 and
// A1 `-W` templates. Read with it, the box of every line and arc member
// lies within 0.76 mm of the member's own extent (a template string's box
// is grown by half its pen); read in 1/32768 m, those six are out by up to
// 529 mm
// (`docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
// §16).
//
// Both decode so that they stop being graphic records nothing reads, and
// both emit nothing: emitting either would draw its members twice.

/// PSM type code for `ComplexString Object` (IGDS class tag `0x21 = 33`): a
/// run of curves joined end to end.
pub const PSM_TYPE_CODE_IGCOMPLEXSTRING2D: u16 = 0x0021;

/// PSM type code for `Group implementation` (IGDS class tag `0x7B = 123`): a
/// set of graphic objects that select and move as one.
pub const PSM_TYPE_CODE_IGGROUP: u16 = 0x007B;

/// Bytes one member entry takes in either record: `u32 oid`, four `i16` of
/// range box, a `u16` word.
pub const ASSOCIATION_MEMBER_LEN: usize = 14;

/// Payload offset of a `ComplexString`'s member count; the entries start
/// twelve bytes on, after the double.
const IGCOMPLEXSTRING2D_COUNT_AT: usize = 21;
/// Payload offset of a `ComplexString`'s first member entry.
const IGCOMPLEXSTRING2D_MEMBERS_AT: usize = 33;
/// Payload offset of a Group's member count.
const IGGROUP_COUNT_AT: usize = 16;
/// Payload offset of a Group's first member entry.
const IGGROUP_MEMBERS_AT: usize = 28;

/// The word a `ComplexString` member entry carries when the member is a
/// constraint between two of its curves rather than a curve.
pub const IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER: u16 = 0x8000;

/// How a `ComplexString` closes on a sheet: these four bytes after its last
/// member entry, and nothing after them.
const IGCOMPLEXSTRING2D_CLOSE_ON_A_SHEET: [u8; 4] = [0x00, 0x02, 0x00, 0x00];
/// How a `ComplexString` closes in a symbol cache: these four bytes after
/// its last member entry, and then one `u32` a member.
const IGCOMPLEXSTRING2D_CLOSE_IN_A_CACHE: [u8; 4] = [0x00, 0x03, 0x00, 0x00];

/// Whether `trailing` — what a `ComplexString`'s payload holds after its
/// `count` member entries — is one of the two ways a string closes.
///
/// This is what tells a string from a run of bytes read as one. A record's
/// length and its member count are stated apart, and a string was held only
/// to a count the payload has room for: any payload long enough passed,
/// with whatever was left over for a trailer. A walk that a wrong length
/// has sent off the record boundaries reads such payloads — on one sheet of
/// the plant backup, 28 bytes into each of two `DependencyObject` records,
/// a type word `0x0021`, a length of 65 536 and a count of 256, with 61 919
/// bytes after the entries — and four of the 1 371 448 records such walks
/// read off the corpora's chains decoded so.
///
/// Every `ComplexString` on a chain of the corpora closes in one of these
/// two ways, 34 records of 885 documents: the eight on sheets in the first
/// (strings of 3, 5, 8 and 11 members), the 26 in the `PSMcluster0` of a
/// symbol cache in the second (3, 4 and 8 members, and so 16, 20 and 36
/// bytes; every one of those `u32` is zero, which is not held). No style
/// table holds a string. What the two forms state is not decoded; that the
/// length of the second follows the count is what the three sizes show.
///
/// A Group's closing is not held the same way. Its records on sheets all
/// close `00 02 00 00`, but 259 more live in style tables, read by
/// `style_link`, and two of those close `00 00 00 00`.
fn igcomplexstring_closes(trailing: &[u8], count: usize) -> bool {
    let Some((close, after)) = trailing.split_first_chunk::<4>() else {
        return false;
    };
    if *close == IGCOMPLEXSTRING2D_CLOSE_ON_A_SHEET {
        after.is_empty()
    } else if *close == IGCOMPLEXSTRING2D_CLOSE_IN_A_CACHE {
        count.checked_mul(4) == Some(after.len())
    } else {
        false
    }
}

/// One member entry of a [`SheetIgComplexString2dDecoded`] or a
/// [`SheetIgGroupDecoded`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetAssociationMember {
    /// Oid of the member record, in the same stream.
    pub oid: u32,
    /// The member's range box, `[min x, min y, max x, max y]`, in units of
    /// the record's range scale over 32768 metres
    /// ([`SheetIgComplexString2dDecoded::range_scale`],
    /// [`SheetIgGroupDecoded::range_scale`]; [`Self::range_box_m`] reads it
    /// in metres). Measured against the members' own geometry: a Group's
    /// rectangle and each curve of `D04`'s string (entry word `0x0005`) read
    /// the bare extent of the curve to within 0.041 mm — an arc's is the box
    /// of the part it sweeps, not of its circle — while the lines of the
    /// template strings (entry word `0x0000`) read it grown by half their
    /// pen on every side: the first edge of `CPECCHB-SQ-A2`'s inner frame,
    /// from x = 35.354 mm to 586.869 mm at y = 7.071 mm, reads
    /// `[1135, 208, 19253, 255]`, which is 34.637 .. 587.555 by 6.348 ..
    /// 7.782 mm.
    pub range_box: [i16; 4],
    /// The word that closes the entry. On a `ComplexString` it is
    /// [`IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER`] for a `KeyPoint Constraint`
    /// and `0x0000` or `0x0005` for a curve; on a Group, `0x0006` and
    /// `0x000C` on a border's two rectangles and `0x0003` or `0x0000` on a
    /// logo's members. What the low values mean is not decoded.
    pub word: u16,
}

impl SheetAssociationMember {
    /// The range box in metres, `[min x, min y, max x, max y]`, read in the
    /// unit of the record that lists the member: `range_scale / 32768` m.
    #[must_use]
    pub fn range_box_m(&self, range_scale: f64) -> [f64; 4] {
        self.range_box
            .map(|units| f64::from(units) * range_scale / 32768.0)
    }
}

/// One decoded `0x0021` `ComplexString` record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgComplexString2dDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` this string sits on (payload `+8`).
    pub sheet_layer_ref: u32,
    /// Sub-type discriminator (payload `+12`); zero on the corpus.
    pub sub_type_word: u16,
    /// Index / style reference (payload `+14`). Every curve member of every
    /// corpus string carries the same one.
    pub index: u32,
    /// The three bytes between the style id and the count (payload `+18`),
    /// verbatim: `01 06 00` on seven of the corpora's eight records and
    /// `01 00 00` on one, oid 31 of `CPECCHB-SQ-A1.igr`. Not decoded.
    pub head: [u8; 3],
    /// The scale of the members' range boxes (payload `+25`, the double
    /// after the count): a box is in units of `range_scale / 32768` m.
    /// `1.0` on seven of the corpora's eight records and `2.0` on one, the
    /// outer frame of `CPECCHB-SQ-A1.igr` (oid 21): its top edge runs from
    /// x = 25 mm to 831 mm at y = 584 mm and reads
    /// `[407, 9565, 13617, 9571]`, which is 24.84 .. 831.12 by 583.80 ..
    /// 584.17 mm at 2/32768 m and half of each at 1/32768 m.
    pub range_scale: f64,
    /// The members, in on-disk order: the curves first, then the constraints
    /// that join them.
    pub members: Vec<SheetAssociationMember>,
    /// The bytes after the last member entry, verbatim: how the record
    /// closes, which is one of two ways and is what a record is held to.
    /// `00 02 00 00` on a sheet; `00 03 00 00` and then one `u32` a member
    /// in a symbol cache, where every such `u32` of the corpora is zero.
    /// What either states is not decoded.
    pub trailing: Vec<u8>,
}

impl SheetIgComplexString2dDecoded {
    /// The members that are curves, in order.
    pub fn curve_members(&self) -> impl Iterator<Item = &SheetAssociationMember> {
        self.members
            .iter()
            .filter(|member| member.word != IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER)
    }

    /// The range box of one of this string's members, in metres.
    #[must_use]
    pub fn range_box_m(&self, member: &SheetAssociationMember) -> [f64; 4] {
        member.range_box_m(self.range_scale)
    }
}

/// One decoded `0x007B` Group record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetIgGroupDecoded {
    /// Byte range covering the full PSM record (envelope + payload).
    pub byte_range: std::ops::Range<usize>,
    /// Top 2 bits of the PSM type word (record-level flags).
    pub type_flags: u16,
    /// Object identifier (payload `+0`).
    pub oid: u32,
    /// Low half of the envelope's `aux` pair (payload `+4`), verbatim.
    pub parent_ref: u32,
    /// Oid of the `JSheetLayer` the group sits on (payload `+8`); zero on
    /// the corpus — a group is on no layer of its own.
    pub sheet_layer_ref: u32,
    /// The `u32` at payload `+12`, verbatim; zero on the corpus.
    pub head: u32,
    /// The scale of the members' range boxes (payload `+20`, the double
    /// after the count): a box is in units of `range_scale / 32768` m.
    /// `1.0` on seven of the corpora's twelve records and `2.0` on five:
    /// the border groups of `A0-W.igr`, `A0-W - New.igr`, `A1-W.igr` and
    /// `A1-W - New.igr`, and the logo of `A0-W.igr`. An A0 border's outer
    /// rectangle reads `[0, 0, 19480, 13779]`, 1189 by 841 mm at 2/32768 m;
    /// a box in 1/32768 m ends at one metre.
    pub range_scale: f64,
    /// The members, in on-disk order.
    pub members: Vec<SheetAssociationMember>,
    /// The bytes after the last member entry, verbatim: `00 02 00 00` on a
    /// sheet.
    pub trailing: Vec<u8>,
}

impl SheetIgGroupDecoded {
    /// The range box of one of this group's members, in metres.
    #[must_use]
    pub fn range_box_m(&self, member: &SheetAssociationMember) -> [f64; 4] {
        member.range_box_m(self.range_scale)
    }
}

/// `count` member entries from `payload[at..]`, or `None` when the payload
/// is too short to hold them.
fn association_members(
    payload: &[u8],
    at: usize,
    count: usize,
) -> Option<Vec<SheetAssociationMember>> {
    let end = at.checked_add(count.checked_mul(ASSOCIATION_MEMBER_LEN)?)?;
    let (entries, _) = payload.get(at..end)?.as_chunks::<ASSOCIATION_MEMBER_LEN>();
    Some(
        entries
            .iter()
            .map(|entry| {
                let i16_at = |i: usize| i16::from_le_bytes([entry[i], entry[i + 1]]);
                SheetAssociationMember {
                    oid: u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]),
                    range_box: [i16_at(4), i16_at(6), i16_at(8), i16_at(10)],
                    word: u16::from_le_bytes([entry[12], entry[13]]),
                }
            })
            .collect(),
    )
}

/// Decode every `ComplexString` record on a record-chain stream.
///
/// At each record the chain starts: type code
/// [`PSM_TYPE_CODE_IGCOMPLEXSTRING2D`]; a member count of at least one whose
/// entries the payload has room for; a finite range scale before them; and
/// after them the record's closing, which is four bytes on a sheet and four
/// bytes with a `u32` a member in a symbol cache
/// ([`SheetIgComplexString2dDecoded::trailing`]). The count is bounded by
/// the payload, not by a constant: a string is as long as the record that
/// holds it — and no longer, which is what the closing holds it to. A
/// payload with room for its count and anything at all after the entries
/// was a string until 2026-11-12, and four runs of bytes a walk reads off
/// a chain of the plant backup decoded as one.
pub fn decode_igcomplexstrings(data: &[u8]) -> Vec<SheetIgComplexString2dDecoded> {
    IgComplexString2dDecoder.scan(data)
}

/// Try to decode one `ComplexString` record at `offset`. `None` on any
/// validation failure; panic-free on arbitrary input.
pub fn decode_igcomplexstring_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetIgComplexString2dDecoded> {
    IgComplexString2dDecoder.decode_at(data, offset)
}

/// Decode every Group record on a record-chain stream.
///
/// At each record the chain starts: type code [`PSM_TYPE_CODE_IGGROUP`]; a
/// member count of at least one whose entries the payload has room for; a
/// finite range scale before them.
pub fn decode_iggroups(data: &[u8]) -> Vec<SheetIgGroupDecoded> {
    IgGroupDecoder.scan(data)
}

/// Try to decode one Group record at `offset`. `None` on any validation
/// failure; panic-free on arbitrary input.
pub fn decode_iggroup_at(data: &[u8], offset: usize) -> Option<SheetIgGroupDecoded> {
    IgGroupDecoder.decode_at(data, offset)
}

/// [`PsmRecordDecoder`] adapter for `0x0021` `ComplexString`.
pub struct IgComplexString2dDecoder;

impl PsmRecordDecoder for IgComplexString2dDecoder {
    type Record = SheetIgComplexString2dDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGCOMPLEXSTRING2D
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGCOMPLEXSTRING2D_MEMBERS_AT + ASSOCIATION_MEMBER_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgComplexString2dDecoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGCOMPLEXSTRING2D {
            return None;
        }
        let end = header
            .body_start
            .checked_add(usize::try_from(header.bytes_to_follow).ok()?)?;
        let payload = data.get(header.body_start..end)?;
        let sub = curve_sub_header(payload)?;
        let head = <[u8; 3]>::try_from(payload.get(CURVE_GEOMETRY_AT..IGCOMPLEXSTRING2D_COUNT_AT)?)
            .ok()?;
        let count = usize::try_from(u32_le(payload, IGCOMPLEXSTRING2D_COUNT_AT)?).ok()?;
        let range_scale = f64_le(payload, IGCOMPLEXSTRING2D_COUNT_AT + 4)?;
        if count == 0 || !range_scale.is_finite() {
            return None;
        }
        let members = association_members(payload, IGCOMPLEXSTRING2D_MEMBERS_AT, count)?;
        let trailing =
            payload.get(IGCOMPLEXSTRING2D_MEMBERS_AT + count * ASSOCIATION_MEMBER_LEN..)?;
        if !igcomplexstring_closes(trailing, count) {
            return None;
        }
        let trailing = trailing.to_vec();
        Some(SheetIgComplexString2dDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: sub.oid,
            parent_ref: sub.parent_ref,
            sheet_layer_ref: sub.sheet_layer_ref,
            sub_type_word: sub.sub_type_word,
            index: sub.index,
            head,
            range_scale,
            members,
            trailing,
        })
    }

    fn advance_of(&self, record: &SheetIgComplexString2dDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

/// [`PsmRecordDecoder`] adapter for `0x007B` Group.
pub struct IgGroupDecoder;

impl PsmRecordDecoder for IgGroupDecoder {
    type Record = SheetIgGroupDecoded;

    fn type_code(&self) -> u16 {
        PSM_TYPE_CODE_IGGROUP
    }

    fn min_record_len(&self) -> usize {
        PSM_ENVELOPE_LEN + IGGROUP_MEMBERS_AT + ASSOCIATION_MEMBER_LEN
    }

    fn decode_at(&self, data: &[u8], offset: usize) -> Option<SheetIgGroupDecoded> {
        let header = parse_live_psm_header(data, offset)?;
        if header.type_code != PSM_TYPE_CODE_IGGROUP {
            return None;
        }
        let end = header
            .body_start
            .checked_add(usize::try_from(header.bytes_to_follow).ok()?)?;
        let payload = data.get(header.body_start..end)?;
        let count = usize::try_from(u32_le(payload, IGGROUP_COUNT_AT)?).ok()?;
        let range_scale = f64_le(payload, IGGROUP_COUNT_AT + 4)?;
        if count == 0 || !range_scale.is_finite() {
            return None;
        }
        let members = association_members(payload, IGGROUP_MEMBERS_AT, count)?;
        let trailing = payload
            .get(IGGROUP_MEMBERS_AT + count * ASSOCIATION_MEMBER_LEN..)?
            .to_vec();
        Some(SheetIgGroupDecoded {
            byte_range: offset..end,
            type_flags: header.type_flags,
            oid: u32_le(payload, 0)?,
            parent_ref: u32_le(payload, 4)?,
            sheet_layer_ref: u32_le(payload, 8)?,
            head: u32_le(payload, 12)?,
            range_scale,
            members,
            trailing,
        })
    }

    fn advance_of(&self, record: &SheetIgGroupDecoded) -> usize {
        record
            .byte_range
            .end
            .saturating_sub(record.byte_range.start)
    }
}

#[cfg(test)]
mod association_family_tests {
    use super::*;

    /// `CPECCHB-SQ-A2.igr`'s `/Sheet6 +2326`, oid 67, payload verbatim: a
    /// string of two lines — the title block's top edge and its left edge —
    /// and the one constraint that joins them.
    const STRING_OF_TWO: [u8; 79] = [
        0x43, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c,
        0x00, 0x00, 0x00, 0x01, 0x06, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xf0, 0x3f, 0x44, 0x00, 0x00, 0x00, 0xb3, 0x31, 0x39, 0x0e, 0x30, 0x4b, 0x60, 0x0e,
        0x00, 0x00, 0x45, 0x00, 0x00, 0x00, 0xb3, 0x31, 0xed, 0xff, 0xd9, 0x31, 0x60, 0x0e, 0x00,
        0x00, 0x46, 0x00, 0x00, 0x00, 0xc6, 0x31, 0x4c, 0x0e, 0xc7, 0x31, 0x4d, 0x0e, 0x00, 0x80,
        0x00, 0x02, 0x00, 0x00,
    ];

    /// `CPE-SQ-A0.igr`'s `/JSite16/PSMcluster0 +1789`, oid 33, payload
    /// verbatim: a string kept in a symbol cache — two curves and the
    /// constraint that joins them — which closes the way the strings of a
    /// cache do, `00 03 00 00` and a `u32` a member.
    const STRING_IN_A_CACHE: [u8; 91] = [
        0x21, 0x00, 0x00, 0x00, 0x1d, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a,
        0x00, 0x00, 0x00, 0x01, 0x06, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xf0, 0x3f, 0x22, 0x00, 0x00, 0x00, 0xb4, 0xff, 0xf2, 0xff, 0x00, 0x00, 0x0e, 0x00,
        0x05, 0x00, 0x23, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf1, 0xff, 0x4c, 0x00, 0x0e, 0x00, 0x05,
        0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x80,
        0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];

    /// `A01.pid`'s `/JSite204/Sheet6 +2872`, oid 311, payload verbatim: the
    /// group of the embedded border's two rectangles.
    const GROUP_OF_TWO: [u8; 60] = [
        0x37, 0x01, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, 0x27, 0x01,
        0x00, 0x00, 0x33, 0x03, 0x47, 0x01, 0xc0, 0x4a, 0x7b, 0x34, 0x0c, 0x00, 0x72, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x4c, 0xc2, 0x35, 0x06, 0x00, 0x00, 0x02, 0x00, 0x00,
    ];

    /// `CPECCHB-SQ-A1.igr`'s `/Sheet6 +8`, oid 21, payload verbatim: the
    /// sheet's outer frame, a string of four lines and the four constraints
    /// that close it — and the one string of the corpora whose range scale
    /// is 2.
    const FRAME_AT_SCALE_TWO: [u8; 149] = [
        0x15, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09,
        0x00, 0x00, 0x00, 0x01, 0x06, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x40, 0x16, 0x00, 0x00, 0x00, 0x97, 0x01, 0x5d, 0x25, 0x31, 0x35, 0x63, 0x25,
        0x00, 0x00, 0x17, 0x00, 0x00, 0x00, 0x2b, 0x35, 0xa1, 0x00, 0x31, 0x35, 0x63, 0x25, 0x00,
        0x00, 0x18, 0x00, 0x00, 0x00, 0x97, 0x01, 0xa1, 0x00, 0x31, 0x35, 0xa7, 0x00, 0x00, 0x00,
        0x19, 0x00, 0x00, 0x00, 0x97, 0x01, 0xa1, 0x00, 0x9d, 0x01, 0x63, 0x25, 0x00, 0x00, 0x1a,
        0x00, 0x00, 0x00, 0x2e, 0x35, 0x5f, 0x25, 0x2f, 0x35, 0x60, 0x25, 0x00, 0x80, 0x1b, 0x00,
        0x00, 0x00, 0x2e, 0x35, 0xa3, 0x00, 0x2f, 0x35, 0xa4, 0x00, 0x00, 0x80, 0x1c, 0x00, 0x00,
        0x00, 0x99, 0x01, 0xa3, 0x00, 0x9a, 0x01, 0xa4, 0x00, 0x00, 0x80, 0x1d, 0x00, 0x00, 0x00,
        0x99, 0x01, 0x5f, 0x25, 0x9a, 0x01, 0x60, 0x25, 0x00, 0x80, 0x00, 0x02, 0x00, 0x00,
    ];

    /// The first two members of that frame, payloads verbatim: `+163`,
    /// oid 22, the top edge from (25, 584) to (831, 584) mm, and `+219`,
    /// oid 23, the right edge from there down to (831, 10) mm.
    const FRAME_EDGES: [[u8; 50]; 2] = [
        [
            0x16, 0x00, 0x00, 0x00, 0x15, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x09, 0x00, 0x00, 0x00, 0x5f, 0xba, 0x2d, 0x93, 0x98, 0x99, 0x99, 0x3f, 0x49, 0x20,
            0x7a, 0xc1, 0x20, 0xb0, 0xe2, 0x3f, 0x6b, 0xdc, 0xab, 0x47, 0x8d, 0x97, 0xea, 0x3f,
            0x49, 0x20, 0x7a, 0xc1, 0x20, 0xb0, 0xe2, 0x3f,
        ],
        [
            0x17, 0x00, 0x00, 0x00, 0x15, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x09, 0x00, 0x00, 0x00, 0x6b, 0xdc, 0xab, 0x47, 0x8d, 0x97, 0xea, 0x3f, 0x49, 0x20,
            0x7a, 0xc1, 0x20, 0xb0, 0xe2, 0x3f, 0x6b, 0xdc, 0xab, 0x47, 0x8d, 0x97, 0xea, 0x3f,
            0x00, 0xac, 0x4c, 0x7f, 0xe0, 0x7a, 0x84, 0x3f,
        ],
    ];

    fn framed(type_code: u16, payload: &[u8]) -> Vec<u8> {
        let mut record = Vec::new();
        record.extend_from_slice(&type_code.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(payload);
        record
    }

    #[test]
    fn a_complex_string_lists_its_curves_and_the_constraints_between_them() {
        let record = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &STRING_OF_TWO);
        let decoded = decode_igcomplexstrings(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let string = &decoded[0];
        assert_eq!(
            string.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + record.len()
        );
        assert_eq!(
            (string.oid, string.parent_ref, string.sheet_layer_ref),
            (67, 6, 8)
        );
        assert_eq!((string.sub_type_word, string.index), (0, 12));
        assert_eq!(string.head, [0x01, 0x06, 0x00]);
        assert_eq!(string.range_scale, 1.0);
        assert_eq!(
            string.members,
            vec![
                SheetAssociationMember {
                    oid: 68,
                    range_box: [12723, 3641, 19248, 3680],
                    word: 0,
                },
                SheetAssociationMember {
                    oid: 69,
                    range_box: [12723, -19, 12761, 3680],
                    word: 0,
                },
                SheetAssociationMember {
                    oid: 70,
                    range_box: [12742, 3660, 12743, 3661],
                    word: IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER,
                },
            ]
        );
        assert_eq!(
            string.curve_members().map(|m| m.oid).collect::<Vec<_>>(),
            vec![68, 69]
        );
        assert_eq!(string.trailing, [0x00, 0x02, 0x00, 0x00]);
        // The box is the member's range in `range_scale / 32768` m, and this
        // string's scale is one: oid 68 is the line from x = 586.869 mm to
        // 388.889 mm at y = 111.717 mm.
        let [min_x, min_y, max_x, max_y] = string.range_box_m(&string.members[0]);
        assert!((min_x - 0.388_889).abs() < 0.001);
        assert!((max_x - 0.586_869).abs() < 0.001);
        assert!((min_y - 0.111_717).abs() < 0.001 && (max_y - 0.111_717).abs() < 0.001);
    }

    /// The double after the count is the unit of the range boxes, and it is
    /// not always one. `CPECCHB-SQ-A1.igr`'s outer frame states 2.0, and its
    /// members' boxes are their own extent — grown by half the pen, as on
    /// every template string — only when read in 2/32768 m. Read in
    /// 1/32768 m each is half the size, a quarter of a sheet away.
    #[test]
    fn the_double_after_the_count_is_the_scale_of_the_range_boxes() {
        let record = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &FRAME_AT_SCALE_TWO);
        let string = decode_igcomplexstring_at(&record, 0).expect("the frame decodes");
        assert_eq!((string.oid, string.index, string.members.len()), (21, 9, 8));
        assert_eq!(string.head, [0x01, 0x06, 0x00]);
        assert_eq!(string.range_scale, 2.0);
        assert_eq!(
            string.members[0],
            SheetAssociationMember {
                oid: 22,
                range_box: [407, 9565, 13617, 9571],
                word: 0,
            }
        );
        assert_eq!(string.curve_members().count(), 4);

        for (member, payload) in string.members.iter().zip(&FRAME_EDGES) {
            let line = decode_igline_at(&framed(PSM_TYPE_CODE_IGLINE2D, payload), 0)
                .expect("an edge of the frame decodes");
            assert_eq!(
                (line.oid, line.parent_ref, line.index),
                (member.oid, string.oid, string.index),
                "the member is the string's own line, with the string's pen"
            );
            let extent = [
                line.start.0.min(line.end.0),
                line.start.1.min(line.end.1),
                line.start.0.max(line.end.0),
                line.start.1.max(line.end.1),
            ];
            // In the record's own unit: the edge, grown by under 0.3 mm.
            let stated = string.range_box_m(member);
            for (side, (boxed, own)) in stated.iter().zip(&extent).enumerate() {
                assert!(
                    (boxed - own).abs() < 0.000_3,
                    "oid {} side {side}: the box reads {boxed} m, the edge {own} m",
                    member.oid
                );
            }
            // Outward on every side: a box, not a guess that happens to land.
            assert!(stated[0] <= extent[0] && stated[1] <= extent[1]);
            assert!(stated[2] >= extent[2] && stated[3] >= extent[3]);
            // In 1/32768 m the far corner is 0.2 m and more from the edge's.
            let unit = member.range_box_m(1.0);
            assert!(
                (unit[2] - extent[2]).abs() > 0.2 && (unit[3] - extent[3]).abs() > 0.2,
                "oid {}: read at scale one the box is {unit:?}, the edge {extent:?}",
                member.oid
            );
        }
        // The first edge, in millimetres.
        let top = string.range_box_m(&string.members[0]).map(|m| m * 1000.0);
        assert!((top[0] - 24.841).abs() < 0.001 && (top[2] - 831.116).abs() < 0.001);
        assert!((top[1] - 583.801).abs() < 0.001 && (top[3] - 584.167).abs() < 0.001);
    }

    #[test]
    fn a_group_lists_its_members() {
        let record = framed(PSM_TYPE_CODE_IGGROUP, &GROUP_OF_TWO);
        let decoded = decode_iggroups(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let group = &decoded[0];
        assert_eq!(
            (group.oid, group.parent_ref, group.sheet_layer_ref),
            (311, 6, 0)
        );
        assert_eq!((group.head, group.range_scale), (0, 1.0));
        assert_eq!(
            group.members,
            vec![
                SheetAssociationMember {
                    oid: 295,
                    range_box: [819, 327, 19136, 13435],
                    word: 0x000c,
                },
                SheetAssociationMember {
                    oid: 114,
                    range_box: [0, 0, 19464, 13762],
                    word: 0x0006,
                },
            ]
        );
        assert_eq!(group.trailing, [0x00, 0x02, 0x00, 0x00]);
        // `A01`'s embedded border is an A2 sheet, and at this group's scale
        // of one the outer rectangle's box is that sheet: 594 by 420 mm.
        let outer = group.range_box_m(&group.members[1]);
        assert_eq!((outer[0], outer[1]), (0.0, 0.0));
        assert!((outer[2] - 0.594).abs() < 0.000_1 && (outer[3] - 0.420).abs() < 0.000_1);
    }

    #[test]
    fn neither_reads_the_other_or_a_record_of_another_type() {
        let string = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &STRING_OF_TWO);
        let group = framed(PSM_TYPE_CODE_IGGROUP, &GROUP_OF_TWO);
        assert!(decode_iggroup_at(&string, 0).is_none());
        assert!(decode_igcomplexstring_at(&group, 0).is_none());
        let line = framed(PSM_TYPE_CODE_IGLINE2D, &STRING_OF_TWO);
        assert!(decode_igcomplexstring_at(&line, 0).is_none());
        assert!(decode_iggroup_at(&line, 0).is_none());
    }

    #[test]
    fn a_count_the_payload_cannot_hold_is_refused() {
        // Four members claimed where the payload has entries for three.
        let mut payload = STRING_OF_TWO.to_vec();
        payload[IGCOMPLEXSTRING2D_COUNT_AT] = 4;
        assert!(
            decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &payload), 0)
                .is_none()
        );
        // No member at all is not a string.
        let mut payload = STRING_OF_TWO.to_vec();
        payload[IGCOMPLEXSTRING2D_COUNT_AT] = 0;
        assert!(
            decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &payload), 0)
                .is_none()
        );
        let mut payload = GROUP_OF_TWO.to_vec();
        payload[IGGROUP_COUNT_AT] = 3;
        assert!(decode_iggroup_at(&framed(PSM_TYPE_CODE_IGGROUP, &payload), 0).is_none());
        let mut payload = GROUP_OF_TWO.to_vec();
        payload[IGGROUP_COUNT_AT] = 0;
        assert!(decode_iggroup_at(&framed(PSM_TYPE_CODE_IGGROUP, &payload), 0).is_none());
    }

    /// A string closes in one of two ways after its last member entry —
    /// four bytes on a sheet, four bytes and a `u32` a member in a symbol
    /// cache — and a record that closes in neither is no string.
    #[test]
    fn a_string_closes_as_a_sheets_or_as_a_caches() {
        let decode = |payload: &[u8]| {
            decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, payload), 0)
        };
        let on_a_sheet = decode(&STRING_OF_TWO).expect("a sheet's string decodes");
        assert_eq!(on_a_sheet.trailing, [0x00, 0x02, 0x00, 0x00]);

        let in_a_cache = decode(&STRING_IN_A_CACHE).expect("a cache's string decodes");
        assert_eq!(
            (
                in_a_cache.oid,
                in_a_cache.parent_ref,
                in_a_cache.sheet_layer_ref
            ),
            (33, 29, 32)
        );
        assert_eq!((in_a_cache.index, in_a_cache.range_scale), (10, 1.0));
        assert_eq!(
            in_a_cache
                .members
                .iter()
                .map(|member| (member.oid, member.range_box, member.word))
                .collect::<Vec<_>>(),
            [
                (34, [-76, -14, 0, 14], 0x0005),
                (35, [0, -15, 76, 14], 0x0005),
                (36, [0, 0, 0, 1], IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER),
            ]
        );
        let mut closing = vec![0x00, 0x03, 0x00, 0x00];
        closing.extend_from_slice(&[0; 12]);
        assert_eq!(in_a_cache.trailing, closing);

        // The three members of the sheet's string, under each closing in
        // turn.
        let entries = &STRING_OF_TWO[..IGCOMPLEXSTRING2D_MEMBERS_AT + 3 * ASSOCIATION_MEMBER_LEN];
        let closed_with = |closing: &[u8]| {
            let mut payload = entries.to_vec();
            payload.extend_from_slice(closing);
            decode(&payload)
        };
        assert!(closed_with(&[0, 2, 0, 0]).is_some());
        // What the `u32` a member states is not decoded, and not held to
        // the zero the corpora's are.
        assert!(closed_with(&[0, 3, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]).is_some());
        for (closing, what) in [
            (&[][..], "no closing"),
            (&[0, 2, 0][..], "a closing cut short"),
            (&[0, 2, 0, 0, 0][..], "a byte after a sheet's closing"),
            (
                &[0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..],
                "a sheet's closing with a word a member",
            ),
            (&[0, 3, 0, 0][..], "a cache's closing with no word"),
            (
                &[0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..],
                "a cache's closing a word short",
            ),
            (
                &[0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..],
                "a cache's closing a word long",
            ),
            (&[0, 4, 0, 0][..], "another second byte"),
            (&[1, 2, 0, 0][..], "another first byte"),
            (&[0, 2, 0, 1][..], "another last byte"),
            (&[0, 0, 0, 0][..], "four bytes of zero"),
        ] {
            assert!(
                closed_with(closing).is_none(),
                "{what} is taken for a string"
            );
        }
    }

    /// A count below the record's own leaves entries of the record behind
    /// it, and they close nothing: the string of three read as one of two
    /// has its third entry and its closing, 18 bytes, where a closing
    /// stands.
    #[test]
    fn a_count_that_leaves_entries_behind_is_refused() {
        for count in [1, 2] {
            let mut payload = STRING_OF_TWO.to_vec();
            payload[IGCOMPLEXSTRING2D_COUNT_AT] = count;
            assert!(
                decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &payload), 0)
                    .is_none(),
                "a count of {count} on a record of three"
            );
        }
    }

    /// What four length changes led a walk to on one sheet of the plant
    /// backup (`G井场进站生产收球筒`'s `/Sheet6`): 28 bytes into a
    /// `DependencyObject`, a type word `0x0021` and a length of 65 536, with
    /// a `u32` of 256 where a string keeps its count and zero, or a
    /// denormal, where it keeps its scale. The payload has room for 256
    /// entries and the scale is finite, which is all a string was held to
    /// until 2026-11-12. What follows the entries is 61 919 bytes of the
    /// stream, and a string closes in four, or in four and a word a member.
    #[test]
    fn a_run_of_bytes_with_room_for_its_count_is_no_string() {
        for scale in [0.0_f64, 5.4e-312] {
            let mut payload: Vec<u8> = (0..65_536_u32)
                .map(|i| (i.wrapping_mul(31) >> 3) as u8)
                .collect();
            payload[12..14].copy_from_slice(&3_u16.to_le_bytes());
            payload[IGCOMPLEXSTRING2D_COUNT_AT..IGCOMPLEXSTRING2D_COUNT_AT + 4]
                .copy_from_slice(&256_u32.to_le_bytes());
            payload[IGCOMPLEXSTRING2D_COUNT_AT + 4..IGCOMPLEXSTRING2D_COUNT_AT + 12]
                .copy_from_slice(&scale.to_le_bytes());
            let record = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &payload);
            assert!(
                decode_igcomplexstring_at(&record, 0).is_none(),
                "scale {scale:e}: 61 919 bytes after the entries"
            );
            assert!(decode_igcomplexstrings(&chain_stream(&record)).is_empty());

            // The closing is the whole of what refuses it: the same bytes,
            // cut off after their 256 entries and closed as a sheet's
            // string closes, are a string by every rule the decoder has.
            let mut closed =
                payload[..IGCOMPLEXSTRING2D_MEMBERS_AT + 256 * ASSOCIATION_MEMBER_LEN].to_vec();
            closed.extend_from_slice(&[0x00, 0x02, 0x00, 0x00]);
            let string =
                decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &closed), 0)
                    .expect("closed as a string closes");
            assert_eq!(string.members.len(), 256);
        }
    }

    #[test]
    fn a_non_finite_range_scale_is_refused() {
        let mut payload = STRING_OF_TWO.to_vec();
        payload[IGCOMPLEXSTRING2D_COUNT_AT + 4..IGCOMPLEXSTRING2D_COUNT_AT + 12]
            .copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(
            decode_igcomplexstring_at(&framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &payload), 0)
                .is_none()
        );
        let mut payload = GROUP_OF_TWO.to_vec();
        payload[IGGROUP_COUNT_AT + 4..IGGROUP_COUNT_AT + 12]
            .copy_from_slice(&f64::INFINITY.to_le_bytes());
        assert!(decode_iggroup_at(&framed(PSM_TYPE_CODE_IGGROUP, &payload), 0).is_none());
    }

    #[test]
    fn a_record_the_native_reader_skips_is_not_decoded() {
        let string = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D | 0x8000, &STRING_OF_TWO);
        assert!(decode_igcomplexstring_at(&string, 0).is_none());
        let group = framed(PSM_TYPE_CODE_IGGROUP | 0x8000, &GROUP_OF_TWO);
        assert!(decode_iggroup_at(&group, 0).is_none());
    }

    #[test]
    fn both_survive_truncation_and_noise() {
        let string = framed(PSM_TYPE_CODE_IGCOMPLEXSTRING2D, &STRING_OF_TWO);
        let group = framed(PSM_TYPE_CODE_IGGROUP, &GROUP_OF_TWO);
        for cut in 0..string.len() {
            assert!(decode_igcomplexstring_at(&string[..cut], 0).is_none());
            assert!(decode_igcomplexstrings(&chain_stream(&string[..cut])).is_empty());
        }
        for cut in 0..group.len() {
            assert!(decode_iggroup_at(&group[..cut], 0).is_none());
            assert!(decode_iggroups(&chain_stream(&group[..cut])).is_empty());
        }
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_igcomplexstrings(&noise);
        let _ = decode_iggroups(&noise);
        for at in 0..noise.len() {
            let _ = decode_igcomplexstring_at(&noise, at);
            let _ = decode_iggroup_at(&noise, at);
        }
        assert!(decode_igcomplexstrings(&[]).is_empty());
        assert!(decode_iggroups(&[]).is_empty());
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

    /// `/JSite121` oid 489's doubles, in payload order: sweep start, sweep
    /// end, centre, major semi-axis, ratio.
    const HEAD_DOUBLES: [f64; 7] = [
        std::f64::consts::TAU,
        std::f64::consts::PI,
        -0.010_296_148_41,
        0.0889,
        4.768_329_956e-18,
        -0.025_958_436_58,
        0.5,
    ];

    fn elliptical_record(doubles: &[f64]) -> Vec<u8> {
        curve_record(PSM_TYPE_CODE_IGELLIPTICALARC2D, 533, doubles, 1)
    }

    /// `record` with its payload cut or zero-padded to `len` bytes and its
    /// `bytes_to_follow` saying so, so the chain still walks.
    fn with_payload_len(mut record: Vec<u8>, len: usize) -> Vec<u8> {
        record.resize(PSM_ENVELOPE_LEN + len, 0);
        record[2..6].copy_from_slice(&(len as u32).to_le_bytes());
        record
    }

    #[test]
    fn an_elliptical_arc_reads_its_angles_first_then_centre_axis_and_ratio() {
        let stream = chain(&[elliptical_record(&HEAD_DOUBLES)]);
        let decoded = decode_igellipticalarcs(&stream);
        assert_eq!(decoded.len(), 1);
        let arc = &decoded[0];
        assert_eq!(
            arc.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + 6 + 75
        );
        assert_eq!(arc.byte_range.end, stream.len());
        assert_eq!(arc.type_flags, 0);
        assert_eq!(arc.oid, 4321);
        assert_eq!(arc.parent_ref, 77);
        assert_eq!(arc.sheet_layer_ref, 533);
        assert_eq!(arc.sub_type_word, 0x0010);
        assert_eq!(arc.index, 9);
        assert_eq!(arc.sweep_start, std::f64::consts::TAU);
        assert_eq!(arc.sweep_end, std::f64::consts::PI);
        assert_eq!(arc.center, (-0.010_296_148_41, 0.0889));
        assert_eq!(arc.major_axis, (4.768_329_956e-18, -0.025_958_436_58));
        assert_eq!(arc.ratio, 0.5);
        assert_eq!(arc.flag, 1);
        assert_eq!(
            decode_igellipticalarc_at(&stream, SHEET_STREAM_HEADER_LEN).as_ref(),
            Some(arc)
        );
        // Nor does either neighbouring family read it.
        assert!(decode_igarcs(&stream).is_empty());
        assert!(decode_igcircles(&stream).is_empty());
        // A ratio of exactly 1 is a circular arc, still admitted.
        let mut round = HEAD_DOUBLES;
        round[6] = 1.0;
        assert_eq!(
            decode_igellipticalarcs(&chain(&[elliptical_record(&round)])).len(),
            1
        );
    }

    #[test]
    fn an_elliptical_arc_off_the_gate_is_refused() {
        let with = |slot: usize, value: f64| {
            let mut doubles = HEAD_DOUBLES;
            doubles[slot] = value;
            elliptical_record(&doubles)
        };
        let mut skipped = elliptical_record(&HEAD_DOUBLES);
        skipped[1] |= 0x80; // type word bit 0x8000: the native reader skips it
        let refused = [
            (
                "btf 74",
                with_payload_len(elliptical_record(&HEAD_DOUBLES), 74),
            ),
            (
                "btf 76",
                with_payload_len(elliptical_record(&HEAD_DOUBLES), 76),
            ),
            ("ratio 0", with(6, 0.0)),
            ("ratio -1", with(6, -1.0)),
            ("zero major axis", {
                let mut doubles = HEAD_DOUBLES;
                doubles[4] = 0.0;
                doubles[5] = 0.0;
                elliptical_record(&doubles)
            }),
            ("NaN angle", with(0, f64::NAN)),
            ("NaN centre", with(2, f64::NAN)),
            ("skip bit", skipped),
        ];
        for (what, record) in refused {
            let stream = chain(&[record]);
            assert!(
                !sheet_record_starts(&stream).is_empty(),
                "{what}: the chain itself must still walk"
            );
            assert!(decode_igellipticalarcs(&stream).is_empty(), "{what}");
            assert!(
                decode_igellipticalarc_at(&stream, SHEET_STREAM_HEADER_LEN).is_none(),
                "{what}"
            );
        }
    }

    /// The nozzle-label symbols' `0x007E` records write `ratio` up to
    /// `3.0` (`Nozzle NPD.sym` `/Sheet43 +138`): accepted, not refused,
    /// the same relaxation [`decode_igellipses`] needed.
    #[test]
    fn an_elliptical_arc_with_ratio_past_one_is_accepted() {
        let mut doubles = HEAD_DOUBLES;
        doubles[6] = 2.384_470_402_521_848;
        assert_eq!(
            decode_igellipticalarcs(&chain(&[elliptical_record(&doubles)])).len(),
            1
        );
    }

    /// `Air Cooler Type 1.sym`'s left fan oval (`Sheet6` oid 726): centre,
    /// major semi-axis, ratio.
    const FAN_OVAL: [f64; 5] = [0.066_225, 0.013_97, 0.002_354_6, 1.44e-19, 0.448_13];

    #[test]
    fn an_ellipse_reads_its_centre_axis_and_ratio() {
        let stream = chain(&[curve_record(PSM_TYPE_CODE_IGELLIPSE2D, 5, &FAN_OVAL, 0)]);
        let decoded = decode_igellipses(&stream);
        assert_eq!(decoded.len(), 1);
        let ellipse = &decoded[0];
        assert_eq!(
            ellipse.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + 6 + IGELLIPSE2D_PAYLOAD_LEN
        );
        assert_eq!(ellipse.oid, 4321);
        assert_eq!(ellipse.parent_ref, 77);
        assert_eq!(ellipse.sheet_layer_ref, 5);
        assert_eq!(ellipse.sub_type_word, 0x0010);
        assert_eq!(ellipse.index, 9);
        assert_eq!(ellipse.center, (FAN_OVAL[0], FAN_OVAL[1]));
        assert_eq!(ellipse.major_axis, (FAN_OVAL[2], FAN_OVAL[3]));
        assert_eq!(ellipse.ratio, FAN_OVAL[4]);
        assert_eq!(ellipse.flag, 0);
        assert_eq!(
            decode_igellipse_at(&stream, SHEET_STREAM_HEADER_LEN).as_ref(),
            Some(ellipse)
        );
        // Same payload length as an arc: the type code is what separates them.
        assert!(decode_igarcs(&stream).is_empty());
        let arc = chain(&[curve_record(PSM_TYPE_CODE_IGARC2D, 5, &FAN_OVAL, 0)]);
        assert!(decode_igellipses(&arc).is_empty());
        assert_eq!(decode_igarcs(&arc).len(), 1);
    }

    #[test]
    fn an_ellipse_off_the_gate_is_refused() {
        let with = |slot: usize, value: f64| {
            let mut doubles = FAN_OVAL;
            doubles[slot] = value;
            curve_record(PSM_TYPE_CODE_IGELLIPSE2D, 5, &doubles, 0)
        };
        let base = || curve_record(PSM_TYPE_CODE_IGELLIPSE2D, 5, &FAN_OVAL, 0);
        let mut skipped = base();
        skipped[1] |= 0x80;
        let refused = [
            ("btf 58", with_payload_len(base(), 58)),
            ("btf 60", with_payload_len(base(), 60)),
            ("ratio 0", with(4, 0.0)),
            ("ratio -1", with(4, -1.0)),
            ("zero major axis", {
                let mut doubles = FAN_OVAL;
                doubles[2] = 0.0;
                doubles[3] = 0.0;
                curve_record(PSM_TYPE_CODE_IGELLIPSE2D, 5, &doubles, 0)
            }),
            ("NaN centre", with(0, f64::NAN)),
            ("infinite axis", with(2, f64::INFINITY)),
            ("skip bit", skipped),
        ];
        for (what, record) in refused {
            let stream = chain(&[record]);
            assert!(
                !sheet_record_starts(&stream).is_empty(),
                "{what}: the chain itself must still walk"
            );
            assert!(decode_igellipses(&stream).is_empty(), "{what}");
            assert!(
                decode_igellipse_at(&stream, SHEET_STREAM_HEADER_LEN).is_none(),
                "{what}"
            );
        }
    }

    /// `Nozzle NPD.sym` / `Nozzle Rating.sym` write `ratio` up to `3.0`:
    /// the field at `+34`/`+42` is the shorter semi-axis there, not the
    /// longer one. The record is still a legitimate ellipse, not a
    /// corrupt one, so a ratio past `1` is accepted rather than refused.
    #[test]
    fn an_ellipse_with_ratio_past_one_is_accepted() {
        let stream = chain(&[curve_record(
            PSM_TYPE_CODE_IGELLIPSE2D,
            5,
            &[0.101_346, 0.147_066, 0.003_048, 0.0, 2.25],
            0,
        )]);
        let decoded = decode_igellipses(&stream);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].ratio, 2.25);
    }

    #[test]
    fn an_ellipse_survives_truncation_at_every_length() {
        let circle = curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &[0.1, 0.2, 0.05], 1);
        let at = SHEET_STREAM_HEADER_LEN + circle.len();
        let stream = chain(&[
            circle,
            curve_record(PSM_TYPE_CODE_IGELLIPSE2D, 5, &FAN_OVAL, 0),
        ]);
        let end = at + PSM_ENVELOPE_LEN + IGELLIPSE2D_PAYLOAD_LEN;
        assert_eq!(decode_igellipses(&stream).len(), 1);
        for cut in 0..=stream.len() {
            let _ = decode_igellipses(&stream[..cut]);
            let _ = decode_igellipse_at(&stream[..cut], cut.saturating_sub(1));
            assert_eq!(
                decode_igellipse_at(&stream[..cut], at).is_some(),
                cut >= end,
                "cut {cut}"
            );
        }
    }

    #[test]
    fn an_elliptical_arc_survives_truncation_at_every_length() {
        let circle = curve_record(PSM_TYPE_CODE_IGCIRCLE2D, 8, &[0.1, 0.2, 0.05], 1);
        let at = SHEET_STREAM_HEADER_LEN + circle.len();
        let stream = chain(&[
            circle,
            elliptical_record(&HEAD_DOUBLES),
            curve_record(PSM_TYPE_CODE_IGARC2D, 8, &[0.1, 0.2, 0.05, 0.0, 1.0], 1),
        ]);
        let end = at + PSM_ENVELOPE_LEN + IGELLIPTICALARC2D_PAYLOAD_LEN;
        assert_eq!(decode_igellipticalarcs(&stream).len(), 1);
        for cut in 0..=stream.len() {
            let _ = decode_igellipticalarcs(&stream[..cut]);
            let _ = decode_igellipticalarc_at(&stream[..cut], cut.saturating_sub(1));
            let _ = decode_igellipticalarc_at(&stream[..cut], cut);
            assert_eq!(
                decode_igellipticalarc_at(&stream[..cut], at).is_some(),
                cut >= end,
                "cut {cut}"
            );
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

/// Byte length of a kind-1 block when `IGDIMENSION_WIDE_BLOCK_FLAG` is
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
/// 5. the block's leading `u32` has `IGDIMENSION_WIDE_BLOCK_FLAG`
///    clear, i.e. the kind-1 block really is 48 bytes;
/// 6. the dimension value is finite and inside the coordinate domain.
///
/// A record that fails any of them is *refused*, not guessed at: the
/// decoder claims no bytes there, which is what lets
/// [`crate::parsers::undecoded_census::refused_record_census`] count it
/// separately from an undecoded type code once this family is registered.
///
/// Read at each record the stream's chain starts. Panic-free and
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
        let header = parse_live_psm_header(data, offset)?;
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
    if !value_m.is_finite() || value_m.abs() > COORDINATE_DOMAIN_LIMIT {
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

    /// A linked `igSmartFrame2d` whose payload carries the content placement
    /// RAD's 2D transform stores at `+164`: the A2 `D06` / `轻烃分离器I` values.
    #[test]
    fn a_smart_frame_reads_where_it_places_its_content() {
        let mut payload = vec![0u8; 238];
        payload[0..4].copy_from_slice(&22_u32.to_le_bytes());
        payload[14..18].copy_from_slice(&0x8000_u32.to_le_bytes());
        payload[26..30].copy_from_slice(&0x40_u32.to_le_bytes());
        let put = |payload: &mut Vec<u8>, at: usize, value: f64| {
            payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut payload, 36, -0.000_051_234);
        put(&mut payload, 44, 0.000_355_000);
        put(&mut payload, 76, 0.594_327_890);
        put(&mut payload, 84, 0.420_565_694);
        put(&mut payload, 148, 0.707_632_438);
        for (index, value) in [1.0, 0.0, 0.0, 1.0, 0.000_388_407, 0.000_565_648, 1.0]
            .into_iter()
            .enumerate()
        {
            put(&mut payload, 164 + 8 * index, value);
        }
        let mut record = Vec::new();
        record.extend_from_slice(&PSM_TYPE_CODE_IGSMARTFRAME.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);

        let frame = decode_smartframe_at(&record, 0).expect("frame");
        assert_eq!(frame.state, SheetSmartFrame2dState::Linked);
        let transform = frame.content_transform.expect("transform");
        assert!(transform.linear_is_identity());
        assert_eq!(transform.translation_m, [0.000_388_407, 0.000_565_648]);
        assert_eq!(transform.scale, 1.0);
        assert_eq!(frame.range_origin_m, Some([-0.000_051_234, 0.000_355_000]));

        // A record cut off before the transform still decodes its page.
        let short = IGSMARTFRAME_MIN_PAYLOAD;
        let mut cut = record[..PSM_ENVELOPE_LEN + short].to_vec();
        cut[2..6].copy_from_slice(&(short as u32).to_le_bytes());
        let frame = decode_smartframe_at(&cut, 0).expect("short frame");
        assert!(frame.content_transform.is_none());
        assert!(frame.page_extent_m().is_some());
    }

    /// One whole `igSmartFrame2d` record: an A1 page, linked.
    fn smart_frame_record(oid: u32) -> Vec<u8> {
        let mut payload = vec![0u8; IGSMARTFRAME_MIN_PAYLOAD];
        payload[0..4].copy_from_slice(&oid.to_le_bytes());
        payload[14..18].copy_from_slice(&0x8000_u32.to_le_bytes());
        payload[26..30].copy_from_slice(&0x40_u32.to_le_bytes());
        for (at, value) in [(76, 0.841_f64), (84, 0.594), (148, 0.706_302)] {
            payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        let mut record = Vec::new();
        record.extend_from_slice(&PSM_TYPE_CODE_IGSMARTFRAME.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);
        record
    }

    /// A frame states its extent twice over: width and height at `+76` and
    /// `+84`, and height over width at `+148`. The three agree on every one
    /// of the 178 frames the corpora's chains hold, to 5e-16; a record whose
    /// third double is not the quotient of the other two is not a frame.
    #[test]
    fn a_frame_states_the_ratio_of_its_own_extent() {
        let ratio_at = PSM_ENVELOPE_LEN + IGSMARTFRAME_ASPECT_AT;
        let with_ratio = |ratio: f64| {
            let mut record = smart_frame_record(22);
            record[ratio_at..ratio_at + 8].copy_from_slice(&ratio.to_le_bytes());
            record
        };
        // The record as built: 0.841 m by 0.594 m, the ratio typed to six
        // places.
        assert!(decode_smartframe_at(&smart_frame_record(22), 0).is_some());
        assert!(decode_smartframe_at(&with_ratio(0.594 / 0.841), 0).is_some());
        for wrong in [0.5, 0.706, 0.7064, 1.0 / 0.706_302, 0.0, -0.706_302, 1e300] {
            assert!(
                decode_smartframe_at(&with_ratio(wrong), 0).is_none(),
                "a frame of 0.841 m by 0.594 m that states {wrong}"
            );
        }
    }

    /// What a walk that has left the record boundaries took for a frame.
    /// A length 32 bytes too long sends the walk into the text of a label
    /// (`A井场`'s `/Sheet6`, the record at `+12725` stating 88 bytes for
    /// 56): the character `=` is `3D 00`, the type word of a frame, and
    /// the four bytes after it state a payload of 262 144. The doubles at
    /// the frame's offsets are whatever the next records hold there — here
    /// three denormals — and until the ratio was held to the extent the
    /// decoder took them for an empty frame. Ten of the 49 058 records such
    /// walks read off the corpora's chains decoded so; none does now
    /// (`examples/probe_what_passes_for_a_page_frame_off_the_chain.rs`).
    #[test]
    fn what_a_walk_off_the_chain_took_for_a_frame_is_not_one() {
        // Width, height and "ratio" of two of the ten: the one named above,
        // and the one an absolute tolerance would let through — a height
        // and a ratio both next to nothing, over a width of 0.6.
        for (flags, width, height, ratio) in [
            (
                0x4C9D_3FD7_u32,
                2.781_257_441_683_414e-309_f64,
                5.562_684_646_279_97e-309_f64,
                4.778_310_705_768_978e-299_f64,
            ),
            (
                0xEBDE_3FD6,
                6.121_645_546_997_917e-1,
                3.861_592_247_633_563_3e-212,
                6.717_881_233_056_225e-138,
            ),
        ] {
            let mut payload = vec![0u8; 262_144];
            payload[14..18].copy_from_slice(&flags.to_le_bytes());
            for (at, value) in [
                (IGSMARTFRAME_WIDTH_AT, width),
                (IGSMARTFRAME_HEIGHT_AT, height),
                (IGSMARTFRAME_ASPECT_AT, ratio),
            ] {
                payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
            }
            let mut record = Vec::new();
            record.extend_from_slice(&PSM_TYPE_CODE_IGSMARTFRAME.to_le_bytes());
            record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            record.extend_from_slice(&payload);
            assert!(
                decode_smartframe_at(&record, 0).is_none(),
                "{width:e} by {height:e} stating {ratio:e}"
            );
        }
    }

    /// One whole `igSmartFrame2d` record of `payload_len` bytes that states
    /// `width` by `height` and `ratio`, and nothing else.
    fn smart_frame_stating(payload_len: usize, width: f64, height: f64, ratio: f64) -> Vec<u8> {
        let mut payload = vec![0u8; payload_len];
        for (at, value) in [
            (IGSMARTFRAME_WIDTH_AT, width),
            (IGSMARTFRAME_HEIGHT_AT, height),
            (IGSMARTFRAME_ASPECT_AT, ratio),
        ] {
            payload[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        let mut record = Vec::new();
        record.extend_from_slice(&PSM_TYPE_CODE_IGSMARTFRAME.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);
        record
    }

    /// The six "frames" the ratio rule of 2026-11-11 still took. That rule
    /// held the ratio to the height over the width only where the width was
    /// above zero, and wrote the comparison so that a quotient of infinity
    /// passed it: nothing lies further from infinity than a millionth of
    /// infinity. With a record's length changed by every amount from −64
    /// to +64, walks read 1 371 448 records off the corpora's chains; these
    /// are the extents of the six that decoded, at five places
    /// (`examples/probe_what_passes_for_a_page_frame_off_the_chain.rs`).
    /// The first is `I井场`'s `/Sheet6` at `+10702`, which two changes
    /// reach: a width that is a count read as a double, under a height of
    /// 3.7e62 m. The other four, on two sheets of the plant backup, are
    /// zeros where a width and a height would be.
    #[test]
    fn what_a_walk_off_the_chain_took_for_a_frame_of_no_width_is_not_one() {
        let taken: Vec<String> = [
            (
                131_072_usize,
                3.819_594_119_6e-313_f64,
                3.728_096_263_309_427_4e62_f64,
                2.263_743_152_185_486_6e164_f64,
            ),
            (65_536, 0.0, 0.0, 2.122_513_856_3e-314),
            (65_536, 0.0, 0.0, 2.434_904_188_060_913_6e144),
            (65_536, 0.0, 0.0, 1.636_760_400_304_213_6e-251),
            (65_536, 0.0, 0.0, 2.151_964_644_286_228_7e-281),
        ]
        .into_iter()
        .filter(|&(payload_len, width, height, ratio)| {
            decode_smartframe_at(&smart_frame_stating(payload_len, width, height, ratio), 0)
                .is_some()
        })
        .map(|(_, width, height, ratio)| format!("{width:e} by {height:e} stating {ratio:e}"))
        .collect();
        assert!(taken.is_empty(), "taken for a frame: {taken:#?}");
    }

    /// The ratio can be held to the extent only where the extent has one:
    /// a width and a height that are lengths — above zero, and not the
    /// denormals a count or a flag word reads as — whose quotient is a
    /// number of its own. Each case here is an extent that states no ratio
    /// to compare, and each passed until 2026-11-12: by skipping the
    /// comparison (no width), or by passing it for want of anything to
    /// differ from (a quotient of zero against a stated zero, of infinity
    /// against anything, of two counts against one).
    #[test]
    fn a_frame_states_an_extent_its_ratio_can_be_held_to() {
        // The smallest frame of the corpora: the micrometre square a nested
        // site's placeholder states. Its sides are lengths, and it decodes.
        let (width, height) = (1.000_000_000_000_000_2e-6_f64, 9.999_999_999_999_997e-7_f64);
        assert!(
            decode_smartframe_at(
                &smart_frame_stating(IGSMARTFRAME_MIN_PAYLOAD, width, height, height / width),
                0
            )
            .is_some(),
            "a micrometre square"
        );

        let denormal = f64::from_bits(18 << 32);
        assert!(denormal > 0.0 && !denormal.is_normal());
        let taken: Vec<String> = [
            (0.0, 0.594, 0.706_302, "no width"),
            (0.0, 0.0, 0.0, "no width and no height"),
            (-0.0, 0.594, 0.706_302, "a width of minus zero"),
            (0.841, 0.0, 0.0, "no height, stating a ratio of zero"),
            (
                denormal,
                0.594,
                1e300,
                "a count for a width: the quotient is infinite",
            ),
            (
                1.0,
                denormal,
                denormal,
                "a count for a height, and the same count for a ratio",
            ),
            (
                denormal,
                denormal,
                1.0,
                "the same count for a width and a height",
            ),
            (
                1e200,
                1e-200,
                0.0,
                "a quotient too small to be a number, stating zero",
            ),
            (
                1e-200,
                1e200,
                f64::MAX,
                "a quotient too large to be a number",
            ),
        ]
        .into_iter()
        .filter(|&(width, height, ratio, _)| {
            decode_smartframe_at(
                &smart_frame_stating(IGSMARTFRAME_MIN_PAYLOAD, width, height, ratio),
                0,
            )
            .is_some()
        })
        .map(|(width, height, ratio, what)| {
            format!("{what}: {width:e} by {height:e} stating {ratio:e}")
        })
        .collect();
        assert!(taken.is_empty(), "taken for a frame: {taken:#?}");
    }

    /// A live frame record the decoder refuses says which part of the rule
    /// it fails, with the record's own numbers, and what state its flag
    /// words put it in: what a page laid out without its frame is told.
    /// The same read gives the decoder its frame, so the two answers are
    /// one or the other of every live frame record and never both.
    #[test]
    fn a_refused_frame_says_which_part_of_the_rule_it_fails() {
        // The record as built is a frame: nothing to say.
        let frame = smart_frame_record(22);
        assert!(decode_smartframe_at(&frame, 0).is_some());
        assert_eq!(smartframe_refusal_at(&frame, 0), None);

        let with = |width: f64, height: f64, ratio: f64| {
            let mut record = smart_frame_record(22);
            for (at, value) in [
                (IGSMARTFRAME_WIDTH_AT, width),
                (IGSMARTFRAME_HEIGHT_AT, height),
                (IGSMARTFRAME_ASPECT_AT, ratio),
            ] {
                let at = PSM_ENVELOPE_LEN + at;
                record[at..at + 8].copy_from_slice(&value.to_le_bytes());
            }
            record
        };
        for (record, why, said) in [
            (
                with(0.0, 0.594, 0.706_302),
                SmartFrameRefusal::NotLengths {
                    width_m: 0.0,
                    height_m: 0.594,
                },
                "it states a width of 0 m and a height of 0.594 m, and a frame's sides are \
                 lengths above zero",
            ),
            (
                with(1e-200, 1e200, 1.0),
                SmartFrameRefusal::NoQuotient {
                    width_m: 1e-200,
                    height_m: 1e200,
                },
                "its height over its width, 1e200 m over 1e-200 m, is no number a ratio can be \
                 held to",
            ),
            (
                with(0.841, 0.594, 0.5),
                SmartFrameRefusal::RatioNotTheExtents {
                    width_m: 0.841,
                    height_m: 0.594,
                    aspect_ratio: 0.5,
                },
                "it states a ratio of 0.500000000 where its height over its width, 0.594000 m \
                 over 0.841000 m, is 0.706302021",
            ),
        ] {
            assert!(decode_smartframe_at(&record, 0).is_none(), "{said}");
            let refused = smartframe_refusal_at(&record, 0).expect(said);
            assert_eq!(refused.why, why);
            assert_eq!(refused.why.describe(), said);
            assert_eq!(
                (refused.byte_range.clone(), refused.oid, refused.state),
                (
                    0..record.len(),
                    Some(22),
                    Some(SheetSmartFrame2dState::Linked)
                ),
                "{said}"
            );
        }

        // A record cut short of the ratio: refused for its length, its oid
        // and its flag words still read.
        let short = IGSMARTFRAME_MIN_PAYLOAD - 1;
        let mut cut = frame[..PSM_ENVELOPE_LEN + short].to_vec();
        cut[2..6].copy_from_slice(&(short as u32).to_le_bytes());
        let refused = smartframe_refusal_at(&cut, 0).expect("a short record");
        assert_eq!(
            refused.why,
            SmartFrameRefusal::ShortPayload { payload_len: short }
        );
        assert_eq!(
            refused.why.describe(),
            "its payload is 155 byte(s), fewer than the 156 that reach the ratio a frame states"
        );
        assert_eq!(
            (refused.oid, refused.state),
            (Some(22), Some(SheetSmartFrame2dState::Linked))
        );

        // What is no live frame record at all gets neither answer: another
        // type code, a superseded copy, a payload the stream does not hold.
        let mut other = frame.clone();
        other[0] = 0x18;
        let mut superseded = frame.clone();
        superseded[1] |= 0x80;
        for (record, what) in [
            (other, "another type code"),
            (superseded, "a superseded copy"),
            (frame[..frame.len() - 1].to_vec(), "a record cut off"),
        ] {
            assert!(decode_smartframe_at(&record, 0).is_none(), "{what}");
            assert_eq!(smartframe_refusal_at(&record, 0), None, "{what}");
        }

        // On a chain: the refused record of a stream of two frames, and
        // nothing for the one that decodes.
        let mut stream = vec![0u8; SHEET_STREAM_HEADER_LEN];
        stream[..4].copy_from_slice(&SHEET_STREAM_MAGIC.to_le_bytes());
        stream[4..8].copy_from_slice(&2u32.to_le_bytes());
        stream.extend_from_slice(&frame);
        let second = stream.len();
        stream.extend_from_slice(&with(0.0, 0.594, 0.706_302));
        assert_eq!(decode_smartframes(&stream).len(), 1);
        let refused = refused_smartframes(&stream);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].byte_range, second..stream.len());
    }

    /// A chain is short when records are left unread: the walk stopped, or
    /// it ended with fewer live records than the header states. A wrong
    /// magic and a header that counts too few are faults that lose nothing.
    #[test]
    fn a_chain_is_short_when_records_are_left_unread() {
        let frame = smart_frame_record(22);
        let stream = |stated: u32, records: usize| {
            let mut stream = vec![0u8; SHEET_STREAM_HEADER_LEN];
            stream[..4].copy_from_slice(&SHEET_STREAM_MAGIC.to_le_bytes());
            stream[4..8].copy_from_slice(&stated.to_le_bytes());
            for _ in 0..records {
                stream.extend_from_slice(&frame);
            }
            stream
        };
        let sound = stream(2, 2);
        let chain = SheetChain::walk(&sound);
        assert!(chain.is_sound() && !chain.is_short());

        // Stopped: a length of zero, stray bytes after the last record, a
        // stream shorter than its header.
        let mut zero = sound.clone();
        let second = SHEET_STREAM_HEADER_LEN + frame.len();
        zero[second + 2..second + 6].copy_from_slice(&0u32.to_le_bytes());
        let mut stray = sound.clone();
        stray.extend_from_slice(&[0xEE; 4]);
        for (stopped, what) in [
            (zero.as_slice(), "a length of zero"),
            (stray.as_slice(), "stray bytes"),
            (&sound[..5], "five bytes"),
        ] {
            let chain = SheetChain::walk(stopped);
            assert!(chain.stop.is_some() && chain.is_short(), "{what}");
        }

        // Not stopped, and short all the same: cut off at a record
        // boundary; a header that counts one more than the chain holds.
        for (short, what) in [
            (sound[..second].to_vec(), "cut off before the second record"),
            (stream(3, 2), "a count one too many"),
        ] {
            let chain = SheetChain::walk(&short);
            assert!(
                chain.stop.is_none() && !chain.is_sound() && chain.is_short(),
                "{what}"
            );
        }

        // Faults that leave no record unread.
        let mut magic = sound.clone();
        magic[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        for (whole, what) in [
            (magic, "a wrong magic"),
            (stream(1, 2), "a count one too few"),
        ] {
            let chain = SheetChain::walk(&whole);
            assert!(!chain.is_sound() && !chain.is_short(), "{what}");
        }

        // An empty stream has nothing to be short of.
        assert!(!SheetChain::walk(&[]).is_short());
    }

    /// A family decodes the records its stream's chain starts, and nothing
    /// else. The bytes of a whole, valid frame carried *inside* another
    /// record's payload are that record's bytes: the native reader walks the
    /// chain and never reads them as a record. A scan that slides a byte at a
    /// time does — which is how the plant backup came to account for 61
    /// frames where its 53 chains hold 51, the other ten sitting inside text
    /// boxes, symbol placements and dependency objects wherever a field's
    /// value was 61 (`0x3D`). See
    /// `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
    /// §3.3.
    #[test]
    fn a_frame_shaped_run_inside_another_record_is_not_a_frame() {
        let inner = smart_frame_record(22);
        let on_chain = smart_frame_record(23);
        let mut stream = vec![0u8; SHEET_STREAM_HEADER_LEN];
        // A chain record of another family whose payload is four bytes and
        // then a frame's bytes, envelope and all.
        let host_at = stream.len();
        stream.extend_from_slice(&PSM_TYPE_CODE_IGTEXTBOX.to_le_bytes());
        stream.extend_from_slice(&((4 + inner.len()) as u32).to_le_bytes());
        stream.extend_from_slice(&[0xAA; 4]);
        let inner_at = stream.len();
        stream.extend_from_slice(&inner);
        let frame_at = stream.len();
        stream.extend_from_slice(&on_chain);

        assert_eq!(sheet_record_starts(&stream), vec![host_at, frame_at]);
        // The run inside the host is a frame's shape, byte for byte, so
        // nothing about the payload can refuse it: only the chain can.
        assert!(decode_smartframe_at(&stream, inner_at).is_some());

        let frames = decode_smartframes(&stream);
        assert_eq!(
            frames.iter().map(|frame| frame.oid).collect::<Vec<_>>(),
            vec![23],
            "only the record the chain starts is a frame"
        );
        assert_eq!(frames[0].byte_range, frame_at..stream.len());

        // A stream whose chain stops short is read as far as the chain
        // reads, and still never by a scan. One stray byte after the last
        // record leaves both records on the chain, and the run inside the
        // host as much the host's as before; the walk says where it stopped
        // and why, and the all-or-nothing reading says the stream is not
        // whole.
        let mut broken = stream.clone();
        broken.push(0);
        assert!(sheet_record_starts(&broken).is_empty());
        let chain = SheetChain::walk(&broken);
        assert_eq!(chain.starts, vec![host_at, frame_at]);
        assert_eq!((chain.end, chain.unread_bytes()), (stream.len(), 1));
        assert_eq!(chain.stop, Some(SheetChainStop::ShortEnvelope));
        assert_eq!(
            decode_smartframes(&broken)
                .iter()
                .map(|frame| frame.oid)
                .collect::<Vec<_>>(),
            vec![23],
            "the record the chain starts, and not the run inside the host"
        );
    }

    /// Three whole records behind the header a file writes for them: two
    /// the native reader reads and, between them, one it skips.
    fn three_record_stream() -> (Vec<u8>, [usize; 3]) {
        let mut records = smart_frame_record(1);
        let second = SHEET_STREAM_HEADER_LEN + records.len();
        let mut skipped = smart_frame_record(2);
        skipped[1] |= 0x80;
        records.extend_from_slice(&skipped);
        let third = SHEET_STREAM_HEADER_LEN + records.len();
        records.extend_from_slice(&smart_frame_record(3));
        (
            chain_stream(&records),
            [SHEET_STREAM_HEADER_LEN, second, third],
        )
    }

    #[test]
    fn a_whole_chain_stream_is_sound() {
        let (stream, starts) = three_record_stream();
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, starts);
        assert_eq!(chain.live_records, 2, "the skipped record is not live");
        assert_eq!(
            chain.stated_records,
            Some(2),
            "the header counts live records"
        );
        assert_eq!(chain.magic, Some(SHEET_STREAM_MAGIC));
        assert_eq!(
            (chain.end, chain.len, chain.stop),
            (stream.len(), stream.len(), None)
        );
        assert_eq!(chain.unread_bytes(), 0);
        assert!(chain.is_sound());
        assert_eq!(sheet_record_starts(&stream), starts);

        // A stream that is its header alone, and an empty one, hold no
        // record and have nothing wrong with them.
        let header_only = chain_stream(&[]);
        let chain = SheetChain::walk(&header_only);
        assert!(chain.starts.is_empty() && chain.is_sound());
        assert_eq!((chain.end, chain.stated_records), (8, Some(0)));
        let chain = SheetChain::walk(&[]);
        assert!(chain.starts.is_empty() && chain.is_sound());
        assert_eq!((chain.end, chain.len, chain.magic), (0, 0, None));
    }

    #[test]
    fn a_chain_that_stops_keeps_the_records_before_the_stop() {
        let (whole, starts) = three_record_stream();
        let [first, second, third] = starts;

        // The second record states a length of zero.
        let mut stream = whole.clone();
        stream[second + 2..second + 6].copy_from_slice(&0u32.to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, [first]);
        assert_eq!(chain.stop, Some(SheetChainStop::ZeroLength));
        assert_eq!(
            (chain.end, chain.unread_bytes()),
            (second, whole.len() - second)
        );
        assert_eq!(chain.live_records, 1);
        assert!(!chain.is_sound());
        assert!(
            !chain.count_differs(),
            "a stop is the fault, not the count it leaves"
        );
        assert!(sheet_record_starts(&stream).is_empty());
        assert_eq!(
            decode_smartframes(&stream)
                .iter()
                .map(|frame| frame.oid)
                .collect::<Vec<_>>(),
            [1],
            "the record before the stop decodes"
        );

        // The last record states one byte more than the stream has.
        let mut stream = whole.clone();
        let stated = u32::from_le_bytes([
            stream[third + 2],
            stream[third + 3],
            stream[third + 4],
            stream[third + 5],
        ]);
        stream[third + 2..third + 6].copy_from_slice(&(stated + 1).to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, [first, second]);
        assert_eq!(
            chain.stop,
            Some(SheetChainStop::PastEnd { stated: stated + 1 })
        );
        assert_eq!(
            (chain.end, chain.unread_bytes()),
            (third, whole.len() - third)
        );
        assert_eq!(chain.live_records, 1);

        // A length that would overflow the offset is past the end too.
        let mut stream = whole.clone();
        stream[third + 2..third + 6].copy_from_slice(&u32::MAX.to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, [first, second]);
        assert_eq!(
            chain.stop,
            Some(SheetChainStop::PastEnd { stated: u32::MAX })
        );

        // Five stray bytes after the last record: too few for an envelope.
        let mut stream = whole.clone();
        stream.extend_from_slice(&[0x3D, 0, 9, 0, 0]);
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, starts);
        assert_eq!(chain.stop, Some(SheetChainStop::ShortEnvelope));
        assert_eq!((chain.end, chain.unread_bytes()), (whole.len(), 5));
        assert_eq!(chain.live_records, 2);

        // Shorter than a header: nothing is read, and that is said.
        for cut in 1..SHEET_STREAM_HEADER_LEN {
            let chain = SheetChain::walk(&whole[..cut]);
            assert!(chain.starts.is_empty());
            assert_eq!(chain.stop, Some(SheetChainStop::ShortHeader), "{cut} bytes");
            assert_eq!((chain.end, chain.unread_bytes()), (0, cut));
            assert!(!chain.is_sound());
        }
    }

    /// A length that is wrong and still inside the stream sends the walk on
    /// from an offset where no record starts. It stops there when the word
    /// it finds is not one a record opens with; when the bytes there read
    /// as a record of a class the reader has, it goes on, and a walk that
    /// lands back on the chain reaches the stream's end with nothing but
    /// the header's count to say so.
    #[test]
    fn a_chain_stops_on_a_word_no_record_opens_with() {
        for (word, opens) in [
            (0x0018u16, true),
            (0x8018, true),
            (0x0001, true),
            (0x0118, true),
            (0x8118, true),
            (0x0000, false),
            (0x8000, false),
            (0x4018, false),
            (0xC018, false),
            (0x0119, false),
            (0x00F8, false),
            (0x80F8, false),
            (0x3FE6, false),
            (0xFFFF, false),
        ] {
            assert_eq!(psm_type_word_opens_a_record(word), opens, "0x{word:04X}");
        }

        let record = |type_word: u16, payload: &[u8]| {
            let mut record = type_word.to_le_bytes().to_vec();
            record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            record.extend_from_slice(payload);
            record
        };
        // The first record's payload ends in four zero bytes and then
        // twelve that read as a line record of six bytes.
        let mut payload = vec![0x11u8; 20];
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(&record(PSM_TYPE_CODE_IGLINE2D, &[0xAB; 6]));
        let a = record(PSM_TYPE_CODE_IGLINE2D, &payload);
        let b = record(PSM_TYPE_CODE_IGTEXTBOX | 0x8000, &[0x22; 9]);
        let c = record(PSM_TYPE_CODE_IGARC2D, &[0x33; 5]);
        let whole = chain_stream(&[a.clone(), b.clone(), c].concat());
        let first = SHEET_STREAM_HEADER_LEN;
        let second = first + a.len();
        let third = second + b.len();
        let chain = SheetChain::walk(&whole);
        assert_eq!(chain.starts, [first, second, third]);
        assert!(chain.is_sound());
        let stated = payload.len() as u32;

        // Sixteen bytes short: the walk goes on from the four zero bytes.
        let mut stream = whole.clone();
        stream[first + 2..first + 6].copy_from_slice(&(stated - 16).to_le_bytes());
        let chain = SheetChain::walk(&stream);
        let stop = first + PSM_ENVELOPE_LEN + payload.len() - 16;
        assert_eq!(
            chain.starts,
            [first],
            "the record whose length is wrong is read"
        );
        assert_eq!(
            chain.stop,
            Some(SheetChainStop::NoSuchClass { type_word: 0 })
        );
        assert_eq!(
            (chain.end, chain.unread_bytes()),
            (stop, whole.len() - stop)
        );
        assert_eq!(chain.live_records, 1);
        assert!(!chain.is_sound());
        assert!(sheet_record_starts(&stream).is_empty());

        // Twelve bytes short: the bytes there read as a record of a class
        // the reader has, and it ends where the second record starts. The
        // walk reaches the stream's end; the header's count is one short
        // of what it read, and that is all that is wrong with it.
        let mut stream = whole.clone();
        stream[first + 2..first + 6].copy_from_slice(&(stated - 12).to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, [first, second - 12, second, third]);
        assert_eq!((chain.stop, chain.end), (None, whole.len()));
        assert_eq!((chain.stated_records, chain.live_records), (Some(2), 3));
        assert!(chain.count_differs() && !chain.is_sound());

        // A record of the chain itself whose word no record opens with: the
        // walk stops on it, whatever its length says.
        let mut stream = whole.clone();
        stream[second..second + 2]
            .copy_from_slice(&(PSM_TYPE_CODE_IGTEXTBOX | 0x4000).to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.starts, [first]);
        assert_eq!(
            chain.stop,
            Some(SheetChainStop::NoSuchClass {
                type_word: PSM_TYPE_CODE_IGTEXTBOX | 0x4000
            })
        );
        assert_eq!(
            (chain.end, chain.unread_bytes()),
            (second, whole.len() - second)
        );
    }

    #[test]
    fn a_header_that_disagrees_with_its_chain_is_not_sound() {
        let (whole, starts) = three_record_stream();
        let [_, _, third] = starts;

        // Cut off at a record boundary: the walk reaches the new end with
        // nothing wrong on the way, and the header still counts the record
        // that is gone.
        let cut = &whole[..third];
        let chain = SheetChain::walk(cut);
        assert_eq!((chain.stop, chain.end), (None, third));
        assert_eq!((chain.stated_records, chain.live_records), (Some(2), 1));
        assert!(chain.count_differs());
        assert!(!chain.is_sound());
        assert_eq!(
            sheet_record_starts(cut).len(),
            2,
            "the all-or-nothing reading does not hold the header to the chain"
        );

        // The count alone.
        let mut stream = whole.clone();
        stream[4..8].copy_from_slice(&3u32.to_le_bytes());
        let chain = SheetChain::walk(&stream);
        assert!(chain.count_differs() && !chain.is_sound());
        assert_eq!(chain.starts, starts, "every record is still read");

        // The magic alone.
        let mut stream = whole.clone();
        stream[..4].copy_from_slice(b"BACK");
        let chain = SheetChain::walk(&stream);
        assert_eq!(chain.wrong_magic(), Some(u32::from_le_bytes(*b"BACK")));
        assert!(!chain.count_differs() && !chain.is_sound());
        assert_eq!(chain.starts, starts, "every record is still read");
    }

    #[test]
    fn the_chain_walk_survives_truncation_and_noise() {
        let (whole, _) = three_record_stream();
        for cut in 0..=whole.len() {
            let chain = SheetChain::walk(&whole[..cut]);
            assert_eq!(chain.len, cut);
            assert!(chain.end <= cut);
            assert_eq!(chain.unread_bytes(), cut - chain.end);
            // A cut inside a record stops; a cut on a boundary does not.
            let on_a_boundary = cut == 0 || chain.end == cut;
            assert_eq!(chain.stop.is_none(), on_a_boundary, "cut at {cut}");
        }
        let noise: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        for at in 0..noise.len() {
            let chain = SheetChain::walk(&noise[at..]);
            assert!(chain.end <= chain.len);
            assert!(chain.starts.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

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

    /// Wrap synthetic records into a `Sheet*` stream: the 8-byte stream
    /// header, then the records nose to tail.
    ///
    /// A family's `scan` only considers offsets that are members of
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
    /// coincidence: the gate every family decodes behind.
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

        // The trait's walk and the family's own wrapper are the same walk:
        // the stream's chain.
        let via_trait = IgLine2dDecoder.scan(&data);
        let via_free_fn = decode_iglines(&data);
        assert_eq!(
            via_trait, via_free_fn,
            "the trait's scan and the free function walk the same chain"
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
        let decoded = decode_iglinestrings(&chain_stream(&record));
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
        let decoded = decode_iglinestrings(&chain_stream(&record));
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
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
        assert_eq!(decode_iglinestrings(&chain_stream(&record)).len(), 1);
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
    }

    #[test]
    fn iglinestring2d_decodes_all_same_vertices_as_an_anchor() {
        // Real drawings carry vc=2 records whose vertices coincide -- a
        // position an annotation or symbol anchors to, not a stroke
        // (probe_real5_linestring_refusals). They decode; the emitter, not
        // the decoder, decides there is nothing to stroke.
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
        let decoded = decode_iglinestrings(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].total_length(), 0.0);
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
        assert!(decode_iglinestring_at(&record, 0).is_none());
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
            assert!(decode_iglinestring_at(&record[..trunc_len], 0).is_none());
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
        let decoded = decode_igpoints(&chain_stream(&record));
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
        assert!(decode_igpoint_at(&record, 0).is_none());
    }

    #[test]
    fn igpoint2d_rejects_wrong_bytes_to_follow() {
        let mut record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.0, 0.0);
        record[2] = 33;
        assert!(decode_igpoint_at(&record, 0).is_none());
    }

    #[test]
    fn igpoint2d_rejects_nan() {
        let mut record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.0, 0.0);
        // x at record offset 6 + 18 = 24
        record[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igpoint_at(&record, 0).is_none());
    }

    #[test]
    fn igpoint2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igpoint2d_record(1, 1, 0x10, 0, 0.5, 0.5);
        for trunc_len in 0..record.len() {
            assert!(decode_igpoint_at(&record[..trunc_len], 0).is_none());
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
        let decoded = decode_igtextboxes(&chain_stream(&record));
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

    /// Where a sub-type 2 record's placement tail starts, record-relative:
    /// the 6-byte envelope, the 22-byte header and the 10 + 2·n body.
    fn textbox_tail(record_len: usize, text_len_chars: usize) -> usize {
        let _ = record_len;
        6 + IGTEXTBOX_BODY_START + 10 + 2 * text_len_chars
    }

    /// Mark the record's tail as `kind` and, for the kind that carries a text
    /// box, grow the record to the 68 bytes that tail occupies.
    fn set_tail_kind(record: &mut Vec<u8>, kind: u16) {
        let at = 6 + IGTEXTBOX_TAIL_KIND_AT;
        record[at..at + 2].copy_from_slice(&kind.to_le_bytes());
        if kind == IGTEXTBOX_TAIL_KIND_BOX {
            let btf = u32::from_le_bytes([record[2], record[3], record[4], record[5]]);
            let grown = btf + (IGTEXTBOX_TAIL_KIND2_LEN - IGTEXTBOX_TAIL_LEN) as u32;
            record[2..6].copy_from_slice(&grown.to_le_bytes());
            record.extend(std::iter::repeat_n(
                0u8,
                IGTEXTBOX_TAIL_KIND2_LEN - IGTEXTBOX_TAIL_LEN,
            ));
        }
    }

    /// Write the text box into a record whose tail is already kind 2.
    fn set_text_box(record: &mut [u8], text_len_chars: usize, width: f64, height: f64) {
        let tail = textbox_tail(record.len(), text_len_chars);
        let mut at = tail + IGTEXTBOX_BOX_WIDTH_AT;
        record[at..at + 8].copy_from_slice(&width.to_le_bytes());
        at = tail + IGTEXTBOX_BOX_HEIGHT_AT;
        record[at..at + 8].copy_from_slice(&height.to_le_bytes());
    }

    /// A title-block `igTextBox` states the box it letters inside.
    ///
    /// The kind-2 tail's bytes past the placement block were the corpus's
    /// oldest open question on this family ("32 or 40 bytes, undecoded").
    /// Two of them are the text box, and the reference plots letter a boxed
    /// record from the box's left edge rather than from its insertion point,
    /// which is what
    /// `docs/analysis/2026-10-19-igtextbox-box-is-where-the-text-starts.md`
    /// measures.
    #[test]
    fn igtextbox_decodes_the_text_box_a_kind_2_tail_carries() {
        let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        set_tail_kind(&mut record, IGTEXTBOX_TAIL_KIND_BOX);
        set_text_box(&mut record, 8, 0.045_307, 0.003_689);

        let decoded = decode_igtextboxes(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].text, "PUMP-101");
        assert!((decoded[0].text_box_width_m - 0.045_307).abs() < 1e-9);
        assert!((decoded[0].text_box_height_m - 0.003_689).abs() < 1e-9);
    }

    /// A kind-1 tail states no box, and neither does a kind-2 one whose box
    /// reads as something that is not a size.
    #[test]
    fn igtextbox_reports_no_box_when_the_tail_states_none() {
        let plain = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
        let decoded = decode_igtextboxes(&chain_stream(&plain));
        assert_eq!(decoded[0].text_box_width_m, 0.0);
        assert_eq!(decoded[0].text_box_height_m, 0.0);

        for (width, height) in [
            (0.0, 0.003_689),
            (-0.045, 0.003_689),
            (f64::NAN, 0.003_689),
            (0.045_307, f64::INFINITY),
        ] {
            let mut record = build_synthetic_igtextbox_record("PUMP-101", 100, 50);
            set_tail_kind(&mut record, IGTEXTBOX_TAIL_KIND_BOX);
            set_text_box(&mut record, 8, width, height);

            let decoded = decode_igtextboxes(&chain_stream(&record));
            assert_eq!(
                decoded.len(),
                1,
                "({width}, {height}) is a field, not a reason to refuse the label"
            );
            assert_eq!(
                (decoded[0].text_box_width_m, decoded[0].text_box_height_m),
                (0.0, 0.0),
                "({width}, {height}) is not a box"
            );
        }
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

            let decoded = decode_igtextboxes(&chain_stream(&record));
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
                decode_igtextbox_at(&record, 0).is_none(),
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

            let decoded = decode_igtextboxes(&chain_stream(&record));
            assert_eq!(
                decoded.len(),
                1,
                "a {extra}-byte longer tail must not refuse the record"
            );
            assert_eq!(decoded[0].text, "PUMP-101");
            assert_eq!(decoded[0].text_length, 8);
            // The record still spans its whole stated length, extra bytes
            // included: that is where the chain steps to.
            assert_eq!(
                decoded[0].byte_range,
                SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + record.len()
            );
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
            decode_igtextbox_at(&record, 0).is_none(),
            "a stated length with no room for text plus the trailing block is noise"
        );

        // The other way a stated length lies: a sub-type 2 record carries its
        // count twice -- packed with a 0x10000 marker at +22 and plain at
        // +30 -- and editing only one of them breaks their agreement.
        let mut record = build_synthetic_igtextbox_record("HELLO", 1, 1);
        record[6 + 30..6 + 32].copy_from_slice(&999u16.to_le_bytes());
        assert!(
            decode_igtextbox_at(&record, 0).is_none(),
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
            let decoded = decode_igtextboxes(&chain_stream(&record));

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
        let decoded = decode_igtextboxes(&chain_stream(&shape_2));
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
        let decoded = decode_igtextboxes(&chain_stream(&shape_3));
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
        let decoded = decode_igtextboxes(&chain_stream(&shape_1));
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
            decode_igtextbox_at(&record, 0).is_none(),
            "the native Load names sub-types 1, 2 and 3; a fourth is not a shape we know"
        );
    }

    #[test]
    fn igtextbox_decodes_chinese_unicode_text() {
        let record = build_synthetic_igtextbox_record("流量计", 200, 100);
        let decoded = decode_igtextboxes(&chain_stream(&record));
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
        assert!(decode_igtextbox_at(&record, 0).is_none());
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
        assert!(decode_igtextbox_at(&record, 0).is_none());
    }

    #[test]
    fn igtextbox_rejects_nan_trailing_double() {
        let mut record = build_synthetic_igtextbox_record("ABC", 1, 1);
        let text_end = 6 + 32 + 3 * 2; // record header + payload header + text
        record[text_end..text_end + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igtextbox_at(&record, 0).is_none());
    }

    #[test]
    fn igtextbox_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igtextbox_record("X", 1, 1);
        for trunc_len in 0..record.len() {
            assert!(decode_igtextbox_at(&record[..trunc_len], 0).is_none());
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
        // The canonical 113-byte payload shape, written literally so the
        // corpus-derived `IGSYMBOL2D_MIN_PAYLOAD_LEN` floor can move without
        // silently resizing every fixture record these tests build.
        const CANONICAL_PAYLOAD_LEN: usize = 113;
        let mut out = Vec::with_capacity(6 + CANONICAL_PAYLOAD_LEN);
        out.extend_from_slice(&PSM_TYPE_CODE_IGSYMBOL2D.to_le_bytes());
        out.extend_from_slice(&(CANONICAL_PAYLOAD_LEN as u32).to_le_bytes());
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
        let decoded = decode_igsymbols(&chain_stream(&short));
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
        let decoded = decode_igsymbols(&chain_stream(&long));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].definition_sheet_ref, 93);
        assert_eq!(decoded[0].definition_site_ref, 145);
    }

    /// The two title-block templates' placement (`CPE-SQ-A0.igr` /
    /// `CPE-SQ-A1.igr` `/Sheet6`): 109 bytes, tag at 33, so the matrix starts
    /// at 37 and the 72 bytes it needs land exactly on the payload's end.
    ///
    /// The floor used to be a corpus-derived 111, which refused this record
    /// whole — and with it the 5 mm wave the templates place in the `比例`
    /// value cell, the last 14.2 mm no page explained
    /// (`docs/analysis/2026-10-24-the-wave-is-a-symbol-in-the-title-block.md`).
    #[test]
    fn igsymbol2d_decodes_the_109_byte_title_block_placement() {
        let mut record =
            build_synthetic_igsymbol2d_record(181, 6, 18, [1.0, 0.0, 0.0, 1.0], (1100.692, 19.0));
        record.truncate(PSM_ENVELOPE_LEN + 109);
        record[2..6].copy_from_slice(&109u32.to_le_bytes());
        let tail = PSM_ENVELOPE_LEN + 109;
        record[tail - 8..tail - 4].copy_from_slice(&29u32.to_le_bytes());
        record[tail - 4..].copy_from_slice(&16u32.to_le_bytes());

        let decoded = decode_igsymbols(&chain_stream(&record));
        assert_eq!(
            decoded.len(),
            1,
            "a placement that holds its own tail decodes"
        );
        assert_eq!(decoded[0].bytes_to_follow, 109);
        assert_eq!(decoded[0].insertion, (1100.692, 19.0));
        assert_eq!(decoded[0].definition_sheet_ref, 29);
        assert_eq!(decoded[0].definition_site_ref, 16);
    }

    /// One byte less than the matrix plus its tail and the record is refused:
    /// the scale would read over the field that names the body.
    #[test]
    fn igsymbol2d_rejects_a_payload_one_byte_short_of_the_matrix_and_tail() {
        let mut record =
            build_synthetic_igsymbol2d_record(181, 6, 18, [1.0, 0.0, 0.0, 1.0], (1100.692, 19.0));
        record.truncate(PSM_ENVELOPE_LEN + 108);
        record[2..6].copy_from_slice(&108u32.to_le_bytes());
        assert!(decode_igsymbol_at(&record, 0).is_none());
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
        let decoded = decode_igsymbols(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let s = &decoded[0];
        assert_eq!(s.type_code, PSM_TYPE_CODE_IGSYMBOL2D);
        assert_eq!(s.bytes_to_follow, 113);
        assert_eq!(s.oid, 500);
        assert_eq!(s.parent_ref, 6);
        assert_eq!(s.jsite_ref, 399);
        assert_eq!(s.transform, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(s.insertion, (0.3, 0.4));
        assert_eq!(s.matrix_tag_third_byte, 0xA7);
    }

    /// `D04` (the plant backup's `test/U01/D04.pid`) writes every one of
    /// its own `igSymbol2d` records with the matrix tag's third byte
    /// `0x27` rather than the canonical `0xA7` — 6 of 6 records in that
    /// one fixture, 0 of 12 411 in the other 56. Every other field keeps
    /// its documented shape under the `0x27` variant: this is the
    /// regression a too-narrow tag match produced (5 live placements
    /// silently dropped — `ISA 2 WAY GENERIC BODY`, `ISA_LOGIC CTRL`,
    /// `CONCENTRIC SIZE CHANGE` ×2, `qiufa.sym`).
    #[test]
    fn igsymbol2d_decodes_the_0x27_matrix_tag_variant_d04_writes() {
        let mut record = build_synthetic_igsymbol2d_record(
            0x5C,
            6,
            151,
            [1.0, 0.0, 0.0, -1.0],
            (0.259_022, 0.153_433),
        );
        let tag_at = PSM_ENVELOPE_LEN + 33;
        record[tag_at..tag_at + IGSYMBOL2D_MATRIX_TAG_ALT.len()]
            .copy_from_slice(&IGSYMBOL2D_MATRIX_TAG_ALT);
        let decoded = decode_igsymbols(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let s = &decoded[0];
        assert_eq!(s.oid, 0x5C);
        assert_eq!(s.jsite_ref, 151);
        assert_eq!(s.transform, [1.0, 0.0, 0.0, -1.0]);
        assert_eq!(s.insertion, (0.259_022, 0.153_433));
        assert_eq!(s.matrix_tag_third_byte, 0x27);
    }

    #[test]
    fn igsymbol2d_rejects_a_matrix_tag_third_byte_neither_variant_writes() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
        let tag_at = PSM_ENVELOPE_LEN + 33;
        // Third byte neither `0xA7` nor `0x27`: still refused, this is not
        // a wildcard match.
        record[tag_at + 2] = 0x37;
        assert!(decode_igsymbol_at(&record, 0).is_none());
    }

    #[test]
    fn igsymbol2d_rejects_wrong_type_code() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
        record[0] = 0x18;
        record[1] = 0x00;
        assert!(decode_igsymbol_at(&record, 0).is_none());
    }

    #[test]
    fn igsymbol2d_rejects_undersized_bytes_to_follow() {
        let mut record = vec![];
        record.extend_from_slice(&PSM_TYPE_CODE_IGSYMBOL2D.to_le_bytes());
        record.extend_from_slice(&100u32.to_le_bytes());
        record.extend_from_slice(&[0u8; 100]);
        assert!(decode_igsymbol_at(&record, 0).is_none());
    }

    #[test]
    fn igsymbol2d_rejects_nan_transform_element() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
        let at = PSM_ENVELOPE_LEN + SYNTHETIC_IGSYMBOL2D_MATRIX_AT;
        record[at..at + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igsymbol_at(&record, 0).is_none());
    }

    #[test]
    fn igsymbol2d_rejects_record_without_matrix_tag() {
        let mut record =
            build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
        let tag_at = PSM_ENVELOPE_LEN + 33;
        record[tag_at..tag_at + IGSYMBOL2D_MATRIX_TAG.len()].copy_from_slice(&[0xFF; 4]);
        assert!(decode_igsymbol_at(&record, 0).is_none());
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

        let decoded = decode_igsymbols(&chain_stream(&shifted));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].jsite_ref, 204);
        assert_eq!(decoded[0].transform, [0.0, 1.0, -1.0, 0.0]);
        assert_eq!(decoded[0].insertion, (0.25, 0.75));
    }

    #[test]
    fn igsymbol2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igsymbol2d_record(1, 1, 0, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
        for trunc_len in 0..record.len() {
            assert!(decode_igsymbol_at(&record[..trunc_len], 0).is_none());
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

    /// The tail a group of `members` needs at the least: `36 + 8·k` bytes of
    /// payload, less the 18 the builder writes itself.
    fn shortest_dependency_tail(members: usize) -> Vec<u8> {
        vec![0x01; 36 + 8 * members - 18]
    }

    #[test]
    fn dependency_object_decodes_canonical_header_and_raw_tail() {
        let raw_tail = shortest_dependency_tail(2);
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);

        let decoded = decode_dependency_objects(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let group = &decoded[0];
        assert_eq!(
            group.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + record.len()
        );
        assert_eq!(group.type_code, PSM_TYPE_CODE_DEPENDENCY_OBJECT);
        assert_eq!(group.type_flags, 0);
        assert_eq!(group.bytes_to_follow, 52);
        assert_eq!(group.oid, 42);
        assert_eq!(group.parent_ref, 6);
        assert_eq!(group.group_kind_word, 2);
        assert_eq!(group.sub_type_word, 0x01A1);
        assert_eq!(group.raw_reference_payload, raw_tail);
    }

    /// `+14` is the member count, bounded by the room the payload has for
    /// members rather than by the largest count one batch of fixtures showed:
    /// DWG-0201's 22-member group (212 bytes = 36 + 8 x 22) decodes, and a
    /// count the payload cannot hold is still refused.
    #[test]
    fn dependency_object_bounds_its_member_count_by_the_room_for_members() {
        let group_of_22 = build_synthetic_dependency_object_record(
            781,
            6,
            22,
            0x02BB,
            &shortest_dependency_tail(22),
        );
        let decoded = decode_dependency_objects(&chain_stream(&group_of_22));
        assert_eq!(
            decoded.len(),
            1,
            "a 22-member group with room for 22 decodes"
        );
        assert_eq!(decoded[0].bytes_to_follow, 212);
        assert_eq!(decoded[0].group_kind_word, 22);

        let too_many = build_synthetic_dependency_object_record(
            781,
            6,
            23,
            0x02BB,
            &shortest_dependency_tail(22),
        );
        assert!(
            decode_dependency_object_at(&too_many, 0).is_none(),
            "23 members do not fit in 212 bytes"
        );
    }

    /// A dependency object that lists no member is the same record with a
    /// count of zero: the 16-byte head, no `(oid, 1)` entries, no flags, the
    /// six bytes every record has before its closing block, and the block.
    /// `A井场`'s `/Sheet6 +251219` is one — oid 46838, 36 bytes, the payload
    /// below verbatim — and it was the one record of the 53 backup sheets a
    /// family refused: the decoder required a count of at least one and,
    /// from it, a payload of at least 44 bytes. `36 + 8·k` holds at `k = 0`.
    #[test]
    fn dependency_object_with_no_member_decodes() {
        let payload: [u8; 36] = [
            0xf6, 0xb6, 0x00, 0x00, // oid 46838
            0x06, 0x00, 0x00, 0x00, // owner: the page
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
            0x00, 0x00, // member count: none
            0x00, 0x00, 0x03, 0x00, 0x00, 0x00, // the six bytes before the block
            0x07, 0x00, 0x01, 0x00, 0x08, 0x00, // block: type 7, selector 1, 8 bytes
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut record = Vec::new();
        record.extend_from_slice(&PSM_TYPE_CODE_DEPENDENCY_OBJECT.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);

        let decoded = decode_dependency_objects(&chain_stream(&record));
        assert_eq!(decoded.len(), 1, "a group of no member is still a record");
        assert_eq!(decoded[0].oid, 46838);
        assert_eq!(decoded[0].parent_ref, 6);
        assert_eq!(decoded[0].group_kind_word, 0);
        assert_eq!(decoded[0].bytes_to_follow, 36);
        // The closing block describes itself: its length word and the bytes
        // after it end the record.
        let tail = &decoded[0].raw_reference_payload;
        assert_eq!(tail.len(), 18);
        assert_eq!(u16::from_le_bytes([tail[8], tail[9]]), 8);

        // One byte short of the head, the six bytes and the shortest block,
        // it is not this record.
        let mut short = record[..record.len() - 2].to_vec();
        short[2..6].copy_from_slice(&34u32.to_le_bytes());
        assert!(decode_dependency_object_at(&short, 0).is_none());
    }

    // Each refusal below starts from a record that decodes — a group of two
    // with room for two — so that the one thing changed is what refuses it.

    #[test]
    fn dependency_object_rejects_wrong_type_code() {
        let raw_tail = shortest_dependency_tail(2);
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        assert!(decode_dependency_object_at(&record, 0).is_some());
        record[0] = 0x18;
        record[1] = 0x00;

        assert!(decode_dependency_object_at(&record, 0).is_none());
    }

    #[test]
    fn dependency_object_rejects_nonzero_type_flags() {
        let raw_tail = shortest_dependency_tail(2);
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        let flagged_type = PSM_TYPE_CODE_DEPENDENCY_OBJECT | 0x4000;
        record[0..2].copy_from_slice(&flagged_type.to_le_bytes());

        assert!(decode_dependency_object_at(&record, 0).is_none());
    }

    #[test]
    fn dependency_object_rejects_invalid_size_and_truncation() {
        // An odd length, with the payload grown to match so that only the
        // parity is wrong.
        let mut raw_tail = shortest_dependency_tail(2);
        raw_tail.push(0x01);
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        assert_eq!(record.len(), 6 + 53);
        assert!(decode_dependency_object_at(&record, 0).is_none());

        let raw_tail = shortest_dependency_tail(2);
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        assert!(decode_dependency_object_at(&record[..record.len() - 1], 0).is_none());
        assert!(decode_dependency_objects(&chain_stream(&record[..record.len() - 1])).is_empty());
    }

    #[test]
    fn dependency_object_rejects_invalid_header_fields() {
        let raw_tail = shortest_dependency_tail(2);
        assert!(decode_dependency_object_at(
            &build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail),
            0
        )
        .is_some());
        assert!(
            decode_dependency_object_at(
                &build_synthetic_dependency_object_record(0, 6, 2, 0x01A1, &raw_tail),
                0
            )
            .is_none(),
            "an oid of zero"
        );
        assert!(
            decode_dependency_object_at(
                &build_synthetic_dependency_object_record(42, 7, 2, 0x01A1, &raw_tail),
                0
            )
            .is_none(),
            "an owner other than the page"
        );
    }

    #[test]
    fn dependency_object_rejects_nonzero_reserved_prefix() {
        let raw_tail = shortest_dependency_tail(2);
        let mut record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        assert!(decode_dependency_object_at(&record, 0).is_some());
        record[6 + 8] = 1;

        assert!(decode_dependency_object_at(&record, 0).is_none());
    }

    #[test]
    fn dependency_object_decoder_is_panic_safe_on_short_input() {
        let raw_tail = shortest_dependency_tail(2);
        let record = build_synthetic_dependency_object_record(42, 6, 2, 0x01A1, &raw_tail);
        for trunc_len in 0..record.len() {
            assert!(decode_dependency_object_at(&record[..trunc_len], 0).is_none());
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
        let first = build_synthetic_dependency_object_record(
            42,
            6,
            2,
            0x01A1,
            &shortest_dependency_tail(2),
        );
        let second = build_synthetic_dependency_object_record(
            43,
            6,
            1,
            0x00B8,
            &shortest_dependency_tail(1),
        );
        let mut data = first;
        data.extend_from_slice(&second);

        let decoded = decode_dependency_objects(&chain_stream(&data));
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 42);
        assert_eq!(decoded[1].oid, 43);
    }

    #[test]
    fn a_record_the_native_reader_skips_is_not_decoded_by_any_family() {
        // The upper flag bit (type word 0x8000) is the one PSMSerializeIn
        // seeks past before reading the oid; `parse_live_psm_header` turns
        // every family away from it, with or without the lower bit beside.
        let live = build_synthetic_igline2d_record(7, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        assert!(decode_igline_at(&live, 0).is_some());
        for flags in [0b10u16, 0b11] {
            let mut record = live.clone();
            let type_word = PSM_TYPE_CODE_IGLINE2D | (flags << 14);
            record[..2].copy_from_slice(&type_word.to_le_bytes());
            assert!(
                decode_igline_at(&record, 0).is_none(),
                "flags {flags:#b} must be skipped"
            );
            let header = parse_psm_header(&record, 0).expect("envelope reads");
            assert!(header.native_reader_skips());
            assert!(parse_live_psm_header(&record, 0).is_none());
        }
    }

    /// A `JSheet` record as the chain stores it: the type word (`0x8000`
    /// set when the native reader is to skip it), `bytes_to_follow = 4`,
    /// then the oid -- all of the record [`jsheet_oids`] reads.
    fn build_synthetic_jsheet_record(oid: u32, skipped: bool) -> Vec<u8> {
        let skip_bit: u16 = if skipped { 0x8000 } else { 0 };
        let type_word = PSM_TYPE_CODE_JSHEET | skip_bit;
        let mut out = Vec::with_capacity(PSM_ENVELOPE_LEN + 4);
        out.extend_from_slice(&type_word.to_le_bytes());
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&oid.to_le_bytes());
        out
    }

    #[test]
    fn jsheet_oids_are_the_live_sheets_for_every_skip_bit_combination() {
        // Feature: pid-import-next-round, Property 6: jsheet_oids 只收活记录
        // **Validates: Requirements 8.1, 8.2, 8.3**
        //
        // Every stream of 1-4 JSheet records under every choice of the
        // native-skip bit, enumerated: 2 + 4 + 8 + 16 = 30 streams. The
        // sheets name only the records without the bit; the chain walk
        // still starts at every record, a skipped one included.
        let mut streams = 0;
        for count in 1..=4usize {
            for mask in 0..(1u32 << count) {
                let skipped = |i: usize| mask & (1 << i) != 0;
                let oids: Vec<u32> = (0..count).map(|i| 0x1000 + i as u32).collect();
                let records: Vec<Vec<u8>> = oids
                    .iter()
                    .enumerate()
                    .map(|(i, &oid)| build_synthetic_jsheet_record(oid, skipped(i)))
                    .collect();
                let stream = synthetic_sheet_stream(&records);
                let case = format!("{count} records, skip mask {mask:#06b}");

                let live: Vec<u32> = oids
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| !skipped(i))
                    .map(|(_, &oid)| oid)
                    .collect();
                assert_eq!(jsheet_oids(&stream), live, "{case}");

                let mut starts = Vec::with_capacity(count);
                let mut at = SHEET_STREAM_HEADER_LEN;
                for record in &records {
                    starts.push(at);
                    at += record.len();
                }
                assert_eq!(sheet_record_starts(&stream), starts, "{case}");
                streams += 1;
            }
        }
        assert_eq!(streams, 30, "every length 1-4 under every skip-bit choice");
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
        let decoded = decode_igboundaries(&chain_stream(&record));
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
        assert_eq!(
            b.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + 6 + 172
        );
    }

    #[test]
    fn igboundary2d_rejects_wrong_type_code() {
        let mut record = canonical_igboundary_triangle();
        record[0] = 0x18; // igLine2d type code instead
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_bytes_to_follow_off_formula() {
        let mut record = canonical_igboundary_triangle();
        record[2] = 171; // 172 -> 171 breaks `49 + 41 × n`
        record.truncate(6 + 171);
        assert!(decode_igboundary_at(&record, 0).is_none());
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
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_wrong_sub_header_flag() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 18] = 2; // u32 at +18 must equal 1
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_missing_segment_tag() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 61] = 0x68; // second group tag corrupted
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_member_count_mismatch() {
        let mut record = canonical_igboundary_triangle();
        // member_count u32 lives at payload +144 (pos=127 + 17).
        record[6 + 144] = 4;
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_nan_segment_coordinate() {
        let mut record = canonical_igboundary_triangle();
        let nan = f64::NAN.to_le_bytes();
        record[6 + 29..6 + 37].copy_from_slice(&nan); // first segment start.x
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_out_of_domain_anchor() {
        let mut record = canonical_igboundary_triangle();
        let big = 1e10f64.to_le_bytes();
        record[6 + 127..6 + 135].copy_from_slice(&big); // anchor.x
        assert!(decode_igboundary_at(&record, 0).is_none());
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
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    #[test]
    fn igboundary2d_rejects_zero_segment_count() {
        let mut record = canonical_igboundary_triangle();
        record[6 + 22..6 + 26].copy_from_slice(&0u32.to_le_bytes());
        assert!(decode_igboundary_at(&record, 0).is_none());
    }

    /// `EF井场 注采阀组工艺及自控流程图.pid` `/Sheet6 +730028`, oid 52844,
    /// all 99 payload bytes as they sit on disk: sub-type `0x0000`, one
    /// `0x73` circle member, then the drawing-side anchor (the circle's
    /// centre), flag, member count and the reference to the `igCircle2d`
    /// it fills (oid 52772, class word `0x00D1`) -- the plant backup's one
    /// sheet boundary the template grammar refuses.
    #[rustfmt::skip]
    const EF_CIRCLE_BOUNDARY_PAYLOAD: [u8; 99] = [
        // +0    oid 52844     parent 6      layer 8       sub 0     index 113
        0x6C, 0xCE, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x71, 0x00,
        // +16         u32 1         count 1       04 01     0x73 + centre_x
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x01, 0x73, 0x3D, 0x3D, 0x0D,
        0xEF, 0x96, 0x42, 0xEB, 0x3F, 0x34, 0x88, 0x64, 0x41, 0x80, 0xB0, 0xC0, 0x3F, 0x00, 0x78, 0x11,
        // +48   radius 0.463 mm, then 0 and 1, and the direction byte at +69
        0x4B, 0xFC, 0x59, 0x3E, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xF0, 0x3F, 0x01, 0x3D, 0x3D, 0x0D, 0xEF, 0x96, 0x42, 0xEB, 0x3F, 0x34, 0x88,
        // +80   anchor ends at +86, flag 1, count 1, member 52772 / 0x00D1 / 5
        0x64, 0x41, 0x80, 0xB0, 0xC0, 0x3F, 0x01, 0x01, 0x00, 0x00, 0x00, 0x24, 0xCE, 0x00, 0x00, 0xD1,
        0x00, 0x05, 0x00,
    ];

    fn ef_circle_boundary_record() -> Vec<u8> {
        let mut record = PSM_TYPE_CODE_IGBOUNDARY2D.to_le_bytes().to_vec();
        record.extend_from_slice(&(EF_CIRCLE_BOUNDARY_PAYLOAD.len() as u32).to_le_bytes());
        record.extend_from_slice(&EF_CIRCLE_BOUNDARY_PAYLOAD);
        record
    }

    /// The circle member closes a ring of chords on the circle itself, so
    /// the emitter's ring is the dot the record fills, and every trailer
    /// field is read where the drawing-side grammar puts it.
    #[test]
    fn igboundary2d_decodes_a_drawing_boundary_over_a_circle() {
        let record = ef_circle_boundary_record();
        let decoded = decode_igboundaries(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let b = &decoded[0];
        assert_eq!(b.oid, 52844);
        assert_eq!(b.sheet_layer_ref, 8);
        assert_eq!(b.sub_type_word, 0x0000);
        assert_eq!(b.index, 113);
        assert_eq!(b.segment_count, 1);
        assert_eq!(b.sub_header_tail, [4, 1]);
        let centre = (
            f64::from_le_bytes(EF_CIRCLE_BOUNDARY_PAYLOAD[29..37].try_into().unwrap()),
            f64::from_le_bytes(EF_CIRCLE_BOUNDARY_PAYLOAD[37..45].try_into().unwrap()),
        );
        let radius = f64::from_le_bytes(EF_CIRCLE_BOUNDARY_PAYLOAD[45..53].try_into().unwrap());
        assert!((radius - 0.000_463).abs() < 1e-6, "{radius}");
        assert_eq!(b.anchor, centre);
        assert_eq!(b.trailer_flag, 1);
        assert_eq!(
            b.member_refs,
            vec![SheetIgBoundary2dMemberRef {
                member_oid: 52772,
                class_word: 0x00D1,
                sub_word: 5,
            }]
        );
        assert_eq!(b.segments.len(), 26, "a whole turn in 0.25 rad chords");
        assert!(b.is_closed_loop(1e-12));
        for segment in &b.segments {
            assert_eq!(segment.tag_offset, 28);
            let off = (segment.start.0 - centre.0).hypot(segment.start.1 - centre.1);
            assert!(
                (off - radius).abs() < 1e-12,
                "a vertex {off} off the circle"
            );
        }
    }

    /// The template grammar still refuses this record -- its member walk
    /// knows no `0x73` and its 21-byte trailer is not this one -- so the
    /// fall-through is what decodes it, and it does not loosen anything
    /// else: a circle with no radius, or one that leaves a byte unread, is
    /// refused.
    #[test]
    fn igboundary2d_circle_member_must_close_the_payload() {
        let record = ef_circle_boundary_record();
        let header = parse_psm_header(&record, 0).unwrap();
        assert!(
            decode_igboundary_template_payload(&record, 0, &header, record.len(), 52844, 6, 8)
                .is_none()
        );

        let mut flat = ef_circle_boundary_record();
        flat[6 + 45..6 + 53].copy_from_slice(&0f64.to_le_bytes());
        assert!(decode_igboundary_at(&flat, 0).is_none(), "zero radius");

        let mut longer = ef_circle_boundary_record();
        longer.push(0);
        longer[2..6].copy_from_slice(&100u32.to_le_bytes());
        assert!(
            decode_igboundary_at(&longer, 0).is_none(),
            "a byte past the refs"
        );

        let mut other_sub = ef_circle_boundary_record();
        other_sub[6 + 12] = 0x11;
        assert!(
            decode_igboundary_at(&other_sub, 0).is_none(),
            "sub-type 0x0011"
        );
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
        let decoded = decode_igboundaries(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        assert!(!decoded[0].is_closed_loop(1e-9));
    }

    #[test]
    fn igboundary2d_decoder_is_panic_safe_on_short_input() {
        let record = canonical_igboundary_triangle();
        for trunc_len in 0..record.len() {
            assert!(decode_igboundary_at(&record[..trunc_len], 0).is_none());
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
        let decoded = decode_igboundaries(&chain_stream(&data));
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 81);
        assert_eq!(decoded[1].oid, 522);
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
        assert!(decoded[1].is_closed_loop(1e-9));
    }

    /// A boundary of two loops: an outer square of side 4 counter-clockwise
    /// and a square hole of side 2 inside it clockwise, the anchor between
    /// the two, and one reference per member — the layout of `XA-A1.igr`'s
    /// two letters with a counter, in eight lines.
    fn two_loop_boundary_payload() -> Vec<u8> {
        let outer = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let hole = [(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)];
        let mut payload = Vec::new();
        payload.extend_from_slice(&5074u32.to_le_bytes());
        payload.extend_from_slice(&6u32.to_le_bytes());
        payload.extend_from_slice(&8u32.to_le_bytes());
        payload.extend_from_slice(&0x0000u16.to_le_bytes());
        payload.extend_from_slice(&40u32.to_le_bytes());
        payload.extend_from_slice(&2u32.to_le_bytes());
        for ring in [outer, hole] {
            payload.extend_from_slice(&4u32.to_le_bytes());
            payload.extend_from_slice(&[2, 1]);
            for index in 0..4 {
                let (from, to) = (ring[index], ring[(index + 1) % 4]);
                payload.push(IGBOUNDARY2D_SEGMENT_TAG);
                for value in [from.0, from.1, to.0, to.1] {
                    payload.extend_from_slice(&f64::to_le_bytes(value));
                }
            }
        }
        for value in [0.5f64, 0.5] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.push(1);
        payload.extend_from_slice(&8u32.to_le_bytes());
        for (index, sub_word) in [5u16, 4, 4, 4, 2, 0, 0, 0].into_iter().enumerate() {
            payload.extend_from_slice(&(1000 + index as u32).to_le_bytes());
            payload.extend_from_slice(&0x00CBu16.to_le_bytes());
            payload.extend_from_slice(&sub_word.to_le_bytes());
        }
        payload
    }

    fn boundary_record(payload: &[u8]) -> Vec<u8> {
        let mut out = PSM_TYPE_CODE_IGBOUNDARY2D.to_le_bytes().to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn igboundary2d_decodes_a_boundary_of_two_loops() {
        let record = boundary_record(&two_loop_boundary_payload());
        let decoded = decode_igboundaries(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let b = &decoded[0];
        assert_eq!((b.oid, b.sheet_layer_ref, b.index), (5074, 8, 40));
        assert_eq!(b.sub_type_word, 0x0000);
        assert_eq!(b.loop_segment_counts, [4, 4]);
        assert_eq!(b.segment_count, 8);
        assert_eq!(b.sub_header_tail, [2, 1]);
        assert_eq!(b.anchor, (0.5, 0.5));
        assert_eq!(b.trailer_flag, 1);
        assert_eq!(
            b.member_refs
                .iter()
                .map(|reference| (
                    reference.member_oid,
                    reference.class_word,
                    reference.sub_word
                ))
                .collect::<Vec<_>>(),
            [
                (1000, 0x00CB, 5),
                (1001, 0x00CB, 4),
                (1002, 0x00CB, 4),
                (1003, 0x00CB, 4),
                (1004, 0x00CB, 2),
                (1005, 0x00CB, 0),
                (1006, 0x00CB, 0),
                (1007, 0x00CB, 0),
            ]
        );
        let loops: Vec<Vec<(f64, f64)>> = b
            .loops()
            .map(|segments| segments.iter().map(|segment| segment.start).collect())
            .collect();
        assert_eq!(
            loops,
            [
                vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
                vec![(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)],
            ]
        );
        // Each loop closes on its own; read as one chain the eight segments
        // would not.
        assert!(b.is_closed_loop(1e-9));
        assert_eq!(
            b.byte_range,
            SHEET_STREAM_HEADER_LEN..SHEET_STREAM_HEADER_LEN + record.len()
        );
    }

    #[test]
    fn igboundary2d_of_two_loops_need_not_name_one_line_a_member() {
        // `XA-A1.igr`'s oid 5076 names 217 lines for 216 members: one more
        // reference than members still closes the payload.
        let mut payload = two_loop_boundary_payload();
        let count_at = payload.len() - 8 * 8 - 4;
        payload[count_at..count_at + 4].copy_from_slice(&9u32.to_le_bytes());
        payload.extend_from_slice(&[0xEC, 0x12, 0, 0, 0xCB, 0, 0, 0]);
        let decoded = decode_igboundary_at(&boundary_record(&payload), 0).expect("decodes");
        assert_eq!(decoded.member_refs.len(), 9);
        assert_eq!(decoded.segment_count, 8);
    }

    #[test]
    fn igboundary2d_of_two_loops_must_close_its_payload() {
        let whole = two_loop_boundary_payload();
        assert!(decode_igboundary_at(&boundary_record(&whole), 0).is_some());
        let refused = |what: &str, payload: Vec<u8>| {
            assert!(
                decode_igboundary_at(&boundary_record(&payload), 0).is_none(),
                "{what}"
            );
        };
        let count_at = whole.len() - 8 * 8 - 4;

        // A reference count the payload does not hold, either way.
        let mut payload = whole.clone();
        payload[count_at..count_at + 4].copy_from_slice(&7u32.to_le_bytes());
        refused("one reference too few is stated", payload);
        let mut payload = whole.clone();
        payload[count_at..count_at + 4].copy_from_slice(&9u32.to_le_bytes());
        refused("one reference too many is stated", payload);
        let mut payload = whole.clone();
        payload[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        refused("a reference count that overflows", payload);

        // Loops the payload does not hold.
        let mut payload = whole.clone();
        payload[18..22].copy_from_slice(&3u32.to_le_bytes());
        refused("three loops stated, two present", payload);
        let mut payload = whole.clone();
        payload[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        refused("a loop count past the payload", payload);
        let mut payload = whole.clone();
        payload[18..22].copy_from_slice(&0u32.to_le_bytes());
        refused("no loop at all", payload);

        // A loop that states no member, or more than it holds.
        let second_loop_at = 22 + 6 + 4 * 33;
        let mut payload = whole.clone();
        payload[second_loop_at..second_loop_at + 4].copy_from_slice(&0u32.to_le_bytes());
        refused("the second loop is empty", payload);
        let mut payload = whole.clone();
        payload[second_loop_at..second_loop_at + 4].copy_from_slice(&5u32.to_le_bytes());
        refused("the second loop states a fifth member", payload);

        // A member tag the template vocabulary does not have.
        let mut payload = whole.clone();
        payload[second_loop_at + 6] = 0x66;
        refused("an unknown member tag", payload);

        // A coordinate or an anchor that is not a number.
        let mut payload = whole.clone();
        payload[29..37].copy_from_slice(&f64::NAN.to_le_bytes());
        refused("a NaN vertex", payload);
        let anchor_at = count_at - 1 - 16;
        let mut payload = whole.clone();
        payload[anchor_at..anchor_at + 8].copy_from_slice(&f64::INFINITY.to_le_bytes());
        refused("an infinite anchor", payload);

        // Every truncation, and the same bytes under the drawing-side
        // sub-type, which states one loop only.
        for cut in 0..whole.len() {
            assert!(
                decode_igboundary_at(&boundary_record(&whole[..cut]), 0).is_none(),
                "cut at {cut}"
            );
        }
        let mut payload = whole.clone();
        payload[12..14].copy_from_slice(&0x0010u16.to_le_bytes());
        refused("two loops under sub-type 0x0010", payload);
    }

    /// Build a linked-template (`0x0000` sub-type) `igBoundary2d` whose
    /// members are one `0x74` arc closed by one `0x67` chord back to the
    /// arc's start: the 18-byte prefix, `u32 1`, the member count, the
    /// `[2, 1]` tail, the members, and the 21-byte trailer.
    fn build_template_boundary_arc_and_chord(
        start_angle: f64,
        end_angle: f64,
        centre: (f64, f64),
        radius: f64,
        direction: u8,
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&111u32.to_le_bytes());
        payload.extend_from_slice(&6u32.to_le_bytes());
        payload.extend_from_slice(&8u32.to_le_bytes());
        payload.extend_from_slice(&0x0000u16.to_le_bytes());
        payload.extend_from_slice(&21u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&[2, 1]);
        payload.push(IGBOUNDARY2D_ARC_TAG);
        for value in [start_angle, end_angle, centre.0, centre.1, radius, 0.0, 1.0] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.push(direction);
        let at = |angle: f64| {
            (
                centre.0 + radius * angle.cos(),
                centre.1 + radius * angle.sin(),
            )
        };
        let (from, to) = (at(end_angle), at(start_angle));
        payload.push(IGBOUNDARY2D_SEGMENT_TAG);
        for value in [from.0, from.1, to.0, to.1] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.extend_from_slice(&[0x5A; 16]);
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.push(1);
        let mut out = PSM_TYPE_CODE_IGBOUNDARY2D.to_le_bytes().to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend(payload);
        out
    }

    /// Angles (radians, counter-clockwise from +X) of every vertex the arc
    /// member flattened to — every segment but the closing chord.
    fn flattened_arc_angles(record: &SheetIgBoundary2dDecoded, centre: (f64, f64)) -> Vec<f64> {
        let arc = &record.segments[..record.segments.len() - 1];
        arc.iter()
            .flat_map(|segment| [segment.start, segment.end])
            .map(|(x, y)| (y - centre.1).atan2(x - centre.0))
            .collect()
    }

    /// A clockwise member that crosses the angle origin runs the short way
    /// across it: `0.172 -> 5.996` clockwise is a 26° arc, the CCHB A2
    /// logo's own oid-111 member, not the 334° the raw difference names.
    #[test]
    fn template_boundary_arc_runs_clockwise_across_the_angle_origin() {
        let centre = (0.394_572, 0.103_313);
        let record =
            build_template_boundary_arc_and_chord(0.172_04, 5.996_42, centre, 0.003_651, 0);
        let decoded = decode_igboundaries(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].sub_type_word, 0x0000);
        assert!(decoded[0].is_closed_loop(1e-12));
        let angles = flattened_arc_angles(&decoded[0], centre);
        // Every vertex sits within the 0.4588 rad wedge around +X.
        let sweep = (0.172_04 - 5.996_42_f64).rem_euclid(std::f64::consts::TAU);
        for angle in &angles {
            assert!(
                *angle <= 0.172_04 + 1e-9 && *angle >= 0.172_04 - sweep - 1e-9,
                "{angle} is off the clockwise arc"
            );
        }
        assert_eq!(
            decoded[0].segments.len(),
            2 + 1,
            "two chords and the closing line"
        );
    }

    /// A counter-clockwise member whose end angle is the smaller number
    /// sweeps forward through the origin: `4.425 -> 0.601` is 141°.
    #[test]
    fn template_boundary_arc_runs_counter_clockwise_through_the_origin() {
        let centre = (0.403_232, 0.100_857);
        let record =
            build_template_boundary_arc_and_chord(4.425_17, 0.601_38, centre, 0.000_096, 1);
        let decoded = decode_igboundaries(&chain_stream(&record));
        assert_eq!(decoded.len(), 1);
        let unwrapped: Vec<f64> = flattened_arc_angles(&decoded[0], centre)
            .into_iter()
            .map(|angle| {
                let turned = (angle - 4.425_17).rem_euclid(std::f64::consts::TAU);
                if turned > std::f64::consts::TAU - 1e-6 {
                    0.0
                } else {
                    turned
                }
            })
            .collect();
        let swept = unwrapped.iter().copied().fold(0.0_f64, f64::max);
        assert!(
            (swept - 2.459_4).abs() < 1e-3,
            "swept {swept} rad, not the 2.4594 the direction byte names"
        );
        assert!(unwrapped.windows(2).all(|pair| pair[1] >= pair[0] - 1e-9));
    }

    /// Two angles a whole turn apart are a full circle in the member's own
    /// direction, not the empty arc a fold to `[0, 2π)` would leave.
    #[test]
    fn template_boundary_arc_keeps_a_whole_turn() {
        use std::f64::consts::TAU;
        assert_eq!(super::igboundary_arc_sweep(0.0, TAU, 1), TAU);
        assert_eq!(super::igboundary_arc_sweep(0.0, TAU, 0), -TAU);
        assert_eq!(super::igboundary_arc_sweep(1.0, 1.0, 1), 0.0);
    }
}
