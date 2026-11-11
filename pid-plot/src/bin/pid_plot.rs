//! Write one `.pid` drawing to a PDF.
//!
//! Exit code `0` means a page was written and `1` that the export failed —
//! the drawing could not be read, a typeface is missing, the file could not
//! be written. `2` is a command line that cannot be run as given: too few
//! arguments, or a `[corpus]` that is not a directory.
//!
//! A page can be written and still lack something the drawing asked for.
//! Three such gaps are a `warning:` line on stderr and leave the exit code
//! and the file alone: no corpus was named and the fallback is not a
//! directory; the drawing names a border template that no file in reach of
//! the corpus answers, so the page carries no frame and no title block; and a
//! stream of the drawing, or of its template, has a record chain that does
//! not hold, so the page carries what the records before the break draw and
//! nothing of what follows it.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ledger::ExportLedger;
use pid_plot::model::{PaperModel, TemplatePlaces, border_template_places, lay_out_drawing};
use pid_plot::write::{PlotFont, write_plot_audited};

/// What a command line has to look like; printed with every argument error.
const USAGE: &str = "usage: pid_plot <drawing.pid> <out.pdf> [corpus]";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(pid) = args.next() else {
        eprintln!("{USAGE}");
        eprintln!("typefaces come from PID_PLOT_FONTS, then <corpus>/fonts");
        return ExitCode::from(2);
    };
    let Some(out) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    // The corpus is where the sheet's border template, the symbol library and
    // the typefaces are looked for. One named on the command line is the
    // caller's own statement, and the layout takes any path without complaint
    // — against one that is not a directory it just draws a page with no
    // frame and no title block — so the mistake is refused here, before a
    // file is written that looks like a finished export. The fallback is only
    // a guess about this machine and a drawing exports without it, so its
    // absence is said and the run goes on.
    let corpus = match args.next() {
        Some(named) => {
            let named = PathBuf::from(named);
            if !named.is_dir() {
                eprintln!("corpus {} is not a directory", named.display());
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            named
        }
        None => {
            let fallback = default_corpus();
            if !fallback.is_dir() {
                eprintln!(
                    "warning: no corpus was named and the fallback {} is not a directory, \
                     so it supplies no border template, symbol library or typefaces; \
                     name one as [corpus] or in PID_PARSE_SELF_CHECK_DIR",
                    fallback.display()
                );
            }
            fallback
        }
    };
    let model = match lay_out_drawing(Path::new(&pid), &corpus) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    // A sheet that names its border template wears only that template, so a
    // corpus that cannot answer the name leaves the page with no frame and no
    // title block — and the file written looks like a finished export of a
    // bare sheet. The page is the drawing's own ink either way and is written
    // all the same; what is missing and where it was looked for is said here,
    // before a typeface is asked for, so a run that stops there has said it
    // too. A sheet that names no template has nothing missing.
    if !model.missing_templates.is_empty() {
        let places = border_template_places(Path::new(&pid), &corpus);
        eprintln!("{}", missing_template_warning(&pid, &model, &places));
    }
    // A stream whose record chain does not hold was read as far as the chain
    // reads: the page carries what the records before the stop draw and
    // nothing of what follows, and it looks like a finished export of a
    // sparser drawing. It is written all the same — those records are the
    // drawing's — and each such stream is named here with the offset it
    // stopped at and the bytes left unread. The account counts it as a
    // suspect item, which is what a gate reads.
    for fault in model.ledger.chain_fault_warnings() {
        eprintln!("warning: {pid}: {fault}");
    }
    let mut prepared = match prepare_fonts(&[&model], &font_dirs(Some(&corpus))) {
        Ok(faces) => faces,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    // `PID_PLOT_CJK_FONT` names one more face to embed, whole and under no
    // name. No label is named in it, and the writer asks it after every face
    // of the borrow order (`fonts::BORROW_ORDER`), so it letters one
    // thing only: a stretch its named face draws none of and no single face
    // of that order draws whole, which would otherwise be split between
    // lenders. It cannot make a drawing exportable. `prepare_fonts` settles
    // what can be drawn from the faces it loads, is not told about this one,
    // and has already stopped the run above for a character none of them
    // draws.
    if let Ok(path) = env::var("PID_PLOT_CJK_FONT") {
        match fs::read(&path) {
            Ok(bytes) => prepared.push(pid_plot::fonts::PreparedFont {
                key: String::new(),
                bytes,
                subset: false,
            }),
            Err(error) => {
                eprintln!("read {path}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let written = match write_plot_audited(&model, &fonts) {
        Ok(written) => written,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    // The page's account, joined from the layout half on the model and the
    // write half the writer just produced. Assembled before `pdf` is moved so
    // the summary describes the same write the file does.
    let ledger = ExportLedger::assemble(&model, &written.ledger);
    let pdf = written.pdf;
    let audit = written.audit;
    if let Some(parent) = PathBuf::from(&out).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("mkdir {}: {error}", parent.display());
        return ExitCode::from(1);
    }
    if let Err(error) = fs::write(&out, &pdf) {
        eprintln!("write {out}: {error}");
        return ExitCode::from(1);
    }
    // The page actually written. A drawing that states none is measured, and
    // this is the number the PDF carries — reporting nothing here would say
    // `0.00 0.00` about a page that was plotted on real paper.
    let page = model.resolved_page_mm().unwrap_or([0.0, 0.0]);
    println!(
        "primitives {} segments_written_page_mm {:.2} {:.2} bytes {}",
        model.primitives.len(),
        page[0],
        page[1],
        pdf.len()
    );
    // Said out loud even when zero: a run the sheet cannot hold at half width
    // loses its tail to the page clip, and nothing on the page records that it
    // happened.
    println!(
        "text_clipped_at_condensation_floor {} deepest_clip_mm {:.3}",
        audit.runs_clipped_at_the_condensation_floor, audit.deepest_clip_mm
    );
    // A run the sheet cannot hold at its set width is condensed instead of
    // clipped wherever it can fit that way, which draws the label narrower than
    // the drawing asked — printed for the same reason, zero included.
    println!(
        "text_condensed {} smallest_condense {:.3}",
        audit.runs_condensed, audit.smallest_condense
    );
    // The ledger's own summary (requirement 14.3): what the page's entities
    // did, how much of the export was deliberately not drawn, and how much
    // nothing accounts for. `unattributed` is printed even at zero — that is
    // the number a reader has to see to know the page balanced.
    println!("{}", ledger.summary());
    ExitCode::SUCCESS
}

/// The one line that says the border template `pid` names was not found.
///
/// `PaperModel::missing_templates` holds the stems the drawing's link records
/// spell, and a template answers one when its file name begins with it — an
/// 8.3 short name such as `CPECCH~2.IGR` keeps only the first six characters
/// — so each is printed as the `<stem>*.igr` it stands for. The places are
/// the library's own account of where it reads templates from, and the count
/// of `.igr` files is what the search saw there: none means the corpus holds
/// no template at all, some means it holds other sheets' templates.
fn missing_template_warning(pid: &str, model: &PaperModel, places: &TemplatePlaces) -> String {
    let listed = |paths: &[PathBuf]| -> String {
        if paths.is_empty() {
            return "none".to_string();
        }
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let names: Vec<String> = model
        .missing_templates
        .iter()
        .map(|stem| format!("{stem}*.igr"))
        .collect();
    let line = format!(
        "warning: {pid} names its border template ({}) and none of that name and the \
         sheet's size was found, so the page carries no frame and no title block; \
         template directories searched: {}; RefData archives searched: {}; \
         .igr files seen there: {}",
        names.join(", "),
        listed(&places.directories),
        listed(&places.archives),
        model.ledger.sources.templates.len()
    );
    let unreadable: Vec<String> = model
        .ledger
        .template_unreadable
        .iter()
        .map(|source| match &source.archive {
            Some(archive) => format!("{} in {archive}", source.name),
            None => source.name.clone(),
        })
        .collect();
    if unreadable.is_empty() {
        line
    } else {
        format!("{line}; could not be read: {}", unreadable.join(", "))
    }
}

fn default_corpus() -> PathBuf {
    env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}
