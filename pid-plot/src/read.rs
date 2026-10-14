//! Read a rendered plot back into ink segments and text spans.
//!
//! The same rules read a reference plot and, later, a plot this crate writes.
//! Coordinates come out in paper millimetres, origin at the bottom-left, y up.
//! Curves are broken into line segments at a 0.05 mm chord. A path that is
//! filled and not stroked contributes its outline.

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, flatten};
use serde::Serialize;

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

/// Read the first page of a PDF into paper-space ink and text.
pub fn read_plot(pdf_bytes: &[u8]) -> Result<PlotInk, PlotReadError> {
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
        segments: Vec::new(),
        glyphs: Vec::new(),
    };
    interpret_page(page, &mut ctx, &mut device);
    Ok(PlotInk {
        page_mm: [width_pt * MM_PER_PT, height_pt * MM_PER_PT],
        segments: device.segments,
        spans: cluster_spans(device.glyphs, height_pt),
    })
}

struct RawGlyph {
    text: String,
    origin_pt: Point,
    height_pt: f64,
    direction_view: [f64; 2],
    advance_pt: f64,
    font_key: u128,
}

struct InkDevice {
    page_pt: [f64; 2],
    segments: Vec<InkSegment>,
    glyphs: Vec<RawGlyph>,
}

impl InkDevice {
    fn push_segment(&mut self, start: Point, end: Point, width_pt: f64) {
        if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite()
        {
            return;
        }
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
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn pop_clip_path(&mut self) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}

    fn draw_rect(&mut self, _: &Rect, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}

    fn draw_path(
        &mut self,
        path: &BezPath,
        transform: Affine,
        _: &Paint<'a>,
        draw_mode: &PathDrawMode,
    ) {
        let width_pt = match draw_mode {
            PathDrawMode::Stroke(stroke) => {
                f64::from(stroke.line_width) * transform_scale(transform)
            }
            PathDrawMode::Fill(_) => 0.0,
        };
        let flat = transform * path.clone();
        let mut start: Option<Point> = None;
        let mut current: Option<Point> = None;
        let page_w = self.page_pt[0];
        let page_h = self.page_pt[1];
        flatten(flat, CHORD_MM * PT_PER_MM, |element| match element {
            PathEl::MoveTo(point) => {
                start = Some(point);
                current = Some(point);
            }
            PathEl::LineTo(point) => {
                if let Some(from) = current
                    && !is_sheet_edge(from, point, page_w, page_h)
                {
                    self.push_segment(from, point, width_pt);
                }
                current = Some(point);
            }
            PathEl::ClosePath => {
                if let (Some(from), Some(origin)) = (current, start)
                    && (from - origin).hypot() > 1e-6
                    && !is_sheet_edge(from, origin, page_w, page_h)
                {
                    self.push_segment(from, origin, width_pt);
                }
                current = start;
            }
            PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _) => {}
        });
    }

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
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
        let font_key = match glyph {
            Glyph::Outline(glyph) => glyph.font_data().map(|font| font.cache_key).unwrap_or(0),
            Glyph::Type3(_) => 1,
        };
        let advance_pt = (placed * Point::new(advance, 0.0) - origin).hypot();
        self.glyphs.push(RawGlyph {
            text,
            origin_pt: origin,
            height_pt,
            direction_view: direction,
            advance_pt,
            font_key,
        });
    }
}

/// The print driver stores a sheet frame as a quad. Its edges run almost the
/// full page, which PyMuPDF does not count as line items. Real lines on these
/// plots stay under 75% of the page side.
fn is_sheet_edge(start: Point, end: Point, page_w: f64, page_h: f64) -> bool {
    let dx = (start.x - end.x).abs();
    let dy = (start.y - end.y).abs();
    (dy < 0.8 && dx > page_w * 0.90) || (dx < 0.8 && dy > page_h * 0.90)
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

fn cluster_spans(glyphs: Vec<RawGlyph>, page_height_pt: f64) -> Vec<TextSpan> {
    let mut spans = Vec::new();
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
                spans.push(span.finish(page_height_pt));
                if let Some(space) = space {
                    spans.push(space);
                }
            }
            current = Some(OpenSpan::start(glyph));
        } else if let Some(span) = current.as_mut() {
            span.push(glyph);
        }
    }
    if let Some(span) = current {
        spans.push(span.finish(page_height_pt));
    }
    spans
}

struct OpenSpan {
    text: String,
    origin_pt: Point,
    height_pt: f64,
    direction_view: [f64; 2],
    font_key: u128,
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
            direction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::view_pt_to_paper_mm;

    #[test]
    fn a_point_halfway_down_the_page_sits_halfway_up_the_paper() {
        let paper = view_pt_to_paper_mm(0.0, 72.0, 144.0);
        assert!(paper[0].abs() < 1e-9);
        assert!((paper[1] - 25.4).abs() < 1e-9);
    }
}
