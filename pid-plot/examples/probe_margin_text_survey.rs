//! What lettering do the five reference plots carry along their page edges,
//! and where does each run start?
//!
//! A sheet's sign-off column and title block are lettering the drawing does
//! not own, so a comparison masks them. The rectangles in
//! `assets/template-masks.json` were drawn around the text origins a reference
//! plot prints there (`docs/analysis/2026-10-12-template-mask-samples.md`), and
//! this prints those origins: per page, how many runs start within
//! [`EDGE_MM`] of an edge, then every run that starts in the left, bottom or
//! right band, as `x y text` in paper millimetres (origin bottom-left, y up),
//! sorted by `x` then `y`.
//!
//! The bands reach past the furniture: on these A0 and A1 sheets the sign-off
//! column ends at 75 mm and the title block is 90 mm high and at most 171 mm
//! wide. The listing therefore also shows the drawing's own labels beside
//! them, which are the ones a rectangle has to leave outside.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_margin_text_survey
//! ```
//!
//! Read-only. The plots are `<name>.pdf` for the five self-check drawings under
//! `PID_PARSE_SELF_CHECK_DIR`; when one of them is absent this names it and
//! exits non-zero without surveying any.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::read::read_plot;

const DRAWINGS: [&str; 5] = [
    "I井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
];

/// How near a page edge a run has to start to count as an edge run,
/// millimetres.
const EDGE_MM: f64 = 55.0;
/// Width of the left band, millimetres: past the 75 mm sign-off column.
const LEFT_BAND_MM: f64 = 90.0;
/// Height of the bottom band, millimetres: past the 90 mm title block.
const BOTTOM_BAND_MM: f64 = 120.0;
/// Width of the right band, millimetres: past the title block's 171 mm.
const RIGHT_BAND_MM: f64 = 220.0;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn main() -> ExitCode {
    match survey(&corpus_dir()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("no margin-text survey; {why}");
            ExitCode::FAILURE
        }
    }
}

/// Print the survey of every reference plot under `corpus`.
///
/// All five plots are looked for before the first is read, so a corpus that
/// lacks one prints no partial survey.
fn survey(corpus: &Path) -> Result<(), String> {
    let plots: Vec<PathBuf> = DRAWINGS
        .iter()
        .map(|name| corpus.join(format!("{name}.pdf")))
        .collect();
    if let Some(absent) = plots.iter().find(|plot| !plot.is_file()) {
        return Err(format!("{} is absent", absent.display()));
    }
    for (name, plot) in DRAWINGS.iter().zip(&plots) {
        let bytes = std::fs::read(plot).map_err(|error| format!("{}: {error}", plot.display()))?;
        let ink = read_plot(&bytes).map_err(|error| format!("{}: {error}", plot.display()))?;
        let [page_w, page_h] = ink.page_mm;
        println!("PAGE {name} {page_w:.1} x {page_h:.1}");
        let edge = ink
            .spans
            .iter()
            .filter(|span| {
                let [x, y] = span.origin_mm;
                x.min(page_w - x).min(y).min(page_h - y) <= EDGE_MM
            })
            .count();
        println!("  edge spans {edge}");
        let mut band: Vec<(&str, [f64; 2])> = ink
            .spans
            .iter()
            .filter(|span| {
                let [x, y] = span.origin_mm;
                x < LEFT_BAND_MM || y < BOTTOM_BAND_MM || x > page_w - RIGHT_BAND_MM
            })
            .map(|span| (span.text.trim(), span.origin_mm))
            .collect();
        band.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.1[1].total_cmp(&b.1[1])));
        println!("  band spans {}", band.len());
        for (text, [x, y]) in &band {
            println!("  {x:8.2} {y:8.2}  {text}");
        }
    }
    Ok(())
}
