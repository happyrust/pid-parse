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
//! - every page is the paper its sheet states, exactly, and the 53 state
//!   three papers ([`PAPERS`]);
//! - exactly one primitive of a drawing's own reaches past its page — a
//!   sideways run in `N井场` whose origin sits 0.208 mm under the bottom edge,
//!   so its first glyph is clipped there; the writer clips an overhang the way
//!   a printer does, and the count is what keeps the reading from drifting to
//!   two or to none (the rule is [`PaperModel::primitives_past_the_edge`]; why
//!   a text is not judged by its origin is stated at `beyond_the_sheet` in
//!   `model.rs`). Which label it is and where it starts are pinned too
//!   ([`PAST_THE_PAGE`]), so another run of that sheet cannot take its place
//!   unnoticed;
//! - what a border frame holds past the paper, sheet by sheet: on the fifteen
//!   sheets whose frame puts the template off the paper's corner, the
//!   template's sheet outline and — on the nine `CPECCHB-SQ-A2` sheets — the
//!   line that restates its right edge, each with how far past the right and
//!   the top edge it reaches ([`TEMPLATE_PAST_THE_PAPER`]). That ink is in
//!   the model and is not printed;
//! - no label reaches the writer with a line break in it — the writer has no
//!   face for `U+000D`, and one such label took the whole page down;
//! - no placement falls back to a placeholder box: a symbol the `.sym`
//!   libraries lack is still drawn from the body the `.pid` carries itself
//!   (`missing_boxes` is empty on all 53 — the 22 library gaps the sheets
//!   report between them are drawn);
//! - every library gap is one of the known ones below, and no gap grows;
//! - every sheet wearing a border template wears the `.igr` it links at the size
//!   and stroke count that file states, its strokes start where the sheet's
//!   border frame places the template, and each symbol its title block places
//!   sits at the point the `.igr` states — on every sheet wearing it, since a
//!   placement is the template's;
//! - every label that template letters is the same characters, at the same
//!   insertion point and the same body height, cell by cell — 225 cells over
//!   the seven templates, pinned per template and required to agree sheet by
//!   sheet, so a re-authored title block that drifts a cell or respells the
//!   company name fails even though its stroke count, extent and placements
//!   are untouched. The 53 sheets wear five of the seven, so a run holds 175
//!   of the 225 against a page, whichever corpus it reads; the other 50 are
//!   the cells of `A0-W - New.igr` and `A1-W - New.igr`, which no sheet wears
//!   (see `What the 53 sheets exercise` below);
//! - how many sheets wear each pinned template, exactly — zero for the two no
//!   sheet wears — and which three wear none, with the 175 cells and the 64
//!   placements that comes to: a pinned template going unread is then a stated
//!   number rather than a silence, and the first sheet to wear one fails here
//!   until what it is held to has been looked at ([`TEMPLATE_SHEETS`],
//!   [`BARE_SHEETS`]);
//! - which fifteen sheets' frames place the template off the page's origin,
//!   and by how much ([`FRAME_OFFSET_SHEETS`]) — every per-template pin is
//!   taken with that offset removed, so this is the only place it is read;
//! - every one of those 64 placements names a file no library carries and is
//!   drawn from the body the `.igr` caches ([`TEMPLATE_NAME_GAPS`]);
//! - each sheet's template is read from the kind of source its corpus calls
//!   for — disk when a template directory of the corpus holds a file of that
//!   name, a `RefData~*.zip` otherwise — which is all 50 from the archive in
//!   a package and all 50 from disk in a share, held sheet by sheet;
//! - the written page reads back at the size the model resolved — the paper
//!   the sheet states, which every one of the 53 does ([`DERIVED_PAGES`]) —
//!   and weighs what a subset-embedded page weighs: each page is
//!   313–676 KiB (the lightest is `D03`, which has no object of its own and
//!   carries its template alone). A subset used to carry the whole face's
//!   vertical metrics, its signature and its hinting tool's private table,
//!   and each page was 642–1006 KiB; with the bitmap strike as well, before
//!   the `EBDT` fix, 6.6–7.1 MiB;
//! - `D04`'s own `igSymbol2d` placements reach the page: `test/U01/D04.pid`
//!   writes its placement matrix tag's third byte `0x27` rather than the
//!   corpus-wide `0xA7`, and a drawing with 0 decoded `igSymbol2d` and 5
//!   refused `0x00CE` would still lay out and export (the refusal is
//!   invisible to every other pin above) — `d04_igsymbol2d_refused_count_is_zero`
//!   pins the two counts themselves, 5 decoded and 0 refused, rather than any
//!   derived page effect.
//!
//! Soft-skips when the extracted backup is absent, which is the case on CI.
//! It does not soft-skip once the backup is there and no face directory is:
//! the written-page pins cannot be read without a face, and reporting them
//! unread is not the same as reading them — see [`require_faces`].
//!
//! # Two corpora, two variables
//!
//! The drawings are laid out against a corpus — where their templates, their
//! symbols and their faces are read from — and a corpus is one of two kinds:
//!
//! - **A backup package** (`D:\work\plant-code\cad\pid-test-data`, or the copy
//!   at `D:\work\cad\pid-test-data`): the `.igr` templates come out of the
//!   package's own `RefData~4~682.zip`, which also holds `A0-W - New.igr` and
//!   `A1-W - New.igr`.
//! - **A share** (`D:\work\cad\PID自研软件验证`): templates come from
//!   `<share>/Ref/Template Files`, which holds the five `CPE*` files and
//!   neither of those two, and faces from `<share>/fonts`.
//!
//! `PID_PARSE_SELF_CHECK_DIR` names the corpus, and a run that sets it reads
//! that directory, whichever kind it is. A run that leaves it unset reads the
//! package the backup was shelled out of, when the `backup-plant-data` it
//! found sits in one — the directory above it holds `RefData~*.zip` — and the
//! share otherwise ([`corpus_for`]). The first line a run prints names the
//! corpus and says which of the three it was ([`describe_sources`]).
//!
//! A package in reach is the default because the corpus decides what the
//! ledger gate at the end compares. Its baseline,
//! `docs/baselines/export-ledger-backup.json`, names a package's
//! `RefData~*.zip` archives as the sources of its symbols and templates, so
//! only a package run has its 53 pages' counts compared with it, page by page.
//! A share reads both from directories; the gate then reports
//! `count comparison skipped` — which a passing run shows only under
//! `--nocapture` — and passes on the unattributed and cross-check rules alone.
//! That is what the ledger's requirement 13.7 asks for counts taken from
//! another corpus, and it holds for any corpus a run names. A package picked
//! by default is held to more: it was picked so that its pages are compared,
//! so a default run whose symbol or template sources are not the baseline's
//! fails for having compared nothing rather than passing on the skip. Where
//! the default is the package, then, a share is a corpus a run asks for by
//! name:
//!
//! ```text
//! PID_PARSE_SELF_CHECK_DIR=D:\work\cad\PID自研软件验证 \
//!   cargo test -p pid-plot --test export_plant_backup --locked
//! ```
//!
//! Every other pin in this file holds in either corpus.
//!
//! The second variable is for the faces. A backup writes no `fonts/` into its
//! package: `D:\work\plant-code\cad\pid-test-data` keeps one beside its
//! archives, `D:\work\cad\pid-test-data` does not. `PID_PLOT_FONTS` is a
//! `;`-separated list like `PATH`, read before `<corpus>/fonts` and the
//! repository's `test-file/fonts` (see [`pid_plot::fonts::font_dirs`]). A
//! package picked by default is searched for `<share>/fonts` after those, so
//! on a machine that has the share the default takes no variable at all
//! ([`face_dirs`]). A named corpus is searched as named, so a package named
//! without a `fonts/` of its own takes both variables:
//!
//! ```text
//! PID_PARSE_SELF_CHECK_DIR=D:\work\cad\pid-test-data \
//! PID_PLOT_FONTS=D:\work\cad\PID自研软件验证\fonts \
//!   cargo test -p pid-plot --test export_plant_backup --locked
//! ```
//!
//! With no face in reach the gate refuses to pass rather than reporting the
//! written-page pins unread ([`require_faces`]).
//!
//! `PID_TEST_DATA_DIR` is an optional third root: it is one of the places
//! `backup-plant-data` is looked for, beside the named corpus — the share,
//! when none is named — and the two `pid-test-data` paths listed in
//! [`backup_dir`].
//!
//! # What the 53 sheets exercise
//!
//! The template pins do not split by corpus: a share and a package draw the
//! same furniture on the same sheets. 8 sheets wear `CPE-SQ-A0`, 22
//! `CPE-SQ-A1`, 7 `CPE-SQ-A2`, 4 `CPECCHB-SQ-A1` and 9 `CPECCHB-SQ-A2` — 50 in
//! all, the other three bare — and those five templates letter 34, 34, 34, 37
//! and 36 cells, so **175 cells are held against a page in either corpus**.
//!
//! The pin's own total is 225. The remaining 50 cells are the 25 each of
//! `A0-W - New.igr` and `A1-W - New.igr`, and no sheet wears either file — in a
//! package, which carries both, no more than in a share, which carries
//! neither. The pick is why: a sheet that names a template wears only a file
//! whose name begins with the stem its link spells (`template_named_by` in
//! `model.rs`), and a link to the template's drawing (`CPE-SQ~2.PID`) is a
//! name as much as one to its `.igr`. All 50 templated sheets spell `CPE-SQ`
//! or `CPECCH`, which neither `W` name begins with; the three that name
//! nothing state no page either, and a sheet whose page had to be measured
//! wears furniture only if it names some. A `W` file lands only on a sheet
//! that names it or — being the size of its `CPE-SQ` sibling — on one that
//! states an A0 or A1 page and names nothing, and the backup holds neither
//! kind.
//!
//! So the two `W` rows and their label tables stay pinned for a sheet that
//! wears one, and nothing in a run compares them. [`TEMPLATE_SHEETS`] pins
//! that as a number — zero sheets on each — so the day one is worn the gate
//! fails and names the template, and its cells get checked against that page
//! before the count moves.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ledger::ExportLedger;
use pid_plot::ledger::baseline::{GateInput, LedgerCorpus, descriptor_for, run_gate};
use pid_plot::model::{
    PaperModel, PaperPrimitive, PaperShape, STANDARD_SHEETS_MM, SymbolBodySource,
    border_template_places, lay_out_drawing,
};
use pid_plot::read::{LineCap, LineJoin, read_plot, read_plot_pens};
use pid_plot::write::{PlotFont, WIDEST_PEN_MM, write_ledger, write_plot_audited};

/// The reference share this machine keeps the licensed faces in.
fn share_dir() -> PathBuf {
    PathBuf::from(r"D:\work\cad\PID自研软件验证")
}

/// The corpus `PID_PARSE_SELF_CHECK_DIR` names, when the run names one.
fn named_corpus() -> Option<PathBuf> {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map(PathBuf::from)
}

/// Where the shelled-out plant backup sits, if this machine has one.
///
/// The extraction is a local artifact, not a repository fixture, so the gate
/// looks beside the corpus and then in the backup package itself. `None`
/// soft-skips the test, which is what CI sees.
///
/// "Beside the corpus" is beside the one the run names, or beside the share
/// when it names none. The corpus an unnamed run ends up reading is not a root
/// here: [`corpus_for`] picks it from where this function found the backup.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(named_corpus().unwrap_or_else(share_dir));
    if named_corpus().is_none() {
        roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
        roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    }
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

/// Why a run lays the backup out against the corpus it does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CorpusOrigin {
    /// `PID_PARSE_SELF_CHECK_DIR` names it.
    Named,
    /// No corpus is named and the backup sits in a package: the directory
    /// above `backup-plant-data` holds the package's `RefData~*.zip`.
    Package,
    /// No corpus is named and no `RefData~*.zip` sits beside the backup, so
    /// the share is read.
    Share,
}

impl CorpusOrigin {
    /// The reason in the words a run prints.
    fn reason(self) -> &'static str {
        match self {
            Self::Named => "named by PID_PARSE_SELF_CHECK_DIR",
            Self::Package => {
                "the backup package, by default: PID_PARSE_SELF_CHECK_DIR is unset and \
                 backup-plant-data sits in it, beside its RefData~*.zip"
            }
            Self::Share => {
                "the share, by default: PID_PARSE_SELF_CHECK_DIR is unset and no \
                 RefData~*.zip sits beside backup-plant-data"
            }
        }
    }
}

/// The corpus a run lays the backup out against, and why it is that one.
struct Corpus {
    dir: PathBuf,
    origin: CorpusOrigin,
}

/// The corpus the drawings under `backup` are laid out against.
///
/// A run that names one gets it, whichever kind it is. A run that names none
/// gets the package the backup was shelled out of, when `backup` sits in one:
/// the directory above it then holds the package's `RefData~*.zip`, and that
/// is the corpus the ledger baseline was taken from — the only one whose 53
/// pages have their counts compared with it. Left to the share, an unnamed run
/// would lay every page out, skip that comparison and pass, and say so only
/// under `--nocapture`. The share is what an unnamed run reads when the
/// extraction sits anywhere else.
fn corpus_for(backup: &Path) -> Corpus {
    if let Some(dir) = named_corpus() {
        return Corpus {
            dir,
            origin: CorpusOrigin::Named,
        };
    }
    match backup.parent() {
        Some(package) if !pid_parse::refdata_archive::refdata_archives(package).is_empty() => {
            Corpus {
                dir: package.to_path_buf(),
                origin: CorpusOrigin::Package,
            }
        }
        _ => Corpus {
            dir: share_dir(),
            origin: CorpusOrigin::Share,
        },
    }
}

/// The directories the faces are looked for in, in search order.
///
/// [`font_dirs`] lists `PID_PLOT_FONTS`, then `<corpus>/fonts`, then the
/// repository's `test-file/fonts`. A package picked by default is searched for
/// the share's `fonts` after those: a backup writes no faces into its package,
/// so one that keeps none beside its archives would have the default refuse to
/// run until a second variable was set. The share's directory is listed
/// whether or not it is there, so a run that finds no face names it among the
/// places it looked. A named corpus is searched as named, and
/// [`require_faces`] says what to set when that finds nothing.
fn face_dirs(corpus: &Corpus) -> Vec<PathBuf> {
    let mut dirs = font_dirs(Some(&corpus.dir));
    let beside = share_dir().join("fonts");
    if corpus.origin == CorpusOrigin::Package && !dirs.contains(&beside) {
        dirs.push(beside);
    }
    dirs
}

/// One primitive, named so a reader can find it: a label by its text and
/// origin, anything else by its shape.
fn describe(primitive: &PaperPrimitive) -> String {
    match &primitive.shape {
        PaperShape::Text {
            origin_mm, value, ..
        } => format!("text {:?} at {origin_mm:?} mm", value.trim()),
        other => format!("{other:?}"),
    }
}

/// A drawing's path under the backup directory, `/`-separated on every
/// platform: the key the ledger baseline files a page under, and the form
/// [`BARE_SHEETS`] names a sheet in.
fn page_key(path: &Path, backup: &Path) -> String {
    path.strip_prefix(backup)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
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
///
/// `D04`'s four entries joined this list the day its `igSymbol2d` placements
/// stopped being silently refused (the matrix tag's third byte is `0x27` on
/// every one of them, not the corpus-wide `0xA7`): the drawing names its own
/// Windows session's temp-folder path rather than a library-relative one, so
/// the writer that authored it never round-tripped these four bodies through
/// a shared library either.
const KNOWN_LIBRARY_GAPS: [&str; 7] = [
    r"piping\specialty components\misc\pulsation damper.sym",
    // The library carries only the numbered variants of this name, so the
    // report names them rather than picking one: `01` and `02` are different
    // wellheads.
    r"equipment\other equipment\wellhead christmas tree.sym (library carries only Wellhead Christmas Tree 01.sym, Wellhead Christmas Tree 02.sym)",
    r"instrumentation\off-line\without implied components\temperature\clone of tt-temperature transmitter(1).sym",
    r"c:\users\administrator.win-194qrump2nj\appdata\local\temp\isa 2 way generic body.sym",
    r"c:\users\administrator.win-194qrump2nj\appdata\local\temp\isa_logic ctrl.sym",
    r"c:\users\administrator.win-194qrump2nj\appdata\local\temp\concentric size change.sym",
    r"c:\users\admini~1.win\appdata\local\temp\qiufa.sym",
];

/// `PlantData~2~711.zip` holds 53 sheets. A ratchet floor, not an equality: it
/// only refuses an extraction that lost sheets. The exact account of the 53 is
/// [`TEMPLATE_SHEETS`] and [`BARE_SHEETS`] — 50 on a template, 3 on none — so a
/// fuller extraction passes here and is asked to re-measure there.
const BACKUP_SHEETS: usize = 53;

/// Every page is a few hundred KiB once the faces are subset. The bitmap
/// strike that used to ride along pushed each one past 6 MiB.
const PAGE_CEILING: usize = 2 * 1024 * 1024;

/// How far past a page a primitive's ink may reach before this gate counts it,
/// millimetres.
///
/// Ink that sits exactly on the edge is not past it. The reading is finer than
/// a millimetre because the one overhang left is: once a frame's content is
/// placed from its own range origin, `N井场`'s sideways run hangs 0.208 mm under
/// the bottom edge rather than 1.546 mm, and half a millimetre of slack would
/// stop seeing it.
const EDGE_SLACK_MM: f64 = 0.1;

/// Sheets whose `.pid` states no page at all.
///
/// None. Four sheets give the layout no frame rectangle to take a page from,
/// and until the paper was read [`PaperModel::resolved_page_mm`] measured one
/// for them: A0, A1, A1 and A2. Three of the four — `D井场`, `换热器CD` and
/// `凝析油一级闪蒸分离器B气相出口改造` — carry neither an `igSmartFrame2d`
/// nor a template name, and are the bare sheets of [`BARE_SHEETS`]. `Q井场`
/// is not one of those: it has two frame records, one linking the border
/// template it wears (`CPECCHB-SQ-A1`) and one embedding a sheet of eight
/// lines, and a page is taken from the frames only when they state one
/// extent between them. Each of the four states its paper in its `JSheet`
/// record like every other sheet, and it is the sheet they were measured at,
/// so this was 4 and is 0 with no page of theirs changed. Pinned as an
/// equality, both ways round — a new drawing that states neither fails here
/// rather than quietly taking the ink's own box.
const DERIVED_PAGES: usize = 0;

/// The papers the 53 sheets state, as `[width, height]` in millimetres, with
/// how many sheets state each: every page is exactly its sheet's paper.
///
/// Until 2026-11-11 a page was the rectangle of its border frame, which is
/// the paper to a ten-thousandth of a millimetre on 34 sheets and is not on
/// the fifteen of [`FRAME_OFFSET_SHEETS`]: 594.33 × 420.57 mm on the nine
/// `CPECCHB-SQ-A2` sheets, 1189.31 × 841.31 on `A井场`, 841.15 × 594.40 on
/// `O井场` and `P井场`, 842.54 × 595.54 and 843.09 × 596.09 on the two
/// `凝析油稳定塔` sheets, 843.46 × 596.71 on `N井场`. The eleven reference
/// plots are printed on the paper, the one of those fifteen that has a plot
/// included.
const PAPERS: [([f64; 2], usize); 3] = [
    ([594.0, 420.0], 17),
    ([841.0, 594.0], 27),
    ([1189.0, 841.0], 9),
];

/// What each sheet's border frame holds past the paper: the sheet, how many
/// primitives of its template reach past the paper by more than
/// [`EDGE_SLACK_MM`], and how far past its right and its top edge the
/// furthest of them reaches, in millimetres.
///
/// These are the fifteen sheets of [`FRAME_OFFSET_SHEETS`], whose frame puts
/// the template off the paper's corner, and on every one of them what is
/// past the paper is the template's sheet outline — a polyline whose top and
/// right pieces lie off the paper — and nothing else of the template but, on
/// the nine `CPECCHB-SQ-A2` sheets, the line that restates the outline's
/// right edge. The title block and the inner frame are on the paper on all
/// fifteen. None of this ink is printed, as none of it is on the A2
/// reference plot; it was on the page while the page was the frame's
/// rectangle, which is why this pin did not exist before 2026-11-11. That
/// plot shows one template at one offset, so the ledger files the nine
/// sheets it covers and the six it does not under two codes
/// ([`PLOTTED_PAST_THE_PAPER`], [`UNPLOTTED_PAST_THE_PAPER`]).
const TEMPLATE_PAST_THE_PAPER: [(&str, usize, [f64; 2]); 15] = [
    (
        "00/00/A井场 注采阀组工艺及自控流程图.pid",
        1,
        [0.152593, 0.152593],
    ),
    (
        "00/00/N井场 注采阀组工艺及自控流程图.pid",
        1,
        [1.297400, 1.553387],
    ),
    (
        "00/00/O井场 注采阀组工艺及自控流程图.pid",
        1,
        [0.150015, 0.399965],
    ),
    (
        "00/00/P井场 注采阀组工艺及自控流程图.pid",
        1,
        [0.150015, 0.399965],
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图.pid",
        1,
        [1.042526, 1.042526],
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图.pid",
        1,
        [0.768634, 0.768634],
    ),
    (
        "00/02/集注站 低压气压缩机入口改造工艺及自控流程图.pid",
        2,
        [0.327801, 0.565648],
    ),
    (
        "00/02/集注站 凝析油一级闪蒸分离器A气相出口改造工艺及自控流程图.pid",
        2,
        [0.327801, 0.565648],
    ),
    (
        "00/02/集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图.pid",
        2,
        [0.327801, 0.565648],
    ),
    ("test/U01/D01.pid", 2, [0.327801, 0.565648]),
    ("test/U01/D03.pid", 2, [0.327801, 0.565648]),
    ("test/U01/D04.pid", 2, [0.327801, 0.565648]),
    ("test/U01/D06.pid", 2, [0.327801, 0.565648]),
    ("test/U01/D07.pid", 2, [0.327801, 0.565648]),
    ("test/U01/TESTDWG-0002TP03-01.pid", 2, [0.327801, 0.565648]),
];

/// The code the ledger files a template's unprinted segment under where a
/// vendor's plot shows the template placed so.
const PLOTTED_CODE: &str = "segment_beyond_the_paper";

/// The code it files the same segment under where no plot does.
const UNPLOTTED_CODE: &str = "segment_beyond_the_paper_no_plot_of_template";

/// The sheets whose template ink past the paper is left unprinted **as a
/// vendor's plot shows it**: the sheet, the template it wears and how many
/// segments the ledger files under [`PLOTTED_CODE`].
///
/// All fifteen frame-offset sheets of the 53-sheet backup now have a row in
/// `pid_plot::witness::PAPER_EDGE_WITNESSES`:
///
/// - The nine `CPECCHB-SQ-A2` sheets at `(0.388407, 0.565648)` mm, witnessed
///   by the acceptance plot of `轻烃分离器I气出口接口位置改造` (three segments
///   each: the top and right pieces of the outline and the line that restates
///   its right edge).
/// - The six sheets whose factory Amyuni plots (#59) confirm their page is
///   their declared ISO paper and print no outline edge past it (`A井场`,
///   `N井场`, `O井场`, `P井场`, `1200td`, `600td`; two segments each: top and
///   right pieces of the outline).
const PLOTTED_PAST_THE_PAPER: [(&str, &str, u64); 15] = [
    (
        "00/00/A井场 注采阀组工艺及自控流程图.pid",
        "CPE-SQ-A0.igr",
        2,
    ),
    (
        "00/00/N井场 注采阀组工艺及自控流程图.pid",
        "CPECCHB-SQ-A1.igr",
        2,
    ),
    (
        "00/00/O井场 注采阀组工艺及自控流程图.pid",
        "CPECCHB-SQ-A1.igr",
        2,
    ),
    (
        "00/00/P井场 注采阀组工艺及自控流程图.pid",
        "CPECCHB-SQ-A1.igr",
        2,
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图.pid",
        "CPE-SQ-A1.igr",
        2,
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图.pid",
        "CPE-SQ-A1.igr",
        2,
    ),
    (
        "00/02/集注站 低压气压缩机入口改造工艺及自控流程图.pid",
        "CPECCHB-SQ-A2.igr",
        3,
    ),
    (
        "00/02/集注站 凝析油一级闪蒸分离器A气相出口改造工艺及自控流程图.pid",
        "CPECCHB-SQ-A2.igr",
        3,
    ),
    (
        "00/02/集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图.pid",
        "CPECCHB-SQ-A2.igr",
        3,
    ),
    ("test/U01/D01.pid", "CPECCHB-SQ-A2.igr", 3),
    ("test/U01/D03.pid", "CPECCHB-SQ-A2.igr", 3),
    ("test/U01/D04.pid", "CPECCHB-SQ-A2.igr", 3),
    ("test/U01/D06.pid", "CPECCHB-SQ-A2.igr", 3),
    ("test/U01/D07.pid", "CPECCHB-SQ-A2.igr", 3),
    ("test/U01/TESTDWG-0002TP03-01.pid", "CPECCHB-SQ-A2.igr", 3),
];

/// The sheets whose template ink past the paper is left unprinted **with no
/// plot of that template placed so**: empty once all fifteen frame-offset
/// sheets are witnessed (#59).
const UNPLOTTED_PAST_THE_PAPER: [(&str, &str, u64); 0] = [];

/// How the 53 pages end their strokes, read back from the PDFs: the inked
/// paths stroked butt-ended and mitred, and the ones stroked round-ended and
/// round-joined. No page strokes a path any other way.
///
/// A page states no cap until its first stroke on a pen wider than one
/// device unit and round ends and joins from there on (`write.rs`,
/// `starts_round_ends`), so the butt-ended paths are what each sheet paints
/// ahead of that pen: its border template's one-unit rules where the frame
/// is listed first, and whatever of the drawing's own the sheet lists ahead
/// of its first wide stroke.
const STROKE_ENDS: (usize, usize) = (4374, 79_651);

/// The sheet whose own thin strokes stand ahead of its first wide pen, and
/// how its page ends its strokes: butt-ended paths, round-ended paths.
///
/// `凝析油一级闪蒸分离器B气相出口改造` wears no template, and its sheet lists
/// thin strokes of the drawing's own ahead of the first wide one. They are
/// stroked butt-ended, as a border's rules are on the reference plots; no
/// plot shows a drawing's own thin stroke ahead of the first wide pen, so
/// for this page the rule is carried over, not read. It is pinned because
/// it is the one page on hand that tells that rule from the one before it —
/// round ends from the drawing's first stroke on any pen — under which
/// these paths are round.
const THIN_AHEAD_OF_THE_WIDE_PEN: (&str, usize, usize) =
    ("凝析油一级闪蒸分离器B气相出口改造", 47, 430);

/// Runs the page cuts because the condensation floor cannot hold them.
///
/// A label the sheet cannot hold is condensed along its own direction until it
/// fits; a run that would have to be squeezed below half width keeps its size
/// and loses its tail to the page clip. That used to happen with nothing said
/// about it, so a page whose labels all fitted and a page whose last glyphs
/// went over the edge read the same. Pinned as an equality both ways round: a
/// new clipped run is a defect to explain, and losing this one means the floor
/// stopped being reached.
const CLIPPED_AT_CONDENSATION_FLOOR: usize = 0;

/// Runs condensed, rather than clipped, to stay on the page.
///
/// A label wider than the room left on its sheet is squeezed along its own
/// direction until it fits, which keeps every glyph but draws the run narrower
/// than the drawing set it. Nothing said so before, so a page that never
/// squeezed a run and one that squeezed one to 0.7 read the same. Pinned as an
/// equality both ways round.
const CONDENSED_RUNS: usize = 0;

/// Sheets that carry a border template. A floor for the same reason
/// [`BACKUP_SHEETS`] is one: a fuller extraction only adds sheets — the 16 that
/// sat on none until the backup's own `RefData~4~682.zip`, whose templates the
/// corpus now carries, gave them their frames.
///
/// The last two are `1200td` and `N井场`: both name their template in a `.PID`
/// link and state a page 2.09 / 2.46–2.71 mm larger than it, so the pick keeps
/// them on the name's 10 mm window rather than the 2 mm an unnamed sheet gets.
///
/// The 50th is `Q井场`: it states no page at all, so it reaches the pick through
/// neither branch above — it names `CPECCH~3.IGR` and gets the sheet its own
/// inked extent lands on, which is CCHB's A1. `换热器CD` states no page either
/// and names nothing, so it stays bare, as its reference plot is.
///
/// The floor is what a corpus with no template in reach fails on, all 50 at
/// once. Which template each of the 50 wears is pinned exactly, template by
/// template, in [`TEMPLATE_SHEETS`], whose counts add up to this number.
const TEMPLATED_SHEETS: usize = 50;

/// One symbol a pinned template places, as the `.igr` states it: the file name
/// the placement stores and the point it sits at, in paper millimetres.
type PinnedPlacement = (&'static str, [f64; 2]);

/// One label a pinned template letters, as the `.igr` states it: the
/// characters, the insertion point in paper millimetres, and the body height in
/// millimetres.
///
/// `f64::NAN` stands for a record that states no height; none of the seven
/// templates here does, and a `NAN` matches only another `NAN`.
type PinnedLabel = (&'static str, [f64; 2], f64);

/// One pinned template: the file name, the extent and stroke count its `.igr`
/// states, what that `.igr` places, and what it letters.
type PinnedTemplate = (
    &'static str,
    [f64; 2],
    usize,
    &'static [PinnedPlacement],
    &'static [PinnedLabel],
);

/// The 34 cells `CPE-SQ-A0.igr` letters, sorted by text then position.
///
/// Twenty of them run down the left edge's sign-off column and fourteen sit
/// in the title block. The `CPE-SQ` family is one template re-authored at
/// three sizes, and the two blocks hang from different edges, which is what
/// the three lists show: the column's `x` is 24 mm from the left edge on all
/// three of them and its `y` counts down from the *top* edge, so A0's column
/// sits 247.000 mm below A1's (the difference between the two page heights)
/// and A1's 174.000 mm below A2's; the title block's `y` is the same on all
/// three — it counts up from the *bottom* edge — and its `x` counts in from
/// the right, so A0's title block is 348.000 mm right of A1's (the difference
/// between the two page widths) and A1's 247.000 mm right of A2's.
static CPE_SQ_A0_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [1137.931554, 66.349430], 3.500000),
    ("1 版", [1172.607467, 12.694865], 3.000000),
    ("2012,05,15", [1100.447381, 12.813829], 3.200000),
    ("A111001641", [1091.960694, 66.560638], 3.500000),
    ("CADD号", [1120.587282, 25.124944], 3.200000),
    ("CPENC-DD11008", [1144.435787, 12.694865], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [1063.413661, 74.145144],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 693.500016], 3.000000),
    ("会 签 专 业", [23.999970, 768.500016], 3.000000),
    ("修改", [24.225908, 643.746055], 3.000000),
    ("修改号", [24.225908, 598.289181], 3.000000),
    ("制  图", [1046.499970, 61.194323], 3.200000),
    ("勘察证书号", [1136.391170, 66.014058], 3.500000),
    ("华北油田苏桥储气库群", [1125.246205, 57.730800], 4.000000),
    ("审  定", [1046.499970, 37.143917], 3.200000),
    ("审  核", [1046.499970, 43.160719], 3.200000),
    ("审核", [24.225908, 658.773122], 3.000000),
    ("批  准", [1046.499970, 31.143917], 3.200000),
    ("文件号", [1120.587282, 19.311584], 3.200000),
    ("施工图", [1099.732930, 24.762086], 3.200000),
    ("日    期", [23.999970, 743.500016], 3.000000),
    ("日    期", [23.999970, 818.500016], 3.000000),
    ("日期", [1081.127194, 13.006008], 3.200000),
    ("日期", [24.225908, 628.906828], 3.000000),
    ("校  对", [1046.499970, 48.923321], 3.200000),
    ("次数", [24.225908, 585.891852], 3.000000),
    ("比例", [1077.328358, 17.550016], 3.200000),
    ("签     署", [23.999970, 718.500016], 3.000000),
    ("签     署", [23.999970, 793.500016], 3.000000),
    ("设  计", [1046.499970, 55.194323], 3.200000),
    ("设计证书号", [1090.531612, 66.240398], 3.500000),
    ("阶段", [1077.279206, 23.542724], 3.200000),
    ("页次", [24.225908, 613.316247], 3.000000),
    ("项目号", [1120.587282, 13.316557], 3.200000),
];

/// The same 34 cells as [`CPE_SQ_A0_LABELS`], on A1's page.
static CPE_SQ_A1_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [789.931554, 66.349412], 3.500000),
    ("1 版", [824.607467, 12.694846], 3.000000),
    ("2012,05,15", [752.447381, 12.813810], 3.200000),
    ("A111001641", [743.960694, 66.560620], 3.500000),
    ("CADD号", [772.587282, 25.124926], 3.200000),
    ("CPENC-DD11008", [796.435787, 12.694846], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [715.413661, 74.145125],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 446.499998], 3.000000),
    ("会 签 专 业", [23.999970, 521.499998], 3.000000),
    ("修改", [24.225908, 396.746037], 3.000000),
    ("修改号", [24.225908, 351.289163], 3.000000),
    ("制  图", [698.499970, 61.194305], 3.200000),
    ("勘察证书号", [788.391170, 66.014040], 3.500000),
    ("华北油田苏桥储气库群", [777.246205, 57.730782], 4.000000),
    ("审  定", [698.499970, 37.143899], 3.200000),
    ("审  核", [698.499970, 43.160701], 3.200000),
    ("审核", [24.225908, 411.773103], 3.000000),
    ("批  准", [698.499970, 31.143899], 3.200000),
    ("文件号", [772.587282, 19.311566], 3.200000),
    ("施工图", [751.732930, 24.762068], 3.200000),
    ("日    期", [23.999970, 496.499998], 3.000000),
    ("日    期", [23.999970, 571.499998], 3.000000),
    ("日期", [733.127194, 13.005989], 3.200000),
    ("日期", [24.225908, 381.906810], 3.000000),
    ("校  对", [698.499970, 48.923303], 3.200000),
    ("次数", [24.225908, 338.891833], 3.000000),
    ("比例", [729.328358, 17.549998], 3.200000),
    ("签     署", [23.999970, 471.499998], 3.000000),
    ("签     署", [23.999970, 546.499998], 3.000000),
    ("设  计", [698.499970, 55.194305], 3.200000),
    ("设计证书号", [742.531612, 66.240380], 3.500000),
    ("阶段", [729.279206, 23.542706], 3.200000),
    ("页次", [24.225908, 366.316229], 3.000000),
    ("项目号", [772.587282, 13.316539], 3.200000),
];

/// The same 34 cells as [`CPE_SQ_A0_LABELS`], on A2's page.
static CPE_SQ_A2_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [542.931554, 66.349408], 3.500000),
    ("1 版", [577.607467, 12.694843], 3.000000),
    ("2012,05,15", [505.447381, 12.813807], 3.200000),
    ("A111001641", [496.960694, 66.560616], 3.500000),
    ("CADD号", [525.587282, 25.124922], 3.200000),
    ("CPENC-DD11008", [549.435787, 12.694843], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [468.413661, 74.145122],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 272.499995], 3.000000),
    ("会 签 专 业", [23.999970, 347.499995], 3.000000),
    ("修改", [24.225908, 222.746034], 3.000000),
    ("修改号", [24.225908, 177.289159], 3.000000),
    ("制  图", [451.499970, 61.194301], 3.200000),
    ("勘察证书号", [541.391170, 66.014037], 3.500000),
    ("华北油田苏桥储气库群", [530.246205, 57.730778], 4.000000),
    ("审  定", [451.499970, 37.143895], 3.200000),
    ("审  核", [451.499970, 43.160697], 3.200000),
    ("审核", [24.225908, 237.773100], 3.000000),
    ("批  准", [451.499970, 31.143895], 3.200000),
    ("文件号", [525.587282, 19.311562], 3.200000),
    ("施工图", [504.732930, 24.762064], 3.200000),
    ("日    期", [23.999970, 322.499995], 3.000000),
    ("日    期", [23.999970, 397.499995], 3.000000),
    ("日期", [486.127194, 13.005986], 3.200000),
    ("日期", [24.225908, 207.906806], 3.000000),
    ("校  对", [451.499970, 48.923299], 3.200000),
    ("次数", [24.225908, 164.891830], 3.000000),
    ("比例", [482.328358, 17.549995], 3.200000),
    ("签     署", [23.999970, 297.499995], 3.000000),
    ("签     署", [23.999970, 372.499995], 3.000000),
    ("设  计", [451.499970, 55.194301], 3.200000),
    ("设计证书号", [495.531612, 66.240376], 3.500000),
    ("阶段", [482.279206, 23.542702], 3.200000),
    ("页次", [24.225908, 192.316225], 3.000000),
    ("项目号", [525.587282, 13.316535], 3.200000),
];

/// The 37 cells `CPECCHB-SQ-A1.igr` letters.
///
/// Different furniture from the `CPE-SQ` family: its title block (`CPECC`, the
/// company name split across two cells, `项目经理`, an unspaced `专业`) and its
/// own set of certificate rows.
///
/// Ten of the 37 repeat a string the file letters twice, a hair apart: `专业`,
/// `比例`, `版次`, `阶段`, `项目号` twice each and `制图`, `审定`, `审核`,
/// `校对`, `设计` three times each, one copy at 2.499745 mm and one at
/// 2.500000 mm with its insertion point a few thousandths of a millimetre
/// away. That is the template, not the reader: the `.igr` carries the cell
/// twice. Pinned as measured — a change that stops double-lettering is worth a
/// look, and this is the only pin that would say so.
///
/// It also letters `签     暑` where the draughtsman meant `署`, which is the
/// kind of thing a text pin is for: the characters are the file's, and a
/// "helpful" correction anywhere in the reader would fail here.
static CPECCHB_SQ_A1_LABELS: [PinnedLabel; 37] = [
    ("CADD号", [698.513504, 5.647319], 2.499745),
    ("CPECC", [698.592631, 66.663703], 2.244375),
    ("专业", [770.298519, 13.179031], 2.499745),
    ("专业", [770.292325, 13.179355], 2.500000),
    (
        "中国石油工程建设有限公司",
        [753.954102, 76.435606],
        5.500000,
    ),
    ("仪  控", [668.757500, 59.296062], 2.499745),
    ("会 签 专 业", [23.589985, 521.499994], 2.500000),
    ("制图", [660.444788, 53.296675], 2.499745),
    ("制图", [695.441217, 59.296062], 2.499745),
    ("制图", [695.427384, 59.301093], 2.500000),
    ("华北分公司", [808.031586, 76.435606], 5.000000),
    ("审定", [660.666459, 29.997946], 2.499745),
    ("审定", [695.662887, 35.997334], 2.499745),
    ("审定", [695.649078, 35.999987], 2.500000),
    ("审核", [660.463523, 36.067340], 2.499745),
    ("审核", [695.459952, 42.066728], 2.499745),
    ("审核", [695.446121, 42.070000], 2.500000),
    (
        "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
        [731.990697, 66.885704],
        2.000000,
    ),
    ("文件号", [770.727377, 19.399462], 2.499745),
    ("日    期", [23.595003, 571.557849], 2.500000),
    ("日期", [733.656859, 13.385600], 2.499745),
    ("校对", [660.463523, 42.077323], 2.499745),
    ("校对", [695.459952, 48.076710], 2.499745),
    ("校对", [695.446121, 48.080596], 2.500000),
    ("核准", [695.366657, 29.628482], 2.499745),
    ("比例", [733.526260, 19.062941], 2.499745),
    ("比例", [733.516314, 19.063866], 2.500000),
    ("版次", [812.184351, 13.356468], 2.499745),
    ("版次", [812.182432, 13.356810], 2.500000),
    ("签     暑", [23.599454, 546.455491], 2.500000),
    ("设计", [660.456644, 47.966667], 2.499745),
    ("设计", [695.453073, 53.966055], 2.499745),
    ("设计", [695.439241, 53.970541], 2.500000),
    ("阶段", [733.436817, 24.609254], 2.499745),
    ("阶段", [733.426862, 24.610744], 2.500000),
    ("项目号", [770.800244, 24.791983], 2.499745),
    ("项目号", [770.794102, 24.793492], 2.500000),
];

/// The 36 cells `CPECCHB-SQ-A2.igr` letters.
///
/// The same furniture as [`CPECCHB_SQ_A1_LABELS`] at 1.414× the size — which
/// is A2's 420 mm against this template's own 593.939 mm extent, so its heights
/// are 3.535354 mm where the A1 file states 2.500000 — and with one more cell,
/// `修改号`, in the sign-off column.
///
/// Its company-name cell is `中国石油工程建设有限公司 ` with a trailing space,
/// which the A1 file's same cell does not carry; both are pinned as the files
/// spell them.
///
/// `CPECC` follows the same ratio — 3.173864 mm against the A1 file's
/// 2.244375 — because that is the height its run style states and the A2
/// reference plot letters it at (`Tf 12.96`); the record scales its own
/// direction pair to 1 mm, which no plot uses.
static CPECCHB_SQ_A2_LABELS: [PinnedLabel; 36] = [
    ("CADD号", [399.494979, 3.731490], 3.535354),
    ("CPECC", [398.236067, 91.605293], 3.173864),
    ("专业", [501.019500, 11.566782], 3.535354),
    (
        "中国石油工程建设有限公司 ",
        [479.407575, 100.801501],
        7.777778,
    ),
    ("仪  控", [357.411488, 76.789441], 3.535354),
    ("会 签 专 业", [33.939423, 218.484857], 4.242424),
    ("会 签 专 业", [33.939423, 324.545463], 4.242424),
    ("修改", [34.258931, 148.125721], 3.535354),
    ("修改号", [34.258931, 83.843272], 3.535354),
    ("制图", [345.654938, 68.304593], 3.535354),
    ("制图", [395.149887, 76.789441], 3.535354),
    ("华北分公司", [559.032975, 100.801501], 7.777778),
    ("审定", [345.968443, 35.353534], 3.535354),
    ("审定", [395.463393, 43.838382], 3.535354),
    ("审核", [345.681434, 43.937391], 3.535354),
    ("审核", [395.176384, 52.422240], 3.535354),
    ("审核", [34.258931, 169.376117], 3.535354),
    (
        "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
        [446.855582, 87.515155],
        2.828283,
    ),
    ("文件号", [501.626028, 20.364249], 3.535354),
    ("日    期", [33.939423, 289.191928], 4.242424),
    ("日    期", [33.939423, 395.252534], 4.242424),
    ("日期", [449.197725, 11.858930], 3.535354),
    ("日期", [34.258931, 127.140954], 3.535354),
    ("校对", [345.681434, 52.437223], 3.535354),
    ("校对", [395.176384, 60.922072], 3.535354),
    ("核准", [395.044438, 34.831005], 3.535354),
    ("次数", [34.258931, 66.311695], 3.535354),
    ("比例", [449.013020, 19.888312], 3.535354),
    ("版次", [561.828118, 11.817728], 3.535354),
    ("签     署", [33.939423, 253.838393], 4.242424),
    ("签     署", [33.939423, 359.898999], 4.242424),
    ("设计", [345.671705, 60.766438], 3.535354),
    ("设计", [395.166655, 69.251287], 3.535354),
    ("阶段", [448.886522, 27.732383], 3.535354),
    ("页次", [34.258931, 105.093668], 3.535354),
    ("项目号", [501.729083, 27.990814], 3.535354),
];

/// The 25 cells `A0-W - New.igr` letters.
///
/// A third furniture family: its title block letters `项目经理` and a
/// wide-spaced `审  定`, and its sign-off column letters a bare `签署` where
/// the `CPE-SQ` family letters `签     署`. Both W files letter the same 25
/// strings at their own page size, and both place no symbol at all
/// (`TEMPLATE_PLACEMENTS` below).
///
/// Only a backup package's `RefData~4~682.zip` carries the two files, and none
/// of the 53 sheets wears either one — [`TEMPLATE_SHEETS`] pins both at zero
/// and says why — so no run compares these cells or [`A1_W_LABELS`]' with a
/// page. They are what the first sheet to wear a `W` file is held to.
static A0_W_LABELS: [PinnedLabel; 25] = [
    ("专业", [20.213572, 674.384346], 3.000000),
    ("专业", [20.187827, 751.052526], 3.000000),
    ("修改", [1098.721629, 88.694982], 2.500000),
    ("审  定", [1041.215048, 44.659907], 2.500000),
    ("审  核", [1041.215048, 50.659907], 2.500000),
    ("审定", [1143.833155, 88.901014], 2.500000),
    ("审核", [1128.948888, 88.922241], 2.500000),
    ("文件号", [1076.215048, 20.659907], 2.500000),
    ("日  期", [1076.215048, 14.659907], 2.500000),
    ("日期", [1162.780233, 88.972189], 2.500000),
    ("日期", [20.110593, 724.638304], 3.000000),
    ("日期", [20.316552, 801.435210], 3.000000),
    ("校  对", [1041.215048, 56.659907], 2.500000),
    ("校对", [1113.380741, 88.808612], 2.500000),
    ("核  准", [1041.391838, 32.844114], 2.500000),
    ("比  例", [1143.967653, 14.731583], 2.500000),
    ("版  次", [1113.215048, 14.659907], 2.500000),
    ("版次", [1039.976561, 88.694982], 2.500000),
    ("签署", [20.290807, 700.798568], 3.000000),
    ("签署", [20.239317, 775.020987], 3.000000),
    ("设  计", [1041.215048, 62.659907], 2.500000),
    ("说明", [1067.899371, 88.808889], 2.500000),
    ("阶  段", [1076.215048, 26.659907], 2.500000),
    ("项目号", [1113.215048, 26.659907], 2.500000),
    ("项目经理", [1039.217319, 38.978732], 2.500000),
];

/// The same 25 cells as [`A0_W_LABELS`], on A1's page.
static A1_W_LABELS: [PinnedLabel; 25] = [
    ("专业", [20.213572, 427.384346], 3.000000),
    ("专业", [20.187827, 504.052526], 3.000000),
    ("修改", [750.153186, 89.177180], 3.000000),
    ("审  定", [691.958746, 44.984658], 3.000000),
    ("审  核", [691.850496, 50.984658], 3.000000),
    ("审定", [795.179527, 89.310615], 3.000000),
    ("审核", [780.407980, 89.384844], 3.000000),
    ("文件号", [726.917851, 21.143769], 3.000000),
    ("日  期", [726.697914, 15.143769], 3.000000),
    ("日期", [814.521916, 89.245664], 3.000000),
    ("日期", [20.110593, 477.638304], 3.000000),
    ("日期", [20.316552, 554.435210], 3.000000),
    ("校  对", [691.958746, 57.092909], 3.000000),
    ("校对", [765.315194, 89.292058], 3.000000),
    ("核  准", [691.735219, 32.963129], 3.000000),
    ("比  例", [794.978368, 15.193452], 3.000000),
    ("版  次", [764.049814, 15.231744], 3.000000),
    ("版次", [691.675367, 89.177180], 3.000000),
    ("签署", [20.290807, 453.798568], 3.000000),
    ("签署", [20.239317, 528.020987], 3.000000),
    ("设  计", [691.843854, 62.868810], 3.000000),
    ("说明", [719.314151, 89.210594], 3.000000),
    ("阶  段", [726.961839, 27.143769], 3.000000),
    ("项目号", [764.049814, 27.187757], 3.000000),
    ("项目经理", [691.628155, 39.059549], 3.000000),
];

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
/// Two of the seven, `A0-W - New.igr` and `A1-W - New.igr`, are in no share —
/// only a backup package's `RefData~4~682.zip` carries them — and no sheet
/// wears either, whichever corpus the gate is run against. They have the
/// extent of their `CPE-SQ` siblings, so between the two the pick's last key —
/// the file name — would decide for a sheet that states that page and names no
/// template; every sheet here that states a page names `CPE-SQ` or `CPECCH`,
/// and a named sheet wears only a file of that name. Neither title block
/// places a symbol. The two rows are what a sheet wearing a `W` file would be
/// held to, and [`TEMPLATE_SHEETS`] pins that there is none, so they cannot
/// start being read unnoticed.
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
/// 593.939 mm wide on a 594.3 mm page, and `CPE-SQ-A1.igr` is 841 × 594 on the
/// `稳定塔 (600td)` sheet's 842.54 × 595.54 page — both inside the pick's
/// tolerance, so pinning the sheet here instead would be pinning the wrong
/// number.
///
/// The fourth number is where each placement sits, in paper millimetres, paired
/// with the symbol's file name and sorted by it. It belongs to the template the
/// same way its extent does: the `.igr` states its placements, the sheet only
/// says which `.igr` it wears, so every one of the twenty-two A1 sheets that
/// wear `CPE-SQ-A1` carries its 比例 wave at the same point. A title block whose
/// cell moved — a re-authored template, a nibble read one field off — shows up
/// here and nowhere else, because the placement list alone still matches.
///
/// The fifth is the template's own lettering: every label it puts on the page,
/// as `(characters, insertion point, body height)`, sorted by text then
/// position, one `static` per template above. This is what the stroke count
/// above cannot say — a title block re-authored with the same number of strokes
/// in the same box, spelling the company name differently or sliding one cell
/// two millimetres, passes extent, strokes and placements alike. Twenty-five to
/// thirty-seven cells per template, 225 in all, and the count is pinned by each
/// list's own length. A template that gains or loses a cell therefore fails
/// here, whatever the drawing it lands on.
const TEMPLATE_PLACEMENTS: [PinnedTemplate; 7] = [
    (
        "CPE-SQ-A0.igr",
        [1189.0, 841.0],
        175,
        &[("blfh.sym", [1100.6925, 19.0])],
        &CPE_SQ_A0_LABELS,
    ),
    (
        "CPE-SQ-A1.igr",
        [841.0, 594.0],
        175,
        &[("blfh.sym", [752.6925, 19.0])],
        &CPE_SQ_A1_LABELS,
    ),
    (
        "CPE-SQ-A2.igr",
        [594.0, 420.0],
        175,
        &[("blfh.sym", [505.6925, 19.0])],
        &CPE_SQ_A2_LABELS,
    ),
    (
        "CPECCHB-SQ-A1.igr",
        [841.0, 594.0],
        96,
        &[],
        &CPECCHB_SQ_A1_LABELS,
    ),
    (
        "CPECCHB-SQ-A2.igr",
        [593.939, 420.0],
        116,
        &[
            ("a$c40d91a4b.sym", [0.0, 59.0404]),
            ("hqz.sym", [35.3536, 412.9293]),
            ("hqz.sym", [35.3536, 306.8687]),
        ],
        &CPECCHB_SQ_A2_LABELS,
    ),
    ("A0-W - New.igr", [1189.0, 841.0], 105, &[], &A0_W_LABELS),
    ("A1-W - New.igr", [841.0, 594.0], 101, &[], &A1_W_LABELS),
];

/// How many of the backup's sheets wear each pinned template, row for row with
/// [`TEMPLATE_PLACEMENTS`].
///
/// Unlike that table's numbers these are the extraction's, not the template's,
/// and each is an equality: `8 + 22 + 7 + 4 + 9` is the 50 of
/// [`TEMPLATED_SHEETS`], and with the three [`BARE_SHEETS`] that is every one
/// of the 53. The counts are the same whether the templates are read from a
/// share's `Ref/Template Files` or out of a package's `RefData~4~682.zip`.
///
/// The two zeroes are why the table is written out. `A0-W - New.igr` and
/// `A1-W - New.igr` are pinned above, 25 label cells each, and no sheet wears
/// either: a sheet that names a template wears only a file whose name begins
/// with the stem its link spells — a `.PID` link to the template's drawing
/// counts as much as one to its `.igr` — and every templated sheet here spells
/// `CPE-SQ` or `CPECCH`; the three sheets that name nothing state no page, so
/// nothing picks furniture for them by size. A pinned template no sheet wears
/// is a pin no run reads, and without a number here that would look the same
/// as one that is read and agrees. With it, the first sheet to wear a `W` file
/// fails the gate by the template's name, and the cells it is held to get
/// looked at before the count moves; so does a sheet that changes template or
/// loses its furniture.
const TEMPLATE_SHEETS: [(&str, usize); 7] = [
    ("CPE-SQ-A0.igr", 8),
    ("CPE-SQ-A1.igr", 22),
    ("CPE-SQ-A2.igr", 7),
    ("CPECCHB-SQ-A1.igr", 4),
    ("CPECCHB-SQ-A2.igr", 9),
    ("A0-W - New.igr", 0),
    ("A1-W - New.igr", 0),
];

/// The sheets that wear no template, by their path under the backup directory.
///
/// All three state no page and link no template, so the pick has neither a
/// size nor a name to go on and the page stays bare. `换热器CD`'s reference
/// plot is bare, and so is the factory plot of `D井场`. The factory plot of
/// `凝析油一级闪蒸分离器B气相出口改造` letters the `CPECCHB-SQ-A2` title
/// block, and the `.pid` still names no template file, so the page stays bare
/// with the other two (`factory_amyuni_plots`). Listed rather than counted:
/// a sheet that loses its furniture while one of these gains some leaves the
/// count at three.
const BARE_SHEETS: [&str; 3] = [
    "00/00/D井场 注采阀组工艺及自控流程图.pid",
    "00/00/集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
    "00/02/集注站 凝析油一级闪蒸分离器B气相出口改造工艺及自控流程图.pid",
];

/// Label cells a run holds against a page: the cells of each template some
/// sheet wears, counted once per template.
///
/// `34 × 3 + 37 + 36`, the five worn templates' lists. The seven pinned lists
/// hold 225; the 50 between the two numbers are the two `W` templates' cells,
/// which [`TEMPLATE_SHEETS`] pins as worn by no sheet.
const COMPARED_LABEL_CELLS: usize = 175;

/// Symbols the templates place, summed over every sheet wearing one.
///
/// One `BLFH.sym` on each of the 37 `CPE-SQ` sheets, none on the four
/// `CPECCHB-SQ-A1` sheets and three on each of the nine `CPECCHB-SQ-A2`
/// sheets: `37 + 0 + 27`.
const TEMPLATE_SYMBOL_PLACEMENTS: usize = 64;

/// Template placements whose stored name no symbol library answers, summed
/// over every sheet wearing a template: every one of the
/// [`TEMPLATE_SYMBOL_PLACEMENTS`].
///
/// A title block's `BLFH.sym`, `HQZ.sym` and `A$C40D91A4B.sym` are stored
/// under absolute paths on the authoring machine, which no library is filed
/// by, so each is drawn from the body the `.igr` caches and its ledger entry
/// carries a `name_gap`. The number is the same in a package and in a share:
/// neither library has a file under those paths. A placement a library starts
/// answering leaves this count without touching the placement total, and the
/// failure names the sheet and the symbol.
const TEMPLATE_NAME_GAPS: usize = 64;

/// The sheets whose border frame places the template off the page's origin,
/// by their path under the backup directory, with that offset in millimetres
/// (`BorderTemplate::offset_mm`).
///
/// A frame lands its template at `s·p + t − lo` (`FramePlacement` in
/// `model.rs`), and on 35 of the 50 templated sheets `t − lo` is zero to the
/// micrometre. On these fifteen it is not, so the sign-off column and the
/// title block sit that far from where the template's own coordinates put
/// them: all nine `CPECCHB-SQ-A2` sheets at (0.388, 0.566) mm — which is
/// where the reference plot of `轻烃分离器I` inks its furniture — the three
/// `CPECCHB-SQ-A1` sheets that state a page (`N井场`, `O井场`, `P井场`), `A井场`
/// on `CPE-SQ-A0`, and the two `凝析油稳定塔` sheets on `CPE-SQ-A1`.
///
/// Every per-template pin above is taken with the offset removed, so nothing
/// else in this file notices a sheet whose offset changed or went away.
/// Listed with the value rather than counted: a sheet that gains an offset
/// while another loses one leaves the count at fifteen.
const FRAME_OFFSET_SHEETS: [(&str, [f64; 2]); 15] = [
    (
        "00/00/A井场 注采阀组工艺及自控流程图.pid",
        [0.152593, 0.152593],
    ),
    (
        "00/00/N井场 注采阀组工艺及自控流程图.pid",
        [1.297400, 1.553387],
    ),
    (
        "00/00/O井场 注采阀组工艺及自控流程图.pid",
        [0.150015, 0.399965],
    ),
    (
        "00/00/P井场 注采阀组工艺及自控流程图.pid",
        [0.150015, 0.399965],
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(1200td)工艺及自控流程图.pid",
        [1.042526, 1.042526],
    ),
    (
        "00/00/集注站 凝析油稳定塔、泵及换热器(600td)工艺及自控流程图.pid",
        [0.768634, 0.768634],
    ),
    (
        "00/02/集注站 低压气压缩机入口改造工艺及自控流程图.pid",
        [0.388407, 0.565648],
    ),
    (
        "00/02/集注站 凝析油一级闪蒸分离器A气相出口改造工艺及自控流程图.pid",
        [0.388407, 0.565648],
    ),
    (
        "00/02/集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图.pid",
        [0.388407, 0.565648],
    ),
    ("test/U01/D01.pid", [0.388407, 0.565648]),
    ("test/U01/D03.pid", [0.388407, 0.565648]),
    ("test/U01/D04.pid", [0.388407, 0.565648]),
    ("test/U01/D06.pid", [0.388407, 0.565648]),
    ("test/U01/D07.pid", [0.388407, 0.565648]),
    ("test/U01/TESTDWG-0002TP03-01.pid", [0.388407, 0.565648]),
];

/// The smallest frame offset, on either axis, that counts as one, in
/// millimetres. A frame that states zero reads back as zero, so anything a
/// micrometre or more from it is a stated offset.
const FRAME_OFFSET_FLOOR_MM: f64 = 0.001;

/// The one primitive of a drawing's own that reaches past its page — a
/// label: the sheet it is on, its characters and its insertion point in
/// paper millimetres.
///
/// `N井场`'s sideways pipeline tag starts 0.208 mm under the bottom edge, so
/// its first glyph is clipped there and the other nineteen are on the sheet.
/// The factory plot clips it the same way: the first glyph's origin is
/// (262.128, −1.524) mm, the insertion the `.pid` states on the 150 dpi grid,
/// and that glyph hangs under the edge. The run stays where the frame places
/// the insertion; it is not moved up onto the page. The count alone would be
/// satisfied by any one primitive on that sheet. What a border template holds
/// past the paper is pinned apart, in [`TEMPLATE_PAST_THE_PAPER`].
const PAST_THE_PAGE: (&str, &str, [f64; 2]) =
    ("N井场", "15-TD-4101416-B2-EH40", [259.113810, -0.207788]);

/// How far a pinned placement point may drift before the gate speaks, in
/// millimetres.
///
/// A ten-thousandth of a millimetre: a micrometre is two orders below what a
/// plotter resolves and orders below the float noise in a metre-to-millimetre
/// conversion, so it holds the point exactly while still tolerating the last
/// bits of `insertion.x * 1000.0`.
const PLACEMENT_SLACK_MM: f64 = 0.0001;

/// How far a pinned extent may drift before the gate speaks, in millimetres.
///
/// Two hundredths of a millimetre: a hundred times tighter than the pick's own
/// 2 mm tolerance, so a scale slip the pick waves through still fails here,
/// while the float noise in a template's authored size (`593.9393939…`) is far
/// inside it. It is deliberately below what a plotter resolves — this is a pin
/// on the file, not on the printed sheet.
const TEMPLATE_EXTENT_SLACK_MM: f64 = 0.02;

/// How far a pinned label's insertion point and body height may drift before
/// the gate speaks, in millimetres.
///
/// The same ten-thousandth as [`PLACEMENT_SLACK_MM`] and for the same reason:
/// a text record's point and size come out of the same metre-to-millimetre
/// conversion as a placement's, so this holds the authored value while
/// tolerating the last bits of the multiply.
///
/// It is deliberately tighter than the one place the corpus asks it to be:
/// `CPECCHB-SQ-A1` letters ten of its cells twice, the two copies 0.000255 mm
/// apart in body height (2.500000 against 2.499745). A slack wider than that
/// would let one copy stand in for the other; this one keeps them apart, and
/// their points — a few thousandths of a millimetre apart — apart as well.
const LABEL_SLACK_MM: f64 = 0.0001;

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

/// One symbol a template places: the file name it stores, lowercased, and the
/// point it sits at on the page.
type Placement = (String, [f64; 2]);

/// What one template placed, over every sheet wearing it: the placements of the
/// first such sheet, and how many sheets those were.
type TemplateAccount = (Vec<Placement>, usize);

/// One label a template letters: the characters, the point it starts at on the
/// page, and its body height — `f64::NAN` where the record states none.
type Label = (String, [f64; 2], f64);

/// What one template lettered, over every sheet wearing it: the labels of the
/// first such sheet, and how many sheets those were.
type LabelAccount = (Vec<Label>, usize);

/// One template's placements as text, for a failure that names them.
fn format_placements(placements: &[Placement]) -> String {
    placements
        .iter()
        .map(|(name, point)| format!("{name}@({:.4}, {:.4})", point[0], point[1]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether two placement lists agree: the same names in the same order, each at
/// the same point within [`PLACEMENT_SLACK_MM`].
///
/// The order is part of the answer. A template that places two `HQZ.sym`s in
/// its sign-off column states which cell comes first, and a decoder that begins
/// reading that column from the other side has changed the file the page came
/// from even though both points are still in the list.
fn same_placements(measured: &[Placement], pinned: &[Placement]) -> bool {
    measured.len() == pinned.len()
        && measured
            .iter()
            .zip(pinned)
            .all(|((name, point), (want_name, want_point))| {
                name == want_name
                    && (point[0] - want_point[0]).abs() < PLACEMENT_SLACK_MM
                    && (point[1] - want_point[1]).abs() < PLACEMENT_SLACK_MM
            })
}

/// Every label the template's furniture letters, in a deterministic order.
///
/// Sorted by text and then by position, so a template that letters one string
/// twice (`CPECCHB-SQ-A1`'s three `制图` cells) keeps a stable order and a
/// respelled cell shifts one entry rather than reshuffling the list.
///
/// Only furniture is read: a drawing's own labels are the drawing's business,
/// and the furniture flag is what tells the template's ink from the sheet's.
fn furniture_labels(model: &PaperModel) -> Vec<Label> {
    let mut out: Vec<Label> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                ..
            } => Some((value.clone(), *origin_mm, height_mm.unwrap_or(f64::NAN))),
            _ => None,
        })
        .collect();
    out.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1[1].total_cmp(&right.1[1]))
            .then(left.1[0].total_cmp(&right.1[0]))
    });
    out
}

/// One label as text, for a failure that names it.
fn format_label(label: &Label) -> String {
    let (text, point, height) = label;
    format!(
        "{text:?} at ({:.4}, {:.4}) mm, {height:.4} mm high",
        point[0], point[1]
    )
}

/// Whether one label matches its pin: the same characters, the same insertion
/// point and the same body height, each within [`LABEL_SLACK_MM`].
///
/// An absent height matches only an absent one, so a record that gains or loses
/// its size is a change rather than a match.
fn same_label(measured: &Label, pinned: &Label) -> bool {
    let (text, point, height) = measured;
    let (want_text, want_point, want_height) = pinned;
    text == want_text
        && (point[0] - want_point[0]).abs() < LABEL_SLACK_MM
        && (point[1] - want_point[1]).abs() < LABEL_SLACK_MM
        && ((height - want_height).abs() < LABEL_SLACK_MM
            || (height.is_nan() && want_height.is_nan()))
}

/// Whether two label lists agree: the same characters in the same order, each
/// at the same insertion point and body height.
fn same_labels(measured: &[Label], pinned: &[Label]) -> bool {
    measured.len() == pinned.len()
        && measured
            .iter()
            .zip(pinned)
            .all(|(measured, pinned)| same_label(measured, pinned))
}

/// Where two label lists part company, one line per difference.
///
/// The lists are compared in order, so a difference is reported by position and
/// the thirty-odd cells that agree are left out: a template that respells one
/// cell should read as one line, not as two walls of text. A length that
/// differs gets its own line plus the cells one side has and the other does
/// not, since those are what a template that gained a cell looks like.
fn label_differences(measured: &[Label], pinned: &[Label]) -> String {
    let mut lines: Vec<String> = measured
        .iter()
        .zip(pinned)
        .enumerate()
        .filter(|(_, (measured, pinned))| !same_label(measured, pinned))
        .map(|(index, (measured, pinned))| {
            format!(
                "    [{index}] page {}, pinned {}",
                format_label(measured),
                format_label(pinned)
            )
        })
        .collect();
    if measured.len() != pinned.len() {
        lines.push(format!(
            "    {} labels on the page, {} pinned",
            measured.len(),
            pinned.len()
        ));
        lines.extend(
            measured
                .iter()
                .skip(pinned.len())
                .map(|extra| format!("    page only: {}", format_label(extra))),
        );
        lines.extend(
            pinned
                .iter()
                .skip(measured.len())
                .map(|extra| format!("    pin only: {}", format_label(extra))),
        );
    }
    lines.join("\n")
}

/// Where the frame offsets a run measured part from [`FRAME_OFFSET_SHEETS`],
/// one line per sheet: a sheet whose frame states an offset the table lacks,
/// one whose offset is another value than the table's, and one the table
/// names whose frame states none. Empty when the two agree.
///
/// A value agrees within [`PLACEMENT_SLACK_MM`]: an offset comes out of the
/// same metre-to-millimetre conversion as a placement point.
fn frame_offset_differences(measured: &[(String, [f64; 2])]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (sheet, offset) in measured {
        match FRAME_OFFSET_SHEETS
            .iter()
            .find(|(pinned, _)| *pinned == sheet.as_str())
        {
            None => lines.push(format!(
                "    {sheet}: its frame states an offset of ({:.6}, {:.6}) mm and it is not \
                 in FRAME_OFFSET_SHEETS",
                offset[0], offset[1]
            )),
            Some((_, pinned)) => {
                if (0..2).any(|axis| (offset[axis] - pinned[axis]).abs() >= PLACEMENT_SLACK_MM) {
                    lines.push(format!(
                        "    {sheet}: its frame states an offset of ({:.6}, {:.6}) mm, \
                         ({:.6}, {:.6}) mm pinned",
                        offset[0], offset[1], pinned[0], pinned[1]
                    ));
                }
            }
        }
    }
    for (sheet, pinned) in &FRAME_OFFSET_SHEETS {
        if !measured.iter().any(|(found, _)| found == sheet) {
            lines.push(format!(
                "    {sheet}: is pinned at ({:.6}, {:.6}) mm and its frame states no offset",
                pinned[0], pinned[1]
            ));
        }
    }
    lines
}

/// What this run is reading: the corpus and why it is that one, the backup,
/// and the face directories that are present, in search order.
///
/// A passing gate has to say what it read. The archive-template run — the
/// corpus pointed at a backup package instead of the share — once took 85 s and
/// reported ok while touching no pin at all, and nothing in its output would
/// have said which corpus that `ok` was about.
///
/// The corpus carries its reason because the reason is what a reader needs
/// when the ledger gate at the end skips its count comparison: a named share
/// asked for that, a share read for want of a package beside the backup did
/// not, and under a package the skip means its sources are not the
/// baseline's, which the gate's own report itemizes — and which fails the
/// run when the package was the default pick.
fn describe_sources(corpus: &Corpus, backup: &Path, drawings: usize, dirs: &[PathBuf]) -> String {
    let faces: Vec<String> = dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .map(|dir| dir.display().to_string())
        .collect();
    format!(
        "corpus {} ({}); backup {} ({drawings} drawings); faces {}",
        corpus.dir.display(),
        corpus.origin.reason(),
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
/// A backup writes its templates into `RefData~*.zip` but no licensed faces,
/// so the archive configuration has a `fonts/` under its corpus only where one
/// is kept beside the archives: `D:\work\plant-code\cad\pid-test-data` has
/// one, `D:\work\cad\pid-test-data` does not. The layout pins all read without
/// a face — how many sheets drew furniture, which template, its extent, its
/// strokes, what it placed — but every pin that needs a written PDF does not,
/// and this used to be an `eprintln!` and a `return`, which the harness
/// reports as a pass. It did, for one run.
///
/// `dirs` is [`face_dirs`]' list, so the message names every place that was
/// looked in, in the order it was looked in, and what it says to do next goes
/// by how the corpus was picked: a package picked by default has already been
/// searched for the share's `fonts`, a named one has not. Naming the share as
/// the corpus is not among the remedies — it would bring its faces, and it
/// would also take the 53 pages out of the ledger gate's count comparison.
///
/// The two configurations, and the command each one takes, are in this file's
/// module docs under `Two corpora, two variables`; the panic repeats the
/// section's name so the message points there too.
fn require_faces(corpus: &Corpus, dirs: &[PathBuf], drawings: usize) {
    if dirs.iter().any(|dir| dir.is_dir()) {
        return;
    }
    let searched = dirs
        .iter()
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let remedy = match corpus.origin {
        CorpusOrigin::Named => {
            "a named corpus is searched as named, and a backup package carries no `fonts/` \
             of its own; set PID_PLOT_FONTS to a directory holding the faces (the share's \
             `fonts`), or leave PID_PARSE_SELF_CHECK_DIR unset, which searches the share's \
             as well"
        }
        CorpusOrigin::Package => {
            "this package keeps no `fonts/` beside its archives, and the share's, searched \
             last, is absent too; set PID_PLOT_FONTS to a directory holding the faces"
        }
        CorpusOrigin::Share => {
            "the share keeps no `fonts/`; set PID_PLOT_FONTS to a directory holding the faces"
        }
    };
    panic!(
        "no font directory is present, so the written-page pins were not read for {drawings} \
         drawings: each PDF's page, its weight, its ink and its lettering, and the \
         {DERIVED_PAGES}-sheet measured-page ratchet.\n  \
         searched, in order: {searched}\n  \
         corpus: {} ({}) — {remedy}.\n  \
         both variables, and the command for each corpus, are in this file's module \
         docs under `Two corpora, two variables`",
        corpus.dir.display(),
        corpus.origin.reason()
    );
}

#[test]
fn every_backup_drawing_exports_a_readable_page() {
    // Read before the corpus is looked for, since it needs none: a template
    // pinned in one table and absent from the other would be a row nothing
    // counts the sheets of.
    assert!(
        TEMPLATE_PLACEMENTS
            .iter()
            .map(|(file, ..)| *file)
            .eq(TEMPLATE_SHEETS.iter().map(|(file, _)| *file)),
        "TEMPLATE_SHEETS has to count the sheets of every template TEMPLATE_PLACEMENTS pins, \
         row for row"
    );
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the backup export; no backup-plant-data directory is present");
        return;
    };
    let corpus = corpus_for(&backup);
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
    let dirs = face_dirs(&corpus);
    let sources = describe_sources(&corpus, &backup, drawings.len(), &dirs);
    eprintln!("{sources}");

    let mut models: Vec<(PathBuf, PaperModel)> = Vec::new();
    for path in &drawings {
        let model = lay_out_drawing(path, &corpus.dir)
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
    // The placements whose stored name a library did answer, as
    // `<sheet>: <symbol>`: what a count short of `TEMPLATE_NAME_GAPS` is made
    // of.
    let mut name_answered: Vec<String> = Vec::new();
    let mut templated_from_archive = 0usize;
    // The sheets whose template was read from the other kind of source than
    // the corpus's own template directories call for, one line each.
    let mut off_source: Vec<String> = Vec::new();
    // The sheets whose frame states an offset, by page key, with the offset.
    let mut frame_offsets: Vec<(String, [f64; 2])> = Vec::new();
    // What each template placed, as the first sheet wearing it drew it, and how
    // many sheets wore it. Keyed by the template's own file name, because a
    // placement is the template's and not the sheet's.
    let mut by_template: BTreeMap<String, TemplateAccount> = BTreeMap::new();
    // Collected rather than asserted per sheet, and keyed by template: one run
    // should report every template that moved, once each, instead of stopping
    // at the first or repeating itself once per sheet wearing it.
    let mut moved: BTreeMap<String, String> = BTreeMap::new();
    // The same two accounts for what each template letters, keyed the same way.
    // A label is the template's text, so the per-sheet agreement and the
    // collect-then-report shape are the placement account's, not a new rule.
    let mut by_template_labels: BTreeMap<String, LabelAccount> = BTreeMap::new();
    let mut mislettered: BTreeMap<String, String> = BTreeMap::new();
    // Who wears what: the sheets on each pinned template, by its pinned file
    // name, and the sheets on none. Tallied as a sheet's template is
    // identified and before anything about it is compared, so the count is of
    // the sheets wearing a template and not of those that agreed with it.
    //
    // A bare sheet keeps its `missing_templates` beside its key — the stems it
    // named that no file answered — which is what tells a sheet this corpus
    // could not give furniture from one that asked for none.
    let mut worn: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut bare: Vec<(String, &[String])> = Vec::new();
    for (path, model) in &models {
        let name = path.display();
        assert!(
            !model
                .missing_symbols
                .iter()
                .chain(&model.library_gaps)
                .any(|missing| missing.to_ascii_lowercase().ends_with("blfh.sym")),
            "{name}: the template's own gap entered the drawing's report: missing={:?} gaps={:?}",
            model.missing_symbols,
            model.library_gaps
        );
        let Some(template) = model.template.as_ref() else {
            bare.push((page_key(path, &backup), model.missing_templates.as_slice()));
            continue;
        };
        templated += 1;
        let archived = from_archive(&template.path);
        if archived {
            templated_from_archive += 1;
        }
        let Some((file, extent, primitives, wanted, wanted_labels)) = TEMPLATE_PLACEMENTS
            .iter()
            .find(|(file, ..)| template.path.ends_with(file))
        else {
            panic!(
                "{name}: {} drew furniture; no pin says what it places",
                template.path
            );
        };
        *worn.entry(*file).or_insert(0) += 1;
        // Which kind of source a sheet's template came from is the corpus's to
        // decide, and it decides it one way: template directories are read
        // before archives, so a template comes out of a `RefData~*.zip`
        // exactly when no template directory of the corpus holds a file of
        // its name. A share keeps all five files under `Ref/Template Files`
        // and has no archive, so every sheet reads from disk; a package keeps
        // them in `RefData~4~682.zip` alone, so every sheet reads from it.
        // The two runs print opposite counts for that reason, and each is
        // held to its own corpus here, sheet by sheet.
        let directories = border_template_places(path, &corpus.dir).directories;
        let on_disk = directories.iter().any(|dir| dir.join(file).is_file());
        if archived == on_disk {
            off_source.push(format!(
                "    {}: {file} was read from {} ({}), and {}",
                page_key(path, &backup),
                if archived { "an archive" } else { "disk" },
                template.path,
                if on_disk {
                    "a template directory of the corpus holds a file of that name, which is \
                     read first"
                } else {
                    "no template directory of the corpus holds a file of that name"
                }
            ));
        }
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
        // The template states its own box; the page has to carry it from
        // where the sheet's border frame places it, because a template is
        // authored in page millimetres and laid out at 1:1. Measuring the
        // strokes — not reusing `extent_mm` — is what makes a stray
        // translation visible: move the ink 2 mm and the box moves with it
        // while `extent_mm` does not budge. Everything below is compared in
        // the template's own coordinates, so the per-template pins hold
        // whichever frame offset a sheet states.
        let offset = template.offset_mm;
        if offset.iter().any(|v| v.abs() > FRAME_OFFSET_FLOOR_MM) {
            frame_offsets.push((page_key(path, &backup), offset));
        }
        let Some([ink_min, ink_max]) = furniture_extent_mm(model) else {
            panic!(
                "{name}: {} is on the page but not one stroke of it landed",
                template.path
            );
        };
        let ink_min = [ink_min[0] - offset[0], ink_min[1] - offset[1]];
        let ink_max = [ink_max[0] - offset[0], ink_max[1] - offset[1]];
        for axis in 0..2 {
            assert!(
                ink_min[axis].abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture starts at {ink_min:?} mm from where the frame \
                 places it ({offset:?} mm)",
                template.path
            );
            assert!(
                (ink_max[axis] - extent[axis]).abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture reaches {ink_max:?} mm, not the {extent:?} mm box \
                 the template states",
                template.path
            );
        }
        // The template's own lettering, cell by cell. `template.primitives`
        // counts these strokes but says nothing about which characters they
        // carry or where the writer put them; this is the pin that does.
        let drawn_labels: Vec<Label> = furniture_labels(model)
            .into_iter()
            .map(|(text, point, height)| {
                (text, [point[0] - offset[0], point[1] - offset[1]], height)
            })
            .collect();
        let pinned_labels: Vec<Label> = wanted_labels
            .iter()
            .map(|(text, point, height)| ((*text).to_string(), *point, *height))
            .collect();
        if same_labels(&drawn_labels, &pinned_labels) {
            // The same per-sheet agreement a placement gets, for the same
            // reason: the text belongs to the template, so a page that letters
            // it at another point is a difference only this can place.
            let seen = by_template_labels
                .entry((*file).to_string())
                .or_insert_with(|| (drawn_labels.clone(), 0));
            assert!(
                same_labels(&seen.0, &drawn_labels),
                "{name}: {} letters differ from the first sheet wearing it:\n{}",
                template.path,
                label_differences(&drawn_labels, &seen.0)
            );
            seen.1 += 1;
        } else {
            mislettered.entry((*file).to_string()).or_insert_with(|| {
                format!(
                    "  {file} (on {}):\n{}",
                    template.path,
                    label_differences(&drawn_labels, &pinned_labels)
                )
            });
        }
        let mut drawn: Vec<Placement> = Vec::new();
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
            } else {
                name_answered.push(format!(
                    "{}: {}",
                    page_key(path, &backup),
                    leaf_name(&symbol.key)
                ));
            }
            drawn.push((
                leaf_name(&symbol.key),
                [
                    symbol.insertion_mm[0] - offset[0],
                    symbol.insertion_mm[1] - offset[1],
                ],
            ));
        }
        // Stable, so the two `HQZ.sym` placements a sign-off column makes keep
        // the template's own order rather than an arbitrary one.
        drawn.sort_by(|a, b| a.0.cmp(&b.0));
        let pinned: Vec<Placement> = wanted
            .iter()
            .map(|(symbol, point)| ((*symbol).to_string(), *point))
            .collect();
        if !same_placements(&drawn, &pinned) {
            moved.entry((*file).to_string()).or_insert_with(|| {
                format!(
                    "  {file} (on {}): places {}, pinned {}",
                    template.path,
                    format_placements(&drawn),
                    format_placements(&pinned)
                )
            });
            continue;
        }
        // The pin is per template, so the sheets that wear one have to agree
        // with each other as well: a per-sheet offset would otherwise hide
        // inside "the template places these" until one sheet wore it alone.
        let seen = by_template
            .entry((*file).to_string())
            .or_insert_with(|| (drawn.clone(), 0));
        assert!(
            same_placements(&seen.0, &drawn),
            "{name}: {} places {} here and {} on the first sheet wearing it",
            template.path,
            format_placements(&drawn),
            format_placements(&seen.0)
        );
        seen.1 += 1;
    }
    assert!(
        moved.is_empty(),
        "{} template(s) place their symbols somewhere other than the pin says:\n{}",
        moved.len(),
        moved.values().cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(
        mislettered.is_empty(),
        "{} template(s) letter something other than the pin says:\n{}",
        mislettered.len(),
        mislettered.values().cloned().collect::<Vec<_>>().join("\n")
    );
    for (file, (measured, sheets)) in &by_template {
        let cells = by_template_labels
            .get(file)
            .map_or(0, |(labels, _)| labels.len());
        eprintln!(
            "  {file}: {sheets} sheets, {cells} labels, {}",
            format_placements(measured)
        );
    }
    // A pinned template no sheet wears is said out loud: its row and its label
    // table were held against nothing in this run.
    for (file, _, _, _, labels) in TEMPLATE_PLACEMENTS
        .iter()
        .filter(|(file, ..)| !worn.contains_key(file))
    {
        eprintln!(
            "  {file}: 0 sheets, its {} pinned labels held against no page",
            labels.len()
        );
    }
    eprintln!(
        "  {} of {templated} templated sheets wear a frame offset",
        frame_offsets.len()
    );
    eprintln!(
        "  {} sheets wear no template: {}",
        bare.len(),
        bare.iter()
            .map(|(sheet, _)| sheet.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        templated >= TEMPLATED_SHEETS,
        "{templated} sheets drew furniture; {TEMPLATED_SHEETS} did when this ratchet was set"
    );
    // No "one placement per sheet" equality: `CPECCHB-SQ-A1` places none and
    // `CPECCHB-SQ-A2` places three. What each template places is pinned above,
    // per sheet, which is where that fact belongs; the sum over the sheets is
    // held to `TEMPLATE_SYMBOL_PLACEMENTS` below.
    let cells: usize = by_template_labels
        .values()
        .map(|(labels, _)| labels.len())
        .sum();
    eprintln!(
        "{templated} sheets drew furniture ({} disk, {} from RefData archives), \
         {placements} placements, {cells} labels, {gap_named} with a named gap",
        templated - templated_from_archive,
        templated_from_archive
    );
    // The account of which sheet wears which template, held to its pins in one
    // report: a sheet that changes template moves the count of the one it
    // left, the count of the one it took and both totals, and those lines
    // belong side by side.
    let mut off_pin: Vec<String> = Vec::new();
    for (file, pinned) in &TEMPLATE_SHEETS {
        let measured = worn.get(file).copied().unwrap_or(0);
        if measured != *pinned {
            off_pin.push(format!(
                "  {file}: {measured} sheet(s) wear it, {pinned} pinned in TEMPLATE_SHEETS"
            ));
        }
    }
    for (sheet, unanswered) in &bare {
        if !BARE_SHEETS.contains(&sheet.as_str()) {
            off_pin.push(format!(
                "  {sheet}: wears no template (unanswered template names: {unanswered:?}) \
                 and is not in BARE_SHEETS"
            ));
        }
    }
    for sheet in BARE_SHEETS {
        if !bare.iter().any(|(measured, _)| measured == sheet) {
            off_pin.push(format!(
                "  {sheet}: is in BARE_SHEETS and is not one of the {} sheet(s) wearing no \
                 template",
                bare.len()
            ));
        }
    }
    if cells != COMPARED_LABEL_CELLS {
        off_pin.push(format!(
            "  {cells} label cells were held against a page, {COMPARED_LABEL_CELLS} pinned in \
             COMPARED_LABEL_CELLS"
        ));
    }
    if placements != TEMPLATE_SYMBOL_PLACEMENTS {
        off_pin.push(format!(
            "  {placements} symbols were placed by the sheets' templates, \
             {TEMPLATE_SYMBOL_PLACEMENTS} pinned in TEMPLATE_SYMBOL_PLACEMENTS"
        ));
    }
    if gap_named != TEMPLATE_NAME_GAPS {
        off_pin.push(format!(
            "  {gap_named} of the {placements} symbols the templates place name a file no \
             library carries, {TEMPLATE_NAME_GAPS} pinned in TEMPLATE_NAME_GAPS; the \
             placements a library answered: {}",
            if name_answered.is_empty() {
                "none".to_string()
            } else {
                name_answered.join(", ")
            }
        ));
    }
    let offset_differences = frame_offset_differences(&frame_offsets);
    if !offset_differences.is_empty() {
        off_pin.push(format!(
            "  {} sheet(s) wear a frame offset, {} pinned in FRAME_OFFSET_SHEETS; the sheets \
             that differ:",
            frame_offsets.len(),
            FRAME_OFFSET_SHEETS.len()
        ));
        off_pin.extend(offset_differences);
    }
    if !off_source.is_empty() {
        off_pin.push(format!(
            "  {} of the {templated} templated sheets read their template from the other \
             kind of source than the corpus {} calls for ({} from disk, \
             {templated_from_archive} from RefData archives):",
            off_source.len(),
            corpus.dir.display(),
            templated - templated_from_archive
        ));
        off_pin.extend(off_source);
    }
    assert!(
        off_pin.is_empty(),
        "the account of which sheet wears which template left its pins in {} place(s):\n{}\n\
         find which sheet changed and why before moving a pin; a template that gains its \
         first sheet has its pinned row and label cells compared with a page for the first \
         time, so check them against that sheet as well",
        off_pin.len(),
        off_pin.join("\n")
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
            subset: face.subset,
        })
        .collect();

    let mut derived = 0usize;
    let mut clipped_at_floor = 0usize;
    let mut deepest_clip_mm = 0.0f64;
    let mut condensed = 0usize;
    let mut smallest_condense = 1.0f64;
    let mut past_the_edge: Vec<String> = Vec::new();
    // The labels among them, as the sheet, the characters and the insertion
    // point: what `PAST_THE_PAGE` is held against.
    let mut labels_past_the_edge: Vec<(String, String, [f64; 2])> = Vec::new();
    // What each sheet's template holds past the paper: the sheet's key, how
    // many primitives, and the furthest reach past the right and top edges.
    let mut template_past_the_paper: Vec<(String, usize, [f64; 2])> = Vec::new();
    // How the ledger files the segments of it the writer leaves unprinted:
    // the sheet's key, the code, how many segments, and the template's file.
    let mut filed_past_the_paper: Vec<(String, &'static str, u64, String)> = Vec::new();
    // The same of a drawing's own, which no page should have.
    let mut own_past_the_paper: Vec<String> = Vec::new();
    let mut papers = [0usize; PAPERS.len()];
    let mut ledger_pages: Vec<(String, ExportLedger)> = Vec::new();
    // Inked paths stroked butt-ended and round-ended, over the 53 pages and
    // on the sheet of `THIN_AHEAD_OF_THE_WIDE_PEN`.
    let mut stroke_ends = (0usize, 0usize);
    let mut thin_ahead: Vec<(usize, usize)> = Vec::new();
    for (path, model) in &models {
        let name = path.display();
        assert_eq!(
            model.off_sheet_primitives, 0,
            "{name}: {} strokes were refused for not fitting the sheet",
            model.off_sheet_primitives
        );
        let sheet = path.file_name().map_or_else(
            || name.to_string(),
            |file| file.to_string_lossy().into_owned(),
        );
        // The page is the paper the sheet states, exactly.
        let paper = PAPERS
            .iter()
            .position(|(paper, _)| model.page_mm == Some(*paper))
            .unwrap_or_else(|| {
                panic!(
                    "{name}: the page is {:?}, which is none of the papers in PAPERS",
                    model.page_mm
                )
            });
        papers[paper] += 1;
        let page = model.page_mm.expect("a paper");
        let mut of_the_template = (0usize, [0.0f64; 2]);
        for primitive in model.primitives_past_the_edge(EDGE_SLACK_MM) {
            if primitive.furniture {
                // A template reaches past the paper's right and top edges
                // and no other: its frame puts it up and right of the corner.
                let points: Vec<[f64; 2]> = match &primitive.shape {
                    PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
                    PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
                    other => panic!("{name}: the template holds {other:?} past the paper"),
                };
                of_the_template.0 += 1;
                for point in points {
                    assert!(
                        point[0] > -EDGE_SLACK_MM && point[1] > -EDGE_SLACK_MM,
                        "{name}: the template reaches past the left or the bottom edge: {}",
                        describe(primitive)
                    );
                    for axis in 0..2 {
                        of_the_template.1[axis] =
                            of_the_template.1[axis].max(point[axis] - page[axis]);
                    }
                }
                continue;
            }
            past_the_edge.push(format!("{sheet}: {}", describe(primitive)));
            if let PaperShape::Text {
                origin_mm, value, ..
            } = &primitive.shape
            {
                labels_past_the_edge.push((sheet.clone(), value.trim().to_string(), *origin_mm));
            }
        }
        if of_the_template.0 > 0 {
            template_past_the_paper.push((
                page_key(path, &backup),
                of_the_template.0,
                of_the_template.1,
            ));
        }
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
        for gap in &model.library_gaps {
            assert!(
                KNOWN_LIBRARY_GAPS.contains(&gap.as_str()),
                "{name}: a symbol no library carries entered the set: {gap}"
            );
        }
        assert!(
            model.missing_symbols.is_empty(),
            "{name}: {} symbol(s) had neither a library body nor a body of the drawing's own: {:?}",
            model.missing_symbols.len(),
            model.missing_symbols
        );
        assert!(
            model.missing_boxes.is_empty(),
            "{name}: {} placement(s) had neither a library body nor a body of the \
             drawing's own and drew a placeholder: {:?}",
            model.missing_boxes.len(),
            model.missing_boxes
        );

        let written =
            write_plot_audited(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        let pdf = written.pdf;
        let audit = written.audit;
        // The write layer's account of this page, and the joined account the
        // baseline stores. `mismatches` is the writer's own check that the
        // verdicts it reached up front are the ones it acted on, so an empty
        // list is the evidence that the account describes the bytes below.
        assert!(
            written.ledger.mismatches.is_empty(),
            "{name}: the writer's verdicts disagreed with what it wrote: {:?}",
            written.ledger.mismatches
        );
        let ledger = ExportLedger::assemble(model, &written.ledger);
        // What the account files as past the paper, by whose it is.
        let worn = model
            .template
            .as_ref()
            .map_or_else(String::new, |template| {
                template
                    .path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&template.path)
                    .to_string()
            });
        for (reason, count) in &ledger.furniture.segments.dropped {
            if reason.code().contains("beyond_the_paper") {
                filed_past_the_paper.push((
                    page_key(path, &backup),
                    reason.code(),
                    *count,
                    worn.clone(),
                ));
            }
        }
        for (reason, count) in &ledger.drawing.segments.dropped {
            if reason.code().contains("beyond_the_paper") {
                own_past_the_paper.push(format!(
                    "  {}: {count} segment(s) of the drawing's own filed as {}",
                    page_key(path, &backup),
                    reason.code()
                ));
            }
        }
        // Requirement 5.5/5.6 on real corpus: the face-free account reaches the
        // same verdicts as the written one, except for the two marks that need
        // a typeface to measure.
        let no_font = write_ledger(model).unwrap_or_else(|error| panic!("{name}: {error}"));
        let no_font_ledger = ExportLedger::assemble(model, &no_font);
        assert!(
            same_account_without_font_marks(&ledger, &no_font_ledger),
            "{name}: the face-free account differs from the written one beyond the two font marks"
        );
        let cross = ledger.cross_check(model, Some(&audit), None);
        assert!(
            cross.is_empty(),
            "{name}: the ledger disagrees with the counts the code already keeps: {cross:#?}"
        );
        ledger_pages.push((page_key(path, &backup), ledger));
        clipped_at_floor += audit.runs_clipped_at_the_condensation_floor;
        deepest_clip_mm = deepest_clip_mm.max(audit.deepest_clip_mm);
        condensed += audit.runs_condensed;
        smallest_condense = smallest_condense.min(audit.smallest_condense);
        assert!(
            pdf.len() < PAGE_CEILING,
            "{name}: the page weighs {} bytes; the faces are not subset",
            pdf.len()
        );
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        // How the page ends its strokes.
        let pens = read_plot_pens(&pdf)
            .unwrap_or_else(|error| panic!("{name}: read back the pens: {error}"));
        let mut ends = (0usize, 0usize);
        for stroked in &pens.ends {
            match (stroked.cap, stroked.join) {
                (LineCap::Butt, LineJoin::Miter) => ends.0 += stroked.paths,
                (LineCap::Round, LineJoin::Round) => ends.1 += stroked.paths,
                other => panic!(
                    "{name}: {} path(s) stroked under {other:?}, which the writer never states",
                    stroked.paths
                ),
            }
        }
        stroke_ends.0 += ends.0;
        stroke_ends.1 += ends.1;
        if sheet.contains(THIN_AHEAD_OF_THE_WIDE_PEN.0) {
            thin_ahead.push(ends);
        }
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
            // A sheet with nothing on it at all exports a blank page rather
            // than failing. None of the 53 is one: `D03`, the sheet with no
            // object of its own, links `CPECCHB-SQ-A2.igr` and so carries that
            // template's 116 furniture primitives, which the two assertions
            // below read like any other page's ink and lettering.
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
    // How the pages end their strokes.
    assert_eq!(
        stroke_ends, STROKE_ENDS,
        "the 53 pages stroke {} path(s) butt-ended and {} round; {} and {} are pinned in \
         STROKE_ENDS",
        stroke_ends.0, stroke_ends.1, STROKE_ENDS.0, STROKE_ENDS.1
    );
    assert_eq!(
        thin_ahead,
        [(THIN_AHEAD_OF_THE_WIDE_PEN.1, THIN_AHEAD_OF_THE_WIDE_PEN.2)],
        "how the page of `{}` ends its strokes (butt-ended paths, round-ended paths)",
        THIN_AHEAD_OF_THE_WIDE_PEN.0
    );
    // Every page is its sheet's paper, and the papers are the three pinned.
    for ((paper, pinned), found) in PAPERS.iter().zip(papers) {
        assert_eq!(
            found, *pinned,
            "{found} sheets are on {paper:?} mm paper; {pinned} are pinned in PAPERS"
        );
    }
    // What a border frame holds past the paper, sheet by sheet.
    let mut template_differences: Vec<String> = Vec::new();
    for (sheet, count, reach) in &template_past_the_paper {
        match TEMPLATE_PAST_THE_PAPER
            .iter()
            .find(|(pinned, ..)| pinned == sheet)
        {
            None => template_differences.push(format!(
                "  {sheet}: {count} template primitive(s) past the paper by ({:.6}, {:.6}) mm;                  the sheet is not in TEMPLATE_PAST_THE_PAPER",
                reach[0], reach[1]
            )),
            Some((_, pinned_count, pinned_reach)) => {
                let same = count == pinned_count
                    && (0..2).all(|axis| (reach[axis] - pinned_reach[axis]).abs() < PLACEMENT_SLACK_MM);
                if !same {
                    template_differences.push(format!(
                        "  {sheet}: {count} template primitive(s) past the paper by                          ({:.6}, {:.6}) mm; pinned {pinned_count} by ({:.6}, {:.6})",
                        reach[0], reach[1], pinned_reach[0], pinned_reach[1]
                    ));
                }
            }
        }
    }
    for (sheet, ..) in &TEMPLATE_PAST_THE_PAPER {
        if !template_past_the_paper
            .iter()
            .any(|(found, ..)| found == sheet)
        {
            template_differences.push(format!(
                "  {sheet}: nothing of its template is past the paper; it is pinned in \
                 TEMPLATE_PAST_THE_PAPER"
            ));
        }
    }
    assert!(
        template_differences.is_empty(),
        "what the border templates hold past the paper is not what is pinned:\n{}",
        template_differences.join("\n")
    );
    // How the ledger files the part of it that is not printed: under the
    // witnessed code on the sheets a plot shows placed so, under the other
    // on the sheets none does, sheet by sheet.
    let mut filing_differences: Vec<String> = own_past_the_paper;
    let pinned_filing = |sheet: &str| -> Vec<(&'static str, u64, &'static str)> {
        let plotted = PLOTTED_PAST_THE_PAPER
            .iter()
            .filter(|(pinned, ..)| *pinned == sheet)
            .map(|(_, template, segments)| (PLOTTED_CODE, *segments, *template));
        let unplotted = UNPLOTTED_PAST_THE_PAPER
            .iter()
            .filter(|(pinned, ..)| *pinned == sheet)
            .map(|(_, template, segments)| (UNPLOTTED_CODE, *segments, *template));
        plotted.chain(unplotted).collect()
    };
    for (sheet, code, count, template) in &filed_past_the_paper {
        match pinned_filing(sheet).as_slice() {
            [(pinned_code, pinned_count, pinned_template)]
                if pinned_code == code && pinned_count == count && pinned_template == template => {}
            [(pinned_code, pinned_count, pinned_template)] => filing_differences.push(format!(
                "  {sheet}: {count} segment(s) of {template} filed as {code}; pinned \
                 {pinned_count} of {pinned_template} as {pinned_code}"
            )),
            pinned => filing_differences.push(format!(
                "  {sheet}: {count} segment(s) of {template} filed as {code}; the sheet is in \
                 {} row(s) of PLOTTED_PAST_THE_PAPER and UNPLOTTED_PAST_THE_PAPER",
                pinned.len()
            )),
        }
    }
    let pinned_sheets = PLOTTED_PAST_THE_PAPER
        .iter()
        .map(|(sheet, ..)| *sheet)
        .chain(UNPLOTTED_PAST_THE_PAPER.iter().map(|(sheet, ..)| *sheet));
    for sheet in pinned_sheets {
        let found = filed_past_the_paper
            .iter()
            .filter(|(filed, ..)| filed == sheet)
            .count();
        if found != 1 {
            filing_differences.push(format!(
                "  {sheet}: the ledger files its template's ink past the paper under {found} \
                 code(s); it is pinned under one"
            ));
        }
    }
    assert!(
        filing_differences.is_empty(),
        "how the ledger files what is past the paper is not what is pinned — a template a \
         plot shows placed so under {PLOTTED_CODE}, any other under {UNPLOTTED_CODE}, nothing \
         of a drawing's own:\n{}",
        filing_differences.join("\n")
    );
    // One primitive of a drawing's own reaches past its page, and it is the
    // sideways run in `N井场`: origin 0.208 mm under the bottom edge, nineteen
    // of its twenty glyphs on the sheet. An equality because both directions
    // matter — a new overhang is a reading to explain, and losing this one
    // means the run was dropped or moved.
    assert_eq!(
        past_the_edge.len(),
        1,
        "{} primitives of a drawing's own reach past their page; one did when this was pinned: \
         {past_the_edge:#?}",
        past_the_edge.len()
    );
    assert!(
        past_the_edge[0].contains("N井场"),
        "the primitive past a page is not the sideways run in `N井场`: {}",
        past_the_edge[0]
    );
    // Which primitive it is, and where: the sheet's name alone would be
    // satisfied by any other run of that sheet sliding over the edge in this
    // one's place.
    let (pinned_sheet, pinned_text, pinned_origin) = PAST_THE_PAGE;
    let is_the_pinned_run = labels_past_the_edge
        .first()
        .is_some_and(|(sheet, text, origin)| {
            sheet.contains(pinned_sheet)
                && text == pinned_text
                && (0..2)
                    .all(|axis| (origin[axis] - pinned_origin[axis]).abs() < PLACEMENT_SLACK_MM)
        });
    assert!(
        is_the_pinned_run,
        "the primitive past a page is {}; pinned in PAST_THE_PAGE is the label {pinned_text:?} \
         of `{pinned_sheet}` at ({:.6}, {:.6}) mm (labels measured past a page: \
         {labels_past_the_edge:?})",
        past_the_edge[0], pinned_origin[0], pinned_origin[1]
    );
    eprintln!("past the page: {}", past_the_edge[0]);
    // Lettering the sheet cannot hold even at half width loses its tail to
    // the page clip. The writer used to do that silently, so no one could tell
    // a page whose labels all fit from one whose last glyphs were cut away.
    assert_eq!(
        clipped_at_floor, CLIPPED_AT_CONDENSATION_FLOOR,
        "{clipped_at_floor} run(s) are cut by the page at the condensation floor \
         (deepest {deepest_clip_mm:.3} mm past the edge); \
         {CLIPPED_AT_CONDENSATION_FLOOR} were when this ratchet was set"
    );
    // The depth is the count's other half: with no run clipped there is no
    // clip to be deep, so a depth on its own is a writer that cut a run and
    // did not count it.
    assert!(
        clipped_at_floor > 0 || deepest_clip_mm == 0.0,
        "no run is counted as clipped at the condensation floor, and the deepest clip is \
         {deepest_clip_mm:.6} mm past the edge rather than 0"
    );
    eprintln!(
        "clipped at the condensation floor: {clipped_at_floor} run(s), deepest {deepest_clip_mm:.3} mm past the edge"
    );
    assert_eq!(
        condensed, CONDENSED_RUNS,
        "{condensed} run(s) were condensed to stay on the page (smallest scale          {smallest_condense:.3}); {CONDENSED_RUNS} did when this ratchet was set"
    );
    // The scale is the count's other half in the same way: with no run
    // condensed, every run is drawn at the width the drawing set it.
    assert!(
        condensed > 0 || smallest_condense == 1.0,
        "no run is counted as condensed, and the smallest scale a run was drawn at is \
         {smallest_condense:.6} rather than 1"
    );
    eprintln!("condensed to fit: {condensed} run(s), smallest scale {smallest_condense:.3}");
    // Which faces a run embeds is the machine's to say as much as the
    // drawings': a face only a formatting run names is optional, and a lender
    // is whichever of the two this machine has. The ledger gate below is told
    // the list and skips its two font marks when it is not the baseline's, so
    // it is printed by name here and not pinned.
    let mut embedded: Vec<&str> = prepared.iter().map(|face| face.key.as_str()).collect();
    embedded.sort_unstable();
    eprintln!(
        "{} backup drawings exported, {} of them on a measured page, {} faces embedded ({})",
        models.len(),
        derived,
        prepared.len(),
        embedded.join(", ")
    );

    // The ledger gate over the whole package. Its baseline is taken from a
    // package corpus — the one an unnamed run reads when the backup sits in a
    // package (see the module docs' `Two corpora, two variables`) — so a page
    // whose account moved fails here rather than on the next release. Under a
    // share the counts are not comparable, and the gate passes on the
    // unattributed and cross-check rules alone; its report says which of the
    // two this run got. The faces are named, so a machine that embeds a
    // different set of typefaces is told that rather than made to look like a
    // regression.
    ledger_pages.sort_by(|left, right| left.0.cmp(&right.0));
    let ledgers: Vec<&pid_plot::ledger::LayoutLedger> =
        models.iter().map(|(_, model)| &model.ledger).collect();
    let font_faces: Vec<String> = prepared.iter().map(|face| face.key.clone()).collect();
    let descriptor = descriptor_for(&ledgers, font_faces);
    let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("baselines")
        .join("export-ledger-backup.json");
    let update = std::env::var("UPDATE_EXPORT_LEDGER").is_ok_and(|value| value == "1");
    let report = run_gate(GateInput {
        corpus: LedgerCorpus::Backup,
        baseline_path: &baseline_path,
        descriptor,
        pages: ledger_pages,
        failures: Vec::new(),
        breaches: Vec::new(),
        update,
        hint: "备份基线取法：在包语料下排版（基线文件头把包内 RefData~*.zip 记作模板与符号来源），\
               只有这时 53 页才逐页比对。不设 PID_PARSE_SELF_CHECK_DIR 时，backup-plant-data \
               的上一级目录带 RefData~*.zip 就默认用它；也可把该变量显式指向包目录，包里没有 \
               fonts 时另用 PID_PLOT_FONTS 给出字型。显式指向共享语料则按需求 13.7 跳过计数\
               比对，只查未归因与核对。命令：cargo test -p pid-plot --test export_plant_backup\
               （详见 docs/baselines/README.md）",
    });
    let rendered = report.render();
    eprintln!("{rendered}");
    assert!(report.passed(), "{rendered}");
    // Whether the counts were compared at all is the one thing `passed` does
    // not say, and the two kinds of run differ in it by design. A corpus a
    // run names is read as named: a share's counts are not the baseline's, so
    // requirement 13.7 skips the comparison and the pass above rests on the
    // unattributed and cross-check rules. A package picked by default is
    // picked so that its pages are compared, so a default run whose symbol or
    // template sources are not the baseline's has compared nothing and says
    // so here instead of passing. A run that rewrites the baseline compares
    // nothing either, by its own choice.
    if corpus.origin == CorpusOrigin::Package && !update {
        assert!(
            report.counts_compared(),
            "the backup was laid out against the package it sits in ({}), picked by default \
             so that its {} pages are compared with the ledger baseline, and no count was \
             compared:\n{rendered}\n\
             if this package is not the one the baseline was taken from, name the corpus in \
             PID_PARSE_SELF_CHECK_DIR: a named corpus runs without the comparison",
            corpus.dir.display(),
            models.len()
        );
    }
}

/// Whether two accounts agree once the two marks that need a typeface are
/// dropped.
///
/// `write_ledger` reads no face, so it cannot measure condensing or clipping:
/// its account carries neither `text_condensed` nor `text_clipped_at_floor`,
/// and its `fonts_measured` is `false`. Everything else — every drop, every
/// on-page count, every segment reason, every remainder — has to be identical,
/// which is what requirement 5.6 asks of the face-free path.
fn same_account_without_font_marks(written: &ExportLedger, face_free: &ExportLedger) -> bool {
    if written.page != face_free.page
        || written.drawing.parse != face_free.drawing.parse
        || written.drawing.entities != face_free.drawing.entities
        || written.drawing.bodies != face_free.drawing.bodies
        || written.drawing.text != face_free.drawing.text
        || written.drawing.segments != face_free.drawing.segments
        || written.drawing.asymmetry != face_free.drawing.asymmetry
        || written.furniture != face_free.furniture
    {
        return false;
    }
    let mut written_primitives = written.drawing.primitives.clone();
    let mut face_free_primitives = face_free.drawing.primitives.clone();
    for account in [&mut written_primitives, &mut face_free_primitives] {
        account
            .marks
            .remove(&pid_plot::ledger::PartialMark::TextCondensed);
        account
            .marks
            .remove(&pid_plot::ledger::PartialMark::TextClippedAtFloor);
    }
    written_primitives == face_free_primitives
}

/// `igSymbol2d` (`0x00CE`) records `D04` decodes: the five live placements on
/// `/Sheet6`, its one sheet stream.
const D04_DECODED_IGSYMBOLS: usize = 5;

/// `D04` pins `0` refused `igSymbol2d` (`0x00CE`) records directly, not
/// through any page effect. `every_backup_drawing_exports_a_readable_page`
/// lays `D04` out along with the other 52 drawings, but a refused
/// `igSymbol2d` placement does not change the page size, the primitive
/// count past the edge, the clip/condense counts or the library-gap set —
/// it is simply absent, the way `decoded_igsymbols.len() == 0` was before
/// `decode_igsymbols` learned `D04`'s own matrix-tag variant. This test
/// pins the refusal count where a regression in that acceptance would
/// actually show up.
///
/// The decoded count is pinned beside it, because no refusal is also what a
/// drawing reports when its placements were never walked at all — a sheet
/// stream that yields no geometry, or records counted under another heading
/// than refused. Five decoded is what tells "none refused" from "none seen".
#[test]
fn d04_igsymbol2d_refused_count_is_zero() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the D04 refusal pin; no backup-plant-data directory is present");
        return;
    };
    let path = backup.join("test").join("U01").join("D04.pid");
    if !path.is_file() {
        eprintln!(
            "skipping the D04 refusal pin; {} is not present",
            path.display()
        );
        return;
    }
    let doc = pid_parse::PidParser::new()
        .parse_file(&path)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let normalized = pid_parse::build_normalized_geometry(&doc);
    let igsymbol_refusals: Vec<_> = normalized
        .refused_graphic_records
        .iter()
        .filter(|record| record.type_code == 0x00CE)
        .collect();
    assert!(
        igsymbol_refusals.is_empty(),
        "D04 should refuse 0 igSymbol2d (0x00CE) records, found {igsymbol_refusals:?}"
    );
    let decoded_igsymbols: usize = doc
        .sheet_streams
        .iter()
        .filter_map(|sheet| sheet.geometry.as_ref())
        .map(|geometry| geometry.decoded_igsymbols.len())
        .sum();
    assert_eq!(
        decoded_igsymbols, D04_DECODED_IGSYMBOLS,
        "D04 decodes {decoded_igsymbols} igSymbol2d (0x00CE) records over its sheet streams; \
         {D04_DECODED_IGSYMBOLS} are pinned"
    );
}
