//! Which does a plot paint first: a stroke in the paper's colour, or the ink
//! it lies on — and a label's origin mark, or the label's text?
//!
//! A label symbol's body carries an origin mark, a short stroke the plots
//! paint in the paper's colour. Where it lies on ink — a box edge, a pipe, a
//! character — the order shows on paper: painted after the ink it takes a
//! piece out of it, painted before it does not. The ink gate counts those
//! places sheet by sheet (`pid_plot::ink::wipe_order`) and holds our page to
//! the reference's order at each. This probe asks what decides that order.
//!
//! For every drawing of the corpus with a plot beside it, it writes our
//! page, reads both pages in painting order, and lists every such place
//! (`pid_plot::ink::wipe_sites`) with:
//!
//! - what the ink is — a character or a stroke — and where;
//! - which of the two the reference paints first, and whether our page
//!   paints them in that order;
//! - the primitive of our page model the mark is, and the one the ink is:
//!   the record each was laid out from, its symbol, and the rank the sheet's
//!   own list gives that record (`pid_parse::parsers::sheet_display_order`);
//! - whether the ink is the mark's own label's text, and which of the two
//!   the sheet's list puts first.
//!
//! A label's text is not its symbol's record: it is an `igTextBox` owned by
//! a `DependencyObject` whose payload names the symbol at `+16`, and the
//! sheet lists the symbol and that object each at a place of its own. So
//! the second half reads **every** label whose symbol carries a mark,
//! whether or not the mark lies on ink (`tests/support/marked_labels.rs`):
//! where the sheet lists the symbol and where the text's owner, and which
//! of the mark and the text the reference paints first. It prints each
//! label the list puts text first and each the plot paints against its
//! list; and for one painted against its list, the same label on any other
//! sheet — the same text, at the same point, at the same places of that
//! sheet's list — with what that sheet's plot does. It prints as well each
//! owner that names a symbol with a mark and gave no label, with why: it
//! holds no text, or the plot does not have the label where the drawing
//! puts it (`marked_labels::NotALabel`).
//!
//! Then, a sheet a line and over all of them: the places, how many the
//! reference paints ink first, how many our page paints in the reference's
//! order, how many the sheet's list puts in the reference's order — a mark
//! on its own label's text apart from a mark on anything else — and the
//! labels. A rule would show as a column that is all one way. The last
//! line asks the same of what else a record carries that could decide an
//! order — its oid, and its place in the stream: for how many labels each
//! says what the plot paints.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_of_a_mark_and_its_ink_a_plot_paints_first -- <corpus>
//! ```
//!
//! On the eleven acceptance sheets (2026-11-12): 91 places, 90 of them with
//! both sides in our model. The reference paints the ink first at 60 and
//! the mark first at 30, so neither comes first as a rule; the sheet's list
//! puts 89 of the 90 in the reference's order. Of 96 labels with a mark the
//! sheets list the text's owner right after the symbol for 49 and before it
//! for 47, and the plots paint 95 as listed. The one against its list is
//! `P70` on `一级闪蒸分离器A` — listed text first, painted mark first — and
//! the same label on `一级闪蒸分离器B`, at the same point and the same
//! places (99 and 104), is painted as listed. The 96 are every such label
//! the drawings and their plots both show: 120 owners name a symbol with a
//! mark, 23 of them hold a point and no text, each on the symbol of one of
//! the 96, and one is a label the plot does not have where the drawing puts
//! it (`SP=-8℃` on the A2 sheet, lettered 16.9 mm away in the instrument
//! cluster a later revision moved). The oids of a label's records, and
//! their places in the stream, say what the plot paints for 48 and 56 of
//! the 96; the sheet's list for 95. See
//! `docs/analysis/2026-11-12-a-mark-and-its-ink-are-painted-in-the-order-the-sheet-lists-them.md`.
//! Read-only: it writes nothing into the corpus.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::compare::{MaskRect, compare_plots_with_model};
use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::ink::{PageInk, WipeSite, wipe_sites};
use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape, lay_out_drawing};
use pid_plot::read::read_plot_sequence;
use pid_plot::write::{PlotFont, write_plot};

#[path = "../tests/support/marked_labels.rs"]
mod marked_labels;
use marked_labels::{
    LeftOut, MarkedLabel, NEAR_MM, NotALabel, apart, live_records, marked_label_census,
    off_the_piece, the_same_mark, the_symbols_own_text, white,
};

fn paired_sheets(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
                && path.with_extension("pdf").is_file()
        })
        .collect();
    paths.sort();
    paths
}

/// How near the stroke of `primitive` passes to `point`; `None` for a
/// primitive that strokes nothing.
fn off_the_stroke(primitive: &PaperPrimitive, point: [f64; 2]) -> Option<f64> {
    match &primitive.shape {
        PaperShape::Line { start_mm, end_mm } => Some(off_the_piece(point, *start_mm, *end_mm)),
        PaperShape::Polyline { points_mm, .. } => points_mm
            .windows(2)
            .map(|pair| off_the_piece(point, pair[0], pair[1]))
            .min_by(f64::total_cmp),
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        }
        | PaperShape::Circle {
            center_mm,
            radius_mm,
        } => Some((apart(point, *center_mm) - radius_mm).abs()),
        PaperShape::Text { .. } | PaperShape::Fill { .. } => None,
    }
}

/// The primitive of the model stroked at `point`, in the paper's colour or
/// in ink: the nearest one.
fn the_stroke(model: &PaperModel, point: [f64; 2], in_paper: bool) -> Option<usize> {
    model
        .primitives
        .iter()
        .enumerate()
        .filter(|(_, primitive)| primitive.stroke.is_some() && white(primitive) == in_paper)
        .filter_map(|(index, primitive)| Some((off_the_stroke(primitive, point)?, index)))
        .filter(|(off, _)| *off <= NEAR_MM)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, index)| index)
}

/// The label that letters `glyph` at `origin`: the nearest text that holds
/// the character, within a label's length of it.
fn the_label(model: &PaperModel, glyph: &str, origin: [f64; 2]) -> Option<usize> {
    model
        .primitives
        .iter()
        .enumerate()
        .filter_map(|(index, primitive)| match &primitive.shape {
            PaperShape::Text {
                origin_mm, value, ..
            } if value.contains(glyph) => Some((apart(*origin_mm, origin), index)),
            _ => None,
        })
        .filter(|(off, _)| *off <= 60.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, index)| index)
}

/// One side of a place, as the page model holds it.
struct Side {
    /// What it is, for the listing.
    what: String,
    /// The record it was laid out from: stream and offset.
    record: Option<(String, usize)>,
    /// Where the sheet's own list ranks that record.
    rank: Option<(u32, u32)>,
    /// Where our page paints it: its position in the model's paint order.
    painted: Option<usize>,
}

fn side(
    model: &PaperModel,
    doc: &pid_parse::PidDocument,
    painted: &[usize],
    index: Option<usize>,
) -> Side {
    let Some((index, primitive)) =
        index.and_then(|index| Some((index, model.primitives.get(index)?)))
    else {
        return Side {
            what: "nothing of ours found there".to_string(),
            record: None,
            rank: None,
            painted: None,
        };
    };
    let record = match (&primitive.stream_path, primitive.byte_range) {
        (Some(stream), Some(range)) => Some((stream.clone(), range[0])),
        _ => None,
    };
    let rank = record.as_ref().and_then(|(stream, start)| {
        if primitive.furniture {
            return None;
        }
        let rank = doc.sheet_display_order.get(stream)?.ranks.get(start)?;
        Some((rank.place, rank.inner))
    });
    let shape = match &primitive.shape {
        PaperShape::Line { .. } => "line".to_string(),
        PaperShape::Polyline { points_mm, .. } => format!("polyline of {}", points_mm.len()),
        PaperShape::Arc { .. } => "arc".to_string(),
        PaperShape::Circle { .. } => "circle".to_string(),
        PaperShape::Text { value, .. } => format!("label {value:?}"),
        PaperShape::Fill { .. } => "fill".to_string(),
    };
    let what = format!(
        "{}{shape}{}{}{}",
        if primitive.furniture { "template " } else { "" },
        primitive
            .symbol
            .as_ref()
            .map_or_else(String::new, |symbol| format!(
                " of {}",
                symbol.rsplit(['\\', '/']).next().unwrap_or(symbol)
            )),
        record
            .as_ref()
            .map_or_else(String::new, |(stream, start)| format!(
                " [{stream} +{start}]"
            )),
        rank.map_or_else(String::new, |(place, inner)| format!(
            " rank ({place}, {inner})"
        )),
    );
    Side {
        what,
        record,
        rank,
        painted: painted.iter().position(|painted| *painted == index),
    }
}

/// The tallies of one kind of place: how many, how many the reference
/// paints ink first, how many our page paints in the reference's order, and
/// how many the sheet's list puts in the reference's order (of those both
/// of whose records it ranks).
#[derive(Clone, Copy, Default)]
struct Tally {
    places: usize,
    ink_first: usize,
    ours_same: usize,
    ranked: usize,
    list_same: usize,
}

impl Tally {
    fn add(&mut self, other: Self) {
        self.places += other.places;
        self.ink_first += other.ink_first;
        self.ours_same += other.ours_same;
        self.ranked += other.ranked;
        self.list_same += other.list_same;
    }

    fn says(self) -> String {
        format!(
            "{} place(s), ink first at {}, ours in the reference's order at {}, the sheet's \
             list in the reference's order at {} of {} it ranks both sides of",
            self.places, self.ink_first, self.ours_same, self.list_same, self.ranked
        )
    }
}

/// The labels of a sheet, tallied: see [`MarkedLabel`].
#[derive(Clone, Copy, Default)]
struct Labels {
    read: usize,
    listed_next: usize,
    listed_text_first: usize,
    painted_text_first: usize,
    painted_next: usize,
    as_listed: usize,
    text_first_as_listed: usize,
}

impl Labels {
    fn of(labels: &[MarkedLabel]) -> Self {
        let count = |test: fn(&MarkedLabel) -> bool| labels.iter().filter(|l| test(l)).count();
        Self {
            read: labels.len(),
            listed_next: count(MarkedLabel::listed_next),
            listed_text_first: count(MarkedLabel::listed_text_first),
            painted_text_first: count(MarkedLabel::painted_text_first),
            painted_next: count(MarkedLabel::painted_next),
            as_listed: count(MarkedLabel::as_listed),
            text_first_as_listed: count(|label| {
                label.listed_text_first() && label.painted_text_first()
            }),
        }
    }

    fn add(&mut self, other: Self) {
        self.read += other.read;
        self.listed_next += other.listed_next;
        self.listed_text_first += other.listed_text_first;
        self.painted_text_first += other.painted_text_first;
        self.painted_next += other.painted_next;
        self.as_listed += other.as_listed;
        self.text_first_as_listed += other.text_first_as_listed;
    }

    fn says(self) -> String {
        format!(
            "{} label(s) with a mark: the sheet lists the text's owner right after the symbol \
             for {} and before it for {}; the reference letters the text right after the mark \
             for {} and before it for {}; it paints {} in the order the sheet lists them, {} \
             of the {} listed text first among them",
            self.read,
            self.listed_next,
            self.listed_text_first,
            self.painted_next,
            self.painted_text_first,
            self.as_listed,
            self.text_first_as_listed,
            self.listed_text_first
        )
    }
}

/// What a sheet reads: places where a mark lies on its own label's text,
/// places where it lies on anything else, places one side of which our
/// model does not have, its labels, and the owners of a marked symbol that
/// gave no label.
#[derive(Default)]
struct SheetReading {
    own: Tally,
    another: Tally,
    unplaced: Tally,
    labels: Vec<MarkedLabel>,
    left_out: Vec<LeftOut>,
}

/// Why an owner gave no label, in a few words.
fn why(why: NotALabel) -> &'static str {
    match why {
        NotALabel::NoText => "it holds no text box",
        NotALabel::TextNotOnOurPage => "our page letters nothing from its text box",
        NotALabel::NoMarkOnThePlot => {
            "the plot strokes nothing in the paper's colour through the mark"
        }
        NotALabel::Unranked => "the sheet's list ranks its symbol or its text nowhere",
        NotALabel::TextNotOnThePlot => "the plot has no run of glyphs that spells its text",
    }
}

/// The owners of a marked symbol that gave no label, tallied by why — and
/// those with no text box by what they hold instead.
fn left_out_says(left_out: &[&LeftOut]) -> String {
    let mut by_why: std::collections::BTreeMap<NotALabel, usize> =
        std::collections::BTreeMap::new();
    let mut held: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for owner in left_out {
        *by_why.entry(owner.why).or_default() += 1;
        if owner.why == NotALabel::NoText {
            let holds: Vec<String> = owner
                .holds
                .iter()
                .map(|kind| format!("0x{kind:04X}"))
                .collect();
            *held.entry(format!("[{}]", holds.join(", "))).or_default() += 1;
        }
    }
    let reasons: Vec<String> = by_why
        .iter()
        .map(|(reason, count)| format!("{count} because {}", why(*reason)))
        .collect();
    let held: Vec<String> = held
        .iter()
        .map(|(holds, count)| format!("{count} hold {holds}"))
        .collect();
    format!(
        "{} owner(s) of a marked symbol gave no label{}{}{}",
        left_out.len(),
        if reasons.is_empty() { "" } else { ": " },
        reasons.join("; "),
        if held.is_empty() {
            String::new()
        } else {
            format!(" (of those with no text box, {})", held.join(", "))
        }
    )
}

fn says(label: &MarkedLabel) -> String {
    format!(
        "label {:?} of {} at ({:.2}, {:.2}), symbol / owner / text oid {:?} at +{:?}: the sheet \
         lists the symbol at {:?} and the text at {:?} ({}); the plot strokes the mark at \
         step(s) {:?} and letters the text from step {} ({}{})",
        label.value,
        label.symbol,
        label.mark_mm[0],
        label.mark_mm[1],
        label.oids,
        label.stored_at,
        label.symbol_rank,
        label.text_rank,
        if label.listed_text_first() {
            "text first"
        } else {
            "symbol first"
        },
        label.mark_steps,
        label.lettered,
        if label.painted_text_first() {
            "text first"
        } else {
            "mark first"
        },
        if label.painted_next() {
            ", right after the mark"
        } else {
            ""
        },
    )
}

fn read_sheet(path: &Path, corpus: &Path, dirs: &[PathBuf]) -> Result<SheetReading, String> {
    let reference_pdf = std::fs::read(path.with_extension("pdf")).map_err(|e| e.to_string())?;
    let model = lay_out_drawing(path, corpus).map_err(|e| e.to_string())?;
    let doc = pid_parse::PidParser::new()
        .parse_file(path)
        .map_err(|e| e.to_string())?;
    let package = pid_parse::PidParser::new()
        .parse_package(path)
        .map_err(|e| e.to_string())?;
    let prepared = prepare_fonts(&[&model], dirs).map_err(|e| e.to_string())?;
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    let our_pdf = write_plot(&model, &fonts).map_err(|e| e.to_string())?;
    let reference = PageInk::read(&reference_pdf).map_err(|e| e.to_string())?;
    let ours = PageInk::read(&our_pdf).map_err(|e| e.to_string())?;
    let missing: Vec<MaskRect> = model
        .missing_boxes
        .iter()
        .map(|missing| MaskRect {
            label: missing.path.clone(),
            min_mm: missing.min_mm,
            max_mm: missing.max_mm,
        })
        .collect();
    let whole = compare_plots_with_model(&reference.ink, &ours.ink, &[], &missing, &model);
    let reference_order = read_plot_sequence(&reference_pdf).map_err(|e| e.to_string())?;
    let sites: Vec<WipeSite> = wipe_sites(
        &reference_order,
        &read_plot_sequence(&our_pdf).map_err(|e| e.to_string())?,
        &whole.fit,
    );
    // The order our page is painted in: the model's own, where no sheet
    // states one.
    let painted: Vec<usize> = model
        .paint_sequence()
        .unwrap_or_else(|| (0..model.primitives.len()).collect());

    let mut reading = SheetReading::default();
    for site in &sites {
        let mark = side(
            &model,
            &doc,
            &painted,
            the_stroke(&model, site.mark_mm, true),
        );
        let ink_index = match &site.glyph {
            Some(glyph) => the_label(&model, glyph, site.ink_mm),
            None => the_stroke(&model, site.ink_mm, false),
        };
        let ink = side(&model, &doc, &painted, ink_index);
        let ours_same = site.ours().is_none();
        // A mark on its own label's text: the text's owner names the
        // mark's symbol.
        let own_text = match (&mark.record, &ink.record) {
            (Some((mark_stream, mark_at)), Some((ink_stream, ink_at)))
                if mark_stream == ink_stream =>
            {
                package.get_stream(mark_stream).is_some_and(|stream| {
                    the_symbols_own_text(&live_records(&stream.data), *mark_at, *ink_at)
                })
            }
            _ => false,
        };
        // Which the sheet's list puts first, where it ranks both.
        let list_ink_first = match (mark.rank, ink.rank) {
            (Some(mark), Some(ink)) if mark != ink => Some(ink < mark),
            _ => None,
        };
        // Which our model paints first.
        let model_ink_first = match (mark.painted, ink.painted) {
            (Some(mark), Some(ink)) => Some(ink < mark),
            _ => None,
        };
        let bucket = if mark.record.is_none() || ink.record.is_none() {
            &mut reading.unplaced
        } else if own_text {
            &mut reading.own
        } else {
            &mut reading.another
        };
        bucket.places += 1;
        bucket.ink_first += usize::from(site.ink_first);
        bucket.ours_same += usize::from(ours_same);
        if let Some(list_ink_first) = list_ink_first {
            bucket.ranked += 1;
            bucket.list_same += usize::from(list_ink_first == site.ink_first);
        }
        let first = |ink_first: Option<bool>| match ink_first {
            Some(true) => "ink first",
            Some(false) => "mark first",
            None => "does not say",
        };
        println!(
            "   mark at ({:.2}, {:.2}) on {} at ({:.2}, {:.2}): reference {} (steps mark {} / ink \
             {}); ours {} (steps mark {:?} / ink {:?}; the model paints {}); the sheet's list: {}; \
             {}",
            site.mark_mm[0],
            site.mark_mm[1],
            site.glyph.as_ref().map_or_else(
                || "a stroke".to_string(),
                |glyph| format!("the character {glyph:?}")
            ),
            site.ink_mm[0],
            site.ink_mm[1],
            first(Some(site.ink_first)),
            site.mark_step,
            site.ink_step,
            if ours_same {
                "the same"
            } else {
                "THE OTHER WAY"
            },
            site.our_mark_steps,
            site.our_ink_steps,
            first(model_ink_first),
            first(list_ink_first),
            if own_text {
                "the mark's own label's text"
            } else {
                "not the mark's label"
            },
        );
        println!("      the mark: {}", mark.what);
        println!("      the ink:  {}", ink.what);
    }
    let census = marked_label_census(path, &model, &doc, &reference_order)?;
    reading.labels = census.labels;
    reading.left_out = census.left_out;
    for owner in &reading.left_out {
        // Whether the symbol it names is one a label read here names too:
        // the same mark, to the last bit.
        let of_a_label = reading
            .labels
            .iter()
            .any(|label| the_same_mark(label.mark_mm, owner.mark_mm));
        println!(
            "   no label: the owner at +{} of {} with its mark at ({:.2}, {:.2}){}: {}{}",
            owner.owner_at,
            owner.symbol,
            owner.mark_mm[0],
            owner.mark_mm[1],
            owner
                .value
                .as_ref()
                .map_or_else(String::new, |value| format!(", text {value:?}")),
            why(owner.why),
            if of_a_label {
                "; its symbol is the symbol of a label read here"
            } else {
                "; no label read here names its symbol"
            },
        );
    }
    for label in &reading.labels {
        if label.listed_text_first() || !label.as_listed() {
            println!(
                "   {}{}",
                says(label),
                if label.as_listed() {
                    ""
                } else {
                    " — AGAINST THE LIST"
                }
            );
        }
    }
    Ok(reading)
}

fn main() -> ExitCode {
    let Some(corpus) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: probe_which_of_a_mark_and_its_ink_a_plot_paints_first <corpus>");
        return ExitCode::from(2);
    };
    let sheets = paired_sheets(&corpus);
    if sheets.is_empty() {
        eprintln!("{}: no .pid with a plot beside it", corpus.display());
        return ExitCode::from(2);
    }
    let mut dirs = font_dirs(Some(&corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    let (mut own, mut another, mut unplaced, mut labels) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Labels::default(),
    );
    let mut read: Vec<(String, Vec<MarkedLabel>)> = Vec::new();
    let mut left_out: Vec<LeftOut> = Vec::new();
    for path in &sheets {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        println!("== {name}");
        match read_sheet(path, &corpus, &dirs) {
            Ok(reading) => {
                let tallied = Labels::of(&reading.labels);
                println!("   a mark on its own label's text: {}", reading.own.says());
                println!(
                    "   a mark on anything else:        {}",
                    reading.another.says()
                );
                if reading.unplaced.places > 0 {
                    println!(
                        "   a side our model lacks:         {}",
                        reading.unplaced.says()
                    );
                }
                println!("   labels: {}", tallied.says());
                if !reading.left_out.is_empty() {
                    println!(
                        "   {}",
                        left_out_says(&reading.left_out.iter().collect::<Vec<_>>())
                    );
                }
                left_out.extend(reading.left_out);
                own.add(reading.own);
                another.add(reading.another);
                unplaced.add(reading.unplaced);
                labels.add(tallied);
                read.push((name, reading.labels));
            }
            Err(error) => println!("   not read: {error}"),
        }
    }
    println!("== all {} sheet(s)", sheets.len());
    println!("   a mark on its own label's text: {}", own.says());
    println!("   a mark on anything else:        {}", another.says());
    println!("   a side our model lacks:         {}", unplaced.says());
    println!("   labels: {}", labels.says());
    println!("   {}", left_out_says(&left_out.iter().collect::<Vec<_>>()));
    // What else a record holds that could say which comes first: its
    // number and its place in the stream. For how many labels each says
    // what the plot paints.
    let all: Vec<&MarkedLabel> = read.iter().flat_map(|(_, labels)| labels).collect();
    let says_what_is_painted = |text_first: fn(&MarkedLabel) -> bool| {
        all.iter()
            .filter(|label| text_first(label) == label.painted_text_first())
            .count()
    };
    println!(
        "   which says what the plot paints, of {} label(s): the sheet's list for {}; the oid \
         of the text's owner against the symbol's for {}; the oid of the text against the \
         symbol's for {}; the owner's place in the stream against the symbol's for {}; the \
         text's place in the stream against the symbol's for {}; \"the mark first\" for {}; \
         \"the text first\" for {}",
        all.len(),
        says_what_is_painted(MarkedLabel::listed_text_first),
        says_what_is_painted(MarkedLabel::owner_numbered_first),
        says_what_is_painted(MarkedLabel::text_numbered_first),
        says_what_is_painted(MarkedLabel::owner_stored_first),
        says_what_is_painted(MarkedLabel::text_stored_first),
        says_what_is_painted(|_| false),
        says_what_is_painted(|_| true),
    );
    // A label painted against its list, and the same label elsewhere: the
    // same text at the same point, listed at the same places.
    for (sheet, sheet_labels) in &read {
        for label in sheet_labels.iter().filter(|label| !label.as_listed()) {
            println!("== against its list, on {sheet}: {}", says(label));
            for (other, other_labels) in read.iter().filter(|(other, _)| other != sheet) {
                for twin in other_labels.iter().filter(|twin| {
                    twin.value == label.value
                        && twin.symbol == label.symbol
                        && apart(twin.mark_mm, label.mark_mm) <= 0.01
                        && twin.symbol_rank == label.symbol_rank
                        && twin.text_rank == label.text_rank
                }) {
                    println!(
                        "   the same label on {other}: {} — {}",
                        says(twin),
                        if twin.as_listed() {
                            "as listed"
                        } else {
                            "against the list too"
                        }
                    );
                }
            }
        }
    }
    ExitCode::SUCCESS
}
