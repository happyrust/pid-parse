//! CLI: generate `SmartPlant` Publish Data XML (`_Data.xml` and its
//! `_Meta.xml` companion) from one of four inputs:
//!
//! * a SQL Server `Export.mdf` (or a legacy `Export_v2.sqlite` mirror)
//!   and the `T_Drawing.SP_ID` of one drawing — the database line, with
//!   `--style a01|dwg`, `--strict` and `--list-drawings`;
//! * a `.pid` alone, written with `--out-dir` beside a `_Coverage.json`;
//! * a `.pid` plus the backup's Oracle `exp` dump (`--oracle-exp`,
//!   optionally `--datamap`), which adds ports, connectivity,
//!   compositions, off-page connectors and synthesized pipelines;
//! * a backup-package directory (one containing `Manifest.txt`), which
//!   publishes every drawing into `--out-dir` with an `_Index.json`;
//!   `--strict` gates on the drawings of a SQL Server package, the ones
//!   the database line writes.
//!
//! `--diff-against` scores the generated document against a reference
//! `SmartPlant` export (or, for a backup directory, a directory of them).
//!
//! Usage:
//!
//! ```text
//! pid_publish_xml <mdf> --drawing UID --out <file> [--meta-out FILE] [--plant NAME]
//! pid_publish_xml <file.pid> --out-dir DIR [--oracle-exp DUMP [--datamap FILE]]
//! pid_publish_xml <backup dir> --out-dir DIR [--diff-against DIR] [--strict]
//! ```
//!
//! The input is the first argument and the flags follow it.
//!
//! Exit codes: 0 = written (and, with `--diff-against`, clean);
//! 1 = an I/O or format error (a stdout that cannot be written is one), a
//! `--diff-against` finding, a failure a package run counted (a drawing
//! that was not written, a `PlantData` zip that could not be opened, a zip
//! entry whose header could not be read), or, with `--strict`, a database
//! drawing that was not written in full; 2 = a usage error, including an
//! `--oracle-exp` dump that holds no `T_DRAWING` row for the `.pid`'s
//! drawing.

use pid_parse::publish::backup_package::{
    write_atomic, BackupPublishReport, PackageChainFault, PackageDrawingShortfalls,
    PackageDuplicateUids,
};
use pid_parse::publish::diff::diff_meta_xml;
use pid_parse::publish::oracle_exp::{attach_dump, DumpAttachError, DumpAttachment};
use pid_parse::publish::pid_writer::{
    composition_rel_counts, connectivity_rel_counts, derived_by_native_rule_count,
};
use pid_parse::publish::sqlite_load::open_readonly;
use pid_parse::publish::{
    database_shortfalls, diff_pid_export, load_drawing_graph, open_mdf_as_sqlite, write_data_xml,
    write_meta_xml, write_pid_coverage_json, write_pid_data_xml, write_pid_meta_xml,
    MalformedPublishDocument, PublishError, PublishStyle,
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
    /// against. When set, the binary prints the object / value / rel
    /// findings of `diff_pid_export` to stdout and exits 0 when there
    /// are none, 1 otherwise.
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
    /// `--strict`: exit 1 when the database line did not write a drawing
    /// in full — an item written as a `<PIDItem>` placeholder, or a
    /// relationship row defined from the item types of its ends alone
    /// (`DatabaseShortfalls`). The files and the warnings are the same
    /// with and without it. Database input, and a backup directory: there
    /// it gates on the drawings of a SQL Server package, and changes
    /// nothing for a package whose drawings are read from the `.pid`.
    strict: bool,
    /// A30 — when true, the CLI prints every `T_Drawing` row in
    /// the input table set and exits 0 without rendering anything.
    /// Mutually exclusive with `--drawing` / `--out` / `--stdout`
    /// / `--diff-against` because there is no per-drawing
    /// output to produce.
    list_drawings: bool,
    verbose: bool,
    /// Oracle conventional `exp` dump. `.pid` input only. Ports whose
    /// parent this file already publishes are emitted from it.
    oracle_exp: Option<PathBuf>,
    /// `SPPIDDataMap.xml`, or a zip that contains that exact entry.
    /// With `--oracle-exp`, supplies pipeline `FluidSystem`.
    datamap: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum OutputTarget {
    Stdout,
    File(PathBuf),
}

/// Print the usage body to `out`. `--help` passes stdout — the stream
/// the caller asked for — and an argument error passes stderr, next to
/// the diagnostic that explains the failure.
fn print_usage<W: std::io::Write>(mut out: W) {
    let _ = writeln!(
        out,
        "Usage: pid_publish_xml <mdf> --drawing UID [--out FILE | --stdout]\n\
         \x20               [--meta-out FILE] [--diff-against FILE] [--plant NAME]\n\
         \x20               [--style a01|dwg] [--strict]\n\
         \x20  pid_publish_xml <mdf> --list-drawings\n\
         \x20  pid_publish_xml <file.pid> [--out FILE | --stdout | --out-dir DIR]\n\
         \x20               [--meta-out FILE] [--diff-against FILE] [--plant NAME]\n\
         \x20               [--oracle-exp FILE] [--datamap FILE]\n\
         \x20  pid_publish_xml <backup-dir> --out-dir DIR [--plant NAME] [--strict]\n\n\
         The input is the first argument and every flag follows it. A flag\n\
         given first is taken for the input: `pid_publish_xml --stdout a.pid`\n\
         is an argument error, `unknown flag: a.pid`.\n\n\
         A .pid input is read on its own, with no plant database: it is one\n\
         drawing, so --drawing / --style / --list-drawings do not apply, and\n\
         only what the file states is written (no derived ports, pipelines or\n\
         default attributes).\n\n\
         A directory that contains Manifest.txt is a backup package. Every\n\
         .pid entry in its PlantData zips is published into --out-dir, plus\n\
         _Index.json. It publishes every drawing, so --drawing /\n\
         --list-drawings / --style / --oracle-exp / --datamap do not apply.\n\
         Export.dmp is read once when it is an Oracle exp. A\n\
         SQL Server MTF dump is stripped to its MDF and every T_Drawing\n\
         row publishes through the database path. --diff-against DIR compares\n\
         every written drawing against <DIR>/<Name>_Data.xml and prints one\n\
         clean / dirty / unpaired / revision-mismatch verdict per drawing; a\n\
         drawing that is dirty, of another revision than its reference, or has\n\
         no reference there exits 1. One drawing that fails\n\
         is recorded; the rest are still written; the process then exits 1.\n\
         A PlantData file that starts with the zip signature and cannot be\n\
         opened, and a zip entry whose header cannot be read, are recorded among\n\
         the problems of _Index.json; the process exits 1 for them as well.\n\n\
         --out-dir is not emptied first: files an earlier run left there are\n\
         not removed. For a backup package _Index.json is what says which\n\
         files this run wrote; a drawing that fails this time may still have\n\
         files of an earlier run there. When one of a package drawing's three\n\
         files cannot be written, the others this run wrote for it are removed\n\
         again.\n\n\
         Every file is first written under a temporary name beside its own,\n\
         <name>.<process id>.tmp, and renamed once it is whole. An interrupted\n\
         run therefore leaves no partly written document. It may leave files\n\
         whose names end in .tmp, which can be deleted, and a drawing with\n\
         only some of its three files.\n\n\
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
         --diff-against PATH compare the generated _Data.xml against a reference\n\
         \x20                   SmartPlant export, or, for a backup directory, a\n\
         \x20                   DIRECTORY of <Name>_Data.xml files: every drawing\n\
         \x20                   is scored and reported. A database or .pid input\n\
         \x20                   scores emitted objects, values and Rels: one the\n\
         \x20                   reference does not carry exits 1. Reference-only\n\
         \x20                   content the exclusion\n\
         \x20                   list does not cover exits 1 too. Excused by\n\
         \x20                   design and printed with their reason: classes\n\
         \x20                   the .pid file cannot state (Port / Enrichment),\n\
         \x20                   relation classes this line never writes (and the\n\
         \x20                   dump's, when the run has no exp content), the\n\
         \x20                   plant database's attribute classes, and the\n\
         \x20                   tag-named duplicate elements. With --oracle-exp\n\
         \x20                   the clean line says oracle-exp mode.\n\
         \x20                   With a .pid or database input, a _Meta.xml this\n\
         \x20                   run writes (--out-dir or --meta-out) is scored\n\
         \x20                   too: the <Name>_Meta.xml beside the reference\n\
         \x20                   <Name>_Data.xml is compared on the 13\n\
         \x20                   <Container> attributes, the top-level block\n\
         \x20                   order, the three Rel DefUIDs and their role\n\
         \x20                   wiring, and the sibling _Data.xml DocUID.\n\
         \x20                   DocVersionDate, FilePath and the six derived\n\
         \x20                   UUID v5 ids are registered accepted\n\
         \x20                   differences. Drift exits 1.\n\
         --oracle-exp FILE   .pid input only: read piping and signal ports from\n\
         \x20                   an Oracle exp dump and emit them for parents this\n\
         \x20                   file already publishes. Pipe runs with a tag\n\
         \x20                   sequence become PIDPipeline objects. Without it\n\
         \x20                   the output is the .pid alone.\n\
         --datamap FILE      .pid input with --oracle-exp: SPPIDDataMap.xml or\n\
         \x20                   a zip that contains that entry. Supplies\n\
         \x20                   FluidSystem. Without it pipelines have no\n\
         \x20                   FluidSystem.\n\
         --plant NAME        Plant value for the <Container> root attribute\n\
         \x20                   (default: \"P01\" — override with the real plant).\n\
         --style a01|dwg     Pick the IObject attribute convention used in the\n\
         \x20                   PIDPipeline / PIDPipingConnector / PIDProcessVessel\n\
         \x20                   blocks. `a01` (default) emits ItemTag attributes\n\
         \x20                   matching the A01 SmartPlant export shape; `dwg`\n\
         \x20                   drops ItemTag in favor of Name (or omits it on\n\
         \x20                   PIDProcessVessel) to match the DWG export shape.\n\
         \x20                   Database input only: a .pid input and a backup\n\
         \x20                   directory reject it instead of ignoring it.\n\
         --strict            Database input or backup directory: exit 1 when\n\
         \x20                   a drawing the database line writes is not\n\
         \x20                   written in full. An item whose type has no\n\
         \x20                   writer of its own is written as a <PIDItem>\n\
         \x20                   placeholder, without its class and attributes,\n\
         \x20                   and a T_Relationship row with an end that is not\n\
         \x20                   a representation gets its definition from the\n\
         \x20                   item types of its ends alone. Each is a warning\n\
         \x20                   on stderr with or without --strict, and the\n\
         \x20                   files are written either way. In a backup\n\
         \x20                   directory only a SQL Server package has such\n\
         \x20                   drawings: each warning names its drawing and is\n\
         \x20                   listed among the problems of _Index.json, and\n\
         \x20                   <Name>_Coverage.json holds the counts under\n\
         \x20                   shortfalls. With an Oracle exp, or no dump,\n\
         \x20                   every drawing comes from its .pid and --strict\n\
         \x20                   changes nothing; which kind a package is shows\n\
         \x20                   only once it is read, so the flag is not refused\n\
         \x20                   there. A .pid input and --list-drawings reject\n\
         \x20                   --strict instead of ignoring it.\n\
         --list-drawings     Print every T_Drawing row in the input table set\n\
         \x20                   (SP_ID, Name, DocumentCategory, DocumentType,\n\
         \x20                   Path) and exit 0. Mutually exclusive with the\n\
         \x20                   render flags.\n\
         -v, --verbose       Print the log to stderr: the tables staged from an\n\
         \x20                   MDF (the input, or a SQL Server package's) with\n\
         \x20                   row counts and timing, and each table a drawing\n\
         \x20                   is read from that the database does not hold.\n\
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
    let mut strict = false;
    let mut verbose = false;
    let mut oracle_exp: Option<PathBuf> = None;
    let mut datamap: Option<PathBuf> = None;

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
            "--strict" => {
                strict = true;
                i += 1;
            }
            "--verbose" | "-v" => {
                verbose = true;
                i += 1;
            }
            "--oracle-exp" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--oracle-exp requires a path".to_string())?;
                oracle_exp = Some(PathBuf::from(value));
                i += 2;
            }
            "--datamap" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--datamap requires a path".to_string())?;
                datamap = Some(PathBuf::from(value));
                i += 2;
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
    let package_input = input_path.is_dir();
    if package_input {
        if !input_path.join("Manifest.txt").is_file() {
            return Err(
                "a directory input is a backup package only when it contains Manifest.txt".into(),
            );
        }
        if oracle_exp.is_some() {
            return Err(
                "a backup directory reads Export.dmp itself; --oracle-exp does not apply".into(),
            );
        }
        // I1: the package line publishes every drawing itself and reads no
        // per-drawing style switch, so `--style` used to be accepted and then
        // ignored. Refuse it like the sibling flags above.
        if style.is_some() {
            return Err("a backup directory publishes every .pid; --style does not apply".into());
        }
        // `--strict` is taken here, unlike the flags above: whether the
        // package holds drawings of the database line — a SQL Server dump —
        // shows only once it is read.
        if datamap.is_some() {
            return Err(
                "a backup directory reads RefData~4~804.zip itself; --datamap does not apply"
                    .into(),
            );
        }
        if list_drawings || drawing_uid.is_some() {
            return Err(
                "a backup directory publishes every .pid; --drawing and --list-drawings do not apply"
                    .into(),
            );
        }
        // G4: --diff-against applies to a backup directory too. The
        // reference is a directory of <Name>_Data.xml files; the package
        // gate scores every written drawing against it and reports one
        // clean / dirty / unpaired verdict per drawing.
        if output.is_some() || meta_out_path.is_some() {
            return Err("a backup directory writes with --out-dir only".into());
        }
        if out_dir.is_none() {
            return Err("a backup directory requires --out-dir DIR".into());
        }
    }
    if oracle_exp.is_some() && !pid_input {
        return Err("--oracle-exp applies to a .pid input only".into());
    }
    if datamap.is_some() && oracle_exp.is_none() {
        return Err("--datamap requires --oracle-exp".into());
    }
    if pid_input && (list_drawings || drawing_uid.is_some()) {
        return Err(
            "a .pid input is one drawing: --list-drawings and --drawing do not apply".into(),
        );
    }
    // I1: the `.pid` line writes what the file states, with no style switch,
    // so an explicit `--style` used to be accepted and then ignored.
    if pid_input && style.is_some() {
        return Err("a .pid input is one drawing: --style does not apply".into());
    }
    // What `--strict` gates on is the database line's: a `.pid` is read
    // without a plant database, and says what it leaves out in its own
    // coverage report.
    if pid_input && strict {
        return Err(
            "a .pid input is read without a plant database: --strict does not apply".into(),
        );
    }
    if out_dir.is_some() {
        if !pid_input && !package_input {
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
        if strict {
            return Err("--list-drawings is mutually exclusive with --strict".into());
        }
    } else {
        if drawing_uid.is_none() && !pid_input && !package_input {
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
        strict,
        list_drawings,
        verbose,
        oracle_exp,
        datamap,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage(std::io::stdout());
        std::process::exit(0);
    }
    let options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("argument error: {e}");
            print_usage(std::io::stderr());
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
/// - `1` — the generated XML differs from the reference (used as a CI
///   gate), a package run counted a failure
///   ([`BackupPublishReport::failed`]), or `--strict` was given and the
///   database line did not write a drawing in full. Real I/O / `SQLite`
///   errors short-circuit through `Err(String)` instead.
/// - `2` — the input cannot be published as asked: a package with an
///   unusable `Export.dmp`, or an `--oracle-exp` dump of another revision.
fn run(options: CliOptions) -> Result<i32, String> {
    if options.input_path.is_dir() {
        return run_package(&options);
    }
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

    let meta_xml = match &options.meta_output {
        Some(meta_path) => {
            let meta_xml = write_meta_xml(&graph, &options.plant_name)
                .map_err(|e| format!("render meta XML: {e}"))?;
            write_file(meta_path, &meta_xml)?;
            eprintln!("Wrote {} ({} bytes).", meta_path.display(), meta_xml.len());
            Some(meta_xml)
        }
        None => None,
    };

    // What the writer could not write in full is said once the files are on
    // disk: the run delivers them either way, and without `--strict` these
    // lines are the only place that says the delivery is short.
    let shortfalls = database_shortfalls(&graph);
    for sentence in shortfalls.describe() {
        eprintln!("warning: {sentence}");
    }

    let diff_exit = match &options.diff_against {
        Some(ref_path) => {
            let reference = Reference::new(ref_path);
            let meta_exit = match &meta_xml {
                Some(meta_xml) => report_meta_diff(&reference, &xml, meta_xml)?,
                None => 0,
            };
            report_pid_diff(&reference, &xml, DiffMode::Database, meta_exit != 0)?
        }
        None => 0,
    };
    if options.strict && !shortfalls.is_empty() {
        eprintln!("--strict: the drawing is not written in full (see the warnings above); exit 1.");
        return Ok(1);
    }
    Ok(diff_exit)
}

/// Backup directory: publish every `.pid` in the `PlantData` zips.
///
/// A drawing with a chain stream whose record chain does not hold gets one
/// `warning:` line a stream on stderr, as [`run_pid`] prints for a single
/// `.pid`, and `_Index.json` lists the same sentence among its `problems`;
/// the exit code, the index's counts and the coverage reports are what
/// they would be without it.
///
/// A drawing of a SQL Server package that the database line did not write
/// in full gets one `warning:` line for each kind of shortfall, under its
/// name, as [`run`] prints for a single database drawing; `_Index.json`
/// lists the same sentences among its `problems`, and the drawing is
/// written and counts as written. With `--strict` such a drawing turns the
/// exit code to 1, and stderr ends with the line that says so.
///
/// A drawing whose `_Data.xml` states a UID on more than one block gets one
/// `warning:` line under its name, with the UIDs; `_Index.json` lists the
/// same sentence among its `problems`, and the drawing is written and
/// counts as written. The exit code is what it would be without the line,
/// with `--strict` as well; where `--diff-against` compares the drawing
/// with its reference, the gate makes a finding of each such UID on its
/// own account.
fn run_package(options: &CliOptions) -> Result<i32, String> {
    let out_dir = options
        .out_dir
        .as_ref()
        .ok_or_else(|| "a backup directory requires --out-dir DIR".to_string())?;
    match pid_parse::publish::backup_package::publish_backup_package(
        &options.input_path,
        out_dir,
        &options.plant_name,
        options.diff_against.as_deref(),
    ) {
        Ok(report) => {
            for line in package_warnings(&report) {
                eprintln!("warning: {line}");
            }
            let exit = package_exit_code(&report, options.strict);
            if let Some(gate) = &report.gate {
                for line in &gate.lines {
                    write_stdout(format_args!("{line}\n"))?;
                }
                let reference = options
                    .diff_against
                    .as_deref()
                    .unwrap_or_else(|| Path::new("."));
                if gate.clean + gate.dirty == 0 {
                    eprintln!(
                        "Package gate against {}: none of the {} written drawings has a \
                         comparable reference there ({} of another revision, {} without a \
                         reference); nothing was compared; exit {}.",
                        reference.display(),
                        gate.unpaired + gate.revision_mismatch,
                        gate.revision_mismatch,
                        gate.unpaired,
                        exit,
                    );
                } else {
                    eprintln!(
                        "Package gate against {}: {} drawings, {} clean, {} with findings, \
                         {} of another revision, {} without a reference; exit {}.",
                        reference.display(),
                        gate.clean + gate.dirty + gate.unpaired + gate.revision_mismatch,
                        gate.clean,
                        gate.dirty,
                        gate.revision_mismatch,
                        gate.unpaired,
                        exit,
                    );
                }
            }
            eprintln!(
                "Published {} drawings from {} ({} failed, {} without a DrawingNo). Wrote {}.",
                report.written,
                options.input_path.display(),
                report.failed,
                report.shells,
                report.index_path.display(),
            );
            if let Some(verdict) = strict_package_verdict(&report, options.strict) {
                eprintln!("{verdict}");
            }
            Ok(exit)
        }
        Err(pid_parse::publish::backup_package::BackupPublishError::Usage(detail)) => {
            eprintln!("argument error: {detail}");
            Ok(2)
        }
        Err(pid_parse::publish::backup_package::BackupPublishError::Io(detail)) => Err(detail),
    }
}

/// The exit code of a backup directory run that published: 1 when the run
/// counted a failure ([`BackupPublishReport::failed`]: a drawing that was
/// not written, a `PlantData` zip that could not be opened, a zip entry
/// whose header could not be read) or the `--diff-against` gate did not
/// find every drawing clean, and, with `--strict`, when a database drawing
/// was not written in full; 0 otherwise.
fn package_exit_code(report: &BackupPublishReport, strict: bool) -> i32 {
    i32::from(
        report.failed > 0
            || report
                .gate
                .as_ref()
                .is_some_and(|gate| gate.not_clean() > 0)
            || strict_package_verdict(report, strict).is_some(),
    )
}

/// The `warning:` lines of a backup directory run, in the order stderr
/// carries them: a line for each chain stream whose record chain does not
/// hold, naming the zip and the entry as the single-`.pid` mode's line
/// does, then a line for each kind of shortfall of each drawing the
/// database line did not write in full, under the drawing's name, then a
/// line for each drawing whose document states a UID on more than one
/// block, under its name as well. Every drawing spoken of is written and
/// counts as written, and `_Index.json` lists the same sentences among its
/// `problems`.
fn package_warnings(report: &BackupPublishReport) -> Vec<String> {
    let chain_faults = report.chain_faults.iter().map(PackageChainFault::describe);
    let shortfalls = report
        .shortfalls
        .iter()
        .flat_map(PackageDrawingShortfalls::describe);
    let duplicate_uids = report
        .duplicate_uids
        .iter()
        .map(PackageDuplicateUids::describe);
    chain_faults
        .chain(shortfalls)
        .chain(duplicate_uids)
        .collect()
}

/// The line a backup directory run ends stderr with when `--strict` turns
/// its exit code to 1: how many of the written drawings the database line
/// did not write in full. `None` without `--strict`, and when there is no
/// such drawing — as in every Oracle or dump-less package, none of whose
/// drawings the database line writes.
fn strict_package_verdict(report: &BackupPublishReport, strict: bool) -> Option<String> {
    let short = report.shortfalls.len();
    (strict && short > 0).then(|| {
        format!(
            "--strict: {short} {} not written in full (see the warnings above); exit 1.",
            if short == 1 {
                "drawing is"
            } else {
                "drawings are"
            }
        )
    })
}

/// `.pid` input: read the drawing from the file alone and write only what
/// it states.
///
/// The file is read once. A chain stream of it whose record chain does not
/// hold is said on stderr, one line a stream, with what the publish reader
/// made of the stream: the reader takes a stream's records all or nothing,
/// so a chain that stops short costs the delivery what that stream holds
/// while the exit code stays 0, and this is the only place that says so.
/// The lines are printed before the drawing is asked for, so a run that
/// ends in "no DrawingNo" because the row's own stream is the damaged one
/// has said why.
fn run_pid(options: &CliOptions) -> Result<i32, String> {
    let read = pid_parse::publish::pid_load::read_publish_from_pid(&options.input_path)
        .map_err(|e| format!("read {}: {e}", options.input_path.display()))?;
    for fault in &read.chain_faults {
        eprintln!(
            "warning: {}: {}",
            options.input_path.display(),
            fault.describe()
        );
    }
    let mut drawing = read
        .drawing
        .map_err(|e| format!("read {}: {e}", options.input_path.display()))?;
    if let Some(dump_path) = &options.oracle_exp {
        let dump =
            std::fs::read(dump_path).map_err(|e| format!("read {}: {e}", dump_path.display()))?;
        let exp_tables = pid_parse::publish::oracle_exp::ExpTables::read(&dump)
            .map_err(|e| format!("read {}: {e}", dump_path.display()))?;
        // A dump of another revision has no content for this drawing. Attach
        // nothing and refuse: publishing the `.pid` alone while claiming the
        // dump's enrichment would be a silent zero.
        let listed =
            pid_parse::publish::oracle_exp::drawing_in_dump(&exp_tables, &drawing.drawing_uid)
                .map_err(|e| format!("read {}: {e}", dump_path.display()))?;
        if !listed {
            eprintln!(
                "argument error: {} has no T_DRAWING row for DocUID {}; the .pid is not this \
                 dump's revision",
                dump_path.display(),
                drawing.drawing_uid,
            );
            return Ok(2);
        }
        // The library attaches the dump, and reads `--datamap` when it comes
        // to the first stage that takes one. A stage that finished has its
        // say here even when a later one refuses: the run ends in that
        // refusal having said how far the dump got.
        let mut attached = DumpAttachment::default();
        let outcome = attach_dump(
            &mut drawing,
            &exp_tables,
            dump_path,
            &options.plant_name,
            || match &options.datamap {
                Some(path) => pid_parse::backup::read_datamap(path)
                    .map(Some)
                    .map_err(|error| format!("read {}: {error}", path.display())),
                None => Ok(None),
            },
            &mut attached,
        );
        let dump_only = attached.dump_only_components.unwrap_or(0);
        if dump_only > 0 {
            eprintln!(
                "oracle-exp: {dump_only} PIDPipingComponent from the dump, not drawn in the .pid"
            );
        }
        if let Some(units) = attached.unit_values {
            eprintln!(
                "oracle-exp unit values: {} written, {} left out because the item's class \
                 carries no element for them",
                units.written, units.left_out
            );
        }
        outcome.map_err(|error| match error {
            DumpAttachError::Dump(error) => format!("read {}: {error}", dump_path.display()),
            DumpAttachError::DataMap(message) => message,
        })?;
        if let Some(codelist_values) = attached.codelist_values {
            eprintln!("oracle-exp codelists: {codelist_values} enumerated values");
        }
        eprintln!(
            "oracle-exp pipelines: {} PIDPipeline, {} PipingConnectors, {} FluidSystem, \
             {} fluid codes missing, {} fluid systems unmapped",
            drawing.pipelines.len(),
            drawing
                .pipelines
                .iter()
                .map(|pipeline| pipeline.connector_uids.len())
                .sum::<usize>(),
            drawing
                .pipelines
                .iter()
                .filter(|pipeline| pipeline.fluid_system.is_some())
                .count(),
            drawing.pipeline_synthesis.fluid_code_missing,
            drawing.pipeline_synthesis.fluid_system_unmapped,
        );
        if let Some(stored) = &attached.stored_values {
            eprintln!(
                "oracle-exp stored values: {} written, {} labels confirmed, {} labels kept \
                 where the dump says otherwise, {} left out unspelled, {} names left out \
                 because a label disagrees with the dump",
                stored.written,
                stored.confirmed_labels,
                stored.disagreeing_labels,
                stored.unspelled,
                stored.withheld,
            );
            for reason in &stored.unread {
                eprintln!("oracle-exp stored values: not read: {reason}");
            }
        }
        let (written, left_out) = connectivity_rel_counts(&drawing);
        eprintln!(
            "oracle-exp connectivity: {} pairs, {written} written as <Rel>, \
             {left_out} left out because an end is not a published object",
            drawing.connectivity_ends.len()
        );
        let (nozzle_rows, nozzle_rels, note_rows, note_rels) = composition_rel_counts(&drawing);
        eprintln!(
            "oracle-exp compositions: {nozzle_rows} nozzle pairs, {nozzle_rels} \
             EquipmentComponentComposition written; {note_rows} note pairs, \
             {note_rels} PBSItemNotes written"
        );
        eprintln!(
            "oracle-exp opc: {} PIDBranchPoint written",
            drawing.opc_points.len()
        );
    }
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
         {} nominal diameters left unconverted, \
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
        stats.nominal_diameter_not_emitted,
        stats.conflicting_attributes,
    );
    emit_data(options, &xml)?;
    // `_Meta.xml` is rendered once, for whichever of `--meta-out` and
    // `--out-dir` asks for it; the gate below scores that same text.
    let meta_xml = match (&options.meta_output, &options.out_dir) {
        (None, None) => None,
        (meta_path, dir) => {
            let meta_xml = write_pid_meta_xml(&drawing, &options.plant_name)
                .map_err(|e| format!("render meta XML: {e}"))?;
            if let Some(meta_path) = meta_path {
                write_file(meta_path, &meta_xml)?;
                eprintln!("Wrote {} ({} bytes).", meta_path.display(), meta_xml.len());
            }
            if let Some(dir) = dir {
                // The delivery: the two documents under the names SmartPlant's
                // own publish gives them, and the coverage report beside them.
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
            Some(meta_xml)
        }
    };
    match &options.diff_against {
        Some(ref_path) => {
            let reference = Reference::new(ref_path);
            // G5 — the `_Meta.xml` companion gate. A meta document this run
            // generates (`--out-dir` / `--meta-out`) is scored against the
            // reference's own sibling meta; a run that generates none has
            // nothing to score.
            let meta_exit = match &meta_xml {
                Some(meta_xml) => report_meta_diff(&reference, &xml, meta_xml)?,
                None => 0,
            };
            let mode = if options.oracle_exp.is_some() {
                DiffMode::OracleExp
            } else {
                DiffMode::Pid
            };
            report_pid_diff(&reference, &xml, mode, meta_exit != 0)
        }
        None => Ok(0),
    }
}

/// Write the rendered `_Data.xml` to the requested target, if any.
fn emit_data(options: &CliOptions, xml: &str) -> Result<(), String> {
    if let Some(target) = &options.output {
        match target {
            OutputTarget::Stdout => write_stdout(xml)?,
            OutputTarget::File(path) => {
                write_file(path, xml)?;
                eprintln!("Wrote {} ({} bytes).", path.display(), xml.len());
            }
        }
    }
    Ok(())
}

/// Write `text` to stdout and flush it. A write that fails (the reader of a
/// pipe is gone, the disk behind a redirect is full) comes back as
/// `write stdout: …`, which ends the run in exit 1; `print!` panics there.
fn write_stdout(text: impl std::fmt::Display) -> Result<(), String> {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{text}")
        .and_then(|()| stdout.flush())
        .map_err(|e| format!("write stdout: {e}"))
}

/// Print the refusal of a document that is not well-formed XML on stdout and
/// stderr, so a reader of either stream sees it; the exit code is 1. Stderr
/// is told even when stdout cannot be written; the run then ends in that
/// failure.
fn refuse(gate: &str, path: &Path, malformed: &MalformedPublishDocument) -> Result<i32, String> {
    let refusal = format!(
        "{gate} against {} refused: {malformed}; exit 1.",
        path.display()
    );
    let written = write_stdout(format_args!("{refusal}\n"));
    eprintln!("{refusal}");
    written.map(|()| 1)
}

/// Which input a `--diff-against` run scored, as its verdict line names it.
#[derive(Clone, Copy)]
enum DiffMode {
    /// A `.pid` read on its own.
    Pid,
    /// A `.pid` with `--oracle-exp`.
    OracleExp,
    /// An `Export.mdf` or `Export_v2.sqlite` with `--drawing`.
    Database,
}

impl DiffMode {
    fn label(self) -> &'static str {
        match self {
            DiffMode::Pid => ".pid",
            DiffMode::OracleExp => "oracle-exp",
            DiffMode::Database => "database",
        }
    }
}

/// The `--diff-against` reference `_Data.xml` of a `.pid` or database run.
/// Both gates of a run compare against its text; it is read once, by the
/// gate that comes to it first.
struct Reference<'a> {
    path: &'a Path,
    text: std::cell::OnceCell<String>,
}

impl<'a> Reference<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            text: std::cell::OnceCell::new(),
        }
    }

    /// The text of the file, read the first time it is asked for.
    fn text(&self) -> Result<&str, String> {
        if let Some(text) = self.text.get() {
            return Ok(text);
        }
        let text = std::fs::read_to_string(self.path)
            .map_err(|e| format!("read reference {}: {e}", self.path.display()))?;
        Ok(self.text.get_or_init(|| text))
    }
}

/// Print the export's object / value / rel score. An emitted object,
/// value or rel the reference does not carry exits 1, and so does
/// reference-only content the explicit exclusion list does not cover:
/// classes the `.pid` file cannot state, relation classes this line never
/// writes (and the dump's, when the run has no exp content), the plant
/// database's attribute classes and the tag-named duplicate elements are
/// excused by design. When the pair is not clean a
/// `reference-only:` accounting line and one `excused` line per excused
/// class follow the findings. A document that is not well-formed XML is
/// not scored: the refusal goes to stdout and stderr and exits 1.
///
/// [`DiffMode::OracleExp`] and [`DiffMode::Database`] narrow one excuse: a
/// class the catalog says the dump supplies (`PidTagRole::Port` /
/// `PidTagRole::Enrichment`) excused with "this run published no <class>" is
/// a finding in these modes, because the dump or the database is in the run.
/// Each such class prints
/// `oracle-exp run excused <class> ×N the dump should supply` (or
/// `database run excused <class> ×N the database should supply`) and the run
/// exits 1.
fn report_pid_diff(
    reference: &Reference,
    xml: &str,
    mode: DiffMode,
    meta_dirty: bool,
) -> Result<i32, String> {
    let ref_path = reference.path;
    let diff = match diff_pid_export(xml, reference.text()?) {
        Ok(diff) => diff,
        Err(malformed) => return refuse("Semantic diff", ref_path, &malformed),
    };
    write_stdout(&diff)?;
    // The dump or the database is in this run, so a class it should have
    // supplied is not an excuse. The `.pid`-alone mode keeps its excuse:
    // there is no dump there.
    let source = match mode {
        DiffMode::Pid => None,
        DiffMode::OracleExp => Some("dump"),
        DiffMode::Database => Some("database"),
    };
    let excused_dump_classes: Vec<(&str, usize)> = match source {
        Some(_) => diff
            .dump_supplied_excused
            .iter()
            .map(|(class, count)| (class.as_str(), *count))
            .collect(),
        None => Vec::new(),
    };
    let mode = mode.label();
    for (class, count) in &excused_dump_classes {
        let source = source.unwrap_or_default();
        write_stdout(format_args!(
            "{mode} run excused {class} ×{count} the {source} should supply\n"
        ))?;
    }
    if diff.is_clean_with_dump(source.is_some()) && !meta_dirty {
        eprintln!(
            "Semantic diff against {} is clean in {mode} mode ({} objects, {} values, {} rels).",
            ref_path.display(),
            diff.objects,
            diff.values,
            diff.rels,
        );
        Ok(0)
    } else if diff.is_clean() && excused_dump_classes.is_empty() {
        // The `_Meta.xml` gate is the only thing that found something: the run
        // is not clean, so no line may say it is.
        eprintln!(
            "Semantic diff against {} has no _Data.xml finding in {mode} mode ({} objects, \
             {} values, {} rels), but the _Meta.xml comparison did; exit 1.",
            ref_path.display(),
            diff.objects,
            diff.values,
            diff.rels,
        );
        Ok(1)
    } else if diff.is_clean() {
        // Only the dump's own classes are missing: the run published no
        // object of a class the dump it carries should have supplied.
        eprintln!(
            "Semantic diff against {} has no emitted-side finding in {mode} mode, but excused \
             {} dump-supplied class(es) the dump should supply: {}; exit 1.",
            ref_path.display(),
            excused_dump_classes.len(),
            excused_dump_classes
                .iter()
                .map(|(class, count)| format!("{class} ×{count}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        Ok(1)
    } else {
        eprintln!(
            "Semantic diff against {} surfaced findings in {mode} mode:\n  \
             objects extra={} wrong_class={} missing={}\n  \
             values different={} not_in_reference={} missing={}\n  \
             rels not_in_reference={} missing={}\n\
             exit 1.",
            ref_path.display(),
            diff.objects_not_in_reference,
            diff.objects_of_another_class,
            diff.objects_not_in_generated,
            diff.values_different,
            diff.values_not_in_reference,
            diff.values_not_in_generated,
            diff.rels_not_in_reference,
            diff.rels_not_in_generated,
        );
        Ok(1)
    }
}

/// G5 — the `<Name>_Meta.xml` that belongs beside a `<Name>_Data.xml`
/// reference. `None` when the reference is not named that way. The name is
/// the file's stored one, so an 8.3 alias of the reference still finds it.
fn meta_reference_sibling(reference: &Path) -> Option<PathBuf> {
    let stored = std::fs::canonicalize(reference).ok();
    let name = stored
        .as_deref()
        .unwrap_or(reference)
        .file_name()?
        .to_str()?;
    let cut = name.len().checked_sub("_Data.xml".len())?;
    let (stem, suffix) = (name.get(..cut)?, name.get(cut..)?);
    suffix
        .eq_ignore_ascii_case("_Data.xml")
        .then(|| reference.with_file_name(format!("{stem}_Meta.xml")))
}

/// G5 — score the generated `_Meta.xml` against the `<Name>_Meta.xml` beside
/// the `--diff-against` reference.
///
/// Returns 0 when the reference has no sibling meta document (there is
/// nothing to score, and stderr says the meta diff did not run), 1 when the
/// pair drifted or the reference meta is not well-formed XML, and 0 when
/// every compared field matches. The report goes to stdout and the verdict to
/// stderr, as the `_Data.xml` gate does.
fn report_meta_diff(reference: &Reference, data_xml: &str, meta_xml: &str) -> Result<i32, String> {
    let Some(meta_path) = meta_reference_sibling(reference.path).filter(|path| path.is_file())
    else {
        eprintln!(
            "Meta diff not run: no sibling <Name>_Meta.xml beside {}.",
            reference.path.display()
        );
        return Ok(0);
    };
    let reference_meta = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("read reference meta {}: {e}", meta_path.display()))?;
    let reference_data = reference.text()?;
    let diff = match diff_meta_xml(meta_xml, &reference_meta, data_xml, reference_data) {
        Ok(diff) => diff,
        Err(malformed) => return refuse("Meta diff", &meta_path, &malformed),
    };
    write_stdout(&diff)?;
    if diff.is_clean() {
        eprintln!(
            "Meta diff against {} matched ({} container attributes, {} blocks, {} rels); \
             accepted by registration: DocVersionDate, FilePath, the six derived UUID v5 ids.",
            meta_path.display(),
            diff.container_attributes,
            diff.blocks,
            diff.rels,
        );
        Ok(0)
    } else {
        eprintln!(
            "Meta diff against {} surfaced {} finding(s); exit 1.",
            meta_path.display(),
            diff.findings.len(),
        );
        Ok(1)
    }
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
    write_stdout(format_args!(
        "{:<34} | {:<24} | {:<22} | {:<10} | Path\n",
        "SP_ID", "Name", "DocumentCategory", "DocumentType",
    ))?;
    write_stdout(format_args!("{}\n", "-".repeat(120)))?;
    let mut count = 0usize;
    for row in rows {
        let (uid, name, cat, dtype, path) = row.map_err(|e| format!("read T_Drawing row: {e}"))?;
        write_stdout(format_args!(
            "{uid:<34} | {name:<24} | {cat:<22} | {dtype:<10} | {path}\n"
        ))?;
        count += 1;
    }
    eprintln!("\nTotal: {count} drawing(s).");
    Ok(())
}

/// Write `contents` to `path`, creating the parent directory chain
/// if needed. Pulled out so `_Data.xml` and `_Meta.xml` go through
/// the same I/O path and surface the same `format!` error messages.
/// The file takes its name once it is written whole ([`write_atomic`]).
fn write_file(path: &std::path::Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
    }
    write_atomic(path, contents.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pid_parse::publish::backup_package::PackageGateReport;
    use pid_parse::publish::DatabaseShortfalls;

    /// What a backup directory run reports when it counted `failed`
    /// failures and `short` of the written drawings were not written in
    /// full.
    fn package_report(failed: usize, short: usize) -> BackupPublishReport {
        let mut shortfalls = DatabaseShortfalls::default();
        shortfalls.placeholder_objects.insert("Exchanger".into(), 1);
        BackupPublishReport {
            failed,
            written: 2,
            index_path: PathBuf::from("_Index.json"),
            pid_entries: 0,
            shells: 0,
            gate: None,
            chain_faults: Vec::new(),
            shortfalls: (0..short)
                .map(|n| PackageDrawingShortfalls {
                    drawing_uid: format!("UID-{n}"),
                    drawing_name: format!("D{n}"),
                    shortfalls: shortfalls.clone(),
                })
                .collect(),
            duplicate_uids: Vec::new(),
            codelist_gaps: pid_parse::publish::codelist_values::CodelistGaps::default(),
        }
    }

    /// A file of the temp directory that is removed when the test ends,
    /// passing or not.
    struct TempFile(PathBuf);

    impl TempFile {
        fn new(label: &str) -> Self {
            let name = format!("pid_publish_xml-{label}-{}.xml", std::process::id());
            Self(std::env::temp_dir().join(name))
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn the_reference_is_read_once() {
        let file = TempFile::new("reference-read-once");
        std::fs::write(&file.0, "first").expect("write the reference");
        let reference = Reference::new(&file.0);
        assert_eq!(reference.text(), Ok("first"));

        // The second gate of a run gets the text the first one read, not
        // what the file holds by then, and not a read that now fails.
        std::fs::write(&file.0, "second").expect("rewrite the reference");
        assert_eq!(reference.text(), Ok("first"));
        std::fs::remove_file(&file.0).expect("remove the reference");
        assert_eq!(reference.text(), Ok("first"));
    }

    #[test]
    fn a_reference_that_cannot_be_read_says_which_file() {
        let file = TempFile::new("reference-missing");
        let reference = Reference::new(&file.0);
        let error = reference.text().expect_err("there is no file to read");
        assert!(
            error.starts_with(&format!("read reference {}: ", file.0.display())),
            "{error}"
        );
    }

    #[test]
    fn strict_turns_a_package_drawing_written_short_into_exit_one() {
        // Every drawing written in full: 0 with or without --strict.
        assert_eq!(package_exit_code(&package_report(0, 0), false), 0);
        assert_eq!(package_exit_code(&package_report(0, 0), true), 0);
        // One written short: only a warning without --strict, exit 1 with it.
        assert_eq!(package_exit_code(&package_report(0, 1), false), 0);
        assert_eq!(package_exit_code(&package_report(0, 1), true), 1);
    }

    #[test]
    fn a_package_drawing_written_short_is_warned_of_under_its_name() {
        const SENTENCE: &str =
            "1 object written as a <PIDItem> placeholder, class and attributes left out: Exchanger 1";
        assert_eq!(
            package_warnings(&package_report(0, 2)),
            [format!("D0: {SENTENCE}"), format!("D1: {SENTENCE}")]
        );
        assert!(package_warnings(&package_report(0, 0)).is_empty());
    }

    #[test]
    fn a_package_drawing_that_states_a_uid_twice_is_warned_of_and_the_exit_code_stays() {
        let mut report = package_report(0, 0);
        report.duplicate_uids = vec![PackageDuplicateUids {
            drawing_uid: "UID-0".into(),
            drawing_name: "D0".into(),
            uids: vec![("A".into(), 2), ("R".into(), 3)],
        }];
        assert_eq!(
            package_warnings(&report),
            ["D0: 2 UIDs written more than once: A ×2, R ×3"]
        );
        // Written but named: neither the exit code nor --strict counts it.
        for strict in [false, true] {
            assert_eq!(package_exit_code(&report, strict), 0);
            assert_eq!(strict_package_verdict(&report, strict), None);
        }
        // It comes after what the database line wrote short.
        let mut both = package_report(0, 1);
        both.duplicate_uids.clone_from(&report.duplicate_uids);
        assert_eq!(
            package_warnings(&both),
            [
                package_warnings(&package_report(0, 1)),
                package_warnings(&report)
            ]
            .concat()
        );
    }

    #[test]
    fn the_strict_verdict_counts_the_drawings_and_is_said_only_with_strict() {
        assert_eq!(
            strict_package_verdict(&package_report(0, 1), true).as_deref(),
            Some("--strict: 1 drawing is not written in full (see the warnings above); exit 1.")
        );
        assert_eq!(
            strict_package_verdict(&package_report(0, 2), true).as_deref(),
            Some("--strict: 2 drawings are not written in full (see the warnings above); exit 1.")
        );
        assert_eq!(strict_package_verdict(&package_report(0, 2), false), None);
        assert_eq!(strict_package_verdict(&package_report(0, 0), true), None);
    }

    #[test]
    fn a_failed_drawing_or_an_unclean_gate_is_exit_one_whatever_strict_says() {
        for strict in [false, true] {
            assert_eq!(package_exit_code(&package_report(1, 0), strict), 1);
            let mut gated = package_report(0, 0);
            gated.gate = Some(PackageGateReport {
                dirty: 1,
                ..PackageGateReport::default()
            });
            assert_eq!(package_exit_code(&gated, strict), 1);
            gated.gate = Some(PackageGateReport {
                clean: 2,
                ..PackageGateReport::default()
            });
            assert_eq!(package_exit_code(&gated, strict), 0);
        }
    }
}
