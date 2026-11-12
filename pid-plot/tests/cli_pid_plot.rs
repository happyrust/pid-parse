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
//! A drawing one of whose streams has a record chain that does not hold is
//! the third: the page carries what the records before the break draw, and
//! exits 0. Three damaged copies of `DWG-0201GP06-01.pid` hold the program to
//! one warning that names the stream, the offset the chain stops at and the
//! bytes left unread, the page to the records before the break, and the
//! page's account to filing the stream as a suspect drop; a fourth copy,
//! rewritten with nothing changed, holds it to saying nothing.
//!
//! Those three lengths run to zero or past the stream, where any walk stops.
//! Two more copies carry a length that is wrong and still inside the stream:
//! the walk goes on from an offset where no record starts and stops on the
//! word it finds there, which no record opens with. Without that tripwire
//! the first of them reads as a sound stream of 359 records — a page short
//! of one text box and not a word said — and the second reads on through
//! bytes that are not records to the stream's end. The program is held to
//! one warning that names the first byte past the damaged record, the word
//! found there and the record itself, and the page to the records before it.
//!
//! A symbol file one of whose streams has a record chain that does not hold
//! is the fourth, and the one a drawing's own bytes cannot show: a placement
//! is drawn from the body its drawing caches, and the library's `.sym` is
//! read only for a placement the drawing holds no body of. A copy of the
//! `一级闪蒸分离器B` fixture whose pressure gauge names a body the drawing
//! does not hold is drawn, for that one placement, from a library of one
//! file: the committed gauge, sound or damaged. The program is held to
//! saying nothing of the sound file and to one warning of the damaged one —
//! the file, the stream, where its chain stops — and the page's account to
//! one suspect item; the fixture as committed, which draws the gauge from
//! its cache, is held to saying nothing whatever the library holds.
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
//! the child's layout against one made here; the symbol-file cases name a
//! library of their own making.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use pid_plot::fonts::font_dirs;
use pid_plot::ledger::{
    DomainAccount, DropReason, ExportLedger, LedgerDomain, PlacementSource, Tier, UnplacedWhy,
};
use pid_plot::model::{
    PaperModel, PaperPrimitive, PaperShape, border_template_places, lay_out_drawing,
    lay_out_with_symbol_roots,
};
use pid_plot::read::read_plot;
use pid_plot::write::write_ledger;

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

/// `test-file/DWG-0201GP06-01.pid`, the drawing an independent review of the
/// chain-gated decoders broke three ways.
fn dwg_0201() -> PathBuf {
    fixtures().join("DWG-0201GP06-01.pid")
}

/// The stream that drawing keeps its own ink in.
const DWG_0201_SHEET: &str = "/Sheet6";

/// One way of damaging the record chain of `DWG-0201GP06-01.pid`'s
/// `/Sheet6`, each a single length field.
#[derive(Clone, Copy, Debug)]
enum Break {
    /// Nothing is changed: the drawing rewritten as it is.
    None,
    /// The record in the middle of the chain states a length of zero.
    MiddleLengthZero,
    /// The last record states one byte more than the stream has.
    LastLengthPlusOne,
    /// The last record states four bytes fewer than it has, which leaves
    /// four stray bytes at the stream's end.
    LastLengthMinusFour,
    /// The header counts one live record more than the chain holds. No
    /// length is touched, so the walk reaches the stream's end.
    HeaderCountPlusOne,
    /// The first record states eight bytes more than it has: a length that
    /// is wrong and still inside the stream. The walk goes on from eight
    /// bytes into the second record.
    FirstLengthPlusEight,
    /// The thirty-first record states sixteen bytes fewer than it has. The
    /// walk goes on from sixteen bytes short of the thirty-second.
    ThirtyFirstLengthMinusSixteen,
}

/// A copy of `DWG-0201GP06-01.pid` with one [`Break`] in its `/Sheet6`.
struct Broken {
    /// The copy, in `case`'s own scratch directory.
    path: PathBuf,
    /// The same drawing with a `/Sheet6` that is the records before the
    /// stop and nothing else, behind a header that counts them: a stream
    /// with nothing wrong with it, holding exactly what the broken one is
    /// read as.
    sound_prefix: PathBuf,
    /// Where the chain walk has to stop, and how many bytes follow it;
    /// `None` for [`Break::None`].
    stop: Option<(usize, usize)>,
    /// Offset of the record the break touches: every record ending at or
    /// before it is untouched. The stream's length when no record is
    /// touched.
    first_touched: usize,
    /// The live-record count the header is made to state, when the break
    /// is that count and nothing stops the walk.
    header_states: Option<u32>,
    /// Offset of the stream's first record.
    first_record: usize,
    /// When the break is a length that is wrong and still in bounds: the
    /// walk stops where that length ends, which is no record's start, and
    /// the fault names the record the length belongs to.
    wrong_length_of: Option<usize>,
}

/// Write `how`'s copy of the drawing under `case`'s scratch directory.
///
/// The copy is the drawing's own compound file rebuilt with one stream
/// replaced (`pid_parse::writer`), so every other stream — the symbol
/// caches, the style tables, the layers — is byte for byte the original's.
fn broken_copy(case: &str, how: Break) -> Broken {
    use pid_parse::parsers::sheet_records::{SheetChain, sheet_record_starts};
    use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};

    let source = dwg_0201();
    let package = pid_parse::PidParser::new()
        .parse_package(&source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    let mut data = package
        .get_stream(DWG_0201_SHEET)
        .unwrap_or_else(|| panic!("{} has no {DWG_0201_SHEET}", source.display()))
        .data
        .clone();
    let starts = sheet_record_starts(&data);
    assert!(
        starts.len() > 10,
        "{}'s {DWG_0201_SHEET} walks {} records",
        source.display(),
        starts.len()
    );
    let len = data.len();
    let last = starts[starts.len() - 1];
    let stated = |data: &[u8], at: usize| {
        u32::from_le_bytes([data[at + 2], data[at + 3], data[at + 4], data[at + 5]])
    };
    let state = |data: &mut [u8], at: usize, value: u32| {
        data[at + 2..at + 6].copy_from_slice(&value.to_le_bytes());
    };
    let mut header_states = None;
    let mut wrong_length_of = None;
    // A length changed by `delta` and still inside the stream: the walk
    // has to stop at the first byte past what the record now states.
    let mut in_bounds = |data: &mut [u8], record: usize, delta: i64| {
        let wrong = u32::try_from(i64::from(stated(data, record)) + delta).expect("a length");
        state(data, record, wrong);
        wrong_length_of = Some(record);
        let stop = record + 6 + wrong as usize;
        assert!(
            stop < len,
            "{how:?}: the wrong length stays inside the stream"
        );
        (Some((stop, len - stop)), record)
    };
    let (stop, first_touched) = match how {
        Break::FirstLengthPlusEight => in_bounds(&mut data, starts[0], 8),
        Break::ThirtyFirstLengthMinusSixteen => in_bounds(&mut data, starts[30], -16),
        Break::None => (None, len),
        Break::HeaderCountPlusOne => {
            let live = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
            data[4..8].copy_from_slice(&(live + 1).to_le_bytes());
            header_states = Some(live + 1);
            (None, len)
        }
        Break::MiddleLengthZero => {
            let middle = starts[starts.len() / 2];
            state(&mut data, middle, 0);
            (Some((middle, len - middle)), middle)
        }
        Break::LastLengthPlusOne => {
            let length = stated(&data, last);
            state(&mut data, last, length + 1);
            (Some((last, len - last)), last)
        }
        Break::LastLengthMinusFour => {
            let length = stated(&data, last);
            state(&mut data, last, length - 4);
            (Some((len - 4, 4)), last)
        }
    };
    // What the walk reads of the broken stream, as a stream of its own.
    let chain = SheetChain::walk(&data);
    assert_eq!(
        chain.stop.is_some().then_some(chain.end),
        stop.map(|(at, _)| at),
        "{how:?}: where the walk stops"
    );
    let mut prefix = data[..chain.end].to_vec();
    let live = u32::try_from(chain.live_records).expect("a record count");
    prefix[4..8].copy_from_slice(&live.to_le_bytes());
    assert!(
        SheetChain::walk(&prefix).is_sound(),
        "{how:?}: the records before the stop are a stream with nothing wrong with it"
    );

    let write = |name: &str, new_data: Vec<u8>| {
        let plan = WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: DWG_0201_SHEET.to_string(),
                new_data,
            }],
            ..WritePlan::default()
        };
        let dir = scratch(case).join(name);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|error| panic!("mkdir {}: {error}", dir.display()));
        let path = dir.join("DWG-0201GP06-01.pid");
        PidWriter::write_to(&package, &plan, &path)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        path
    };
    Broken {
        path: write("broken", data),
        sound_prefix: write("sound-prefix", prefix),
        stop,
        first_touched,
        header_states,
        first_record: starts[0],
        wrong_length_of,
    }
}

/// What one page frame says about the page. The layout places the
/// drawing's ink by these.
#[derive(Debug, PartialEq)]
struct PageFrame {
    /// The page's extent, metres.
    extent_m: Option<(f64, f64)>,
    /// The frame's own corner, metres.
    corner_m: Option<[f64; 2]>,
    /// Where the frame puts its content: the translation in metres, and
    /// the scale.
    content: Option<[f64; 3]>,
}

/// The page frames a drawing's sheets hold, in stream order.
fn page_frames(path: &Path) -> Vec<PageFrame> {
    let doc = pid_parse::PidParser::new()
        .parse_file(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    doc.sheet_streams
        .iter()
        .filter_map(|sheet| sheet.geometry.as_ref())
        .flat_map(|geometry| &geometry.decoded_igsmartframes)
        .map(|frame| PageFrame {
            extent_m: frame.page_extent_m(),
            corner_m: frame.range_origin_m,
            content: frame.content_transform.as_ref().map(|placed| {
                [
                    placed.translation_m[0],
                    placed.translation_m[1],
                    placed.scale,
                ]
            }),
        })
        .collect()
}

/// What a primitive is, apart from where the page puts it.
fn identity(primitive: &PaperPrimitive) -> impl PartialEq + std::fmt::Debug + '_ {
    (
        primitive.confidence,
        primitive.stream_path.as_deref(),
        primitive.byte_range,
        primitive.symbol.as_deref(),
        primitive.stroke.as_ref(),
        std::mem::discriminant(&primitive.shape),
    )
}

/// The drawing's own decoded primitives that come from records of `/Sheet6`
/// ending at or before `before`.
fn drawn_from_the_sheet_before(model: &PaperModel, before: usize) -> Vec<&PaperPrimitive> {
    model
        .primitives
        .iter()
        .filter(|primitive| {
            !primitive.furniture
                && primitive.confidence == "decoded"
                && primitive.stream_path.as_deref() == Some(DWG_0201_SHEET)
                && primitive.byte_range.is_some_and(|range| range[1] <= before)
        })
        .collect()
}

/// The count after `word` on the ledger line `pid_plot` prints.
fn ledger_count(outcome: &Outcome, word: &str) -> u64 {
    let ledger = outcome
        .stdout
        .lines()
        .find(|line| line.starts_with("ledger "))
        .unwrap_or_else(|| panic!("stdout carries no ledger summary\n{outcome}"));
    let mut words = ledger.split_whitespace();
    assert!(
        words.any(|found| found == word),
        "the ledger summary states no {word}: {ledger}"
    );
    words
        .next()
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("the ledger summary's {word} is not a count: {ledger}"))
}

/// Export `how`'s copy of `DWG-0201GP06-01.pid` against the repository's
/// fixtures, as far as this machine lets any export go, and hold what the
/// program said and what the library laid out to the break.
///
/// A sheet whose record chain stops short still exports — the records
/// before the stop are the drawing's, and a page with most of the drawing on
/// it is worth more than none — so the exit code is the one the intact
/// drawing gets. What it must not do is look like a finished export: the
/// page is short of ink nobody listed. So the program says, in one warning,
/// which stream stopped, at which offset and how many bytes were left
/// unread, and the page's ledger files the stream as a suspect drop with
/// those bytes, where the gates that read ledgers see it.
///
/// What the page carries is held twice. It is, primitive for primitive, the
/// page of the same drawing with a sound `/Sheet6` that ends where the walk
/// stopped: nothing past the stop reaches the page by any route. And every
/// record before the stop draws what it draws on the drawing as committed —
/// the same primitives from the same bytes with the same pens — at the same
/// place wherever the page frame is among the records read. This drawing
/// keeps its one live frame as the last record of the stream, after five
/// superseded copies, so a chain that stops anywhere short of it is laid out
/// as a sheet with no frame is: on the page its template's name states, and
/// without the 1.105 mm by 1.841 mm the frame moves the drawing's ink by.
fn export_a_broken_chain(case: &str, how: Break) {
    let broken = broken_copy(case, how);
    let corpus = fixtures();
    let out = unwritten(&scratch(case), "page.pdf");
    let fonts = fonts_for_a_child();
    let mut command = pid_plot();
    if let Ok(fonts) = &fonts {
        command.env("PID_PLOT_FONTS", fonts);
    }
    // The child's layout is held against ones made here; see the D06 export.
    if let Some(library) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        command.env("PID_SYMBOL_LIBRARY", library);
    }
    let outcome = run(command.arg(&broken.path).arg(&out).arg(&corpus));

    // What the library lays out: the copy, and the drawing as committed.
    let model = lay_out_drawing(&broken.path, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", broken.path.display()));
    let intact = lay_out_drawing(&dwg_0201(), &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", dwg_0201().display()));

    // The page is the page of the records before the stop: the same
    // drawing with a sound stream that ends there lays out to the same
    // primitives on the same sheet, so nothing past the stop is drawn.
    let sound = lay_out_drawing(&broken.sound_prefix, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", broken.sound_prefix.display()));
    assert_eq!(
        model.resolved_page_mm(),
        sound.resolved_page_mm(),
        "{how:?}: the sheet"
    );
    assert_eq!(
        model.template.as_ref().map(|template| &template.path),
        sound.template.as_ref().map(|template| &template.path),
        "{how:?}: the border template"
    );
    assert_eq!(
        model.primitives.len(),
        sound.primitives.len(),
        "{how:?}: primitives on the page, against the page of the sound prefix"
    );
    for (index, (drawn, expected)) in model.primitives.iter().zip(&sound.primitives).enumerate() {
        assert_eq!(
            drawn, expected,
            "{how:?}: primitive {index} is not the sound prefix's"
        );
    }
    // And it is painted in the sheet's own order as far as the sheet was
    // read. The list of what the sheet draws is in the document storage,
    // which the break does not touch, so it still places every record
    // before the stop; a page that fell back to strokes first and labels
    // after them would paint the same records another way than the drawing
    // as committed does.
    let painted = model.paint_sequence();
    assert!(
        painted.is_some(),
        "{how:?}: the records before the stop are not painted in the sheet's order"
    );
    assert_eq!(
        painted,
        sound.paint_sequence(),
        "{how:?}: the order the page is painted in, against the page of the sound prefix"
    );

    // Every record the break leaves alone puts on the page what it puts
    // there on the intact drawing: the same primitives, from the same bytes,
    // with the same pens, in the same order.
    let before = drawn_from_the_sheet_before(&intact, broken.first_touched);
    let after = drawn_from_the_sheet_before(&model, broken.first_touched);
    // Something is drawn from the records before the break, unless the
    // break is in the first record and there are none.
    assert_eq!(
        before.is_empty(),
        broken.first_touched == broken.first_record,
        "{how:?}: what the intact drawing draws from the records before +{}",
        broken.first_touched
    );
    assert_eq!(
        after.len(),
        before.len(),
        "{how:?}: primitives drawn from the records before +{}",
        broken.first_touched
    );
    for (drawn, expected) in after.iter().zip(&before) {
        assert_eq!(
            identity(drawn),
            identity(expected),
            "{how:?}: the same primitives, in the same order"
        );
    }
    // And at the same place exactly when the frames read say the same of
    // the page as the intact drawing's do; see the note above. A break in
    // the first record leaves nothing before it to place.
    let same_frames = page_frames(&broken.path) == page_frames(&dwg_0201());
    if !before.is_empty() {
        assert_eq!(
            after == before,
            same_frames,
            "{how:?}: the records before the stop are placed as on the intact drawing exactly \
             when the page frame is among them (frames read: {:?})",
            page_frames(&broken.path)
        );
    }

    // The page's ledger: the stream, once, as a suspect drop of the bytes
    // the walk left unread — or not at all when nothing is broken.
    let filed: Vec<_> = model
        .ledger
        .parse
        .drawing
        .items
        .iter()
        .filter(|item| item.reason == DropReason::SheetStreamWithoutGeometry)
        .map(|item| (item.stream.as_str(), item.count, item.bytes))
        .collect();
    let warned = warnings(&outcome.stderr);
    match (broken.stop, broken.header_states) {
        (None, None) => {
            assert_eq!(model.primitives.len(), intact.primitives.len());
            assert!(filed.is_empty(), "{how:?}: filed {filed:?}");
            assert!(
                warned.is_empty(),
                "{how:?}: a drawing rewritten as it is drew a warning\n{outcome}"
            );
        }
        // Every record is read and the page is the drawing's. The header
        // is all that says records may be missing, so the stream is filed
        // all the same, with no byte unread, and the program says it.
        (None, Some(states)) => {
            assert_eq!(model.primitives.len(), intact.primitives.len());
            assert_eq!(
                filed,
                [(DWG_0201_SHEET, 1, Some(0))],
                "{how:?}: the stream is filed once, with no byte unread"
            );
            assert_eq!(
                warned.len(),
                1,
                "{how:?}: a header that disagrees with its chain is exactly one warning\n{outcome}"
            );
            let line = warned[0];
            for needle in [
                broken.path.display().to_string(),
                DWG_0201_SHEET.to_string(),
                format!("live record(s) where the header states {states}"),
            ] {
                assert!(
                    line.contains(&needle),
                    "{how:?}: the warning does not say `{needle}`\n{outcome}"
                );
            }
            assert!(
                !line.contains("stops at"),
                "{how:?}: the walk reaches the stream's end\n{outcome}"
            );
        }
        (Some((stop, unread)), _) => {
            assert_eq!(
                filed,
                [(DWG_0201_SHEET, 1, Some(unread as u64))],
                "{how:?}: the stream is filed once with the bytes left unread"
            );
            assert_eq!(
                warned.len(),
                1,
                "{how:?}: a chain that stops short is exactly one warning\n{outcome}"
            );
            let line = warned[0];
            for needle in [
                broken.path.display().to_string(),
                DWG_0201_SHEET.to_string(),
                format!("stops at +{stop} "),
                format!("{unread} byte(s) from it on are not"),
            ] {
                assert!(
                    line.contains(&needle),
                    "{how:?}: the warning does not say `{needle}`\n{outcome}"
                );
            }
            // A length that is wrong and in bounds: the stop is the first
            // byte past the damaged record, on a word no record opens
            // with, and the warning names that record as the last one read.
            if let Some(record) = broken.wrong_length_of {
                for needle in [
                    "is not one a record opens with".to_string(),
                    format!("the last of them at +{record},"),
                ] {
                    assert!(
                        line.contains(&needle),
                        "{how:?}: the warning does not say `{needle}`\n{outcome}"
                    );
                }
            }
        }
    }

    // The account a gate reads, assembled the way the ledger gates assemble
    // it, which takes no typeface: it balances, it agrees with the counts
    // the parser keeps, and the stream is one of its suspect sites.
    let write = write_ledger(&model).unwrap_or_else(|error| panic!("{how:?}: {error}"));
    let account = ExportLedger::assemble(&model, &write);
    assert!(
        account.conservation_breaches().is_empty(),
        "{how:?}: {:?}",
        account.conservation_breaches()
    );
    let reparsed = pid_parse::PidParser::new()
        .parse_file(&broken.path)
        .unwrap_or_else(|error| panic!("{}: {error}", broken.path.display()));
    let geometry = pid_parse::build_normalized_geometry(&reparsed);
    let mismatches = account.cross_check(&model, None, Some(&geometry));
    assert!(
        mismatches.is_empty(),
        "{how:?}: the account disagrees with the counts the code keeps: {mismatches:#?}"
    );
    let summary = account.summary();
    assert_eq!(summary.unattributed, 0, "{how:?}: {summary}");
    let sites: Vec<_> = account
        .drawing
        .suspect_sites
        .iter()
        .filter(|site| site.reason == DropReason::SheetStreamWithoutGeometry)
        .map(|site| (site.stream.as_deref(), site.count))
        .collect();
    // The drawing as committed, for what the break alone changed. Its own
    // suspect count is the fixture ledger gate's to hold, against the
    // symbol sources that gate declares; this layout searches for them.
    let intact_summary = {
        let write = write_ledger(&intact).unwrap_or_else(|error| panic!("intact: {error}"));
        ExportLedger::assemble(&intact, &write).summary()
    };
    match (broken.stop, broken.header_states) {
        (None, None) => {
            assert!(sites.is_empty(), "{how:?}: {sites:?}");
            assert_eq!(summary, intact_summary, "{how:?}");
        }
        _ => {
            assert_eq!(
                sites,
                [(Some(DWG_0201_SHEET), 1)],
                "{how:?}: the stream is a suspect site of the account"
            );
            assert!(
                summary.suspect >= 1,
                "{how:?}: the account counts no suspect drop: {summary}"
            );
        }
    }

    // What the chain read and no list places. The sheet's list is in the
    // document storage, which no break touches, so it places every record
    // the chain read — as long as the chain read one of the sheet's own to
    // hold the list to. A first record eight bytes too long stops the chain
    // behind that record, which hangs under an object the walk never
    // reached: the one record is drawn, two primitives, and has no place.
    // That is the chain's stop and nothing else, so the account files it
    // beside the stream — two suspect items — and the program starts no
    // second line for it: the one warning asserted above is the chain's.
    let no_place: Vec<_> = model
        .ledger
        .unplaced
        .iter()
        .map(|item| {
            (
                item.domain,
                item.stream.as_str(),
                item.why,
                item.records,
                item.primitives,
            )
        })
        .collect();
    let filed_no_place: Vec<_> = account
        .drawing
        .suspect_sites
        .iter()
        .filter(|site| site.reason == DropReason::RecordWithoutDisplayPlace)
        .map(|site| (site.stream.as_deref(), site.count))
        .collect();
    if matches!(how, Break::FirstLengthPlusEight) {
        assert_eq!(
            no_place,
            [(
                LedgerDomain::Drawing,
                DWG_0201_SHEET,
                UnplacedWhy::NoRecordOfTheSheet,
                1,
                2
            )],
            "{how:?}: what the chain read and no list places"
        );
        assert_eq!(
            filed_no_place,
            [(Some(DWG_0201_SHEET), 1)],
            "{how:?}: the account files the record beside the stream"
        );
        assert_eq!(
            summary.suspect,
            intact_summary.suspect + 2,
            "{how:?}: the stream and the record it left without a place: {summary}"
        );
    } else {
        assert!(no_place.is_empty(), "{how:?}: {no_place:?}");
        assert!(filed_no_place.is_empty(), "{how:?}: {filed_no_place:?}");
    }
    assert!(
        model.ledger.display_order_warnings().is_empty(),
        "{how:?}: a line of its own for what the chain's stop accounts for: {:?}",
        model.ledger.display_order_warnings()
    );
    // The chain's one line is the library's sentence, and where the stop
    // left the records read without a place it goes on to say so and to
    // say what the page is: with no record of the sheet's own the sheet's
    // list was not taken, nor the paper stated with it, and this copy read
    // no frame either, so its page is the size its template's name states.
    if broken.stop.is_some() {
        let line = warned[0]
            .strip_prefix(&format!("warning: {}: ", broken.path.display()))
            .unwrap_or_else(|| panic!("{how:?}: the warning does not open with the copy"));
        assert_eq!(
            model.ledger.chain_fault_warnings(),
            [line],
            "{how:?}: the program does not say what the library found"
        );
        let goes_on = [
            "none of the records read is the sheet's own",
            "1 record(s), 2 primitive(s) on the page, take no place in the display order",
            "the page is the size its template's name states, 594.000 x 420.000 mm",
        ];
        if matches!(how, Break::FirstLengthPlusEight) {
            for needle in goes_on {
                assert!(
                    line.contains(needle),
                    "{how:?}: the chain's warning does not say `{needle}`\n{outcome}"
                );
            }
        } else {
            assert!(
                !line.contains("take no place") && !line.contains("the page is"),
                "{how:?}: the chain's warning says more than the chain cost\n{outcome}"
            );
        }
    }

    match fonts {
        Ok(_) => {
            assert_eq!(outcome.code, Some(0), "{how:?}\n{outcome}");
            let pdf = std::fs::read(&out)
                .unwrap_or_else(|error| panic!("{}: {error}\n{outcome}", out.display()));
            assert_eq!(
                outcome.stdout.lines().next(),
                Some(page_line(&model, pdf.len()).as_str()),
                "{how:?}: the command line did not report the page the library lays out\n{outcome}"
            );
            assert_eq!(
                ledger_count(&outcome, "unattributed"),
                0,
                "{how:?}: the page's account does not balance\n{outcome}"
            );
            // The summary the program prints is the account's own.
            assert_eq!(
                ledger_count(&outcome, "suspect"),
                summary.suspect,
                "{how:?}: the program's summary is not the account's\n{outcome}"
            );
        }
        Err(searched) => {
            eprintln!(
                "{case}: no font directory is present (searched {searched}), so the run \
                 can only get as far as the first missing face"
            );
            assert_eq!(outcome.code, Some(1), "{how:?}\n{outcome}");
        }
    }
}

/// The harness itself: the drawing rewritten with nothing changed is the
/// drawing — same page, no warning, nothing filed.
#[test]
fn a_drawing_rewritten_as_it_is_exports_without_a_word() {
    export_a_broken_chain("chain-intact", Break::None);
}

#[test]
fn a_chain_that_stops_in_the_middle_exports_the_records_before_it_and_says_so() {
    export_a_broken_chain("chain-middle-zero", Break::MiddleLengthZero);
}

#[test]
fn a_chain_whose_last_record_runs_past_the_stream_exports_the_rest_and_says_so() {
    export_a_broken_chain("chain-last-plus-one", Break::LastLengthPlusOne);
}

#[test]
fn a_chain_with_stray_bytes_at_its_end_exports_the_records_and_says_so() {
    export_a_broken_chain("chain-last-minus-four", Break::LastLengthMinusFour);
}

/// A header that counts one live record more than the chain holds. The
/// walk reaches the stream's end, every record is read and the page is the
/// drawing's own — and the count is the only thing that says records may
/// be missing, as it is for a stream cut off at a record boundary. Held end
/// to end here: one warning that names no stop, the stream filed in the
/// page's ledger with no byte unread, the same exit code.
#[test]
fn a_header_that_counts_one_record_too_many_exports_the_page_and_says_so() {
    export_a_broken_chain("chain-header-count", Break::HeaderCountPlusOne);
}

/// The first record eight bytes too long: a length that is
/// wrong and still inside the stream. Before the walk held a type word to
/// the native reader's class table this copy read as a sound stream — 359
/// records to the last byte, the live count the header states — and the
/// page was one text box short with nothing said. The walk now stops at
/// the first byte past the record, on a word no record opens with; the
/// page is the first record's and the template's, and the program says so.
#[test]
fn a_first_record_too_long_stops_the_chain_past_it_and_says_so() {
    export_a_broken_chain("chain-first-plus-eight", Break::FirstLengthPlusEight);
}

/// The thirty-first record sixteen bytes too short. The walk used to read
/// on from inside it through whatever lay there; it stops where the wrong
/// length ends, the page is the thirty-one records before that, and the
/// warning names the record whose length is in doubt.
#[test]
fn a_record_too_short_stops_the_chain_inside_it_and_says_so() {
    export_a_broken_chain(
        "chain-thirty-first-minus-sixteen",
        Break::ThirtyFirstLengthMinusSixteen,
    );
}

/// A copy of `D06.pid` whose `JSheet` record lists, first, an object the
/// sheet does not hold: one oid of the cluster changed and nothing else.
///
/// A list is taken for the sheet's only when every object it names is a
/// record of the sheet's own (`pid_parse::parsers::sheet_display_order`),
/// so this drawing states no display order that can be read. Its `/Sheet6`
/// is the committed one, byte for byte.
fn d06_with_a_list_that_is_not_its_sheets(case: &str) -> PathBuf {
    use pid_parse::parsers::sheet_display_order::find_sheet_child_table;
    use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};

    const CLUSTER: &str = "/PSMcluster0";
    let source = d06();
    let package = pid_parse::PidParser::new()
        .parse_package(&source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    assert!(
        package.parsed.sheet_display_order.contains_key("/Sheet6"),
        "{}: the committed drawing states no display order",
        source.display()
    );
    let mut cluster = package
        .get_stream(CLUSTER)
        .unwrap_or_else(|| panic!("{} has no {CLUSTER}", source.display()))
        .data
        .clone();
    let table = find_sheet_child_table(&cluster, 6, |_| true)
        .unwrap_or_else(|| panic!("{}: no JSheet record for sheet 6", source.display()));
    // The first entry as the record spells it: oid, range box, flags.
    let first = table.children[0];
    let mut entry = first.oid.to_le_bytes().to_vec();
    for word in first.range {
        entry.extend_from_slice(&word.to_le_bytes());
    }
    entry.extend_from_slice(&first.flags.to_le_bytes());
    let found: Vec<usize> = cluster
        .windows(entry.len())
        .enumerate()
        .filter(|(_, window)| *window == entry.as_slice())
        .map(|(at, _)| at)
        .collect();
    assert_eq!(found.len(), 1, "the list's first entry, in the cluster");
    // An oid no record of the sheet carries.
    cluster[found[0]..found[0] + 4].copy_from_slice(&0x7fff_fff0u32.to_le_bytes());
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: CLUSTER.to_string(),
            new_data: cluster,
        }],
        ..WritePlan::default()
    };
    let path = scratch(case).join("D06.pid");
    PidWriter::write_to(&package, &plan, &path)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    path
}

/// A sheet whose list of objects cannot be read still exports: every record
/// is drawn, and the page lacks nothing but the drawing's word on what
/// covers what. That is not a finished export either, and nothing on the
/// page says so — so the program says it, in one warning that names the
/// stream, how many records and primitives no list places and why, and the
/// page's ledger files the stream as a suspect item, where the gates that
/// read ledgers see it. The drawing as committed, against the same corpus,
/// draws no such warning and files nothing.
///
/// The record that lists the objects states the paper as well, so a sheet
/// whose list is not read has no paper either and its page is the border
/// frame's rectangle. The same warning says so, with the rectangle: a
/// reader told only about the order would take the page for the paper.
#[test]
fn a_sheet_whose_list_is_not_its_own_exports_and_says_so() {
    let case = "order-unread";
    let corpus = fixtures();
    let committed = d06();
    let drawing = d06_with_a_list_that_is_not_its_sheets(case);
    let sound = lay_out_drawing(&committed, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", committed.display()));
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));

    // The committed drawing: every primitive stands where a sheet lists its
    // record, and the account files nothing about the order.
    assert!(
        sound.ledger.unplaced.is_empty(),
        "{:?}",
        sound.ledger.unplaced
    );
    assert!(sound.ledger.display_order_warnings().is_empty());

    // The copy holds the same page: the same primitives, in the model's
    // own order, the template among them.
    let primitives = |model: &PaperModel| -> Vec<String> {
        model
            .primitives
            .iter()
            .map(|primitive| format!("{:?} {:?}", identity(primitive), primitive.shape))
            .collect()
    };
    assert_eq!(
        primitives(&model),
        primitives(&sound),
        "the page lost or gained ink"
    );
    assert!(model.template.is_some(), "the copy wears no template");
    // The record that was not read states the paper too, so the page is
    // not the paper: it is the border frame's rectangle, where the
    // committed drawing's is the 594 x 420 mm its sheet states.
    let frame = sound.frame_mm.expect("D06 has a border frame");
    assert_eq!(sound.page_mm, Some([594.0, 420.0]));
    assert_eq!(model.frame_mm, Some(frame));
    assert_eq!(
        model.page_mm,
        Some(frame),
        "the page of a sheet whose paper was not read"
    );

    // What no list places: every record of `/Sheet6` that draws.
    let own: Vec<&PaperPrimitive> = model
        .primitives
        .iter()
        .filter(|primitive| !primitive.furniture)
        .collect();
    assert!(
        own.iter()
            .all(|primitive| primitive.stream_path.as_deref() == Some("/Sheet6"))
    );
    let mut records: Vec<usize> = own
        .iter()
        .map(|primitive| primitive.byte_range.expect("a record")[0])
        .collect();
    records.sort_unstable();
    records.dedup();
    assert_eq!(
        model.ledger.unplaced.len(),
        1,
        "{:?}",
        model.ledger.unplaced
    );
    let unplaced = &model.ledger.unplaced[0];
    assert_eq!(
        (unplaced.domain, unplaced.stream.as_str(), unplaced.why),
        (LedgerDomain::Drawing, "/Sheet6", UnplacedWhy::NoList)
    );
    assert_eq!(unplaced.records, records.len() as u64);
    assert_eq!(unplaced.primitives, own.len() as u64);
    // The template states its own order, so the page is painted in one:
    // the template first, then the drawing's own as the model holds them.
    let painted = model
        .paint_sequence()
        .expect("the template states an order");
    let (template, drawn): (Vec<usize>, Vec<usize>) = painted
        .iter()
        .partition(|index| model.primitives[**index].furniture);
    assert_eq!(
        painted[..template.len()],
        template[..],
        "the template is not first"
    );
    assert!(drawn.windows(2).all(|pair| pair[0] < pair[1]));
    // The account: one suspect item for the stream, counted in records.
    let items: Vec<(DropReason, &str, u64)> = model
        .ledger
        .parse
        .drawing
        .items
        .iter()
        .filter(|item| item.reason == DropReason::RecordWithoutDisplayPlace)
        .map(|item| (item.reason, item.stream.as_str(), item.count))
        .collect();
    assert_eq!(
        items,
        [(
            DropReason::RecordWithoutDisplayPlace,
            "/Sheet6",
            unplaced.records
        )]
    );
    assert_eq!(
        DropReason::RecordWithoutDisplayPlace.tier(),
        pid_plot::ledger::Tier::Suspect
    );

    // The program, on both drawings.
    let fonts = fonts_for_a_child();
    let export = |drawing: &Path, name: &str| {
        let out = unwritten(&scratch(case), name);
        let mut command = pid_plot();
        if let Ok(fonts) = &fonts {
            command.env("PID_PLOT_FONTS", fonts);
        }
        if let Some(library) = std::env::var_os("PID_SYMBOL_LIBRARY") {
            command.env("PID_SYMBOL_LIBRARY", library);
        }
        let outcome = run(command.arg(drawing).arg(&out).arg(&corpus));
        match &fonts {
            Ok(_) => {
                assert_eq!(outcome.code, Some(0), "{outcome}");
                assert!(out.exists(), "{} was not written\n{outcome}", out.display());
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
    };
    let said = export(&drawing, "unlisted.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        1,
        "a sheet whose list cannot be read is exactly one warning\n{said}"
    );
    let line = warned[0];
    for needle in [
        drawing.display().to_string(),
        format!("{} record(s) of /Sheet6", unplaced.records),
        format!("{} primitive(s) on the page", unplaced.primitives),
        "take no place in the display order".to_string(),
        "states no list of its objects that could be read".to_string(),
        "painted after everything that has a place".to_string(),
        // Two things were not read, and the one line says both.
        "the paper is stated in the same record and was not read either".to_string(),
        format!(
            "the page is the border frame's rectangle, {:.3} x {:.3} mm",
            frame[0], frame[1]
        ),
    ] {
        assert!(
            line.contains(&needle),
            "the warning does not say {needle:?}\n{said}"
        );
    }
    assert_eq!(
        model.ledger.display_order_warnings(),
        [line
            .strip_prefix(&format!("warning: {}: ", drawing.display()))
            .expect("the warning opens with the drawing")],
        "the program does not say what the library found"
    );
    let silent = export(&committed, "committed.pdf");
    assert!(
        warnings(&silent.stderr).is_empty(),
        "the committed drawing drew a warning\n{silent}"
    );
}

/// What to change in the `JSheet` record of a drawing's sheet 6.
#[derive(Clone, Copy, Debug)]
enum SheetRecord {
    /// The paper the record states, as `[width, height]` in metres. The
    /// record is otherwise the drawing's.
    Paper([f64; 2]),
    /// The record's count of children, raised by four: four entries are
    /// then read out of the tail and the paper from 56 bytes further on.
    FourChildrenMore,
    /// The entry of the child with this oid, given an oid no record of the
    /// sheet carries: the list then names an object that is not the
    /// sheet's and leaves out one that is.
    NotListed(u32),
}

/// A copy of `D06.pid` with one thing changed in its `JSheet` record and
/// nothing else: every stream but `/PSMcluster0` is the committed one, and
/// that one differs in the bytes [`SheetRecord`] names.
///
/// The record's tail is nine bytes, the names `Sheet1` and `Section1`, and
/// a fixed part that states the paper: the height 102 bytes into the tail
/// and the width 114 (`pid_parse::parsers::sheet_display_order`). The copy
/// is checked against that: the two doubles read back as the 0.42 m by
/// 0.594 m the sheet states before they are overwritten.
fn d06_with_another_sheet_record(case: &str, change: SheetRecord) -> PathBuf {
    let path = scratch(case).join("D06.pid");
    with_another_sheet_record(&d06(), change, &path);
    path
}

/// The same for any drawing whose sheet is oid 6 and states an A2 paper:
/// `source` written to `path` with one thing changed in the `JSheet` record
/// of its `/PSMcluster0`, every other stream as `source` holds it.
fn with_another_sheet_record(source: &Path, change: SheetRecord, path: &Path) {
    use pid_parse::parsers::sheet_display_order::find_sheet_child_table;
    use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};

    const CLUSTER: &str = "/PSMcluster0";
    let package = pid_parse::PidParser::new()
        .parse_package(source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    let mut cluster = package
        .get_stream(CLUSTER)
        .unwrap_or_else(|| panic!("{} has no {CLUSTER}", source.display()))
        .data
        .clone();
    let table = find_sheet_child_table(&cluster, 6, |_| true)
        .unwrap_or_else(|| panic!("{}: no JSheet record for sheet 6", source.display()));
    let first = table.children[0];
    let mut entry = first.oid.to_le_bytes().to_vec();
    for word in first.range {
        entry.extend_from_slice(&word.to_le_bytes());
    }
    entry.extend_from_slice(&first.flags.to_le_bytes());
    let found: Vec<usize> = cluster
        .windows(entry.len())
        .enumerate()
        .filter(|(_, window)| *window == entry.as_slice())
        .map(|(at, _)| at)
        .collect();
    assert_eq!(found.len(), 1, "the list's first entry, in the cluster");
    let entries = found[0];
    let tail = entries + 14 * table.children.len();
    let double = |cluster: &[u8], at: usize| {
        f64::from_le_bytes(cluster[at..at + 8].try_into().expect("eight bytes"))
    };
    assert_eq!(
        [double(&cluster, tail + 114), double(&cluster, tail + 102)],
        [0.594, 0.42],
        "the paper {} states, where the record keeps it",
        source.display()
    );
    match change {
        SheetRecord::Paper([width, height]) => {
            cluster[tail + 102..tail + 110].copy_from_slice(&height.to_le_bytes());
            cluster[tail + 114..tail + 122].copy_from_slice(&width.to_le_bytes());
        }
        SheetRecord::FourChildrenMore => {
            let count = u32::try_from(table.children.len()).expect("a count") + 4;
            cluster[entries - 4..entries].copy_from_slice(&count.to_le_bytes());
        }
        SheetRecord::NotListed(oid) => {
            let index = table
                .children
                .iter()
                .position(|child| child.oid == oid)
                .unwrap_or_else(|| panic!("{}: the list names no oid {oid}", source.display()));
            let at = entries + 14 * index;
            assert_eq!(
                cluster[at..at + 4],
                oid.to_le_bytes(),
                "the entry of oid {oid}, where the list keeps it"
            );
            cluster[at..at + 4].copy_from_slice(&0x7fff_fff0u32.to_le_bytes());
        }
    }
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: CLUSTER.to_string(),
            new_data: cluster,
        }],
        ..WritePlan::default()
    };
    PidWriter::write_to(&package, &plan, path)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// Start `pid_plot` on `drawing` against the fixtures, as far as this
/// machine lets an export go: exit 0 and a page with fonts to hand, exit 1
/// at the first missing face without. Either way every warning has been
/// printed by then.
fn export_against_the_fixtures(case: &str, drawing: &Path, name: &str) -> Outcome {
    let out = unwritten(&scratch(case), name);
    let fonts = fonts_for_a_child();
    let mut command = pid_plot();
    if let Ok(fonts) = &fonts {
        command.env("PID_PLOT_FONTS", fonts);
    }
    if let Some(library) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        command.env("PID_SYMBOL_LIBRARY", library);
    }
    let outcome = run(command.arg(drawing).arg(&out).arg(fixtures()));
    match fonts {
        Ok(_) => {
            assert_eq!(outcome.code, Some(0), "{outcome}");
            assert!(out.exists(), "{} was not written\n{outcome}", out.display());
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

/// A paper that is not the border frame's is not taken for the page.
///
/// The paper a sheet states is the page a drawing is written on, and
/// nothing in the file holds it to anything: two doubles in the tail of one
/// record. Read wrong — or right, from a file that is wrong there — they
/// would cut the page to whatever they said, and everything of the drawing
/// that fell off it would be filed as ink the paper never held. So the
/// paper is held to the frame: farther from it than the paper of any
/// drawing on hand, it is refused, the page is the frame's rectangle as it
/// was before papers were read, and the drawing is whole. The program says
/// so in one warning that names the drawing, the paper and the frame; the
/// account files one suspect item for the page and nothing as left off the
/// paper; the exit code is the committed drawing's.
fn export_with_a_paper_that_is_not_the_frames(case: &str, paper_m: [f64; 2], paper_mm: &str) {
    let corpus = fixtures();
    let committed = d06();
    let drawing = d06_with_another_sheet_record(case, SheetRecord::Paper(paper_m));
    let sound = lay_out_drawing(&committed, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", committed.display()));
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));

    // The page is the frame's rectangle, which is the committed drawing's
    // frame; the committed drawing's page is its paper.
    let frame = sound.frame_mm.expect("D06 has a border frame");
    assert_eq!(sound.page_mm, Some([594.0, 420.0]));
    assert_eq!(model.frame_mm, Some(frame), "{case}: the frame");
    assert_eq!(
        model.page_mm,
        Some(frame),
        "{case}: the page is not the frame's rectangle"
    );
    // The drawing is whole: the same primitives as the committed one.
    let primitives = |model: &PaperModel| -> Vec<String> {
        model
            .primitives
            .iter()
            .map(|primitive| format!("{:?} {:?}", identity(primitive), primitive.shape))
            .collect()
    };
    assert_eq!(
        primitives(&model),
        primitives(&sound),
        "{case}: the page lost or gained ink"
    );

    // The account: one suspect item for the page, nothing left off a paper,
    // and no more filed as intended than on the committed page.
    let account = |model: &PaperModel| {
        let write = write_ledger(model).unwrap_or_else(|error| panic!("{case}: {error}"));
        ExportLedger::assemble(model, &write)
    };
    let (ledger, sound_ledger) = (account(&model), account(&sound));
    assert!(ledger.conservation_breaches().is_empty(), "{case}");
    let page: Vec<(&str, u64)> = ledger
        .page
        .dropped
        .iter()
        .map(|(reason, count)| (reason.code(), *count))
        .collect();
    assert_eq!(
        page,
        [("paper_unlike_the_frame", 1)],
        "{case}: the page's own items"
    );
    assert_eq!(ledger.summary().suspect, 1, "{case}: {}", ledger.summary());
    for domain in [&ledger.drawing, &ledger.furniture] {
        let off_the_paper: Vec<&str> = domain
            .segments
            .dropped
            .keys()
            .chain(domain.primitives.dropped.keys())
            .map(|reason| reason.code())
            .filter(|code| code.contains("beyond_the_paper"))
            .collect();
        assert!(
            off_the_paper.is_empty(),
            "{case}: ink filed as off a paper that was refused: {off_the_paper:?}"
        );
    }
    assert!(
        ledger.summary().intentional <= sound_ledger.summary().intentional,
        "{case}: {} against the committed page's {}",
        ledger.summary(),
        sound_ledger.summary()
    );

    // The program: one warning, with the drawing, the paper and the frame.
    let said = export_against_the_fixtures(case, &drawing, "refused.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        1,
        "{case}: a paper that is not the frame's is exactly one warning\n{said}"
    );
    let line = warned[0];
    for needle in [
        drawing.display().to_string(),
        format!("the paper its sheet states, {paper_mm} mm"),
        format!("{:.3} x {:.3} mm", frame[0], frame[1]),
        "the page is the frame's rectangle".to_string(),
    ] {
        assert!(
            line.contains(&needle),
            "{case}: the warning does not say {needle:?}\n{said}"
        );
    }
}

#[test]
fn a_paper_half_the_frame_wide_is_refused_and_the_drawing_is_whole() {
    export_with_a_paper_that_is_not_the_frames(
        "paper-half-wide",
        [0.297, 0.42],
        "297.000 x 420.000",
    );
}

#[test]
fn a_paper_with_no_height_is_refused_and_the_drawing_is_whole() {
    export_with_a_paper_that_is_not_the_frames(
        "paper-no-height",
        [1.0, 1e-300],
        "1000.000 x 0.000",
    );
}

#[test]
fn a_paper_a_metre_too_large_is_refused_and_the_drawing_is_whole() {
    export_with_a_paper_that_is_not_the_frames(
        "paper-a-metre-more",
        [1.594, 1.42],
        "1594.000 x 1420.000",
    );
}

/// Wrong in its height alone: the width is the frame's to a third of a
/// millimetre, and the height is 29 mm off. Each side is held on its own.
#[test]
fn a_paper_too_tall_for_the_frame_is_refused_and_the_drawing_is_whole() {
    export_with_a_paper_that_is_not_the_frames(
        "paper-too-tall",
        [0.594, 0.45],
        "594.000 x 450.000",
    );
}

#[test]
fn a_paper_turned_on_its_side_is_refused_and_the_drawing_is_whole() {
    export_with_a_paper_that_is_not_the_frames("paper-upright", [0.42, 0.594], "420.000 x 594.000");
}

/// A record that counts four children too many still lists the sheet's 26
/// objects first and in order, so the order is read as on the committed
/// drawing. The paper is not: it stands behind the entries, the count says
/// the entries end 56 bytes later than they do, and what is read there is
/// a paper of 0 x 1000 mm. Nothing in the record says so. The frame does:
/// the paper is refused like any other that is not the frame's, the page is
/// the frame's rectangle and the drawing is whole.
#[test]
fn a_list_four_entries_too_long_misplaces_the_paper_and_the_frame_refuses_it() {
    let case = "list-four-more";
    let corpus = fixtures();
    let committed = d06();
    let drawing = d06_with_another_sheet_record(case, SheetRecord::FourChildrenMore);
    let sound = lay_out_drawing(&committed, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", committed.display()));
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let frame = sound.frame_mm.expect("D06 has a border frame");
    assert_eq!(
        model.page_mm,
        Some(frame),
        "the page is not the frame's rectangle"
    );
    assert_eq!(model.primitives.len(), sound.primitives.len());
    // The order is the committed drawing's: the list's own 26 entries are
    // where they were, and no record is left without a place.
    assert!(
        model.ledger.unplaced.is_empty(),
        "{:?}",
        model.ledger.unplaced
    );
    assert!(model.paint_sequence().is_some());
    assert_eq!(model.paint_sequence(), sound.paint_sequence());
    let write = write_ledger(&model).unwrap_or_else(|error| panic!("{case}: {error}"));
    let ledger = ExportLedger::assemble(&model, &write);
    let page: Vec<(&str, u64)> = ledger
        .page
        .dropped
        .iter()
        .map(|(reason, count)| (reason.code(), *count))
        .collect();
    assert_eq!(page, [("paper_unlike_the_frame", 1)]);
    assert_eq!(ledger.summary().suspect, 1, "{}", ledger.summary());

    let said = export_against_the_fixtures(case, &drawing, "four-more.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(warned.len(), 1, "one warning\n{said}");
    for needle in [
        drawing.display().to_string(),
        "the paper its sheet states, 0.000 x 1000.000 mm".to_string(),
        format!("{:.3} x {:.3} mm", frame[0], frame[1]),
        "the page is the frame's rectangle".to_string(),
    ] {
        assert!(
            warned[0].contains(&needle),
            "the warning does not say {needle:?}\n{said}"
        );
    }
}

/// What takes no place in the display order of a page, as the layout found
/// it: the domain, the stream, why, the records and the primitives.
fn no_place(model: &PaperModel) -> Vec<(LedgerDomain, &str, UnplacedWhy, u64, u64)> {
    model
        .ledger
        .unplaced
        .iter()
        .map(|item| {
            (
                item.domain,
                item.stream.as_str(),
                item.why,
                item.records,
                item.primitives,
            )
        })
        .collect()
}

/// A sound stream that holds no record of its sheet's own says so itself,
/// and says what is so.
///
/// `DWG-0201GP06-01.pid` cut cleanly behind its first record — the header's
/// count put right, so the chain holds — keeps one record, a symbol that
/// hangs under an object of the sheet's the stream no longer holds. The
/// sheet's list is in the document storage, whole, with its 160 children,
/// and there is no record of the sheet's own to hold it to: no order is
/// read, and the record is drawn with no place in one. No chain fault says
/// anything of this drawing, so the line about the display order is the
/// only word there is, and it is printed. It does not say the sheet states
/// no list, which it does; nor that the page is a border frame's rectangle,
/// when no frame was read and the page is the size the template's name
/// stands for.
///
/// The same stream behind a chain that stops there — the copy this one is
/// the sound prefix of — is `a_first_record_too_long_stops_the_chain_past_
/// it_and_says_so`: there the chain's warning is the word on it, and this
/// line is not started.
#[test]
fn a_sound_stream_with_no_record_of_the_sheets_own_says_what_takes_no_place() {
    use pid_parse::parsers::sheet_display_order::find_sheet_child_table;

    let case = "order-no-own-record";
    let corpus = fixtures();
    let drawing = broken_copy(case, Break::FirstLengthPlusEight).sound_prefix;
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));

    // The chain holds, and the list is there.
    assert!(
        model.ledger.chain_faults.is_empty(),
        "{:?}",
        model.ledger.chain_faults
    );
    let package = pid_parse::PidParser::new()
        .parse_package(&drawing)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let cluster = &package
        .get_stream("/PSMcluster0")
        .expect("the document's cluster")
        .data;
    let listed = find_sheet_child_table(cluster, 6, |_| true).map(|table| table.children.len());
    assert_eq!(listed, Some(160), "the sheet's list, in the cluster");

    // One record, two primitives, and the reason is the stream's.
    assert_eq!(
        no_place(&model),
        [(
            LedgerDomain::Drawing,
            DWG_0201_SHEET,
            UnplacedWhy::NoRecordOfTheSheet,
            1,
            2
        )]
    );
    let write = write_ledger(&model).unwrap_or_else(|error| panic!("{case}: {error}"));
    let account = ExportLedger::assemble(&model, &write);
    let sites: Vec<_> = account
        .drawing
        .suspect_sites
        .iter()
        .map(|site| (site.reason, site.stream.as_deref(), site.count))
        .collect();
    assert_eq!(
        sites,
        [(
            DropReason::RecordWithoutDisplayPlace,
            Some(DWG_0201_SHEET),
            1
        )]
    );
    assert_eq!(account.summary().suspect, 1, "{}", account.summary());

    let said = export_against_the_fixtures(case, &drawing, "no-own-record.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        1,
        "a stream with no record of the sheet's own is exactly one warning\n{said}"
    );
    let line = warned[0];
    for needle in [
        drawing.display().to_string(),
        "1 record(s) of /Sheet6, 2 primitive(s) on the page, take no place in the display order"
            .to_string(),
        "the stream holds no record of the sheet's own, so the list its sheet states has \
         nothing to be held to"
            .to_string(),
        "the paper is stated in the same record and was not read either".to_string(),
        "so the page is the size its template's name states, 594.000 x 420.000 mm".to_string(),
    ] {
        assert!(
            line.contains(&needle),
            "the warning does not say {needle:?}\n{said}"
        );
    }
    for untrue in ["states no list", "border frame", "does not hold"] {
        assert!(
            !line.contains(untrue),
            "the warning says {untrue:?} of a drawing whose list is whole, of which no frame \
             was read and whose chain holds\n{said}"
        );
    }
    assert_eq!(
        model.ledger.display_order_warnings(),
        [line
            .strip_prefix(&format!("warning: {}: ", drawing.display()))
            .expect("the warning opens with the drawing")],
        "the program does not say what the library found"
    );
}

/// A chain that stops, in a drawing whose list is not its sheet's: two
/// things wrong, and two warnings.
///
/// The chain of `/Sheet6` stops in the middle, having read 179 records, the
/// sheet's own among them, and the list in the document storage names, for
/// the first of those, an oid no record carries. The list is then not the
/// sheet's — it names an object that is not a record of the sheet's and
/// leaves out one the chain read — and it would not be had the chain read
/// to the end. So the records the chain read take no place for a reason
/// that is not the stop, and the line that says so stands beside the
/// chain's. Only records a stop alone leaves without a place go unsaid.
#[test]
fn a_stopped_chain_beside_a_list_that_is_not_the_sheets_is_two_warnings() {
    let case = "chain-and-list";
    let corpus = fixtures();
    let broken = broken_copy(case, Break::MiddleLengthZero);
    let (stop, unread) = broken.stop.expect("the chain stops");

    // The first live record of the sheet's own the chain reads: its oid.
    let package = pid_parse::PidParser::new()
        .parse_package(&broken.path)
        .unwrap_or_else(|error| panic!("{}: {error}", broken.path.display()));
    let sheet = &package.get_stream(DWG_0201_SHEET).expect("the sheet").data;
    let word = |at: usize| u32::from_le_bytes(sheet[at..at + 4].try_into().expect("four bytes"));
    let mut at = 8usize;
    let mut own = None;
    while at + 14 <= stop {
        let kind = u16::from_le_bytes(sheet[at..at + 2].try_into().expect("two bytes"));
        if kind & 0x8000 == 0 && word(at + 10) == 6 {
            own = Some(word(at + 6));
            break;
        }
        at += 6 + word(at + 2) as usize;
    }
    let own = own.expect("a record of the sheet's own before the stop");
    let drawing = scratch(case).join("DWG-0201GP06-01-not-listed.pid");
    with_another_sheet_record(&broken.path, SheetRecord::NotListed(own), &drawing);

    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    // Both findings: the chain, and every record that draws without a
    // place — for want of a list, not for want of a record to hold one to.
    assert_eq!(model.ledger.chain_faults.len(), 1);
    let mut records: Vec<usize> = model
        .primitives
        .iter()
        .filter(|primitive| !primitive.furniture)
        .map(|primitive| primitive.byte_range.expect("a record")[0])
        .collect();
    let primitives = records.len() as u64;
    records.sort_unstable();
    records.dedup();
    assert!(!records.is_empty());
    assert_eq!(
        no_place(&model),
        [(
            LedgerDomain::Drawing,
            DWG_0201_SHEET,
            UnplacedWhy::NoList,
            records.len() as u64,
            primitives
        )]
    );
    let write = write_ledger(&model).unwrap_or_else(|error| panic!("{case}: {error}"));
    let account = ExportLedger::assemble(&model, &write);
    let mut sites: Vec<_> = account
        .drawing
        .suspect_sites
        .iter()
        .map(|site| (site.reason, site.stream.as_deref(), site.count))
        .collect();
    sites.sort();
    assert_eq!(
        sites,
        [
            (
                DropReason::SheetStreamWithoutGeometry,
                Some(DWG_0201_SHEET),
                1
            ),
            (
                DropReason::RecordWithoutDisplayPlace,
                Some(DWG_0201_SHEET),
                records.len() as u64
            ),
        ]
    );

    let said = export_against_the_fixtures(case, &drawing, "chain-and-list.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        2,
        "a stopped chain and a list that is not the sheet's are two warnings\n{said}"
    );
    let chain = warned
        .iter()
        .find(|line| line.contains("the record chain of /Sheet6 does not hold"))
        .unwrap_or_else(|| panic!("no warning of the chain\n{said}"));
    for needle in [
        drawing.display().to_string(),
        format!("stops at +{stop} "),
        format!("{unread} byte(s) from it on are not"),
    ] {
        assert!(
            chain.contains(&needle),
            "the chain's warning does not say {needle:?}\n{said}"
        );
    }
    let order = warned
        .iter()
        .find(|line| line.contains("take no place in the display order"))
        .unwrap_or_else(|| panic!("no warning of the display order\n{said}"));
    for needle in [
        drawing.display().to_string(),
        format!(
            "{} record(s) of /Sheet6, {primitives} primitive(s) on the page",
            records.len()
        ),
        "the sheet states no list of its objects that could be read".to_string(),
        // The paper is in the list's record, and the chain stopped ahead
        // of the one live frame: the page is the template name's size.
        "so the page is the size its template's name states, 594.000 x 420.000 mm".to_string(),
    ] {
        assert!(
            order.contains(&needle),
            "the display order's warning does not say {needle:?}\n{said}"
        );
    }
    assert_eq!(
        model.ledger.display_order_warnings(),
        [order
            .strip_prefix(&format!("warning: {}: ", drawing.display()))
            .expect("the warning opens with the drawing")],
        "the program does not say what the library found"
    );
}

/// A paper beside a template's name alone is the page, and one that is not
/// the size the name states says so.
///
/// `DWG-0201GP06-01.pid` less its last record — its one live border frame —
/// is a sound drawing no frame of which is read: all it has for its sheet
/// beside the paper is the 594 by 420 mm its template's name stands for.
/// With the paper's width halved in the list's record the two are 297 mm
/// apart. A paper is refused for a frame's rectangle, which is a reading of
/// the same sheet; a name is a label, of the template the drawing was
/// started from, and overrules no reading. So the page is the paper the
/// sheet states, and that the two differ is said: one suspect item of its
/// own code — not the frame's — and one warning that names both sizes, says
/// no frame was read and says which the page is. No drawing on hand states
/// a paper beside a name alone, so nothing witnesses this path.
#[test]
fn a_paper_unlike_its_templates_name_is_the_page_and_says_so() {
    let case = "paper-no-frame";
    let corpus = fixtures();
    let frameless = broken_copy(case, Break::LastLengthPlusOne).sound_prefix;
    let doc = pid_parse::PidParser::new()
        .parse_file(&frameless)
        .unwrap_or_else(|error| panic!("{}: {error}", frameless.display()));
    assert_eq!(
        pid_parse::framed_page_dimensions_mm(&doc),
        None,
        "the drawing less its last record keeps a frame that gives a page"
    );
    assert_eq!(
        pid_parse::build_normalized_geometry(&doc).page_dimensions_mm,
        Some((594.0, 420.0)),
        "the size the template's name states"
    );
    // As it is, the paper is the name's size and is the page; nothing is
    // said of the page.
    let sound = lay_out_drawing(&frameless, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", frameless.display()));
    assert_eq!(sound.page_mm, Some([594.0, 420.0]));
    assert!(sound.ledger.page_warnings().is_empty());

    let drawing = scratch(case).join("DWG-0201GP06-01-halved.pid");
    with_another_sheet_record(&frameless, SheetRecord::Paper([0.297, 0.42]), &drawing);
    let model = lay_out_drawing(&drawing, &corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    assert_eq!(
        model.page_mm,
        Some([297.0, 420.0]),
        "the page is not the paper the sheet states"
    );
    // The model holds what the sheet at its largest holds; the paper cuts
    // nothing out of it.
    let primitives = |model: &PaperModel| -> Vec<String> {
        model
            .primitives
            .iter()
            .map(|primitive| format!("{:?} {:?}", identity(primitive), primitive.shape))
            .collect()
    };
    assert_eq!(
        primitives(&model),
        primitives(&sound),
        "the model lost or gained primitives"
    );

    let write = write_ledger(&model).unwrap_or_else(|error| panic!("{case}: {error}"));
    let account = ExportLedger::assemble(&model, &write);
    let page: Vec<(&str, Option<&str>, u64)> = account
        .page
        .suspect_sites
        .iter()
        .map(|site| (site.reason.code(), site.name.as_deref(), site.count))
        .collect();
    assert_eq!(
        page,
        [(
            "paper_unlike_the_template_name",
            Some("paper 297.000 x 420.000 mm, template name's size 594.000 x 420.000 mm"),
            1
        )],
        "the page's own suspect items: the name's code, not the frame's"
    );
    let dropped: Vec<(&str, u64)> = account
        .page
        .dropped
        .iter()
        .map(|(reason, count)| (reason.code(), *count))
        .collect();
    assert_eq!(dropped, [("paper_unlike_the_template_name", 1)]);

    let said = export_against_the_fixtures(case, &drawing, "halved.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        1,
        "a paper that is not the name's size is exactly one warning\n{said}"
    );
    let line = warned[0];
    for needle in [
        drawing.display().to_string(),
        "the paper its sheet states, 297.000 x 420.000 mm".to_string(),
        "is not the size its template's name states, 594.000 x 420.000 mm".to_string(),
        "no border frame was read".to_string(),
        "the page is the paper".to_string(),
    ] {
        assert!(
            line.contains(&needle),
            "the warning does not say {needle:?}\n{said}"
        );
    }
    for untrue in [
        "border frame's,",
        "frame's rectangle",
        "not taken",
        "no drawing on hand",
    ] {
        assert!(
            !line.contains(untrue),
            "the warning says {untrue:?} of a paper that is the page, in a drawing no frame \
             of which was read\n{said}"
        );
    }
    assert_eq!(
        model.ledger.page_warnings(),
        [line
            .strip_prefix(&format!("warning: {}: ", drawing.display()))
            .expect("the warning opens with the drawing")],
        "the program does not say what the library found"
    );
}

/// `test-file`'s `一级闪蒸分离器B`, whose pressure gauge these cases draw
/// from a symbol library.
fn flash_b() -> PathBuf {
    fixtures().join("集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid")
}

/// Where a symbol root keeps the pressure gauge that drawing places once.
const GAUGE: [&str; 5] = [
    "Instrumentation",
    "Off-Line",
    "Without Implied Components",
    "Pressure",
    "PG-Pressure Gauge.sym",
];

/// The committed gauge, `test-file/symbols/…/PG-Pressure Gauge.sym`.
fn the_gauge() -> Vec<u8> {
    let path = GAUGE
        .iter()
        .fold(fixtures().join("symbols"), |path, part| path.join(part));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A copy of the drawing or border template `source`, written to `out`, in
/// which no placement of the symbol file named `symbol` has a cached body;
/// and the path each of those placements names, as the file stores it.
///
/// A document carries the body of every symbol it places, and a placement
/// is drawn from that copy; the `.sym` it names is read only when the
/// document holds none. Every page of the corpora is drawn from cached
/// bodies alone. To reach the library, the copy's placements of the symbol
/// name a body the document does not hold: an `igSymbol2d` record ends on
/// the two words that name its cached body, the definition sheet's oid and
/// the site's, and the first of them is given an oid no definition carries.
/// Every other byte is the source's, the caches included, so every other
/// placement is drawn as before.
fn without_the_cached_bodies_of(source: &Path, symbol: &str, out: &Path) -> Vec<String> {
    use pid_parse::geometry::PidGraphicKind;
    use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};

    let package = pid_parse::PidParser::new()
        .parse_package(source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    let doc = pid_parse::PidParser::new()
        .parse_file(source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let wanted = symbol.to_ascii_lowercase();
    // Each placement of the symbol: the path it names, its stream, where
    // its record ends, and the definition sheet it names.
    let placements: Vec<(String, String, usize, u32)> = geometry
        .entities
        .iter()
        .filter_map(|entity| match &entity.kind {
            PidGraphicKind::SymbolInstance {
                symbol_path: Some(path),
                definition: Some(definition),
                ..
            } if path.to_ascii_lowercase().ends_with(&wanted) => Some((
                path.clone(),
                entity.source.stream_path.clone()?,
                entity.source.byte_range.as_ref()?.end,
                definition.sheet,
            )),
            _ => None,
        })
        .collect();
    let mut streams: std::collections::BTreeMap<String, Vec<u8>> =
        std::collections::BTreeMap::new();
    for (_, stream, end, definition_sheet) in &placements {
        let sheet = streams.entry(stream.clone()).or_insert_with(|| {
            package
                .get_stream(stream)
                .unwrap_or_else(|| panic!("{} has no {stream}", source.display()))
                .data
                .clone()
        });
        let at = end - 8;
        assert_eq!(
            sheet[at..at + 4],
            definition_sheet.to_le_bytes(),
            "the body the placement names, where its record keeps it"
        );
        sheet[at..at + 4].copy_from_slice(&0x7fff_fff0u32.to_le_bytes());
    }
    let plan = WritePlan {
        stream_replacements: streams
            .into_iter()
            .map(|(path, new_data)| StreamReplacement { path, new_data })
            .collect(),
        ..WritePlan::default()
    };
    PidWriter::write_to(&package, &plan, out)
        .unwrap_or_else(|error| panic!("write {}: {error}", out.display()));
    placements.into_iter().map(|(path, ..)| path).collect()
}

/// A copy of [`flash_b`] whose gauge has no cached body
/// ([`without_the_cached_bodies_of`]): its one placement of the gauge is
/// drawn from the symbol library, the other 64 from the bodies the drawing
/// caches, as before.
fn flash_b_without_the_gauges_cached_body(case: &str) -> PathBuf {
    let path = scratch(case).join("flash-b-no-cached-gauge.pid");
    let placed = without_the_cached_bodies_of(&flash_b(), "PG-Pressure Gauge.sym", &path);
    assert_eq!(
        placed.len(),
        1,
        "the fixture places the gauge once: {placed:?}"
    );
    path
}

/// A symbol root that holds one file, `bytes`, at `relative`.
fn a_library_holding(case: &str, relative: &[&str], bytes: &[u8]) -> PathBuf {
    let root = scratch(case).join("library");
    let path = relative
        .iter()
        .fold(root.clone(), |path, part| path.join(part));
    std::fs::create_dir_all(path.parent().expect("a directory"))
        .unwrap_or_else(|error| panic!("mkdir for {}: {error}", path.display()));
    std::fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    root
}

/// A symbol root that holds one file, the gauge as `bytes`.
fn a_library_of_the_gauge(case: &str, bytes: &[u8]) -> PathBuf {
    a_library_holding(case, &GAUGE, bytes)
}

/// What a chain stream opens with.
const CHAIN_MAGIC: [u8; 4] = [0x44, 0xF5, 0x90, 0x6C];

/// The record starts of a chain stream: an eight-byte header, then records
/// end to end.
fn record_starts(data: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut at = 8usize;
    while at + 6 <= data.len() {
        starts.push(at);
        let length = u32::from_le_bytes(data[at + 2..at + 6].try_into().expect("four bytes"));
        at += 6 + length as usize;
    }
    starts
}

/// The gauge with each stream of `streams` changed by `damage` — which is
/// handed the stream's bytes and the offset of its middle record — and what
/// a reader of each chain finds wrong with it, in the order of `streams`.
///
/// The file keeps four chain streams a body is read from: `/Sheet6`, its
/// normal view; `/Sheet68`, a second sheet the sound file is not drawn
/// from; `/StyleCluster`, its pens; and `/PSMcluster0`, its layers and
/// which of them each sheet shows.
fn a_gauge_damaged_in(
    streams: &[&str],
    damage: fn(&mut [u8], usize),
) -> (Vec<u8>, Vec<pid_parse::StreamChainFault>) {
    use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};

    let package = pid_parse::PidParser::new()
        .parse_package_from_reader(std::io::Cursor::new(the_gauge()))
        .unwrap_or_else(|error| panic!("the gauge: {error}"));
    let mut plan = WritePlan::default();
    let mut faults = Vec::new();
    for stream in streams {
        let mut data = package
            .get_stream(stream)
            .unwrap_or_else(|| panic!("the gauge has no {stream}"))
            .data
            .clone();
        assert!(
            data.starts_with(&CHAIN_MAGIC),
            "{stream} is no chain stream"
        );
        let records = record_starts(&data);
        assert!(
            records.len() >= 2,
            "{stream} holds {} record(s)",
            records.len()
        );
        damage(&mut data, records[records.len() / 2]);
        let fault = pid_parse::StreamChainFault::of_stream(stream, &data)
            .unwrap_or_else(|| panic!("the chain of {stream} holds"));
        assert!(fault.stop.is_some(), "{fault:?}");
        faults.push(fault);
        plan.stream_replacements.push(StreamReplacement {
            path: (*stream).to_string(),
            new_data: data,
        });
    }
    let bytes = PidWriter::write_to_bytes(&package, &plan)
        .unwrap_or_else(|error| panic!("the damaged gauge is written: {error}"));
    (bytes, faults)
}

/// The gauge with its fullest sheet — `/Sheet6`, its normal view — changed
/// by `damage`, which is handed the stream's bytes and the offset of its
/// middle record, and what a reader of the chain finds wrong with it.
fn a_damaged_gauge(damage: fn(&mut [u8], usize)) -> (Vec<u8>, pid_parse::StreamChainFault) {
    let package = pid_parse::PidParser::new()
        .parse_package_from_reader(std::io::Cursor::new(the_gauge()))
        .unwrap_or_else(|error| panic!("the gauge: {error}"));
    let fullest = package
        .streams
        .iter()
        .filter(|(path, raw)| {
            path.rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("Sheet"))
                && raw.data.starts_with(&CHAIN_MAGIC)
        })
        .max_by_key(|(_, raw)| record_starts(&raw.data).len())
        .map(|(path, _)| path.clone())
        .expect("the gauge has a sheet");
    assert_eq!(fullest, "/Sheet6", "the gauge's fullest sheet");
    let (bytes, mut faults) = a_gauge_damaged_in(&["/Sheet6"], damage);
    (bytes, faults.remove(0))
}

/// The middle record of the sheet states a length of zero: the chain stops
/// on it, and the symbol reader's own walk does not reach the stream's end
/// either.
fn a_length_of_zero(data: &mut [u8], record: usize) {
    data[record + 2..record + 6].copy_from_slice(&0u32.to_le_bytes());
}

/// The middle record's type word carries the `0x4000` flag: the chain stops
/// on it, as the native reader does, and the symbol reader — which masks a
/// type word to its class — reads the record like any other.
fn a_flagged_type_word(data: &mut [u8], record: usize) {
    data[record + 1] |= 0x40;
}

/// The page of `drawing` laid out against `corpus` with `library` for its
/// one symbol root, and its account.
fn laid_out_against(drawing: &Path, corpus: &Path, library: &Path) -> (PaperModel, ExportLedger) {
    let model = lay_out_with_symbol_roots(
        drawing,
        corpus,
        vec![library.to_path_buf()],
        Vec::new(),
        &["a library of one gauge".to_string()],
    )
    .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let write =
        write_ledger(&model).unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let account = ExportLedger::assemble(&model, &write);
    assert!(
        account.conservation_breaches().is_empty(),
        "{}: {:?}",
        drawing.display(),
        account.conservation_breaches()
    );
    let doc = pid_parse::PidParser::new()
        .parse_file(drawing)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mismatches = account.cross_check(&model, None, Some(&geometry));
    assert!(
        mismatches.is_empty(),
        "{}: the account disagrees with the counts the code keeps: {mismatches:#?}",
        drawing.display()
    );
    (model, account)
}

/// [`laid_out_against`] the repository's fixtures.
fn laid_out_with(drawing: &Path, library: &Path) -> (PaperModel, ExportLedger) {
    laid_out_against(drawing, &fixtures(), library)
}

/// Start `pid_plot` on `drawing` against `corpus` with `library` for its
/// symbol library, as far as this machine lets an export go.
fn export_against(
    case: &str,
    drawing: &Path,
    corpus: &Path,
    library: &Path,
    name: &str,
) -> Outcome {
    let out = unwritten(&scratch(case), name);
    let fonts = fonts_for_a_child();
    let mut command = pid_plot();
    if let Ok(fonts) = &fonts {
        command.env("PID_PLOT_FONTS", fonts);
    }
    command.env("PID_SYMBOL_LIBRARY", library);
    let outcome = run(command.arg(drawing).arg(&out).arg(corpus));
    match fonts {
        Ok(_) => {
            assert_eq!(outcome.code, Some(0), "{outcome}");
            assert!(out.exists(), "{} was not written\n{outcome}", out.display());
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

/// [`export_against`] the repository's fixtures.
fn export_with_a_library(case: &str, drawing: &Path, library: &Path, name: &str) -> Outcome {
    export_against(case, drawing, &fixtures(), library, name)
}

/// Whether a primitive of the page is one of the gauge's placement.
fn of_the_gauge(primitive: &PaperPrimitive) -> bool {
    primitive.symbol.as_deref().is_some_and(|symbol| {
        symbol
            .to_ascii_lowercase()
            .ends_with("pg-pressure gauge.sym")
    })
}

/// What a page draws of the gauge: each primitive of its placement by the
/// kind of shape it is — and, for lettering, what it says — sorted.
fn the_gauge_as_drawn(model: &PaperModel) -> Vec<String> {
    let mut drawn: Vec<String> = model
        .primitives
        .iter()
        .filter(|primitive| of_the_gauge(primitive))
        .map(|primitive| match &primitive.shape {
            PaperShape::Circle { .. } => "circle".to_string(),
            PaperShape::Text { value, .. } => format!("text {value:?}"),
            PaperShape::Line { .. } => "line".to_string(),
            PaperShape::Arc { .. } => "arc".to_string(),
            PaperShape::Fill { .. } => "fill".to_string(),
            PaperShape::Polyline { .. } => "polyline".to_string(),
        })
        .collect();
    drawn.sort();
    drawn
}

/// Every primitive of the page that is not the gauge's.
fn all_but_the_gauge(model: &PaperModel) -> Vec<PaperPrimitive> {
    model
        .primitives
        .iter()
        .filter(|primitive| !of_the_gauge(primitive))
        .cloned()
        .collect()
}

/// How many placements of one domain a page draws from `source`.
fn drawn_from_in(domain: &DomainAccount, source: PlacementSource) -> u64 {
    domain.bodies.placements.get(&source).copied().unwrap_or(0)
}

/// How many placements of the drawing's own a page draws from each source.
fn drawn_from(account: &ExportLedger, source: PlacementSource) -> u64 {
    drawn_from_in(&account.drawing, source)
}

/// What one domain's account files of damaged symbol files: the site, the
/// count and the bytes left unread.
fn symbol_files_filed_in(domain: &DomainAccount) -> Vec<(&str, u64, Option<u64>)> {
    domain
        .parse
        .items
        .iter()
        .filter(|item| item.reason == DropReason::SymbolFileChainFault)
        .map(|item| (item.stream.as_str(), item.count, item.bytes))
        .collect()
}

/// What the account files of damaged symbol files for the drawing's own
/// domain.
fn symbol_files_filed(account: &ExportLedger) -> Vec<(&str, u64, Option<u64>)> {
    symbol_files_filed_in(&account.drawing)
}

/// Every suspect count of the page's account, by code, over the page and
/// every layer of both domains: what the baseline gate holds a page to.
fn suspects(account: &ExportLedger) -> std::collections::BTreeMap<&'static str, u64> {
    let mut by_code = std::collections::BTreeMap::new();
    let mut add = |reason: DropReason, count: u64| {
        if reason.tier() == Tier::Suspect && count > 0 {
            *by_code.entry(reason.code()).or_insert(0) += count;
        }
    };
    for (reason, count) in &account.page.dropped {
        add(*reason, *count);
    }
    for domain in [&account.drawing, &account.furniture] {
        for dropped in [
            &domain.entities.dropped,
            &domain.bodies.dropped,
            &domain.primitives.dropped,
            &domain.segments.dropped,
        ] {
            for (reason, count) in dropped {
                add(*reason, *count);
            }
        }
        for item in &domain.parse.items {
            add(item.reason, item.count);
        }
    }
    by_code
}

/// A placement the drawing caches no body for is drawn from the symbol
/// library, and a sound symbol file is no occasion for a word.
///
/// The control for the case below, and the measure of how a page comes to
/// read a `.sym` at all: the fixture as committed draws all 65 of its
/// placements from the bodies it caches, whatever the library holds; the
/// copy whose gauge names a body the drawing does not hold draws 64 from
/// its caches and the gauge from the library.
#[test]
fn a_placement_with_no_cached_body_is_drawn_from_the_library_without_a_word() {
    let case = "symbol-sound";
    let library = a_library_of_the_gauge(case, &the_gauge());
    let (committed_model, committed) = laid_out_with(&flash_b(), &library);
    assert_eq!(
        (
            drawn_from(&committed, PlacementSource::Cached),
            drawn_from(&committed, PlacementSource::Library)
        ),
        (65, 0),
        "the fixture as committed"
    );
    let drawing = flash_b_without_the_gauges_cached_body(case);
    let (model, account) = laid_out_with(&drawing, &library);
    assert_eq!(
        (
            drawn_from(&account, PlacementSource::Cached),
            drawn_from(&account, PlacementSource::Library)
        ),
        (64, 1),
        "the copy whose gauge has no cached body"
    );
    assert!(symbol_files_filed(&account).is_empty());
    assert!(model.ledger.symbol_chain_faults.is_empty());
    // The file's body and the one the drawing caches of it are the same
    // gauge: one circle, and the rest of the page is untouched.
    assert_eq!(the_gauge_as_drawn(&committed_model), ["circle"]);
    assert_eq!(the_gauge_as_drawn(&model), ["circle"]);
    assert_eq!(
        all_but_the_gauge(&model),
        all_but_the_gauge(&committed_model),
        "the rest of the page"
    );
    assert_eq!(suspects(&account), suspects(&committed), "the suspects");
    let said = export_with_a_library(case, &drawing, &library, "sound.pdf");
    assert!(
        warnings(&said.stderr).is_empty(),
        "a sound symbol file drew a warning\n{said}"
    );
}

/// A symbol file whose record chain stops is said where a placement is
/// drawn from it.
///
/// The reader of a `.sym` takes a sheet only where its own walk of it ends
/// on the stream's last byte, so a sheet with a length of zero in the
/// middle gives the body nothing and the placement is drawn from the
/// sheets that were read. The parser says so of the file
/// (`SymbolGeometry::chain_faults`); a page drawn from such a body has to
/// say so too. The copy whose gauge has no cached body, with a library
/// whose gauge has one length of its fullest sheet set to zero: the program
/// prints exactly one warning, which names the symbol file and says the
/// reader's own sentence of the stream — its path, where the chain stops,
/// what is left unread — and the account files the file once as a suspect
/// item of the drawing's. The exit code is the sound library's.
///
/// What the page then draws of the gauge is not less, it is other. The
/// file keeps its body on two sheets: `/Sheet6`, the normal view, whose one
/// displayed stroke is the gauge's circle, and `/Sheet68`, the same circle
/// and the lettering `PG`. With `/Sheet6` unread the body is `/Sheet68`
/// alone, and the page draws the circle and a `PG` the sound file does not
/// letter; every other primitive of the page is the same.
///
/// The fixture as committed, with the same damaged library, says nothing
/// and files nothing: it draws the gauge from the body it caches, and a
/// file that no placement is drawn from costs the page nothing.
#[test]
fn a_symbol_file_whose_chain_stops_is_said_where_a_placement_is_drawn_from_it() {
    let case = "symbol-chain-stops";
    let (bytes, fault) = a_damaged_gauge(a_length_of_zero);
    let library = a_library_of_the_gauge(case, &bytes);
    let sentence = fault.describe();
    assert!(
        sentence.contains(&fault.stream_path) && sentence.contains("stops at +"),
        "{sentence}"
    );

    // Drawn from it: the copy whose gauge has no cached body.
    let drawing = flash_b_without_the_gauges_cached_body(case);
    let (model, account) = laid_out_with(&drawing, &library);
    assert_eq!(
        drawn_from(&account, PlacementSource::Library),
        1,
        "the gauge is drawn from the library's file"
    );
    let filed = symbol_files_filed(&account);
    assert_eq!(
        filed,
        [(
            format!("{}:{}", GAUGE.join("\\"), fault.stream_path).as_str(),
            1,
            Some(fault.unread_bytes as u64)
        )],
        "the symbol file, filed once with what its chain left unread"
    );
    assert_eq!(DropReason::SymbolFileChainFault.tier(), Tier::Suspect);
    assert_eq!(
        model.ledger.symbol_chain_faults.len(),
        1,
        "{:?}",
        model.ledger.symbol_chain_faults
    );
    let found = &model.ledger.symbol_chain_faults[0];
    assert_eq!(
        (
            found.domain,
            found.symbol.as_str(),
            found.placements,
            &found.fault
        ),
        (LedgerDomain::Drawing, GAUGE.join("\\").as_str(), 1, &fault)
    );
    // What the damaged file changes of the page's suspects, against the
    // same drawing with the sound file: the file itself, once.
    let (sound_model, sound) = laid_out_with(
        &drawing,
        &a_library_of_the_gauge("symbol-chain-stops-sound", &the_gauge()),
    );
    let mut expected = suspects(&sound);
    *expected.entry("symbol_file_chain_fault").or_insert(0) += 1;
    assert_eq!(suspects(&account), expected, "the suspects of the page");
    // The count a gate reads, and the program prints, is the summary's:
    // one suspect more than with the sound file.
    assert_eq!(
        account.summary().suspect,
        sound.summary().suspect + 1,
        "the summary with the damaged file, `{}`, against the sound file's, `{}`",
        account.summary(),
        sound.summary()
    );
    // And of the page: the gauge is drawn from the sheet that was read.
    assert_eq!(the_gauge_as_drawn(&sound_model), ["circle"]);
    assert_eq!(the_gauge_as_drawn(&model), ["circle", "text \"PG\""]);
    assert_eq!(
        all_but_the_gauge(&model),
        all_but_the_gauge(&sound_model),
        "the rest of the page"
    );
    let said = export_with_a_library(case, &drawing, &library, "stopped.pdf");
    let warned = warnings(&said.stderr);
    assert_eq!(
        warned.len(),
        1,
        "a symbol file whose chain stops, drawn from, is exactly one warning\n{said}"
    );
    eprintln!("{case}: pid_plot said: {}", warned[0]);
    for needle in [
        drawing.display().to_string(),
        "PG-Pressure Gauge.sym".to_string(),
        sentence,
        "1 placement(s)".to_string(),
    ] {
        assert!(
            warned[0].contains(&needle),
            "the warning does not say {needle:?}\n{said}"
        );
    }

    assert_eq!(
        model.ledger.symbol_chain_fault_warnings(),
        [format!(
            "the symbol file {}, which 1 placement(s) on the page are drawn from, is damaged: {}",
            GAUGE.join("\\"),
            fault.describe()
        )],
        "the sentence the program prints after the drawing's path"
    );
    if said.code == Some(0) {
        // The export ran to its end, so the program printed its summary:
        // the account's own, with the file among its suspects.
        assert_eq!(
            ledger_count(&said, "suspect"),
            account.summary().suspect,
            "the program's summary is not the account's\n{said}"
        );
    }

    // Not drawn from it: the fixture as committed, the same library.
    let (_, committed) = laid_out_with(&flash_b(), &library);
    assert_eq!(drawn_from(&committed, PlacementSource::Library), 0);
    assert!(symbol_files_filed(&committed).is_empty());
    let silent = export_with_a_library(case, &flash_b(), &library, "committed.pdf");
    assert!(
        warnings(&silent.stderr).is_empty(),
        "a symbol file no placement is drawn from drew a warning\n{silent}"
    );
}

/// A fault the symbol reader reads through is said all the same.
///
/// The reader of a `.sym` masks a type word to its class before it reads
/// the record, so a record whose type word carries the `0x4000` flag —
/// which the chain walk stops on, as the native reader does — is read like
/// any other: the sheet is taken whole, and the page drawn from the file is
/// the page drawn from the sound one, primitive for primitive. The file is
/// damaged all the same, and nothing on the page says that it lost nothing
/// by it: the file is filed and said exactly as the one whose sheet is left
/// out of the body.
#[test]
fn a_symbol_file_fault_the_symbol_reader_reads_through_is_said_all_the_same() {
    let case = "symbol-chain-read-through";
    let (bytes, fault) = a_damaged_gauge(a_flagged_type_word);
    assert!(
        fault.describe().contains("is not one a record opens with"),
        "{}",
        fault.describe()
    );
    let drawing = flash_b_without_the_gauges_cached_body(case);
    let (model, account) = laid_out_with(&drawing, &a_library_of_the_gauge(case, &bytes));
    let (sound_model, sound) = laid_out_with(
        &drawing,
        &a_library_of_the_gauge("symbol-chain-read-through-sound", &the_gauge()),
    );
    assert_eq!(model.primitives, sound_model.primitives, "the page");
    assert_eq!(
        symbol_files_filed(&account),
        [(
            format!("{}:{}", GAUGE.join("\\"), fault.stream_path).as_str(),
            1,
            Some(fault.unread_bytes as u64)
        )]
    );
    let mut expected = suspects(&sound);
    *expected.entry("symbol_file_chain_fault").or_insert(0) += 1;
    assert_eq!(suspects(&account), expected, "the suspects of the page");
    // The count a gate reads is the summary's: one suspect more, for a
    // page that is the sound file's primitive for primitive.
    assert_eq!(
        account.summary().suspect,
        sound.summary().suspect + 1,
        "the summary with the damaged file, `{}`, against the sound file's, `{}`",
        account.summary(),
        sound.summary()
    );
    assert_eq!(
        model.ledger.symbol_chain_fault_warnings(),
        [format!(
            "the symbol file {}, which 1 placement(s) on the page are drawn from, is damaged: {}",
            GAUGE.join("\\"),
            fault.describe()
        )]
    );
    assert!(sound_model.ledger.symbol_chain_fault_warnings().is_empty());
}
