//! `pid_inspect` — thin CLI adapter over [`pid_parse::inspect::commands`].
//!
//! All orchestration lives in the library: this binary only parses the
//! argument vector into [`InspectCommand`]s, runs them against the
//! parsed [`PidPackage`], and prints the rendered output (stdout text,
//! stderr diagnostics, or `--json`). Exit codes are the library's
//! [`InspectOutput::exit_code`] / [`InspectError::exit_code`].

use pid_parse::inspect::commands::{
    ExportBundleOptions, InspectCommand, InspectError, InspectOutput,
};
use pid_parse::PidParser;
use std::path::PathBuf;

const USAGE: &str = "Usage: pid_inspect <file.pid> [--json] [--schema]\n                    [--geometry-json] [--geometry-summary]\n                    [--probe-cluster] [--probe-dynamic] [--probe-sheet]\n                    [--probe-sheet-chunks [Sheet<N>]]\n                    [--probe-relationships] [--probe-endpoints]\n                    [--crossref] [--graph-mermaid] [--crossref-mermaid]\n                    [--coverage] [--byte-audit [--byte-audit-baseline <audit.json>]]\n                    [--round-trip <output.pid> [--verify]]\n                    [--export-bundle <dir> [--export-bundle-raw-streams]\n                     [--export-bundle-publish <Export.mdf|Export_v2.sqlite>]\n                     [--publish-drawing <UID>] [--publish-plant <NAME>]\n                     [--publish-style a01|dwg]\n                     [--publish-diff-against <Data.xml>]]\n                    [--set-drawing-number <NEW> --output <output.pid>]\n                    [--set-xml-tag <stream> <tag> <value> --output <output.pid>]\n                    [--diff <other.pid>]\n                    [--controlled-diff-dir <dir>]";

/// How a command's output should be rendered — `--json` picks
/// [`InspectOutput::to_json_pretty`], everything else the text view.
/// Commands whose payload *is* text (schema / doc JSON dumps, probes)
/// always take `Text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Render {
    Text,
    Json,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        std::process::exit(0);
    }
    if args.len() < 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let path = &args[1];
    let plan = parse_command_plan(path, &args);

    // Only commands that consume a parsed package pay for the parse —
    // `--schema` and `--controlled-diff-dir` run input-free (preserving
    // their behaviour on files that fail to parse).
    let pkg = if plan.iter().any(|(cmd, _)| cmd.needs_package()) {
        match PidParser::new().parse_package(path) {
            Ok(p) => Some(p),
            Err(e) => {
                // `--diff` labelled the input file `A` in its parse
                // error; every other command printed "Parse error".
                let label = plan
                    .iter()
                    .map(|(cmd, _)| cmd.parse_error_label())
                    .find(|l| *l != "Parse")
                    .unwrap_or("Parse");
                eprintln!("{label} error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    for (cmd, render) in &plan {
        let out = match cmd.run(pkg.as_ref()) {
            Ok(out) => out,
            Err(e) => exit_with_error(e, *render),
        };
        // The two mermaid flags used to emit a "nothing to render" note
        // on stderr instead of an empty stdout — preserve that.
        if let InspectOutput::Text(text) = &out {
            if text.is_empty() {
                match cmd {
                    InspectCommand::GraphMermaid => {
                        eprintln!("(no object graph available — nothing to render)");
                    }
                    InspectCommand::CrossrefMermaid => {
                        eprintln!("(no cross-reference graph — nothing to render)");
                    }
                    _ => {}
                }
            }
        }
        emit(&out, *render);
        let code = out.exit_code();
        if code != 0 {
            std::process::exit(code);
        }
    }
}

/// Print one output: stdout text or `--json` body, then the stderr
/// diagnostics when present.
fn emit(out: &InspectOutput, render: Render) {
    match render {
        Render::Text => {
            let text = out.render_text();
            if !text.is_empty() {
                print!("{text}");
            }
        }
        Render::Json => match out.json_body() {
            Ok(Some(json)) => println!("{json}"),
            Ok(None) => {}
            Err(e) => {
                eprintln!("JSON serialization error: {e}");
                std::process::exit(1);
            }
        },
    }
    let diag = match render {
        Render::Text => out.render_diag(),
        Render::Json => out.render_diag_json(),
    };
    if !diag.is_empty() {
        eprint!("{diag}");
    }
}

/// `run` failed: emit the partial stdout payload (if the failure came
/// after output was produced), the message, and the carried exit code.
/// The pre-B3 binary only produced partial stdout in text mode — under
/// `--json` the same failures printed nothing to stdout, so the
/// partial is suppressed there too.
fn exit_with_error(e: InspectError, render: Render) -> ! {
    if let Some(partial) = &e.partial {
        if render == Render::Text {
            emit(partial, Render::Text);
        }
    }
    eprintln!("{}", e.message);
    std::process::exit(e.exit_code);
}

/// Map the argument vector to the ordered command list, preserving the
/// original dispatch precedence exactly: writer/diff early-exit modes
/// first, then `--schema` / `--probe-sheet-chunks`, then the shared-parse
/// read flags (exclusive `--json` modes ahead of additive text flags,
/// default report when nothing matched).
fn parse_command_plan(path: &str, args: &[String]) -> Vec<(InspectCommand, Render)> {
    let _ = path;
    let json_mode = args.iter().any(|a| a == "--json");
    let schema_mode = args.iter().any(|a| a == "--schema");
    let geometry_json = args.iter().any(|a| a == "--geometry-json");
    let geometry_summary = args.iter().any(|a| a == "--geometry-summary");
    let probe_cluster = args.iter().any(|a| a == "--probe-cluster");
    let probe_dynamic = args.iter().any(|a| a == "--probe-dynamic");
    let probe_sheet = args.iter().any(|a| a == "--probe-sheet");
    let (probe_sheet_chunks, chunk_target) = parse_probe_sheet_chunks(args);
    let probe_relationships = args.iter().any(|a| a == "--probe-relationships");
    let probe_endpoints = args.iter().any(|a| a == "--probe-endpoints");
    let crossref = args.iter().any(|a| a == "--crossref");
    let graph_mermaid = args.iter().any(|a| a == "--graph-mermaid");
    let crossref_mermaid = args.iter().any(|a| a == "--crossref-mermaid");
    let coverage_flag = args.iter().any(|a| a == "--coverage");
    let byte_audit = args.iter().any(|a| a == "--byte-audit");
    let byte_audit_baseline = flag_value(args, "--byte-audit-baseline");

    let round_trip = flag_value(args, "--round-trip");
    let export_bundle_dir = flag_value(args, "--export-bundle");
    let export_bundle_raw_streams = args.iter().any(|a| a == "--export-bundle-raw-streams");
    let export_bundle_publish = flag_value(args, "--export-bundle-publish");
    let publish_drawing = flag_value(args, "--publish-drawing");
    let publish_plant = flag_value(args, "--publish-plant");
    let publish_style = flag_value(args, "--publish-style");
    let publish_diff_against = flag_value(args, "--publish-diff-against");
    let set_drawing_number = flag_value(args, "--set-drawing-number");
    let set_xml_tag_args = flag_triple(args, "--set-xml-tag");
    let output = flag_value(args, "--output");
    let diff_other = flag_value(args, "--diff");
    let controlled_diff_dir = flag_value(args, "--controlled-diff-dir");
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

    // Writer / diff modes are exclusive and precede everything else.
    if let Some(dir) = controlled_diff_dir {
        return vec![(
            InspectCommand::ControlledDiffDir {
                dir: PathBuf::from(dir),
            },
            if json_mode {
                Render::Json
            } else {
                Render::Text
            },
        )];
    }
    if let Some(other) = diff_other {
        return vec![(
            InspectCommand::Diff {
                other: PathBuf::from(other),
            },
            Render::Text,
        )];
    }
    if let Some(out) = round_trip {
        return vec![(
            InspectCommand::RoundTrip {
                output: PathBuf::from(out),
                verify,
            },
            Render::Text,
        )];
    }
    if let Some(out_dir) = export_bundle_dir {
        return vec![(
            InspectCommand::ExportBundle(ExportBundleOptions {
                output_dir: PathBuf::from(out_dir),
                include_raw_streams: export_bundle_raw_streams,
                publish_input: export_bundle_publish.map(PathBuf::from),
                publish_drawing,
                publish_plant,
                publish_style,
                publish_diff_against: publish_diff_against.map(PathBuf::from),
            }),
            Render::Text,
        )];
    }
    if let Some(new_number) = set_drawing_number {
        let Some(out) = output.clone() else {
            eprintln!("--set-drawing-number requires --output <file.pid>");
            std::process::exit(2);
        };
        return vec![(
            InspectCommand::SetDrawingNumber {
                new_number,
                output: PathBuf::from(out),
            },
            Render::Text,
        )];
    }
    if let Some((stream, tag, value)) = set_xml_tag_args {
        let Some(out) = output else {
            eprintln!("--set-xml-tag requires --output <file.pid>");
            std::process::exit(2);
        };
        return vec![(
            InspectCommand::SetXmlTag {
                stream,
                tag,
                value,
                output: PathBuf::from(out),
            },
            Render::Text,
        )];
    }
    if byte_audit_baseline.is_some() && !byte_audit {
        eprintln!("--byte-audit-baseline requires --byte-audit");
        std::process::exit(2);
    }
    if schema_mode {
        return vec![(InspectCommand::Schema, Render::Text)];
    }
    if probe_sheet_chunks {
        return vec![(
            InspectCommand::ProbeSheetChunks {
                target: chunk_target,
            },
            if json_mode {
                Render::Json
            } else {
                Render::Text
            },
        )];
    }

    // Shared-parse region.
    if geometry_json {
        return vec![(InspectCommand::Geometry, Render::Json)];
    }
    if geometry_summary {
        return vec![(InspectCommand::Geometry, Render::Text)];
    }
    if json_mode {
        // `--json` is exclusive: coverage / byte-audit emit just their
        // payload, everything else falls back to the whole-document
        // dump — probe flags are swallowed entirely.
        if coverage_flag {
            return vec![(InspectCommand::Coverage, Render::Json)];
        }
        if byte_audit {
            return vec![(
                InspectCommand::ByteAudit {
                    baseline: byte_audit_baseline.map(PathBuf::from),
                },
                Render::Json,
            )];
        }
        return vec![(InspectCommand::JsonDocument, Render::Text)];
    }

    // Text mode: probe flags compose in a fixed order; the default
    // report fills in when nothing was selected.
    let mut plan: Vec<(InspectCommand, Render)> = Vec::new();
    if probe_cluster {
        plan.push((InspectCommand::ProbeCluster, Render::Text));
    }
    if probe_dynamic {
        plan.push((InspectCommand::ProbeDynamic, Render::Text));
    }
    if probe_sheet {
        plan.push((InspectCommand::ProbeSheet, Render::Text));
    }
    if probe_relationships {
        plan.push((InspectCommand::ProbeRelationships, Render::Text));
    }
    if probe_endpoints {
        plan.push((InspectCommand::ProbeEndpoints, Render::Text));
    }
    if crossref {
        plan.push((InspectCommand::Crossref, Render::Text));
    }
    if graph_mermaid {
        plan.push((InspectCommand::GraphMermaid, Render::Text));
    }
    if crossref_mermaid {
        plan.push((InspectCommand::CrossrefMermaid, Render::Text));
    }
    if coverage_flag {
        plan.push((InspectCommand::Coverage, Render::Text));
    }
    if byte_audit {
        plan.push((
            InspectCommand::ByteAudit {
                baseline: byte_audit_baseline.map(PathBuf::from),
            },
            Render::Text,
        ));
    }
    if plan.is_empty() {
        plan.push((InspectCommand::Report, Render::Text));
    }
    plan
}

/// `--probe-sheet-chunks` takes an optional `Sheet<N>` positional —
/// peel it off without letting a following flag look like the target.
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
