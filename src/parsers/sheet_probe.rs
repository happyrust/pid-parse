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
use std::collections::BTreeMap;

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
    const MAX_ABS_COORD: i32 = 1_000_000;

    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= data.len() && out.len() < MAX_HINTS {
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
    fn empty_input_is_safe() {
        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &[], &opts);
        assert_eq!(report.size, 0);
        assert!(report.chunks.is_empty());
    }
}
