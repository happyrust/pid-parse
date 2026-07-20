use pid_parse::inspect::controlled_diff::{
    build_evidence_report, ControlledDiffCaseReport, ControlledDiffEvidenceReport,
    ControlledDiffMetadata, ControlledDiffStreamReport,
};
use pid_parse::{
    export_bundle_publish_xml, ExportBundlePlan, ExportBundlePublishPlan, PidParser, PidWriter,
    WritePlan,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: pid_inspect <file.pid> [--json] [--schema]\n                    [--geometry-json] [--geometry-summary]\n                    [--probe-cluster] [--probe-dynamic] [--probe-sheet]\n                    [--probe-sheet-chunks [Sheet<N>]]\n                    [--probe-relationships] [--probe-endpoints]\n                    [--crossref] [--graph-mermaid] [--crossref-mermaid]\n                    [--coverage] [--byte-audit [--byte-audit-baseline <audit.json>]]\n                    [--round-trip <output.pid> [--verify]]\n                    [--export-bundle <dir> [--export-bundle-raw-streams]\n                     [--export-bundle-publish <Export.mdf|Export_v2.sqlite>]\n                     [--publish-drawing <UID>] [--publish-plant <NAME>]\n                     [--publish-style a01|dwg]\n                     [--publish-diff-against <Data.xml>]]\n                    [--set-drawing-number <NEW> --output <output.pid>]\n                    [--set-xml-tag <stream> <tag> <value> --output <output.pid>]\n                    [--diff <other.pid>]\n                    [--controlled-diff-dir <dir>]"
        );
        std::process::exit(1);
    }

    let path = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");
    let schema_mode = args.iter().any(|a| a == "--schema");
    let geometry_json = args.iter().any(|a| a == "--geometry-json");
    let geometry_summary = args.iter().any(|a| a == "--geometry-summary");
    let probe_cluster = args.iter().any(|a| a == "--probe-cluster");
    let probe_dynamic = args.iter().any(|a| a == "--probe-dynamic");
    let probe_sheet = args.iter().any(|a| a == "--probe-sheet");
    let (probe_sheet_chunks, chunk_target) = parse_probe_sheet_chunks(&args);
    let probe_relationships = args.iter().any(|a| a == "--probe-relationships");
    let probe_endpoints = args.iter().any(|a| a == "--probe-endpoints");
    let crossref = args.iter().any(|a| a == "--crossref");
    let graph_mermaid = args.iter().any(|a| a == "--graph-mermaid");
    let crossref_mermaid = args.iter().any(|a| a == "--crossref-mermaid");
    let coverage_flag = args.iter().any(|a| a == "--coverage");
    let byte_audit = args.iter().any(|a| a == "--byte-audit");
    let byte_audit_baseline = flag_value(&args, "--byte-audit-baseline");

    let round_trip = flag_value(&args, "--round-trip");
    let export_bundle_dir = flag_value(&args, "--export-bundle");
    let export_bundle_raw_streams = args.iter().any(|a| a == "--export-bundle-raw-streams");
    let export_bundle_publish = flag_value(&args, "--export-bundle-publish");
    let publish_drawing = flag_value(&args, "--publish-drawing");
    let publish_plant = flag_value(&args, "--publish-plant");
    let publish_style = flag_value(&args, "--publish-style");
    let publish_diff_against = flag_value(&args, "--publish-diff-against");
    let set_drawing_number = flag_value(&args, "--set-drawing-number");
    let set_xml_tag_args = flag_triple(&args, "--set-xml-tag");
    let output = flag_value(&args, "--output");
    let diff_other = flag_value(&args, "--diff");
    let controlled_diff_dir = flag_value(&args, "--controlled-diff-dir");
    let verify = args.iter().any(|a| a == "--verify");
    let has_publish_bundle_options = export_bundle_publish.is_some()
        || publish_drawing.is_some()
        || publish_plant.is_some()
        || publish_style.is_some()
        || publish_diff_against.is_some();

    if export_bundle_dir.is_none() {
        if export_bundle_raw_streams {
            eprintln!("--export-bundle-raw-streams requires --export-bundle <dir>");
            std::process::exit(2);
        }
        if has_publish_bundle_options {
            eprintln!("publish bundle flags require --export-bundle <dir>");
            std::process::exit(2);
        }
    }
    if export_bundle_publish.is_none()
        && (publish_drawing.is_some()
            || publish_plant.is_some()
            || publish_style.is_some()
            || publish_diff_against.is_some())
    {
        eprintln!("publish option flags require --export-bundle-publish <input>");
        std::process::exit(2);
    }
    if export_bundle_publish.is_some() && publish_drawing.is_none() {
        eprintln!("--export-bundle-publish requires --publish-drawing <UID>");
        std::process::exit(2);
    }

    // Writer / diff modes are handled up-front because they don't print
    // the standard report and always exit after completing.
    if let Some(dir) = controlled_diff_dir {
        run_controlled_diff_dir(&dir, json_mode);
        return;
    }
    if let Some(other) = diff_other {
        run_diff(path, &other);
        return;
    }
    if let Some(out) = round_trip {
        run_round_trip(path, &out, verify);
        return;
    }
    if let Some(out_dir) = export_bundle_dir {
        run_export_bundle(
            path,
            ExportBundleCliOptions {
                output_dir: out_dir,
                include_raw_streams: export_bundle_raw_streams,
                publish_input: export_bundle_publish,
                publish_drawing,
                publish_plant,
                publish_style,
                publish_diff_against,
            },
        );
        return;
    }
    if let Some(new_number) = set_drawing_number {
        let Some(out) = output.clone() else {
            eprintln!("--set-drawing-number requires --output <file.pid>");
            std::process::exit(2);
        };
        run_set_drawing_number(path, &new_number, &out);
        return;
    }
    if let Some((stream, tag, value)) = set_xml_tag_args {
        let Some(out) = output else {
            eprintln!("--set-xml-tag requires --output <file.pid>");
            std::process::exit(2);
        };
        run_set_xml_tag(path, &stream, &tag, &value, &out);
        return;
    }
    if byte_audit_baseline.is_some() && !byte_audit {
        eprintln!("--byte-audit-baseline requires --byte-audit");
        std::process::exit(2);
    }
    if schema_mode {
        match pid_parse::schema::pid_document_schema_pretty() {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Schema serialization error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if probe_sheet_chunks {
        run_probe_sheet_chunks(path, chunk_target.as_deref(), json_mode);
        return;
    }

    let parser = PidParser::new();
    // Use parse_package so the default report can surface container-level
    // CLSID metadata (root + non-root storages) captured since v0.3.2+.
    let pkg = match parser.parse_package(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };
    let doc = &pkg.parsed;

    if geometry_json {
        emit_inspect_outcome(
            &pkg,
            &pid_parse::inspect::commands::InspectCommand::GeometryJson,
        );
        return;
    }

    if geometry_summary {
        emit_inspect_outcome(
            &pkg,
            &pid_parse::inspect::commands::InspectCommand::GeometrySummary,
        );
        return;
    }

    if json_mode {
        use pid_parse::inspect::commands::{
            InspectCommand, InspectOutputFormat, InspectRequest, InspectView,
        };

        let mut request = InspectRequest::new(InspectOutputFormat::Json);
        if coverage_flag {
            request = request.with_view(InspectView::Coverage);
        }
        if byte_audit {
            request = request.with_view(InspectView::ByteAudit);
            // Coverage historically wins JSON precedence without touching a
            // byte-audit baseline, even when both flags are present.
            if !coverage_flag {
                if let Some(baseline_path) = byte_audit_baseline.as_deref() {
                    request =
                        request.with_byte_audit_baseline(load_byte_audit_baseline(baseline_path));
                }
            }
        }
        emit_inspect_outcome(&pkg, &InspectCommand::Views(request));
        return;
    }

    if probe_cluster {
        print_probe_cluster(doc);
    }

    if probe_dynamic {
        print_probe_dynamic(doc);
    }

    if probe_sheet {
        print_probe_sheet(doc);
    }

    if probe_relationships {
        print_probe_relationships(doc);
    }

    if probe_endpoints {
        print_probe_endpoints(doc);
    }

    if crossref {
        print_crossref(doc);
    }

    use pid_parse::inspect::commands::{
        InspectCommand, InspectOutputFormat, InspectRequest, InspectView,
    };
    let mut request = InspectRequest::new(InspectOutputFormat::Text);
    if graph_mermaid {
        request = request.with_view(InspectView::ObjectGraphMermaid);
    }
    if crossref_mermaid {
        request = request.with_view(InspectView::CrossReferenceMermaid);
    }
    if coverage_flag {
        request = request.with_view(InspectView::Coverage);
    }
    if byte_audit {
        request = request.with_view(InspectView::ByteAudit);
        if let Some(baseline_path) = byte_audit_baseline.as_deref() {
            request = request.with_byte_audit_baseline(load_byte_audit_baseline(baseline_path));
        }
    }

    if !probe_cluster
        && !probe_dynamic
        && !probe_sheet
        && !probe_sheet_chunks
        && !probe_relationships
        && !probe_endpoints
        && !crossref
        && !graph_mermaid
        && !crossref_mermaid
        && !coverage_flag
        && !byte_audit
    {
        request = request.with_view(InspectView::Report);
    }
    emit_inspect_outcome(&pkg, &InspectCommand::Views(request));
}

fn emit_inspect_outcome(
    pkg: &pid_parse::PidPackage,
    command: &pid_parse::inspect::commands::InspectCommand,
) {
    match pid_parse::inspect::commands::run(pkg, command) {
        Ok(outcome) => {
            print!("{}", outcome.stdout);
            eprint!("{}", outcome.stderr);
            if outcome.status == pid_parse::inspect::commands::InspectStatus::Findings {
                std::process::exit(3);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn load_byte_audit_baseline(path: &str) -> pid_parse::byte_audit::ByteAuditReport {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Byte audit baseline read error: {e}");
            std::process::exit(1);
        }
    };
    match serde_json::from_str(&json) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("Byte audit baseline JSON parse error: {e}");
            std::process::exit(1);
        }
    }
}

fn parse_probe_sheet_chunks(args: &[String]) -> (bool, Option<String>) {
    let Some(idx) = args.iter().position(|a| a == "--probe-sheet-chunks") else {
        return (false, None);
    };
    let target = args.get(idx + 1).filter(|a| !a.starts_with("--")).cloned();
    (true, target)
}

/// Extract the value of a `--flag <value>` pair. Returns `None` when the
/// flag is absent; exits with a friendly error when the flag is present
/// but unterminated.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    match args.get(idx + 1) {
        Some(v) if !v.starts_with("--") => Some(v.clone()),
        _ => {
            eprintln!("{flag} requires a value");
            std::process::exit(2);
        }
    }
}

/// Extract three consecutive positional values after `flag`. Used for
/// `--set-xml-tag <stream> <tag> <value>`.
fn flag_triple(args: &[String], flag: &str) -> Option<(String, String, String)> {
    let idx = args.iter().position(|a| a == flag)?;
    let fetch = |offset: usize, label: &str| -> String {
        match args.get(idx + offset) {
            Some(v) if !v.starts_with("--") => v.clone(),
            _ => {
                eprintln!("{flag} requires <{label}> as argument #{offset}");
                std::process::exit(2);
            }
        }
    };
    Some((fetch(1, "stream"), fetch(2, "tag"), fetch(3, "value")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlledPidDiffCase {
    name: String,
    before_path: PathBuf,
    after_path: PathBuf,
    metadata_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct ControlledPidDiffMetadata {
    case: String,
    operation: String,
    expected: serde_json::Value,
    #[serde(default)]
    notes: Option<String>,
}

// The DTO + evidence builder live in `pid_parse::inspect::controlled_diff`.
// Filesystem types remain local (`ControlledPidDiffCase` /
// `ControlledPidDiffMetadata`) so disk I/O stays out of the library.

fn controlled_pid_diff_cases(root: &Path) -> Vec<ControlledPidDiffCase> {
    let before_root = root.join("before");
    let after_root = root.join("after");
    let metadata_root = root.join("metadata");
    let Ok(entries) = fs::read_dir(&before_root) else {
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

fn run_controlled_diff_dir(root: &str, json_mode: bool) {
    let root_path = Path::new(root);
    let cases = controlled_pid_diff_cases(root_path);
    if cases.is_empty() {
        if json_mode {
            let empty = build_evidence_report(root_path.display().to_string(), std::iter::empty());
            print_controlled_diff_json(&empty);
            return;
        }
        eprintln!(
            "No controlled PID diff pairs found under {} (expected before/*.pid + after/*.pid)",
            root_path.display()
        );
        return;
    }

    let parser = PidParser::new();
    if !json_mode {
        println!("Controlled PID diff directory: {}", root_path.display());
        println!("Cases: {}", cases.len());
    }

    // Parse all packages up front so the inspect-module builder can
    // take borrowed references. We also pre-build the per-case
    // reports here (rather than via `build_evidence_report` on the
    // owned slice) so we can short-circuit the
    // "zero stream-level changes" exit before aggregating the rest.
    let mut case_reports: Vec<ControlledDiffCaseReport> = Vec::with_capacity(cases.len());
    for case in cases {
        let metadata_struct = load_controlled_diff_metadata(&case);
        let before = parser
            .parse_package(&case.before_path)
            .unwrap_or_else(|err| {
                eprintln!(
                    "Parse before error for {}: {err}",
                    case.before_path.display()
                );
                std::process::exit(1);
            });
        let after = parser
            .parse_package(&case.after_path)
            .unwrap_or_else(|err| {
                eprintln!("Parse after error for {}: {err}", case.after_path.display());
                std::process::exit(1);
            });

        let inspect_metadata = ControlledDiffMetadata {
            case: case.name.clone(),
            operation: metadata_struct.operation,
            expected: metadata_struct.expected,
            notes: metadata_struct.notes,
        };
        let report = pid_parse::inspect::controlled_diff::build_case_report(
            &before,
            &after,
            inspect_metadata,
        );
        if !json_mode {
            print_controlled_diff_case(&report, &case.metadata_path);
        }
        if report.stream_diffs == 0 {
            eprintln!(
                "controlled diff case `{}` has no stream-level changes; evidence is not usable",
                report.case
            );
            std::process::exit(3);
        }
        case_reports.push(report);
    }
    let evidence = ControlledDiffEvidenceReport {
        root: root_path.display().to_string(),
        cases: case_reports,
        promoted_geometry: false,
    };
    if json_mode {
        print_controlled_diff_json(&evidence);
    } else {
        println!("\nNo geometry promotion was performed.");
    }
}

/// Render a per-case report on stdout. The `metadata_path` is a
/// CLI-only concern (the inspect-module DTO is filesystem-free), so
/// the renderer is the one that prints it for human triage.
fn print_controlled_diff_case(report: &ControlledDiffCaseReport, metadata_path: &Path) {
    let notes = report
        .notes
        .as_deref()
        .map(|value| format!(" notes={value:?}"))
        .unwrap_or_default();
    println!(
        "\ncase={} operation={} metadata={} stream_diffs={} modified_sheet_streams={} only_in_before={} only_in_after={}{}",
        report.case,
        report.operation,
        metadata_path.display(),
        report.stream_diffs,
        report.modified_sheet_streams,
        report.only_in_before,
        report.only_in_after,
        notes
    );
    if let Some(stream) = &report.first_modified {
        print_controlled_diff_first_modified(stream);
    }
}

fn print_controlled_diff_first_modified(stream: &ControlledDiffStreamReport) {
    println!(
        "first_modified path={} len_before={} len_after={} first_mismatch_offset={}",
        stream.path, stream.len_before, stream.len_after, stream.first_mismatch_offset
    );
    println!("before_context {}", stream.before_context);
    println!("after_context  {}", stream.after_context);
}

fn print_controlled_diff_json(report: &ControlledDiffEvidenceReport) {
    match serde_json::to_string_pretty(report) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Controlled diff JSON serialization error: {e}");
            std::process::exit(1);
        }
    }
}

fn load_controlled_diff_metadata(case: &ControlledPidDiffCase) -> ControlledPidDiffMetadata {
    let metadata = fs::read_to_string(&case.metadata_path).unwrap_or_else(|err| {
        eprintln!(
            "Failed to read controlled diff metadata {}: {err}",
            case.metadata_path.display()
        );
        std::process::exit(2);
    });
    let metadata: ControlledPidDiffMetadata =
        serde_json::from_str(&metadata).unwrap_or_else(|err| {
            eprintln!(
                "Controlled diff metadata {} must be valid JSON: {err}",
                case.metadata_path.display()
            );
            std::process::exit(2);
        });
    if metadata.case != case.name {
        eprintln!(
            "Controlled diff metadata {} case field {:?} must match filename stem {:?}",
            case.metadata_path.display(),
            metadata.case,
            case.name
        );
        std::process::exit(2);
    }
    if metadata.operation.trim().is_empty() {
        eprintln!(
            "Controlled diff metadata {} must include a non-empty operation",
            case.metadata_path.display()
        );
        std::process::exit(2);
    }
    if metadata.expected.is_null() {
        eprintln!(
            "Controlled diff metadata {} must include expected payload",
            case.metadata_path.display()
        );
        std::process::exit(2);
    }
    metadata
}

fn run_probe_sheet_chunks(path: &str, target: Option<&str>, json_mode: bool) {
    use pid_parse::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions};

    let parser = PidParser::new();
    let package = match parser.parse_package(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };

    let opts = SheetProbeOptions::default();
    let mut reports = Vec::new();
    for stream in package.streams.values() {
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

    if reports.is_empty() {
        if let Some(t) = target {
            eprintln!("(no Sheet stream named '{t}' found)");
        } else {
            eprintln!("(no Sheet* streams found)");
        }
        return;
    }

    if json_mode {
        match serde_json::to_string_pretty(&reports) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("JSON serialization error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    print_sheet_chunk_reports(&reports);
}

fn print_sheet_chunk_reports(reports: &[pid_parse::parsers::sheet_probe::SheetProbeReport]) {
    println!("=== Sheet Chunk Probe [EXPERIMENTAL] ===\n");
    for rep in reports {
        println!("--- {} ---", rep.sheet_name);
        println!("  path: {}", rep.path);
        println!("  size: {} bytes (0x{:X})", rep.size, rep.size);
        println!(
            "  boundaries: {}   chunks: {}",
            rep.candidate_boundaries.len(),
            rep.chunks.len()
        );
        if rep.record_type_counts.is_empty() {
            println!("  record types: []");
        } else {
            let counts = rep
                .record_type_counts
                .iter()
                .map(|(record_type, count)| format!("{record_type}={count}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  record types: {counts}");
        }
        println!("  text runs: {}", rep.text_runs.len());
        println!("  coordinate hints: {}", rep.coordinate_hints.len());
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
            println!(
                "  [{:>3}] 0x{:06X}..0x{:06X} len={} kind={:?} zero={:.2} u32_dens={:.2} rep={}",
                i,
                chunk.start,
                chunk.end,
                chunk.len,
                chunk.kind_hint,
                chunk.zero_ratio,
                chunk.aligned_u32_density,
                chunk.repeated_u32_hits,
            );
            if !chunk.ascii_preview.is_empty() {
                println!("         ascii: {ascii}");
            }
            if !chunk.utf16_preview.is_empty() {
                println!("         utf16: {utf16}");
            }
        }
        println!();
    }
}

/// Passthrough round-trip: re-serialize the package to a new CFB without
/// any plan changes. Proves the writer pipeline on the full fixture and
/// is useful as a diff baseline. When `verify` is true, the written file
/// is re-parsed and diffed against the source; the run exits with code 1
/// if any diffs are found.
fn run_round_trip(input: &str, output: &str, verify: bool) {
    let parser = PidParser::new();
    let pkg = match parser.parse_package(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new(output)) {
        eprintln!("Write error: {e}");
        std::process::exit(1);
    }
    eprintln!("round-trip ok: {input} -> {output}");
    eprintln!("  streams written: {}", pkg.streams.len());
    if let Some(clsid) = pkg.root_clsid {
        eprintln!("  root CLSID preserved: {{{clsid}}}");
    } else {
        eprintln!("  root CLSID: (none in source)");
    }

    if verify {
        let pkg_back = match parser.parse_package(output) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Verify parse error: {e}");
                std::process::exit(1);
            }
        };
        let diff = pid_parse::diff_packages(&pkg, &pkg_back);
        if diff.is_empty() {
            eprintln!("  verified: 0 diffs");
        } else {
            eprintln!(
                "  verification FAILED: {} diff(s) — see report below",
                diff.diff_count()
            );
            print!("{}", pid_parse::inspect::diff::render(&diff));
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone)]
struct ExportBundleCliOptions {
    output_dir: String,
    include_raw_streams: bool,
    publish_input: Option<String>,
    publish_drawing: Option<String>,
    publish_plant: Option<String>,
    publish_style: Option<String>,
    publish_diff_against: Option<String>,
}

/// Export a parsed `.pid` package to a Phase 32 `.pid.bundle/` directory.
fn run_export_bundle(input: &str, options: ExportBundleCliOptions) {
    let parser = PidParser::new();
    let pkg = match parser.parse_package(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };
    let mut plan = if options.include_raw_streams {
        ExportBundlePlan::default().with_raw_stream_bytes()
    } else {
        ExportBundlePlan::default()
    };
    if let Some(input_path) = options.publish_input {
        let drawing_uid = options
            .publish_drawing
            .expect("validated --export-bundle-publish requires --publish-drawing");
        let mut publish = ExportBundlePublishPlan::new(input_path).with_drawing_uid(drawing_uid);
        if let Some(plant) = options.publish_plant {
            publish = publish.with_plant(plant);
        }
        if let Some(style) = options.publish_style {
            publish = publish.with_style(style);
        }
        if let Some(reference) = options.publish_diff_against {
            publish = publish.with_reference_data_xml(reference);
        }
        plan = plan.with_publish(publish);
    }
    if let Err(e) =
        pid_parse::export_bundle::export_bundle(&pkg, &plan, Path::new(&options.output_dir))
    {
        eprintln!("Export bundle error: {e}");
        std::process::exit(1);
    }
    if let Some(publish) = plan.publish.as_ref() {
        let publish_dir = Path::new(&options.output_dir).join("publish");
        if let Err(e) = export_bundle_publish_xml(publish, &publish_dir) {
            eprintln!("Export bundle publish error: {e}");
            std::process::exit(1);
        }
    }
    eprintln!("export-bundle ok: {input} -> {}", options.output_dir);
    eprintln!("  streams indexed: {}", pkg.streams.len());
    eprintln!("  raw stream bytes: {}", options.include_raw_streams);
    eprintln!("  publish XML: {}", plan.publish.is_some());
}

/// Print a byte-level diff between two `.pid` packages.
fn run_diff(a_path: &str, b_path: &str) {
    let parser = PidParser::new();
    let a = match parser.parse_package(a_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse A error: {e}");
            std::process::exit(1);
        }
    };
    let b = match parser.parse_package(b_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse B error: {e}");
            std::process::exit(1);
        }
    };
    let diff = pid_parse::diff_packages(&a, &b);
    eprintln!("A: {a_path}");
    eprintln!("B: {b_path}");
    print!("{}", pid_parse::inspect::diff::render(&diff));
    // Non-empty diff exits with non-zero to be CI-friendly.
    if !diff.is_empty() {
        std::process::exit(1);
    }
}

/// Rewrite the `<DrawingNumber>` element inside `/TaggedTxtData/Drawing`.
fn run_set_drawing_number(input: &str, new_number: &str, output: &str) {
    let old = perform_xml_tag_write(
        input,
        "/TaggedTxtData/Drawing",
        "DrawingNumber",
        new_number,
        output,
    );
    eprintln!(
        "set-drawing-number ok: DrawingNumber {old:?} -> {new_number:?}  ({input} -> {output})"
    );
}

/// Replace the text of a simple `<tag>...</tag>` element inside the
/// provided `/TaggedTxtData/*` stream and write the result.
fn run_set_xml_tag(input: &str, stream: &str, tag: &str, value: &str, output: &str) {
    let old = perform_xml_tag_write(input, stream, tag, value, output);
    eprintln!("set-xml-tag ok: {stream} <{tag}>: {old:?} -> {value:?}  ({input} -> {output})");
}

/// Shared implementation for `--set-drawing-number` and `--set-xml-tag`.
/// Returns the pre-edit text of the target tag so the caller can log it.
fn perform_xml_tag_write(
    input: &str,
    stream: &str,
    tag: &str,
    value: &str,
    output: &str,
) -> String {
    let parser = PidParser::new();
    let mut pkg = match parser.parse_package(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };
    let old = match pkg.set_xml_tag(stream, tag, value) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("XML edit failed: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new(output)) {
        eprintln!("Write error: {e}");
        std::process::exit(1);
    }
    old
}

fn print_probe_endpoints(doc: &pid_parse::PidDocument) {
    println!("=== Relationship Endpoint Resolution ===\n");
    let Some(ref graph) = doc.object_graph else {
        println!("(no object graph available)");
        return;
    };
    if graph.relationships.is_empty() {
        println!("(no relationships in graph)");
        return;
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
    println!(
        "relationships = {}   sheet endpoint records = {}",
        graph.relationships.len(),
        total_eps
    );
    println!("resolution   : {fully} fully / {partial} partial / {unresolved} unresolved\n");

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
        println!(
            "[{:>3}]  {}   field_x={:?}\n         {}  ->  {}",
            i, id, rel.field_x, src, tgt
        );
    }
}

fn print_crossref(doc: &pid_parse::PidDocument) {
    println!("=== Cross Reference ===\n");
    let Some(ref xr) = doc.cross_reference else {
        println!("(no cross-reference graph)");
        return;
    };

    println!("--- Cluster Coverage ---");
    let cov = &xr.cluster_coverage;
    println!("  declared: {:?}", cov.declared);
    println!("  found:    {:?}", cov.found);
    println!("  matched:  {:?}", cov.matched);
    if !cov.declared_missing.is_empty() {
        println!("  declared_missing: {:?}", cov.declared_missing);
    }
    if !cov.found_extra.is_empty() {
        println!("  found_extra: {:?}", cov.found_extra);
    }

    println!("\n--- Symbol Usage ({} unique) ---", xr.symbol_usage.len());
    for u in &xr.symbol_usage {
        println!(
            "  [{}x] {}",
            u.usage_count,
            u.symbol_name
                .clone()
                .unwrap_or_else(|| u.symbol_path.clone())
        );
        println!("      path: {}", u.symbol_path);
        println!("      jsites: {:?}", u.jsite_names);
    }

    println!(
        "\n--- Attribute Classes ({}) ---",
        xr.attribute_classes.len()
    );
    for c in &xr.attribute_classes {
        println!(
            "  {} (records={}, attr_names={}, drawings={}, models={})",
            c.class_name,
            c.record_count,
            c.unique_attribute_names.len(),
            c.drawing_ids.len(),
            c.model_ids.len()
        );
        if !c.drawing_ids.is_empty() {
            println!("    drawings: {:?}", c.drawing_ids);
        }
        if !c.model_ids.is_empty() {
            let preview: Vec<_> = c.model_ids.iter().take(8).cloned().collect();
            println!(
                "    models (first {}): {:?}",
                preview.len().min(c.model_ids.len()),
                preview
            );
        }
        if !c.unique_attribute_names.is_empty() {
            println!("    attr names: {:?}", c.unique_attribute_names);
        }
    }

    println!("\n--- Root Presence ---");
    for r in &xr.root_presence {
        let where_ = match (r.found_as_storage, r.found_as_stream) {
            (true, _) => "STORAGE",
            (_, true) => "STREAM",
            _ => "MISSING",
        };
        println!("  [{}] id=0x{:08X}  {}", where_, r.id, r.name);
    }
}

fn print_probe_cluster(doc: &pid_parse::PidDocument) {
    println!("=== Cluster Probe ===\n");
    for c in &doc.clusters {
        println!("--- {} ---", c.name);
        println!("  path: {}", c.path);
        println!("  size: {} bytes (0x{:X})", c.size, c.size);
        if let Some(m) = c.magic_u32_le {
            println!("  magic: 0x{m:08X}");
        }
        if let Some(ref hdr) = c.header {
            println!(
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        } else {
            println!("  header: (not detected / wrong magic)");
        }
        if let Some(ref pi) = c.probe_info {
            println!(
                "  [PROBE] string_table_offset=0x{:04X} ({} decimal)",
                pi.string_table_offset, pi.string_table_offset
            );
            println!("  [PROBE] detection_method={}", pi.detection_method);
            println!("  [PROBE] entries_parsed={}", pi.entries_parsed);
            println!(
                "  [PROBE] end_offset=0x{:04X} ({} decimal)",
                pi.end_offset, pi.end_offset
            );
        }
        if let Some(ref table) = c.string_table {
            println!("  string_table: {} entries", table.len());
            for entry in table {
                println!("    [{:>4}] \"{}\"", entry.index, entry.value);
            }
        }
        println!();
    }
}

fn print_probe_sheet(doc: &pid_parse::PidDocument) {
    println!("=== Sheet Probe ===\n");
    if doc.sheet_streams.is_empty() {
        println!("(no sheet streams found)");
        return;
    }
    for sh in &doc.sheet_streams {
        println!("--- {} ---", sh.name);
        println!("  path: {}", sh.path);
        println!("  size: {} bytes (0x{:X})", sh.size, sh.size);
        if let Some(m) = sh.magic_u32_le {
            print!("  magic: 0x{m:08X}");
            if let Some(ref tag) = sh.magic_tag {
                print!(" '{tag}'");
            }
            println!();
        }
        if let Some(ref hdr) = sh.header {
            println!(
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        } else {
            println!("  header: (not detected / wrong magic)");
        }
        if let Some(ref ps) = sh.probe_summary {
            println!(
                "  [PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            );
            println!("  [PROBE] 0x89 markers found: {}", ps.marker_count);
            println!("  [PROBE] records extracted: {}", ps.records_extracted);
            println!(
                "  [PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, sh.size
            );
        }
        if !sh.attribute_records.is_empty() {
            println!(
                "\n  records: {} [EXPERIMENTAL/heuristic]",
                sh.attribute_records.len()
            );
            for (i, rec) in sh.attribute_records.iter().enumerate() {
                println!(
                    "    [{}] class=\"{}\" attrs={} confidence={}",
                    i,
                    rec.class_name,
                    rec.attributes.len(),
                    rec.confidence
                );
                for attr in &rec.attributes {
                    println!("         {}: {:?}", attr.name, attr.value);
                }
            }
        }
        if !sh.extracted_texts.is_empty() {
            println!(
                "\n  ASCII preview ({} strings, first 10):",
                sh.extracted_texts.len()
            );
            for t in sh.extracted_texts.iter().take(10) {
                println!("    {t}");
            }
        }
        println!();
    }
}

fn print_probe_relationships(doc: &pid_parse::PidDocument) {
    println!("=== Relationship Probe ===\n");
    println!("Scope note: this probe only inspects bytes adjacent to each");
    println!("  Relationship.<GUID> record inside /Unclustered Dynamic");
    println!("  Attributes. Endpoint (source/target) decoding is NOT performed");
    println!("  because the Relationship GUIDs occur nowhere else in the CFB");
    println!("  container (neither raw nor Windows GUID layouts).\n");

    let Some(ref da) = doc.dynamic_attributes else {
        println!("(no dynamic attributes stream found)");
        return;
    };
    if da.relationship_probes.is_empty() {
        println!("(no Relationship.<GUID> records detected in the stream)");
        return;
    }

    println!(
        "probed {} relationship records in {}\n",
        da.relationship_probes.len(),
        da.path
    );
    for (i, p) in da.relationship_probes.iter().enumerate() {
        println!(
            "[{:>3}] guid={} @0x{:06X}  window=[0x{:06X}..0x{:06X})",
            i, p.guid, p.ascii_offset, p.window_start, p.window_end
        );
        if p.nearby_ascii_guids.is_empty() {
            println!("      nearby GUIDs: (none)");
        } else {
            for (off, g) in &p.nearby_ascii_guids {
                let annotation = if *g == p.guid { " (this record)" } else { "" };
                println!("      nearby GUID @0x{off:06X}  {g}{annotation}");
            }
        }
        if !p.trailing_tokens.is_empty() {
            let summary: Vec<String> = p
                .trailing_tokens
                .iter()
                .map(|t| format!("{}=0x{:04X}@0x{:06X}", t.label, t.value, t.offset))
                .collect();
            println!("      trailing tokens: {}", summary.join(", "));
        }
    }
}

fn print_probe_dynamic(doc: &pid_parse::PidDocument) {
    println!("=== Dynamic Attributes Probe ===\n");
    if let Some(ref da) = doc.dynamic_attributes {
        println!("path: {}", da.path);
        println!("size: {} bytes (0x{:X})", da.size, da.size);
        if let Some(m) = da.magic_u32_le {
            println!("magic: 0x{m:08X}");
        }
        if let Some(ref hdr) = da.header {
            println!(
                "header: type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        }
        if let Some(ref ps) = da.probe_summary {
            println!(
                "\n[PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            );
            println!("[PROBE] 0x89 markers found: {}", ps.marker_count);
            println!("[PROBE] records extracted: {}", ps.records_extracted);
            println!(
                "[PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, da.size
            );
        }
        println!(
            "\nrecords: {} [EXPERIMENTAL/heuristic]",
            da.attribute_records.len()
        );
        for (i, rec) in da.attribute_records.iter().enumerate() {
            println!(
                "  [{}] class=\"{}\" attrs={} confidence={}",
                i,
                rec.class_name,
                rec.attributes.len(),
                rec.confidence
            );
            for attr in &rec.attributes {
                println!("       {}: {:?}", attr.name, attr.value);
            }
        }
    } else {
        println!("(no dynamic attributes stream found)");
    }
}
