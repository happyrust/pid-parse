//! Every label of a drawing whose symbol carries an origin mark, with where
//! the sheet lists its symbol and its text and where a plot paints them.
//!
//! A label on a sheet is two listed things. Its symbol is an `igSymbol2d`
//! record the sheet lists at a place of its own; the body of a label symbol
//! such as `Remarks.sym` carries an origin mark, a short stroke the plots
//! paint in the paper's colour. Its text is an `igTextBox` owned by a
//! `DependencyObject`, and that object's payload names the symbol's oid at
//! `+16`: the sheet lists the object at another place, right after the
//! symbol or somewhere before it. So the sheet's display order
//! (`pid_parse::parsers::sheet_display_order`) says, label by label, which
//! of the mark and the text is drawn first, and a plot read in painting
//! order (`pid_plot::read::read_plot_sequence`) shows which it painted
//! first: the step of its stroke in the paper's colour through the mark,
//! and the step of the first glyph of the run that spells the text.
//!
//! An owner that names a symbol with a mark and gives no label is kept
//! too, with why ([`NotALabel`]): a second object hanging on a label's
//! symbol that holds no text, or a label the plot does not have where the
//! drawing puts it. So a count of labels says what it left out.
//!
//! Shared by the ink gate, which pins what the eleven reference plots do,
//! and by `probe_which_of_a_mark_and_its_ink_a_plot_paints_first`, which
//! lists it. Included by path from both, so neither uses all of it.
#![allow(dead_code)]

use std::path::Path;

use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape};
use pid_plot::read::PlotSequence;

/// How near a stroke has to pass to a point to be the thing painted there,
/// in millimetres: two pages of one drawing are fitted to hundredths, and a
/// mark is a fifth of a millimetre long.
pub const NEAR_MM: f64 = 0.5;

/// The type word of an `igSymbol2d` record.
const SYMBOL: u16 = 0x00CE;
/// The type word of an `igTextBox` record.
const TEXT_BOX: u16 = 0x004D;
/// The type word of a `DependencyObject` record.
const DEPENDENCY: u16 = 0x00FA;
/// Where a `DependencyObject`'s payload names the object it hangs on.
const DEPENDS_ON_AT: usize = 16;

pub fn apart(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Whether two marks are one: the same point, to the last bit. A mark is
/// read off the symbol's record, so two owners that name one symbol have
/// the same mark.
pub fn the_same_mark(a: [f64; 2], b: [f64; 2]) -> bool {
    a.map(f64::to_bits) == b.map(f64::to_bits)
}

/// How near `point` passes to the straight piece `a`–`b`.
pub fn off_the_piece(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let direction = [b[0] - a[0], b[1] - a[1]];
    let squared = direction[0] * direction[0] + direction[1] * direction[1];
    if squared < 1e-18 {
        return apart(point, a);
    }
    let along = (((point[0] - a[0]) * direction[0] + (point[1] - a[1]) * direction[1]) / squared)
        .clamp(0.0, 1.0);
    apart(
        point,
        [a[0] + along * direction[0], a[1] + along * direction[1]],
    )
}

/// Whether the primitive is stroked in the paper's colour.
pub fn white(primitive: &PaperPrimitive) -> bool {
    primitive
        .stroke
        .as_ref()
        .is_some_and(|stroke| stroke.rgb == [255, 255, 255])
}

/// A point of a stroked primitive: the middle of its first piece.
pub fn a_point_of(primitive: &PaperPrimitive) -> Option<[f64; 2]> {
    let middle = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    match &primitive.shape {
        PaperShape::Line { start_mm, end_mm } => Some(middle(*start_mm, *end_mm)),
        PaperShape::Polyline { points_mm, .. } => {
            Some(middle(*points_mm.first()?, *points_mm.get(1)?))
        }
        _ => None,
    }
}

/// One live record of a sheet stream's chain: where its envelope starts,
/// its type word, its oid, its parent and its payload.
pub struct Record<'a> {
    pub at: usize,
    pub kind: u16,
    pub oid: u32,
    pub parent: u32,
    pub payload: &'a [u8],
}

fn word(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at.checked_add(4)?)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
}

/// The live records of a sheet stream, read off its chain: an eight-byte
/// header, then records end to end, each a type word, a length and a
/// payload that opens with the record's oid and its parent's. A record
/// flagged `0x8000` is a superseded copy and is left out.
pub fn live_records(stream: &[u8]) -> Vec<Record<'_>> {
    let mut records = Vec::new();
    let mut at = 8usize;
    while let (Some(kind), Some(length)) = (
        stream
            .get(at..at + 2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_le_bytes),
        word(stream, at + 2),
    ) {
        let Some(payload) = stream.get(at + 6..at + 6 + length as usize) else {
            break;
        };
        if kind & 0x8000 == 0
            && let (Some(oid), Some(parent)) = (word(payload, 0), word(payload, 4))
        {
            records.push(Record {
                at,
                kind,
                oid,
                parent,
                payload,
            });
        }
        at += 6 + length as usize;
    }
    records
}

/// Whether the text record at `text_at` is the label text of the symbol
/// record at `symbol_at`: its owner is a `DependencyObject` whose payload
/// names the symbol's oid.
pub fn the_symbols_own_text(records: &[Record<'_>], symbol_at: usize, text_at: usize) -> bool {
    let find = |at: usize| records.iter().find(|record| record.at == at);
    let (Some(symbol), Some(text)) = (find(symbol_at), find(text_at)) else {
        return false;
    };
    records.iter().any(|owner| {
        owner.kind == DEPENDENCY
            && owner.oid == text.parent
            && word(owner.payload, DEPENDS_ON_AT) == Some(symbol.oid)
    })
}

/// The step `page` letters `value` at, near `origin`: the first glyph of a
/// run of inked glyphs that spells it, the nearest such run.
pub fn lettered_at(page: &PlotSequence, value: &str, origin: [f64; 2]) -> Option<u32> {
    let wanted: Vec<String> = value
        .chars()
        .filter(|character| !character.is_whitespace() && !character.is_control())
        .map(|character| character.to_string())
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let inked: Vec<_> = page.glyphs.iter().filter(|glyph| !glyph.paper).collect();
    inked
        .windows(wanted.len())
        .filter(|run| {
            run.iter()
                .zip(&wanted)
                .all(|(glyph, text)| glyph.text == *text)
        })
        .map(|run| (apart(run[0].origin_mm, origin), run[0].step))
        .filter(|(off, _)| *off <= 80.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, step)| step)
}

/// The steps `page` strokes in the paper's colour through `point`.
pub fn marked_at(page: &PlotSequence, point: [f64; 2]) -> Vec<u32> {
    let mut steps: Vec<u32> = page
        .strokes
        .iter()
        .filter(|stroke| stroke.paper && !stroke.filled)
        .filter(|stroke| {
            off_the_piece(point, stroke.segment.start_mm, stroke.segment.end_mm) <= NEAR_MM
        })
        .map(|stroke| stroke.step)
        .collect();
    steps.sort_unstable();
    steps.dedup();
    steps
}

/// A label whose symbol carries a mark, on the sheet's list and on a plot.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkedLabel {
    /// The text the label letters.
    pub value: String,
    /// The symbol file the label places, by its file name.
    pub symbol: String,
    /// A point of the mark, paper millimetres.
    pub mark_mm: [f64; 2],
    /// Where the sheet lists the symbol: its place, and the rank within it.
    pub symbol_rank: (u32, u32),
    /// Where the sheet lists the text: the place of its owner, and the
    /// text's rank among the owner's.
    pub text_rank: (u32, u32),
    /// The steps the plot strokes the mark at, in order.
    pub mark_steps: Vec<u32>,
    /// The step the plot letters the first glyph of the text at.
    pub lettered: u32,
    /// The oids of the symbol's record, of the text's owner and of the
    /// text's record: numbers of the drawing's own, which another drawing
    /// holding the same label does not share.
    pub oids: [u32; 3],
    /// Where the same three records start in their stream.
    pub stored_at: [usize; 3],
}

impl MarkedLabel {
    /// Whether the sheet lists the text ahead of the symbol.
    pub fn listed_text_first(&self) -> bool {
        self.text_rank < self.symbol_rank
    }

    /// Whether the sheet lists the text's owner at the place right after
    /// the symbol's.
    pub fn listed_next(&self) -> bool {
        self.text_rank.0 == self.symbol_rank.0 + 1
    }

    /// Whether the plot letters the text ahead of the mark.
    pub fn painted_text_first(&self) -> bool {
        self.mark_steps
            .first()
            .is_some_and(|mark| self.lettered < *mark)
    }

    /// Whether the plot letters the text at the step right after the mark.
    pub fn painted_next(&self) -> bool {
        self.mark_steps
            .last()
            .is_some_and(|mark| self.lettered == mark + 1)
    }

    /// Whether the plot paints the two in the order the sheet lists them.
    pub fn as_listed(&self) -> bool {
        self.listed_text_first() == self.painted_text_first()
    }

    /// Whether the text's owner carries a lower oid than the symbol.
    pub fn owner_numbered_first(&self) -> bool {
        self.oids[1] < self.oids[0]
    }

    /// Whether the text carries a lower oid than the symbol.
    pub fn text_numbered_first(&self) -> bool {
        self.oids[2] < self.oids[0]
    }

    /// Whether the stream holds the text's owner ahead of the symbol.
    pub fn owner_stored_first(&self) -> bool {
        self.stored_at[1] < self.stored_at[0]
    }

    /// Whether the stream holds the text ahead of the symbol.
    pub fn text_stored_first(&self) -> bool {
        self.stored_at[2] < self.stored_at[0]
    }
}

/// Why an owner that names a symbol with a mark gave [`marked_labels`] no
/// label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotALabel {
    /// The owner holds no text box. A label's symbol can have a second
    /// `DependencyObject` hanging on it that holds something else — a
    /// point, on the sheets on hand — and letters nothing.
    NoText,
    /// The page model letters nothing from the owner's text box, so there
    /// is no text to look for on the plot.
    TextNotOnOurPage,
    /// The plot strokes nothing in the paper's colour through the mark:
    /// the label is not where the drawing on hand puts it.
    NoMarkOnThePlot,
    /// The sheet's list ranks the symbol or the text nowhere.
    Unranked,
    /// The plot has no run of glyphs that spells the text.
    TextNotOnThePlot,
}

/// An owner that names a symbol with a mark and is no label of
/// [`marked_labels`], and why.
#[derive(Clone, Debug, PartialEq)]
pub struct LeftOut {
    /// Why it is no label.
    pub why: NotALabel,
    /// Where the owner's record starts in its stream.
    pub owner_at: usize,
    /// The type words of the live records the owner holds, in stream order.
    pub holds: Vec<u16>,
    /// The symbol file the owner names, by its file name.
    pub symbol: String,
    /// A point of the symbol's mark, paper millimetres.
    pub mark_mm: [f64; 2],
    /// The text the label would letter, where the page model letters one.
    pub value: Option<String>,
}

/// What a drawing's owners of marked symbols come to on a plot: the labels
/// read, and the owners that gave none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkedLabels {
    /// Every label whose mark and text the plot paints.
    pub labels: Vec<MarkedLabel>,
    /// Every owner — and, for an owner of several text boxes, every text —
    /// that names a symbol with a mark and gave no label.
    pub left_out: Vec<LeftOut>,
}

/// Every label of the drawing's own sheets whose symbol carries a mark and
/// whose mark and text `plot` paints, in the order the stream holds the
/// text's owners.
///
/// `model` and `doc` are the drawing at `path` laid out and parsed: the
/// model says where a record's mark and text are on the page, the document
/// where the sheet's list ranks the records. A label is left out when its
/// symbol draws no stroke in the paper's colour, when the list ranks one of
/// its two records nowhere, or when the plot has no such stroke or no run
/// of glyphs that spells the text. [`marked_label_census`] is the same
/// reading with what was left out.
pub fn marked_labels(
    path: &Path,
    model: &PaperModel,
    doc: &pid_parse::PidDocument,
    plot: &PlotSequence,
) -> Result<Vec<MarkedLabel>, String> {
    Ok(marked_label_census(path, model, doc, plot)?.labels)
}

/// [`marked_labels`], and with them every owner that names a symbol with a
/// mark and gave no label ([`LeftOut`]).
///
/// The owners counted are the `DependencyObject`s the sheet lists as its
/// own children that name, at payload `+16`, an `igSymbol2d` of the same
/// stream from which the page model draws a stroke in the paper's colour.
/// Each either holds text boxes — and every one of them is a label read or
/// a text left out, for the first of the reasons in [`NotALabel`]'s order
/// that holds — or holds none and is left out once.
pub fn marked_label_census(
    path: &Path,
    model: &PaperModel,
    doc: &pid_parse::PidDocument,
    plot: &PlotSequence,
) -> Result<MarkedLabels, String> {
    let package = pid_parse::PidParser::new()
        .parse_package(path)
        .map_err(|error| error.to_string())?;
    let mut read = MarkedLabels::default();
    for (stream_path, order) in &doc.sheet_display_order {
        let Some(stream) = package.get_stream(stream_path) else {
            continue;
        };
        let records = live_records(&stream.data);
        // What the model drew from a record of this stream.
        let drawn_from = |at: usize| {
            model.primitives.iter().filter(move |primitive| {
                !primitive.furniture
                    && primitive.stream_path.as_deref() == Some(stream_path.as_str())
                    && primitive.byte_range.is_some_and(|range| range[0] == at)
            })
        };
        let rank = |at: usize| order.ranks.get(&at).map(|rank| (rank.place, rank.inner));
        for owner in records
            .iter()
            .filter(|record| record.kind == DEPENDENCY && record.parent == order.sheet_oid)
        {
            let Some(symbol) = word(owner.payload, DEPENDS_ON_AT).and_then(|oid| {
                records
                    .iter()
                    .find(|record| record.oid == oid && record.kind == SYMBOL)
            }) else {
                continue;
            };
            let Some(mark_mm) = drawn_from(symbol.at)
                .filter(|primitive| white(primitive))
                .find_map(a_point_of)
            else {
                continue;
            };
            let symbol_name = drawn_from(symbol.at)
                .find_map(|primitive| primitive.symbol.as_deref())
                .map_or_else(String::new, |symbol| {
                    symbol
                        .rsplit(['\\', '/'])
                        .next()
                        .unwrap_or(symbol)
                        .to_string()
                });
            let holds: Vec<u16> = records
                .iter()
                .filter(|record| record.parent == owner.oid)
                .map(|record| record.kind)
                .collect();
            let left_out = |why: NotALabel, value: Option<String>| LeftOut {
                why,
                owner_at: owner.at,
                holds: holds.clone(),
                symbol: symbol_name.clone(),
                mark_mm,
                value,
            };
            let texts: Vec<&Record<'_>> = records
                .iter()
                .filter(|record| record.kind == TEXT_BOX && record.parent == owner.oid)
                .collect();
            if texts.is_empty() {
                read.left_out.push(left_out(NotALabel::NoText, None));
                continue;
            }
            let mark_steps = marked_at(plot, mark_mm);
            let symbol_rank = rank(symbol.at);
            for text in texts {
                let Some((value, origin)) =
                    drawn_from(text.at).find_map(|primitive| match &primitive.shape {
                        PaperShape::Text {
                            value, origin_mm, ..
                        } => Some((value.clone(), *origin_mm)),
                        _ => None,
                    })
                else {
                    read.left_out
                        .push(left_out(NotALabel::TextNotOnOurPage, None));
                    continue;
                };
                if mark_steps.is_empty() {
                    read.left_out
                        .push(left_out(NotALabel::NoMarkOnThePlot, Some(value)));
                    continue;
                }
                let (Some(symbol_rank), Some(text_rank)) = (symbol_rank, rank(text.at)) else {
                    read.left_out
                        .push(left_out(NotALabel::Unranked, Some(value)));
                    continue;
                };
                let Some(lettered) = lettered_at(plot, &value, origin) else {
                    read.left_out
                        .push(left_out(NotALabel::TextNotOnThePlot, Some(value)));
                    continue;
                };
                read.labels.push(MarkedLabel {
                    value,
                    symbol: symbol_name.clone(),
                    mark_mm,
                    symbol_rank,
                    text_rank,
                    mark_steps: mark_steps.clone(),
                    lettered,
                    oids: [symbol.oid, owner.oid, text.oid],
                    stored_at: [symbol.at, owner.at, text.at],
                });
            }
        }
    }
    Ok(read)
}
