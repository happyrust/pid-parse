//! The text matrix as the reference plots' driver writes it.
//!
//! A label is scaled twice on its way to the page: `Tf` states the size of
//! its font, and the text matrix `Tm` scales that along the baseline and
//! across it. The plotter's own arithmetic — the cell it asks GDI for, the
//! average character width a style states ([`super::permille_scale`]) — says
//! what the two scales are: the stretch along the baseline, and `1` across
//! it. The reference plots do not write those numbers. Every text matrix on
//! them that is not the plain `1 0 0 1` is a whole number of 256ths, written
//! to three decimals, and it is neither the stretch rounded nor the stretch
//! truncated: a SimSun label at `800` on a 30-unit cell is stretched by 0.8
//! and written `0.797`; on a 26-unit cell it is stretched by 0.7692 and
//! written `0.77`; and a turned Arial label, stretched by nothing at all, is
//! written `0 1.012 -1.012 0`.
//!
//! This module holds the arithmetic that gives those numbers ([`written`]).
//! It is the PDF driver's, not GDI's and not the plotter's: GDI condenses a
//! font by exactly the stretch it is asked for (read off unhinted outlines to
//! one part in a million), and the driver then states that font's transform
//! in its own fixed point.
//!
//! # The rule
//!
//! The driver is handed the font's transform from design units to device
//! units: `across` device units per design unit across the baseline — the
//! font's em in device units over its units per em — and `along` along it,
//! `across` times the stretch. It writes
//!
//! ```text
//! length = trunc(across × 1 250 000)     the vector across, as a whole number
//! q      = length div 256                a whole-number division
//! n      = trunc(e × 1 250 000 / q)      for e = along and e = across
//! Tm     = n / 256, to three decimals
//! ```
//!
//! so both entries come out larger than the transform by `length / (256 q)`,
//! a factor between 1 and `1 + 1/q` that depends on nothing but how many
//! device units one design unit of the font is: under 0.2 % for a SimSun of
//! 256 units per em at these sizes, which is why its entry across still
//! reads `1`, and 0.8 % to 1.5 % for an Arial or Arial Narrow of 2048. A font
//! that is upright and not stretched never reaches this arithmetic: the
//! driver writes `1 0 0 1` for it outright.
//!
//! # Where it comes from
//!
//! The reference plots were written by version 4.5.2.9 of the driver, which
//! is not at hand. Version 5.0.1.9 is, and its text-matrix routine computes
//! `len = hypot(eM21 × 1250000, eM22 × 1250000)`, `a = len / 256`,
//! `scale = 1250000 / a`, and prints each element of the transform times
//! `scale`, over 256 — the same steps in floating point with nothing
//! truncated, so it would write `0.8` where the reference plots write
//! `0.797`. The rule above is that routine with the division by 256 done on
//! whole numbers and each entry cut to a whole number of 256ths.
//!
//! It was found by search, not read out of the binary that wrote the plots.
//! What holds it is the witnesses below and how narrowly they pin it. Of
//! every whole constant from 1000 to 20 000 000 put in the place of
//! 1 250 000, only 345 reproduce all fourteen: 1 249 941 to 1 250 225 and
//! 1 250 714 to 1 250 773. The whole-number division and the truncation of
//! the entries are both needed: rounding either one, or dividing exactly,
//! misses at least three of the fourteen. `tools/driver_text_matrix.py`
//! runs the search.
//!
//! # The witnesses
//!
//! Fourteen matrices of the eleven reference plots, each a face on a cell of
//! so many device units (1/150 in), with the average width it states where
//! it states one. All fourteen are reproduced; the table is this module's
//! test.
//!
//! | face | cell | stated width | written along | written across |
//! |---|---:|---:|---:|---:|
//! | SimSun | 24 | 10 | 0.832 | 1 |
//! | SimSun | 26 | 10 | 0.77 | 1 |
//! | SimSun | 28 | 11 | 0.785 | 1 |
//! | SimSun | 30 | 12 | 0.797 | 1 |
//! | SimSun | 31 | 12 | 0.773 | 1 |
//! | SimSun | 35 | 13 | 0.742 | 1 |
//! | SimSun, turned | 37 | 14 | 0.754 | 1 |
//! | SimSun | 39 | 15 | 0.77 | 1 |
//! | SimSun | 67 | 26 | 0.773 | 1 |
//! | SimSun, turned | 31 | none | 1 | 1 |
//! | Arial | 19 | 6 | 0.809 | 1.012 |
//! | Arial, turned | 29 | none | 1.012 | 1.012 |
//! | Arial Narrow, turned | 24 | none | 1.008 | 1.008 |
//! | Arial Narrow, turned | 37 | none | 1.012 | 1.012 |
//!
//! The 26-unit SimSun and the 19-unit Arial are written both upright and
//! turned, with the same two numbers. Arial Narrow on 37 units is the
//! tightest of them: its entry is 259.012 before it is cut to 259, and it is
//! that only because the face's cell is 2319 design units (`usWinAscent +
//! usWinDescent`); 2320 would give 258.
//!
//! # The letter-spacing
//!
//! The driver states a letter-spacing in text space by dividing the spacing
//! it wants on the device by the entry along the baseline in 256ths — `n /
//! 256`, not the three decimals `Tm` shows: [`TextMatrix::spacing`]. The
//! reference's `Tc` is that quotient cut to three decimals; the cut is not
//! made here.
//!
//! # What the witnesses do not show
//!
//! - a face on another design grid than 256 or 2048 units per em;
//! - a label lettered at an angle that is no quarter turn. The driver takes
//!   the length of the vector across from its two components and cuts each
//!   of the four entries on its own; here such a label takes the two scales
//!   above times the sine and cosine of its angle. No label of any corpus is
//!   lettered so: an angle within a twentieth of a degree of a quarter turn
//!   is lettered on it ([`super::escapement`]), and the farthest any label
//!   lies from one is 0.012°;
//! - a turned or stretched label of more than one font. Each font of a label
//!   is written under its own matrix (`SetFont` in the writer), which is
//!   this rule taken to a font the driver is known to be handed; the
//!   borrowed glyphs the reference plots show are level and state no width,
//!   so every font of theirs is written plain;
//! - an entry that is a whole number of 256ths before it is cut, which the
//!   driver's single-precision arithmetic decides;
//! - `n / 256` halfway between two third decimals (`n` = 16, 48, 80, …),
//!   written here rounded up.
//!
//! See `docs/analysis/2026-11-11-the-driver-states-a-font-transform-in-256ths.md`.

/// The constant the driver multiplies a font transform by before it takes
/// whole numbers of it.
const DRIVER_UNITS: f64 = 1_250_000.0;

/// The scales a label's text matrix states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TextMatrix {
    /// Along the baseline, as `Tm` states it.
    pub(super) along: f64,
    /// Across the baseline, as `Tm` states it.
    pub(super) across: f64,
    /// Along the baseline in the 256ths the driver keeps: what a spacing on
    /// the device is divided by to be stated in text space.
    pub(super) spacing: f64,
}

/// The matrix the plotter's own arithmetic gives a font stretched by
/// `stretch`: the stretch along the baseline and nothing across it.
fn as_asked(stretch: f64) -> TextMatrix {
    TextMatrix {
        along: stretch,
        across: 1.0,
        spacing: stretch,
    }
}

/// The scales the driver writes for a font of `px_per_unit` device units per
/// design unit across its baseline, stretched by `stretch` along it.
///
/// `upright` is whether the baseline runs along the page's own; a font that
/// is upright and not stretched is written plain. `px_per_unit` is `None`
/// where the label has no cell to size a font from, and such a label, like
/// one whose font is too small for the driver's division to mean anything,
/// keeps the stretch as it was asked for.
pub(super) fn written(px_per_unit: Option<f64>, stretch: f64, upright: bool) -> TextMatrix {
    // The driver compares the transform's own two numbers; a stretch of
    // exactly one is the plotter stating no width at all.
    #[allow(clippy::float_cmp)]
    let plain = upright && stretch == 1.0;
    let across = px_per_unit.filter(|across| across.is_finite() && *across > 0.0);
    let (Some(across), false, true) = (across, plain, stretch.is_finite() && stretch > 0.0) else {
        return as_asked(stretch);
    };
    let length = (across * DRIVER_UNITS).trunc();
    let quotient = (length / 256.0).trunc();
    if quotient < 1.0 {
        return as_asked(stretch);
    }
    let entry = |element: f64| (element * DRIVER_UNITS / quotient).trunc() / 256.0;
    let spacing = entry(across * stretch);
    if spacing <= 0.0 {
        return as_asked(stretch);
    }
    TextMatrix {
        along: three_decimals(spacing),
        across: three_decimals(entry(across)),
        spacing,
    }
}

/// `value` as the driver prints it: to the nearest thousandth.
fn three_decimals(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::{TextMatrix, written};

    /// A face as the rule needs it: its cell (`usWinAscent + usWinDescent`)
    /// and its average character width (`xAvgCharWidth`), in design units.
    #[derive(Clone, Copy)]
    struct Face {
        cell_units: f64,
        average_units: f64,
    }

    const SIMSUN: Face = Face {
        cell_units: 256.0,
        average_units: 128.0,
    };
    const ARIAL: Face = Face {
        cell_units: 2288.0,
        average_units: 904.0,
    };
    const ARIAL_NARROW: Face = Face {
        cell_units: 2319.0,
        average_units: 741.0,
    };

    /// The transform GDI states for `face` on a cell of `cell_px` device
    /// units with a stated average width of `width_px`: device units per
    /// design unit across the baseline, and the stretch along it.
    fn transform(face: Face, cell_px: f64, width_px: Option<f64>) -> (f64, f64) {
        let across = cell_px / face.cell_units;
        let along = width_px.map_or(across, |width| width / face.average_units);
        (across, along / across)
    }

    /// Every text matrix of the eleven reference plots that is not the plain
    /// one: the face, its cell and stated width in device units, whether the
    /// label is upright, and the two scales the reference writes.
    #[test]
    fn the_fourteen_matrices_of_the_reference_plots_are_reproduced() {
        let witnessed = [
            ("SimSun", SIMSUN, 24.0, Some(10.0), true, 0.832, 1.0),
            ("SimSun", SIMSUN, 26.0, Some(10.0), true, 0.77, 1.0),
            ("SimSun", SIMSUN, 28.0, Some(11.0), true, 0.785, 1.0),
            ("SimSun", SIMSUN, 30.0, Some(12.0), true, 0.797, 1.0),
            ("SimSun", SIMSUN, 31.0, Some(12.0), true, 0.773, 1.0),
            ("SimSun", SIMSUN, 35.0, Some(13.0), true, 0.742, 1.0),
            ("SimSun", SIMSUN, 37.0, Some(14.0), false, 0.754, 1.0),
            ("SimSun", SIMSUN, 39.0, Some(15.0), true, 0.77, 1.0),
            ("SimSun", SIMSUN, 67.0, Some(26.0), true, 0.773, 1.0),
            ("SimSun", SIMSUN, 31.0, None, false, 1.0, 1.0),
            ("Arial", ARIAL, 19.0, Some(6.0), true, 0.809, 1.012),
            ("Arial", ARIAL, 29.0, None, false, 1.012, 1.012),
            (
                "Arial Narrow",
                ARIAL_NARROW,
                24.0,
                None,
                false,
                1.008,
                1.008,
            ),
            (
                "Arial Narrow",
                ARIAL_NARROW,
                37.0,
                None,
                false,
                1.012,
                1.012,
            ),
        ];
        let mut missed = Vec::new();
        for (name, face, cell_px, width_px, upright, along, across) in witnessed {
            let (px_per_unit, stretch) = transform(face, cell_px, width_px);
            let got = written(Some(px_per_unit), stretch, upright);
            if (got.along - along).abs() > 1e-12 || (got.across - across).abs() > 1e-12 {
                missed.push(format!(
                    "{name} on {cell_px} units, width {width_px:?}: written {} / {}, the reference \
                     writes {along} / {across}",
                    got.along, got.across
                ));
            }
        }
        assert!(
            missed.is_empty(),
            "{} of 14 witnessed matrices are not reproduced:\n{}",
            missed.len(),
            missed.join("\n")
        );
    }

    /// The same two fonts the reference writes both ways read the same turned
    /// as upright: the 26-unit SimSun at width 10 and the 19-unit Arial at
    /// width 6.
    #[test]
    fn a_stretched_font_is_written_the_same_turned_as_upright() {
        for (face, cell_px, width_px) in [(SIMSUN, 26.0, 10.0), (ARIAL, 19.0, 6.0)] {
            let (px_per_unit, stretch) = transform(face, cell_px, Some(width_px));
            assert_eq!(
                written(Some(px_per_unit), stretch, true),
                written(Some(px_per_unit), stretch, false),
            );
        }
    }

    /// An upright font that is not stretched is written `1 0 0 1`, whatever
    /// its size: the 29-unit Arial that is `1.012` both ways when turned.
    #[test]
    fn an_upright_font_that_is_not_stretched_is_written_plain() {
        let (px_per_unit, stretch) = transform(ARIAL, 29.0, None);
        assert_eq!(
            written(Some(px_per_unit), stretch, true),
            TextMatrix {
                along: 1.0,
                across: 1.0,
                spacing: 1.0
            }
        );
        let turned = written(Some(px_per_unit), stretch, false);
        assert!(
            (turned.along - 1.012).abs() < 1e-12 && (turned.across - 1.012).abs() < 1e-12,
            "turned, the same font is written {} / {}",
            turned.along,
            turned.across
        );
    }

    /// A letter-spacing is divided by the entry in 256ths, not by the three
    /// decimals `Tm` shows: 197/256 for the 26-unit SimSun at width 10, whose
    /// `Tm` reads `0.77`, and 204/256 for the 30-unit one, whose reads
    /// `0.797`.
    #[test]
    fn the_spacing_is_the_entry_in_256ths() {
        for (cell_px, width_px, entry) in [(26.0, 10.0, 197.0), (30.0, 12.0, 204.0)] {
            let (px_per_unit, stretch) = transform(SIMSUN, cell_px, Some(width_px));
            let got = written(Some(px_per_unit), stretch, true);
            assert!(
                (got.spacing - entry / 256.0).abs() < 1e-12,
                "{cell_px} units at width {width_px}: {} against {entry}/256",
                got.spacing
            );
        }
    }

    /// A label with no cell to size a font from, a font too small for the
    /// driver's division, and a stretch that is no number all keep what the
    /// plotter asked for.
    #[test]
    fn a_font_the_rule_cannot_reach_keeps_the_stretch_it_was_asked_for() {
        let asked = TextMatrix {
            along: 0.8,
            across: 1.0,
            spacing: 0.8,
        };
        assert_eq!(written(None, 0.8, true), asked);
        assert_eq!(written(Some(0.0), 0.8, true), asked);
        assert_eq!(written(Some(f64::NAN), 0.8, true), asked);
        // One design unit is 1/10000 of a device unit: 125 before the
        // division by 256, nothing after it.
        assert_eq!(written(Some(1e-4), 0.8, true), asked);
        let odd = written(Some(0.1), f64::NAN, false);
        assert!(odd.along.is_nan() && odd.across == 1.0);
    }
}
