//! CLI: generate SmartPlant Publish Data XML from the SQLite
//! mirror produced by `tools/orca-mdf-probe`.
//!
//! Stage-1 terminal binary. Given
//!
//! 1. a SQLite file produced by `OrcaMdfProbe --to-sqlite`,
//! 2. a SmartPlant drawing UID (the `T_Drawing.SP_ID` value),
//!
//! this tool emits a `_Data.xml` document that carries the
//! drawing's structural skeleton. Business-interface fields
//! (`<PIDProcessVessel>` / `<PIDNozzle>` / ...) land in a later
//! commit once the loader reads their subtables; the current
//! output is intentionally minimal but well-formed.
//!
//! Usage:
//!
//! ```text
//! pid_publish_xml <sqlite> --drawing UID --out <file> [--plant NAME]
//! pid_publish_xml <sqlite> --drawing UID --stdout [--plant NAME]
//! ```
//!
//! Exit codes: 0 = wrote document, 1 = I/O / format error, 2 =
//! usage error.

use pid_parse::publish::sqlite_load::open_readonly;
use pid_parse::publish::{diff_publish_xml, load_drawing_graph, write_data_xml, write_meta_xml};
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct CliOptions {
    sqlite_path: PathBuf,
    drawing_uid: String,
    output: Option<OutputTarget>,
    /// Optional `_Meta.xml` companion sink. Only honored when
    /// `output` is a file target — pairing meta output with stdout
    /// would interleave two unrelated XML documents on the same
    /// stream.
    meta_output: Option<PathBuf>,
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
}

#[derive(Debug, Clone)]
enum OutputTarget {
    Stdout,
    File(PathBuf),
}

fn print_usage() {
    eprintln!(
        "Usage: pid_publish_xml <sqlite> --drawing UID [--out FILE | --stdout]\n\
         \x20               [--meta-out FILE] [--diff-against FILE] [--plant NAME]\n\n\
         --drawing UID       T_Drawing.SP_ID of the drawing to emit.\n\
         --out FILE          write the _Data.xml document to FILE.\n\
         --stdout            write the _Data.xml document to stdout instead.\n\
         --meta-out FILE     write the companion _Meta.xml document to FILE.\n\
         \x20                   Requires --out (incompatible with --stdout).\n\
         --diff-against FILE compare the generated _Data.xml against a reference\n\
         \x20                   SmartPlant export and print a SemanticDiffReport\n\
         \x20                   to stdout. Exits 1 when any tag varieties differ.\n\
         --plant NAME        Plant value for the <Container> root attribute\n\
         \x20                   (default: \"P01\" — override with the real plant).\n\n\
         At least one of --out / --stdout / --diff-against is required."
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 2 {
        return Err("missing <sqlite> argument".into());
    }
    let sqlite_path = PathBuf::from(&args[1]);
    let mut drawing_uid: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut meta_out_path: Option<PathBuf> = None;
    let mut diff_against: Option<PathBuf> = None;
    let mut stdout = false;
    let mut plant_name: Option<String> = None;

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
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let drawing_uid = drawing_uid.ok_or_else(|| "--drawing UID is required".to_string())?;
    let output = match (out_path, stdout) {
        (Some(path), false) => Some(OutputTarget::File(path)),
        (None, true) => Some(OutputTarget::Stdout),
        (Some(_), true) => {
            return Err("--out and --stdout are mutually exclusive".into())
        }
        (None, false) => None,
    };

    if output.is_none() && diff_against.is_none() {
        return Err(
            "at least one of --out FILE / --stdout / --diff-against FILE is required".into(),
        );
    }

    if meta_out_path.is_some() && matches!(output, Some(OutputTarget::Stdout)) {
        return Err("--meta-out requires --out (incompatible with --stdout)".into());
    }

    Ok(CliOptions {
        sqlite_path,
        drawing_uid,
        output,
        meta_output: meta_out_path,
        diff_against,
        plant_name: plant_name.unwrap_or_else(|| "P01".to_string()),
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
///   from the reference (used as a CI gate). Real I/O / SQLite
///   errors short-circuit through `Err(String)` instead.
fn run(options: CliOptions) -> Result<i32, String> {
    let conn = open_readonly(&options.sqlite_path)
        .map_err(|e| format!("open {}: {e}", options.sqlite_path.display()))?;
    let graph = load_drawing_graph(&conn, &options.drawing_uid)
        .map_err(|e| format!("load drawing {}: {e}", options.drawing_uid))?;
    let xml = write_data_xml(&graph, &options.plant_name)
        .map_err(|e| format!("render data XML: {e}"))?;

    eprintln!(
        "Loaded drawing `{}` ({} UID): {} objects, {} representations, {} relationships",
        graph.drawing_name,
        graph.drawing_uid,
        graph.objects.len(),
        graph.representations.len(),
        graph.relationships.len(),
    );

    if let Some(target) = &options.output {
        match target {
            OutputTarget::Stdout => {
                print!("{xml}");
            }
            OutputTarget::File(path) => {
                write_file(path, &xml)?;
                eprintln!("Wrote {} ({} bytes).", path.display(), xml.len());
            }
        }
    }

    if let Some(meta_path) = &options.meta_output {
        let meta_xml = write_meta_xml(&graph, &options.plant_name)
            .map_err(|e| format!("render meta XML: {e}"))?;
        write_file(meta_path, &meta_xml)?;
        eprintln!(
            "Wrote {} ({} bytes).",
            meta_path.display(),
            meta_xml.len()
        );
    }

    let mut exit_code = 0;
    if let Some(ref_path) = &options.diff_against {
        let reference = std::fs::read_to_string(ref_path)
            .map_err(|e| format!("read reference {}: {e}", ref_path.display()))?;
        let report = diff_publish_xml(&xml, &reference);
        // Surface the report on stdout so it can be redirected to
        // a file or compared in tests; the "Loaded drawing..." line
        // stays on stderr.
        println!("{report}");
        if !report.is_clean() {
            eprintln!(
                "Semantic diff against {} surfaced findings (missing={} extra={} count_deltas={}); exit 1.",
                ref_path.display(),
                report.missing_from_generated,
                report.extra_in_generated,
                report.count_deltas,
            );
            exit_code = 1;
        } else {
            eprintln!(
                "Semantic diff against {} is clean ({} matching tag varieties).",
                ref_path.display(),
                report.matching,
            );
        }
    }

    Ok(exit_code)
}

/// Write `contents` to `path`, creating the parent directory chain
/// if needed. Pulled out so `_Data.xml` and `_Meta.xml` go through
/// the same I/O path and surface the same `format!` error messages.
fn write_file(path: &std::path::Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
    }
    std::fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}
