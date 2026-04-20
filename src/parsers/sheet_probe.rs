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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetProbeOptions {
    pub min_chunk_len: usize,
    pub max_preview_strings: usize,
    pub zero_run_threshold: usize,
    pub ascii_burst_threshold: usize,
    pub utf16_burst_threshold: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetProbeReport {
    pub sheet_name: String,
    pub path: String,
    pub size: u64,
    pub candidate_boundaries: Vec<CandidateBoundary>,
    pub chunks: Vec<SheetChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateBoundary {
    pub offset: usize,
    pub score: u32,
    pub reasons: Vec<BoundaryReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryReason {
    ZeroRun,
    AsciiBurst,
    Utf16Burst,
    Alignment4,
    Alignment8,
    RepeatedU32Pattern,
    OffsetLikeSequence,
    MarkerTransition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetChunk {
    pub start: usize,
    pub end: usize,
    pub len: usize,
    pub ascii_preview: Vec<String>,
    pub utf16_preview: Vec<String>,
    pub zero_ratio: f32,
    pub aligned_u32_density: f32,
    pub repeated_u32_hits: usize,
    pub kind_hint: SheetChunkKindHint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetChunkKindHint {
    TextHeavy,
    BinaryDense,
    Mixed,
    OffsetTableLike,
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

    SheetProbeReport {
        sheet_name: sheet_name.to_string(),
        path: path.to_string(),
        size: data.len() as u64,
        candidate_boundaries,
        chunks,
    }
}

/// Collect every candidate boundary offset along with the heuristics that
/// voted for it. Same-offset hits are merged (scores summed, reasons
/// deduped).
pub fn find_candidate_boundaries(
    data: &[u8],
    opts: &SheetProbeOptions,
) -> Vec<CandidateBoundary> {
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

fn add_repeated_u32_boundaries(
    data: &[u8],
    map: &mut BTreeMap<usize, CandidateBoundary>,
) {
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

fn add_offset_like_boundaries(
    data: &[u8],
    map: &mut BTreeMap<usize, CandidateBoundary>,
) {
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
        let plausible =
            a < data.len() && b < data.len() && c < data.len() && d < data.len();

        if monotonic && plausible {
            add_boundary(map, i, 3, BoundaryReason::OffsetLikeSequence);
        }

        i += 4;
    }
}

fn u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
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
            data.extend(0xDEADBEEFu32.to_le_bytes());
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
    fn empty_input_is_safe() {
        let opts = SheetProbeOptions::default();
        let report = probe_sheet_stream("SheetX", "/SheetX", &[], &opts);
        assert_eq!(report.size, 0);
        assert!(report.chunks.is_empty());
    }
}
