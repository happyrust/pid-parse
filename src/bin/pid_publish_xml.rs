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
use pid_parse::publish::{load_drawing_graph, write_data_xml};
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct CliOptions {
    sqlite_path: PathBuf,
    drawing_uid: String,
    output: OutputTarget,
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
         \x20               [--plant NAME]\n\n\
         --drawing UID  T_Drawing.SP_ID of the drawing to emit.\n\
         --out FILE     write the XML document to FILE (directory created).\n\
         --stdout       write the XML document to stdout instead of a file.\n\
         --plant NAME   Plant value for the <Container> root attribute\n\
         \x20              (default: \"P01\" — override with the real plant)."
    );
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 2 {
        return Err("missing <sqlite> argument".into());
    }
    let sqlite_path = PathBuf::from(&args[1]);
    let mut drawing_uid: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
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
        (Some(path), false) => OutputTarget::File(path),
        (None, true) => OutputTarget::Stdout,
        (Some(_), true) => {
            return Err("--out and --stdout are mutually exclusive".into())
        }
        (None, false) => return Err("either --out FILE or --stdout is required".into()),
    };

    Ok(CliOptions {
        sqlite_path,
        drawing_uid,
        output,
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
    if let Err(e) = run(options) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(options: CliOptions) -> Result<(), String> {
    let conn = open_readonly(&options.sqlite_path)
        .map_err(|e| format!("open {}: {e}", options.sqlite_path.display()))?;
    let graph = load_drawing_graph(&conn, &options.drawing_uid)
        .map_err(|e| format!("load drawing {}: {e}", options.drawing_uid))?;
    let xml = write_data_xml(&graph, &options.plant_name)
        .map_err(|e| format!("render XML: {e}"))?;

    eprintln!(
        "Loaded drawing `{}` ({} UID): {} objects, {} representations, {} relationships",
        graph.drawing_name,
        graph.drawing_uid,
        graph.objects.len(),
        graph.representations.len(),
        graph.relationships.len(),
    );

    match options.output {
        OutputTarget::Stdout => {
            print!("{xml}");
        }
        OutputTarget::File(path) => {
            if let Some(dir) = path.parent() {
                if !dir.as_os_str().is_empty() {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| format!("create {}: {e}", dir.display()))?;
                }
            }
            std::fs::write(&path, &xml).map_err(|e| format!("write {}: {e}", path.display()))?;
            eprintln!("Wrote {} ({} bytes).", path.display(), xml.len());
        }
    }
    Ok(())
}
