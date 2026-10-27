//! Which border template did each sheet wear — and what did that template place?
//!
//! `pid_plot::model` picks a sheet's furniture by extent: every `.igr` under the
//! corpus's template directories is parsed, its stroked extent measured, and the
//! one whose extent is the page is the template the sheet was drawn on. This
//! probe prints, per drawing, the file that won that pick and every symbol the
//! file places, so a change in either is visible as a list rather than as a
//! number that moved.
//!
//! It is the probe that settled two questions the counts alone cannot. Two
//! templates of one size (`CPE-SQ-A1.igr` at `841.0000000000001` mm,
//! `CPECCHB-SQ-A1.igr` at `841.0000000003785`) differ only in the last bits of
//! their `f64` metres, so which one is nearer a page stating `841.0000500894253`
//! is an accident of noise — and the pick now reads the sheet's own link record
//! before it reads that. And what a template places is not one symbol per sheet
//! either: `CPECCHB-SQ-A1.igr` places none and `CPECCHB-SQ-A2.igr` places three.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_template_each_sheet_wore -- <a.pid|dir> [more ...]
//! ```
//!
//! Read-only: layout only, no PDF is written. The corpus is
//! `PID_PARSE_SELF_CHECK_DIR`, or `D:\work\cad\PID自研软件验证`.

use std::path::{Path, PathBuf};

use pid_plot::model::lay_out_drawing;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
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

/// The file name a stored path ends in, for a one-line placement report.
fn leaf_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn main() {
    let corpus = corpus_dir();
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(&arg);
        if path.is_dir() {
            collect_pids(&path, &mut files);
        } else {
            files.push(path);
        }
    }
    if files.is_empty() {
        eprintln!("usage: probe_which_template_each_sheet_wore <a.pid|dir> [more ...]");
        std::process::exit(2);
    }

    let mut templated = 0usize;
    let mut placements = 0usize;
    for path in &files {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let model = match lay_out_drawing(path, &corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("ERR  {error}  {name}");
                continue;
            }
        };
        let page = model.resolved_page_mm().unwrap_or([0.0, 0.0]);
        let Some(template) = model.template.as_ref() else {
            println!("none {:8.2} x {:<8.2} {name}", page[0], page[1]);
            continue;
        };
        templated += 1;
        println!(
            "TMPL {:8.2} x {:<8.2} {} -> {} extent {:.2} x {:.2} primitives {}",
            page[0],
            page[1],
            name,
            leaf_name(&template.path),
            template.extent_mm[0],
            template.extent_mm[1],
            template.primitives
        );
        for symbol in &template.symbols {
            placements += 1;
            println!(
                "       places {:?} at ({:.3}, {:.3}) mm from {:?}, library {} ({:?}), ink {:?}",
                leaf_name(&symbol.key),
                symbol.insertion_mm[0],
                symbol.insertion_mm[1],
                symbol.body,
                symbol.library_available,
                symbol.name_gap,
                symbol.extent_mm
            );
        }
    }
    println!(
        "\n{} files: {templated} wore a template, {placements} placements",
        files.len()
    );
}
