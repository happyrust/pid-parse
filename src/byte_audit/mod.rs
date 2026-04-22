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
