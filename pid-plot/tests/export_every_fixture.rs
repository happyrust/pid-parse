//! Every `.pid` fixture in this repository exports its whole page — the
//! drawing on the border template it links — and the PDF reads back with that
//! page's ink and lettering, the template's included.
//!
//! The five corpus drawings have their own gate (`compare_five_drawings`,
//! `write_five_drawings`); this one covers the drawings that ship inside
//! `test-file/`, where a page that cannot be written at all — DWG-0202's note
//! carried a `U+000D` no typeface can encode — would otherwise only be found by
//! running the CLI by hand.
//!
//! # Which page
//!
//! The one `docs/baselines/export-ledger-fixtures.json` accounts for. Each
//! drawing is laid out the way `export_ledger_fixtures` lays it out: with
//! `test-file/` as its corpus and the committed symbol sources stated
//! ([`symbol_roots`], [`archives`]) rather than discovered.
//!
//! The corpus is what a frame's link is answered from, and only this one
//! answers all seven: four from `.igr` files under
//! `test-file/Ref/Template Files`, three from `CPECCHBA2.igr` and
//! `XIONGANA2.igr` inside a backup package's `RefData~4~682.zip` under
//! `test-file/backup-test`. Laid out against the reference share instead,
//! `DWG-0201GP06-01`, both copies of `DWG-0202GP06-01` and
//! `工艺管道及仪表流程-1` find no template and export a bare sheet — which
//! still has a page size, ink and lettering. So the gate states which template
//! each page wears ([`FIXTURE_TEMPLATES`]) and holds the page read back to the
//! strokes and the lettering of the model, furniture included.
//!
//! The fixtures are found by walking `test-file/`, so a drawing committed
//! there is exported without being listed. A drawing the table has no row for
//! fails the gate rather than passing unframed.
//!
//! # What is asserted
//!
//! Of the walk, before a typeface is needed:
//!
//! - it finds at least the seven committed drawings, and every drawing it
//!   finds has a row in [`FIXTURE_TEMPLATES`].
//!
//! Of each page as laid out:
//!
//! - it wears the template its row names — the `page.template.name` the ledger
//!   baseline records for it — and no template its frame links went unanswered
//!   (`missing_templates`);
//! - no label reaches the writer with a line break in it;
//! - no stroke is refused for not fitting the sheet (`off_sheet_primitives`);
//! - no placement is left as a placeholder box (`missing_boxes`);
//! - the drawing states its own page.
//!
//! Of each page as written, which is what `export_ledger_fixtures` cannot
//! check without a typeface:
//!
//! - the writer acted on every verdict it reached (`WriteLedger::mismatches`
//!   is empty);
//! - the ledger assembled from that write agrees with the page model and with
//!   the writer's own audit of condensed and clipped runs (`cross_check`).
//!
//! Of the PDF read back:
//!
//! - the page is the size the drawing states, and carries ink and lettering;
//! - it holds as many ink segments as the model strokes, the template's among
//!   them;
//! - it letters the characters the model letters, in writing order, the
//!   template's among them.
//!
//! Of the symbol libraries, last: no fixture names a symbol file that every
//! library this machine can search lacks. That is asked of the library search
//! itself rather than of the pages above, and only where the bundled library
//! is present — see the note at the assertion. The names the committed sources
//! alone do not carry are a gap in those sources, not an export failure (each
//! is drawn from the body the drawing keeps), so they are reported and not
//! asserted.
//!
//! # Fonts
//!
//! A page cannot be written without its faces. They come from [`font_dirs`] —
//! `PID_PLOT_FONTS`, then the reference share's `fonts`, then the repository's
//! `test-file/fonts` — and when none of those directories is present the gate
//! says so and returns, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ledger::ExportLedger;
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing, lay_out_with_symbol_roots};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, WriteLedger, stroked_segments, write_plot_audited, written_order};

/// The reference share: where this machine keeps the licensed faces, and the
/// `Ref` library the symbol check at the end of the gate searches.
fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// The repository root: `pid-plot/`'s parent.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot is a directory in the repository")
        .to_path_buf()
}

/// The border template each fixture's page wears: the fixture's path under
/// `test-file/`, `/`-separated, and the template's file name.
///
/// Both are read off `docs/baselines/export-ledger-fixtures.json`, which keys
/// its pages by that path and records the template under `page.template.name`,
/// so a row is the ledger baseline's statement about the page and this gate
/// holds the exported PDF to it. `XIONGANA2.igr` and `CPECCHBA2.igr` are
/// members of `DWG-0202GP06-01_p`'s `RefData~4~682.zip`; the other four are
/// files under `test-file/Ref/Template Files`.
///
/// The table does not decide which drawings are exported — the walk does. A
/// drawing with no row fails the gate: a page that is exported has to be held
/// to a frame and a title block, and a new one is not held to any until its
/// row says which.
const FIXTURE_TEMPLATES: [(&str, &str); 7] = [
    ("D06.pid", "CPECCHB-SQ-A2.igr"),
    ("DWG-0201GP06-01.pid", "XIONGANA2.igr"),
    ("DWG-0202GP06-01.pid", "CPECCHBA2.igr"),
    ("export-test/publish-data/A01/A01.pid", "CPE-SQ-A2.igr"),
    (
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        "CPECCHBA2.igr",
    ),
    ("工艺管道及仪表流程-1.pid", "XA-A1.igr"),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        "CPE-SQ-A1.igr",
    ),
];

/// One fixture drawing as this gate exports it.
struct FixturePage {
    /// The drawing's path under `test-file/`, `/`-separated.
    key: String,
    /// The `.pid` itself.
    path: PathBuf,
    /// The border template [`FIXTURE_TEMPLATES`] names for it.
    template: &'static str,
    /// The page, laid out from the committed sources.
    model: PaperModel,
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

/// A fixture's path under `test-file/`, `/`-separated: its row in
/// [`FIXTURE_TEMPLATES`], and its key in the ledger baseline.
fn page_key(fixture_dir: &Path, path: &Path) -> String {
    path.strip_prefix(fixture_dir)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The symbol roots the pages are laid out from: the committed sample library
/// and the project's own `Xa …` files — the roots `export_ledger_fixtures`
/// states, so the page exported here is the page its baseline accounts for.
///
/// Stated rather than discovered: [`lay_out_drawing`] would also search the
/// gitignored `test-file/symbols-full`, so what a page resolved, and which
/// names it reports unanswered, would depend on a directory a clean checkout
/// does not have.
fn symbol_roots(fixture_dir: &Path) -> Vec<PathBuf> {
    vec![
        fixture_dir.join("symbols"),
        fixture_dir.join("Ref").join("Symbols"),
    ]
}

/// Every `RefData~*.zip` the committed backup packages carry, path-sorted —
/// the symbol bodies searched after [`symbol_roots`].
///
/// The packages are part of the repository, so none of this is a conditional
/// input. A `backup-test` that cannot be read, or that yields no archive, is a
/// broken checkout, and pages laid out without the archives are not the pages
/// the ledger baseline accounts for — so this fails instead of returning
/// nothing.
fn archives(fixture_dir: &Path) -> Vec<PathBuf> {
    let backup = fixture_dir.join("backup-test");
    let entries = std::fs::read_dir(&backup).unwrap_or_else(|error| {
        panic!(
            "{} holds the committed backup packages and cannot be read: {error}",
            backup.display()
        )
    });
    let mut packages: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    packages.sort();
    let mut found: Vec<PathBuf> = Vec::new();
    for package in packages {
        if package.is_dir() {
            found.extend(pid_parse::refdata_archive::refdata_archives(&package));
        }
    }
    found.sort();
    found.dedup();
    assert!(
        !found.is_empty(),
        "{} holds the committed backup packages, and none of them yielded a RefData~*.zip",
        backup.display()
    );
    found
}

/// The file name of the template a page wears, as the ledger baseline records
/// it under `page.template.name`: the last component of the template's path,
/// which is the `.igr` on disk or — for one read out of a backup's
/// `RefData~*.zip` — the member's own name.
fn template_file_name(model: &PaperModel) -> Option<String> {
    let template = model.template.as_ref()?;
    Path::new(&template.path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// How many of the segments [`stroked_segments`] lists are the border
/// template's.
///
/// The list does not say which primitive a segment came from, so the template
/// is stroked on its own: the same page holding only its furniture.
fn template_segments(model: &PaperModel) -> usize {
    let furniture = PaperModel {
        primitives: model
            .primitives
            .iter()
            .filter(|primitive| primitive.furniture)
            .cloned()
            .collect(),
        ..model.clone()
    };
    stroked_segments(&furniture).len()
}

/// One character a page inks, and whether the border template letters it.
type InkedChar = (char, bool);

/// The characters of `text` that put ink on the page.
///
/// A page read back does not keep a label's boundaries or its spaces. The
/// reader starts a new span where the face changes or a gap opens, joins two
/// labels that share a baseline, and reports a wide gap as a space of its own;
/// the writer letters no trailing pad. On `工艺管道及仪表流程-1` the template's
/// `COUNTERSIGNED` comes back as `C`, `O`, `U`, `N`, `TER`, `S`, `IG`, `N`,
/// `ED` with a space between each, and its `APPR.` and `FINAL APPR` as one
/// `APPR.FINAL APPR` — 136 labels in 404 spans. None of that adds, drops or
/// reorders a character that inks, so both sides are compared as these.
fn inked_characters(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().filter(|character| !character.is_whitespace())
}

/// Every character the writer lettered, in the order it wrote them: the
/// `decoded` layer and then the `inferred` one, each in the order the sheet
/// draws its objects in — a label where its record stands in the sheet's
/// list, which is neither the model's order nor the stream's.
///
/// Both halves are the writer's own account of the page rather than a second
/// copy of its rules: which labels were lettered is its [`WriteLedger`], and
/// the order it visited them in is [`written_order`].
fn lettered_characters(model: &PaperModel, ledger: &WriteLedger) -> Vec<InkedChar> {
    let mut lettered = Vec::new();
    for index in written_order(model) {
        let primitive = &model.primitives[index];
        let PaperShape::Text { value, .. } = &primitive.shape else {
            continue;
        };
        if !ledger.fate(index).is_some_and(|fate| fate.is_on_page()) {
            continue;
        }
        lettered.extend(inked_characters(value).map(|character| (character, primitive.furniture)));
    }
    lettered
}

/// How many characters either side of a difference a failure quotes.
const QUOTED_CHARACTERS: usize = 12;

/// Where the lettering a page reads back parts from what the writer lettered,
/// or `None` when the two are the same characters in the same order.
///
/// The difference is placed in the template's lettering or the drawing's own,
/// because the two say different things: a page that lost its title block
/// parts inside the template's, at its first cell.
fn lettering_difference(read: &[char], lettered: &[InkedChar]) -> Option<String> {
    let at = read
        .iter()
        .zip(lettered)
        .position(|(read, (model, _))| read != model)
        .or_else(|| (read.len() != lettered.len()).then_some(read.len().min(lettered.len())))?;
    let whose = match lettered.get(at) {
        Some((_, true)) => "inside the border template's lettering",
        Some((_, false)) => "inside the drawing's own lettering",
        None => "past the last character the model letters",
    };
    let from = at.saturating_sub(QUOTED_CHARACTERS);
    let read_there: String = read.iter().skip(from).take(2 * QUOTED_CHARACTERS).collect();
    let model_there: String = lettered
        .iter()
        .skip(from)
        .take(2 * QUOTED_CHARACTERS)
        .map(|(character, _)| *character)
        .collect();
    let template = lettered.iter().filter(|(_, furniture)| *furniture).count();
    Some(format!(
        "the page reads back {} inked characters and the model letters {} ({template} of them \
         the border template's); they part at character {at}, {whose}: read back \
         {read_there:?}, model {model_there:?}",
        read.len(),
        lettered.len()
    ))
}

#[test]
fn every_fixture_drawing_exports_a_pdf() {
    let fixture_dir = repo_root().join("test-file");
    let mut fixtures = Vec::new();
    collect_pids(&fixture_dir, &mut fixtures);
    // The fixtures are committed, so a walk that comes back short is a broken
    // checkout rather than an absent corpus: it fails instead of skipping.
    assert!(
        fixtures.len() >= FIXTURE_TEMPLATES.len(),
        "{} fixture drawings found under {}; the repository commits at least {}",
        fixtures.len(),
        fixture_dir.display(),
        FIXTURE_TEMPLATES.len()
    );
    let mut listed: Vec<(String, PathBuf, &'static str)> = Vec::new();
    let mut unlisted: Vec<String> = Vec::new();
    for path in fixtures {
        let key = page_key(&fixture_dir, &path);
        match FIXTURE_TEMPLATES
            .iter()
            .find(|(fixture, _)| *fixture == key)
        {
            Some((_, template)) => listed.push((key, path, template)),
            None => unlisted.push(key),
        }
    }
    assert!(
        unlisted.is_empty(),
        "{} fixture drawing(s) have no row in FIXTURE_TEMPLATES: {unlisted:?}. A drawing \
         committed under test-file/ is exported by this gate as soon as it is there; add a \
         row naming the border template its page wears, so the page is held to its frame and \
         title block instead of passing without them",
        unlisted.len()
    );

    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    if !dirs.iter().any(|dir| dir.is_dir()) {
        eprintln!("skipping fixture export; no font directory is present");
        return;
    }

    let roots = symbol_roots(&fixture_dir);
    let archives = archives(&fixture_dir);
    let library = [
        "test-file/symbols".to_string(),
        "test-file/Ref/Symbols".to_string(),
    ];
    let mut pages: Vec<FixturePage> = Vec::new();
    for (key, path, template) in listed {
        let model = lay_out_with_symbol_roots(
            &path,
            &fixture_dir,
            roots.clone(),
            archives.clone(),
            &library,
        )
        .unwrap_or_else(|error| panic!("{key}: {error}"));
        pages.push(FixturePage {
            key,
            path,
            template,
            model,
        });
    }

    // Every page is asked before any is written, so one run names every page
    // that lost its furniture rather than stopping at the first.
    let unframed: Vec<String> = pages
        .iter()
        .filter_map(|fixture| {
            let worn = template_file_name(&fixture.model);
            let answered = fixture.model.missing_templates.is_empty();
            (worn.as_deref() != Some(fixture.template) || !answered).then(|| {
                format!(
                    "  {}: wears {}, its row names {}; template names no file answered: {:?}",
                    fixture.key,
                    worn.as_deref().unwrap_or("no template"),
                    fixture.template,
                    fixture.model.missing_templates
                )
            })
        })
        .collect();
    assert!(
        unframed.is_empty(),
        "{} of {} fixture pages do not wear the border template FIXTURE_TEMPLATES names, so \
         they would export without their frame and title block:\n{}",
        unframed.len(),
        pages.len(),
        unframed.join("\n")
    );

    let refs: Vec<&PaperModel> = pages.iter().map(|fixture| &fixture.model).collect();
    let prepared = prepare_fonts(&refs, &dirs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();

    for fixture in &pages {
        let name = fixture.key.as_str();
        let model = &fixture.model;
        // A lettering value the writer cannot encode takes the whole page
        // down, so the model is where a line break has to be spent.
        for primitive in &model.primitives {
            if let PaperShape::Text { value, .. } = &primitive.shape {
                assert!(
                    !value.contains(['\r', '\n', '\t']),
                    "{name}: a label reached the page with a line break in it: {value:?}"
                );
            }
        }
        assert_eq!(
            model.off_sheet_primitives, 0,
            "{name}: {} strokes were refused for not fitting the sheet",
            model.off_sheet_primitives
        );
        // The committed symbol sources do not carry every file a fixture
        // names, and a name they lack still has to draw — from the body the
        // drawing keeps for it. A placement with neither leaves a 30 mm
        // placeholder square on the exported page.
        assert!(
            model.missing_boxes.is_empty(),
            "{name}: {} placement(s) had neither a library body nor a body of the \
             drawing's own and drew a placeholder: {:?}",
            model.missing_boxes.len(),
            model.missing_boxes
        );
        // Every fixture states its own sheet, so none of them needs the
        // measured page [`PaperModel::resolved_page_mm`] falls back to. Stated
        // rather than assumed: a fixture that stops declaring a page is a
        // decode regression, not a page to guess.
        let page = model
            .page_mm
            .unwrap_or_else(|| panic!("{name}: the fixture states no page"));

        let written =
            write_plot_audited(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        // The writer's own check that the verdicts it reached up front are the
        // ones it acted on. `export_ledger_fixtures` takes its verdicts from
        // `write_ledger`, which writes nothing and returns this list empty by
        // construction; only a page that was written can fill it.
        assert!(
            written.ledger.mismatches.is_empty(),
            "{name}: the writer's verdicts disagreed with what it wrote: {:?}",
            written.ledger.mismatches
        );
        // The account of the page as written, against the counts the model
        // keeps and — the part that needs a face — against the writer's audit
        // of the runs it condensed or clipped.
        let ledger = ExportLedger::assemble(model, &written.ledger);
        let cross = ledger.cross_check(model, Some(&written.audit), None);
        assert!(
            cross.is_empty(),
            "{name}: the ledger disagrees with the counts the code already keeps: {cross:#?}"
        );

        let ink =
            read_plot(&written.pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        assert!((ink.page_mm[0] - page[0]).abs() < 0.05, "{name} width");
        assert!((ink.page_mm[1] - page[1]).abs() < 0.05, "{name} height");
        assert!(
            !ink.segments.is_empty(),
            "{name}: the page carries no ink at all"
        );
        assert!(
            !ink.spans.is_empty(),
            "{name}: the page carries no lettering at all"
        );
        // A page size, some ink and some lettering are all the checks above
        // ask of the PDF, and a page written without its frame and title block
        // has all three; so the page read back is held to the model itself.
        // `stroked_segments` lists what the writer strokes the way the reader
        // reports it — arcs as their chords, a dashed pen as the pieces it
        // inks, a fill as its outline, no line along the sheet edge — so the
        // two counts are the same number, and a PDF without the template
        // comes back short by the template's share.
        let strokes = stroked_segments(model).len();
        let template_strokes = template_segments(model);
        assert!(
            template_strokes > 0,
            "{name}: the border template strokes nothing, so the page read back could not \
             show whether its frame was written"
        );
        assert_eq!(
            ink.segments.len(),
            strokes,
            "{name}: the page reads back {} ink segments; the model strokes {strokes}, \
             {template_strokes} of them the border template's",
            ink.segments.len()
        );
        // The same for the lettering: every character the writer lettered,
        // the template's cells among them, in the order it wrote them.
        let lettered = lettered_characters(model, &written.ledger);
        let template_characters = lettered.iter().filter(|(_, furniture)| *furniture).count();
        assert!(
            template_characters > 0,
            "{name}: the border template letters nothing, so the page read back could not \
             show whether its title block was written"
        );
        let read: Vec<char> = ink
            .spans
            .iter()
            .flat_map(|span| inked_characters(&span.text))
            .collect();
        if let Some(difference) = lettering_difference(&read, &lettered) {
            panic!("{name}: {difference}");
        }
        eprintln!(
            "{name}: {} on {:.2} x {:.2} mm, {strokes} ink segments ({template_strokes} the \
             template's), {} inked characters ({template_characters} the template's), {} \
             bytes; {}",
            fixture.template,
            page[0],
            page[1],
            lettered.len(),
            written.pdf.len(),
            ledger.summary()
        );
        // A name the committed sources lack is a gap in those sources, not an
        // export failure — the placement drew, as `missing_boxes` says above —
        // so it is reported and not asserted.
        if !model.missing_symbols.is_empty() {
            eprintln!(
                "{name}: {} symbol file(s) the committed sources do not carry, each drawn \
                 from the drawing's own body: {:?}",
                model.missing_symbols.len(),
                model.missing_symbols
            );
        }
    }
    eprintln!(
        "{} fixture pages exported and read back, {} faces embedded",
        pages.len(),
        prepared.len()
    );

    // The pages above search the committed symbol sources alone, so the names
    // reported for them are what those sources lack — five on the flash
    // separator sheet, whose `.sym` files only the reference share's `Ref`
    // holds — and say nothing about the reader.
    //
    // Whether a symbol is missing from every library is asked of the library
    // search itself: each drawing is laid out once more with
    // `lay_out_drawing` against the reference share, only to read which names
    // that search leaves unanswered. It unions the corpus's `Ref` with the
    // bundled `test-file/symbols-full` and the committed
    // `test-file/Ref/Symbols`, so a symbol missing from all three is missing
    // from the repository, not from the reader. No fixture is in that state:
    // the three `XA …` files the 熊安 drawing names, its own plant's library,
    // are committed beside the template it links. A regression that loses a
    // symbol fails here rather than printing a quieter page.
    //
    // That reasoning needs all three roots to be present, and the bundled one
    // is not: `.gitignore:47` keeps its 618 files out of the repository, so
    // every CI checkout searches only two. A gap reported there may be a file
    // the bundle would have answered, which is the gap in the library this
    // test's own header says to report rather than assert.
    let searched: Vec<(&str, PaperModel)> = pages
        .iter()
        .map(|fixture| {
            let model = lay_out_drawing(&fixture.path, &corpus)
                .unwrap_or_else(|error| panic!("{}: {error}", fixture.key));
            (fixture.key.as_str(), model)
        })
        .collect();
    for (name, model) in &searched {
        if !model.missing_symbols.is_empty() {
            eprintln!(
                "{name}: {} symbol files no library carries: {:?}",
                model.missing_symbols.len(),
                model.missing_symbols
            );
        }
    }
    let bundled_library = fixture_dir.join("symbols-full");
    if !bundled_library.is_dir() {
        let gaps: usize = searched
            .iter()
            .map(|(_, model)| model.missing_symbols.len())
            .sum();
        eprintln!(
            "note: {} is absent from this checkout, so the {gaps} unanswered \
             symbol name(s) above cannot be told from a repository gap",
            bundled_library.display()
        );
        return;
    }
    for (name, model) in &searched {
        assert!(
            model.missing_symbols.is_empty(),
            "{name}: symbols no library carries: {:?}",
            model.missing_symbols
        );
    }
}
