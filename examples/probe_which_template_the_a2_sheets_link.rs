//! Does an A2 sheet link a border template — and if so, what is it called?
//!
//! `2026-10-23-the-border-template-draws-the-sheet-furniture.md` resolves a
//! sheet's furniture by extent: every `.igr` under the corpus's template
//! directories is parsed and the one whose stroked extent *is* the page is the
//! template ([`pid_plot::model`]). The corpus ships `CPE-SQ-A0.igr` and
//! `CPE-SQ-A1.igr` only, so every A2 sheet in the plant backup draws no
//! furniture at all.
//!
//! Two different findings wear the same outward face, and this probe separates
//! them:
//!
//! * the sheet links an `.igr` this machine does not have — then the missing
//!   file has a name, and the fix is to obtain it; or
//! * no `.igr` is linked at all — then the sheet was authored without a frame,
//!   drawing none is correct, and nothing is missing.
//!
//! The link record is the only evidence either way: the drawing's own geometry
//! carries no furniture (that is why the template is needed), and the page size
//! alone cannot tell a framed A2 from an unframed one.
//!
//! ```text
//! cargo run --example probe_which_template_the_a2_sheets_link -- <a.pid|dir> [more ...]
//! ```

use pid_parse::{build_normalized_geometry, ParseOptions, PidParser};
use std::path::{Path, PathBuf};

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

fn main() {
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
        eprintln!("usage: probe_which_template_the_a2_sheets_link <a.pid|dir> [more ...]");
        return;
    }

    let mut igr_linked = 0usize;
    let mut igr_unlinked = 0usize;
    let mut no_page = 0usize;
    for path in &files {
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                println!("{}\n  parse failed: {error}", path.display());
                continue;
            }
        };
        let page = build_normalized_geometry(&doc).page_dimensions_mm;
        let frames: Vec<String> = doc
            .sheet_streams
            .iter()
            .filter_map(|s| s.geometry.as_ref())
            .flat_map(|g| g.decoded_igsmartframes.iter())
            .map(|frame| {
                format!(
                    "state={:?} {:.2}x{:.2}mm",
                    frame.state,
                    frame.extent_width_m * 1000.0,
                    frame.extent_height_m * 1000.0
                )
            })
            .collect();
        // Every OLE link the document states, `igr` or not: a sheet that links
        // some *other* kind of object is a different reading than a bare one.
        let mut links: Vec<&str> = doc
            .jsites
            .iter()
            .flat_map(|site| site.ole_links.iter())
            .map(String::as_str)
            .collect();
        links.sort_unstable();
        links.dedup();
        let igr: Vec<&str> = links
            .iter()
            .copied()
            .filter(|link| link.to_lowercase().contains(".igr"))
            .collect();

        let sheet = match page {
            Some((width, height)) => {
                let (short, long) = if width <= height {
                    (width, height)
                } else {
                    (height, width)
                };
                match (short.round() as i64, long.round() as i64) {
                    (594, 841) => "A1",
                    (841, 1189) => "A0",
                    (420, 594) => "A2",
                    (297, 420) => "A3",
                    _ => "other",
                }
            }
            None => "no page",
        };
        match (page, igr.is_empty()) {
            (None, _) => no_page += 1,
            (Some(_), false) => igr_linked += 1,
            (Some(_), true) => igr_unlinked += 1,
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        println!(
            "{sheet:<4} page={:<16} frames=[{}] igr={:?} links={linkcount}  {name}",
            page.map_or("none".to_string(), |(w, h)| format!("{w:.2}x{h:.2}")),
            frames.join("; "),
            igr,
            linkcount = links.len(),
        );
    }

    println!(
        "\n{} files: {igr_linked} link an .igr, {igr_unlinked} link none, {no_page} declare no page",
        files.len()
    );
}
