//! Process-independent execution for read-only `pid_inspect` commands.
//!
//! The CLI remains responsible for argument parsing, filesystem-oriented
//! setup, writing output streams, and mapping [`InspectStatus`] to an exit
//! code. This module renders command results into owned strings so callers
//! can exercise inspect behavior without spawning a process.

use crate::byte_audit::{
    compare_byte_audit_reports, ByteAuditComparison, ByteAuditImprovementKind,
    ByteAuditRegressionKind, ByteAuditReport,
};
use crate::geometry::{NormalizedPidGeometry, PidGeometryConfidence, PidGraphicKind};
use crate::model::{ParseCoverageStatus, PidDocument};
use crate::package::PidPackage;
use std::fmt::{self, Write};

/// An inspect operation that is exclusive at the CLI boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectCommand {
    /// Render one or more compatible read-only views.
    Views(InspectRequest),
    /// Serialize normalized source-backed geometry as pretty JSON.
    GeometryJson,
    /// Render a human-readable normalized-geometry summary.
    GeometrySummary,
}

/// One read-only view that may be combined with other views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectView {
    /// Render the full package-aware report.
    Report,
    /// Render parse-coverage inventory.
    Coverage,
    /// Render whole-package byte-audit information.
    ByteAudit,
    /// Render the semantic object graph as Mermaid.
    ObjectGraphMermaid,
    /// Render the cross-reference graph as Mermaid.
    CrossReferenceMermaid,
}

/// Output encoding for a set of inspect views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectOutputFormat {
    /// Human-readable text.
    Text,
    /// Pretty-printed JSON following the legacy CLI precedence rules.
    Json,
}

/// A composable set of read-only inspect views.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectRequest {
    /// Requested output encoding.
    pub format: InspectOutputFormat,
    /// Views to render, in caller-selected order for text output.
    pub views: Vec<InspectView>,
    /// Optional byte-audit baseline used when [`InspectView::ByteAudit`] runs.
    pub byte_audit_baseline: Option<ByteAuditReport>,
}

impl InspectRequest {
    /// Construct an empty request in the selected output format.
    pub fn new(format: InspectOutputFormat) -> Self {
        Self {
            format,
            views: Vec::new(),
            byte_audit_baseline: None,
        }
    }

    /// Append a view while preserving the requested rendering order.
    pub fn with_view(mut self, view: InspectView) -> Self {
        self.views.push(view);
        self
    }

    /// Attach a byte-audit baseline for comparison.
    pub fn with_byte_audit_baseline(mut self, baseline: ByteAuditReport) -> Self {
        self.byte_audit_baseline = Some(baseline);
        self
    }
}

/// Semantic completion state returned by an inspect command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectStatus {
    /// Rendering completed without policy findings.
    Success,
    /// Rendering completed but found a CI-relevant regression.
    Findings,
}

/// Owned output from a process-independent inspect command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectOutcome {
    /// Bytes intended for standard output, represented as UTF-8 text.
    pub stdout: String,
    /// Diagnostics intended for standard error, represented as UTF-8 text.
    pub stderr: String,
    /// Semantic completion state for CLI exit-code mapping.
    pub status: InspectStatus,
}

impl InspectOutcome {
    fn success() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            status: InspectStatus::Success,
        }
    }
}

/// Failure while rendering an inspect command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectError {
    message: String,
}

impl InspectError {
    fn serialization(context: &str, source: serde_json::Error) -> Self {
        Self {
            message: format!("{context}: {source}"),
        }
    }
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InspectError {}

/// Execute a read-only inspect command without writing to process streams or
/// terminating the process.
pub fn run(pkg: &PidPackage, command: &InspectCommand) -> Result<InspectOutcome, InspectError> {
    match command {
        InspectCommand::Views(request) => run_views(pkg, request),
        InspectCommand::GeometryJson => {
            let geometry = crate::build_normalized_geometry(&pkg.parsed);
            let stdout = pretty_json_line(&geometry, "Geometry JSON serialization error")?;
            Ok(InspectOutcome {
                stdout,
                ..InspectOutcome::success()
            })
        }
        InspectCommand::GeometrySummary => {
            let geometry = crate::build_normalized_geometry(&pkg.parsed);
            let mut outcome = InspectOutcome::success();
            render_geometry_summary(&mut outcome.stdout, &geometry);
            Ok(outcome)
        }
    }
}

fn run_views(pkg: &PidPackage, request: &InspectRequest) -> Result<InspectOutcome, InspectError> {
    if request.format == InspectOutputFormat::Json {
        return run_json_views(pkg, request);
    }

    let mut outcome = InspectOutcome::success();
    for view in &request.views {
        match view {
            InspectView::Report => {
                outcome
                    .stdout
                    .push_str(&crate::inspect::report::generate_package_report(pkg));
            }
            InspectView::Coverage => render_coverage(&mut outcome.stdout, &pkg.parsed),
            InspectView::ByteAudit => render_byte_audit(pkg, request, &mut outcome),
            InspectView::ObjectGraphMermaid => {
                let rendered = crate::inspect::mermaid::object_graph_mermaid(&pkg.parsed);
                if rendered.is_empty() {
                    outcome
                        .stderr
                        .push_str("(no object graph available — nothing to render)\n");
                } else {
                    outcome.stdout.push_str(&rendered);
                }
            }
            InspectView::CrossReferenceMermaid => {
                let rendered = crate::inspect::mermaid::crossref_mermaid(&pkg.parsed);
                if rendered.is_empty() {
                    outcome
                        .stderr
                        .push_str("(no cross-reference graph — nothing to render)\n");
                } else {
                    outcome.stdout.push_str(&rendered);
                }
            }
        }
    }
    Ok(outcome)
}

fn run_json_views(
    pkg: &PidPackage,
    request: &InspectRequest,
) -> Result<InspectOutcome, InspectError> {
    // Preserve the CLI's historical JSON precedence: coverage wins over
    // byte-audit, and any other combination serializes the whole document.
    if request.views.contains(&InspectView::Coverage) {
        let report = crate::inspect::coverage::coverage_report(&pkg.parsed);
        return Ok(InspectOutcome {
            stdout: pretty_json_line(&report, "Coverage JSON serialization error")?,
            ..InspectOutcome::success()
        });
    }

    if request.views.contains(&InspectView::ByteAudit) {
        let current = crate::byte_audit_report(pkg);
        if let Some(baseline) = request.byte_audit_baseline.as_ref() {
            let comparison = compare_byte_audit_reports(baseline, &current);
            let status = if comparison.is_clean() {
                InspectStatus::Success
            } else {
                InspectStatus::Findings
            };
            return Ok(InspectOutcome {
                stdout: pretty_json_line(
                    &comparison,
                    "Byte audit comparison JSON serialization error",
                )?,
                stderr: String::new(),
                status,
            });
        }
        return Ok(InspectOutcome {
            stdout: pretty_json_line(&current, "Byte audit JSON serialization error")?,
            ..InspectOutcome::success()
        });
    }

    Ok(InspectOutcome {
        stdout: pretty_json_line(&pkg.parsed, "JSON serialization error")?,
        ..InspectOutcome::success()
    })
}

fn pretty_json_line<T: serde::Serialize>(value: &T, context: &str) -> Result<String, InspectError> {
    serde_json::to_string_pretty(value)
        .map(|json| format!("{json}\n"))
        .map_err(|source| InspectError::serialization(context, source))
}

fn render_coverage(out: &mut String, doc: &PidDocument) {
    let report = crate::inspect::coverage::coverage_report(doc);
    if report.entries.is_empty() {
        writeln!(out, "--- Coverage ---").ok();
        writeln!(out, "(no top-level entries found; document may be empty)").ok();
        return;
    }
    let [full, partial, ident, unk] = report.status_counts();
    writeln!(out, "--- Coverage ---").ok();
    writeln!(out, "  Fully decoded:     {full}").ok();
    writeln!(out, "  Partially decoded: {partial}").ok();
    writeln!(out, "  Identified only:   {ident}").ok();
    writeln!(out, "  Unknown:           {unk}").ok();
    for entry in &report.entries {
        let tag = match entry.status {
            ParseCoverageStatus::FullyDecoded => "[FULL]",
            ParseCoverageStatus::PartiallyDecoded => "[PART]",
            ParseCoverageStatus::IdentifiedOnly => "[ID]  ",
            ParseCoverageStatus::Unknown => "[UNK] ",
        };
        let field = entry
            .document_field
            .as_deref()
            .map(|field| format!(" -> {field}"))
            .unwrap_or_default();
        let note = entry
            .note
            .as_deref()
            .map(|note| format!("  ({note})"))
            .unwrap_or_default();
        writeln!(out, "  {tag} {}{}{}", entry.name, field, note).ok();
    }
}

fn render_byte_audit(pkg: &PidPackage, request: &InspectRequest, outcome: &mut InspectOutcome) {
    let current = crate::byte_audit_report(pkg);
    if let Some(baseline) = request.byte_audit_baseline.as_ref() {
        let comparison = compare_byte_audit_reports(baseline, &current);
        render_byte_audit_comparison(&mut outcome.stdout, &comparison);
        if !comparison.is_clean() {
            outcome.status = InspectStatus::Findings;
        }
    } else {
        render_byte_audit_report(&mut outcome.stdout, &current);
    }
}

fn render_byte_audit_report(out: &mut String, report: &ByteAuditReport) {
    writeln!(out, "--- Byte Audit ---").ok();
    writeln!(out, "Total stream bytes: {}", report.total_file_bytes).ok();
    writeln!(out, "Overall consumed:   {}", report.overall_consumed).ok();
    writeln!(out, "Overall leftover:   {}", report.overall_leftover).ok();
    writeln!(
        out,
        "Overall coverage:   {:.1}%",
        report.overall_coverage_ratio * 100.0
    )
    .ok();
    writeln!(
        out,
        "Fully consumed traced streams: {}",
        report.fully_consumed_stream_count()
    )
    .ok();
    writeln!(
        out,
        "Unregistered streams: {}",
        report.unregistered_paths.len()
    )
    .ok();

    for summary in report.per_stream.values() {
        let parser = summary.parser_name.as_deref().unwrap_or("unregistered");
        writeln!(
            out,
            "  [{:>5.1}%] {} ({} B consumed / {} B total, {} B leftover) {}",
            summary.coverage_ratio * 100.0,
            summary.path,
            summary.consumed_bytes,
            summary.total_bytes,
            summary.leftover_bytes,
            parser
        )
        .ok();
    }
}

fn render_byte_audit_comparison(out: &mut String, comparison: &ByteAuditComparison) {
    writeln!(out, "--- Byte Audit Baseline Comparison ---").ok();
    writeln!(out, "Regressions: {}", comparison.regressions.len()).ok();
    for regression in &comparison.regressions {
        writeln!(
            out,
            "  [{}] {} baseline={} current={}",
            byte_audit_regression_kind_label(regression.kind),
            regression.path.as_deref().unwrap_or("(overall)"),
            regression.baseline_value,
            regression.current_value,
        )
        .ok();
    }
    writeln!(out, "Improvements: {}", comparison.improvements.len()).ok();
    for improvement in &comparison.improvements {
        writeln!(
            out,
            "  [{}] {} baseline={} current={}",
            byte_audit_improvement_kind_label(improvement.kind),
            improvement.path.as_deref().unwrap_or("(overall)"),
            improvement.baseline_value,
            improvement.current_value,
        )
        .ok();
    }
}

fn byte_audit_regression_kind_label(kind: ByteAuditRegressionKind) -> &'static str {
    match kind {
        ByteAuditRegressionKind::OverallCoverageDecreased => "overall_coverage_decreased",
        ByteAuditRegressionKind::StreamMissing => "stream_missing",
        ByteAuditRegressionKind::StreamConsumedBytesDecreased => "stream_consumed_bytes_decreased",
        ByteAuditRegressionKind::StreamBecameUnregistered => "stream_became_unregistered",
    }
}

fn byte_audit_improvement_kind_label(kind: ByteAuditImprovementKind) -> &'static str {
    match kind {
        ByteAuditImprovementKind::StreamBecameTraced => "stream_became_traced",
        ByteAuditImprovementKind::NewTracedStream => "new_traced_stream",
    }
}

fn render_geometry_summary(out: &mut String, geometry: &NormalizedPidGeometry) {
    let mut decoded_lines = 0usize;
    let mut decoded_polylines = 0usize;
    let mut decoded_points = 0usize;
    let mut decoded_texts = 0usize;
    let mut decoded_symbols = 0usize;
    let mut inferred_lines = 0usize;
    let mut inferred_points = 0usize;
    let mut inferred_annotations = 0usize;
    let mut probe_only_unknown = 0usize;
    let mut other = 0usize;

    let mut sample_decoded_texts: Vec<String> = Vec::new();
    let mut sample_decoded_symbol_oids: Vec<u32> = Vec::new();

    for entity in &geometry.entities {
        match (&entity.confidence, &entity.kind) {
            (PidGeometryConfidence::Decoded, PidGraphicKind::Line { .. }) => {
                decoded_lines += 1;
            }
            (PidGeometryConfidence::Decoded, PidGraphicKind::Polyline { .. }) => {
                decoded_polylines += 1;
            }
            (PidGeometryConfidence::Decoded, PidGraphicKind::Point { .. }) => {
                decoded_points += 1;
            }
            (PidGeometryConfidence::Decoded, PidGraphicKind::Text { value, .. }) => {
                decoded_texts += 1;
                if sample_decoded_texts.len() < 8 {
                    sample_decoded_texts.push(value.clone());
                }
            }
            (PidGeometryConfidence::Decoded, PidGraphicKind::SymbolInstance { .. }) => {
                decoded_symbols += 1;
                if let Some(oid) = entity.graphic_oid {
                    if sample_decoded_symbol_oids.len() < 5 {
                        sample_decoded_symbol_oids.push(oid);
                    }
                }
            }
            (PidGeometryConfidence::Inferred, PidGraphicKind::Line { .. }) => {
                inferred_lines += 1;
            }
            (PidGeometryConfidence::Inferred, PidGraphicKind::Point { .. }) => {
                inferred_points += 1;
            }
            (PidGeometryConfidence::Inferred, PidGraphicKind::Annotation { .. }) => {
                inferred_annotations += 1;
            }
            (PidGeometryConfidence::ProbeOnly, PidGraphicKind::Unknown { .. }) => {
                probe_only_unknown += 1;
            }
            _ => {
                other += 1;
            }
        }
    }

    let total = geometry.entities.len();
    writeln!(out, "=== Sheet stream geometry summary ===").ok();
    writeln!(out, "Total entities: {total}").ok();
    writeln!(out).ok();
    writeln!(out, "Decoded (PSM record geometry):").ok();
    writeln!(
        out,
        "  Lines (GLine2d / igLine2d):              {decoded_lines}"
    )
    .ok();
    writeln!(
        out,
        "  Polylines (igLineString2d):               {decoded_polylines}"
    )
    .ok();
    writeln!(
        out,
        "  Points (igPoint2d):                       {decoded_points}"
    )
    .ok();
    writeln!(
        out,
        "  Texts (igTextBox, UTF-16LE):              {decoded_texts}"
    )
    .ok();
    writeln!(
        out,
        "  SymbolInstances (igSymbol2d):             {decoded_symbols}"
    )
    .ok();
    writeln!(
        out,
        "  Total decoded:                            {}",
        decoded_lines + decoded_polylines + decoded_points + decoded_texts + decoded_symbols
    )
    .ok();
    writeln!(out).ok();
    writeln!(out, "Inferred (probe-derived):").ok();
    writeln!(
        out,
        "  Points (coordinate hints):                {inferred_points}"
    )
    .ok();
    writeln!(
        out,
        "  Lines (endpoint pairs):                   {inferred_lines}"
    )
    .ok();
    writeln!(
        out,
        "  Annotations (JStyleOverride, PSM 0x0030): {inferred_annotations}"
    )
    .ok();
    writeln!(
        out,
        "  Total inferred:                           {}",
        inferred_points + inferred_lines + inferred_annotations
    )
    .ok();
    writeln!(out).ok();
    writeln!(out, "ProbeOnly (raw evidence, undecoded):").ok();
    writeln!(
        out,
        "  Unknown:                                  {probe_only_unknown}"
    )
    .ok();
    if other > 0 {
        writeln!(out).ok();
        writeln!(out, "Other:                                    {other}").ok();
    }

    if !sample_decoded_texts.is_empty() {
        writeln!(out).ok();
        writeln!(out, "Sample decoded texts:").ok();
        for text in &sample_decoded_texts {
            writeln!(out, "  {text:?}").ok();
        }
    }
    if !sample_decoded_symbol_oids.is_empty() {
        writeln!(out).ok();
        writeln!(
            out,
            "Sample decoded symbol oids: {}",
            sample_decoded_symbol_oids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
        .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::PidPackage;
    use std::collections::BTreeMap;

    fn package(doc: PidDocument) -> PidPackage {
        PidPackage::new(None, BTreeMap::new(), doc)
    }

    #[test]
    fn report_view_matches_public_package_report_byte_for_byte() {
        let pkg = package(PidDocument::default());
        let command = InspectCommand::Views(
            InspectRequest::new(InspectOutputFormat::Text).with_view(InspectView::Report),
        );

        let outcome = run(&pkg, &command).expect("render report");

        assert_eq!(
            outcome.stdout,
            crate::inspect::report::generate_package_report(&pkg)
        );
        assert!(outcome.stderr.is_empty());
        assert_eq!(outcome.status, InspectStatus::Success);
    }

    #[test]
    fn text_request_combines_coverage_and_byte_audit_in_order() {
        let pkg = package(PidDocument::default());
        let command = InspectCommand::Views(
            InspectRequest::new(InspectOutputFormat::Text)
                .with_view(InspectView::Coverage)
                .with_view(InspectView::ByteAudit),
        );

        let outcome = run(&pkg, &command).expect("render combined views");

        let coverage = outcome.stdout.find("--- Coverage ---").unwrap();
        let audit = outcome.stdout.find("--- Byte Audit ---").unwrap();
        assert!(coverage < audit);
        assert_eq!(outcome.status, InspectStatus::Success);
    }

    #[test]
    fn json_coverage_preserves_legacy_precedence_over_byte_audit() {
        let pkg = package(PidDocument::default());
        let command = InspectCommand::Views(
            InspectRequest::new(InspectOutputFormat::Json)
                .with_view(InspectView::ByteAudit)
                .with_view(InspectView::Coverage),
        );

        let outcome = run(&pkg, &command).expect("render coverage JSON");
        let value: serde_json::Value = serde_json::from_str(&outcome.stdout).expect("valid JSON");

        assert!(value.get("entries").is_some());
        assert!(value.get("per_stream").is_none());
    }

    #[test]
    fn byte_audit_regression_returns_findings_without_process_exit() {
        let pkg = package(PidDocument::default());
        let mut baseline = crate::byte_audit_report(&pkg);
        baseline.overall_coverage_ratio = 1.0;
        let command = InspectCommand::Views(
            InspectRequest::new(InspectOutputFormat::Text)
                .with_view(InspectView::ByteAudit)
                .with_byte_audit_baseline(baseline),
        );

        let outcome = run(&pkg, &command).expect("render comparison");

        assert_eq!(outcome.status, InspectStatus::Findings);
        assert!(outcome.stdout.contains("Regressions: 1"));
        assert!(outcome.stdout.contains("overall_coverage_decreased"));
    }

    #[test]
    fn geometry_summary_is_returned_as_owned_output() {
        let pkg = package(PidDocument::default());

        let outcome = run(&pkg, &InspectCommand::GeometrySummary).expect("render geometry summary");

        assert!(outcome
            .stdout
            .starts_with("=== Sheet stream geometry summary ===\n"));
        assert!(outcome.stdout.contains("Total entities: 0\n"));
        assert!(outcome.stderr.is_empty());
    }
}
