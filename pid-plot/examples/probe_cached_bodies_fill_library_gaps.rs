//! Report, per drawing, how many symbol gaps a drawing's own cached bodies fill.
//!
//! `lay_out_drawing` prefers the body a `.pid` carries over the one in a
//! `.sym` library, so a placement can draw fine while still being counted as a
//! library gap. `PaperModel::missing_symbols` is the set the library could not
//! answer; `missing_boxes` is the set nothing could draw — the drawing has no
//! cached body either, so the page shows a placeholder box.
//!
//! ```text
//! cargo run -p pid-plot --example probe_cached_bodies_fill_library_gaps -- <a.pid> [b.pid ...]
//! ```
//!
//! Prints `primitives`, `library_gaps`, `undrawn` and the undrawn names per
//! drawing, then a total. Exit 1 when a drawing cannot be laid out at all.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::lay_out_drawing;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let mut drawings: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if drawings.is_empty() {
        eprintln!("usage: probe_cached_bodies_fill_library_gaps <drawing.pid | dir> ...");
        return ExitCode::from(2);
    }
    let mut expanded = Vec::new();
    for path in drawings.drain(..) {
        if path.is_dir() {
            collect(&path, &mut expanded);
        } else {
            expanded.push(path);
        }
    }
    let corpus = corpus_dir();
    let (mut primitives, mut gaps, mut undrawn) = (0usize, 0usize, 0usize);
    for path in &expanded {
        match lay_out_drawing(path, &corpus) {
            Ok(model) => {
                primitives += model.primitives.len();
                gaps += model.missing_symbols.len();
                undrawn += model.missing_boxes.len();
                println!(
                    "{} primitives={} library_gaps={} undrawn={}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    model.primitives.len(),
                    model.missing_symbols.len(),
                    model.missing_boxes.len()
                );
                for key in &model.missing_symbols {
                    println!("    gap  {key}");
                }
                for key in &model.missing_boxes {
                    println!("    none {key:?}");
                }
            }
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(1);
            }
        }
    }
    println!(
        "{} drawings: primitives={primitives} library_gaps={gaps} undrawn={undrawn}",
        expanded.len()
    );
    ExitCode::SUCCESS
}
