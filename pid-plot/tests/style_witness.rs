//! The plant backup's style vocabulary, against the pages a factory plot has
//! been read for.
//!
//! The eleven acceptance sheets are one witness set. Three backup drawings
//! now have a factory Amyuni plot, each compared with what this crate draws
//! before the drawing was added here:
//!
//! - `O井场` wears `CPECCHB-SQ-A1` and its plot prints that furniture. Its
//!   `0.30` mm strokes are two device units, its `0.50` mm strokes three,
//!   and its `0.75` and `0.7999` mm strokes four — see `pen_width_mm`.
//! - `A井场进站生产收球筒` wears `CPE-SQ-A2`, and the plot's furniture lies
//!   on the furniture this crate draws.
//! - `凝析油一级闪蒸分离器A气相出口改造` draws `3.500/1.750` unfitted, and
//!   the plot dashes it at that pitch on the pen the width already prints.
//!
//! What is still unwitnessed is one dash: `0.000`, ten strokes of `D04`'s
//! own ink, which `write::dash_array` draws solid because a zero period
//! inks nothing. No factory plot of `D04` is in the set above.
//!
//! Soft-skips when the corpus or the extracted backup is absent, which is the
//! case on CI.
//!
//! ```text
//! PID_TEST_DATA_DIR=D:\work\plant-code\cad\pid-test-data \
//!   cargo test -p pid-plot --test style_witness --locked
//! ```
//!
//! The eleven sheets are `PID_ACCEPTANCE_CORPUS`, or
//! `D:\work\cad\pid-test-data\验收数据`. The three factory plots are the
//! PDFs of those names under `<backup>/../publish/sppid导出/导出dxf pdf/导出pdf`.
//! The backup sheets are laid out against `PID_PARSE_SELF_CHECK_DIR`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::witness::{Dimension, Witness};

const PEN_WIDTHS: [&str; 14] = [
    "0.0100", "0.1300", "0.1800", "0.2500", "0.3000", "0.3500", "0.5000", "0.7000", "0.7071",
    "0.7500", "0.7999", "1.0606", "1.1313", "1.4141",
];
const DASH_PATTERNS: [&str; 3] = ["1.905/1.270", "3.500/1.750", "7.000/1.750/0.350/1.750"];
const FACES: [&str; 4] = ["Arial", "Arial Narrow", "FangSong", "SimSun"];
const TEMPLATES: [&str; 5] = [
    "CPE-SQ-A0.igr",
    "CPE-SQ-A1.igr",
    "CPE-SQ-A2.igr",
    "CPECCHB-SQ-A1.igr",
    "CPECCHB-SQ-A2.igr",
];

/// Backup drawings whose factory plot was compared before the drawing was
/// allowed to witness its vocabulary. One sheet of each kind.
const FACTORY_SHEETS: [&str; 3] = [
    r"00\00\O井场 注采阀组工艺及自控流程图.pid",
    r"00\00\集注站 进出站收发球单元 A井场进站生产收球筒工艺及自控流程图.pid",
    r"00\02\集注站 凝析油一级闪蒸分离器A气相出口改造工艺及自控流程图.pid",
];

/// Factory plots named beside the backup package, not beside the `.pid`.
fn factory_plot_dir(backup: &Path) -> PathBuf {
    backup
        .parent()
        .unwrap_or(backup)
        .join(r"publish\sppid导出\导出dxf pdf\导出pdf")
}

/// The corpus `PID_PARSE_SELF_CHECK_DIR` names, when the run names one.
fn named_corpus() -> Option<PathBuf> {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map(PathBuf::from)
}

fn corpus_dir() -> PathBuf {
    named_corpus().unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"))
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
    if named_corpus().is_none() {
        roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
        roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    }
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

    let mut witnessed = sheets(&witness_paths, &witness_root);
    let factory = factory_plot_dir(&backup);
    for relative in FACTORY_SHEETS {
        let pid = backup.join(relative);
        let name = stem(&pid);
        let pdf = factory.join(format!("{name}.pdf"));
        assert!(
            pdf.is_file(),
            "{name}: the factory plot is not {}",
            pdf.display()
        );
        let model = lay_out_drawing(&pid, &corpus)
            .unwrap_or_else(|error| panic!("{}: {error}", pid.display()));
        witnessed.push((name, model));
    }
    let witness = Witness::of(witnessed.iter().map(|(name, model)| (name.as_str(), model)));
    // What the validated pages exercise is the floor this gate rests on: if a
    // plot or a drawing goes missing, the witness set shrinks and an entry
    // below comes back, so pin it too.
    assert_eq!(
        witness.values(Dimension::PenWidth),
        PEN_WIDTHS,
        "the declared widths the reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::DashPattern),
        DASH_PATTERNS,
        "the dash patterns the reference pages draw with"
    );
    assert_eq!(
        witness.values(Dimension::Face),
        FACES,
        "the faces the reference pages letter with"
    );
    assert_eq!(
        witness.values(Dimension::Template),
        TEMPLATES,
        "the templates the reference pages wear"
    );

    let targets = sheets(&target_paths, &corpus);
    let report = witness.unwitnessed(targets.iter().map(|(name, model)| (name.as_str(), model)));

    assert_eq!(
        report
            .of_dimension(Dimension::PenWidth)
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>(),
        Vec::<&str>::new(),
        "every pen width a backup sheet declares is one a factory plot prints"
    );
    let mut dashes: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for entry in report.of_dimension(Dimension::DashPattern) {
        dashes.insert(&entry.value, (entry.drawing, entry.furniture));
    }
    assert_eq!(
        dashes.keys().copied().collect::<Vec<_>>(),
        ["0.000"],
        "the dash patterns no reference page draws with"
    );
    assert_eq!(
        dashes["0.000"],
        (10, 0),
        "the zero-length pattern's strokes, on D04's own ink; the writer draws it solid"
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
        Vec::<&str>::new(),
        "every border template a backup sheet wears is one a reference page wears"
    );

    assert_eq!(report.sheets_checked, 53, "sheets examined");
    assert_eq!(
        report.sheets_clean, 52,
        "sheets drawing with witnessed vocabulary alone"
    );
    assert_eq!(
        report.sheets_affected(),
        ["D04"].into_iter().collect(),
        "the sheet still carrying an unwitnessed entry"
    );
    let drawing_ink_unexercised: BTreeSet<&str> = report
        .entries
        .iter()
        .filter(|entry| entry.drawing > 0)
        .map(|entry| entry.value.as_str())
        .collect();
    assert_eq!(
        drawing_ink_unexercised,
        ["0.000"].into_iter().collect(),
        "the only unwitnessed entry on a drawing's own ink"
    );
}
