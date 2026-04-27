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
use pid_parse::writer::{apply_plan_to_package, EncodedString, PidWriter, WritePlan};
use pid_parse::PidParser;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
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
    /// Phase 9m: `--set-summary KEY=VALUE` edits, accumulated into the
    /// writer's `MetadataUpdates.summary_updates` map. Empty = no summary
    /// edits requested.
    summary_edits: BTreeMap<String, String>,
    /// Phase 9n: `--delete-summary KEY` deletions, accumulated into the
    /// writer's `MetadataUpdates.summary_deletions` vec. Empty = no
    /// deletions requested.
    summary_deletions: Vec<String>,
    /// Phase 10i: `--set-summary-encoded KEY:ENCODING=VALUE` edits,
    /// accumulated into the writer's
    /// `MetadataUpdates.summary_updates_encoded` map. Empty = no
    /// encoded edits requested.
    summary_edits_encoded: BTreeMap<String, EncodedString>,
    plan_path: Option<PathBuf>,
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
    /// Populated when the round-trip was driven by `--apply-plan`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_applied: Option<WritePlan>,
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
    PlanLoad(String),
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidateError::SourceParse(e) => write!(f, "failed to parse source: {e}"),
            ValidateError::Edit(e) => write!(f, "failed to apply edit: {e}"),
            ValidateError::Write(e) => write!(f, "failed to write round-trip: {e}"),
            ValidateError::RoundtripParse(e) => write!(f, "failed to re-parse round-trip: {e}"),
            ValidateError::PlanLoad(e) => write!(f, "failed to load --apply-plan file: {e}"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(i32::from(args.len() < 2));
    }

    let options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("argument error: {e}");
            print_usage();
            std::process::exit(1);
        }
    };

    let report = if let Some(ref plan_path) = options.plan_path {
        let plan = match load_plan(plan_path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                cleanup_output(&options.out_spec);
                std::process::exit(2);
            }
        };
        match run_validate_with_plan(
            &options.input,
            &options.out_spec.path,
            options.max_diff_bytes,
            &plan,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                cleanup_output(&options.out_spec);
                std::process::exit(2);
            }
        }
    } else {
        match run_validate(
            &options.input,
            &options.out_spec.path,
            options.max_diff_bytes,
            &options.edits,
            &options.summary_edits,
            &options.summary_deletions,
            &options.summary_edits_encoded,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                cleanup_output(&options.out_spec);
                std::process::exit(2);
            }
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

    let exit_code = i32::from(!report.ok);
    cleanup_output(&options.out_spec);
    std::process::exit(exit_code);
}

fn print_usage() {
    eprintln!(
        "Usage: pid_writer_validate <input.pid> [--out <path>] [--keep] [--json] [--quiet]\n\
         \x20                       [--max-diff-bytes N]\n\
         \x20                       [--edit ATTR=VALUE]+ [--general-edit ELEMENT=VALUE]+\n\
         \x20                       [--set-summary KEY=VALUE]+ [--delete-summary KEY]+\n\
         \x20                       [--set-summary-encoded KEY:ENCODING=VALUE]+\n\
         \x20                       [--apply-plan <plan.json>]\n\n\
         --set-summary / --delete-summary edit `/\\x05SummaryInformation` or\n\
         `/\\x05DocumentSummaryInformation` properties by symbolic key (title / author /\n\
         subject / keywords / comments / template / last_author / rev_number / app_name /\n\
         category / manager / company). --set-summary creates or overwrites a property;\n\
         --delete-summary removes one (silent no-op if the property was not present).\n\
         A key cannot appear in both at once.\n\n\
         --set-summary-encoded (Phase 10i) is like --set-summary but encodes VT_LPSTR\n\
         values with an explicit code page via `encoding_rs` label (UTF-8 / windows-1252\n\
         / GBK / Shift_JIS / ...). Lossy inputs fail fast instead of silently emitting\n\
         replacement characters. VT_LPWSTR targets ignore the encoding hint (UTF-16LE is\n\
         unambiguous). A key cannot appear in both --set-summary and --set-summary-encoded.\n\n\
         --apply-plan reads a serialized WritePlan (see pid_parse::writer::plan) and\n\
         applies it in a single pass. Cannot be combined with --edit / --general-edit /\n\
         --set-summary / --delete-summary / --set-summary-encoded."
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
    let mut summary_edits: BTreeMap<String, String> = BTreeMap::new();
    let mut summary_deletions: Vec<String> = Vec::new();
    let mut summary_edits_encoded: BTreeMap<String, EncodedString> = BTreeMap::new();
    let mut plan_path: Option<PathBuf> = None;

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
            "--set-summary" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--set-summary requires KEY=VALUE".to_string())?;
                let (key, val) = value
                    .split_once('=')
                    .ok_or_else(|| format!("--set-summary must be KEY=VALUE; got `{value}`"))?;
                if key.is_empty() {
                    return Err("--set-summary KEY must be non-empty".to_string());
                }
                // Note: we intentionally do NOT validate the key set here —
                // the writer's `apply_summary_updates` maintains the canonical
                // known-keys table and produces a uniform error listing all
                // supported keys. Keeping CLI in sync with lib would be an
                // untracked maintenance cost.
                summary_edits.insert(key.to_string(), val.to_string());
                i += 2;
            }
            "--delete-summary" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--delete-summary requires KEY".to_string())?;
                if value.is_empty() {
                    return Err("--delete-summary KEY must be non-empty".to_string());
                }
                // Key validation is deferred to the writer's
                // `apply_summary_deletions` for the same reason as
                // `--set-summary` above.
                summary_deletions.push(value.clone());
                i += 2;
            }
            "--set-summary-encoded" => {
                // Phase 10i: syntax is `KEY:ENCODING=VALUE`.
                // - first `:` separates KEY from ENCODING
                // - first `=` (after the `:`) separates ENCODING from VALUE
                // - VALUE may contain further `=` or `:` unchanged
                let raw = args.get(i + 1).ok_or_else(|| {
                    "--set-summary-encoded requires KEY:ENCODING=VALUE".to_string()
                })?;
                let (key, rest) = raw.split_once(':').ok_or_else(|| {
                    format!(
                        "--set-summary-encoded must be KEY:ENCODING=VALUE; missing `:`; got `{raw}`"
                    )
                })?;
                let (encoding, val) = rest.split_once('=').ok_or_else(|| {
                    format!(
                        "--set-summary-encoded must be KEY:ENCODING=VALUE; missing `=`; got `{raw}`"
                    )
                })?;
                if key.is_empty() {
                    return Err("--set-summary-encoded KEY must be non-empty".to_string());
                }
                if encoding.is_empty() {
                    return Err("--set-summary-encoded ENCODING must be non-empty".to_string());
                }
                summary_edits_encoded.insert(key.to_string(), EncodedString::new(val, encoding));
                i += 2;
            }
            "--apply-plan" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--apply-plan requires a path to a plan.json".to_string())?;
                plan_path = Some(PathBuf::from(value));
                i += 2;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    if plan_path.is_some()
        && (!edits.is_empty()
            || !summary_edits.is_empty()
            || !summary_deletions.is_empty()
            || !summary_edits_encoded.is_empty())
    {
        return Err(
            "--apply-plan cannot be combined with --edit / --general-edit / --set-summary / --delete-summary / --set-summary-encoded (they describe mutually exclusive edit semantics)"
                .to_string(),
        );
    }

    let user_provided = out_path.is_some();
    let path = out_path.unwrap_or_else(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
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
        summary_edits,
        summary_deletions,
        summary_edits_encoded,
        plan_path,
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
#[allow(clippy::too_many_arguments)]
pub fn run_validate(
    input: &Path,
    output: &Path,
    max_diff_bytes: usize,
    edits: &[EditOp],
    summary_edits: &BTreeMap<String, String>,
    summary_deletions: &[String],
    summary_edits_encoded: &BTreeMap<String, EncodedString>,
) -> Result<ValidateReport, ValidateError> {
    // Phase 9n / 10i: mirror the lib-layer conflict checks so the CLI
    // surfaces a clean single error rather than crashing mid-write.
    for del_key in summary_deletions {
        if summary_edits.contains_key(del_key) {
            return Err(ValidateError::Edit(format!(
                "--set-summary and --delete-summary both target key '{del_key}'; \
                 at most one must be specified per key"
            )));
        }
        if summary_edits_encoded.contains_key(del_key) {
            return Err(ValidateError::Edit(format!(
                "--set-summary-encoded and --delete-summary both target key '{del_key}'; \
                 at most one must be specified per key"
            )));
        }
    }
    for enc_key in summary_edits_encoded.keys() {
        if summary_edits.contains_key(enc_key) {
            return Err(ValidateError::Edit(format!(
                "--set-summary and --set-summary-encoded both target key '{enc_key}'; \
                 use one or the other, not both"
            )));
        }
    }
    let parser = PidParser::new();
    let original = parser
        .parse_package(input)
        .map_err(|e| ValidateError::SourceParse(e.to_string()))?;

    let mut edited = apply_edits_to_package(&original, edits)?;
    if !summary_deletions.is_empty() {
        pid_parse::writer::summary_write::apply_summary_deletions(&mut edited, summary_deletions)
            .map_err(|e| ValidateError::Edit(format!("--delete-summary: {e}")))?;
    }
    if !summary_edits.is_empty() {
        pid_parse::writer::summary_write::apply_summary_updates(&mut edited, summary_edits)
            .map_err(|e| ValidateError::Edit(format!("--set-summary: {e}")))?;
    }
    if !summary_edits_encoded.is_empty() {
        pid_parse::writer::summary_write::apply_summary_updates_encoded(
            &mut edited,
            summary_edits_encoded,
        )
        .map_err(|e| ValidateError::Edit(format!("--set-summary-encoded: {e}")))?;
    }

    PidWriter::write_to(&edited, &WritePlan::default(), output)
        .map_err(|e| ValidateError::Write(e.to_string()))?;

    let roundtrip = parser
        .parse_package(output)
        .map_err(|e| ValidateError::RoundtripParse(e.to_string()))?;

    let report = compare_packages(
        input,
        output,
        &edited,
        &roundtrip,
        edits,
        summary_edits,
        summary_deletions,
        summary_edits_encoded,
        max_diff_bytes,
    );
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
                "stream {path} is not UTF-8 (BOM/UTF-16 not yet supported): {e}"
            ))
        })?;
        let new_xml = match op.kind {
            EditKind::Drawing => pid_parse::writer::set_drawing_attribute(xml, &op.key, &op.value)
                .map_err(|e| ValidateError::Edit(format!("--edit {}: {e}", op.key)))?,
            EditKind::General => pid_parse::writer::set_element_text(xml, &op.key, &op.value)
                .map_err(|e| ValidateError::Edit(format!("--general-edit {}: {e}", op.key)))?,
        };
        working.replace_stream(path, new_xml.into_bytes());
    }
    Ok(working)
}

// This helper has grown one parameter per Writer-layer edit channel
// (XML edits / summary updates / summary deletions / encoded summary
// updates). Grouping them into a struct would require plumbing a
// `CompareContext` through both `run_validate` and `run_validate_with_plan`,
// without much semantic gain — each parameter is orthogonal. Keep as-is
// and silence clippy.
#[allow(clippy::too_many_arguments)]
fn compare_packages(
    source_path: &Path,
    output_path: &Path,
    expected: &PidPackage,
    roundtrip: &PidPackage,
    edits: &[EditOp],
    summary_edits: &BTreeMap<String, String>,
    summary_deletions: &[String],
    summary_edits_encoded: &BTreeMap<String, EncodedString>,
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

    // A stream is "edited" if it's the target of at least one EditOp or
    // any `--set-summary` update. Phase 9m: we cannot tell statically
    // which of the two summary streams (SummaryInformation /
    // DocumentSummaryInformation) was touched, so conservatively mark
    // both as edited whenever `summary_edits` is non-empty. Streams that
    // were not actually rewritten will still round-trip byte-identically
    // and thus fall into the `matched` bucket via the equality branch
    // below — only ones really touched end up in `edited_count`.
    let mut edited_paths: BTreeSet<&str> = edits
        .iter()
        .map(|op| match op.kind {
            EditKind::Drawing => DRAWING_PATH,
            EditKind::General => GENERAL_PATH,
        })
        .collect();
    if !summary_edits.is_empty()
        || !summary_deletions.is_empty()
        || !summary_edits_encoded.is_empty()
    {
        edited_paths.insert(pid_parse::writer::summary_write::SUMMARY_INFO_PATH);
        edited_paths.insert(pid_parse::writer::summary_write::DOC_SUMMARY_PATH);
    }

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
                path: (*path).clone(),
                source_len: exp.data.len(),
                roundtrip_len: dst.data.len(),
                first_diff_offset,
                source_window,
                roundtrip_window,
            });
        }
    }
    let mismatched = mismatches.len();

    let ok = only_in_source.is_empty() && only_in_roundtrip.is_empty() && mismatches.is_empty();

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
        plan_applied: None,
        ok,
    }
}

/// Load and deserialize a JSON [`WritePlan`] from disk. Errors map to
/// [`ValidateError::PlanLoad`] with a human-readable reason.
pub fn load_plan(path: &Path) -> Result<WritePlan, ValidateError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ValidateError::PlanLoad(format!("reading {}: {e}", path.display())))?;
    serde_json::from_str::<WritePlan>(&content).map_err(|e| {
        ValidateError::PlanLoad(format!("parsing {} as WritePlan JSON: {e}", path.display()))
    })
}

/// Like [`run_validate`] but drives the round-trip from a [`WritePlan`]
/// instead of a list of CLI [`EditOp`]s. The writer applies the plan
/// natively (metadata updates / stream replacements / sheet patches);
/// the verification step diffs the round-trip against the post-plan
/// expected package.
pub fn run_validate_with_plan(
    input: &Path,
    output: &Path,
    max_diff_bytes: usize,
    plan: &WritePlan,
) -> Result<ValidateReport, ValidateError> {
    let parser = PidParser::new();
    let original = parser
        .parse_package(input)
        .map_err(|e| ValidateError::SourceParse(e.to_string()))?;

    // Build the "expected" package through the same helper that writer entry
    // points use, so validator expectations cannot drift from write order.
    let mut expected = original.clone();
    apply_plan_to_package(&mut expected, plan)
        .map_err(|e| ValidateError::Edit(format!("apply_plan: {e}")))?;

    PidWriter::write_to(&original, plan, output)
        .map_err(|e| ValidateError::Write(e.to_string()))?;

    let roundtrip = parser
        .parse_package(output)
        .map_err(|e| ValidateError::RoundtripParse(e.to_string()))?;

    let edited_paths = collect_edited_paths_from_plan(plan);
    let mut report = compare_with_edited_paths(
        input,
        output,
        &expected,
        &roundtrip,
        &edited_paths,
        max_diff_bytes,
    );
    report.plan_applied = Some(plan.clone());
    Ok(report)
}

/// Set of stream paths touched by a [`WritePlan`]. Used by the comparison
/// step to classify each diff as "expected edit" vs "unexpected mismatch".
fn collect_edited_paths_from_plan(plan: &WritePlan) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    if plan.metadata_updates.drawing_xml.is_some() {
        set.insert(DRAWING_PATH.to_string());
    }
    if plan.metadata_updates.general_xml.is_some() {
        set.insert(GENERAL_PATH.to_string());
    }
    if !plan.metadata_updates.summary_updates.is_empty()
        || !plan.metadata_updates.summary_deletions.is_empty()
        || !plan.metadata_updates.summary_updates_encoded.is_empty()
    {
        // Phase 9m/9n + 12b-2 (parser-api-consistency Task 1):
        // summary_updates / summary_deletions / summary_updates_encoded
        // can each target SummaryInformation 或 DocumentSummaryInformation
        // section. We conservatively mark both paths as edited; streams
        // that turn out byte-equal (no real rewrite) still drop into the
        // `matched` bucket via the equality check below.
        set.insert(pid_parse::writer::summary_write::SUMMARY_INFO_PATH.to_string());
        set.insert(pid_parse::writer::summary_write::DOC_SUMMARY_PATH.to_string());
    }
    for repl in &plan.stream_replacements {
        set.insert(repl.path.clone());
    }
    for patch in &plan.sheet_patches {
        set.insert(patch.sheet_path.clone());
    }
    set
}

fn compare_with_edited_paths(
    source_path: &Path,
    output_path: &Path,
    expected: &PidPackage,
    roundtrip: &PidPackage,
    edited_paths: &BTreeSet<String>,
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
                path: (*path).clone(),
                source_len: exp.data.len(),
                roundtrip_len: dst.data.len(),
                first_diff_offset,
                source_window,
                roundtrip_window,
            });
        }
    }
    let mismatched = mismatches.len();

    let ok = only_in_source.is_empty() && only_in_roundtrip.is_empty() && mismatches.is_empty();

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
        edits_applied: Vec::new(),
        plan_applied: None,
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

    if let Some(plan) = &report.plan_applied {
        println!();
        println!("== Plan applied (via --apply-plan) ==");
        if plan.metadata_updates.drawing_xml.is_some() {
            println!("    metadata     /TaggedTxtData/Drawing  (XML body replaced)");
        }
        if plan.metadata_updates.general_xml.is_some() {
            println!("    metadata     /TaggedTxtData/General  (XML body replaced)");
        }
        for repl in &plan.stream_replacements {
            println!(
                "    replacement  {}  ({} B)",
                repl.path,
                repl.new_data.len()
            );
        }
        for patch in &plan.sheet_patches {
            let chunks: usize = patch.chunk_patches.len();
            println!(
                "    sheet-patch  {}  ({} chunk patch{})",
                patch.sheet_path,
                chunks,
                if chunks == 1 { "" } else { "es" }
            );
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
        i32::from(!report.ok)
    );
}

fn hex_pretty(buf: &[u8]) -> String {
    if buf.is_empty() {
        return "(empty)".to_string();
    }
    buf.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
