//! Every `.pid` fixture in this repository exports a PDF.
//!
//! The five corpus drawings have their own gate (`compare_five_drawings`,
//! `write_five_drawings`); this one covers the drawings that ship inside
//! `test-file/`, where a page that cannot be written at all — DWG-0202's note
//! carried a `U+000D` no typeface can encode — would otherwise only be found by
//! running the CLI by hand.
//!
//! A drawing may still name symbol files the reference library does not carry;
//! that is a gap in the library, not an export failure, so it is reported and
//! not asserted here.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, write_plot};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
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

#[test]
fn every_fixture_drawing_exports_a_pdf() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-file");
    let mut fixtures = Vec::new();
    collect_pids(&fixture_dir, &mut fixtures);
    if fixtures.is_empty() {
        eprintln!("skipping fixture export; test-file/ carries no .pid");
        return;
    }
    let corpus = corpus_dir();
    let dirs = font_dirs(Some(&corpus));
    if !dirs.iter().any(|dir| dir.is_dir()) {
        eprintln!("skipping fixture export; no font directory is present");
        return;
    }

    let mut models: Vec<(PathBuf, PaperModel)> = Vec::new();
    for path in &fixtures {
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
        let pdf = write_plot(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        assert!(
            !ink.segments.is_empty(),
            "{name}: the page carries no ink at all"
        );
        assert!(
            !ink.spans.is_empty(),
            "{name}: the page carries no lettering at all"
        );
        if !model.missing_symbols.is_empty() {
            eprintln!(
                "{name}: {} symbol files no library carries: {:?}",
                model.missing_symbols.len(),
                model.missing_symbols
            );
        }
    }

    // The library search now unions the corpus's `Ref` with the bundled
    // `test-file/symbols-full`, so a symbol missing from both is missing from
    // the repository, not from the reader. Only one fixture is in that state:
    // its three `XA …` files are the project library this set was drawn with,
    // and no `.sym` of those names exists anywhere in the tree. Pinned as a
    // ratchet so a regression that loses a symbol shows up as a failure rather
    // than a quieter page.
    const KNOWN_LIBRARY_GAPS: [&str; 3] = [
        "design\\annotation\\labels\\xa item note & label.sym",
        "piping\\piping opc's\\xa chu.sym",
        "piping\\piping opc's\\xa.sym",
    ];
    for (path, model) in &models {
        let missing: Vec<&str> = model.missing_symbols.iter().map(String::as_str).collect();
        if missing == KNOWN_LIBRARY_GAPS {
            continue;
        }
        assert!(
            missing.is_empty(),
            "{}: symbols no library carries: {missing:?}",
            path.display()
        );
    }
}
