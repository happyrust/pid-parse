//! Which style entries do these pages draw with that no page with a reference
//! plot has ever exercised?
//!
//! Most of the corpus's sheets have no plot of their own to be compared
//! against, so "is this page right?" cannot be answered page by page. It can be
//! answered per *style entry*: a pen width, a dash pattern, a face and a border
//! template are each either one a validated page also uses — in which case the
//! machinery that turned it into ink has been checked against a real plot — or
//! they are not, and then only that entry is in doubt.
//!
//! A validated page is a `.pid` with a plot of its own beside it, taken from
//! `PID_ACCEPTANCE_CORPUS` (the eleven acceptance sheets), not from the drawing
//! corpus the targets are laid out against. That split is deliberate: a page
//! used as a witness certifies its vocabulary, so a page that is itself wrong
//! would certify the wrong thing, and the acceptance directory is the one the
//! reference-plot gate already holds to a reading. Run that gate first — if a
//! page's comparison fails, move its plot out of the directory before running
//! this.
//!
//! ```text
//! cargo run -p pid-plot --example probe_style_witness -- <a.pid|dir> [more ...]
//! ```
//!
//! The witness directory is `PID_ACCEPTANCE_CORPUS`, or
//! `D:\work\cad\pid-test-data\验收数据`; witness sheets are laid out against that
//! directory. Target sheets are laid out against `PID_PARSE_SELF_CHECK_DIR`, or
//! `D:\work\cad\PID自研软件验证`. Read-only: layout only, no PDF is written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::witness::{Dimension, Witness};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// Where the reference plots that certify a vocabulary live.
fn witness_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

/// The sheets of `paths` that have a plot of their own beside them.
///
/// Laid out against `corpus`, which for a witness is the acceptance directory
/// itself: that is the package those drawings name their templates from.
fn validated(paths: &[PathBuf], corpus: &Path) -> Vec<(String, PaperModel)> {
    let mut out = Vec::new();
    for path in paths {
        if !path.with_extension("pdf").is_file() {
            continue;
        }
        match lay_out_drawing(path, corpus) {
            Ok(model) => out.push((stem(path), model)),
            Err(error) => println!("FAIL {}: {error}", path.display()),
        }
    }
    out
}

fn main() {
    let corpus = corpus_dir();
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            collect_pids(&path, &mut paths);
        } else {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        eprintln!("usage: probe_style_witness <a.pid|dir> [more ...]");
        std::process::exit(2);
    }

    // The witness set is every drawing in the acceptance directory that has a
    // plot beside it. Targets stay on `corpus`: a backup sheet resolves its
    // own package, and must not inherit the acceptance directory's search root.
    let witness_root = witness_dir();
    let mut witness_paths: Vec<PathBuf> = Vec::new();
    collect_pids(&witness_root, &mut witness_paths);
    let validated_sheets = validated(&witness_paths, &witness_root);
    if validated_sheets.is_empty() {
        eprintln!(
            "{}: no drawing with a plot beside it, so nothing is witnessed",
            witness_root.display()
        );
        std::process::exit(2);
    }
    let witness = Witness::of(
        validated_sheets
            .iter()
            .map(|(name, model)| (name.as_str(), model)),
    );
    println!(
        "witnessed by {} page(s) with a plot of their own:",
        validated_sheets.len()
    );
    for dimension in [
        Dimension::PenWidth,
        Dimension::DashPattern,
        Dimension::Face,
        Dimension::Template,
    ] {
        println!(
            "   {dimension:<13} {}",
            witness.values(dimension).join(", ")
        );
    }

    let mut targets: Vec<(String, PaperModel)> = Vec::new();
    for path in &paths {
        match lay_out_drawing(path, &corpus) {
            Ok(model) => targets.push((stem(path), model)),
            Err(error) => println!("FAIL {}: {error}", path.display()),
        }
    }
    let report = witness.unwitnessed(targets.iter().map(|(name, model)| (name.as_str(), model)));

    println!(
        "\n{} sheet(s) checked; {} draw with witnessed vocabulary alone",
        report.sheets_checked, report.sheets_clean
    );
    if report.entries.is_empty() {
        println!("nothing is unexercised");
        return;
    }
    for dimension in [
        Dimension::PenWidth,
        Dimension::DashPattern,
        Dimension::Face,
        Dimension::Template,
    ] {
        let entries: Vec<_> = report.of_dimension(dimension).collect();
        if entries.is_empty() {
            continue;
        }
        println!(
            "\n{dimension} — {} entry(ies) no validated page uses",
            entries.len()
        );
        for entry in entries {
            println!(
                "   {:<24} {:>5} drawing / {:>5} furniture use(s) on {} sheet(s)",
                entry.value,
                entry.drawing,
                entry.furniture,
                entry.sheets.len()
            );
            let sheets: BTreeSet<&str> = entry.sheets.iter().map(String::as_str).collect();
            if sheets.len() <= 6 {
                println!(
                    "      {}",
                    sheets.into_iter().collect::<Vec<_>>().join(", ")
                );
            }
        }
    }
    let affected = report.sheets_affected();
    println!(
        "\n{} sheet(s) carry at least one unexercised entry; the drawing's own ink is {}",
        affected.len(),
        if report.drawing_is_witnessed() {
            "witnessed on every one of them".to_string()
        } else {
            format!(
                "unexercised on {} entry(ies)",
                report
                    .entries
                    .iter()
                    .filter(|entry| entry.drawing > 0)
                    .count()
            )
        }
    );
}
