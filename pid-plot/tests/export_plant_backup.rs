//! Every `.pid` a plant backup holds exports a page.
//!
//! The five corpus drawings and the `test-file/` fixtures are checked
//! elsewhere. This gate covers the wider set the vendor's own
//! `PlantData~2~711.zip` carries — 53 drawings from one plant, shelled out of
//! the backup into `<corpus>/backup-plant-data` with their GBK names restored.
//!
//! What it pins, in the order a regression would show up:
//!
//! - every drawing lays out at all (a decoder that stops seeing a record
//!   family shows up as a page of the wrong size, or none);
//! - no stroke is refused for not fitting the sheet (`off_sheet_primitives`);
//! - no label reaches the writer with a line break in it — the writer has no
//!   face for `U+000D`, and one such label took the whole page down;
//! - every symbol the drawing places is either drawn or named in the known
//!   library gaps below, and no gap grows;
//! - the written page reads back with its declared size, and weighs what a
//!   subset-embedded page weighs — after the `EBDT` fix each page is
//!   175–846 KiB (an empty sheet is 722 B); before it, 6.6–7.1 MiB.
//!
//! Soft-skips when the extracted backup is absent, which is the case on CI.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

/// The reference share this machine keeps the licensed faces in.
fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// Where the shelled-out plant backup sits, if this machine has one.
///
/// The extraction is a local artifact, not a repository fixture, so the gate
/// looks beside the corpus and then in the backup package itself. `None`
/// soft-skips the test, which is what CI sees.
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

/// Symbol files the plant's own reference share does not carry.
///
/// `Pulsation Damper.sym`, a bare `Wellhead Christmas Tree.sym` (the share has
/// the `01`/`02` variants only) and one renamed temperature transmitter are
/// absent from both the corpus `Ref` and the bundled `test-file/symbols-full`,
/// and from the backup archives themselves. Pinned so a *new* gap fails rather
/// than quietly drawing less.
const KNOWN_LIBRARY_GAPS: [&str; 3] = [
    r"piping\specialty components\misc\pulsation damper.sym",
    r"equipment\other equipment\wellhead christmas tree.sym",
    r"instrumentation\off-line\without implied components\temperature\clone of tt-temperature transmitter(1).sym",
];

/// `PlantData~2~711.zip` holds 53 sheets. A ratchet floor, not an equality:
/// a fuller extraction supersedes this one rather than failing the gate.
const BACKUP_SHEETS: usize = 53;

/// Every page is a few hundred KiB once the faces are subset. The bitmap
/// strike that used to ride along pushed each one past 6 MiB.
const PAGE_CEILING: usize = 2 * 1024 * 1024;

#[test]
fn every_backup_drawing_exports_a_readable_page() {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the backup export; no backup-plant-data directory is present");
        return;
    };
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    if drawings.is_empty() {
        eprintln!(
            "skipping the backup export; {} carries no .pid",
            backup.display()
        );
        return;
    }
    assert!(
        drawings.len() >= BACKUP_SHEETS,
        "{} drawings found under {}, expected at least {BACKUP_SHEETS}",
        drawings.len(),
        backup.display()
    );
    let dirs = font_dirs(Some(&corpus));
    if !dirs.iter().any(|dir| dir.is_dir()) {
        eprintln!("skipping the backup export; no font directory is present");
        return;
    }

    let mut models: Vec<(PathBuf, PaperModel)> = Vec::new();
    for path in &drawings {
        let model = lay_out_drawing(path, &corpus)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        models.push((path.clone(), model));
    }
    let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
    let prepared = prepare_fonts(&refs, &dirs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();

    for (path, model) in &models {
        let name = path.display();
        assert_eq!(
            model.off_sheet_primitives, 0,
            "{name}: {} strokes were refused for not fitting the sheet",
            model.off_sheet_primitives
        );
        for primitive in &model.primitives {
            if let PaperShape::Text { value, .. } = &primitive.shape {
                assert!(
                    !value.contains(['\r', '\n', '\t']),
                    "{name}: a label reached the page with a line break in it: {value:?}"
                );
            }
        }
        for missing in &model.missing_symbols {
            assert!(
                KNOWN_LIBRARY_GAPS.contains(&missing.as_str()),
                "{name}: a symbol no library carries entered the set: {missing}"
            );
        }

        let pdf = write_plot(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            pdf.len() < PAGE_CEILING,
            "{name}: the page weighs {} bytes; the faces are not subset",
            pdf.len()
        );
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        if let Some(page) = model.page_mm {
            assert!((ink.page_mm[0] - page[0]).abs() < 0.05, "{name} width");
            assert!((ink.page_mm[1] - page[1]).abs() < 0.05, "{name} height");
        }
        if model.primitives.is_empty() {
            // `D03` is an authored but empty sheet: a declared page with no
            // objects. It exports a blank page rather than failing.
            assert!(ink.segments.is_empty(), "{name}: unexpected ink");
            continue;
        }
        assert!(
            !ink.segments.is_empty(),
            "{name}: the page carries no ink at all"
        );
        assert!(
            !ink.spans.is_empty(),
            "{name}: the page carries no lettering at all"
        );
    }
    eprintln!(
        "{} backup drawings exported, {} faces embedded",
        models.len(),
        prepared.len()
    );
}
