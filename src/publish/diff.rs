//! Semantic diff between a `pid-parse`-generated `_Data.xml` and a
//! SmartPlant-produced reference `_Data.xml`.
//!
//! Stage-1 A12 establishes the comparison baseline: every PID tag
//! variety surfaces in a side-by-side count table together with a
//! `(generated - reference)` delta. The intent is *not* a textual
//! diff — `SmartPlant`'s exporter and ours emit different per-element
//! formatting, attribute ordering, and inline whitespace, all of
//! which would noise out a byte-level comparison.
//!
//! Instead the report answers two strategic questions:
//!
//! 1. **Coverage** — which PID tag varieties are missing from our
//!    output, and which extras are we emitting that `SmartPlant` never
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
    /// that `SmartPlant` skips on the source drawing.
    pub extra_in_generated: usize,
    /// Number of shared tag varieties whose counts differ.
    pub count_deltas: usize,
}

impl SemanticDiffReport {
    /// True when no actionable findings exist — every shared tag
    /// matches and neither side has unique tags. Useful as the CLI
    /// exit code gate.
    pub fn is_clean(&self) -> bool {
        self.missing_from_generated == 0 && self.extra_in_generated == 0 && self.count_deltas == 0
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
/// are NOT counted — `SmartPlant` always opens these as block
/// elements with an explicit `</PIDxxx>` closer, so a self-closing
/// match would always be a false positive.
///
/// The scanner ignores attributes; e.g. `<PIDDrawing Foo="bar">`
/// tallies as `"PIDDrawing"`. Tag-name characters are restricted
/// to `[A-Za-z0-9_]` to keep the scan robust to garbage payloads.
///
/// # Example
///
/// ```
/// use pid_parse::publish::parse_pid_tag_counts;
///
/// let xml = "<PIDDrawing></PIDDrawing>\
///            <PIDPipeline Foo=\"x\"></PIDPipeline>\
///            <PIDPipeline></PIDPipeline>";
/// let counts = parse_pid_tag_counts(xml);
/// assert_eq!(counts.get("PIDDrawing"), Some(&1));
/// assert_eq!(counts.get("PIDPipeline"), Some(&2));
/// ```
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
            let name = std::str::from_utf8(&bytes[start..end]).expect("tag-name bytes are ASCII");
            *out.entry(name.to_string()).or_insert(0) += 1;
        }
        i = end;
    }
    out
}

/// True when `b` is a legal continuation byte for an XML tag name
/// in our restricted scanner. We deliberately reject Unicode bytes
/// to keep the implementation simple — `SmartPlant`'s tag names are
/// always ASCII and the false-negative rate on real fixtures is
/// zero.
fn is_tag_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Compute the [`SemanticDiffReport`] between `generated_xml` and
/// `reference_xml`. Both inputs are full `_Data.xml` document
/// strings; the function is pure (no I/O) so it is cheap to call
/// from tests, CI gates, and the CLI alike.
/// PID tag varieties the writer is known to emit today. Sorted
/// for determinism. The list is the executable counterpart of the
/// `subtables_for_item_type` dispatch matrix in `xml_writer.rs`
/// plus the four virtual nodes (`PIDDrawing` / `PIDRepresentation` /
/// derived `PIDPipingPort` / `PIDProcessPoint`) that any non-trivial
/// drawing emits.
///
/// Used by [`coverage_against_reference`] to classify a reference
/// `SmartPlant` `_Data.xml` into "tags we already know how to emit"
/// vs "tags that form the next-phase backlog".
pub fn supported_pid_tags() -> &'static [&'static str] {
    &[
        "PIDBranchPoint",
        "PIDControlSystemFunction",
        "PIDDrawing",
        "PIDNote",
        "PIDNozzle",
        "PIDPipeline",
        "PIDPipingBranchPoint",
        "PIDPipingComponent",
        "PIDPipingConnector",
        "PIDPipingPort",
        "PIDProcessPoint",
        "PIDProcessVessel",
        "PIDRepresentation",
        "PIDSignalConnector",
        "PIDSignalPort",
    ]
}

/// One row of the [`WriterCoverage`] report. `count` is the
/// number of times the tag appears in the reference document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageRow {
    /// Tag name, e.g. `"PIDPipingPort"`.
    pub tag: String,
    /// Open-tag count observed in the reference document.
    pub count: usize,
}

/// Coverage report — the reference document's PID tag inventory
/// split into two buckets relative to [`supported_pid_tags`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriterCoverage {
    /// Tag varieties the writer can already emit, with their
    /// reference-side counts. Sorted alphabetically.
    pub supported_in_reference: Vec<CoverageRow>,
    /// Tag varieties the writer cannot yet emit. The actionable
    /// backlog. Sorted by descending count then alphabetically so
    /// the "biggest impact" rows come first.
    pub unsupported_in_reference: Vec<CoverageRow>,
}

impl WriterCoverage {
    /// Total reference PID tags accounted for by supported
    /// varieties — the numerator of the writer-coverage ratio.
    pub fn supported_total(&self) -> usize {
        self.supported_in_reference.iter().map(|r| r.count).sum()
    }

    /// Total reference PID tags the writer cannot emit yet.
    pub fn unsupported_total(&self) -> usize {
        self.unsupported_in_reference.iter().map(|r| r.count).sum()
    }

    /// True when every PID tag in the reference is one the writer
    /// knows how to emit.
    pub fn is_complete(&self) -> bool {
        self.unsupported_in_reference.is_empty()
    }
}

impl fmt::Display for WriterCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.supported_total();
        let u = self.unsupported_total();
        let total = s + u;
        let pct = if total == 0 {
            100.0_f64
        } else {
            (s as f64) * 100.0_f64 / (total as f64)
        };
        writeln!(f, "=== Publish writer coverage ===")?;
        writeln!(
            f,
            "Reference PID tags: {total}; supported tags: {s} ({pct:.1}%); backlog tags: {u}",
        )?;
        if !self.unsupported_in_reference.is_empty() {
            writeln!(f)?;
            writeln!(f, "{:<32} {:>6}", "Unsupported tag (backlog)", "Count")?;
            writeln!(f, "{}", "-".repeat(40))?;
            for row in &self.unsupported_in_reference {
                writeln!(f, "{:<32} {:>6}", row.tag, row.count)?;
            }
        }
        if !self.supported_in_reference.is_empty() {
            writeln!(f)?;
            writeln!(f, "{:<32} {:>6}", "Supported tag", "Count")?;
            writeln!(f, "{}", "-".repeat(40))?;
            for row in &self.supported_in_reference {
                writeln!(f, "{:<32} {:>6}", row.tag, row.count)?;
            }
        }
        Ok(())
    }
}

/// Compute the [`WriterCoverage`] of `reference_xml` against the
/// writer's [`supported_pid_tags`] set. Pure function — no I/O.
///
/// # Example
///
/// ```
/// use pid_parse::publish::coverage_against_reference;
///
/// let reference = "<PIDDrawing></PIDDrawing>\
///                  <PIDPhantom></PIDPhantom>";
/// let coverage = coverage_against_reference(reference);
/// // PIDDrawing is a supported tag (writer can emit it).
/// assert_eq!(coverage.supported_total(), 1);
/// // PIDPhantom is a fabricated backlog tag the writer cannot emit.
/// assert_eq!(coverage.unsupported_total(), 1);
/// assert!(!coverage.is_complete());
/// ```
pub fn coverage_against_reference(reference_xml: &str) -> WriterCoverage {
    let counts = parse_pid_tag_counts(reference_xml);
    let supported_set: std::collections::BTreeSet<&str> =
        supported_pid_tags().iter().copied().collect();

    let mut supported = Vec::new();
    let mut unsupported = Vec::new();
    for (tag, count) in counts {
        let row = CoverageRow {
            tag: tag.clone(),
            count,
        };
        if supported_set.contains(tag.as_str()) {
            supported.push(row);
        } else {
            unsupported.push(row);
        }
    }
    // Backlog wants impact-first ordering: descending count, then
    // alphabetical for ties.
    unsupported.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    // Supported list stays alphabetical (already sorted by BTreeMap iteration).

    WriterCoverage {
        supported_in_reference: supported,
        unsupported_in_reference: unsupported,
    }
}

/// Map from PID tag (e.g. `"PIDPipingConnector"`) to the set of
/// immediate child `<IFoo...>` interface names observed inside
/// the FIRST occurrence of that tag in `xml`.
///
/// # Example
///
/// ```
/// use pid_parse::publish::parse_interfaces_per_tag;
///
/// let xml = "<PIDPipeline>\
///                <IObject UID=\"X\"/>\
///                <IPBSItem/>\
///                <IPipeline/>\
///            </PIDPipeline>";
/// let ifaces = parse_interfaces_per_tag(xml);
/// let pipeline = ifaces.get("PIDPipeline").expect("entry");
/// assert!(pipeline.contains("IObject"));
/// assert!(pipeline.contains("IPBSItem"));
/// assert!(pipeline.contains("IPipeline"));
/// ```
///
/// Scanning strategy:
/// * Walk byte-by-byte looking for opening tag starts (`<`).
/// * When a PID container opens (`<PIDxxx>` or
///   `<PIDxxx attrs...>`, never self-closing), note the tag name
///   as the `active` container IFF we haven't already recorded
///   its first occurrence. Interfaces encountered while this
///   active container is still open will be inserted into the
///   tag's entry in the output map.
/// * When we see `</PIDxxx>` matching the active container, the
///   first occurrence is considered recorded and we stop
///   tracking interfaces for this tag.
/// * Second, third, ... occurrences of the same PID tag are
///   skipped — the first one is the representative template
///   (`SmartPlant` emits identical interface lists for every
///   instance of the same tag).
/// * Interfaces are any opening element whose name begins with
///   `I` and contains only ASCII alphanumerics plus `_`. Both
///   self-closing (`<IFoo/>`) and open forms (`<IFoo x="y"/>` or
///   `<IFoo>...</IFoo>`) are recorded identically — `SmartPlant`
///   always emits interfaces as self-closers inside PID
///   containers, so this is a non-issue in practice.
///
/// The helper is the counterpart of [`parse_pid_tag_counts`]
/// for interface-level fidelity analysis. Tests use it to assert
/// that every supported PID tag emits the same interface set as
/// the `SmartPlant` reference, closing the gap that coverage-level
/// (tag-only) checks leave open.
pub fn parse_interfaces_per_tag(xml: &str) -> BTreeMap<String, std::collections::BTreeSet<String>> {
    let bytes = xml.as_bytes();
    let mut out: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    // Name of the PID tag we are currently collecting interfaces
    // for. `None` when we are outside any tracked container (or
    // inside a container whose first occurrence we already
    // recorded).
    let mut active: Option<String> = None;
    let mut recorded: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let after_lt = i + 1;
        if after_lt >= bytes.len() {
            break;
        }
        let is_closing = bytes[after_lt] == b'/';
        let name_start = if is_closing { after_lt + 1 } else { after_lt };
        let mut name_end = name_start;
        while name_end < bytes.len() && is_tag_name_byte(bytes[name_end]) {
            name_end += 1;
        }
        if name_end == name_start {
            i = name_end.max(i + 1);
            continue;
        }
        let name = std::str::from_utf8(&bytes[name_start..name_end]).unwrap_or("");
        // Walk to `>` and detect self-close.
        let mut close = name_end;
        while close < bytes.len() && bytes[close] != b'>' {
            close += 1;
        }
        let self_closing =
            !is_closing && close > name_end && bytes[close.saturating_sub(1)] == b'/';
        if is_closing {
            if name.starts_with("PID") && active.as_deref() == Some(name) {
                // First occurrence complete — mark as recorded
                // and stop tracking interfaces.
                recorded.insert(name.to_string());
                active = None;
            }
        } else if name.starts_with("PID") && !self_closing {
            // Only start tracking the FIRST occurrence.
            if !recorded.contains(name) && active.is_none() {
                active = Some(name.to_string());
                // Ensure the entry exists even if the tag has no
                // interfaces (empty set is still meaningful).
                out.entry(name.to_string()).or_default();
            }
        } else if name.starts_with('I') {
            if let Some(tag) = active.as_deref() {
                out.entry(tag.to_string())
                    .or_default()
                    .insert(name.to_string());
            }
        }
        i = if close < bytes.len() {
            close + 1
        } else {
            close
        };
    }
    out
}

/// A26 · Attribute-name-level structural inventory per PID tag.
///
/// Returns a nested map: `PID tag -> interface -> set of
/// attribute names`. Only attribute NAMES are collected; values
/// are deliberately ignored so that data-driven content
/// differences (e.g. `FluidCode="@{abc}"` vs
/// `FluidCode="@{xyz}"`) do not register as drift while genuine
/// shape drift (e.g. DWG's `IEquipment` carries five attrs A01's
/// doesn't) does. Like [`parse_interfaces_per_tag`], only the
/// FIRST occurrence of each PID tag is considered representative
/// — `SmartPlant` emits identical attribute shapes for every
/// instance of the same tag on the same interface, so
/// first-occurrence-per-tag is lossless.
///
/// # Example
///
/// ```
/// use pid_parse::publish::parse_attrs_per_interface_per_tag;
///
/// let xml = "<PIDPipeline>\
///                <IObject UID=\"abc\" ItemTag=\"PH-001\"/>\
///                <IFluidSystem FluidCode=\"@{x}\" FluidSystem=\"@{y}\"/>\
///            </PIDPipeline>";
/// let attrs = parse_attrs_per_interface_per_tag(xml);
/// let pipeline = attrs.get("PIDPipeline").expect("entry");
/// // Each interface maps to an alphabetically-sorted attr name set.
/// assert!(pipeline.get("IObject").unwrap().contains("UID"));
/// assert!(pipeline.get("IObject").unwrap().contains("ItemTag"));
/// assert!(pipeline.get("IFluidSystem").unwrap().contains("FluidCode"));
/// ```
///
/// The helper is the attribute-level counterpart to
/// [`parse_interfaces_per_tag`] (A23) and the cross-fixture
/// parity gate (A24). Tests use it to pin attribute-shape
/// invariants across fixtures — any newly appeared attribute
/// on DWG that A01 doesn't have (or vice versa) shows up as
/// a concrete `(tag, interface, attr)` triplet diagnostic.
///
/// Internally this reuses [`parse_interfaces_per_tag`]'s
/// first-occurrence tracking loop, extended to parse attribute
/// names from the interface opening tag. Attribute parsing is
/// deliberately tolerant: an attribute is any maximal run of
/// ASCII alphanumerics plus `_` that sits between whitespace
/// and an `=` character, inside the tag's `<...>` brackets
/// and outside of any quoted value.
pub fn parse_attrs_per_interface_per_tag(
    xml: &str,
) -> BTreeMap<String, BTreeMap<String, std::collections::BTreeSet<String>>> {
    let bytes = xml.as_bytes();
    let mut out: BTreeMap<String, BTreeMap<String, std::collections::BTreeSet<String>>> =
        BTreeMap::new();
    let mut active: Option<String> = None;
    let mut recorded: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let after_lt = i + 1;
        if after_lt >= bytes.len() {
            break;
        }
        let is_closing = bytes[after_lt] == b'/';
        let name_start = if is_closing { after_lt + 1 } else { after_lt };
        let mut name_end = name_start;
        while name_end < bytes.len() && is_tag_name_byte(bytes[name_end]) {
            name_end += 1;
        }
        if name_end == name_start {
            i = name_end.max(i + 1);
            continue;
        }
        let name = std::str::from_utf8(&bytes[name_start..name_end]).unwrap_or("");
        let mut close = name_end;
        while close < bytes.len() && bytes[close] != b'>' {
            close += 1;
        }
        let self_closing =
            !is_closing && close > name_end && bytes[close.saturating_sub(1)] == b'/';
        if is_closing {
            if name.starts_with("PID") && active.as_deref() == Some(name) {
                recorded.insert(name.to_string());
                active = None;
            }
        } else if name.starts_with("PID") && !self_closing {
            if !recorded.contains(name) && active.is_none() {
                active = Some(name.to_string());
                out.entry(name.to_string()).or_default();
            }
        } else if name.starts_with('I') {
            if let Some(tag) = active.as_deref() {
                // Parse attr names out of the opening tag bytes
                // (between `name_end` and `close`, exclusive of
                // any trailing `/` for self-closing forms).
                let attrs_end = if self_closing {
                    close.saturating_sub(1)
                } else {
                    close
                };
                let attr_bytes = &bytes[name_end..attrs_end];
                let attrs = parse_attr_names(attr_bytes);
                let interface_entry = out
                    .entry(tag.to_string())
                    .or_default()
                    .entry(name.to_string())
                    .or_default();
                for a in attrs {
                    interface_entry.insert(a);
                }
            }
        }
        i = if close < bytes.len() {
            close + 1
        } else {
            close
        };
    }
    out
}

/// Extract attribute NAMES from the interior bytes of an
/// opening tag (between the tag-name end and the `>` close,
/// excluding any trailing `/` for self-closers).
///
/// Scans left-to-right: skips whitespace, reads an attr-name
/// run (ASCII alphanumeric + `_`), expects `=`, then skips the
/// quoted value. Malformed runs terminate attribute collection
/// gracefully without panicking — the caller gets whatever was
/// parsed cleanly up to the first malformed token.
fn parse_attr_names(inside: &[u8]) -> std::collections::BTreeSet<String> {
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut i = 0usize;
    while i < inside.len() {
        // Skip whitespace.
        while i < inside.len() && inside[i].is_ascii_whitespace() {
            i += 1;
        }
        let name_start = i;
        while i < inside.len() && is_attr_name_byte(inside[i]) {
            i += 1;
        }
        if i == name_start {
            // No name found here — advance one byte and retry
            // so a stray char doesn't loop forever.
            i += 1;
            continue;
        }
        let name = std::str::from_utf8(&inside[name_start..i]).unwrap_or("");
        // Skip whitespace and expect `=`.
        while i < inside.len() && inside[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= inside.len() || inside[i] != b'=' {
            // Name not followed by `=` — valueless attribute,
            // skip but still record the name.
            if !name.is_empty() {
                names.insert(name.to_string());
            }
            continue;
        }
        i += 1;
        while i < inside.len() && inside[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= inside.len() {
            break;
        }
        let quote = inside[i];
        if quote != b'"' && quote != b'\'' {
            // Unquoted value — skip until whitespace or `/`.
            while i < inside.len() && !inside[i].is_ascii_whitespace() && inside[i] != b'/' {
                i += 1;
            }
        } else {
            i += 1;
            while i < inside.len() && inside[i] != quote {
                i += 1;
            }
            if i < inside.len() {
                i += 1;
            }
        }
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    names
}

/// Attribute names follow the `SmartPlant` XML convention: ASCII
/// alphanumeric plus `_`. Hyphens and colons never appear in
/// the fixtures we deal with, so we don't extend the charset.
fn is_attr_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// A33 · Per-`DefUID` Rel inventory.
///
/// `SmartPlant` Publish Data XML carries two top-level
/// element families: `<PIDxxx>` business objects (covered
/// by A12 / A23 / A27) and `<Rel>` relationship records
/// (this helper). Each `<Rel>` wraps an `<IRel UID1="..."
/// UID2="..." DefUID="..."/>` line whose `DefUID` is an
/// enum-like identifier such as `DrawingItems`,
/// `DwgRepresentationComposition`, `PipingConnectors`,
/// `PipingEnd1Conn`, `ProcessPointCollection`. The `DefUID`
/// classifies the relationship semantically; a writer that
/// emits the right per-tag interface and attribute set but
/// the wrong Rel inventory still produces broken
/// `SmartPlant` input.
///
/// This helper scans the XML byte stream for
/// `DefUID="..."` occurrences inside `<IRel ...>` opening
/// tags and returns a `DefUID -> count` map. It is
/// deliberately byte-level (no XML parser) so it stays
/// cheap and resilient to formatting drift, mirroring
/// [`parse_pid_tag_counts`]'s strategy.
///
/// Counting strategy:
/// * Walk byte-by-byte looking for `<IRel`.
/// * From there, scan forward to the next `>` (the `IRel`
///   opening tag closer); inside that span, locate
///   `DefUID="..."` and capture the quoted value.
/// * If multiple `DefUID=` instances exist on the same
///   element (impossible in well-formed SPPID XML), only
///   the FIRST one counts.
/// * Self-closing form (`/>`) and verbose closing
///   (`</IRel>`) are treated identically — only the
///   opening tag matters.
///
/// # Example
///
/// ```
/// use pid_parse::publish::parse_rel_defuid_counts;
///
/// let xml = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
///            <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>\
///            <IRel UID1=\"e\" UID2=\"f\" DefUID=\"PipingConnectors\"/>";
/// let counts = parse_rel_defuid_counts(xml);
/// assert_eq!(counts.get("DrawingItems"), Some(&2));
/// assert_eq!(counts.get("PipingConnectors"), Some(&1));
/// ```
pub fn parse_rel_defuid_counts(xml: &str) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    let needle = b"<IRel";
    while i + needle.len() <= bytes.len() {
        if bytes[i..].starts_with(needle) {
            // Found `<IRel`; ensure the next byte is whitespace
            // or `>` so we don't false-positive on `<IRelations>`
            // or any future hypothetical sibling element.
            let after = bytes.get(i + needle.len()).copied();
            let valid_after = matches!(after, Some(b) if b == b'>' || b.is_ascii_whitespace());
            if !valid_after {
                i += 1;
                continue;
            }
            // Walk to the closing `>` to bound the search.
            let Some(scan_end) = bytes[i..]
                .iter()
                .position(|&b| b == b'>')
                .map(|off| i + off)
            else {
                break;
            };
            // Search within [i + needle.len(), scan_end) for
            // `DefUID="..."`.
            if let Some(defuid) = extract_defuid(&bytes[i + needle.len()..scan_end]) {
                *out.entry(defuid).or_insert(0) += 1;
            }
            i = scan_end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// A36 · One parsed `<IRel ...>` record.
///
/// Carries the three attributes the Stage-1 fidelity gates
/// care about — `UID1`, `UID2`, `DefUID`. Values are owned
/// strings so the parser output detaches from the input XML
/// lifetime. Missing attributes surface as empty strings
/// rather than `Option<...>` to keep the downstream gate
/// assertions succinct; well-formed SPPID output always
/// populates all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelDetail {
    /// Source side of the relationship (`UID1` on `<IRel>`).
    pub uid1: String,
    /// Target side of the relationship (`UID2` on `<IRel>`).
    pub uid2: String,
    /// Relationship kind (`DefUID` on `<IRel>`), e.g.
    /// `"PipingEnd1Conn"`, `"DrawingItems"`.
    pub def_uid: String,
}

/// A36 · Per-`<IRel>` inventory with `UID1` / `UID2` / `DefUID`
/// triples.
///
/// Sibling of [`parse_rel_defuid_counts`]: same byte-level
/// scan, same robustness to formatting drift, but instead of
/// collapsing to per-DefUID counts it returns every
/// individual Rel record so callers can assert UID-level
/// fidelity gates.
///
/// The primary consumer is the A36 / A36b `publish_rel_parity`
/// gate: after A34c switched `PipingEnd1Conn.UID2` from an
/// intra-connector `.PPT` placeholder to the real upstream
/// `ModelItem` UID, a future refactor could silently reintroduce
/// the placeholder and still satisfy the count-only A33 gate.
/// This helper lets tests distinguish the two.
///
/// Parsing rules mirror [`parse_rel_defuid_counts`]:
/// * Byte-level scan for `<IRel`.
/// * Only the opening tag is consumed — self-closing form
///   (`/>`) and verbose closing (`</IRel>`) are treated
///   identically.
/// * Attributes are looked up within the span `<IRel ...>`.
///   Whichever order `SmartPlant` emits them in (and the two
///   reference fixtures differ here) is accepted.
/// * Missing attributes produce empty-string fields rather
///   than dropping the record; downstream code decides what
///   to do with the partial data.
/// * Single- and double-quoted values are both accepted.
///
/// # Example
///
/// ```
/// use pid_parse::publish::{parse_rel_details, RelDetail};
///
/// let xml = "<IRel UID1=\"a.1\" UID2=\"b\" DefUID=\"PipingEnd1Conn\"/>\
///            <IRel UID1=\"a.2\" UID2=\"a.PPT\" DefUID=\"PipingEnd2Conn\"/>";
/// let rels = parse_rel_details(xml);
/// assert_eq!(rels.len(), 2);
/// assert_eq!(
///     rels[0],
///     RelDetail {
///         uid1: "a.1".into(),
///         uid2: "b".into(),
///         def_uid: "PipingEnd1Conn".into(),
///     },
/// );
/// // The second rel carries a `.PPT` placeholder UID2 — the
/// // A36b gate uses this exact distinguishing property to
/// // tell "unconnected port" from "connected port".
/// assert_eq!(rels[1].uid2, "a.PPT");
/// ```
pub fn parse_rel_details(xml: &str) -> Vec<RelDetail> {
    let mut out: Vec<RelDetail> = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    let needle = b"<IRel";
    while i + needle.len() <= bytes.len() {
        if bytes[i..].starts_with(needle) {
            // Guard against `<IRelations>` / future siblings.
            let after = bytes.get(i + needle.len()).copied();
            let valid_after = matches!(after, Some(b) if b == b'>' || b.is_ascii_whitespace());
            if !valid_after {
                i += 1;
                continue;
            }
            let Some(scan_end) = bytes[i..]
                .iter()
                .position(|&b| b == b'>')
                .map(|off| i + off)
            else {
                break;
            };
            let inside = &bytes[i + needle.len()..scan_end];
            out.push(RelDetail {
                uid1: extract_rel_attr(inside, b"UID1=").unwrap_or_default(),
                uid2: extract_rel_attr(inside, b"UID2=").unwrap_or_default(),
                def_uid: extract_rel_attr(inside, b"DefUID=").unwrap_or_default(),
            });
            i = scan_end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Extract a quoted attribute value from an `<IRel ...>` interior
/// byte slice. Accepts either `"..."` or `'...'` quoting. Returns
/// `None` when the attribute is absent / malformed / has no
/// closing quote.
///
/// Shared between [`parse_rel_details`] and [`extract_defuid`];
/// the latter is kept as a thin wrapper so A33's public API and
/// its call site stay byte-level identical.
fn extract_rel_attr(inside: &[u8], needle: &[u8]) -> Option<String> {
    let mut i = 0usize;
    while i + needle.len() <= inside.len() {
        if inside[i..].starts_with(needle) {
            let after_eq = i + needle.len();
            if after_eq >= inside.len() {
                return None;
            }
            let quote = inside[after_eq];
            if quote != b'"' && quote != b'\'' {
                return None;
            }
            let value_start = after_eq + 1;
            let close = inside[value_start..]
                .iter()
                .position(|&b| b == quote)
                .map(|off| value_start + off)?;
            return std::str::from_utf8(&inside[value_start..close])
                .ok()
                .map(str::to_string);
        }
        i += 1;
    }
    None
}

/// Extract the value of a `DefUID="..."` attribute from a
/// byte slice that represents the interior of an `<IRel ...>`
/// opening tag (excluding the `<IRel` prefix and the trailing
/// `>`). Returns `None` when the attribute is absent or
/// malformed.
///
/// A36: thin wrapper over the generic [`extract_rel_attr`] so
/// both the A33 DefUID-only fast path and the A36
/// full-triple parser share one quoting implementation.
fn extract_defuid(inside: &[u8]) -> Option<String> {
    extract_rel_attr(inside, b"DefUID=")
}

/// Compare a generated publish `_Data.xml` body against a reference
/// XML from an existing `SmartPlant` export. Returns a semantic diff
/// focusing on tag counts, interfaces, and relationship families.
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
        order(a.status)
            .cmp(&order(b.status))
            .then_with(|| a.tag.cmp(&b.tag))
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

/// A40 — one row of per-`DefUID` count comparison.
///
/// Mirrors [`TagCountDiff`] one level deeper: every `DefUID`
/// that appears in either the generated or reference document
/// gets a row. Reuses [`TagDiffStatus`] so CLI display code
/// can treat `<PIDxxx>` counts and `<IRel>` `DefUID` counts
/// with one formatting path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelDefUidDiff {
    /// The `DefUID` value, e.g. `"PipingEnd1Conn"`,
    /// `"DrawingItems"`.
    pub def_uid: String,
    /// Count of `<IRel DefUID="..."/>` occurrences in the
    /// generated document.
    pub generated: usize,
    /// Count of the same in the reference document.
    pub reference: usize,
    /// Classification reusing the `<PIDxxx>` tag-status enum.
    pub status: TagDiffStatus,
}

impl RelDefUidDiff {
    /// Signed delta `generated - reference`.
    pub fn delta(&self) -> i64 {
        self.generated as i64 - self.reference as i64
    }
}

/// A40 — aggregate Rel-level diff report, sibling of
/// [`SemanticDiffReport`].
///
/// Counts every `<IRel>` in both documents grouped by
/// `DefUID`. Use via [`diff_rel_defuids`]; the `Display`
/// impl renders a canonical text table the CLI's
/// `--diff-against` flow appends after the `<PIDxxx>` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelDefUidDiffReport {
    /// Total `<IRel>` count in the generated document.
    pub generated_total: usize,
    /// Total `<IRel>` count in the reference document.
    pub reference_total: usize,
    /// Per-DefUID rows, sorted action-priority (MISSING >
    /// EXTRA > DELTA > MATCH, alphabetical within each).
    pub rows: Vec<RelDefUidDiff>,
    /// Number of `DefUIDs` whose counts match exactly.
    pub matching: usize,
    /// Number of `DefUIDs` present in reference but not
    /// generated.
    pub missing_from_generated: usize,
    /// Number of `DefUIDs` present in generated but not
    /// reference.
    pub extra_in_generated: usize,
    /// Number of shared `DefUIDs` with different counts.
    pub count_deltas: usize,
}

impl RelDefUidDiffReport {
    /// True when the generated document's Rel inventory
    /// matches the reference exactly at DefUID-count
    /// granularity.
    pub fn is_clean(&self) -> bool {
        self.missing_from_generated == 0 && self.extra_in_generated == 0 && self.count_deltas == 0
    }

    /// Convenience: only the problematic rows, in priority
    /// order. Skips `Match`.
    pub fn problems(&self) -> impl Iterator<Item = &RelDefUidDiff> {
        self.rows
            .iter()
            .filter(|r| !matches!(r.status, TagDiffStatus::Match))
    }
}

impl fmt::Display for RelDefUidDiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Publish Data XML Rel DefUID diff ===")?;
        writeln!(
            f,
            "Generated Rels: {}; Reference Rels: {}; DefUIDs matched: {}; missing: {}; extra: {}; count deltas: {}",
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
            "{:<8} {:>9} {:>9} {:>7}  DefUID",
            "Status", "Generated", "Reference", "Delta",
        )?;
        writeln!(f, "{}", "-".repeat(56))?;
        for row in &self.rows {
            writeln!(
                f,
                "{:<8} {:>9} {:>9} {:>+7}  {}",
                row.status.to_string(),
                row.generated,
                row.reference,
                row.delta(),
                row.def_uid,
            )?;
        }
        Ok(())
    }
}

/// A40 — compute a DefUID-level diff between two Publish
/// Data XML documents.
///
/// Sibling of [`diff_publish_xml`] but for `<IRel>` records.
/// `<PIDxxx>` counts and Rel counts answer complementary
/// questions — the former says "are all business objects
/// emitted?", the latter says "are all cross-references
/// emitted?" A drawing can pass one gate and fail the other
/// (the writer could emit every PID tag while dropping every
/// `T_Relationship` row, or vice versa).
///
/// The returned report uses the same `MISSING > EXTRA >
/// DELTA > MATCH` priority ordering as
/// [`SemanticDiffReport`] so the CLI can render both tables
/// with one formatter routine.
///
/// # Example
///
/// ```
/// use pid_parse::publish::diff_rel_defuids;
///
/// let gen = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
///            <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>";
/// let refr = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
///             <IRel UID1=\"a\" UID2=\"b\" DefUID=\"DwgRepresentationComposition\"/>";
/// let report = diff_rel_defuids(gen, refr);
/// assert!(!report.is_clean());
/// assert_eq!(report.generated_total, 2);
/// assert_eq!(report.reference_total, 2);
/// // `DrawingItems` count differs (2 vs 1) so it lands as DELTA;
/// // `DwgRepresentationComposition` is missing from generated.
/// assert_eq!(report.missing_from_generated, 1);
/// assert_eq!(report.count_deltas, 1);
/// ```
pub fn diff_rel_defuids(generated_xml: &str, reference_xml: &str) -> RelDefUidDiffReport {
    let gen_counts = parse_rel_defuid_counts(generated_xml);
    let ref_counts = parse_rel_defuid_counts(reference_xml);

    let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    all.extend(gen_counts.keys().cloned());
    all.extend(ref_counts.keys().cloned());

    let mut rows: Vec<RelDefUidDiff> = Vec::with_capacity(all.len());
    let mut matching = 0;
    let mut missing = 0;
    let mut extra = 0;
    let mut count_deltas = 0;

    for def_uid in all {
        let g = gen_counts.get(&def_uid).copied().unwrap_or(0);
        let r = ref_counts.get(&def_uid).copied().unwrap_or(0);
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
        rows.push(RelDefUidDiff {
            def_uid,
            generated: g,
            reference: r,
            status,
        });
    }

    // Same action-priority sort as the PID-tag report so the
    // combined --diff-against output reads uniformly.
    rows.sort_by(|a, b| {
        let order = |s: TagDiffStatus| match s {
            TagDiffStatus::MissingFromGenerated => 0,
            TagDiffStatus::ExtraInGenerated => 1,
            TagDiffStatus::CountDelta => 2,
            TagDiffStatus::Match => 3,
        };
        order(a.status)
            .cmp(&order(b.status))
            .then_with(|| a.def_uid.cmp(&b.def_uid))
    });

    RelDefUidDiffReport {
        generated_total: gen_counts.values().sum(),
        reference_total: ref_counts.values().sum(),
        rows,
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
        let xml = "<PIDDrawing></PIDDrawing><PIDNozzle></PIDNozzle><PIDNozzle></PIDNozzle>";
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
        assert!(
            report.is_clean(),
            "identical XML should diff clean: {report}"
        );
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
        let row = report
            .tag_diffs
            .iter()
            .find(|r| r.tag == "PIDRep")
            .expect("PIDRep");
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

    // -----------------------------------------------------------------
    // A15 — writer coverage classifier
    // -----------------------------------------------------------------

    #[test]
    fn supported_pid_tags_is_sorted_and_non_empty() {
        let tags = supported_pid_tags();
        assert!(!tags.is_empty(), "writer should declare at least one tag");
        for win in tags.windows(2) {
            assert!(
                win[0] < win[1],
                "supported tags must be sorted (kept stable for diffing); offender: {win:?}"
            );
        }
        // Spot-check the post-A18 milestone set: every tag the
        // writer emits today must appear here. `PIDSignalPort`
        // joined in A16 (derived from InstrFunction);
        // `PIDPipingComponent` joined in A17 (PipingComp writer arm);
        // `PIDSignalConnector` joined in A18 (SignalRun writer arm).
        for must_have in [
            "PIDBranchPoint",
            "PIDControlSystemFunction",
            "PIDDrawing",
            "PIDNote",
            "PIDNozzle",
            "PIDPipeline",
            "PIDPipingBranchPoint",
            "PIDPipingComponent",
            "PIDPipingConnector",
            "PIDPipingPort",
            "PIDProcessPoint",
            "PIDProcessVessel",
            "PIDRepresentation",
            "PIDSignalConnector",
            "PIDSignalPort",
        ] {
            assert!(
                tags.contains(&must_have),
                "supported set missing {must_have}; got {tags:?}"
            );
        }
    }

    #[test]
    fn coverage_on_empty_xml_is_complete_and_zero_total() {
        let cov = coverage_against_reference("");
        assert!(cov.is_complete(), "empty xml has no backlog");
        assert_eq!(cov.supported_total(), 0);
        assert_eq!(cov.unsupported_total(), 0);
    }

    #[test]
    fn coverage_recognizes_only_supported_tags() {
        // Reference contains exclusively supported tags.
        let xml = "<PIDDrawing></PIDDrawing><PIDDrawing></PIDDrawing><PIDNozzle></PIDNozzle>";
        let cov = coverage_against_reference(xml);
        assert!(cov.is_complete(), "all tags supported -> complete");
        assert_eq!(cov.supported_total(), 3);
        assert_eq!(cov.unsupported_total(), 0);
        assert_eq!(cov.supported_in_reference.len(), 2);
        // Alphabetical order on supported rows.
        assert_eq!(cov.supported_in_reference[0].tag, "PIDDrawing");
        assert_eq!(cov.supported_in_reference[0].count, 2);
        assert_eq!(cov.supported_in_reference[1].tag, "PIDNozzle");
        assert_eq!(cov.supported_in_reference[1].count, 1);
    }

    #[test]
    fn coverage_classifies_unsupported_tags_into_backlog() {
        // Mixed: PIDDrawing + PIDBranchPoint + PIDPipingBranchPoint
        // are all supported now. Backlog tags use fabricated names
        // (PIDTypical, PIDPhantom, PIDFuture) that will never ship
        // so the test is immune to future writer expansion.
        let xml = concat!(
            "<PIDDrawing></PIDDrawing>",
            "<PIDBranchPoint></PIDBranchPoint>",
            "<PIDPipingBranchPoint></PIDPipingBranchPoint>",
            "<PIDPhantom></PIDPhantom><PIDPhantom></PIDPhantom><PIDPhantom></PIDPhantom>",
            "<PIDTypical></PIDTypical><PIDTypical></PIDTypical>",
            "<PIDFuture></PIDFuture>",
        );
        let cov = coverage_against_reference(xml);
        assert!(!cov.is_complete());
        assert_eq!(cov.supported_total(), 3);
        assert_eq!(cov.unsupported_total(), 6);
        // Backlog ordering: descending count, then alphabetical.
        assert_eq!(cov.unsupported_in_reference[0].tag, "PIDPhantom");
        assert_eq!(cov.unsupported_in_reference[0].count, 3);
        assert_eq!(cov.unsupported_in_reference[1].tag, "PIDTypical");
        assert_eq!(cov.unsupported_in_reference[1].count, 2);
        assert_eq!(cov.unsupported_in_reference[2].tag, "PIDFuture");
        assert_eq!(cov.unsupported_in_reference[2].count, 1);
    }

    #[test]
    fn coverage_display_includes_percentage_and_two_blocks() {
        // Use a fabricated backlog tag (PIDPhantom) so the
        // percentage stays interesting.
        let xml = "<PIDDrawing></PIDDrawing><PIDPhantom></PIDPhantom>";
        let cov = coverage_against_reference(xml);
        let s = format!("{cov}");
        assert!(s.contains("Publish writer coverage"));
        assert!(s.contains("Reference PID tags: 2"));
        assert!(s.contains("supported tags: 1 (50.0%)"));
        assert!(s.contains("backlog tags: 1"));
        assert!(s.contains("Unsupported tag (backlog)"));
        assert!(s.contains("Supported tag"));
        assert!(s.contains("PIDPhantom"));
        assert!(s.contains("PIDDrawing"));
    }

    #[test]
    fn coverage_display_omits_blocks_when_one_side_is_empty() {
        // All supported -> only one block.
        let xml = "<PIDDrawing></PIDDrawing>";
        let cov = coverage_against_reference(xml);
        let s = format!("{cov}");
        assert!(s.contains("Supported tag"));
        assert!(!s.contains("Unsupported tag (backlog)"));

        // None supported -> only the backlog block. PIDPhantom
        // is a fabricated tag that will never be supported.
        let xml = "<PIDPhantom></PIDPhantom>";
        let cov = coverage_against_reference(xml);
        let s = format!("{cov}");
        assert!(s.contains("Unsupported tag (backlog)"));
        assert!(!s.contains("Supported tag\n"));
    }

    #[test]
    fn coverage_total_equals_sum_of_two_buckets() {
        let xml = concat!(
            "<PIDDrawing></PIDDrawing>",
            "<PIDNozzle></PIDNozzle>",
            "<PIDBranchPoint></PIDBranchPoint>",
            "<PIDPipingBranchPoint></PIDPipingBranchPoint>",
        );
        let cov = coverage_against_reference(xml);
        assert_eq!(
            cov.supported_total() + cov.unsupported_total(),
            4,
            "totals should partition every PID tag exactly once"
        );
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
        assert_eq!(
            report.count_deltas, 2,
            "PIDPipingPort and PIDRepresentation"
        );
        assert_eq!(report.generated_total, 12);
        assert_eq!(report.reference_total, 12);
    }

    // -----------------------------------------------------------------
    // A23 — interface-per-tag parser
    // -----------------------------------------------------------------

    #[test]
    fn parse_interfaces_per_tag_captures_all_children_of_single_pid_tag() {
        let xml = concat!(
            "<PIDPipeline>",
            "<IObject/>",
            "<IPBSItem/>",
            r#"<IFluidSystem FluidCode="" FluidSystem=""/>"#,
            "<IPIDTypical/>",
            "</PIDPipeline>",
        );
        let ifaces = parse_interfaces_per_tag(xml);
        let pipeline = ifaces.get("PIDPipeline").expect("pipeline entry");
        for expected in ["IObject", "IPBSItem", "IFluidSystem", "IPIDTypical"] {
            assert!(
                pipeline.contains(expected),
                "PIDPipeline interfaces must include `{expected}`; got {pipeline:?}"
            );
        }
    }

    #[test]
    fn parse_interfaces_per_tag_ignores_second_occurrence_of_same_tag() {
        // Two PIDPipeline blocks. The first has IObject + IPBSItem;
        // the second has IObject + INewInterface. The function
        // must return only the first occurrence's interface set.
        let xml = concat!(
            "<PIDPipeline>",
            "<IObject/>",
            "<IPBSItem/>",
            "</PIDPipeline>",
            "<PIDPipeline>",
            "<IObject/>",
            "<INewInterface/>",
            "</PIDPipeline>",
        );
        let ifaces = parse_interfaces_per_tag(xml);
        let pipeline = ifaces.get("PIDPipeline").expect("pipeline entry");
        assert!(pipeline.contains("IObject"));
        assert!(pipeline.contains("IPBSItem"));
        assert!(
            !pipeline.contains("INewInterface"),
            "second-occurrence-only interface must not leak into the first-occurrence set; got {pipeline:?}"
        );
    }

    #[test]
    fn parse_interfaces_per_tag_collects_multiple_tag_types_independently() {
        let xml = concat!(
            "<PIDPipeline>",
            "<IPipeline/>",
            "</PIDPipeline>",
            "<PIDNozzle>",
            "<INozzle/>",
            "<INozzleOcc/>",
            "</PIDNozzle>",
        );
        let ifaces = parse_interfaces_per_tag(xml);
        assert_eq!(ifaces.len(), 2);
        assert!(ifaces["PIDPipeline"].contains("IPipeline"));
        assert!(ifaces["PIDNozzle"].contains("INozzle"));
        assert!(ifaces["PIDNozzle"].contains("INozzleOcc"));
        assert!(!ifaces["PIDPipeline"].contains("INozzle"));
    }

    #[test]
    fn parse_interfaces_per_tag_handles_attribute_heavy_opens() {
        // SmartPlant interfaces carry attributes; the parser must
        // ignore attribute content and just record the element
        // name.
        let xml = concat!(
            "<PIDPipingConnector>",
            r#"<IObject UID="X" Name="Y"/>"#,
            r#"<IConnector FlowDirection="@EE872" RepresentationsAreAllZeroLength="False"/>"#,
            r#"<INamedPipingConnector PipingConnectorPrefix="" PipingConnectorSeqNo="0101" PipingConnectorSuff=""/>"#,
            "</PIDPipingConnector>",
        );
        let ifaces = parse_interfaces_per_tag(xml);
        let entry = ifaces.get("PIDPipingConnector").expect("entry");
        for expected in ["IObject", "IConnector", "INamedPipingConnector"] {
            assert!(
                entry.contains(expected),
                "attribute-heavy interfaces must still register; missing `{expected}`"
            );
        }
    }

    #[test]
    fn parse_interfaces_per_tag_tolerates_unicode_text_content() {
        // SPPID ships attribute content with Chinese characters
        // and entity references. The parser must not be confused
        // by non-ASCII bytes in attribute values.
        let xml = concat!(
            "<PIDNote>",
            r#"<IObject UID="D317F1AAC79641E985B955779CCDF051"/>"#,
            "<IDrawingItem/>",
            "<IPBSNote/>",
            r#"<INote NoteText="量液孔"/>"#,
            "<IDocumentItem/>",
            "</PIDNote>",
        );
        let ifaces = parse_interfaces_per_tag(xml);
        let note = ifaces.get("PIDNote").expect("entry");
        assert_eq!(note.len(), 5);
        for expected in [
            "IObject",
            "IDrawingItem",
            "IPBSNote",
            "INote",
            "IDocumentItem",
        ] {
            assert!(
                note.contains(expected),
                "interface `{expected}` must round-trip through unicode attribute content; got {note:?}"
            );
        }
    }

    #[test]
    fn parse_interfaces_per_tag_returns_empty_for_xml_with_no_pid_tags() {
        let ifaces = parse_interfaces_per_tag("<Container><Foo/></Container>");
        assert!(
            ifaces.is_empty(),
            "no PID tags means no entries; got {ifaces:?}"
        );
    }

    // -----------------------------------------------------------------
    // A26 — parse_attrs_per_interface_per_tag: attribute-name-level
    // structural inventory that sits one layer below
    // parse_interfaces_per_tag. Tests below pin the parser's
    // behavior on:
    //   * bare interfaces (no attrs)
    //   * interfaces with single and multiple attrs
    //   * unicode attribute content
    //   * entity-escaped attribute values
    //   * open-form interfaces with nested children
    //   * first-occurrence-only semantics (same as A23)
    //   * empty input
    // -----------------------------------------------------------------

    #[test]
    fn parse_attrs_collects_single_attr_from_self_closing_interface() {
        let xml = concat!(
            "<PIDPipeline>",
            r#"<IObject UID="abc"/>"#,
            r#"<IFluidSystem FluidCode="@{guid}" FluidSystem="@x"/>"#,
            "<IExpandableThing/>",
            "</PIDPipeline>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let pipeline = attrs.get("PIDPipeline").expect("PIDPipeline entry");
        let iobject = pipeline.get("IObject").expect("IObject entry");
        assert_eq!(
            iobject.iter().cloned().collect::<Vec<_>>(),
            vec!["UID".to_string()],
            "IObject should surface its UID attr name only; got {iobject:?}"
        );
        let fluid = pipeline.get("IFluidSystem").expect("IFluidSystem entry");
        assert_eq!(
            fluid.iter().cloned().collect::<Vec<_>>(),
            vec!["FluidCode".to_string(), "FluidSystem".to_string()],
            "IFluidSystem should surface both attr names in sorted order; got {fluid:?}"
        );
        let expand = pipeline
            .get("IExpandableThing")
            .expect("IExpandableThing entry");
        assert!(
            expand.is_empty(),
            "bare interface should have empty attr set; got {expand:?}"
        );
    }

    #[test]
    fn parse_attrs_ignores_attribute_values_only_records_names() {
        // Two PIDPipelines with the same interface shape but
        // different attr VALUES. The parser should report
        // identical attr-name sets — values must not leak in.
        let xml = concat!(
            "<PIDPipeline>",
            r#"<IObject UID="one" Name="alpha"/>"#,
            "</PIDPipeline>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let iobject_attrs = attrs
            .get("PIDPipeline")
            .and_then(|ifs| ifs.get("IObject"))
            .expect("IObject entry");
        assert_eq!(
            iobject_attrs.iter().cloned().collect::<Vec<_>>(),
            vec!["Name".to_string(), "UID".to_string()],
            "attr name set should be independent of values; got {iobject_attrs:?}"
        );
    }

    #[test]
    fn parse_attrs_tolerates_unicode_in_attribute_values() {
        // SPPID ships Chinese text in attribute values. The
        // parser must skip past them without corrupting the
        // parse state.
        let xml = concat!(
            "<PIDProcessVessel>",
            r#"<IObject UID="C57494A1B154442C9DF0F4BA713E88EC" Description="污水池"/>"#,
            r#"<ISpecifiedMatlItem LongMaterialDescription="新建"/>"#,
            "</PIDProcessVessel>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let pvessel = attrs
            .get("PIDProcessVessel")
            .expect("PIDProcessVessel entry");
        let iobject_attrs = pvessel.get("IObject").expect("IObject");
        assert_eq!(
            iobject_attrs.iter().cloned().collect::<Vec<_>>(),
            vec!["Description".to_string(), "UID".to_string()],
            "unicode in values must not break name parsing; got {iobject_attrs:?}"
        );
        let matl_attrs = pvessel
            .get("ISpecifiedMatlItem")
            .expect("ISpecifiedMatlItem");
        assert_eq!(
            matl_attrs.iter().cloned().collect::<Vec<_>>(),
            vec!["LongMaterialDescription".to_string()],
        );
    }

    #[test]
    fn parse_attrs_handles_entity_escaped_values() {
        // Entity references like &amp; / &#13;&#10; appear in
        // SPPID DocTitle. The parser walks the quoted value
        // opaquely so escapes are non-issues for attr-name
        // parsing.
        let xml = concat!(
            "<PIDDrawing>",
            r#"<IObject UID="UID-D" Name="DWG-0202GP06-01" Description=""/>"#,
            r#"<IDocument DocCategory="P&amp;ID Documents" DocTitle="title&#13;&#10;with entities" DocType="P&amp;ID" DocSubtype=""/>"#,
            "</PIDDrawing>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let idoc_attrs = attrs
            .get("PIDDrawing")
            .and_then(|ifs| ifs.get("IDocument"))
            .expect("IDocument entry");
        assert_eq!(
            idoc_attrs.iter().cloned().collect::<Vec<_>>(),
            vec![
                "DocCategory".to_string(),
                "DocSubtype".to_string(),
                "DocTitle".to_string(),
                "DocType".to_string(),
            ],
        );
    }

    #[test]
    fn parse_attrs_first_occurrence_only_semantics_same_as_interfaces() {
        // Second PIDPipeline adds an attribute SmartPlant
        // would never actually change at runtime — but the
        // parser must still ignore it, matching A23 semantics.
        let xml = concat!(
            "<PIDPipeline>",
            r#"<IObject UID="first"/>"#,
            "</PIDPipeline>",
            "<PIDPipeline>",
            r#"<IObject UID="second" Shadow="secondary"/>"#,
            "</PIDPipeline>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let iobject_attrs = attrs
            .get("PIDPipeline")
            .and_then(|ifs| ifs.get("IObject"))
            .expect("IObject entry");
        assert_eq!(
            iobject_attrs.iter().cloned().collect::<Vec<_>>(),
            vec!["UID".to_string()],
            "second occurrence's attrs must not leak into the first's set; got {iobject_attrs:?}"
        );
    }

    #[test]
    fn parse_attrs_returns_empty_for_xml_with_no_pid_tags() {
        let attrs = parse_attrs_per_interface_per_tag("<Container><Foo bar=\"x\"/></Container>");
        assert!(
            attrs.is_empty(),
            "no PID tags means no entries; got {attrs:?}"
        );
    }

    #[test]
    fn parse_attrs_bare_interfaces_have_empty_attr_sets() {
        // Every empty interface inside a PID tag should show
        // up as an empty-set entry (not missing from the map).
        let xml = concat!(
            "<PIDPipingPort>",
            "<IObject/>",
            "<IPortComposition/>",
            "<IPipingConnection/>",
            "</PIDPipingPort>"
        );
        let attrs = parse_attrs_per_interface_per_tag(xml);
        let port = attrs.get("PIDPipingPort").expect("PIDPipingPort entry");
        for iface in ["IObject", "IPortComposition", "IPipingConnection"] {
            let s = port.get(iface).unwrap_or_else(|| {
                panic!("interface {iface} missing from map, expected empty-set entry: {port:?}")
            });
            assert!(
                s.is_empty(),
                "{iface} should map to an empty attr set (bare form); got {s:?}"
            );
        }
    }

    // -----------------------------------------------------------------
    // A33 — Rel-level DefUID counter
    // -----------------------------------------------------------------

    #[test]
    fn parse_rel_defuid_counts_returns_empty_for_empty_input() {
        let counts = parse_rel_defuid_counts("");
        assert!(counts.is_empty());
    }

    #[test]
    fn parse_rel_defuid_counts_returns_empty_when_no_rel_present() {
        let xml = "<Container><PIDDrawing></PIDDrawing></Container>";
        let counts = parse_rel_defuid_counts(xml);
        assert!(
            counts.is_empty(),
            "no <IRel> means no defuid entries; got {counts:?}"
        );
    }

    #[test]
    fn parse_rel_defuid_counts_extracts_single_defuid_value() {
        let xml = concat!(
            "<Rel><IObject UID=\"R1\"/>",
            "<IRel UID1=\"A\" UID2=\"B\" DefUID=\"DrawingItems\"/>",
            "</Rel>"
        );
        let counts = parse_rel_defuid_counts(xml);
        assert_eq!(counts.get("DrawingItems"), Some(&1));
        assert_eq!(counts.len(), 1);
    }

    #[test]
    fn parse_rel_defuid_counts_aggregates_across_multiple_rel_blocks() {
        // 3 of the same DefUID + 1 different — should produce
        // two entries with the right counts.
        let xml = concat!(
            "<IRel UID1=\"A\" UID2=\"B\" DefUID=\"DrawingItems\"/>",
            "<IRel UID1=\"C\" UID2=\"D\" DefUID=\"DrawingItems\"/>",
            "<IRel UID1=\"E\" UID2=\"F\" DefUID=\"DrawingItems\"/>",
            "<IRel UID1=\"G\" UID2=\"H\" DefUID=\"PipingConnectors\"/>"
        );
        let counts = parse_rel_defuid_counts(xml);
        assert_eq!(counts.get("DrawingItems"), Some(&3));
        assert_eq!(counts.get("PipingConnectors"), Some(&1));
    }

    #[test]
    fn parse_rel_defuid_counts_ignores_irelations_lookalikes() {
        // A future hypothetical sibling element whose name
        // begins with `<IRel...` (e.g. `<IRelations>`) must
        // NOT register as an `<IRel>`. The next byte after
        // `<IRel` must be whitespace or `>` to count.
        let xml = "<IRelationship UID=\"X\"/><IRelations Foo=\"y\"/>";
        let counts = parse_rel_defuid_counts(xml);
        assert!(
            counts.is_empty(),
            "lookalike elements must not match; got {counts:?}"
        );
    }

    #[test]
    fn parse_rel_defuid_counts_handles_self_closing_form() {
        // Bothered elements <IRel .../> (self-closing) and
        // <IRel ...>...</IRel> (open) must yield identical
        // results — only the opening tag is consulted.
        let xml = concat!(
            "<IRel UID1=\"A\" UID2=\"B\" DefUID=\"FoosBars\"/>",
            "<IRel UID1=\"C\" UID2=\"D\" DefUID=\"FoosBars\"></IRel>",
        );
        let counts = parse_rel_defuid_counts(xml);
        assert_eq!(counts.get("FoosBars"), Some(&2));
    }

    #[test]
    fn parse_rel_defuid_counts_skips_irel_without_defuid_attribute() {
        // Defensive: an `<IRel/>` lacking DefUID must not
        // crash and must not insert any entry. Real SmartPlant
        // exports always carry DefUID; this branch protects
        // against malformed inputs.
        let xml = "<IRel UID1=\"A\" UID2=\"B\"/>";
        let counts = parse_rel_defuid_counts(xml);
        assert!(
            counts.is_empty(),
            "DefUID-less IRel must not register; got {counts:?}"
        );
    }

    #[test]
    fn parse_rel_defuid_counts_accepts_single_quoted_defuid_value() {
        // SmartPlant ships `DefUID="..."` exclusively, but the
        // parser should be tolerant to single quotes for
        // synthetic / hand-crafted fixtures.
        let xml = "<IRel UID1=\"A\" UID2=\"B\" DefUID='DrawingItems'/>";
        let counts = parse_rel_defuid_counts(xml);
        assert_eq!(counts.get("DrawingItems"), Some(&1));
    }

    // -----------------------------------------------------------------
    // A36 — parse_rel_details (full UID1/UID2/DefUID triple)
    // -----------------------------------------------------------------

    #[test]
    fn parse_rel_details_returns_empty_for_empty_input() {
        assert!(parse_rel_details("").is_empty());
    }

    #[test]
    fn parse_rel_details_extracts_single_triple_in_fixed_attribute_order() {
        let xml = "<IRel UID1=\"PORT.1\" UID2=\"NOZZ-1\" DefUID=\"PipingEnd1Conn\"/>";
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 1);
        assert_eq!(
            rels[0],
            RelDetail {
                uid1: "PORT.1".into(),
                uid2: "NOZZ-1".into(),
                def_uid: "PipingEnd1Conn".into(),
            }
        );
    }

    #[test]
    fn parse_rel_details_preserves_relative_order_across_multiple_rels() {
        // A36b gate's soundness check walks the Rel list in
        // document order to correlate (UID1 prefix → UID2
        // type). Preserve emit order so that logic is
        // deterministic.
        let xml = concat!(
            "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"X\"/>",
            "<IRel UID1=\"c\" UID2=\"d\" DefUID=\"Y\"/>",
            "<IRel UID1=\"e\" UID2=\"f\" DefUID=\"X\"/>",
        );
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 3);
        assert_eq!(rels[0].uid1, "a");
        assert_eq!(rels[1].uid1, "c");
        assert_eq!(rels[2].uid1, "e");
        assert_eq!(rels[0].def_uid, "X");
        assert_eq!(rels[1].def_uid, "Y");
        assert_eq!(rels[2].def_uid, "X");
    }

    #[test]
    fn parse_rel_details_tolerates_attributes_in_any_order() {
        // SPPID emits `UID1 UID2 DefUID` but hand-edited fixtures
        // (and any future attribute reordering in the exporter)
        // should still parse cleanly.
        let xml = "<IRel DefUID=\"PipingEnd1Conn\" UID2=\"B\" UID1=\"A\"/>";
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].uid1, "A");
        assert_eq!(rels[0].uid2, "B");
        assert_eq!(rels[0].def_uid, "PipingEnd1Conn");
    }

    #[test]
    fn parse_rel_details_ignores_irelations_lookalikes() {
        // Same guard as the A33 parser — `<IRelations>` or any
        // other element whose name starts with `IRel` but
        // continues with extra letters must not match.
        let xml = "<IRelations UID1=\"X\" UID2=\"Y\" DefUID=\"Fake\"/>";
        let rels = parse_rel_details(xml);
        assert!(rels.is_empty(), "lookalike must not register; got {rels:?}");
    }

    #[test]
    fn parse_rel_details_fills_empty_strings_for_missing_attributes() {
        // A malformed Rel without UID2 / DefUID is still
        // surfaced so the A36 gate can count records, but the
        // missing fields are empty strings. The A36 soundness
        // check is responsible for rejecting them.
        let xml = "<IRel UID1=\"ORPHAN\"/>";
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].uid1, "ORPHAN");
        assert_eq!(rels[0].uid2, "");
        assert_eq!(rels[0].def_uid, "");
    }

    #[test]
    fn parse_rel_details_handles_self_closing_and_verbose_forms() {
        let xml = concat!(
            "<IRel UID1=\"A\" UID2=\"B\" DefUID=\"DrawingItems\"/>",
            "<IRel UID1=\"C\" UID2=\"D\" DefUID=\"DrawingItems\"></IRel>",
        );
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].uid2, "B");
        assert_eq!(rels[1].uid2, "D");
    }

    #[test]
    fn parse_rel_details_accepts_single_quoted_values() {
        let xml = "<IRel UID1='A' UID2='B' DefUID='DrawingItems'/>";
        let rels = parse_rel_details(xml);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].uid1, "A");
        assert_eq!(rels[0].uid2, "B");
        assert_eq!(rels[0].def_uid, "DrawingItems");
    }

    // -----------------------------------------------------------------
    // A40 — diff_rel_defuids (per-DefUID diff report)
    // -----------------------------------------------------------------

    #[test]
    fn diff_rel_defuids_reports_clean_when_counts_match_exactly() {
        let gen = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
                   <IRel UID1=\"c\" UID2=\"d\" DefUID=\"PipingConnectors\"/>";
        let refr = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
                    <IRel UID1=\"c\" UID2=\"d\" DefUID=\"PipingConnectors\"/>";
        let report = diff_rel_defuids(gen, refr);
        assert!(report.is_clean());
        assert_eq!(report.generated_total, 2);
        assert_eq!(report.reference_total, 2);
        assert_eq!(report.matching, 2);
        assert_eq!(report.missing_from_generated, 0);
        assert_eq!(report.extra_in_generated, 0);
        assert_eq!(report.count_deltas, 0);
    }

    #[test]
    fn diff_rel_defuids_classifies_missing_and_extra_and_delta_buckets() {
        // gen has PC×1 + DrI×1; ref has PC×1 + EQC×2 + DrI×3.
        // Expected: PC matches; EQC missing from generated;
        // DrI count-delta; (nothing extra-in-generated).
        let gen = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"PipingConnectors\"/>\
                   <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>";
        let refr = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"PipingConnectors\"/>\
                    <IRel UID1=\"x\" UID2=\"y\" DefUID=\"EquipmentComponentComposition\"/>\
                    <IRel UID1=\"x\" UID2=\"y\" DefUID=\"EquipmentComponentComposition\"/>\
                    <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>\
                    <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>\
                    <IRel UID1=\"c\" UID2=\"d\" DefUID=\"DrawingItems\"/>";
        let report = diff_rel_defuids(gen, refr);
        assert!(!report.is_clean());
        assert_eq!(report.generated_total, 2);
        assert_eq!(report.reference_total, 6);
        assert_eq!(report.matching, 1);
        assert_eq!(report.missing_from_generated, 1);
        assert_eq!(report.extra_in_generated, 0);
        assert_eq!(report.count_deltas, 1);
        // Row ordering — missing bucket first.
        assert_eq!(report.rows[0].status, TagDiffStatus::MissingFromGenerated);
        assert_eq!(report.rows[0].def_uid, "EquipmentComponentComposition");
    }

    #[test]
    fn diff_rel_defuids_surfaces_writer_extras_in_their_own_bucket() {
        // Writer emits a DefUID the reference does not. This is
        // valid (the writer sometimes over-emits derived rels
        // SmartPlant skips) but should still be visible in the
        // report's EXTRA bucket.
        let gen = "<IRel UID1=\"x\" UID2=\"y\" DefUID=\"DerivedExtra\"/>";
        let refr = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>";
        let report = diff_rel_defuids(gen, refr);
        assert_eq!(report.matching, 0);
        assert_eq!(report.missing_from_generated, 1);
        assert_eq!(report.extra_in_generated, 1);
    }

    #[test]
    fn diff_rel_defuids_handles_fully_empty_inputs() {
        let report = diff_rel_defuids("", "");
        assert!(report.is_clean());
        assert!(report.rows.is_empty());
        assert_eq!(report.generated_total, 0);
        assert_eq!(report.reference_total, 0);
    }

    #[test]
    fn rel_def_uid_diff_report_problems_iterator_skips_matches() {
        let gen = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>\
                   <IRel UID1=\"a\" UID2=\"b\" DefUID=\"ExtraOnly\"/>";
        let refr = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>";
        let report = diff_rel_defuids(gen, refr);
        let problems: Vec<&str> = report.problems().map(|r| r.def_uid.as_str()).collect();
        assert_eq!(problems, vec!["ExtraOnly"]);
    }

    #[test]
    fn rel_def_uid_diff_report_display_has_table_header_and_summary_line() {
        let gen = "<IRel UID1=\"a\" UID2=\"b\" DefUID=\"DrawingItems\"/>";
        let refr = gen;
        let s = format!("{}", diff_rel_defuids(gen, refr));
        assert!(s.contains("=== Publish Data XML Rel DefUID diff ==="));
        assert!(
            s.contains("DefUID"),
            "Display header must label DefUID column"
        );
        assert!(s.contains("MATCH"));
        assert!(s.contains("DrawingItems"));
    }
}
