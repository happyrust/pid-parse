//! Glyph-level reading shared by the text-residual probes.
//!
//! `read::read_plot` clusters a page's glyphs into runs and reports each run's
//! start, advance and body size, but not the per-glyph facts a residual
//! question turns on: each glyph's own origin, its horizontal scale (`Tm`'s
//! `a` against its `d`), the face it was drawn with, and the letter spacing
//! between neighbours. This module reads exactly those, the same way
//! `probe_why_paired_labels_miss_the_reference` does, so a probe can quote the
//! numbers a rule has to reproduce.
//!
//! Not an example of its own: cargo only looks for `examples/*.rs` and
//! `examples/*/main.rs`, so a file under `examples/support/` is source, not a
//! target. Each probe compiles it in with `#[path = "support/glyphs.rs"]`, so
//! the helper carries items no single probe reads.
#![allow(dead_code)]

use std::collections::HashMap;

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::Pdf;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, Point, Rect};
use pid_plot::read::{PlotInk, TextSpan, view_pt_to_paper_mm};

/// One point, millimetres.
pub const MM_PER_PT: f64 = 25.4 / 72.0;
/// One 150 dpi pixel of the reference plot's device grid, millimetres.
pub const PIXEL_MM: f64 = 25.4 / 150.0;
/// One 150 dpi pixel, points.
pub const PIXEL_PT: f64 = 72.0 / 150.0;

/// One glyph as the page draws it.
#[derive(Clone, Debug)]
pub struct PlacedGlyph {
    /// The character this glyph stands for, as the PDF's `ToUnicode` names it.
    pub text: String,
    /// Baseline origin, paper millimetres, y up.
    pub origin_mm: [f64; 2],
    /// Body size in millimetres.
    pub body_mm: f64,
    /// Horizontal scale against the body: `Tm`'s own, times `Tz`.
    pub sx: f64,
    /// The glyph's own advance on the page, its letter spacing excluded.
    pub advance_mm: f64,
    /// PostScript name of the face, subset prefix stripped.
    pub face: String,
}

/// Collects every glyph of a page, keeping the face and the horizontal scale.
struct GlyphDevice {
    page_height_pt: f64,
    glyphs: Vec<PlacedGlyph>,
}

impl<'a> Device<'a> for GlyphDevice {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}

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
        let (advance, face) = match glyph {
            Glyph::Outline(glyph) => (
                glyph
                    .advance_width()
                    .filter(|width| *width > 0.0)
                    .map_or(500.0, f64::from),
                glyph
                    .font_data()
                    .and_then(|data| data.postscript_name)
                    .map(|name| match name.split_once('+') {
                        Some((prefix, rest)) if prefix.len() == 6 => rest.to_string(),
                        _ => name,
                    })
                    .unwrap_or_default(),
            ),
            Glyph::Type3(glyph) => (
                glyph
                    .advance_width()
                    .filter(|width| width.is_finite() && *width > 0.0)
                    .map_or(600.0, f64::from),
                "Type3".to_string(),
            ),
        };
        let placed = transform * glyph_transform;
        let origin = placed * Point::ORIGIN;
        let height_pt = (placed * Point::new(0.0, 1000.0) - origin).hypot();
        if !height_pt.is_finite() || height_pt > 10_000.0 {
            return;
        }
        let width_pt = (placed * Point::new(1000.0, 0.0) - origin).hypot();
        let advance_pt = (placed * Point::new(advance, 0.0) - origin).hypot();
        self.glyphs.push(PlacedGlyph {
            text,
            origin_mm: view_pt_to_paper_mm(origin.x, origin.y, self.page_height_pt),
            body_mm: height_pt * MM_PER_PT,
            sx: if height_pt > 0.0 {
                width_pt / height_pt
            } else {
                1.0
            },
            advance_mm: advance_pt * MM_PER_PT,
            face,
        });
    }
}

/// Every glyph of a PDF's first page, in drawing order.
pub fn read_glyphs(bytes: &[u8]) -> Option<Vec<PlacedGlyph>> {
    let pdf = Pdf::new(bytes.to_vec()).ok()?;
    let pages = pdf.pages();
    let page = pages.first()?;
    let (width_pt, height_pt) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let settings = InterpreterSettings {
        render_annotations: false,
        ..Default::default()
    };
    let mut ctx = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, f64::from(width_pt), f64::from(height_pt)),
        &cache,
        page.xref(),
        settings,
    );
    let mut device = GlyphDevice {
        page_height_pt: f64::from(height_pt),
        glyphs: Vec::new(),
    };
    interpret_page(page, &mut ctx, &mut device);
    Some(device.glyphs)
}

fn origin_key(point: [f64; 2]) -> (i64, i64) {
    (
        (point[0] * 1e5).round() as i64,
        (point[1] * 1e5).round() as i64,
    )
}

/// The glyphs of one page, findable by the origin a span starts at.
pub struct PageGlyphs {
    glyphs: Vec<PlacedGlyph>,
    by_origin: HashMap<(i64, i64), Vec<usize>>,
}

impl PageGlyphs {
    /// Index a page's glyphs by origin.
    pub fn new(glyphs: Vec<PlacedGlyph>) -> Self {
        let mut by_origin: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (slot, glyph) in glyphs.iter().enumerate() {
            by_origin
                .entry(origin_key(glyph.origin_mm))
                .or_default()
                .push(slot);
        }
        Self { glyphs, by_origin }
    }

    /// The glyphs `span` was clustered from: the run of glyphs, in drawing
    /// order, that starts where the span starts and spells it.
    pub fn of(&self, span: &TextSpan) -> &[PlacedGlyph] {
        let Some(starts) = self.by_origin.get(&origin_key(span.origin_mm)) else {
            return &[];
        };
        for &start in starts {
            let mut text = String::new();
            let mut end = start;
            while end < self.glyphs.len() && text.len() < span.text.len() {
                text.push_str(&self.glyphs[end].text);
                end += 1;
            }
            if text == span.text {
                return &self.glyphs[start..end];
            }
        }
        &[]
    }
}

/// A run of glyphs read off one page, with the per-gap letter spacing.
pub struct Run {
    /// The glyphs of the run, in drawing order.
    pub glyphs: Vec<PlacedGlyph>,
    /// Letter spacing after each glyph that has a successor in this run,
    /// text-space points, indexed like `glyphs` (`NaN` at the last glyph).
    pub gaps_pt: Vec<f64>,
}

impl Run {
    /// The run behind `span`, from `page`.
    pub fn of(page: &PageGlyphs, ink: &PlotInk, run: usize) -> Self {
        let span = &ink.spans[run];
        let glyphs = page.of(span).to_vec();
        let direction = span.direction;
        let gaps_pt = glyphs
            .iter()
            .enumerate()
            .map(|(index, glyph)| {
                glyphs.get(index + 1).map_or(f64::NAN, |next| {
                    let delta = [
                        next.origin_mm[0] - glyph.origin_mm[0],
                        next.origin_mm[1] - glyph.origin_mm[1],
                    ];
                    let step = delta[0] * direction[0] + delta[1] * direction[1];
                    (step - glyph.advance_mm) / MM_PER_PT / glyph.sx
                })
            })
            .collect();
        Self { glyphs, gaps_pt }
    }

    /// Mean letter spacing over the gaps inside this run, points.
    pub fn mean_tc_pt(&self) -> f64 {
        let gaps: Vec<f64> = self
            .gaps_pt
            .iter()
            .copied()
            .filter(|gap| gap.is_finite())
            .collect();
        if gaps.is_empty() {
            0.0
        } else {
            gaps.iter().sum::<f64>() / gaps.len() as f64
        }
    }

    /// Each glyph's origin along the run's writing direction, millimetres from
    /// the run's first glyph.
    pub fn offsets_mm(&self, direction: [f64; 2]) -> Vec<f64> {
        let Some(first) = self.glyphs.first() else {
            return Vec::new();
        };
        self.glyphs
            .iter()
            .map(|glyph| {
                let delta = [
                    glyph.origin_mm[0] - first.origin_mm[0],
                    glyph.origin_mm[1] - first.origin_mm[1],
                ];
                delta[0] * direction[0] + delta[1] * direction[1]
            })
            .collect()
    }
}

/// `value`'s spans on a page: every run whose text contains it, with the run
/// index it came from.
pub fn spans_with<'a>(ink: &'a PlotInk, value: &str) -> Vec<(usize, &'a TextSpan)> {
    ink.spans
        .iter()
        .enumerate()
        .filter(|(_, span)| span.text.contains(value))
        .collect()
}

/// A millimetre reading formatted the way the probes' tables print it.
#[must_use]
pub fn mm(value: f64) -> String {
    if value.is_finite() {
        format!("{value:+.4}")
    } else {
        "     —".to_string()
    }
}
