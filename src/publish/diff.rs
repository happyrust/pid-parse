//! Semantic diff between a `pid-parse`-generated `_Data.xml` and a
//! SmartPlant-produced reference `_Data.xml`.
//!
//! Stage-1 A12 establishes the comparison baseline: every PID tag
//! variety surfaces in a side-by-side count table together with a
//! `(generated - reference)` delta. The intent is *not* a textual
//! diff — SmartPlant's exporter and ours emit different per-element
//! formatting, attribute ordering, and inline whitespace, all of
//! which would noise out a byte-level comparison.
//!
//! Instead the report answers two strategic questions:
//!
//! 1. **Coverage** — which PID tag varieties are missing from our
//!    output, and which extras are we emitting that SmartPlant never
//!    produces?
//! 2. **Volume** — for shared tag varieties, do counts match? A
//!    discrepancy hints at loader-side under- or over-counting.
//!
//! Both questions are answered without parsing XML structurally;
//! the implementation is a deliberate, focused scan over the byte
//! stream looking for `<PID...>` opening tags. This keeps the
//! dependency surface zero-cost (no quick-xml needed) and makes the
//! comparison robust to the formatting drift between exporters.

use std::collections::BTreeMap;
use std::fmt;

/// Per-tag comparison row. `generated` and `reference` are the
/// raw open-tag counts; `status` classifies the relationship so
/// callers can filter for action items at a glance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCountDiff {
    /// The tag name, sans angle brackets, e.g. `"PIDProcessVessel"`.
    pub tag: String,
    /// Number of `<tag>` opens observed in the generated document.
    pub generated: usize,
    /// Number of `<tag>` opens observed in the reference document.
    pub reference: usize,
    /// Classification — see [`TagDiffStatus`].
    pub status: TagDiffStatus,
}

impl TagCountDiff {
    /// Signed delta `generated - reference`. Positive when we emit
    /// more than reference, negative when we under-emit.
    pub fn delta(&self) -> i64 {
        self.generated as i64 - self.reference as i64
    }
}

/// Classification of a single tag's count relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagDiffStatus {
    /// Both sides emit the same non-zero count for this tag.
    Match,
    /// Both sides emit this tag, but with different counts.
    CountDelta,
    /// Reference emits this tag; generated does not (`generated == 0`).
    MissingFromGenerated,
    /// Generated emits this tag; reference does not (`reference == 0`).
    ExtraInGenerated,
}

impl fmt::Display for TagDiffStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Match => f.write_str("MATCH"),
            Self::CountDelta => f.write_str("DELTA"),
            Self::MissingFromGenerated => f.write_str("MISSING"),
            Self::ExtraInGenerated => f.write_str("EXTRA"),
        }
    }
}

/// Aggregate report — one entry per tag variety observed in either
/// side, plus rolled-up totals so callers can decide on an exit
/// code or threshold without re-tabulating.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticDiffReport {
    /// Sum of every `<PID...>` open across the generated document.
    pub generated_total: usize,
    /// Sum of every `<PID...>` open across the reference document.
    pub reference_total: usize,
    /// Per-tag rows, sorted by status priority then tag name so
    /// readers see actionable rows (`MISSING` / `EXTRA` / `DELTA`)
    /// before the green `MATCH` rows.
    pub tag_diffs: Vec<TagCountDiff>,
    /// Number of tag varieties whose counts match exactly. Use as
    /// a coverage proxy when reading the report at a glance.
    pub matching: usize,
    /// Number of tag varieties present in reference but missing in
    /// generated. The most actionable signal in the report.
    pub missing_from_generated: usize,
    /// Number of tag varieties present in generated but missing in
    /// reference. Usually means we are over-emitting an interface
    /// that SmartPlant skips on the source drawing.
    pub extra_in_generated: usize,
    /// Number of shared tag varieties whose counts differ.
    pub count_deltas: usize,
}

impl SemanticDiffReport {
    /// True when no actionable findings exist — every shared tag
    /// matches and neither side has unique tags. Useful as the CLI
    /// exit code gate.
    pub fn is_clean(&self) -> bool {
        self.missing_from_generated == 0
            && self.extra_in_generated == 0
            && self.count_deltas == 0
    }

    /// Convenience: just the rows that need investigation, in the
    /// canonical priority order (missing > extra > delta). Skips
    /// `Match` rows.
    pub fn problems(&self) -> impl Iterator<Item = &TagCountDiff> {
        self.tag_diffs
            .iter()
            .filter(|r| !matches!(r.status, TagDiffStatus::Match))
    }
}

impl fmt::Display for SemanticDiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Publish Data XML semantic diff ===")?;
        writeln!(
            f,
            "Generated PID tags: {}; Reference PID tags: {}; varieties matched: {}; missing: {}; extra: {}; count deltas: {}",
            self.generated_total,
            self.reference_total,
            self.matching,
            self.missing_from_generated,
            self.extra_in_generated,
            self.count_deltas,
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "{:<8} {:>9} {:>9} {:>7}  Tag",
            "Status", "Generated", "Reference", "Delta",
        )?;
        writeln!(f, "{}", "-".repeat(56))?;
        for row in &self.tag_diffs {
            writeln!(
                f,
                "{:<8} {:>9} {:>9} {:>+7}  {}",
                row.status.to_string(),
                row.generated,
                row.reference,
                row.delta(),
                row.tag,
            )?;
        }
        Ok(())
    }
}

/// Scan `xml` and return a `tag_name -> count` map for every
/// `<PIDxxx>` open we encounter. Self-closing tags (`<PIDxxx/>`)
/// are NOT counted — SmartPlant always opens these as block
/// elements with an explicit `</PIDxxx>` closer, so a self-closing
/// match would always be a false positive.
///
/// The scanner ignores attributes; e.g. `<PIDDrawing Foo="bar">`
/// tallies as `"PIDDrawing"`. Tag-name characters are restricted
/// to `[A-Za-z0-9_]` to keep the scan robust to garbage payloads.
pub fn parse_pid_tag_counts(xml: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    while i + 4 < bytes.len() {
        // Look for the literal "<PID". We slide one byte at a time
        // because the prefix could overlap with itself (e.g.
        // `<PID<PIDFoo>` would still want to start at the second
        // `<`). The cost is one comparison per byte — negligible
        // for documents in the kilobyte-to-megabyte range we care
        // about.
        if bytes[i] != b'<' || &bytes[i + 1..i + 4] != b"PID" {
            i += 1;
            continue;
        }
        let start = i + 1; // first byte of the tag name
        let mut end = start;
        while end < bytes.len() && is_tag_name_byte(bytes[end]) {
            end += 1;
        }
        // Reject if no real tag-name body, or if we hit EOF.
        if end == start || end >= bytes.len() {
            i = end.max(i + 1);
            continue;
        }
        // The byte right after the tag name decides whether this
        // was an opener or a self-closer / unrelated text:
        // - `>`            → opener, count it
        // - whitespace + ` `... `>` → opener with attributes, count
        // - `/`            → self-closing, skip
        // Everything else is treated as not-a-tag.
        let after = bytes[end];
        let opens = match after {
            b'>' => true,
            b'/' => false,
            c if c.is_ascii_whitespace() => {
                // Walk forward to the matching `>` and decide based
                // on whether the byte right before it is `/`
                // (self-closer) or anything else (opener).
                let close = bytes[end..].iter().position(|&b| b == b'>');
                match close {
                    Some(close_off) if close_off > 0 => bytes[end + close_off - 1] != b'/',
                    _ => false,
                }
            }
            _ => false,
        };
        if opens {
            // SAFETY: tag-name bytes are restricted to ASCII via
            // `is_tag_name_byte`, so the slice is valid UTF-8.
            let name = std::str::from_utf8(&bytes[start..end])
                .expect("tag-name bytes are ASCII");
            *out.entry(name.to_string()).or_insert(0) += 1;
        }
        i = end;
    }
    out
}

/// True when `b` is a legal continuation byte for an XML tag name
/// in our restricted scanner. We deliberately reject Unicode bytes
/// to keep the implementation simple — SmartPlant's tag names are
/// always ASCII and the false-negative rate on real fixtures is
/// zero.
fn is_tag_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Compute the [`SemanticDiffReport`] between `generated_xml` and
/// `reference_xml`. Both inputs are full `_Data.xml` document
/// strings; the function is pure (no I/O) so it is cheap to call
/// from tests, CI gates, and the CLI alike.
pub fn diff_publish_xml(generated_xml: &str, reference_xml: &str) -> SemanticDiffReport {
    let gen_counts = parse_pid_tag_counts(generated_xml);
    let ref_counts = parse_pid_tag_counts(reference_xml);

    let mut all_tags: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    all_tags.extend(gen_counts.keys().cloned());
    all_tags.extend(ref_counts.keys().cloned());

    let mut rows: Vec<TagCountDiff> = Vec::with_capacity(all_tags.len());
    let mut matching = 0;
    let mut missing = 0;
    let mut extra = 0;
    let mut count_deltas = 0;

    for tag in all_tags {
        let g = gen_counts.get(&tag).copied().unwrap_or(0);
        let r = ref_counts.get(&tag).copied().unwrap_or(0);
        let status = match (g, r) {
            (g, r) if g == r => {
                matching += 1;
                TagDiffStatus::Match
            }
            (0, _r) => {
                missing += 1;
                TagDiffStatus::MissingFromGenerated
            }
            (_g, 0) => {
                extra += 1;
                TagDiffStatus::ExtraInGenerated
            }
            _ => {
                count_deltas += 1;
                TagDiffStatus::CountDelta
            }
        };
        rows.push(TagCountDiff {
            tag,
            generated: g,
            reference: r,
            status,
        });
    }

    // Action-priority sort: MISSING > EXTRA > DELTA > MATCH, then
    // alphabetical within each bucket so the report is stable.
    rows.sort_by(|a, b| {
        let order = |s: TagDiffStatus| match s {
            TagDiffStatus::MissingFromGenerated => 0,
            TagDiffStatus::ExtraInGenerated => 1,
            TagDiffStatus::CountDelta => 2,
            TagDiffStatus::Match => 3,
        };
        order(a.status).cmp(&order(b.status)).then_with(|| a.tag.cmp(&b.tag))
    });

    SemanticDiffReport {
        generated_total: gen_counts.values().sum(),
        reference_total: ref_counts.values().sum(),
        tag_diffs: rows,
        matching,
        missing_from_generated: missing,
        extra_in_generated: extra,
        count_deltas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pid_tag_counts_handles_empty_input() {
        let counts = parse_pid_tag_counts("");
        assert!(counts.is_empty());
    }

    #[test]
    fn parse_pid_tag_counts_ignores_non_pid_tags() {
        let xml = "<Container><Foo/><PIDDrawing></PIDDrawing></Container>";
        let counts = parse_pid_tag_counts(xml);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts.get("PIDDrawing"), Some(&1));
    }

    #[test]
    fn parse_pid_tag_counts_skips_self_closing_pid_tags() {
        // `<IPID...>` interface markers and `<PIDxxx/>` self-closers
        // must not inflate the count — SmartPlant always emits
        // PID containers as paired open/close pairs.
        let xml = "<PIDFoo/><PIDBar></PIDBar>";
        let counts = parse_pid_tag_counts(xml);
        assert_eq!(counts.get("PIDFoo"), None);
        assert_eq!(counts.get("PIDBar"), Some(&1));
    }

    #[test]
    fn parse_pid_tag_counts_handles_attributes_in_open_tag() {
        let xml = r#"<PIDDrawing UID="X" Name="Y"></PIDDrawing>"#;
        let counts = parse_pid_tag_counts(xml);
        assert_eq!(counts.get("PIDDrawing"), Some(&1));
    }

    #[test]
    fn parse_pid_tag_counts_tallies_repeated_opens() {
        let xml = "<PIDFoo></PIDFoo><PIDFoo></PIDFoo><PIDFoo></PIDFoo>";
        let counts = parse_pid_tag_counts(xml);
        assert_eq!(counts.get("PIDFoo"), Some(&3));
    }

    #[test]
    fn parse_pid_tag_counts_handles_multiple_tags() {
        let xml =
            "<PIDDrawing></PIDDrawing><PIDNozzle></PIDNozzle><PIDNozzle></PIDNozzle>";
        let counts = parse_pid_tag_counts(xml);
        assert_eq!(counts.get("PIDDrawing"), Some(&1));
        assert_eq!(counts.get("PIDNozzle"), Some(&2));
    }

    #[test]
    fn parse_pid_tag_counts_does_not_match_pid_inside_attribute_text() {
        // `IsPID="1"` should NOT trigger a match because the byte
        // before `PID` is `s`, not `<`. This is critical to avoid
        // false positives in attribute-heavy SmartPlant payloads.
        let xml = r#"<Foo IsPID="1"></Foo>"#;
        let counts = parse_pid_tag_counts(xml);
        assert!(counts.is_empty());
    }

    #[test]
    fn diff_report_is_clean_when_both_sides_are_identical() {
        let xml = "<PIDFoo></PIDFoo><PIDBar></PIDBar>";
        let report = diff_publish_xml(xml, xml);
        assert!(report.is_clean(), "identical XML should diff clean: {report}");
        assert_eq!(report.matching, 2);
        assert_eq!(report.missing_from_generated, 0);
        assert_eq!(report.extra_in_generated, 0);
        assert_eq!(report.count_deltas, 0);
        assert_eq!(report.generated_total, 2);
        assert_eq!(report.reference_total, 2);
    }

    #[test]
    fn diff_report_flags_missing_tags_from_generated() {
        let gen = "<PIDFoo></PIDFoo>";
        let refer = "<PIDFoo></PIDFoo><PIDBar></PIDBar>";
        let report = diff_publish_xml(gen, refer);
        assert!(!report.is_clean());
        assert_eq!(report.missing_from_generated, 1);
        // `PIDBar` is in reference but not generated.
        let problem = report.problems().next().expect("one problem");
        assert_eq!(problem.tag, "PIDBar");
        assert_eq!(problem.generated, 0);
        assert_eq!(problem.reference, 1);
        assert!(matches!(
            problem.status,
            TagDiffStatus::MissingFromGenerated
        ));
        assert_eq!(problem.delta(), -1);
    }

    #[test]
    fn diff_report_flags_extra_tags_in_generated() {
        let gen = "<PIDFoo></PIDFoo><PIDExtra></PIDExtra>";
        let refer = "<PIDFoo></PIDFoo>";
        let report = diff_publish_xml(gen, refer);
        assert!(!report.is_clean());
        assert_eq!(report.extra_in_generated, 1);
        let problem = report.problems().next().expect("one problem");
        assert_eq!(problem.tag, "PIDExtra");
        assert_eq!(problem.generated, 1);
        assert_eq!(problem.reference, 0);
        assert!(matches!(problem.status, TagDiffStatus::ExtraInGenerated));
        assert_eq!(problem.delta(), 1);
    }

    #[test]
    fn diff_report_flags_count_delta_when_both_sides_have_tag() {
        let gen = "<PIDRep></PIDRep><PIDRep></PIDRep><PIDRep></PIDRep>";
        let refer = "<PIDRep></PIDRep>";
        let report = diff_publish_xml(gen, refer);
        assert!(!report.is_clean());
        assert_eq!(report.count_deltas, 1);
        let row = report.tag_diffs.iter().find(|r| r.tag == "PIDRep").expect("PIDRep");
        assert_eq!(row.generated, 3);
        assert_eq!(row.reference, 1);
        assert_eq!(row.delta(), 2);
        assert!(matches!(row.status, TagDiffStatus::CountDelta));
    }

    #[test]
    fn problems_iter_skips_match_rows_and_orders_by_severity() {
        let gen = "<PIDA></PIDA><PIDExtra></PIDExtra>";
        let refer = "<PIDA></PIDA><PIDMissing></PIDMissing>";
        let report = diff_publish_xml(gen, refer);
        let problems: Vec<&str> = report.problems().map(|p| p.tag.as_str()).collect();
        // MISSING ranks above EXTRA in our priority order.
        assert_eq!(problems, vec!["PIDMissing", "PIDExtra"]);
    }

    #[test]
    fn display_report_includes_summary_and_per_tag_rows() {
        let gen = "<PIDA></PIDA>";
        let refer = "<PIDA></PIDA><PIDB></PIDB>";
        let report = diff_publish_xml(gen, refer);
        let s = format!("{report}");
        assert!(s.contains("Publish Data XML semantic diff"));
        assert!(s.contains("Generated PID tags: 1"));
        assert!(s.contains("Reference PID tags: 2"));
        assert!(s.contains("MISSING"));
        assert!(s.contains("PIDB"));
        assert!(s.contains("MATCH"));
        assert!(s.contains("PIDA"));
    }

    #[test]
    fn diff_handles_real_fixture_subset_without_panics() {
        // Smoke test mimicking the ratio we measured on A01: ours
        // emits PIDPipingPort once, reference twice; reference adds
        // PIDProcessPoint we don't emit; ours emits more
        // PIDRepresentation than reference. The diff must classify
        // each correctly without panicking.
        let gen = concat!(
            "<PIDDrawing></PIDDrawing>",
            "<PIDProcessVessel></PIDProcessVessel>",
            "<PIDNozzle></PIDNozzle>",
            "<PIDPipeline></PIDPipeline>",
            "<PIDPipingConnector></PIDPipingConnector>",
            "<PIDPipingPort></PIDPipingPort>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
        );
        let refer = concat!(
            "<PIDDrawing></PIDDrawing>",
            "<PIDProcessVessel></PIDProcessVessel>",
            "<PIDNozzle></PIDNozzle>",
            "<PIDPipeline></PIDPipeline>",
            "<PIDPipingConnector></PIDPipingConnector>",
            "<PIDPipingPort></PIDPipingPort>",
            "<PIDPipingPort></PIDPipingPort>",
            "<PIDProcessPoint></PIDProcessPoint>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
            "<PIDRepresentation></PIDRepresentation>",
        );
        let report = diff_publish_xml(gen, refer);
        assert!(!report.is_clean());
        assert_eq!(report.missing_from_generated, 1, "PIDProcessPoint");
        assert_eq!(report.extra_in_generated, 0);
        assert_eq!(report.count_deltas, 2, "PIDPipingPort and PIDRepresentation");
        assert_eq!(report.generated_total, 12);
        assert_eq!(report.reference_total, 12);
    }
}
