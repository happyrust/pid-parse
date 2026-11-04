//! The plant backup's style vocabulary, against the only pages a plot can be
//! compared with.
//!
//! Forty-two of the corpus's fifty-three plant sheets have no reference plot,
//! so no page-by-page verdict is possible for them. What *is* possible — and is
//! what this gate pins — is a per-entry verdict: every pen width, dash pattern,
//! face and border template a sheet draws with is either one of the eleven
//! validated pages' or it is not, and the ones that are not are named here.
//!
//! The lists below are the *result* of that decomposition, and each one is
//! short enough to settle by hand:
//!
//! - **Pen widths.** Four widths, and every one of them sits inside a band
//!   boundary the eleven plots leave open — see [`WIDENED_BY_THE_BOUNDARY`].
//!   Three are furniture of `CPECCHB-SQ-A1.igr`, which four sheets wear. The
//!   fourth, `0.5000`, is also on the drawing's own ink: `D04` draws 24
//!   strokes with it. The A2 acceptance sheet now witnesses `CPECCHB-SQ-A2`
//!   and the four widths that used to head this list (`0.7071` and the three
//!   rails above 1.06 mm).
//! - **Dash patterns.** Both are the drawing's own ink, and both are resolved
//!   without a reference plot: `0.000` is a single zero-length step, which
//!   `write::dash_array` draws solid on purpose, and `3.500/1.750` is a
//!   two-step pattern whose record literally stores `-3.500/-1.750` — the same
//!   shape as the witnessed `-1.905/-1.270`, read by the same code. The A2
//!   acceptance sheet does not draw this pattern, so it stays unwitnessed.
//! - **Faces.** None: every sheet letters with the four faces the validated
//!   pages already use.
//! - **Templates.** Two files no validated page wears — `CPE-SQ-A2` and
//!   `CPECCHB-SQ-A1` — which is why the three furniture widths above are
//!   unexercised. `CPECCHB-SQ-A2` is witnessed by the A2 acceptance sheet.
//!
//! `37` of the fifty-three sheets draw with witnessed vocabulary alone, and
//! `16` carry at least one entry from the lists below.
//!
//! Soft-skips when the corpus or the extracted backup is absent, which is the
//! case on CI.
//!
//! ```text
//! PID_TEST_DATA_DIR=D:\work\plant-code\cad\pid-test-data \
//!   cargo test -p pid-plot --test style_witness --locked
//! ```
//!
//! The witness set is `PID_ACCEPTANCE_CORPUS`, or
//! `D:\work\cad\pid-test-data\验收数据`: the eleven sheets that gate already
//! holds to a reading. It does not fall back to the five-sheet corpus. The
//! backup sheets are still laid out against `PID_PARSE_SELF_CHECK_DIR`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::witness::{Dimension, Witness};

const PEN_WIDTHS: [&str; 10] = [
    "0.0100", "0.1300", "0.1800", "0.2500", "0.3500", "0.7000", "0.7071", "1.0606", "1.1313",
    "1.4141",
];
const DASH_PATTERNS: [&str; 2] = ["1.905/1.270", "7.000/1.750/0.350/1.750"];
const FACES: [&str; 4] = ["Arial", "Arial Narrow", "FangSong", "SimSun"];
const TEMPLATES: [&str; 3] = ["CPE-SQ-A0.igr", "CPE-SQ-A1.igr", "CPECCHB-SQ-A2.igr"];

/// The unexercised widths whose pens the references cannot settle.
///
/// The pen table snaps a declared width onto four plotter pens, and the
/// references bound each boundary into an open interval, not onto a value.
/// A width is listed here only when no validated page declares it *and* it
/// sits inside one of those intervals. `0.7071` is declared by the A2
/// acceptance sheet and that sheet's pen column agrees, so it is witnessed
/// rather than listed, even though it sits inside the thick/wide interval.
///
/// - **thin/mid `(0.25, 0.35)`** — 0.25 mm rules print thin and 0.35 mm
///   strokes print mid (9665 straight symbol strokes on the eleven sheets
///   agree) — so `0.3000` could be either.
/// - **mid/thick `(0.35, 0.70)`** — 0.35 prints mid and 0.70 prints thick, and
///   nothing in between is declared by a validated page. `0.5000` sits inside
///   it. The old `(0.50, 0.70)` writing rested on an `As Drawn` placeholder
///   that prints thin (see
///   `docs/analysis/2026-10-30-as-drawn-is-not-a-pen.md`).
/// - **thick/wide `(0.70, 1.06)`** — 0.70 prints thick and the A2 title block's
///   1.0606 mm rails print wide (17 strokes, the reference's 1.02 mm). The
///   straight-stroke tally did not name a declaration between them, so the
///   interval is not tighter than this. `0.7500` and `0.7999` sit inside it
///   and no validated page declares either.
const WIDENED_BY_THE_BOUNDARY: [&str; 4] = ["0.3000", "0.5000", "0.7500", "0.7999"];

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// The eleven acceptance sheets, laid out against their own directory.
fn witness_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
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

fn sheets(paths: &[PathBuf], corpus: &Path) -> Vec<(String, PaperModel)> {
    paths
        .iter()
        .map(|path| {
            let model = lay_out_drawing(path, corpus)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            (stem(path), model)
        })
        .collect()
}

/// The drawings in `dir` that have a plot of their own beside them.
fn validated_sheets(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_pids(dir, &mut paths);
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
    let witness_root = witness_dir();
    let witness_paths = validated_sheets(&witness_root);
    if witness_paths.is_empty() {
        eprintln!(
            "skipping the style-witness gate; {} carries no drawing with a plot beside it",
            witness_root.display()
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

    let witnessed = sheets(&witness_paths, &witness_root);
    let witness = Witness::of(witnessed.iter().map(|(name, model)| (name.as_str(), model)));
    // What the validated pages exercise is the floor this gate rests on: if a
    // plot or a drawing goes missing, the witness set shrinks and the
    // unexercised lists below grow, so pin it too.
    assert_eq!(
        witness.values(Dimension::PenWidth),
        PEN_WIDTHS,
        "the declared widths the eleven reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::DashPattern),
        DASH_PATTERNS,
        "the dash patterns the eleven reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::Face),
        FACES,
        "the faces the eleven reference pages letter with"
    );
    assert_eq!(
        witness.values(Dimension::Template),
        TEMPLATES,
        "the templates the eleven reference pages wear"
    );

    let targets = sheets(&target_paths, &corpus);
    let report = witness.unwitnessed(targets.iter().map(|(name, model)| (name.as_str(), model)));

    // Every unexercised entry, by dimension, with the drawing/furniture split.
    let mut widths: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for entry in report.of_dimension(Dimension::PenWidth) {
        widths.insert(&entry.value, (entry.drawing, entry.furniture));
    }
    assert_eq!(
        widths.keys().copied().collect::<Vec<_>>(),
        ["0.3000", "0.5000", "0.7500", "0.7999"],
        "the pen widths no reference page draws with"
    );
    // The load-bearing half of the decomposition: the pen table is measured
    // from the references, so an unexercised width on the drawing's own ink is
    // a process line whose weight nothing has checked. Three of the four are
    // furniture; `0.5000` is the one that is not, and every one of the four
    // sits inside a boundary the references leave open.
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
    for (width, expected) in [
        ("0.3000", (0, 32)),
        ("0.5000", (24, 16)),
        ("0.7500", (0, 12)),
        ("0.7999", (0, 52)),
    ] {
        assert_eq!(
            widths[width], expected,
            "{width} mm is an unexercised width inside a band boundary the \
             references leave open; its stroke count is what a tighter boundary \
             would re-pen"
        );
    }
    assert_eq!(
        WIDENED_BY_THE_BOUNDARY.len(),
        4,
        "three boundaries, four widths the references cannot settle"
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
        ["CPE-SQ-A2.igr", "CPECCHB-SQ-A1.igr"],
        "the border templates no reference page wears"
    );

    // The two numbers that make the decomposition legible: how many sheets
    // need nothing explained, and how many carry something from the lists.
    assert_eq!(report.sheets_checked, 53, "sheets examined");
    assert_eq!(
        report.sheets_clean, 37,
        "sheets drawing with witnessed vocabulary alone"
    );
    assert_eq!(
        report.sheets_affected().len(),
        16,
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
