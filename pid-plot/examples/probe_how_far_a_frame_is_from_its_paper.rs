//! How far is the rectangle of a drawing's border frame from the paper its
//! sheet states?
//!
//! A sheet's `0x0114 JSheet` record states its paper, and that paper is the
//! page a drawing is written on. The border frame states a rectangle of its
//! own — the page until 2026-11-11 — and the two are not held to each other
//! by anything in the file: a paper read wrong, or read from a record that
//! is not the sheet's, would cut the page to whatever it said. So a paper is
//! taken for the page only when it lies as near the frame as the papers of
//! real drawings do (`pid_plot::model::paper_is_the_frames`), and this is
//! where "as near as real drawings" is measured.
//!
//! For every `.pid` and `.igr` it is given — each distinct file once, by its
//! bytes — it prints the paper the top-level sheets state, the frame's
//! rectangle and the frame less the paper on each side, in millimetres. A
//! template states a paper and holds no page frame; it is listed with the
//! paper alone. The last lines give how many drawings state both, how many
//! of those lie within a micrometre, the largest distance on either side
//! with the drawings that show it, and how many the limit would refuse.
//! Exit code 1 if the limit refuses a paper of any file read.
//!
//! ```text
//! cargo run -p pid-plot --example probe_how_far_a_frame_is_from_its_paper -- <file | directory> [...]
//! ```
//!
//! Over the corpora on hand — both `pid-test-data` copies (the plant backup
//! and the acceptance sheets), the shared five and the repository's
//! `test-file` — it reads 71 distinct files: 65 drawings and six templates.
//! 60 of the drawings state a paper and a frame's rectangle, and 39 of
//! those lie within a micrometre of their paper. Of the 21 that do not the
//! farthest is `N井场`, whose frame is 2.456811 mm wider and 2.706782 mm
//! taller than its paper; then the two `凝析油稳定塔` sheets (2.085102 and
//! 1.537317 mm), the sheets on `CPECCHB-SQ-A2` (0.565694 mm, eleven distinct
//! files), `O井场` and `P井场` (0.400007), `DWG-0202GP06-01`, whose frame is
//! the smaller of the two (−0.370831, two files), `DWG-0201GP06-01`
//! (0.313725), `A井场` (0.305344) and `A01` (0.001041). Five drawings state
//! a paper and no frame's rectangle — `D井场`, `Q井场`, the `换热器CD` sheet
//! in its two copies, `一级闪蒸分离器B气相出口改造` — so nothing holds their
//! paper to anything. Read-only.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::{PAPER_FRAME_REACH_MM, paper_is_the_frames};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            // A machine's own symbol library, a delivery and build output
            // hold no drawing to measure.
            let skipped = entry
                .file_name()
                .is_some_and(|name| name == "symbols-full" || name == "plot" || name == "target");
            if !skipped {
                collect(&entry, out);
            }
        }
    } else if path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("pid") || extension.eq_ignore_ascii_case("igr")
    }) {
        out.push(path.to_path_buf());
    }
}

/// The paper every top-level sheet that states one agrees on, millimetres.
fn stated_paper_mm(doc: &pid_parse::PidDocument) -> Option<[f64; 2]> {
    let millimetres = |metres: f64| (metres * 1e9).round() / 1e6;
    let mut papers = doc
        .sheet_display_order
        .values()
        .filter_map(|order| order.paper)
        .map(|paper| [millimetres(paper.width_m), millimetres(paper.height_m)]);
    let first = papers.next()?;
    papers.all(|paper| paper == first).then_some(first)
}

fn main() -> ExitCode {
    let mut files = Vec::new();
    for arg in std::env::args().skip(1) {
        collect(Path::new(&arg), &mut files);
    }
    if files.is_empty() {
        eprintln!("usage: probe_how_far_a_frame_is_from_its_paper <file | directory> [...]");
        return ExitCode::from(2);
    }
    let mut seen = BTreeSet::new();
    let (mut drawings, mut both, mut within_a_micrometre, mut refused) =
        (0usize, 0usize, 0usize, 0usize);
    // (distance, name, frame less paper) for every drawing off its paper.
    let mut off: Vec<(f64, String, [f64; 2])> = Vec::new();
    println!("file | paper mm | frame mm | frame less paper mm");
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        if !seen.insert((bytes.len(), hasher.finish())) {
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let doc = match pid_parse::PidParser::new().parse_file(path) {
            Ok(doc) => doc,
            Err(error) => {
                println!("{name} | does not parse: {error}");
                continue;
            }
        };
        let template = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("igr"));
        let paper = stated_paper_mm(&doc);
        let frame = pid_parse::build_normalized_geometry(&doc)
            .page_dimensions_mm
            .map(|(width, height)| [width, height]);
        let side = |value: Option<[f64; 2]>| {
            value.map_or_else(
                || "none".to_string(),
                |value| format!("{:.6} x {:.6}", value[0], value[1]),
            )
        };
        if !template {
            drawings += 1;
        }
        match (paper, frame) {
            (Some(paper), Some(frame)) => {
                let apart = [frame[0] - paper[0], frame[1] - paper[1]];
                let distance = apart[0].abs().max(apart[1].abs());
                println!(
                    "{name} | {} | {} | ({:+.6}, {:+.6}){}",
                    side(Some(paper)),
                    side(Some(frame)),
                    apart[0],
                    apart[1],
                    if paper_is_the_frames(paper, frame) {
                        ""
                    } else {
                        "  <- refused"
                    }
                );
                both += 1;
                if distance <= 0.001 {
                    within_a_micrometre += 1;
                } else {
                    off.push((distance, name.clone(), apart));
                }
                if !paper_is_the_frames(paper, frame) {
                    refused += 1;
                }
            }
            _ => println!("{name} | {} | {} | -", side(paper), side(frame)),
        }
    }
    off.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!(
        "\nin all: {} distinct file(s), {drawings} drawing(s); {both} state a paper and a frame, \
         {within_a_micrometre} of them within a micrometre of each other",
        seen.len()
    );
    println!("the {} that are not, farthest first:", off.len());
    for (distance, name, apart) in &off {
        println!(
            "  {distance:.6} mm  ({:+.6}, {:+.6})  {name}",
            apart[0], apart[1]
        );
    }
    println!(
        "the limit is {PAPER_FRAME_REACH_MM} mm on either side; it refuses {refused} of the {both}"
    );
    if refused == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
