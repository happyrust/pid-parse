//! `pid_inspect` command surface, lifted into the library (wayfinder B).
//!
//! [`InspectCommand`] declares the full flag surface the binary knows —
//! one enum variant per command family, per the shape pinned in #17 —
//! and [`InspectCommand::run`] executes it, returning an
//! [`InspectOutput`] the caller renders as text (stdout), diagnostics
//! (stderr) or JSON. Commands split into two families:
//!
//! - **Read family** — consume an already-parsed [`PidPackage`] and are
//!   pure: `run` performs no filesystem I/O.
//! - **Writer family** — carry their own output/input paths and do
//!   file I/O inside `run` (round-trip writes, `--set-*` edits,
//!   `--export-bundle`, `--diff`, `--controlled-diff-dir`).
//!
//! [`InspectCommand::needs_package`] tells the caller whether `run`
//! requires a parsed package: [`InspectCommand::Schema`] and
//! [`InspectCommand::ControlledDiffDir`] are input-free.
//!
//! The binary stays a thin adapter: parse args → build the command →
//! `run` → render. Observable CLI behaviour is preserved end-to-end:
//! stdout text, stderr diagnostics and exit codes (0/1/2/3) all match
//! the pre-B3 binary — the text renderers below are the same bodies
//! that used to live as `print_*` helpers in `src/bin/pid_inspect.rs`.

use crate::byte_audit::{
    compare_byte_audit_reports, ByteAuditComparison, ByteAuditImprovementKind,
    ByteAuditRegressionKind, ByteAuditReport,
};
use crate::error::PidError;
use crate::geometry::{build_normalized_geometry, NormalizedPidGeometry};
use crate::model::{CoverageReport, ParseCoverageStatus, PidDocument};
use crate::package::{diff_packages, PackageDiff, PidPackage};
use crate::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions, SheetProbeReport};
use crate::PidParser;
use serde::Serialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::controlled_diff::{
    ControlledDiffCaseReport, ControlledDiffEvidenceReport, ControlledDiffMetadata,
};

/// One `pid_inspect` invocation: the flag the user asked for plus its
/// payload. Variants group by family exactly like the CLI usage text —
/// read-side commands that consume a parsed [`PidPackage`] first, then
/// the writer/diff family that takes filesystem paths.
///
/// Every variant is executed through [`InspectCommand::run`]; the
/// `pid_inspect` binary builds the enum and renders the result.
#[derive(Debug, Clone)]
pub enum InspectCommand {
    /// Default report — `generate_package_report` output when no other
    /// flag is given.
    Report,
    /// `--json` whole-document dump (`PidDocument` serialized pretty).
    JsonDocument,
    /// `--schema` — JSON Schema for `PidDocument`. Input-free: `run`
    /// ignores the package argument.
    Schema,
    /// `--geometry-json` / `--geometry-summary` — one payload
    /// ([`NormalizedPidGeometry`]), two renderings: JSON or the text
    /// summary table.
    Geometry,
    /// `--probe-cluster` — cluster inventory probe (text only).
    ProbeCluster,
    /// `--probe-dynamic` — dynamic-attributes probe (text only).
    ProbeDynamic,
    /// `--probe-sheet` — per-sheet stream probe (text only).
    ProbeSheet,
    /// `--probe-sheet-chunks [Sheet<N>]` — raw chunk probe that works on
    /// the package's raw streams.
    ProbeSheetChunks {
        /// Optional `Sheet<N>` target; `None` probes every sheet.
        target: Option<String>,
    },
    /// `--probe-relationships` — object-graph relationships probe.
    ProbeRelationships,
    /// `--probe-endpoints` — endpoint inventory probe.
    ProbeEndpoints,
    /// `--crossref` — cross-reference coverage report.
    Crossref,
    /// `--graph-mermaid` — object graph as mermaid.
    GraphMermaid,
    /// `--crossref-mermaid` — cross-reference graph as mermaid.
    CrossrefMermaid,
    /// `--coverage` — per-stream decode coverage (`CoverageReport`
    /// payload; text or JSON rendering chosen by the caller).
    Coverage,
    /// `--byte-audit [--byte-audit-baseline <audit.json>]` — raw-byte
    /// audit of the package, optionally compared against a baseline file.
    ByteAudit {
        /// Baseline report file to compare against, if any.
        baseline: Option<PathBuf>,
    },
    /// `--diff <other.pid>` — structural diff between two `.pid` files.
    /// `run` parses `other` internally and diffs it against the package.
    Diff {
        /// Path to the second `.pid` file.
        other: PathBuf,
    },
    /// `--controlled-diff-dir <dir>` — batch controlled-diff cases from
    /// a directory. Input-free: `run` ignores the package argument and
    /// parses each case's own `before/`/`after/` `.pid` files.
    ControlledDiffDir {
        /// Case directory root.
        dir: PathBuf,
    },
    /// `--round-trip <output.pid> [--verify]` — re-serialize the package
    /// to a new CFB.
    RoundTrip {
        /// Destination `.pid` path.
        output: PathBuf,
        /// Re-parse the written file and diff against the source.
        verify: bool,
    },
    /// `--export-bundle <dir>` plus its publish sub-options.
    ExportBundle(ExportBundleOptions),
    /// `--set-drawing-number <NEW> --output <out.pid>` — write a copy
    /// with the drawing number replaced.
    SetDrawingNumber {
        /// Replacement drawing number.
        new_number: String,
        /// Destination `.pid` path.
        output: PathBuf,
    },
    /// `--set-xml-tag <stream> <tag> <value> --output <out.pid>` — write
    /// a copy with one XML tag edited inside a stream.
    SetXmlTag {
        /// Stream path inside the package.
        stream: String,
        /// XML tag to set.
        tag: String,
        /// New tag value.
        value: String,
        /// Destination `.pid` path.
        output: PathBuf,
    },
}

/// Options carried by [`InspectCommand::ExportBundle`], mirroring the
/// bin's former `ExportBundleCliOptions` flag set.
#[derive(Debug, Clone, Default)]
pub struct ExportBundleOptions {
    /// `--export-bundle <dir>` — extraction output directory.
    pub output_dir: PathBuf,
    /// `--export-bundle-raw-streams` — also dump raw stream bytes.
    pub include_raw_streams: bool,
    /// `--export-bundle-publish <Export.mdf|Export_v2.sqlite>`.
    pub publish_input: Option<PathBuf>,
    /// `--publish-drawing <UID>`.
    pub publish_drawing: Option<String>,
    /// `--publish-plant <NAME>`.
    pub publish_plant: Option<String>,
    /// `--publish-style a01|dwg`.
    pub publish_style: Option<String>,
    /// `--publish-diff-against <Data.xml>`.
    pub publish_diff_against: Option<PathBuf>,
}

/// Structured outcome of [`InspectCommand::Diff`]: the package diff
/// plus the two labels the diagnostics refer to (`A:` / `B:` lines).
#[derive(Debug, Clone, Serialize)]
pub struct DiffOutcome {
    /// `diff_packages(pkg_a, pkg_b)` result.
    pub diff: PackageDiff,
    /// Display label for the input package (its `source_path`, or a
    /// placeholder for in-memory packages).
    pub a_label: String,
    /// Display label for the second package (the `other` path).
    pub b_label: String,
}

/// Structured outcome of [`InspectCommand::ControlledDiffDir`]. The
/// [`ControlledDiffEvidenceReport`] is the JSON payload (serializes
/// bare, preserving the CLI's `--json` shape); `metadata_paths` rides
/// along for the text renderer, which prints each case's metadata file
/// for human triage.
#[derive(Debug)]
pub struct ControlledDiffOutcome {
    /// Aggregate evidence report (one case entry per processed pair).
    pub evidence: ControlledDiffEvidenceReport,
    /// Per-case `metadata/<name>.json` path, aligned with
    /// `evidence.cases` order — the CLI-only provenance the text
    /// renderer shows.
    pub metadata_paths: Vec<PathBuf>,
    /// Total `before/after` pairs discovered in the directory — the
    /// number the `Cases: N` header reported even when processing
    /// truncated early.
    pub total_cases: usize,
    /// `false` when no `before/*.pid` + `after/*.pid` pairs were found.
    /// The text renderer emits nothing and the diagnostic carries the
    /// "no pairs found" note instead.
    pub cases_found: bool,
    /// `true` when rendering stopped mid-way because a case produced
    /// zero stream-level changes (the corresponding
    /// [`InspectError::exit_code`] is 3).
    pub truncated: bool,
}

/// Structured outcome of [`InspectCommand::RoundTrip`].
#[derive(Debug, Clone, Serialize)]
pub struct RoundTripOutcome {
    /// Display label for the input package.
    pub source: String,
    /// Destination `.pid` path that was written.
    pub output: PathBuf,
    /// Number of streams written into the new CFB.
    pub streams_written: usize,
    /// Root CLSID carried over from the source, if any.
    pub root_clsid: Option<crate::Uuid>,
    /// `true` when `--verify` re-parsed the written file.
    pub verify: bool,
    /// `diff_packages` result of the verification pass; `Some` whenever
    /// `verify` ran (including the "0 diffs" success case).
    pub verify_diff: Option<PackageDiff>,
}

/// Structured outcome of [`InspectCommand::ExportBundle`].
#[derive(Debug, Clone, Serialize)]
pub struct ExportBundleOutcome {
    /// Display label for the input package.
    pub source: String,
    /// Bundle output directory that was written.
    pub output_dir: PathBuf,
    /// Number of streams indexed into the manifest.
    pub streams_indexed: usize,
    /// Whether raw stream bytes were dumped alongside the manifest.
    pub include_raw_streams: bool,
    /// Whether publish XML generation ran.
    pub publish_xml: bool,
}

/// Structured outcome of [`InspectCommand::SetDrawingNumber`] and
/// [`InspectCommand::SetXmlTag`].
#[derive(Debug, Clone, Serialize)]
pub struct XmlTagEditOutcome {
    /// The CLI verb that produced this (`"set-drawing-number"` or
    /// `"set-xml-tag"`) — selects the diagnostic line format.
    pub operation: String,
    /// Display label for the input package.
    pub source: String,
    /// Stream that was edited.
    pub stream: String,
    /// Tag whose text was replaced.
    pub tag: String,
    /// Pre-edit tag text.
    pub old: String,
    /// New tag text.
    pub value: String,
    /// Destination `.pid` path that was written.
    pub output: PathBuf,
}

/// The payload a finished [`InspectCommand`] produced. Serializing a
/// value emits exactly the payload's own JSON (the `--coverage --json`
/// output stays the bare `CoverageReport`, `--controlled-diff-dir
/// --json` the bare [`ControlledDiffEvidenceReport`]) — rendering into
/// an enum wrapper never changes the CLI's JSON shape.
///
/// Three renderings per variant:
///
/// - [`InspectOutput::render_text`] — stdout text.
/// - [`InspectOutput::render_diag`] — stderr diagnostics (the
///   `... ok: a -> b` bookkeeping lines); empty for quiet commands.
/// - [`InspectOutput::exit_code`] — the CLI exit status the output
///   maps to (non-zero for byte-audit regressions, non-empty diffs and
///   failed round-trip verification).
#[derive(Debug)]
pub enum InspectOutput {
    /// Free-form text produced by the command (the package report, the
    /// schema / document JSON dumps, the text probes, mermaid renders —
    /// anything inherently textual).
    Text(String),
    /// `--coverage` payload.
    Coverage(CoverageReport),
    /// `--geometry-*` payload.
    Geometry(NormalizedPidGeometry),
    /// `--probe-sheet-chunks` payload — serialized `SheetProbeReport`s.
    SheetChunks {
        /// Probe reports, one per matching `Sheet*` stream.
        reports: Vec<SheetProbeReport>,
        /// The `Sheet<N>` filter that was applied, for the
        /// "no matching stream" diagnostic.
        target: Option<String>,
    },
    /// `--byte-audit` payload: the audit report plus the baseline
    /// comparison when one was requested.
    ByteAudit {
        /// `byte_audit_report(pkg)` result.
        report: Box<ByteAuditReport>,
        /// Baseline comparison, present when `--byte-audit-baseline`
        /// was given.
        comparison: Option<Box<ByteAuditComparison>>,
    },
    /// `--diff` payload.
    Diff(Box<DiffOutcome>),
    /// `--controlled-diff-dir` payload.
    ControlledDiff(Box<ControlledDiffOutcome>),
    /// `--round-trip` payload.
    RoundTrip(Box<RoundTripOutcome>),
    /// `--export-bundle` payload.
    ExportBundle(Box<ExportBundleOutcome>),
    /// `--set-drawing-number` / `--set-xml-tag` payload.
    XmlTagEdit(Box<XmlTagEditOutcome>),
}

/// Failure from [`InspectCommand::run`]. Carries everything the CLI
/// needs to stay byte-faithful: the stderr message, the exit code the
/// original binary used (1 I/O-parse / 2 usage / 3 clean-check
/// violation), and an optional partial output for failures that occur
/// *after* some stdout payload was already produced (the
/// controlled-diff "zero stream diffs" case prints the offending case
/// report before exiting 3).
#[derive(Debug)]
pub struct InspectError {
    /// Exit code the CLI should pass to `std::process::exit`.
    pub exit_code: i32,
    /// Human-facing message for stderr (already prefixed, e.g.
    /// `"Parse error: ..."` — callers print it verbatim).
    pub message: String,
    /// Partial output to render before exiting, if any.
    pub partial: Option<Box<InspectOutput>>,
}

impl InspectError {
    fn new(exit_code: i32, message: impl Into<String>) -> Self {
        Self {
            exit_code,
            message: message.into(),
            partial: None,
        }
    }

    /// Exit 1 — parse / I/O / serialization failures.
    fn failure(message: impl Into<String>) -> Self {
        Self::new(1, message)
    }

    /// Exit 2 — flag/metadata validation failures.
    fn usage(message: impl Into<String>) -> Self {
        Self::new(2, message)
    }

    /// Exit 3 — clean-check violations (controlled-diff zero-change
    /// case). `partial` carries the stdout emitted so far.
    fn clean_violation(message: impl Into<String>, partial: InspectOutput) -> Self {
        Self {
            exit_code: 3,
            message: message.into(),
            partial: Some(Box::new(partial)),
        }
    }
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InspectError {}

impl From<PidError> for InspectError {
    fn from(e: PidError) -> Self {
        Self::failure(e.to_string())
    }
}

impl InspectCommand {
    /// Whether `run` needs a parsed [`PidPackage`]. `false` only for
    /// the input-free commands ([`Self::Schema`],
    /// [`Self::ControlledDiffDir`]); the binary skips parsing for
    /// those, preserving the flag's behaviour on unparseable inputs.
    pub fn needs_package(&self) -> bool {
        !matches!(self, Self::Schema | Self::ControlledDiffDir { .. })
    }

    /// The label the CLI used when the *input* `.pid` fails to parse
    /// before this command can run: `--diff` distinguished the two
    /// files as `A`/`B` (`"Parse A error: ..."`), everything else
    /// printed the plain `"Parse error: ..."`. Callers prepend this
    /// to form the stderr line.
    pub fn parse_error_label(&self) -> &'static str {
        match self {
            Self::Diff { .. } => "Parse A",
            _ => "Parse",
        }
    }

    /// Execute the command. `pkg` must be `Some` for every variant
    /// except [`Self::Schema`] and [`Self::ControlledDiffDir`] (see
    /// [`Self::needs_package`]).
    ///
    /// Read-family variants are pure; writer-family variants perform
    /// their file I/O inside this call.
    pub fn run(&self, pkg: Option<&PidPackage>) -> Result<InspectOutput, InspectError> {
        match self {
            Self::Report => {
                let pkg = require_pkg(pkg)?;
                Ok(InspectOutput::Text(super::report::generate_package_report(
                    pkg,
                )))
            }
            Self::JsonDocument => {
                let pkg = require_pkg(pkg)?;
                // `render_text` is the exact stdout payload — the CLI
                // printed the JSON via `println!`, so the trailing
                // newline folds into the text.
                let json = serde_json::to_string_pretty(&pkg.parsed)
                    .map_err(|e| InspectError::failure(format!("JSON serialization error: {e}")))?;
                Ok(InspectOutput::Text(format!("{json}\n")))
            }
            Self::Schema => Ok(InspectOutput::Text(format!(
                "{}\n",
                crate::schema::pid_document_schema_pretty().map_err(|e| {
                    InspectError::failure(format!("Schema serialization error: {e}"))
                })?
            ))),
            Self::Geometry => {
                let pkg = require_pkg(pkg)?;
                Ok(InspectOutput::Geometry(build_normalized_geometry(
                    &pkg.parsed,
                )))
            }
            Self::ProbeCluster => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_probe_cluster(doc)))
            }
            Self::ProbeDynamic => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_probe_dynamic(doc)))
            }
            Self::ProbeSheet => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_probe_sheet(doc)))
            }
            Self::ProbeSheetChunks { target } => {
                let pkg = require_pkg(pkg)?;
                let opts = SheetProbeOptions::default();
                let mut reports = Vec::new();
                for stream in pkg.streams.values() {
                    let Some(leaf) = stream.path.rsplit('/').next() else {
                        continue;
                    };
                    if !leaf.starts_with("Sheet") {
                        continue;
                    }
                    if let Some(t) = target {
                        if leaf != t {
                            continue;
                        }
                    }
                    reports.push(probe_sheet_stream(leaf, &stream.path, &stream.data, &opts));
                }
                Ok(InspectOutput::SheetChunks {
                    reports,
                    target: target.clone(),
                })
            }
            Self::ProbeRelationships => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_probe_relationships(doc)))
            }
            Self::ProbeEndpoints => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_probe_endpoints(doc)))
            }
            Self::Crossref => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(render_crossref(doc)))
            }
            Self::GraphMermaid => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(super::mermaid::object_graph_mermaid(
                    doc,
                )))
            }
            Self::CrossrefMermaid => {
                let doc = &require_pkg(pkg)?.parsed;
                Ok(InspectOutput::Text(super::mermaid::crossref_mermaid(doc)))
            }
            Self::Coverage => {
                let pkg = require_pkg(pkg)?;
                Ok(InspectOutput::Coverage(super::coverage::coverage_report(
                    &pkg.parsed,
                )))
            }
            Self::ByteAudit { baseline } => {
                let pkg = require_pkg(pkg)?;
                let report = crate::byte_audit::byte_audit_report(pkg);
                let comparison = match baseline {
                    Some(path) => {
                        let baseline = load_byte_audit_baseline(path)?;
                        Some(Box::new(compare_byte_audit_reports(&baseline, &report)))
                    }
                    None => None,
                };
                Ok(InspectOutput::ByteAudit {
                    report: Box::new(report),
                    comparison,
                })
            }
            Self::Diff { other } => {
                let pkg = require_pkg(pkg)?;
                let b = PidParser::new()
                    .parse_package(other)
                    .map_err(|e| InspectError::failure(format!("Parse B error: {e}")))?;
                let diff = diff_packages(pkg, &b);
                Ok(InspectOutput::Diff(Box::new(DiffOutcome {
                    diff,
                    a_label: package_label(pkg),
                    b_label: other.display().to_string(),
                })))
            }
            Self::ControlledDiffDir { dir } => run_controlled_diff_dir(dir),
            Self::RoundTrip { output, verify } => {
                let pkg = require_pkg(pkg)?;
                crate::writer::PidWriter::write_to(
                    pkg,
                    &crate::writer::WritePlan::default(),
                    output,
                )
                .map_err(|e| InspectError::failure(format!("Write error: {e}")))?;
                let verify_diff = if *verify {
                    let pkg_back = PidParser::new()
                        .parse_package(output)
                        .map_err(|e| InspectError::failure(format!("Verify parse error: {e}")))?;
                    Some(diff_packages(pkg, &pkg_back))
                } else {
                    None
                };
                Ok(InspectOutput::RoundTrip(Box::new(RoundTripOutcome {
                    source: package_label(pkg),
                    output: output.clone(),
                    streams_written: pkg.streams.len(),
                    root_clsid: pkg.root_clsid,
                    verify: *verify,
                    verify_diff,
                })))
            }
            Self::ExportBundle(options) => {
                let pkg = require_pkg(pkg)?;
                export_bundle_outcome(pkg, options)
            }
            Self::SetDrawingNumber { new_number, output } => {
                let pkg = require_pkg(pkg)?;
                let old = perform_xml_tag_write(
                    pkg,
                    "/TaggedTxtData/Drawing",
                    "DrawingNumber",
                    new_number,
                    output,
                )?;
                Ok(InspectOutput::XmlTagEdit(Box::new(XmlTagEditOutcome {
                    operation: "set-drawing-number".to_string(),
                    source: package_label(pkg),
                    stream: String::new(),
                    tag: "DrawingNumber".to_string(),
                    old,
                    value: new_number.clone(),
                    output: output.clone(),
                })))
            }
            Self::SetXmlTag {
                stream,
                tag,
                value,
                output,
            } => {
                let pkg = require_pkg(pkg)?;
                let old = perform_xml_tag_write(pkg, stream, tag, value, output)?;
                Ok(InspectOutput::XmlTagEdit(Box::new(XmlTagEditOutcome {
                    operation: "set-xml-tag".to_string(),
                    source: package_label(pkg),
                    stream: stream.clone(),
                    tag: tag.clone(),
                    old,
                    value: value.clone(),
                    output: output.clone(),
                })))
            }
        }
    }
}

/// Fetch the parsed package or fail — every variant except `Schema`
/// and `ControlledDiffDir` requires one.
fn require_pkg(pkg: Option<&PidPackage>) -> Result<&PidPackage, InspectError> {
    pkg.ok_or_else(|| InspectError::failure("this inspect command requires a parsed .pid package"))
}

/// Display label for the input package: its `source_path` when the
/// package came from a file, a placeholder otherwise.
fn package_label(pkg: &PidPackage) -> String {
    pkg.source_path.as_deref().map_or_else(
        || "(in-memory package)".to_string(),
        |p| p.display().to_string(),
    )
}

impl Serialize for InspectOutput {
    /// Serialize the payload itself (not the variant tag) so the
    /// `--json` rendering matches the pre-B3 CLI byte-for-byte:
    /// `--coverage --json` is the bare `CoverageReport`,
    /// `--controlled-diff-dir --json` the bare evidence report.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => text.serialize(serializer),
            Self::Coverage(report) => report.serialize(serializer),
            Self::Geometry(geometry) => geometry.serialize(serializer),
            Self::SheetChunks { reports, .. } => reports.serialize(serializer),
            Self::ByteAudit {
                comparison: Some(comparison),
                ..
            } => comparison.serialize(serializer),
            Self::ByteAudit { report, .. } => report.serialize(serializer),
            Self::Diff(outcome) => outcome.serialize(serializer),
            Self::ControlledDiff(outcome) => outcome.evidence.serialize(serializer),
            Self::RoundTrip(outcome) => outcome.serialize(serializer),
            Self::ExportBundle(outcome) => outcome.serialize(serializer),
            Self::XmlTagEdit(outcome) => outcome.serialize(serializer),
        }
    }
}

impl InspectOutput {
    /// Human-readable rendering — the same bytes the binary's
    /// `print_*` helpers used to emit on stdout (including trailing
    /// newlines). Empty when the command produces no stdout (the
    /// writer-family commands print only diagnostics).
    pub fn render_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Coverage(report) => render_coverage_text(report),
            Self::Geometry(geometry) => render_geometry_summary_text(geometry),
            Self::SheetChunks { reports, .. } => render_sheet_chunk_reports(reports),
            Self::ByteAudit { report, comparison } => match comparison {
                Some(c) => render_byte_audit_comparison(c),
                None => render_byte_audit_report(report),
            },
            Self::Diff(outcome) => super::diff::render(&outcome.diff),
            Self::ControlledDiff(outcome) => render_controlled_diff(outcome),
            Self::RoundTrip(outcome) => match &outcome.verify_diff {
                Some(diff) if !diff.is_empty() => super::diff::render(diff),
                _ => String::new(),
            },
            Self::ExportBundle(_) | Self::XmlTagEdit(_) => String::new(),
        }
    }

    /// stderr diagnostics — the `... ok:` bookkeeping lines and
    /// "nothing found" notes the binary used to `eprintln!`. Empty for
    /// commands that stay quiet on stderr.
    pub fn render_diag(&self) -> String {
        match self {
            Self::SheetChunks { reports, target } => {
                if reports.is_empty() {
                    match target {
                        Some(t) => format!("(no Sheet stream named '{t}' found)\n"),
                        None => "(no Sheet* streams found)\n".to_string(),
                    }
                } else {
                    String::new()
                }
            }
            Self::Diff(outcome) => {
                format!("A: {}\nB: {}\n", outcome.a_label, outcome.b_label)
            }
            Self::ControlledDiff(outcome) => {
                if outcome.cases_found {
                    String::new()
                } else {
                    format!(
                        "No controlled PID diff pairs found under {} (expected before/*.pid + after/*.pid)\n",
                        outcome.evidence.root
                    )
                }
            }
            Self::RoundTrip(outcome) => {
                let mut out = String::new();
                writeln!(out, "round-trip ok: {} -> {}", outcome.source, outcome.output.display()).ok();
                writeln!(out, "  streams written: {}", outcome.streams_written).ok();
                match outcome.root_clsid {
                    Some(clsid) => writeln!(out, "  root CLSID preserved: {{{clsid}}}").ok(),
                    None => writeln!(out, "  root CLSID: (none in source)").ok(),
                };
                match &outcome.verify_diff {
                    Some(diff) if diff.is_empty() => {
                        writeln!(out, "  verified: 0 diffs").ok();
                    }
                    Some(diff) => {
                        writeln!(
                            out,
                            "  verification FAILED: {} diff(s) — see report below",
                            diff.diff_count()
                        )
                        .ok();
                    }
                    None => {}
                }
                out
            }
            Self::ExportBundle(outcome) => format!(
                "export-bundle ok: {} -> {}\n  streams indexed: {}\n  raw stream bytes: {}\n  publish XML: {}\n",
                outcome.source,
                outcome.output_dir.display(),
                outcome.streams_indexed,
                outcome.include_raw_streams,
                outcome.publish_xml
            ),
            Self::XmlTagEdit(outcome) => {
                if outcome.operation == "set-drawing-number" {
                    format!(
                        "set-drawing-number ok: DrawingNumber {:?} -> {:?}  ({} -> {})\n",
                        outcome.old,
                        outcome.value,
                        outcome.source,
                        outcome.output.display()
                    )
                } else {
                    format!(
                        "set-xml-tag ok: {} <{}>: {:?} -> {:?}  ({} -> {})\n",
                        outcome.stream,
                        outcome.tag,
                        outcome.old,
                        outcome.value,
                        outcome.source,
                        outcome.output.display()
                    )
                }
            }
            _ => String::new(),
        }
    }

    /// stderr diagnostics as seen in `--json` mode. Identical to
    /// [`Self::render_diag`] for every output except
    /// [`Self::ControlledDiff`]: the "no pairs found" note is the
    /// *substitute* for text output, so the original CLI suppressed it
    /// when the (empty) JSON evidence printed instead.
    pub fn render_diag_json(&self) -> String {
        match self {
            Self::ControlledDiff(_) => String::new(),
            _ => self.render_diag(),
        }
    }

    /// The `--json` body for this output, or `None` when the output has
    /// no JSON representation at all — the `--probe-sheet-chunks` empty
    /// case prints nothing on stdout in either mode (the "not found"
    /// note goes to stderr).
    pub fn json_body(&self) -> Result<Option<String>, serde_json::Error> {
        match self {
            Self::SheetChunks { reports, .. } if reports.is_empty() => Ok(None),
            _ => serde_json::to_string_pretty(self).map(Some),
        }
    }

    /// The CLI exit status this output maps to — non-zero for
    /// byte-audit regressions (3), non-empty package diffs (1) and
    /// failed round-trip verification (1).
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ByteAudit {
                comparison: Some(c),
                ..
            } if !c.is_clean() => 3,
            Self::Diff(outcome) if !outcome.diff.is_empty() => 1,
            Self::RoundTrip(outcome) => match &outcome.verify_diff {
                Some(diff) if !diff.is_empty() => 1,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// JSON rendering — what `--json` mode prints for this command.
    /// Equivalent to `serde_json::to_string_pretty(output)`; kept as a
    /// named method so callers don't need to know the payload-level
    /// serialize convention.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// `--export-bundle` orchestration — feature-gated because
/// [`crate::export_bundle`] only exists under `backup`.
#[cfg(feature = "backup")]
fn export_bundle_outcome(
    pkg: &PidPackage,
    options: &ExportBundleOptions,
) -> Result<InspectOutput, InspectError> {
    use crate::export_bundle::{self, export_bundle, ExportBundlePlan, ExportBundlePublishPlan};

    let mut plan = if options.include_raw_streams {
        ExportBundlePlan::default().with_raw_stream_bytes()
    } else {
        ExportBundlePlan::default()
    };
    if let Some(input_path) = &options.publish_input {
        let drawing_uid = options.publish_drawing.clone().ok_or_else(|| {
            InspectError::usage("--export-bundle-publish requires --publish-drawing")
        })?;
        let mut publish = ExportBundlePublishPlan::new(input_path).with_drawing_uid(drawing_uid);
        if let Some(plant) = &options.publish_plant {
            publish = publish.with_plant(plant.clone());
        }
        if let Some(style) = &options.publish_style {
            publish = publish.with_style(style.clone());
        }
        if let Some(reference) = &options.publish_diff_against {
            publish = publish.with_reference_data_xml(reference.clone());
        }
        plan = plan.with_publish(publish);
    }
    export_bundle(pkg, &plan, &options.output_dir)
        .map_err(|e| InspectError::failure(format!("Export bundle error: {e}")))?;
    if let Some(publish) = plan.publish.as_ref() {
        let publish_dir = options.output_dir.join("publish");
        export_bundle::export_bundle_publish_xml(publish, &publish_dir)
            .map_err(|e| InspectError::failure(format!("Export bundle publish error: {e}")))?;
    }
    Ok(InspectOutput::ExportBundle(Box::new(ExportBundleOutcome {
        source: package_label(pkg),
        output_dir: options.output_dir.clone(),
        streams_indexed: pkg.streams.len(),
        include_raw_streams: options.include_raw_streams,
        publish_xml: plan.publish.is_some(),
    })))
}

/// `--export-bundle` stub for non-`backup` builds — the variant exists
/// so the command surface is stable, but running it needs the feature.
#[cfg(not(feature = "backup"))]
fn export_bundle_outcome(
    _pkg: &PidPackage,
    _options: &ExportBundleOptions,
) -> Result<InspectOutput, InspectError> {
    Err(InspectError::failure(
        "export-bundle requires the `backup` feature",
    ))
}

/// Shared implementation behind `--set-drawing-number` and
/// `--set-xml-tag`: clone the package, rewrite the tag text inside the
/// target stream, and write the result. Returns the pre-edit text so
/// the diagnostic can report what was replaced.
fn perform_xml_tag_write(
    pkg: &PidPackage,
    stream: &str,
    tag: &str,
    value: &str,
    output: &Path,
) -> Result<String, InspectError> {
    let mut pkg = pkg.clone();
    let old = pkg
        .set_xml_tag(stream, tag, value)
        .map_err(|e| InspectError::failure(format!("XML edit failed: {e}")))?;
    crate::writer::PidWriter::write_to(&pkg, &crate::writer::WritePlan::default(), output)
        .map_err(|e| InspectError::failure(format!("Write error: {e}")))?;
    Ok(old)
}

/// `--byte-audit-baseline <file>` loader — reads + parses the baseline
/// audit report; failures are exit-1 like the original CLI path.
fn load_byte_audit_baseline(path: &Path) -> Result<ByteAuditReport, InspectError> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| InspectError::failure(format!("Byte audit baseline read error: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| InspectError::failure(format!("Byte audit baseline JSON parse error: {e}")))
}

/// `--controlled-diff-dir` orchestration. Faithful port of the binary's
/// `run_controlled_diff_dir`: discovers `before/*.pid` + `after/*.pid`
/// pairs under `dir`, loads each `metadata/<name>.json`, diffs the pair
/// and aggregates the evidence report. A case with zero stream-level
/// changes aborts the run with exit 3 *after* emitting its case report
/// (carried as [`InspectError::partial`]).
fn run_controlled_diff_dir(dir: &Path) -> Result<InspectOutput, InspectError> {
    let cases = controlled_pid_diff_cases(dir);
    if cases.is_empty() {
        return Ok(InspectOutput::ControlledDiff(Box::new(
            ControlledDiffOutcome {
                evidence: super::controlled_diff::build_evidence_report(
                    dir.display().to_string(),
                    std::iter::empty(),
                ),
                metadata_paths: Vec::new(),
                total_cases: 0,
                cases_found: false,
                truncated: false,
            },
        )));
    }
    let total_cases = cases.len();

    let parser = PidParser::new();
    let mut case_reports: Vec<ControlledDiffCaseReport> = Vec::with_capacity(cases.len());
    let mut metadata_paths: Vec<PathBuf> = Vec::with_capacity(cases.len());
    for case in &cases {
        let metadata_struct = load_controlled_diff_metadata(case)?;
        let before = parser.parse_package(&case.before_path).map_err(|e| {
            InspectError::failure(format!(
                "Parse before error for {}: {e}",
                case.before_path.display()
            ))
        })?;
        let after = parser.parse_package(&case.after_path).map_err(|e| {
            InspectError::failure(format!(
                "Parse after error for {}: {e}",
                case.after_path.display()
            ))
        })?;

        let inspect_metadata = ControlledDiffMetadata {
            case: case.name.clone(),
            operation: metadata_struct.operation,
            expected: metadata_struct.expected,
            notes: metadata_struct.notes,
        };
        let report = super::controlled_diff::build_case_report(&before, &after, inspect_metadata);
        case_reports.push(report);
        metadata_paths.push(case.metadata_path.clone());

        if case_reports.last().map_or(0, |r| r.stream_diffs) == 0 {
            let name = case.name.clone();
            return Err(InspectError::clean_violation(
                format!(
                    "controlled diff case `{name}` has no stream-level changes; evidence is not usable"
                ),
                InspectOutput::ControlledDiff(Box::new(ControlledDiffOutcome {
                    evidence: ControlledDiffEvidenceReport {
                        root: dir.display().to_string(),
                        cases: case_reports,
                        promoted_geometry: false,
                    },
                    metadata_paths,
                    total_cases,
                    cases_found: true,
                    truncated: true,
                })),
            ));
        }
    }

    Ok(InspectOutput::ControlledDiff(Box::new(
        ControlledDiffOutcome {
            evidence: ControlledDiffEvidenceReport {
                root: dir.display().to_string(),
                cases: case_reports,
                promoted_geometry: false,
            },
            metadata_paths,
            total_cases,
            cases_found: true,
            truncated: false,
        },
    )))
}

/// One `before/<name>.pid` + `after/<name>.pid` + `metadata/<name>.json`
/// triple discovered under a controlled-diff directory.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlledPidDiffCase {
    name: String,
    before_path: PathBuf,
    after_path: PathBuf,
    metadata_path: PathBuf,
}

/// On-disk `metadata/<name>.json` payload — a filesystem concern, kept
/// separate from the library's [`ControlledDiffMetadata`] DTO.
#[derive(Debug, Clone, serde::Deserialize)]
struct ControlledPidDiffMetadata {
    case: String,
    operation: String,
    expected: serde_json::Value,
    #[serde(default)]
    notes: Option<String>,
}

/// Discover controlled-diff pairs under `root`: every `before/*.pid`
/// with a matching `after/*.pid`, sorted by case name. Missing `after`
/// files are skipped with a stderr note — matching the original CLI
/// behaviour, the note surfaces through `eprintln` here because the
/// discovery phase predates the output object.
fn controlled_pid_diff_cases(root: &Path) -> Vec<ControlledPidDiffCase> {
    let before_root = root.join("before");
    let after_root = root.join("after");
    let metadata_root = root.join("metadata");
    let Ok(entries) = std::fs::read_dir(&before_root) else {
        return Vec::new();
    };

    let mut cases = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let before_path = entry.path();
            let extension = before_path.extension().and_then(|value| value.to_str());
            if extension != Some("pid") {
                return None;
            }
            let name = before_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)?;
            let after_path = after_root.join(format!("{name}.pid"));
            if !after_path.exists() {
                eprintln!(
                    "skipping controlled diff case `{name}`: missing after file {}",
                    after_path.display()
                );
                return None;
            }
            Some(ControlledPidDiffCase {
                metadata_path: metadata_root.join(format!("{name}.json")),
                name,
                before_path,
                after_path,
            })
        })
        .collect::<Vec<_>>();
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    cases
}

/// Load + validate `metadata/<name>.json`. Validation failures are
/// exit-2 usage errors, matching the original CLI semantics.
fn load_controlled_diff_metadata(
    case: &ControlledPidDiffCase,
) -> Result<ControlledPidDiffMetadata, InspectError> {
    let metadata = std::fs::read_to_string(&case.metadata_path).map_err(|e| {
        InspectError::usage(format!(
            "Failed to read controlled diff metadata {}: {e}",
            case.metadata_path.display()
        ))
    })?;
    let metadata: ControlledPidDiffMetadata = serde_json::from_str(&metadata).map_err(|e| {
        InspectError::usage(format!(
            "Controlled diff metadata {} must be valid JSON: {e}",
            case.metadata_path.display()
        ))
    })?;
    if metadata.case != case.name {
        return Err(InspectError::usage(format!(
            "Controlled diff metadata {} case field {:?} must match filename stem {:?}",
            case.metadata_path.display(),
            metadata.case,
            case.name
        )));
    }
    if metadata.operation.trim().is_empty() {
        return Err(InspectError::usage(format!(
            "Controlled diff metadata {} must include a non-empty operation",
            case.metadata_path.display()
        )));
    }
    if metadata.expected.is_null() {
        return Err(InspectError::usage(format!(
            "Controlled diff metadata {} must include expected payload",
            case.metadata_path.display()
        )));
    }
    Ok(metadata)
}

// ---------------------------------------------------------------------------
// Text renderers — ported verbatim (println → writeln) from the binary's
// former `print_*` helpers so stdout stays byte-identical.
// ---------------------------------------------------------------------------

/// Text rendering of a [`CoverageReport`] — the binary's former
/// `print_coverage`, byte-identical including the empty-report early
/// exit.
fn render_coverage_text(report: &CoverageReport) -> String {
    let mut out = String::new();
    if report.entries.is_empty() {
        writeln!(out, "--- Coverage ---").ok();
        writeln!(out, "(no top-level entries found; document may be empty)").ok();
        return out;
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
            .map(|f| format!(" -> {f}"))
            .unwrap_or_default();
        let note = entry
            .note
            .as_deref()
            .map(|n| format!("  ({n})"))
            .unwrap_or_default();
        writeln!(out, "  {tag} {}{}{}", entry.name, field, note).ok();
    }
    out
}

/// Text rendering of the `--geometry-summary` view — the binary's
/// former `print_geometry_summary`, byte-identical.
fn render_geometry_summary_text(geometry: &NormalizedPidGeometry) -> String {
    use crate::geometry::{PidGeometryConfidence, PidGraphicKind};

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

    let mut out = String::new();
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
        for t in &sample_decoded_texts {
            writeln!(out, "  {t:?}").ok();
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
    out
}

/// `--byte-audit` text view — the binary's former
/// `print_byte_audit_report`.
fn render_byte_audit_report(report: &ByteAuditReport) -> String {
    let mut out = String::new();
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
    out
}

/// `--byte-audit --byte-audit-baseline` text view — the binary's former
/// `print_byte_audit_comparison`.
fn render_byte_audit_comparison(comparison: &ByteAuditComparison) -> String {
    let mut out = String::new();
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
    out
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

/// `--controlled-diff-dir` text view — the directory header, one case
/// block per processed pair (including the aborting case when
/// `truncated`), and the trailing "no geometry promotion" line only
/// for completed runs.
fn render_controlled_diff(outcome: &ControlledDiffOutcome) -> String {
    let mut out = String::new();
    if !outcome.cases_found {
        return out;
    }
    writeln!(
        out,
        "Controlled PID diff directory: {}",
        outcome.evidence.root
    )
    .ok();
    writeln!(out, "Cases: {}", outcome.total_cases).ok();
    for (report, metadata_path) in outcome
        .evidence
        .cases
        .iter()
        .zip(outcome.metadata_paths.iter())
    {
        render_controlled_diff_case(&mut out, report, metadata_path);
    }
    if !outcome.truncated {
        writeln!(out, "\nNo geometry promotion was performed.").ok();
    }
    out
}

/// One `case=...` block — the binary's former
/// `print_controlled_diff_case`.
fn render_controlled_diff_case(
    out: &mut String,
    report: &ControlledDiffCaseReport,
    metadata_path: &Path,
) {
    let notes = report
        .notes
        .as_deref()
        .map(|value| format!(" notes={value:?}"))
        .unwrap_or_default();
    writeln!(
        out,
        "\ncase={} operation={} metadata={} stream_diffs={} modified_sheet_streams={} only_in_before={} only_in_after={}{}",
        report.case,
        report.operation,
        metadata_path.display(),
        report.stream_diffs,
        report.modified_sheet_streams,
        report.only_in_before,
        report.only_in_after,
        notes
    )
    .ok();
    if let Some(stream) = &report.first_modified {
        writeln!(
            out,
            "first_modified path={} len_before={} len_after={} first_mismatch_offset={}",
            stream.path, stream.len_before, stream.len_after, stream.first_mismatch_offset
        )
        .ok();
        writeln!(out, "before_context {}", stream.before_context).ok();
        writeln!(out, "after_context  {}", stream.after_context).ok();
    }
}

/// `--probe-sheet-chunks` text view — the binary's former
/// `print_sheet_chunk_reports`.
fn render_sheet_chunk_reports(reports: &[SheetProbeReport]) -> String {
    let mut out = String::new();
    if reports.is_empty() {
        return out;
    }
    writeln!(out, "=== Sheet Chunk Probe [EXPERIMENTAL] ===\n").ok();
    for rep in reports {
        writeln!(out, "--- {} ---", rep.sheet_name).ok();
        writeln!(out, "  path: {}", rep.path).ok();
        writeln!(out, "  size: {} bytes (0x{:X})", rep.size, rep.size).ok();
        writeln!(
            out,
            "  boundaries: {}   chunks: {}",
            rep.candidate_boundaries.len(),
            rep.chunks.len()
        )
        .ok();
        if rep.record_type_counts.is_empty() {
            writeln!(out, "  record types: []").ok();
        } else {
            let counts = rep
                .record_type_counts
                .iter()
                .map(|(record_type, count)| format!("{record_type}={count}"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "  record types: {counts}").ok();
        }
        writeln!(out, "  text runs: {}", rep.text_runs.len()).ok();
        writeln!(out, "  coordinate hints: {}", rep.coordinate_hints.len()).ok();
        for (i, chunk) in rep.chunks.iter().enumerate() {
            let ascii = if chunk.ascii_preview.is_empty() {
                String::from("[]")
            } else {
                format!("{:?}", chunk.ascii_preview)
            };
            let utf16 = if chunk.utf16_preview.is_empty() {
                String::from("[]")
            } else {
                format!("{:?}", chunk.utf16_preview)
            };
            writeln!(
                out,
                "  [{:>3}] 0x{:06X}..0x{:06X} len={} kind={:?} zero={:.2} u32_dens={:.2} rep={}",
                i,
                chunk.start,
                chunk.end,
                chunk.len,
                chunk.kind_hint,
                chunk.zero_ratio,
                chunk.aligned_u32_density,
                chunk.repeated_u32_hits,
            )
            .ok();
            if !chunk.ascii_preview.is_empty() {
                writeln!(out, "         ascii: {ascii}").ok();
            }
            if !chunk.utf16_preview.is_empty() {
                writeln!(out, "         utf16: {utf16}").ok();
            }
        }
        writeln!(out).ok();
    }
    out
}

/// `--probe-endpoints` text view — the binary's former
/// `print_probe_endpoints`.
fn render_probe_endpoints(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Relationship Endpoint Resolution ===\n").ok();
    let Some(ref graph) = doc.object_graph else {
        writeln!(out, "(no object graph available)").ok();
        return out;
    };
    if graph.relationships.is_empty() {
        writeln!(out, "(no relationships in graph)").ok();
        return out;
    }

    let fully = graph
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_some() && r.target_drawing_id.is_some())
        .count();
    let partial = graph
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_some() ^ r.target_drawing_id.is_some())
        .count();
    let unresolved = graph.relationships.len() - fully - partial;

    let total_eps: usize = doc
        .sheet_streams
        .iter()
        .map(|s| s.endpoint_records.len())
        .sum();
    writeln!(
        out,
        "relationships = {}   sheet endpoint records = {}",
        graph.relationships.len(),
        total_eps
    )
    .ok();
    writeln!(
        out,
        "resolution   : {fully} fully / {partial} partial / {unresolved} unresolved\n"
    )
    .ok();

    let item_type_by_did: std::collections::HashMap<&str, &str> = graph
        .objects
        .iter()
        .map(|o| (o.drawing_id.as_str(), o.item_type.as_str()))
        .collect();
    let render = |did: Option<&str>| -> String {
        match did {
            Some(d) => {
                let ty = item_type_by_did.get(d).copied().unwrap_or("?");
                format!("{d} [{ty}]")
            }
            None => "(off-drawing)".to_string(),
        }
    };

    for (i, rel) in graph.relationships.iter().enumerate() {
        let id = if rel.guid.is_empty() {
            format!("(template rec={:?})", rel.record_id)
        } else {
            rel.guid.clone()
        };
        let src = render(rel.source_drawing_id.as_deref());
        let tgt = render(rel.target_drawing_id.as_deref());
        writeln!(
            out,
            "[{:>3}]  {}   field_x={:?}\n         {}  ->  {}",
            i, id, rel.field_x, src, tgt
        )
        .ok();
    }
    out
}

/// `--crossref` text view — the binary's former `print_crossref`.
fn render_crossref(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Cross Reference ===\n").ok();
    let Some(ref xr) = doc.cross_reference else {
        writeln!(out, "(no cross-reference graph)").ok();
        return out;
    };

    writeln!(out, "--- Cluster Coverage ---").ok();
    let cov = &xr.cluster_coverage;
    writeln!(out, "  declared: {:?}", cov.declared).ok();
    writeln!(out, "  found:    {:?}", cov.found).ok();
    writeln!(out, "  matched:  {:?}", cov.matched).ok();
    if !cov.declared_missing.is_empty() {
        writeln!(out, "  declared_missing: {:?}", cov.declared_missing).ok();
    }
    if !cov.found_extra.is_empty() {
        writeln!(out, "  found_extra: {:?}", cov.found_extra).ok();
    }

    writeln!(
        out,
        "\n--- Symbol Usage ({} unique) ---",
        xr.symbol_usage.len()
    )
    .ok();
    for u in &xr.symbol_usage {
        writeln!(
            out,
            "  [{}x] {}",
            u.usage_count,
            u.symbol_name
                .clone()
                .unwrap_or_else(|| u.symbol_path.clone())
        )
        .ok();
        writeln!(out, "      path: {}", u.symbol_path).ok();
        writeln!(out, "      jsites: {:?}", u.jsite_names).ok();
    }

    writeln!(
        out,
        "\n--- Attribute Classes ({}) ---",
        xr.attribute_classes.len()
    )
    .ok();
    for c in &xr.attribute_classes {
        writeln!(
            out,
            "  {} (records={}, attr_names={}, drawings={}, models={})",
            c.class_name,
            c.record_count,
            c.unique_attribute_names.len(),
            c.drawing_ids.len(),
            c.model_ids.len()
        )
        .ok();
        if !c.drawing_ids.is_empty() {
            writeln!(out, "    drawings: {:?}", c.drawing_ids).ok();
        }
        if !c.model_ids.is_empty() {
            let preview: Vec<_> = c.model_ids.iter().take(8).cloned().collect();
            writeln!(
                out,
                "    models (first {}): {:?}",
                preview.len().min(c.model_ids.len()),
                preview
            )
            .ok();
        }
        if !c.unique_attribute_names.is_empty() {
            writeln!(out, "    attr names: {:?}", c.unique_attribute_names).ok();
        }
    }

    writeln!(out, "\n--- Root Presence ---").ok();
    for r in &xr.root_presence {
        let where_ = match (r.found_as_storage, r.found_as_stream) {
            (true, _) => "STORAGE",
            (_, true) => "STREAM",
            _ => "MISSING",
        };
        writeln!(out, "  [{}] id=0x{:08X}  {}", where_, r.id, r.name).ok();
    }
    out
}

/// `--probe-cluster` text view — the binary's former
/// `print_probe_cluster`.
fn render_probe_cluster(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Cluster Probe ===\n").ok();
    for c in &doc.clusters {
        writeln!(out, "--- {} ---", c.name).ok();
        writeln!(out, "  path: {}", c.path).ok();
        writeln!(out, "  size: {} bytes (0x{:X})", c.size, c.size).ok();
        if let Some(m) = c.magic_u32_le {
            writeln!(out, "  magic: 0x{m:08X}").ok();
        }
        if let Some(ref hdr) = c.header {
            writeln!(
                out,
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            )
            .ok();
        } else {
            writeln!(out, "  header: (not detected / wrong magic)").ok();
        }
        if let Some(ref pi) = c.probe_info {
            writeln!(
                out,
                "  [PROBE] string_table_offset=0x{:04X} ({} decimal)",
                pi.string_table_offset, pi.string_table_offset
            )
            .ok();
            writeln!(out, "  [PROBE] detection_method={}", pi.detection_method).ok();
            writeln!(out, "  [PROBE] entries_parsed={}", pi.entries_parsed).ok();
            writeln!(
                out,
                "  [PROBE] end_offset=0x{:04X} ({} decimal)",
                pi.end_offset, pi.end_offset
            )
            .ok();
        }
        if let Some(ref table) = c.string_table {
            writeln!(out, "  string_table: {} entries", table.len()).ok();
            for entry in table {
                writeln!(out, "    [{:>4}] \"{}\"", entry.index, entry.value).ok();
            }
        }
        writeln!(out).ok();
    }
    out
}

/// `--probe-sheet` text view — the binary's former `print_probe_sheet`.
fn render_probe_sheet(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Sheet Probe ===\n").ok();
    if doc.sheet_streams.is_empty() {
        writeln!(out, "(no sheet streams found)").ok();
        return out;
    }
    for sh in &doc.sheet_streams {
        writeln!(out, "--- {} ---", sh.name).ok();
        writeln!(out, "  path: {}", sh.path).ok();
        writeln!(out, "  size: {} bytes (0x{:X})", sh.size, sh.size).ok();
        if let Some(m) = sh.magic_u32_le {
            write!(out, "  magic: 0x{m:08X}").ok();
            if let Some(ref tag) = sh.magic_tag {
                write!(out, " '{tag}'").ok();
            }
            writeln!(out).ok();
        }
        if let Some(ref hdr) = sh.header {
            writeln!(
                out,
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            )
            .ok();
        } else {
            writeln!(out, "  header: (not detected / wrong magic)").ok();
        }
        if let Some(ref ps) = sh.probe_summary {
            writeln!(
                out,
                "  [PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            )
            .ok();
            writeln!(out, "  [PROBE] 0x89 markers found: {}", ps.marker_count).ok();
            writeln!(out, "  [PROBE] records extracted: {}", ps.records_extracted).ok();
            writeln!(
                out,
                "  [PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, sh.size
            )
            .ok();
        }
        if !sh.attribute_records.is_empty() {
            writeln!(
                out,
                "\n  records: {} [EXPERIMENTAL/heuristic]",
                sh.attribute_records.len()
            )
            .ok();
            for (i, rec) in sh.attribute_records.iter().enumerate() {
                writeln!(
                    out,
                    "    [{}] class=\"{}\" attrs={} confidence={}",
                    i,
                    rec.class_name,
                    rec.attributes.len(),
                    rec.confidence
                )
                .ok();
                for attr in &rec.attributes {
                    writeln!(out, "         {}: {:?}", attr.name, attr.value).ok();
                }
            }
        }
        if !sh.extracted_texts.is_empty() {
            writeln!(
                out,
                "\n  ASCII preview ({} strings, first 10):",
                sh.extracted_texts.len()
            )
            .ok();
            for t in sh.extracted_texts.iter().take(10) {
                writeln!(out, "    {t}").ok();
            }
        }
        writeln!(out).ok();
    }
    out
}

/// `--probe-relationships` text view — the binary's former
/// `print_probe_relationships`.
fn render_probe_relationships(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Relationship Probe ===\n").ok();
    writeln!(
        out,
        "Scope note: this probe only inspects bytes adjacent to each"
    )
    .ok();
    writeln!(
        out,
        "  Relationship.<GUID> record inside /Unclustered Dynamic"
    )
    .ok();
    writeln!(
        out,
        "  Attributes. Endpoint (source/target) decoding is NOT performed"
    )
    .ok();
    writeln!(
        out,
        "  because the Relationship GUIDs occur nowhere else in the CFB"
    )
    .ok();
    writeln!(out, "  container (neither raw nor Windows GUID layouts).\n").ok();

    let Some(ref da) = doc.dynamic_attributes else {
        writeln!(out, "(no dynamic attributes stream found)").ok();
        return out;
    };
    if da.relationship_probes.is_empty() {
        writeln!(
            out,
            "(no Relationship.<GUID> records detected in the stream)"
        )
        .ok();
        return out;
    }

    writeln!(
        out,
        "probed {} relationship records in {}\n",
        da.relationship_probes.len(),
        da.path
    )
    .ok();
    for (i, p) in da.relationship_probes.iter().enumerate() {
        writeln!(
            out,
            "[{:>3}] guid={} @0x{:06X}  window=[0x{:06X}..0x{:06X})",
            i, p.guid, p.ascii_offset, p.window_start, p.window_end
        )
        .ok();
        if p.nearby_ascii_guids.is_empty() {
            writeln!(out, "      nearby GUIDs: (none)").ok();
        } else {
            for (off, g) in &p.nearby_ascii_guids {
                let annotation = if *g == p.guid { " (this record)" } else { "" };
                writeln!(out, "      nearby GUID @0x{off:06X}  {g}{annotation}").ok();
            }
        }
        if !p.trailing_tokens.is_empty() {
            let summary: Vec<String> = p
                .trailing_tokens
                .iter()
                .map(|t| format!("{}=0x{:04X}@0x{:06X}", t.label, t.value, t.offset))
                .collect();
            writeln!(out, "      trailing tokens: {}", summary.join(", ")).ok();
        }
    }
    out
}

/// `--probe-dynamic` text view — the binary's former
/// `print_probe_dynamic`.
fn render_probe_dynamic(doc: &PidDocument) -> String {
    let mut out = String::new();
    writeln!(out, "=== Dynamic Attributes Probe ===\n").ok();
    if let Some(ref da) = doc.dynamic_attributes {
        writeln!(out, "path: {}", da.path).ok();
        writeln!(out, "size: {} bytes (0x{:X})", da.size, da.size).ok();
        if let Some(m) = da.magic_u32_le {
            writeln!(out, "magic: 0x{m:08X}").ok();
        }
        if let Some(ref hdr) = da.header {
            writeln!(
                out,
                "header: type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            )
            .ok();
        }
        if let Some(ref ps) = da.probe_summary {
            writeln!(
                out,
                "\n[PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            )
            .ok();
            writeln!(out, "[PROBE] 0x89 markers found: {}", ps.marker_count).ok();
            writeln!(out, "[PROBE] records extracted: {}", ps.records_extracted).ok();
            writeln!(
                out,
                "[PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, da.size
            )
            .ok();
        }
        writeln!(
            out,
            "\nrecords: {} [EXPERIMENTAL/heuristic]",
            da.attribute_records.len()
        )
        .ok();
        for (i, rec) in da.attribute_records.iter().enumerate() {
            writeln!(
                out,
                "  [{}] class=\"{}\" attrs={} confidence={}",
                i,
                rec.class_name,
                rec.attributes.len(),
                rec.confidence
            )
            .ok();
            for attr in &rec.attributes {
                writeln!(out, "       {}: {:?}", attr.name, attr.value).ok();
            }
        }
    } else {
        writeln!(out, "(no dynamic attributes stream found)").ok();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use std::collections::BTreeMap;

    fn empty_pkg() -> PidPackage {
        PidPackage::new(None, BTreeMap::new(), PidDocument::default())
    }

    #[test]
    fn report_command_emits_package_report() {
        let out = InspectCommand::Report
            .run(Some(&empty_pkg()))
            .expect("report run");
        let text = out.render_text();
        assert!(
            text.contains("=== PID Document Report ==="),
            "report text should carry the document report header; got:\n{text}"
        );
    }

    #[test]
    fn coverage_command_emits_renderable_report() {
        let out = InspectCommand::Coverage
            .run(Some(&empty_pkg()))
            .expect("coverage run");
        let text = out.render_text();
        assert!(
            text.contains("--- Coverage ---"),
            "coverage text should carry the section header; got:\n{text}"
        );
        let json = out.to_json_pretty().expect("coverage json");
        assert!(
            json.contains("\"entries\""),
            "coverage JSON should be the bare report object; got: {json}"
        );
    }

    #[test]
    fn geometry_command_emits_summary_text_and_json() {
        let out = InspectCommand::Geometry
            .run(Some(&empty_pkg()))
            .expect("geometry run");
        let text = out.render_text();
        assert!(
            text.contains("=== Sheet stream geometry summary ==="),
            "geometry summary should carry its header; got:\n{text}"
        );
        let json = out.to_json_pretty().expect("geometry json");
        assert!(json.starts_with('{'), "geometry JSON should be an object");
    }

    #[test]
    fn schema_command_ignores_the_package() {
        let out = InspectCommand::Schema.run(None).expect("schema run");
        let text = out.render_text();
        assert!(
            text.contains("PidDocument"),
            "schema should mention PidDocument; got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[test]
    fn probe_commands_render_their_sections() {
        let pkg = empty_pkg();
        for (cmd, needle) in [
            (InspectCommand::ProbeCluster, "=== Cluster Probe ==="),
            (
                InspectCommand::ProbeDynamic,
                "=== Dynamic Attributes Probe ===",
            ),
            (InspectCommand::ProbeSheet, "=== Sheet Probe ==="),
            (
                InspectCommand::ProbeRelationships,
                "=== Relationship Probe ===",
            ),
            (
                InspectCommand::ProbeEndpoints,
                "=== Relationship Endpoint Resolution ===",
            ),
            (InspectCommand::Crossref, "=== Cross Reference ==="),
        ] {
            let out = cmd.run(Some(&pkg)).expect("probe run");
            let text = out.render_text();
            assert!(
                text.contains(needle),
                "{cmd:?} should emit `{needle}`; got:\n{text}"
            );
        }
    }

    #[test]
    fn controlled_diff_dir_without_cases_reports_none_found() {
        let dir = std::env::temp_dir().join("pid_inspect_empty_controlled_diff");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let out = InspectCommand::ControlledDiffDir { dir }
            .run(None)
            .expect("controlled diff run");
        assert_eq!(out.exit_code(), 0);
        assert!(out.render_text().is_empty());
        assert!(
            out.render_diag().contains("No controlled PID diff pairs"),
            "diag should carry the no-pairs note; got: {:?}",
            out.render_diag()
        );
    }

    #[test]
    fn missing_package_is_an_error_for_pkg_commands() {
        match InspectCommand::ProbeCluster.run(None) {
            Err(e) => assert_eq!(e.exit_code, 1),
            Ok(_) => panic!("pkg command must reject a missing package"),
        }
    }

    #[test]
    fn needs_package_flags_input_free_commands() {
        assert!(!InspectCommand::Schema.needs_package());
        assert!(!InspectCommand::ControlledDiffDir {
            dir: PathBuf::from("x")
        }
        .needs_package());
        assert!(InspectCommand::Report.needs_package());
        assert!(InspectCommand::Diff {
            other: PathBuf::from("b.pid")
        }
        .needs_package());
    }
}
