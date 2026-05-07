//! Byte-chunk probe for `Sheet*` streams.
//!
//! Status: experimental / evidence-only. This probe does NOT attempt any
//! semantic decoding of sheet records or geometry — it just turns an
//! opaque byte stream into a sequence of candidate "chunks" with byte
//! ranges, so downstream tooling (or a human reverse engineer) can work
//! with a smaller, explainable surface.
//!
//! Chunk byte ranges (`start..end`) are **deliberately field-compatible**
//! with [`crate::writer::plan::SheetChunkPatch`]. The intent is that a
//! future `probe → patch` helper can copy `start`/`end` verbatim when
//! composing surgical byte-range edits via
//! [`crate::writer::PidWriter::write_to`]; this module itself writes
//! nothing and does not depend on the writer.
//!
//! Current heuristics:
//! - zero runs (>= `zero_run_threshold` consecutive 0 bytes)
//! - ASCII printable bursts (>= `ascii_burst_threshold` chars)
//! - UTF-16LE printable bursts (>= `utf16_burst_threshold` chars)
//! - repeated aligned u32 (4 equal dwords in a row)
//! - "offset-like" monotonic u32 sequences (4 strictly ascending dwords
//!   whose values stay inside the stream's length)
//!
//! [`BoundaryReason::Alignment8`] and [`BoundaryReason::MarkerTransition`]
//! are reserved for future heuristics and are not yet emitted.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Tunable thresholds for the sheet-probe heuristics; all downstream
/// functions read their cut-offs from this struct. [`Default`] values
/// match the original Phase 8 hand-tuning against `SmartPlant` fixtures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetProbeOptions {
    /// Chunks shorter than this (in bytes) are discarded by
    /// [`split_by_boundaries`]; prevents cutting the stream into
    /// near-empty slices.
    pub min_chunk_len: usize,
    /// Upper bound on how many ASCII / UTF-16 runs
    /// [`summarize_chunk`] keeps in each chunk's preview.
    pub max_preview_strings: usize,
    /// Minimum run length (in bytes) that counts as a "zero run"
    /// boundary hint.
    pub zero_run_threshold: usize,
    /// Minimum consecutive ASCII-printable bytes that count as an
    /// "ASCII burst" boundary hint.
    pub ascii_burst_threshold: usize,
    /// Minimum consecutive UTF-16LE printable code units that count
    /// as a "UTF-16 burst" boundary hint.
    pub utf16_burst_threshold: usize,
    /// Minimum cumulative score a boundary must gather before
    /// [`split_by_boundaries`] will actually cut the stream there.
    pub min_boundary_score: u32,
}

impl Default for SheetProbeOptions {
    fn default() -> Self {
        Self {
            min_chunk_len: 32,
            max_preview_strings: 8,
            zero_run_threshold: 8,
            ascii_burst_threshold: 6,
            utf16_burst_threshold: 4,
            min_boundary_score: 3,
        }
    }
}

/// End-to-end result of [`probe_sheet_stream`] for a single sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetProbeReport {
    /// Local CFB name of the sheet (e.g. `"Sheet6"`).
    pub sheet_name: String,
    /// Full `/`-joined CFB path of the sheet stream.
    pub path: String,
    /// Byte length of the stream that was probed.
    pub size: u64,
    /// Every boundary the heuristics voted for, merged by offset and
    /// sorted ascending. Unrelated to [`Self::chunks`] — this is the
    /// raw per-heuristic view, `chunks` is the post-threshold slicing.
    pub candidate_boundaries: Vec<CandidateBoundary>,
    /// Chunks emitted by [`split_by_boundaries`] after applying
    /// `min_chunk_len` / `min_boundary_score` filtering.
    pub chunks: Vec<SheetChunk>,
    /// Candidate record-type frequencies observed immediately after
    /// `0x89` marker bytes. Keys are uppercase hex (`"0x00CE"`).
    /// This is evidence-only; it does not claim a stable record schema.
    pub record_type_counts: BTreeMap<String, usize>,
    /// Printable text runs with offsets in the original Sheet stream.
    /// Unlike per-chunk previews, this is a report-wide index for
    /// reverse-engineering labels and annotation payloads.
    pub text_runs: Vec<SheetTextRun>,
    /// Plausible adjacent `(x, y)` integer pairs found on 4-byte
    /// alignment. These are coordinate hints, not confirmed geometry.
    pub coordinate_hints: Vec<SheetCoordinateHint>,
}

/// One printable text run found in a Sheet stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetTextRun {
    /// Byte offset where the run begins inside the Sheet stream.
    pub offset: usize,
    /// Encoding family used to decode [`Self::text`].
    pub encoding: SheetTextEncoding,
    /// Decoded printable text.
    pub text: String,
    /// Number of bytes consumed by the run in the original stream.
    pub byte_len: usize,
}

/// Encoding family for a [`SheetTextRun`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetTextEncoding {
    /// Single-byte printable ASCII.
    Ascii,
    /// Little-endian UTF-16 printable text.
    Utf16Le,
}

/// Investigation-only candidate linking a text run to a nearby coordinate hint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetTextWindowCandidate {
    /// Byte offset where the text run begins.
    pub text_offset: usize,
    /// Number of bytes consumed by the text run.
    pub text_byte_len: usize,
    /// Encoding family used by the text run.
    pub text_encoding: SheetTextEncoding,
    /// Decoded text payload.
    pub text: String,
    /// Byte offset of the nearby coordinate pair.
    pub coordinate_offset: usize,
    /// Candidate insertion X coordinate.
    pub x: i32,
    /// Candidate insertion Y coordinate.
    pub y: i32,
    /// Whether text and coordinate fit inside the same probed Sheet chunk.
    pub same_chunk: bool,
    /// Minimum byte distance between the text run and coordinate pair.
    pub byte_distance: usize,
    /// Whether the coordinate pair passes structural-value filters.
    pub quality_passed: bool,
    /// Start offset of the common chunk, when `same_chunk` is true.
    pub chunk_start: Option<usize>,
    /// End offset of the common chunk, when `same_chunk` is true.
    pub chunk_end: Option<usize>,
}

/// Investigation-only score for a text-to-coordinate candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetTextWindowScore {
    /// Candidate being scored.
    pub candidate: SheetTextWindowCandidate,
    /// Investigation score. This never directly promotes text geometry.
    pub score: i32,
    /// Explainable scoring reasons.
    pub reasons: Vec<SheetTextWindowScoreReason>,
}

/// Explainable scoring reason for [`SheetTextWindowScore`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetTextWindowScoreReason {
    /// Text looks like an engineering label or annotation, not binary noise.
    TextQualityPassed,
    /// Text looks too much like binary payload decoded as text.
    TextQualityRejected,
    /// Text and coordinate fall inside the same Sheet chunk.
    SameChunk,
    /// Coordinate is near the text byte run.
    NearbyCoordinate {
        /// Minimum byte distance between the text and coordinate ranges.
        distance: usize,
    },
    /// Coordinate passed structural-value filters.
    HighQualityCoordinate,
    /// Multiple text runs share the same text-to-coordinate byte delta.
    RepeatedTextCoordinateDelta {
        /// Signed byte delta from text offset to coordinate offset.
        delta: isize,
        /// Distinct text runs supporting this delta.
        support: usize,
    },
}

/// Plausible adjacent integer pair that may represent a Sheet
/// coordinate. Kept as a hint until multiple fixtures confirm the
/// surrounding record layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetCoordinateHint {
    /// Byte offset of the first `i32` in the pair.
    pub offset: usize,
    /// First coordinate-like value.
    pub x: i32,
    /// Second coordinate-like value.
    pub y: i32,
}

/// Experimental byte-window hit for an object `field_x` inside a Sheet stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXWindow {
    /// Object `field_x` value that matched in little-endian form.
    pub field_x: u32,
    /// Byte offset where the matched `field_x` starts.
    pub offset: usize,
    /// Start offset of a surrounding endpoint-record signature, if this hit
    /// appears to be part of one.
    pub endpoint_record_start: Option<usize>,
    /// Inclusive start of the inspected byte window.
    pub window_start: usize,
    /// Exclusive end of the inspected byte window.
    pub window_end: usize,
    /// Plausible coordinate pairs found inside the same bounded window.
    pub nearby_coordinates: Vec<SheetCoordinateHint>,
}

/// Investigation-only score for a [`SheetFieldXWindow`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXWindowScore {
    /// Object `field_x` value being scored.
    pub field_x: u32,
    /// Byte offset where the matched `field_x` starts.
    pub offset: usize,
    /// Investigation score. This never directly promotes geometry.
    pub score: i32,
    /// Explainable scoring reasons.
    pub reasons: Vec<SheetFieldXWindowScoreReason>,
    /// Nearest candidate position from the same window, if any.
    pub candidate_position: Option<SheetCoordinateHint>,
}

/// Investigation-only features derived from a [`SheetFieldXWindow`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXWindowFeatures {
    /// Object `field_x` value being analyzed.
    pub field_x: u32,
    /// Byte offset where the matched `field_x` starts.
    pub offset: usize,
    /// Start offset of a surrounding endpoint-record signature, if present.
    pub endpoint_record_start: Option<usize>,
    /// Start offset of the containing Sheet chunk, if found.
    pub chunk_start: Option<usize>,
    /// End offset of the containing Sheet chunk, if found.
    pub chunk_end: Option<usize>,
    /// Signed byte delta from chunk start to the `field_x` hit.
    pub field_delta_from_chunk: Option<isize>,
    /// Signed byte delta from chunk start to the selected coordinate candidate.
    pub coordinate_delta_from_chunk: Option<isize>,
    /// Nearest non-overlapping coordinate candidate in the window, if any.
    pub candidate_position: Option<SheetCoordinateHint>,
    /// Stable marker candidates reserved for the next evidence pass.
    pub stable_markers: Vec<SheetWindowMarker>,
}

/// Marker-like value observed near a [`SheetFieldXWindow`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetWindowMarker {
    /// Byte offset where the marker-like value starts.
    pub offset: usize,
    /// Signed byte delta from the `field_x` hit to this marker.
    pub delta_from_field: isize,
    /// Marker value interpreted as little-endian `u32`.
    pub value_u32: u32,
}

/// Source-backed identity values for an object-like DA trailer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetObjectIdentity {
    /// Dynamic Attributes `field_x` value.
    pub field_x: u32,
    /// DA trailer `record_id`.
    pub record_id: u32,
    /// DA trailer `class_id`.
    pub class_id: u32,
    /// Optional 32-hex `DrawingID` resolved from the DA record.
    pub drawing_id: Option<String>,
}

/// Investigation index used to resolve Sheet-window identity candidates
/// back to source-backed DA records.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetIdentityIndex {
    /// Object identities keyed by `field_x`.
    pub by_field_x: BTreeMap<u32, SheetObjectIdentity>,
    drawing_id_to_field_x: BTreeMap<String, u32>,
}

impl SheetIdentityIndex {
    /// Resolve a `DrawingID` to the owning `field_x`, case-insensitively.
    pub fn field_x_for_drawing_id(&self, drawing_id: &str) -> Option<u32> {
        self.drawing_id_to_field_x
            .get(&drawing_id.to_ascii_lowercase())
            .copied()
    }
}

/// Kind of source-backed identity found near a field-x window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetFieldXWindowIdentityKind {
    /// ASCII 32-hex `DrawingID`.
    DrawingIdAscii,
    /// UTF-16LE 32-hex `DrawingID`.
    DrawingIdUtf16Le,
    /// DA trailer `record_id`.
    TrailerRecordId,
    /// DA trailer `class_id`.
    TrailerClassId,
    /// High-entropy marker that has not resolved to source identity.
    UnknownMarker,
}

/// Value carried by a [`SheetFieldXWindowIdentity`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetFieldXWindowIdentityValue {
    /// Text identity value, such as `DrawingID`.
    Text(String),
    /// Numeric identity value, such as DA trailer `record_id`.
    U32(u32),
}

/// Investigation-only identity evidence found near a field-x window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXWindowIdentity {
    /// Field-x value of the inspected window.
    pub field_x: u32,
    /// Byte offset where the identity value starts.
    pub offset: usize,
    /// Signed byte delta from the field-x hit to the identity value.
    pub delta_from_field: isize,
    /// Identity kind.
    pub kind: SheetFieldXWindowIdentityKind,
    /// Identity value.
    pub value: SheetFieldXWindowIdentityValue,
    /// Resolved owning field-x, when the value maps to a known object.
    pub resolves_to_field_x: Option<u32>,
    /// Whether the identity resolves to the same object as the window.
    pub resolves_to_same_object: bool,
}

/// Explainable reason attached to a [`SheetFieldXWindowScore`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetFieldXWindowScoreReason {
    /// Hit is part of an endpoint-record signature, so it is relationship
    /// evidence rather than object geometry.
    EndpointRecordReference,
    /// The hit is not inside an endpoint-record signature.
    NonEndpointHit,
    /// The `field_x` resolves to a known object record.
    ObjectFieldResolves,
    /// The window contains a coordinate candidate at this byte delta from the
    /// matched `field_x`.
    CoordinateCandidateAtDelta {
        /// Signed byte delta from the `field_x` hit to the coordinate pair.
        delta: isize,
    },
    /// The same coordinate delta appears for multiple distinct object fields.
    RepeatedDeltaAcrossFields {
        /// Signed byte delta from the `field_x` hit to the coordinate pair.
        delta: isize,
        /// Number of distinct `field_x` values supporting this delta.
        support: usize,
    },
    /// The field and candidate coordinate share a repeated chunk-relative shape.
    StableChunkShape {
        /// Signed byte delta from chunk start to the `field_x` hit.
        field_delta: isize,
        /// Signed byte delta from chunk start to the candidate coordinate.
        coordinate_delta: isize,
        /// Number of distinct `field_x` values supporting this shape.
        support: usize,
    },
    /// The window has a repeated non-generic marker nearby.
    StableMarkerNearby {
        /// Signed byte delta from the `field_x` hit to the marker.
        delta: isize,
        /// Marker-like value interpreted as little-endian `u32`.
        value_u32: u32,
        /// Number of distinct `field_x` values supporting this marker shape.
        support: usize,
    },
    /// The window has a source-backed identity that resolves to the same object.
    GraphicIdentityNearby {
        /// Identity kind that resolved to the same object.
        kind: SheetFieldXWindowIdentityKind,
        /// Signed byte delta from the `field_x` hit to the identity value.
        delta: isize,
    },
}

/// Bounded byte slice attached to an investigation candidate dump.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetCandidateRecordWindow {
    /// Inclusive start offset inside the Sheet stream.
    pub start: usize,
    /// Exclusive end offset inside the Sheet stream.
    pub end: usize,
    /// Uppercase space-separated bytes from `start..end`.
    pub hex: String,
}

/// Ranked field-x / identity candidate dump for record-shape investigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXCandidateRecordDump {
    /// 1-based rank after score-descending ordering.
    pub rank: usize,
    /// Object `field_x` value being investigated.
    pub field_x: u32,
    /// Investigation score.
    pub score: i32,
    /// Byte offset where `field_x` was observed.
    pub field_offset: usize,
    /// Reasons that explain the score.
    pub reasons: Vec<SheetFieldXWindowScoreReason>,
    /// Byte window around [`Self::field_offset`].
    pub field_window: SheetCandidateRecordWindow,
    /// Candidate coordinate byte offset, when scoring found one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub coordinate_offset: Option<usize>,
    /// Byte window around [`Self::coordinate_offset`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub coordinate_window: Option<SheetCandidateRecordWindow>,
}

/// Ranked text placement candidate dump for record-shape investigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetTextCandidateRecordDump {
    /// 1-based rank after score-descending ordering.
    pub rank: usize,
    /// Investigation score.
    pub score: i32,
    /// Decoded text payload.
    pub text: String,
    /// Byte offset where the text starts.
    pub text_offset: usize,
    /// Byte offset where the candidate coordinate starts.
    pub coordinate_offset: usize,
    /// Reasons that explain the score.
    pub reasons: Vec<SheetTextWindowScoreReason>,
    /// Byte window around [`Self::text_offset`].
    pub text_window: SheetCandidateRecordWindow,
    /// Byte window around [`Self::coordinate_offset`].
    pub coordinate_window: SheetCandidateRecordWindow,
}

/// First-pass classifier output for repeated object-like Sheet record shapes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SheetFieldXRecordShapeClass {
    /// Signed byte delta from chunk start to the `field_x` hit.
    pub field_delta_from_chunk: isize,
    /// Signed byte delta from chunk start to the selected coordinate candidate.
    pub coordinate_delta_from_chunk: isize,
    /// Number of distinct non-endpoint `field_x` values supporting this shape.
    pub support: usize,
    /// Sorted distinct `field_x` values supporting this shape.
    pub field_xs: Vec<u32>,
    /// Example byte offset where a supporting `field_x` was observed.
    pub example_field_offset: usize,
    /// Example coordinate byte offset for the same shape.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub example_coordinate_offset: Option<usize>,
}

/// One byte offset inside the sheet that at least one heuristic thinks
/// is a record / region boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateBoundary {
    /// Absolute byte offset inside the sheet stream.
    pub offset: usize,
    /// Sum of per-heuristic scores contributing to this offset.
    /// Compare against [`SheetProbeOptions::min_boundary_score`].
    pub score: u32,
    /// Each reason that voted for this offset (deduplicated).
    pub reasons: Vec<BoundaryReason>,
}

/// Which heuristic suggested a [`CandidateBoundary`]. Documented beside
/// [`find_candidate_boundaries`]; unreserved variants currently do not
/// fire (see module-level note).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryReason {
    /// A run of at least `zero_run_threshold` `0x00` bytes surrounds
    /// this offset.
    ZeroRun,
    /// A printable-ASCII burst of at least `ascii_burst_threshold`
    /// starts at this offset.
    AsciiBurst,
    /// A printable UTF-16LE burst of at least `utf16_burst_threshold`
    /// starts at this offset.
    Utf16Burst,
    /// 4-byte alignment hit, used for the implicit start/end markers.
    Alignment4,
    /// 8-byte alignment hit. Reserved for future heuristics; not
    /// currently emitted.
    Alignment8,
    /// Four consecutive 32-bit words at this offset carry the same
    /// value (fill pattern / header padding).
    RepeatedU32Pattern,
    /// Four consecutive 32-bit words form a strictly ascending,
    /// in-range sequence (offset-table signature).
    OffsetLikeSequence,
    /// A known record-marker byte gives way to a different family at
    /// this offset. Reserved for future heuristics; not currently
    /// emitted.
    MarkerTransition,
}

/// One chunk produced by [`split_by_boundaries`] — a post-thresholding
/// slice of the sheet stream with its per-chunk probe stats. The
/// `start..end` range is deliberately layout-compatible with
/// [`crate::writer::plan::SheetChunkPatch`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetChunk {
    /// Inclusive start byte offset inside the sheet stream.
    pub start: usize,
    /// Exclusive end byte offset inside the sheet stream.
    pub end: usize,
    /// Convenience `end - start` byte count (cached so consumers
    /// don't recompute on every access).
    pub len: usize,
    /// ASCII-printable runs lifted out of the chunk; capped by
    /// [`SheetProbeOptions::max_preview_strings`].
    pub ascii_preview: Vec<String>,
    /// UTF-16LE printable runs lifted out of the chunk; same cap as
    /// [`Self::ascii_preview`].
    pub utf16_preview: Vec<String>,
    /// Fraction of bytes in the chunk that are `0x00` (in `[0, 1]`).
    pub zero_ratio: f32,
    /// Fraction of aligned `u32` reads whose value is smaller than
    /// the whole stream's length — a rough "could this be an offset
    /// table?" score in `[0, 1]`.
    pub aligned_u32_density: f32,
    /// Number of adjacent aligned `u32` pairs that were equal — a
    /// fill-pattern / repeated-header signal.
    pub repeated_u32_hits: usize,
    /// High-level classification derived from the other stats; see
    /// [`SheetChunkKindHint`] for the buckets.
    pub kind_hint: SheetChunkKindHint,
}

/// Quick classification bucket emitted by [`summarize_chunk`] from the
/// per-chunk probe stats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetChunkKindHint {
    /// Multiple ASCII / UTF-16 runs and few zero bytes — looks like
    /// a text payload.
    TextHeavy,
    /// No text runs and very few zero bytes — looks like compact
    /// binary data.
    BinaryDense,
    /// Some text runs mixed with non-text bytes.
    Mixed,
    /// High offset-like u32 density plus repeated u32 hits — looks
    /// like an offset table / index block.
    OffsetTableLike,
    /// None of the above heuristics triggered confidently.
    Unknown,
}

/// Run the full probe pipeline against a single `Sheet*` stream's raw bytes.
pub fn probe_sheet_stream(
    sheet_name: &str,
    path: &str,
    data: &[u8],
    opts: &SheetProbeOptions,
) -> SheetProbeReport {
    let candidate_boundaries = find_candidate_boundaries(data, opts);
    let chunks = split_by_boundaries(data, &candidate_boundaries, opts);
    let record_type_counts = record_type_counts(data);
    let text_runs = scan_text_runs(data, opts);
    let coordinate_hints = coordinate_hints(data);

    SheetProbeReport {
        sheet_name: sheet_name.to_string(),
        path: path.to_string(),
        size: data.len() as u64,
        candidate_boundaries,
        chunks,
        record_type_counts,
        text_runs,
        coordinate_hints,
    }
}

/// Find raw Sheet byte windows where object `field_x` values appear.
///
/// This is investigation-only. A hit does not prove geometry ownership;
/// callers must still inspect the surrounding record shape before promoting
/// anything to `SheetObjectGeometryHint`.
pub fn field_x_windows(
    data: &[u8],
    field_xs: &[u32],
    window_radius: usize,
) -> Vec<SheetFieldXWindow> {
    if data.len() < 4 || field_xs.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let end = data.len() - 4;
    for offset in 0..=end {
        let value = u32_le(data, offset);
        if !field_xs.contains(&value) {
            continue;
        }
        let window_start = offset.saturating_sub(window_radius);
        let window_end = (offset + 4 + window_radius).min(data.len());
        out.push(SheetFieldXWindow {
            field_x: value,
            offset,
            endpoint_record_start: endpoint_record_signature_start(data, offset),
            window_start,
            window_end,
            nearby_coordinates: coordinate_hints_in_range(data, window_start, window_end),
        });
    }
    out
}

/// Score field-x windows for investigation without promoting geometry.
pub fn score_field_x_windows(
    windows: &[SheetFieldXWindow],
    object_field_xs: &HashSet<u32>,
) -> Vec<SheetFieldXWindowScore> {
    let repeated_delta_support = repeated_delta_support(windows, object_field_xs);
    windows
        .iter()
        .map(|window| {
            if window.endpoint_record_start.is_some() {
                return SheetFieldXWindowScore {
                    field_x: window.field_x,
                    offset: window.offset,
                    score: -100,
                    reasons: vec![SheetFieldXWindowScoreReason::EndpointRecordReference],
                    candidate_position: None,
                };
            }

            let mut score = 10;
            let mut reasons = vec![SheetFieldXWindowScoreReason::NonEndpointHit];
            if object_field_xs.contains(&window.field_x) {
                score += 10;
                reasons.push(SheetFieldXWindowScoreReason::ObjectFieldResolves);
            }
            let candidate_position = repeated_delta_candidate(window, &repeated_delta_support)
                .or_else(|| nearest_coordinate(window));
            if let Some(position) = &candidate_position {
                let delta = offset_delta(position.offset, window.offset);
                score += 5;
                reasons.push(SheetFieldXWindowScoreReason::CoordinateCandidateAtDelta { delta });
                if let Some(support) = repeated_delta_support.get(&delta).copied() {
                    if support >= 2 {
                        score += 40;
                        reasons.push(SheetFieldXWindowScoreReason::RepeatedDeltaAcrossFields {
                            delta,
                            support,
                        });
                    }
                }
            }

            SheetFieldXWindowScore {
                field_x: window.field_x,
                offset: window.offset,
                score,
                reasons,
                candidate_position,
            }
        })
        .collect()
}

/// Extract investigation features for field-x windows relative to Sheet chunks.
pub fn field_x_window_features(
    data: &[u8],
    windows: &[SheetFieldXWindow],
    chunks: &[SheetChunk],
) -> Vec<SheetFieldXWindowFeatures> {
    windows
        .iter()
        .map(|window| {
            let chunk = chunks
                .iter()
                .find(|chunk| chunk.start <= window.offset && window.offset + 4 <= chunk.end);
            let candidate_position = nearest_coordinate(window);
            SheetFieldXWindowFeatures {
                field_x: window.field_x,
                offset: window.offset,
                endpoint_record_start: window.endpoint_record_start,
                chunk_start: chunk.map(|chunk| chunk.start),
                chunk_end: chunk.map(|chunk| chunk.end),
                field_delta_from_chunk: chunk.map(|chunk| offset_delta(window.offset, chunk.start)),
                coordinate_delta_from_chunk: chunk.and_then(|chunk| {
                    candidate_position
                        .as_ref()
                        .map(|position| offset_delta(position.offset, chunk.start))
                }),
                candidate_position,
                stable_markers: marker_candidates(data, window),
            }
        })
        .collect()
}

/// Find nearby coordinate hints for decoded text runs without promoting geometry.
pub fn sheet_text_window_candidates(
    text_runs: &[SheetTextRun],
    coordinates: &[SheetCoordinateHint],
    chunks: &[SheetChunk],
    radius: usize,
) -> Vec<SheetTextWindowCandidate> {
    let mut candidates = Vec::new();
    for text in text_runs {
        let text_end = text.offset.saturating_add(text.byte_len);
        for coordinate in coordinates {
            let coordinate_end = coordinate.offset.saturating_add(8);
            let byte_distance =
                byte_range_distance(text.offset, text_end, coordinate.offset, coordinate_end);
            if byte_distance > radius {
                continue;
            }
            let text_chunk = chunk_containing_range(chunks, text.offset, text_end);
            let coordinate_chunk =
                chunk_containing_range(chunks, coordinate.offset, coordinate_end);
            let same_chunk = text_chunk
                .zip(coordinate_chunk)
                .is_some_and(|(left, right)| left.start == right.start && left.end == right.end);
            candidates.push(SheetTextWindowCandidate {
                text_offset: text.offset,
                text_byte_len: text.byte_len,
                text_encoding: text.encoding.clone(),
                text: text.text.clone(),
                coordinate_offset: coordinate.offset,
                x: coordinate.x,
                y: coordinate.y,
                same_chunk,
                byte_distance,
                quality_passed: is_high_quality_coordinate_candidate(coordinate),
                chunk_start: same_chunk
                    .then(|| text_chunk.expect("same chunk has text chunk").start),
                chunk_end: same_chunk.then(|| text_chunk.expect("same chunk has text chunk").end),
            });
        }
    }
    candidates.sort_by_key(|candidate| {
        (
            candidate.text_offset,
            candidate.byte_distance,
            candidate.coordinate_offset,
        )
    });
    candidates
}

/// Return whether a decoded text run is plausible enough for placement scoring.
pub fn is_high_quality_text_candidate(text: &str) -> bool {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if !(2..=128).contains(&char_count) {
        return false;
    }
    if trimmed.contains('\u{fffd}') {
        return false;
    }

    let ascii_alnum = trimmed.chars().filter(char::is_ascii_alphanumeric).count();
    if ascii_alnum == 0 {
        return false;
    }
    if trimmed
        .chars()
        .any(|ch| !ch.is_ascii() && !is_cjk_text_char(ch))
    {
        return false;
    }

    let tag_like = trimmed
        .chars()
        .filter(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(
                    ch,
                    '-' | '_' | '/' | '.' | ' ' | '#' | '(' | ')' | '&' | '+' | ':'
                )
        })
        .count();
    tag_like * 100 / char_count >= 70
}

fn is_cjk_text_char(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
        || ('\u{3400}'..='\u{4dbf}').contains(&ch)
        || ('\u{f900}'..='\u{faff}').contains(&ch)
}

/// Score text placement candidates without promoting them to render geometry.
pub fn score_sheet_text_window_candidates(
    candidates: &[SheetTextWindowCandidate],
) -> Vec<SheetTextWindowScore> {
    let repeated_delta_support = text_coordinate_delta_support(candidates);
    let mut scores: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            let mut score = 0;
            let mut reasons = Vec::new();
            if is_high_quality_text_candidate(&candidate.text) {
                score += 20;
                reasons.push(SheetTextWindowScoreReason::TextQualityPassed);
            } else {
                score -= 100;
                reasons.push(SheetTextWindowScoreReason::TextQualityRejected);
            }
            if candidate.same_chunk {
                score += 20;
                reasons.push(SheetTextWindowScoreReason::SameChunk);
            }
            if candidate.byte_distance <= 32 {
                score += 10;
                reasons.push(SheetTextWindowScoreReason::NearbyCoordinate {
                    distance: candidate.byte_distance,
                });
            }
            if candidate.quality_passed {
                score += 20;
                reasons.push(SheetTextWindowScoreReason::HighQualityCoordinate);
            }
            let delta = offset_delta(candidate.coordinate_offset, candidate.text_offset);
            if let Some(support) = repeated_delta_support.get(&delta).copied() {
                if support >= 2 {
                    score += 30;
                    reasons.push(SheetTextWindowScoreReason::RepeatedTextCoordinateDelta {
                        delta,
                        support,
                    });
                }
            }

            SheetTextWindowScore {
                candidate: candidate.clone(),
                score,
                reasons,
            }
        })
        .collect();
    scores.sort_by_key(|score| {
        (
            std::cmp::Reverse(score.score),
            score.candidate.text_offset,
            score.candidate.coordinate_offset,
        )
    });
    scores
}

/// Score extracted window features with stronger repeated record-shape evidence.
pub fn score_field_x_window_features(
    features: &[SheetFieldXWindowFeatures],
    object_field_xs: &HashSet<u32>,
) -> Vec<SheetFieldXWindowScore> {
    let chunk_support = stable_chunk_shape_support(features);
    let marker_support = stable_marker_support(features);
    features
        .iter()
        .map(|feature| {
            if feature.endpoint_record_start.is_some() {
                return SheetFieldXWindowScore {
                    field_x: feature.field_x,
                    offset: feature.offset,
                    score: -100,
                    reasons: vec![SheetFieldXWindowScoreReason::EndpointRecordReference],
                    candidate_position: None,
                };
            }

            let mut score = 10;
            let mut reasons = vec![SheetFieldXWindowScoreReason::NonEndpointHit];
            if object_field_xs.contains(&feature.field_x) {
                score += 10;
                reasons.push(SheetFieldXWindowScoreReason::ObjectFieldResolves);
            }
            if let Some(position) = &feature.candidate_position {
                score += 5;
                reasons.push(SheetFieldXWindowScoreReason::CoordinateCandidateAtDelta {
                    delta: offset_delta(position.offset, feature.offset),
                });
            }
            if let (Some(field_delta), Some(coordinate_delta)) = (
                feature.field_delta_from_chunk,
                feature.coordinate_delta_from_chunk,
            ) {
                if let Some(support) = chunk_support
                    .get(&(field_delta, coordinate_delta))
                    .copied()
                    .filter(|support| *support >= 2)
                {
                    score += 25;
                    reasons.push(SheetFieldXWindowScoreReason::StableChunkShape {
                        field_delta,
                        coordinate_delta,
                        support,
                    });
                }
            }
            for marker in &feature.stable_markers {
                if let Some(support) = marker_support
                    .get(&(marker.delta_from_field, marker.value_u32))
                    .copied()
                    .filter(|support| *support >= 3)
                {
                    score += 20;
                    reasons.push(SheetFieldXWindowScoreReason::StableMarkerNearby {
                        delta: marker.delta_from_field,
                        value_u32: marker.value_u32,
                        support,
                    });
                    break;
                }
            }

            SheetFieldXWindowScore {
                field_x: feature.field_x,
                offset: feature.offset,
                score,
                reasons,
                candidate_position: feature.candidate_position.clone(),
            }
        })
        .collect()
}

/// Score extracted window features with additional same-object identity evidence.
pub fn score_field_x_window_features_with_identities(
    features: &[SheetFieldXWindowFeatures],
    object_field_xs: &HashSet<u32>,
    identities: &[SheetFieldXWindowIdentity],
) -> Vec<SheetFieldXWindowScore> {
    let mut scores = score_field_x_window_features(features, object_field_xs);
    for score in &mut scores {
        if score.score < 0 {
            continue;
        }
        if let Some(identity) = identities
            .iter()
            .find(|identity| identity_supports_score(identity, score))
        {
            score.score += 35;
            score
                .reasons
                .push(SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                    kind: identity.kind.clone(),
                    delta: identity.delta_from_field,
                });
        }
    }
    scores
}

/// Build ranked field-x candidate record dumps for human review.
///
/// This is investigation-only: it preserves nearby bytes and score reasons,
/// but it does not promote any candidate into geometry.
pub fn top_field_x_candidate_record_dumps(
    data: &[u8],
    scores: &[SheetFieldXWindowScore],
    limit: usize,
    radius: usize,
) -> Vec<SheetFieldXCandidateRecordDump> {
    let mut top: Vec<_> = scores.iter().collect();
    top.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.offset.cmp(&right.offset))
            .then_with(|| left.field_x.cmp(&right.field_x))
    });
    top.into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, score)| {
            let coordinate_offset = score
                .candidate_position
                .as_ref()
                .map(|coordinate| coordinate.offset);
            SheetFieldXCandidateRecordDump {
                rank: index + 1,
                field_x: score.field_x,
                score: score.score,
                field_offset: score.offset,
                reasons: score.reasons.clone(),
                field_window: candidate_record_window(data, score.offset, radius),
                coordinate_offset,
                coordinate_window: coordinate_offset
                    .map(|offset| candidate_record_window(data, offset, radius)),
            }
        })
        .collect()
}

/// Build ranked text candidate record dumps for human review.
///
/// This is investigation-only: text remains unpromoted until its placement is
/// source-proven by record shape and fixture evidence.
pub fn top_text_candidate_record_dumps(
    data: &[u8],
    scores: &[SheetTextWindowScore],
    limit: usize,
    radius: usize,
) -> Vec<SheetTextCandidateRecordDump> {
    let mut top: Vec<_> = scores.iter().collect();
    top.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.candidate.text_offset.cmp(&right.candidate.text_offset))
            .then_with(|| {
                left.candidate
                    .coordinate_offset
                    .cmp(&right.candidate.coordinate_offset)
            })
    });
    top.into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, score)| SheetTextCandidateRecordDump {
            rank: index + 1,
            score: score.score,
            text: score.candidate.text.clone(),
            text_offset: score.candidate.text_offset,
            coordinate_offset: score.candidate.coordinate_offset,
            reasons: score.reasons.clone(),
            text_window: candidate_record_window(data, score.candidate.text_offset, radius),
            coordinate_window: candidate_record_window(
                data,
                score.candidate.coordinate_offset,
                radius,
            ),
        })
        .collect()
}

fn candidate_record_window(
    data: &[u8],
    center: usize,
    radius: usize,
) -> SheetCandidateRecordWindow {
    let start = center.saturating_sub(radius);
    let end = center.saturating_add(radius).min(data.len());
    let hex = data[start..end]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    SheetCandidateRecordWindow { start, end, hex }
}

/// Build a source-backed identity index from DA record trailers.
///
/// Trailers without a `DrawingID` are skipped for now: relationship trailers
/// can still carry useful `field_x` values, but they do not prove object
/// identity for geometry ownership.
pub fn sheet_identity_index_from_trailers(
    trailers: &[crate::model::DaRecordTrailer],
) -> SheetIdentityIndex {
    let mut index = SheetIdentityIndex::default();
    for trailer in trailers {
        let Some(drawing_id) = trailer.drawing_id.clone() else {
            continue;
        };
        index.by_field_x.insert(
            trailer.field_x,
            SheetObjectIdentity {
                field_x: trailer.field_x,
                record_id: trailer.record_id,
                class_id: trailer.class_id,
                drawing_id: Some(drawing_id.clone()),
            },
        );
        index
            .drawing_id_to_field_x
            .entry(drawing_id.to_ascii_lowercase())
            .or_insert(trailer.field_x);
    }
    index
}

/// Scan field-x windows for source-backed identity values.
pub fn field_x_window_identities(
    data: &[u8],
    windows: &[SheetFieldXWindow],
    identity_index: &SheetIdentityIndex,
) -> Vec<SheetFieldXWindowIdentity> {
    let mut identities = Vec::new();
    for window in windows {
        let mut text_offset = window.window_start;
        while text_offset + 32 <= window.window_end && text_offset + 32 <= data.len() {
            let bytes = &data[text_offset..text_offset + 32];
            if is_ascii_hex_32(bytes) {
                let value = String::from_utf8_lossy(bytes).into_owned();
                if let Some(field_x) = identity_index.field_x_for_drawing_id(&value) {
                    identities.push(SheetFieldXWindowIdentity {
                        field_x: window.field_x,
                        offset: text_offset,
                        delta_from_field: (text_offset as isize) - (window.offset as isize),
                        kind: SheetFieldXWindowIdentityKind::DrawingIdAscii,
                        value: SheetFieldXWindowIdentityValue::Text(value),
                        resolves_to_field_x: Some(field_x),
                        resolves_to_same_object: field_x == window.field_x,
                    });
                }
            }
            text_offset += 1;
        }

        let mut utf16_offset = window.window_start;
        while utf16_offset + 64 <= window.window_end && utf16_offset + 64 <= data.len() {
            let bytes = &data[utf16_offset..utf16_offset + 64];
            if let Some(value) = utf16_le_hex_32(bytes) {
                if let Some(field_x) = identity_index.field_x_for_drawing_id(&value) {
                    identities.push(SheetFieldXWindowIdentity {
                        field_x: window.field_x,
                        offset: utf16_offset,
                        delta_from_field: (utf16_offset as isize) - (window.offset as isize),
                        kind: SheetFieldXWindowIdentityKind::DrawingIdUtf16Le,
                        value: SheetFieldXWindowIdentityValue::Text(value),
                        resolves_to_field_x: Some(field_x),
                        resolves_to_same_object: field_x == window.field_x,
                    });
                }
            }
            utf16_offset += 1;
        }

        let mut offset = window.window_start;
        while offset + 4 <= window.window_end && offset + 4 <= data.len() {
            let value = u32_le(data, offset);
            if let Some(identity) = identity_index
                .by_field_x
                .values()
                .find(|identity| identity.record_id == value)
            {
                identities.push(SheetFieldXWindowIdentity {
                    field_x: window.field_x,
                    offset,
                    delta_from_field: (offset as isize) - (window.offset as isize),
                    kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                    value: SheetFieldXWindowIdentityValue::U32(value),
                    resolves_to_field_x: Some(identity.field_x),
                    resolves_to_same_object: identity.field_x == window.field_x,
                });
            }
            offset += 1;
        }
    }
    identities
}

fn is_ascii_hex_32(bytes: &[u8]) -> bool {
    bytes.len() == 32 && bytes.iter().all(u8::is_ascii_hexdigit)
}

fn utf16_le_hex_32(bytes: &[u8]) -> Option<String> {
    if bytes.len() != 64 {
        return None;
    }
    let mut text = String::with_capacity(32);
    for pair in bytes.chunks_exact(2) {
        if pair[1] != 0 || !pair[0].is_ascii_hexdigit() {
            return None;
        }
        text.push(char::from(pair[0]));
    }
    Some(text)
}

fn identity_supports_score(
    identity: &SheetFieldXWindowIdentity,
    score: &SheetFieldXWindowScore,
) -> bool {
    identity.resolves_to_same_object && identity.field_x == score.field_x
}

fn chunk_containing_range(chunks: &[SheetChunk], start: usize, end: usize) -> Option<&SheetChunk> {
    chunks
        .iter()
        .find(|chunk| chunk.start <= start && end <= chunk.end)
}

fn byte_range_distance(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> usize {
    if left_end < right_start {
        right_start - left_end
    } else {
        left_start.saturating_sub(right_end)
    }
}

fn text_coordinate_delta_support(
    candidates: &[SheetTextWindowCandidate],
) -> BTreeMap<isize, usize> {
    let mut text_offsets_by_delta: BTreeMap<isize, HashSet<usize>> = BTreeMap::new();
    for candidate in candidates {
        if !candidate.same_chunk
            || !candidate.quality_passed
            || !is_high_quality_text_candidate(&candidate.text)
        {
            continue;
        }
        text_offsets_by_delta
            .entry(offset_delta(
                candidate.coordinate_offset,
                candidate.text_offset,
            ))
            .or_default()
            .insert(candidate.text_offset);
    }
    text_offsets_by_delta
        .into_iter()
        .map(|(delta, text_offsets)| (delta, text_offsets.len()))
        .collect()
}

/// Return whether a coordinate-like pair is strong enough for object mapping.
pub fn is_high_quality_coordinate_candidate(hint: &SheetCoordinateHint) -> bool {
    let abs_x = checked_abs_i32(hint.x);
    let abs_y = checked_abs_i32(hint.y);
    if abs_x <= 16 && abs_y <= 16 {
        return false;
    }
    if abs_x < 1_000 && abs_y < 1_000 {
        return false;
    }
    if is_structural_coordinate_value(hint.x) || is_structural_coordinate_value(hint.y) {
        return false;
    }
    true
}

/// Count distinct object fields supporting each chunk-relative shape.
pub fn stable_chunk_shape_support(
    features: &[SheetFieldXWindowFeatures],
) -> BTreeMap<(isize, isize), usize> {
    let mut fields_by_shape: BTreeMap<(isize, isize), HashSet<u32>> = BTreeMap::new();
    for feature in features {
        if feature.endpoint_record_start.is_some() {
            continue;
        }
        let (Some(field_delta), Some(coordinate_delta)) = (
            feature.field_delta_from_chunk,
            feature.coordinate_delta_from_chunk,
        ) else {
            continue;
        };
        fields_by_shape
            .entry((field_delta, coordinate_delta))
            .or_default()
            .insert(feature.field_x);
    }

    fields_by_shape
        .into_iter()
        .map(|(shape, fields)| (shape, fields.len()))
        .collect()
}

/// Classify repeated, chunk-relative object-like Sheet record shapes.
pub fn classify_field_x_record_shapes(
    features: &[SheetFieldXWindowFeatures],
) -> Vec<SheetFieldXRecordShapeClass> {
    let mut fields_by_shape: BTreeMap<(isize, isize), HashSet<u32>> = BTreeMap::new();
    let mut examples_by_shape: BTreeMap<(isize, isize), (usize, Option<usize>)> = BTreeMap::new();

    for feature in features {
        if feature.endpoint_record_start.is_some() {
            continue;
        }
        let (Some(field_delta), Some(coordinate_delta)) = (
            feature.field_delta_from_chunk,
            feature.coordinate_delta_from_chunk,
        ) else {
            continue;
        };
        let shape = (field_delta, coordinate_delta);
        fields_by_shape
            .entry(shape)
            .or_default()
            .insert(feature.field_x);
        examples_by_shape.entry(shape).or_insert_with(|| {
            (
                feature.offset,
                feature
                    .candidate_position
                    .as_ref()
                    .map(|position| position.offset),
            )
        });
    }

    let mut classes: Vec<_> = fields_by_shape
        .into_iter()
        .map(|((field_delta, coordinate_delta), fields)| {
            let mut field_xs: Vec<_> = fields.into_iter().collect();
            field_xs.sort_unstable();
            let (example_field_offset, example_coordinate_offset) = examples_by_shape
                .remove(&(field_delta, coordinate_delta))
                .unwrap_or_default();
            SheetFieldXRecordShapeClass {
                field_delta_from_chunk: field_delta,
                coordinate_delta_from_chunk: coordinate_delta,
                support: field_xs.len(),
                field_xs,
                example_field_offset,
                example_coordinate_offset,
            }
        })
        .collect();
    classes.sort_by(|left, right| {
        right
            .support
            .cmp(&left.support)
            .then_with(|| {
                left.field_delta_from_chunk
                    .cmp(&right.field_delta_from_chunk)
            })
            .then_with(|| {
                left.coordinate_delta_from_chunk
                    .cmp(&right.coordinate_delta_from_chunk)
            })
    });
    classes
}

/// Count distinct object fields supporting each non-generic marker shape.
pub fn stable_marker_support(
    features: &[SheetFieldXWindowFeatures],
) -> BTreeMap<(isize, u32), usize> {
    let mut fields_by_marker: BTreeMap<(isize, u32), HashSet<u32>> = BTreeMap::new();
    for feature in features {
        if feature.endpoint_record_start.is_some() {
            continue;
        }
        for marker in &feature.stable_markers {
            if is_structural_marker_value(marker.value_u32) {
                continue;
            }
            fields_by_marker
                .entry((marker.delta_from_field, marker.value_u32))
                .or_default()
                .insert(feature.field_x);
        }
    }

    fields_by_marker
        .into_iter()
        .map(|(marker, fields)| (marker, fields.len()))
        .collect()
}

/// Aggregate gate summary for object geometry promotion readiness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectGeometryPromotionGateSummary {
    /// Total scored candidates evaluated.
    pub total_candidates: usize,
    /// Candidates with at least one `GraphicIdentityNearby` reason.
    pub identity_supported: usize,
    /// Candidates with at least one `StableChunkShape` reason.
    pub stable_shape_supported: usize,
    /// Candidates whose score meets or exceeds the threshold.
    pub over_threshold: usize,
    /// Candidates meeting all three: over threshold, identity-backed,
    /// and stable-shape-backed. Only these are safe to promote.
    pub promotable_candidates: usize,
}

/// Summarize how many scored candidates pass the three-pronged promotion gate.
///
/// A candidate is promotable when **all** of:
/// 1. `score >= threshold`
/// 2. has at least one [`SheetFieldXWindowScoreReason::GraphicIdentityNearby`]
/// 3. has at least one [`SheetFieldXWindowScoreReason::StableChunkShape`]
pub fn summarize_object_geometry_promotion_gate(
    scores: &[SheetFieldXWindowScore],
    threshold: i32,
) -> ObjectGeometryPromotionGateSummary {
    let mut identity_supported = 0usize;
    let mut stable_shape_supported = 0usize;
    let mut over_threshold = 0usize;
    let mut promotable_candidates = 0usize;

    for score in scores {
        let has_identity = score.reasons.iter().any(|r| {
            matches!(
                r,
                SheetFieldXWindowScoreReason::GraphicIdentityNearby { .. }
            )
        });
        let has_stable_shape = score
            .reasons
            .iter()
            .any(|r| matches!(r, SheetFieldXWindowScoreReason::StableChunkShape { .. }));
        let meets_threshold = score.score >= threshold;

        if has_identity {
            identity_supported += 1;
        }
        if has_stable_shape {
            stable_shape_supported += 1;
        }
        if meets_threshold {
            over_threshold += 1;
        }
        if has_identity && has_stable_shape && meets_threshold {
            promotable_candidates += 1;
        }
    }

    ObjectGeometryPromotionGateSummary {
        total_candidates: scores.len(),
        identity_supported,
        stable_shape_supported,
        over_threshold,
        promotable_candidates,
    }
}

/// Produce [`SheetObjectGeometryHint`] entries for candidates that pass the
/// three-pronged promotion gate (score + identity + stable shape).
pub fn populate_object_geometry_hints(
    scores: &[SheetFieldXWindowScore],
    threshold: i32,
) -> Vec<crate::model::SheetObjectGeometryHint> {
    scores
        .iter()
        .filter(|score| {
            score.score >= threshold
                && score.reasons.iter().any(|r| {
                    matches!(
                        r,
                        SheetFieldXWindowScoreReason::GraphicIdentityNearby { .. }
                    )
                })
                && score
                    .reasons
                    .iter()
                    .any(|r| matches!(r, SheetFieldXWindowScoreReason::StableChunkShape { .. }))
        })
        .map(|score| {
            let position =
                score
                    .candidate_position
                    .as_ref()
                    .map(|pos| crate::model::SheetCoordinateHintDto {
                        offset: pos.offset,
                        x: pos.x,
                        y: pos.y,
                    });
            crate::model::SheetObjectGeometryHint {
                offset: score.offset,
                field_x: score.field_x,
                position,
                graphic_oid: None,
                note: Some(object_geometry_hint_note(score)),
            }
        })
        .collect()
}

fn object_geometry_hint_note(score: &SheetFieldXWindowScore) -> String {
    let mut parts = vec![format!("score={}", score.score)];
    if score.reasons.iter().any(|reason| {
        matches!(
            reason,
            SheetFieldXWindowScoreReason::GraphicIdentityNearby { .. }
        )
    }) {
        parts.push("identity=graphic_nearby".to_string());
    }
    if let Some((field_delta, coordinate_delta, support)) =
        score.reasons.iter().find_map(|reason| match reason {
            SheetFieldXWindowScoreReason::StableChunkShape {
                field_delta,
                coordinate_delta,
                support,
            } => Some((*field_delta, *coordinate_delta, *support)),
            _ => None,
        })
    {
        parts.push(format!(
            "stable_shape=field_delta:{field_delta},coordinate_delta:{coordinate_delta},support:{support}"
        ));
    }
    parts.join(";")
}

fn is_structural_marker_value(value: u32) -> bool {
    value <= 16
        || matches!(value, 65_536 | 393_216 | 524_288)
        || (value.is_power_of_two() && value <= 1_048_576)
}

fn is_structural_coordinate_value(value: i32) -> bool {
    let abs = checked_abs_i32(value);
    matches!(
        value,
        -65_536 | -65_535 | 0 | 1 | 2 | 4 | 6 | 8 | 16 | 65_535 | 65_536 | 65_537
    ) || matches!(
        abs,
        32_768 | 65_536 | 131_072 | 262_144 | 524_288 | 1_048_576
    ) || ((abs as u32).is_power_of_two() && abs <= 1_048_576)
        || is_small_aligned_structural_value(abs)
        || is_packed_field_like_value(abs)
}

fn checked_abs_i32(value: i32) -> i32 {
    value.checked_abs().unwrap_or(i32::MAX)
}

fn is_small_aligned_structural_value(abs: i32) -> bool {
    abs > 0 && abs <= 65_536 && abs % 256 == 0
}

fn is_packed_field_like_value(abs: i32) -> bool {
    if abs <= 0 {
        return false;
    }
    let value = abs as u32;
    let high = value >> 16;
    let low = value & 0xFFFF;
    (1..=8).contains(&high) && (low <= 4096 || low >= 61_440)
}

fn marker_candidates(data: &[u8], window: &SheetFieldXWindow) -> Vec<SheetWindowMarker> {
    let mut markers = Vec::new();
    let mut offset = window.window_start;
    while offset + 4 <= window.window_end && offset + 4 <= data.len() {
        if offset != window.offset && offset.is_multiple_of(4) {
            let value_u32 = u32_le(data, offset);
            if value_u32 != 0 && value_u32 != window.field_x {
                markers.push(SheetWindowMarker {
                    offset,
                    delta_from_field: offset_delta(offset, window.offset),
                    value_u32,
                });
            }
        }
        offset += 1;
    }
    markers
}

fn nearest_coordinate(window: &SheetFieldXWindow) -> Option<SheetCoordinateHint> {
    non_overlapping_coordinates(window)
        .filter(|hint| is_high_quality_coordinate_candidate(hint))
        .min_by_key(|hint| hint.offset.abs_diff(window.offset))
        .cloned()
}

fn repeated_delta_candidate(
    window: &SheetFieldXWindow,
    repeated_delta_support: &BTreeMap<isize, usize>,
) -> Option<SheetCoordinateHint> {
    non_overlapping_coordinates(window)
        .filter(|hint| is_high_quality_coordinate_candidate(hint))
        .filter(|hint| {
            repeated_delta_support
                .get(&offset_delta(hint.offset, window.offset))
                .is_some_and(|support| *support >= 2)
        })
        .min_by_key(|hint| hint.offset.abs_diff(window.offset))
        .cloned()
}

fn non_overlapping_coordinates(
    window: &SheetFieldXWindow,
) -> impl Iterator<Item = &SheetCoordinateHint> {
    window
        .nearby_coordinates
        .iter()
        .filter(|hint| hint.offset + 8 <= window.offset || hint.offset >= window.offset + 4)
}

fn repeated_delta_support(
    windows: &[SheetFieldXWindow],
    object_field_xs: &HashSet<u32>,
) -> BTreeMap<isize, usize> {
    let mut field_xs_by_delta: BTreeMap<isize, HashSet<u32>> = BTreeMap::new();
    for window in windows {
        if window.endpoint_record_start.is_some() || !object_field_xs.contains(&window.field_x) {
            continue;
        }
        for hint in non_overlapping_coordinates(window)
            .filter(|hint| is_high_quality_coordinate_candidate(hint))
        {
            field_xs_by_delta
                .entry(offset_delta(hint.offset, window.offset))
                .or_default()
                .insert(window.field_x);
        }
    }

    field_xs_by_delta
        .into_iter()
        .map(|(delta, field_xs)| (delta, field_xs.len()))
        .collect()
}

fn offset_delta(lhs: usize, rhs: usize) -> isize {
    if lhs >= rhs {
        (lhs - rhs) as isize
    } else {
        -((rhs - lhs) as isize)
    }
}

fn endpoint_record_signature_start(data: &[u8], field_offset: usize) -> Option<usize> {
    const ENDPOINT_RECORD_LEN: usize = 26;
    const DISCRIMINATOR: u32 = 0x0000_0006;
    const ENDPOINT_TYPE_TAG: u16 = 0x0002;
    const ENDPOINT_DELIMITER: u16 = 0x0001;

    for start in [
        Some(field_offset),
        field_offset.checked_sub(16),
        field_offset.checked_sub(22),
    ]
    .into_iter()
    .flatten()
    {
        if start + ENDPOINT_RECORD_LEN > data.len() {
            continue;
        }
        if u32_le(data, start + 4) != DISCRIMINATOR {
            continue;
        }
        if !data[start + 8..start + 14].iter().all(|&b| b == 0) {
            continue;
        }
        if u16_le(data, start + 14) != ENDPOINT_TYPE_TAG {
            continue;
        }
        if u16_le(data, start + 20) != ENDPOINT_DELIMITER {
            continue;
        }
        return Some(start);
    }
    None
}

/// Collect every candidate boundary offset along with the heuristics that
/// voted for it. Same-offset hits are merged (scores summed, reasons
/// deduped).
pub fn find_candidate_boundaries(data: &[u8], opts: &SheetProbeOptions) -> Vec<CandidateBoundary> {
    let mut map: BTreeMap<usize, CandidateBoundary> = BTreeMap::new();

    add_boundary(&mut map, 0, 5, BoundaryReason::Alignment4);

    add_zero_run_boundaries(data, opts, &mut map);
    add_ascii_burst_boundaries(data, opts, &mut map);
    add_utf16_burst_boundaries(data, opts, &mut map);
    add_repeated_u32_boundaries(data, &mut map);
    add_offset_like_boundaries(data, &mut map);

    if !data.is_empty() {
        add_boundary(&mut map, data.len(), 5, BoundaryReason::Alignment4);
    }

    let mut out: Vec<_> = map.into_values().collect();
    out.sort_by_key(|b| b.offset);
    out
}

fn add_boundary(
    map: &mut BTreeMap<usize, CandidateBoundary>,
    offset: usize,
    score: u32,
    reason: BoundaryReason,
) {
    let entry = map.entry(offset).or_insert(CandidateBoundary {
        offset,
        score: 0,
        reasons: vec![],
    });
    entry.score += score;
    if !entry.reasons.contains(&reason) {
        entry.reasons.push(reason);
    }
}

fn add_zero_run_boundaries(
    data: &[u8],
    opts: &SheetProbeOptions,
    map: &mut BTreeMap<usize, CandidateBoundary>,
) {
    let mut i = 0;
    while i < data.len() {
        if data[i] != 0 {
            i += 1;
            continue;
        }

        let start = i;
        while i < data.len() && data[i] == 0 {
            i += 1;
        }

        let len = i - start;
        if len >= opts.zero_run_threshold {
            add_boundary(map, start, 3, BoundaryReason::ZeroRun);
            if i < data.len() {
                add_boundary(map, i, 3, BoundaryReason::ZeroRun);
            }
        }
    }
}

fn add_ascii_burst_boundaries(
    data: &[u8],
    opts: &SheetProbeOptions,
    map: &mut BTreeMap<usize, CandidateBoundary>,
) {
    let mut i = 0;
    while i < data.len() {
        if !is_ascii_printable(data[i]) {
            i += 1;
            continue;
        }

        let start = i;
        while i < data.len() && is_ascii_printable(data[i]) {
            i += 1;
        }

        let len = i - start;
        if len >= opts.ascii_burst_threshold {
            add_boundary(map, start, 2, BoundaryReason::AsciiBurst);
        }
    }
}

fn is_ascii_printable(b: u8) -> bool {
    (0x20..=0x7e).contains(&b) || b == b'\t'
}

fn add_utf16_burst_boundaries(
    data: &[u8],
    opts: &SheetProbeOptions,
    map: &mut BTreeMap<usize, CandidateBoundary>,
) {
    let mut i = 0;
    while i + 1 < data.len() {
        let start = i;
        let mut count = 0usize;

        while i + 1 < data.len() {
            let ch = u16::from_le_bytes([data[i], data[i + 1]]);
            if ch == 0 {
                break;
            }
            if (0x20..=0x7e).contains(&ch) || ch > 0x7f {
                count += 1;
                i += 2;
            } else {
                break;
            }
        }

        if count >= opts.utf16_burst_threshold {
            add_boundary(map, start, 2, BoundaryReason::Utf16Burst);
        }

        if i == start {
            i += 1;
        }
    }
}

fn add_repeated_u32_boundaries(data: &[u8], map: &mut BTreeMap<usize, CandidateBoundary>) {
    if data.len() < 16 {
        return;
    }
    let mut i = 0;
    while i + 16 <= data.len() {
        if i % 4 != 0 {
            i += 1;
            continue;
        }

        let a = u32_le(data, i);
        let b = u32_le(data, i + 4);
        let c = u32_le(data, i + 8);
        let d = u32_le(data, i + 12);

        if a == b && b == c && c == d {
            add_boundary(map, i, 2, BoundaryReason::RepeatedU32Pattern);
        }

        i += 4;
    }
}

fn add_offset_like_boundaries(data: &[u8], map: &mut BTreeMap<usize, CandidateBoundary>) {
    if data.len() < 16 {
        return;
    }
    let mut i = 0;
    while i + 16 <= data.len() {
        if i % 4 != 0 {
            i += 1;
            continue;
        }

        let a = u32_le(data, i) as usize;
        let b = u32_le(data, i + 4) as usize;
        let c = u32_le(data, i + 8) as usize;
        let d = u32_le(data, i + 12) as usize;

        let monotonic = a < b && b < c && c < d;
        let plausible = a < data.len() && b < data.len() && c < data.len() && d < data.len();

        if monotonic && plausible {
            add_boundary(map, i, 3, BoundaryReason::OffsetLikeSequence);
        }

        i += 4;
    }
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn i32_le(data: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn record_type_counts(data: &[u8]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    let mut i = 0usize;
    while i + 2 < data.len() {
        if data[i] == 0x89 {
            let candidate = u16::from_le_bytes([data[i + 1], data[i + 2]]);
            let key = format!("0x{candidate:04X}");
            *counts.entry(key).or_insert(0) += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    counts
}

fn scan_text_runs(data: &[u8], opts: &SheetProbeOptions) -> Vec<SheetTextRun> {
    let mut runs = Vec::new();
    runs.extend(scan_ascii_text_runs(
        data,
        opts.ascii_burst_threshold,
        opts.max_preview_strings,
    ));
    runs.extend(scan_utf16_text_runs(
        data,
        opts.utf16_burst_threshold,
        opts.max_preview_strings,
    ));
    runs.sort_by_key(|run| (run.offset, run.byte_len));
    runs
}

fn scan_ascii_text_runs(data: &[u8], min_len: usize, max_runs: usize) -> Vec<SheetTextRun> {
    let mut runs = Vec::new();
    let mut i = 0usize;
    while i < data.len() && runs.len() < max_runs {
        if !is_ascii_printable(data[i]) {
            i += 1;
            continue;
        }

        let start = i;
        while i < data.len() && is_ascii_printable(data[i]) {
            i += 1;
        }
        let len = i - start;
        if len >= min_len {
            runs.push(SheetTextRun {
                offset: start,
                encoding: SheetTextEncoding::Ascii,
                text: String::from_utf8_lossy(&data[start..i]).to_string(),
                byte_len: len,
            });
        }
    }
    runs
}

fn scan_utf16_text_runs(data: &[u8], min_chars: usize, max_runs: usize) -> Vec<SheetTextRun> {
    let mut runs = Vec::new();
    let mut i = 0usize;
    while i + 1 < data.len() && runs.len() < max_runs {
        let start = i;
        let mut words = Vec::new();
        while i + 1 < data.len() {
            let ch = u16::from_le_bytes([data[i], data[i + 1]]);
            if ch == 0 {
                break;
            }
            if is_plausible_utf16_text_char(ch) {
                words.push(ch);
                i += 2;
            } else {
                break;
            }
        }

        let accepted = words.len() >= min_chars;
        if accepted {
            runs.push(SheetTextRun {
                offset: start,
                encoding: SheetTextEncoding::Utf16Le,
                text: String::from_utf16_lossy(&words),
                byte_len: words.len() * 2,
            });
        }

        if accepted {
            i += 2;
        } else {
            i = start + 1;
        }
    }
    runs
}

fn is_plausible_utf16_text_char(ch: u16) -> bool {
    (0x20..=0x7e).contains(&ch)
        || (0x4e00..=0x9fff).contains(&ch)
        || (0x3040..=0x30ff).contains(&ch)
        || (0xac00..=0xd7af).contains(&ch)
}

fn coordinate_hints(data: &[u8]) -> Vec<SheetCoordinateHint> {
    const MAX_HINTS: usize = 64;

    coordinate_hints_in_range(data, 0, data.len())
        .into_iter()
        .take(MAX_HINTS)
        .collect()
}

fn coordinate_hints_in_range(data: &[u8], start: usize, end: usize) -> Vec<SheetCoordinateHint> {
    const MAX_ABS_COORD: i32 = 1_000_000;

    let mut out = Vec::new();
    let mut i = start.min(data.len());
    let end = end.min(data.len());
    while i + 8 <= end {
        if !i.is_multiple_of(4) {
            i += 1;
            continue;
        }
        let x = i32_le(data, i);
        let y = i32_le(data, i + 4);
        let abs_x = x.checked_abs().unwrap_or(i32::MAX);
        let abs_y = y.checked_abs().unwrap_or(i32::MAX);
        let plausible = (x != 0 || y != 0) && abs_x <= MAX_ABS_COORD && abs_y <= MAX_ABS_COORD;
        if plausible {
            out.push(SheetCoordinateHint { offset: i, x, y });
        }
        i += 4;
    }
    out
}

/// Slice the stream at every boundary whose score meets the threshold,
/// dropping chunks shorter than `opts.min_chunk_len`. Returns at least one
/// chunk whenever `data` is non-empty.
pub fn split_by_boundaries(
    data: &[u8],
    boundaries: &[CandidateBoundary],
    opts: &SheetProbeOptions,
) -> Vec<SheetChunk> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut cuts: Vec<usize> = vec![0];
    for b in boundaries {
        if b.score >= opts.min_boundary_score && b.offset > 0 && b.offset < data.len() {
            cuts.push(b.offset);
        }
    }
    cuts.push(data.len());

    cuts.sort_unstable();
    cuts.dedup();

    let mut chunks = Vec::new();
    for pair in cuts.windows(2) {
        let start = pair[0];
        let end = pair[1];

        if end <= start {
            continue;
        }
        if end - start < opts.min_chunk_len {
            continue;
        }

        chunks.push(summarize_chunk(data, start, end, opts));
    }

    if chunks.is_empty() {
        chunks.push(summarize_chunk(data, 0, data.len(), opts));
    }

    chunks
}

/// Build a single chunk summary for the half-open range `[start..end)`.
pub fn summarize_chunk(
    data: &[u8],
    start: usize,
    end: usize,
    opts: &SheetProbeOptions,
) -> SheetChunk {
    let slice = &data[start..end];

    let ascii_preview =
        crate::parsers::string_scan::scan_ascii_strings(slice, opts.max_preview_strings);
    let utf16_preview =
        crate::parsers::string_scan::scan_utf16le_strings(slice, 4, opts.max_preview_strings);

    let zero_ratio = if slice.is_empty() {
        0.0
    } else {
        slice.iter().filter(|&&b| b == 0).count() as f32 / slice.len() as f32
    };

    let (aligned_u32_density, repeated_u32_hits) = analyze_u32_patterns(slice);
    let kind_hint = classify_chunk(
        &ascii_preview,
        &utf16_preview,
        zero_ratio,
        aligned_u32_density,
        repeated_u32_hits,
    );

    SheetChunk {
        start,
        end,
        len: end - start,
        ascii_preview,
        utf16_preview,
        zero_ratio,
        aligned_u32_density,
        repeated_u32_hits,
        kind_hint,
    }
}

fn analyze_u32_patterns(data: &[u8]) -> (f32, usize) {
    if data.len() < 4 {
        return (0.0, 0);
    }

    let mut total = 0usize;
    let mut offset_like = 0usize;
    let mut repeated_hits = 0usize;
    let mut prev: Option<u32> = None;

    let end = data.len() - (data.len() % 4);
    let mut i = 0;
    while i + 4 <= end {
        let v = u32_le(data, i);
        total += 1;

        if (v as usize) < data.len() {
            offset_like += 1;
        }

        if let Some(p) = prev {
            if p == v {
                repeated_hits += 1;
            }
        }
        prev = Some(v);

        i += 4;
    }

    let density = if total == 0 {
        0.0
    } else {
        offset_like as f32 / total as f32
    };

    (density, repeated_hits)
}

fn classify_chunk(
    ascii_preview: &[String],
    utf16_preview: &[String],
    zero_ratio: f32,
    aligned_u32_density: f32,
    repeated_u32_hits: usize,
) -> SheetChunkKindHint {
    let text_count = ascii_preview.len() + utf16_preview.len();

    if aligned_u32_density > 0.45 && repeated_u32_hits > 1 {
        return SheetChunkKindHint::OffsetTableLike;
    }

    if text_count >= 3 && zero_ratio < 0.20 {
        return SheetChunkKindHint::TextHeavy;
    }

    if text_count == 0 && zero_ratio < 0.10 {
        return SheetChunkKindHint::BinaryDense;
    }

    if text_count > 0 {
        return SheetChunkKindHint::Mixed;
    }

    SheetChunkKindHint::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zero_run_boundaries() {
        let mut data = vec![1u8, 2, 3, 4];
        data.extend(vec![0u8; 16]);
        data.extend(vec![9u8, 8, 7, 6]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(report
            .candidate_boundaries
            .iter()
            .any(|b| b.reasons.contains(&BoundaryReason::ZeroRun)));
    }

    #[test]
    fn detects_ascii_burst() {
        let mut data = vec![0x01u8, 0x02, 0x03];
        data.extend_from_slice(b"HELLO-WORLD-123");
        data.extend_from_slice(&[0x00, 0xFF]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(report
            .candidate_boundaries
            .iter()
            .any(|b| b.reasons.contains(&BoundaryReason::AsciiBurst)));
    }

    #[test]
    fn detects_offset_like_sequence() {
        let mut data = Vec::new();
        for v in [16u32, 32, 64, 96] {
            data.extend(v.to_le_bytes());
        }
        data.extend(vec![0xAAu8; 128]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(report
            .candidate_boundaries
            .iter()
            .any(|b| b.reasons.contains(&BoundaryReason::OffsetLikeSequence)));
    }

    #[test]
    fn chunk_ranges_are_ordered_and_non_empty() {
        let mut data = vec![0x11u8; 64];
        data.extend_from_slice(b"DWG-0201GP06-01");
        data.extend(vec![0u8; 16]);
        data.extend(vec![0x22u8; 64]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(!report.chunks.is_empty());
        let mut prev_end = 0usize;
        for chunk in &report.chunks {
            assert!(chunk.start < chunk.end);
            assert_eq!(chunk.len, chunk.end - chunk.start);
            assert!(chunk.start >= prev_end);
            prev_end = chunk.end;
        }
    }

    #[test]
    fn repeated_u32_pattern_detected() {
        let mut data = Vec::new();
        for _ in 0..4 {
            data.extend(0xDEAD_BEEFu32.to_le_bytes());
        }
        data.extend(vec![0x55u8; 64]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(report
            .candidate_boundaries
            .iter()
            .any(|b| b.reasons.contains(&BoundaryReason::RepeatedU32Pattern)));
    }

    #[test]
    fn records_marker_following_u16_type_counts() {
        let mut data = vec![0x11u8; 32];
        data.extend_from_slice(&[0x89, 0xCE, 0x00, 0xAA]);
        data.extend_from_slice(&[0x89, 0xCE, 0x00, 0xBB]);
        data.extend_from_slice(&[0x89, 0x02, 0x00, 0xCC]);
        data.extend(vec![0x22u8; 32]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert_eq!(report.record_type_counts.get("0x00CE"), Some(&2));
        assert_eq!(report.record_type_counts.get("0x0002"), Some(&1));
    }

    #[test]
    fn record_type_counts_remain_probe_evidence_only() {
        let data = [
            0x89, 0xCE, 0x00, 0xAA, 0x89, 0xCE, 0x00, 0xBB, 0x89, 0x02, 0x00, 0xCC,
        ];
        let opts = SheetProbeOptions::default();

        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert_eq!(report.record_type_counts.get("0x00CE"), Some(&2));
        assert_eq!(report.record_type_counts.get("0x0002"), Some(&1));
        assert!(
            report.text_runs.is_empty(),
            "marker counts must not manufacture decoded text evidence"
        );
        assert!(
            report.coordinate_hints.is_empty(),
            "marker counts must not manufacture coordinate semantics"
        );
    }

    #[test]
    fn captures_text_runs_with_offsets() {
        let mut data = vec![0x00u8, 0xFF, 0xAA, 0x01];
        data.extend_from_slice(b"ASCII-TAGS");
        data.extend_from_slice(&[0x00, 0x00]);
        let utf16_offset = data.len();
        for ch in "PUMP-101".encode_utf16() {
            data.extend_from_slice(&ch.to_le_bytes());
        }

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);
        assert!(report.text_runs.iter().any(|run| {
            matches!(run.encoding, SheetTextEncoding::Ascii)
                && run.offset == 4
                && run.text == "ASCII-TAGS"
        }));
        assert!(report.text_runs.iter().any(|run| {
            matches!(run.encoding, SheetTextEncoding::Utf16Le)
                && run.offset == utf16_offset
                && run.text == "PUMP-101"
        }));
    }

    #[test]
    fn captures_coordinate_pair_hints() {
        let mut data = vec![0x00u8; 16];
        let coord_offset = data.len();
        data.extend_from_slice(&1200i32.to_le_bytes());
        data.extend_from_slice(&(-450i32).to_le_bytes());
        data.extend(vec![0x33u8; 32]);

        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &data, &opts);

        assert!(report
            .coordinate_hints
            .iter()
            .any(|hint| { hint.offset == coord_offset && hint.x == 1200 && hint.y == -450 }));
    }

    #[test]
    fn text_window_candidates_link_text_to_nearby_quality_coordinates_without_promotion() {
        let text = SheetTextRun {
            offset: 16,
            encoding: SheetTextEncoding::Ascii,
            text: "PUMP-101".to_string(),
            byte_len: 8,
        };
        let coordinate = SheetCoordinateHint {
            offset: 40,
            x: 1200,
            y: -450,
        };
        let chunks = vec![SheetChunk {
            start: 0,
            end: 96,
            len: 96,
            ascii_preview: vec![],
            utf16_preview: vec![],
            zero_ratio: 0.0,
            aligned_u32_density: 0.0,
            repeated_u32_hits: 0,
            kind_hint: SheetChunkKindHint::Mixed,
        }];

        let candidates = sheet_text_window_candidates(&[text], &[coordinate], &chunks, 32);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].text_offset, 16);
        assert_eq!(candidates[0].text, "PUMP-101");
        assert_eq!(candidates[0].coordinate_offset, 40);
        assert_eq!(candidates[0].x, 1200);
        assert_eq!(candidates[0].y, -450);
        assert!(candidates[0].same_chunk);
        assert_eq!(candidates[0].byte_distance, 16);
        assert!(candidates[0].quality_passed);
        assert_eq!(candidates[0].chunk_start, Some(0));
        assert_eq!(candidates[0].chunk_end, Some(96));
    }

    #[test]
    fn text_window_scoring_rejects_binary_like_text_before_position_scoring() {
        assert!(is_high_quality_text_candidate("PUMP-101"));
        assert!(is_high_quality_text_candidate("LINE A-101"));
        assert!(!is_high_quality_text_candidate("봽렎卆툦"));
        assert!(!is_high_quality_text_candidate(" 060101럀"));

        let good = SheetTextWindowCandidate {
            text_offset: 16,
            text_byte_len: 8,
            text_encoding: SheetTextEncoding::Ascii,
            text: "PUMP-101".to_string(),
            coordinate_offset: 40,
            x: 1200,
            y: -450,
            same_chunk: true,
            byte_distance: 16,
            quality_passed: true,
            chunk_start: Some(0),
            chunk_end: Some(96),
        };
        let binary_like = SheetTextWindowCandidate {
            text_offset: 80,
            text_byte_len: 8,
            text_encoding: SheetTextEncoding::Utf16Le,
            text: "봽렎卆툦".to_string(),
            coordinate_offset: 104,
            x: 1200,
            y: -450,
            same_chunk: true,
            byte_distance: 16,
            quality_passed: true,
            chunk_start: Some(0),
            chunk_end: Some(128),
        };

        let scores = score_sheet_text_window_candidates(&[binary_like, good]);

        assert_eq!(scores[0].candidate.text, "PUMP-101");
        assert!(scores[0].score > 0);
        assert!(scores[0]
            .reasons
            .contains(&SheetTextWindowScoreReason::TextQualityPassed));
        assert_eq!(scores[1].candidate.text, "봽렎卆툦");
        assert!(scores[1].score < 0);
        assert!(scores[1]
            .reasons
            .contains(&SheetTextWindowScoreReason::TextQualityRejected));
    }

    #[test]
    fn field_x_windows_capture_nearby_coordinate_candidates_without_promoting_geometry() {
        let mut data = vec![0xAAu8; 16];
        let field_x_offset = data.len();
        data.extend_from_slice(&229u32.to_le_bytes());
        data.extend_from_slice(&[0x00; 4]);
        let coordinate_offset = data.len();
        data.extend_from_slice(&1200i32.to_le_bytes());
        data.extend_from_slice(&(-450i32).to_le_bytes());
        data.extend_from_slice(&[0xBB; 16]);

        let windows = field_x_windows(&data, &[229, 740], 16);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].field_x, 229);
        assert_eq!(windows[0].offset, field_x_offset);
        assert_eq!(windows[0].endpoint_record_start, None);
        assert!(windows[0]
            .nearby_coordinates
            .iter()
            .any(|hint| hint.offset == coordinate_offset && hint.x == 1200 && hint.y == -450));
    }

    #[test]
    fn field_x_windows_mark_hits_inside_endpoint_record_signatures() {
        let mut data = vec![0xAAu8; 12];
        let endpoint_record_start = data.len();
        data.extend_from_slice(&949u32.to_le_bytes());
        data.extend_from_slice(&0x0000_0006u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 6]);
        data.extend_from_slice(&0x0002u16.to_le_bytes());
        data.extend_from_slice(&229u32.to_le_bytes());
        data.extend_from_slice(&0x0001u16.to_le_bytes());
        data.extend_from_slice(&326u32.to_le_bytes());

        let windows = field_x_windows(&data, &[949, 229, 326], 8);

        assert_eq!(windows.len(), 3);
        assert!(windows
            .iter()
            .all(|window| window.endpoint_record_start == Some(endpoint_record_start)));
    }

    #[test]
    fn field_x_windows_do_not_mark_truncated_endpoint_signatures() {
        let mut data = vec![0xAAu8; 12];
        data.extend_from_slice(&949u32.to_le_bytes());
        data.extend_from_slice(&0x0000_0006u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 6]);
        data.extend_from_slice(&0x0002u16.to_le_bytes());
        data.extend_from_slice(&229u32.to_le_bytes());
        data.extend_from_slice(&0x0001u16.to_le_bytes());
        data.extend_from_slice(&326u32.to_le_bytes());
        data.pop();
        assert_eq!(data.len(), 12 + 25);

        let windows = field_x_windows(&data, &[949, 229], 8);

        assert!(
            !windows.is_empty(),
            "truncated bytes should still be searchable as field_x probe evidence"
        );
        assert!(windows
            .iter()
            .all(|window| window.endpoint_record_start.is_none()));
    }

    #[test]
    fn score_field_x_windows_downranks_endpoint_record_references() {
        let mut data = vec![0xAAu8; 12];
        data.extend_from_slice(&949u32.to_le_bytes());
        data.extend_from_slice(&0x0000_0006u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 6]);
        data.extend_from_slice(&0x0002u16.to_le_bytes());
        data.extend_from_slice(&229u32.to_le_bytes());
        data.extend_from_slice(&0x0001u16.to_le_bytes());
        data.extend_from_slice(&326u32.to_le_bytes());
        let windows = field_x_windows(&data, &[229], 8);
        let object_fields = HashSet::from([229]);

        let scores = score_field_x_windows(&windows, &object_fields);

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].score, -100);
        assert_eq!(scores[0].candidate_position, None);
        assert_eq!(
            scores[0].reasons,
            vec![SheetFieldXWindowScoreReason::EndpointRecordReference]
        );
    }

    #[test]
    fn score_field_x_windows_scores_non_endpoint_candidates_without_promoting() {
        let mut data = vec![0xAAu8; 16];
        data.extend_from_slice(&229u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        let coordinate_offset = data.len();
        data.extend_from_slice(&1200i32.to_le_bytes());
        data.extend_from_slice(&(-450i32).to_le_bytes());
        let windows = field_x_windows(&data, &[229], 16);
        let object_fields = HashSet::from([229]);

        let scores = score_field_x_windows(&windows, &object_fields);

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].score, 25);
        assert_eq!(
            scores[0].candidate_position,
            Some(SheetCoordinateHint {
                offset: coordinate_offset,
                x: 1200,
                y: -450,
            })
        );
        assert_eq!(
            scores[0].reasons,
            vec![
                SheetFieldXWindowScoreReason::NonEndpointHit,
                SheetFieldXWindowScoreReason::ObjectFieldResolves,
                SheetFieldXWindowScoreReason::CoordinateCandidateAtDelta { delta: 8 },
            ]
        );
    }

    #[test]
    fn score_field_x_windows_marks_repeated_coordinate_delta_across_fields() {
        let mut data = vec![0xAAu8; 16];
        data.extend_from_slice(&101u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        data.extend_from_slice(&1200i32.to_le_bytes());
        data.extend_from_slice(&(-450i32).to_le_bytes());
        data.extend_from_slice(&[0xAA; 8]);
        data.extend_from_slice(&202u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        data.extend_from_slice(&2400i32.to_le_bytes());
        data.extend_from_slice(&(-900i32).to_le_bytes());
        let windows = field_x_windows(&data, &[101, 202], 16);
        let object_fields = HashSet::from([101, 202]);

        let scores = score_field_x_windows(&windows, &object_fields);

        assert_eq!(scores.len(), 2);
        assert!(scores.iter().all(|score| {
            score.score == 65
                && score.reasons.contains(
                    &SheetFieldXWindowScoreReason::RepeatedDeltaAcrossFields {
                        delta: 8,
                        support: 2,
                    },
                )
        }));
    }

    #[test]
    fn field_x_window_features_map_hits_and_coordinates_to_containing_chunks() {
        let mut data = vec![0xAAu8; 16];
        data.extend_from_slice(&101u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        data.extend_from_slice(&1200i32.to_le_bytes());
        data.extend_from_slice(&(-450i32).to_le_bytes());
        data.extend_from_slice(&[0xAA; 16]);
        let windows = field_x_windows(&data, &[101], 16);
        let chunks = vec![SheetChunk {
            start: 8,
            end: 40,
            len: 32,
            ascii_preview: Vec::new(),
            utf16_preview: Vec::new(),
            zero_ratio: 0.0,
            aligned_u32_density: 0.0,
            repeated_u32_hits: 0,
            kind_hint: SheetChunkKindHint::BinaryDense,
        }];

        let features = field_x_window_features(&data, &windows, &chunks);

        assert_eq!(features.len(), 1);
        assert_eq!(features[0].field_x, 101);
        assert_eq!(features[0].chunk_start, Some(8));
        assert_eq!(features[0].chunk_end, Some(40));
        assert_eq!(features[0].field_delta_from_chunk, Some(8));
        assert_eq!(features[0].coordinate_delta_from_chunk, Some(16));
        assert_eq!(
            features[0].candidate_position,
            Some(SheetCoordinateHint {
                offset: 24,
                x: 1200,
                y: -450,
            })
        );
        assert!(features[0]
            .stable_markers
            .iter()
            .any(|marker| marker.offset == 24 && marker.value_u32 == 1200));
    }

    #[test]
    fn stable_chunk_shape_support_counts_distinct_non_endpoint_fields() {
        let features = vec![
            feature_for_support(101, None, Some(10), Some(20), vec![]),
            feature_for_support(101, None, Some(10), Some(20), vec![]),
            feature_for_support(202, None, Some(10), Some(20), vec![]),
            feature_for_support(303, Some(64), Some(10), Some(20), vec![]),
        ];

        let support = stable_chunk_shape_support(&features);

        assert_eq!(support.get(&(10, 20)), Some(&2));
    }

    #[test]
    fn record_shape_classifier_groups_distinct_non_endpoint_field_shapes() {
        let features = vec![
            feature_for_support(101, None, Some(10), Some(20), vec![]),
            feature_for_support(101, None, Some(10), Some(20), vec![]),
            feature_for_support(202, None, Some(10), Some(20), vec![]),
            feature_for_support(303, None, Some(12), Some(28), vec![]),
            feature_for_support(404, Some(64), Some(10), Some(20), vec![]),
            feature_for_support(505, None, Some(10), None, vec![]),
        ];

        let classes = classify_field_x_record_shapes(&features);

        assert_eq!(classes.len(), 2);
        assert_eq!(classes[0].field_delta_from_chunk, 10);
        assert_eq!(classes[0].coordinate_delta_from_chunk, 20);
        assert_eq!(classes[0].support, 2);
        assert_eq!(classes[0].field_xs, vec![101, 202]);
        assert_eq!(classes[0].example_field_offset, 16);
        assert_eq!(classes[0].example_coordinate_offset, Some(28));
        assert_eq!(classes[1].field_delta_from_chunk, 12);
        assert_eq!(classes[1].coordinate_delta_from_chunk, 28);
        assert_eq!(classes[1].support, 1);
        assert_eq!(classes[1].field_xs, vec![303]);
    }

    #[test]
    fn promotion_gate_summary_requires_score_identity_and_stable_shape() {
        let scores = vec![
            field_x_score_for_gate(
                101,
                80,
                vec![
                    SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                        kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                        delta: 12,
                    },
                    SheetFieldXWindowScoreReason::StableChunkShape {
                        field_delta: 10,
                        coordinate_delta: 20,
                        support: 2,
                    },
                ],
            ),
            field_x_score_for_gate(
                202,
                80,
                vec![SheetFieldXWindowScoreReason::StableChunkShape {
                    field_delta: 10,
                    coordinate_delta: 20,
                    support: 2,
                }],
            ),
            field_x_score_for_gate(
                303,
                45,
                vec![
                    SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                        kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                        delta: 12,
                    },
                    SheetFieldXWindowScoreReason::StableChunkShape {
                        field_delta: 10,
                        coordinate_delta: 20,
                        support: 2,
                    },
                ],
            ),
        ];

        let summary = summarize_object_geometry_promotion_gate(&scores, 70);

        assert_eq!(summary.total_candidates, 3);
        assert_eq!(summary.identity_supported, 2);
        assert_eq!(summary.stable_shape_supported, 3);
        assert_eq!(summary.over_threshold, 2);
        assert_eq!(summary.promotable_candidates, 1);
    }

    #[test]
    fn stable_marker_support_filters_structural_constants_and_endpoint_hits() {
        let features = vec![
            feature_for_support(
                101,
                None,
                Some(10),
                Some(20),
                vec![(2, 65_536), (6, 8), (4, 123_456)],
            ),
            feature_for_support(202, None, Some(10), Some(20), vec![(4, 123_456)]),
            feature_for_support(303, Some(64), Some(10), Some(20), vec![(4, 123_456)]),
        ];

        let support = stable_marker_support(&features);

        assert_eq!(support.get(&(2, 65_536)), None);
        assert_eq!(support.get(&(4, 123_456)), Some(&2));
    }

    #[test]
    fn high_quality_coordinate_filter_rejects_structural_pairs() {
        for (x, y) in [
            (1536, 0),
            (6, 8),
            (0, 65_536),
            (65_537, -65_535),
            (56, 664),
            (35_328, 1_536),
            (196_727, 196_712),
            (131_067, 970),
        ] {
            assert!(
                !is_high_quality_coordinate_candidate(&SheetCoordinateHint { offset: 0, x, y }),
                "expected ({x}, {y}) to be rejected"
            );
        }
    }

    #[test]
    fn high_quality_coordinate_filter_accepts_drawing_like_pairs() {
        assert!(is_high_quality_coordinate_candidate(&SheetCoordinateHint {
            offset: 0,
            x: 1200,
            y: -450,
        }));
    }

    #[test]
    fn score_field_x_window_features_adds_stable_shape_and_marker_reasons() {
        let features = vec![
            feature_for_support(101, None, Some(10), Some(20), vec![(-28, 3_194_542_878)]),
            feature_for_support(202, None, Some(10), Some(20), vec![(-28, 3_194_542_878)]),
            feature_for_support(303, None, Some(10), Some(20), vec![(-28, 3_194_542_878)]),
        ];
        let object_fields = HashSet::from([101, 202, 303]);

        let scores = score_field_x_window_features(&features, &object_fields);

        assert!(scores.iter().all(|score| {
            score.score == 70
                && score
                    .reasons
                    .contains(&SheetFieldXWindowScoreReason::StableChunkShape {
                        field_delta: 10,
                        coordinate_delta: 20,
                        support: 3,
                    })
                && score
                    .reasons
                    .contains(&SheetFieldXWindowScoreReason::StableMarkerNearby {
                        delta: -28,
                        value_u32: 3_194_542_878,
                        support: 3,
                    })
        }));
    }

    #[test]
    fn identity_index_maps_field_x_to_source_ids() {
        let index = sheet_identity_index_from_trailers(&[
            trailer_with_identity(35, 0x6009, 0x109, Some("0123456789ABCDEF0123456789ABCDEF")),
            trailer_with_identity(37, 0x600B, 0x0F6, None),
        ]);

        let identity = index.by_field_x.get(&35).expect("object identity");
        assert_eq!(identity.field_x, 35);
        assert_eq!(identity.record_id, 0x6009);
        assert_eq!(identity.class_id, 0x109);
        assert_eq!(
            identity.drawing_id.as_deref(),
            Some("0123456789ABCDEF0123456789ABCDEF")
        );
        assert_eq!(
            index.field_x_for_drawing_id("0123456789abcdef0123456789abcdef"),
            Some(35)
        );
        assert!(
            !index.by_field_x.contains_key(&37),
            "relationship trailers without DrawingID are not object identities"
        );
    }

    #[test]
    fn field_x_window_identities_find_same_object_record_id() {
        let mut data = vec![0xAAu8; 16];
        let field_x_offset = data.len();
        data.extend_from_slice(&35u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        let record_id_offset = data.len();
        data.extend_from_slice(&0x6009u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 16]);
        let windows = field_x_windows(&data, &[35], 16);
        let index = sheet_identity_index_from_trailers(&[trailer_with_identity(
            35,
            0x6009,
            0x109,
            Some("0123456789ABCDEF0123456789ABCDEF"),
        )]);

        let identities = field_x_window_identities(&data, &windows, &index);

        assert_eq!(
            identities,
            vec![SheetFieldXWindowIdentity {
                field_x: 35,
                offset: record_id_offset,
                delta_from_field: (record_id_offset as isize) - (field_x_offset as isize),
                kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                value: SheetFieldXWindowIdentityValue::U32(0x6009),
                resolves_to_field_x: Some(35),
                resolves_to_same_object: true,
            }]
        );
    }

    #[test]
    fn field_x_window_identities_reject_wrong_object_record_id() {
        let mut data = vec![0xAAu8; 16];
        let field_x_offset = data.len();
        data.extend_from_slice(&35u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        let record_id_offset = data.len();
        data.extend_from_slice(&0x7001u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 16]);
        let windows = field_x_windows(&data, &[35], 16);
        let index = sheet_identity_index_from_trailers(&[trailer_with_identity(
            99,
            0x7001,
            0x109,
            Some("FEDCBA9876543210FEDCBA9876543210"),
        )]);

        let identities = field_x_window_identities(&data, &windows, &index);

        assert_eq!(
            identities,
            vec![SheetFieldXWindowIdentity {
                field_x: 35,
                offset: record_id_offset,
                delta_from_field: (record_id_offset as isize) - (field_x_offset as isize),
                kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                value: SheetFieldXWindowIdentityValue::U32(0x7001),
                resolves_to_field_x: Some(99),
                resolves_to_same_object: false,
            }]
        );
    }

    #[test]
    fn field_x_window_identities_resolve_ascii_drawing_id_case_insensitively() {
        let mut data = vec![0xAAu8; 16];
        let field_x_offset = data.len();
        data.extend_from_slice(&35u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        let drawing_id_offset = data.len();
        data.extend_from_slice(b"0123456789abcdef0123456789abcdef");
        data.extend_from_slice(&[0xAA; 16]);
        let windows = field_x_windows(&data, &[35], 48);
        let index = sheet_identity_index_from_trailers(&[trailer_with_identity(
            35,
            0x6009,
            0x109,
            Some("0123456789ABCDEF0123456789ABCDEF"),
        )]);

        let identities = field_x_window_identities(&data, &windows, &index);

        assert_eq!(
            identities,
            vec![SheetFieldXWindowIdentity {
                field_x: 35,
                offset: drawing_id_offset,
                delta_from_field: (drawing_id_offset as isize) - (field_x_offset as isize),
                kind: SheetFieldXWindowIdentityKind::DrawingIdAscii,
                value: SheetFieldXWindowIdentityValue::Text(
                    "0123456789abcdef0123456789abcdef".to_string()
                ),
                resolves_to_field_x: Some(35),
                resolves_to_same_object: true,
            }]
        );
    }

    #[test]
    fn field_x_window_identities_resolve_utf16_drawing_id() {
        let mut data = vec![0xAAu8; 16];
        let field_x_offset = data.len();
        data.extend_from_slice(&35u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 4]);
        let drawing_id_offset = data.len();
        for ch in "0123456789abcdef0123456789abcdef".encode_utf16() {
            data.extend_from_slice(&ch.to_le_bytes());
        }
        data.extend_from_slice(&[0xAA; 16]);
        let windows = field_x_windows(&data, &[35], 96);
        let index = sheet_identity_index_from_trailers(&[trailer_with_identity(
            35,
            0x6009,
            0x109,
            Some("0123456789ABCDEF0123456789ABCDEF"),
        )]);

        let identities = field_x_window_identities(&data, &windows, &index);

        assert_eq!(
            identities,
            vec![SheetFieldXWindowIdentity {
                field_x: 35,
                offset: drawing_id_offset,
                delta_from_field: (drawing_id_offset as isize) - (field_x_offset as isize),
                kind: SheetFieldXWindowIdentityKind::DrawingIdUtf16Le,
                value: SheetFieldXWindowIdentityValue::Text(
                    "0123456789abcdef0123456789abcdef".to_string()
                ),
                resolves_to_field_x: Some(35),
                resolves_to_same_object: true,
            }]
        );
    }

    #[test]
    fn score_field_x_window_features_adds_graphic_identity_nearby_only_when_resolved() {
        let features = vec![
            feature_for_support(101, None, Some(10), Some(20), vec![]),
            feature_for_support(202, None, Some(10), Some(20), vec![]),
        ];
        let identities = vec![
            SheetFieldXWindowIdentity {
                field_x: 101,
                offset: 20,
                delta_from_field: 4,
                kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                value: SheetFieldXWindowIdentityValue::U32(0x6009),
                resolves_to_field_x: Some(101),
                resolves_to_same_object: true,
            },
            SheetFieldXWindowIdentity {
                field_x: 202,
                offset: 20,
                delta_from_field: 4,
                kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                value: SheetFieldXWindowIdentityValue::U32(0x7001),
                resolves_to_field_x: Some(999),
                resolves_to_same_object: false,
            },
        ];
        let object_fields = HashSet::from([101, 202]);

        let scores =
            score_field_x_window_features_with_identities(&features, &object_fields, &identities);

        let score_101 = scores.iter().find(|score| score.field_x == 101).unwrap();
        let score_202 = scores.iter().find(|score| score.field_x == 202).unwrap();
        assert_eq!(score_101.score, 85);
        assert!(score_101
            .reasons
            .contains(&SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                delta: 4,
            }));
        assert_eq!(score_202.score, 50);
        assert!(!score_202.reasons.iter().any(|reason| {
            matches!(
                reason,
                SheetFieldXWindowScoreReason::GraphicIdentityNearby { .. }
            )
        }));
    }

    #[test]
    fn top_candidate_record_dumps_rank_scores_and_keep_hex_windows() {
        let data: Vec<u8> = (0u8..96).collect();
        let field_scores = vec![
            SheetFieldXWindowScore {
                field_x: 101,
                offset: 20,
                score: 10,
                reasons: vec![SheetFieldXWindowScoreReason::NonEndpointHit],
                candidate_position: Some(SheetCoordinateHint {
                    offset: 40,
                    x: 1200,
                    y: -450,
                }),
            },
            SheetFieldXWindowScore {
                field_x: 202,
                offset: 24,
                score: 80,
                reasons: vec![SheetFieldXWindowScoreReason::GraphicIdentityNearby {
                    kind: SheetFieldXWindowIdentityKind::TrailerRecordId,
                    delta: 8,
                }],
                candidate_position: None,
            },
        ];
        let text_scores = vec![
            SheetTextWindowScore {
                candidate: SheetTextWindowCandidate {
                    text_offset: 12,
                    text_byte_len: 8,
                    text_encoding: SheetTextEncoding::Ascii,
                    text: "LOW".into(),
                    coordinate_offset: 48,
                    x: 1,
                    y: 2,
                    same_chunk: false,
                    byte_distance: 28,
                    quality_passed: false,
                    chunk_start: None,
                    chunk_end: None,
                },
                score: -50,
                reasons: vec![SheetTextWindowScoreReason::TextQualityRejected],
            },
            SheetTextWindowScore {
                candidate: SheetTextWindowCandidate {
                    text_offset: 16,
                    text_byte_len: 8,
                    text_encoding: SheetTextEncoding::Ascii,
                    text: "PUMP-101".into(),
                    coordinate_offset: 56,
                    x: 1200,
                    y: -450,
                    same_chunk: true,
                    byte_distance: 32,
                    quality_passed: true,
                    chunk_start: Some(0),
                    chunk_end: Some(96),
                },
                score: 90,
                reasons: vec![SheetTextWindowScoreReason::TextQualityPassed],
            },
        ];

        let field_dumps = top_field_x_candidate_record_dumps(&data, &field_scores, 1, 4);
        let text_dumps = top_text_candidate_record_dumps(&data, &text_scores, 1, 4);

        assert_eq!(field_dumps.len(), 1);
        assert_eq!(field_dumps[0].rank, 1);
        assert_eq!(field_dumps[0].field_x, 202);
        assert_eq!(field_dumps[0].field_offset, 24);
        assert_eq!(field_dumps[0].field_window.start, 20);
        assert_eq!(field_dumps[0].field_window.end, 28);
        assert!(field_dumps[0].coordinate_window.is_none());

        assert_eq!(text_dumps.len(), 1);
        assert_eq!(text_dumps[0].rank, 1);
        assert_eq!(text_dumps[0].text, "PUMP-101");
        assert_eq!(text_dumps[0].text_window.start, 12);
        assert_eq!(text_dumps[0].text_window.end, 20);
        assert_eq!(text_dumps[0].coordinate_window.start, 52);
        assert_eq!(text_dumps[0].coordinate_window.end, 60);
        assert!(text_dumps[0].text_window.hex.contains("0C 0D 0E 0F"));
    }

    fn feature_for_support(
        field_x: u32,
        endpoint_record_start: Option<usize>,
        field_delta_from_chunk: Option<isize>,
        coordinate_delta_from_chunk: Option<isize>,
        markers: Vec<(isize, u32)>,
    ) -> SheetFieldXWindowFeatures {
        SheetFieldXWindowFeatures {
            field_x,
            offset: 16,
            endpoint_record_start,
            chunk_start: Some(8),
            chunk_end: Some(40),
            field_delta_from_chunk,
            coordinate_delta_from_chunk,
            candidate_position: coordinate_delta_from_chunk.map(|delta| SheetCoordinateHint {
                offset: 8usize.saturating_add_signed(delta),
                x: 1200,
                y: -450,
            }),
            stable_markers: markers
                .into_iter()
                .map(|(delta_from_field, value_u32)| SheetWindowMarker {
                    offset: 16usize.saturating_add_signed(delta_from_field),
                    delta_from_field,
                    value_u32,
                })
                .collect(),
        }
    }

    fn field_x_score_for_gate(
        field_x: u32,
        score: i32,
        reasons: Vec<SheetFieldXWindowScoreReason>,
    ) -> SheetFieldXWindowScore {
        SheetFieldXWindowScore {
            field_x,
            offset: 16,
            score,
            reasons,
            candidate_position: Some(SheetCoordinateHint {
                offset: 28,
                x: 1200,
                y: -450,
            }),
        }
    }

    fn trailer_with_identity(
        field_x: u32,
        record_id: u32,
        class_id: u32,
        drawing_id: Option<&str>,
    ) -> crate::model::DaRecordTrailer {
        crate::model::DaRecordTrailer {
            record_start: 0,
            trailer_offset: 16,
            size: 42,
            record_id,
            field_x,
            class_id,
            drawing_id: drawing_id.map(str::to_string),
            relationship_guid: None,
        }
    }

    #[test]
    fn empty_input_is_safe() {
        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &[], &opts);
        assert_eq!(report.size, 0);
        assert!(report.chunks.is_empty());
    }
}
