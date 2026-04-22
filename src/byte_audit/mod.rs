//! Byte-level audit infrastructure (roadmap Phase 4 / Phase 12b-1).
//!
//! The top-level [`inspect::coverage`](crate::inspect::coverage) module
//! answers coarse-grained *stream-level* questions ("is this stream known
//! / fully-decoded / partially-decoded / unknown?"). This module answers
//! the finer-grained *byte-level* question: for every byte inside a
//! stream, did any parser claim it?
//!
//! The immediate 12b-1 scope is deliberately minimal — see
//! `docs/plans/2026-04-22-phase-12b-1-minimal-byte-audit-scaffold-plan.md`
//! — so downstream parser migrations can follow a stable template. The
//! wider Phase 12b plan
//! (`docs/plans/2026-04-21-phase-12b-byte-audit-framework.md`) covers
//! full-fleet migration, aggregate reporting, and CI regression guarding.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A half-open byte range `[start, end)` inside some stream. Ranges are
/// stored with `u64` coordinates because PID streams can in theory exceed
/// 4 GiB (CFB sector tables allow it even if real-world SmartPlant
/// fixtures never reach that size).
///
/// Empty ranges (`start >= end`) are permitted but never produced by the
/// builder logic; callers may receive them when deserializing older data
/// and should treat them as zero-length.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    /// Construct a range. When `end < start` the range is treated as
    /// empty (length 0) rather than panicking — this keeps range
    /// arithmetic robust against bad parser code without propagating an
    /// error type through every builder call.
    pub fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// Inclusive-exclusive length of the range. Saturates at 0 for empty
    /// / inverted ranges.
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Whether `self` and `other` share at least one byte.
    ///
    /// Adjacent-but-disjoint ranges (e.g. `[0..4]` and `[4..8]`) are
    /// **not** considered overlapping — the end is exclusive.
    pub fn overlaps(&self, other: &Self) -> bool {
        !self.is_empty() && !other.is_empty() && self.start < other.end && other.start < self.end
    }

    /// Whether the absolute byte offset `offset` sits inside the range.
    pub fn contains_offset(&self, offset: u64) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// How sure is a parser about the bytes it just claimed?
///
/// Mirrors the language used by Phase 11a-probe / 11b-probe (`Probed`
/// for byte-layout-but-no-semantics) and the long-standing `Decoded`
/// verbiage used throughout `src/inspect/coverage.rs`. `Raw` exists for
/// bytes that are deliberately passed through without interpretation
/// (e.g. attached as opaque `prefix_bytes` today, or that Writer's
/// passthrough mode preserves verbatim).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TraceConfidence {
    /// Parser claims stable semantic meaning for these bytes.
    Decoded,
    /// Parser isolated the byte layout but intentionally declines to
    /// name fields (Phase 11a-probe / 11b-probe style).
    Probed,
    /// Bytes passed through without interpretation.
    Raw,
}

/// Result of running one parser against one stream: which bytes it
/// claimed, with what confidence, and which bytes it left untouched.
///
/// `consumed_ranges` are sorted by `start`, same-confidence adjacent /
/// overlapping ranges are merged, and `leftover_ranges` are the
/// complement inside `[0, total_bytes)`. `ranges_by_confidence` is a
/// convenience projection of the merged ranges keyed by their
/// confidence bucket — useful for UI rendering and future aggregate
/// reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParserTrace {
    pub parser_name: String,
    pub stream_path: String,
    pub total_bytes: u64,
    pub consumed_ranges: Vec<ByteRange>,
    pub leftover_ranges: Vec<ByteRange>,
    pub ranges_by_confidence: BTreeMap<TraceConfidence, Vec<ByteRange>>,
}

impl ParserTrace {
    /// Sum of `consumed_ranges`; invariant: `consumed_bytes +
    /// leftover_bytes == total_bytes`.
    pub fn consumed_bytes(&self) -> u64 {
        self.consumed_ranges.iter().map(|r| r.len()).sum()
    }

    /// Sum of `leftover_ranges`.
    pub fn leftover_bytes(&self) -> u64 {
        self.leftover_ranges.iter().map(|r| r.len()).sum()
    }

    /// `consumed_bytes / total_bytes` as a ratio in `[0.0, 1.0]`.
    /// Returns `0.0` (not NaN) when `total_bytes == 0` so UI / CI code
    /// does not need to special-case empty streams.
    pub fn coverage_ratio(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.consumed_bytes() as f32 / self.total_bytes as f32
        }
    }
}

/// Side-channel accumulator that parsers write `consume` calls into.
///
/// Parser migration pattern (Phase 12b-1+):
/// ```text
/// pub fn parse_foo(data: &[u8]) -> Option<Foo> {
///     let mut trace = ParserTraceBuilder::new("parse_foo");
///     parse_foo_with_trace(data, &mut trace)
/// }
///
/// pub fn parse_foo_with_trace(
///     data: &[u8],
///     trace: &mut ParserTraceBuilder,
/// ) -> Option<Foo> {
///     trace.consume(ByteRange::new(0, 4), TraceConfidence::Decoded);
///     // ...
/// }
/// ```
///
/// `build` is where sorting / merging / leftover calculation happens;
/// individual `consume` calls are cheap push-backs so hot parser loops
/// can call them freely without quadratic cost.
#[derive(Debug, Clone)]
pub struct ParserTraceBuilder {
    parser_name: String,
    ranges: Vec<(ByteRange, TraceConfidence)>,
}

impl ParserTraceBuilder {
    pub fn new(parser_name: impl Into<String>) -> Self {
        Self {
            parser_name: parser_name.into(),
            ranges: Vec::new(),
        }
    }

    /// Record that the parser consumed `range` with confidence `conf`.
    /// Empty ranges are silently dropped so parsers can emit them from
    /// conditional branches without guarding every call-site.
    pub fn consume(&mut self, range: ByteRange, conf: TraceConfidence) {
        if range.is_empty() {
            return;
        }
        self.ranges.push((range, conf));
    }

    /// Consume the builder and produce a [`ParserTrace`] for the given
    /// stream path and total byte count.
    ///
    /// Invariants of the output:
    /// - `consumed_ranges` is sorted by `start`, same-confidence
    ///   adjacent / overlapping ranges are merged, but different
    ///   confidences keep separate ranges.
    /// - `leftover_ranges` is the complement of `consumed_ranges`
    ///   inside `[0, total_bytes)`, clipped to never extend past
    ///   `total_bytes`.
    /// - `consumed_bytes + leftover_bytes == total_bytes` (always, even
    ///   when `total_bytes` is zero).
    pub fn build(self, stream_path: impl Into<String>, total_bytes: u64) -> ParserTrace {
        let mut entries = self.ranges;
        // Clip ranges to `[0, total_bytes)` so a buggy parser cannot
        // inflate coverage past the stream size.
        for (range, _) in entries.iter_mut() {
            range.end = range.end.min(total_bytes);
            if range.start > total_bytes {
                range.start = total_bytes;
            }
        }
        entries.retain(|(r, _)| !r.is_empty());
        entries.sort_by_key(|(r, _)| (r.start, r.end));

        // Merge adjacent / overlapping ranges with the same confidence.
        let mut merged: Vec<(ByteRange, TraceConfidence)> = Vec::new();
        for (range, conf) in entries {
            if let Some((last_range, last_conf)) = merged.last_mut() {
                if *last_conf == conf && last_range.end >= range.start {
                    last_range.end = last_range.end.max(range.end);
                    continue;
                }
            }
            merged.push((range, conf));
        }

        let consumed_ranges: Vec<ByteRange> = merged.iter().map(|(r, _)| *r).collect();
        let mut ranges_by_confidence: BTreeMap<TraceConfidence, Vec<ByteRange>> = BTreeMap::new();
        for (range, conf) in &merged {
            ranges_by_confidence.entry(*conf).or_default().push(*range);
        }

        let leftover_ranges = compute_leftover(&consumed_ranges, total_bytes);

        ParserTrace {
            parser_name: self.parser_name,
            stream_path: stream_path.into(),
            total_bytes,
            consumed_ranges,
            leftover_ranges,
            ranges_by_confidence,
        }
    }
}

/// Compute the complement of `consumed` inside `[0, total_bytes)`.
/// `consumed` **must** already be sorted by `start` and non-overlapping
/// within the same confidence bucket; cross-confidence overlap is
/// tolerated (the leftover calculation only cares about *any* coverage).
fn compute_leftover(consumed: &[ByteRange], total_bytes: u64) -> Vec<ByteRange> {
    let mut flattened: Vec<ByteRange> = consumed.to_vec();
    flattened.sort_by_key(|r| (r.start, r.end));

    // Flatten overlap across confidence buckets by simple sweep merge.
    let mut flat_merged: Vec<ByteRange> = Vec::new();
    for r in flattened {
        if let Some(last) = flat_merged.last_mut() {
            if last.end >= r.start {
                last.end = last.end.max(r.end);
                continue;
            }
        }
        flat_merged.push(r);
    }

    let mut leftover = Vec::new();
    let mut cursor: u64 = 0;
    for r in flat_merged {
        if r.start > cursor {
            leftover.push(ByteRange::new(cursor, r.start));
        }
        cursor = cursor.max(r.end);
    }
    if cursor < total_bytes {
        leftover.push(ByteRange::new(cursor, total_bytes));
    }
    leftover
}

#[cfg(test)]
mod byte_range_tests {
    use super::*;

    #[test]
    fn len_of_non_empty_range_is_end_minus_start() {
        assert_eq!(ByteRange::new(0, 4).len(), 4);
        assert_eq!(ByteRange::new(8, 12).len(), 4);
        assert_eq!(ByteRange::new(0, 0).len(), 0);
    }

    #[test]
    fn inverted_range_is_empty_and_has_zero_len() {
        let r = ByteRange::new(10, 5);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn adjacent_ranges_do_not_overlap() {
        let a = ByteRange::new(0, 4);
        let b = ByteRange::new(4, 8);
        assert!(!a.overlaps(&b));
        assert!(!b.overlaps(&a));
    }

    #[test]
    fn overlapping_ranges_report_overlap_symmetrically() {
        let a = ByteRange::new(0, 6);
        let b = ByteRange::new(4, 10);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn nested_range_overlaps_enclosing() {
        let outer = ByteRange::new(0, 20);
        let inner = ByteRange::new(5, 12);
        assert!(outer.overlaps(&inner));
        assert!(inner.overlaps(&outer));
    }

    #[test]
    fn empty_ranges_never_overlap() {
        let empty = ByteRange::new(4, 4);
        let filled = ByteRange::new(0, 10);
        assert!(!empty.overlaps(&filled));
        assert!(!filled.overlaps(&empty));
    }

    #[test]
    fn contains_offset_is_inclusive_start_exclusive_end() {
        let r = ByteRange::new(4, 8);
        assert!(r.contains_offset(4));
        assert!(r.contains_offset(7));
        assert!(!r.contains_offset(8));
        assert!(!r.contains_offset(3));
    }

    #[test]
    fn ord_is_lexicographic_over_start_then_end() {
        // Ord is derived (start, end). Test that sorting behaves as
        // documented so ParserTrace merge logic can rely on it later.
        let mut ranges = vec![
            ByteRange::new(8, 12),
            ByteRange::new(0, 4),
            ByteRange::new(0, 6), // same start as the one above, larger end
            ByteRange::new(4, 8),
        ];
        ranges.sort();
        assert_eq!(
            ranges,
            vec![
                ByteRange::new(0, 4),
                ByteRange::new(0, 6),
                ByteRange::new(4, 8),
                ByteRange::new(8, 12),
            ]
        );
    }
}

#[cfg(test)]
mod parser_trace_tests {
    use super::*;

    fn build_simple(
        total: u64,
        items: &[(u64, u64, TraceConfidence)],
    ) -> ParserTrace {
        let mut b = ParserTraceBuilder::new("test_parser");
        for &(s, e, c) in items {
            b.consume(ByteRange::new(s, e), c);
        }
        b.build("/TestStream", total)
    }

    #[test]
    fn builder_drops_empty_ranges() {
        let trace = build_simple(
            10,
            &[
                (0, 0, TraceConfidence::Decoded),   // empty
                (5, 5, TraceConfidence::Decoded),   // empty
                (2, 6, TraceConfidence::Decoded),   // real
            ],
        );
        assert_eq!(trace.consumed_ranges, vec![ByteRange::new(2, 6)]);
    }

    #[test]
    fn builder_merges_adjacent_same_confidence_ranges() {
        let trace = build_simple(
            10,
            &[
                (0, 4, TraceConfidence::Decoded),
                (4, 8, TraceConfidence::Decoded),
            ],
        );
        assert_eq!(trace.consumed_ranges, vec![ByteRange::new(0, 8)]);
    }

    #[test]
    fn builder_keeps_different_confidence_ranges_separate_even_when_adjacent() {
        let trace = build_simple(
            10,
            &[
                (0, 4, TraceConfidence::Decoded),
                (4, 8, TraceConfidence::Probed),
            ],
        );
        assert_eq!(
            trace.consumed_ranges,
            vec![ByteRange::new(0, 4), ByteRange::new(4, 8)]
        );
        let by_conf = &trace.ranges_by_confidence;
        assert_eq!(
            by_conf.get(&TraceConfidence::Decoded),
            Some(&vec![ByteRange::new(0, 4)])
        );
        assert_eq!(
            by_conf.get(&TraceConfidence::Probed),
            Some(&vec![ByteRange::new(4, 8)])
        );
    }

    #[test]
    fn builder_merges_overlapping_same_confidence_ranges() {
        let trace = build_simple(
            20,
            &[
                (0, 6, TraceConfidence::Decoded),
                (4, 10, TraceConfidence::Decoded),
            ],
        );
        assert_eq!(trace.consumed_ranges, vec![ByteRange::new(0, 10)]);
    }

    #[test]
    fn builder_computes_leftover_as_complement_over_total_bytes() {
        let trace = build_simple(
            10,
            &[
                (0, 4, TraceConfidence::Decoded),
                (6, 8, TraceConfidence::Decoded),
            ],
        );
        assert_eq!(
            trace.leftover_ranges,
            vec![ByteRange::new(4, 6), ByteRange::new(8, 10)]
        );
    }

    #[test]
    fn builder_clips_ranges_extending_past_total_bytes() {
        let trace = build_simple(
            8,
            &[(0, 100, TraceConfidence::Decoded)],
        );
        assert_eq!(trace.consumed_ranges, vec![ByteRange::new(0, 8)]);
        assert!(trace.leftover_ranges.is_empty());
    }

    #[test]
    fn builder_handles_mixed_confidence_with_overlap_for_leftover_calculation() {
        // Decoded [0..6] + Probed [4..10]; leftover should be complement
        // of the *union* [0..10], i.e. [10..16].
        let trace = build_simple(
            16,
            &[
                (0, 6, TraceConfidence::Decoded),
                (4, 10, TraceConfidence::Probed),
            ],
        );
        assert_eq!(trace.leftover_ranges, vec![ByteRange::new(10, 16)]);
    }

    #[test]
    fn consumed_plus_leftover_equals_total_bytes_invariant() {
        // Fuzz-style exercise of the conservation law across shapes.
        let cases: &[(u64, &[(u64, u64, TraceConfidence)])] = &[
            (0, &[]),
            (16, &[]),
            (16, &[(0, 16, TraceConfidence::Decoded)]),
            (
                16,
                &[
                    (0, 4, TraceConfidence::Decoded),
                    (10, 14, TraceConfidence::Probed),
                ],
            ),
            (
                10,
                &[(5, 20, TraceConfidence::Decoded)], // over-shoots
            ),
        ];
        for (total, items) in cases {
            let trace = build_simple(*total, items);
            assert_eq!(
                trace.consumed_bytes() + trace.leftover_bytes(),
                *total,
                "invariant broken for total={total} items={items:?}; \
                 trace={trace:?}",
            );
        }
    }

    #[test]
    fn coverage_ratio_is_zero_for_empty_stream_no_nan() {
        let trace = build_simple(0, &[]);
        assert_eq!(trace.coverage_ratio(), 0.0);
        assert!(!trace.coverage_ratio().is_nan());
    }

    #[test]
    fn coverage_ratio_reflects_consumed_over_total() {
        let trace = build_simple(
            16,
            &[
                (0, 8, TraceConfidence::Decoded),
                (12, 16, TraceConfidence::Probed),
            ],
        );
        assert!(
            (trace.coverage_ratio() - 0.75).abs() < f32::EPSILON,
            "expected 0.75, got {}",
            trace.coverage_ratio()
        );
    }

    #[test]
    fn build_carries_parser_name_and_stream_path_verbatim() {
        let mut b = ParserTraceBuilder::new("parse_unit_test");
        b.consume(ByteRange::new(0, 2), TraceConfidence::Raw);
        let trace = b.build("/Foo/Bar", 8);
        assert_eq!(trace.parser_name, "parse_unit_test");
        assert_eq!(trace.stream_path, "/Foo/Bar");
        assert_eq!(trace.total_bytes, 8);
    }
}
