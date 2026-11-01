//! The plant backup's style vocabulary, against the only pages a plot can be
//! compared with.
//!
//! Forty-nine of the corpus's fifty-three plant sheets have no reference plot,
//! so no page-by-page verdict is possible for them. What *is* possible — and is
//! what this gate pins — is a per-entry verdict: every pen width, dash pattern,
//! face and border template a sheet draws with is either one of the five
//! validated pages' or it is not, and the ones that are not are named here.
//!
//! The lists below are the *result* of that decomposition, and each one is
//! short enough to settle by hand:
//!
//! - **Pen widths.** Eight widths, of which seven are furniture of the two
//!   `CPECCHB` templates and the A2 file, which four to nine sheets each wear. The
//!   eighth, `0.5000`, is the one on the drawing's own ink: `D04` draws 24
//!   strokes with it. That matters because the pen table is a three-band
//!   mapping measured from the reference plots: an unexercised width is only in
//!   doubt if it sits inside a band boundary the references leave open. Six of
//!   the eight are far above the thick boundary and print thick wherever it
//!   lies; `0.3000` and `0.5000` are the two that are not — see the note at
//!   [`WIDENED_BY_THE_BOUNDARY`].
//! - **Dash patterns.** Both are the drawing's own ink, and both are resolved
//!   without a reference plot: `0.000` is a single zero-length step, which
//!   `write::dash_array` draws solid on purpose, and `3.500/1.750` is a
//!   two-step pattern whose record literally stores `-3.500/-1.750` — the same
//!   shape as the witnessed `-1.905/-1.270`, read by the same code.
//! - **Faces.** None: every sheet letters with the four faces the validated
//!   pages already use.
//! - **Templates.** Three files no validated page wears — `CPE-SQ-A2` and both
//!   `CPECCHB` — which is the real reason the seven widths above are
//!   unexercised rather than a separate problem.
//!
//! `32` of the fifty-three sheets draw with witnessed vocabulary alone, and
//! `21` carry at least one entry from the lists below.
//!
//! Soft-skips when the corpus or the extracted backup is absent, which is the
//! case on CI.
//!
//! ```text
//! PID_TEST_DATA_DIR=D:\work\plant-code\cad\pid-test-data \
//!   cargo test -p pid-plot --test style_witness --locked
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::witness::{Dimension, Witness};

const PEN_WIDTHS: [&str; 6] = ["0.0100", "0.1300", "0.1800", "0.2500", "0.3500", "0.7000"];
const DASH_PATTERNS: [&str; 2] = ["1.905/1.270", "7.000/1.750/0.350/1.750"];
const FACES: [&str; 4] = ["Arial", "Arial Narrow", "FangSong", "SimSun"];
const TEMPLATES: [&str; 2] = ["CPE-SQ-A0.igr", "CPE-SQ-A1.igr"];

/// The two unexercised widths whose pens the references cannot settle.
///
/// The pen table snaps a declared width onto three plotter pens, and the
/// references bound each boundary into an open interval, not onto a value:
///
/// - **thin/mid `(0.25, 0.35)`** — their 0.25 mm rules print thin and their
///   0.35 mm strokes print mid, 2807 straight symbol strokes agreeing — so
///   `0.3000` could be either.
/// - **mid/thick `(0.35, 0.70)`** — 0.35 prints mid and 0.70 prints thick, and
///   nothing in between is exercised by a real plot. `0.5000` sits inside it,
///   so it could be either too. This boundary used to be written as
///   `(0.50, 0.70)` on the strength of a 0.50 mm symbol stroke printing mid;
///   that anchor was the drawing's `As Drawn` placeholder, whose 0.50 prints
///   thin on the reference (see
///   `docs/analysis/2026-10-30-as-drawn-is-not-a-pen.md`), so it never anchored
///   anything.
///
/// The six remaining unexercised widths are all past 0.70 and print thick
/// wherever either boundary lies.
const WIDENED_BY_THE_BOUNDARY: [&str; 2] = ["0.3000", "0.5000"];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// Where the shelled-out plant backup sits, if this machine has one.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(corpus_dir());
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
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

fn sheets(paths: &[PathBuf]) -> Vec<(String, PaperModel)> {
    let corpus = corpus_dir();
    paths
        .iter()
        .map(|path| {
            let model = lay_out_drawing(path, &corpus)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            (stem(path), model)
        })
        .collect()
}

/// The five drawings that have a plot of their own beside them.
fn validated_sheets(corpus: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_pids(corpus, &mut paths);
    paths.retain(|path| path.with_extension("pdf").is_file());
    paths
}

#[test]
fn the_plant_backup_draws_only_vocabulary_a_reference_plot_has_exercised() {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the style-witness gate; no extracted plant backup here");
        return;
    };
    let witness_paths = validated_sheets(&corpus);
    if witness_paths.is_empty() {
        eprintln!(
            "skipping the style-witness gate; {} carries no drawing with a plot beside it",
            corpus.display()
        );
        return;
    }
    let mut target_paths = Vec::new();
    collect_pids(&backup, &mut target_paths);
    assert!(
        !target_paths.is_empty(),
        "{} carries no .pid",
        backup.display()
    );

    let witnessed = sheets(&witness_paths);
    let witness = Witness::of(witnessed.iter().map(|(name, model)| (name.as_str(), model)));
    // What the validated pages exercise is the floor this gate rests on: if a
    // plot or a drawing goes missing, the witness set shrinks and the
    // unexercised lists below grow, so pin it too.
    assert_eq!(
        witness.values(Dimension::PenWidth),
        PEN_WIDTHS,
        "the declared widths the five reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::DashPattern),
        DASH_PATTERNS,
        "the dash patterns the five reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::Face),
        FACES,
        "the faces the five reference pages letter with"
    );
    assert_eq!(
        witness.values(Dimension::Template),
        TEMPLATES,
        "the templates the five reference pages wear"
    );

    let targets = sheets(&target_paths);
    let report = witness.unwitnessed(targets.iter().map(|(name, model)| (name.as_str(), model)));

    // Every unexercised entry, by dimension, with the drawing/furniture split.
    let mut widths: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for entry in report.of_dimension(Dimension::PenWidth) {
        widths.insert(&entry.value, (entry.drawing, entry.furniture));
    }
    assert_eq!(
        widths.keys().copied().collect::<Vec<_>>(),
        [
            "0.3000", "0.5000", "0.7071", "0.7500", "0.7999", "1.0606", "1.1313", "1.4141"
        ],
        "the pen widths no reference page draws with"
    );
    // The load-bearing half of the decomposition: the pen table is measured
    // from the references, so an unexercised width on the drawing's own ink is
    // a process line whose weight nothing has checked. Seven of the eight are
    // furniture; `0.5000` is the one that is not, and it is also one of the two
    // that sit inside a boundary the references leave open.
    for (width, (drawing, furniture)) in &widths {
        if *width == "0.5000" {
            continue;
        }
        assert_eq!(
            *drawing, 0,
            "{width} mm is used by the drawing's own ink, which no reference page exercises"
        );
        assert!(*furniture > 0, "{width} mm is listed with no furniture use");
    }
    assert_eq!(
        widths["0.5000"],
        (24, 16),
        "`0.5000` mm on D04's own ink and on four sheets' furniture"
    );
    for (width, expected) in [("0.3000", (0, 32)), ("0.5000", (24, 16))] {
        assert_eq!(
            widths[width], expected,
            "{width} mm is an unexercised width inside a band boundary the \
             references leave open; its stroke count is what a tighter boundary \
             would re-pen"
        );
    }
    assert_eq!(
        WIDENED_BY_THE_BOUNDARY.len(),
        2,
        "two boundaries, two widths the references cannot settle"
    );

    let mut dashes: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for entry in report.of_dimension(Dimension::DashPattern) {
        dashes.insert(&entry.value, (entry.drawing, entry.furniture));
    }
    assert_eq!(
        dashes.keys().copied().collect::<Vec<_>>(),
        ["0.000", "3.500/1.750"],
        "the dash patterns no reference page draws with"
    );
    assert_eq!(
        dashes["0.000"].0, 10,
        "the zero-length pattern's strokes; the writer draws it solid"
    );
    assert_eq!(
        dashes["3.500/1.750"].0, 459,
        "the two-step pattern's strokes, read from a record that stores -3.500/-1.750"
    );

    assert_eq!(
        report
            .of_dimension(Dimension::Face)
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>(),
        Vec::<&str>::new(),
        "every sheet letters with a face a reference page already uses"
    );
    assert_eq!(
        report
            .of_dimension(Dimension::Template)
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>(),
        ["CPE-SQ-A2.igr", "CPECCHB-SQ-A1.igr", "CPECCHB-SQ-A2.igr"],
        "the border templates no reference page wears"
    );

    // The two numbers that make the decomposition legible: how many sheets
    // need nothing explained, and how many carry something from the lists.
    assert_eq!(report.sheets_checked, 53, "sheets examined");
    assert_eq!(
        report.sheets_clean, 32,
        "sheets drawing with witnessed vocabulary alone"
    );
    assert_eq!(
        report.sheets_affected().len(),
        21,
        "sheets carrying an unexercised entry"
    );
    let drawing_ink_unexercised: BTreeSet<&str> = report
        .entries
        .iter()
        .filter(|entry| entry.drawing > 0)
        .map(|entry| entry.value.as_str())
        .collect();
    assert_eq!(
        drawing_ink_unexercised,
        ["0.000", "0.5000", "3.500/1.750"].into_iter().collect(),
        "the only unexercised entries on the drawing's own ink: D04's 0.50 mm \
         strokes and the two dash patterns"
    );
}
