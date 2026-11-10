//! What two plots print, where [`crate::compare`] cannot say.
//!
//! The comparison pairs paint with paint: a path stroked in the paper's own
//! white is a stroke to it like any other, a line that runs along the sheet
//! is left out of both pages before it starts, and a stroke has a width but
//! no cap, no join and no colour. Two pages can therefore read as the same
//! page and print differently — the eleven `验收数据` sheets did, for the
//! whole of their title blocks
//! (`docs/analysis/2026-11-10-the-acceptance-gate-reads-paint-not-ink.md`).
//!
//! This module reads a page for what it prints ([`PageInk`]) and takes the
//! readings that close those gaps, each against a reference page:
//!
//! * [`stroke_colours`] and [`label_colours`] — of the strokes and labels the
//!   two pages share, which print in the same colour and which one page inks
//!   where the other only paints the paper;
//! * [`sheet_edges`] — the strokes along the sheet, paired by where they lie
//!   and held to their widths;
//! * [`ends_apart`] — how much of our ink is stroked with another cap or
//!   another join than the reference's stroke at the same place.
//!
//! The comparison of ink itself — coverage, extra ink, label pairing, pen
//! agreement — stays [`crate::compare`]'s: it is run on the two pages' ink
//! ([`PageInk::ink`]) instead of their paint, and its report is what the
//! colour readings start from.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::compare::{CompareReport, Fit};
use crate::read::{
    InkSegment, LineCap, LineJoin, Painted, PlotInk, PlotPens, PlotReadError, SheetEdge,
    read_plot_painted, read_plot_pens,
};

/// How far off a reference stroke's own line a stroke of ours may lie and
/// still be on it, paper millimetres: the comparison's own tolerance for a
/// stroke of the same pen (`PEN_SAME_PEN_ON_MM`), which is what the plotter's
/// 1/150 in grid and a coarser chord leave between two drawings of one line.
const ON_MM: f64 = 0.20;
/// How far apart the points checked along a stroke are, paper millimetres:
/// the comparison's `PEN_CHAIN_STEP_MM`.
const STEP_MM: f64 = 0.05;
/// How far a reference stroke may turn from ours and still run along it, as
/// the absolute cosine of their angle: the comparison's `PEN_CHAIN_DOT`.
const ALONG_DOT: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// The shortest stroke that is held to a colour, paper millimetres: the
/// comparison's `PEN_MIN_MM`. A shorter one is a dash stub.
const MIN_STROKE_MM: f64 = 0.2;
/// How far apart two runs of the same text may start and be the same label,
/// paper millimetres: the comparison's `TEXT_MM`.
const LABEL_ON_MM: f64 = 1.0;
/// How far apart, across their length, two strokes along the sheet may lie
/// and be the same edge, paper millimetres.
const EDGE_ACROSS_MM: f64 = 0.2;
/// How far apart the ends of two strokes along the sheet may lie and be the
/// same edge, paper millimetres.
///
/// The plotter ends a polyline one device unit (0.169 mm) past its last
/// vertex, and a sheet edge of the A2 reference stops at its paper's edge
/// where ours runs on to the sheet's: 0.45 mm at one end.
const EDGE_END_MM: f64 = 1.0;
/// How far apart two widths may be and be the same pen, paper millimetres:
/// under a quarter of the step between the plotter's two thinnest pens.
const EDGE_WIDTH_MM: f64 = 0.02;
/// How long the parts are a stroke is read in when its cap and join are held
/// to the reference's, paper millimetres. A cap and a join are a whole
/// path's, so a stroke need not be read finer than the shortest dash a page
/// draws (0.35 mm).
const ENDS_STEP_MM: f64 = 0.25;

/// One page read for what it prints.
#[derive(Clone, Debug, PartialEq)]
pub struct PageInk {
    /// What leaves ink: every path and glyph not painted in the paper's own
    /// colour ([`Painted::Inked`]).
    pub ink: PlotInk,
    /// What is painted in the paper's own colour and prints nothing
    /// ([`Painted::Paper`]).
    pub paper: PlotInk,
    /// What the page's pens state beside: the strokes along the sheet, the
    /// caps and joins, the colours.
    pub pens: PlotPens,
}

impl PageInk {
    /// Read the first page of a PDF.
    ///
    /// # Errors
    ///
    /// The bytes are not a PDF the reader can open, or it has no page.
    pub fn read(pdf_bytes: &[u8]) -> Result<Self, PlotReadError> {
        Ok(Self {
            ink: read_plot_painted(pdf_bytes, Painted::Inked)?,
            paper: read_plot_painted(pdf_bytes, Painted::Paper)?,
            pens: read_plot_pens(pdf_bytes)?,
        })
    }
}

/// Our strokes, by what they and the reference's stroke at the same place
/// print.
///
/// A stroke is filed under the reference stroke of its own colour when there
/// is one at its place, and under the other colour only when there is not:
/// the reference draws a label's origin mark in the paper's colour on top of
/// the box edge it touches, and our edge there is on ink, not on paper.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StrokeColours {
    /// Inked strokes of ours on an inked stroke of the reference: the pen
    /// pairing of the two pages' ink.
    pub ink_on_ink: usize,
    /// Strokes of ours in the paper's colour on a reference stroke in the
    /// paper's colour: neither page prints anything there.
    pub paper_on_paper: usize,
    /// Inked strokes of ours where the reference paints only the paper: ink
    /// the reference does not print.
    pub ink_on_paper: usize,
    /// Strokes of ours in the paper's colour where the reference only inks:
    /// ink of the reference's we leave blank.
    pub paper_on_ink: usize,
    /// Inked strokes of ours with no reference stroke at their place in
    /// either colour.
    pub ink_alone: usize,
    /// Strokes of ours in the paper's colour with no reference stroke at
    /// their place in either colour.
    pub paper_alone: usize,
}

impl StrokeColours {
    /// Strokes both pages have and print alike.
    #[must_use]
    pub fn same(&self) -> usize {
        self.ink_on_ink + self.paper_on_paper
    }

    /// Strokes both pages have, which one inks and the other does not.
    #[must_use]
    pub fn other(&self) -> usize {
        self.ink_on_paper + self.paper_on_ink
    }
}

/// File our strokes by colour against the reference's.
///
/// `inked` is the comparison of the two pages' ink — `reference.ink` with
/// `ours.ink` — and its pen pairing is the first cell: an inked stroke of
/// ours it pairs is on reference ink. The strokes it leaves unpaired, and our
/// strokes in the paper's colour, which that comparison never saw, are held
/// against the reference's strokes point by point the way the comparison
/// holds a stroke against a chain: every point of the stroke, checked every
/// 0.05 mm, within 0.20 mm of a reference stroke running along it.
#[must_use]
pub fn stroke_colours(reference: &PageInk, ours: &PageInk, inked: &CompareReport) -> StrokeColours {
    let on_paper = Along::new(&reference.paper.segments);
    let on_ink = Along::new(&reference.ink.segments);
    let mut colours = StrokeColours {
        ink_on_ink: inked.pen.paired,
        ..StrokeColours::default()
    };
    // Already in the reference's frame, and none shorter than a stroke.
    for stroke in &inked.pen.unpaired_strokes {
        if on_paper.carries(stroke.start_mm, stroke.end_mm) {
            colours.ink_on_paper += 1;
        } else {
            colours.ink_alone += 1;
        }
    }
    for segment in &ours.paper.segments {
        let start = into_reference(segment.start_mm, &inked.fit);
        let end = into_reference(segment.end_mm, &inked.fit);
        if hypot(start, end) < MIN_STROKE_MM {
            continue;
        }
        if on_paper.carries(start, end) {
            colours.paper_on_paper += 1;
        } else if on_ink.carries(start, end) {
            colours.paper_on_ink += 1;
        } else {
            colours.paper_alone += 1;
        }
    }
    colours
}

/// The labels the two pages share, by what each page letters them in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LabelColours {
    /// Labels both pages letter in ink: the label pairing of the two pages'
    /// ink.
    pub ink_on_ink: usize,
    /// The characters of those labels as the reference letters them,
    /// whitespace not counted.
    pub glyphs_ink_on_ink: usize,
    /// Runs of ours in the paper's colour where the reference letters the
    /// same text in the paper's colour.
    pub paper_on_paper: usize,
    /// Inked runs of ours where the reference letters the same text only in
    /// the paper's colour.
    pub ink_on_paper: usize,
    /// Runs of ours in the paper's colour where the reference inks the same
    /// text.
    pub paper_on_ink: usize,
    /// Runs lettered in the paper's colour, blanks not counted: the
    /// reference's, then ours.
    pub paper_runs: [usize; 2],
}

impl LabelColours {
    /// Labels both pages have and letter alike.
    #[must_use]
    pub fn same(&self) -> usize {
        self.ink_on_ink + self.paper_on_paper
    }

    /// Labels both pages have, which one inks and the other does not.
    #[must_use]
    pub fn other(&self) -> usize {
        self.ink_on_paper + self.paper_on_ink
    }
}

/// File the labels the two pages share by colour.
///
/// `inked` is the comparison of the two pages' ink, as for
/// [`stroke_colours`]: the labels it pairs are lettered in ink on both pages.
/// A run in the paper's colour is no label of that comparison, so each of
/// ours is held here against the reference's runs — the same text, started
/// within a millimetre — in its own colour first, and each inked run of ours
/// the comparison left unpaired against the reference's runs in the paper's
/// colour.
#[must_use]
pub fn label_colours(reference: &PageInk, ours: &PageInk, inked: &CompareReport) -> LabelColours {
    let runs = |page: &PlotInk| -> Vec<(String, [f64; 2])> {
        page.spans
            .iter()
            .map(|span| (normalize(&span.text), span.origin_mm))
            .filter(|(text, _)| !text.is_empty())
            .collect()
    };
    let reference_paper = runs(&reference.paper);
    let reference_ink = runs(&reference.ink);
    let our_paper = runs(&ours.paper);
    let lettered = |runs: &[(String, [f64; 2])], text: &str, at: [f64; 2]| {
        runs.iter()
            .any(|(other, origin)| other == text && hypot(*origin, at) <= LABEL_ON_MM)
    };
    let mut colours = LabelColours {
        ink_on_ink: inked.text_matched,
        glyphs_ink_on_ink: inked
            .text_pairs
            .iter()
            .flat_map(|pair| &pair.reference_runs)
            .filter_map(|index| reference.ink.spans.get(*index))
            .map(|span| span.text.chars().filter(|ch| !ch.is_whitespace()).count())
            .sum(),
        paper_runs: [reference_paper.len(), our_paper.len()],
        ..LabelColours::default()
    };
    for (text, origin) in &our_paper {
        let at = into_reference(*origin, &inked.fit);
        if lettered(&reference_paper, text, at) {
            colours.paper_on_paper += 1;
        } else if lettered(&reference_ink, text, at) {
            colours.paper_on_ink += 1;
        }
    }
    for run in &inked.unmatched_our_text {
        let at = into_reference(run.origin_mm, &inked.fit);
        if lettered(&reference_paper, &normalize(&run.text), at) {
            colours.ink_on_paper += 1;
        }
    }
    colours
}

/// One of our sheet edges the reference prints at another width.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EdgeWidths {
    /// Our edge, in the reference's paper millimetres.
    pub ours: SheetEdge,
    /// The width the reference prints the same edge at, millimetres.
    pub reference_width_mm: f64,
}

/// The inked strokes along the sheet of two pages, paired by where they lie.
///
/// [`crate::read::read_plot`] leaves these out of both pages, so they are in
/// no coverage, no extra ink and no pen pairing. An edge painted in the
/// paper's colour prints nothing and is not one of them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SheetEdgePairs {
    /// Inked sheet edges of the reference.
    pub reference: usize,
    /// Inked sheet edges of ours.
    pub ours: usize,
    /// Edges both pages print at the same place and the same width.
    pub same: usize,
    /// Edges both pages print at the same place and at different widths.
    pub other_width: Vec<EdgeWidths>,
    /// Edges of ours with none of the reference's at their place, in the
    /// reference's paper millimetres.
    pub ours_only: Vec<SheetEdge>,
    /// Edges of the reference's with none of ours at their place.
    pub reference_only: Vec<SheetEdge>,
}

impl SheetEdgePairs {
    /// Edges one page lacks or the two print at different widths.
    #[must_use]
    pub fn apart(&self) -> usize {
        self.other_width.len() + self.ours_only.len() + self.reference_only.len()
    }
}

/// Pair the two pages' inked sheet edges.
///
/// `fit` is how our page sits on the reference ([`CompareReport::fit`]). Two
/// edges are the same edge when they run the same way, lie within 0.2 mm of
/// each other across their length, and end within 1 mm of each other; each
/// edge of ours takes the nearest reference edge no other has taken.
#[must_use]
pub fn sheet_edges(reference: &PlotPens, ours: &PlotPens, fit: &Fit) -> SheetEdgePairs {
    let reference_edges: Vec<&SheetEdge> = reference
        .sheet_edges
        .iter()
        .filter(|edge| !edge.paper)
        .collect();
    let our_edges: Vec<SheetEdge> = ours
        .sheet_edges
        .iter()
        .filter(|edge| !edge.paper)
        .map(|edge| SheetEdge {
            start_mm: into_reference(edge.start_mm, fit),
            end_mm: into_reference(edge.end_mm, fit),
            ..edge.clone()
        })
        .collect();
    let mut pairs = SheetEdgePairs {
        reference: reference_edges.len(),
        ours: our_edges.len(),
        ..SheetEdgePairs::default()
    };
    let mut taken = vec![false; reference_edges.len()];
    for edge in our_edges {
        let partner = reference_edges
            .iter()
            .enumerate()
            .filter(|(index, _)| !taken[*index])
            .filter_map(|(index, other)| same_edge(&edge, other).map(|across| (across, index)))
            .min_by(|left, right| left.0.total_cmp(&right.0));
        match partner {
            Some((_, index)) => {
                taken[index] = true;
                let reference_width_mm = reference_edges[index].width_mm;
                if (reference_width_mm - edge.width_mm).abs() <= EDGE_WIDTH_MM {
                    pairs.same += 1;
                } else {
                    pairs.other_width.push(EdgeWidths {
                        ours: edge,
                        reference_width_mm,
                    });
                }
            }
            None => pairs.ours_only.push(edge),
        }
    }
    pairs.reference_only = reference_edges
        .iter()
        .zip(&taken)
        .filter(|(_, taken)| !**taken)
        .map(|(edge, _)| (*edge).clone())
        .collect();
    pairs
}

/// How far apart two edges lie across their length when they are the same
/// edge, or `None` when they are not.
fn same_edge(ours: &SheetEdge, reference: &SheetEdge) -> Option<f64> {
    let length = hypot(reference.start_mm, reference.end_mm);
    let our_length = hypot(ours.start_mm, ours.end_mm);
    if length < 1e-9 || our_length < 1e-9 {
        return None;
    }
    let unit = [
        (reference.end_mm[0] - reference.start_mm[0]) / length,
        (reference.end_mm[1] - reference.start_mm[1]) / length,
    ];
    let ours_unit = [
        (ours.end_mm[0] - ours.start_mm[0]) / our_length,
        (ours.end_mm[1] - ours.start_mm[1]) / our_length,
    ];
    if (unit[0] * ours_unit[0] + unit[1] * ours_unit[1]).abs() < 0.99 {
        return None;
    }
    // Each end of ours along and across the reference's line.
    let place = |point: [f64; 2]| {
        let delta = [
            point[0] - reference.start_mm[0],
            point[1] - reference.start_mm[1],
        ];
        (
            delta[0] * unit[0] + delta[1] * unit[1],
            (delta[0] * unit[1] - delta[1] * unit[0]).abs(),
        )
    };
    let (start_along, start_across) = place(ours.start_mm);
    let (end_along, end_across) = place(ours.end_mm);
    let across = start_across.max(end_across);
    if across > EDGE_ACROSS_MM {
        return None;
    }
    let (near, far) = (start_along.min(end_along), start_along.max(end_along));
    (near.abs() <= EDGE_END_MM && (far - length).abs() <= EDGE_END_MM).then_some(across)
}

/// How a stroke ends and how it turns its corners.
pub type Ends = (LineCap, LineJoin);

/// Ink of ours under one cap and join where the reference's stroke at the
/// same place is under another.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EndsPair {
    /// The cap and join we stroke it with.
    pub ours: Ends,
    /// The cap and join the reference strokes that place with.
    pub reference: Ends,
    /// How much of our ink, millimetres.
    pub length_mm: f64,
}

/// Our inked strokes, by whether the reference's stroke at the same place
/// ends and turns its corners the way ours does.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EndsApart {
    /// Millimetres of our ink on a reference stroke with the same cap and
    /// the same join.
    pub same_mm: f64,
    /// Millimetres of our ink on a reference stroke, none there with our
    /// cap and join.
    pub other_mm: f64,
    /// Millimetres of our ink with no reference stroke at its place: extra
    /// ink, whose ends have nothing to be held to.
    pub alone_mm: f64,
    /// [`Self::other_mm`] by the two pages' caps and joins, ascending.
    pub apart: Vec<EndsPair>,
}

/// Hold the cap and join of each stroke of ours to those of the reference's
/// stroke at the same place.
///
/// The comparison reads centre lines, so a page of butt-ended strokes and a
/// page of round-ended ones are the same page to it. A round cap prints half
/// a pen's width past each end and a round join fills the outside of each
/// corner; on a 0.7 mm main that is a third of a millimetre of ink at every
/// end, and every dash has two.
///
/// Each printed piece of ours ([`crate::read::StrokeEnds::strokes`], a sheet
/// edge included), carried into the reference's frame by `fit`, is read every
/// quarter millimetre: the point is filed under the reference pieces within
/// 0.20 mm of it that run within 45° of ours — *same* when one of them has
/// our cap and join, *other* under the nearest one when none has, *alone*
/// when there is none. A count of the pairs each page uses cannot do this: a
/// page can stroke with the reference's pairs and put them on the wrong
/// strokes.
#[must_use]
pub fn ends_apart(reference: &PlotPens, ours: &PlotPens, fit: &Fit) -> EndsApart {
    let mut pieces: Vec<InkSegment> = Vec::new();
    let mut ends_of: Vec<Ends> = Vec::new();
    for ends in &reference.ends {
        pieces.extend(ends.strokes.iter().cloned());
        ends_of.extend(std::iter::repeat_n(
            (ends.cap, ends.join),
            ends.strokes.len(),
        ));
    }
    let along = Along::new(&pieces);
    let mut apart = EndsApart::default();
    // Our ends, then the reference's.
    let mut by_pair: BTreeMap<(Ends, Ends), f64> = BTreeMap::new();
    for ends in &ours.ends {
        let mine = (ends.cap, ends.join);
        for stroke in &ends.strokes {
            let start = into_reference(stroke.start_mm, fit);
            let end = into_reference(stroke.end_mm, fit);
            let length = hypot(start, end);
            if length < 1e-9 || !length.is_finite() {
                continue;
            }
            let unit = [(end[0] - start[0]) / length, (end[1] - start[1]) / length];
            let parts = ((length / ENDS_STEP_MM).ceil() as usize).max(1);
            let part_mm = length / parts as f64;
            for part in 0..parts {
                // The middle of each part stands for the part.
                let t = (part as f64 + 0.5) / parts as f64;
                let point = [
                    start[0] + (end[0] - start[0]) * t,
                    start[1] + (end[1] - start[1]) * t,
                ];
                let mut same = false;
                let mut nearest: Option<(f64, Ends)> = None;
                for (index, distance) in along.at(point, unit) {
                    let theirs = ends_of[index];
                    if theirs == mine {
                        same = true;
                        break;
                    }
                    if nearest.is_none_or(|(least, _)| distance < least) {
                        nearest = Some((distance, theirs));
                    }
                }
                match (same, nearest) {
                    (true, _) => apart.same_mm += part_mm,
                    (false, Some((_, theirs))) => {
                        apart.other_mm += part_mm;
                        *by_pair.entry((mine, theirs)).or_default() += part_mm;
                    }
                    (false, None) => apart.alone_mm += part_mm,
                }
            }
        }
    }
    apart.apart = by_pair
        .into_iter()
        .map(|((ours, reference), length_mm)| EndsPair {
            ours,
            reference,
            length_mm,
        })
        .collect();
    apart
}

/// A point of our page in the reference's paper millimetres: the inverse of
/// `our ≈ scale × reference + translation`.
fn into_reference(point: [f64; 2], fit: &Fit) -> [f64; 2] {
    [
        (point[0] - fit.translation_mm[0]) / fit.scale,
        (point[1] - fit.translation_mm[1]) / fit.scale,
    ]
}

fn hypot(start: [f64; 2], end: [f64; 2]) -> f64 {
    (end[0] - start[0]).hypot(end[1] - start[1])
}

/// Whitespace collapsed, as the comparison matches text.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn dist_point_seg(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length2 = dx * dx + dy * dy;
    let t = if length2 <= 1e-18 {
        0.0
    } else {
        (((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / length2).clamp(0.0, 1.0)
    };
    (point[0] - (start[0] + dx * t)).hypot(point[1] - (start[1] + dy * t))
}

/// Segments on a grid, asked whether a stroke lies along them.
struct Along<'a> {
    segments: &'a [InkSegment],
    cells: HashMap<(i64, i64), Vec<usize>>,
}

/// Grid cell, paper millimetres.
const CELL_MM: f64 = 2.0;
/// How far apart the points a segment is filed under are, paper millimetres.
const FILE_STEP_MM: f64 = 0.25;

#[allow(clippy::cast_possible_truncation)]
fn cell(value: f64) -> i64 {
    (value / CELL_MM).floor() as i64
}

impl<'a> Along<'a> {
    fn new(segments: &'a [InkSegment]) -> Self {
        // A segment is filed under every cell within reach of it: each point
        // along it, [`FILE_STEP_MM`] apart, claims the cells a point
        // [`ON_MM`] off the segment and half a step along it could fall in.
        // Walking it, rather than claiming its bounding box, keeps a
        // page-long diagonal out of the cells it never comes near.
        let reach = ON_MM + FILE_STEP_MM / 2.0;
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, segment) in segments.iter().enumerate() {
            let length = hypot(segment.start_mm, segment.end_mm);
            if !length.is_finite() {
                continue;
            }
            let steps = ((length / FILE_STEP_MM).ceil() as usize).max(1);
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let x = segment.start_mm[0] + (segment.end_mm[0] - segment.start_mm[0]) * t;
                let y = segment.start_mm[1] + (segment.end_mm[1] - segment.start_mm[1]) * t;
                for column in cell(x - reach)..=cell(x + reach) {
                    for row in cell(y - reach)..=cell(y + reach) {
                        // One segment's claims are made together, so its
                        // index is the bucket's last entry when it is in.
                        let bucket = cells.entry((column, row)).or_default();
                        if bucket.last() != Some(&index) {
                            bucket.push(index);
                        }
                    }
                }
            }
        }
        Self { segments, cells }
    }

    /// The segments `point` lies on that run along the direction `unit`:
    /// each one's index and how far off its line the point is.
    fn at(&self, point: [f64; 2], unit: [f64; 2]) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.cells
            .get(&(cell(point[0]), cell(point[1])))
            .into_iter()
            .flatten()
            .filter_map(move |index| {
                let segment = &self.segments[*index];
                let along = [
                    segment.end_mm[0] - segment.start_mm[0],
                    segment.end_mm[1] - segment.start_mm[1],
                ];
                let along_length = along[0].hypot(along[1]);
                if along_length < 1e-9
                    || ((along[0] * unit[0] + along[1] * unit[1]) / along_length).abs() < ALONG_DOT
                {
                    return None;
                }
                let distance = dist_point_seg(point, segment.start_mm, segment.end_mm);
                (distance <= ON_MM).then_some((*index, distance))
            })
    }

    /// Whether every point of `start` → `end` lies on one of the segments
    /// that runs along it.
    fn carries(&self, start: [f64; 2], end: [f64; 2]) -> bool {
        let length = hypot(start, end);
        if length < 1e-9 {
            return false;
        }
        let unit = [(end[0] - start[0]) / length, (end[1] - start[1]) / length];
        let steps = ((length / STEP_MM).ceil() as usize).max(1);
        (0..=steps).all(|step| {
            let t = step as f64 / steps as f64;
            let point = [
                start[0] + (end[0] - start[0]) * t,
                start[1] + (end[1] - start[1]) * t,
            ];
            self.at(point, unit).next().is_some()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PageInk, SheetEdgePairs, ends_apart, label_colours, sheet_edges, stroke_colours};
    use crate::compare::{Fit, compare_plots};
    use crate::read::{
        InkSegment, LineCap, LineJoin, PlotInk, PlotPens, SheetEdge, StrokeEnds, TextSpan,
    };

    const PAGE: [f64; 2] = [200.0, 100.0];

    fn stroke(start: [f64; 2], end: [f64; 2]) -> InkSegment {
        InkSegment {
            start_mm: start,
            end_mm: end,
            width_mm: 0.169,
        }
    }

    fn run(text: &str, origin: [f64; 2]) -> TextSpan {
        TextSpan {
            text: text.to_string(),
            origin_mm: origin,
            height_mm: 3.0,
            advance_mm: 2.0 * text.chars().count() as f64,
            direction: [1.0, 0.0],
        }
    }

    fn pens() -> PlotPens {
        PlotPens {
            page_mm: PAGE,
            sheet_edges: Vec::new(),
            ends: Vec::new(),
            inks: Vec::new(),
        }
    }

    fn page(
        ink: Vec<InkSegment>,
        paper: Vec<InkSegment>,
        ink_runs: Vec<TextSpan>,
        paper_runs: Vec<TextSpan>,
    ) -> PageInk {
        PageInk {
            ink: PlotInk {
                page_mm: PAGE,
                segments: ink,
                spans: ink_runs,
            },
            paper: PlotInk {
                page_mm: PAGE,
                segments: paper,
                spans: paper_runs,
            },
            pens: pens(),
        }
    }

    fn identity() -> Fit {
        Fit {
            scale: 1.0,
            translation_mm: [0.0, 0.0],
            pairs: 0,
            within_threshold: true,
            finding: None,
        }
    }

    /// The reference inks a box edge and a pipe and paints a label's origin
    /// mark in the paper's colour; what each stroke of ours is filed under
    /// follows from what lies at its place.
    #[test]
    fn a_stroke_is_filed_by_what_the_reference_prints_at_its_place() {
        let edge = stroke([10.0, 10.0], [60.0, 10.0]);
        let pipe = stroke([10.0, 30.0], [60.0, 30.0]);
        let mark = stroke([80.0, 50.0], [82.0, 50.0]);
        let reference = page(
            vec![edge.clone(), pipe.clone()],
            vec![mark.clone()],
            vec![],
            vec![],
        );
        let ours = page(
            vec![
                // On the reference's edge: ink on ink.
                edge.clone(),
                // On the reference's mark, which is only paint: ink we print
                // and the reference does not.
                mark.clone(),
                // Nowhere near anything.
                stroke([150.0, 80.0], [160.0, 80.0]),
            ],
            vec![
                // On the reference's mark, in the paper's colour as it is.
                stroke([80.0, 50.05], [82.0, 50.05]),
                // On the reference's pipe: we leave blank what it inks.
                stroke([20.0, 30.0], [50.0, 30.0]),
                // Nowhere near anything.
                stroke([150.0, 20.0], [160.0, 20.0]),
                // Too short to be held to a colour.
                stroke([100.0, 90.0], [100.1, 90.0]),
            ],
            vec![],
            vec![],
        );
        let inked = compare_plots(&reference.ink, &ours.ink, &[]);
        assert_eq!(inked.pen.paired, 1, "{:?}", inked.pen);
        assert_eq!(inked.pen.unpaired, 2, "{:?}", inked.pen);

        let colours = stroke_colours(&reference, &ours, &inked);
        assert_eq!(colours.ink_on_ink, 1);
        assert_eq!(colours.ink_on_paper, 1);
        assert_eq!(colours.ink_alone, 1);
        assert_eq!(colours.paper_on_paper, 1);
        assert_eq!(colours.paper_on_ink, 1);
        assert_eq!(colours.paper_alone, 1);
        assert_eq!(colours.same(), 2);
        assert_eq!(colours.other(), 2);
    }

    /// The reference draws an origin mark in the paper's colour over the box
    /// edge it touches. Our mark there is on the reference's mark, and our
    /// edge is on the reference's edge: neither is a colour apart.
    #[test]
    fn a_mark_drawn_over_an_edge_is_filed_under_its_own_colour() {
        let edge = stroke([10.0, 10.0], [60.0, 10.0]);
        let mark = stroke([30.0, 10.0], [32.0, 10.0]);
        let reference = page(vec![edge.clone()], vec![mark.clone()], vec![], vec![]);
        let ours = page(vec![edge], vec![mark], vec![], vec![]);
        let inked = compare_plots(&reference.ink, &ours.ink, &[]);
        let colours = stroke_colours(&reference, &ours, &inked);
        assert_eq!(colours.ink_on_ink, 1);
        assert_eq!(colours.paper_on_paper, 1);
        assert_eq!(colours.other(), 0, "{colours:?}");
    }

    /// A stroke that turns across the reference's, or leaves it part of the
    /// way, is not on it.
    #[test]
    fn a_stroke_across_a_reference_stroke_is_not_on_it() {
        let reference = page(
            vec![],
            vec![stroke([10.0, 10.0], [20.0, 10.0])],
            vec![],
            vec![],
        );
        let ours = page(
            vec![],
            vec![
                // Crosses it at a right angle.
                stroke([15.0, 9.0], [15.0, 11.0]),
                // Starts on it and runs on 5 mm past its end.
                stroke([15.0, 10.0], [25.0, 10.0]),
                // Runs beside it, 0.3 mm off.
                stroke([12.0, 10.3], [18.0, 10.3]),
            ],
            vec![],
            vec![],
        );
        let inked = compare_plots(&reference.ink, &ours.ink, &[]);
        let colours = stroke_colours(&reference, &ours, &inked);
        assert_eq!(colours.paper_on_paper, 0, "{colours:?}");
        assert_eq!(colours.paper_alone, 3, "{colours:?}");
    }

    #[test]
    fn a_label_is_filed_by_what_the_reference_letters_it_in() {
        let reference = page(
            vec![],
            vec![],
            vec![run("PT 0101", [10.0, 10.0]), run("TT", [10.0, 30.0])],
            vec![run("hidden", [10.0, 50.0]), run("ghost", [10.0, 70.0])],
        );
        let ours = page(
            vec![],
            vec![],
            vec![
                // Paired by the comparison of ink.
                run("PT 0101", [10.1, 10.0]),
                // The reference letters this only in the paper's colour.
                run("hidden", [10.0, 50.2]),
            ],
            vec![
                // The reference inks this one.
                run("TT", [10.0, 30.0]),
                // And letters this one in the paper's colour too.
                run("ghost", [10.3, 70.0]),
                // A blank run names no label.
                run("   ", [10.0, 90.0]),
            ],
        );
        let inked = compare_plots(&reference.ink, &ours.ink, &[]);
        assert_eq!(inked.text_matched, 1, "{:?}", inked.unmatched_our_text);
        let colours = label_colours(&reference, &ours, &inked);
        assert_eq!(colours.ink_on_ink, 1);
        assert_eq!(colours.glyphs_ink_on_ink, 6, "PT 0101 without its space");
        assert_eq!(colours.paper_on_paper, 1);
        assert_eq!(colours.ink_on_paper, 1);
        assert_eq!(colours.paper_on_ink, 1);
        assert_eq!(colours.paper_runs, [2, 2]);
        assert_eq!(colours.same(), 2);
        assert_eq!(colours.other(), 2);
    }

    fn edge(start: [f64; 2], end: [f64; 2], width_mm: f64, paper: bool) -> SheetEdge {
        SheetEdge {
            start_mm: start,
            end_mm: end,
            width_mm,
            paper,
        }
    }

    /// The A2 sheet in small: the reference prints three thin sheet edges
    /// and a heavy inner frame line; ours prints the three, a fourth the
    /// reference's paper is too short for, and the frame line with a lighter
    /// pen; and each page paints one edge in the paper's colour.
    #[test]
    fn sheet_edges_pair_by_place_and_are_held_to_their_widths() {
        let mut reference = pens();
        reference.sheet_edges = vec![
            edge([0.169, 0.508], [199.8, 0.508], 0.169, false),
            edge([199.8, 0.508], [199.8, 99.6], 0.169, false),
            edge([0.169, 99.6], [0.169, 0.508], 0.169, false),
            edge([10.0, 5.0], [190.0, 5.0], 1.355, false),
            // Painted in the paper's colour: not an edge that prints.
            edge([10.0, 95.0], [190.0, 95.0], 0.169, true),
        ];
        let mut ours = pens();
        // Our page sits 0.1 mm right of and 0.05 mm above the reference's.
        let fit = Fit {
            translation_mm: [0.1, 0.05],
            ..identity()
        };
        let moved = |x: f64, y: f64| [x + 0.1, y + 0.05];
        ours.sheet_edges = vec![
            edge(moved(0.2, 0.5), moved(199.8, 0.5), 0.169, false),
            // Drawn the other way round, and half a millimetre longer.
            edge(moved(199.8, 100.1), moved(199.8, 0.5), 0.169, false),
            edge(moved(0.2, 0.5), moved(0.2, 100.1), 0.169, false),
            // The top edge the reference's paper cuts off.
            edge(moved(0.2, 100.1), moved(199.8, 100.1), 0.169, false),
            // The inner frame line, with the wide pen instead of the heavy.
            edge(moved(10.0, 5.0), moved(190.0, 5.0), 1.017, false),
            // In the paper's colour where the reference has paint too.
            edge(moved(10.0, 95.0), moved(190.0, 95.0), 0.169, true),
        ];
        let pairs = sheet_edges(&reference, &ours, &fit);
        assert_eq!((pairs.reference, pairs.ours), (4, 5));
        assert_eq!(pairs.same, 3, "{pairs:?}");
        assert_eq!(pairs.other_width.len(), 1, "{pairs:?}");
        assert!((pairs.other_width[0].reference_width_mm - 1.355).abs() < 1e-9);
        assert!((pairs.other_width[0].ours.width_mm - 1.017).abs() < 1e-9);
        // Reported in the reference's frame.
        assert!((pairs.other_width[0].ours.start_mm[0] - 10.0).abs() < 1e-9);
        assert_eq!(pairs.ours_only.len(), 1, "{pairs:?}");
        assert!((pairs.ours_only[0].start_mm[1] - 100.1).abs() < 1e-9);
        assert!(pairs.reference_only.is_empty(), "{pairs:?}");
        assert_eq!(pairs.apart(), 2);
    }

    /// An edge 0.3 mm off, or one that stops 2 mm short, is another edge.
    #[test]
    fn an_edge_out_of_place_is_no_pair() {
        let mut reference = pens();
        reference.sheet_edges = vec![
            edge([0.0, 5.0], [190.0, 5.0], 0.169, false),
            edge([0.0, 50.0], [190.0, 50.0], 0.169, false),
        ];
        let mut ours = pens();
        ours.sheet_edges = vec![
            edge([0.0, 5.3], [190.0, 5.3], 0.169, false),
            edge([2.0, 50.0], [190.0, 50.0], 0.169, false),
        ];
        let pairs = sheet_edges(&reference, &ours, &identity());
        assert_eq!(
            pairs,
            SheetEdgePairs {
                reference: 2,
                ours: 2,
                same: 0,
                other_width: Vec::new(),
                ours_only: ours.sheet_edges.clone(),
                reference_only: reference.sheet_edges.clone(),
            }
        );
        assert_eq!(pairs.apart(), 4);
    }

    fn ends(cap: LineCap, join: LineJoin, strokes: Vec<InkSegment>) -> StrokeEnds {
        StrokeEnds {
            cap,
            join,
            paths: strokes.len(),
            length_mm: strokes
                .iter()
                .map(|stroke| {
                    (stroke.end_mm[0] - stroke.start_mm[0])
                        .hypot(stroke.end_mm[1] - stroke.start_mm[1])
                })
                .sum(),
            strokes,
        }
    }

    const BUTT: (LineCap, LineJoin) = (LineCap::Butt, LineJoin::Miter);
    const ROUND: (LineCap, LineJoin) = (LineCap::Round, LineJoin::Round);

    /// A reference plot in small: a frame rule stroked before the page
    /// states a cap, then a pipe and a dash under round ends.
    #[test]
    fn a_stroke_is_held_to_the_ends_of_the_reference_stroke_at_its_place() {
        let rule = stroke([10.0, 10.0], [110.0, 10.0]);
        let pipe = stroke([10.0, 30.0], [60.0, 30.0]);
        let dash = stroke([10.0, 50.0], [12.0, 50.0]);
        let stray = stroke([150.0, 80.0], [160.0, 80.0]);
        let mut reference = pens();
        reference.ends = vec![
            ends(BUTT.0, BUTT.1, vec![rule.clone()]),
            ends(ROUND.0, ROUND.1, vec![pipe.clone(), dash.clone()]),
        ];

        // Ours all butt and mitred, as the writer stated them before it knew
        // better: the rule is as the reference's, the pipe and the dash are
        // not, and a stroke the reference has none for is held to nothing.
        let mut ours = pens();
        ours.ends = vec![ends(
            BUTT.0,
            BUTT.1,
            vec![rule.clone(), pipe.clone(), dash.clone(), stray.clone()],
        )];
        let apart = ends_apart(&reference, &ours, &identity());
        assert!((apart.same_mm - 100.0).abs() < 1e-9, "{apart:?}");
        assert!((apart.other_mm - 52.0).abs() < 1e-9, "{apart:?}");
        assert!((apart.alone_mm - 10.0).abs() < 1e-9, "{apart:?}");
        assert_eq!(apart.apart.len(), 1, "{apart:?}");
        assert_eq!(
            (apart.apart[0].ours, apart.apart[0].reference),
            (BUTT, ROUND)
        );
        assert!((apart.apart[0].length_mm - 52.0).abs() < 1e-9, "{apart:?}");

        // Round everything of ours and the rule is what is left.
        ours.ends = vec![ends(
            ROUND.0,
            ROUND.1,
            vec![rule.clone(), pipe.clone(), dash.clone(), stray],
        )];
        let apart = ends_apart(&reference, &ours, &identity());
        assert!((apart.same_mm - 52.0).abs() < 1e-9, "{apart:?}");
        assert!((apart.other_mm - 100.0).abs() < 1e-9, "{apart:?}");
        assert_eq!(
            (apart.apart[0].ours, apart.apart[0].reference),
            (ROUND, BUTT)
        );

        // The reference's two pairs, on the wrong strokes: a tally of which
        // pairs each page uses reads these two pages as the same.
        ours.ends = vec![
            ends(BUTT.0, BUTT.1, vec![pipe.clone(), dash.clone()]),
            ends(ROUND.0, ROUND.1, vec![rule.clone()]),
        ];
        let apart = ends_apart(&reference, &ours, &identity());
        assert!(apart.same_mm.abs() < 1e-9, "{apart:?}");
        assert!((apart.other_mm - 152.0).abs() < 1e-9, "{apart:?}");
        assert_eq!(apart.apart.len(), 2, "{apart:?}");

        // And on the right ones, drawn 0.1 mm off and with the page moved:
        // nothing apart.
        let fit = Fit {
            translation_mm: [3.0, -2.0],
            ..identity()
        };
        let moved = |stroke: &InkSegment| InkSegment {
            start_mm: [stroke.start_mm[0] + 3.0, stroke.start_mm[1] - 1.9],
            end_mm: [stroke.end_mm[0] + 3.0, stroke.end_mm[1] - 1.9],
            width_mm: stroke.width_mm,
        };
        ours.ends = vec![
            ends(BUTT.0, BUTT.1, vec![moved(&rule)]),
            ends(ROUND.0, ROUND.1, vec![moved(&pipe), moved(&dash)]),
        ];
        let apart = ends_apart(&reference, &ours, &fit);
        assert!((apart.same_mm - 152.0).abs() < 1e-9, "{apart:?}");
        assert!(
            apart.other_mm.abs() < 1e-9 && apart.alone_mm.abs() < 1e-9,
            "{apart:?}"
        );
        assert!(apart.apart.is_empty(), "{apart:?}");
    }

    /// A stroke of ours that runs on past the reference's is held to the
    /// reference's ends only where the reference has a stroke, and one that
    /// crosses a reference stroke is not on it.
    #[test]
    fn only_the_part_on_a_reference_stroke_is_held_to_its_ends() {
        let mut reference = pens();
        reference.ends = vec![ends(
            ROUND.0,
            ROUND.1,
            vec![stroke([10.0, 10.0], [20.0, 10.0])],
        )];
        let mut ours = pens();
        ours.ends = vec![ends(
            BUTT.0,
            BUTT.1,
            vec![
                // The first 10 mm on the reference's stroke, 10 mm past it.
                stroke([10.0, 10.0], [30.0, 10.0]),
                // Across it at a right angle.
                stroke([15.0, 5.0], [15.0, 15.0]),
            ],
        )];
        let apart = ends_apart(&reference, &ours, &identity());
        // The reach past the reference's end is the 0.20 mm a point may lie
        // off a stroke, read in quarter-millimetre parts.
        assert!((apart.other_mm - 10.0).abs() <= 0.25, "{apart:?}");
        assert!((apart.alone_mm - 20.0).abs() <= 0.25, "{apart:?}");
        assert!(apart.same_mm.abs() < 1e-9, "{apart:?}");
    }
}
