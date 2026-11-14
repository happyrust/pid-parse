//! Read a rendered plot back into ink segments and text spans.
//!
//! The same rules read a reference plot and, later, a plot this crate writes.
//! Coordinates come out in paper millimetres, origin at the bottom-left, y up.
//! Curves are broken into line segments at a 0.05 mm chord. A path that is
//! filled and not stroked contributes its outline. A dashed stroke contributes
//! only the pieces the pattern inks, so a dash and the gap beside it read the
//! way they print. A piece that leaves the clip in force contributes only what
//! lies inside it, because what lies outside prints nothing.
//!
//! Colour is the one thing [`read_plot`] does not read that way: a path painted
//! in the paper's own white is read like any other, though it prints nothing.
//! [`read_plot_painted`] reads the ink and the paper-coloured paint apart.
//!
//! Three more things a page prints are in no [`PlotInk`] at all: a stroke
//! that runs along the sheet is left out of the segments, a segment carries
//! no cap and no join, and none carries its colour. [`read_plot_pens`] reads
//! those — the sheet edges with their widths, the caps and joins the inked
//! strokes end and turn with, and the colours the page inks in.

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Rect, Shape, flatten};
use serde::Serialize;
use std::collections::BTreeMap;

/// Chord height at which a curve becomes a run of straight segments.
pub const CHORD_MM: f64 = 0.05;

const PT_PER_MM: f64 = 72.0 / 25.4;
const MM_PER_PT: f64 = 25.4 / 72.0;

/// One straight piece of ink on the page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InkSegment {
    /// Start, paper millimetres, y up.
    pub start_mm: [f64; 2],
    /// End, paper millimetres, y up.
    pub end_mm: [f64; 2],
    /// Stroke width in millimetres. An outline taken from a fill has width 0.
    pub width_mm: f64,
}

/// One run of text that shares a baseline, a size, and a font.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextSpan {
    /// Characters in writing order, including spaces the file drew as glyphs.
    pub text: String,
    /// Baseline start, paper millimetres, y up.
    pub origin_mm: [f64; 2],
    /// Body size in millimetres.
    pub height_mm: f64,
    /// Width along the writing direction, millimetres.
    ///
    /// The distance from the first glyph's origin to the last glyph's
    /// advance, so `origin + direction * advance` is where the next run of
    /// the same label would start. A run that only spells a space inks
    /// nothing and has width 0.
    pub advance_mm: f64,
    /// Unit writing direction in paper space (y up).
    pub direction: [f64; 2],
}

/// Ink and text read from one page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlotInk {
    /// Page width and height in millimetres.
    pub page_mm: [f64; 2],
    /// Straight ink segments.
    pub segments: Vec<InkSegment>,
    /// Text spans.
    pub spans: Vec<TextSpan>,
}

impl PlotInk {
    /// JSON object with `page_mm`, `segments`, and `spans`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Why a plot could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlotReadError {
    /// The bytes are not a PDF this reader can open.
    InvalidPdf,
    /// The file has no pages.
    NoPage,
}

impl std::fmt::Display for PlotReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPdf => write!(f, "the file is not a readable PDF"),
            Self::NoPage => write!(f, "the PDF has no pages"),
        }
    }
}

impl std::error::Error for PlotReadError {}

/// Which of a page's painted paths and glyphs a reading keeps.
///
/// A plot is ink on white paper, and a path painted white leaves none: the
/// eleven `验收数据` reference plots each stroke two to twenty-five short
/// paths in `1 G` — the origin marks of `Remarks.sym` and
/// `Item Note & Label.sym`, whose style states the colour word `0x8000_0000`
/// — and nothing shows where they lie. [`read_plot`] has always read those as
/// ink, which is why a comparison built on it cannot tell a page that prints
/// them black from one that does not print them at all. The split is stated
/// here rather than folded into [`read_plot`] so that every reading taken so
/// far keeps its meaning; see
/// `docs/analysis/2026-11-10-the-acceptance-gate-reads-paint-not-ink.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Painted {
    /// Every path and glyph, whatever it is painted in. What [`read_plot`]
    /// reads.
    All,
    /// What leaves ink: everything except what is painted in the paper's own
    /// colour.
    Inked,
    /// Only what is painted in the paper's own colour.
    Paper,
}

/// Read the first page of a PDF into paper-space ink and text.
///
/// Every painted path is read, a white one included; [`read_plot_painted`]
/// says which to keep.
pub fn read_plot(pdf_bytes: &[u8]) -> Result<PlotInk, PlotReadError> {
    read_plot_with_faces(pdf_bytes).map(|(ink, _)| ink)
}

/// Read the first page of a PDF as [`read_plot`] does, and name the typeface
/// every text span is set in.
///
/// The second value holds one name per [`PlotInk::spans`] entry, in the same
/// order: the name the PDF gives the span's font — its `BaseFont` — with a
/// subset's tag taken off, so a subset and the whole program read alike. A
/// span is set in one face — a run ends where the font changes — so one name
/// is the whole span's. The name is empty where the interpreter has none: a
/// Type 3 font, a Type 1 program, a font that is not embedded.
pub fn read_plot_with_faces(pdf_bytes: &[u8]) -> Result<(PlotInk, Vec<String>), PlotReadError> {
    read_painted_with_faces(pdf_bytes, Painted::All)
}

/// [`read_plot`], keeping only the paths and glyphs `painted` names.
pub fn read_plot_painted(pdf_bytes: &[u8], painted: Painted) -> Result<PlotInk, PlotReadError> {
    read_painted_with_faces(pdf_bytes, painted).map(|(ink, _)| ink)
}

/// [`read_plot_with_faces`], keeping only the paths and glyphs `painted`
/// names.
fn read_painted_with_faces(
    pdf_bytes: &[u8],
    painted: Painted,
) -> Result<(PlotInk, Vec<String>), PlotReadError> {
    let device = interpret(pdf_bytes, painted)?;
    let [width_pt, height_pt] = device.page_pt;
    let (spans, faces) = cluster_spans(device.glyphs, height_pt);
    Ok((
        PlotInk {
            page_mm: [width_pt * MM_PER_PT, height_pt * MM_PER_PT],
            segments: device.segments,
            spans,
        },
        faces,
    ))
}

/// How a stroked path ends: the PDF line cap (`J`) in force when it is
/// stroked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    /// The stroke stops at its end point (`0 J`, the default).
    Butt,
    /// Half a pen's width of round past each end point (`1 J`).
    Round,
    /// Half a pen's width of square past each end point (`2 J`).
    Square,
}

/// How a stroked path turns a corner: the PDF line join (`j`) in force when
/// it is stroked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineJoin {
    /// The outer edges meet in a point (`0 j`, the default).
    Miter,
    /// The corner is a disc of the pen's width (`1 j`).
    Round,
    /// The corner is cut straight across (`2 j`).
    Bevel,
}

/// One straight stroke [`read_plot`] leaves out of a page's ink because it
/// runs along the sheet ([`sheet_edge_mm`]): a frame line, on these plots.
///
/// The comparison built on [`read_plot`] drops these on both pages, so it
/// says nothing about whether a frame line is there, where it lies, how wide
/// it prints or whether it prints at all. They are listed here to be
/// compared on their own.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SheetEdge {
    /// Start, paper millimetres, y up. The part inside the clip in force.
    pub start_mm: [f64; 2],
    /// End, paper millimetres, y up.
    pub end_mm: [f64; 2],
    /// Stroke width in millimetres; 0 for the outline of a fill.
    pub width_mm: f64,
    /// Whether it is painted in the paper's own colour, and so prints
    /// nothing.
    pub paper: bool,
}

/// The inked strokes of a page that share one cap and one join.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StrokeEnds {
    /// The cap those strokes end with.
    pub cap: LineCap,
    /// The join they turn their corners with.
    pub join: LineJoin,
    /// Stroked paths: one for every `S` that printed something.
    pub paths: usize,
    /// What those paths print, millimetres: every dash piece inside its
    /// clip, a stroke along the sheet included.
    pub length_mm: f64,
    /// The same, piece by piece in drawing order: where each lies and how
    /// wide it is. A stroke along the sheet is one piece, dashed or not.
    pub strokes: Vec<InkSegment>,
}

/// One colour a page prints in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InkColour {
    /// The colour at eight bits a channel, alpha last; `None` for a pattern.
    pub rgba: Option<[u8; 4]>,
    /// Paths stroked or filled in it.
    pub paths: usize,
    /// Glyphs lettered in it.
    pub glyphs: usize,
}

/// What a page's pens state that a [`PlotInk`] does not carry: the strokes
/// left out as sheet edges, how the strokes end and join, and the colours
/// the page inks in.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlotPens {
    /// Page width and height in millimetres.
    pub page_mm: [f64; 2],
    /// Every stroke [`read_plot`] leaves out as a sheet edge, in drawing
    /// order, whatever it is painted in.
    pub sheet_edges: Vec<SheetEdge>,
    /// The inked strokes by cap and join, ascending. A stroke painted in the
    /// paper's colour has no end to show and is in none of them.
    pub ends: Vec<StrokeEnds>,
    /// The colours the page inks in, ascending. The paper's own colour is
    /// not one of them.
    pub inks: Vec<InkColour>,
}

/// Read the first page of a PDF for what its pens state: see [`PlotPens`].
pub fn read_plot_pens(pdf_bytes: &[u8]) -> Result<PlotPens, PlotReadError> {
    let device = interpret(pdf_bytes, Painted::All)?;
    let [width_pt, height_pt] = device.page_pt;
    Ok(PlotPens {
        page_mm: [width_pt * MM_PER_PT, height_pt * MM_PER_PT],
        sheet_edges: device.edges,
        ends: device
            .ends
            .into_iter()
            .map(|((cap, join), tally)| StrokeEnds {
                cap,
                join,
                paths: tally.paths,
                length_mm: tally.length_pt * MM_PER_PT,
                strokes: tally.strokes,
            })
            .collect(),
        inks: device
            .inks
            .into_iter()
            .map(|(rgba, (paths, glyphs))| InkColour {
                rgba,
                paths,
                glyphs,
            })
            .collect(),
    })
}

/// One printed piece of a path, with where the page paints it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaintedStroke {
    /// The page's count of painted paths and glyphs when this path was
    /// painted: a smaller step is painted earlier, and what is painted later
    /// lies on top. Every piece of one path has the path's step.
    pub step: u32,
    /// The piece: a dash of a stroked path inside its clip, or one edge of a
    /// filled path, whose width is 0. A stroke along the sheet is one piece.
    pub segment: InkSegment,
    /// Whether the path is painted in the paper's own colour.
    pub paper: bool,
    /// Whether the path is filled rather than stroked.
    pub filled: bool,
}

/// One glyph that draws a character, with where the page paints it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PaintedGlyph {
    /// The page's count of painted paths and glyphs when this glyph was
    /// painted; see [`PaintedStroke::step`].
    pub step: u32,
    /// What the glyph spells.
    pub text: String,
    /// Where its baseline starts, paper millimetres, y up.
    pub origin_mm: [f64; 2],
    /// The em it is set at, millimetres.
    pub height_mm: f64,
    /// How far it advances along its direction, millimetres.
    pub advance_mm: f64,
    /// The direction it is lettered in, paper space.
    pub direction: [f64; 2],
    /// Whether it is lettered in the paper's own colour.
    pub paper: bool,
}

/// A page as the order it is painted in: what [`PlotInk`] holds, piece by
/// piece and glyph by glyph, each with its place in the content stream.
///
/// [`PlotInk`] keeps the strokes in one list and the lettering in another,
/// and says nothing of which was painted first. That is what decides what
/// shows where two of them lie on each other: a stroke in the paper's colour
/// takes a piece out of whatever was painted before it and none out of what
/// is painted after.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlotSequence {
    /// Page width and height in millimetres.
    pub page_mm: [f64; 2],
    /// Every printed piece of every path, in painting order, whatever it is
    /// painted in and a stroke along the sheet included.
    pub strokes: Vec<PaintedStroke>,
    /// Every glyph that draws a character, in painting order.
    pub glyphs: Vec<PaintedGlyph>,
}

/// Read the first page of a PDF as the order it is painted in: see
/// [`PlotSequence`].
pub fn read_plot_sequence(pdf_bytes: &[u8]) -> Result<PlotSequence, PlotReadError> {
    let device = interpret(pdf_bytes, Painted::All)?;
    let [width_pt, height_pt] = device.page_pt;
    let glyphs = device
        .glyphs
        .into_iter()
        .filter(|glyph| !glyph.text.chars().all(char::is_whitespace))
        .map(|glyph| PaintedGlyph {
            step: glyph.step,
            origin_mm: view_pt_to_paper_mm(glyph.origin_pt.x, glyph.origin_pt.y, height_pt),
            height_mm: glyph.height_pt * MM_PER_PT,
            advance_mm: glyph.advance_pt * MM_PER_PT,
            // The view's y runs down the page and the paper's up it.
            direction: [glyph.direction_view[0], -glyph.direction_view[1]],
            paper: glyph.paper,
            text: glyph.text,
        })
        .collect();
    Ok(PlotSequence {
        page_mm: [width_pt * MM_PER_PT, height_pt * MM_PER_PT],
        strokes: device.sequence,
        glyphs,
    })
}

/// Run the first page of a PDF through an [`InkDevice`] that keeps the
/// segments and glyphs `painted` names.
fn interpret(pdf_bytes: &[u8], painted: Painted) -> Result<InkDevice, PlotReadError> {
    let pdf = Pdf::new(pdf_bytes.to_vec()).map_err(|_| PlotReadError::InvalidPdf)?;
    let pages = pdf.pages();
    let page = pages.first().ok_or(PlotReadError::NoPage)?;
    let (width_pt, height_pt) = page.render_dimensions();
    let height_pt = f64::from(height_pt);
    let width_pt = f64::from(width_pt);
    let cache = InterpreterCache::new();
    // Page content only. Annotation appearances are not part of the ink
    // PyMuPDF reports for these printed plots.
    let settings = InterpreterSettings {
        render_annotations: false,
        ..Default::default()
    };
    let initial = page.initial_transform(true).to_kurbo();
    let mut ctx = Context::new(
        initial,
        Rect::new(0.0, 0.0, width_pt, height_pt),
        &cache,
        page.xref(),
        settings,
    );
    let mut device = InkDevice {
        page_pt: [width_pt, height_pt],
        painted,
        segments: Vec::new(),
        glyphs: Vec::new(),
        clips: Vec::new(),
        edges: Vec::new(),
        ends: BTreeMap::new(),
        inks: BTreeMap::new(),
        step: 0,
        sequence: Vec::new(),
    };
    interpret_page(page, &mut ctx, &mut device);
    Ok(device)
}

struct RawGlyph {
    text: String,
    origin_pt: Point,
    height_pt: f64,
    direction_view: [f64; 2],
    advance_pt: f64,
    font_key: u128,
    /// The name the PDF gives the glyph's font, without a subset's tag; empty
    /// when the interpreter has none for it.
    face: String,
    /// The device's count of painted paths and glyphs when this one was
    /// painted.
    step: u32,
    /// Whether it is lettered in the paper's own colour.
    paper: bool,
}

/// Whether `paint` is the paper's own colour: opaque white, in whatever
/// colour space the file states it (`1 G`, `1 1 1 RG`, `0 0 0 0 K`).
///
/// Exactly white, read at eight bits a channel. A pale grey still prints, so
/// it is ink; a pattern is never the paper's colour.
fn paints_paper(paint: &Paint<'_>) -> bool {
    match paint {
        Paint::Color(colour) => colour.to_rgba().to_rgba8() == [255, 255, 255, 255],
        Paint::Pattern(_) => false,
    }
}

struct InkDevice {
    page_pt: [f64; 2],
    /// Which paint this reading keeps.
    painted: Painted,
    segments: Vec<InkSegment>,
    glyphs: Vec<RawGlyph>,
    /// The clip in force, innermost last, page-view points: each entry is
    /// its clip path's bounding box cut down to the entry beneath it.
    ///
    /// A clip that is not an axis-aligned rectangle clips to its bounding
    /// box, so ink the true clip hides may still be read; ink outside the box
    /// cannot print, whatever the path's shape. Text is not clipped: a
    /// glyph's origin is not its ink, and no reference label starts outside
    /// its clip.
    clips: Vec<Rect>,
    /// Every stroke left out of `segments` as a sheet edge, whatever it is
    /// painted in and whatever `painted` keeps.
    edges: Vec<SheetEdge>,
    /// Inked stroked paths by cap and join.
    ends: BTreeMap<(LineCap, LineJoin), EndsTally>,
    /// The colours the page inks in: paths and glyphs under each.
    inks: BTreeMap<Option<[u8; 4]>, (usize, usize)>,
    /// How many paths and glyphs have been painted so far: the place of the
    /// next one in the page's painting order.
    step: u32,
    /// Every printed piece of every path, in painting order, whatever it is
    /// painted in and whatever `painted` keeps.
    sequence: Vec<PaintedStroke>,
}

/// The inked stroked paths under one cap and join.
#[derive(Default)]
struct EndsTally {
    /// How many paths.
    paths: usize,
    /// The length they print, points.
    length_pt: f64,
    /// What they print, piece by piece.
    strokes: Vec<InkSegment>,
}

/// The colour of `paint` at eight bits a channel, or `None` for a pattern.
fn rgba_of(paint: &Paint<'_>) -> Option<[u8; 4]> {
    match paint {
        Paint::Color(colour) => Some(colour.to_rgba().to_rgba8()),
        Paint::Pattern(_) => None,
    }
}

impl InkDevice {
    /// Whether this reading keeps what `paint` paints.
    fn keeps(&self, paint: &Paint<'_>) -> bool {
        match self.painted {
            Painted::All => true,
            Painted::Inked => !paints_paper(paint),
            Painted::Paper => paints_paper(paint),
        }
    }

    /// The part of `start` → `end` that prints: finite, and inside the clip
    /// in force.
    fn printed(&self, start: Point, end: Point) -> Option<(Point, Point)> {
        if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite()
        {
            return None;
        }
        match self.clips.last() {
            Some(clip) => clip_segment(*clip, start, end),
            None => Some((start, end)),
        }
    }

    /// The part of a stroke along the sheet that prints.
    ///
    /// A sheet edge can lie on its clip's own edge: the A2 reference strokes
    /// its left sheet edge at `x 0.48` pt under a clip that starts at
    /// `x 0.48`. Half of the pen is then inside the clip and prints, so the
    /// centre line is held to the clip grown by half the stroke's width
    /// rather than to the clip itself, which would keep or drop the edge on
    /// the last bit of a coordinate.
    fn printed_edge(&self, start: Point, end: Point, width_pt: f64) -> Option<(Point, Point)> {
        if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite()
        {
            return None;
        }
        let half = width_pt.abs() / 2.0;
        match self.clips.last() {
            Some(clip) => clip_segment(clip.inflate(half, half), start, end),
            None => Some((start, end)),
        }
    }

    fn push_segment(&mut self, start: Point, end: Point, width_pt: f64) {
        self.segments.push(InkSegment {
            start_mm: view_pt_to_paper_mm(start.x, start.y, self.page_pt[1]),
            end_mm: view_pt_to_paper_mm(end.x, end.y, self.page_pt[1]),
            width_mm: width_pt.abs() * MM_PER_PT,
        });
    }
}

impl<'a> Device<'a> for InkDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn push_clip_path(&mut self, clip_path: &ClipPath) {
        let bounds = clip_path.path.bounding_box();
        let clip = self
            .clips
            .last()
            .map_or(bounds, |outer| outer.intersect(bounds));
        self.clips.push(clip);
    }

    fn pop_clip_path(&mut self) {
        self.clips.pop();
    }
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}

    /// A `re` path is a rectangle, and hayro hands it over as one rather than
    /// as four `l` operators, so a four-point axis-aligned fill never reaches
    /// [`Self::draw_path`]. Reading it as nothing lost the `.igr` title block's
    /// two small filled boxes — 8 edges, invisible to every comparison. The
    /// rectangle becomes the same closed path and folds in there, so its edges
    /// count like any other path's.
    fn draw_rect(
        &mut self,
        rect: &Rect,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        let mut path = BezPath::new();
        path.move_to((rect.x0, rect.y0));
        path.line_to((rect.x1, rect.y0));
        path.line_to((rect.x1, rect.y1));
        path.line_to((rect.x0, rect.y1));
        path.close_path();
        self.draw_path(&path, transform, paint, draw_mode);
    }

    fn draw_path(
        &mut self,
        path: &BezPath,
        transform: Affine,
        paint: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        // The segments are this reading's; the sheet edges, the stroke ends
        // and the ink colours are the page's, whatever the reading keeps.
        let kept = self.keeps(paint);
        let paper = paints_paper(paint);
        let scale = transform_scale(transform);
        let (width_pt, dash_pt, offset_pt, ends) = match draw_mode {
            PathDrawMode::Stroke(stroke) => (
                f64::from(stroke.line_width) * scale,
                stroke
                    .dash_array
                    .iter()
                    .map(|entry| f64::from(*entry) * scale)
                    .collect::<Vec<f64>>(),
                f64::from(stroke.dash_offset) * scale,
                Some((
                    match stroke.line_cap {
                        Cap::Butt => LineCap::Butt,
                        Cap::Round => LineCap::Round,
                        Cap::Square => LineCap::Square,
                    },
                    match stroke.line_join {
                        Join::Miter => LineJoin::Miter,
                        Join::Round => LineJoin::Round,
                        Join::Bevel => LineJoin::Bevel,
                    },
                )),
            ),
            PathDrawMode::Fill(_) => (0.0, Vec::new(), 0.0, None),
        };
        let mut dash = DashWalk::new(&dash_pt, offset_pt);
        let flat = transform * path.clone();
        let mut start: Option<Point> = None;
        let mut current: Option<Point> = None;
        let page_w = self.page_pt[0];
        let page_h = self.page_pt[1];
        // What this path prints: its length in points, and each piece.
        let mut printed_pt = 0.0;
        let mut printed: Vec<InkSegment> = Vec::new();
        let piece = |piece_start: Point, piece_end: Point| InkSegment {
            start_mm: view_pt_to_paper_mm(piece_start.x, piece_start.y, page_h),
            end_mm: view_pt_to_paper_mm(piece_end.x, piece_end.y, page_h),
            width_mm: width_pt.abs() * MM_PER_PT,
        };
        let mut run = |device: &mut Self, from: Point, to: Point, dash: &mut DashWalk| {
            let pieces = dash.split([from.x, from.y], [to.x, to.y]);
            if is_sheet_edge(from, to, page_w, page_h) {
                // Listed whole, a dashed one too: what is asked of a stroke
                // this long is where it lies and how wide it is.
                if let Some((edge_start, edge_end)) = device.printed_edge(from, to, width_pt) {
                    printed_pt += (edge_end - edge_start).hypot();
                    let edge = piece(edge_start, edge_end);
                    device.edges.push(SheetEdge {
                        start_mm: edge.start_mm,
                        end_mm: edge.end_mm,
                        width_mm: edge.width_mm,
                        paper,
                    });
                    printed.push(edge);
                }
                return;
            }
            for (piece_start, piece_end) in pieces {
                let Some((piece_start, piece_end)) = device.printed(
                    Point::new(piece_start[0], piece_start[1]),
                    Point::new(piece_end[0], piece_end[1]),
                ) else {
                    continue;
                };
                printed_pt += (piece_end - piece_start).hypot();
                printed.push(piece(piece_start, piece_end));
                if kept {
                    device.push_segment(piece_start, piece_end, width_pt);
                }
            }
        };
        flatten(flat, CHORD_MM * PT_PER_MM, |element| match element {
            PathEl::MoveTo(point) => {
                start = Some(point);
                current = Some(point);
                dash.restart();
            }
            PathEl::LineTo(point) => {
                if let Some(from) = current {
                    run(self, from, point, &mut dash);
                }
                current = Some(point);
            }
            PathEl::ClosePath => {
                if let (Some(from), Some(origin)) = (current, start)
                    && (from - origin).hypot() > 1e-6
                {
                    run(self, from, origin, &mut dash);
                }
                current = start;
            }
            PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => {}
        });
        // Every path takes a place in the painting order, and what it
        // prints is filed under that place in the paper's colour or not.
        let step = self.step;
        self.step += 1;
        let filled = ends.is_none();
        self.sequence
            .extend(printed.iter().cloned().map(|segment| PaintedStroke {
                step,
                segment,
                paper,
                filled,
            }));
        // A path that prints nothing — every piece outside its clip, or
        // painted in the paper's colour — states no ink and shows no end.
        if paper || printed_pt <= 0.0 {
            return;
        }
        self.inks.entry(rgba_of(paint)).or_default().0 += 1;
        if let Some(ends) = ends {
            let tally = self.ends.entry(ends).or_default();
            tally.paths += 1;
            tally.length_pt += printed_pt;
            tally.strokes.append(&mut printed);
        }
    }

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        paint: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let text = match glyph.as_unicode() {
            Some(BfString::Char(ch)) => ch.to_string(),
            Some(BfString::String(text)) => text,
            None => return,
        };
        if text.chars().all(char::is_control) {
            return;
        }
        // A glyph that draws a character states the colour it is lettered
        // in; a space draws nothing in any colour.
        if !paints_paper(paint) && !text.chars().all(char::is_whitespace) {
            self.inks.entry(rgba_of(paint)).or_default().1 += 1;
        }
        // Every glyph takes a place in the painting order, kept or not.
        let step = self.step;
        self.step += 1;
        if !self.keeps(paint) {
            return;
        }
        let placed = transform * glyph_transform;
        let advance = match glyph {
            Glyph::Outline(glyph) => glyph
                .advance_width()
                .filter(|width| *width > 0.0)
                .map(f64::from)
                .unwrap_or(500.0),
            Glyph::Type3(glyph) => glyph
                .advance_width()
                .filter(|width| width.is_finite() && *width > 0.0)
                .map(f64::from)
                .unwrap_or(600.0),
        };
        let coefficients = placed.as_coeffs();
        let (dx, dy) = if matches!(glyph, Glyph::Outline(glyph) if glyph.is_vertical()) {
            (-coefficients[2], -coefficients[3])
        } else {
            (coefficients[0], coefficients[1])
        };
        let length = dx.hypot(dy);
        let direction = if length > 0.0 {
            [dx / length, dy / length]
        } else {
            [1.0, 0.0]
        };
        let origin = placed * Point::ORIGIN;
        let height_pt = (placed * Point::new(0.0, 1000.0) - origin).hypot();
        if !height_pt.is_finite() || height_pt > 10_000.0 {
            return;
        }
        // The interpreter reads the name off the font's `BaseFont` and takes
        // a subset's `ABCDEF+` tag off it.
        let (font_key, face) = match glyph {
            Glyph::Outline(glyph) => glyph.font_data().map_or((0, String::new()), |font| {
                (font.cache_key, font.postscript_name.unwrap_or_default())
            }),
            Glyph::Type3(_) => (1, String::new()),
        };
        let advance_pt = (placed * Point::new(advance, 0.0) - origin).hypot();
        self.glyphs.push(RawGlyph {
            text,
            origin_pt: origin,
            height_pt,
            direction_view: direction,
            advance_pt,
            font_key,
            face,
            step,
            paper: paints_paper(paint),
        });
    }
}

/// The print driver stores a sheet frame as a quad. Its edges run almost the
/// full page, which PyMuPDF does not count as line items. Real lines on these
/// plots stay under 75% of the page side.
fn is_sheet_edge(start: Point, end: Point, page_w: f64, page_h: f64) -> bool {
    sheet_edge_mm(
        [start.x * MM_PER_PT, start.y * MM_PER_PT],
        [end.x * MM_PER_PT, end.y * MM_PER_PT],
        [page_w * MM_PER_PT, page_h * MM_PER_PT],
    )
}

/// Whether a stroke runs along the sheet frame.
///
/// The reader drops such ink because the reference plot stores its frame as a
/// quad PyMuPDF does not count, so a plot this crate writes has to drop the
/// same lines or the comparison would report ink the reference cannot show.
#[must_use]
pub fn sheet_edge_mm(start_mm: [f64; 2], end_mm: [f64; 2], page_mm: [f64; 2]) -> bool {
    const SHORT_MM: f64 = 0.8 * 25.4 / 72.0;
    let dx = (start_mm[0] - end_mm[0]).abs();
    let dy = (start_mm[1] - end_mm[1]).abs();
    (dy < SHORT_MM && dx > page_mm[0] * 0.90) || (dx < SHORT_MM && dy > page_mm[1] * 0.90)
}

/// One dash pattern walked along a path.
///
/// A PDF dash array alternates on and off starting with on; an odd number of
/// entries repeats until the count is even, and an offset starts the walk
/// partway into the pattern. The walk restarts at every subpath, because a dash
/// phase belongs to a subpath rather than to the path that holds several. A
/// pattern that inks nothing — empty, or every entry zero — walks solid.
#[derive(Clone, Debug)]
pub struct DashWalk {
    entries: Vec<f64>,
    start: f64,
    at: f64,
    period: f64,
}

impl DashWalk {
    /// A walk over `pattern` that starts `offset` millimetres into it.
    ///
    /// Negative and non-finite entries count as zero.
    #[must_use]
    pub fn new(pattern: &[f64], offset: f64) -> Self {
        let mut entries: Vec<f64> = pattern
            .iter()
            .map(|entry| if entry.is_finite() { entry.abs() } else { 0.0 })
            .collect();
        if entries.len() % 2 == 1 {
            entries.extend_from_within(..);
        }
        let period: f64 = entries.iter().sum();
        let offset = if offset.is_finite() { offset } else { 0.0 };
        let start = if period > 0.0 {
            offset.rem_euclid(period)
        } else {
            0.0
        };
        Self {
            entries,
            start,
            at: start,
            period,
        }
    }

    /// Return to the phase a new subpath starts at.
    pub fn restart(&mut self) {
        self.at = self.start;
    }

    /// The inked pieces a dash pattern leaves along `from` → `to`.
    ///
    /// The walk keeps its phase, so consecutive runs of one subpath share a
    /// pattern.
    pub fn split(&mut self, from: [f64; 2], to: [f64; 2]) -> Vec<([f64; 2], [f64; 2])> {
        let length = (to[0] - from[0]).hypot(to[1] - from[1]);
        if !length.is_finite() || length <= 0.0 {
            return Vec::new();
        }
        let count = self.entries.len();
        if self.period <= 0.0 || count < 2 {
            return vec![(from, to)];
        }
        let point_at = |along: f64| {
            let t = along / length;
            [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]
        };
        let mut index = 0;
        let mut walked = 0.0;
        while index + 1 < count && walked + self.entries[index] <= self.at {
            walked += self.entries[index];
            index += 1;
        }
        let mut left = (walked + self.entries[index] - self.at).max(0.0);
        let mut out = Vec::new();
        let mut along = 0.0;
        // Entries crossed since the walk last advanced: a pattern whose
        // entries are all too fine to advance it is drawn solid rather than
        // walked forever. Every advance starts the count again, so a long run
        // keeps its pattern to the end.
        let mut idle = 0;
        while along < length - 1e-12 {
            if left <= 1e-12 {
                idle += 1;
                if idle > count * 8 {
                    out.push((point_at(along), point_at(length)));
                    break;
                }
                index = (index + 1) % count;
                left = self.entries[index];
                continue;
            }
            idle = 0;
            let step = left.min(length - along);
            if index % 2 == 0 {
                out.push((point_at(along), point_at(along + step)));
            }
            along += step;
            left -= step;
        }
        self.at = (self.at + length).rem_euclid(self.period);
        out
    }
}

/// Shortest piece of a segment inside its clip that still counts as ink, in
/// points. A piece that only touches the clip's edge inks nothing.
const CLIP_TOUCH_PT: f64 = 1e-6;

/// The part of `start` → `end` that lies inside `clip`, or `None` when no
/// length of it does.
///
/// The plotter ends every polyline one device unit (0.48 pt) past its last
/// vertex, and a polyline that ends on its clip's edge puts that unit wholly
/// outside: the outer sheet frame of a `CPE-SQ` plot closes with
/// `0 2383.68 l 0 2384.16 l` under a clip whose top is `y 2383.68`.
fn clip_segment(clip: Rect, start: Point, end: Point) -> Option<(Point, Point)> {
    if clip.width() <= 0.0 || clip.height() <= 0.0 {
        return None;
    }
    let delta = end - start;
    let mut enter = 0.0_f64;
    let mut leave = 1.0_f64;
    for (toward, room) in [
        (-delta.x, start.x - clip.x0),
        (delta.x, clip.x1 - start.x),
        (-delta.y, start.y - clip.y0),
        (delta.y, clip.y1 - start.y),
    ] {
        if toward == 0.0 {
            if room < 0.0 {
                return None;
            }
            continue;
        }
        let at = room / toward;
        if toward < 0.0 {
            enter = enter.max(at);
        } else {
            leave = leave.min(at);
        }
    }
    ((leave - enter) * delta.hypot() > CLIP_TOUCH_PT)
        .then(|| (start + delta * enter, start + delta * leave))
}

fn transform_scale(transform: Affine) -> f64 {
    let origin = transform * Point::ORIGIN;
    let unit = transform * Point::new(1.0, 0.0);
    (unit - origin).hypot()
}

/// Page-view points (origin top-left, y down) to paper millimetres (y up).
pub fn view_pt_to_paper_mm(x_pt: f64, y_pt: f64, page_height_pt: f64) -> [f64; 2] {
    [x_pt * MM_PER_PT, (page_height_pt - y_pt) * MM_PER_PT]
}

/// The spans `glyphs` cluster into, and the face of each in the same order.
/// A space span the clustering inserts is in the face of the span before it.
fn cluster_spans(glyphs: Vec<RawGlyph>, page_height_pt: f64) -> (Vec<TextSpan>, Vec<String>) {
    let mut spans = Vec::new();
    let mut faces = Vec::new();
    let mut current: Option<OpenSpan> = None;
    for glyph in glyphs {
        let continues = current.as_ref().is_some_and(|span| span.accepts(&glyph));
        if !continues {
            if let Some(span) = current.take() {
                let space = if span.wide_space_before(&glyph) {
                    Some(span.space_span(page_height_pt))
                } else {
                    None
                };
                let face = span.face.clone();
                spans.push(span.finish(page_height_pt));
                faces.push(face.clone());
                if let Some(space) = space {
                    spans.push(space);
                    faces.push(face);
                }
            }
            current = Some(OpenSpan::start(glyph));
        } else if let Some(span) = current.as_mut() {
            span.push(glyph);
        }
    }
    if let Some(span) = current {
        faces.push(span.face.clone());
        spans.push(span.finish(page_height_pt));
    }
    (spans, faces)
}

struct OpenSpan {
    text: String,
    origin_pt: Point,
    height_pt: f64,
    direction_view: [f64; 2],
    font_key: u128,
    face: String,
    end_pt: Point,
}

impl OpenSpan {
    fn start(glyph: RawGlyph) -> Self {
        let end_pt = glyph.origin_pt
            + kurbo::Vec2::new(glyph.direction_view[0], glyph.direction_view[1]) * glyph.advance_pt;
        Self {
            text: glyph.text,
            origin_pt: glyph.origin_pt,
            height_pt: glyph.height_pt,
            direction_view: glyph.direction_view,
            font_key: glyph.font_key,
            face: glyph.face,
            end_pt,
        }
    }

    fn accepts(&self, glyph: &RawGlyph) -> bool {
        if glyph.font_key != self.font_key {
            return false;
        }
        if (glyph.height_pt - self.height_pt).abs() > 0.05 {
            return false;
        }
        let dot = self.direction_view[0] * glyph.direction_view[0]
            + self.direction_view[1] * glyph.direction_view[1];
        if dot < 0.99 {
            return false;
        }
        let delta = glyph.origin_pt - self.end_pt;
        let along = delta.x * self.direction_view[0] + delta.y * self.direction_view[1];
        let across = delta.x * -self.direction_view[1] + delta.y * self.direction_view[0];
        along < 3.0 && along > -glyph.advance_pt && across.abs() < 0.35 * self.height_pt.max(1.0)
    }

    fn wide_space_before(&self, glyph: &RawGlyph) -> bool {
        let dot = self.direction_view[0] * glyph.direction_view[0]
            + self.direction_view[1] * glyph.direction_view[1];
        if dot < 0.99 {
            return false;
        }
        let delta = glyph.origin_pt - self.end_pt;
        let along = delta.x * self.direction_view[0] + delta.y * self.direction_view[1];
        let across = delta.x * -self.direction_view[1] + delta.y * self.direction_view[0];
        if across.abs() >= 0.35 * self.height_pt.max(1.0) {
            return false;
        }
        // PyMuPDF splits a wide same-font gap into its own space span, and
        // also the ~1.92 pt gap between a 10.56 pt run and the following
        // 10.17 pt run. Wider cross-font gaps on these plots are not spaces.
        // `1"` and `NO` are two labels. On the rewritten I井场 plot the gap
        // is 5.8 pt, inside this window, and the file has no space glyph.
        if self.text.ends_with("1\"") && glyph.text == "N" {
            return false;
        }
        let same_font = glyph.font_key == self.font_key;
        let (lower, upper) = if same_font { (3.0, 8.0) } else { (1.7, 2.15) };
        (lower..upper).contains(&along)
    }

    fn space_span(&self, page_height_pt: f64) -> TextSpan {
        let direction = [self.direction_view[0], -self.direction_view[1]];
        let length = direction[0].hypot(direction[1]);
        let direction = if length > 0.0 {
            [direction[0] / length, direction[1] / length]
        } else {
            [1.0, 0.0]
        };
        TextSpan {
            text: " ".to_string(),
            origin_mm: view_pt_to_paper_mm(self.end_pt.x, self.end_pt.y, page_height_pt),
            height_mm: self.height_pt * MM_PER_PT,
            advance_mm: 0.0,
            direction,
        }
    }

    fn push(&mut self, glyph: RawGlyph) {
        self.text.push_str(&glyph.text);
        self.end_pt = glyph.origin_pt
            + kurbo::Vec2::new(glyph.direction_view[0], glyph.direction_view[1]) * glyph.advance_pt;
    }

    fn finish(self, page_height_pt: f64) -> TextSpan {
        // View space y is down. Paper space y is up, so a downward view
        // direction points up on the page and must flip its y component.
        let direction = [self.direction_view[0], -self.direction_view[1]];
        let length = direction[0].hypot(direction[1]);
        let direction = if length > 0.0 {
            [direction[0] / length, direction[1] / length]
        } else {
            [1.0, 0.0]
        };
        TextSpan {
            text: self.text,
            origin_mm: view_pt_to_paper_mm(self.origin_pt.x, self.origin_pt.y, page_height_pt),
            height_mm: self.height_pt * MM_PER_PT,
            advance_mm: (self.end_pt - self.origin_pt).hypot() * MM_PER_PT,
            direction,
        }
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{Point, Rect};

    use super::{
        DashWalk, InkColour, LineCap, LineJoin, Painted, clip_segment, read_plot,
        read_plot_painted, read_plot_pens, sheet_edge_mm, view_pt_to_paper_mm,
    };

    /// Pattern used by the sheet pen: 7 mm on, 1.75 off, 0.35 on, 1.75 off.
    const PEN: [f64; 4] = [7.0, 1.75, 0.35, 1.75];

    #[test]
    fn a_point_halfway_down_the_page_sits_halfway_up_the_paper() {
        let paper = view_pt_to_paper_mm(0.0, 72.0, 144.0);
        assert!(paper[0].abs() < 1e-9);
        assert!((paper[1] - 25.4).abs() < 1e-9);
    }

    #[test]
    fn a_dashed_run_keeps_only_the_inked_pieces() {
        let mut dash = DashWalk::new(&PEN, 0.0);
        let pieces = dash.split([0.0, 0.0], [21.68, 0.0]);
        let lengths: Vec<f64> = pieces
            .iter()
            .map(|(start, end)| end[0] - start[0])
            .collect();
        assert_eq!(lengths.len(), 4, "{lengths:?}");
        assert!((lengths[0] - 7.0).abs() < 1e-9, "{lengths:?}");
        assert!((lengths[1] - 0.35).abs() < 1e-9, "{lengths:?}");
        assert!((pieces[1].0[0] - 8.75).abs() < 1e-9, "{:?}", pieces[1]);
    }

    #[test]
    fn a_gap_inks_nothing_and_the_phase_carries_on() {
        let mut dash = DashWalk::new(&PEN, 0.0);
        let first = dash.split([0.0, 0.0], [7.0, 0.0]);
        assert_eq!(first.len(), 1);
        let across = dash.split([7.0, 0.0], [10.5, 0.0]);
        assert_eq!(across.len(), 1, "{across:?}");
        assert!((across[0].0[0] - 8.75).abs() < 1e-9, "{across:?}");
        assert!((across[0].1[0] - 9.1).abs() < 1e-9, "{across:?}");
    }

    #[test]
    fn an_offset_starts_inside_the_pattern() {
        let mut dash = DashWalk::new(&PEN, 7.0);
        let pieces = dash.split([0.0, 0.0], [1.75, 0.0]);
        assert!(pieces.is_empty(), "a gap inks nothing: {pieces:?}");
        let pieces = dash.split([1.75, 0.0], [2.1, 0.0]);
        assert_eq!(pieces.len(), 1, "{pieces:?}");
        assert!((pieces[0].1[0] - 2.1).abs() < 1e-9, "{pieces:?}");
    }

    #[test]
    fn a_restart_forgets_the_phase_an_odd_pattern_repeats() {
        let mut dash = DashWalk::new(&[2.0, 1.0, 0.5], 0.0);
        let pieces = dash.split([0.0, 0.0], [6.0, 0.0]);
        assert_eq!(pieces.len(), 3, "{pieces:?}");
        assert!((pieces[0].1[0] - 2.0).abs() < 1e-9, "{pieces:?}");
        assert!((pieces[1].0[0] - 3.0).abs() < 1e-9, "{pieces:?}");
        assert!((pieces[2].0[0] - 5.5).abs() < 1e-9, "{pieces:?}");
        dash.restart();
        let pieces = dash.split([6.0, 0.0], [8.0, 0.0]);
        assert_eq!(pieces.len(), 1, "{pieces:?}");
        assert!((pieces[0].1[0] - 8.0).abs() < 1e-9, "{pieces:?}");
    }

    #[test]
    fn a_long_dashed_run_keeps_its_pattern_past_the_eighth_period() {
        // I井场's signal line at x 551.78: one 395.5 mm piece of the
        // 1.905/1.270 pattern, 124 periods. The walk used to stop after eight
        // and read the remaining 368 mm as one solid stroke.
        let mut dash = DashWalk::new(&[1.905, 1.27], 0.0);
        let pieces = dash.split([0.0, 0.0], [0.0, 395.5]);
        assert_eq!(pieces.len(), 125, "{}", pieces.len());
        let longest = pieces
            .iter()
            .map(|(start, end)| end[1] - start[1])
            .fold(0.0_f64, f64::max);
        assert!((longest - 1.905).abs() < 1e-9, "{longest}");
        assert!(
            (pieces[124].0[1] - 124.0 * 3.175).abs() < 1e-9,
            "{:?}",
            pieces[124]
        );
    }

    #[test]
    fn a_pattern_too_fine_to_advance_still_ends() {
        // Entries under the walk's own epsilon never advance it; the run is
        // drawn solid rather than walked forever.
        let mut dash = DashWalk::new(&[1e-13, 1e-13], 0.0);
        let pieces = dash.split([0.0, 0.0], [5.0, 0.0]);
        assert_eq!(pieces, vec![([0.0, 0.0], [5.0, 0.0])]);
    }

    #[test]
    fn an_empty_or_zero_pattern_inks_the_whole_run() {
        for pattern in [&[][..], &[0.0, 0.0][..]] {
            let mut dash = DashWalk::new(pattern, 3.0);
            let pieces = dash.split([0.0, 0.0], [40.0, 10.0]);
            assert_eq!(pieces, vec![([0.0, 0.0], [40.0, 10.0])]);
        }
    }

    #[test]
    fn a_frame_line_is_not_ink_but_a_page_long_dash_is() {
        let page = [1189.0, 841.0];
        assert!(sheet_edge_mm([0.0, 0.0], [1189.0, 0.0], page));
        assert!(!sheet_edge_mm([0.0, 0.0], [1050.0, 0.0], page));
    }

    #[test]
    fn a_tail_that_leaves_its_clip_from_the_edge_inks_nothing() {
        // The I井场 sheet frame's last device unit, in page-view points.
        let clip = Rect::new(0.0, 0.253, 3370.56, 2384.413);
        let tail = clip_segment(clip, Point::new(0.0, 0.253), Point::new(0.0, -0.227));
        assert_eq!(tail, None);
    }

    #[test]
    fn a_segment_that_crosses_its_clip_keeps_the_inside() {
        let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
        let (start, end) =
            clip_segment(clip, Point::new(-5.0, 5.0), Point::new(5.0, 5.0)).expect("inside");
        assert!((start - Point::new(0.0, 5.0)).hypot() < 1e-9, "{start:?}");
        assert!((end - Point::new(5.0, 5.0)).hypot() < 1e-9, "{end:?}");
        let edge = clip_segment(clip, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert_eq!(edge, Some((Point::new(0.0, 0.0), Point::new(10.0, 0.0))));
        assert_eq!(
            clip_segment(clip, Point::new(11.0, 0.0), Point::new(11.0, 10.0)),
            None
        );
        let none = Rect::new(5.0, 5.0, 5.0, 10.0);
        assert_eq!(
            clip_segment(none, Point::new(5.0, 5.0), Point::new(5.0, 10.0)),
            None
        );
    }

    #[test]
    fn the_reader_keeps_only_the_ink_inside_the_clip() {
        // A polyline that ends on the clip's top edge and steps one device
        // unit past it, and a stroke that crosses the same edge.
        let paths = "0.48 w 20 20 m 20 99.52 l 20 100 l 50 50 m 50 120 l S";
        let clipped = read_plot(&one_page_pdf(
            &format!("q 0 99.52 100 -100 re W n {paths} Q"),
            100.0,
        ))
        .expect("read");
        let ends: Vec<[f64; 2]> = clipped
            .segments
            .iter()
            .map(|segment| segment.end_mm)
            .collect();
        let top_mm = 99.52 * 25.4 / 72.0;
        assert_eq!(ends.len(), 2, "{ends:?}");
        assert!(
            ends.iter().all(|end| (end[1] - top_mm).abs() < 1e-6),
            "{ends:?}"
        );
        let open = read_plot(&one_page_pdf(&format!("q {paths} Q"), 100.0)).expect("read");
        assert_eq!(open.segments.len(), 3, "{:?}", open.segments);
    }

    #[test]
    fn a_path_painted_in_the_paper_colour_is_paint_but_not_ink() {
        // The I井场 reference strokes a `Remarks.sym` origin mark this way:
        // `1 G` and three short pieces, between black strokes at `0 G`. The
        // same white is stated here in grey and in RGB, stroked and filled.
        let content = "0.48 w \
            0 G 10 10 m 60 10 l S \
            1 G 10 30 m 60 30 l S \
            1 1 1 RG 10 50 m 60 50 l S \
            0 0 0 RG 10 70 m 60 70 l S \
            1 g 10 80 m 60 80 l 60 85 l h f \
            0 g 10 90 m 60 90 l 60 95 l h f";
        let pdf = one_page_pdf(content, 100.0);
        let row = |segment: &super::InkSegment| (segment.start_mm[1] * 72.0 / 25.4).round();

        // The reading every comparison has used so far: paint is ink.
        let all = read_plot(&pdf).expect("read");
        assert_eq!(all.segments.len(), 4 + 2 * 3, "{:?}", all.segments);
        assert_eq!(read_plot_painted(&pdf, Painted::All).expect("read"), all);

        // What prints: the two black strokes and the black fill's outline.
        let inked = read_plot_painted(&pdf, Painted::Inked).expect("read");
        let rows: Vec<f64> = inked.segments.iter().map(row).collect();
        assert_eq!(inked.segments.len(), 2 + 3, "{rows:?}");
        assert!(
            rows.iter().all(|y| [10.0, 70.0, 90.0, 95.0].contains(y)),
            "{rows:?}"
        );

        // What does not: the two white strokes and the white fill's outline.
        let paper = read_plot_painted(&pdf, Painted::Paper).expect("read");
        let rows: Vec<f64> = paper.segments.iter().map(row).collect();
        assert_eq!(paper.segments.len(), 2 + 3, "{rows:?}");
        assert!(
            rows.iter().all(|y| [30.0, 50.0, 80.0, 85.0].contains(y)),
            "{rows:?}"
        );
    }

    #[test]
    fn a_stroke_along_the_sheet_is_left_out_of_the_ink_and_listed_as_an_edge() {
        // A 100 pt page. Three strokes run more than nine tenths of it: a
        // thin black one along the bottom, a 3.84 pt one up the right side —
        // the A2 reference's inner frame is stroked `3.84 w` — and a white
        // one along the top. A fourth stops at half the page.
        let content = "0.48 w 0 G 2 5 m 98 5 l S \
            3.84 w 95 2 m 95 98 l S \
            0.48 w 1 G 2 95 m 98 95 l S \
            0 G 10 50 m 60 50 l S";
        let pdf = one_page_pdf(content, 100.0);
        let mm = 25.4 / 72.0;

        // The ink is the short stroke alone, as it always was.
        let ink = read_plot(&pdf).expect("read");
        assert_eq!(ink.segments.len(), 1, "{:?}", ink.segments);
        assert!((ink.segments[0].start_mm[1] - 50.0 * mm).abs() < 1e-9);

        // The three long ones are the page's sheet edges, in drawing order,
        // each with the width it is stroked at and whether it prints.
        let pens = read_plot_pens(&pdf).expect("read");
        let edges: Vec<(f64, f64, bool)> = pens
            .sheet_edges
            .iter()
            .map(|edge| {
                (
                    (edge.end_mm[0] - edge.start_mm[0]).hypot(edge.end_mm[1] - edge.start_mm[1]),
                    edge.width_mm,
                    edge.paper,
                )
            })
            .collect();
        assert_eq!(edges.len(), 3, "{edges:?}");
        for (edge, (width_pt, paper)) in
            edges
                .iter()
                .zip([(0.48, false), (3.84, false), (0.48, true)])
        {
            assert!((edge.0 - 96.0 * mm).abs() < 1e-6, "{edges:?}");
            assert!((edge.1 - width_pt * mm).abs() < 1e-6, "{edges:?}");
            assert_eq!(edge.2, paper, "{edges:?}");
        }
        // Where: the second one runs up the page at x 95 pt.
        let up = &pens.sheet_edges[1];
        assert!((up.start_mm[0] - 95.0 * mm).abs() < 1e-9, "{up:?}");
        assert!((up.start_mm[1] - 2.0 * mm).abs() < 1e-9, "{up:?}");
        assert!((up.end_mm[1] - 98.0 * mm).abs() < 1e-9, "{up:?}");
        // Read as ink or as paper, the page lists the same edges: they are
        // the page's, not a reading's.
        assert!(
            read_plot_painted(&pdf, Painted::Inked)
                .expect("read")
                .segments
                .len()
                == 1
        );
    }

    #[test]
    fn an_edge_outside_its_clip_is_not_an_edge() {
        // The same bottom stroke under a clip that stops short of it.
        let content = "q 0 100 100 -90 re W n 0.48 w 2 5 m 98 5 l S Q";
        let pens = read_plot_pens(&one_page_pdf(content, 100.0)).expect("read");
        assert!(pens.sheet_edges.is_empty(), "{:?}", pens.sheet_edges);
        assert!(pens.ends.is_empty(), "{:?}", pens.ends);
        assert!(pens.inks.is_empty(), "{:?}", pens.inks);
    }

    #[test]
    fn an_edge_on_its_clips_own_edge_prints_half_its_pen_and_is_an_edge() {
        // The A2 reference clips at `x 0.48` pt and strokes its left sheet
        // edge at `x 0.48` with a 0.48 pt pen: the centre line is on the
        // clip's edge — a hair outside it once the coordinates are floats —
        // and half the pen prints. A stroke a whole pen outside prints
        // nothing.
        let clip = "q 0.48 100 99.52 -100 re W n 0.48 w";
        let on_the_edge = format!("{clip} 0.47 98 m 0.47 2 l S Q");
        let pens = read_plot_pens(&one_page_pdf(&on_the_edge, 100.0)).expect("read");
        assert_eq!(pens.sheet_edges.len(), 1, "{:?}", pens.sheet_edges);
        let outside = format!("{clip} 0 98 m 0 2 l S Q");
        let pens = read_plot_pens(&one_page_pdf(&outside, 100.0)).expect("read");
        assert!(pens.sheet_edges.is_empty(), "{:?}", pens.sheet_edges);
    }

    #[test]
    fn the_caps_and_joins_are_tallied_over_what_prints() {
        // The reference plots stroke their first paths before they state a
        // cap — butt ends, mitred corners, PDF's defaults — and every later
        // one under `1 j 1 J`. Two paths under the defaults, then two under
        // round ends, one of them white; then a filled triangle.
        let content = "0.48 w 0 G 10 10 m 60 10 l S \
            10 20 m 40 20 l 40 30 l S \
            1 j 1 J 10 40 m 30 40 l S \
            1 G 10 50 m 90 50 l S \
            0 g 10 60 m 20 60 l 20 70 l h f";
        let pens = read_plot_pens(&one_page_pdf(content, 100.0)).expect("read");
        let mm = 25.4 / 72.0;
        assert_eq!(pens.ends.len(), 2, "{:?}", pens.ends);
        let butt = &pens.ends[0];
        assert_eq!((butt.cap, butt.join), (LineCap::Butt, LineJoin::Miter));
        assert_eq!(butt.paths, 2);
        assert!(
            (butt.length_mm - (50.0 + 30.0 + 10.0) * mm).abs() < 1e-6,
            "{butt:?}"
        );
        // Piece by piece, in drawing order: the line, then the two legs of
        // the corner.
        assert_eq!(butt.strokes.len(), 3, "{butt:?}");
        assert!((butt.strokes[0].end_mm[0] - 60.0 * mm).abs() < 1e-9);
        assert!((butt.strokes[2].start_mm[0] - 40.0 * mm).abs() < 1e-9);
        assert!((butt.strokes[2].end_mm[1] - 30.0 * mm).abs() < 1e-9);
        let round = &pens.ends[1];
        assert_eq!((round.cap, round.join), (LineCap::Round, LineJoin::Round));
        // The white stroke has no end to show: one path, 20 pt.
        assert_eq!(round.paths, 1);
        assert!((round.length_mm - 20.0 * mm).abs() < 1e-6, "{round:?}");
        assert_eq!(round.strokes.len(), 1, "{round:?}");
        assert!((round.strokes[0].start_mm[1] - 40.0 * mm).abs() < 1e-9);
        // A width is a single-precision number in the page.
        assert!((round.strokes[0].width_mm - 0.48 * mm).abs() < 1e-6);
        // One ink: three black strokes and a black fill. The white stroke
        // states none.
        assert_eq!(
            pens.inks,
            vec![InkColour {
                rgba: Some([0, 0, 0, 255]),
                paths: 4,
                glyphs: 0,
            }]
        );
    }

    /// What is painted later lies on top, and the reader says which that
    /// is: every path and every glyph has its place in one count.
    #[test]
    fn a_page_is_read_in_the_order_it_is_painted() {
        use super::read_plot_sequence;
        // A black rule, a white stroke across it, a second black rule, and
        // a filled triangle: four paths, the white one second.
        let content = "0.48 w 0 G 10 10 m 60 10 l S \
            1 G 30 5 m 30 15 l S \
            0 G 10 20 m 60 20 l 60 30 l S \
            0 g 10 60 m 20 60 l 20 70 l h f";
        let page = read_plot_sequence(&one_page_pdf(content, 100.0)).expect("read");
        let steps: Vec<(u32, bool, bool)> = page
            .strokes
            .iter()
            .map(|stroke| (stroke.step, stroke.paper, stroke.filled))
            .collect();
        assert_eq!(
            steps,
            vec![
                (0, false, false),
                (1, true, false),
                // Both legs of the third path under the path's one step.
                (2, false, false),
                (2, false, false),
                // The triangle's three edges.
                (3, false, true),
                (3, false, true),
                (3, false, true),
            ]
        );
        let mm = 25.4 / 72.0;
        // The white stroke where it is, in the paper's millimetres.
        let white = &page.strokes[1].segment;
        assert!((white.start_mm[0] - 30.0 * mm).abs() < 1e-9, "{white:?}");
        assert!((white.width_mm - 0.48 * mm).abs() < 1e-6, "{white:?}");
        assert!(page.glyphs.is_empty());
        // The reading that keeps no order is what it was.
        assert_eq!(
            read_plot(&one_page_pdf(content, 100.0))
                .expect("read")
                .segments
                .len(),
            7
        );
    }

    #[test]
    fn a_second_ink_is_a_second_colour() {
        // Every reference plot inks in black alone; a page that strokes one
        // path in grey says so.
        let content = "0.48 w 0 G 10 10 m 60 10 l S 0.5 G 10 20 m 60 20 l S";
        let pens = read_plot_pens(&one_page_pdf(content, 100.0)).expect("read");
        let colours: Vec<Option<[u8; 4]>> = pens.inks.iter().map(|ink| ink.rgba).collect();
        assert_eq!(colours.len(), 2, "{colours:?}");
        assert_eq!(colours[0], Some([0, 0, 0, 255]));
        assert!(
            pens.inks.iter().all(|ink| ink.paths == 1),
            "{:?}",
            pens.inks
        );
    }

    #[test]
    fn a_light_grey_is_ink() {
        // Only the paper's own colour inks nothing. A pale stroke still
        // prints, so it is not dropped for being nearly white.
        let pdf = one_page_pdf("0.48 w 0.99 G 10 10 m 60 10 l S", 100.0);
        let inked = read_plot_painted(&pdf, Painted::Inked).expect("read");
        assert_eq!(inked.segments.len(), 1, "{:?}", inked.segments);
        let paper = read_plot_painted(&pdf, Painted::Paper).expect("read");
        assert!(paper.segments.is_empty(), "{:?}", paper.segments);
    }

    /// A one-page PDF, `side` points square, that runs `content`.
    fn one_page_pdf(content: &str, side: f64) -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {side} {side}] \
                 /Resources << >> /Contents 4 0 R >>"
            ),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        let count = objects.len() + 1;
        pdf.extend_from_slice(format!("xref\n0 {count}\n0000000000 65535 f \n").as_bytes());
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
        pdf
    }
}
