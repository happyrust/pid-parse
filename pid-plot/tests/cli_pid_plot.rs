//! `pid_plot`, the program a PDF is exported with, run as a process.
//!
//! Every other export gate calls the library. The argument handling lives in
//! the binary alone, so this gate starts the executable Cargo built and reads
//! what a shell would see: the exit code, the two streams, and whether a file
//! was written.
//!
//! The case that needs a process most is a corpus named on the command line
//! that is not a directory. The library lays a drawing out against such a path
//! without complaint — the page just carries no frame and no title block — so
//! refusing it is the program's own job, and only its exit code and stderr can
//! show that it did.
//!
//! A corpus that is a directory and still lacks the border template the
//! drawing names is the quieter version of the same mistake: the page is
//! written, bare, and exits 0. The program says so in one warning that names
//! the template and the places it was looked for, and a drawing that names no
//! template draws no such warning.
//!
//! The usage, missing-drawing and corpus cases need no typeface and assert on
//! every machine. The export embeds the faces `D06.pid` letters in, so it runs
//! wherever a font directory is present and says why it did not where none is;
//! a CI checkout carries no fonts. The two template cases assert what stderr
//! says on every machine — the warning is printed before a typeface is asked
//! for — and what was written wherever a font directory is present.
//!
//! The variables the program reads are stated for every child, never
//! inherited. `PID_PARSE_SELF_CHECK_DIR` (the corpus when none is named),
//! `PID_PLOT_FONTS` (the font search path), `PID_PLOT_CJK_FONT` (one more face
//! to embed) and `PID_SYMBOL_LIBRARY` (which replaces the symbol search) each
//! change the page or the verdict, so every case starts from all four cleared
//! and sets only what it is about. The export case is the one that hands a
//! value on — the symbol library this process would search — because it holds
//! the child's layout against one made here.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use pid_plot::fonts::font_dirs;
use pid_plot::model::{PaperModel, border_template_places, lay_out_drawing};
use pid_plot::read::read_plot;

/// The line every argument error puts on stderr.
const USAGE: &str = "usage: pid_plot <drawing.pid> <out.pdf> [corpus]";

/// Every variable the program reads; see the module note.
const VARIABLES_THE_PROGRAM_READS: [&str; 4] = [
    "PID_PARSE_SELF_CHECK_DIR",
    "PID_PLOT_FONTS",
    "PID_PLOT_CJK_FONT",
    "PID_SYMBOL_LIBRARY",
];

/// The repository's fixture directory. It is a corpus as well as a set of
/// drawings: its `Ref` carries the border templates and the project symbols
/// the fixtures name.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test-file")
}

/// `test-file/D06.pid`, an A2 sheet whose template `CPECCHB-SQ-A2.igr` is
/// committed under `test-file/Ref/Template Files`.
fn d06() -> PathBuf {
    fixtures().join("D06.pid")
}

/// `test-file/export-test/publish-data/A01/A01.pid`, an A2 sheet whose frame
/// links no template file at all: the one fixture that names none.
fn a01() -> PathBuf {
    fixtures()
        .join("export-test")
        .join("publish-data")
        .join("A01")
        .join("A01.pid")
}

/// A directory of `case`'s own under Cargo's test tmpdir. Cases run side by
/// side, so none of them shares an output name with another.
fn scratch(case: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli_pid_plot")
        .join(case);
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|error| panic!("mkdir {}: {error}", dir.display()));
    dir
}

/// `name` inside `dir`, with whatever an earlier run left there removed, so a
/// file found afterwards is one this run wrote.
fn unwritten(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("remove {}: {error}", path.display()),
    }
    path
}

/// What one run of the program left behind.
struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        )
    }
}

/// `pid_plot` with every variable it reads cleared.
fn pid_plot() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pid_plot"));
    for name in VARIABLES_THE_PROGRAM_READS {
        command.env_remove(name);
    }
    command
}

fn run(command: &mut Command) -> Outcome {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("spawn pid_plot: {error}"));
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The lines of `stderr` that are warnings.
fn warnings(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect()
}

/// The corpus a machine that has the licensed faces keeps them under:
/// `PID_PARSE_SELF_CHECK_DIR`, else the path `pid_plot` itself falls back to.
fn shared_corpus() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || {
            Path::new("D:/")
                .join("work")
                .join("cad")
                .join("PID自研软件验证")
        },
        PathBuf::from,
    )
}

/// The font directories present on this machine, as the `PID_PLOT_FONTS`
/// value a child is handed. `Err` lists where the search looked and found no
/// directory at all.
fn fonts_for_a_child() -> Result<OsString, String> {
    let searched = font_dirs(Some(&shared_corpus()));
    let present: Vec<&PathBuf> = searched.iter().filter(|dir| dir.is_dir()).collect();
    if present.is_empty() {
        let searched: Vec<String> = searched
            .iter()
            .map(|dir| dir.display().to_string())
            .collect();
        return Err(searched.join(", "));
    }
    Ok(std::env::join_paths(present).unwrap_or_else(|error| {
        panic!("the font directories do not join into one search path: {error}")
    }))
}

#[test]
fn no_arguments_print_the_usage_and_exit_2() {
    let outcome = run(&mut pid_plot());
    assert_eq!(outcome.code, Some(2), "{outcome}");
    assert!(outcome.stderr.contains(USAGE), "{outcome}");
    assert!(outcome.stdout.is_empty(), "{outcome}");
}

#[test]
fn a_drawing_with_no_output_named_prints_the_usage_and_exits_2() {
    let outcome = run(pid_plot().arg(d06()));
    assert_eq!(outcome.code, Some(2), "{outcome}");
    assert!(outcome.stderr.contains(USAGE), "{outcome}");
    assert!(outcome.stdout.is_empty(), "{outcome}");
}

#[test]
fn a_drawing_that_does_not_exist_exits_1_and_writes_nothing() {
    let dir = scratch("drawing-absent");
    let drawing = dir.join("no-such-drawing.pid");
    let out = unwritten(&dir, "no-such-drawing.pdf");
    let outcome = run(pid_plot().arg(&drawing).arg(&out).arg(fixtures()));
    assert_eq!(outcome.code, Some(1), "{outcome}");
    assert!(
        outcome.stderr.contains(&drawing.display().to_string()),
        "stderr does not name the drawing {}\n{outcome}",
        drawing.display()
    );
    assert!(
        outcome.stdout.is_empty(),
        "a failed run reported a page\n{outcome}"
    );
    assert!(
        !out.exists(),
        "{} was written for a drawing that does not exist",
        out.display()
    );
}

/// Start `pid_plot` on `D06.pid` with `corpus` named on the command line and
/// check it was turned away as an argument error.
///
/// The drawing is real and the fonts are handed over wherever this machine
/// has them, so a program that went on to export would have everything a page
/// needs: the file that is not there afterwards is the refusal's doing, not a
/// shortage's.
fn assert_the_named_corpus_is_refused(case: &str, corpus: &Path) {
    let out = unwritten(&scratch(case), "D06.pdf");
    let mut command = pid_plot();
    if let Ok(fonts) = fonts_for_a_child() {
        command.env("PID_PLOT_FONTS", fonts);
    }
    let outcome = run(command.arg(d06()).arg(&out).arg(corpus));
    assert_eq!(
        outcome.code,
        Some(2),
        "the corpus {} is not a directory and was not refused\n{outcome}",
        corpus.display()
    );
    assert!(
        outcome.stderr.contains(&corpus.display().to_string()),
        "stderr does not name the corpus {}\n{outcome}",
        corpus.display()
    );
    assert!(
        outcome.stdout.is_empty(),
        "a refused run reported a page\n{outcome}"
    );
    assert!(
        !out.exists(),
        "{} was written against a corpus that is not a directory",
        out.display()
    );
}

#[test]
fn a_named_corpus_that_does_not_exist_is_refused() {
    let corpus = scratch("corpus-absent").join("no-such-corpus");
    assert_the_named_corpus_is_refused("corpus-absent", &corpus);
}

/// A path that exists is still not a corpus unless it is a directory. The
/// file named here is another fixture rather than the drawing itself, so a
/// message about the drawing cannot pass for one about the corpus.
#[test]
fn a_named_corpus_that_is_a_file_is_refused() {
    let corpus = fixtures().join("DWG-0201GP06-01.pid");
    assert!(corpus.is_file(), "{} is not a file", corpus.display());
    assert_the_named_corpus_is_refused("corpus-is-a-file", &corpus);
}

/// Start `pid_plot` on `D06.pid` with no corpus named and `fallback` in
/// `PID_PARSE_SELF_CHECK_DIR`, and check the export went as far as this
/// machine lets any export go.
///
/// With a font directory to hand that is a written page. Without one the run
/// stops at the first face `D06` letters in, exit 1 — which is still past the
/// corpus, because an argument error is exit 2.
fn export_d06_against_a_fallback(case: &str, fallback: &Path) -> Outcome {
    let out = unwritten(&scratch(case), "D06.pdf");
    let fonts = fonts_for_a_child();
    let mut command = pid_plot();
    command.env("PID_PARSE_SELF_CHECK_DIR", fallback);
    if let Ok(fonts) = &fonts {
        command.env("PID_PLOT_FONTS", fonts);
    }
    let outcome = run(command.arg(d06()).arg(&out));
    match fonts {
        Ok(_) => {
            assert_eq!(outcome.code, Some(0), "{outcome}");
            let pdf = std::fs::read(&out)
                .unwrap_or_else(|error| panic!("{}: {error}\n{outcome}", out.display()));
            let ink = read_plot(&pdf)
                .unwrap_or_else(|error| panic!("{}: read back: {error}", out.display()));
            assert!(
                !ink.segments.is_empty(),
                "{}: the page carries no ink at all",
                out.display()
            );
        }
        Err(searched) => {
            eprintln!(
                "{case}: no font directory is present (searched {searched}), so the run \
                 can only get as far as the first missing face"
            );
            assert_eq!(outcome.code, Some(1), "{outcome}");
        }
    }
    outcome
}

/// The fallback corpus is a guess about the machine — the built-in path, or
/// whatever `PID_PARSE_SELF_CHECK_DIR` says — so one that is absent is said on
/// stderr and the drawing is exported all the same.
#[test]
fn an_absent_fallback_corpus_is_a_warning_and_the_export_goes_on() {
    let corpus = scratch("fallback-absent").join("no-such-corpus");
    let outcome = export_d06_against_a_fallback("fallback-absent", &corpus);
    // The warning about the template `D06` names lists the directories under
    // this corpus too, so naming the path is not enough to be the warning
    // about the corpus: it has to say what is wrong with it.
    let named = corpus.display().to_string();
    assert!(
        warnings(&outcome.stderr)
            .iter()
            .any(|line| line.contains(&named) && line.contains("is not a directory")),
        "no warning on stderr says the fallback corpus {named} is not a directory\n{outcome}"
    );
}

/// The other half of the rule above: a fallback that is there is not worth a
/// word.
#[test]
fn a_fallback_corpus_that_exists_draws_no_warning() {
    let outcome = export_d06_against_a_fallback("fallback-present", &fixtures());
    assert!(
        warnings(&outcome.stderr).is_empty(),
        "a corpus that exists drew a warning\n{outcome}"
    );
}

/// The first line `pid_plot` prints for a page: how many primitives were laid
/// out, on what sheet, into how many bytes.
fn page_line(model: &PaperModel, pdf_bytes: usize) -> String {
    let page = model
        .resolved_page_mm()
        .unwrap_or_else(|| panic!("the drawing states no page and has no ink to measure"));
    format!(
        "primitives {} segments_written_page_mm {:.2} {:.2} bytes {}",
        model.primitives.len(),
        page[0],
        page[1],
        pdf_bytes
    )
}

/// Start `pid_plot` on `drawing` with an empty corpus of `case`'s own named
/// on the command line, and check the export went as far as this machine lets
/// any export go. Returns what the run left, the library's layout of the same
/// drawing against the same corpus, and the corpus.
///
/// The corpus is a directory, so the program takes it; neither it nor the
/// directory it sits in holds a `Ref`, a `RefData~*.zip` or a backup package,
/// so no border template is in reach and the page is bare. With a font
/// directory to hand the run exits 0 and the file is the page the library
/// lays out — the same primitives, none of them a frame's. Without one it
/// stops at the first face the drawing letters in, exit 1, with nothing
/// written.
fn export_against_a_corpus_with_no_templates(
    case: &str,
    drawing: &Path,
) -> (Outcome, PaperModel, PathBuf) {
    let dir = scratch(case);
    let corpus = dir.join("corpus");
    std::fs::create_dir_all(&corpus)
        .unwrap_or_else(|error| panic!("mkdir {}: {error}", corpus.display()));
    let out = unwritten(&dir, "page.pdf");
    let fonts = fonts_for_a_child();
    let mut command = pid_plot();
    if let Ok(fonts) = &fonts {
        command.env("PID_PLOT_FONTS", fonts);
    }
    // The child's layout is held against one made here, so it is handed the
    // symbol library this process would search; see the D06 export below.
    if let Some(library) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        command.env("PID_SYMBOL_LIBRARY", library);
    }
    let outcome = run(command.arg(drawing).arg(&out).arg(&corpus));
    let model = lay_out_drawing(drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    assert!(
        model.template.is_none(),
        "{} found a border template under the empty corpus {}",
        drawing.display(),
        corpus.display()
    );
    match fonts {
        Ok(_) => {
            assert_eq!(outcome.code, Some(0), "{outcome}");
            let pdf = std::fs::read(&out)
                .unwrap_or_else(|error| panic!("{}: {error}\n{outcome}", out.display()));
            assert_eq!(
                outcome.stdout.lines().next(),
                Some(page_line(&model, pdf.len()).as_str()),
                "the command line did not report the bare page the library lays out\n{outcome}"
            );
            let ink = read_plot(&pdf)
                .unwrap_or_else(|error| panic!("{}: read back: {error}", out.display()));
            assert!(
                !ink.segments.is_empty(),
                "{}: the page carries no ink at all",
                out.display()
            );
        }
        Err(searched) => {
            eprintln!(
                "{case}: no font directory is present (searched {searched}), so the run \
                 can only get as far as the first missing face"
            );
            assert_eq!(outcome.code, Some(1), "{outcome}");
            assert!(
                !out.exists(),
                "{} was written by a run that failed",
                out.display()
            );
        }
    }
    (outcome, model, corpus)
}

/// A sheet that names its border template wears only that template, so a
/// corpus that cannot answer the name leaves the page with no frame and no
/// title block. The export goes on as it would for any bare page; what is
/// missing, and where it was looked for, is one warning on stderr.
#[test]
fn a_named_template_the_corpus_lacks_is_one_warning_and_the_export_goes_on() {
    let drawing = d06();
    let (outcome, model, corpus) =
        export_against_a_corpus_with_no_templates("template-absent", &drawing);
    // The library's own account of the gap: the stems `D06`'s two link
    // records spell, the 8.3 short name `CPECCH~2.IGR` and the long
    // `CPECCHB-SQ-A2.igr`.
    assert_eq!(
        model.missing_templates,
        ["CPECCH", "CPECCHB-SQ-A2"],
        "the library names another gap for D06"
    );
    let warned = warnings(&outcome.stderr);
    assert_eq!(
        warned.len(),
        1,
        "a template that was named and not found is exactly one warning\n{outcome}"
    );
    let line = warned[0];
    assert!(
        line.contains(&drawing.display().to_string()),
        "the warning does not say which drawing it is about\n{outcome}"
    );
    for stem in &model.missing_templates {
        assert!(
            line.contains(stem.as_str()),
            "the warning does not name the template {stem}\n{outcome}"
        );
    }
    // Where the file would have to be, in the order it is looked for: the
    // corpus's own template directory comes first.
    let places = border_template_places(&drawing, &corpus);
    assert_eq!(
        places.directories.first(),
        Some(&corpus.join("Ref").join("Template Files")),
        "the first place a template is looked for is not the corpus's own"
    );
    for directory in &places.directories {
        assert!(
            line.contains(&directory.display().to_string()),
            "the warning does not name the searched directory {}\n{outcome}",
            directory.display()
        );
    }
}

/// The other half of the rule above: a sheet whose frame links no template
/// has named nothing, so the same empty corpus is not worth a word about one.
#[test]
fn a_drawing_that_names_no_template_draws_no_warning() {
    let (outcome, model, _) = export_against_a_corpus_with_no_templates("template-unnamed", &a01());
    assert!(
        model.missing_templates.is_empty(),
        "A01 names a template after all: {:?}",
        model.missing_templates
    );
    assert!(
        warnings(&outcome.stderr).is_empty(),
        "a sheet that names no template drew a warning\n{outcome}"
    );
}

#[test]
fn d06_exports_the_page_the_library_lays_out() {
    let fonts = match fonts_for_a_child() {
        Ok(fonts) => fonts,
        Err(searched) => {
            eprintln!(
                "skipping the D06 export; no font directory is present (searched {searched})"
            );
            return;
        }
    };
    let drawing = d06();
    let corpus = fixtures();
    let out = unwritten(&scratch("export-d06"), "D06.pdf");
    let mut command = pid_plot();
    command.env("PID_PLOT_FONTS", &fonts);
    // `lay_out_drawing` below reads `PID_SYMBOL_LIBRARY` in this process, and
    // a process cannot clear a variable for itself while its other threads
    // read the environment. The two layouts are compared, so the child is
    // handed the value this process has and both are asked the same question.
    if let Some(library) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        command.env("PID_SYMBOL_LIBRARY", library);
    }
    let outcome = run(command.arg(&drawing).arg(&out).arg(&corpus));
    assert_eq!(outcome.code, Some(0), "{outcome}");
    assert!(
        outcome.stderr.is_empty(),
        "a clean export said something on stderr\n{outcome}"
    );

    // The library's layout of the same drawing against the same corpus. The
    // frame is in it — the template's own primitives are part of the count —
    // so a command line that lost the corpus on the way, and with it the
    // frame, reports fewer.
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let template = model.template.as_ref().unwrap_or_else(|| {
        panic!(
            "the library found D06 no border template under {}",
            corpus.display()
        )
    });
    assert!(
        template.primitives > 0,
        "{} put nothing on the page",
        template.path
    );
    let page = model
        .resolved_page_mm()
        .unwrap_or_else(|| panic!("D06 states no page and has no ink to measure"));
    let pdf = std::fs::read(&out).unwrap_or_else(|error| panic!("{}: {error}", out.display()));

    // The first line is the page's own account: how many primitives were laid
    // out, on what sheet, into how many bytes. Each figure is held against the
    // thing it describes — the library's model, and the file on disk.
    assert_eq!(
        outcome.stdout.lines().next(),
        Some(page_line(&model, pdf.len()).as_str()),
        "the command line did not report the page the library lays out\n{outcome}"
    );

    let ink =
        read_plot(&pdf).unwrap_or_else(|error| panic!("{}: read back: {error}", out.display()));
    // 0.05 mm is what every export gate reads a written page's size back to.
    assert!(
        (ink.page_mm[0] - page[0]).abs() < 0.05,
        "page width {} against the model's {}",
        ink.page_mm[0],
        page[0]
    );
    assert!(
        (ink.page_mm[1] - page[1]).abs() < 0.05,
        "page height {} against the model's {}",
        ink.page_mm[1],
        page[1]
    );
    assert!(!ink.segments.is_empty(), "the page carries no ink at all");
    assert!(
        !ink.spans.is_empty(),
        "the page carries no lettering at all"
    );

    // The ledger's summary line ends with what no layer accounts for, and a
    // page that balanced says zero.
    let ledger = outcome
        .stdout
        .lines()
        .find(|line| line.starts_with("ledger "))
        .unwrap_or_else(|| panic!("stdout carries no ledger summary\n{outcome}"));
    let mut words = ledger.split_whitespace();
    assert!(
        words.any(|word| word == "unattributed"),
        "the ledger summary states no remainder: {ledger}"
    );
    assert_eq!(
        words.next(),
        Some("0"),
        "the page's account does not balance: {ledger}"
    );
}
