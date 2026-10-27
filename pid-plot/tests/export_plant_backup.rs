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
//! - no placement falls back to a placeholder box: a symbol the `.sym`
//!   libraries lack is still drawn from the body the `.pid` carries itself
//!   (`missing_boxes` is empty on all 53 — the 18 library gaps are drawn);
//! - every library gap is one of the known ones below, and no gap grows;
//! - the written page reads back at the size the model resolved — the declared
//!   sheet, or the standard sheet measured for the four drawings that state
//!   none — and weighs what a subset-embedded page weighs: after the `EBDT`
//!   fix each page is 175–846 KiB (an empty sheet is 722 B), before it
//!   6.6–7.1 MiB;
//!
//! Soft-skips when the extracted backup is absent, which is the case on CI.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{
    PaperModel, PaperShape, STANDARD_SHEETS_MM, SymbolBodySource, lay_out_drawing,
};
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

/// Sheets whose `.pid` states no page at all.
///
/// `D井场`, `Q井场`, `换热器CD` and `凝析油一级闪蒸分离器B气相出口改造` carry
/// neither an `igSmartFrame2d` nor a template name, so
/// [`PaperModel::resolved_page_mm`] measures one instead: A0, A1, A1 and A2
/// respectively. Pinned as an equality, both ways round — a new drawing with no
/// sheet fails here rather than quietly taking the ink's own box, and a sheet
/// that starts stating its page lowers this number deliberately.
const DERIVED_PAGES: usize = 4;

/// Sheets that carry a border template. A floor for the same reason
/// [`BACKUP_SHEETS`] is one: a fuller extraction only adds sheets — the 16 that
/// sat on none until the backup's own `RefData~4~682.zip`, whose templates the
/// corpus now carries, gave them their frames.
const TEMPLATED_SHEETS: usize = 47;

/// What a sheet's template places, by the template's own file name: the file
/// name each placement records, sorted.
///
/// The `CPE-SQ` family keeps the `比例` value cell's 5 mm wave in one placement,
/// whose stored path is the authoring machine's own
/// `C:\Users\ADMINI~1.WIN\AppData\Local\Temp\BLFH.sym`; no share in this corpus
/// carries that file, so the body is the template's own cache. `CPECCHB-SQ-A1`
/// places nothing at all — its title block has no `比例` cell. `CPECCHB-SQ-A2`
/// places three: the wave under another authoring-machine temp name, and the two
/// sign-off cells' `HQZ.sym`.
///
/// Pinned per sheet rather than per package, so the numbers are the template's
/// and not the extraction's — a fuller backup moves nothing here, while a
/// template that starts placing something else fails.
const TEMPLATE_PLACEMENTS: [(&str, &[&str]); 5] = [
    ("CPE-SQ-A0.igr", &["blfh.sym"]),
    ("CPE-SQ-A1.igr", &["blfh.sym"]),
    ("CPE-SQ-A2.igr", &["blfh.sym"]),
    ("CPECCHB-SQ-A1.igr", &[]),
    (
        "CPECCHB-SQ-A2.igr",
        &["a$c40d91a4b.sym", "hqz.sym", "hqz.sym"],
    ),
];

/// The file name a placement's stored path ends in, lowercased.
fn leaf_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

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

    // The furniture's symbol account, over the whole package. A sheet's border
    // is a linked `.igr`, so the symbols its title block places are the
    // template's own and are not in the drawing's `missing_symbols` — that is
    // what keeps the source gaps (`BLFH.sym` and `HQZ.sym`, whose stored paths
    // are absolute paths on the authoring machine) out of the drawing's report
    // and on the page's own ledger, where `--residual` reads it.
    let mut templated = 0usize;
    let mut placements = 0usize;
    let mut gap_named = 0usize;
    for (path, model) in &models {
        let name = path.display();
        assert!(
            !model
                .missing_symbols
                .iter()
                .any(|missing| missing.to_ascii_lowercase().ends_with("blfh.sym")),
            "{name}: the template's own gap entered the drawing's report: {:?}",
            model.missing_symbols
        );
        let Some(template) = model.template.as_ref() else {
            continue;
        };
        templated += 1;
        let Some((_, wanted)) = TEMPLATE_PLACEMENTS
            .iter()
            .find(|(file, _)| template.path.ends_with(file))
        else {
            panic!(
                "{name}: {} drew furniture; no pin says what it places",
                template.path
            );
        };
        let mut drawn: Vec<String> = Vec::new();
        for symbol in &template.symbols {
            placements += 1;
            assert_eq!(
                symbol.body,
                SymbolBodySource::Cached,
                "{name}: {} drew from the library; a template's own copy is the one",
                symbol.key
            );
            // The two halves of the account must agree: a name lookup that
            // found nothing and a body that resolved cannot both be true.
            assert_eq!(
                symbol.library_available,
                symbol.name_gap.is_none(),
                "{name}: the ledger disagrees with the lookup for {:?}",
                symbol
            );
            if symbol.name_gap.is_some() {
                gap_named += 1;
            }
            drawn.push(leaf_name(&symbol.key));
        }
        drawn.sort();
        assert_eq!(drawn, *wanted, "{name}: {} places {drawn:?}", template.path);
    }
    assert!(
        templated >= TEMPLATED_SHEETS,
        "{templated} sheets drew furniture; {TEMPLATED_SHEETS} did when this ratchet was set"
    );
    // No "one placement per sheet" equality: it held of the two templates the
    // corpus had and not of the six it has now — `CPECCHB-SQ-A1` places none
    // and `CPECCHB-SQ-A2` places three. What each template places is pinned
    // above, per sheet, which is where that fact belongs.
    eprintln!(
        "{templated} sheets drew furniture, {placements} placements, {gap_named} with a named gap"
    );
    let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
    let prepared = prepare_fonts(&refs, &dirs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();

    let mut derived = 0usize;
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
        assert!(
            model.missing_boxes.is_empty(),
            "{name}: {} placement(s) had neither a library body nor a body of the \
             drawing's own and drew a placeholder: {:?}",
            model.missing_boxes.len(),
            model.missing_boxes
        );

        let pdf = write_plot(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            pdf.len() < PAGE_CEILING,
            "{name}: the page weighs {} bytes; the faces are not subset",
            pdf.len()
        );
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        // The page the PDF got is the page the model promised, for every
        // sheet — the ones that state a sheet and the ones measured for want
        // of one. This used to be skipped when `page_mm` was `None`, which is
        // exactly the case that needed watching: a measured page could drift
        // to any box the ink reached and no gate would say so.
        let page = model.resolved_page_mm().unwrap_or_else(|| {
            panic!("{name}: the model resolved no page; the writer must have errored")
        });
        assert!((ink.page_mm[0] - page[0]).abs() < 0.05, "{name} width");
        assert!((ink.page_mm[1] - page[1]).abs() < 0.05, "{name} height");
        if model.page_mm.is_none() {
            derived += 1;
            let standard = STANDARD_SHEETS_MM.iter().any(|sheet| {
                [*sheet, [sheet[1], sheet[0]]]
                    .iter()
                    .any(|paper| paper == &page)
            });
            assert!(
                standard,
                "{name}: a page the drawing never stated was measured as {page:?}, \
                 which is not a standard sheet"
            );
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
    assert_eq!(
        derived, DERIVED_PAGES,
        "{derived} sheets stated no page and were measured; {DERIVED_PAGES} did when this ratchet was set"
    );
    eprintln!(
        "{} backup drawings exported, {} of them on a measured page, {} faces embedded",
        models.len(),
        derived,
        prepared.len()
    );
}
