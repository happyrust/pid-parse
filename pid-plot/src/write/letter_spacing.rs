//! The letter-spacing of a text object, as the reference plots' driver
//! states it.
//!
//! The plotter hands GDI a string with the whole device unit each glyph is
//! stepped by ([`super::run_advance`]). A glyph's own advance in its font is
//! not a whole unit, and the PDF driver, which writes the string as one text
//! object, states what the steps leave over once, as the letter-spacing
//! `Tc`: one amount after every glyph of the object. This module holds that
//! amount ([`of`]).
//!
//! # The rule
//!
//! ```text
//! advance = round(a glyph's advance in the device font × 256) / 256
//! Tc      = cut((S − Σ advance) × 0.48 / ((n − 1) × m)), to three decimals
//! ```
//!
//! for a text object of `n` glyphs, the sum running over its first `n − 1`.
//! `S` is the whole device units (1/150 in) from the first glyph's origin to
//! the last one's, which is the steps of those `n − 1` glyphs; a device unit
//! is 0.48 pt; `m` is what the text matrix states along the baseline, in the
//! 256ths the driver keeps ([`super::text_matrix::TextMatrix::spacing`]) —
//! `1` for a font written plain — and the quotient is cut toward nought at
//! its third decimal, a trailing nought not written.
//!
//! It is the fixed point of the text matrix again: 256ths. A glyph's advance
//! is taken to the *nearest* 256th of a device unit; the quotient is *cut*.
//!
//! The spacing follows every glyph of the object, the last one too, where
//! it moves nothing that is drawn: the driver starts every text object under
//! a text matrix of its own. So a string is written whole here as well, and
//! only one that another piece of its label carries on from keeps its last
//! glyph apart (`last_glyph_apart` in the writer).
//!
//! # Where it comes from
//!
//! The sixth review found it by search over the eleven reference plots. They
//! hold 3 986 text objects of more than one glyph, all of them upright — a
//! turned label is written a glyph an object and states no spacing — and the
//! rule prints the `Tc` of every one, in five faces on seventeen fonts, nine
//! of them plain and eight condensed SimSun. No neighbouring rule does: with
//! the advances left unrounded 2 434 of the 3 805 plain ones come out, to a
//! 512th 2 894, to a 1024th 2 545, and rounding the quotient or cutting it
//! toward minus infinity under 2 900.
//!
//! It is also the shape of the text routine of the driver's version 5.5.2.7,
//! the one at hand (the plots were written by 4.5.2.9): a glyph's advance as
//! a whole number of 256ths of a device unit, its step in whole units, the
//! difference summed over every interval of the object, and that sum divided
//! by the intervals times the matrix's 256ths. Two things are not in that
//! text and rest on the plots alone: that an advance is rounded to its 256th
//! rather than cut, and the cut of the quotient to three decimals.
//!
//! # What it does to the page
//!
//! A cut spacing sets the glyphs a hair off the plotter's steps, as far as
//! the gaps of the object add the cut up: under a thousandth of a point a
//! gap, 0.007 mm over the longest object. What the driver's matrix does is
//! larger — a condensed font's entry is written to three decimals of a 256th
//! — and both are in the reference: the last glyph of the title block's
//! company name, 21 SimSun glyphs condensed to `0.77`, is drawn 604.578
//! device units after the first where its step is 604, on our page as on the
//! plot. `S` is the plotter's steps, never the place a glyph is drawn at.
//!
//! # What the reference plots do not show
//!
//! - an advance exactly halfway between two 256ths, rounded away from nought
//!   here;
//! - a turned string under a spacing: every turned glyph is a text object of
//!   its own on the plots, and is set on its own step here;
//! - a string of more than one font. The driver would write a text object
//!   for each font, each under its own matrix on a whole unit; here a piece
//!   in another font carries on from where the piece before it ended, and
//!   that piece keeps its last glyph apart so that it ends where it did.
//!
//! Read back off the eleven pages this crate writes for those plots
//! (`tools/pdf_letter_spacing.py`): of the
//! 3 986 text objects, 3 904 pair with one of ours by place and text and 80
//! more by text alone, each of ours the same string whole under a `Tc`
//! written character for character as the reference's; the other two are
//! strings the drawing on hand does not hold.

use super::PLOT_DPI;
use super::script_runs::Stepped;

/// The driver's fixed point: 256ths of a device unit, and of a text matrix's
/// entry.
const FRACTION: f64 = 256.0;

/// The letter-spacing of a text object whose glyphs are `glyphs`, in points
/// of text space, under a text matrix that states `along` along the
/// baseline in the 256ths the driver keeps.
///
/// A string of one glyph has no gap to space, and neither has none.
pub(super) fn of(glyphs: &[Stepped], along: f64) -> f64 {
    let Some((_, leading)) = glyphs.split_last() else {
        return 0.0;
    };
    if leading.is_empty() || !(along.is_finite() && along > 0.0) {
        return 0.0;
    }
    // Both in 256ths of a device unit, and both whole numbers: what the
    // plotter steps the leading glyphs by, and their own advances.
    #[allow(clippy::cast_precision_loss)]
    let stepped: f64 = leading
        .iter()
        .map(|glyph| glyph.step_px as f64 * FRACTION)
        .sum();
    let advances: f64 = leading
        .iter()
        .map(|glyph| (glyph.natural_px * FRACTION).round())
        .sum();
    #[allow(clippy::cast_precision_loss)]
    let gaps = leading.len() as f64;
    // Thousandths of a point. A device unit is 72/150 pt, so the numerator
    // is a whole number, and so is the denominator wherever the matrix is in
    // whole 256ths: the division is then exact where the quotient is a whole
    // number and a millionth or more from one where it is not, and the cut
    // is the cut of the true quotient.
    let thousandths =
        (stepped - advances) * (1000.0 * 72.0 / PLOT_DPI) / (gaps * (along * FRACTION));
    if !thousandths.is_finite() {
        return 0.0;
    }
    // Nought is written `0`, whichever side it was cut from.
    let cut = thousandths.trunc() / 1000.0;
    if cut == 0.0 { 0.0 } else { cut }
}

#[cfg(test)]
mod tests {
    use super::{Stepped, of};

    /// The glyphs of a text object from the design advances of its leading
    /// glyphs, the device units a design unit is along the baseline, as a
    /// fraction, and the steps the plotter gives them. The last glyph, which
    /// the spacing does not read, is a copy of the one before it.
    fn object(advances: &[u32], along: (u32, u32), steps: &[i64]) -> Vec<Stepped> {
        assert_eq!(advances.len(), steps.len());
        let mut glyphs: Vec<Stepped> = advances
            .iter()
            .zip(steps)
            .map(|(advance, step)| Stepped {
                step_px: *step,
                natural_px: f64::from(*advance) * f64::from(along.0) / f64::from(along.1),
            })
            .collect();
        glyphs.push(*glyphs.last().expect("a glyph"));
        glyphs
    }

    /// The title block's company name, `中国石油集团工程设计有限责任公司华北分公司`:
    /// 21 SimSun glyphs of 256 design units on a 39-unit cell at a stated
    /// width of 15, so each is 30 device units wide; the plotter steps them
    /// 604 units in all, and the matrix is `0.77`, 197/256. The reference
    /// writes `Tc 0.124` on seven sheets. The A2 sheet's two lines are the
    /// same face on 67 units at a width of 26, 52 units a glyph, under
    /// `0.773`, 198/256: twelve glyphs over 575 units read `0.169` and five
    /// over 209 read `0.155`.
    #[test]
    fn the_title_blocks_condensed_lines_read_what_the_reference_writes() {
        let steps = |total: i64, gaps: usize| -> Vec<i64> {
            // The spacing reads the sum of the steps alone.
            let mut steps = vec![0; gaps];
            steps[0] = total;
            steps
        };
        for (gaps, width, total, entry, written) in [
            (20usize, 15u32, 604i64, 197.0f64, 0.124f64),
            (11, 26, 575, 198.0, 0.169),
            (4, 26, 209, 198.0, 0.155),
        ] {
            let glyphs = object(&vec![256; gaps], (2 * width, 256), &steps(total, gaps));
            let spacing = of(&glyphs, entry / 256.0);
            assert_eq!(
                spacing.to_bits(),
                written.to_bits(),
                "{gaps} gaps over {total} units at width {width}: {spacing}, the reference \
                 writes {written}"
            );
        }
    }

    /// The certificate line of the A2 sheet: SimSun on a cell of 24 device
    /// units with a stated width of 10, so a full-width glyph is 20 units
    /// wide and a half-width one 10, stepped 19 and 10 or 9. The reference's
    /// first two text objects are spaced `Tc -0.494` and `Tc -0.288` under a
    /// matrix of 213/256.
    #[test]
    fn the_certificate_line_reads_what_the_reference_writes() {
        let glyph = |step_px: i64, natural_px: f64| Stepped {
            step_px,
            natural_px,
        };
        let mut glyphs = vec![glyph(19, 20.0); 6];
        glyphs.extend([glyph(10, 10.0), glyph(10, 10.0)]);
        for step in [9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9] {
            glyphs.push(glyph(step, 10.0));
        }
        let written = 213.0 / 256.0;
        assert_eq!(of(&glyphs[..8], written).to_bits(), (-0.494f64).to_bits());
        assert_eq!(of(&glyphs[8..19], written).to_bits(), (-0.288f64).to_bits());
    }

    /// An advance is taken to the nearest 256th of a device unit before it
    /// is taken from the steps, and the quotient is cut, not rounded. One
    /// gap of a glyph 9.6494 units wide — 2470.2 256ths, taken as 2470 —
    /// stepped 10 units: `(2560 − 2470) / 256 × 0.48 = 0.16875`, written
    /// `0.168`. The advance as it stands gives `0.16836…`, also `0.168`; one
    /// of 9.6504 units, 2470.5 256ths, is taken as 2471 and gives `0.166`
    /// where the advance as it stands would give `0.167`.
    #[test]
    fn an_advance_is_taken_to_its_nearest_256th_and_the_quotient_is_cut() {
        let one_gap = |natural_px: f64| {
            let glyph = Stepped {
                step_px: 10,
                natural_px,
            };
            of(&[glyph, glyph], 1.0)
        };
        assert_eq!(one_gap(2470.2 / 256.0).to_bits(), 0.168f64.to_bits());
        assert_eq!(one_gap(2470.5 / 256.0).to_bits(), 0.166f64.to_bits());
        assert_eq!(one_gap(2470.49 / 256.0).to_bits(), 0.168f64.to_bits());
        // A spacing below nought is cut toward nought too: −0.1669 is
        // written −0.166, not −0.167.
        let wide = Stepped {
            step_px: 9,
            natural_px: 2393.0 / 256.0,
        };
        assert_eq!(of(&[wide, wide], 1.0).to_bits(), (-0.166f64).to_bits());
    }

    /// The spacing is stated against the matrix's entry: the same glyphs and
    /// steps under a font written `0.797`, 204/256, are spaced by 256/204 of
    /// what they are under a plain one, before the cut.
    #[test]
    fn the_spacing_is_divided_by_the_matrix_entry() {
        let glyph = Stepped {
            step_px: 20,
            natural_px: 19.0,
        };
        let glyphs = [glyph; 4];
        // One unit a gap: 0.48 pt plain, 0.48 × 256 / 204 = 0.60235… condensed.
        assert_eq!(of(&glyphs, 1.0).to_bits(), 0.48f64.to_bits());
        assert_eq!(of(&glyphs, 204.0 / 256.0).to_bits(), 0.602f64.to_bits());
    }

    /// A string of one glyph or of none has no gap; a spacing that cuts to
    /// nought is nought, never minus nought; and a matrix that is no number
    /// spaces nothing.
    #[test]
    fn nothing_to_space_is_nought() {
        let glyph = Stepped {
            step_px: 10,
            natural_px: 10.0,
        };
        assert_eq!(of(&[], 1.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(of(&[glyph], 1.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(of(&[glyph, glyph], 1.0).to_bits(), 0.0f64.to_bits());
        // A 256th of a unit over, on two gaps: −0.0009 pt, cut to nought.
        let hair = Stepped {
            step_px: 10,
            natural_px: 10.0 + 1.0 / 256.0,
        };
        assert_eq!(of(&[hair, glyph, glyph], 1.0).to_bits(), 0.0f64.to_bits());
        for along in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(of(&[hair, glyph], along).to_bits(), 0.0f64.to_bits());
        }
    }
}
