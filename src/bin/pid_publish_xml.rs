//! CLI: generate `SmartPlant` Publish Data XML from a `SmartPlant` MDF
//! file using the Rust `oxidized-mdf` reader.
//!
//! Stage-1 terminal binary. Given
//!
//! 1. a SQL Server MDF file extracted from `Export.dmp`,
//! 2. a `SmartPlant` drawing UID (the `T_Drawing.SP_ID` value),
//!
//! this tool emits the drawing's `_Data.xml` and, optionally,
//! `_Meta.xml` companion. The binary also exposes the publish
//! fidelity helpers that are now part of the normal workflow:
//! `--style a01|dwg`, `--diff-against <reference.xml>`, and
//! `--list-drawings`.
//!
//! Historical `Export_v2.sqlite` mirrors remain accepted as a
//! legacy compatibility adapter, but MDF is now the only public
//! publish-fidelity baseline.
//!
//! The unresolved work is no longer "can the CLI render business
//! objects?" but rather DWG-mirror-gated fidelity closure:
//! loader canonical-field enrichment and the remaining branch-point
//! writer arms.
//!
//! Usage:
//!
//! ```text
//! pid_publish_xml <mdf> --drawing UID --out <file> [--plant NAME]
//! pid_publish_xml <mdf> --drawing UID --stdout [--plant NAME]
//! ```
//!
//! Exit codes: 0 = wrote document, 1 = I/O / format error, 2 =
//! usage error.

use pid_parse::publish::pid_writer::derived_by_native_rule_count;
use pid_parse::publish::sqlite_load::open_readonly;
use pid_parse::publish::{
    diff_pid_export, diff_publish_xml, diff_rel_defuids, load_drawing_graph, load_publish_from_pid,
    open_mdf_as_sqlite, write_data_xml, write_meta_xml, write_pid_coverage_json,
    write_pid_data_xml, write_pid_meta_xml, PublishError, PublishStyle,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct CliOptions {
    input_path: PathBuf,
    /// Required for the normal "render this drawing" flow;
    /// `None` is only valid when `list_drawings` is true.
    drawing_uid: Option<String>,
    output: Option<OutputTarget>,
    /// Optional `_Meta.xml` companion sink. Only honored when
    /// `output` is a file target — pairing meta output with stdout
    /// would interleave two unrelated XML documents on the same
    /// stream.
    meta_output: Option<PathBuf>,
    /// `.pid` input only: the directory the delivery goes to, as
    /// `<DocName>_Data.xml`, `<DocName>_Meta.xml` and the coverage
    /// report `<DocName>_Coverage.json`. Mutually exclusive with
    /// `--out` / `--stdout` / `--meta-out`.
    out_dir: Option<PathBuf>,
    /// Optional reference `_Data.xml` to diff the generated output
    /// against. When set, the binary prints a [`SemanticDiffReport`]
    /// to stdout and exits 0 if no differences (clean) or 1 if any
    /// missing / extra / count-delta rows are surfaced.
    ///
    /// Compatible with `--out` and `--stdout` (those still write
    /// the generated XML before the diff report) and with no
    /// output flag at all (in which case only the diff is printed).
    diff_against: Option<PathBuf>,
    plant_name: String,
    /// `SmartPlant` project flavor selector (A29b). Picks between
    /// the A01 and DWG `IObject` attribute conventions on the
    /// `PIDPipeline` / `PIDPipingConnector` / `PIDProcessVessel`
    /// blocks. Defaults to A01 to keep every pre-A29b CLI
    /// invocation byte-identical.
    style: PublishStyle,
    /// A30 — when true, the CLI prints every `T_Drawing` row in
    /// the input table set and exits 0 without rendering anything.
    /// Mutually exclusive with `--drawing` / `--out` / `--stdout`
    /// / `--diff-against` because there is no per-drawing
    /// output to produce.
    list_drawings: bool,
    verbose: bool,
}

#[derive(Debug, Clone)]
enum OutputTarget {
    Stdout,
    File(PathBuf),
}

fn print_usage() {
    eprintln!(
        "Usage: pid_publish_xml <mdf> --drawing UID [--out FILE | --stdout]\n\
         \x20               [--meta-out FILE] [--diff-against FILE] [--plant NAME]\n\
         \x20               [--style a01|dwg]\n\
         \x20  pid_publish_xml <mdf> --list-drawings\n\
         \x20  pid_publish_xml <file.pid> [--out FILE | --stdout | --out-dir DIR]\n\
         \x20               [--meta-out FILE] [--diff-against FILE] [--plant NAME]\n\n\
         A .pid input is read on its own, with no plant database: it is one\n\
         drawing, so --drawing / --style / --list-drawings do not apply, and\n\
         only what the file states is written (no derived ports, pipelines or\n\
         default attributes).\n\n\
         --drawing UID       T_Drawing.SP_ID of the drawing to emit.\n\
         --out FILE          write the _Data.xml document to FILE.\n\
         --stdout            write the _Data.xml document to stdout instead.\n\
         --meta-out FILE     write the companion _Meta.xml document to FILE.\n\
         \x20                   Requires --out (incompatible with --stdout).\n\
         --out-dir DIR       .pid input only: write <DocName>_Data.xml,\n\
         \x20                   <DocName>_Meta.xml and the coverage report\n\
         \x20                   <DocName>_Coverage.json into DIR (created if\n\
         \x20                   needed). Mutually exclusive with --out / --stdout\n\
         \x20                   / --meta-out.\n\
         --diff-against FILE compare the generated _Data.xml against a reference\n\
         \x20                   SmartPlant export. A database input prints the tag\n\
         \x20                   and Rel-DefUID count report and exits 1 when any\n\
         \x20                   variety differs. A .pid input scores emitted objects,\n\
         \x20                   values and Rels: one the reference does not carry\n\
         \x20                   exits 1. Classes this mode does not publish are not\n\
         \x20                   MISSING.\n\
         --plant NAME        Plant value for the <Container> root attribute\n\
         \x20                   (default: \"P01\" — override with the real plant).\n\
         --style a01|dwg     Pick the IObject attribute convention used in the\n\
         \x20                   PIDPipeline / PIDPipingConnector / PIDProcessVessel\n\
         \x20                   blocks. `a01` (default) emits ItemTag attributes\n\
         \x20                   matching the A01 SmartPlant export shape; `dwg`\n\
         \x20                   drops ItemTag in favor of Name (or omits it on\n\
         \x20                   PIDProcessVessel) to match the DWG export shape.\n\
         --list-drawings     Print every T_Drawing row in the input table set\n\
         \x20                   (SP_ID, Name, DocumentCategory, DocumentType,\n\
         \x20                   Path) and exit 0. Mutually exclusive with the\n\
         \x20                   render flags.\n\
         -v, --verbose       Print MDF loading diagnostics (table row counts,\n\
         \x20                   timing) to stderr.\n\
         Legacy compatibility historical `Export_v2.sqlite` mirrors are still\n\
         \x20                   accepted during the transition, but MDF is the\n\
         \x20                   only public publish-fidelity baseline and `.sqlite`\n\
         \x20                   inputs print a deprecation warning at runtime.\n\n\
         At least one of --out / --stdout / --out-dir / --diff-against /\n\
         --list-drawings is required."
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 2 {
        return Err(
            "missing <mdf> argument (legacy .sqlite is still accepted during the transition)"
                .into(),
        );
    }
    let input_path = PathBuf::from(&args[1]);
    let mut drawing_uid: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut meta_out_path: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut diff_against: Option<PathBuf> = None;
    let mut stdout = false;
    let mut plant_name: Option<String> = None;
    let mut style: Option<PublishStyle> = None;
    let mut list_drawings = false;
    let mut verbose = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--drawing" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--drawing requires a UID".to_string())?;
                drawing_uid = Some(value.clone());
                i += 2;
            }
            "--out" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--out requires a path".to_string())?;
                out_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--meta-out" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--meta-out requires a path".to_string())?;
                meta_out_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--out-dir" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--out-dir requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
                i += 2;
            }
            "--diff-against" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--diff-against requires a path".to_string())?;
                diff_against = Some(PathBuf::from(value));
                i += 2;
            }
            "--stdout" => {
                stdout = true;
                i += 1;
            }
            "--plant" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--plant requires a name".to_string())?;
                plant_name = Some(value.clone());
                i += 2;
            }
            "--style" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--style requires a value (a01 | dwg)".to_string())?;
                let parsed = match value.to_ascii_lowercase().as_str() {
                    "a01" => PublishStyle::A01,
                    "dwg" => PublishStyle::Dwg,
                    other => {
                        return Err(format!(
                            "--style accepts `a01` or `dwg` (case-insensitive); got `{other}`"
                        ))
                    }
                };
                style = Some(parsed);
                i += 2;
            }
            "--list-drawings" => {
                list_drawings = true;
                i += 1;
            }
            "--verbose" | "-v" => {
                verbose = true;
                i += 1;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let output = match (out_path, stdout) {
        (Some(path), false) => Some(OutputTarget::File(path)),
        (None, true) => Some(OutputTarget::Stdout),
        (Some(_), true) => return Err("--out and --stdout are mutually exclusive".into()),
        (None, false) => None,
    };

    let pid_input = is_pid_path(&input_path);
    if pid_input && (list_drawings || drawing_uid.is_some()) {
        return Err(
            "a .pid input is one drawing: --list-drawings and --drawing do not apply".into(),
        );
    }
    if out_dir.is_some() {
        if !pid_input {
            return Err("--out-dir applies to a .pid input only".into());
        }
        if output.is_some() || meta_out_path.is_some() {
            return Err(
                "--out-dir names every file itself: it is mutually exclusive with \
                 --out / --stdout / --meta-out"
                    .into(),
            );
        }
    }

    if list_drawings {
        // --list-drawings is its own self-contained mode. It
        // requires NONE of --drawing / --out / --stdout /
        // --diff-against / --meta-out (the input path is the
        // only mandatory positional). Reject explicit
        // combinations rather than silently doing both — that
        // would conflate "list" output with rendered XML on
        // stdout, breaking shell pipelines.
        if drawing_uid.is_some() {
            return Err("--list-drawings is mutually exclusive with --drawing".into());
        }
        if output.is_some() {
            return Err("--list-drawings is mutually exclusive with --out / --stdout".into());
        }
        if diff_against.is_some() {
            return Err("--list-drawings is mutually exclusive with --diff-against".into());
        }
        if meta_out_path.is_some() {
            return Err("--list-drawings is mutually exclusive with --meta-out".into());
        }
    } else {
        if drawing_uid.is_none() && !pid_input {
            return Err("--drawing UID is required (or use --list-drawings)".into());
        }
        if output.is_none() && diff_against.is_none() && out_dir.is_none() {
            return Err(
                "at least one of --out FILE / --stdout / --diff-against FILE / --list-drawings \
                 is required (or --out-dir DIR for a .pid input)"
                    .into(),
            );
        }
    }

    if meta_out_path.is_some() && matches!(output, Some(OutputTarget::Stdout)) {
        return Err("--meta-out requires --out (incompatible with --stdout)".into());
    }

    Ok(CliOptions {
        input_path,
        drawing_uid,
        output,
        meta_output: meta_out_path,
        out_dir,
        diff_against,
        plant_name: plant_name.unwrap_or_else(|| "P01".to_string()),
        style: style.unwrap_or_default(),
        list_drawings,
        verbose,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        std::process::exit(0);
    }
    let options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("argument error: {e}");
            print_usage();
            std::process::exit(2);
        }
    };
    if options.verbose {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
    match run(options) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

/// Returned `i32` is the process exit code:
/// - `0` — success and (when `--diff-against` is set) the diff
///   surfaced no actionable findings.
/// - `1` — diff-only failure: the generated XML differs semantically
///   from the reference (used as a CI gate). Real I/O / `SQLite`
///   errors short-circuit through `Err(String)` instead.
fn run(options: CliOptions) -> Result<i32, String> {
    if is_pid_path(&options.input_path) {
        return run_pid(&options);
    }
    warn_if_legacy_sqlite_input(&options.input_path);
    let conn = open_input_as_sqlite(&options.input_path)?;

    // A30 — list mode prints all drawings and exits.
    if options.list_drawings {
        return list_drawings(&conn).map(|_| 0);
    }

    // After parse_args' validation, drawing_uid must be Some when
    // we reach this point. Unwrap is safe and an internal error
    // (rather than a user-facing one) if it ever fires.
    let drawing_uid = options
        .drawing_uid
        .as_deref()
        .expect("drawing_uid required in non-list-drawings mode");
    let mut graph = load_drawing_graph(&conn, drawing_uid).map_err(|e| {
        // A30 · attach a discoverability hint when the drawing
        // UID was not found. The error message stays single-line
        // so test harnesses can keep matching on the prefix.
        match &e {
            PublishError::DrawingNotFound { uid } => format!(
                "load drawing {uid}: {e}; use `--list-drawings` to see available drawing UIDs"
            ),
            _ => format!("load drawing {drawing_uid}: {e}"),
        }
    })?;
    // A29b: thread CLI --style choice through the loaded graph
    // so the writer routes IObject shape via PublishStyle. The
    // default branch keeps every pre-A29b CLI invocation
    // byte-identical (the CLI default is PublishStyle::A01,
    // which matches the model-side Default impl).
    graph.style = options.style;
    let xml =
        write_data_xml(&graph, &options.plant_name).map_err(|e| format!("render data XML: {e}"))?;

    eprintln!(
        "Loaded drawing `{}` ({} UID): {} objects, {} representations, {} relationships",
        graph.drawing_name,
        graph.drawing_uid,
        graph.objects.len(),
        graph.representations.len(),
        graph.relationships.len(),
    );

    emit_data(&options, &xml)?;

    if let Some(meta_path) = &options.meta_output {
        let meta_xml = write_meta_xml(&graph, &options.plant_name)
            .map_err(|e| format!("render meta XML: {e}"))?;
        write_file(meta_path, &meta_xml)?;
        eprintln!("Wrote {} ({} bytes).", meta_path.display(), meta_xml.len());
    }

    match &options.diff_against {
        Some(ref_path) => report_diff(ref_path, &xml),
        None => Ok(0),
    }
}

/// `.pid` input: read the drawing from the file alone and write only what
/// it states.
fn run_pid(options: &CliOptions) -> Result<i32, String> {
    let drawing = load_publish_from_pid(&options.input_path)
        .map_err(|e| format!("read {}: {e}", options.input_path.display()))?;
    let xml = write_pid_data_xml(&drawing, &options.plant_name);
    let classified = drawing
        .items
        .iter()
        .filter(|item| item.class.is_some())
        .count();
    let stats = drawing.stats;
    let derived = derived_by_native_rule_count(&drawing);
    eprintln!(
        "Read drawing `{}` ({} UID) from the .pid alone: {} items ({} with a class, \
         {} instruments classed by their symbol's library folder), {} representations; \
         {} of {} label values reach an item, {} attributes published, \
         {} derived by a native rule, \
         {} left out on disagreeing labels",
        drawing.drawing_name,
        drawing.drawing_uid,
        drawing.items.len(),
        classified,
        stats.instruments_classed_by_library,
        drawing.representations.len(),
        stats.label_values_reached,
        stats.label_values,
        stats.label_values_published,
        derived,
        stats.conflicting_attributes,
    );
    emit_data(options, &xml)?;
    if let Some(meta_path) = &options.meta_output {
        let meta_xml = write_pid_meta_xml(&drawing, &options.plant_name)
            .map_err(|e| format!("render meta XML: {e}"))?;
        write_file(meta_path, &meta_xml)?;
        eprintln!("Wrote {} ({} bytes).", meta_path.display(), meta_xml.len());
    }
    if let Some(dir) = &options.out_dir {
        // The delivery: the two documents under the names SmartPlant's
        // own publish gives them, and the coverage report beside them.
        let meta_xml = write_pid_meta_xml(&drawing, &options.plant_name)
            .map_err(|e| format!("render meta XML: {e}"))?;
        let report = write_pid_coverage_json(&drawing, &options.input_path);
        for (suffix, contents) in [
            ("_Data.xml", xml.as_str()),
            ("_Meta.xml", meta_xml.as_str()),
            ("_Coverage.json", report.as_str()),
        ] {
            let path = dir.join(format!("{}{suffix}", drawing.drawing_name));
            write_file(&path, contents)?;
            eprintln!("Wrote {} ({} bytes).", path.display(), contents.len());
        }
    }
    match &options.diff_against {
        Some(ref_path) => report_pid_diff(ref_path, &xml),
        None => Ok(0),
    }
}

/// Write the rendered `_Data.xml` to the requested target, if any.
fn emit_data(options: &CliOptions, xml: &str) -> Result<(), String> {
    if let Some(target) = &options.output {
        match target {
            OutputTarget::Stdout => {
                print!("{xml}");
            }
            OutputTarget::File(path) => {
                write_file(path, xml)?;
                eprintln!("Wrote {} ({} bytes).", path.display(), xml.len());
            }
        }
    }
    Ok(())
}

/// Print the `.pid` export's object / value / rel score. Absence of a class
/// this mode does not publish is not a finding; an emitted object, value or
/// rel the reference does not carry exits 1.
fn report_pid_diff(ref_path: &Path, xml: &str) -> Result<i32, String> {
    let reference = std::fs::read_to_string(ref_path)
        .map_err(|e| format!("read reference {}: {e}", ref_path.display()))?;
    let diff = diff_pid_export(xml, &reference);
    print!("{diff}");
    if diff.is_clean() {
        eprintln!(
            "Semantic diff against {} is clean in .pid mode ({} objects, {} values, {} rels).",
            ref_path.display(),
            diff.objects,
            diff.values,
            diff.rels,
        );
        Ok(0)
    } else {
        eprintln!(
            "Semantic diff against {} surfaced findings in .pid mode:\n  \
             objects extra={} wrong_class={}\n  \
             values different={} not_in_reference={}\n  \
             rels not_in_reference={}\n\
             exit 1.",
            ref_path.display(),
            diff.objects_not_in_reference,
            diff.objects_of_another_class,
            diff.values_different,
            diff.values_not_in_reference,
            diff.rels_not_in_reference,
        );
        Ok(1)
    }
}

/// Print the semantic diff of `xml` against the reference at `ref_path`;
/// the exit code is 1 when it surfaces findings.
fn report_diff(ref_path: &Path, xml: &str) -> Result<i32, String> {
    let mut exit_code = 0;
    {
        let reference = std::fs::read_to_string(ref_path)
            .map_err(|e| format!("read reference {}: {e}", ref_path.display()))?;
        let tag_report = diff_publish_xml(xml, &reference);
        // Surface the report on stdout so it can be redirected to
        // a file or compared in tests; the "Loaded drawing..." line
        // stays on stderr.
        println!("{tag_report}");

        // A40: append a Rel-DefUID-level diff block. `<PIDxxx>` tag
        // counts answer "are all business objects emitted?"; the
        // Rel block answers the complementary question "are all
        // cross-references emitted?" A drawing can pass one and
        // fail the other — the combined report catches both.
        let rel_report = diff_rel_defuids(xml, &reference);
        println!();
        println!("{rel_report}");

        let tag_clean = tag_report.is_clean();
        let rel_clean = rel_report.is_clean();
        if !tag_clean || !rel_clean {
            eprintln!(
                "Semantic diff against {} surfaced findings:\n  \
                 PID tag  findings — missing={} extra={} count_deltas={}\n  \
                 Rel DefUID findings — missing={} extra={} count_deltas={}\n\
                 exit 1.",
                ref_path.display(),
                tag_report.missing_from_generated,
                tag_report.extra_in_generated,
                tag_report.count_deltas,
                rel_report.missing_from_generated,
                rel_report.extra_in_generated,
                rel_report.count_deltas,
            );
            exit_code = 1;
        } else {
            eprintln!(
                "Semantic diff against {} is clean ({} matching tag varieties, \
                 {} matching Rel DefUIDs).",
                ref_path.display(),
                tag_report.matching,
                rel_report.matching,
            );
        }
    }

    Ok(exit_code)
}

fn open_input_as_sqlite(path: &std::path::Path) -> Result<rusqlite::Connection, String> {
    if is_mdf_path(path) {
        open_mdf_as_sqlite(path).map_err(|e| format!("open MDF {}: {e}", path.display()))
    } else {
        open_readonly(path).map_err(|e| format!("open SQLite {}: {e}", path.display()))
    }
}

fn is_mdf_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdf"))
}

fn is_pid_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pid"))
}

fn is_sqlite_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sqlite"))
}

fn warn_if_legacy_sqlite_input(path: &std::path::Path) {
    if is_sqlite_path(path) {
        eprintln!(
            "warning: `.sqlite` input is deprecated for publish fidelity; \
             prefer `Export.mdf`. The SQLite path remains available only as \
             a legacy compatibility adapter."
        );
    }
}

/// A30 · `--list-drawings` mode: print every `T_Drawing` row in
/// the input table set as a tab-aligned table and return. Reuses
/// the same `Connection` already opened by `run`, so the query
/// path validation is centralized.
///
/// Output layout pins to fixed-width columns so an operator can
/// `grep` for a Name / Path substring; the trailing summary
/// ("Total: N drawing(s)") goes to stderr to keep stdout
/// pipe-friendly.
fn list_drawings(conn: &rusqlite::Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT SP_ID, Name, DocumentCategory, DocumentType, Path FROM T_Drawing")
        .map_err(|e| format!("prepare T_Drawing query: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("query T_Drawing rows: {e}"))?;
    println!(
        "{:<34} | {:<24} | {:<22} | {:<10} | Path",
        "SP_ID", "Name", "DocumentCategory", "DocumentType",
    );
    println!("{}", "-".repeat(120));
    let mut count = 0usize;
    for row in rows {
        let (uid, name, cat, dtype, path) = row.map_err(|e| format!("read T_Drawing row: {e}"))?;
        println!("{uid:<34} | {name:<24} | {cat:<22} | {dtype:<10} | {path}",);
        count += 1;
    }
    eprintln!("\nTotal: {count} drawing(s).");
    Ok(())
}

/// Write `contents` to `path`, creating the parent directory chain
/// if needed. Pulled out so `_Data.xml` and `_Meta.xml` go through
/// the same I/O path and surface the same `format!` error messages.
fn write_file(path: &std::path::Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
    }
    std::fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}
