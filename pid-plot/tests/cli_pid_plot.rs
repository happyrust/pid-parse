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
use pid_plot::ledger::{DropReason, ExportLedger};
use pid_plot::model::{PaperModel, PaperPrimitive, border_template_places, lay_out_drawing};
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
    /// before it is untouched. The stream's length for [`Break::None`].
    first_touched: usize,
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
    let (stop, first_touched) = match how {
        Break::None => (None, len),
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
    assert!(
        !before.is_empty(),
        "{how:?}: the intact drawing draws nothing from the records before +{}",
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
    // the page as the intact drawing's do; see the note above.
    let same_frames = page_frames(&broken.path) == page_frames(&dwg_0201());
    assert_eq!(
        after == before,
        same_frames,
        "{how:?}: the records before the stop are placed as on the intact drawing exactly \
         when the page frame is among them (frames read: {:?})",
        page_frames(&broken.path)
    );

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
    match broken.stop {
        None => {
            assert_eq!(model.primitives.len(), intact.primitives.len());
            assert!(filed.is_empty(), "{how:?}: filed {filed:?}");
            assert!(
                warned.is_empty(),
                "{how:?}: a drawing rewritten as it is drew a warning\n{outcome}"
            );
        }
        Some((stop, unread)) => {
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
    match broken.stop {
        None => {
            assert!(sites.is_empty(), "{how:?}: {sites:?}");
            assert_eq!(summary, intact_summary, "{how:?}");
        }
        Some(_) => {
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
#[test]
fn a_sheet_whose_list_is_not_its_own_exports_and_says_so() {
    use pid_plot::ledger::{LedgerDomain, UnplacedWhy};

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
