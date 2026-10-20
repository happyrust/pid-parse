//! `pid_inspect` command surface, lifted into the library (wayfinder B1).
//!
//! [`InspectCommand`] declares the full flag surface the binary knows —
//! one enum variant per command family, per the shape pinned in #17 —
//! and [`InspectCommand::run`] executes it against an already-parsed
//! [`PidPackage`], returning an [`InspectOutput`] the caller renders as
//! text or JSON. B1 wires the three read-side pilots (`Report`,
//! `Coverage`, `Geometry`); the remaining variants are declared so the
//! interface is stable and return `Unsupported` until B3 routes them.
//!
//! The binary stays a thin adapter: parse args → build the command →
//! `run` → render. Observable CLI output (text and `--json`) is
//! unchanged — the text renderers below are the same bodies that used
//! to live as `print_*` helpers in `src/bin/pid_inspect.rs`.

use crate::error::PidError;
use crate::geometry::{build_normalized_geometry, NormalizedPidGeometry};
use crate::model::{CoverageReport, ParseCoverageStatus};
use crate::package::PidPackage;
use serde::Serialize;
use std::fmt::Write;
use std::path::PathBuf;

/// One `pid_inspect` invocation: the flag the user asked for plus its
/// payload. Variants group by family exactly like the CLI usage text —
/// read-side commands that consume a parsed [`PidPackage`] first, then
/// the writer/diff family that takes filesystem paths.
///
/// B1 implements `run` for [`InspectCommand::Report`],
/// [`InspectCommand::Coverage`], [`InspectCommand::Geometry`], and the
/// input-free [`InspectCommand::Schema`]. Every other variant returns
/// [`PidError::Unsupported`] until B3 moves its orchestration out of the
/// binary.
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
        /// Baseline report to compare against, if any.
        baseline: Option<PathBuf>,
    },
    /// `--diff <other.pid>` — structural diff between two `.pid` files.
    /// Writer family: `run` is not the entry point; B3 will route it via
    /// a path-driven method.
    Diff {
        /// Path to the second `.pid` file.
        other: PathBuf,
    },
    /// `--controlled-diff-dir <dir>` — batch controlled-diff cases from
    /// a directory. Takes a directory, not a `.pid`.
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
/// bin's `ExportBundleCliOptions` flag set.
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

/// The payload a finished [`InspectCommand`] produced. Untagged so the
/// JSON rendering of a variant is exactly the payload's own JSON (the
/// `--coverage --json` output stays the bare `CoverageReport`, not an
/// enum-wrapped object). [`InspectOutput::Schema`] carries the schema
/// as already-pretty JSON text; serializing the enum writes it as a
/// JSON string — callers that want the schema document should use
/// [`InspectOutput::to_json_pretty`], which passes the text through.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum InspectOutput {
    /// Free-form text produced by the command (the package report, and
    /// the pretty schema JSON — both are inherently text).
    Text(String),
    /// `--coverage` payload.
    Coverage(CoverageReport),
    /// `--geometry-*` payload.
    Geometry(NormalizedPidGeometry),
}

impl InspectOutput {
    /// Human-readable rendering — the same bytes the binary's
    /// `print_*` helpers used to emit (including the trailing newline).
    pub fn render_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Coverage(report) => render_coverage_text(report),
            Self::Geometry(geometry) => render_geometry_summary_text(geometry),
        }
    }

    /// JSON rendering — what `--json` mode prints for this command.
    /// `Text` payloads pass through verbatim (the schema output is
    /// already JSON; a plain report text becomes a JSON string via
    /// `serde_json` if a caller ever requests it — the CLI never does).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        match self {
            Self::Text(text) => Ok(text.clone()),
            Self::Coverage(report) => serde_json::to_string_pretty(report),
            Self::Geometry(geometry) => serde_json::to_string_pretty(geometry),
        }
    }
}

impl InspectCommand {
    /// Execute a read-side command against an already-parsed package.
    ///
    /// B1 wires `Report` / `Coverage` / `Geometry` / `Schema`; the
    /// remaining read variants and the whole writer family return
    /// [`PidError::Unsupported`] until B3 moves their orchestration out
    /// of `src/bin/pid_inspect.rs`.
    pub fn run(&self, pkg: &PidPackage) -> Result<InspectOutput, PidError> {
        match self {
            Self::Report => Ok(InspectOutput::Text(super::report::generate_package_report(
                pkg,
            ))),
            Self::Coverage => Ok(InspectOutput::Coverage(super::coverage::coverage_report(
                &pkg.parsed,
            ))),
            Self::Geometry => Ok(InspectOutput::Geometry(build_normalized_geometry(
                &pkg.parsed,
            ))),
            Self::Schema => Ok(InspectOutput::Text(
                crate::schema::pid_document_schema_pretty().map_err(|e| {
                    PidError::ParseFailure {
                        context: "pid_document_schema".to_string(),
                        message: e.to_string(),
                    }
                })?,
            )),
            other => Err(PidError::Unsupported(format!(
                "inspect command `{other:?}` is not wired into the library yet (B3)"
            ))),
        }
    }
}

/// Text rendering of a [`CoverageReport`] — the body of the binary's
/// former `print_coverage`, byte-identical including the empty-report
/// early exit.
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

/// Text rendering of the `--geometry-summary` view — the body of the
/// binary's former `print_geometry_summary`, byte-identical.
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
            .run(&empty_pkg())
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
            .run(&empty_pkg())
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
            .run(&empty_pkg())
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
        let out = InspectCommand::Schema
            .run(&empty_pkg())
            .expect("schema run");
        let text = out.render_text();
        assert!(
            text.contains("PidDocument"),
            "schema should mention PidDocument; got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[test]
    fn unwired_variants_return_unsupported() {
        let pkg = empty_pkg();
        for cmd in [
            InspectCommand::ProbeCluster,
            InspectCommand::Crossref,
            InspectCommand::Diff {
                other: PathBuf::from("b.pid"),
            },
        ] {
            match cmd.run(&pkg) {
                Err(PidError::Unsupported(msg)) => {
                    assert!(msg.contains("B3"), "error should name the follow-up: {msg}")
                }
                other => panic!("expected Unsupported, got {other:?}"),
            }
        }
    }
}
