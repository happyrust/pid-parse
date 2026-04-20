//! CLI: round-trip a `.pid` file through the `pid-parse` writer and
//! report per-stream byte equality.
//!
//! This is the validation tool for the v0.4.x writer layer's
//! "passthrough preserves every stream verbatim" guarantee — useful for
//! confirming on real Smart P&ID files (which can't be checked into the
//! repo) that `parse_package → PidWriter::write_to → re-parse_package`
//! does not lose or corrupt any stream.
//!
//! Usage:
//!
//! ```text
//! pid_writer_validate <input.pid> [--out <path>] [--keep] [--json]
//!                                  [--quiet] [--max-diff-bytes N]
//! ```
//!
//! Exit codes: 0 = all streams match, 1 = mismatch, 2 = parse / IO failure.

use pid_parse::package::PidPackage;
use pid_parse::writer::{PidWriter, WritePlan};
use pid_parse::PidParser;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MAX_DIFF_BYTES: usize = 16;
const DRAWING_PATH: &str = "/TaggedTxtData/Drawing";
const GENERAL_PATH: &str = "/TaggedTxtData/General";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum EditKind {
    Drawing,
    General,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditOp {
    pub kind: EditKind,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
struct CliOptions {
    input: PathBuf,
    out_spec: OutputSpec,
    json: bool,
    quiet: bool,
    max_diff_bytes: usize,
    edits: Vec<EditOp>,
}

#[derive(Debug, Clone)]
struct OutputSpec {
    path: PathBuf,
    keep: bool,
    user_provided: bool,
}

/// Result of a round-trip + per-stream diff. Public for the integration
/// tests in `tests/writer_validate_cli.rs`.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateReport {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub source_stream_count: usize,
    pub roundtrip_stream_count: usize,
    pub matched: usize,
    pub edited: usize,
    pub mismatched: usize,
    pub only_in_source: Vec<String>,
    pub only_in_roundtrip: Vec<String>,
    pub mismatches: Vec<StreamMismatch>,
    pub edits_applied: Vec<EditOp>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamMismatch {
    pub path: String,
    pub source_len: usize,
    pub roundtrip_len: usize,
    pub first_diff_offset: usize,
    pub source_window: Vec<u8>,
    pub roundtrip_window: Vec<u8>,
}

#[derive(Debug)]
pub enum ValidateError {
    SourceParse(String),
    Edit(String),
    Write(String),
    RoundtripParse(String),
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidateError::SourceParse(e) => write!(f, "failed to parse source: {e}"),
            ValidateError::Edit(e) => write!(f, "failed to apply edit: {e}"),
            ValidateError::Write(e) => write!(f, "failed to write round-trip: {e}"),
            ValidateError::RoundtripParse(e) => write!(f, "failed to re-parse round-trip: {e}"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    let options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("argument error: {e}");
            print_usage();
            std::process::exit(1);
        }
    };

    let report = match run_validate(
        &options.input,
        &options.out_spec.path,
        options.max_diff_bytes,
        &options.edits,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            cleanup_output(&options.out_spec);
            std::process::exit(2);
        }
    };

    if options.json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("JSON serialization error: {e}");
                cleanup_output(&options.out_spec);
                std::process::exit(2);
            }
        }
    } else {
        print_human(&report, options.quiet);
    }

    let exit_code = if report.ok { 0 } else { 1 };
    cleanup_output(&options.out_spec);
    std::process::exit(exit_code);
}

fn print_usage() {
    eprintln!(
        "Usage: pid_writer_validate <input.pid> [--out <path>] [--keep] [--json] [--quiet]\n\
         \x20                       [--max-diff-bytes N] [--edit ATTR=VALUE]+ [--general-edit ELEMENT=VALUE]+"
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let input = PathBuf::from(&args[1]);

    let mut out_path: Option<PathBuf> = None;
    let mut keep = false;
    let mut json = false;
    let mut quiet = false;
    let mut max_diff_bytes = DEFAULT_MAX_DIFF_BYTES;
    let mut edits: Vec<EditOp> = Vec::new();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--out requires a path".to_string())?;
                out_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--keep" => {
                keep = true;
                i += 1;
            }
            "--json" => {
                json = true;
                i += 1;
            }
            "--quiet" => {
                quiet = true;
                i += 1;
            }
            "--max-diff-bytes" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--max-diff-bytes requires a number".to_string())?;
                max_diff_bytes = value
                    .parse()
                    .map_err(|e| format!("--max-diff-bytes parse: {e}"))?;
                i += 2;
            }
            "--edit" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--edit requires ATTR=VALUE".to_string())?;
                edits.push(parse_edit_op(value, EditKind::Drawing)?);
                i += 2;
            }
            "--general-edit" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--general-edit requires ELEMENT=VALUE".to_string())?;
                edits.push(parse_edit_op(value, EditKind::General)?);
                i += 2;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let user_provided = out_path.is_some();
    let path = out_path.unwrap_or_else(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("pid-writer-validate-{pid}-{nanos}.pid"))
    });

    Ok(CliOptions {
        input,
        out_spec: OutputSpec {
            path,
            keep,
            user_provided,
        },
        json,
        quiet,
        max_diff_bytes,
        edits,
    })
}

/// Parse a single `KEY=VALUE` argument into an [`EditOp`]. The first
/// `=` separates key from value; subsequent `=` characters are kept in
/// the value as-is so attribute values containing `=` work.
fn parse_edit_op(raw: &str, kind: EditKind) -> Result<EditOp, String> {
    let (key, value) = raw.split_once('=').ok_or_else(|| {
        format!(
            "{} must be ATTR=VALUE; got `{}`",
            match kind {
                EditKind::Drawing => "--edit",
                EditKind::General => "--general-edit",
            },
            raw
        )
    })?;
    let key = key.trim().to_string();
    let value = value.to_string();
    if key.is_empty() {
        return Err(format!(
            "{} key cannot be empty",
            match kind {
                EditKind::Drawing => "--edit",
                EditKind::General => "--general-edit",
            }
        ));
    }
    Ok(EditOp { kind, key, value })
}

fn cleanup_output(spec: &OutputSpec) {
    if !spec.keep && !spec.user_provided {
        let _ = std::fs::remove_file(&spec.path);
    }
}

/// Round-trip `input` through the writer and produce a per-stream diff
/// report. `edits` are applied to the source package before write-out;
/// the round-trip is then compared against the **edited** package, so
/// untouched streams must match byte-for-byte and edited streams must
/// match the post-edit expectation.
///
/// Public so integration tests can drive the same code path without
/// spawning the binary every time.
pub fn run_validate(
    input: &Path,
    output: &Path,
    max_diff_bytes: usize,
    edits: &[EditOp],
) -> Result<ValidateReport, ValidateError> {
    let parser = PidParser::new();
    let original = parser
        .parse_package(input)
        .map_err(|e| ValidateError::SourceParse(e.to_string()))?;

    let edited = apply_edits_to_package(&original, edits)?;

    PidWriter::write_to(&edited, &WritePlan::default(), output)
        .map_err(|e| ValidateError::Write(e.to_string()))?;

    let roundtrip = parser
        .parse_package(output)
        .map_err(|e| ValidateError::RoundtripParse(e.to_string()))?;

    let report = compare_packages(input, output, &edited, &roundtrip, edits, max_diff_bytes);
    Ok(report)
}

/// Apply a chain of edits to a clone of `package` and return the edited
/// clone. Translates `MetadataEditError` into `ValidateError::Edit` so
/// CLI exits 2 with a clear message.
pub fn apply_edits_to_package(
    package: &PidPackage,
    edits: &[EditOp],
) -> Result<PidPackage, ValidateError> {
    let mut working = package.clone();
    for op in edits {
        let path = match op.kind {
            EditKind::Drawing => DRAWING_PATH,
            EditKind::General => GENERAL_PATH,
        };
        let raw = working.get_stream(path).ok_or_else(|| {
            ValidateError::Edit(format!(
                "source PID is missing {} stream (needed for {:?} edit `{}`)",
                path, op.kind, op.key
            ))
        })?;
        let xml = std::str::from_utf8(&raw.data).map_err(|e| {
            ValidateError::Edit(format!(
                "stream {} is not UTF-8 (BOM/UTF-16 not yet supported): {e}",
                path
            ))
        })?;
        let new_xml = match op.kind {
            EditKind::Drawing => {
                pid_parse::writer::set_drawing_attribute(xml, &op.key, &op.value)
                    .map_err(|e| ValidateError::Edit(format!("--edit {}: {e}", op.key)))?
            }
            EditKind::General => {
                pid_parse::writer::set_element_text(xml, &op.key, &op.value)
                    .map_err(|e| ValidateError::Edit(format!("--general-edit {}: {e}", op.key)))?
            }
        };
        working.replace_stream(path, new_xml.into_bytes());
    }
    Ok(working)
}

fn compare_packages(
    source_path: &Path,
    output_path: &Path,
    expected: &PidPackage,
    roundtrip: &PidPackage,
    edits: &[EditOp],
    max_diff_bytes: usize,
) -> ValidateReport {
    let src_keys: BTreeSet<&String> = expected.streams.keys().collect();
    let dst_keys: BTreeSet<&String> = roundtrip.streams.keys().collect();

    let only_in_source: Vec<String> = src_keys
        .difference(&dst_keys)
        .map(|s| (*s).clone())
        .collect();
    let only_in_roundtrip: Vec<String> = dst_keys
        .difference(&src_keys)
        .map(|s| (*s).clone())
        .collect();

    // A stream is "edited" if it's the target of at least one EditOp.
    let edited_paths: BTreeSet<&str> = edits
        .iter()
        .map(|op| match op.kind {
            EditKind::Drawing => DRAWING_PATH,
            EditKind::General => GENERAL_PATH,
        })
        .collect();

    let mut matched = 0usize;
    let mut edited_count = 0usize;
    let mut mismatches: Vec<StreamMismatch> = Vec::new();
    for path in src_keys.intersection(&dst_keys) {
        let exp = &expected.streams[*path];
        let dst = &roundtrip.streams[*path];
        let was_edited = edited_paths.contains(path.as_str());
        if exp.data == dst.data {
            if was_edited {
                edited_count += 1;
            } else {
                matched += 1;
            }
        } else {
            let first_diff_offset = first_diff_offset(&exp.data, &dst.data);
            let (source_window, roundtrip_window) =
                diff_windows(&exp.data, &dst.data, first_diff_offset, max_diff_bytes);
            mismatches.push(StreamMismatch {
                path: (*path).to_string(),
                source_len: exp.data.len(),
                roundtrip_len: dst.data.len(),
                first_diff_offset,
                source_window,
                roundtrip_window,
            });
        }
    }
    let mismatched = mismatches.len();

    let ok = only_in_source.is_empty()
        && only_in_roundtrip.is_empty()
        && mismatches.is_empty();

    ValidateReport {
        source_path: source_path.to_path_buf(),
        output_path: output_path.to_path_buf(),
        source_stream_count: expected.streams.len(),
        roundtrip_stream_count: roundtrip.streams.len(),
        matched,
        edited: edited_count,
        mismatched,
        only_in_source,
        only_in_roundtrip,
        mismatches,
        edits_applied: edits.to_vec(),
        ok,
    }
}

fn first_diff_offset(a: &[u8], b: &[u8]) -> usize {
    let len = a.len().min(b.len());
    for i in 0..len {
        if a[i] != b[i] {
            return i;
        }
    }
    // Common prefix matches; first divergence is at the shorter length.
    len
}

fn diff_windows(
    src: &[u8],
    dst: &[u8],
    center: usize,
    max_diff_bytes: usize,
) -> (Vec<u8>, Vec<u8>) {
    let half = max_diff_bytes / 2;
    let make = |buf: &[u8]| {
        if buf.is_empty() {
            return Vec::new();
        }
        let start = center.saturating_sub(half);
        let end = (center + max_diff_bytes - half).min(buf.len());
        buf[start..end].to_vec()
    };
    (make(src), make(dst))
}

fn print_human(report: &ValidateReport, quiet: bool) {
    println!(
        "Reading source {} ... {} streams",
        report.source_path.display(),
        report.source_stream_count
    );
    println!(
        "Re-emitted via PidWriter (passthrough) → {}",
        report.output_path.display()
    );
    println!(
        "Re-parsing round-trip ... {} streams",
        report.roundtrip_stream_count
    );

    println!();
    println!("== Stream key set ==");
    println!("Matched keys: {}", report.matched + report.mismatched);
    println!("Only in source: {}", report.only_in_source.len());
    if !quiet {
        for k in &report.only_in_source {
            println!("    - {k}");
        }
    }
    println!("Only in round-trip: {}", report.only_in_roundtrip.len());
    if !quiet {
        for k in &report.only_in_roundtrip {
            println!("    + {k}");
        }
    }

    if !report.edits_applied.is_empty() {
        println!();
        println!("== Edits applied ==");
        for op in &report.edits_applied {
            let kind = match op.kind {
                EditKind::Drawing => "drawing-attr",
                EditKind::General => "general-elem",
            };
            println!("    {kind}  {} = {}", op.key, op.value);
        }
    }

    println!();
    println!("== Per-stream byte equality ==");
    println!(
        "Total: {} matched, {} edited, {} mismatched.",
        report.matched, report.edited, report.mismatched
    );
    for m in &report.mismatches {
        println!(
            "DIFF  {}  source={} B  roundtrip={} B  first diff @offset={}",
            m.path, m.source_len, m.roundtrip_len, m.first_diff_offset
        );
        if !quiet {
            println!("    source window:    {}", hex_pretty(&m.source_window));
            println!("    roundtrip window: {}", hex_pretty(&m.roundtrip_window));
        }
    }

    println!();
    println!(
        "Result: {} (exit {})",
        if report.ok { "PASS" } else { "FAIL" },
        if report.ok { 0 } else { 1 }
    );
}

fn hex_pretty(buf: &[u8]) -> String {
    if buf.is_empty() {
        return "(empty)".to_string();
    }
    buf.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
