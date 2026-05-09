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
}
