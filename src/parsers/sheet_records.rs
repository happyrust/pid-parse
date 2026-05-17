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

    SheetCoordinatePageMetadataInvestigationReport {
        candidates,
        coordinate_hint_bounds,
        f64_coordinate_bounds,
        normalized_f64_pair_count,
        page_dimension_scalar_matches,
    }
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

fn normalized_f64_pair_count(bytes: &[u8]) -> usize {
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
            normalized_f64_pair_values(x, y)
        })
        .count()
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
/// `(type_code, bytes_to_follow)` header): 18 bytes of sub-header
/// (`oid` + `parent_ref` + `remaining=12` + `sub_type_word` +
/// `index`) + 32 bytes of `(start.x, start.y, end.x, end.y)`
/// f64-LE = **50 bytes total**.
///
/// Layout revealed by `examples/probe_igline2d_shape.rs` fixture
/// byte dump; see
/// `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
/// section "igLine2d 字节布局已揭示" for the full evidence.
pub const IGLINE2D_PAYLOAD_LEN: usize = 50;

/// Magic constant inside the `igLine2d` payload's sub-header at
/// byte offset 8..11: `remaining_header == 12` (4 bytes for
/// `sub_type_word + index`). Records whose `remaining_header`
/// doesn't equal 12 are rejected as not real `igLine2d`.
const IGLINE2D_REMAINING_HEADER: u32 = 12;

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

/// Constant byte overhead of an `igTextBox` payload: 32-byte
/// sub-header + 36-byte trailing geometry/style data. Total
/// payload size is `68 + text_length * 2` where `text_length` is
/// the number of UTF-16LE chars in the inline text.
pub const IGTEXTBOX_PAYLOAD_OVERHEAD: usize = 68;

/// Maximum `text_length` (UTF-16LE chars) accepted. Real
/// `SmartPlant` fixture texts are short labels (e.g. tag names);
/// cap at 1024 chars to reject obvious noise.
const IGTEXTBOX_MAX_TEXT_LENGTH: u16 = 1024;

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

/// PSM type code for suspected `GraphicGroup` / `GraphicPersist` records.
///
/// Phase 15 probe evidence (`examples/probe_psm_0x00fa_shape.rs` and
/// `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md`)
/// shows these records are standalone variable-size PSM records whose
/// payload starts with `oid`, `parent_ref`, a small kind/count word,
/// and a sub-type discriminator, followed by a reference-like raw tail.
pub const PSM_TYPE_CODE_GRAPHIC_GROUP: u16 = 0x00FA;

/// Minimum `0x00FA` payload size observed in current fixtures.
pub const GRAPHIC_GROUP_MIN_PAYLOAD_LEN: usize = 44;

/// Maximum `0x00FA` payload size accepted by the conservative decoder.
///
/// Current fixtures top out at 200 bytes. The cap leaves room for
/// variant growth while rejecting obvious wide-scan false positives.
const GRAPHIC_GROUP_MAX_PAYLOAD_LEN: usize = 512;

/// PSM type code that identifies a `GLine2d` `PrimitiveLine` record.
///
/// Empirically validated against all three Sheet-bearing fixtures in
/// the project registry (`DWG-0201GP06-01.pid`,
/// `工艺管道及仪表流程-1.pid`, `A01.pid`): every record whose
/// 18-byte PSM header is followed by 48 bytes matching the `GLine2d`
/// validation rules (all-finite doubles, unit direction vector,
/// `param_start < param_end`) carries `type_code == 0x3FE6`. The
/// authoritative `type → GUID` lookup table lives in `SmartPlant`'s
/// `guidtab.h` (named explicitly in `PSMSerializeIn`'s error
/// message `"... OID=%d nType= %d in guidtab.h"`).
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
/// Walk every byte offset; at each offset, check whether an 18-byte
/// PSM header followed by 48 bytes of `GLine2d` payload satisfies
/// all of:
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
/// The decoder is **conservative**: it accepts only records whose
/// inner payload matches the validation rules captured by the
/// reverse-engineered `GLine2d::Validate`. Adversarial bytes pass
/// through without panics; the output is bounded by the input
/// length.
pub fn decode_primitive_lines(data: &[u8]) -> Vec<SheetPrimitiveLineDecoded> {
    let mut out = Vec::new();
    if data.len() < PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (PSM_RECORD_HEADER_LEN + GLINE2D_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_primitive_line_at(data, off) {
            // Advance past the entire record (per bytes_to_follow + 6
            // header overhead) to avoid emitting overlapping records.
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `GLine2d` `PrimitiveLine` starting at
/// `offset` in `data`. Returns `None` when any of the validation
/// rules in [`decode_primitive_lines`] fail. Bounds-checked: passing
/// `offset >= data.len()` or a truncated tail simply returns `None`.
pub fn decode_primitive_line_at(data: &[u8], offset: usize) -> Option<SheetPrimitiveLineDecoded> {
    let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
    let payload_end = header_end.checked_add(GLINE2D_PAYLOAD_LEN)?;
    if payload_end > data.len() {
        return None;
    }

    // PSM header.
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_GLINE2D {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let bytes_to_follow_usize = bytes_to_follow as usize;
    // bytes_to_follow must cover at least the 48-byte payload.
    if bytes_to_follow_usize < GLINE2D_PAYLOAD_LEN {
        return None;
    }
    // The full record (header + bytes_to_follow trailer) must fit.
    let record_end = offset.checked_add(6 + bytes_to_follow_usize)?;
    if record_end > data.len() {
        return None;
    }
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
///   0..3   u32 LE   oid
///   4..7   u32 LE   parent_ref
///   8..11  u32 LE   remaining_header == 12 (constant)
///   12..13 u16 LE   sub_type_word
///   14..17 u32 LE   index / sub_oid
///   18..25 f64 LE   start.x
///   26..33 f64 LE   start.y
///   34..41 f64 LE   end.x
///   42..49 f64 LE   end.y
/// ```
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
    /// Parent reference (payload bytes 4..7) — typically the
    /// `PrimitiveCluster` or owning entity. Semantic precise meaning
    /// is hypothesis; the byte field is reliably present.
    pub parent_ref: u32,
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
/// Walk every byte offset; at each offset check whether a 6-byte
/// `(type_code, bytes_to_follow)` PSM header is followed by 50
/// bytes of `igLine2d` payload satisfying all of:
///
/// 1. PSM `type_code` == [`PSM_TYPE_CODE_IGLINE2D`] (`0x0018`);
/// 2. `bytes_to_follow` == 50 (igLine2d records are fixed-size);
/// 3. Payload `remaining_header` field (bytes 8..11) ==
///    `IGLINE2D_REMAINING_HEADER` (`12`);
/// 4. All four geometry `f64`s are finite and in domain
///    `[-1e9, 1e9]`;
/// 5. `start != end` (line must be non-degenerate; collapse
///    rejected to avoid noise).
///
/// Panic-free and bounds-checked: adversarial bytes simply fail
/// validation and are skipped.
pub fn decode_iglines(data: &[u8]) -> Vec<SheetIgLine2dDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + IGLINE2D_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (6 + IGLINE2D_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_igline_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `igLine2d` record starting at
/// `offset` in `data`. Returns `None` when any of the validation
/// rules in [`decode_iglines`] fail. Bounds-checked; tolerant of
/// truncated input.
pub fn decode_igline_at(data: &[u8], offset: usize) -> Option<SheetIgLine2dDecoded> {
    let header_end = offset.checked_add(6)?;
    let payload_end = header_end.checked_add(IGLINE2D_PAYLOAD_LEN)?;
    if payload_end > data.len() {
        return None;
    }

    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_IGLINE2D {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if bytes_to_follow as usize != IGLINE2D_PAYLOAD_LEN {
        return None;
    }

    let payload = data.get(header_end..payload_end)?;
    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let remaining_header = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    if remaining_header != IGLINE2D_REMAINING_HEADER {
        return None;
    }
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
pub fn decode_iglinestrings(data: &[u8]) -> Vec<SheetIgLineString2dDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + IGLINESTRING2D_MIN_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (6 + IGLINESTRING2D_MIN_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_iglinestring_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `igLineString2d` record starting at
/// `offset`. Returns `None` when any validation rule in
/// [`decode_iglinestrings`] fails. Bounds-checked and panic-free.
pub fn decode_iglinestring_at(data: &[u8], offset: usize) -> Option<SheetIgLineString2dDecoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_IGLINESTRING2D {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let btf = bytes_to_follow as usize;
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

    let payload_end = header_end.checked_add(btf)?;
    if payload_end > data.len() {
        return None;
    }
    let payload = data.get(header_end..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
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
pub fn decode_igpoints(data: &[u8]) -> Vec<SheetIgPoint2dDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + IGPOINT2D_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (6 + IGPOINT2D_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_igpoint_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `igPoint2d` record starting at
/// `offset`. Returns `None` on validation failure.
pub fn decode_igpoint_at(data: &[u8], offset: usize) -> Option<SheetIgPoint2dDecoded> {
    let header_end = offset.checked_add(6)?;
    let payload_end = header_end.checked_add(IGPOINT2D_PAYLOAD_LEN)?;
    if payload_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_IGPOINT2D {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if bytes_to_follow as usize != IGPOINT2D_PAYLOAD_LEN {
        return None;
    }

    let payload = data.get(header_end..payload_end)?;
    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
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
/// + `68 + text_length * 2`-byte payload):
///
/// ```text
/// PSM header (6 bytes):
///   0..1   u16   type_code = 0x004D
///   2..5   u32   bytes_to_follow = 68 + text_length * 2
///
/// Payload (68 + text_length * 2 bytes):
///   0..3    u32   oid
///   4..7    u32   parent_ref
///   8..11   u32   remaining_header
///   12..13  u16   sub_type_word
///   14..17  u32   index
///   18..29  12 bytes  sub-fields (length flags + sub-index)
///   30..31  u16   text_length (UTF-16LE chars, redundant with
///                 bytes_to_follow-derived length)
///   32..    UTF-16LE chars × text_length × 2 bytes
///   then    24 bytes  3 × f64 (insertion point + scale or similar)
///   then    12 bytes  trailer
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
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// Index / sub-oid.
    pub index: u32,
    /// Inline text length (UTF-16LE chars).
    pub text_length: u16,
    /// Decoded text content (lossy UTF-16LE → UTF-8 conversion).
    pub text: String,
    /// First trailing f64 triple (presumed `insertion.x`).
    pub trailing_double_1: f64,
    /// Second trailing f64 (`insertion.y`).
    pub trailing_double_2: f64,
    /// Third trailing f64 (often `1.0` — scale or marker).
    pub trailing_double_3: f64,
}

/// Decode every PSM-encoded `igTextBox` record in a `Sheet*` stream.
///
/// Validation:
/// 1. `type_code == 0x004D`;
/// 2. `bytes_to_follow >= 68` and consistent `text_length`;
/// 3. `(bytes_to_follow - 68) % 2 == 0` and the resulting derived
///    `text_length` matches the inline `text_length` at payload
///    offset 30;
/// 4. `text_length <= 1024` (reject obvious noise);
/// 5. 3 trailing doubles finite + in domain.
pub fn decode_igtextboxes(data: &[u8]) -> Vec<SheetIgTextBoxDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + IGTEXTBOX_PAYLOAD_OVERHEAD {
        return out;
    }
    let max_offset = data.len() - (6 + IGTEXTBOX_PAYLOAD_OVERHEAD);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_igtextbox_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `igTextBox` record starting at
/// `offset`. Returns `None` on validation failure.
pub fn decode_igtextbox_at(data: &[u8], offset: usize) -> Option<SheetIgTextBoxDecoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_IGTEXTBOX {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let btf = bytes_to_follow as usize;
    if btf < IGTEXTBOX_PAYLOAD_OVERHEAD {
        return None;
    }
    if !(btf - IGTEXTBOX_PAYLOAD_OVERHEAD).is_multiple_of(2) {
        return None;
    }
    let derived_text_len_words = ((btf - IGTEXTBOX_PAYLOAD_OVERHEAD) / 2) as u16;
    if derived_text_len_words > IGTEXTBOX_MAX_TEXT_LENGTH {
        return None;
    }

    let payload_end = header_end.checked_add(btf)?;
    if payload_end > data.len() {
        return None;
    }
    let payload = data.get(header_end..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);
    let index = u32::from_le_bytes([payload[14], payload[15], payload[16], payload[17]]);
    // Inline text_length at payload offset 30 (u16).
    let inline_text_length = u16::from_le_bytes([payload[30], payload[31]]);
    if inline_text_length != derived_text_len_words {
        return None;
    }

    // UTF-16LE text starts at payload offset 32.
    let text_byte_len = (inline_text_length as usize) * 2;
    let text_end = 32 + text_byte_len;
    if text_end + 36 > payload.len() {
        return None;
    }
    let mut u16_chars = Vec::with_capacity(inline_text_length as usize);
    for i in 0..inline_text_length as usize {
        let pos = 32 + i * 2;
        u16_chars.push(u16::from_le_bytes([payload[pos], payload[pos + 1]]));
    }
    let text = String::from_utf16_lossy(&u16_chars);

    // 3 trailing doubles immediately after the text.
    let mut trailing = [0f64; 3];
    for (i, slot) in trailing.iter_mut().enumerate() {
        let pos = text_end + i * 8;
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

    Some(SheetIgTextBoxDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sub_type_word,
        index,
        text_length: inline_text_length,
        text,
        trailing_double_1: trailing[0],
        trailing_double_2: trailing[1],
        trailing_double_3: trailing[2],
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
///   14..39  26 bytes  sub-fields (flags, references, sub-IDs)
///   40..47  f64   transform[0] (typically scale_x or rotation_cos)
///   48..55  f64   transform[1]
///   56..63  f64   transform[2]
///   64..71  f64   transform[3]
///   72..79  f64   insertion.x
///   80..87  f64   insertion.y
///   88..    variable tail (symbol library ref + class ID + flags)
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
    /// Sub-type discriminator.
    pub sub_type_word: u16,
    /// 4-element `f64` transform matrix at payload offsets 40..71.
    /// Without `radsrvitem.dll` decompile this is named generically;
    /// for canonical un-rotated/un-scaled symbols it's
    /// `[1.0, 0.0, 0.0, 1.0]`.
    pub transform: [f64; 4],
    /// Insertion point (extracted from payload offsets 72..87).
    pub insertion: (f64, f64),
}

/// Decode every PSM-encoded `igSymbol2d` record.
///
/// Validation:
/// 1. `type_code == 0x00CE`;
/// 2. `bytes_to_follow ∈ [113, 200]`;
/// 3. 6 doubles at payload offsets 40..87 finite + in domain.
pub fn decode_igsymbols(data: &[u8]) -> Vec<SheetIgSymbol2dDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + IGSYMBOL2D_MIN_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (6 + IGSYMBOL2D_MIN_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_igsymbol_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `igSymbol2d` record starting at
/// `offset`. Returns `None` on validation failure.
pub fn decode_igsymbol_at(data: &[u8], offset: usize) -> Option<SheetIgSymbol2dDecoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_IGSYMBOL2D {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let btf = bytes_to_follow as usize;
    if !(IGSYMBOL2D_MIN_PAYLOAD_LEN..=IGSYMBOL2D_MAX_PAYLOAD_LEN).contains(&btf) {
        return None;
    }
    let payload_end = header_end.checked_add(btf)?;
    if payload_end > data.len() {
        return None;
    }
    let payload = data.get(header_end..payload_end)?;

    let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let parent_ref = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let sub_type_word = u16::from_le_bytes([payload[12], payload[13]]);

    // Read 6 doubles starting at payload offset 40: 4 for transform,
    // 2 for insertion.
    let mut doubles = [0f64; 6];
    for (i, slot) in doubles.iter_mut().enumerate() {
        let pos = 40 + i * 8;
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

    Some(SheetIgSymbol2dDecoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        oid,
        parent_ref,
        sub_type_word,
        transform: [doubles[0], doubles[1], doubles[2], doubles[3]],
        insertion: (doubles[4], doubles[5]),
    })
}

// ---------------------------------------------------------------------------
// Phase 15 Slice C: PSM-encoded GraphicGroup / GraphicPersist decoder
// ---------------------------------------------------------------------------

/// One decoded PSM `0x00FA` `GraphicGroup` / `GraphicPersist` record.
///
/// This DTO intentionally exposes only the stable header fields proven
/// by fixture byte dumps. The variable tail is retained as raw bytes
/// because candidate child-OID extraction is still an audit-layer
/// hypothesis, not a stable schema contract.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetGraphicGroupDecoded {
    /// Byte range covering the full PSM record.
    pub byte_range: std::ops::Range<usize>,
    /// PSM 14-bit type code. Always [`PSM_TYPE_CODE_GRAPHIC_GROUP`].
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

/// Decode every conservative PSM `0x00FA` `GraphicGroup` record.
///
/// Validation:
/// 1. `type_code == 0x00FA` and type flags are zero;
/// 2. `bytes_to_follow` is even and in `[44, 512]`;
/// 3. payload exists, `oid != 0`, `parent_ref == 6`;
/// 4. payload bytes 8..13 are zero in the current fixture family;
/// 5. `group_kind_word` is a small non-zero discriminator.
pub fn decode_graphic_groups(data: &[u8]) -> Vec<SheetGraphicGroupDecoded> {
    let mut out = Vec::new();
    if data.len() < 6 + GRAPHIC_GROUP_MIN_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (6 + GRAPHIC_GROUP_MIN_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_graphic_group_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode one conservative PSM `0x00FA` `GraphicGroup` record at
/// `offset`. Returns `None` on validation failure.
pub fn decode_graphic_group_at(data: &[u8], offset: usize) -> Option<SheetGraphicGroupDecoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_GRAPHIC_GROUP {
        return None;
    }
    let type_flags = type_word >> 14;
    if type_flags != 0 {
        return None;
    }
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let btf = bytes_to_follow as usize;
    if !(GRAPHIC_GROUP_MIN_PAYLOAD_LEN..=GRAPHIC_GROUP_MAX_PAYLOAD_LEN).contains(&btf) {
        return None;
    }
    if !btf.is_multiple_of(2) {
        return None;
    }
    let payload_end = header_end.checked_add(btf)?;
    if payload_end > data.len() {
        return None;
    }
    let payload = data.get(header_end..payload_end)?;
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

    Some(SheetGraphicGroupDecoded {
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
    let mut out = Vec::new();
    if data.len() < PSM_RECORD_HEADER_LEN + JSTYLE_OVERRIDE_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (PSM_RECORD_HEADER_LEN + JSTYLE_OVERRIDE_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_jstyle_override_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
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
pub fn decode_jstyle_override_at(data: &[u8], offset: usize) -> Option<SheetJStyleOverrideDecoded> {
    let header_end = offset.checked_add(PSM_RECORD_HEADER_LEN)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_JSTYLE_OVERRIDE {
        return None;
    }
    let type_flags = type_word >> 14;
    if type_flags != 0 {
        return None;
    }
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if !(JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW..=JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW)
        .contains(&bytes_to_follow)
    {
        return None;
    }
    let btf_usize = bytes_to_follow as usize;
    let record_end = offset
        .checked_add(6)
        .and_then(|p| p.checked_add(btf_usize))?;
    if record_end > data.len() {
        return None;
    }
    let oid = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);

    let payload_start = offset + PSM_RECORD_HEADER_LEN;
    let payload_end = payload_start + JSTYLE_OVERRIDE_PAYLOAD_LEN;
    if payload_end > data.len() {
        return None;
    }
    let payload = data.get(payload_start..payload_end)?;

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
// sub-kind discriminator, this decoder follows the Phase 15 GraphicGroup
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
    let mut out = Vec::new();
    let min_record_len = 6usize.saturating_add(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW as usize);
    if data.len() < min_record_len {
        return out;
    }
    let max_offset = data.len() - min_record_len;
    let mut off = 0usize;
    while off <= max_offset {
        if let Some(decoded) = decode_sub_record_0x0010_at(data, off) {
            let advance = (decoded.byte_range.end - off).max(1);
            out.push(decoded);
            off = off.saturating_add(advance);
            continue;
        }
        off += 1;
    }
    out
}

/// Try to decode a single PSM `0x0010` sub-record starting at `offset`
/// in `data`. Returns `None` when any validation rule from
/// [`decode_sub_records_0x0010`] fails. Bounds-checked: passing
/// `offset >= data.len()` or a truncated tail returns `None`.
pub fn decode_sub_record_0x0010_at(
    data: &[u8],
    offset: usize,
) -> Option<SheetSubRecord0x0010Decoded> {
    let header_end = offset.checked_add(6)?;
    if header_end > data.len() {
        return None;
    }
    let header = data.get(offset..header_end)?;
    let type_word = u16::from_le_bytes([header[0], header[1]]);
    let type_code = type_word & 0x3FFF;
    if type_code != PSM_TYPE_CODE_SUB_RECORD_0X0010 {
        return None;
    }
    let type_flags = type_word >> 14;
    let bytes_to_follow = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    if !(SUB_RECORD_0X0010_MIN_BYTES_TO_FOLLOW..=SUB_RECORD_0X0010_MAX_BYTES_TO_FOLLOW)
        .contains(&bytes_to_follow)
    {
        return None;
    }
    let btf = bytes_to_follow as usize;
    let payload_end = header_end.checked_add(btf)?;
    if payload_end > data.len() {
        return None;
    }
    let raw_payload = data.get(header_end..payload_end)?.to_vec();
    Some(SheetSubRecord0x0010Decoded {
        byte_range: offset..payload_end,
        type_code,
        type_flags,
        bytes_to_follow,
        raw_payload,
    })
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

    /// Build a single synthetic PSM `GLine2d` record:
    /// 18-byte header (type=0x3FE6, `bytes_to_follow`=48, oid=`oid`)
    /// + 6×f64 `GLine2d` payload (origin, direction, params).
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
        // bytes_to_follow = 48 (just the payload, no attribute tail).
        out.extend_from_slice(&(GLINE2D_PAYLOAD_LEN as u32).to_le_bytes());
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

    #[test]
    fn primitive_line_decodes_canonical_synthetic_record() {
        // Canonical synthetic line: origin at (0,0), unit horizontal
        // direction, params [0, 1.0]. Endpoints A=(0,0) B=(1,0).
        let record = build_synthetic_gline2d_record(42, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        let decoded = decode_primitive_lines(&record);
        assert_eq!(decoded.len(), 1, "expected exactly one decoded line");
        let line = &decoded[0];
        assert_eq!(line.type_code, PSM_TYPE_CODE_GLINE2D);
        assert_eq!(line.type_flags, 0);
        assert_eq!(line.bytes_to_follow, 48);
        assert_eq!(line.oid, 42);
        assert_eq!(line.origin, (0.0, 0.0));
        assert!((line.direction.0 - 1.0).abs() < 1e-12);
        assert!(line.direction.1.abs() < 1e-12);
        assert_eq!(line.param_start, 0.0);
        assert_eq!(line.param_end, 1.0);
        assert_eq!(line.byte_range.start, 0);
        // byte_range covers header (6 prefix) + bytes_to_follow (48).
        assert_eq!(line.byte_range.end, 6 + 48);
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
        let decoded = decode_primitive_lines(&record);
        assert!(decoded.is_empty(), "wrong type_code must be rejected");
    }

    #[test]
    fn primitive_line_rejects_non_unit_direction() {
        // direction = (2.0, 0.0): length 2, not unit.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (2.0, 0.0), 0.0, 1.0);
        assert!(
            decode_primitive_lines(&record).is_empty(),
            "non-unit direction vector must be rejected"
        );
    }

    #[test]
    fn primitive_line_rejects_zero_direction() {
        // direction = (0.0, 0.0): zero vector.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (0.0, 0.0), 0.0, 1.0);
        assert!(
            decode_primitive_lines(&record).is_empty(),
            "zero direction vector must be rejected"
        );
    }

    #[test]
    fn primitive_line_rejects_reversed_param_range() {
        // param_start == param_end.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.0, 1.0);
        assert!(
            decode_primitive_lines(&record).is_empty(),
            "param_start >= param_end must be rejected"
        );
        // param_start > param_end.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 1.5, 0.5);
        assert!(
            decode_primitive_lines(&record).is_empty(),
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
            decode_primitive_lines(&record).is_empty(),
            "NaN coordinate must be rejected"
        );
    }

    #[test]
    fn primitive_line_decoder_is_panic_safe_on_truncated_input() {
        // Build a complete record, then truncate at various sizes.
        let record = build_synthetic_gline2d_record(1, (0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
        for trunc_len in 0..record.len() {
            // Must not panic, must return empty / no decoded line.
            let decoded = decode_primitive_lines(&record[..trunc_len]);
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
        let mut data = build_synthetic_gline2d_record(7, (0.10, 0.10), (1.0, 0.0), 0.0, 0.5);
        data.extend(build_synthetic_gline2d_record(
            8,
            (0.20, 0.30),
            (0.0, 1.0),
            0.0,
            0.8,
        ));
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
    /// 6-byte PSM header + 50-byte payload.
    fn build_synthetic_igline2d_record(
        oid: u32,
        parent_ref: u32,
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
        out.extend_from_slice(&IGLINE2D_REMAINING_HEADER.to_le_bytes());
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
        let decoded = decode_iglines(&record);
        assert_eq!(decoded.len(), 1);
        let line = &decoded[0];
        assert_eq!(line.type_code, PSM_TYPE_CODE_IGLINE2D);
        assert_eq!(line.bytes_to_follow, 50);
        assert_eq!(line.oid, 177);
        assert_eq!(line.parent_ref, 1212);
        assert_eq!(line.sub_type_word, 0x0010);
        assert_eq!(line.index, 86);
        assert!((line.start.0 - 0.4719).abs() < 1e-9);
        assert!((line.start.1 - 0.3897).abs() < 1e-9);
        assert!((line.end.0 - 0.5736).abs() < 1e-9);
        assert!((line.end.1 - 0.3897).abs() < 1e-9);
        assert_eq!(line.byte_range.start, 0);
        assert_eq!(line.byte_range.end, 6 + 50);
        assert!((line.length() - 0.1017).abs() < 1e-3);
    }

    #[test]
    fn igline2d_rejects_wrong_type_code() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        record[0] = 0xE6;
        record[1] = 0x3F;
        assert!(decode_iglines(&record).is_empty());
    }

    #[test]
    fn igline2d_rejects_wrong_bytes_to_follow() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        // Overwrite bytes_to_follow with 49 (not exactly 50).
        record[2] = 49;
        assert!(decode_iglines(&record).is_empty());
    }

    #[test]
    fn igline2d_rejects_wrong_remaining_header() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 0.0));
        // Overwrite remaining_header at payload offset 8 (record offset 14).
        record[6 + 8] = 11; // change 0x0C to 0x0B
        assert!(decode_iglines(&record).is_empty());
    }

    #[test]
    fn igline2d_rejects_degenerate_zero_length_line() {
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.5, 0.5), (0.5, 0.5));
        assert!(decode_iglines(&record).is_empty());
    }

    #[test]
    fn igline2d_rejects_nan_coordinate() {
        let mut record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (0.0, 0.0), (1.0, 1.0));
        let nan_bytes = f64::NAN.to_le_bytes();
        // start.x is at record offset 6 + 18 = 24.
        record[24..32].copy_from_slice(&nan_bytes);
        assert!(decode_iglines(&record).is_empty());
    }

    #[test]
    fn igline2d_rejects_out_of_domain_coordinate() {
        let record = build_synthetic_igline2d_record(1, 1, 0x10, 0, (1e10, 0.0), (1.0, 1.0));
        assert!(decode_iglines(&record).is_empty());
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
        let mut data = build_synthetic_igline2d_record(7, 100, 0x10, 1, (0.1, 0.1), (0.2, 0.1));
        data.extend(build_synthetic_igline2d_record(
            8,
            100,
            0x65,
            2,
            (0.3, 0.3),
            (0.3, 0.5),
        ));
        let decoded = decode_iglines(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 7);
        assert_eq!(decoded[1].oid, 8);
        assert_eq!(decoded[0].sub_type_word, 0x10);
        assert_eq!(decoded[1].sub_type_word, 0x65);
        assert!(decoded[0].byte_range.end <= decoded[1].byte_range.start);
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
                                                    // 18..30: 12 bytes of sub-fields (we just zero them; decoder
                                                    // doesn't validate these bytes).
        out.extend_from_slice(&[0u8; 12]);
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

    #[test]
    fn igtextbox_rejects_inconsistent_text_length() {
        let mut record = build_synthetic_igtextbox_record("HELLO", 1, 1);
        // Overwrite inline text_length at payload offset 30 (record offset 6+30=36).
        record[36..38].copy_from_slice(&999u16.to_le_bytes());
        assert!(decode_igtextboxes(&record).is_empty());
    }

    #[test]
    fn igtextbox_rejects_zero_length_overhead_violation() {
        // bytes_to_follow < 68 (constant overhead) is rejected.
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
        transform: [f64; 4],
        insertion: (f64, f64),
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + IGSYMBOL2D_MIN_PAYLOAD_LEN);
        out.extend_from_slice(&PSM_TYPE_CODE_IGSYMBOL2D.to_le_bytes());
        out.extend_from_slice(&(IGSYMBOL2D_MIN_PAYLOAD_LEN as u32).to_le_bytes());
        // Payload bytes 0..40: oid + parent_ref + 8B remaining +
        // 2B sub_type + 26 bytes of sub-fields (zeroed)
        out.extend_from_slice(&oid.to_le_bytes()); // 0..4
        out.extend_from_slice(&parent_ref.to_le_bytes()); // 4..8
        out.extend_from_slice(&8u32.to_le_bytes()); // 8..12 remaining_header
        out.extend_from_slice(&0x0010u16.to_le_bytes()); // 12..14 sub_type
        out.extend_from_slice(&[0u8; 26]); // 14..40 sub-fields
                                           // 40..72: 4 transform doubles
        for v in &transform {
            out.extend_from_slice(&v.to_le_bytes());
        }
        // 72..88: insertion x, y
        out.extend_from_slice(&insertion.0.to_le_bytes());
        out.extend_from_slice(&insertion.1.to_le_bytes());
        // 88..113: 25 bytes of trailing (symbol library ref + flags)
        out.extend_from_slice(&[0u8; 25]);
        out
    }

    #[test]
    fn igsymbol2d_decodes_canonical_unrotated_symbol() {
        let record = build_synthetic_igsymbol2d_record(
            500,
            6,
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
        assert_eq!(s.transform, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(s.insertion, (0.3, 0.4));
    }

    #[test]
    fn igsymbol2d_rejects_wrong_type_code() {
        let mut record = build_synthetic_igsymbol2d_record(1, 1, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
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
        let mut record = build_synthetic_igsymbol2d_record(1, 1, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
        // transform[0] at record offset 6 + 40 = 46
        record[46..54].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_igsymbols(&record).is_empty());
    }

    #[test]
    fn igsymbol2d_decoder_is_panic_safe_on_short_input() {
        let record = build_synthetic_igsymbol2d_record(1, 1, [1.0, 0.0, 0.0, 1.0], (0.5, 0.5));
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
    // Phase 15 Slice C: PSM 0x00FA GraphicGroup decoder tests
    // -----------------------------------------------------------------

    fn build_synthetic_graphic_group_record(
        oid: u32,
        parent_ref: u32,
        group_kind_word: u16,
        sub_type_word: u16,
        raw_reference_payload: &[u8],
    ) -> Vec<u8> {
        let bytes_to_follow = 18 + raw_reference_payload.len();
        assert!(bytes_to_follow >= GRAPHIC_GROUP_MIN_PAYLOAD_LEN);
        let mut out = Vec::with_capacity(6 + bytes_to_follow);
        out.extend_from_slice(&PSM_TYPE_CODE_GRAPHIC_GROUP.to_le_bytes());
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
    fn graphic_group_decodes_canonical_header_and_raw_tail() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);

        let decoded = decode_graphic_groups(&record);
        assert_eq!(decoded.len(), 1);
        let group = &decoded[0];
        assert_eq!(group.byte_range, 0..record.len());
        assert_eq!(group.type_code, PSM_TYPE_CODE_GRAPHIC_GROUP);
        assert_eq!(group.type_flags, 0);
        assert_eq!(group.bytes_to_follow, GRAPHIC_GROUP_MIN_PAYLOAD_LEN as u32);
        assert_eq!(group.oid, 42);
        assert_eq!(group.parent_ref, 6);
        assert_eq!(group.group_kind_word, 2);
        assert_eq!(group.sub_type_word, 0x01A1);
        assert_eq!(group.raw_reference_payload, raw_tail);
    }

    #[test]
    fn graphic_group_rejects_wrong_type_code() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        record[0] = 0x18;
        record[1] = 0x00;

        assert!(decode_graphic_groups(&record).is_empty());
    }

    #[test]
    fn graphic_group_rejects_nonzero_type_flags() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        let flagged_type = PSM_TYPE_CODE_GRAPHIC_GROUP | 0x4000;
        record[0..2].copy_from_slice(&flagged_type.to_le_bytes());

        assert!(decode_graphic_groups(&record).is_empty());
    }

    #[test]
    fn graphic_group_rejects_invalid_size_and_truncation() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        record[2..6].copy_from_slice(&43u32.to_le_bytes());
        assert!(decode_graphic_groups(&record).is_empty());

        let record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        assert!(decode_graphic_groups(&record[..record.len() - 1]).is_empty());
    }

    #[test]
    fn graphic_group_rejects_invalid_header_fields() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        assert!(decode_graphic_groups(&build_synthetic_graphic_group_record(
            0, 6, 2, 0x01A1, &raw_tail
        ))
        .is_empty());
        assert!(decode_graphic_groups(&build_synthetic_graphic_group_record(
            42, 7, 2, 0x01A1, &raw_tail
        ))
        .is_empty());
        assert!(decode_graphic_groups(&build_synthetic_graphic_group_record(
            42, 6, 0, 0x01A1, &raw_tail
        ))
        .is_empty());
    }

    #[test]
    fn graphic_group_rejects_nonzero_reserved_prefix() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let mut record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        record[6 + 8] = 1;

        assert!(decode_graphic_groups(&record).is_empty());
    }

    #[test]
    fn graphic_group_decoder_is_panic_safe_on_short_input() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let record = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        for trunc_len in 0..record.len() {
            assert!(decode_graphic_groups(&record[..trunc_len]).is_empty());
        }
        assert!(decode_graphic_groups(&[]).is_empty());
    }

    #[test]
    fn graphic_group_decoder_is_panic_safe_on_random_noise() {
        let noise: Vec<u8> = (0..4096).map(|i| (i & 0xFF) as u8).collect();
        let _ = decode_graphic_groups(&noise);
        assert!(decode_graphic_groups(&vec![0u8; 4096]).is_empty());
        assert!(decode_graphic_groups(&vec![0xFFu8; 4096]).is_empty());
    }

    #[test]
    fn graphic_group_decodes_two_back_to_back_records() {
        let raw_tail = vec![0x01; GRAPHIC_GROUP_MIN_PAYLOAD_LEN - 18];
        let first = build_synthetic_graphic_group_record(42, 6, 2, 0x01A1, &raw_tail);
        let second = build_synthetic_graphic_group_record(43, 6, 1, 0x00B8, &raw_tail);
        let mut data = first;
        data.extend_from_slice(&second);

        let decoded = decode_graphic_groups(&data);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].oid, 42);
        assert_eq!(decoded[1].oid, 43);
    }

    #[test]
    fn primitive_line_byte_range_covers_full_record_when_attribute_tail_present() {
        // Build a record with bytes_to_follow = 48 (payload) + 200
        // (mock attribute tail).
        let mut record = Vec::new();
        let type_word: u16 = PSM_TYPE_CODE_GLINE2D;
        record.extend_from_slice(&type_word.to_le_bytes());
        // bytes_to_follow = 48 + 200 = 248.
        record.extend_from_slice(&248u32.to_le_bytes());
        // oid
        record.extend_from_slice(&123u32.to_le_bytes());
        // 8-byte aux
        record.extend_from_slice(&[0u8; 8]);
        // GLine2d payload
        for v in [0.0f64, 0.0, 1.0, 0.0, 0.0, 1.0] {
            record.extend_from_slice(&v.to_le_bytes());
        }
        // 200 bytes of mock attribute tail
        record.extend_from_slice(&[0xAB; 200]);

        let decoded = decode_primitive_lines(&record);
        assert_eq!(decoded.len(), 1);
        let line = &decoded[0];
        assert_eq!(line.byte_range.start, 0);
        // byte_range covers full record: 6 (type+bytes_to_follow) + 248.
        assert_eq!(line.byte_range.end, 6 + 248);
        assert_eq!(line.bytes_to_follow, 248);
    }

    // -----------------------------------------------------------------
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
}
