//! Byte-audit baseline comparison helpers.
//!
//! This module is intentionally data-model only for now: it compares two
//! already generated reports and classifies coverage deltas as regressions
//! or improvements. CLI / CI policy can decide how to render or fail on the
//! resulting comparison.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ByteAuditReport;

/// Result of comparing a committed byte-audit baseline with a
/// freshly generated report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct ByteAuditComparison {
    /// Coverage regressions that should fail CI unless intentionally
    /// accepted by regenerating the baseline.
    pub regressions: Vec<ByteAuditRegression>,
    /// Coverage improvements worth reporting to reviewers. These do
    /// not make the comparison dirty.
    pub improvements: Vec<ByteAuditImprovement>,
}

impl ByteAuditComparison {
    /// True when the comparison found no regressions.
    pub fn is_clean(&self) -> bool {
        self.regressions.is_empty()
    }
}

/// One byte-audit regression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ByteAuditRegression {
    /// Stream path affected by the regression. `None` means package-level
    /// aggregate regression rather than a single stream.
    pub path: Option<String>,
    /// Regression category.
    pub kind: ByteAuditRegressionKind,
    /// Baseline value rendered for diagnostics.
    pub baseline_value: String,
    /// Current value rendered for diagnostics.
    pub current_value: String,
}

/// Categories of byte-audit regression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ByteAuditRegressionKind {
    /// `overall_coverage_ratio` decreased.
    OverallCoverageDecreased,
    /// A stream present in the baseline is missing in the current report.
    StreamMissing,
    /// A stream's `consumed_bytes` decreased.
    StreamConsumedBytesDecreased,
    /// A stream had a registered parser in the baseline but is
    /// unregistered in the current report.
    StreamBecameUnregistered,
}

/// One byte-audit improvement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ByteAuditImprovement {
    /// Stream path affected by the improvement.
    pub path: Option<String>,
    /// Improvement category.
    pub kind: ByteAuditImprovementKind,
    /// Baseline value rendered for diagnostics.
    pub baseline_value: String,
    /// Current value rendered for diagnostics.
    pub current_value: String,
}

/// Categories of byte-audit improvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ByteAuditImprovementKind {
    /// A baseline-unregistered stream now has a registered parser.
    StreamBecameTraced,
    /// A new stream appeared and already has a registered parser.
    NewTracedStream,
}

/// Compare two [`ByteAuditReport`] values and return coverage
/// regressions / improvements.
pub fn compare_byte_audit_reports(
    baseline: &ByteAuditReport,
    current: &ByteAuditReport,
) -> ByteAuditComparison {
    let mut comparison = ByteAuditComparison::default();

    if current.overall_coverage_ratio < baseline.overall_coverage_ratio {
        comparison.regressions.push(ByteAuditRegression {
            path: None,
            kind: ByteAuditRegressionKind::OverallCoverageDecreased,
            baseline_value: format!("{:.6}", baseline.overall_coverage_ratio),
            current_value: format!("{:.6}", current.overall_coverage_ratio),
        });
    }

    for (path, baseline_summary) in &baseline.per_stream {
        let Some(current_summary) = current.per_stream.get(path) else {
            comparison.regressions.push(ByteAuditRegression {
                path: Some(path.clone()),
                kind: ByteAuditRegressionKind::StreamMissing,
                baseline_value: "present".into(),
                current_value: "missing".into(),
            });
            continue;
        };

        if current_summary.consumed_bytes < baseline_summary.consumed_bytes {
            comparison.regressions.push(ByteAuditRegression {
                path: Some(path.clone()),
                kind: ByteAuditRegressionKind::StreamConsumedBytesDecreased,
                baseline_value: baseline_summary.consumed_bytes.to_string(),
                current_value: current_summary.consumed_bytes.to_string(),
            });
        }

        match (
            baseline_summary.parser_name.as_deref(),
            current_summary.parser_name.as_deref(),
        ) {
            (Some(parser), None) => comparison.regressions.push(ByteAuditRegression {
                path: Some(path.clone()),
                kind: ByteAuditRegressionKind::StreamBecameUnregistered,
                baseline_value: parser.to_string(),
                current_value: "unregistered".into(),
            }),
            (None, Some(parser)) => comparison.improvements.push(ByteAuditImprovement {
                path: Some(path.clone()),
                kind: ByteAuditImprovementKind::StreamBecameTraced,
                baseline_value: "unregistered".into(),
                current_value: parser.to_string(),
            }),
            _ => {}
        }
    }

    for (path, current_summary) in &current.per_stream {
        if baseline.per_stream.contains_key(path) {
            continue;
        }
        if let Some(parser) = current_summary.parser_name.as_deref() {
            comparison.improvements.push(ByteAuditImprovement {
                path: Some(path.clone()),
                kind: ByteAuditImprovementKind::NewTracedStream,
                baseline_value: "missing".into(),
                current_value: parser.to_string(),
            });
        }
    }

    comparison
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::byte_audit::{ByteAuditReport, StreamAuditSummary};

    fn summary(path: &str, consumed: u64, total: u64, parser: Option<&str>) -> StreamAuditSummary {
        StreamAuditSummary {
            path: path.to_string(),
            total_bytes: total,
            consumed_bytes: consumed,
            leftover_bytes: total.saturating_sub(consumed),
            coverage_ratio: if total == 0 {
                0.0
            } else {
                consumed as f32 / total as f32
            },
            parser_name: parser.map(str::to_string),
        }
    }

    fn report(entries: Vec<StreamAuditSummary>) -> ByteAuditReport {
        let per_stream: BTreeMap<String, StreamAuditSummary> = entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect();
        let total_file_bytes = per_stream.values().map(|s| s.total_bytes).sum();
        let overall_consumed = per_stream.values().map(|s| s.consumed_bytes).sum();
        let overall_leftover = per_stream.values().map(|s| s.leftover_bytes).sum();
        let overall_coverage_ratio = if total_file_bytes == 0 {
            0.0
        } else {
            overall_consumed as f32 / total_file_bytes as f32
        };
        let unregistered_paths = per_stream
            .values()
            .filter(|s| s.parser_name.is_none())
            .map(|s| s.path.clone())
            .collect();

        ByteAuditReport {
            traces: Vec::new(),
            total_file_bytes,
            overall_consumed,
            overall_leftover,
            overall_coverage_ratio,
            per_stream,
            unregistered_paths,
        }
    }

    #[test]
    fn comparison_flags_overall_coverage_decrease() {
        let baseline = report(vec![summary(
            "/DocVersion3",
            100,
            100,
            Some("parse_doc_version3"),
        )]);
        let current = report(vec![summary(
            "/DocVersion3",
            50,
            100,
            Some("parse_doc_version3"),
        )]);

        let comparison = compare_byte_audit_reports(&baseline, &current);

        assert!(comparison.regressions.iter().any(|r| {
            r.kind == ByteAuditRegressionKind::OverallCoverageDecreased && r.path.is_none()
        }));
        assert!(!comparison.is_clean());
    }

    #[test]
    fn comparison_flags_stream_consumed_bytes_decrease() {
        let baseline = report(vec![summary("/PSMroots", 80, 100, Some("parse_psm_roots"))]);
        let current = report(vec![summary("/PSMroots", 60, 100, Some("parse_psm_roots"))]);

        let comparison = compare_byte_audit_reports(&baseline, &current);

        assert!(comparison.regressions.iter().any(|r| {
            r.kind == ByteAuditRegressionKind::StreamConsumedBytesDecreased
                && r.path.as_deref() == Some("/PSMroots")
        }));
    }

    #[test]
    fn comparison_flags_traced_stream_becoming_unregistered() {
        let baseline = report(vec![summary(
            "/AppObject",
            32,
            32,
            Some("parse_app_object"),
        )]);
        let current = report(vec![summary("/AppObject", 0, 32, None)]);

        let comparison = compare_byte_audit_reports(&baseline, &current);

        assert!(comparison.regressions.iter().any(|r| {
            r.kind == ByteAuditRegressionKind::StreamBecameUnregistered
                && r.path.as_deref() == Some("/AppObject")
        }));
    }

    #[test]
    fn comparison_reports_newly_traced_stream_as_improvement() {
        let baseline = report(vec![summary("/Mystery", 0, 40, None)]);
        let current = report(vec![summary("/Mystery", 40, 40, Some("parse_mystery"))]);

        let comparison = compare_byte_audit_reports(&baseline, &current);

        assert!(comparison.regressions.is_empty());
        assert!(comparison.improvements.iter().any(|i| {
            i.kind == ByteAuditImprovementKind::StreamBecameTraced
                && i.path.as_deref() == Some("/Mystery")
        }));
        assert!(comparison.is_clean());
    }
}
