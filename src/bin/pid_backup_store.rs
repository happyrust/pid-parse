//! CLI: build the Backup Store of one Plant Backup -- the SQLite file
//! holding its file list, its `Manifest.txt` line by line and the
//! tables of its Database Dump (plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`,
//! Q17; ADR-0004).
//!
//! Usage:
//!
//! ```text
//! pid_backup_store <Plant Backup> -o <store.sqlite> [--force] [--keep-secrets] [--embed-files]
//! ```
//!
//! `<Plant Backup>` is the `<Plant>_p.zip` `SmartPlant` wrote or the
//! directory it unpacks to. The store is written to `<store>.tmp` and
//! renamed into place when complete; an existing output is refused
//! unless `--force` is given. By default the Manifest's credentials
//! are replaced by their SHA-256 (`--keep-secrets` keeps them) and
//! file bytes are not stored (`--embed-files` stores them).
//!
//! On success the summary goes to stdout -- files, Manifest lines and
//! redactions, dumped tables, rows, Ghost Rows and LOBs -- and every
//! warning to stderr as `warning: ...`.
//!
//! Exit codes: 0 = success, 1 = the input could not be read or the
//! store not written (an existing output without `--force` included),
//! 2 = usage error.

use std::path::PathBuf;

use pid_parse::backup::{build_backup_store, StoreOptions, StoreSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    input: PathBuf,
    out: PathBuf,
    force: bool,
    store: StoreOptions,
}

const USAGE: &str = "Usage: pid_backup_store <Plant Backup> -o <store.sqlite> [--force] [--keep-secrets] [--embed-files]

Builds the Backup Store of one SmartPlant P&ID Plant Backup: one SQLite
file with the backup's file list (backup_file), its Manifest.txt line by
line (manifest_line / manifest_field and views) and the tables of its
Database Dump (<role>__<table>, dump_table, dump_column, ...).

<Plant Backup>   the <Plant>_p.zip SmartPlant wrote, or the directory it
                 unpacks to (top-level files only)
-o, --out        the SQLite file to write; written as <store>.tmp first
                 and renamed into place when complete
--force          replace an existing output (refused otherwise)
--keep-secrets   keep the Manifest's credentials as written instead of
                 replacing them with their SHA-256
--embed-files    store every file's bytes in backup_file_content

Exit codes: 0 success, 1 the input could not be read or the store not
written, 2 usage error.";

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut force = false;
    let mut store = StoreOptions::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{} requires a file path", args[i]))?;
                if out.replace(PathBuf::from(value)).is_some() {
                    return Err("-o given twice".to_string());
                }
                i += 2;
            }
            "--force" => {
                force = true;
                i += 1;
            }
            "--keep-secrets" => {
                store.keep_secrets = true;
                i += 1;
            }
            "--embed-files" => {
                store.embed_files = true;
                i += 1;
            }
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown flag: {flag}"));
            }
            positional => {
                if let Some(first) = &input {
                    return Err(format!(
                        "more than one input given ({} and {positional})",
                        first.display()
                    ));
                }
                input = Some(PathBuf::from(positional));
                i += 1;
            }
        }
    }
    let input = input.ok_or_else(|| "missing the Plant Backup to read".to_string())?;
    let out = out.ok_or_else(|| "-o <store.sqlite> is required".to_string())?;
    Ok(CliOptions {
        input,
        out,
        force,
        store,
    })
}

/// The summary as the command prints it.
fn render_summary(out: &std::path::Path, summary: &StoreSummary) -> String {
    format!(
        "{}: Backup Store written\n  files           {} ({} embedded)\n  manifest lines  {}, redactions {}\n  dump tables     {}, rows {}, ghost rows {}, LOBs {}\n  warnings        {}\n",
        out.display(),
        summary.files,
        summary.files_embedded,
        summary.manifest_lines,
        summary.redactions,
        summary.tables,
        summary.rows,
        summary.ghost_rows,
        summary.lobs,
        summary.warnings.len(),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().skip(1).any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        std::process::exit(0);
    }
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("argument error: {err}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(err) = run(&options) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(options: &CliOptions) -> Result<(), String> {
    if options.out.exists() && !options.force {
        return Err(format!(
            "{} already exists; pass --force to replace it",
            options.out.display()
        ));
    }
    let summary = build_backup_store(&options.input, &options.out, &options.store)
        .map_err(|err| err.to_string())?;
    for warning in &summary.warnings {
        eprintln!("warning: {warning}");
    }
    print!("{}", render_summary(&options.out, &summary));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("pid_backup_store")
            .chain(list.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn the_input_and_the_output_are_required_and_the_flags_optional() {
        let parsed = parse_args(&args(&["TEST02_p.zip", "-o", "store.sqlite"])).unwrap();
        assert_eq!(
            CliOptions {
                input: PathBuf::from("TEST02_p.zip"),
                out: PathBuf::from("store.sqlite"),
                force: false,
                store: StoreOptions::default(),
            },
            parsed
        );
        let parsed = parse_args(&args(&[
            "--force",
            "--out",
            "store.sqlite",
            "--keep-secrets",
            "backup-dir",
            "--embed-files",
        ]))
        .unwrap();
        assert_eq!(
            CliOptions {
                input: PathBuf::from("backup-dir"),
                out: PathBuf::from("store.sqlite"),
                force: true,
                store: StoreOptions {
                    keep_secrets: true,
                    embed_files: true,
                },
            },
            parsed
        );
    }

    #[test]
    fn usage_errors_name_what_is_wrong() {
        assert_eq!(
            Err("-o <store.sqlite> is required".to_string()),
            parse_args(&args(&["TEST02_p.zip"]))
        );
        assert_eq!(
            Err("missing the Plant Backup to read".to_string()),
            parse_args(&args(&["-o", "store.sqlite"]))
        );
        assert_eq!(
            Err("-o requires a file path".to_string()),
            parse_args(&args(&["TEST02_p.zip", "-o"]))
        );
        assert_eq!(
            Err("unknown flag: --fast".to_string()),
            parse_args(&args(&["TEST02_p.zip", "-o", "s.sqlite", "--fast"]))
        );
        assert_eq!(
            Err("more than one input given (a.zip and b.zip)".to_string()),
            parse_args(&args(&["a.zip", "b.zip", "-o", "s.sqlite"]))
        );
        assert_eq!(
            Err("-o given twice".to_string()),
            parse_args(&args(&["a.zip", "-o", "s.sqlite", "-o", "t.sqlite"]))
        );
    }

    #[test]
    fn the_summary_lists_every_count() {
        let summary = StoreSummary {
            files: 1_554,
            files_embedded: 0,
            manifest_lines: 319,
            redactions: 14,
            tables: 154,
            rows: 37_470,
            ghost_rows: 5,
            lobs: 4,
            warnings: vec!["note".to_string()],
        };
        assert_eq!(
            "store.sqlite: Backup Store written\n  files           1554 (0 embedded)\n  manifest lines  319, redactions 14\n  dump tables     154, rows 37470, ghost rows 5, LOBs 4\n  warnings        1\n",
            render_summary(std::path::Path::new("store.sqlite"), &summary)
        );
    }
}
