//! Write `.pid` drawings to PDF: one drawing, or every drawing under a
//! directory.
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
//!
//! `pid_plot --batch <directory> <out-directory> [corpus]` exports every
//! `.pid` under the directory, at any depth and in path order, each as
//! `pid_plot <drawing.pid> <out.pdf> [corpus]` would: against the same
//! corpus, with the same typefaces and warnings, into the same bytes. A
//! drawing's page goes where the drawing sits, under the out-directory in
//! place of the directory, as `<drawing's file stem>.pdf`, and a drawing
//! that fails does not stop the ones after it. stdout says `exported
//! <drawing> -> <pdf>` and the lines a single export prints, or `failed
//! <drawing>`, for each drawing in turn; then a `batch` line of totals, an
//! `error` line for each drawing that failed, a `drawing` line of counts for
//! each page that lacks something, with a `missing_symbols` line naming the
//! symbol files, and the path of `pid_plot-batch.json` in the
//! out-directory, which holds all of it drawing by drawing with each one's
//! warnings and ledger summary. A batch exits `0` when every page was
//! written, every directory under the batch directory was listed and every
//! page's account balanced; `1` when one or more was not written, a
//! directory under it could not be listed, or a page left a remainder
//! nothing accounts for — such a page is named by a `drawing <path>
//! unattributed <n>` line after the totals; `2` is too few arguments, a
//! directory or `[corpus]` that is not a directory, or a directory with no
//! `.pid` under it.

use std::collections::hash_map::{self, HashMap};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ledger::{ExportLedger, LedgerSummary};
use pid_plot::model::{PaperModel, TemplatePlaces, border_template_places, lay_out_drawing};
use pid_plot::write::{PlotFont, write_plot_titled};

/// What a command line has to look like; printed with every argument error.
const USAGE: &str = "usage: pid_plot <drawing.pid> <out.pdf> [corpus]";
/// The batch form, printed on the line after [`USAGE`].
const BATCH_USAGE: &str = "       pid_plot --batch <directory> <out-directory> [corpus]";
/// The account a batch leaves in its out-directory.
const BATCH_SUMMARY: &str = "pid_plot-batch.json";

fn main() -> ExitCode {
    let mut args = env::args().skip(1).peekable();
    if args.next_if(|first| first == "--batch").is_some() {
        return batch(args);
    }
    single(args)
}

fn usage() {
    eprintln!("{USAGE}");
    eprintln!("{BATCH_USAGE}");
}

/// `pid_plot <drawing.pid> <out.pdf> [corpus]`.
fn single(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(pid) = args.next() else {
        usage();
        eprintln!("typefaces come from PID_PLOT_FONTS, then <corpus>/fonts");
        return ExitCode::from(2);
    };
    let Some(out) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let Some(corpus) = corpus(args.next()) else {
        return ExitCode::from(2);
    };
    let page = export(Path::new(&pid), Path::new(&out), &corpus);
    if page.error.is_some() {
        return ExitCode::from(1);
    }
    for line in &page.report {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// `pid_plot --batch <directory> <out-directory> [corpus]`.
fn batch(mut args: impl Iterator<Item = String>) -> ExitCode {
    let (Some(directory), Some(out_directory)) = (args.next(), args.next()) else {
        usage();
        return ExitCode::from(2);
    };
    let directory = PathBuf::from(directory);
    let out_directory = PathBuf::from(out_directory);
    if !directory.is_dir() {
        eprintln!("batch directory {} is not a directory", directory.display());
        usage();
        return ExitCode::from(2);
    }
    let Some(corpus) = corpus(args.next()) else {
        return ExitCode::from(2);
    };
    let listing = drawings_under(&directory);
    for unlisted in &listing.unlisted {
        eprintln!("list {}: {}", unlisted.directory, unlisted.error);
    }
    if listing.drawings.is_empty() && listing.unlisted.is_empty() {
        eprintln!("no .pid drawing under {}", directory.display());
        usage();
        return ExitCode::from(2);
    }
    // Two drawings whose names differ only in the case of `.pid` would have
    // one page; the second is refused rather than written over the first.
    let mut claimed: HashMap<PathBuf, &Path> = HashMap::new();
    let mut pages = Vec::with_capacity(listing.drawings.len());
    for drawing in &listing.drawings {
        let relative = drawing.strip_prefix(&directory).unwrap_or(drawing);
        let pdf = relative.with_extension("pdf");
        let out = out_directory.join(&pdf);
        let page = match claimed.entry(out.clone()) {
            hash_map::Entry::Occupied(first) => Page::default().failed(format!(
                "{}: its page would overwrite the page of {} at {}",
                drawing.display(),
                first.get().display(),
                out.display()
            )),
            hash_map::Entry::Vacant(slot) => {
                slot.insert(drawing);
                export(drawing, &out, &corpus)
            }
        };
        if page.error.is_none() {
            println!("exported {} -> {}", drawing.display(), out.display());
            for line in &page.report {
                println!("{line}");
            }
        } else {
            println!("failed {}", drawing.display());
        }
        pages.push(BatchPage {
            path: drawing.clone(),
            drawing: relative.display().to_string(),
            pdf: page.error.is_none().then(|| pdf.display().to_string()),
            page,
        });
    }
    let summary = BatchSummary::of(&directory, &out_directory, &corpus, listing.unlisted, pages);
    summary.print();
    let path = out_directory.join(BATCH_SUMMARY);
    if let Err(error) = summary.save(&out_directory, &path) {
        eprintln!("write {}: {error}", path.display());
        return ExitCode::from(1);
    }
    println!("summary {}", path.display());
    ExitCode::from(summary.exit_code())
}

/// The corpus to lay drawings out against, or `None` when the one named is
/// not a directory, which has been said, with the usage.
///
/// The corpus is where the sheet's border template, the symbol library and
/// the typefaces are looked for. One named on the command line is the
/// caller's own statement, and the layout takes any path without complaint
/// — against one that is not a directory it just draws a page with no
/// frame and no title block — so the mistake is refused here, before a
/// file is written that looks like a finished export. The fallback is only
/// a guess about this machine and a drawing exports without it, so its
/// absence is said and the run goes on.
fn corpus(named: Option<String>) -> Option<PathBuf> {
    match named {
        Some(named) => {
            let named = PathBuf::from(named);
            if !named.is_dir() {
                eprintln!("corpus {} is not a directory", named.display());
                usage();
                return None;
            }
            Some(named)
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
            Some(fallback)
        }
    }
}

/// What exporting one drawing came to, as far as the export got: one that
/// failed still carries what was said of it before it stopped.
#[derive(Default, Serialize)]
struct Page {
    /// The lines a single export prints on stdout once its file is written.
    #[serde(skip)]
    report: Vec<String>,
    /// Why the export stopped, as stderr has it; `None` once the file is
    /// written.
    error: Option<String>,
    /// The size of the file written.
    bytes: Option<usize>,
    /// The primitives on the page written.
    primitives: Option<usize>,
    /// The page's account, summed.
    ledger: Option<LedgerSummary>,
    /// The border template stems no file in reach answered, so the page is
    /// bare.
    missing_templates: Vec<String>,
    /// The symbol files the library lacks or holds damaged.
    missing_symbols: Vec<String>,
    /// Streams of the drawing, its template or a symbol file whose record
    /// chain does not hold.
    chain_breaks: usize,
    /// The rest of the warnings: records that take no place in the drawing
    /// order, a paper that is not the frame's, records a decoder refused.
    other_warnings: usize,
    /// Every `warning:` line said of the drawing, as stderr has it.
    warnings: Vec<String>,
}

impl Page {
    /// Say `line` on stderr and keep it.
    fn warn(&mut self, line: String) {
        eprintln!("{line}");
        self.warnings.push(line);
    }

    /// Say `error` on stderr and keep it as the reason the export stopped.
    fn failed(mut self, error: String) -> Self {
        eprintln!("{error}");
        self.error = Some(error);
        self
    }
}

/// Lay `drawing` out against `corpus` and write its page to `out`, saying on
/// stderr what is said of it.
fn export(drawing: &Path, out: &Path, corpus: &Path) -> Page {
    let pid = drawing.display().to_string();
    let mut page = Page::default();
    let model = match lay_out_drawing(drawing, corpus) {
        Ok(model) => model,
        Err(error) => return page.failed(format!("{pid}: {error}")),
    };
    page.missing_templates.clone_from(&model.missing_templates);
    page.missing_symbols.clone_from(&model.missing_symbols);
    // A sheet that names its border template wears only that template, so a
    // corpus that cannot answer the name leaves the page with no frame and no
    // title block — and the file written looks like a finished export of a
    // bare sheet. The page is the drawing's own ink either way and is written
    // all the same; what is missing and where it was looked for is said here,
    // before a typeface is asked for, so a run that stops there has said it
    // too. A sheet that names no template has nothing missing.
    if !model.missing_templates.is_empty() {
        let places = border_template_places(drawing, corpus);
        page.warn(missing_template_warning(&pid, &model, &places));
    }
    // A stream whose record chain does not hold was read as far as the chain
    // reads: the page carries what the records before the stop draw and
    // nothing of what follows, and it looks like a finished export of a
    // sparser drawing. It is written all the same — those records are the
    // drawing's — and each such stream is named here with the offset it
    // stopped at and the bytes left unread. The account counts it as a
    // suspect item, which is what a gate reads.
    for fault in model.ledger.chain_fault_warnings() {
        page.chain_breaks += 1;
        page.warn(format!("warning: {pid}: {fault}"));
    }
    // The same of a symbol file: a placement drawn from a library body
    // whose file has a chain that does not hold is drawn from whatever was
    // read of the file. Each such file and stream is named, with the
    // reader's sentence of it; a file no placement is drawn from is not —
    // a drawing caches the body of every symbol it places, and reads the
    // library only for one it holds no body of.
    for fault in model.ledger.symbol_chain_fault_warnings() {
        page.chain_breaks += 1;
        page.warn(format!("warning: {pid}: {fault}"));
    }
    // A record that draws and takes no place in the order its sheet draws
    // in is on the page, painted after the rest; what covers what there is
    // not the drawing's word. Each stream of them is named, and the account
    // counts it as a suspect item. Records a stopped chain alone leaves
    // without a place — the chain read none of the sheet's own to hold the
    // sheet's list to — are the chain's warning above and get no line here;
    // the account still counts both.
    for unplaced in model.ledger.display_order_warnings() {
        page.other_warnings += 1;
        page.warn(format!("warning: {pid}: {unplaced}"));
    }
    // A paper that is not the border frame's is not taken for the page: the
    // page is the frame's rectangle and the drawing is whole, but it is not
    // on the paper its sheet states. One line, with both rectangles; the
    // account counts it as a suspect item. Where no frame was read there
    // is only the size the template's name states, which refuses no paper:
    // the page is the paper, and a paper that is not the name's size is one
    // line all the same, with both sizes, and a suspect item of its own.
    for paper in model.ledger.page_warnings() {
        page.other_warnings += 1;
        page.warn(format!("warning: {pid}: {paper}"));
    }
    // Graphic records in a sheet or cached symbol body that their family's
    // decoder refused are dropped from the page and filed as suspect
    // (`DropReason::RecordRefused`). Name each site here (border frames are
    // already spoken for by `page_warnings` above).
    for refused in model.ledger.refused_record_warnings() {
        page.other_warnings += 1;
        page.warn(format!("warning: {pid}: {refused}"));
    }
    let mut prepared = match prepare_fonts(&[&model], &font_dirs(Some(corpus))) {
        Ok(faces) => faces,
        Err(error) => return page.failed(format!("{pid}: {error}")),
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
            Err(error) => return page.failed(format!("read {path}: {error}")),
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
    // The page is titled with the drawing's file name. The gates that hold
    // their page to this one byte for byte title theirs the same way.
    let title = drawing.file_stem().map(|stem| stem.to_string_lossy());
    let written = match write_plot_titled(&model, &fonts, title.as_deref()) {
        Ok(written) => written,
        Err(error) => return page.failed(format!("{pid}: {error}")),
    };
    // The page's account, joined from the layout half on the model and the
    // write half the writer just produced. Assembled before `pdf` is moved so
    // the summary describes the same write the file does.
    let ledger = ExportLedger::assemble(&model, &written.ledger);
    let pdf = written.pdf;
    let audit = written.audit;
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return page.failed(format!("mkdir {}: {error}", parent.display()));
    }
    if let Err(error) = fs::write(out, &pdf) {
        return page.failed(format!("write {}: {error}", out.display()));
    }
    // The page actually written. A drawing that states none is measured, and
    // this is the number the PDF carries — reporting nothing here would say
    // `0.00 0.00` about a page that was plotted on real paper.
    let size = model.resolved_page_mm().unwrap_or([0.0, 0.0]);
    page.report.push(format!(
        "primitives {} segments_written_page_mm {:.2} {:.2} bytes {}",
        model.primitives.len(),
        size[0],
        size[1],
        pdf.len()
    ));
    // Said out loud even when zero: a run the sheet cannot hold at half width
    // loses its tail to the page clip, and nothing on the page records that it
    // happened.
    page.report.push(format!(
        "text_clipped_at_condensation_floor {} deepest_clip_mm {:.3}",
        audit.runs_clipped_at_the_condensation_floor, audit.deepest_clip_mm
    ));
    // A run the sheet cannot hold at its set width is condensed instead of
    // clipped wherever it can fit that way, which draws the label narrower than
    // the drawing asked — printed for the same reason, zero included.
    page.report.push(format!(
        "text_condensed {} smallest_condense {:.3}",
        audit.runs_condensed, audit.smallest_condense
    ));
    // The ledger's own summary (requirement 14.3): what the page's entities
    // did, how much of the export was deliberately not drawn, and how much
    // nothing accounts for. `unattributed` is printed even at zero — that is
    // the number a reader has to see to know the page balanced.
    let summary = ledger.summary();
    page.report.push(summary.to_string());
    page.bytes = Some(pdf.len());
    page.primitives = Some(model.primitives.len());
    page.ledger = Some(summary);
    page
}

/// The `.pid` files under a batch's directory, in path order, and the
/// directories under it that could not be listed.
struct Listing {
    drawings: Vec<PathBuf>,
    unlisted: Vec<Unlisted>,
}

/// A directory under a batch's that could not be listed, and why.
#[derive(Serialize)]
struct Unlisted {
    directory: String,
    error: String,
}

/// Every `.pid` under `directory` at any depth, the extension matched in any
/// case. A link to a file is followed; a link to a directory is not, so no
/// loop of links can hold the walk and no drawing is reached twice through
/// one.
fn drawings_under(directory: &Path) -> Listing {
    let mut listing = Listing {
        drawings: Vec::new(),
        unlisted: Vec::new(),
    };
    let mut pending = vec![directory.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let unlisted = |error: io::Error| Unlisted {
            directory: dir.display().to_string(),
            error: error.to_string(),
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                listing.unlisted.push(unlisted(error));
                continue;
            }
        };
        for entry in entries {
            let (path, kind) = match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?)))
            {
                Ok(found) => found,
                Err(error) => {
                    listing.unlisted.push(unlisted(error));
                    continue;
                }
            };
            if kind.is_dir() {
                pending.push(path);
            } else if (kind.is_file() || (kind.is_symlink() && path.is_file()))
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
            {
                listing.drawings.push(path);
            }
        }
    }
    listing.drawings.sort();
    listing
        .unlisted
        .sort_by(|one, other| one.directory.cmp(&other.directory));
    listing
}

/// One drawing of a batch: where it is under the directory, where its page
/// went under the out-directory, and what its export came to.
#[derive(Serialize)]
struct BatchPage {
    /// The drawing as stdout and stderr name it.
    #[serde(skip)]
    path: PathBuf,
    drawing: String,
    /// `None` when no page was written.
    pdf: Option<String>,
    #[serde(flatten)]
    page: Page,
}

/// What a batch came to: its totals, the directories it could not list, and
/// each drawing's page in path order.
#[derive(Serialize)]
struct BatchSummary {
    directory: String,
    out_directory: String,
    corpus: String,
    drawings: usize,
    exported: usize,
    failed: usize,
    missing_templates: usize,
    missing_symbols: usize,
    chain_breaks: usize,
    other_warnings: usize,
    /// Summed over the pages written.
    suspect: u64,
    /// Summed over the pages written, and forced to zero by the exit code:
    /// a batch whose `unattributed` is not zero exits 1 and names each page
    /// that leaves one (see [`BatchSummary::unattributed_lines`]). It is the
    /// one number of a page's account that no reason covers — something was
    /// dropped and nothing said what — so a page that carries one is not a
    /// page to hand on, however complete it looks.
    unattributed: u64,
    unlisted: Vec<Unlisted>,
    pages: Vec<BatchPage>,
}

impl BatchSummary {
    fn of(
        directory: &Path,
        out_directory: &Path,
        corpus: &Path,
        unlisted: Vec<Unlisted>,
        pages: Vec<BatchPage>,
    ) -> Self {
        let ledgers = || pages.iter().filter_map(|entry| entry.page.ledger);
        Self {
            directory: directory.display().to_string(),
            out_directory: out_directory.display().to_string(),
            corpus: corpus.display().to_string(),
            drawings: pages.len(),
            exported: pages
                .iter()
                .filter(|entry| entry.page.error.is_none())
                .count(),
            failed: pages
                .iter()
                .filter(|entry| entry.page.error.is_some())
                .count(),
            missing_templates: pages
                .iter()
                .map(|entry| entry.page.missing_templates.len())
                .sum(),
            missing_symbols: pages
                .iter()
                .map(|entry| entry.page.missing_symbols.len())
                .sum(),
            chain_breaks: pages.iter().map(|entry| entry.page.chain_breaks).sum(),
            other_warnings: pages.iter().map(|entry| entry.page.other_warnings).sum(),
            suspect: ledgers().map(|ledger| ledger.suspect).sum(),
            unattributed: ledgers().map(|ledger| ledger.unattributed).sum(),
            unlisted,
            pages,
        }
    }

    /// The totals, why each failed drawing failed, and what each page lacks,
    /// on stdout.
    fn print(&self) {
        println!(
            "batch drawings {} exported {} failed {} missing_templates {} missing_symbols {} \
             chain_breaks {} other_warnings {} suspect {} unattributed {}",
            self.drawings,
            self.exported,
            self.failed,
            self.missing_templates,
            self.missing_symbols,
            self.chain_breaks,
            self.other_warnings,
            self.suspect,
            self.unattributed
        );
        for unlisted in &self.unlisted {
            println!("unlisted {}: {}", unlisted.directory, unlisted.error);
        }
        for entry in &self.pages {
            let page = &entry.page;
            if let Some(error) = &page.error {
                println!("error {error}");
            }
            let gaps = page.missing_templates.len()
                + page.missing_symbols.len()
                + page.chain_breaks
                + page.other_warnings;
            if gaps > 0 {
                println!(
                    "drawing {} missing_templates {} missing_symbols {} chain_breaks {} \
                     other_warnings {}",
                    entry.path.display(),
                    page.missing_templates.len(),
                    page.missing_symbols.len(),
                    page.chain_breaks,
                    page.other_warnings
                );
            }
            if !page.missing_symbols.is_empty() {
                println!(
                    "missing_symbols {}: {}",
                    entry.path.display(),
                    page.missing_symbols.join(", ")
                );
            }
        }
        // A page whose account does not balance is named last, after every
        // page's own line: it is the reason the batch will exit 1.
        for line in self.unattributed_lines() {
            println!("{line}");
        }
    }

    /// The exit code the batch ends on: `0` only when every page was written,
    /// every directory was listed and every page's account balanced.
    ///
    /// `unattributed` is a page's remainder — what the export dropped with no
    /// reason to cover it — so a batch that leaves one has not exported what
    /// it claims to, whatever its pages look like. It is held to zero here
    /// rather than left to a reader of the totals, and the page that carries
    /// one is named by [`Self::unattributed_lines`].
    fn exit_code(&self) -> u8 {
        if self.failed == 0 && self.unlisted.is_empty() && self.unattributed == 0 {
            0
        } else {
            1
        }
    }

    /// One line for each page whose account does not balance.
    ///
    /// Said in the shape of the other per-drawing lines (`drawing <path> …`)
    /// rather than appended to the `drawing` line of gaps above it: that line
    /// is printed only when a page lacks a template, a symbol or a place, and
    /// a page can carry a remainder with none of those, so an appended field
    /// would go unsaid exactly when it is the only thing wrong.
    fn unattributed_lines(&self) -> Vec<String> {
        self.pages
            .iter()
            .filter_map(|entry| {
                let remainder = entry.page.ledger?.unattributed;
                (remainder > 0)
                    .then(|| format!("drawing {} unattributed {remainder}", entry.path.display()))
            })
            .collect()
    }

    /// Write the account to `path` in `out_directory`, as JSON.
    fn save(&self, out_directory: &Path, path: &Path) -> io::Result<()> {
        let mut json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        json.push(b'\n');
        fs::create_dir_all(out_directory)?;
        fs::write(path, json)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// One drawing whose page leaves a remainder nothing accounts for.
    ///
    /// Built rather than exported: every `.pid` in reach — the seven fixtures
    /// and the acceptance corpus — balances, so a real page that does not is
    /// a ledger bug, and the case that has to hold is what the batch does
    /// when one shows up. The summary is the one a batch of that page builds:
    /// one drawing, one page written, the page's account carrying the
    /// remainder.
    fn a_page_whose_account_does_not_balance() -> BatchSummary {
        BatchSummary {
            directory: "in".to_string(),
            out_directory: "out".to_string(),
            corpus: "corpus".to_string(),
            drawings: 1,
            exported: 1,
            failed: 0,
            missing_templates: 0,
            missing_symbols: 0,
            chain_breaks: 0,
            other_warnings: 0,
            suspect: 0,
            unattributed: 1,
            unlisted: Vec::new(),
            pages: vec![BatchPage {
                path: PathBuf::from("in").join("D06.pid"),
                drawing: "D06.pid".to_string(),
                pdf: Some(PathBuf::from("out").join("D06.pdf").display().to_string()),
                page: Page {
                    ledger: Some(LedgerSummary {
                        entities: 251,
                        on_page: 89,
                        intentional: 285,
                        suspect: 0,
                        unattributed: 1,
                    }),
                    ..Page::default()
                },
            }],
        }
    }

    /// A page whose account does not balance fails the batch and is named.
    ///
    /// `unattributed` is the one number of a page's account that no reason
    /// covers: the export dropped something and did not say what. The file it
    /// wrote is not one to hand on, so the batch exits 1 as it does for a
    /// drawing that could not be read, and the line that names the drawing
    /// says how much is left over. A page that balances says nothing.
    #[test]
    fn a_page_whose_account_does_not_balance_fails_the_batch() {
        let summary = a_page_whose_account_does_not_balance();
        assert_eq!(
            summary.exit_code(),
            1,
            "a batch of a page with a remainder nothing accounts for passed"
        );
        assert_eq!(
            summary.unattributed_lines(),
            [format!(
                "drawing {} unattributed 1",
                PathBuf::from("in").join("D06.pid").display()
            )],
            "the batch did not name the drawing whose account does not balance"
        );
    }

    /// A batch of pages that all balance exits 0 and names none of them.
    #[test]
    fn a_balanced_batch_exits_0_and_names_no_page() {
        let mut summary = a_page_whose_account_does_not_balance();
        summary.unattributed = 0;
        if let Some(page) = summary.pages.first_mut()
            && let Some(ledger) = page.page.ledger.as_mut()
        {
            ledger.unattributed = 0;
            ledger.intentional = 286;
        }
        assert_eq!(summary.exit_code(), 0, "a balanced batch failed");
        assert!(
            summary.unattributed_lines().is_empty(),
            "a balanced batch named a page"
        );
    }
}
