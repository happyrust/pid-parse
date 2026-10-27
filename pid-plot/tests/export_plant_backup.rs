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
//! It does not soft-skip once the backup is there and no face directory is:
//! the written-page pins cannot be read without a face, and reporting them
//! unread is not the same as reading them — see [`require_faces`].

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{
    PaperModel, PaperShape, STANDARD_SHEETS_MM, SymbolBodySource, lay_out_drawing,
};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, WIDEST_PEN_MM, write_plot};

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
///
/// Two of the seven only reach a page when the corpus can read the backup's own
/// `RefData~*.zip` (`A0-W - New.igr` on `A井场`, `A1-W - New.igr` on `稳定塔
/// (600td)`): same extent as their `CPE-SQ` siblings, so the pick's last key —
/// the file name on disk — decides, and the archive offers both. Neither title
/// block places a symbol; the two are pinned so the gate holds whichever corpus
/// it is run against — `PID_PARSE_SELF_CHECK_DIR` at a share (`Ref/Template
/// Files` first) or at a backup package, whose templates then come out of its
/// own `RefData~4~682.zip`. A backup package is not a share: it carries no
/// `fonts/`, so that run also has to name the share's faces in `PID_PLOT_FONTS`
/// — otherwise the gate refuses to pass for want of a face (see
/// [`require_faces`]) instead of reporting these two pins unread.
///
/// The second number is `BorderTemplate::extent_mm` — the template's own ink
/// box, the size its outer frame spans. The pick already refuses a template
/// more than 2 mm off the sheet (`TEMPLATE_EXTENT_TOLERANCE_MM` in the model),
/// so this pins the exact size rather than "close enough": a template that
/// loses a millimetre to a scale slip still gets picked, and only this says so.
/// It cannot see a *translation* — `extent_mm` is a width and a height, with no
/// origin — which is why the strokes are measured on the page as well, below.
///
/// The third number is `BorderTemplate::primitives` — every stroke the
/// template contributed, labels included. It is the same file whether the
/// corpus found it on disk or inside the backup's own archive, so the count is
/// the template's and not the extraction's; a title block that gains a cell
/// table line or loses a piece of its frame fails here even when it places no
/// symbol. (`CPECCHB-SQ-A1`'s 96 against `CPE-SQ-A1`'s 175 is the sign-off
/// column plus the `比例` cell it does not have.)
///
/// The extent is the template's, not the sheet's: `CPECCHB-SQ-A2` is authored
/// 593.939 mm wide on a 594.3 mm page, and `A1-W - New.igr` is 841 × 594 on the
/// `600td` sheet's 842.54 × 595.54 page — both inside the pick's tolerance, so
/// pinning the sheet here instead would be pinning the wrong number.
const TEMPLATE_PLACEMENTS: [(&str, [f64; 2], usize, &[&str]); 7] = [
    ("CPE-SQ-A0.igr", [1189.0, 841.0], 175, &["blfh.sym"]),
    ("CPE-SQ-A1.igr", [841.0, 594.0], 175, &["blfh.sym"]),
    ("CPE-SQ-A2.igr", [594.0, 420.0], 175, &["blfh.sym"]),
    ("CPECCHB-SQ-A1.igr", [841.0, 594.0], 96, &[]),
    (
        "CPECCHB-SQ-A2.igr",
        [593.939, 420.0],
        116,
        &["a$c40d91a4b.sym", "hqz.sym", "hqz.sym"],
    ),
    ("A0-W - New.igr", [1189.0, 841.0], 105, &[]),
    ("A1-W - New.igr", [841.0, 594.0], 101, &[]),
];

/// How far a pinned extent may drift before the gate speaks, in millimetres.
///
/// Two hundredths of a millimetre: a hundred times tighter than the pick's own
/// 2 mm tolerance, so a scale slip the pick waves through still fails here,
/// while the float noise in a template's authored size (`593.9393939…`) is far
/// inside it. It is deliberately below what a plotter resolves — this is a pin
/// on the file, not on the printed sheet.
const TEMPLATE_EXTENT_SLACK_MM: f64 = 0.02;

/// Where a page's furniture ink reaches, paper millimetres.
///
/// Not the same measure as `BorderTemplate::extent_mm`: that one is taken
/// from the `.igr` before it is drawn, so it is blind to where the template
/// was put. This one is the box the template's *strokes* cover once they are on
/// the page, which is what moves when a template is translated and what shrinks
/// when it is scaled. Labels are left out, as the template's own measure leaves
/// them out.
fn furniture_extent_mm(model: &PaperModel) -> Option<[[f64; 2]; 2]> {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    let mut seen = false;
    for primitive in model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
    {
        let mut take = |point: [f64; 2]| {
            seen = true;
            for axis in 0..2 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        };
        match &primitive.shape {
            PaperShape::Line { start_mm, end_mm } => {
                take(*start_mm);
                take(*end_mm);
            }
            PaperShape::Polyline { points_mm, .. } => {
                for point in points_mm {
                    take(*point);
                }
            }
            PaperShape::Fill { chain_mm, .. } => {
                for point in chain_mm {
                    take(*point);
                }
            }
            // A circle's box is its whole circle, as the model measures an
            // arc when it picks the template; a template's arcs are small
            // corner rounds, so this overstates by at most the round.
            PaperShape::Arc {
                center_mm,
                radius_mm,
                ..
            }
            | PaperShape::Circle {
                center_mm,
                radius_mm,
            } => {
                take([center_mm[0] - radius_mm, center_mm[1] - radius_mm]);
                take([center_mm[0] + radius_mm, center_mm[1] + radius_mm]);
            }
            PaperShape::Text { .. } => {}
        }
    }
    seen.then_some([min, max])
}

/// The file name a placement's stored path ends in, lowercased.
fn leaf_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

/// Whether a template's path names a `RefData~*.zip` member rather than a file
/// exploded on a share.
///
/// A member is labelled as the archive path with the member's own name on the
/// end (`…\RefData~4~682.zip\CPE-SQ-A1.igr`), so the `.zip` component is what
/// tells the two corpora apart.
fn from_archive(path: &str) -> bool {
    Path::new(path).components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".zip")
    })
}

/// The corpus this run is reading, one line per source.
///
/// A passing gate has to say what it read. The archive-template run — the
/// corpus pointed at a backup package instead of the share — once took 85 s and
/// reported ok while touching no pin at all, and nothing in its output would
/// have said which corpus that `ok` was about.
fn describe_sources(corpus: &Path, backup: &Path, drawings: usize, dirs: &[PathBuf]) -> String {
    let origin = if std::env::var_os("PID_PARSE_SELF_CHECK_DIR").is_some() {
        "PID_PARSE_SELF_CHECK_DIR"
    } else {
        "built-in default"
    };
    let faces: Vec<String> = dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .map(|dir| dir.display().to_string())
        .collect();
    format!(
        "corpus {} ({origin}); backup {} ({drawings} drawings); faces {}",
        corpus.display(),
        backup.display(),
        if faces.is_empty() {
            "none present".to_string()
        } else {
            faces.join(", ")
        }
    )
}

/// Refuse to pass for want of a face, naming what the page pins never saw.
///
/// A backup package keeps its templates in `RefData~*.zip` but not the
/// licensed faces, so the archive configuration has no `fonts/` under its
/// corpus. The layout pins all read without one — how many sheets drew
/// furniture, which template, its extent, its strokes, what it placed — but
/// every pin that needs a written PDF does not, and this used to be an
/// `eprintln!` and a `return`, which the harness reports as a pass. It did, for
/// one run.
fn require_faces(corpus: &Path, dirs: &[PathBuf], drawings: usize) {
    if dirs.iter().any(|dir| dir.is_dir()) {
        return;
    }
    let searched = dirs
        .iter()
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    panic!(
        "no font directory is present, so the written-page pins were not read for {drawings} \
         drawings: each PDF's page, its weight, its ink and its lettering, and the \
         {DERIVED_PAGES}-sheet measured-page ratchet.\n  \
         searched: {searched}\n  \
         corpus: {} — a backup package carries no `fonts/`; set PID_PLOT_FONTS to the \
         share's fonts, or point PID_PARSE_SELF_CHECK_DIR at the share",
        corpus.display()
    );
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
    let sources = describe_sources(&corpus, &backup, drawings.len(), &dirs);
    eprintln!("{sources}");

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
    let mut templated_from_archive = 0usize;
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
        if from_archive(&template.path) {
            templated_from_archive += 1;
        }
        let Some((_, extent, primitives, wanted)) = TEMPLATE_PLACEMENTS
            .iter()
            .find(|(file, ..)| template.path.ends_with(file))
        else {
            panic!(
                "{name}: {} drew furniture; no pin says what it places",
                template.path
            );
        };
        for axis in 0..2 {
            assert!(
                (template.extent_mm[axis] - extent[axis]).abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {} measures {:?} mm, not the {extent:?} mm it is pinned at",
                template.path,
                template.extent_mm
            );
        }
        assert_eq!(
            template.primitives, *primitives,
            "{name}: {} contributed {} strokes",
            template.path, template.primitives
        );
        // The template states its own box; the page has to carry it from the
        // page's origin, because a template is authored in page millimetres and
        // laid out at 1:1. Measuring the strokes — not reusing `extent_mm` — is
        // what makes a translation visible: move the ink 2 mm and the box moves
        // with it while `extent_mm` does not budge.
        let Some([ink_min, ink_max]) = furniture_extent_mm(model) else {
            panic!(
                "{name}: {} is on the page but not one stroke of it landed",
                template.path
            );
        };
        for axis in 0..2 {
            assert!(
                ink_min[axis].abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture starts at {ink_min:?} mm instead of the page origin",
                template.path
            );
            assert!(
                (ink_max[axis] - extent[axis]).abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture reaches {ink_max:?} mm, not the {extent:?} mm box \
                 the template states",
                template.path
            );
        }
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
        "{templated} sheets drew furniture ({} disk, {} from RefData archives), \
         {placements} placements, {gap_named} with a named gap",
        templated - templated_from_archive,
        templated_from_archive
    );
    // Last, because every pin above reads without a face and should speak
    // first: a template that moved is worth knowing about before a missing
    // face.
    require_faces(&corpus, &dirs, drawings.len());
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
            // A drawing's own ink never carries a width no pen carries. The one
            // that did is `Q井场`'s globe valve, placed under `BandAid` — the
            // 10 mm band SmartPlant paints over a flagged object — which the
            // model now treats as a mark rather than a pen. Furniture is out of
            // scope: a template's strokes are its own, drawn from its own body.
            if !primitive.furniture
                && let Some(stroke) = &primitive.stroke
            {
                assert!(
                    stroke.width_mm <= WIDEST_PEN_MM,
                    "{name}: a {:?} paints at {} mm, past the widest pen",
                    primitive.shape,
                    stroke.width_mm
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
