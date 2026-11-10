//! The lettering of a borrowed character where a reference plot shows it.
//!
//! A character the face its label names cannot draw is borrowed: another
//! face draws it ([`crate::fonts::BORROW_ORDER`]). The writer's general rule
//! sets it at the size of the stretch it stands in and spaces it by the
//! lender's own widths. This module holds what replaces that rule on the
//! lettering the reference plots carry a sample of — the size of the
//! borrowed glyph, and the step of a label borrowed whole — together with
//! the test of whether a label is such lettering ([`witnessed_run`]).
//! Nothing here reaches past the samples: a label they do not show keeps the
//! general rule until a reference plot shows one like it.
//!
//! # The samples
//!
//! Sixteen labels on the eleven acceptance sheets borrow. Fourteen end in a
//! `℃` after Arial Narrow digits (`L=35℃`); two are the A2 sheet's `动火点`,
//! borrowed whole. All sixteen are the drawing's own labels, name Arial
//! Narrow, are upright, state no character width, and stand 2.469 mm or
//! 2.5 mm of cap height tall — both of which the plotter letters on a cell
//! of 24 device units (1/150 in), `Tf 10.17`.
//!
//! The reference sets every borrowed glyph of the sixteen in NSimSun at
//! `Tf 9.6`, an em of 20 device units, under a text matrix with no
//! horizontal scale. Each glyph of `动火点` is placed 9 units after the one
//! before it, so its three full-width glyphs overlap on the reference page.
//!
//! # Where the 20 and the 9 come from
//!
//! The plotter hands GDI the label in the font it names. Nine units is that
//! font's `?`: a character outside the ANSI code page is measured as one in
//! the plotter's reference font ([`super::reference_extent_mm`]), and that
//! measure is the step it asks for after the character.
//!
//! GDI draws the character itself in the font a Chinese Windows associates
//! with a font of the ANSI character set, 新宋体, realized at the *character
//! height* of the font it stands in for — `tmHeight − tmInternalLeading`.
//! For Arial Narrow at a 24-unit cell that is 20. Measured on this machine's
//! GDI, which carries the same association: `动` and `℃` drawn through Arial
//! Narrow at `lfHeight = 24` are the bitmap of 新宋体 at an em of 20, pixel
//! for pixel.
//!
//! The character height is the named face's own device metric, not a
//! proportion of its cell. The same GDI answers 17 for Arial Narrow's 22-unit
//! cell, 19 for 23, 21 for 25 and 24 for 29, and 21 for Arial's 24-unit cell.
//! No reference plot letters a borrowed character on any of those, so no
//! size is written down for them here.
//!
//! # What the samples do not show
//!
//! - a label that names another face, or Arial Narrow on another cell;
//! - a rotated label, a border template's label, a label that states a
//!   character width, or one lettered in more than one style;
//! - a borrowed character with a character of the named face after it
//!   (`总  体  院`): its glyph would be the same 20 units, but no sample shows
//!   the step that follows it, and a guessed step moves every glyph behind;
//! - two or more borrowed characters after ones the named face draws.
//!
//! See `docs/analysis/2026-11-11-a-borrowed-glyph-is-set-at-the-named-fonts-character-height.md`.

use pdfcraft_fonts::pdf::Metrics;

use super::{
    FaceRun, PLOT_DPI, ReadyFace, RunAdvance, ShownText, TextSegment, pixel_steps, plotted_size_pt,
    reference_advance_px, reference_scale, run_width_mm, stepped_advance, whole_pixel_cell,
};
use crate::model::TextHeightBasis;

/// The face the samples name.
const NAMED_FACE: &str = "Arial Narrow";

/// The cell the samples letter that face on, in device units.
const NAMED_CELL: f64 = 24.0;

/// The em the reference sets a borrowed glyph of the samples at, in device
/// units: the character height GDI realizes [`NAMED_FACE`] with at
/// [`NAMED_CELL`].
const BORROWED_EM: f64 = 20.0;

/// The size the reference sets a borrowed glyph of the samples at, in
/// points: `Tf 9.6`.
pub(super) fn size_pt() -> f64 {
    BORROWED_EM * 72.0 / PLOT_DPI
}

/// The run of a label that the samples show how to letter, as an index into
/// `runs`; `None` for a label they do not reach.
///
/// The label has to be lettered the way the samples are — the drawing's own,
/// upright, named in [`NAMED_FACE`] on a [`NAMED_CELL`]-unit cell, with no
/// stated character width, in one style — and what it borrows has to stand
/// where the samples' does: the whole label is one borrowed run, or its last
/// character alone is. That run is the one returned. See the module notes
/// for what falls outside and why.
pub(super) fn witnessed_run(
    faces: &[ReadyFace<'_>],
    label: &ShownText<'_>,
    height_mm: f64,
    upright: bool,
    segments: &[TextSegment<'_>],
    runs: &[(f64, FaceRun<'_>)],
) -> Option<usize> {
    if label.typeface != NAMED_FACE
        || label.furniture
        || !upright
        || label.width_permille.is_some_and(|width| width != 1000)
    {
        return None;
    }
    let named = faces.iter().find(|face| face.key == NAMED_FACE)?;
    if whole_pixel_cell(named.heights, label.height_basis, height_mm) != Some(NAMED_CELL) {
        return None;
    }
    let [style] = segments else {
        return None;
    };
    let named_pt = plotted_size_pt(named.heights, label.height_basis, height_mm);
    if style.typeface != NAMED_FACE || (style.size_pt - named_pt).abs() > 1e-9 {
        return None;
    }
    let ((_, last), before) = runs.split_last()?;
    if !last.borrowed || before.iter().any(|(_, run)| run.borrowed) {
        return None;
    }
    let whole = before.is_empty();
    let lone = last.face.metrics.codes(&last.bytes).len() == 1;
    (whole || lone).then_some(runs.len() - 1)
}

/// How the run [`witnessed_run`] names advances, set at [`size_pt`].
///
/// A lone borrowed glyph — the `℃` that ends `L=35℃` — has no glyph after
/// it to step to and takes its own width. In a label borrowed whole, every
/// glyph but the last steps by what the plotter measures it as: the `?` of
/// `named`, the face the label names, in the plotter's reference font,
/// rounded to whole device units the way every other step is
/// ([`pixel_steps`]). The glyphs themselves are a full em of the lender
/// wide, so they overlap, as they do on the reference page.
///
/// `sx` is the horizontal scale of the label's text matrix and `device` the
/// scale of the plotter's own sheet, as [`super::run_advance`] takes them.
#[allow(clippy::too_many_arguments)]
pub(super) fn advance(
    named: &ReadyFace<'_>,
    lender: &Metrics,
    bytes: &[u8],
    basis: TextHeightBasis,
    height_mm: f64,
    size_pt: f64,
    sx: f64,
    device: f64,
) -> RunAdvance {
    let widths_em: Vec<f64> = lender
        .codes(bytes)
        .iter()
        .map(|(code, _)| lender.width(*code))
        .collect();
    if widths_em.len() < 2 || !(sx.is_finite() && sx > 0.0) {
        return RunAdvance {
            tc_pt: 0.0,
            width_mm: run_width_mm(lender, size_pt, bytes),
            steps_px: Vec::new(),
        };
    }
    let cell = named.heights.ascent + named.heights.descent;
    let question_em = named
        .metrics
        .encode("?")
        .and_then(|encoded| {
            named
                .metrics
                .codes(&encoded)
                .first()
                .map(|(code, _)| named.metrics.width(*code))
        })
        .unwrap_or(0.0);
    let a8 = reference_scale(named.key, named.heights, basis, height_mm * device);
    // A label the samples reach states no character width, so the reference
    // font is at its natural proportions.
    let question_px = reference_advance_px(named.key, u32::from('?'), question_em, cell, 1.0) * a8;
    let steps = pixel_steps(std::iter::repeat_n(question_px, widths_em.len()));
    stepped_advance(
        steps,
        &widths_em,
        size_pt * PLOT_DPI / 72.0,
        size_pt,
        sx,
        true,
    )
}
