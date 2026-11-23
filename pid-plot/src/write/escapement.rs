//! The angle a label is lettered at, read the way the plotter states it.
//!
//! A drawing keeps a label's angle as a double, and the angles a person would
//! call level or upright are rarely that to the last bit. The plant backup
//! holds ten labels between 9e-8 and 2e-4 rad off level and two a couple of
//! millionths of a radian off a quarter turn; a fixture holds one more; and
//! of the 470 turned labels on the eleven sheets that have a reference plot,
//! 45 are short of their quarter turn as this crate reads them — seven by a
//! bit or two of the double, the rest by up to 9e-13 rad.
//!
//! The plotter does not letter at the angle a hair off level. `render.dll` hands GDI a
//! font whose `LOGFONT.lfEscapement` counts whole tenths of a degree. Two
//! routines of `RADInstallA~\render.dll` make that number from `atan2` of the
//! label's matrix, times `-180/π`, times ten. `sub_1000B1F0` rounds to the
//! nearest tenth, and a matrix whose sine is under 2^-23 is written `0`
//! whatever the cosine, so a half turn is level. `sub_10080B50` cuts the same
//! product toward zero (`cvttsd2si`); the same cutoff returns `1800` when the
//! cosine is negative and `0` otherwise, so a half turn is upside down. The
//! text path calls `sub_100808F0` (`0x1007F107`, `0x1008040E`), which passes
//! that cut integer to `sub_1007EBA0`. The builder writes it over
//! `lfEscapement` and `lfOrientation` after the virtual call that fills the
//! `LOGFONT`, and `CreateFontIndirectA` (`0x1001B57D`, the only call) is
//! handed that `LOGFONT`. The rounding routine's thunk is not taken up by
//! any call. The tenth that reaches GDI is the one cut toward zero.
//!
//! # What the reference plots show
//!
//! - Every one of their 470 turned labels is stated exactly on its quarter
//!   turn, in whole numbers (`0 1.012 -1.012 0`). Of ours, the 425 that are
//!   not short of it are too; the 45 that are short are kept at the drawing's
//!   angle, which those plots do not separate from the quarter (below).
//! - A label lettered level looks as the ten of the backup do under this
//!   rule: an upright Arial at `Tf 12.45` or Arial Narrow at `Tf 10.17` under
//!   `1 0 0 1`, not the same face 1.2 % or 0.8 % larger under the matrix of a
//!   turned label.
//!
//! # What they do not show
//!
//! - How far from a quarter turn a label may lie and still be lettered on
//!   it. No label of theirs is more than 1.2e-10° from one. Read by the cut,
//!   a label short of its quarter turn is not on it, so those 45 are kept at
//!   the angle the drawing states. At 89.9° the longest of them, 23 device
//!   units, would stand 0.04 of a unit aside, on the same whole unit, and the
//!   driver's 256ths ([`super::text_matrix`]) would still write
//!   `0 1.012 -1.012 0`. The plots cannot tell the cut from the nearest tenth.
//!
//! On the 71 pages on hand the two readings letter one label differently:
//! the plant backup's `25-WO-0103528-B2-EH40`, 1.06e-4° short of a quarter
//! turn and 296 device units long, whose last glyphs the cut stands one
//! unit aside. There is no factory plot of that sheet. The other 223 labels
//! the two part on are 284 units long or less, and half a unit aside takes
//! 286.5. Two fixture labels sit at exactly half a turn: the cut reads 1800
//! and the rounding routine reads 0, and there is no factory plot of them
//! either.
//!
//! [`of`] reads an angle that way for one purpose, telling a label on a
//! quarter turn from a label turned to some other angle. The tenth is cut
//! toward zero. A label whose cut tenth is a multiple of 900 is lettered on
//! that quarter turn: one that has passed it by less than a tenth of a
//! degree, and one either side of level by less than a tenth. A label short
//! of a quarter turn keeps the angle the drawing states, unless its sine is
//! under 2^-23, where the routine returns 0 or 1800 from the cosine. Any
//! other angle is kept as the drawing states it; it is not cut to its tenth
//! of a degree, which no reference plot has a label to show.

use std::f64::consts::{FRAC_PI_2, PI};

/// The tenths of a degree in a quarter turn.
const QUARTER_TURN_TENTHS: f64 = 900.0;

/// The direction a label's baseline is lettered along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lettered {
    /// The angle of the baseline, radians counter-clockwise from the page's
    /// x axis: a whole number of quarter turns when the tenth of a degree
    /// cut toward zero is a multiple of 900, the drawing's own angle
    /// otherwise.
    pub(super) rotation: f64,
    /// Sine of [`Self::rotation`], exact on a quarter turn.
    pub(super) sin: f64,
    /// Cosine of [`Self::rotation`], exact on a quarter turn.
    pub(super) cos: f64,
    /// Whether the baseline runs along the page's x axis, either way: what
    /// the writer sets as a string on whole-pixel starts rather than glyph
    /// by glyph.
    pub(super) upright: bool,
}

/// The direction a label at `rotation` is lettered along.
///
/// The tenth of a degree is cut toward zero, as `render.dll` `sub_10080B50`
/// does for the font `CreateFontIndirectA` is handed. A sine under 2^-23 is
/// `0` or `1800` from the sign of the cosine, which is that routine's cutoff.
pub(super) fn of(rotation: f64) -> Lettered {
    let (sin, cos) = rotation.sin_cos();
    // `2^-23`, the cutoff `sub_10080B50` compares the matrix sine with.
    let tenths = if sin.abs() < 1.0 / 8388608.0 {
        if cos < 0.0 { 1800.0 } else { 0.0 }
    } else {
        (rotation.to_degrees() * 10.0).trunc()
    };
    if !tenths.is_finite() || tenths.rem_euclid(QUARTER_TURN_TENTHS) != 0.0 {
        return Lettered {
            rotation,
            sin,
            cos,
            upright: false,
        };
    }
    // The quarter turn in 0..4. Its sine and cosine are whole numbers, which
    // `sin_cos` of the nearest double to a multiple of π/2 does not return.
    let quarter = (tenths / QUARTER_TURN_TENTHS).rem_euclid(4.0);
    let (rotation, sin, cos) = if quarter < 0.5 {
        (0.0, 0.0, 1.0)
    } else if quarter < 1.5 {
        (FRAC_PI_2, 1.0, 0.0)
    } else if quarter < 2.5 {
        (PI, 0.0, -1.0)
    } else {
        (3.0 * FRAC_PI_2, -1.0, 0.0)
    };
    Lettered {
        rotation,
        sin,
        cos,
        upright: sin == 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{Lettered, of};
    use std::f64::consts::{FRAC_PI_2, PI};

    /// The angles the corpora hold a hair off level are level: sine nought
    /// and cosine one, exactly.
    #[test]
    fn an_angle_a_hair_off_level_is_level() {
        for rotation in [0.0, 9.0e-8, -2.59e-6, 2.0e-4, -2.016e-4, 2.22e-16] {
            assert_eq!(
                of(rotation),
                Lettered {
                    rotation: 0.0,
                    sin: 0.0,
                    cos: 1.0,
                    upright: true,
                },
                "{rotation} rad"
            );
        }
    }

    /// An angle that has passed a quarter turn, and is still inside the tenth
    /// the cut keeps, is that quarter turn. A shortfall is not: rounding
    /// would put it on the quarter.
    #[test]
    fn an_angle_past_a_quarter_turn_is_the_quarter_turn() {
        let (_, cos) = FRAC_PI_2.sin_cos();
        assert!(cos != 0.0, "the cosine of the double nearest π/2: {cos}");
        // Past +π/2 and +3π/2, away from zero.
        for off in [0.0, 2.17e-6, 4.14e-13] {
            let up = of(FRAC_PI_2 + off);
            assert_eq!((up.sin, up.cos, up.upright), (1.0, 0.0, false), "{off}");
            assert_eq!(up.rotation.to_bits(), FRAC_PI_2.to_bits(), "{off}");
            let down = of(3.0 * FRAC_PI_2 + off);
            assert_eq!(
                (down.sin, down.cos, down.upright),
                (-1.0, 0.0, false),
                "{off}"
            );
            assert_eq!(
                down.rotation.to_bits(),
                (3.0 * FRAC_PI_2).to_bits(),
                "{off}"
            );
        }
        // Short of +π/2. The same shortfall past −π/2, away from zero, is
        // still the downward quarter.
        for off in [-1.84e-6, -8.84e-13] {
            let rotation = FRAC_PI_2 + off;
            let kept = of(rotation);
            assert_eq!(kept.rotation.to_bits(), rotation.to_bits(), "{off}");
            let down = of(-FRAC_PI_2 + off);
            assert_eq!(
                (down.sin, down.cos, down.upright),
                (-1.0, 0.0, false),
                "{off}"
            );
            assert_eq!(
                down.rotation.to_bits(),
                (3.0 * FRAC_PI_2).to_bits(),
                "{off}"
            );
        }
        // Exactly on −π/2, and a step back toward zero that the cut drops.
        let down = of(-FRAC_PI_2);
        assert_eq!((down.sin, down.cos), (-1.0, 0.0));
        assert_eq!(down.rotation.to_bits(), (3.0 * FRAC_PI_2).to_bits());
        let toward = -FRAC_PI_2 + 2.17e-6;
        assert_eq!(of(toward).rotation.to_bits(), toward.to_bits());
        // Half a turn runs along the x axis, the other way. `5π − 1e-7` is
        // inside the sine cutoff, so the routine returns 1800; the cut of
        // the product is 8999.
        for rotation in [PI, -PI, PI + 3.0e-6, 3.0 * PI, 5.0 * PI - 1.0e-7] {
            let over = of(rotation);
            assert_eq!(
                (over.sin, over.cos, over.upright),
                (0.0, -1.0, true),
                "{rotation}"
            );
            assert_eq!(over.rotation.to_bits(), PI.to_bits(), "{rotation}");
        }
        // Just outside the cutoff, short of a half turn. Rounding calls this
        // a half turn.
        let shy = PI - 2.0e-7;
        assert_eq!(of(shy).rotation.to_bits(), shy.to_bits());
        // More than a turn is the same direction.
        assert_eq!(of(2.0 * PI + 1.0e-9), of(0.0));
        assert_eq!(of(5.0 * FRAC_PI_2), of(FRAC_PI_2));
    }

    /// The tenth of a degree is cut toward zero, not taken to the nearest.
    /// `0.06°` is level, `90.06°` is a quarter turn, and `89.96°` — and a
    /// hair short of one — stays short. Each of those is the other way when
    /// the tenth is the nearest one.
    #[test]
    fn the_tenth_of_a_degree_is_cut_toward_zero() {
        for degrees in [0.04_f64, -0.04, 0.06, -0.06, 0.099, -0.099] {
            assert!(of(degrees.to_radians()).upright, "{degrees}°");
        }
        for degrees in [90.0_f64, 90.04, 90.06, 90.099] {
            let turned = of(degrees.to_radians());
            assert_eq!(
                (turned.sin, turned.cos, turned.upright),
                (1.0, 0.0, false),
                "{degrees}°"
            );
        }
        for degrees in [180.0_f64, 180.06] {
            let turned = of(degrees.to_radians());
            assert_eq!(
                (turned.sin, turned.cos, turned.upright),
                (0.0, -1.0, true),
                "{degrees}°"
            );
        }
        let down = of((-90.06_f64).to_radians());
        assert_eq!((down.sin, down.cos, down.upright), (-1.0, 0.0, false));
        for degrees in [0.1_f64, -0.1, 89.96, 90.1, 179.96, -89.96] {
            let rotation = degrees.to_radians();
            let kept = of(rotation);
            assert!(!kept.upright, "{degrees}°");
            assert_eq!(kept.rotation.to_bits(), rotation.to_bits(), "{degrees}°");
        }
        let short = FRAC_PI_2 - 1.0e-13;
        assert_eq!(of(short).rotation.to_bits(), short.to_bits());
    }

    /// An angle that is no quarter turn keeps its value, sine and cosine: it
    /// is not rounded to its tenth of a degree.
    #[test]
    fn an_angle_that_is_no_quarter_turn_keeps_its_value() {
        for degrees in [37.26_f64, -12.5, 45.0, 135.049, 301.0, 1234.5] {
            let rotation = degrees.to_radians();
            let (sin, cos) = rotation.sin_cos();
            assert_eq!(
                of(rotation),
                Lettered {
                    rotation,
                    sin,
                    cos,
                    upright: false,
                },
                "{degrees}°"
            );
        }
    }

    /// An angle that is no number is handed back as it came, and one too
    /// large to have a tenth of a degree still reads as a direction.
    #[test]
    fn an_angle_that_is_no_number_is_not_a_quarter_turn() {
        for rotation in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let read = of(rotation);
            assert!(!read.upright, "{rotation}");
            assert_eq!(read.rotation.to_bits(), rotation.to_bits(), "{rotation}");
        }
        for rotation in [f64::MAX, f64::MIN, 1.0e300, -1.0e300] {
            let read = of(rotation);
            assert!(
                (read.sin.hypot(read.cos) - 1.0).abs() < 1e-9,
                "{rotation}: {read:?}"
            );
        }
    }
}
