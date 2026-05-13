//! Controlled `.pid` before/after diff evidence reports.
//!
//! Per `docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`,
//! this module owns the **report-building** layer of the
//! controlled-diff investigation pathway. The CLI binary
//! (`pid_inspect --controlled-diff-dir`) keeps filesystem scanning
//! and stdout rendering, but the evidence construction itself moves
//! here so it can be unit-tested without spawning a process and so
//! future decoder spikes can consume the same DTO directly.
//!
//! ## Phase 14 boundaries
//!
//! These reports are explicitly **investigation evidence**, not
//! geometry promotion:
//!
//! * `promoted_geometry` is hard-coded to `false` on every report.
//!   Promotion to a typed `SheetRecordKind::PrimitiveLine` requires
//!   bounded byte range + repeated shape + round-tripped coordinates
//!   (see `docs/plans/2026-05-09-phase-14-sppid-full-geometry-plan-cn.md`),
//!   none of which is the responsibility of this module.
//! * The `expected` metadata payload from the controlled-diff
//!   author is preserved verbatim so a future decoder can compare
//!   parsed coordinates against the intended geometry.
//!
//! ## Inputs
//!
//! * A "before" [`PidPackage`] (drawing as it existed before the
//!   user performed a known operation in `SmartPlant` P&ID).
//! * An "after" [`PidPackage`] (same drawing immediately after that
//!   operation, saved by `SmartPlant`).
//! * A validated [`ControlledDiffMetadata`] describing the case ID,
//!   the operation that was performed, the expected geometry
//!   payload, and optional human notes.
//!
//! ## Outputs
//!
//! * [`ControlledDiffCaseReport`] for one pair.
//! * [`ControlledDiffEvidenceReport`] aggregating several cases under
//!   a single investigation root (matches the
//!   `before/`, `after/`, `metadata/` directory layout the CLI
//!   accepts on `--controlled-diff-dir`).
//!
//! ## Determinism
//!
//! Builders are pure functions of their inputs. They do not touch
//! the filesystem, network, or system clock. Stream diff order,
//! modified-Sheet-stream counts, and first-mismatch offsets are all
//! taken from [`crate::diff_packages`] which itself iterates over a
//! [`BTreeMap`](std::collections::BTreeMap), so two runs over the
//! same input produce byte-identical reports.
//!
//! ## Quick start
//!
//! Build a single-case evidence report from two in-memory
//! [`PidPackage`] values plus the author's metadata payload. The
//! example below is the same shape `pid_inspect --controlled-diff-dir`
//! drives at runtime, minus the CLI's filesystem scanner.
//!
//! ```
//! use std::collections::BTreeMap;
//! use serde_json::json;
//! use pid_parse::inspect::controlled_diff::{
//!     build_case_report, ControlledDiffMetadata,
//! };
//! use pid_parse::model::PidDocument;
//! use pid_parse::package::{PidPackage, RawStream};
//!
//! // Two minimal packages that differ in exactly one `/Sheet6` byte.
//! fn raw_stream(path: &str, data: &[u8]) -> RawStream {
//!     RawStream {
//!         path: path.to_string(),
//!         data: data.to_vec(),
//!         modified: false,
//!     }
//! }
//! let mut before_streams = BTreeMap::new();
//! before_streams.insert("/Sheet6".to_string(), raw_stream("/Sheet6", b"before"));
//! let mut after_streams = BTreeMap::new();
//! after_streams.insert("/Sheet6".to_string(), raw_stream("/Sheet6", b"AFTER!"));
//! let before = PidPackage::new(None, before_streams, PidDocument::default());
//! let after = PidPackage::new(None, after_streams, PidDocument::default());
//!
//! let report = build_case_report(
//!     &before,
//!     &after,
//!     ControlledDiffMetadata {
//!         case: "one-line".into(),
//!         operation: "place_line".into(),
//!         expected: json!({"start": [0, 0], "end": [1, 0]}),
//!         notes: None,
//!     },
//! );
//!
//! assert_eq!(report.modified_sheet_streams, 1);
//! assert_eq!(
//!     report.first_modified.as_ref().map(|s| s.path.as_str()),
//!     Some("/Sheet6"),
//! );
//! ```

use serde::{Deserialize, Serialize};

use crate::package::{diff_packages, PidPackage, StreamDiff};

/// CFB path prefix that separates Sheet-bearing streams from other
/// per-drawing storage. Matches the convention `SmartPlant` itself
/// follows: every drawing's primitive geometry lives under
/// `/Sheet0`, `/Sheet1`, …, `/SheetN`.
const SHEET_PATH_PREFIX: &str = "/Sheet";

/// Per-case metadata authored by the user who produced the
/// controlled diff in `SmartPlant` P&ID. The CLI loads this from a
/// `metadata/<case>.json` file and validates the `case` field
/// matches the filename; this module assumes that validation has
/// already happened.
///
/// The `expected` payload is a free-form JSON value so the
/// controlled-diff author can describe the geometry they placed
/// (e.g. `{"start":[0,0],"end":[1,0]}` for a single line). The
/// report carries it forward verbatim so future decoders can
/// cross-check parsed values against intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlledDiffMetadata {
    /// Case identifier; must match the stem of the `before` /
    /// `after` `.pid` filenames so the CLI can pair them up.
    pub case: String,
    /// Human-readable operation name, e.g. `place_line` /
    /// `place_circle` / `move_symbol`. Used to scope which decoder
    /// spike the resulting evidence is supposed to unblock.
    pub operation: String,
    /// Author-declared expected geometry payload. Verbatim JSON;
    /// no parsing on this side.
    pub expected: serde_json::Value,
    /// Optional free-form notes (typically logged into the report
    /// JSON for downstream humans). Set to `None` when the author
    /// did not supply notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// One stream's diff slice that the report should surface. The CLI
/// previously embedded the first modified stream inline; this
/// module keeps the same shape so the JSON contract stays stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlledDiffStreamReport {
    /// Normalized CFB path of the stream, e.g. `/Sheet6`.
    pub path: String,
    /// Byte length of the stream in the `before` package.
    pub len_before: usize,
    /// Byte length of the stream in the `after` package.
    pub len_after: usize,
    /// First byte index at which `before` and `after` disagree.
    /// Equal to `min(len_before, len_after)` when one stream is a
    /// prefix of the other.
    pub first_mismatch_offset: usize,
    /// 16-byte hex context starting at `first_mismatch_offset` of
    /// the `before` stream. Empty when the stream did not exist
    /// in the `before` package or the mismatch is past EOF.
    pub before_context: String,
    /// 16-byte hex context starting at `first_mismatch_offset` of
    /// the `after` stream. Empty under the same conditions as
    /// [`Self::before_context`].
    pub after_context: String,
}

impl ControlledDiffStreamReport {
    fn from_stream_diff(diff: &StreamDiff) -> Self {
        Self {
            path: diff.path.clone(),
            len_before: diff.len_a,
            len_after: diff.len_b,
            first_mismatch_offset: diff.first_mismatch_offset,
            before_context: diff.context_before.clone(),
            after_context: diff.context_after.clone(),
        }
    }
}

/// One controlled-diff case: a single before / after `.pid` pair
/// plus the metadata that explains what the user did between them.
///
/// `stream_diffs` is the total count of stream-level differences
/// (paths only in `before`, paths only in `after`, modified
/// paths). `modified_sheet_streams` is the subset of `modified`
/// whose CFB path starts with `/Sheet` — the proxy used
/// everywhere in the project for "this diff touches drawing
/// geometry".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlledDiffCaseReport {
    /// Case identifier carried forward from the metadata.
    pub case: String,
    /// Operation name carried forward from the metadata.
    pub operation: String,
    /// Expected geometry payload carried forward from the metadata.
    pub expected: serde_json::Value,
    /// Total stream-level diff count (only-in-before + only-in-after
    /// + modified).
    pub stream_diffs: usize,
    /// Number of modified streams whose path starts with `/Sheet`.
    pub modified_sheet_streams: usize,
    /// Number of stream paths present only in the `before` package.
    pub only_in_before: usize,
    /// Number of stream paths present only in the `after` package.
    pub only_in_after: usize,
    /// The first modified stream slice if any exists. The first
    /// element of [`crate::package::PackageDiff::modified`] is used
    /// because `diff_packages` iterates the modified set in
    /// `BTreeMap` order — deterministic across runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_modified: Option<ControlledDiffStreamReport>,
    /// Optional notes carried forward from the metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Top-level evidence report for a controlled-diff investigation
/// directory. Aggregates one [`ControlledDiffCaseReport`] per
/// `(before/<case>.pid, after/<case>.pid, metadata/<case>.json)`
/// triple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlledDiffEvidenceReport {
    /// Display string for the investigation directory (relative or
    /// absolute, depending on the CLI invocation). The builder
    /// does not normalize this value.
    pub root: String,
    /// One report per controlled-diff case under `root`. Order is
    /// caller-controlled — the CLI sorts by case name so a JSON
    /// re-render diffs cleanly in version control.
    pub cases: Vec<ControlledDiffCaseReport>,
    /// Phase 14 anti-promotion guarantee: always `false` for any
    /// report produced by this module. A future decoder will
    /// graduate geometry into typed `NormalizedPidGeometry` only
    /// after the controlled-diff evidence has been verified by a
    /// separate code path, never by this builder alone.
    pub promoted_geometry: bool,
}

/// Build the evidence DTO for a single controlled-diff case.
///
/// This is the canonical entry point. It is a pure function of
/// `(before, after, metadata)` and never:
///
/// * touches the filesystem
/// * mutates its inputs
/// * promotes any geometry — `promoted_geometry` is constructed by
///   [`build_evidence_report`], not here, and is always `false`.
///
/// The function does **not** validate that
/// `metadata.case` matches the source filenames; that check
/// happens earlier in the CLI's metadata loader because it is a
/// filesystem-level concern.
pub fn build_case_report(
    before: &PidPackage,
    after: &PidPackage,
    metadata: ControlledDiffMetadata,
) -> ControlledDiffCaseReport {
    let diff = diff_packages(before, after);
    let stream_diffs = diff.only_in_a.len() + diff.only_in_b.len() + diff.modified.len();
    let modified_sheet_streams = diff
        .modified
        .iter()
        .filter(|stream| stream.path.starts_with(SHEET_PATH_PREFIX))
        .count();
    let first_modified = diff
        .modified
        .first()
        .map(ControlledDiffStreamReport::from_stream_diff);

    ControlledDiffCaseReport {
        case: metadata.case,
        operation: metadata.operation,
        expected: metadata.expected,
        stream_diffs,
        modified_sheet_streams,
        only_in_before: diff.only_in_a.len(),
        only_in_after: diff.only_in_b.len(),
        first_modified,
        notes: metadata.notes,
    }
}

/// Build the multi-case evidence report rooted at `root`.
///
/// Each tuple in `cases` carries a `(before, after, metadata)`
/// triple that the caller has already paired up. The builder does
/// no filesystem work — wrapping with disk loading is the CLI's
/// responsibility per the
/// `docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`
/// boundary.
///
/// `promoted_geometry` is forced to `false`. Any caller that
/// believes geometry was promoted must construct a different
/// report type; this module deliberately makes the false outcome
/// the only one reachable.
pub fn build_evidence_report<'a, I>(
    root: impl Into<String>,
    cases: I,
) -> ControlledDiffEvidenceReport
where
    I: IntoIterator<Item = (&'a PidPackage, &'a PidPackage, ControlledDiffMetadata)>,
{
    let cases: Vec<ControlledDiffCaseReport> = cases
        .into_iter()
        .map(|(before, after, metadata)| build_case_report(before, after, metadata))
        .collect();
    ControlledDiffEvidenceReport {
        root: root.into(),
        cases,
        promoted_geometry: false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::model::PidDocument;
    use crate::package::{PidPackage, RawStream};

    /// Build an in-memory [`PidPackage`] containing exactly the
    /// streams listed in `streams`. Streams are turned into
    /// [`RawStream`] entries keyed on the path they ship with so
    /// the package's `streams` `BTreeMap` follows the same
    /// convention as a parsed package.
    fn synthetic_package(streams: &[(&str, &[u8])]) -> PidPackage {
        let streams_map: BTreeMap<String, RawStream> = streams
            .iter()
            .map(|(path, data)| {
                (
                    (*path).to_string(),
                    RawStream {
                        path: (*path).to_string(),
                        data: data.to_vec(),
                        modified: false,
                    },
                )
            })
            .collect();
        PidPackage::new(None, streams_map, PidDocument::default())
    }

    fn sample_metadata() -> ControlledDiffMetadata {
        ControlledDiffMetadata {
            case: "one-line".into(),
            operation: "place_line".into(),
            expected: json!({"start": [0, 0], "end": [1, 0]}),
            notes: Some("synthetic for unit test".into()),
        }
    }

    #[test]
    fn case_report_pins_metadata_and_diff_shape_for_single_sheet_change() {
        // The smallest meaningful controlled diff: identical packages
        // except for a single byte inside `/Sheet6`. The report must
        // reflect every contract point listed in
        // `docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`
        // section "First Red Test".
        let before = synthetic_package(&[("/Sheet6", b"before-sheet-bytes")]);
        let after = synthetic_package(&[("/Sheet6", b"AFTER-sheet-bytes")]);

        let metadata = sample_metadata();
        let report = build_case_report(&before, &after, metadata);

        // (1) metadata fields propagate verbatim.
        assert_eq!(report.case, "one-line");
        assert_eq!(report.operation, "place_line");
        assert_eq!(report.expected, json!({"start": [0, 0], "end": [1, 0]}));
        assert_eq!(report.notes.as_deref(), Some("synthetic for unit test"));

        // (2) deterministic counts: exactly one modified stream,
        //     0 only-in-before, 0 only-in-after, 1 sheet stream
        //     modified.
        assert_eq!(report.stream_diffs, 1);
        assert_eq!(report.only_in_before, 0);
        assert_eq!(report.only_in_after, 0);
        assert_eq!(report.modified_sheet_streams, 1);

        // (3) first_modified surfaces /Sheet6 and the first
        //     mismatch context. byte 0 of `before` is `b`,
        //     byte 0 of `after` is `A` — `first_mismatch_offset`
        //     must be 0.
        let first = report.first_modified.expect("Sheet6 must surface");
        assert_eq!(first.path, "/Sheet6");
        assert_eq!(first.len_before, b"before-sheet-bytes".len());
        assert_eq!(first.len_after, b"AFTER-sheet-bytes".len());
        assert_eq!(first.first_mismatch_offset, 0);
        // hex context must be non-empty so the human reader can
        // see the bytes; we do not assert exact content because the
        // upstream `diff_packages` formatter owns that detail.
        assert!(!first.before_context.is_empty());
        assert!(!first.after_context.is_empty());
    }

    #[test]
    fn evidence_report_aggregates_cases_and_forces_promoted_false() {
        // Two cases under one root. The aggregate report's
        // promoted_geometry field is the most important phase 14
        // contract: it MUST stay false even if a caller tried to
        // claim otherwise — the builder has no input that could
        // flip it.
        let case_a_before = synthetic_package(&[("/Sheet6", b"line-before")]);
        let case_a_after = synthetic_package(&[("/Sheet6", b"line-after-X")]);
        let case_b_before = synthetic_package(&[("/Sheet6", b"circle-before")]);
        let case_b_after = synthetic_package(&[("/Sheet6", b"circle-after-Y")]);

        let report = build_evidence_report(
            "test-root",
            [
                (
                    &case_a_before,
                    &case_a_after,
                    ControlledDiffMetadata {
                        case: "one-line".into(),
                        operation: "place_line".into(),
                        expected: json!({"start": [0, 0], "end": [1, 0]}),
                        notes: None,
                    },
                ),
                (
                    &case_b_before,
                    &case_b_after,
                    ControlledDiffMetadata {
                        case: "one-circle".into(),
                        operation: "place_circle".into(),
                        expected: json!({"center": [0, 0], "radius": 10}),
                        notes: None,
                    },
                ),
            ],
        );

        assert_eq!(report.root, "test-root");
        assert_eq!(report.cases.len(), 2);
        assert_eq!(report.cases[0].case, "one-line");
        assert_eq!(report.cases[1].case, "one-circle");
        assert!(
            !report.promoted_geometry,
            "controlled diff evidence MUST NOT promote geometry; \
             phase 14 anti-goal",
        );
    }

    #[test]
    fn case_report_counts_only_in_before_and_only_in_after_separately() {
        // Asymmetric streams: `before` has /Sheet6, `after` has
        // /Sheet6 + /Sheet9. The /Sheet6 bytes are identical, so
        // the only diff is the new /Sheet9 in `after`.
        let before = synthetic_package(&[("/Sheet6", b"identical-bytes")]);
        let after = synthetic_package(&[
            ("/Sheet6", b"identical-bytes"),
            ("/Sheet9", b"newly-added-sheet-stream"),
        ]);
        let metadata = ControlledDiffMetadata {
            case: "add-sheet".into(),
            operation: "add_drawing_sheet".into(),
            expected: json!({"new_sheet": 9}),
            notes: None,
        };
        let report = build_case_report(&before, &after, metadata);
        assert_eq!(report.stream_diffs, 1);
        assert_eq!(report.only_in_before, 0);
        assert_eq!(report.only_in_after, 1);
        // The new Sheet stream did not appear in `modified`, only in
        // `only_in_after`, so the sheet-modification count stays 0.
        assert_eq!(report.modified_sheet_streams, 0);
        assert!(
            report.first_modified.is_none(),
            "no modified stream slot was filled when only `only_in_after` diverged",
        );
    }

    #[test]
    fn case_report_does_not_count_non_sheet_modifications_as_sheet_changes() {
        // A SmartPlant save may also touch SummaryInformation or
        // DocVersion3 streams. Those are real diffs and contribute
        // to `stream_diffs`, but must NOT inflate
        // `modified_sheet_streams` — that counter is the proxy for
        // "drawing geometry actually changed".
        let before = synthetic_package(&[
            ("/Sheet6", b"identical-bytes"),
            ("/\u{5}SummaryInformation", b"summary-before"),
        ]);
        let after = synthetic_package(&[
            ("/Sheet6", b"identical-bytes"),
            ("/\u{5}SummaryInformation", b"summary-after-X"),
        ]);
        let metadata = ControlledDiffMetadata {
            case: "metadata-only".into(),
            operation: "save_without_edits".into(),
            expected: json!({}),
            notes: None,
        };
        let report = build_case_report(&before, &after, metadata);
        assert_eq!(report.stream_diffs, 1);
        assert_eq!(report.modified_sheet_streams, 0);
        // The first modified stream is the summary, not a Sheet.
        let first = report.first_modified.expect("summary stream modified");
        assert!(
            !first.path.starts_with(SHEET_PATH_PREFIX),
            "first modified must point at the non-Sheet stream, got `{}`",
            first.path,
        );
    }

    #[test]
    fn evidence_report_with_no_cases_keeps_promoted_geometry_false() {
        // Edge case: empty directory. The report still serializes
        // cleanly and the phase 14 anti-promotion guarantee holds.
        let report = build_evidence_report("/tmp/empty", std::iter::empty());
        assert_eq!(report.root, "/tmp/empty");
        assert!(report.cases.is_empty());
        assert!(!report.promoted_geometry);
    }
}
